//! The match clock: the load barrier, pause, the adaptive input delay (through a
//! proxy that adds a real round trip), link stats and desync details.

use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::*;

/// Waits `for_` while pumping, then returns: time passing, not a condition, is the point.
fn pump_for(clients: &mut [&mut NetClient], for_: Duration) {
    let until = Instant::now() + for_;
    while Instant::now() < until {
        for c in clients.iter_mut() {
            c.pump();
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn the_clock_waits_until_everyone_has_loaded() {
    let relay = relay(2, |_| {});
    let addr = relay.local_addr();
    let mut quick = connect_with(addr, "quick", |_| {});
    pump_until(&mut [&mut quick], "joined", |c| c[0].welcome.is_some());
    let mut slow = connect_with(addr, "slow", |_| {});
    slow.auto_load = false;
    quick.session.set_ready(true);
    slow.session.set_ready(true);
    pump_until(&mut [&mut quick, &mut slow], "started", |cs| {
        cs.iter().all(|c| c.started.is_some())
    });
    pump_until(&mut [&mut quick, &mut slow], "quick has loaded", |cs| {
        cs[1].loading == Some(0b01)
    });
    // At 1 ms a tick, a clock that did not wait would have closed hundreds by now.
    pump_for(&mut [&mut quick, &mut slow], Duration::from_millis(150));
    assert_eq!(quick.ticks(), 0, "the clock ran before everyone loaded");

    slow.session.loaded();
    pump_until(&mut [&mut quick, &mut slow], "ticks flow", |cs| {
        cs.iter().all(|c| c.ticks() >= 30)
    });
    assert_eq!(quick.loading, Some(0b11));
    assert_same_history(&quick.sim, &slow.sim, 30);
    relay.shutdown().unwrap();
}

#[test]
fn a_player_who_never_loads_is_not_waited_for_past_the_timeout() {
    let relay = relay(2, |c| c.load_timeout = Duration::from_millis(100));
    let addr = relay.local_addr();
    let mut ready = connect_with(addr, "ready", |_| {});
    pump_until(&mut [&mut ready], "joined", |c| c[0].welcome.is_some());
    let mut stuck = connect_with(addr, "stuck", |_| {});
    stuck.auto_load = false;
    ready.session.set_ready(true);
    stuck.session.set_ready(true);
    pump_until(&mut [&mut ready, &mut stuck], "the match runs", |cs| {
        cs[0].ticks() >= 50
    });
    // The one that never loaded still receives every tick to catch up with.
    pump_until(&mut [&mut ready, &mut stuck], "stuck has the ticks", |cs| {
        cs[1].ticks() >= 50
    });
    relay.shutdown().unwrap();
}

#[test]
fn any_player_pauses_and_any_player_resumes() {
    let relay = relay(2, |_| {});
    let mut clients = start_match(relay.local_addr(), 2);
    pump_until(&mut refs(&mut clients), "running", |cs| {
        cs.iter().all(|c| c.ticks() >= 20)
    });
    assert!(clients[0].session.set_paused(true));
    pump_until(&mut refs(&mut clients), "paused", |cs| {
        cs.iter().all(|c| {
            c.clock
                .is_some_and(|(p, by, _)| p && by == Some(PlayerId(0)))
        })
    });
    // Let anything already in flight arrive, then nothing more may.
    pump_for(&mut refs(&mut clients), Duration::from_millis(30));
    let held = clients[0].ticks();
    pump_for(&mut refs(&mut clients), Duration::from_millis(150));
    assert_eq!(clients[0].ticks(), held, "ticks closed while paused");

    assert!(clients[1].session.set_paused(false));
    pump_until(&mut refs(&mut clients), "resumed by the other", |cs| {
        cs.iter().all(|c| {
            c.clock
                .is_some_and(|(p, by, _)| !p && by == Some(PlayerId(1)))
        })
    });
    pump_until(&mut refs(&mut clients), "running again", |cs| {
        cs.iter().all(|c| c.ticks() >= held + 30)
    });
    assert_same_history(&clients[0].sim, &clients[1].sim, held + 30);
    relay.shutdown().unwrap();
}

/// Forwards loopback TCP both ways, holding every chunk back `delay`: a slow link.
struct LaggyProxy {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
}

impl LaggyProxy {
    fn new(target: SocketAddr, delay: Duration) -> LaggyProxy {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            while !flag.load(Ordering::SeqCst) {
                let Ok((client, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                client.set_nonblocking(false).unwrap();
                let server = TcpStream::connect(target).unwrap();
                for (from, to) in [
                    (client.try_clone().unwrap(), server.try_clone().unwrap()),
                    (server, client),
                ] {
                    pipe(from, to, delay);
                }
            }
        });
        LaggyProxy { addr, stop }
    }
}

impl Drop for LaggyProxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// One direction of the proxy: read, stamp, and write each chunk once it is `delay` old.
fn pipe(mut from: TcpStream, mut to: TcpStream, delay: Duration) {
    let (tx, rx) = mpsc::channel::<(Instant, Vec<u8>)>();
    thread::spawn(move || {
        let mut buf = [0u8; 64 << 10];
        while let Ok(n) = from.read(&mut buf) {
            if n == 0
                || tx
                    .send((Instant::now() + delay, buf[..n].to_vec()))
                    .is_err()
            {
                break;
            }
        }
    });
    thread::spawn(move || {
        for (due, chunk) in rx {
            thread::sleep(due.saturating_duration_since(Instant::now()));
            if to.write_all(&chunk).is_err() {
                break;
            }
        }
        let _ = to.shutdown(std::net::Shutdown::Both);
    });
}

#[test]
fn the_input_delay_rises_to_cover_a_slow_link_and_nobody_desyncs() {
    let relay = relay(2, |c| {
        c.tick_interval = Duration::from_millis(10);
        c.ping_interval = Duration::from_millis(20);
        c.adaptive_delay = true;
        c.turn_timeout = Duration::from_secs(5);
    });
    let addr = relay.local_addr();
    let mut near = connect_with(addr, "near", |_| {});
    pump_until(&mut [&mut near], "joined", |c| c[0].welcome.is_some());
    // 25 ms each way: a 50 ms round trip is five 10 ms ticks, plus the margin.
    let proxy = LaggyProxy::new(addr, Duration::from_millis(25));
    let mut far = connect_with(proxy.addr, "far", |_| {});
    near.session.set_ready(true);
    far.session.set_ready(true);
    pump_until(&mut [&mut near, &mut far], "the delay rose", |cs| {
        cs.iter().all(|c| c.clock.is_some_and(|(_, _, d)| d >= 6))
    });
    let at = near.ticks();
    pump_until(&mut [&mut near, &mut far], "ticks after the change", |cs| {
        cs.iter().all(|c| c.ticks() >= at + 100)
    });
    // Every command either side sent ran, once, in the order it was sent.
    for c in [&near, &far] {
        let slot = c.session.local_player().unwrap();
        let ran = near.commands_of(slot);
        assert_eq!(ran, c.sent[..ran.len()], "{slot:?}'s commands out of order");
    }
    assert_same_history(&near.sim, &far.sim, at + 100);
    relay.shutdown().unwrap();
}

#[test]
fn stats_show_each_seats_link() {
    let relay = relay(2, |c| c.ping_interval = Duration::from_millis(20));
    let mut clients = start_match(relay.local_addr(), 2);
    pump_until(&mut refs(&mut clients), "stats", |cs| {
        cs[0].stats.len() == 2 && cs[0].stats.iter().all(|s| s.link == Link::Connected)
    });
    let gone = clients.pop().unwrap();
    drop(gone);
    pump_until(&mut refs(&mut clients), "the drop shows", |cs| {
        cs[0]
            .stats
            .iter()
            .any(|s| s.slot == PlayerId(1) && s.link == Link::Dropped)
    });
    relay.shutdown().unwrap();
}

#[test]
fn every_player_hears_every_players_desync_sections() {
    let relay = relay(3, |_| {});
    let mut clients = start_match(relay.local_addr(), 3);
    clients[2].corrupt_hash_at = Some(25);
    pump_until(&mut refs(&mut clients), "desync", |cs| {
        cs.iter().all(|c| c.desync.is_some())
    });
    for (i, c) in clients.iter_mut().enumerate() {
        let sections = vec![1, 2, 3 + (i == 2) as u64];
        c.session.report_desync(25, &sections);
        // A second report, or one for another tick, is not passed on.
        c.session.report_desync(25, &[9, 9]);
        c.session.report_desync(26, &[9, 9]);
    }
    pump_until(&mut refs(&mut clients), "all details", |cs| {
        cs.iter().all(|c| c.details.len() == 3)
    });
    let mut details = clients[0].details.clone();
    details.sort_by_key(|(slot, _)| *slot);
    assert_eq!(
        details,
        vec![
            (PlayerId(0), vec![1, 2, 3]),
            (PlayerId(1), vec![1, 2, 3]),
            (PlayerId(2), vec![1, 2, 4]),
        ]
    );
    relay.shutdown().unwrap();
}

/// The client never stamps a tick at or below one it already sent, whatever the delay
/// does: a hand-driven relay moves the delay 4 -> 1 -> 3 and reads the stamps.
#[test]
fn stamps_only_rise_when_the_delay_moves() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut c = connect_with(listener.local_addr().unwrap(), "solo", |_| {});
    let (mut relay, _) = listener.accept().unwrap();
    relay.set_read_timeout(Some(DEADLINE)).unwrap();
    assert!(matches!(read_frame(&mut relay), Ok(Message::Hello(_))));
    let config = MatchConfig {
        max_players: 1,
        input_delay: 4,
        tick_ms: 1,
    };
    let start = MatchStart {
        content: CONTENT,
        seed: 1,
        input_delay: 4,
        players: vec![PlayerSetup {
            slot: PlayerId(0),
            name: "solo".into(),
            data: vec![],
        }],
        options: vec![],
    };
    for m in [
        Message::Welcome(Welcome {
            slot: Some(PlayerId(0)),
            token: 1,
            config,
            in_progress: false,
        }),
        Message::Start(start),
    ] {
        write_frame(&mut relay, &m).unwrap();
    }
    pump_until(&mut [&mut c], "started", |c| c[0].started.is_some());

    let clock = |input_delay| Message::Clock {
        paused: false,
        by: None,
        input_delay,
    };
    let mut stamps = Vec::new();
    for tick in 0..30u32 {
        match tick {
            10 => write_frame(&mut relay, &clock(1)).unwrap(),
            20 => write_frame(&mut relay, &clock(3)).unwrap(),
            _ => {}
        }
        write_frame(&mut relay, &Message::Bundle(TickBundle::empty(tick))).unwrap();
        // Commands answer each bundle, unless the stamp would not rise.
        pump_until(&mut [&mut c], "bundle seen", |c| {
            c[0].ticks() > tick as usize
        });
    }
    // Delay 4 for bundles 0-9, then nothing until 1 catches up (bundle 13 -> 14), then
    // bundles 20+ jump to +3.
    let expected: Vec<u32> = (0..10)
        .map(|r| r + 4)
        .chain((13..20).map(|r| r + 1))
        .chain((20..30).map(|r| r + 3))
        .collect();
    // The client pings throughout; read until every stamp is in (or nothing more comes).
    relay
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    while stamps.len() < expected.len() {
        match read_frame(&mut relay) {
            Ok(Message::Commands { tick, .. }) => stamps.push(tick),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    assert!(stamps.windows(2).all(|w| w[0] < w[1]), "stamps {stamps:?}");
    assert_eq!(stamps, expected);
}
