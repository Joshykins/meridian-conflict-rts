//! A build's manifest: its identity and every file in it by hash, signed with
//! the release key.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use mc_core::Channel;
use serde::{Deserialize, Serialize};

use crate::{hex, unhex, Error, Result, PUBLIC_KEY};

/// The most a manifest may be, read from a store or an install.
pub const MAX_MANIFEST: u64 = 4 << 20;
const MAX_FILES: usize = 20_000;
const MAX_FILE: u64 = 4 << 30;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// `MERIDIAN_BUILD`.
    pub build: String,
    /// `MERIDIAN_BUILD_NUMBER`: which of two builds of a channel is newer.
    pub number: u32,
    pub channel: String,
    pub commit: String,
    /// `MERIDIAN_SIM`, 16 hex digits.
    pub sim: String,
    pub platform: String,
    /// When it was published, in Unix seconds.
    pub created: u64,
    pub files: Vec<Entry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Relative, `/`-separated.
    pub path: String,
    pub size: u64,
    /// 64 hex digits; the blob is `blobs/<sha256>.zst`.
    pub sha256: String,
    /// The blob's size: what downloading this file costs.
    pub packed: u64,
}

impl Manifest {
    pub fn channel(&self) -> Option<Channel> {
        self.channel.parse().ok()
    }

    pub fn key(&self) -> String {
        crate::key_of(&self.build)
    }

    /// Where the manifest is kept in a store.
    pub fn store_path(platform: &str, build: &str) -> String {
        format!("builds/{platform}/{}.json", crate::key_of(build))
    }

    /// Where a channel's newest build is named in a store.
    pub fn channel_path(channel: Channel, platform: &str) -> String {
        format!("channels/{channel}/{platform}.json")
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = serde_json::to_vec_pretty(self).expect("a manifest serialises");
        out.push(b'\n');
        out
    }

    /// Parses and checks a manifest that is already trusted (verified, or
    /// written by this machine).
    pub fn parse(bytes: &[u8]) -> Result<Manifest> {
        if bytes.len() as u64 > MAX_MANIFEST {
            return Err(Error::Malformed("manifest too large".into()));
        }
        let m: Manifest =
            serde_json::from_slice(bytes).map_err(|e| Error::Malformed(e.to_string()))?;
        m.check()?;
        Ok(m)
    }

    /// Checks the signature against the release key, then parses.
    pub fn verify(bytes: &[u8], signature: &[u8]) -> Result<Manifest> {
        verify_with(bytes, signature, &public_key())?;
        Manifest::parse(bytes)
    }

    /// The signature file's contents for `bytes`.
    pub fn sign(bytes: &[u8], key: &SigningKey) -> String {
        hex(&key.sign(bytes).to_bytes()) + "\n"
    }

    /// What a build from a store may hold: relative paths that stay inside
    /// its folder, each once, hashes that are hashes, sizes within reason.
    fn check(&self) -> Result<()> {
        let bad = |why: &str| Err(Error::Malformed(format!("manifest {}: {why}", self.build)));
        if self.build.is_empty() || self.build.len() > 64 {
            return bad("build name");
        }
        if self.files.len() > MAX_FILES {
            return bad("too many files");
        }
        let mut seen = BTreeSet::new();
        for e in &self.files {
            let p = Path::new(&e.path);
            let plain = !e.path.is_empty()
                && !e.path.contains('\\')
                && !e.path.contains(':')
                && p.components().all(|c| matches!(c, Component::Normal(_)));
            if !plain {
                return bad(&format!("path {:?}", e.path));
            }
            if !seen.insert(e.path.to_ascii_lowercase()) {
                return bad(&format!("{} twice", e.path));
            }
            if unhex::<32>(&e.sha256).is_none() || e.sha256 != e.sha256.to_ascii_lowercase() {
                return bad(&format!("hash of {}", e.path));
            }
            if e.size > MAX_FILE || e.packed > MAX_FILE {
                return bad(&format!("size of {}", e.path));
            }
        }
        if !self.files.iter().any(|e| e.path == crate::game_exe())
            && self.platform == crate::platform()
        {
            return bad("no game executable");
        }
        Ok(())
    }

    /// What downloading the files a filter keeps costs.
    pub fn packed_size(&self, mut keep: impl FnMut(&Entry) -> bool) -> u64 {
        self.files
            .iter()
            .filter(|e| keep(e))
            .map(|e| e.packed)
            .sum()
    }
}

pub fn public_key() -> VerifyingKey {
    let bytes = unhex::<32>(PUBLIC_KEY).expect("signing.pub holds 64 hex digits");
    VerifyingKey::from_bytes(&bytes).expect("signing.pub holds a public key")
}

fn verify_with(bytes: &[u8], signature: &[u8], key: &VerifyingKey) -> Result<()> {
    let text = std::str::from_utf8(signature).map_err(|_| Error::Signature)?;
    let sig = unhex::<64>(text).ok_or(Error::Signature)?;
    key.verify_strict(bytes, &Signature::from_bytes(&sig))
        .map_err(|_| Error::Signature)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample() -> Manifest {
        Manifest {
            build: "0.1.0-playtest+0123456789".into(),
            number: 7,
            channel: "playtest".into(),
            commit: "0123456789".into(),
            sim: "00000000000000ff".into(),
            platform: crate::platform().into(),
            created: 1,
            files: vec![Entry {
                path: crate::game_exe(),
                size: 3,
                sha256: "ab".repeat(32),
                packed: 2,
            }],
        }
    }

    #[test]
    fn signed_manifests_verify_and_tampered_ones_do_not() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let bytes = sample().to_bytes();
        let sig = Manifest::sign(&bytes, &key);
        verify_with(&bytes, sig.as_bytes(), &key.verifying_key()).unwrap();
        let mut tampered = bytes.clone();
        tampered[10] ^= 1;
        assert!(verify_with(&tampered, sig.as_bytes(), &key.verifying_key()).is_err());
        let other = SigningKey::from_bytes(&[8; 32]);
        assert!(verify_with(&bytes, sig.as_bytes(), &other.verifying_key()).is_err());
        assert!(verify_with(&bytes, b"not hex", &key.verifying_key()).is_err());
        // Not signed by the release key either.
        assert!(matches!(
            Manifest::verify(&bytes, sig.as_bytes()),
            Err(Error::Signature)
        ));
    }

    #[test]
    fn paths_stay_inside_the_build() {
        assert!(Manifest::parse(&sample().to_bytes()).is_ok());
        for path in ["../evil", "/etc/passwd", "a/../../b", "C:/x", "a\\b", ""] {
            let mut m = sample();
            m.files.push(Entry {
                path: path.into(),
                ..m.files[0].clone()
            });
            assert!(Manifest::parse(&m.to_bytes()).is_err(), "{path}");
        }
        let mut twice = sample();
        twice.files.push(twice.files[0].clone());
        assert!(Manifest::parse(&twice.to_bytes()).is_err());
        let mut bad_hash = sample();
        bad_hash.files[0].sha256 = "x".repeat(64);
        assert!(Manifest::parse(&bad_hash.to_bytes()).is_err());
        assert!(Manifest::parse(b"{").is_err());
    }
}
