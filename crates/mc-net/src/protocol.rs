//! The lockstep wire protocol: message types, their encoding, and framing.
//!
//! A frame is a `u32` little-endian payload length followed by the payload;
//! the first payload byte is the message tag. Frames never exceed
//! [`MAX_FRAME_LEN`]; anything that could (snapshots) is chunked above this
//! layer. Decoding is total: any byte string yields a message or an error.
//!
//! The protocol knows nothing about the game. Commands, snapshots, player
//! setup and match options are opaque blobs owned by `mc-sim` / `mc-game`.
//!
//! The message enum and its codec are in [`message`]; this module has the
//! types they carry, the limits, framing and snapshot chunking.

mod message;
#[cfg(test)]
mod tests;

pub use message::Message;

use std::io::{self, Read, Write};

use mc_core::{PlayerId, MAX_PLAYERS};

use crate::wire::{Dec, Enc, NetError, Result};

/// Bumped on any incompatible change. Checked before anything else in `Hello`.
pub const PROTOCOL_VERSION: u32 = 21;

/// Hard cap on a frame payload, enforced on both send and receive.
pub const MAX_FRAME_LEN: usize = 1 << 20;
/// Largest single command blob.
pub const MAX_COMMAND_LEN: usize = 64 << 10;
/// Budget for one player's commands in one tick, counted by [`command_cost`].
/// Eight players at full budget still fit one bundle frame. Commands over the
/// budget are carried into the following tick, never dropped.
pub const MAX_COMMANDS_BYTES: usize = 96 << 10;
/// Snapshot blobs travel in chunks of this size.
pub const SNAPSHOT_CHUNK_LEN: usize = 256 << 10;
/// Largest snapshot either end will assemble.
pub const MAX_SNAPSHOT_LEN: usize = 256 << 20;
pub const MAX_NAME_LEN: usize = 64;
pub const MAX_CHAT_LEN: usize = 512;
pub const MAX_SETUP_LEN: usize = 4 << 10;
pub const MAX_OPTIONS_LEN: usize = 64 << 10;
pub const MAX_INPUT_DELAY: u32 = 50;
/// The most ticks the relay's adaptive input delay goes to (0.8 s at 10 ticks a second).
pub const MAX_ADAPTIVE_DELAY: u32 = 8;
pub const MAX_BUILD_LEN: usize = 64;
pub const MAX_TITLE_LEN: usize = 64;
/// State hash sections a desync report may carry.
pub const MAX_SECTIONS: usize = 64;

const HELLO_MAGIC: u32 = u32::from_le_bytes(*b"MCNT");
const MAX_DETAIL_LEN: usize = 256;

/// What a command counts against [`MAX_COMMANDS_BYTES`]: its bytes plus its
/// length prefix, so a flood of empty commands is bounded too.
#[inline]
pub fn command_cost(command: &[u8]) -> usize {
    command.len() + 4
}

/// Identifies the immutable inputs of a match. Two machines with different
/// values would desync on tick 0, so the relay refuses the mismatch up front.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ContentId {
    pub map_id: u64,
    pub blueprint_hash: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Player,
    Observer,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlayerSetup {
    pub slot: PlayerId,
    pub name: String,
    /// Faction, team, colour, start spot… defined by the game, opaque here.
    pub data: Vec<u8>,
}

/// Everything a machine needs to build tick-0 state. Also the replay header.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MatchStart {
    pub content: ContentId,
    pub seed: u64,
    /// Ticks between issuing a command and the tick that executes it.
    pub input_delay: u32,
    /// In slot order.
    pub players: Vec<PlayerSetup>,
    /// Match-wide options chosen by the host, opaque here.
    pub options: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlayerCommands {
    pub slot: PlayerId,
    pub commands: Vec<Vec<u8>>,
}

/// The complete input of one sim tick.
///
/// Canonical form: only slots with at least one command appear, in ascending
/// slot order. Applying [`TickBundle::commands`] in iteration order is what
/// makes every machine execute commands identically.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TickBundle {
    pub tick: u32,
    pub players: Vec<PlayerCommands>,
}

