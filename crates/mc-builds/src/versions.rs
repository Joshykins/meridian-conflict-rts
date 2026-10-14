//! The builds installed side by side in an install's `versions/` folder:
//!
//! ```text
//! versions/<key>/            one build: meridian(.exe), data/, maps/, manifest.json
//! versions/<key>/last-used   touched each time the launcher runs it
//! versions/current           the key of the build the launcher starts
//! versions/.partial-<key>/   a build being installed; never run
//! ```
//!
//! A build is installed whole or not at all: it is put together in its
//! `.partial-` folder and renamed into place. A file another installed build
//! already has (same hash) is linked or copied from there, not downloaded.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::fetch::{Job, Store};
use crate::manifest::{Manifest, MAX_MANIFEST};
use crate::{hex, Error, Result};

const MANIFEST: &str = "manifest.json";
const CURRENT: &str = "current";
const LAST_USED: &str = "last-used";
const PARTIAL: &str = ".partial-";

pub struct Versions {
    dir: PathBuf,
}

/// How far an install has got, for a progress bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

impl Versions {
    /// The `versions/` folder of the install at `root`.
    pub fn new(root: &Path) -> Versions {
        Versions {
            dir: root.join("versions"),
        }
    }

    pub fn dir(&self, key: &str) -> PathBuf {
        self.dir.join(key)
    }

    /// The game executable of an installed build.
    pub fn exe(&self, key: &str) -> PathBuf {
        self.dir(key).join(crate::game_exe())
    }

