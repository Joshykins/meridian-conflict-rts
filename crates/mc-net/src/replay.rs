//! The `.mcreplay` file: a [`MatchStart`] header followed by the bundle log.
//!
//! ```text
//! "MCRP"  u32 format version  u32 header length  MatchStart
//! then records:  u8 tag  u32 length  payload
//!     1 Bundle   one TickBundle; ticks are contiguous from 0
//!     2 Hash     u32 tick, u64 agreed state hash (optional, any subset of ticks)
//!     3 End      u32 tick count; the match finished cleanly
//!     4 Held     one TickBundle given while the clock was held; its `tick` is the
//!                next tick to run, and its commands are carried out before it
//!     5 Build    UTF-8 name of the build that recorded it (optional, first)
//!     6 Note     u32 tick, bytes: something the recording machine showed rather
//!                than simulated (the test range's weather), opaque to this crate;
//!                `tick` is the next tick to run when it changed
//! ```
//!
//! Records are appended as the match runs, so a crash leaves a replay that is
//! valid up to the last complete record. Unknown record tags are skipped.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use crate::protocol::{MatchStart, TickBundle, MAX_FRAME_LEN};
use crate::wire::{Dec, Enc, NetError};

pub const REPLAY_FORMAT_VERSION: u32 = 26;
pub const REPLAY_EXTENSION: &str = "mcreplay";

const MAGIC: [u8; 4] = *b"MCRP";
const REC_BUNDLE: u8 = 1;
const REC_HASH: u8 = 2;
const REC_END: u8 = 3;
const REC_HELD: u8 = 4;
const REC_BUILD: u8 = 5;
const REC_NOTE: u8 = 6;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReplayRecord {
    Bundle(TickBundle),
    Hash { tick: u32, hash: u64 },
    End { ticks: u32 },
    Held(TickBundle),
    Build(String),
    Note { tick: u32, note: Vec<u8> },
}

pub struct ReplayWriter<W: Write> {
    out: W,
    next_tick: u32,
    finished: bool,
}

impl ReplayWriter<BufWriter<File>> {
    pub fn create(path: impl AsRef<Path>, start: &MatchStart) -> io::Result<Self> {
        ReplayWriter::new(BufWriter::new(File::create(path)?), start)
    }
}

impl<W: Write> ReplayWriter<W> {
    pub fn new(mut out: W, start: &MatchStart) -> io::Result<Self> {
        start.validate()?;
        let mut e = Enc::new();
        start.encode(&mut e);
        out.write_all(&MAGIC)?;
        out.write_all(&REPLAY_FORMAT_VERSION.to_le_bytes())?;
        out.write_all(&(e.buf.len() as u32).to_le_bytes())?;
        out.write_all(&e.buf)?;
        Ok(ReplayWriter {
            out,
            next_tick: 0,
            finished: false,
        })
    }

    fn record(&mut self, tag: u8, payload: &[u8]) -> io::Result<()> {
        if payload.len() > MAX_FRAME_LEN {
            return Err(NetError::FrameTooLarge {
                len: payload.len(),
                max: MAX_FRAME_LEN,
            }
            .into());
        }
        self.out.write_all(&[tag])?;
        self.out.write_all(&(payload.len() as u32).to_le_bytes())?;
        self.out.write_all(payload)
    }

