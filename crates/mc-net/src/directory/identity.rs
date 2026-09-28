//! A player's device key: made once, kept beside the settings, and used to
//! answer the server's sign-in challenge.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use super::NONCE_LEN;

/// The file name the game keeps its identity under, in the settings directory.
pub const IDENTITY_FILE: &str = "identity.key";

/// What a sign-in signature covers besides the nonce, so that it proves nothing
/// anywhere else the key might be used.
const SIGN_IN_CONTEXT: &[u8] = b"Meridian Conflict directory sign-in v1\0";

/// An ed25519 signing key. It never leaves the machine: the server sees the
/// public key and signatures only.
#[derive(Clone)]
pub struct Identity {
    key: SigningKey,
}

impl Identity {
    /// A new key from OS randomness.
    pub fn generate() -> io::Result<Identity> {
        Ok(Identity::from_bytes(random_bytes()?))
    }

    /// The key whose 32-byte secret is `secret`.
    pub fn from_bytes(secret: [u8; 32]) -> Identity {
        Identity {
            key: SigningKey::from_bytes(&secret),
        }
    }

    /// The 32-byte secret. Whoever has it can sign in under this player's names.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.key.to_bytes()
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    /// Answers a sign-in challenge.
    pub fn sign(&self, nonce: &[u8; NONCE_LEN]) -> [u8; 64] {
        self.key.sign(&sign_in_message(nonce)).to_bytes()
    }

    /// Short and readable, for a player to compare: `3fa2-91c0`.
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.public_key())
    }

    /// Reads the identity kept at `path`, or makes one and keeps it there. The
    /// file holds the secret as 64 hex digits; it is written whole or not at all,
    /// readable by its owner only. A file that is there but damaged is an error,
    /// never replaced: that would lose the player's names on every server.
    pub fn load_or_create(path: &Path) -> io::Result<Identity> {
        match fs::read(path) {
            Ok(bytes) => parse_secret(&bytes)
                .map(Identity::from_bytes)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{} is not an identity file", path.display()),
                    )
                }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let identity = Identity::generate()?;
                write_private(path, format!("{}\n", hex(&identity.to_bytes())).as_bytes())?;
                Ok(identity)
            }
            Err(e) => Err(e),
        }
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Identity({})", self.fingerprint())
    }
}

/// The fingerprint of a public key: the first four bytes of its SHA-256, as
/// two groups of four hex digits.
pub fn fingerprint(public_key: &[u8; 32]) -> String {
    let digest = Sha256::digest(public_key);
    let h = hex(&digest[..4]);
    format!("{}-{}", &h[..4], &h[4..])
}

/// Whether `signature` answers the challenge `nonce` for the key `public_key`.
pub fn verify_proof(public_key: &[u8; 32], nonce: &[u8; NONCE_LEN], signature: &[u8; 64]) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(public_key) else {
        return false;
    };
    key.verify_strict(&sign_in_message(nonce), &Signature::from_bytes(signature))
        .is_ok()
}

/// `N` bytes from the operating system's random source.
pub fn random_bytes<const N: usize>() -> io::Result<[u8; N]> {
    let mut out = [0u8; N];
    getrandom::fill(&mut out).map_err(|e| io::Error::other(format!("no OS randomness: {e}")))?;
    Ok(out)
}

fn sign_in_message(nonce: &[u8; NONCE_LEN]) -> Vec<u8> {
    [SIGN_IN_CONTEXT, nonce].concat()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_secret(bytes: &[u8]) -> Option<[u8; 32]> {
    let text = std::str::from_utf8(bytes).ok()?.trim();
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Writes a temporary file beside `path` (owner-only on Unix), then renames it
/// over `path`, so a crash leaves the old file or the new one, never half.
fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = Path::new(&tmp);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    // A leftover from a crash would make `create_new` fail; it holds nothing we need.
    let _ = fs::remove_file(tmp);
    let written = options.open(tmp).and_then(|mut f| {
        f.write_all(contents)?;
        f.sync_all()
    });
    match written.and_then(|()| fs::rename(tmp, path)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(tmp);
            Err(e)
        }
    }
}
