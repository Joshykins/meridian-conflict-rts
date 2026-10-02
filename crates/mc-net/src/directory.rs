//! The game directory: signing in with a device key, hosting and finding rooms,
//! and the list of open games.
//!
//! A player's machine keeps one directory connection to `meridian-server` while
//! the game browser is open, and opens match connections (the protocol in
//! [`crate::protocol`]) to the room it joins, on the same address. Both kinds
//! share the server's one port and the same framing: a `u32` little-endian
//! length, then the payload, whose first byte is the message tag. Match tags
//! start at 1 and directory tags at 100, so the server's front door tells them
//! apart by the first frame alone ([`read_first`]).
//!
//! **Identity.** A player is a name plus an ed25519 key made on first run
//! ([`Identity`]). Signing in is a challenge: the client says `DirHello` with its
//! name and public key, the server sends a random nonce, the client signs it.
//! The server remembers which key first claimed each name (case-insensitive) and
//! from then on refuses that name to any other key. Nothing secret crosses the
//! wire and the server stores only public keys.
//!
//! **Tickets.** A signed-in player holds a ticket: 16 random bytes the server
//! binds to the verified name. A match connection presents it in `Hello`, and
//! the room then shows the player as verified under that name, whatever name
//! the `Hello` carried.
//!
//! **Rooms.** A room is one match hub on the server, known by a [`RoomCode`].
//! Public rooms are listed to every subscribed client; private rooms are found
//! only by their code.

mod client;
mod identity;
mod message;
#[cfg(test)]
mod tests;

pub use client::{DirectoryClient, DirectoryEvent};
pub use identity::{fingerprint, random_bytes, verify_proof, Identity, IDENTITY_FILE};
pub use message::DirMessage;

use std::fmt;
use std::io::{Read, Write};
use std::str::FromStr;

use mc_core::MAX_PLAYERS;

use crate::protocol::{
    decode_payload, read_payload, seal_frame, ContentId, Hello, Message, MAX_BUILD_LEN,
    MAX_NAME_LEN, MAX_TITLE_LEN,
};
use crate::relay::{RoomPhase, RoomStatus};
use crate::wire::{Dec, Enc, NetError, Result};

/// Bumped on any incompatible change to the directory messages. Checked first in `DirHello`.
pub const DIRECTORY_VERSION: u32 = 1;
/// The longest player name, in characters (names are ASCII, so also in bytes).
pub const MAX_PLAYER_NAME: usize = 24;
/// Every new player's name until they choose one. A server gives a name to the
/// first computer that signs in with it, so this one is never given out: each
/// player picks a callsign of their own before going online.
pub const DEFAULT_PLAYER_NAME: &str = "Commander";
pub const MAX_MOTD_LEN: usize = 1024;
/// Rooms one `Rooms` message may list; the server's room cap stays at or below it.
pub const MAX_LISTED_ROOMS: usize = 512;
pub const NONCE_LEN: usize = 32;
pub const TICKET_LEN: usize = 16;

const DIR_MAGIC: u32 = u32::from_le_bytes(*b"MCDR");
const MAX_DETAIL_LEN: usize = 256;

/// Whether `name` may be a player's name: 1 to [`MAX_PLAYER_NAME`] characters of
/// ASCII letters, digits, space, `_`, `-` and `.`, with no space at either end
/// and no two in a row, and not [`DEFAULT_PLAYER_NAME`]. ASCII only, so no two
/// names look alike but differ in their letters. `Err` says what is wrong, for
/// the player.
pub fn check_name(name: &str) -> std::result::Result<(), &'static str> {
    if name.is_empty() {
        return Err("a name is needed");
    }
    if name.eq_ignore_ascii_case(DEFAULT_PLAYER_NAME) {
        return Err("everyone starts as Commander, so choose a callsign of your own");
    }
    if name.len() > MAX_PLAYER_NAME {
        return Err("names are at most 24 characters");
    }
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.');
    if !name.chars().all(allowed) {
        return Err("names use letters, digits, spaces, _ - and . only");
    }
    if name.starts_with(' ') || name.ends_with(' ') || name.contains("  ") {
        return Err("no spaces at the ends of a name, or two in a row");
    }
    Ok(())
}

