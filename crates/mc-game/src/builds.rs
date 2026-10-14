//! The game's side of the build store (docs/RELEASES.md): when the launcher
//! started it, it downloads this channel's updates and the old builds that
//! old replays need, in the background, and asks the launcher to run them.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use mc_builds::versions::Progress;
use mc_builds::{Manifest, Store, Versions};

/// The install this game runs in: the launcher that started it, its folder,
/// and the store its builds come from.
pub struct Install {
    launcher: PathBuf,
    root: PathBuf,
    store: Store,
}

impl Install {
    /// Only a game the launcher started (`MERIDIAN_LAUNCHER`), in a published
    /// build (one made with a store), has an install to manage.
    pub fn find() -> Option<Install> {
        let launcher = PathBuf::from(std::env::var_os("MERIDIAN_LAUNCHER")?);
        let root = launcher.parent()?.to_path_buf();
        let store = Store::published().ok()?;
        launcher.is_file().then_some(Install {
            launcher,
            root,
            store,
        })
    }

    fn versions(&self) -> Versions {
        Versions::new(&self.root)
    }

    /// The build is installed and can be run.
    pub fn has(&self, build: &str) -> bool {
        self.versions().has(build)
    }

    /// Has the launcher play `replay` in the build that recorded it. The game
    /// quits; the launcher starts it again afterwards.
    pub fn watch_in_its_build(&self, replay: &Path) -> std::io::Result<()> {
        let replay = std::fs::canonicalize(replay)?;
        Command::new(&self.launcher)
            .arg("--replay")
            .arg(replay)
            .current_dir(&self.root)
            .spawn()
            .map(drop)
    }

    /// Starts the launcher again, which starts the current build. The game quits.
    pub fn restart(&self) -> std::io::Result<()> {
        Command::new(&self.launcher)
            .current_dir(&self.root)
            .spawn()
            .map(drop)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Checking,
    /// No newer build is published.
    UpToDate,
    Downloading {
        progress: Progress,
    },
    /// Installed; `number` is its build number.
    Ready {
        build: String,
        number: u32,
    },
    Failed(String),
}

/// A download running in the background; dropping it cancels it.
pub struct Download {
    status: Arc<Mutex<Status>>,
    cancel: Arc<AtomicBool>,
}

impl Download {
    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    /// Fetches this channel's newest build, if it is newer than this one, and
    /// makes it the one the launcher starts next. The launcher's own update
    /// comes with it.
    pub fn update(install: &Install) -> Download {
        let ours = crate::build_info::number().unwrap_or(0);
        Download::start(install, move |store| {
            let newest = store.newest(crate::build_info::channel())?;
            Ok((newest.number > ours && newest.build != crate::BUILD).then_some(newest))
        })
    }

    /// Fetches the build named `build` (an old replay's).
    pub fn build(install: &Install, build: String) -> Download {
        Download::start(install, move |store| store.manifest(&build).map(Some))
    }

    fn start(
        install: &Install,
        find: impl FnOnce(&Store) -> mc_builds::Result<Option<Manifest>> + Send + 'static,
    ) -> Download {
        let status = Arc::new(Mutex::new(Status::Checking));
        let cancel = Arc::new(AtomicBool::new(false));
        let (out, stop) = (status.clone(), cancel.clone());
        let (store, root, launcher) = (
            install.store.clone(),
            install.root.clone(),
            install.launcher.clone(),
        );
        std::thread::Builder::new()
            .name("build download".into())
            .spawn(move || {
                let done = fetch(&store, &root, &launcher, find, &out, &stop);
                let end = match done {
                    Ok(Some(m)) => Status::Ready {
                        build: m.build,
                        number: m.number,
                    },
                    Ok(None) => Status::UpToDate,
                    Err(mc_builds::Error::Cancelled) => return,
                    Err(e) => {
                        log::warn!("build download: {e}");
                        Status::Failed(e.to_string())
                    }
                };
                *out.lock().unwrap() = end;
            })
            .expect("a thread for the download");
        Download { status, cancel }
    }
}

impl Drop for Download {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Finds the build and installs it. One newer than this build on this channel
/// (an update, or a newer build's replay) becomes the one the launcher starts,
/// and the launcher it brings is staged.
fn fetch(
    store: &Store,
    root: &Path,
    launcher: &Path,
    find: impl FnOnce(&Store) -> mc_builds::Result<Option<Manifest>>,
    status: &Mutex<Status>,
    cancel: &AtomicBool,
) -> mc_builds::Result<Option<Manifest>> {
    let Some(m) = find(store)? else {
        return Ok(None);
    };
    let versions = Versions::new(root);
    let dir = versions.install(
        store,
        &m,
        &mut |progress| *status.lock().unwrap() = Status::Downloading { progress },
        cancel,
    )?;
    if m.channel() == Some(crate::build_info::channel())
        && m.number > crate::build_info::number().unwrap_or(0)
    {
        versions.set_current(&m)?;
        stage_launcher(&dir, launcher)?;
    }
    Ok(Some(m))
}

/// A build carries the launcher it was published with
/// (`launcher/MeridianConflict.exe`); when it differs from the running one it
/// goes beside it as `.new`, and the launcher swaps it in on its next start.
fn stage_launcher(build: &Path, launcher: &Path) -> std::io::Result<()> {
    let Some(name) = launcher.file_name() else {
        return Ok(());
    };
    let theirs = build.join("launcher").join(name);
    let Ok(new) = std::fs::read(&theirs) else {
        return Ok(());
    };
    if std::fs::read(launcher).is_ok_and(|ours| ours == new) {
        return Ok(());
    }
    let mut staged = launcher.as_os_str().to_owned();
    staged.push(".new");
    std::fs::write(PathBuf::from(staged), new)
}
