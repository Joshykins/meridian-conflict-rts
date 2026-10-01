//! Flagging a bad moment in a match so it can be staged again later.
//!
//! Every recorded match gets an id (its start time, `20260926-143012`) and its
//! own replay, `replays/<id>.mcreplay`. The profiler panel (F1) shows the id,
//! copies it, and has Mark Issue: that appends the tick, the camera, the frame,
//! GPU and sim timings and a note to `replays/issues.log`, with the command
//! that plays the replay headless up to that tick from that camera.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Beside the working directory, as the replays always were.
pub const DIR: &str = "replays";
const LOG: &str = "issues.log";
/// Replays with no mark kept; older ones are deleted when a match starts. A
/// replay that was marked is never deleted: it is why the log exists.
const KEEP: usize = 20;

/// The recorded match this machine is playing.
pub struct MatchRecord {
    pub id: String,
    pub replay: PathBuf,
}

impl MatchRecord {
    /// A fresh id and replay path for a match starting now, after clearing old replays.
    pub fn new() -> std::io::Result<MatchRecord> {
        let dir = Path::new(DIR);
        std::fs::create_dir_all(dir)?;
        prune(dir);
        let now = unix_secs();
        let base = stamp(now);
        // Two matches in the same second (a quick restart) get a suffix.
        let mut id = base.clone();
        let mut n = 1;
        while replay_of(&id).exists() {
            n += 1;
            id = format!("{base}-{n}");
        }
        Ok(MatchRecord {
            replay: replay_of(&id),
            id,
        })
    }
}

/// One mark of a match, read back from the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub number: u32,
    pub tick: u32,
    pub note: String,
}

/// Every mark made on match `id`, in the order they were made.
pub fn marks_of(id: &str) -> Vec<Mark> {
    parse_marks(
        &std::fs::read_to_string(Path::new(DIR).join(LOG)).unwrap_or_default(),
        id,
    )
}

fn parse_marks(log: &str, id: &str) -> Vec<Mark> {
    let mut out: Vec<Mark> = Vec::new();
    let mut lines = log.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("== match ") else {
            continue;
        };
        let words: Vec<&str> = rest.split_whitespace().collect();
        let (Some(&this), Some(number), Some(tick)) = (
            words.first(),
            words
                .iter()
                .position(|w| *w == "mark")
                .and_then(|i| words.get(i + 1)?.parse().ok()),
            words
                .iter()
                .position(|w| *w == "tick")
                .and_then(|i| words.get(i + 1)?.parse().ok()),
        ) else {
            continue;
        };
        if this != id {
            continue;
        }
        let note = lines
            .peek()
            .and_then(|l| l.strip_prefix("note: "))
            .filter(|n| *n != "-")
            .unwrap_or("")
            .to_owned();
        out.push(Mark { number, tick, note });
    }
    out
}

/// Where the window's picture of mark `n` of match `id` is saved.
pub fn shot_of(id: &str, n: u32) -> PathBuf {
    Path::new(DIR).join(format!("{id}-mark{n}.png"))
}

/// The replay recorded under `id`.
pub fn replay_of(id: &str) -> PathBuf {
    Path::new(DIR).join(format!("{id}.{}", mc_net::REPLAY_EXTENSION))
}

/// Appends one mark to the log; returns the log's path.
pub fn append(entry: &str) -> std::io::Result<PathBuf> {
    let path = Path::new(DIR).join(LOG);
    std::fs::create_dir_all(DIR)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    f.write_all(entry.as_bytes())?;
    Ok(path)
}

/// The header line of a mark, which `prune` reads the match id back from.
pub fn header(id: &str, mark: u32, tick: u32) -> String {
    let secs = tick / mc_core::TICKS_PER_SECOND;
    format!(
        "== match {id}  mark {mark}  tick {tick} ({}:{:02})  written {} UTC\n",
        secs / 60,
        secs % 60,
        stamp(unix_secs())
    )
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `YYYYMMDD-HHMMSS` in UTC: sorts by time and reads out loud.
fn stamp(unix: u64) -> String {
    let (days, rem) = (unix / 86_400, unix % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Match ids that have a mark in the log.
fn marked(dir: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(dir.join(LOG))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.strip_prefix("== match "))
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

/// Deletes all but the newest `KEEP` unmarked replays. The names sort by time.
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let keep = marked(dir);
    let mut old: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == mc_net::REPLAY_EXTENSION))
        .filter(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| !keep.contains(s))
        })
        .collect();
    old.sort();
    let excess = old.len().saturating_sub(KEEP);
    for p in &old[..excess] {
        let _ = std::fs::remove_file(p);
        // Its battle report's record goes with it.
        let _ = std::fs::remove_file(crate::chronicle::file_for(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_read_as_utc_dates() {
        assert_eq!(stamp(0), "19700101-000000");
        // 2026-09-26 14:30:12 UTC
        assert_eq!(stamp(1_790_433_012), "20260926-143012");
        // A leap day.
        assert_eq!(stamp(951_782_400), "20000229-000000");
    }

    #[test]
    fn marked_ids_come_from_headers() {
        let dir = std::env::temp_dir().join(format!("mc-issues-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = format!(
            "{}note: lag\n\n{}",
            header("20260926-143012", 1, 5234),
            header("20260926-150000-2", 1, 10)
        );
        std::fs::write(dir.join(LOG), &log).unwrap();
        let ids = marked(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            ids.into_iter().collect::<Vec<_>>(),
            ["20260926-143012", "20260926-150000-2"]
        );
        let marks = parse_marks(&log, "20260926-143012");
        assert_eq!(
            marks,
            [Mark {
                number: 1,
                tick: 5234,
                note: "lag".into()
            }]
        );
        assert_eq!(parse_marks(&log, "20260926-150000-2")[0].note, "");
    }
}
