//! The launcher as its own installer and uninstaller (Windows).
//!
//! The website serves the launcher as `MeridianConflictSetup-<Channel>.exe`. Run
//! under a name with `Setup` in it, it installs itself, for this user and with no admin
//! prompt, to `%LOCALAPPDATA%\Programs\Meridian Conflict`:
//!
//! - copies itself there as `MeridianConflict.exe`;
//! - puts "Meridian Conflict" shortcuts in the Start menu and on the desktop;
//! - lists the game in Settings > Apps (uninstall runs `MeridianConflict --uninstall`);
//!
//! and starts the installed launcher, where the player can pick another folder
//! for the builds before pressing Install. Run again over an install, it
//! starts that install.
//!
//! `--uninstall` takes all of that away again, but keeps the replays folder.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::install::{self, Log};

/// The name the game is listed and its shortcuts made under.
const NAME: &str = "Meridian Conflict";
/// The key under HKCU\...\Uninstall.
const UNINSTALL_KEY: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\MeridianConflict";

/// Run as the setup: its file name says so (`MeridianConflictSetup-Playtest.exe`,
/// or a browser's `... (1).exe` of it).
pub(crate) fn is_setup(exe: &Path) -> bool {
    exe.file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|s| s.contains("setup"))
}

/// Where the setup installs: `%LOCALAPPDATA%\Programs\Meridian Conflict`.
fn install_dir() -> Result<PathBuf, String> {
    let local = std::env::var_os("LOCALAPPDATA")
        .ok_or("the setup installs on Windows (no %LOCALAPPDATA% here)")?;
    Ok(PathBuf::from(local).join("Programs").join(NAME))
}

/// Installs this program and starts the installed launcher.
pub(crate) fn run(exe: &Path, log: &mut Log) -> Result<(), String> {
    let dir = install_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let target = dir.join("MeridianConflict.exe");
    log.line(&format!("setup: installing to {}", dir.display()));
    // A launcher already there and running cannot be overwritten; it updates
    // itself anyway, so the setup then just starts it.
    if let Err(e) = std::fs::copy(exe, &target) {
        if !target.is_file() {
            return Err(format!(
                "could not copy the launcher to {}: {e}",
                target.display()
            ));
        }
        log.line(&format!("setup: kept the launcher already installed ({e})"));
    }
    shortcuts(&target, &dir, log);
    register(&target, &dir, log);
    Command::new(&target)
        .current_dir(&dir)
        .spawn()
        .map(drop)
        .map_err(|e| format!("{}: {e}", target.display()))
}

/// The Start-menu and desktop shortcuts, made with Windows' own shell object.
fn shortcuts(target: &Path, dir: &Path, log: &mut Log) {
    let script = "$s = New-Object -ComObject WScript.Shell; \
        foreach ($f in 'Programs', 'Desktop') { \
            $l = $s.CreateShortcut((Join-Path ([Environment]::GetFolderPath($f)) \"$env:MC_NAME.lnk\")); \
            $l.TargetPath = $env:MC_TARGET; $l.WorkingDirectory = $env:MC_DIR; \
            $l.IconLocation = \"$env:MC_TARGET,0\"; $l.Description = $env:MC_NAME; $l.Save() }";
    let done = powershell(script)
        .env("MC_TARGET", target)
        .env("MC_DIR", dir)
        .env("MC_NAME", NAME)
        .status();
    if !done.is_ok_and(|s| s.success()) {
        log.line("setup: the shortcuts were not made");
    }
}

/// The game's entry in Settings > Apps.
fn register(target: &Path, dir: &Path, log: &mut Log) {
    let target = target.to_string_lossy();
    let uninstall = format!("\"{target}\" --uninstall");
    let icon = format!("{target},0");
    let dir = dir.to_string_lossy();
    let values: [(&str, &str, &str); 8] = [
        ("DisplayName", "REG_SZ", NAME),
        ("DisplayIcon", "REG_SZ", &icon),
        ("DisplayVersion", "REG_SZ", env!("CARGO_PKG_VERSION")),
        ("Publisher", "REG_SZ", NAME),
        ("InstallLocation", "REG_SZ", &dir),
        ("UninstallString", "REG_SZ", &uninstall),
        ("NoModify", "REG_DWORD", "1"),
        ("NoRepair", "REG_DWORD", "1"),
    ];
    for (name, kind, data) in values {
        let mut c = Command::new("reg");
        c.args([
            "add",
            UNINSTALL_KEY,
            "/v",
            name,
            "/t",
            kind,
            "/d",
            data,
            "/f",
        ]);
        install::no_console(&mut c);
        if !c.status().is_ok_and(|s| s.success()) {
            log.line(&format!("setup: could not register {name}"));
        }
    }
}

/// Takes the install away: shortcuts, the Apps entry, the builds and the
/// launcher. The replays stay, in the folder they are in.
pub(crate) fn uninstall(exe: &Path, root: &Path, log: &mut Log) -> Result<(), String> {
    if !install::ask(&format!(
        "Uninstall {NAME}?\n\nThe game and its builds are removed. Your replays stay in\n{}",
        root.join("replays").display()
    )) {
        return Ok(());
    }
    log.line("uninstalling");
    // Builds kept in a folder of the player's choosing go too, and that
    // folder with them once nothing else is in it.
    let builds = install::Install::new(exe.to_path_buf(), root.to_path_buf()).builds;
    if builds != root {
        let _ = std::fs::remove_dir_all(builds.join("versions"));
        let _ = std::fs::remove_dir(&builds);
    }
    let script = "foreach ($f in 'Programs', 'Desktop') { \
            Remove-Item -LiteralPath (Join-Path ([Environment]::GetFolderPath($f)) \"$env:MC_NAME.lnk\") -ErrorAction SilentlyContinue }";
    let _ = powershell(script).env("MC_NAME", NAME).status();
    let mut reg = Command::new("reg");
    reg.args(["delete", UNINSTALL_KEY, "/f"]);
    install::no_console(&mut reg);
    let _ = reg.status();
    for name in ["versions", "launcher.json", "launcher.log", "meridian.log"] {
        let p = root.join(name);
        let _ = if p.is_dir() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
    }
    delete_after_exit(exe, root)
}

/// A running program cannot delete itself: a shell waits for it to close,
/// deletes it, and removes the folder if nothing (no replays) is left.
#[cfg(windows)]
fn delete_after_exit(exe: &Path, root: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new("cmd");
    c.arg("/c").raw_arg(format!(
        "ping -n 3 127.0.0.1 >nul & del /f /q \"{}\" \"{}.old\" & rmdir \"{}\"",
        exe.display(),
        exe.display(),
        root.display()
    ));
    install::no_console(&mut c);
    c.spawn()
        .map(drop)
        .map_err(|e| format!("could not finish the uninstall: {e}"))
}

#[cfg(not(windows))]
fn delete_after_exit(_exe: &Path, _root: &Path) -> Result<(), String> {
    Err("uninstalling is for a Windows install".into())
}

fn powershell(script: &str) -> Command {
    let mut c = Command::new("powershell.exe");
    c.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    install::no_console(&mut c);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setup_is_known_by_its_name() {
        assert!(is_setup(Path::new(
            r"C:\Users\a\Downloads\MeridianConflictSetup-Playtest.exe"
        )));
        assert!(is_setup(Path::new(
            "/tmp/MeridianConflictSetup-Playtest (1).exe"
        )));
        assert!(is_setup(Path::new("MeridianConflictSetup")));
        assert!(!is_setup(Path::new(
            r"C:\Games\Meridian Conflict\MeridianConflict.exe"
        )));
    }
}
