//! Writing builds into a store folder (`mc-release pack`), which
//! scripts/release.sh then copies up to the published store.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ed25519_dalek::SigningKey;
use mc_core::Channel;
use sha2::{Digest, Sha256};

use crate::manifest::{Entry, Manifest};
use crate::{hex, unhex, Error, Result};

/// zstd's level for blobs: slow to pack, once, and small to download, often.
const LEVEL: i32 = 19;

/// What a build says of itself (`meridian --version`).
pub struct Identity {
    pub build: String,
    pub number: u32,
    pub channel: Channel,
    pub commit: String,
    pub sim: String,
    pub platform: String,
}

/// What packing a build did.
pub struct Packed {
    pub manifest: Manifest,
    /// Blobs the store did not have yet.
    pub new_blobs: usize,
    pub new_bytes: u64,
}

/// Packs every file under `stage` into `store`'s blobs (those it lacks) and
/// returns the build's manifest. Nothing is signed or named yet: [`publish`].
pub fn pack(stage: &Path, store: &Path, id: Identity, created: u64) -> Result<Packed> {
    let mut files = Vec::new();
    walk(stage, &mut files)?;
    std::fs::create_dir_all(store.join("blobs"))?;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = files.len().div_ceil(threads).max(1);
    let results: Vec<Result<Vec<PackedFile>>> = std::thread::scope(|s| {
        let handles: Vec<_> = files
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(|f| pack_one(stage, store, f)).collect()))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("packing thread panicked"))
            .collect()
    });
    let mut entries = Vec::new();
    let mut fresh_blobs = std::collections::BTreeMap::new();
    for r in results {
        for (entry, fresh) in r? {
            if let Some(n) = fresh {
                fresh_blobs.insert(entry.sha256.clone(), n);
            }
            entries.push(entry);
        }
    }
    let (new_blobs, new_bytes) = (fresh_blobs.len(), fresh_blobs.values().sum());
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let manifest = Manifest {
        build: id.build,
        number: id.number,
        channel: id.channel.name().to_owned(),
        commit: id.commit,
        sim: id.sim,
        platform: id.platform,
        created,
        files: entries,
    };
    // What a player's install will refuse, refuse here.
    Manifest::parse(&manifest.to_bytes())?;
    Ok(Packed {
        manifest,
        new_blobs,
        new_bytes,
    })
}

/// A file's entry, and its blob's size when the blob is new.
type PackedFile = (Entry, Option<u64>);

/// Hashes one file and packs its blob if the store lacks it; the blob's size
/// is returned when it is new.
fn pack_one(stage: &Path, store: &Path, file: &Path) -> Result<PackedFile> {
    let rel = file
        .strip_prefix(stage)
        .map_err(|_| Error::Malformed(format!("{} is outside the stage", file.display())))?;
    let path = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let mut hash = Sha256::new();
    let mut input = BufReader::new(File::open(file)?);
    let mut buf = vec![0u8; 1 << 16];
    let mut size = 0u64;
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        hash.update(&buf[..n]);
    }
    let sha256 = hex(&hash.finalize());
    let blob = store.join("blobs").join(format!("{sha256}.zst"));
    let fresh = if blob.exists() {
        None
    } else {
        // Two files with the same bytes may be packed at once: each into its own name.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let tmp = blob.with_extension(format!("zst.{}-{n}", std::process::id()));
        zstd::stream::copy_encode(
            BufReader::new(File::open(file)?),
            File::create(&tmp)?,
            LEVEL,
        )?;
        std::fs::rename(&tmp, &blob)?;
        Some(blob.metadata()?.len())
    };
    let packed = blob.metadata()?.len();
    Ok((
        Entry {
            path,
            size,
            sha256,
            packed,
        },
        fresh,
    ))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<_>>()?;
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out)?;
        } else {
            out.push(p);
        }
    }
    Ok(())
}

/// Signs `m` into the store: its own manifest always, and with `newest` also
/// as its channel's newest build (written last, so a player never sees a
/// channel naming a build whose manifest is not there yet).
pub fn publish(store: &Path, m: &Manifest, key: &SigningKey, newest: bool) -> Result<()> {
    let bytes = m.to_bytes();
    let sig = Manifest::sign(&bytes, key);
    let mut paths = vec![Manifest::store_path(&m.platform, &m.build)];
    if newest {
        let channel = m
            .channel()
            .ok_or_else(|| Error::Malformed(format!("channel {}", m.channel)))?;
        paths.push(Manifest::channel_path(channel, &m.platform));
    }
    for p in paths {
        let path = store.join(&p);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, &bytes)?;
        std::fs::write(store.join(format!("{p}.sig")), &sig)?;
    }
    Ok(())
}

/// Makes a new release key at `path` (which must not exist) and returns the
/// public half as hex, for `signing.pub`.
pub fn keygen(path: &Path) -> Result<String> {
    if path.exists() {
        return Err(Error::Malformed(format!(
            "{} already exists: a new key would orphan every build signed with it",
            path.display()
        )));
    }
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|e| Error::Malformed(format!("no OS randomness: {e}")))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, hex(&secret) + "\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(hex(SigningKey::from_bytes(&secret)
        .verifying_key()
        .as_bytes()))
}

pub fn load_key(path: &Path) -> Result<SigningKey> {
    let text = std::fs::read_to_string(path)?;
    let secret = unhex::<32>(&text)
        .ok_or_else(|| Error::Malformed(format!("{}: not a release key", path.display())))?;
    let key = SigningKey::from_bytes(&secret);
    if key.verifying_key() != crate::manifest::public_key() {
        return Err(Error::Malformed(format!(
            "{} is not the key signing.pub names",
            path.display()
        )));
    }
    Ok(key)
}

#[cfg(test)]
mod tests;
