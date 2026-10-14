//! The game's side of the install (docs/RELEASES.md). The launcher keeps the
//! game up to date and fetches old builds; the game only asks it to play an
//! old replay in the build that recorded it.

use std::path::{Path, PathBuf};
use std::process::Command;

use mc_builds::Versions;

/// The install this game runs in: the launcher that started it and its folder.
pub struct Install {
    launcher: PathBuf,
    root: PathBuf,
}

impl Install {
    /// Only a game the launcher started (`MERIDIAN_LAUNCHER`), in a published
    /// build (one made with a store), has an install to manage.
    pub fn find() -> Option<Install> {
        let launcher = PathBuf::from(std::env::var_os("MERIDIAN_LAUNCHER")?);
        let root = launcher.parent()?.to_path_buf();
        mc_builds::STORE_URL.filter(|url| !url.is_empty())?;
        launcher.is_file().then_some(Install { launcher, root })
    }

    fn versions(&self) -> Versions {
        Versions::new(&self.root)
    }

    /// The build is installed and can be run.
    pub fn has(&self, build: &str) -> bool {
        self.versions().has(build)
    }

    /// Has the launcher play `replay` in the build that recorded it, fetching
    /// that build first when it is not installed. The game quits; the launcher
    /// starts it again afterwards.
    pub fn watch_in_its_build(&self, replay: &Path) -> std::io::Result<()> {
        let replay = std::fs::canonicalize(replay)?;
        Command::new(&self.launcher)
            .arg("--replay")
            .arg(replay)
            .current_dir(&self.root)
            .spawn()
            .map(drop)
    }
}
