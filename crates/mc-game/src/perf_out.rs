//! Where `--perf FILE` (or `MERIDIAN_PERF=FILE`) sends reports.
//!
//! A headless run writes `FILE` with the `.json` swapped for `.sim.json` (every
//! simulated tick) and, when it renders, `.frames.json` (every followed frame:
//! GPU scopes with triangle/fragment counts, CPU render spans), each with a
//! readable `.txt` beside it.

use std::path::PathBuf;
use std::sync::OnceLock;

use mc_core::perf::Report;

static PATH: OnceLock<PathBuf> = OnceLock::new();

/// Set once from the command line.
pub fn set(path: PathBuf) {
    let _ = PATH.set(path);
}

pub fn path() -> Option<PathBuf> {
    PATH.get()
        .cloned()
        .or_else(|| std::env::var_os("MERIDIAN_PERF").map(PathBuf::from))
}

pub fn enabled() -> bool {
    path().is_some()
}

/// Writes `report` as `<stem>.<kind>.json` and `.txt`, and prints the table.
pub fn save(report: &Report, kind: &str) {
    let Some(path) = path() else { return };
    let stem = path.with_extension("");
    let out = PathBuf::from(format!("{}.{kind}.json", stem.display()));
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(dir);
    }
    println!("{}", report.text());
    match report.save(&out) {
        Ok(()) => println!("perf report: {}", out.display()),
        Err(e) => eprintln!("perf: could not write {}: {e}", out.display()),
    }
}
