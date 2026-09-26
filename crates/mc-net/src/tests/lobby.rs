//! The lobby: seats the host opens, moving seats, kicking, the countdown, chat to
//! allies, build checks, and a room that adopts connections a server routed to it.

use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use super::*;

fn seat_of(c: &NetClient) -> Option<PlayerId> {
    c.welcome.as_ref().and_then(|w| w.slot)
}

#[test]
fn the_host_opens_seats_moves_players_and_kicks() {
    let relay = relay(3, |c| c.auto_start = false);
    let addr = relay.local_addr();
    let mut host = connect_with(addr, "host", |_| {});
    pump_until(&mut [&mut host], "host in", |c| c[0].welcome.is_some());
    // Seat 1 is for an AI: people may take 0 and 2 only.
    host.session.set_open_seats(0b101);
    pump_until(&mut [&mut host], "seats set", |c| {
        c[0].lobby.as_ref().is_some_and(|l| l.open == 0b101)
    });
    let mut guest = connect_with(addr, "guest", |_| {});
    pump_until(&mut [&mut host, &mut guest], "guest seated", |cs| {
        seat_of(cs[1]).is_some()
    });
    assert_eq!(seat_of(&guest), Some(PlayerId(2)));

    // A closed seat cannot be taken; once opened it can, and the guest is told its new seat.
    guest.session.take_seat(PlayerId(1));
    host.session.set_open_seats(0b111);
    // Two connections: wait for the host's change before the guest's second try.
    pump_until(&mut [&mut host, &mut guest], "seat opened", |cs| {
        cs[1].lobby.as_ref().is_some_and(|l| l.open == 0b111)
    });
    guest.session.take_seat(PlayerId(1));
    pump_until(&mut [&mut host, &mut guest], "guest moved", |cs| {
        seat_of(cs[1]) == Some(PlayerId(1))
            && cs[0]
                .lobby
                .as_ref()
                .is_some_and(|l| l.players.iter().any(|p| p.slot == PlayerId(1)))
    });
    let lobby = host.lobby.clone().unwrap();
    assert_eq!(lobby.players.len(), 2);
    assert_eq!(lobby.host, Some(PlayerId(0)));

    // Only the host kicks, and the kicked player cannot walk back in.
    guest.session.kick(PlayerId(0));
    host.session.kick(PlayerId(1));
    pump_until(&mut [&mut host, &mut guest], "kicked", |cs| {
        cs[1].ended.is_some()
    });
    assert!(matches!(
        guest.ended,
        Some(EndReason::Refused {
            reason: RefuseReason::Kicked,
            ..
        })
    ));
    let mut again = connect_with(addr, "Guest", |_| {});
    pump_until(&mut [&mut again], "refused again", |c| c[0].ended.is_some());
    assert!(matches!(
        again.ended,
        Some(EndReason::Refused {
            reason: RefuseReason::Kicked,
            ..
        })
    ));
    pump_until(&mut [&mut host], "seat free", |c| {
        c[0].lobby.as_ref().is_some_and(|l| l.players.len() == 1)
    });
    relay.shutdown().unwrap();
}

#[test]
fn the_host_passes_on_when_it_leaves_the_lobby() {
    let relay = relay(3, |c| c.auto_start = false);
    let addr = relay.local_addr();
    let mut first = connect_with(addr, "first", |_| {});
    pump_until(&mut [&mut first], "in", |c| c[0].welcome.is_some());
    let mut second = connect_with(addr, "second", |_| {});
    pump_until(&mut [&mut first, &mut second], "both in", |cs| {
        cs[1].lobby.as_ref().is_some_and(|l| l.players.len() == 2)
    });
    drop(first);
    pump_until(&mut [&mut second], "new host", |c| {
        c[0].lobby
            .as_ref()
            .is_some_and(|l| l.players.len() == 1 && l.host == Some(PlayerId(1)))
    });
    relay.shutdown().unwrap();
}

#[test]
fn the_countdown_runs_and_any_change_stops_it() {
    let relay = relay(2, |c| {
        c.auto_start = false;
        c.countdown = Duration::from_millis(300);
    });
    let addr = relay.local_addr();
    let mut host = connect_with(addr, "host", |_| {});
    pump_until(&mut [&mut host], "in", |c| c[0].welcome.is_some());
    let mut guest = connect_with(addr, "guest", |_| {});
    guest.session.set_ready(true);
    pump_until(&mut [&mut host, &mut guest], "guest ready", |cs| {
        cs[0]
            .lobby
            .as_ref()
            .is_some_and(|l| l.players.len() == 2 && l.players[1].ready)
    });
    host.session.request_start();
    pump_until(&mut [&mut host, &mut guest], "counting down", |cs| {
        cs.iter()
            .all(|c| c.lobby.as_ref().is_some_and(|l| l.countdown_ms > 0))
    });
    guest.session.set_ready(false);
    pump_until(&mut [&mut host, &mut guest], "stopped", |cs| {
        cs.iter()
            .all(|c| c.lobby.as_ref().is_some_and(|l| l.countdown_ms == 0))
    });
    thread::sleep(Duration::from_millis(400));
    host.pump();
    assert!(
        host.started.is_none(),
        "a cancelled countdown started the match"
    );

    guest.session.set_ready(true);
    pump_until(&mut [&mut host, &mut guest], "ready again", |cs| {
        cs[0].lobby.as_ref().is_some_and(|l| l.players[1].ready)
    });
    let asked = Instant::now();
    host.session.request_start();
    pump_until(&mut [&mut host, &mut guest], "started", |cs| {
        cs.iter().all(|c| c.started.is_some())
    });
    assert!(asked.elapsed() >= Duration::from_millis(250));
    relay.shutdown().unwrap();
}

