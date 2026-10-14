//! `MeridianConflict(.exe)`: what a player starts (docs/RELEASES.md).
//!
//! An install is this program and the builds it runs:
//!
//! ```text
//! MeridianConflict.exe      this
//! versions/<build>/         each build, whole (mc_builds::versions)
//! replays/, ...             the game's working folder: the install's root
//! launcher.log              what the last start did
//! ```
//!
//! - `MeridianConflict [ARGS...]` starts the current build (`versions/current`,
//!   or the newest of this channel) with ARGS, in the install's root, and exits.
//!   With nothing installed it installs its channel's newest build first.
//! - `MeridianConflict --replay FILE` plays an old replay in the build that
//!   recorded it, installing that build first if need be, then starts the
//!   current build again on Match History.
//!
//! The game finds this program through `MERIDIAN_LAUNCHER`, downloads
//! updates and old builds itself (with a progress bar), and asks this program
//! to run them. An update to this program arrives as `MeridianConflict.exe.new`
//! beside it and is swapped in on the next start.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::AtomicBool;

use mc_builds::versions::Progress;
use mc_builds::{Store, Versions};
use mc_core::Channel;

/// Installed builds kept, by last use; identical files are hard links, so an
/// old build mostly costs the files that changed.
const KEEP: usize = 8;

fn main() -> ExitCode {
    let Ok(exe) = std::env::current_exe() else {
        return ExitCode::FAILURE;
    };
    let root = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut log = Log::open(&root.join("launcher.log"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if swap_in_update(&exe, &args, &mut log) {
        return ExitCode::SUCCESS;
    }
    let result = match args.as_slice() {
        [flag, file] if flag == "--replay" => replay(&exe, &root, Path::new(file), &mut log),
        _ => start(&exe, &root, &args, &mut log),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log.line(&format!("failed: {e}"));
            tell(&format!(
                "Meridian Conflict could not start.\n\n{e}\n\nSee {}",
                root.join("launcher.log").display()
            ));
            ExitCode::FAILURE
        }
    }
}

fn channel() -> Channel {
    option_env!("MERIDIAN_CHANNEL")
        .and_then(|c| c.parse().ok())
        .unwrap_or(Channel::Dev)
}

/// Starts the current build with `args` and leaves it running.
fn start(exe: &Path, root: &Path, args: &[String], log: &mut Log) -> Result<(), String> {
    let versions = Versions::new(root);
    let current = match versions.current(channel()) {
        Some(m) => m,
        None => {
            log.line("nothing installed: fetching this channel's newest build");
            let store = Store::published().map_err(|e| e.to_string())?;
            let newest = store.newest(channel()).map_err(|e| e.to_string())?;
            install(&versions, &store, &newest, log)?;
            versions.set_current(&newest).map_err(|e| e.to_string())?;
            newest
        }
    };
    let key = current.key();
    versions.touch(&key);
    versions.prune(KEEP, std::slice::from_ref(&key));
    log.line(&format!("starting {}", current.build));
    game(exe, root, &versions.exe(&key), args)
        .spawn()
        .map_err(|e| format!("{}: {e}", versions.exe(&key).display()))?;
    Ok(())
}

/// Plays `file` in the build that recorded it, then starts the current build.
fn replay(exe: &Path, root: &Path, file: &Path, log: &mut Log) -> Result<(), String> {
    let origin = mc_net::Origin::peek_file(file)
        .map_err(|e| format!("{}: {e}", file.display()))?
        .origin;
    let versions = Versions::new(root);
    if !versions.has(&origin.build) {
        let store = Store::published().map_err(|e| e.to_string())?;
        let m = store.manifest(&origin.build).map_err(|e| e.to_string())?;
        install(&versions, &store, &m, log)?;
    }
    let key = mc_builds::key_of(&origin.build);
    versions.touch(&key);
    log.line(&format!("playing {} in {}", file.display(), origin.build));
    let args = [
        "--replay".to_owned(),
        file.to_string_lossy().into_owned(),
        "--replay-only".to_owned(),
    ];
    let status = game(exe, root, &versions.exe(&key), &args)
        .status()
        .map_err(|e| format!("{}: {e}", versions.exe(&key).display()))?;
    log.line(&format!("{} closed ({status})", origin.build));
    start(exe, root, &["--open".to_owned(), "history".to_owned()], log)
}

fn install(
    versions: &Versions,
    store: &Store,
    m: &mc_builds::Manifest,
    log: &mut Log,
) -> Result<(), String> {
    log.line(&format!(
        "installing {} ({:.1} MB to download)",
        m.build,
        versions.download_size(m) as f64 / 1e6
    ));
    let mut tenth = 0;
    let mut progress = |p: Progress| {
        let t = (p.done * 10).checked_div(p.total).unwrap_or(10);
        if t > tenth {
            tenth = t;
            log.line(&format!("  {}%", t * 10));
        }
    };
    versions
        .install(store, m, &mut progress, &AtomicBool::new(false))
        .map_err(|e| format!("installing {}: {e}", m.build))?;
    Ok(())
}

/// A build's game, run from the install's root (where `replays/` lives) and
/// told where this program is.
fn game(exe: &Path, root: &Path, game: &Path, args: &[String]) -> Command {
    let mut c = Command::new(game);
    c.args(args).current_dir(root).env("MERIDIAN_LAUNCHER", exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the game is a console program and this one has no
        // console to lend it, so Windows would open one beside the game. Its
        // log goes to meridian.log anyway.
        c.creation_flags(0x0800_0000);
    }
    c
}

/// `MeridianConflict.exe.new` beside this program is its update: swap it in
/// and start it in this one's place. True when it did.
fn swap_in_update(exe: &Path, args: &[String], log: &mut Log) -> bool {
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

/// Says something to a player who has no window to read it in.
fn tell(message: &str) {
    if cfg!(windows) {
        let quoted = message.replace('\'', "''");
        let _ = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Add-Type -AssemblyName PresentationFramework; [System.Windows.MessageBox]::Show('{quoted}', 'Meridian Conflict') | Out-Null"
                ),
            ])
            .status();
    } else {
        eprintln!("{message}");
    }
}

struct Log(Option<File>);

impl Log {
    fn open(path: &Path) -> Log {
        Log(File::create(path).ok())
    }

    fn line(&mut self, text: &str) {
        if let Some(f) = &mut self.0 {
            let _ = writeln!(f, "{text}");
        }
    }
}
