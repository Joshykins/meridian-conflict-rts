//! A host and a guest through a real server, on a map baked for the test: the
//! host opens a room with a plan, the guest finds it in the list and joins, both
//! see the same seats, the guest readies up, the host starts, and both come out
//! with the same match to launch. Once for skirmish, once for co-op survival.

use super::lobby::{Launch, Lobby, Place};
use super::open;
use super::server::{Answer, Server, Status};
use crate::ui::lineup::{Catalog, Lineup, Mode};
use crate::ui::maps::MapCard;
use crate::ui::survival::Theatre;
use mc_data::survival::{Domain, FrontLayout, SpawnZone, SurvivalLayout};
use mc_data::weather::{TimeOfDay, WeatherPreset};
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

/// A small map, baked for the test: 2 tiles give two start positions, 5 give four.
fn map_card(stem: &str, size_tiles: u32) -> MapCard {
    let path = temp_dir(stem).join(format!("{stem}.mcmap"));
    let params = mc_map::bake::BakeParams::square("Lobby Test", size_tiles, 7);
    mc_map::bake::bake(&params, &path).unwrap();
    let map = mc_map::MapFile::open(&path).unwrap();
    MapCard::new(stem.into(), Arc::new(map), &Default::default())
}

fn wait(what: &str, mut step: impl FnMut() -> bool) {
    let deadline = Instant::now() + DEADLINE;
    while !step() {
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The host opens a room with `plan`, the guest joins and readies, the host
/// starts: both launches, the host's first.
fn meet(catalog: &Catalog, plan: Lineup, room: &str) -> (Launch, Launch) {
    let server = mc_server::start(mc_server::ServerConfig {
        bind: "127.0.0.1:0".into(),
        data_dir: temp_dir(&format!("server-{room}")),
        ..Default::default()
    })
    .unwrap();
    let addr = server.local_addr().to_string();
    let card = plan.card(catalog).unwrap();
    let content = mc_net::ContentId {
        map_id: card.map.content_id(),
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
        .create_room(room, 8, false, content)
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
    let mode = plan.mode;
    let mut host = Lobby::new(
        open(addr.clone(), host_config.clone()),
        Place::Server {
            code,
            private: false,
        },
        addr.clone(),
        host_config,
        Some(plan),
        room.into(),
    );
    wait("the host's lobby", || {
        host.pump(catalog);
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
    assert_eq!(listing.title, room);
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
        host.pump(catalog);
        guest.pump(catalog);
        guest.slot == Some(1)
            && guest.lineup.as_ref().is_some_and(|l| l.mode == mode)
            && host.occupants().iter().flatten().count() == 2
    });
    assert!(!guest.is_host());
    assert!(!guest.planning());
    assert_eq!(
        guest.lineup.as_ref().unwrap().roster,
        host.lineup.as_ref().unwrap().roster,
        "the guest sees the host's seats"
    );
    let people = host.occupants();
    assert_eq!(
        people
            .iter()
            .flatten()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["Host", "Guest"]
    );

    // Ready, start, and both have the same match to launch.
    guest.set_ready(true);
    wait("the guest ready", || {
        host.pump(catalog);
        guest.pump(catalog);
        host.state
            .as_ref()
            .is_some_and(|s| s.players.iter().any(|p| p.slot.0 == 1 && p.ready))
    });
    host.start();
    let (mut a, mut b) = (None, None);
    wait("both launched", || {
        host.pump(catalog);
        guest.pump(catalog);
        if a.is_none() {
            a = host.launch(catalog, Vec::new());
        }
        if b.is_none() {
            b = guest.launch(catalog, Vec::new());
        }
        a.is_some() && b.is_some()
    });
    assert!(matches!(host_link.status, Status::Online { .. }));
    server.shutdown();
    (a.unwrap().unwrap(), b.unwrap().unwrap())
}

#[test]
fn a_host_and_a_guest_meet_in_a_lobby_and_start_the_same_match() {
    let catalog = Catalog::new(vec![map_card("lobby_test", 2)], Vec::new(), Vec::new());
    let mut plan = Lineup::new(&catalog, Mode::Skirmish, 0, 2, 0);
    // The host's sky is everyone's: the guest sees the match under it too.
    plan.sky.preset = Some(WeatherPreset::Cloudy);
    plan.sky.time = Some(TimeOfDay::Dawn);
    let sky = plan.sky;
    let (a, b) = meet(&catalog, plan, "Friday");
    assert_eq!((a.local, b.local), (0, 1));
    assert_eq!(a.options.config.seed, b.options.config.seed);
    assert_eq!((a.options.sky, b.options.sky), (sky, sky));
    assert_eq!(a.options.map_id, catalog.maps[0].map.content_id());
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
        assert!(launch.options.survival.is_none());
    }
}

#[test]
fn friends_defend_together_in_co_op_survival() {
    // Two landing zones and the facility's start, of the map's four.
    let card = map_card("coop_test", 5);
    let layout = SurvivalLayout {
        engine: (3000.0, 3000.0),
        engine_start: 2,
        spawns: (0..2)
            .map(|start| SpawnZone {
                name: format!("Zone {start}"),
                start,
                blurb: String::new(),
            })
            .collect(),
        fronts: vec![FrontLayout {
            name: "The Road".into(),
            domain: Domain::Land,
            path: vec![(2000.0, 2000.0)],
        }],
        ..Default::default()
    };
    let theatre = Theatre {
        stem: card.stem.clone(),
        map: card.map.clone(),
        layout,
        look: card.look.clone(),
    };
    let catalog = Catalog::new(Vec::new(), vec![theatre], vec![card]);
    let plan = Lineup::new(&catalog, Mode::Survival, 0, 2, 0);
    assert!(plan.problem(&catalog).is_none());
    let (a, b) = meet(&catalog, plan, "Hold the Line");
    assert_eq!((a.local, b.local), (0, 1));
    for launch in [&a, &b] {
        let survival = launch.options.survival.as_ref().expect("a survival match");
        let seats = &launch.options.config.players;
        assert_eq!(seats.len(), 3, "two defenders and the Progenitor");
        assert_eq!(survival.engine_player, 2);
        for (i, name) in ["Host", "Guest"].into_iter().enumerate() {
            assert_eq!(
                (seats[i].name.as_str(), seats[i].controller, seats[i].team),
                (name, Controller::Human, 0)
            );
        }
        assert_ne!(
            seats[0].start, seats[1].start,
            "each on a zone of their own"
        );
        assert_eq!(seats[2].team, 1);
        assert_eq!(seats[2].start, 2);
        assert_eq!(launch.options.colors[2], crate::survival::ENGINE_COLOR);
    }
}
