//! The build store: every published build, kept so that any replay can be
//! played by the build that recorded it (docs/RELEASES.md).
//!
//! A store is a plain tree of files behind a URL (Cloudflare R2 for published
//! builds, a folder for tests):
//!
//! ```text
//! blobs/<sha256>.zst                       one file's bytes, zstd-packed; shared by
//!                                          every build that has that file
//! builds/<platform>/<key>.json (+ .sig)    a build's manifest: its files by hash
//! channels/<channel>/<platform>.json (+ .sig)   the channel's newest build
//! ```
//!
//! Manifests are signed with the release key; a build only trusts manifests
//! that verify against the public key it was built with (`signing.pub`), and
//! every file it downloads is checked against its manifest's hash.
//!
//! - [`manifest`]: what a build is made of, signed.
//! - [`fetch`]: reading a store (through `curl`, on every supported system).
//! - [`versions`]: the builds installed side by side under an install's `versions/`.
//! - [`pack`]: writing a build into a store (the publishing side, `mc-release`).

pub mod fetch;
pub mod manifest;
pub mod pack;
pub mod versions;

use std::fmt;
use std::io;

pub use fetch::Store;
pub use manifest::{Entry, Manifest};
pub use versions::Versions;

/// The store published builds of this channel read from, set at compile time
/// (`MERIDIAN_STORE`, by scripts/release.sh). A dev build has none.
pub const STORE_URL: Option<&str> = option_env!("MERIDIAN_STORE");

/// The release key's public half, as 64 hex digits (`mc-release keygen`).
const PUBLIC_KEY: &str = include_str!("../signing.pub");

/// What this machine runs: a build is published once per platform.
pub const fn platform() -> &'static str {
    if cfg!(windows) {
        "windows-x64"
    } else if cfg!(target_os = "macos") {
        "macos-arm64"
    } else {
        "linux-x64"
    }
}

/// The game's executable inside a build.
pub fn game_exe() -> String {
    format!("meridian{}", std::env::consts::EXE_SUFFIX)
}

/// A build's name as a file and store key: `+` and anything else a URL or a
/// file system might read differently becomes `_`.
pub fn key_of(build: &str) -> String {
    build
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' => c,
            _ => '_',
        })
        .collect()
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// The store could not be reached, or did not have the file.
    Fetch(String),
    /// A manifest's signature does not verify: not published by us.
    Signature,
    /// Something read from the store or an install is not well formed.
    Malformed(String),
    /// A file's bytes are not the ones its manifest names.
    Hash(String),
    /// This build was made without a store to read from.
    NoStore,
    Cancelled,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Fetch(e) => write!(f, "download failed: {e}"),
            Error::Signature => {
                f.write_str("the build's manifest is not signed by the release key")
            }
            Error::Malformed(e) => write!(f, "malformed: {e}"),
            Error::Hash(path) => write!(f, "{path} did not match its manifest"),
            Error::NoStore => f.write_str("this build has no build store to download from"),
            Error::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let text = text.trim();
    if text.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_plain() {
        assert_eq!(key_of("0.1.0-playtest+0123abcd"), "0.1.0-playtest_0123abcd");
        assert_eq!(key_of("../x"), ".._x");
    }

    #[test]
    fn hex_round_trips() {
        let b = [0u8, 1, 0xab, 0xff];
        assert_eq!(unhex::<4>(&hex(&b)), Some(b));
        assert_eq!(unhex::<4>("00"), None);
        assert_eq!(unhex::<1>("zz"), None);
    }

    #[test]
    fn the_public_key_is_a_key() {
        let key = unhex::<32>(PUBLIC_KEY).expect("signing.pub holds 64 hex digits");
        assert!(ed25519_dalek::VerifyingKey::from_bytes(&key).is_ok());
    }
}
