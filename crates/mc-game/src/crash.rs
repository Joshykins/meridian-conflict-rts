//! Crash reports, and the window that shows one. The Windows build has no
//! console, so whatever ends the game must say so on screen and leave a file:
//!
//! - A panic, on any thread, is written with a backtrace to `crash-<unix
//!   seconds>.log` beside the settings file. One that ends the game (it reaches
//!   `main`, or ends the sim thread: `FatalGuard`) is then shown in the crash
//!   window.
//! - An error `run` returns (no Vulkan driver, broken data) is written to
//!   `error-<secs>.log` and shown.
//! - A native fault (an access violation in a driver, say) is written to
//!   `crash-<secs>.log` with a minidump beside it, and shown (`native.rs`).
//! - The whole log of a windowed run goes to `meridian.log` (the run before it
//!   is kept as `meridian-previous.log`, `session.rs`), so a run that was killed
//!   from outside, which nothing can catch, still leaves its trace.
//!
//! The window (`dialog.rs`) says what happened in a sentence, and has Copy
//! details (the whole report onto the clipboard, to paste into a message) and
//! Open folder (the file, selected in Explorer).
//!
//! Every report carries the build, the first lines of the log (the GPU and
//! driver), its last ones (how far the game got), and on Windows the raw stack
//! as `module+offset`, which the build's `.pdb` turns back into names when the
//! player's copy has none (`scripts/package-windows.sh` keeps it).
//!
//! Dialogs are only shown to a player: a run with no arguments, or one that
//! opened the game's window. A tool run (a headless shot, the shot server, a
//! soak bot) writes its reports and exits without waiting on a window.

mod dialog;
mod drill;
#[cfg(windows)]
mod native;
mod reporter;
mod screen;
mod session;
#[cfg(windows)]
mod stack;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub use drill::{arm as arm_drill, tick as drill_tick, Drill};
pub(crate) use reporter::{run as run_screen, screenshot as screen_shot};
pub(crate) use screen::Summary;
pub use session::LogTee;

/// Keeps this many reports of each kind; older ones are deleted when a new one is
/// written.
const KEEP: usize = 10;

/// Minidumps are larger (a few MB each): fewer are kept.
const KEEP_DUMPS: usize = 3;

/// Whether a player is in front of this run (see the module notes).
static INTERACTIVE: AtomicBool = AtomicBool::new(false);

/// A panic this run, for the window and for the reports after it.
struct Panicked {
    thread: std::thread::ThreadId,
    /// Thread name and message, one line: "earlier panics" in later reports.
    summary: String,
    /// The message alone, for the window.
    message: String,
    path: Option<PathBuf>,
    report: String,
}

/// The panics so far. Capped: a panic in a loop (an audio callback caught and
/// retried) must not grow it; past the cap only the count goes up.
const KEEP_PANICS: usize = 8;
static PANICS: Mutex<(Vec<Panicked>, usize)> = Mutex::new((Vec::new(), 0));

pub fn install() {
    if std::env::args_os().len() <= 1 {
        set_interactive();
    }
    #[cfg(windows)]
    native::install();
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let panicked = panic_report(info);
        // Never `eprintln!` here: it panics when stderr is a closed pipe, and a
        // panic inside the hook aborts with no report at all.
        if let Some(path) = &panicked.path {
            let _ = writeln!(
                std::io::stderr(),
                "crash report written to {}",
                path.display()
            );
            session::note(&format!("crash report written to {}", path.display()));
        }
        if let Ok(mut panics) = PANICS.lock() {
            panics.1 += 1;
            if panics.0.len() < KEEP_PANICS {
                panics.0.push(panicked);
            }
        }
        default(info);
    }));
}

/// A player is watching from here on: errors and crashes are shown in a window,
/// and the log goes to `meridian.log`. Called for a run with no arguments, and
/// when the game opens its window.
pub fn set_interactive() {
    if !INTERACTIVE.swap(true, Ordering::Relaxed) {
        session::open();
    }
}

fn interactive() -> bool {
    INTERACTIVE.load(Ordering::Relaxed)
}

/// Whether this caller may show the crash window: a player's run, and the first
/// failure. A second one while the first is on screen (the main thread tripping
/// over the sim thread's death) waits here for good: the first ends the process
/// when its window closes, and its report is the cause.
fn claim_window() -> bool {
    static SHOWN: AtomicBool = AtomicBool::new(false);
    if !interactive() {
        return false;
    }
    if SHOWN.swap(true, Ordering::SeqCst) {
        loop {
            std::thread::park();
        }
    }
    true
}

/// Runs the game, and reports whatever ends it badly: an error it returns, or a
/// panic that reaches here. Returns the process's exit code.
pub fn run_guarded(run: impl FnOnce() -> Result<(), String>) -> i32 {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => {
            let _ = writeln!(std::io::stderr(), "error: {e}");
            report_error(&e);
            1
        }
        Err(_) => {
            show_panic();
            101
        }
    }
}

