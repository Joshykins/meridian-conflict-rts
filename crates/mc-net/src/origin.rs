//! Who recorded a replay: the first thing in the file after the magic and the
//! format version, from format 27 on.
//!
//! The rest of a replay changes with the format; this block does not. Every
//! later build can read it, so the newest game can say of any replay which
//! build plays it, and fetch that build (docs/RELEASES.md). Its layout is fixed
//! for good:
//!
//! ```text
//! "MCRP"  u32 format version  u32 origin length  origin
//! origin: fields, each  u8 tag  u16 length  payload; unknown tags are skipped
//!     1 build      UTF-8, e.g. 0.1.0-playtest+0123456789
//!     2 commit     UTF-8, the full commit hash; absent outside a git checkout
//!     3 channel    u8, mc_core::Channel
//!     4 sim        u64, the simulation fingerprint (MERIDIAN_SIM)
//!     5 map        u64, the map's content id
//!     6 blueprints u64, the unit data's content hash
//! ```
//!
//! New fields get new tags. A tag's meaning never changes, and a retired tag
//! is never reused.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use mc_core::Channel;

use crate::protocol::{ContentId, MAX_BUILD_LEN, MAX_FRAME_LEN};
use crate::replay::{MAGIC, REC_BUILD_LEGACY};
use crate::wire::{Dec, Enc, NetError};

/// The first format with an origin block.
pub const ORIGIN_SINCE: u32 = 27;
const MAX_ORIGIN_LEN: usize = 4096;

const TAG_BUILD: u8 = 1;
const TAG_COMMIT: u8 = 2;
const TAG_CHANNEL: u8 = 3;
const TAG_SIM: u8 = 4;
const TAG_MAP: u8 = 5;
const TAG_BLUEPRINTS: u8 = 6;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Origin {
    /// The build's name (`MERIDIAN_BUILD`).
    pub build: String,
    /// The full commit hash, empty when not known.
    pub commit: String,
    pub channel: Option<Channel>,
    /// The simulation fingerprint (`MERIDIAN_SIM`), 0 when not known.
    pub sim: u64,
    pub content: ContentId,
}

/// What [`Origin::peek`] found at the top of a file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Peeked {
    /// The replay format the rest of the file is in.
    pub format: u32,
    /// From the origin block, or for an older format, as much as its header
    /// and build record say.
    pub origin: Origin,
}

