//! The lockstep wire protocol: message types, their encoding, and framing.
//!
//! A frame is a `u32` little-endian payload length followed by the payload;
//! the first payload byte is the message tag. Frames never exceed
//! [`MAX_FRAME_LEN`]; anything that could (snapshots) is chunked above this
//! layer. Decoding is total: any byte string yields a message or an error.
//!
//! The protocol knows nothing about the game. Commands, snapshots, player
//! setup and match options are opaque blobs owned by `mc-sim` / `mc-game`.

use std::io::{self, Read, Write};

use mc_core::{PlayerId, MAX_PLAYERS};

use crate::wire::{Dec, Enc, NetError, Result};

/// Bumped on any incompatible change. Checked before anything else in `Hello`.
pub const PROTOCOL_VERSION: u32 = 14;

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
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LobbyState {
    /// The player who may start the match: the lowest occupied slot.
    pub host: Option<PlayerId>,
    pub players: Vec<LobbyPlayer>,
    pub observers: u16,
    pub options: Vec<u8>,
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
    Other,
}

impl RefuseReason {
    fn code(self) -> u8 {
        match self {
            RefuseReason::VersionMismatch => 1,
            RefuseReason::ContentMismatch => 2,
            RefuseReason::LobbyFull => 3,
            RefuseReason::MatchInProgress => 4,
            RefuseReason::BadToken => 5,
            RefuseReason::NoHost => 6,
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
            _ => RefuseReason::Other,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Message {
    // client -> relay
    Hello(Hello),
    Ready(bool),
    SetSetup(Vec<u8>),
    /// Host only.
    SetOptions(Vec<u8>),
    /// Host only. Honoured once every other player is ready.
    StartRequest,
    /// Sent for every tick, empty or not, so the relay can close the turn.
    Commands {
        tick: u32,
        commands: Vec<Vec<u8>>,
    },
    Hash {
        tick: u32,
        hash: u64,
    },
    Leave,
    // relay -> client
    Welcome(Welcome),
    /// Layout is frozen across protocol versions so a mismatched peer can read it.
    Refused {
        reason: RefuseReason,
        detail: String,
    },
    Lobby(LobbyState),
    Start(MatchStart),
    Bundle(TickBundle),
    Desync {
        tick: u32,
        hashes: Vec<(PlayerId, u64)>,
    },
    /// Asks for the state as it is right after stepping `tick`.
    SnapshotRequest {
        tick: u32,
    },
    PlayerDropped(PlayerId),
    PlayerRejoined(PlayerId),
    MatchEnd,
    // both directions
    SnapshotChunk {
        tick: u32,
        total_len: u32,
        offset: u32,
        data: Vec<u8>,
    },
    Ping(u32),
    Pong(u32),
    /// `from` is filled in by the relay; `None` is an observer.
    Chat {
        from: Option<PlayerId>,
        text: String,
    },
}

mod tag {
    pub const HELLO: u8 = 1;
    pub const WELCOME: u8 = 2;
    pub const REFUSED: u8 = 3;
    pub const LOBBY: u8 = 4;
    pub const READY: u8 = 5;
    pub const SET_SETUP: u8 = 6;
    pub const SET_OPTIONS: u8 = 7;
    pub const START_REQUEST: u8 = 8;
    pub const START: u8 = 9;
    pub const COMMANDS: u8 = 10;
    pub const BUNDLE: u8 = 11;
    pub const HASH: u8 = 12;
    pub const DESYNC: u8 = 13;
    pub const SNAPSHOT_REQUEST: u8 = 14;
    pub const SNAPSHOT_CHUNK: u8 = 15;
    pub const PLAYER_DROPPED: u8 = 16;
    pub const PLAYER_REJOINED: u8 = 17;
    pub const PING: u8 = 18;
    pub const PONG: u8 = 19;
    pub const CHAT: u8 = 20;
    pub const LEAVE: u8 = 21;
    pub const MATCH_END: u8 = 22;
}

fn encode_opt_slot(e: &mut Enc, slot: Option<PlayerId>) {
    e.u8(slot.map_or(0xFF, |s| s.0));
}

fn decode_opt_slot(d: &mut Dec) -> Result<Option<PlayerId>> {
    match d.u8()? {
        0xFF => Ok(None),
        s if (s as usize) < MAX_PLAYERS => Ok(Some(PlayerId(s))),
        _ => Err(NetError::Malformed("player slot out of range")),
    }
}

impl Message {
    fn encode(&self, e: &mut Enc) {
        match self {
            Message::Hello(h) => {
                e.u8(tag::HELLO);
                e.u32(HELLO_MAGIC);
                e.u32(PROTOCOL_VERSION);
                e.str(&h.name);
                e.u8(match h.role {
                    Role::Player => 0,
                    Role::Observer => 1,
                });
                e.bool(h.token.is_some());
                e.u64(h.token.unwrap_or(0));
                e.u64(h.content.map_id);
                e.u64(h.content.blueprint_hash);
                e.bytes(&h.setup);
            }
            Message::Welcome(w) => {
                e.u8(tag::WELCOME);
                encode_opt_slot(e, w.slot);
                e.u64(w.token);
                e.u8(w.config.max_players);
                e.u32(w.config.input_delay);
                e.u32(w.config.tick_ms);
                e.bool(w.in_progress);
            }
            Message::Refused { reason, detail } => {
                e.u8(tag::REFUSED);
                e.u8(reason.code());
                e.str(detail);
            }
            Message::Lobby(l) => {
                e.u8(tag::LOBBY);
                encode_opt_slot(e, l.host);
                e.u8(l.players.len() as u8);
                for p in &l.players {
                    e.u8(p.slot.0);
                    e.str(&p.name);
                    e.bool(p.ready);
                    e.bytes(&p.setup);
                }
                e.u16(l.observers);
                e.bytes(&l.options);
            }
            Message::Ready(r) => {
                e.u8(tag::READY);
                e.bool(*r);
            }
            Message::SetSetup(b) => {
                e.u8(tag::SET_SETUP);
                e.bytes(b);
            }
            Message::SetOptions(b) => {
                e.u8(tag::SET_OPTIONS);
                e.bytes(b);
            }
            Message::StartRequest => e.u8(tag::START_REQUEST),
            Message::Start(s) => {
                e.u8(tag::START);
                s.encode(e);
            }
            Message::Commands { tick, commands } => {
                e.u8(tag::COMMANDS);
                e.u32(*tick);
                encode_commands(e, commands);
            }
            Message::Bundle(b) => {
                e.u8(tag::BUNDLE);
                b.encode(e);
            }
            Message::Hash { tick, hash } => {
                e.u8(tag::HASH);
                e.u32(*tick);
                e.u64(*hash);
            }
            Message::Desync { tick, hashes } => {
                e.u8(tag::DESYNC);
                e.u32(*tick);
                e.u8(hashes.len() as u8);
                for (slot, hash) in hashes {
                    e.u8(slot.0);
                    e.u64(*hash);
                }
            }
            Message::SnapshotRequest { tick } => {
                e.u8(tag::SNAPSHOT_REQUEST);
                e.u32(*tick);
            }
            Message::SnapshotChunk {
                tick,
                total_len,
                offset,
                data,
            } => {
                e.u8(tag::SNAPSHOT_CHUNK);
                e.u32(*tick);
                e.u32(*total_len);
                e.u32(*offset);
                e.bytes(data);
            }
            Message::PlayerDropped(s) => {
                e.u8(tag::PLAYER_DROPPED);
                e.u8(s.0);
            }
            Message::PlayerRejoined(s) => {
                e.u8(tag::PLAYER_REJOINED);
                e.u8(s.0);
            }
            Message::Ping(n) => {
                e.u8(tag::PING);
                e.u32(*n);
            }
            Message::Pong(n) => {
                e.u8(tag::PONG);
                e.u32(*n);
            }
            Message::Chat { from, text } => {
                e.u8(tag::CHAT);
                encode_opt_slot(e, *from);
                e.str(text);
            }
            Message::Leave => e.u8(tag::LEAVE),
            Message::MatchEnd => e.u8(tag::MATCH_END),
        }
    }

    fn decode(d: &mut Dec) -> Result<Message> {
        Ok(match d.u8()? {
            tag::HELLO => {
                if d.u32()? != HELLO_MAGIC {
                    return Err(NetError::Malformed("not a Meridian Conflict client"));
                }
                // The rest of the layout belongs to the peer's version; stop here if it is not ours.
                let version = d.u32()?;
                if version != PROTOCOL_VERSION {
                    return Err(NetError::Version { theirs: version });
                }
                let name = d.str(MAX_NAME_LEN)?;
                let role = match d.u8()? {
                    0 => Role::Player,
                    1 => Role::Observer,
                    _ => return Err(NetError::Malformed("unknown role")),
                };
                let has_token = d.bool()?;
                let token = d.u64()?;
                let content = ContentId {
                    map_id: d.u64()?,
                    blueprint_hash: d.u64()?,
                };
                let setup = d.bytes(MAX_SETUP_LEN)?;
                Message::Hello(Hello {
                    name,
                    role,
                    token: has_token.then_some(token),
                    content,
                    setup,
                })
            }
            tag::WELCOME => Message::Welcome(Welcome {
                slot: decode_opt_slot(d)?,
                token: d.u64()?,
                config: MatchConfig {
                    max_players: d.u8()?,
                    input_delay: d.u32()?,
                    tick_ms: d.u32()?,
                },
                in_progress: d.bool()?,
            }),
            tag::REFUSED => Message::Refused {
                reason: RefuseReason::from_code(d.u8()?),
                detail: d.str(MAX_DETAIL_LEN)?,
            },
            tag::LOBBY => {
                let host = decode_opt_slot(d)?;
                let n = d.u8()? as usize;
                if n > MAX_PLAYERS {
                    return Err(NetError::Malformed("too many lobby players"));
                }
                let mut players = Vec::with_capacity(n);
                for _ in 0..n {
                    players.push(LobbyPlayer {
                        slot: decode_slot(d)?,
                        name: d.str(MAX_NAME_LEN)?,
                        ready: d.bool()?,
                        setup: d.bytes(MAX_SETUP_LEN)?,
                    });
                }
                Message::Lobby(LobbyState {
                    host,
                    players,
                    observers: d.u16()?,
                    options: d.bytes(MAX_OPTIONS_LEN)?,
                })
            }
            tag::READY => Message::Ready(d.bool()?),
            tag::SET_SETUP => Message::SetSetup(d.bytes(MAX_SETUP_LEN)?),
            tag::SET_OPTIONS => Message::SetOptions(d.bytes(MAX_OPTIONS_LEN)?),
            tag::START_REQUEST => Message::StartRequest,
            tag::START => Message::Start(MatchStart::decode(d)?),
            tag::COMMANDS => Message::Commands {
                tick: d.u32()?,
                commands: decode_commands(d)?,
            },
            tag::BUNDLE => Message::Bundle(TickBundle::decode(d)?),
            tag::HASH => Message::Hash {
                tick: d.u32()?,
                hash: d.u64()?,
            },
            tag::DESYNC => {
                let tick = d.u32()?;
                let n = d.u8()? as usize;
                if n > MAX_PLAYERS {
                    return Err(NetError::Malformed("too many desync entries"));
                }
                let mut hashes = Vec::with_capacity(n);
                for _ in 0..n {
                    hashes.push((decode_slot(d)?, d.u64()?));
                }
                Message::Desync { tick, hashes }
            }
            tag::SNAPSHOT_REQUEST => Message::SnapshotRequest { tick: d.u32()? },
            tag::SNAPSHOT_CHUNK => Message::SnapshotChunk {
                tick: d.u32()?,
                total_len: d.u32()?,
                offset: d.u32()?,
                data: d.bytes(SNAPSHOT_CHUNK_LEN)?,
            },
            tag::PLAYER_DROPPED => Message::PlayerDropped(decode_slot(d)?),
            tag::PLAYER_REJOINED => Message::PlayerRejoined(decode_slot(d)?),
            tag::PING => Message::Ping(d.u32()?),
            tag::PONG => Message::Pong(d.u32()?),
            tag::CHAT => Message::Chat {
                from: decode_opt_slot(d)?,
                text: d.str(MAX_CHAT_LEN)?,
            },
            tag::LEAVE => Message::Leave,
            tag::MATCH_END => Message::MatchEnd,
            _ => return Err(NetError::Malformed("unknown message tag")),
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::Rng;

    fn samples() -> Vec<Message> {
        let content = ContentId {
            map_id: 0x1122_3344_5566_7788,
            blueprint_hash: 42,
        };
        let start = MatchStart {
            content,
            seed: 0xFEED,
            input_delay: 2,
            players: vec![
                PlayerSetup {
                    slot: PlayerId(0),
                    name: "ada".into(),
                    data: vec![1, 2, 3],
                },
                PlayerSetup {
                    slot: PlayerId(3),
                    name: "grace".into(),
                    data: vec![],
                },
            ],
            options: vec![9; 17],
        };
        vec![
            Message::Hello(Hello {
                name: "ada".into(),
                role: Role::Player,
                token: None,
                content,
                setup: vec![5],
            }),
            Message::Hello(Hello {
                name: "".into(),
                role: Role::Observer,
                token: Some(77),
                content,
                setup: vec![],
            }),
            Message::Welcome(Welcome {
                slot: Some(PlayerId(7)),
                token: u64::MAX,
                config: MatchConfig {
                    max_players: 8,
                    input_delay: 3,
                    tick_ms: 100,
                },
                in_progress: true,
            }),
            Message::Welcome(Welcome {
                slot: None,
                token: 0,
                config: MatchConfig {
                    max_players: 2,
                    input_delay: 1,
                    tick_ms: 1,
                },
                in_progress: false,
            }),
            Message::Refused {
                reason: RefuseReason::ContentMismatch,
                detail: "map differs".into(),
            },
            Message::Lobby(LobbyState {
                host: Some(PlayerId(0)),
                players: vec![LobbyPlayer {
                    slot: PlayerId(0),
                    name: "ada".into(),
                    ready: true,
                    setup: vec![4, 4],
                }],
                observers: 3,
                options: vec![1],
            }),
            Message::Lobby(LobbyState::default()),
            Message::Ready(true),
            Message::SetSetup(vec![1, 2]),
            Message::SetOptions(vec![]),
            Message::StartRequest,
            Message::Start(start),
            Message::Commands {
                tick: 9,
                commands: vec![],
            },
            Message::Commands {
                tick: u32::MAX,
                commands: vec![vec![], vec![1], vec![0; 300]],
            },
            Message::Bundle(TickBundle::empty(0)),
            Message::Bundle(TickBundle::new(
                12,
                [
                    (PlayerId(5), vec![vec![1, 2]]),
                    (PlayerId(1), vec![vec![], vec![3]]),
                ],
            )),
            Message::Hash {
                tick: 4,
                hash: 0xABCD_EF01_2345_6789,
            },
            Message::Desync {
                tick: 4,
                hashes: vec![(PlayerId(0), 1), (PlayerId(1), 2)],
            },
            Message::SnapshotRequest { tick: 100 },
            Message::SnapshotChunk {
                tick: 100,
                total_len: 10,
                offset: 5,
                data: vec![1, 2, 3, 4, 5],
            },
            Message::PlayerDropped(PlayerId(2)),
            Message::PlayerRejoined(PlayerId(2)),
            Message::Ping(123),
            Message::Pong(123),
            Message::Chat {
                from: None,
                text: "gl hf".into(),
            },
            Message::Chat {
                from: Some(PlayerId(1)),
                text: "".into(),
            },
            Message::Leave,
            Message::MatchEnd,
        ]
    }

    #[test]
    fn every_message_round_trips() {
        let mut stream = Vec::new();
        for m in samples() {
            write_frame(&mut stream, &m).unwrap();
        }
        let mut r = stream.as_slice();
        for m in samples() {
            assert_eq!(read_frame(&mut r).unwrap(), m);
        }
        assert!(matches!(read_frame(&mut r), Err(NetError::Closed)));
    }

    #[test]
    fn bundle_is_canonical() {
        let b = TickBundle::new(
            1,
            [
                (PlayerId(2), vec![vec![9]]),
                (PlayerId(0), vec![]),
                (PlayerId(1), vec![vec![1]]),
                (PlayerId(2), vec![vec![8]]),
            ],
        );
        let order: Vec<(u8, Vec<u8>)> = b.commands().map(|(s, c)| (s.0, c.to_vec())).collect();
        assert_eq!(order, vec![(1, vec![1]), (2, vec![9]), (2, vec![8])]);

        // Out-of-order and empty slots are rejected so equal bundles have equal bytes.
        let mut e = Enc::new();
        e.u8(tag::BUNDLE);
        e.u32(1);
        e.u8(2);
        for slot in [3u8, 1] {
            e.u8(slot);
            e.u32(1);
            e.bytes(&[0]);
        }
        assert!(matches!(
            decode_payload(&e.buf),
            Err(NetError::Malformed(_))
        ));
        let mut e = Enc::new();
        e.u8(tag::BUNDLE);
        e.u32(1);
        e.u8(1);
        e.u8(0);
        e.u32(0);
        assert!(matches!(
            decode_payload(&e.buf),
            Err(NetError::Malformed(_))
        ));
    }

    #[test]
    fn oversized_frames_are_rejected_both_ways() {
        let mut header = ((MAX_FRAME_LEN + 1) as u32).to_le_bytes().to_vec();
        header.extend_from_slice(&[0; 16]);
        assert!(matches!(
            read_frame(&mut header.as_slice()),
            Err(NetError::FrameTooLarge { .. })
        ));
        assert!(matches!(
            read_frame(&mut [0u8; 4].as_slice()),
            Err(NetError::Malformed(_))
        ));

        let too_big = Message::Bundle(TickBundle {
            tick: 0,
            players: (0..8)
                .map(|s| PlayerCommands {
                    slot: PlayerId(s),
                    commands: vec![vec![0; 60_000]; 3],
                })
                .collect(),
        });
        assert!(matches!(
            encode_frame(&too_big),
            Err(NetError::FrameTooLarge { .. })
        ));

        // A full-budget bundle from eight players does fit.
        let mut pending = vec![vec![0u8; 1020]; 200];
        let mut budget = MAX_COMMANDS_BYTES;
        let per_player = take_commands(&mut pending, &mut budget);
        assert_eq!(per_player.len(), MAX_COMMANDS_BYTES / 1024);
        assert_eq!(pending.len(), 200 - per_player.len());
        let full = Message::Bundle(TickBundle {
            tick: 0,
            players: (0..8)
                .map(|s| PlayerCommands {
                    slot: PlayerId(s),
                    commands: per_player.clone(),
                })
                .collect(),
        });
        let frame = encode_frame(&full).unwrap();
        assert_eq!(read_frame(&mut frame.as_slice()).unwrap(), full);
    }

    #[test]
    fn truncated_and_foreign_input_is_an_error() {
        assert!(matches!(
            read_frame(&mut [1u8, 0].as_slice()),
            Err(NetError::Io(_))
        ));
        assert!(matches!(
            read_frame(&mut [8u8, 0, 0, 0, 1, 2].as_slice()),
            Err(NetError::Io(_))
        ));
        assert!(matches!(
            decode_payload(&[200]),
            Err(NetError::Malformed(_))
        ));
        assert!(matches!(
            decode_payload(&[tag::PING, 1]),
            Err(NetError::Malformed(_))
        ));
        assert!(matches!(
            decode_payload(&[tag::LEAVE, 0]),
            Err(NetError::Malformed(_))
        ));
        assert!(matches!(
            decode_payload(&[tag::PLAYER_DROPPED, 8]),
            Err(NetError::Malformed(_))
        ));

        // Over-budget command list.
        let mut e = Enc::new();
        e.u8(tag::COMMANDS);
        e.u32(0);
        e.u32(2);
        e.bytes(&vec![0; MAX_COMMAND_LEN]);
        e.bytes(&vec![0; MAX_COMMAND_LEN]);
        assert!(matches!(
            decode_payload(&e.buf),
            Err(NetError::Malformed(_))
        ));
        // Command count that the payload cannot possibly hold.
        let mut e = Enc::new();
        e.u8(tag::COMMANDS);
        e.u32(0);
        e.u32(u32::MAX);
        assert!(matches!(
            decode_payload(&e.buf),
            Err(NetError::Malformed(_))
        ));

        // A future client is told apart from garbage.
        let mut e = Enc::new();
        e.u8(tag::HELLO);
        e.u32(HELLO_MAGIC);
        e.u32(PROTOCOL_VERSION + 1);
        e.u64(0xFFFF_FFFF_FFFF_FFFF);
        assert!(
            matches!(decode_payload(&e.buf), Err(NetError::Version { theirs }) if theirs == PROTOCOL_VERSION + 1)
        );
    }

    #[test]
    fn decoder_never_panics_on_noise() {
        let mut rng = Rng::new(99);
        // Pure noise, then valid frames with random corruption.
        for _ in 0..2000 {
            let len = rng.below(64) as usize;
            let noise: Vec<u8> = (0..len).map(|_| rng.below(256) as u8).collect();
            let _ = decode_payload(&noise);
        }
        for m in samples() {
            let frame = encode_frame(&m).unwrap();
            for _ in 0..200 {
                let mut bad = frame[4..].to_vec();
                let i = rng.below(bad.len() as u32) as usize;
                bad[i] = rng.below(256) as u8;
                bad.truncate(bad.len() - rng.below(2) as usize);
                let _ = decode_payload(&bad);
            }
        }
    }

    #[test]
    fn snapshots_chunk_and_reassemble() {
        let mut rng = Rng::new(5);
        for len in [0usize, 1, SNAPSHOT_CHUNK_LEN, SNAPSHOT_CHUNK_LEN * 2 + 17] {
            let blob: Vec<u8> = (0..len).map(|_| rng.below(256) as u8).collect();
            let mut asm = SnapshotAssembler::new();
            let mut out = None;
            for m in snapshot_chunks(31, &blob).unwrap() {
                assert!(out.is_none());
                // Each chunk must survive framing.
                let frame = encode_frame(&m).unwrap();
                match read_frame(&mut frame.as_slice()).unwrap() {
                    Message::SnapshotChunk {
                        tick,
                        total_len,
                        offset,
                        data,
                    } => {
                        out = asm.push(tick, total_len, offset, &data).unwrap();
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert_eq!(out, Some((31, blob)));
        }
        let mut asm = SnapshotAssembler::new();
        assert!(asm.push(1, 10, 5, &[0; 5]).is_err());
        let mut asm = SnapshotAssembler::new();
        asm.push(1, 10, 0, &[0; 5]).unwrap();
        assert!(asm.push(2, 10, 5, &[0; 5]).is_err());
        let mut asm = SnapshotAssembler::new();
        asm.push(1, 10, 0, &[0; 5]).unwrap();
        assert!(asm.push(1, 10, 5, &[0; 6]).is_err());
    }
}
