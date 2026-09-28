//! Every message of the directory protocol and its byte layout.

use mc_core::MAX_PLAYERS;

use super::{
    decode_content, encode_content, DirHello, DirRefuseReason, NewRoom, RoomCode, RoomListing,
    DIRECTORY_VERSION, DIR_MAGIC, MAX_DETAIL_LEN, MAX_LISTED_ROOMS, MAX_MOTD_LEN, NONCE_LEN,
    TICKET_LEN,
};
use crate::protocol::{MAX_BUILD_LEN, MAX_NAME_LEN, MAX_TITLE_LEN};
use crate::wire::{Dec, Enc, NetError, Result};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DirMessage {
    // client -> server
    Hello(DirHello),
    /// The challenge's nonce, signed with the key from `Hello`.
    Proof {
        signature: [u8; 64],
    },
    CreateRoom(NewRoom),
    FindRoom(RoomCode),
    /// Start (`true`) or stop receiving `Rooms` and `Stats` as they change.
    Subscribe(bool),
    Leave,
    // server -> client
    Challenge {
        nonce: [u8; NONCE_LEN],
    },
    SignedIn {
        /// The name as the server knows it: the one asked for, in its casing.
        name: String,
        /// Present in a match connection's `Hello` to be seated under `name`.
        ticket: [u8; TICKET_LEN],
        online: u32,
        rooms: u32,
        /// The operator's message of the day.
        motd: String,
    },
    /// Sign-in failed; the server closes the connection. Layout is frozen across
    /// versions so a mismatched client can read it.
    Refused {
        reason: DirRefuseReason,
        detail: String,
    },
    RoomCreated(RoomCode),
    /// `CreateRoom` failed; the connection stays.
    RoomRefused {
        reason: DirRefuseReason,
        detail: String,
    },
    RoomFound(RoomListing),
    RoomNotFound(RoomCode),
    /// Every public room, sent when the list changes.
    Rooms(Vec<RoomListing>),
    Stats {
        online: u32,
        rooms: u32,
    },
    // both directions
    Ping(u32),
    Pong(u32),
}

/// `DirHello`'s tag, which the server's front door looks for.
pub(super) const DIR_HELLO: u8 = tag::HELLO;

// Tags are the wire format: never renumber, never reuse a retired one. They
// start at 100 so they never meet the match protocol's.
mod tag {
    pub(super) const HELLO: u8 = 100;
    pub(super) const CHALLENGE: u8 = 101;
    pub(super) const PROOF: u8 = 102;
    pub(super) const SIGNED_IN: u8 = 103;
    pub(super) const REFUSED: u8 = 104;
    pub(super) const CREATE_ROOM: u8 = 105;
    pub(super) const ROOM_CREATED: u8 = 106;
    pub(super) const ROOM_REFUSED: u8 = 107;
    pub(super) const FIND_ROOM: u8 = 108;
    pub(super) const ROOM_FOUND: u8 = 109;
    pub(super) const ROOM_NOT_FOUND: u8 = 110;
    pub(super) const SUBSCRIBE: u8 = 111;
    pub(super) const ROOMS: u8 = 112;
    pub(super) const STATS: u8 = 113;
    pub(super) const PING: u8 = 114;
    pub(super) const PONG: u8 = 115;
    pub(super) const LEAVE: u8 = 116;
}

impl DirMessage {
    pub(super) fn encode(&self, e: &mut Enc) {
        match self {
            DirMessage::Hello(h) => {
                e.u8(tag::HELLO);
                e.u32(DIR_MAGIC);
                e.u32(DIRECTORY_VERSION);
                e.str(&h.name);
                e.raw(&h.public_key);
                e.str(&h.build);
            }
            DirMessage::Proof { signature } => {
                e.u8(tag::PROOF);
                e.raw(signature);
            }
            DirMessage::CreateRoom(r) => {
                e.u8(tag::CREATE_ROOM);
                e.str(&r.title);
                e.u8(r.seats);
                e.bool(r.private);
                encode_content(e, Some(r.content));
                e.str(&r.build);
            }
            DirMessage::FindRoom(code) => {
                e.u8(tag::FIND_ROOM);
                e.u32(code.to_u32());
            }
            DirMessage::Subscribe(on) => {
                e.u8(tag::SUBSCRIBE);
                e.bool(*on);
            }
            DirMessage::Leave => e.u8(tag::LEAVE),
            DirMessage::Challenge { nonce } => {
                e.u8(tag::CHALLENGE);
                e.raw(nonce);
            }
            DirMessage::SignedIn {
                name,
                ticket,
                online,
                rooms,
                motd,
            } => {
                e.u8(tag::SIGNED_IN);
                e.str(name);
                e.raw(ticket);
                e.u32(*online);
                e.u32(*rooms);
                e.str(motd);
            }
            DirMessage::Refused { reason, detail } => {
                e.u8(tag::REFUSED);
                e.u8(reason.code());
                e.str(detail);
            }
            DirMessage::RoomCreated(code) => {
                e.u8(tag::ROOM_CREATED);
                e.u32(code.to_u32());
            }
            DirMessage::RoomRefused { reason, detail } => {
                e.u8(tag::ROOM_REFUSED);
                e.u8(reason.code());
                e.str(detail);
            }
            DirMessage::RoomFound(listing) => {
                e.u8(tag::ROOM_FOUND);
                listing.encode(e);
            }
            DirMessage::RoomNotFound(code) => {
                e.u8(tag::ROOM_NOT_FOUND);
                e.u32(code.to_u32());
            }
            DirMessage::Rooms(rooms) => {
                e.u8(tag::ROOMS);
                e.u16(rooms.len() as u16);
                for r in rooms {
                    r.encode(e);
                }
            }
            DirMessage::Stats { online, rooms } => {
                e.u8(tag::STATS);
                e.u32(*online);
                e.u32(*rooms);
            }
            DirMessage::Ping(n) => {
                e.u8(tag::PING);
                e.u32(*n);
            }
            DirMessage::Pong(n) => {
                e.u8(tag::PONG);
                e.u32(*n);
            }
        }
    }