impl Origin {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut e = Enc::new();
        let mut field = |tag: u8, payload: &[u8]| {
            e.u8(tag);
            e.u16(payload.len() as u16);
            e.raw(payload);
        };
        field(TAG_BUILD, self.build.as_bytes());
        if !self.commit.is_empty() {
            field(TAG_COMMIT, self.commit.as_bytes());
        }
        if let Some(c) = self.channel {
            field(TAG_CHANNEL, &[c as u8]);
        }
        if self.sim != 0 {
            field(TAG_SIM, &self.sim.to_le_bytes());
        }
        field(TAG_MAP, &self.content.map_id.to_le_bytes());
        field(TAG_BLUEPRINTS, &self.content.blueprint_hash.to_le_bytes());
        e.buf
    }

    pub(crate) fn validate(&self) -> io::Result<()> {
        if self.build.len() > MAX_BUILD_LEN || self.commit.len() > MAX_BUILD_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "build name or commit too long",
            ));
        }
        Ok(())
    }

    pub(crate) fn decode(buf: &[u8]) -> Result<Origin, NetError> {
        let mut d = Dec::new(buf);
        let mut o = Origin::default();
        while d.remaining() > 0 {
            let tag = d.u8()?;
            let len = d.u16()? as usize;
            let payload = d.take_raw(len)?;
            let text = || {
                std::str::from_utf8(payload)
                    .map(str::to_owned)
                    .map_err(|_| NetError::Malformed("origin text is not UTF-8"))
            };
            let word = || -> Result<u64, NetError> {
                let bytes: [u8; 8] = payload
                    .try_into()
                    .map_err(|_| NetError::Malformed("origin field has the wrong length"))?;
                Ok(u64::from_le_bytes(bytes))
            };
            match tag {
                TAG_BUILD => o.build = text()?,
                TAG_COMMIT => o.commit = text()?,
                TAG_CHANNEL => o.channel = payload.first().copied().and_then(Channel::from_u8),
                TAG_SIM => o.sim = word()?,
                TAG_MAP => o.content.map_id = word()?,
                TAG_BLUEPRINTS => o.content.blueprint_hash = word()?,
                _ => {}
            }
        }
        if o.build.len() > MAX_BUILD_LEN || o.commit.len() > MAX_BUILD_LEN {
            return Err(NetError::Malformed("origin text over its limit"));
        }
        Ok(o)
    }

    /// The commit a build name carries after its `+`, for replays older than
    /// the origin block (and builds outside a checkout, which have none).
    pub fn commit_of_build(build: &str) -> Option<&str> {
        let hash = build.rsplit_once('+')?.1;
        (hash.len() >= 7 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hash)
    }

    /// Reads who recorded a replay without reading the replay: works on every
    /// format from 27 on, including formats newer than this build's, and on
    /// older ones as far as they say (the build record, the content ids).
    pub fn peek_file(path: impl AsRef<Path>) -> Result<Peeked, NetError> {
        Origin::peek(BufReader::new(File::open(path)?))
    }

    pub fn peek(mut input: impl Read) -> Result<Peeked, NetError> {
        let mut head = [0u8; 8];
        input.read_exact(&mut head)?;
        if head[..4] != MAGIC {
            return Err(NetError::Malformed("not a replay file"));
        }
        let format = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
        if format >= ORIGIN_SINCE {
            let buf = read_block(&mut input, MAX_ORIGIN_LEN)?;
            return Ok(Peeked {
                format,
                origin: Origin::decode(&buf)?,
            });
        }
        // Before the origin block: the start message opens with the content
        // ids, and the build, when named, is the first record after it.
        let header = read_block(&mut input, MAX_FRAME_LEN)?;
        let mut d = Dec::new(&header);
        let mut origin = Origin {
            content: ContentId {
                map_id: d.u64()?,
                blueprint_hash: d.u64()?,
            },
            ..Origin::default()
        };
        let mut tag = [0u8; 1];
        if input.read_exact(&mut tag).is_ok() && tag[0] == REC_BUILD_LEGACY {
            let name = read_block(&mut input, MAX_BUILD_LEN)?;
            origin.build = String::from_utf8_lossy(&name).into_owned();
            origin.commit = Origin::commit_of_build(&origin.build)
                .unwrap_or_default()
                .to_owned();
        }
        Ok(Peeked { format, origin })
    }
}

/// A `u32` length, then that many bytes.
pub(crate) fn read_block(input: &mut impl Read, max: usize) -> Result<Vec<u8>, NetError> {
    let mut len = [0u8; 4];
    input.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > max {
        return Err(NetError::FrameTooLarge { len, max });
    }
    let mut buf = vec![0u8; len];
    input.read_exact(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Origin {
        Origin {
            build: "0.1.0-playtest+0123456789".into(),
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            channel: Some(Channel::Playtest),
            sim: 0xfeed_beef,
            content: ContentId {
                map_id: 1,
                blueprint_hash: 2,
            },
        }
    }

    #[test]
    fn round_trip_and_unknown_fields_skipped() {
        let o = sample();
        let mut bytes = o.encode();
        assert_eq!(Origin::decode(&bytes).unwrap(), o);
        // A field from a later build.
        bytes.extend_from_slice(&[99, 3, 0, 1, 2, 3]);
        assert_eq!(Origin::decode(&bytes).unwrap(), o);
    }

    #[test]
    fn commit_from_a_build_name() {
        assert_eq!(
            Origin::commit_of_build("0.1.0+e9fea68abc"),
            Some("e9fea68abc")
        );
        assert_eq!(Origin::commit_of_build("0.1.0-dev"), None);
        assert_eq!(Origin::commit_of_build("0.1.0+nothex!"), None);
    }

    #[test]
    fn damaged_origins_are_errors() {
        assert!(Origin::decode(&[TAG_SIM, 3, 0, 1, 2, 3]).is_err());
        assert!(Origin::decode(&[TAG_BUILD, 2, 0, 0xff, 0xfe]).is_err());
        assert!(Origin::decode(&[TAG_BUILD, 9, 0, 1]).is_err());
    }
}
