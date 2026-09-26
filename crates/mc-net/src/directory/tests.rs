//! Round trips and hostile input for the directory protocol; room codes, names
//! and identities.

use mc_core::Rng;

use super::*;
use crate::protocol::{encode_frame, Role};

fn listing(code: u32, private: bool) -> RoomListing {
    RoomListing {
        code: RoomCode::from_u32(code).unwrap(),
        title: "Friday night".into(),
        host: "Ada".into(),
        players: vec!["Ada".into(), "Grace".into()],
        seats: 4,
        free: 2,
        observers: 3,
        phase: RoomPhase::Playing { tick: 1234 },
        map: "Twin Shoals".into(),
        mode: "2 v 2".into(),
        build: "0.1.0+abc".into(),
        content: Some(ContentId {
            map_id: 9,
            blueprint_hash: 10,
        }),
        private,
    }
}

fn samples() -> Vec<DirMessage> {
    let mut quiet = listing(1, true);
    quiet.phase = RoomPhase::Lobby;
    quiet.content = None;
    quiet.players.clear();
    vec![
        DirMessage::Hello(DirHello {
            name: "Ada".into(),
            public_key: [3; 32],
            build: "0.1.0+abc".into(),
        }),
        DirMessage::Proof {
            signature: [0xAB; 64],
        },
        DirMessage::CreateRoom(NewRoom {
            title: "Friday".into(),
            seats: 8,
            private: true,
            content: ContentId {
                map_id: u64::MAX,
                blueprint_hash: 1,
            },
            build: String::new(),
        }),
        DirMessage::FindRoom(RoomCode::from_u32(CODE_SPACE).unwrap()),
        DirMessage::Subscribe(true),
        DirMessage::Leave,
        DirMessage::Challenge { nonce: [9; 32] },
        DirMessage::SignedIn {
            name: "Ada".into(),
            ticket: [1; 16],
            online: 12,
            rooms: 3,
            motd: "Welcome.".into(),
        },
        DirMessage::Refused {
            reason: DirRefuseReason::NameTaken,
            detail: "taken".into(),
        },
        DirMessage::RoomCreated(RoomCode::from_u32(77).unwrap()),
        DirMessage::RoomRefused {
            reason: DirRefuseReason::TooManyRooms,
            detail: String::new(),
        },
        DirMessage::RoomFound(listing(5, true)),
        DirMessage::RoomNotFound(RoomCode::from_u32(6).unwrap()),
        DirMessage::Rooms(vec![listing(5, false), quiet]),
        DirMessage::Rooms(Vec::new()),
        DirMessage::Stats {
            online: 1,
            rooms: 0,
        },
        DirMessage::Ping(7),
        DirMessage::Pong(u32::MAX),
    ]
}

#[test]
fn every_message_round_trips() {
    for m in samples() {
        let frame = encode_dir_frame(&m).unwrap();
        assert_eq!(read_dir_frame(&mut frame.as_slice()).unwrap(), m);
    }
}

#[test]
fn refusal_codes_are_frozen() {
    let all = [
        (DirRefuseReason::Other, 0),
        (DirRefuseReason::VersionMismatch, 1),
        (DirRefuseReason::BadName, 2),
        (DirRefuseReason::NameTaken, 3),
        (DirRefuseReason::BadSignature, 4),
        (DirRefuseReason::ServerFull, 5),
        (DirRefuseReason::Banned, 6),
        (DirRefuseReason::TooManyConnections, 7),
        (DirRefuseReason::TooManyRooms, 8),
    ];
    for (reason, code) in all {
        assert_eq!(reason.code(), code);
        assert_eq!(DirRefuseReason::from_code(code), reason);
    }
    assert_eq!(DirRefuseReason::from_code(200), DirRefuseReason::Other);
}

#[test]
fn decoder_never_panics_on_noise() {
    let mut rng = Rng::new(7);
    for _ in 0..2000 {
        let len = rng.below(64) as usize;
        let mut noise: Vec<u8> = (0..len).map(|_| rng.below(256) as u8).collect();
        // Aim some of it at real tags.
        if let Some(first) = noise.first_mut() {
            *first = 100 + rng.below(20) as u8;
        }
        let _ = decode_dir_payload(&noise);
        let _ = read_first(&mut noise.as_slice());
    }
    for m in samples() {
        let frame = encode_dir_frame(&m).unwrap();
        for _ in 0..300 {
            let mut bad = frame[4..].to_vec();
            let i = rng.below(bad.len() as u32) as usize;
            bad[i] = rng.below(256) as u8;
            bad.truncate(bad.len() - rng.below(2) as usize);
            let _ = decode_dir_payload(&bad);
        }
    }
}

#[test]
fn hostile_counts_and_values_are_errors() {
    // A room list that announces more rooms than it may, carrying nothing.
    let mut e = Enc::new();
    e.u8(112);
    e.u16(u16::MAX);
    assert!(decode_dir_payload(&e.buf).is_err());
    // Codes out of range, including 0.
    for code in [0, CODE_SPACE + 1, u32::MAX] {
        let mut e = Enc::new();
        e.u8(108);
        e.u32(code);
        assert!(decode_dir_payload(&e.buf).is_err());
    }
    // Nine seats.
    let mut frame = encode_dir_frame(&samples()[2]).unwrap();
    let seats_at = 4 + 1 + 2 + "Friday".len();
    assert_eq!(frame[seats_at], 8);
    frame[seats_at] = 9;
    assert!(decode_dir_payload(&frame[4..]).is_err());
    // Trailing bytes.
    let mut frame = encode_dir_frame(&DirMessage::Leave).unwrap();
    frame.push(0);
    assert!(decode_dir_payload(&frame[4..]).is_err());
}