/// Held by a thread whose death ends the game (the sim thread): when it unwinds
/// from a panic, the crash window is shown and the process exits, instead of the
/// game freezing on a world that no longer moves. A tool run unwinds as before.
pub struct FatalGuard;

impl Drop for FatalGuard {
    fn drop(&mut self) {
        if std::thread::panicking() && interactive() {
            show_panic();
            std::process::exit(101);
        }
    }
}

/// Shows a failure: the crash screen, a process of its own; or, when that cannot
/// come up (or the report could not be saved for it to read), the task dialog.
fn present(shown: &dialog::Shown<'_>) {
    if let Some(path) = shown.path {
        let summary = Summary {
            title: shown.title.to_owned(),
            message: shown.message.to_owned(),
            hint: shown.hint.map(str::to_owned),
            path: path.to_owned(),
        };
        if reporter::launch(&summary) {
            return;
        }
    }
    dialog::show(shown);
}

/// Shows `path` in the system's file browser: selected in Explorer on Windows,
/// its folder elsewhere.
fn open_folder(path: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Explorer reads `/select,` and the quoted path as one argument, which the
        // standard quoting would split.
        let _ = std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        if let Some(dir) = path.parent() {
            let _ = std::process::Command::new(opener).arg(dir).spawn();
        }
    }
}

/// Shows the panic this thread raised last (or, if none was kept, the last one).
fn show_panic() {
    let me = std::thread::current().id();
    let shown = PANICS.lock().ok().and_then(|panics| {
        let p = panics
            .0
            .iter()
            .rev()
            .find(|p| p.thread == me)
            .or(panics.0.last())?;
        Some((p.message.clone(), p.path.clone(), p.report.clone()))
    });
    let (message, path, report) =
        shown.unwrap_or_else(|| ("(the panic was not recorded)".into(), None, String::new()));
    if !claim_window() {
        return;
    }
    present(&dialog::Shown {
        heading: "Meridian Conflict crashed",
        title: "The game crashed",
        content: &format!(
            "Something went wrong inside the game and it had to close.\n\n{}",
            clip(&message, 600)
        ),
        message: &message,
        hint: None,
        report: &report,
        path: path.as_deref(),
    });
}

/// Reports an error that ended the game without a panic (no Vulkan driver, no
/// window, broken data): written to `error-<unix seconds>.log` and shown.
pub fn report_error(message: &str) {
    let report = format!(
        "Meridian Conflict {} stopped\n{}\n\n{message}\n{}",
        env!("MERIDIAN_BUILD"),
        system_line(),
        session::kept_lines()
    );
    let path = save("error", "log", report.as_bytes());
    if let Some(path) = &path {
        session::note(&format!("error report written to {}", path.display()));
    }
    if !claim_window() {
        return;
    }
    // How ash words VK_ERROR_DEVICE_LOST.
    let hint = message.contains("device has been lost").then_some(
        "The graphics driver reset the GPU. Updating the graphics driver usually fixes \
         this; if not, please send us the details.",
    );
    present(&dialog::Shown {
        heading: "Meridian Conflict had to stop",
        title: "The game had to stop",
        content: &clip(message, 1200),
        message,
        hint,
        report: &report,
        path: path.as_deref(),
    });
}

fn panic_report(info: &std::panic::PanicHookInfo<'_>) -> Panicked {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("<unnamed>").to_owned();
    let payload = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".into());
    let at = info
        .location()
        .map(|l| format!("{}:{}", l.file(), l.line()))
        .unwrap_or_default();
    let message = format!("{payload}\n\nat {at}, thread {name}");
    let earlier = PANICS
        .try_lock()
        .map(|panics| {
            let mut text = String::new();
            for p in &panics.0 {
                text.push_str(&format!("  {}\n", p.summary));
            }
            if panics.1 > panics.0.len() {
                text.push_str(&format!("  ... {} more\n", panics.1 - panics.0.len()));
            }
            text
        })
        .unwrap_or_default();
    let mut report = format!(
        "Meridian Conflict {} crashed\n{}\nthread: {name}\n{info}\n\nbacktrace:\n{}\n",
        env!("MERIDIAN_BUILD"),
        system_line(),
        from_the_panic(&std::backtrace::Backtrace::force_capture().to_string()),
    );
    #[cfg(windows)]
    report.push_str(&format!("stack:\n{}\n", stack::here()));
    if !earlier.is_empty() {
        report.push_str(&format!("earlier panics this run:\n{earlier}\n"));
    }
    report.push_str(&session::kept_lines());
    let path = save("crash", "log", report.as_bytes());
    Panicked {
        thread: thread.id(),
        summary: format!("thread {name}: {}", payload.lines().next().unwrap_or("")),
        message,
        path,
        report,
    }
}

