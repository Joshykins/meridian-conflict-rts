//! Rooms: hosting, the game list, joining with a ticket, and rooms closing.

use mc_net::{ClientConfig, DirRefuseReason, RefuseReason, RoomPhase};

use super::*;

#[test]
fn a_public_room_is_listed_and_joined_with_a_ticket() {
    let server = TestServer::start(|_| {});
    let mut ada = Dir::sign_in(&server, "Ada", &Identity::generate().unwrap());
    let mut bob = Dir::sign_in(&server, "Bob", &Identity::generate().unwrap());
    bob.client.subscribe(true);
    bob.listed("the empty list", |r| r.is_empty());

    let code = ada.create("Friday", 2, false);
    let listed = bob.listed("the room", |r| r.iter().any(|l| l.code == code));
    let room = &listed[0];
    assert_eq!((room.title.as_str(), room.seats), ("Friday", 2));
    assert_eq!((room.host.as_str(), room.phase), ("", RoomPhase::Lobby));
    assert_eq!(room.content, Some(CONTENT));
    assert!(!room.private);

    // The host comes in under the name the ticket proves, whatever the Hello said.
    let mut config = ada.client.join_config(code, Role::Player, CONTENT).unwrap();
    config.name = "Impostor".into();
    let mut host = Match::connect(server.addr, config);
    host.until("the lobby", |m| m.lobby.is_some());
    let lobby = host.lobby.clone().unwrap();
    assert_eq!(lobby.players[0].name, "Ada");
    assert!(lobby.players[0].verified);
    let listed = bob.listed("the host", |r| r.iter().any(|l| l.host == "Ada"));
    assert_eq!(listed[0].players, ["Ada"]);

    // The host alone may sit until the host opens more seats.
    let mut early = bob.join(code, Role::Player);
    assert_eq!(early.refused(), RefuseReason::LobbyFull);
    host.session.set_open_seats(0b11);
    bob.listed("the open seat", |r| r.iter().any(|l| l.free == 1));
    let mut guest = bob.join(code, Role::Player);
    host.until("the guest", |m| {
        m.lobby.as_ref().is_some_and(|l| l.players.len() == 2)
    });
    let lobby = host.lobby.clone().unwrap();
    assert_eq!(lobby.players[1].name, "Bob");
    assert!(lobby.players[1].verified);
    guest.until("the guest's welcome", |m| m.welcome.is_some());
}

#[test]
fn nobody_but_the_creator_enters_before_the_host() {
    let server = TestServer::start(|_| {});
    let mut ada = Dir::sign_in(&server, "Ada", &Identity::generate().unwrap());
    let bob = Dir::sign_in(&server, "Bob", &Identity::generate().unwrap());
    let code = ada.create("", 4, false);
    assert_eq!(ada.find(code).unwrap().title, "Ada's game");
    let mut early = bob.join(code, Role::Observer);
    assert_eq!(early.refused(), RefuseReason::NoHost);
    ada.client.subscribe(true);
    let mut host = ada.join(code, Role::Player);
    host.until("the lobby", |m| m.lobby.is_some());
    ada.listed("the host in the list", |r| {
        r.iter().any(|l| l.code == code && l.host == "Ada")
    });
    let mut watcher = bob.join(code, Role::Observer);
    watcher.until("the observer's welcome", |m| m.welcome.is_some());
}

#[test]
fn a_private_room_is_found_by_its_code_only() {
    let server = TestServer::start(|_| {});
    let mut ada = Dir::sign_in(&server, "Ada", &Identity::generate().unwrap());
    let mut bob = Dir::sign_in(&server, "Bob", &Identity::generate().unwrap());
    bob.client.subscribe(true);
    let hidden = ada.create("Secret", 2, true);
    let open = ada.create("Open", 2, false);
    // The list that shows the later public room would show the private one too.
    let listed = bob.listed("the public room", |r| r.iter().any(|l| l.code == open));
    assert_eq!(listed.len(), 1);
    let found = bob.find(hidden).unwrap();
    assert!(found.private);
    assert_eq!(found.title, "Secret");
    let typed: RoomCode = hidden.to_string().to_lowercase().parse().unwrap();
    assert_eq!(bob.find(typed).map(|l| l.code), Some(hidden));
    let nowhere = (1..)
        .map(|n| RoomCode::from_u32(n).unwrap())
        .find(|c| *c != hidden && *c != open);
    assert_eq!(bob.find(nowhere.unwrap()), None);
}

