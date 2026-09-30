//! Round trips, canonical forms and hostile input for the match protocol.

use super::message::tag;
use super::*;
use crate::wire::Enc;
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
            build: "0.1.0+abc".into(),
            room: 0,
            ticket: None,
        }),
        Message::Hello(Hello {
            name: "".into(),
            role: Role::Observer,
            token: Some(77),
            content,
            setup: vec![],
            build: String::new(),
            room: u32::MAX,
            ticket: Some([7; 16]),
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
                verified: true,
            }],
            observers: 3,
            options: vec![1],
            open: 0b1011,
            countdown_ms: 2500,
            title: "Friday night".into(),
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
            name: "watcher".into(),
            to: 0,
            text: "gl hf".into(),
        },
        Message::Chat {
            from: Some(PlayerId(1)),
            name: "grace".into(),
            to: 0b101,
            text: "".into(),
        },
        Message::Leave,
        Message::MatchEnd,
        Message::Loaded,
        Message::DesyncReport {
            tick: 88,
            sections: vec![1, 2, 3],
        },
        Message::DesyncReport {
            tick: 0,
            sections: vec![],
        },
        Message::Pause(true),
        Message::SetOpenSeats(0b1110),
        Message::TakeSeat(PlayerId(6)),
        Message::Kick(PlayerId(2)),
        Message::SetContent(ContentId {
            map_id: 5,
            blueprint_hash: 6,
        }),
        Message::Listing {
            map: "Halden's Grip".into(),
            mode: "4 v 4".into(),
        },
        Message::Loading { loaded: 0b11 },
        Message::DesyncDetail {
            tick: 88,
            slot: PlayerId(1),
            sections: vec![u64::MAX; 14],
        },
        Message::Clock {
            paused: true,
            by: Some(PlayerId(3)),
            input_delay: 4,
        },
        Message::Clock {
            paused: false,
            by: None,
            input_delay: 1,
        },
        Message::NetStats(vec![
            PeerStat {
                slot: PlayerId(0),
                rtt_ms: 43,
                link: Link::Connected,
            },
            PeerStat {
                slot: PlayerId(4),
                rtt_ms: u16::MAX,
                link: Link::Dropped,
            },
        ]),
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
        players: (0..MAX_PLAYERS as u8)
            .map(|s| PlayerCommands {
                slot: PlayerId(s),
                commands: vec![vec![0; 60_000]; MAX_FRAME_LEN / 60_000 / MAX_PLAYERS + 1],
            })
            .collect(),
    });
    assert!(matches!(
        encode_frame(&too_big),
        Err(NetError::FrameTooLarge { .. })
    ));

    // A full-budget bundle from every player a match can have does fit.
    let mut pending = vec![vec![0u8; 1020]; 200];
    let mut budget = MAX_COMMANDS_BYTES;
    let per_player = take_commands(&mut pending, &mut budget);
    assert_eq!(per_player.len(), MAX_COMMANDS_BYTES / 1024);
    assert_eq!(pending.len(), 200 - per_player.len());
    let full = Message::Bundle(TickBundle {
        tick: 0,
        players: (0..MAX_PLAYERS as u8)
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
        decode_payload(&[tag::PLAYER_DROPPED, MAX_PLAYERS as u8]),
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
