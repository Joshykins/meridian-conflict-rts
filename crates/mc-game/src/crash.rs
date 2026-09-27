//! Crash reports. The Windows build has no console, so a panic would otherwise
//! vanish with the window. Every panic, on any thread, is written with a
//! backtrace to `crash-<unix seconds>.log` beside the settings file, and the
//! default hook still prints it to stderr.

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Keeps this many reports; older ones are deleted when a new one is written.
const KEEP: usize = 10;

pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(path) = write_report(info) {
            eprintln!("crash report written to {}", path.display());
        }
        default(info);
    }));
}

fn write_report(info: &std::panic::PanicHookInfo<'_>) -> Option<PathBuf> {
    let dir = crate::settings::config_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = dir.join(format!("crash-{secs}.log"));
    let thread = std::thread::current();
    let report = format!(
        "meridian {} crashed\nthread: {}\n{info}\n\n{}\n",
        env!("CARGO_PKG_VERSION"),
        thread.name().unwrap_or("<unnamed>"),
        std::backtrace::Backtrace::force_capture(),
    );
    std::fs::File::create(&path)
        .and_then(|mut f| f.write_all(report.as_bytes()))
        .ok()?;
    prune(&dir);
    Some(path)
}

/// Reports an error that ended the game without a panic (no Vulkan driver, no
/// window): written to `error-<unix seconds>.log` beside the settings file and,
/// on Windows, shown in a message box, since the console that printed it closes
/// with the program.
pub fn report_error(message: &str) {
    let path = write_error(message);
    #[cfg(windows)]
    {
        let mut text = format!("Meridian Conflict could not continue:\n\n{message}");
        if let Some(path) = &path {
            text.push_str(&format!("\n\nThis was saved to {}", path.display()));
        }
        message_box(&text);
    }
    #[cfg(not(windows))]
    let _ = path;
}

fn write_error(message: &str) -> Option<PathBuf> {
    let dir = crate::settings::config_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = dir.join(format!("error-{secs}.log"));
    let report = format!("meridian {} stopped\n{message}\n", env!("MERIDIAN_BUILD"));
    std::fs::write(&path, report).ok()?;
    Some(path)
}

#[cfg(windows)]
fn message_box(text: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(window: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    const MB_ICONERROR: u32 = 0x10;
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (text, caption) = (wide(text), wide("Meridian Conflict"));
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the
    // call, and a null owner window is allowed.
    unsafe { MessageBoxW(0, text.as_ptr(), caption.as_ptr(), MB_ICONERROR) };
}

/// Deletes all but the newest `KEEP` reports. The names sort by time.
fn prune(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut reports: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".log"))
        })
        .collect();
    reports.sort_by_key(|p| {
        p.file_stem()
            .and_then(|n| n.to_str())
            .and_then(|n| n.trim_start_matches("crash-").parse::<u64>().ok())
            .unwrap_or(0)
    });
    let excess = reports.len().saturating_sub(KEEP);
    for old in &reports[..excess] {
        let _ = std::fs::remove_file(old);
    }
}
