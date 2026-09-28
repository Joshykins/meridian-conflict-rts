//! Signing in: names and keys, refusals, caps, and what the front door does
//! with anything that is not a game.

use std::io::{Read, Write};
use std::net::TcpStream;

use mc_net::directory::{encode_dir_frame, read_dir_frame, DirHello};
use mc_net::{DirMessage, DirRefuseReason, DIRECTORY_VERSION};

use super::*;

#[test]
fn a_name_belongs_to_the_first_key_that_signs_in_with_it() {
    let server = TestServer::start(|_| {});
    let ada = Identity::generate().unwrap();
    let eve = Identity::generate().unwrap();

    let mut first = Dir::sign_in(&server, "Ada", &ada);
    let (name, motd) = first.until("sign-in", |d| {
        d.events.iter().find_map(|e| match e {
            DirectoryEvent::SignedIn { name, motd, .. } => Some((name.clone(), motd.clone())),
            _ => None,
        })
    });
    assert_eq!(
        (name.as_str(), motd.as_str()),
        ("Ada", "Welcome to the test server.")
    );
    assert!(first.client.ticket().is_some());

    // Another key, in any casing, is refused.
    for taken in ["Ada", "ada", "ADA"] {
        let mut other = Dir::connect(&server, taken, &eve);
        assert_eq!(other.refusal(), DirRefuseReason::NameTaken, "{taken}");
    }
    // The owner may change the casing, on a second device session at once.
    let second = Dir::sign_in(&server, "aDA", &ada);
    assert_eq!(second.client.name(), "aDA");

    // The claim outlives the server.
    drop((first, second));
    let dir = server.stop();
    let server = TestServer::start_in(dir, |_| {});
    let mut other = Dir::connect(&server, "ada", &eve);
    assert_eq!(other.refusal(), DirRefuseReason::NameTaken);
    Dir::sign_in(&server, "Ada", &ada);
    let names = std::fs::read_to_string(server.dir.join("names.json")).unwrap();
    assert!(names.contains("\"ada\"") && !names.contains(&hex(&ada.to_bytes())));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Speaks the directory protocol by hand, to say what a real client never would.
fn raw_hello(server: &TestServer, hello: &DirMessage) -> TcpStream {
    let mut stream = TcpStream::connect(server.addr).unwrap();
    stream.set_read_timeout(Some(DEADLINE)).unwrap();
    stream.write_all(&encode_dir_frame(hello).unwrap()).unwrap();
    stream
}

fn hello(name: &str, identity: &Identity) -> DirMessage {
    DirMessage::Hello(DirHello {
        name: name.into(),
        public_key: identity.public_key(),
        build: BUILD.into(),
    })
}

fn refused(stream: &mut TcpStream) -> DirRefuseReason {
    match read_dir_frame(stream) {
        Ok(DirMessage::Refused { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_bad_name_or_a_bad_proof_is_refused() {
    let server = TestServer::start(|_| {});
    let ada = Identity::generate().unwrap();
    // The client will not even send a bad name.
    let e = DirectoryClient::connect(server.addr, " Ada", &ada, BUILD).err();
    assert_eq!(e.map(|e| e.kind()), Some(std::io::ErrorKind::InvalidInput));
    // The server refuses it all the same.
    for bad in ["", " Ada", "Ada!", "a\u{7}b", "abcdefghijklmnopqrstuvwxyz"] {
        let mut s = raw_hello(&server, &hello(bad, &ada));
        assert_eq!(refused(&mut s), DirRefuseReason::BadName, "{bad:?}");
    }
    // A proof signed with another key.
    let mut s = raw_hello(&server, &hello("Ada", &ada));
    let Ok(DirMessage::Challenge { nonce }) = read_dir_frame(&mut s) else {
        panic!("no challenge");
    };
    let eve = Identity::generate().unwrap();
    let proof = DirMessage::Proof {
        signature: eve.sign(&nonce),
    };
    s.write_all(&encode_dir_frame(&proof).unwrap()).unwrap();
    assert_eq!(refused(&mut s), DirRefuseReason::BadSignature);
    // Nothing was claimed by it.
    Dir::sign_in(&server, "Ada", &eve);
}

#[test]
fn another_directory_version_is_told_so() {
    let server = TestServer::start(|_| {});
    let mut frame = encode_dir_frame(&hello("Ada", &Identity::generate().unwrap())).unwrap();
    // Tag, magic, then the version.
    frame[9..13].copy_from_slice(&(DIRECTORY_VERSION + 1).to_le_bytes());
    let mut s = TcpStream::connect(server.addr).unwrap();
    s.set_read_timeout(Some(DEADLINE)).unwrap();
    s.write_all(&frame).unwrap();
    assert_eq!(refused(&mut s), DirRefuseReason::VersionMismatch);
}

#[test]
fn garbage_at_the_front_door_is_closed_on() {
    let server = TestServer::start(|_| {});
    let junk: [&[u8]; 4] = [
        b"GET / HTTP/1.1\r\nHost: x\r\n\r\n",
        &[0xFF, 0xFF, 0xFF, 0xFF, 1, 2, 3],
        &[5, 0, 0, 0, 100, 1, 2, 3, 4],
        &[1, 0, 0, 0, 18],
    ];
    for bytes in junk {
        let mut s = TcpStream::connect(server.addr).unwrap();
        s.set_read_timeout(Some(DEADLINE)).unwrap();
        s.write_all(bytes).unwrap();
        let mut buf = [0u8; 64];
        // Closed without a word: end of stream, or a reset.
        assert!(matches!(s.read(&mut buf), Ok(0) | Err(_)), "{bytes:?}");
    }
    // The server carries on.
    Dir::sign_in(&server, "Ada", &Identity::generate().unwrap());
}

#[test]
fn one_address_may_hold_only_so_many_connections() {
    let server = TestServer::start(|c| c.max_per_ip = 2);
    let mut a = Dir::sign_in(&server, "A", &Identity::generate().unwrap());
    let _b = Dir::sign_in(&server, "B", &Identity::generate().unwrap());
    let mut c = Dir::connect(&server, "C", &Identity::generate().unwrap());
    assert_eq!(c.refusal(), DirRefuseReason::TooManyConnections);

    // A match connection over the cap is told too.
    let code = a.create("Busy", 2, false);
    let mut m = a.join(code, Role::Player);
    assert_eq!(m.refused(), mc_net::RefuseReason::ServerFull);

    // Once one leaves, there is room again.
    a.client.leave();
    let c = Identity::generate().unwrap();
    wait_until("a free place", || {
        let mut d = Dir::connect(&server, "C", &c);
        d.until("an answer", |d| {
            d.events.iter().find_map(|e| match e {
                DirectoryEvent::SignedIn { .. } => Some(true),
                DirectoryEvent::Refused { .. } | DirectoryEvent::Lost(_) => Some(false),
                _ => None,
            })
        })
    });
}