    /// Every build installed whole, newest first.
    pub fn installed(&self) -> Vec<Manifest> {
        let mut out: Vec<Manifest> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| read_manifest(&e.path().join(MANIFEST)).ok())
            .filter(|m| self.exe(&m.key()).is_file())
            .collect();
        out.sort_by(|a, b| b.number.cmp(&a.number).then_with(|| a.build.cmp(&b.build)));
        out
    }

    pub fn has(&self, build: &str) -> bool {
        let key = crate::key_of(build);
        self.exe(&key).is_file()
            && read_manifest(&self.dir(&key).join(MANIFEST)).is_ok_and(|m| m.build == build)
    }

    /// The build the launcher starts: `current` if it is installed, else the
    /// newest installed build of `channel`.
    pub fn current(&self, channel: mc_core::Channel) -> Option<Manifest> {
        let installed = self.installed();
        let named = std::fs::read_to_string(self.dir.join(CURRENT)).ok();
        let named = named.as_deref().map(str::trim);
        installed
            .iter()
            .find(|m| Some(m.key().as_str()) == named)
            .or_else(|| installed.iter().find(|m| m.channel() == Some(channel)))
            .cloned()
    }

    pub fn set_current(&self, m: &Manifest) -> Result<()> {
        write_atomic(&self.dir.join(CURRENT), m.key().as_bytes())
    }

    pub fn touch(&self, key: &str) {
        let _ = std::fs::write(self.dir(key).join(LAST_USED), b"");
    }

    /// Every installed file, by hash.
    fn local_copies(&self) -> HashMap<String, PathBuf> {
        let mut out = HashMap::new();
        for m in self.installed() {
            let dir = self.dir(&m.key());
            for e in m.files {
                out.entry(e.sha256).or_insert_with(|| dir.join(&e.path));
            }
        }
        out
    }

    /// What installing `m` would download, in bytes.
    pub fn download_size(&self, m: &Manifest) -> u64 {
        if self.has(&m.build) {
            return 0;
        }
        let local = self.local_copies();
        m.packed_size(|e| !local.contains_key(&e.sha256))
    }

    /// Installs `m` from `store`, unless it already is. Returns its folder.
    pub fn install(
        &self,
        store: &Store,
        m: &Manifest,
        progress: &mut dyn FnMut(Progress),
        cancel: &AtomicBool,
    ) -> Result<PathBuf> {
        let key = m.key();
        let dest = self.dir(&key);
        if self.has(&m.build) {
            return Ok(dest);
        }
        let stage = self.dir.join(format!("{PARTIAL}{key}"));
        if stage.exists() {
            std::fs::remove_dir_all(&stage)?;
        }
        let blobs = stage.join(".blobs");
        std::fs::create_dir_all(&blobs)?;
        let local = self.local_copies();
        let mut jobs = Vec::new();
        for e in &m.files {
            let to = stage.join(&e.path);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            match local.get(&e.sha256) {
                Some(from) if from.metadata().is_ok_and(|md| md.len() == e.size) => {
                    if std::fs::hard_link(from, &to).is_err() {
                        std::fs::copy(from, &to)?;
                    }
                }
                _ => jobs.push(Job {
                    path: format!("blobs/{}.zst", e.sha256),
                    dest: blobs.join(format!("{}.zst", e.sha256)),
                    max: e.packed,
                }),
            }
        }
        // Two files with the same bytes are one blob.
        jobs.sort_by(|a, b| a.path.cmp(&b.path));
        jobs.dedup_by(|a, b| a.path == b.path);
        let total = jobs.iter().map(|j| j.max).sum();
        progress(Progress { done: 0, total });
        store.download(
            &jobs,
            &mut |done| progress(Progress { done, total }),
            cancel,
        )?;
        for e in &m.files {
            let to = stage.join(&e.path);
            if to.exists() {
                continue;
            }
            unpack(
                &blobs.join(format!("{}.zst", e.sha256)),
                &to,
                e.size,
                &e.sha256,
            )
            .map_err(|err| match err {
                Error::Hash(_) => Error::Hash(e.path.clone()),
                other => other,
            })?;
        }
        std::fs::remove_dir_all(&blobs)?;
        mark_executable(&stage.join(crate::game_exe()))?;
        std::fs::write(stage.join(MANIFEST), m.to_bytes())?;
        if dest.exists() {
            // Another process installed it meanwhile.
            std::fs::remove_dir_all(&stage)?;
        } else {
            std::fs::rename(&stage, &dest)?;
        }
        Ok(dest)
    }

    /// Removes installed builds beyond the newest `keep` by last use, never
    /// one named in `spare`, and any half-installed build.
    pub fn prune(&self, keep: usize, spare: &[String]) {
        for e in std::fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().starts_with(PARTIAL) {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
        let mut builds: Vec<(SystemTime, String)> = self
            .installed()
            .into_iter()
            .map(|m| {
                let key = m.key();
                let used = std::fs::metadata(self.dir(&key).join(LAST_USED))
                    .and_then(|md| md.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                (used, key)
            })
            .collect();
        builds.sort_by(|a, b| b.cmp(a));
        for (_, key) in builds.into_iter().skip(keep) {
            if !spare.contains(&key) {
                let _ = std::fs::remove_dir_all(self.dir(&key));
            }
        }
    }
}

fn read_manifest(path: &Path) -> Result<Manifest> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    Manifest::parse(&bytes)
}

/// Unpacks a blob to `to`, checking it is `size` bytes hashing to `sha256`.
fn unpack(blob: &Path, to: &Path, size: u64, sha256: &str) -> Result<()> {
    let mut input = zstd::stream::read::Decoder::new(File::open(blob)?)?.take(size + 1);
    let mut out = BufWriter::new(File::create(to)?);
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut n = 0u64;
    loop {
        let read = match input.read(&mut buf) {
            Ok(0) => break,
            Ok(r) => r,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Error::Hash(to.display().to_string())),
        };
        n += read as u64;
        hash.update(&buf[..read]);
        out.write_all(&buf[..read])?;
    }
    out.flush()?;
    if n != size || hex(&hash.finalize()) != sha256 {
        return Err(Error::Hash(to.display().to_string()));
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if path.exists() {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn mark_executable(_: &Path) -> io::Result<()> {
    Ok(())
}
