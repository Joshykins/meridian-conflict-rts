//! End-to-end tests: real sockets on loopback, a relay on background threads,
//! and a stand-in sim whose state is a running hash of every bundle applied.
//!
//! Nothing here sleeps to synchronise. Every wait is a condition polled under
//! a generous deadline, and ticks run at 1 ms so a few hundred take well under
//! a second.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mc_core::{PlayerId, Rng, StateHasher};

use crate::protocol::{encode_frame, read_frame, write_frame};
use crate::*;

const DEADLINE: Duration = Duration::from_secs(30);

const CONTENT: ContentId = ContentId { map_id: 0xA57E_2000, blueprint_hash: 0xB1E0_0001 };

#[derive(Default)]
struct FakeSim {
    state: u64,
    /// State after each tick this machine stepped.
    history: BTreeMap<u32, u64>,
}

impl FakeSim {
    fn step(&mut self, bundle: &TickBundle) {
        let mut h = StateHasher::new();
        h.write_u64(self.state);
        h.write_u32(bundle.tick);
        for (slot, command) in bundle.commands() {
            h.write_u32(slot.0 as u32);
            h.write_u8s(command);
        }
        self.state = h.finish();
        self.history.insert(bundle.tick, self.state);
    }
}

/// A game loop in miniature, driving any `Session` exactly as the crate docs prescribe.
struct Client<S: Session> {
    session: S,
    sim: FakeSim,
    rng: Rng,
    started: Option<MatchStart>,
    welcome: Option<Welcome>,
    lobby: Option<LobbyState>,
    bundles: Vec<TickBundle>,
    sent: Vec<Vec<u8>>,
    /// Stop issuing commands once this many have gone out.
    send_limit: usize,
    snapshot_at: Option<u32>,
    loaded_at: Option<u32>,
    snapshots_given: u32,
    corrupt_hash_at: Option<u32>,
    desync: Option<(u32, Vec<(PlayerId, u64)>)>,
    diverged: bool,
    dropped: Vec<PlayerId>,
    rejoined: Vec<PlayerId>,
    chat: Vec<(Option<PlayerId>, String)>,
    ended: Option<EndReason>,
}

impl<S: Session> Client<S> {
    fn new(mut session: S, seed: u64) -> Self {
        session.set_tick_budget(10_000);
        Client {
            session,
            sim: FakeSim::default(),
            rng: Rng::new(seed),
            started: None,
            welcome: None,
            lobby: None,
            bundles: Vec::new(),
            sent: Vec::new(),
            send_limit: usize::MAX,
            snapshot_at: None,
            loaded_at: None,
            snapshots_given: 0,
            corrupt_hash_at: None,
            desync: None,
            diverged: false,
            dropped: Vec::new(),
            rejoined: Vec::new(),
            chat: Vec::new(),
            ended: None,
        }
    }

    fn ticks(&self) -> usize {
        self.bundles.len()
    }

    fn last_tick(&self) -> Option<u32> {
        self.bundles.last().map(|b| b.tick)
    }