/// A room's code: six characters from an alphabet without look-alikes (no 0, O,
/// 1, I or L), shown as `ABC-DEF`. It travels as the `u32` in `Hello.room`,
/// never 0.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RoomCode(u32);

const CODE_ALPHABET: &[u8; 31] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
const CODE_LEN: u32 = 6;
/// How many codes there are: 31^6, which fits a `u32` with room to spare.
const CODE_SPACE: u32 = 887_503_681;

impl RoomCode {
    /// The code a `Hello.room` names, if it names one.
    pub fn from_u32(v: u32) -> Option<RoomCode> {
        (1..=CODE_SPACE).contains(&v).then_some(RoomCode(v))
    }

    pub fn to_u32(self) -> u32 {
        self.0
    }

    /// A code from OS randomness.
    pub fn random() -> std::io::Result<RoomCode> {
        // Only whole multiples of the code space, so every code is equally likely.
        let limit = u32::MAX - u32::MAX % CODE_SPACE;
        loop {
            let v = u32::from_le_bytes(random_bytes()?);
            if v < limit {
                return Ok(RoomCode(v % CODE_SPACE + 1));
            }
        }
    }

    fn chars(self) -> [u8; CODE_LEN as usize] {
        let mut v = self.0 - 1;
        let mut out = [0; CODE_LEN as usize];
        for c in out.iter_mut().rev() {
            *c = CODE_ALPHABET[(v % 31) as usize];
            v /= 31;
        }
        out
    }

    pub(crate) fn decode(d: &mut Dec) -> Result<RoomCode> {
        RoomCode::from_u32(d.u32()?).ok_or(NetError::Malformed("room code out of range"))
    }
}

impl fmt::Display for RoomCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.chars();
        let text = |s: &[u8]| s.iter().map(|&b| b as char).collect::<String>();
        write!(f, "{}-{}", text(&c[..3]), text(&c[3..]))
    }
}

/// A string that is not a room code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BadRoomCode;

impl fmt::Display for BadRoomCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a game code is six letters and digits, like K7M-Q2X")
    }
}

impl std::error::Error for BadRoomCode {}

impl FromStr for RoomCode {
    type Err = BadRoomCode;

    /// Any case, with or without the dash (spaces are ignored too).
    fn from_str(s: &str) -> std::result::Result<RoomCode, BadRoomCode> {
        let mut v: u32 = 0;
        let mut n = 0;
        for b in s.bytes().filter(|b| !matches!(b, b'-' | b' ')) {
            let b = b.to_ascii_uppercase();
            let digit = CODE_ALPHABET
                .iter()
                .position(|&a| a == b)
                .ok_or(BadRoomCode)?;
            n += 1;
            if n > CODE_LEN {
                return Err(BadRoomCode);
            }
            v = v * 31 + digit as u32;
        }
        if n != CODE_LEN {
            return Err(BadRoomCode);
        }
        Ok(RoomCode(v + 1))
    }
}

/// A room as the game browser lists it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RoomListing {
    pub code: RoomCode,
    pub title: String,
    /// The host's name; empty until the host has arrived.
    pub host: String,
    /// Seated players, in seat order.
    pub players: Vec<String>,
    pub seats: u8,
    /// Open seats nobody has taken.
    pub free: u8,
    pub observers: u16,
    pub phase: RoomPhase,
    pub map: String,
    pub mode: String,
    /// The game build the room plays; others cannot join.
    pub build: String,
    pub content: Option<ContentId>,
    pub private: bool,
}

impl RoomListing {
    /// What the browser shows of a room the server hosts.
    pub fn new(code: RoomCode, status: &RoomStatus, private: bool) -> RoomListing {
        RoomListing {
            code,
            title: status.title.clone(),
            host: status.host.clone(),
            players: status.players.clone(),
            seats: status.seats,
            free: status.free,
            observers: status.observers,
            phase: status.phase,
            map: status.map.clone(),
            mode: status.mode.clone(),
            build: status.build.clone(),
            content: status.content,
            private,
        }
    }

