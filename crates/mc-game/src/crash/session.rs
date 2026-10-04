//! The log, kept for the reports: everything still goes to stderr, the first and
//! last lines are held in memory for a report, and a player's run writes all of
//! it to `meridian.log` beside the settings file.

use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

/// Log lines kept for the reports: the first ones (the GPU and driver are named
/// at start-up), and the last ones.
const HEAD_LINES: usize = 40;
const TAIL_LINES: usize = 400;

struct Kept {
    head: Vec<String>,
    tail: VecDeque<String>,
    /// Lines dropped between the head and the tail.
    skipped: usize,
}

static KEPT: Mutex<Kept> = Mutex::new(Kept {
    head: Vec::new(),
    tail: VecDeque::new(),
    skipped: 0,
});

/// `meridian.log`, once a player's run has opened it (`open`).
static FILE: Mutex<Option<File>> = Mutex::new(None);

/// The log's writer. A failed write to stderr is ignored: when whoever started
/// the game goes away, stderr is a closed pipe, and that must not end the game.
pub struct LogTee;

impl Write for LogTee {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut kept) = KEPT.lock() {
            for line in String::from_utf8_lossy(buf).lines() {
                if kept.head.len() < HEAD_LINES {
                    kept.head.push(line.to_owned());
                    continue;
                }
                if kept.tail.len() == TAIL_LINES {
                    kept.tail.pop_front();
                    kept.skipped += 1;
                }
                kept.tail.push_back(line.to_owned());
            }
        }
        if let Ok(mut file) = FILE.lock() {
            if let Some(f) = file.as_mut() {
                let _ = f.write_all(buf);
            }
        }
        let _ = std::io::stderr().write_all(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        Ok(())
    }
}

/// Starts `meridian.log` (the last run's becomes `meridian-previous.log`) with
/// the lines logged so far.
pub(super) fn open() {
    let Some(dir) = crate::settings::config_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("meridian.log");
    let _ = std::fs::rename(&path, dir.join("meridian-previous.log"));
    let Ok(mut file) = File::create(&path) else {
        return;
    };
    let _ = writeln!(
        file,
        "Meridian Conflict {}, {}",
        env!("MERIDIAN_BUILD"),
        super::system_line()
    );
    if let Ok(kept) = KEPT.lock() {
        for line in kept.head.iter().chain(&kept.tail) {
            let _ = writeln!(file, "{line}");
        }
    }
    if let Ok(mut slot) = FILE.lock() {
        *slot = Some(file);
    }
}

/// Adds a line to `meridian.log` only (where a report went). Never waits: it is
/// called from the panic hook, which may have interrupted a log write.
pub(super) fn note(line: &str) {
    if let Ok(mut file) = FILE.try_lock() {
        if let Some(f) = file.as_mut() {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// The kept log lines for a report, or nothing when the lock is held or poisoned
/// (a panic while a line was being kept): a report never waits on it.
pub(super) fn kept_lines() -> String {
    let Ok(kept) = KEPT.try_lock() else {
        return String::new();
    };
    let mut text = String::from("\nlog:\n");
    for line in &kept.head {
        text.push_str(line);
        text.push('\n');
    }
    if kept.skipped > 0 {
        text.push_str(&format!("... {} lines ...\n", kept.skipped));
    }
    for line in &kept.tail {
        text.push_str(line);
        text.push('\n');
    }
    text
}
