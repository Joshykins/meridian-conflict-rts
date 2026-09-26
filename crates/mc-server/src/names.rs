//! Which device key owns which name: `names.json` in the data directory.
//!
//! A name belongs to the first key that signs in with it, compared without
//! regard to case. The same key may later sign in with other casing, which
//! becomes the name shown. The file maps the lower-case name to its record:
//!
//! ```json
//! { "ada": { "display": "Ada", "key": "3b6a...", "first_seen": 1790000000,
//!            "last_seen": 1790003600, "banned": false } }
//! ```
//!
//! An operator bars a name by stopping the server, setting its `banned` to
//! `true` and starting it again.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use mc_net::DirRefuseReason;

/// Sign-in times are written at most this often; a claim is written at once.
const SAVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Serialize, Deserialize)]
struct Record {
    display: String,
    /// The owner's public key, 64 hex digits.
    key: String,
    first_seen: u64,
    last_seen: u64,
    #[serde(default)]
    banned: bool,
}

pub(crate) struct Names {
    path: PathBuf,
    records: BTreeMap<String, Record>,
    /// Holds changes not yet written, since this moment.
    dirty_since: Option<Instant>,
}

pub(crate) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl Names {
    /// No file is an empty registry. A file that does not read is an error: the
    /// server must not start and hand its players' names to whoever comes first.
    pub(crate) fn load(path: &Path) -> io::Result<Names> {
        let records = match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{} does not read: {e}", path.display()),
                )
            })?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e),
        };
        Ok(Names {
            path: path.to_owned(),
            records,
            dirty_since: None,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    /// Records a sign-in by `key` as `name`, which [`mc_net::check_name`] has passed.
    pub(crate) fn sign_in(&mut self, name: &str, key: &[u8; 32]) -> Result<(), DirRefuseReason> {
        let now = unix_now();
        let key = hex(key);
        match self.records.get_mut(&name.to_ascii_lowercase()) {
            Some(r) if r.banned => Err(DirRefuseReason::Banned),
            Some(r) if r.key != key => Err(DirRefuseReason::NameTaken),
            Some(r) => {
                r.display = name.to_owned();
                r.last_seen = now;
                self.dirty_since.get_or_insert_with(Instant::now);
                Ok(())
            }
            None => {
                self.records.insert(
                    name.to_ascii_lowercase(),
                    Record {
                        display: name.to_owned(),
                        key,
                        first_seen: now,
                        last_seen: now,
                        banned: false,
                    },
                );
                log::info!("{name} claimed");
                // A claim must survive a crash: write it now.
                if let Err(e) = self.save() {
                    log::error!("could not save the claim of {name}: {e}");
                }
                Ok(())
            }
        }
    }

    /// Writes the changes of the last minute or more.
    pub(crate) fn save_if_due(&mut self) {
        if self
            .dirty_since
            .is_some_and(|at| at.elapsed() >= SAVE_INTERVAL)
        {
            if let Err(e) = self.save() {
                log::error!("could not save the names: {e}");
            }
        }
    }

    /// Writes a temporary file and renames it over the old one: a crash leaves one
    /// or the other, never half of either.
    pub(crate) fn save(&mut self) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(&self.records).map_err(io::Error::other)?;
        let mut tmp = self.path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, &self.path)?;
        self.dirty_since = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_belongs_to_its_first_key() {
        let dir = std::env::temp_dir().join(format!("mc-names-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("names.json");
        let _ = fs::remove_file(&path);
        let mut names = Names::load(&path).unwrap();
        assert_eq!(names.sign_in("Ada", &[1; 32]), Ok(()));
        assert_eq!(
            names.sign_in("ADA", &[2; 32]),
            Err(DirRefuseReason::NameTaken)
        );
        assert_eq!(names.sign_in("aDa", &[1; 32]), Ok(()));
        names.save().unwrap();

        // The claim and the latest casing survive a restart.
        let mut names = Names::load(&path).unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(names.records["ada"].display, "aDa");
        assert_eq!(
            names.sign_in("ada", &[2; 32]),
            Err(DirRefuseReason::NameTaken)
        );

        // An operator's ban holds against everyone.
        names.records.get_mut("ada").unwrap().banned = true;
        assert_eq!(names.sign_in("Ada", &[1; 32]), Err(DirRefuseReason::Banned));
        assert_eq!(names.sign_in("Ada", &[2; 32]), Err(DirRefuseReason::Banned));

        fs::write(&path, b"{ not json").unwrap();
        assert!(Names::load(&path).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