    /// Bundles must be written for every tick, in order, starting at 0.
    pub fn bundle(&mut self, bundle: &TickBundle) -> io::Result<()> {
        if bundle.tick != self.next_tick {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "replay bundles must be contiguous",
            ));
        }
        let mut e = Enc::new();
        bundle.encode(&mut e);
        self.record(REC_BUNDLE, &e.buf)?;
        self.next_tick += 1;
        Ok(())
    }

    /// Commands carried out while the clock was held, in front of the next tick.
    pub fn held(&mut self, bundle: &TickBundle) -> io::Result<()> {
        if bundle.tick != self.next_tick {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "held commands go in front of the next tick",
            ));
        }
        let mut e = Enc::new();
        bundle.encode(&mut e);
        self.record(REC_HELD, &e.buf)
    }

    /// Names the build that records, so a player can say which build plays it back.
    pub fn build(&mut self, name: &str) -> io::Result<()> {
        self.record(REC_BUILD, name.as_bytes())
    }

    /// Something shown, not simulated, that changed in front of the next tick.
    pub fn note(&mut self, note: &[u8]) -> io::Result<()> {
        let mut e = Enc::new();
        e.u32(self.next_tick);
        e.bytes(note);
        self.record(REC_NOTE, &e.buf)
    }

    pub fn hash(&mut self, tick: u32, hash: u64) -> io::Result<()> {
        let mut e = Enc::new();
        e.u32(tick);
        e.u64(hash);
        self.record(REC_HASH, &e.buf)
    }

    /// Ticks written so far.
    pub fn ticks(&self) -> u32 {
        self.next_tick
    }

    /// Writes the end marker and flushes. Later calls only flush.
    pub fn finish(&mut self) -> io::Result<()> {
        if !self.finished {
            self.finished = true;
            let ticks = self.next_tick.to_le_bytes();
            self.record(REC_END, &ticks)?;
        }
        self.out.flush()
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }

    pub fn into_inner(self) -> W {
        self.out
    }
}

pub struct ReplayReader<R: Read> {
    input: R,
    start: MatchStart,
    next_tick: u32,
}

impl ReplayReader<BufReader<File>> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, NetError> {
        ReplayReader::new(BufReader::new(File::open(path)?))
    }
}

impl<R: Read> ReplayReader<R> {
    pub fn new(mut input: R) -> Result<Self, NetError> {
        let mut head = [0u8; 12];
        input.read_exact(&mut head)?;
        if head[..4] != MAGIC {
            return Err(NetError::Malformed("not a replay file"));
        }
        let [_, _, _, _, v0, v1, v2, v3, l0, l1, l2, l3] = head;
        let version = u32::from_le_bytes([v0, v1, v2, v3]);
        if version != REPLAY_FORMAT_VERSION {
            return Err(NetError::Version { theirs: version });
        }
        let len = u32::from_le_bytes([l0, l1, l2, l3]) as usize;
        if len > MAX_FRAME_LEN {
            return Err(NetError::FrameTooLarge {
                len,
                max: MAX_FRAME_LEN,
            });
        }
        let mut buf = vec![0u8; len];
        input.read_exact(&mut buf)?;
        let mut d = Dec::new(&buf);
        let start = MatchStart::decode(&mut d)?;
        d.finish()?;
        Ok(ReplayReader {
            input,
            start,
            next_tick: 0,
        })
    }

    pub fn start(&self) -> &MatchStart {
        &self.start
    }

    /// `Ok(None)` at a clean end of file. A record cut short by a crash is an
    /// `Io(UnexpectedEof)` error; everything returned before it is good.
    pub fn next_record(&mut self) -> Result<Option<ReplayRecord>, NetError> {
        loop {
            let mut tag = [0u8; 1];
            loop {
                match self.input.read(&mut tag) {
                    Ok(0) => return Ok(None),
                    Ok(_) => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e.into()),
                }
            }
            let mut len = [0u8; 4];
            self.input.read_exact(&mut len)?;
            let len = u32::from_le_bytes(len) as usize;
            if len > MAX_FRAME_LEN {
                return Err(NetError::FrameTooLarge {
                    len,
                    max: MAX_FRAME_LEN,
                });
            }
            let mut buf = vec![0u8; len];
            self.input.read_exact(&mut buf)?;
            let mut d = Dec::new(&buf);
            let record = match tag[0] {
                REC_BUNDLE => {
                    let bundle = TickBundle::decode(&mut d)?;
                    if bundle.tick != self.next_tick {
                        return Err(NetError::Malformed("replay bundles are not contiguous"));
                    }
                    self.next_tick += 1;
                    ReplayRecord::Bundle(bundle)
                }
                REC_HASH => ReplayRecord::Hash {
                    tick: d.u32()?,
                    hash: d.u64()?,
                },
                REC_END => ReplayRecord::End { ticks: d.u32()? },
                REC_BUILD => {
                    let name = String::from_utf8_lossy(&buf).into_owned();
                    return Ok(Some(ReplayRecord::Build(name)));
                }
                REC_HELD => {
                    let bundle = TickBundle::decode(&mut d)?;
                    if bundle.tick != self.next_tick {
                        return Err(NetError::Malformed("held commands out of place"));
                    }
                    ReplayRecord::Held(bundle)
                }
                REC_NOTE => ReplayRecord::Note {
                    tick: d.u32()?,
                    note: d.bytes(MAX_FRAME_LEN)?,
                },
                _ => continue,
            };
            d.finish()?;
            return Ok(Some(record));
        }
    }
}

