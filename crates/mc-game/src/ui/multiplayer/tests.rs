//! A host and a guest through a real server, on a map baked for the test: the
//! host opens a room with a plan, the guest finds it in the list and joins, both
//! see the same seats, the guest readies up, the host starts, and both come out
//! with the same match to launch.

use super::lobby::{Lobby, Place, Plan};
use super::open;
use super::server::{Answer, Server, Status};
use crate::ui::maps::MapCard;
use mc_net::{Identity, Role};
use mc_sim::tables::Controller;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(40);

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mc-lobby-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A small two-player map, baked for the test.
fn map_card() -> MapCard {
    let path = temp_dir("map").join("lobby_test.mcmap");
    let params = mc_map::bake::BakeParams::square("Lobby Test", 2, 7);
    mc_map::bake::bake(&params, &path).unwrap();
    let map = mc_map::MapFile::open(&path).unwrap();
    MapCard::new("lobby_test".into(), Arc::new(map), &Default::default())
}

fn wait(what: &str, mut step: impl FnMut() -> bool) {
    let deadline = Instant::now() + DEADLINE;
    while !step() {
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_host_and_a_guest_meet_in_a_lobby_and_start_the_same_match() {
    let server = mc_server::start(mc_server::ServerConfig {
        bind: "127.0.0.1:0".into(),
        data_dir: temp_dir("server"),
        ..Default::default()
    })
    .unwrap();
    let addr = server.local_addr().to_string();
    let maps = vec![map_card()];
    let content = mc_net::ContentId {
        map_id: maps[0].map.content_id(),
        blueprint_hash: 42,
    };

    let mut host_link = Server::new(&addr, "Host", Some(Identity::from_bytes([1; 32])));
    let mut guest_link = Server::new(&addr, "Guest", Some(Identity::from_bytes([2; 32])));
    wait("both signed in", || {
        host_link.pump();
        guest_link.pump();
        host_link.online() && guest_link.online()
    });

    // The host asks for a room and opens its lobby with a plan.
    host_link
        .client()
        .unwrap()
        .create_room("Friday", 8, false, content)
        .unwrap();
    let mut code = None;
    wait("the room", || {
        host_link.pump();
        for a in std::mem::take(&mut host_link.answers) {
            if let Answer::Created(c) = a {
                code = Some(c);
            }
        }
        code.is_some()
    });
    let code = code.unwrap();
    let host_config = host_link
        .client()
        .unwrap()
        .join_config(code, Role::Player, content)
        .unwrap();
    let mut host = Lobby::new(
        open(addr.clone(), host_config.clone()),
        Place::Server {
            code,
            private: false,
        },
        addr.clone(),
        host_config,
        Some(Plan::new(0, 2, 2, 99)),
        "Friday".into(),
    );
    wait("the host's lobby", || {
        host.pump(&maps);
        host.is_host() && host.options.is_some()
    });

    // The guest finds it in the list and joins.
    wait("the room listed", || {
        guest_link.pump();
        guest_link
            .rooms
            .iter()
            .any(|r| r.code == code && r.map == "Lobby Test")
    });
    let listing = guest_link
        .rooms
        .iter()
        .find(|r| r.code == code)
        .unwrap()
        .clone();
    assert_eq!(listing.title, "Friday");
    let guest_config = guest_link
        .client()
        .unwrap()
        .join_config(code, Role::Player, listing.content.unwrap_or(content))
        .unwrap();
    let mut guest = Lobby::new(
        open(addr.clone(), guest_config.clone()),
        Place::Server {
            code,
            private: false,
        },
        addr.clone(),
        guest_config,
        None,
        listing.title.clone(),
    );
    wait("the guest seated and shown the plan", || {
        host.pump(&maps);
        guest.pump(&maps);
        guest.slot == Some(1) && guest.options.is_some() && guest.map_index(&maps) == Some(0)
    });
    assert!(!guest.is_host());
    assert!(guest.plan.is_none());

    // Ready, start, and both have the same match to launch.
    guest.set_ready(true);
    wait("the guest ready", || {
        host.pump(&maps);
        guest.pump(&maps);
        host.state
            .as_ref()
            .is_some_and(|s| s.players.iter().any(|p| p.slot.0 == 1 && p.ready))
    });
    host.start();
    let (mut a, mut b) = (None, None);
    wait("both launched", || {
        host.pump(&maps);
        guest.pump(&maps);
        if a.is_none() {
            a = host.launch(&maps, Vec::new());
        }
        if b.is_none() {
            b = guest.launch(&maps, Vec::new());
        }
        a.is_some() && b.is_some()
    });
    let (a, b) = (a.unwrap().unwrap(), b.unwrap().unwrap());
    assert_eq!((a.local, b.local), (0, 1));
    assert_eq!(a.options.config.seed, b.options.config.seed);
    assert_eq!(a.options.map_id, maps[0].map.content_id());
    for launch in [&a, &b] {
        let seats = &launch.options.config.players;
        assert_eq!(seats.len(), 2);
        assert_eq!(
            (seats[0].name.as_str(), seats[0].controller),
            ("Host", Controller::Human)
        );
        assert_eq!(
            (seats[1].name.as_str(), seats[1].controller),
            ("Guest", Controller::Human)
        );
        assert_ne!(seats[0].team, seats[1].team);
    }
    assert!(matches!(host_link.status, Status::Online { .. }));
    server.shutdown();
}