impl TickBundle {
    pub fn empty(tick: u32) -> TickBundle {
        TickBundle {
            tick,
            players: Vec::new(),
        }
    }

    /// Builds the canonical form from per-slot lists in any order.
    pub fn new(
        tick: u32,
        per_slot: impl IntoIterator<Item = (PlayerId, Vec<Vec<u8>>)>,
    ) -> TickBundle {
        let mut players: Vec<PlayerCommands> = Vec::new();
        for (slot, commands) in per_slot {
            if commands.is_empty() {
                continue;
            }
            match players.iter_mut().find(|p| p.slot == slot) {
                Some(p) => p.commands.extend(commands),
                None => players.push(PlayerCommands { slot, commands }),
            }
        }
        players.sort_by_key(|p| p.slot);
        TickBundle { tick, players }
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    /// Every command with its issuer, in the order the sim must apply them.
    pub fn commands(&self) -> impl Iterator<Item = (PlayerId, &[u8])> {
        self.players
            .iter()
            .flat_map(|p| p.commands.iter().map(move |c| (p.slot, c.as_slice())))
    }

    pub(crate) fn encode(&self, e: &mut Enc) {
        e.u32(self.tick);
        e.u8(self.players.len() as u8);
        for p in &self.players {
            e.u8(p.slot.0);
            encode_commands(e, &p.commands);
        }
    }

    pub(crate) fn decode(d: &mut Dec) -> Result<TickBundle> {
        let tick = d.u32()?;
        let n = d.u8()? as usize;
        if n > MAX_PLAYERS {
            return Err(NetError::Malformed("bundle has too many slots"));
        }
        let mut players = Vec::with_capacity(n);
        let mut prev: Option<u8> = None;
        for _ in 0..n {
            let slot = decode_slot(d)?;
            if prev.is_some_and(|p| p >= slot.0) {
                return Err(NetError::Malformed("bundle slots out of order"));
            }
            prev = Some(slot.0);
            let commands = decode_commands(d)?;
            if commands.is_empty() {
                return Err(NetError::Malformed("bundle lists an empty slot"));
            }
            players.push(PlayerCommands { slot, commands });
        }
        Ok(TickBundle { tick, players })
    }
}

fn encode_commands(e: &mut Enc, commands: &[Vec<u8>]) {
    e.u32(commands.len() as u32);
    for c in commands {
        e.bytes(c);
    }
}

fn decode_commands(d: &mut Dec) -> Result<Vec<Vec<u8>>> {
    let n = d.u32()? as usize;
    // Every command occupies at least its length prefix.
    if n > d.remaining() / 4 {
        return Err(NetError::Malformed("command count exceeds payload"));
    }
    let mut commands = Vec::with_capacity(n);
    let mut cost = 0usize;
    for _ in 0..n {
        let c = d.bytes(MAX_COMMAND_LEN)?;
        cost += command_cost(&c);
        commands.push(c);
    }
    if cost > MAX_COMMANDS_BYTES {
        return Err(NetError::Malformed("command list over its byte budget"));
    }
    Ok(commands)
}

fn decode_slot(d: &mut Dec) -> Result<PlayerId> {
    let slot = d.u8()?;
    if slot as usize >= MAX_PLAYERS {
        return Err(NetError::Malformed("player slot out of range"));
    }
    Ok(PlayerId(slot))
}

/// Removes commands from the front of `pending` until the per-tick budget is
/// spent. Always makes progress: one command never exceeds the budget.
pub(crate) fn take_commands(pending: &mut Vec<Vec<u8>>, budget: &mut usize) -> Vec<Vec<u8>> {
    let mut n = 0;
    for c in pending.iter() {
        let cost = command_cost(c);
        if cost > *budget {
            break;
        }
        *budget -= cost;
        n += 1;
    }
    if n == pending.len() {
        std::mem::take(pending)
    } else {
        pending.drain(..n).collect()
    }
}

/// Checks caller-supplied commands against the protocol limits.
pub(crate) fn check_commands(commands: &[Vec<u8>]) -> Result<()> {
    if commands.iter().any(|c| c.len() > MAX_COMMAND_LEN) {
        return Err(NetError::Limit("command larger than MAX_COMMAND_LEN"));
    }
    Ok(())
}

impl MatchStart {
    pub(crate) fn encode(&self, e: &mut Enc) {
        e.u64(self.content.map_id);
        e.u64(self.content.blueprint_hash);
        e.u64(self.seed);
        e.u32(self.input_delay);
        e.u8(self.players.len() as u8);
        for p in &self.players {
            e.u8(p.slot.0);
            e.str(&p.name);
            e.bytes(&p.data);
        }
        e.bytes(&self.options);
    }