/// A replay held in memory.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Replay {
    pub start: MatchStart,
    /// `bundles[t].tick == t`.
    pub bundles: Vec<TickBundle>,
    /// Commands given while the clock was held, in order; each goes in front of
    /// `bundles[tick]` (or after the last bundle when `tick` is past it).
    pub held: Vec<TickBundle>,
    /// Hashes the recording machines agreed on, for verifying playback.
    pub hashes: BTreeMap<u32, u64>,
    /// False when the end marker is missing: the recorder crashed or was killed.
    pub complete: bool,
    /// The build that recorded it, when it said.
    pub build: Option<String>,
    /// `(tick, note)` in order: what the recording machine showed (`ReplayWriter::note`).
    pub notes: Vec<(u32, Vec<u8>)>,
}

impl Replay {
    pub fn load(path: impl AsRef<Path>) -> Result<Replay, NetError> {
        Replay::read(BufReader::new(File::open(path)?))
    }

    /// Reads to the end. A truncated tail is tolerated and reported through
    /// `complete`; any other damage is an error.
    pub fn read(input: impl Read) -> Result<Replay, NetError> {
        let mut reader = ReplayReader::new(input)?;
        let mut replay = Replay {
            start: reader.start().clone(),
            bundles: Vec::new(),
            held: Vec::new(),
            hashes: BTreeMap::new(),
            complete: false,
            build: None,
            notes: Vec::new(),
        };
        loop {
            match reader.next_record() {
                Ok(Some(ReplayRecord::Bundle(b))) => replay.bundles.push(b),
                Ok(Some(ReplayRecord::Held(b))) => replay.held.push(b),
                Ok(Some(ReplayRecord::Build(name))) => replay.build = Some(name),
                Ok(Some(ReplayRecord::Note { tick, note })) => replay.notes.push((tick, note)),
                Ok(Some(ReplayRecord::Hash { tick, hash })) => {
                    replay.hashes.insert(tick, hash);
                }
                Ok(Some(ReplayRecord::End { ticks })) => {
                    if ticks as usize != replay.bundles.len() {
                        return Err(NetError::Malformed(
                            "replay end marker disagrees with the log",
                        ));
                    }
                    replay.complete = true;
                    return Ok(replay);
                }
                Ok(None) => return Ok(replay),
                Err(NetError::Io(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    return Ok(replay)
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ContentId, PlayerSetup};
    use mc_core::PlayerId;

    fn start() -> MatchStart {
        MatchStart {
            content: ContentId {
                map_id: 1,
                blueprint_hash: 2,
            },
            seed: 3,
            input_delay: 2,
            players: vec![PlayerSetup {
                slot: PlayerId(0),
                name: "solo".into(),
                data: vec![7],
            }],
            options: vec![1, 2, 3],
        }
    }

    fn write_sample() -> (Vec<u8>, Vec<TickBundle>) {
        let bundles: Vec<TickBundle> = (0..20)
            .map(|t| {
                if t % 3 == 0 {
                    TickBundle::new(t, [(PlayerId(0), vec![vec![t as u8; 5]])])
                } else {
                    TickBundle::empty(t)
                }
            })
            .collect();
        let mut w = ReplayWriter::new(Vec::new(), &start()).unwrap();
        for b in &bundles {
            if b.tick == 5 {
                w.held(&held()).unwrap();
            }
            w.bundle(b).unwrap();
            if b.tick % 10 == 0 {
                w.hash(b.tick, 1000 + b.tick as u64).unwrap();
            }
        }
        w.finish().unwrap();
        (w.into_inner(), bundles)
    }

    /// Orders given on pause, in front of tick 5.
    fn held() -> TickBundle {
        TickBundle::new(5, [(PlayerId(0), vec![vec![9; 3]])])
    }

    #[test]
    fn round_trip() {
        let (bytes, bundles) = write_sample();
        let replay = Replay::read(bytes.as_slice()).unwrap();
        assert_eq!(replay.start, start());
        assert_eq!(replay.bundles, bundles);
        assert_eq!(replay.held, [held()]);
        assert_eq!(replay.hashes, BTreeMap::from([(0, 1000), (10, 1010)]));
        assert!(replay.complete);
    }

    #[test]
    fn the_recording_build_is_named() {
        let mut w = ReplayWriter::new(Vec::new(), &start()).unwrap();
        w.build("0.1.0-dev").unwrap();
        w.bundle(&TickBundle::empty(0)).unwrap();
        w.finish().unwrap();
        let replay = Replay::read(w.into_inner().as_slice()).unwrap();
        assert_eq!(replay.build.as_deref(), Some("0.1.0-dev"));
        assert_eq!(replay.bundles.len(), 1);
        assert!(replay.complete);
    }

    #[test]
    fn notes_keep_their_tick() {
        let mut w = ReplayWriter::new(Vec::new(), &start()).unwrap();
        w.note(b"overcast").unwrap();
        w.bundle(&TickBundle::empty(0)).unwrap();
        w.bundle(&TickBundle::empty(1)).unwrap();
        w.note(b"storm").unwrap();
        w.finish().unwrap();
        let replay = Replay::read(w.into_inner().as_slice()).unwrap();
        assert_eq!(
            replay.notes,
            [(0, b"overcast".to_vec()), (2, b"storm".to_vec())]
        );
        assert_eq!(replay.bundles.len(), 2);
    }

    #[test]
    fn writer_refuses_gaps() {
        let mut w = ReplayWriter::new(Vec::new(), &start()).unwrap();
        assert!(w.bundle(&TickBundle::empty(1)).is_err());
        w.bundle(&TickBundle::empty(0)).unwrap();
        assert!(w.bundle(&TickBundle::empty(0)).is_err());
        assert!(w.held(&TickBundle::empty(0)).is_err());
        w.held(&TickBundle::empty(1)).unwrap();
    }

    #[test]
    fn truncated_tail_is_usable_but_flagged() {
        let (bytes, bundles) = write_sample();
        // Cut inside the final bundle record (the end marker is 9 bytes).
        let cut = &bytes[..bytes.len() - 9 - 3];
        let replay = Replay::read(cut).unwrap();
        assert!(!replay.complete);
        assert_eq!(replay.bundles, bundles[..19]);
    }

    #[test]
    fn damage_is_an_error() {
        let (bytes, _) = write_sample();
        assert!(matches!(
            Replay::read(&b"NOPE\x01\0\0\0\0\0\0\0"[..]),
            Err(NetError::Malformed(_))
        ));
        let mut future = bytes.clone();
        let unsupported = REPLAY_FORMAT_VERSION + 1;
        future[4..8].copy_from_slice(&unsupported.to_le_bytes());
        assert!(matches!(
            Replay::read(future.as_slice()),
            Err(NetError::Version { theirs }) if theirs == unsupported
        ));
        let mut huge = bytes.clone();
        let header_len = 12 + u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        huge[header_len + 1..header_len + 5].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Replay::read(huge.as_slice()),
            Err(NetError::FrameTooLarge { .. })
        ));
        assert!(Replay::read(&bytes[..6]).is_err());
    }
}