    pub(super) fn decode(d: &mut Dec) -> Result<DirMessage> {
        Ok(match d.u8()? {
            tag::HELLO => {
                if d.u32()? != DIR_MAGIC {
                    return Err(NetError::Malformed("not a Meridian Conflict client"));
                }
                // The rest of the layout belongs to the peer's version; stop here if it is not ours.
                let version = d.u32()?;
                if version != DIRECTORY_VERSION {
                    return Err(NetError::Version { theirs: version });
                }
                DirMessage::Hello(DirHello {
                    name: d.str(MAX_NAME_LEN)?,
                    public_key: d.array()?,
                    build: d.str(MAX_BUILD_LEN)?,
                })
            }
            tag::PROOF => DirMessage::Proof {
                signature: d.array()?,
            },
            tag::CREATE_ROOM => {
                let title = d.str(MAX_TITLE_LEN)?;
                let seats = d.u8()?;
                if seats == 0 || seats as usize > MAX_PLAYERS {
                    return Err(NetError::Malformed("seats out of range"));
                }
                let private = d.bool()?;
                let Some(content) = decode_content(d)? else {
                    return Err(NetError::Malformed("a new room names its content"));
                };
                DirMessage::CreateRoom(NewRoom {
                    title,
                    seats,
                    private,
                    content,
                    build: d.str(MAX_BUILD_LEN)?,
                })
            }
            tag::FIND_ROOM => DirMessage::FindRoom(RoomCode::decode(d)?),
            tag::SUBSCRIBE => DirMessage::Subscribe(d.bool()?),
            tag::LEAVE => DirMessage::Leave,
            tag::CHALLENGE => DirMessage::Challenge { nonce: d.array()? },
            tag::SIGNED_IN => DirMessage::SignedIn {
                name: d.str(MAX_NAME_LEN)?,
                ticket: d.array()?,
                online: d.u32()?,
                rooms: d.u32()?,
                motd: d.str(MAX_MOTD_LEN)?,
            },
            tag::REFUSED => DirMessage::Refused {
                reason: DirRefuseReason::from_code(d.u8()?),
                detail: d.str(MAX_DETAIL_LEN)?,
            },
            tag::ROOM_CREATED => DirMessage::RoomCreated(RoomCode::decode(d)?),
            tag::ROOM_REFUSED => DirMessage::RoomRefused {
                reason: DirRefuseReason::from_code(d.u8()?),
                detail: d.str(MAX_DETAIL_LEN)?,
            },
            tag::ROOM_FOUND => DirMessage::RoomFound(RoomListing::decode(d)?),
            tag::ROOM_NOT_FOUND => DirMessage::RoomNotFound(RoomCode::decode(d)?),
            tag::ROOMS => {
                let n = d.u16()? as usize;
                if n > MAX_LISTED_ROOMS {
                    return Err(NetError::Malformed("too many rooms listed"));
                }
                // No preallocation from the count: each listing proves itself byte by byte.
                let mut rooms = Vec::new();
                for _ in 0..n {
                    rooms.push(RoomListing::decode(d)?);
                }
                DirMessage::Rooms(rooms)
            }
            tag::STATS => DirMessage::Stats {
                online: d.u32()?,
                rooms: d.u32()?,
            },
            tag::PING => DirMessage::Ping(d.u32()?),
            tag::PONG => DirMessage::Pong(d.u32()?),
            _ => return Err(NetError::Malformed("unknown directory message tag")),
        })
    }
}