#[test]
fn the_front_door_tells_the_protocols_apart() {
    let dir = encode_dir_frame(&samples()[0]).unwrap();
    assert!(matches!(
        read_first(&mut dir.as_slice()),
        Ok(FirstFrame::Directory(h)) if h.name == "Ada"
    ));
    let hello = Message::Hello(Hello {
        name: "Bob".into(),
        role: Role::Player,
        token: None,
        content: ContentId::default(),
        setup: Vec::new(),
        build: String::new(),
        room: 42,
        ticket: Some([1; 16]),
    });
    let frame = encode_frame(&hello).unwrap();
    assert!(matches!(
        read_first(&mut frame.as_slice()),
        Ok(FirstFrame::Match(h)) if h.room == 42
    ));
    // Another version of either protocol is recognised, so it can be told why.
    let mut e = Enc::new();
    e.u32(0);
    e.u8(100);
    e.u32(DIR_MAGIC);
    e.u32(DIRECTORY_VERSION + 1);
    let mut old = e.buf;
    let len = (old.len() - 4) as u32;
    old[..4].copy_from_slice(&len.to_le_bytes());
    assert_eq!(
        read_first(&mut old.as_slice()).unwrap(),
        FirstFrame::DirectoryVersion(DIRECTORY_VERSION + 1)
    );
    // Any other first message is not a game's.
    let ping = encode_dir_frame(&DirMessage::Ping(1)).unwrap();
    assert!(read_first(&mut ping.as_slice()).is_err());
    let ping = encode_frame(&Message::Ping(1)).unwrap();
    assert!(read_first(&mut ping.as_slice()).is_err());
    assert!(read_first(&mut &b"GET / HTTP/1.1\r\n\r\n"[..]).is_err());
}

#[test]
fn room_codes_read_and_print() {
    let first = RoomCode::from_u32(1).unwrap();
    assert_eq!(first.to_string(), "222-222");
    let last = RoomCode::from_u32(CODE_SPACE).unwrap();
    assert_eq!(last.to_string(), "ZZZ-ZZZ");
    assert_eq!(CODE_SPACE, 31u32.pow(CODE_LEN));
    assert!(RoomCode::from_u32(0).is_none());
    assert!(RoomCode::from_u32(CODE_SPACE + 1).is_none());
    for text in ["k7m-q2x", "K7MQ2X", " k7m q2x "] {
        let code: RoomCode = text.parse().unwrap();
        assert_eq!(code.to_string(), "K7M-Q2X");
    }
    for bad in [
        "", "K7M-Q2", "K7M-Q2XX", "K0M-Q2X", "KIM-Q2X", "KLM-Q2X", "K7M_Q2X",
    ] {
        assert_eq!(bad.parse::<RoomCode>(), Err(BadRoomCode), "{bad}");
    }
    let mut rng = Rng::new(3);
    for _ in 0..1000 {
        let code = RoomCode::from_u32(1 + rng.below(CODE_SPACE)).unwrap();
        assert_eq!(code.to_string().parse::<RoomCode>(), Ok(code));
    }
    let random = RoomCode::random().unwrap();
    assert_eq!(RoomCode::from_u32(random.to_u32()), Some(random));
}

#[test]
fn names_follow_the_rules() {
    for good in [
        "Ada",
        "grace_hopper",
        "x",
        "A.B-C 9",
        "abcdefghijklmnopqrstuvwx",
    ] {
        assert_eq!(check_name(good), Ok(()), "{good}");
    }
    for bad in [
        "",
        " Ada",
        "Ada ",
        "Ada  Lovelace",
        "abcdefghijklmnopqrstuvwxy",
        "Ada\n",
        "Ada\u{0}",
        "Adä",
        "Ada!",
        "\u{202E}Ada",
    ] {
        assert!(check_name(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn identities_sign_and_verify() {
    let ada = Identity::generate().unwrap();
    let eve = Identity::generate().unwrap();
    assert_ne!(ada.public_key(), eve.public_key());
    let nonce = random_bytes::<NONCE_LEN>().unwrap();
    let proof = ada.sign(&nonce);
    assert!(verify_proof(&ada.public_key(), &nonce, &proof));
    assert!(!verify_proof(&eve.public_key(), &nonce, &proof));
    let mut other = nonce;
    other[0] ^= 1;
    assert!(!verify_proof(&ada.public_key(), &other, &proof));
    assert!(!verify_proof(&[0xFF; 32], &nonce, &proof));

    let again = Identity::from_bytes(ada.to_bytes());
    assert_eq!(again.public_key(), ada.public_key());
    let print = ada.fingerprint();
    assert_eq!(print.len(), 9);
    assert_eq!(&print[4..5], "-");
    assert_eq!(print, fingerprint(&ada.public_key()));
    // The secret never shows in a debug print.
    let secret_hex: String = ada.to_bytes().iter().map(|b| format!("{b:02x}")).collect();
    assert!(!format!("{ada:?}").contains(&secret_hex[..8]));
}

#[test]
fn identity_file_is_made_once_and_kept() {
    let dir = std::env::temp_dir().join(format!("mc-identity-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("nested").join(IDENTITY_FILE);
    let made = Identity::load_or_create(&path).unwrap();
    let read = Identity::load_or_create(&path).unwrap();
    assert_eq!(made.public_key(), read.public_key());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // Only the key file is left behind.
    let files: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
    assert_eq!(files.len(), 1);
    // A damaged file is an error, and stays as it was.
    std::fs::write(&path, b"not a key").unwrap();
    assert!(Identity::load_or_create(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"not a key");
    std::fs::remove_dir_all(&dir).unwrap();
}