    pub(crate) fn decode(d: &mut Dec) -> Result<MatchStart> {
        let content = ContentId {
            map_id: d.u64()?,
            blueprint_hash: d.u64()?,
        };
        let seed = d.u64()?;
        let input_delay = d.u32()?;
        if input_delay > MAX_INPUT_DELAY {
            return Err(NetError::Malformed("input delay out of range"));
        }
        let n = d.u8()? as usize;
        if n > MAX_PLAYERS {
            return Err(NetError::Malformed("too many players"));
        }
        let mut players = Vec::with_capacity(n);
        for _ in 0..n {
            let slot = decode_slot(d)?;
            if players.last().is_some_and(|p: &PlayerSetup| p.slot >= slot) {
                return Err(NetError::Malformed("player setup out of slot order"));
            }
            players.push(PlayerSetup {
                slot,
                name: d.str(MAX_NAME_LEN)?,
                data: d.bytes(MAX_SETUP_LEN)?,
            });
        }
        let options = d.bytes(MAX_OPTIONS_LEN)?;
        Ok(MatchStart {
            content,
            seed,
            input_delay,
            players,
            options,
        })
    }

    /// Enforces on the sending side what `decode` enforces on the receiving side.
    pub fn validate(&self) -> Result<()> {
        if self.input_delay > MAX_INPUT_DELAY {
            return Err(NetError::Limit("input delay over MAX_INPUT_DELAY"));
        }
        if self.players.len() > MAX_PLAYERS {
            return Err(NetError::Limit("more than MAX_PLAYERS players"));
        }
        if self.options.len() > MAX_OPTIONS_LEN {
            return Err(NetError::Limit("match options over MAX_OPTIONS_LEN"));
        }
        for (i, p) in self.players.iter().enumerate() {
            if p.slot.index() >= MAX_PLAYERS || (i > 0 && self.players[i - 1].slot >= p.slot) {
                return Err(NetError::Limit(
                    "player slots must be ascending and below MAX_PLAYERS",
                ));
            }
            if p.name.len() > MAX_NAME_LEN || p.data.len() > MAX_SETUP_LEN {
                return Err(NetError::Limit("player name or setup blob too long"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hello {
    pub name: String,
    pub role: Role,
    /// The token from an earlier `Welcome`, to reclaim that slot mid-match.
    pub token: Option<u64>,
    pub content: ContentId,
    pub setup: Vec<u8>,
    /// The game build, e.g. `0.1.0+e9fea68`. Players in one match must agree: the
    /// simulation's code is not part of [`ContentId`].
    pub build: String,
    /// The room to join on a server that hosts many; 0 on a relay that hosts one match.
    pub room: u32,
    /// From the server's directory sign-in: proves `name` belongs to this player.
    pub ticket: Option<[u8; 16]>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MatchConfig {
    pub max_players: u8,
    pub input_delay: u32,
    /// Wall-clock length of a tick as paced by the relay.
    pub tick_ms: u32,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Welcome {
    /// `None` for observers.
    pub slot: Option<PlayerId>,
    /// Present this in a later `Hello` to reclaim the slot after a disconnect.
    pub token: u64,
    pub config: MatchConfig,
    /// The match is already running; a snapshot and the bundles after it follow.
    pub in_progress: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LobbyPlayer {
    pub slot: PlayerId,
    pub name: String,
    pub ready: bool,
    pub setup: Vec<u8>,
    /// The server checked the name against the player's device key.
    pub verified: bool,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LobbyState {
    /// The player who may start the match: whoever opened the room, then the
    /// lowest occupied seat once they leave.
    pub host: Option<PlayerId>,
    pub players: Vec<LobbyPlayer>,
    pub observers: u16,
    pub options: Vec<u8>,
    /// Seats a person may take, one bit per slot; the host sets it.
    pub open: u8,
    /// Milliseconds left before the match starts; 0 when no start is under way.
    pub countdown_ms: u32,
    pub title: String,
}

/// How one seat's connection stands, for every player's scoreboard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Link {
    Connected,
    /// Still building the match after the start.
    Loading,
    /// Did not send its commands in time; the match does not wait for it.
    Lagging,
    /// Gone; may come back with its token.
    Dropped,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PeerStat {
    pub slot: PlayerId,
    /// Round trip to the relay as the relay measured it; 0 until measured.
    pub rtt_ms: u16,
    pub link: Link,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefuseReason {
    VersionMismatch,
    ContentMismatch,
    LobbyFull,
    MatchInProgress,
    BadToken,
    /// Observers cannot enter before a player has defined the match content.
    NoHost,
    /// The host removed this player from the lobby.
    Kicked,
    /// No room with that code on this server (or it has closed).
    RoomNotFound,
    /// The sign-in ticket is unknown or expired.
    BadTicket,
    /// The game build differs from the match's.
    BuildMismatch,
    /// The server is at its limit of rooms or connections.
    ServerFull,
    Other,
}

impl RefuseReason {
    // Codes are part of the frozen `Refused` layout: never renumber, never reuse.
    fn code(self) -> u8 {
        match self {
            RefuseReason::VersionMismatch => 1,
            RefuseReason::ContentMismatch => 2,
            RefuseReason::LobbyFull => 3,
            RefuseReason::MatchInProgress => 4,
            RefuseReason::BadToken => 5,
            RefuseReason::NoHost => 6,
            RefuseReason::Kicked => 7,
            RefuseReason::RoomNotFound => 8,
            RefuseReason::BadTicket => 9,
            RefuseReason::BuildMismatch => 10,
            RefuseReason::ServerFull => 11,
            RefuseReason::Other => 0,
        }
    }

    fn from_code(code: u8) -> RefuseReason {
        match code {
            1 => RefuseReason::VersionMismatch,
            2 => RefuseReason::ContentMismatch,
            3 => RefuseReason::LobbyFull,
            4 => RefuseReason::MatchInProgress,
            5 => RefuseReason::BadToken,
            6 => RefuseReason::NoHost,
            7 => RefuseReason::Kicked,
            8 => RefuseReason::RoomNotFound,
            9 => RefuseReason::BadTicket,
            10 => RefuseReason::BuildMismatch,
            11 => RefuseReason::ServerFull,
            _ => RefuseReason::Other,
        }
    }

    /// What to tell the player.
    pub fn describe(self) -> &'static str {
        match self {
            RefuseReason::VersionMismatch => "a different version of the game",
            RefuseReason::ContentMismatch => "a different map or unit data",
            RefuseReason::LobbyFull => "the game is full",
            RefuseReason::MatchInProgress => "the match has already started",
            RefuseReason::BadToken => "your seat in that match is gone",
            RefuseReason::NoHost => "nobody is hosting yet",
            RefuseReason::Kicked => "the host removed you from the game",
            RefuseReason::RoomNotFound => "there is no game with that code",
            RefuseReason::BadTicket => "your sign-in has expired",
            RefuseReason::BuildMismatch => "a different build of the game",
            RefuseReason::ServerFull => "the server is full",
            RefuseReason::Other => "refused",
        }
    }
}

/// Encodes `msg` as a complete frame: length prefix plus payload.
pub fn encode_frame(msg: &Message) -> Result<Vec<u8>> {
    let mut e = Enc::new();
    e.u32(0);
    msg.encode(&mut e);
    let len = e.buf.len() - 4;
    if len > MAX_FRAME_LEN {
        return Err(NetError::FrameTooLarge {
            len,
            max: MAX_FRAME_LEN,
        });
    }
    e.buf[..4].copy_from_slice(&(len as u32).to_le_bytes());
    Ok(e.buf)
}

/// Decodes one frame payload (without its length prefix).
pub fn decode_payload(payload: &[u8]) -> Result<Message> {
    let mut d = Dec::new(payload);
    let msg = Message::decode(&mut d)?;
    d.finish()?;
    Ok(msg)
}

pub fn write_frame(w: &mut impl Write, msg: &Message) -> Result<()> {
    w.write_all(&encode_frame(msg)?)?;
    Ok(())
}

/// Blocks for one frame. End of stream exactly between frames is
/// [`NetError::Closed`]; anywhere else it is an io error.
pub fn read_frame(r: &mut impl Read) -> Result<Message> {
    let mut header = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        match r.read(&mut header[got..]) {
            Ok(0) if got == 0 => return Err(NetError::Closed),
            Ok(0) => return Err(NetError::Io(io::ErrorKind::UnexpectedEof.into())),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(NetError::Io(e)),
        }
    }
    let len = u32::from_le_bytes(header) as usize;
    if len == 0 {
        return Err(NetError::Malformed("empty frame"));
    }
    if len > MAX_FRAME_LEN {
        return Err(NetError::FrameTooLarge {
            len,
            max: MAX_FRAME_LEN,
        });
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    decode_payload(&payload)
}

/// Splits a snapshot into `SnapshotChunk` messages. An empty blob is one empty chunk.
pub fn snapshot_chunks(tick: u32, blob: &[u8]) -> Result<Vec<Message>> {
    if blob.len() > MAX_SNAPSHOT_LEN {
        return Err(NetError::Limit("snapshot larger than MAX_SNAPSHOT_LEN"));
    }
    let total_len = blob.len() as u32;
    if blob.is_empty() {
        return Ok(vec![Message::SnapshotChunk {
            tick,
            total_len,
            offset: 0,
            data: Vec::new(),
        }]);
    }
    Ok(blob
        .chunks(SNAPSHOT_CHUNK_LEN)
        .enumerate()
        .map(|(i, c)| Message::SnapshotChunk {
            tick,
            total_len,
            offset: (i * SNAPSHOT_CHUNK_LEN) as u32,
            data: c.to_vec(),
        })
        .collect())
}

/// Reassembles chunks. They must arrive in order and agree on tick and total
/// length. Memory grows with the bytes received, never with the announced size.
#[derive(Default)]
pub struct SnapshotAssembler {
    current: Option<(u32, u32)>,
    buf: Vec<u8>,
}

impl SnapshotAssembler {
    pub fn new() -> SnapshotAssembler {
        SnapshotAssembler::default()
    }

    /// Returns the finished `(tick, blob)` once the last chunk is in.
    pub fn push(
        &mut self,
        tick: u32,
        total_len: u32,
        offset: u32,
        data: &[u8],
    ) -> Result<Option<(u32, Vec<u8>)>> {
        if total_len as usize > MAX_SNAPSHOT_LEN {
            return Err(NetError::Malformed("snapshot over MAX_SNAPSHOT_LEN"));
        }
        if offset == 0 {
            self.current = Some((tick, total_len));
            self.buf.clear();
        }
        if self.current != Some((tick, total_len)) || offset as usize != self.buf.len() {
            return Err(NetError::Malformed("snapshot chunk out of sequence"));
        }
        if self.buf.len() + data.len() > total_len as usize || (data.is_empty() && total_len != 0) {
            return Err(NetError::Malformed("snapshot chunk has a bad length"));
        }
        self.buf.extend_from_slice(data);
        if self.buf.len() == total_len as usize {
            self.current = None;
            return Ok(Some((tick, std::mem::take(&mut self.buf))));
        }
        Ok(None)
    }
}