    pub(crate) fn encode(&self, e: &mut Enc) {
        e.u32(self.code.to_u32());
        e.str(&self.title);
        e.str(&self.host);
        e.u8(self.players.len() as u8);
        for p in &self.players {
            e.str(p);
        }
        e.u8(self.seats);
        e.u8(self.free);
        e.u16(self.observers);
        match self.phase {
            RoomPhase::Lobby => e.u8(0),
            RoomPhase::Loading => e.u8(1),
            RoomPhase::Playing { tick } => {
                e.u8(2);
                e.u32(tick);
            }
            RoomPhase::Ended => e.u8(3),
        }
        e.str(&self.map);
        e.str(&self.mode);
        e.str(&self.build);
        encode_content(e, self.content);
        e.bool(self.private);
    }

    pub(crate) fn decode(d: &mut Dec) -> Result<RoomListing> {
        let code = RoomCode::decode(d)?;
        let title = d.str(MAX_TITLE_LEN)?;
        let host = d.str(MAX_NAME_LEN)?;
        let n = d.u8()? as usize;
        if n > MAX_PLAYERS {
            return Err(NetError::Malformed("too many players in a listing"));
        }
        let players = (0..n).map(|_| d.str(MAX_NAME_LEN)).collect::<Result<_>>()?;
        Ok(RoomListing {
            code,
            title,
            host,
            players,
            seats: d.u8()?,
            free: d.u8()?,
            observers: d.u16()?,
            phase: match d.u8()? {
                0 => RoomPhase::Lobby,
                1 => RoomPhase::Loading,
                2 => RoomPhase::Playing { tick: d.u32()? },
                3 => RoomPhase::Ended,
                _ => return Err(NetError::Malformed("unknown room phase")),
            },
            map: d.str(MAX_TITLE_LEN)?,
            mode: d.str(MAX_TITLE_LEN)?,
            build: d.str(MAX_BUILD_LEN)?,
            content: decode_content(d)?,
            private: d.bool()?,
        })
    }
}

fn encode_content(e: &mut Enc, content: Option<ContentId>) {
    e.bool(content.is_some());
    let c = content.unwrap_or_default();
    e.u64(c.map_id);
    e.u64(c.blueprint_hash);
}

fn decode_content(d: &mut Dec) -> Result<Option<ContentId>> {
    let present = d.bool()?;
    let c = ContentId {
        map_id: d.u64()?,
        blueprint_hash: d.u64()?,
    };
    Ok(present.then_some(c))
}

/// What a client asks for when it hosts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NewRoom {
    /// Empty: the server names it after the host.
    pub title: String,
    /// Player seats, `1..=MAX_PLAYERS`.
    pub seats: u8,
    /// Not listed; joined by its code only.
    pub private: bool,
    /// The map and unit data the host will bring, listed until the host is in.
    pub content: ContentId,
    pub build: String,
}

/// The first message on a directory connection.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DirHello {
    pub name: String,
    pub public_key: [u8; 32],
    pub build: String,
}

/// Why the server turned a directory request down.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirRefuseReason {
    VersionMismatch,
    /// The name breaks [`check_name`].
    BadName,
    /// Another device key claimed the name first.
    NameTaken,
    /// The answer to the challenge did not verify against the key.
    BadSignature,
    /// The server is at its limit of rooms or connections.
    ServerFull,
    /// The server's operator barred this name.
    Banned,
    /// Too many connections from one address.
    TooManyConnections,
    /// This player already has as many open rooms as one player may.
    TooManyRooms,
    Other,
}

