//! Every message of the match protocol and its byte layout.

use mc_core::{PlayerId, MAX_PLAYERS};

use super::{
    decode_commands, decode_slot, encode_commands, ContentId, Hello, Link, LobbyPlayer, LobbyState,
    MatchConfig, MatchStart, PeerStat, RefuseReason, Role, TickBundle, Welcome, HELLO_MAGIC,
    MAX_BUILD_LEN, MAX_CHAT_LEN, MAX_DETAIL_LEN, MAX_NAME_LEN, MAX_OPTIONS_LEN, MAX_SECTIONS,
    MAX_SETUP_LEN, MAX_TITLE_LEN, PROTOCOL_VERSION, SNAPSHOT_CHUNK_LEN,
};
use crate::wire::{Dec, Enc, NetError, Result};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Message {
    // client -> relay
    Hello(Hello),
    Ready(bool),
    SetSetup(Vec<u8>),
    /// Host only.
    SetOptions(Vec<u8>),
    /// Host only. Once every other player is ready, starts the countdown.
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
    /// This machine has built the match from `Start`; the clock waits for everyone.
    Loaded,
    /// After `Desync`: this machine's state hash sections at that tick.
    DesyncReport {
        tick: u32,
        sections: Vec<u64>,
    },
    /// Stop (`true`) or restart the match clock. Any player may do either.
    Pause(bool),
    /// Host only, in the lobby: the seats a person may take, one bit per slot.
    SetOpenSeats(u8),
    /// In the lobby: move to this free, open seat.
    TakeSeat(PlayerId),
    /// Host only, in the lobby: remove that player.
    Kick(PlayerId),
    /// Host only: what the game browser shows for this room (map name, mode).
    Listing {
        map: String,
        mode: String,
    },
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
    /// After `Start`: the slots that have loaded, one bit each.
    Loading {
        loaded: u8,
    },
    /// One player's `DesyncReport`, passed on to everyone.
    DesyncDetail {
        tick: u32,
        slot: PlayerId,
        sections: Vec<u64>,
    },
    /// The match clock: paused (and by whom; `None` is an observer or the relay)
    /// and the input delay commands are now stamped with.
    Clock {
        paused: bool,
        by: Option<PlayerId>,
        input_delay: u32,
    },
    /// Every seat's link, about once a second.
    NetStats(Vec<PeerStat>),
    // both directions
    SnapshotChunk {
        tick: u32,
        total_len: u32,
        offset: u32,
        data: Vec<u8>,
    },
    Ping(u32),
    Pong(u32),
    /// Filled in by the relay: `from` (`None` is an observer) and `name`. `to` is
    /// a mask of the slots it is for, the sender included; 0 is everyone.
    Chat {
        from: Option<PlayerId>,
        name: String,
        to: u8,
        text: String,
    },
}

