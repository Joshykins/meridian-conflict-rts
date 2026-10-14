//! `MeridianConflict(.exe)`: what a player starts (docs/RELEASES.md).
//!
//! A window in the game's look that keeps the game up to date and starts it:
//! it checks the store for the picked channel's newest build and installs it
//! before Play lights up, shows each build's patch notes and the news, lists
//! the installed builds (checking, repairing and removing them), and says
//! whether the channel's server answers. It draws on the CPU, so it runs
//! where the game's GPU driver will not.
//!
//! - `MeridianConflict` opens the window.
//! - `MeridianConflict --replay FILE` plays an old replay in the build that
//!   recorded it, fetching that build first (with its progress in the window),
//!   then starts the current build again on Match History.
//! - `MeridianConflict --screenshot DIR` draws the window in each of its
//!   states into DIR as PNGs, and exits.
//!
//! An update to this program comes with the build it was published with; it
//! goes beside it as `MeridianConflict.exe.new` and is swapped in by
//! restarting at once.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod install;
mod notes;
mod paint;
mod screens;
mod shots;
mod ui;
mod window;
mod work;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use app::{Action, App};
use install::{Install, Log};

const USAGE: &str = "\
MeridianConflict: keeps Meridian Conflict up to date and starts it.

  MeridianConflict                 open the launcher
  MeridianConflict --replay FILE   play FILE in the build that recorded it (fetched
                                   first if need be), then the current build again
  MeridianConflict --screenshot DIR  draw the launcher in each of its states into DIR
                                   (one PNG each, at 1x and 1.5x) and exit
  MeridianConflict --help          this

Built in at compile time (scripts/release.sh): MERIDIAN_CHANNEL (the channel it opens
on), MERIDIAN_STORE (the build store), MERIDIAN_SERVER_PLAYTEST and
MERIDIAN_SERVER_RELEASE (the servers it checks). docs/RELEASES.md has the rest.
";

fn main() -> ExitCode {
    let Ok(exe) = std::env::current_exe() else {
        return ExitCode::FAILURE;
    };
    let root = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        [flag, dir] if flag == "--screenshot" => {
            return match shots::write_all(Path::new(dir)) {
                Ok(paths) => {
                    for p in paths {
                        println!("{}", p.display());
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("MeridianConflict: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        _ => {}
    }
    let mut log = Log::open(&root.join("launcher.log"));
    if install::swap_in_update(&exe, &args, &mut log) {
        return ExitCode::SUCCESS;
    }
    let replay = match args.as_slice() {
        [] => None,
        [flag, file] if flag == "--replay" => Some(PathBuf::from(file)),
        _ => {
            install::tell(&format!("Unknown arguments: {}\n\n{USAGE}", args.join(" ")));
            return ExitCode::FAILURE;
        }
    };
    match run(exe, root.clone(), replay, &mut log) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log.line(&format!("failed: {e}"));
            install::tell(&format!(
                "Meridian Conflict could not start.\n\n{e}\n\nSee {}",
                root.join("launcher.log").display()
            ));
            ExitCode::FAILURE
        }
    }
}

fn run(exe: PathBuf, root: PathBuf, replay: Option<PathBuf>, log: &mut Log) -> Result<(), String> {
    let store = mc_builds::Store::published().ok();
    log.line(&format!(
        "launcher {} ({}), store {}",
        env!("CARGO_PKG_VERSION"),
        install::home_channel(),
        store.as_ref().map_or("none", |s| s.base())
    ));
    let mut app = App::new(Install::new(exe.clone(), root), store);
    if let Some(file) = replay {
        app.start_replay(file, log);
    }
    if let Some(Action::Relaunch) = window::run(app, log)? {
        if !install::swap_in_update(&exe, &[], log) {
            return Err("the launcher's update did not go in".into());
        }
    }
    Ok(())
}