impl DirRefuseReason {
    // Codes are part of the frozen `Refused` layout: never renumber, never reuse.
    fn code(self) -> u8 {
        match self {
            DirRefuseReason::Other => 0,
            DirRefuseReason::VersionMismatch => 1,
            DirRefuseReason::BadName => 2,
            DirRefuseReason::NameTaken => 3,
            DirRefuseReason::BadSignature => 4,
            DirRefuseReason::ServerFull => 5,
            DirRefuseReason::Banned => 6,
            DirRefuseReason::TooManyConnections => 7,
            DirRefuseReason::TooManyRooms => 8,
        }
    }

    fn from_code(code: u8) -> DirRefuseReason {
        match code {
            1 => DirRefuseReason::VersionMismatch,
            2 => DirRefuseReason::BadName,
            3 => DirRefuseReason::NameTaken,
            4 => DirRefuseReason::BadSignature,
            5 => DirRefuseReason::ServerFull,
            6 => DirRefuseReason::Banned,
            7 => DirRefuseReason::TooManyConnections,
            8 => DirRefuseReason::TooManyRooms,
            _ => DirRefuseReason::Other,
        }
    }

    /// What to tell the player.
    pub fn describe(self) -> &'static str {
        match self {
            DirRefuseReason::VersionMismatch => "the server runs a different version of the game",
            DirRefuseReason::BadName => "that name cannot be used",
            DirRefuseReason::NameTaken => "that name belongs to another player on this server",
            DirRefuseReason::BadSignature => "the sign-in did not verify",
            DirRefuseReason::ServerFull => "the server is full",
            DirRefuseReason::Banned => "that name is barred from this server",
            DirRefuseReason::TooManyConnections => "too many connections from your address",
            DirRefuseReason::TooManyRooms => "you already host as many games as one player may",
            DirRefuseReason::Other => "refused",
        }
    }
}

/// Encodes `msg` as a complete frame: length prefix plus payload.
pub fn encode_dir_frame(msg: &DirMessage) -> Result<Vec<u8>> {
    let mut e = Enc::new();
    e.u32(0);
    msg.encode(&mut e);
    seal_frame(e)
}

/// Decodes one frame payload (without its length prefix).
pub fn decode_dir_payload(payload: &[u8]) -> Result<DirMessage> {
    let mut d = Dec::new(payload);
    let msg = DirMessage::decode(&mut d)?;
    d.finish()?;
    Ok(msg)
}

pub fn write_dir_frame(w: &mut impl Write, msg: &DirMessage) -> Result<()> {
    w.write_all(&encode_dir_frame(msg)?)?;
    Ok(())
}

/// Blocks for one frame, like [`crate::protocol::read_frame`].
pub fn read_dir_frame(r: &mut impl Read) -> Result<DirMessage> {
    decode_dir_payload(&read_payload(r)?)
}

/// What a connection to the server's one port opened with.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FirstFrame {
    /// A match connection, for the room `Hello.room`.
    Match(Hello),
    /// A directory connection.
    Directory(DirHello),
    /// A client of another match protocol version: answer with `Message::Refused`,
    /// whose layout every version reads.
    MatchVersion(u32),
    /// A client of another directory version: answer with `DirMessage::Refused`.
    DirectoryVersion(u32),
}

/// Reads a connection's first frame and says which protocol it speaks. Anything
/// but a `Hello` or a `DirHello` is an error: the connection is not a game's.
pub fn read_first(r: &mut impl Read) -> Result<FirstFrame> {
    let payload = read_payload(r)?;
    if payload.first() == Some(&message::DIR_HELLO) {
        return match decode_dir_payload(&payload) {
            Ok(DirMessage::Hello(hello)) => Ok(FirstFrame::Directory(hello)),
            Ok(_) => Err(NetError::Malformed("expected DirHello")),
            Err(NetError::Version { theirs }) => Ok(FirstFrame::DirectoryVersion(theirs)),
            Err(e) => Err(e),
        };
    }
    match decode_payload(&payload) {
        Ok(Message::Hello(hello)) => Ok(FirstFrame::Match(hello)),
        Ok(_) => Err(NetError::Malformed("expected Hello")),
        Err(NetError::Version { theirs }) => Ok(FirstFrame::MatchVersion(theirs)),
        Err(e) => Err(e),
    }
}