#[test]
fn chat_to_allies_reaches_only_them_with_the_senders_name() {
    let relay = relay(3, |_| {});
    let mut clients = start_match(relay.local_addr(), 3);
    // Player 0 to itself and player 1; player 2 is the enemy.
    clients[0].session.chat("flank left", 0b011).unwrap();
    clients[2].session.chat("gg", 0).unwrap();
    pump_until(&mut refs(&mut clients), "chat", |cs| {
        cs[0].chat.len() == 2 && cs[1].chat.len() == 2 && cs[2].chat.len() == 1
    });
    assert_eq!(clients[2].chat, [(Some(PlayerId(2)), "gg".to_owned())]);
    assert!(clients[1]
        .chat
        .contains(&(Some(PlayerId(0)), "flank left".to_owned())));
    relay.shutdown().unwrap();
}

#[test]
fn chat_floods_are_slowed() {
    let relay = relay(1, |_| {});
    let mut clients = start_match(relay.local_addr(), 1);
    for i in 0..12 {
        clients[0].session.chat(&format!("spam {i}"), 0).unwrap();
    }
    pump_until(&mut refs(&mut clients), "slowed", |cs| {
        cs[0].chat.iter().any(|(from, _)| from.is_none())
    });
    let passed = clients[0]
        .chat
        .iter()
        .filter(|(from, _)| from.is_some())
        .count();
    assert!(passed <= 6, "{passed} messages got through a burst of 12");
    relay.shutdown().unwrap();
}

#[test]
fn a_different_build_is_refused() {
    let relay = relay(2, |_| {});
    let addr = relay.local_addr();
    let mut host = connect_with(addr, "host", |c| c.build = "0.1.0+aaaa".into());
    pump_until(&mut [&mut host], "in", |c| c[0].welcome.is_some());
    let mut other = connect_with(addr, "other", |c| c.build = "0.1.0+bbbb".into());
    pump_until(&mut [&mut other], "refused", |c| c[0].ended.is_some());
    assert!(matches!(
        other.ended,
        Some(EndReason::Refused {
            reason: RefuseReason::BuildMismatch,
            ..
        })
    ));
    relay.shutdown().unwrap();
}

#[test]
fn a_standalone_relay_refuses_a_room_code() {
    let relay = relay(2, |_| {});
    let mut c = connect_with(relay.local_addr(), "lost", |c| c.room = 42);
    pump_until(&mut [&mut c], "refused", |c| c[0].ended.is_some());
    assert!(matches!(
        c.ended,
        Some(EndReason::Refused {
            reason: RefuseReason::RoomNotFound,
            ..
        })
    ));
    relay.shutdown().unwrap();
}

/// What a server does: read `Hello` at its own door, then hand the stream to the room.
#[test]
fn a_room_adopts_routed_connections_and_reports_its_status() {
    let room = Room::spawn(RelayConfig {
        players: 2,
        tick_interval: Duration::from_millis(1),
        countdown: Duration::ZERO,
        adaptive_delay: false,
        title: "Friday".into(),
        ..RelayConfig::default()
    })
    .unwrap();
    let door = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = door.local_addr().unwrap();
    let route = |name: &str, verified: bool| {
        let c = connect_with(addr, name, |c| c.room = 7);
        let (mut stream, _) = door.accept().unwrap();
        let Ok(Message::Hello(hello)) = read_frame(&mut stream) else {
            panic!("no Hello at the door");
        };
        assert_eq!(hello.room, 7);
        assert!(room.adopt(stream, hello, verified));
        c
    };
    let mut host = route("host", true);
    pump_until(&mut [&mut host], "in", |c| c[0].welcome.is_some());
    host.session.set_listing("Twin Shoals", "1 v 1").unwrap();
    let mut guest = route("guest", false);
    pump_until(&mut [&mut host, &mut guest], "both in", |cs| {
        cs[0].lobby.as_ref().is_some_and(|l| l.players.len() == 2)
    });
    let lobby = host.lobby.clone().unwrap();
    assert_eq!(lobby.title, "Friday");
    assert!(lobby.players[0].verified && !lobby.players[1].verified);

    // The listing and the guest came in on different connections, in either order.
    let deadline = Instant::now() + DEADLINE;
    while room.status().map.is_empty() {
        assert!(Instant::now() < deadline, "the listing never showed");
        thread::sleep(Duration::from_millis(2));
    }
    let status = room.status();
    assert_eq!(status.players, ["host", "guest"]);
    assert_eq!(
        (status.map.as_str(), status.mode.as_str()),
        ("Twin Shoals", "1 v 1")
    );
    assert_eq!((status.seats, status.free), (2, 0));
    assert_eq!(status.phase, RoomPhase::Lobby);

    guest.session.set_ready(true);
    pump_until(&mut [&mut host, &mut guest], "guest ready", |cs| {
        cs[0].lobby.as_ref().is_some_and(|l| l.players[1].ready)
    });
    host.session.request_start();
    pump_until(&mut [&mut host, &mut guest], "playing", |cs| {
        cs.iter().all(|c| c.ticks() >= 20)
    });
    let deadline = Instant::now() + DEADLINE;
    while !matches!(room.status().phase, RoomPhase::Playing { .. }) {
        assert!(Instant::now() < deadline, "status never showed the match");
        thread::sleep(Duration::from_millis(5));
        host.pump();
        guest.pump();
    }
    assert_same_history(&host.sim, &guest.sim, 20);
    room.shutdown().unwrap();
}
