//! The install this launcher sits in, and what it does to it outside the
//! window: the channel the player picked, starting a build, swapping in its
//! own update, and opening folders and links.
//!
//! ```text
//! MeridianConflict.exe      this program
//! launcher.json             the player's choices (the channel)
//! launcher.log              what the last run did
//! versions/                 the builds (mc_builds::versions)
//! replays/, ...             the game's working folder: the install's root
//! ```

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use mc_builds::{Manifest, Versions};
use mc_core::Channel;
use serde::{Deserialize, Serialize};

/// Installed builds kept, by last use; identical files are hard links, so an
/// old build mostly costs the files that changed.
pub(crate) const KEEP: usize = 8;

/// The channel this launcher was published on (`MERIDIAN_CHANNEL`): what it
/// starts on until the player picks another.
pub(crate) fn home_channel() -> Channel {
    option_env!("MERIDIAN_CHANNEL")
        .and_then(|c| c.parse().ok())
        .unwrap_or(Channel::Dev)
}

/// The channels a player can pick between. A dev launcher (no store) still
/// lists them, so the window looks as it will.
pub(crate) const CHANNELS: [Channel; 2] = [Channel::Release, Channel::Playtest];

/// The multiplayer server a channel's builds play on, set when the launcher
/// is built (scripts/release.sh, as for the game).
pub(crate) fn server(channel: Channel) -> Option<&'static str> {
    match channel {
        Channel::Release => option_env!("MERIDIAN_SERVER_RELEASE"),
        Channel::Playtest => option_env!("MERIDIAN_SERVER_PLAYTEST"),
        Channel::Dev => None,
    }
    .filter(|s| !s.is_empty())
}

#[derive(Serialize, Deserialize, Default)]
struct Prefs {
    #[serde(default)]
    channel: Option<String>,
}

pub(crate) struct Install {
    /// This program.
    pub(crate) exe: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) versions: Versions,
}

impl Install {
    pub(crate) fn new(exe: PathBuf, root: PathBuf) -> Install {
        let versions = Versions::new(&root);
        Install {
            exe,
            root,
            versions,
        }
    }

