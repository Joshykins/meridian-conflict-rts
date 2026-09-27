//! Keeping the replay directory from growing without end.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use mc_net::REPLAY_EXTENSION;

/// How often the housekeeping thread prunes.
pub(crate) const PRUNE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Deletes the replays in `dir` last written more than `days` days ago; 0 keeps
/// them all. Anything that is not a replay is left alone. Returns how many went.
pub(crate) fn prune(dir: &Path, days: u32) -> usize {
    if days == 0 {
        return 0;
    }
    let max_age = Duration::from_secs(u64::from(days) * 24 * 60 * 60);
    let now = SystemTime::now();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::error!("could not read {} to prune replays: {e}", dir.display());
            return 0;
        }
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(REPLAY_EXTENSION) {
            continue;
        }
        let modified = entry.metadata().and_then(|m| m.modified());
        let old = modified.is_ok_and(|t| now.duration_since(t).is_ok_and(|age| age > max_age));
        if old {
            match fs::remove_file(&path) {
                Ok(()) => removed += 1,
                Err(e) => log::error!("could not delete {}: {e}", path.display()),
            }
        }
    }
    if removed > 0 {
        log::info!("deleted {removed} replays older than {days} days");
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_old_replays_go() {
        let dir = std::env::temp_dir().join(format!("mc-replays-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let old = dir.join(format!("old.{REPLAY_EXTENSION}"));
        let new = dir.join(format!("new.{REPLAY_EXTENSION}"));
        let other = dir.join("notes.txt");
        for p in [&old, &new, &other] {
            fs::write(p, b"x").unwrap();
        }
        let long_ago = SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60);
        for p in [&old, &other] {
            fs::File::options()
                .write(true)
                .open(p)
                .unwrap()
                .set_modified(long_ago)
                .unwrap();
        }
        assert_eq!(prune(&dir, 0), 0);
        assert_eq!(prune(&dir, 14), 1);
        assert!(!old.exists() && new.exists() && other.exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
