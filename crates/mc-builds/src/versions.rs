//! The builds installed side by side in an install's `versions/` folder:
//!
//! ```text
//! versions/<key>/            one build: meridian(.exe), data/, maps/, manifest.json
//! versions/<key>/last-used   touched each time the launcher runs it
//! versions/current-<channel> the key of the build the launcher starts on that channel
//! versions/.partial-<key>/   a build being installed; never run
//! versions/.repair-<key>/    the files a repair is downloading for that build
//! ```
//!
//! A build is installed whole or not at all: it is put together in its
//! `.partial-` folder and renamed into place. A file another installed build
//! already has (same hash) is linked or copied from there, not downloaded.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::fetch::{Job, Store};
use crate::manifest::{Entry, Manifest, MAX_MANIFEST};
use crate::{hex, Error, Result};

const MANIFEST: &str = "manifest.json";
const CURRENT: &str = "current-";
const LAST_USED: &str = "last-used";
const PARTIAL: &str = ".partial-";
const REPAIR: &str = ".repair-";
/// Beside `versions/`: builds being copied there from another drive.
const MOVING: &str = ".moving-versions";

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

    /// The build the launcher starts on `channel`: the one named current there
    /// if it is installed, else the newest installed build of `channel`.
    pub fn current(&self, channel: mc_core::Channel) -> Option<Manifest> {
        let installed = self.installed();
        let named = std::fs::read_to_string(self.dir.join(format!("{CURRENT}{channel}"))).ok();
        let named = named.as_deref().map(str::trim);
        installed
            .iter()
            .find(|m| Some(m.key().as_str()) == named && m.channel() == Some(channel))
            .or_else(|| installed.iter().find(|m| m.channel() == Some(channel)))
            .cloned()
    }

    /// Makes `m` the build its channel starts.
    pub fn set_current(&self, m: &Manifest) -> Result<()> {
        let channel = m
            .channel()
            .ok_or_else(|| Error::Malformed(format!("channel {}", m.channel)))?;
        write_atomic(
            &self.dir.join(format!("{CURRENT}{channel}")),
            m.key().as_bytes(),
        )
    }

    /// Removes an installed build.
    pub fn remove(&self, key: &str) -> Result<()> {
        let dir = self.dir(key);
        if key.is_empty() || key.starts_with('.') || !dir.join(MANIFEST).is_file() {
            return Err(Error::Malformed(format!("{key} is not an installed build")));
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    /// Every installed build, newest first, with the bytes only it holds:
    /// what removing it frees, since a file shared with another build is one
    /// file on disk (a hard link). The total is what all of them take.
    pub fn usage(&self) -> (Vec<(Manifest, u64)>, u64) {
        let installed = self.installed();
        // Each file's size and how many builds hold it; a build holding the
        // same bytes twice counts once.
        let unique = |m: &Manifest| -> HashMap<String, u64> {
            m.files.iter().map(|e| (e.sha256.clone(), e.size)).collect()
        };
        let sets: Vec<HashMap<String, u64>> = installed.iter().map(unique).collect();
        let mut holders: HashMap<&str, (u64, usize)> = HashMap::new();
        for set in &sets {
            for (sha, size) in set {
                holders.entry(sha.as_str()).or_insert((*size, 0)).1 += 1;
            }
        }
        let total = holders.values().map(|(size, _)| size).sum();
        let own = installed
            .iter()
            .zip(&sets)
            .map(|(m, set)| {
                let alone = set
                    .iter()
                    .filter(|(sha, _)| holders.get(sha.as_str()).is_some_and(|h| h.1 == 1))
                    .map(|(_, size)| size)
                    .sum();
                (m.clone(), alone)
            })
            .collect();
        (own, total)
    }

    /// Reads every file of an installed build against its manifest; returns
    /// the ones missing or changed. `progress` counts bytes read.
    pub fn verify(
        &self,
        m: &Manifest,
        progress: &mut dyn FnMut(Progress),
        cancel: &AtomicBool,
    ) -> Result<Vec<Entry>> {
        let dir = self.dir(&m.key());
        let total = m.files.iter().map(|e| e.size).sum();
        let mut done = 0;
        let mut bad = Vec::new();
        for e in &m.files {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let path = dir.join(&e.path);
            let sound = path.metadata().is_ok_and(|md| md.len() == e.size)
                && hash_file(&path).is_ok_and(|h| h == e.sha256);
            if !sound {
                bad.push(e.clone());
            }
            done += e.size;
            progress(Progress { done, total });
        }
        Ok(bad)
    }

    /// Downloads `bad` (from [`Versions::verify`]) again and puts each in
    /// place. A file shared with another build gets a copy of its own: the
    /// other build keeps what it had.
    pub fn repair(
        &self,
        store: &Store,
        m: &Manifest,
        bad: &[Entry],
        progress: &mut dyn FnMut(Progress),
        cancel: &AtomicBool,
    ) -> Result<()> {
        let key = m.key();
        let dest = self.dir(&key);
        let stage = self.dir.join(format!("{REPAIR}{key}"));
        if stage.exists() {
            std::fs::remove_dir_all(&stage)?;
        }
        std::fs::create_dir_all(&stage)?;
        let mut jobs: Vec<Job> = bad
            .iter()
            .map(|e| Job {
                path: format!("blobs/{}.zst", e.sha256),
                dest: stage.join(format!("{}.zst", e.sha256)),
                max: e.packed,
            })
            .collect();
        jobs.sort_by(|a, b| a.path.cmp(&b.path));
        jobs.dedup_by(|a, b| a.path == b.path);
        let total = jobs.iter().map(|j| j.max).sum();
        progress(Progress { done: 0, total });
        store.download(
            &jobs,
            &mut |done| progress(Progress { done, total }),
            cancel,
        )?;
        for e in bad {
            let to = dest.join(&e.path);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let fresh = stage.join(format!("{}.file", e.sha256));
            unpack(
                &stage.join(format!("{}.zst", e.sha256)),
                &fresh,
                e.size,
                &e.sha256,
            )
            .map_err(|err| match err {
                Error::Hash(_) => Error::Hash(e.path.clone()),
                other => other,
            })?;
            // A rename gives this build a file of its own, where writing into
            // the old one would write into every build linked to it.
            std::fs::rename(&fresh, &to)?;
        }
        if bad.iter().any(|e| e.path == crate::game_exe()) {
            mark_executable(&self.exe(&key))?;
        }
        std::fs::remove_dir_all(&stage)?;
        Ok(())
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

    /// The most room installing `m` takes on its drive while it runs, in
    /// bytes: the files no installed build has, with their downloads beside
    /// them until they are unpacked.
    pub fn room_needed(&self, m: &Manifest) -> u64 {
        if self.has(&m.build) {
            return 0;
        }
        let local = self.local_copies();
        m.files
            .iter()
            .filter(|e| !local.contains_key(&e.sha256))
            .map(|e| e.size + e.packed)
            .sum()
    }

    /// Moves every installed build, with which build each channel starts, to
    /// the `versions/` folder of `root`, and returns the builds there. On one
    /// drive that is a rename; to another the builds are copied (a file two
    /// builds share is copied once and linked) into a staging folder that is
    /// renamed into place once whole, and only then are the old ones removed.
    /// `progress` counts bytes copied.
    pub fn move_to(
        &self,
        root: &Path,
        progress: &mut dyn FnMut(Progress),
        cancel: &AtomicBool,
    ) -> Result<Versions> {
        let to = Versions::new(root);
        if to.dir == self.dir {
            return Ok(to);
        }
        if to.dir.starts_with(&self.dir) {
            return Err(Error::Folder(format!(
                "{} is inside the builds' own folder",
                root.display()
            )));
        }
        if std::fs::read_dir(&to.dir).is_ok_and(|mut d| d.next().is_some()) {
            return Err(Error::Folder(format!(
                "{} already holds builds",
                to.dir.display()
            )));
        }
        std::fs::create_dir_all(root)?;
        if !self.dir.exists() {
            return Ok(to);
        }
        // An empty versions/ there would stop the rename.
        let _ = std::fs::remove_dir(&to.dir);
        if std::fs::rename(&self.dir, &to.dir).is_ok() {
            return Ok(to);
        }
        let stage = root.join(MOVING);
        if stage.exists() {
            std::fs::remove_dir_all(&stage)?;
        }
        std::fs::create_dir_all(&stage)?;
        if let Err(e) = self.copy_into(&stage, progress, cancel) {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(e);
        }
        std::fs::rename(&stage, &to.dir)?;
        // The builds are whole in their new place; what cannot be removed
        // from the old one is left behind, never half of the new.
        let _ = std::fs::remove_dir_all(&self.dir);
        Ok(to)
    }

    /// Copies the installed builds and the channels' current files into
    /// `stage`, a file shared between builds once.
    pub(crate) fn copy_into(
        &self,
        stage: &Path,
        progress: &mut dyn FnMut(Progress),
        cancel: &AtomicBool,
    ) -> Result<()> {
        let installed = self.installed();
        let mut sizes = HashMap::new();
        for m in &installed {
            for e in &m.files {
                sizes.insert(e.sha256.as_str(), e.size);
            }
        }
        let total = sizes.values().sum();
        let mut done = 0;
        progress(Progress { done, total });
        let mut copied: HashMap<String, PathBuf> = HashMap::new();
        for m in &installed {
            let (from, to) = (self.dir(&m.key()), stage.join(m.key()));
            for e in &m.files {
                let dest = to.join(&e.path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                if let Some(first) = copied.get(&e.sha256) {
                    if std::fs::hard_link(first, &dest).is_ok() {
                        continue;
                    }
                }
                copy_file(&from.join(&e.path), &dest, cancel, &mut |n| {
                    progress(Progress {
                        done: (done + n).min(total),
                        total,
                    })
                })?;
                done += e.size;
                copied.insert(e.sha256.clone(), dest);
            }
            // The rest of its folder: its manifest, and when it was last used.
            copy_rest(&from, &to)?;
        }
        for e in std::fs::read_dir(&self.dir)?.flatten() {
            if e.file_name().to_string_lossy().starts_with(CURRENT) {
                std::fs::copy(e.path(), stage.join(e.file_name()))?;
            }
        }
        Ok(())
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
    /// one named in `spare`, and any half-installed build or repair.
    pub fn prune(&self, keep: usize, spare: &[String]) {
        for e in std::fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(PARTIAL) || name.starts_with(REPAIR) {
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

/// Copies `from` to `to`, telling `progress` the bytes copied so far.
fn copy_file(
    from: &Path,
    to: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<()> {
    let mut input = File::open(from)?;
    let mut out = BufWriter::new(File::create(to)?);
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        match input.read(&mut buf) {
            Ok(0) => break,
            Ok(k) => {
                out.write_all(&buf[..k])?;
                n += k as u64;
                progress(n);
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    out.flush()?;
    drop(out);
    // The game stays executable.
    std::fs::set_permissions(to, from.metadata()?.permissions())?;
    Ok(())
}

/// Copies what of `from` is not yet in `to`, keeping each file's modified
/// time (`last-used` orders the builds by it).
fn copy_rest(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let (src, dest) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_rest(&src, &dest)?;
        } else if !dest.exists() {
            std::fs::copy(&src, &dest)?;
            if let Ok(when) = e.metadata().and_then(|md| md.modified()) {
                File::options()
                    .write(true)
                    .open(&dest)?
                    .set_modified(when)?;
            }
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> io::Result<String> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        match input.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hash.update(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(hex(&hash.finalize()))
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
