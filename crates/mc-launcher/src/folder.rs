//! Where the builds go. Nothing is downloaded until the player presses
//! Install, so the folder can be picked first; the Builds page changes it
//! later, moving what is installed. The folder dialog is the system's own,
//! open on a thread so the window keeps drawing, and the room left on the
//! folder's drive is shown beside what the download needs.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mc_builds::Manifest;

use crate::work::Job;

/// A folder of the game's own inside the one the player picks.
const OWN: &str = "Meridian Conflict";

/// How often the room on the drive is read again.
const ROOM_AGAIN: Duration = Duration::from_secs(3);

/// The builds' folder: being picked, being moved to, and the room around it.
#[derive(Default)]
pub(crate) struct Folder {
    /// The folder dialog.
    pub(crate) asking: Option<Job<Option<PathBuf>>>,
    /// The installed builds on their way to a new folder, and that folder.
    pub(crate) moving: Option<(PathBuf, Job<()>)>,
    /// The last reading of [`Room`], for this folder and build.
    room: Option<(PathBuf, String, Instant, Room)>,
}

/// What installing a build takes on the builds' drive, and what it has.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Room {
    pub(crate) download: u64,
    /// The most it takes while it installs (mc_builds `room_needed`).
    pub(crate) needed: u64,
    /// `None` when the drive would not say.
    pub(crate) free: Option<u64>,
}

impl Room {
    /// The drive is known to be too full for it.
    pub(crate) fn short(&self) -> bool {
        self.free.is_some_and(|f| f < self.needed)
    }
}

impl Folder {
    /// The dialog is open or the builds are moving: nothing else may touch them.
    pub(crate) fn busy(&self) -> bool {
        self.asking.as_ref().is_some_and(Job::running)
            || self.moving.as_ref().is_some_and(|(_, j)| j.running())
    }

    /// The room installing `m` into `versions` takes, read again every few
    /// seconds; `still` (the screenshot scenes) keeps what is there.
    pub(crate) fn room(
        &mut self,
        versions: &mc_builds::Versions,
        builds: &Path,
        m: &Manifest,
        still: bool,
    ) -> Room {
        let fresh = |(dir, key, at, _): &(PathBuf, String, Instant, Room)| {
            dir == builds && *key == m.key() && (still || at.elapsed() < ROOM_AGAIN)
        };
        if let Some(r) = self.room.as_ref().filter(|r| fresh(r)) {
            return r.3;
        }
        let room = Room {
            download: versions.download_size(m),
            needed: versions.room_needed(m),
            free: free(builds),
        };
        self.room = Some((builds.to_path_buf(), m.key(), Instant::now(), room));
        room
    }

    /// Sets the room shown for `m` in `builds`, for the screenshot scenes.
    pub(crate) fn set_room(&mut self, builds: &Path, m: &Manifest, room: Room) {
        self.room = Some((builds.to_path_buf(), m.key(), Instant::now(), room));
    }
}

/// The builds' folder for the folder the player picked: a folder of the
/// game's own inside it, unless it is one already (named for the game, or
/// holding builds).
pub(crate) fn place(picked: &Path) -> PathBuf {
    let own = picked
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case(OWN))
        || picked.join("versions").is_dir();
    if own {
        picked.to_path_buf()
    } else {
        picked.join(OWN)
    }
}

/// Opens the system's folder dialog at `start`, on a thread of its own.
/// `Ok(None)`: the player closed it without picking.
pub(crate) fn ask(start: &Path) -> Job<Option<PathBuf>> {
    let start = existing(start);
    Job::spawn("folder dialog", move |_| dialog(&start))
}

#[cfg(windows)]
fn dialog(start: &Path) -> Result<Option<PathBuf>, String> {
    Ok(rfd::FileDialog::new()
        .set_title("Choose where Meridian Conflict's builds go")
        .set_directory(start)
        .pick_folder())
}

/// Away from Windows the launcher asks zenity, where there is one.
#[cfg(not(windows))]
fn dialog(start: &Path) -> Result<Option<PathBuf>, String> {
    let out = std::process::Command::new("zenity")
        .args([
            "--file-selection",
            "--directory",
            "--title=Choose where Meridian Conflict's builds go",
        ])
        .arg(format!("--filename={}/", start.display()))
        .output()
        .map_err(|e| format!("there is no folder dialog here (zenity: {e})"))?;
    let picked = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    Ok((out.status.success() && !picked.is_empty()).then(|| PathBuf::from(picked)))
}

/// Makes `dir` and writes a file in it, to find out now that the builds
/// cannot go there rather than halfway through a download.
pub(crate) fn writable(dir: &Path) -> Result<(), String> {
    let probe = dir.join(".meridian-write-test");
    std::fs::create_dir_all(dir)
        .and_then(|()| std::fs::write(&probe, b""))
        .and_then(|()| std::fs::remove_file(&probe))
        .map_err(|e| format!("The builds cannot go in {}: {e}", dir.display()))
}

/// The bytes free on `dir`'s drive, `dir` made or not.
pub(crate) fn free(dir: &Path) -> Option<u64> {
    fs4::available_space(existing(dir)).ok()
}

/// `dir`, or the nearest folder above it that is there.
fn existing(dir: &Path) -> PathBuf {
    dir.ancestors()
        .find(|a| a.is_dir())
        .unwrap_or(dir)
        .to_path_buf()
}

/// "D:" for a folder on drive D, for "free on D:"; elsewhere nothing.
pub(crate) fn drive(dir: &Path) -> Option<String> {
    match dir.components().next()? {
        std::path::Component::Prefix(p) => Some(p.as_os_str().to_string_lossy().into_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_folder_gets_a_folder_of_the_games_own() {
        let tmp = std::env::temp_dir().join(format!("mc-folder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        assert_eq!(place(&tmp.join("Games")), tmp.join("Games").join(OWN));
        assert_eq!(place(&tmp.join(OWN)), tmp.join(OWN));
        assert_eq!(
            place(&tmp.join("meridian conflict")),
            tmp.join("meridian conflict")
        );
        // A folder already holding builds is taken as it is.
        std::fs::create_dir_all(tmp.join("Old").join("versions")).unwrap();
        assert_eq!(place(&tmp.join("Old")), tmp.join("Old"));
        assert!(writable(&tmp.join("New")).is_ok());
        assert!(free(&tmp.join("not").join("made")).is_some());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