    /// The channel the player last picked, else this launcher's own.
    pub(crate) fn channel(&self) -> Channel {
        std::fs::read(self.root.join("launcher.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Prefs>(&b).ok())
            .and_then(|p| p.channel?.parse().ok())
            .filter(|c| CHANNELS.contains(c))
            .unwrap_or_else(|| match home_channel() {
                Channel::Dev => Channel::Release,
                c => c,
            })
    }

    pub(crate) fn set_channel(&self, channel: Channel) {
        let prefs = Prefs {
            channel: Some(channel.name().to_owned()),
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&prefs) {
            let _ = std::fs::write(self.root.join("launcher.json"), bytes);
        }
    }

    /// A build's game, run from the install's root (where `replays/` lives)
    /// and told where this program is.
    pub(crate) fn game(&self, m: &Manifest, args: &[String]) -> Command {
        let key = m.key();
        self.versions.touch(&key);
        let mut c = Command::new(self.versions.exe(&key));
        c.args(args)
            .current_dir(&self.root)
            .env("MERIDIAN_LAUNCHER", &self.exe);
        no_console(&mut c);
        c
    }

    /// Starts `m` and leaves it running.
    pub(crate) fn play(&self, m: &Manifest, args: &[String], log: &mut Log) -> Result<(), String> {
        let key = m.key();
        let spare: Vec<String> = CHANNELS
            .iter()
            .filter_map(|c| self.versions.current(*c))
            .map(|c| c.key())
            .chain([key.clone()])
            .collect();
        self.versions.prune(KEEP, &spare);
        log.line(&format!("starting {}", m.build));
        self.game(m, args)
            .spawn()
            .map(drop)
            .map_err(|e| format!("{}: {e}", self.versions.exe(&key).display()))
    }

    /// A build carries the launcher it was published with
    /// (`launcher/MeridianConflict.exe`). When it differs from this one it is
    /// put beside it as `.new`; true when it was.
    pub(crate) fn stage_launcher(&self, m: &Manifest) -> std::io::Result<bool> {
        let Some(name) = self.exe.file_name() else {
            return Ok(false);
        };
        let theirs = self.versions.dir(&m.key()).join("launcher").join(name);
        let Ok(new) = std::fs::read(&theirs) else {
            return Ok(false);
        };
        if std::fs::read(&self.exe).is_ok_and(|ours| ours == new) {
            return Ok(false);
        }
        std::fs::write(with_suffix(&self.exe, ".new"), new)?;
        Ok(true)
    }

    /// Where the game keeps its settings, log and crash reports (mc-game
    /// `settings::dir`).
    pub(crate) fn game_files() -> Option<PathBuf> {
        let base = if cfg!(windows) {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        };
        Some(base?.join("meridian-conflict"))
    }
}

/// `MeridianConflict.exe.new` beside this program is its update: swap it in
/// and start it in this one's place. True when it did.
pub(crate) fn swap_in_update(exe: &Path, args: &[String], log: &mut Log) -> bool {
    let old = with_suffix(exe, ".old");
    let _ = std::fs::remove_file(&old);
    let new = with_suffix(exe, ".new");
    if !new.is_file() {
        return false;
    }
    // A running program can be renamed, though not overwritten.
    let swapped = std::fs::rename(exe, &old).and_then(|()| std::fs::rename(&new, exe));
    match swapped {
        Ok(()) => {
            log.line("updated the launcher");
            Command::new(exe).args(args).spawn().is_ok()
        }
        Err(e) => {
            log.line(&format!("could not update the launcher: {e}"));
            if !exe.exists() {
                let _ = std::fs::rename(&old, exe);
            }
            false
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Opens a folder in the file manager, or an `https://` link (from the
/// signed news) in the browser.
pub(crate) fn open(target: &str) {
    let mut c = if cfg!(windows) {
        let mut c = Command::new("explorer.exe");
        c.arg(target);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(target);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(target);
        c
    };
    let _ = c.spawn();
}

/// The game is a console program and the launcher has no console to lend it,
/// so Windows would open one beside the game (CREATE_NO_WINDOW). Its log goes
/// to meridian.log anyway.
pub(crate) fn no_console(c: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = c;
}

/// Says something to a player when there is no window to say it in.
pub(crate) fn tell(message: &str) {
    if cfg!(windows) {
        let quoted = message.replace('\'', "''");
        let mut c = Command::new("powershell.exe");
        c.args([
            "-NoProfile",
            "-Command",
            &format!(
                "Add-Type -AssemblyName PresentationFramework; [System.Windows.MessageBox]::Show('{quoted}', 'Meridian Conflict') | Out-Null"
            ),
        ]);
        no_console(&mut c);
        let _ = c.status();
    } else {
        eprintln!("{message}");
    }
}

/// Asks a yes-or-no question in a system dialog; true for yes.
pub(crate) fn ask(question: &str) -> bool {
    if !cfg!(windows) {
        eprintln!("{question} (answering no: there is no dialog here)");
        return false;
    }
    let mut c = Command::new("powershell.exe");
    c.args([
        "-NoProfile",
        "-Command",
        "Add-Type -AssemblyName PresentationFramework; [System.Windows.MessageBox]::Show($env:MC_QUESTION, 'Meridian Conflict', 'YesNo', 'Question')",
    ])
    .env("MC_QUESTION", question);
    no_console(&mut c);
    c.output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "Yes")
}

/// `launcher.log`: what this run did, for a report.
pub(crate) struct Log(Option<File>);

impl Log {
    pub(crate) fn open(path: &Path) -> Log {
        Log(File::create(path).ok())
    }

    pub(crate) fn line(&mut self, text: &str) {
        if let Some(f) = &mut self.0 {
            let _ = writeln!(f, "{text}");
        }
    }
}