/// A backtrace from where the panic was raised: the frames of the capture and
/// of the panic machinery above it (the hook, `panic_fmt`) are cut, so the
/// first frame is the code that panicked. Unchanged when the machinery is not
/// found (a symbol-less build prints `<unknown>` for every frame).
fn from_the_panic(trace: &str) -> String {
    // Frames start "  N: name"; their "at file:line" lines follow.
    let starts: Vec<(usize, &str)> = trace
        .match_indices('\n')
        .map(|(i, _)| i + 1)
        .chain([0])
        .filter_map(|at| {
            let line = trace[at..].lines().next()?;
            let (n, name) = line.trim_start().split_once(": ")?;
            n.parse::<u32>().ok().map(|_| (at, name))
        })
        .collect();
    // The run of frames that raise a panic, from the hook down to `panic_fmt`
    // and the `unwrap`/`expect` that called it. Only the first run is cut: a
    // `catch_unwind` further down the stack is the game's own.
    const MACHINERY: [&str; 7] = [
        "std::panicking::",
        "core::panicking::",
        "rust_begin_unwind",
        "std::sys::backtrace::__rust_end_short_backtrace",
        "core::option::unwrap_failed",
        "core::option::expect_failed",
        "core::result::unwrap_failed",
    ];
    let raising = |name: &str| MACHINERY.iter().any(|m| name.starts_with(m));
    let last = starts
        .iter()
        .position(|(_, name)| raising(name))
        .map(|first| {
            first
                + starts[first..]
                    .iter()
                    .take_while(|(_, name)| raising(name))
                    .count()
                - 1
        });
    match last.and_then(|i| starts.get(i + 1)) {
        Some(&(at, _)) => trace[at..].to_owned(),
        None => trace.to_owned(),
    }
}

/// The platform and when, for the top of a report.
fn system_line() -> String {
    let secs = unix_now();
    format!(
        "{} {}, unix time {secs}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// At most `max` characters of `text`, with an ellipsis when cut.
fn clip(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_owned(),
    }
}

/// Writes `<kind>-<unix seconds>.<ext>` beside the settings file (`-2`, `-3`, ...
/// when that second already has one) and prunes the old ones of that kind.
fn save(kind: &str, ext: &str, bytes: &[u8]) -> Option<PathBuf> {
    let dir = crate::settings::config_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let secs = unix_now();
    let (path, mut file) = (1..100).find_map(|n| {
        let name = match n {
            1 => format!("{kind}-{secs}.{ext}"),
            n => format!("{kind}-{secs}-{n}.{ext}"),
        };
        let path = dir.join(name);
        let file = std::fs::File::create_new(&path).ok()?;
        Some((path, file))
    })?;
    file.write_all(bytes).ok()?;
    prune(
        &dir,
        kind,
        ext,
        if ext == "dmp" { KEEP_DUMPS } else { KEEP },
    );
    Some(path)
}

/// Deletes all but the newest `keep` files named `<kind>-<secs>[-n].<ext>`.
fn prune(dir: &Path, kind: &str, ext: &str, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let prefix = format!("{kind}-");
    let suffix = format!(".{ext}");
    let mut reports: Vec<(u64, u32, PathBuf)> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?;
            let stamp = name.strip_prefix(&prefix)?.strip_suffix(&suffix)?;
            let (secs, n) = stamp.split_once('-').unwrap_or((stamp, "1"));
            Some((secs.parse().ok()?, n.parse().ok()?, p))
        })
        .collect();
    reports.sort_unstable();
    let excess = reports.len().saturating_sub(keep);
    for (_, _, old) in &reports[..excess] {
        let _ = std::fs::remove_file(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_keeps_the_newest_of_one_kind() {
        let dir = std::env::temp_dir().join(format!("mc-crash-prune-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "crash-100.log",
            "crash-300.log",
            "crash-300-2.log",
            "crash-200.log",
            "error-50.log",
            "crash-400.dmp",
            "meridian.log",
        ] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        prune(&dir, "crash", "log", 2);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort_unstable();
        assert_eq!(
            left,
            [
                "crash-300-2.log",
                "crash-300.log",
                "crash-400.dmp",
                "error-50.log",
                "meridian.log"
            ]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_backtrace_starts_where_the_panic_was_raised() {
        let trace = "   0: std::backtrace::Backtrace::create\n             at backtrace.rs:331\n   1: meridian::crash::panic_report\n   2: std::panicking::panic_handler\n   3: std::sys::backtrace::__rust_end_short_backtrace<x>\n  10: core::panicking::panic_fmt\n             at panicking.rs:80\n  11: meridian::game::tick\n             at game.rs:12\n  12: std::panicking::catch_unwind\n  13: main\n";
        let trimmed = from_the_panic(trace);
        assert!(
            trimmed.starts_with("  11: meridian::game::tick"),
            "{trimmed}"
        );
        assert!(trimmed.contains("  13: main"));
        // A trace without the machinery is kept whole.
        let bare = "   0: <unknown>\n   1: <unknown>\n";
        assert_eq!(from_the_panic(bare), bare);
    }

    #[test]
    fn clip_cuts_on_characters() {
        assert_eq!(clip("héllo", 2), "hé…");
        assert_eq!(clip("hi", 2), "hi");
    }
}