#[test]
fn a_match_connection_needs_a_room_and_a_good_ticket() {
    let server = TestServer::start(|_| {});
    let mut ada = Dir::sign_in(&server, "Ada", &Identity::generate().unwrap());
    let code = ada.create("Friday", 2, false);
    let attempt = |room: u32, ticket: Option<[u8; 16]>| {
        let mut config = ClientConfig::new("Ada", Role::Player, CONTENT);
        config.build = BUILD.into();
        config.room = room;
        config.ticket = ticket;
        Match::connect(server.addr, config).refused()
    };
    assert_eq!(attempt(code.to_u32(), None), RefuseReason::BadTicket);
    assert_eq!(
        attempt(code.to_u32(), Some([7; 16])),
        RefuseReason::BadTicket
    );
    let ticket = ada.client.ticket();
    assert_eq!(attempt(0, ticket), RefuseReason::RoomNotFound);
    let other = if code.to_u32() == 1 { 2 } else { 1 };
    assert_eq!(attempt(other, ticket), RefuseReason::RoomNotFound);
    assert_eq!(attempt(u32::MAX, ticket), RefuseReason::RoomNotFound);
}

#[test]
fn rooms_are_capped_per_player_and_per_server() {
    let server = TestServer::start(|c| {
        c.rooms_per_player = 2;
        c.max_rooms = 3;
    });
    let mut ada = Dir::sign_in(&server, "Ada", &Identity::generate().unwrap());
    let mut bob = Dir::sign_in(&server, "Bob", &Identity::generate().unwrap());
    ada.create("One", 2, false);
    ada.create("Two", 2, true);
    let refusal = |d: &mut Dir| {
        let seen = d.events.len();
        d.client.create_room("More", 2, false, CONTENT).unwrap();
        d.until("the refusal", |d| {
            d.events[seen..].iter().find_map(|e| match e {
                DirectoryEvent::RoomRefused { reason, .. } => Some(*reason),
                _ => None,
            })
        })
    };
    assert_eq!(refusal(&mut ada), DirRefuseReason::TooManyRooms);
    bob.create("Three", 2, false);
    assert_eq!(refusal(&mut bob), DirRefuseReason::ServerFull);
}

#[test]
fn a_room_leaves_the_list_when_its_match_ends() {
    let server = TestServer::start(|_| {});
    let mut ada = Dir::sign_in(&server, "Ada", &Identity::generate().unwrap());
    ada.client.subscribe(true);
    let code = ada.create("Solo", 1, false);
    ada.listed("the room", |r| r.iter().any(|l| l.code == code));
    let mut host = ada.join(code, Role::Player);
    host.until("the lobby", |m| m.lobby.is_some());
    host.session.request_start();
    host.until("the match", |m| m.ticks >= 20);
    ada.listed("the match in the list", |r| {
        r.iter()
            .any(|l| l.code == code && matches!(l.phase, RoomPhase::Playing { .. }))
    });
    drop(host);
    ada.listed("the room gone", |r| r.is_empty());
    ada.until("the counts", |d| {
        d.events.iter().rev().find_map(|e| match e {
            DirectoryEvent::Stats { rooms: 0, .. } => Some(()),
            _ => None,
        })
    });
    assert_eq!(ada.find(code), None);
    // The server kept the replay.
    let replays = std::fs::read_dir(server.dir.join("replays"))
        .unwrap()
        .count();
    assert_eq!(replays, 1);
}

#[test]
fn a_room_nobody_hosts_is_closed() {
    let server = TestServer::start(|c| c.host_timeout = Duration::from_millis(100));
    let mut ada = Dir::sign_in(&server, "Ada", &Identity::generate().unwrap());
    let code = ada.create("Abandoned", 2, false);
    wait_until("the room to close", || ada.find(code).is_none());
    // Closed rooms no longer count against their creator.
    ada.create("Again", 2, false);
    ada.create("And again", 2, false);
}

#[test]
fn shutting_down_ends_the_games_and_keeps_their_replays() {
    let server = TestServer::start(|_| {});
    let mut ada = Dir::sign_in(&server, "Ada", &Identity::generate().unwrap());
    let code = ada.create("Solo", 1, false);
    let mut host = ada.join(code, Role::Player);
    host.until("the lobby", |m| m.lobby.is_some());
    host.session.request_start();
    host.until("the match", |m| m.ticks >= 5);
    let dir = server.stop();
    host.until("the end", |m| m.ended.is_some());
    assert_eq!(host.ended, Some(EndReason::Finished));
    let replays = std::fs::read_dir(dir.join("replays")).unwrap().count();
    assert_eq!(replays, 1);
    std::fs::remove_dir_all(&dir).unwrap();
}