    fn pump(&mut self) {
        if let (Some(_), Some(slot), None) = (&self.started, self.session.local_player(), &self.ended) {
            if self.sent.len() < self.send_limit {
                let mut commands = Vec::new();
                for _ in 0..self.rng.below(3) {
                    // Slot and sequence number make every blob unique and attributable.
                    let mut c = vec![slot.0];
                    c.extend_from_slice(&(self.sent.len() as u32).to_le_bytes());
                    c.extend((0..self.rng.below(40)).map(|_| self.rng.below(256) as u8));
                    self.sent.push(c.clone());
                    commands.push(c);
                }
                self.session.submit(commands).unwrap();
            }
        }
        for event in self.session.poll() {
            assert!(self.ended.is_none(), "event after Ended: {event:?}");
            match event {
                SessionEvent::Joined(w) => self.welcome = Some(w),
                SessionEvent::Lobby(l) => self.lobby = Some(l),
                SessionEvent::Started(start) => {
                    assert!(self.started.is_none() && self.bundles.is_empty());
                    self.started = Some(start);
                }
                SessionEvent::SnapshotLoaded { tick, blob } => {
                    assert!(self.started.is_some() && self.bundles.is_empty());
                    self.sim.state = u64::from_le_bytes(blob.as_slice().try_into().unwrap());
                    self.loaded_at = Some(tick);
                }
                SessionEvent::SnapshotWanted { tick } => {
                    assert!(self.last_tick().is_none_or(|t| t < tick), "asked for a tick already stepped");
                    self.snapshot_at = Some(tick);
                }
                SessionEvent::TickReady(bundle) => {
                    assert!(self.started.is_some());
                    let expected = self.last_tick().map(|t| t + 1).or(self.loaded_at.map(|t| t + 1)).unwrap_or(0);
                    assert_eq!(bundle.tick, expected, "ticks must arrive in order without gaps");
                    self.sim.step(&bundle);
                    let mut hash = self.sim.state;
                    if self.corrupt_hash_at == Some(bundle.tick) {
                        hash ^= 1;
                    }
                    self.session.report_hash(bundle.tick, hash);
                    if self.snapshot_at == Some(bundle.tick) {
                        self.session.provide_snapshot(bundle.tick, self.sim.state.to_le_bytes().to_vec()).unwrap();
                        self.snapshots_given += 1;
                    }
                    self.bundles.push(bundle);
                }
                SessionEvent::Desync { tick, hashes } => self.desync = Some((tick, hashes)),
                SessionEvent::ReplayDiverged { .. } => self.diverged = true,
                SessionEvent::PlayerDropped(p) => self.dropped.push(p),
                SessionEvent::PlayerRejoined(p) => self.rejoined.push(p),
                SessionEvent::Chat { from, text } => self.chat.push((from, text)),
                SessionEvent::ReplayWriteFailed(e) => panic!("replay write failed: {e}"),
                SessionEvent::Ended(reason) => self.ended = Some(reason),
            }
        }
    }

    /// Commands of `slot` in execution order, as this machine saw them.
    fn commands_of(&self, slot: PlayerId) -> Vec<Vec<u8>> {
        self.bundles.iter().flat_map(|b| b.commands()).filter(|(s, _)| *s == slot).map(|(_, c)| c.to_vec()).collect()
    }
}

type NetClient = Client<NetSession>;

fn pump_until(clients: &mut [&mut NetClient], what: &str, done: impl Fn(&[&mut NetClient]) -> bool) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        for c in clients.iter_mut() {
            c.pump();
        }
        if done(clients) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        // Back-off between polls, not synchronisation: the condition above is what ends the wait.
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn relay(players: u8, tweak: impl FnOnce(&mut RelayConfig)) -> RelayHandle {
    let mut config = RelayConfig {
        players,
        tick_interval: Duration::from_millis(1),
        // Long enough that a busy CI box never turns a healthy client into a straggler.
        turn_timeout: Duration::from_secs(20),
        auto_start: true,
        ..RelayConfig::default()
    };
    tweak(&mut config);
    RelayServer::bind("127.0.0.1:0", config).unwrap().spawn().unwrap()
}

fn connect_with(addr: SocketAddr, name: &str, tweak: impl FnOnce(&mut ClientConfig)) -> NetClient {
    let mut config = ClientConfig::new(name, Role::Player, CONTENT);
    config.ping_interval = Duration::from_millis(20);
    tweak(&mut config);
    let seed = name.bytes().fold(7u64, |a, b| a * 31 + b as u64);
    Client::new(NetSession::connect(addr, config).unwrap(), seed)
}

/// Connects `n` players one at a time (so slots follow the order), readies them, and waits for the start.
fn start_match(addr: SocketAddr, n: usize) -> Vec<NetClient> {
    let mut clients: Vec<NetClient> = Vec::new();
    for i in 0..n {
        let mut c = connect_with(addr, &format!("player{i}"), |_| {});
        pump_until(&mut [&mut c], "joined", |c| c[0].welcome.is_some());
        assert_eq!(c.welcome.as_ref().unwrap().slot, Some(PlayerId(i as u8)));
        assert!(!c.welcome.as_ref().unwrap().in_progress);
        c.session.set_ready(true);
        clients.push(c);
    }
    let mut refs: Vec<&mut NetClient> = clients.iter_mut().collect();
    pump_until(&mut refs, "match started", |cs| cs.iter().all(|c| c.started.is_some()));
    clients
}