// Tags are the wire format: never renumber, never reuse a retired one.
pub(super) mod tag {
    pub(in crate::protocol) const HELLO: u8 = 1;
    pub(in crate::protocol) const WELCOME: u8 = 2;
    pub(in crate::protocol) const REFUSED: u8 = 3;
    pub(in crate::protocol) const LOBBY: u8 = 4;
    pub(in crate::protocol) const READY: u8 = 5;
    pub(in crate::protocol) const SET_SETUP: u8 = 6;
    pub(in crate::protocol) const SET_OPTIONS: u8 = 7;
    pub(in crate::protocol) const START_REQUEST: u8 = 8;
    pub(in crate::protocol) const START: u8 = 9;
    pub(in crate::protocol) const COMMANDS: u8 = 10;
    pub(in crate::protocol) const BUNDLE: u8 = 11;
    pub(in crate::protocol) const HASH: u8 = 12;
    pub(in crate::protocol) const DESYNC: u8 = 13;
    pub(in crate::protocol) const SNAPSHOT_REQUEST: u8 = 14;
    pub(in crate::protocol) const SNAPSHOT_CHUNK: u8 = 15;
    pub(in crate::protocol) const PLAYER_DROPPED: u8 = 16;
    pub(in crate::protocol) const PLAYER_REJOINED: u8 = 17;
    pub(in crate::protocol) const PING: u8 = 18;
    pub(in crate::protocol) const PONG: u8 = 19;
    pub(in crate::protocol) const CHAT: u8 = 20;
    pub(in crate::protocol) const LEAVE: u8 = 21;
    pub(in crate::protocol) const MATCH_END: u8 = 22;
    pub(in crate::protocol) const LOADED: u8 = 23;
    pub(in crate::protocol) const DESYNC_REPORT: u8 = 24;
    pub(in crate::protocol) const PAUSE: u8 = 25;
    pub(in crate::protocol) const SET_OPEN_SEATS: u8 = 26;
    pub(in crate::protocol) const TAKE_SEAT: u8 = 27;
    pub(in crate::protocol) const KICK: u8 = 28;
    pub(in crate::protocol) const LOADING: u8 = 29;
    pub(in crate::protocol) const DESYNC_DETAIL: u8 = 30;
    pub(in crate::protocol) const CLOCK: u8 = 31;
    pub(in crate::protocol) const NET_STATS: u8 = 32;
    pub(in crate::protocol) const LISTING: u8 = 33;
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

fn encode_sections(e: &mut Enc, sections: &[u64]) {
    e.u8(sections.len() as u8);
    for &s in sections {
        e.u64(s);
    }
}

fn decode_sections(d: &mut Dec) -> Result<Vec<u64>> {
    let n = d.u8()? as usize;
    if n > MAX_SECTIONS {
        return Err(NetError::Malformed("too many hash sections"));
    }
    (0..n).map(|_| d.u64()).collect()
}

fn link_code(link: Link) -> u8 {
    match link {
        Link::Connected => 0,
        Link::Loading => 1,
        Link::Lagging => 2,
        Link::Dropped => 3,
    }
}

fn decode_link(d: &mut Dec) -> Result<Link> {
    Ok(match d.u8()? {
        0 => Link::Connected,
        1 => Link::Loading,
        2 => Link::Lagging,
        3 => Link::Dropped,
        _ => return Err(NetError::Malformed("unknown link state")),
    })
}

impl Message {
    pub(super) fn encode(&self, e: &mut Enc) {
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
                e.str(&h.build);
                e.u32(h.room);
                e.bool(h.ticket.is_some());
                e.raw(&h.ticket.unwrap_or_default());
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
                    e.bool(p.verified);
                }
                e.u16(l.observers);
                e.bytes(&l.options);
                e.u8(l.open);
                e.u32(l.countdown_ms);
                e.str(&l.title);
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
            Message::Chat {
                from,
                name,
                to,
                text,
            } => {
                e.u8(tag::CHAT);
                encode_opt_slot(e, *from);
                e.str(name);
                e.u8(*to);
                e.str(text);
            }
            Message::Leave => e.u8(tag::LEAVE),
            Message::MatchEnd => e.u8(tag::MATCH_END),
            Message::Loaded => e.u8(tag::LOADED),
            Message::DesyncReport { tick, sections } => {
                e.u8(tag::DESYNC_REPORT);
                e.u32(*tick);
                encode_sections(e, sections);
            }
            Message::Pause(paused) => {
                e.u8(tag::PAUSE);
                e.bool(*paused);
            }
            Message::SetOpenSeats(mask) => {
                e.u8(tag::SET_OPEN_SEATS);
                e.u8(*mask);
            }
            Message::TakeSeat(s) => {
                e.u8(tag::TAKE_SEAT);
                e.u8(s.0);
            }
            Message::Kick(s) => {
                e.u8(tag::KICK);
                e.u8(s.0);
            }
            Message::Listing { map, mode } => {
                e.u8(tag::LISTING);
                e.str(map);
                e.str(mode);
            }
            Message::Loading { loaded } => {
                e.u8(tag::LOADING);
                e.u8(*loaded);
            }
            Message::DesyncDetail {
                tick,
                slot,
                sections,
            } => {
                e.u8(tag::DESYNC_DETAIL);
                e.u32(*tick);
                e.u8(slot.0);
                encode_sections(e, sections);
            }
            Message::Clock {
                paused,
                by,
                input_delay,
            } => {
                e.u8(tag::CLOCK);
                e.bool(*paused);
                encode_opt_slot(e, *by);
                e.u32(*input_delay);
            }
            Message::NetStats(stats) => {
                e.u8(tag::NET_STATS);
                e.u8(stats.len() as u8);
                for s in stats {
                    e.u8(s.slot.0);
                    e.u16(s.rtt_ms);
                    e.u8(link_code(s.link));
                }
            }
        }
    }

    pub(super) fn decode(d: &mut Dec) -> Result<Message> {
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
                let build = d.str(MAX_BUILD_LEN)?;
                let room = d.u32()?;
                let has_ticket = d.bool()?;
                let ticket = d.array::<16>()?;
                Message::Hello(Hello {
                    name,
                    role,
                    token: has_token.then_some(token),
                    content,
                    setup,
                    build,
                    room,
                    ticket: has_ticket.then_some(ticket),
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
                        verified: d.bool()?,
                    });
                }
                Message::Lobby(LobbyState {
                    host,
                    players,
                    observers: d.u16()?,
                    options: d.bytes(MAX_OPTIONS_LEN)?,
                    open: d.u8()?,
                    countdown_ms: d.u32()?,
                    title: d.str(MAX_TITLE_LEN)?,
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
                name: d.str(MAX_NAME_LEN)?,
                to: d.u8()?,
                text: d.str(MAX_CHAT_LEN)?,
            },
            tag::LEAVE => Message::Leave,
            tag::MATCH_END => Message::MatchEnd,
            tag::LOADED => Message::Loaded,
            tag::DESYNC_REPORT => Message::DesyncReport {
                tick: d.u32()?,
                sections: decode_sections(d)?,
            },
            tag::PAUSE => Message::Pause(d.bool()?),
            tag::SET_OPEN_SEATS => Message::SetOpenSeats(d.u8()?),
            tag::TAKE_SEAT => Message::TakeSeat(decode_slot(d)?),
            tag::KICK => Message::Kick(decode_slot(d)?),
            tag::LISTING => Message::Listing {
                map: d.str(MAX_TITLE_LEN)?,
                mode: d.str(MAX_TITLE_LEN)?,
            },
            tag::LOADING => Message::Loading { loaded: d.u8()? },
            tag::DESYNC_DETAIL => Message::DesyncDetail {
                tick: d.u32()?,
                slot: decode_slot(d)?,
                sections: decode_sections(d)?,
            },
            tag::CLOCK => Message::Clock {
                paused: d.bool()?,
                by: decode_opt_slot(d)?,
                input_delay: d.u32()?,
            },
            tag::NET_STATS => {
                let n = d.u8()? as usize;
                if n > MAX_PLAYERS {
                    return Err(NetError::Malformed("too many peer stats"));
                }
                let mut stats = Vec::with_capacity(n);
                for _ in 0..n {
                    stats.push(PeerStat {
                        slot: decode_slot(d)?,
                        rtt_ms: d.u16()?,
                        link: decode_link(d)?,
                    });
                }
                Message::NetStats(stats)
            }
            _ => return Err(NetError::Malformed("unknown message tag")),
        })
    }
}