fn refs(clients: &mut [NetClient]) -> Vec<&mut NetClient> {
    clients.iter_mut().collect()
}

fn assert_same_history(a: &FakeSim, b: &FakeSim, at_least: usize) {
    let common: Vec<u32> = a.history.keys().filter(|t| b.history.contains_key(t)).copied().collect();
    assert!(common.len() >= at_least, "only {} ticks in common", common.len());
    for t in common {
        assert_eq!(a.history[&t], b.history[&t], "state differs at tick {t}");
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mc-net-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn three_clients_receive_identical_bundles() {
    let relay = relay(3, |c| c.seed = Some(1234));
    let mut clients = start_match(relay.local_addr(), 3);
    pump_until(&mut refs(&mut clients), "200 ticks everywhere", |cs| cs.iter().all(|c| c.ticks() >= 200));

    let start = clients[0].started.clone().unwrap();
    assert_eq!(start.seed, 1234);
    assert_eq!(start.content, CONTENT);
    assert_eq!(start.players.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["player0", "player1", "player2"]);
    for c in &clients[1..] {
        assert_eq!(c.started.as_ref(), Some(&start));
        assert_eq!(c.bundles[..200], clients[0].bundles[..200]);
    }
    // Every slot got commands through; each arrives once, in the order issued, under its own slot,
    // and never before the input delay.
    for (i, c) in clients.iter().enumerate() {
        let executed = clients[0].commands_of(PlayerId(i as u8));
        assert!(executed.len() > 10, "slot {i} only got {} commands through", executed.len());
        assert_eq!(executed[..], c.sent[..executed.len()]);
    }
    assert!(clients[0].bundles[..start.input_delay as usize].iter().all(|b| b.is_empty()));

    pump_until(&mut refs(&mut clients), "latency measured", |cs| cs.iter().all(|c| c.session.latency().is_some()));
    assert!(clients.iter().all(|c| c.desync.is_none() && c.ended.is_none()));

    let summary = relay.shutdown().unwrap();
    assert!(summary.ticks >= 200 && summary.desync_tick.is_none());
    pump_until(&mut refs(&mut clients), "relay shutdown seen", |cs| cs.iter().all(|c| c.ended.is_some()));
    assert!(clients.iter().all(|c| c.ended == Some(EndReason::Finished)));
}

#[test]
fn hash_mismatch_reaches_everyone() {
    let relay = relay(3, |_| {});
    let mut clients = start_match(relay.local_addr(), 3);
    clients[1].corrupt_hash_at = Some(40);
    pump_until(&mut refs(&mut clients), "desync reported", |cs| cs.iter().all(|c| c.desync.is_some()));

    let good = clients[0].sim.history[&40];
    for c in &clients {
        let (tick, hashes) = c.desync.clone().unwrap();
        assert_eq!(tick, 40);
        assert_eq!(hashes, vec![(PlayerId(0), good), (PlayerId(1), good ^ 1), (PlayerId(2), good)]);
    }
    // The match itself goes on; what to do about a desync is the game's call.
    let seen = clients[0].ticks();
    pump_until(&mut refs(&mut clients), "ticks after the desync", |cs| cs[0].ticks() > seen + 20);
    assert_eq!(relay.shutdown().unwrap().desync_tick, Some(40));
}

#[test]
fn match_continues_when_a_player_drops() {
    let relay = relay(3, |_| {});
    let mut clients = start_match(relay.local_addr(), 3);
    pump_until(&mut refs(&mut clients), "50 ticks", |cs| cs.iter().all(|c| c.ticks() >= 50));

    drop(clients.pop());
    pump_until(&mut refs(&mut clients), "drop noticed", |cs| cs.iter().all(|c| c.dropped == [PlayerId(2)]));
    let seen = clients.iter().map(|c| c.ticks()).max().unwrap();
    pump_until(&mut refs(&mut clients), "200 more ticks", |cs| cs.iter().all(|c| c.ticks() >= seen + 200));

    let n = clients[0].ticks().min(clients[1].ticks());
    assert_eq!(clients[0].bundles[..n], clients[1].bundles[..n]);
    // The survivors' commands still flow after the drop.
    assert!(clients[0].bundles[seen..n].iter().any(|b| !b.is_empty()));
    assert!(clients.iter().all(|c| c.desync.is_none() && c.ended.is_none()));

    // The relay ends the match by itself once the last player leaves.
    drop(clients);
    let summary = relay.join().unwrap();
    assert!(summary.ticks as usize >= n);
}

#[test]
fn observer_joins_late_from_a_snapshot() {
    let relay = relay(2, |_| {});
    let mut clients = start_match(relay.local_addr(), 2);
    pump_until(&mut refs(&mut clients), "30 ticks", |cs| cs.iter().all(|c| c.ticks() >= 30));

    let observer = connect_with(relay.local_addr(), "watcher", |c| c.role = Role::Observer);
    clients.push(observer);
    pump_until(&mut refs(&mut clients), "observer caught up and 100 ticks on", |cs| {
        let target = cs[2].loaded_at.map(|t| t + 100);
        target.is_some() && cs.iter().all(|c| c.last_tick() >= target)
    });

    let observer = &clients[2];
    let welcome = observer.welcome.as_ref().unwrap();
    assert!(welcome.in_progress && welcome.slot.is_none());
    assert_eq!(observer.started, clients[0].started);
    let snapshot_tick = observer.loaded_at.unwrap();
    assert!(snapshot_tick >= 30);
    assert_eq!(observer.bundles[0].tick, snapshot_tick + 1);
    // Exactly one player was asked, and only once.
    assert_eq!(clients[0].snapshots_given + clients[1].snapshots_given, 1);
    assert_same_history(&observer.sim, &clients[0].sim, 100);
    assert_same_history(&observer.sim, &clients[1].sim, 100);
    assert!(clients.iter().all(|c| c.desync.is_none() && c.ended.is_none()));
    assert!(clients[0].rejoined.is_empty() && clients[0].dropped.is_empty());
    relay.shutdown().unwrap();
}

#[test]
fn player_reconnects_with_token_and_plays_on() {
    let relay = relay(2, |_| {});
    let addr = relay.local_addr();
    let mut clients = start_match(addr, 2);
    pump_until(&mut refs(&mut clients), "30 ticks", |cs| cs.iter().all(|c| c.ticks() >= 30));

    let gone = clients.pop().unwrap();
    let token = gone.session.token().unwrap();
    let sent_before = gone.sent.len();
    drop(gone);
    pump_until(&mut refs(&mut clients), "drop noticed", |cs| cs[0].dropped == [PlayerId(1)]);

    // A started match admits players only by token.
    for (wrong, reason) in [(None, RefuseReason::MatchInProgress), (Some(token ^ 1), RefuseReason::BadToken)] {
        let mut stranger = connect_with(addr, "stranger", |c| c.token = wrong);
        pump_until(&mut [&mut stranger], "refused", |c| c[0].ended.is_some());
        assert!(matches!(&stranger.ended, Some(EndReason::Refused { reason: r, .. }) if *r == reason));
    }

    let mut back = connect_with(addr, "player1", |c| c.token = Some(token));
    // Continue the old numbering so the blobs stay unique across both lives of the slot.
    back.sent = vec![Vec::new(); sent_before];
    clients.push(back);
    pump_until(&mut refs(&mut clients), "rejoined and commands executed", |cs| {
        let Some(loaded) = cs[1].loaded_at else { return false };
        let new_commands = cs[0].bundles.iter().filter(|b| b.tick > loaded).flat_map(|b| b.commands());
        cs[0].rejoined == [PlayerId(1)]
            && new_commands.filter(|(s, _)| *s == PlayerId(1)).count() >= 10
            && cs[1].last_tick() >= Some(loaded + 100)
            && cs[0].last_tick() >= Some(loaded + 100)
    });

    let back = &clients[1];
    assert_eq!(back.welcome.as_ref().unwrap().slot, Some(PlayerId(1)));
    assert_eq!(back.welcome.as_ref().unwrap().token, token);
    assert_same_history(&back.sim, &clients[0].sim, 100);
    // What the rejoined player issued is executed once, in order, under its old slot.
    let loaded = back.loaded_at.unwrap();
    let executed: Vec<Vec<u8>> = clients[0]
        .bundles
        .iter()
        .filter(|b| b.tick > loaded)
        .flat_map(|b| b.commands())
        .filter(|(s, c)| *s == PlayerId(1) && u32::from_le_bytes(c[1..5].try_into().unwrap()) as usize >= sent_before)
        .map(|(_, c)| c.to_vec())
        .collect();
    assert_eq!(executed[..], back.sent[sent_before..sent_before + executed.len()]);
    assert!(clients.iter().all(|c| c.desync.is_none() && c.ended.is_none()));
    relay.shutdown().unwrap();
}

#[test]
fn relay_and_local_replays_play_back_identically() {
    let dir = temp_dir("replays");

    // Recorded by the relay.
    let relay = relay(2, |c| c.replay_dir = Some(dir.clone()));
    let mut clients = start_match(relay.local_addr(), 2);
    pump_until(&mut refs(&mut clients), "150 ticks", |cs| cs.iter().all(|c| c.ticks() >= 150));
    let live = clients.remove(0);
    let (live_sim, live_bundles, live_start) = (live.sim, live.bundles, live.started.unwrap());
    drop(live.session);
    drop(clients);
    let summary = relay.join().unwrap();
    assert!(summary.replay_error.is_none());
    let path = summary.replay_path.unwrap();
    assert_eq!(path.extension().unwrap(), REPLAY_EXTENSION);

    let replay = Replay::load(&path).unwrap();
    assert!(replay.complete);
    assert_eq!(replay.start, live_start);
    assert_eq!(replay.bundles.len(), summary.ticks as usize);
    assert_eq!(replay.bundles[..live_bundles.len()], live_bundles[..]);
    assert!(replay.hashes.len() >= 100, "agreed hashes are recorded for verification");

    let mut playback = Client::new(ReplaySession::new(replay, Pacing::PerPoll(64)), 0);
    while playback.ended.is_none() {
        playback.pump();
    }
    assert!(!playback.diverged);
    assert_eq!(playback.started, Some(live_start));
    assert_same_history(&playback.sim, &live_sim, 150);

    // Recorded by a LocalSession.
    let path = dir.join(format!("local.{REPLAY_EXTENSION}"));
    let start = MatchStart {
        content: CONTENT,
        seed: 99,
        input_delay: 0,
        players: vec![PlayerSetup { slot: PlayerId(0), name: "solo".into(), data: vec![1] }],
        options: vec![2],
    };
    let mut session = LocalSession::new(start.clone(), PlayerId(0), Pacing::PerPoll(7)).unwrap();
    session.record_to(&path).unwrap();
    let mut local = Client::new(session, 5);
    while local.ticks() < 300 {
        local.pump();
    }
    local.session.finish().unwrap();
    local.send_limit = 0;
    local.pump();
    assert_eq!(local.ended, Some(EndReason::Finished));
    assert_eq!(local.commands_of(PlayerId(0)), local.sent);

    let mut playback = Client::new(ReplaySession::open(&path, Pacing::PerPoll(1000)).unwrap(), 0);
    while playback.ended.is_none() {
        playback.pump();
    }
    assert!(!playback.diverged);
    assert_eq!(playback.started, Some(start));
    assert_eq!(playback.bundles, local.bundles);
    assert_eq!(playback.sim.history, local.sim.history);

    // A sim that goes wrong during playback is caught by the recorded hashes.
    let mut broken = Client::new(ReplaySession::open(&path, Pacing::PerPoll(1000)).unwrap(), 0);
    broken.corrupt_hash_at = Some(123);
    while broken.ended.is_none() {
        broken.pump();
    }
    assert!(broken.diverged);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lobby_host_starts_the_match() {
    let relay = relay(4, |c| c.auto_start = false);
    let addr = relay.local_addr();
    let mut host = connect_with(addr, "host", |c| c.setup = vec![1]);
    pump_until(&mut [&mut host], "host joined", |c| c[0].welcome.is_some());
    let mut guest = connect_with(addr, "guest", |c| c.setup = vec![2]);
    let mut watcher = connect_with(addr, "watcher", |c| c.role = Role::Observer);
    pump_until(&mut [&mut host, &mut guest, &mut watcher], "everyone in the lobby", |cs| {
        cs.iter().all(|c| c.lobby.as_ref().is_some_and(|l| l.players.len() == 2 && l.observers == 1))
    });

    // Not everyone is ready: the request is ignored. Only the host may change options.
    host.session.request_start();
    guest.session.set_match_options(vec![6, 6, 6]).unwrap();
    host.session.set_match_options(vec![9, 9]).unwrap();
    guest.session.set_setup(vec![3, 3]).unwrap();
    guest.session.chat("glhf").unwrap();
    assert!(matches!(guest.session.submit(vec![vec![1]]), Err(NetError::Limit(_))));
    pump_until(&mut [&mut host, &mut guest, &mut watcher], "lobby updated", |cs| {
        cs.iter().all(|c| {
            let l = c.lobby.as_ref().unwrap();
            l.options == [9, 9] && l.players[1].setup == [3, 3] && c.chat == [(Some(PlayerId(1)), "glhf".to_owned())]
        })
    });
    let lobby = host.lobby.clone().unwrap();
    assert_eq!(lobby.host, Some(PlayerId(0)));
    assert_eq!((lobby.players[0].name.as_str(), lobby.players[0].ready), ("host", false));
    assert!(host.started.is_none());

    guest.session.set_ready(true);
    pump_until(&mut [&mut host, &mut guest, &mut watcher], "guest ready", |cs| {
        cs[0].lobby.as_ref().unwrap().players[1].ready
    });
    guest.session.request_start(); // not the host: ignored
    host.session.request_start();
    pump_until(&mut [&mut host, &mut guest, &mut watcher], "20 ticks for all", |cs| cs.iter().all(|c| c.ticks() >= 20));

    let start = host.started.clone().unwrap();
    assert_eq!(start.options, [9, 9]);
    assert_eq!(start.players.len(), 2);
    assert_eq!(start.players[1], PlayerSetup { slot: PlayerId(1), name: "guest".into(), data: vec![3, 3] });
    assert_eq!(watcher.started, Some(start));
    assert_eq!(watcher.bundles[..20], host.bundles[..20]);
    relay.shutdown().unwrap();
}

#[test]
fn handshake_refusals() {
    let relay = relay(1, |c| c.auto_start = false);
    let addr = relay.local_addr();
    let refused = |c: &mut NetClient| {
        pump_until(&mut [c], "refused", |c| c[0].ended.is_some());
        match c.ended.clone().unwrap() {
            EndReason::Refused { reason, .. } => reason,
            other => panic!("expected a refusal, got {other:?}"),
        }
    };

    let mut early = connect_with(addr, "early", |c| c.role = Role::Observer);
    assert_eq!(refused(&mut early), RefuseReason::NoHost);

    let mut host = connect_with(addr, "host", |_| {});
    pump_until(&mut [&mut host], "host joined", |c| c[0].welcome.is_some());

    let mut modded = connect_with(addr, "modded", |c| c.content.blueprint_hash ^= 1);
    assert_eq!(refused(&mut modded), RefuseReason::ContentMismatch);
    let mut modded = connect_with(addr, "modded", |c| {
        c.role = Role::Observer;
        c.content.map_id ^= 1;
    });
    assert_eq!(refused(&mut modded), RefuseReason::ContentMismatch);
    let mut extra = connect_with(addr, "extra", |_| {});
    assert_eq!(refused(&mut extra), RefuseReason::LobbyFull);

    // A client from the future: same magic, other version, unknown layout after it.
    let mut raw = TcpStream::connect(addr).unwrap();
    raw.set_read_timeout(Some(DEADLINE)).unwrap();
    let mut hello = encode_frame(&Message::Hello(Hello {
        name: "future".into(),
        role: Role::Player,
        token: None,
        content: CONTENT,
        setup: vec![],
    }))
    .unwrap();
    hello[9..13].copy_from_slice(&(PROTOCOL_VERSION + 1).to_le_bytes());
    raw.write_all(&hello).unwrap();
    assert!(matches!(read_frame(&mut raw), Ok(Message::Refused { reason: RefuseReason::VersionMismatch, .. })));
    assert!(matches!(read_frame(&mut raw), Err(NetError::Closed)));

    // Anything but Hello first, and garbage, just get the door.
    let mut raw = TcpStream::connect(addr).unwrap();
    raw.set_read_timeout(Some(DEADLINE)).unwrap();
    write_frame(&mut raw, &Message::Ready(true)).unwrap();
    assert!(read_frame(&mut raw).is_err());
    let mut raw = TcpStream::connect(addr).unwrap();
    raw.set_read_timeout(Some(DEADLINE)).unwrap();
    raw.write_all(&[0xFF; 64]).unwrap();
    assert!(read_frame(&mut raw).is_err());

    // None of that disturbed the lobby.
    pump_until(&mut [&mut host], "lobby intact", |c| c[0].lobby.as_ref().is_some_and(|l| l.players.len() == 1));
    assert!(host.ended.is_none());
    relay.shutdown().unwrap();
}

/// Reads frames until `want` returns something, skipping the rest.
fn read_until<T>(stream: &mut TcpStream, mut want: impl FnMut(Message) -> Option<T>) -> T {
    let deadline = Instant::now() + DEADLINE;
    loop {
        assert!(Instant::now() < deadline, "timed out reading from the relay");
        if let Some(found) = want(read_frame(stream).expect("relay closed the connection")) {
            return found;
        }
    }
}

#[test]
fn stragglers_are_restamped_and_bad_frames_disconnect() {
    let relay = relay(2, |c| c.turn_timeout = Duration::from_millis(50));
    let addr = relay.local_addr();
    let mut good = connect_with(addr, "good", |_| {});
    pump_until(&mut [&mut good], "joined", |c| c[0].welcome.is_some());
    good.session.set_ready(true);

    // A hand-driven second player that never answers a bundle.
    let mut raw = TcpStream::connect(addr).unwrap();
    raw.set_nodelay(true).unwrap();
    raw.set_read_timeout(Some(DEADLINE)).unwrap();
    let hello = Hello { name: "slow".into(), role: Role::Player, token: None, content: CONTENT, setup: vec![] };
    write_frame(&mut raw, &Message::Hello(hello)).unwrap();
    let welcome = read_until(&mut raw, |m| if let Message::Welcome(w) = m { Some(w) } else { None });
    assert_eq!(welcome.slot, Some(PlayerId(1)));
    write_frame(&mut raw, &Message::Ready(true)).unwrap();
    let delay = read_until(&mut raw, |m| if let Message::Start(s) = m { Some(s.input_delay) } else { None });

    // Tick `delay` is the first that waits for us. Seeing its bundle means the relay gave up waiting.
    let closed = read_until(&mut raw, |m| match m {
        Message::Bundle(b) if b.tick == delay => Some(b),
        _ => None,
    });
    assert!(closed.commands().all(|(slot, _)| slot == PlayerId(0)));
    // Too late for that tick: the command must run in a later one instead of vanishing.
    write_frame(&mut raw, &Message::Commands { tick: delay, commands: vec![b"late".to_vec()] }).unwrap();
    let ran_at = read_until(&mut raw, |m| match m {
        Message::Bundle(b) if b.commands().any(|(slot, c)| slot == PlayerId(1) && c == b"late") => Some(b.tick),
        _ => None,
    });
    assert!(ran_at > delay);

    // The other player saw the very same thing, and was never held up for long.
    pump_until(&mut [&mut good], "late command seen", |c| c[0].last_tick() >= Some(ran_at));
    let bundle = &good.bundles[ran_at as usize];
    assert_eq!(bundle.commands().filter(|(slot, _)| *slot == PlayerId(1)).count(), 1);

    // An oversized frame costs the sender its connection and nobody else anything.
    raw.write_all(&((MAX_FRAME_LEN + 1) as u32).to_le_bytes()).unwrap();
    raw.write_all(&[0; 32]).unwrap();
    let mut sink = [0u8; 4096];
    let deadline = Instant::now() + DEADLINE;
    loop {
        assert!(Instant::now() < deadline);
        match raw.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
    pump_until(&mut [&mut good], "drop noticed", |c| c[0].dropped == [PlayerId(1)]);
    let seen = good.ticks();
    pump_until(&mut [&mut good], "match goes on", |c| c[0].ticks() >= seen + 50);
    assert!(good.ended.is_none());
    relay.shutdown().unwrap();
}
