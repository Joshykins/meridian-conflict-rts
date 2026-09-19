//! The relay server: lobby, turn closing, desync detection, join-in-progress, replay.
//!
//! One thread (the hub) owns all state and makes every decision, so bundle
//! contents have a single author. Around it: an accept thread, and per
//! connection a reader thread (socket → hub channel) and a writer thread
//! (outbox channel → socket), so a slow peer never stalls the hub.
//!
//! **Turns.** The relay is the match clock. Tick `N` is due one tick interval
//! after tick `N-1` closed. When it is due the relay waits until every
//! connected, caught-up player has sent `Commands` for `N`, but no longer than
//! `turn_timeout`; then it closes the tick with whatever it has. Whatever the
//! relay puts in the bundle *is* the tick, on every machine, so closing without
//! a straggler cannot desync anyone. The straggler is marked lagging and not
//! waited for again until one of its `Commands` arrives on time. Commands that
//! arrive for an already closed tick run in the next open tick instead; they
//! are never dropped. Ticks below the input delay are closed unconditionally:
//! clients send `Commands` for `R + input_delay` in response to bundle `R`, so
//! nobody can have sent anything for them.
//!
//! **Joining a running match.** The relay asks one healthy player for a
//! snapshot at the next tick to close, `S`. The request travels in front of
//! bundle `S` on the same stream, so the player is guaranteed not to have
//! stepped `S` yet. The joiner then receives the snapshot, every bundle after
//! `S` from the log, and from that point the live stream. If no player can
//! provide a snapshot the joiner gets the whole log from tick 0 instead.

use std::collections::hash_map::RandomState;
use std::collections::BTreeMap;
use std::fs::File;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, BufWriter, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mc_core::{PlayerId, MAX_PLAYERS, TICKS_PER_SECOND};

use crate::protocol::{
    command_cost, encode_frame, read_frame, snapshot_chunks, take_commands, ContentId, Hello, LobbyPlayer, LobbyState,
    MatchConfig, MatchStart, Message, PlayerSetup, RefuseReason, Role, SnapshotAssembler, TickBundle, Welcome,
    MAX_COMMANDS_BYTES, MAX_INPUT_DELAY, MAX_SNAPSHOT_LEN,
};
use crate::replay::{ReplayWriter, REPLAY_EXTENSION};
use crate::wire::NetError;

/// Commands a slot may have queued at the relay before it is dropped as abusive.
const MAX_SLOT_BACKLOG_BYTES: usize = 4 << 20;
/// Hash reports are compared once complete, or once this many newer ticks have piled up.
const HASH_WINDOW: usize = 512;
/// Bytes that may sit in a peer's outbox on top of one snapshot before the peer is dropped as too slow.
const OUTBOX_SLACK_BYTES: usize = 64 << 20;
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_POLL: Duration = Duration::from_millis(10);
/// How long `run` waits at exit for peers to read their last frames and hang up.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub struct RelayConfig {
    /// Player slots, `1..=MAX_PLAYERS`.
    pub players: u8,
    pub max_observers: u16,
    /// Ticks between a command being issued and executed, `1..=MAX_INPUT_DELAY`.
    /// Round trips up to `input_delay` tick intervals cost no stalls.
    pub input_delay: u32,
    /// Wall-clock length of a tick. Shorter only for tests and benchmarks.
    pub tick_interval: Duration,
    /// How long past its due time a tick waits for stragglers.
    pub turn_timeout: Duration,
    /// A connection must say `Hello` within this.
    pub handshake_timeout: Duration,
    /// A connection silent for this long is dropped. Clients ping every second.
    pub peer_timeout: Duration,
    /// How long a player has to deliver a requested snapshot before another is asked.
    pub snapshot_timeout: Duration,
    /// Start as soon as every slot is filled and ready, without waiting for the host.
    pub auto_start: bool,
    /// Fixed match seed; random when `None`.
    pub seed: Option<u64>,
    /// Required content; when `None` the first player to join defines it.
    pub content: Option<ContentId>,
    /// Where to write `match-<unix time>-<seed>.mcreplay`.
    pub replay_dir: Option<PathBuf>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        RelayConfig {
            players: MAX_PLAYERS as u8,
            max_observers: 16,
            input_delay: 2,
            tick_interval: Duration::from_millis(1000 / TICKS_PER_SECOND as u64),
            turn_timeout: Duration::from_millis(300),
            handshake_timeout: Duration::from_secs(5),
            peer_timeout: Duration::from_secs(30),
            snapshot_timeout: Duration::from_secs(20),
            auto_start: false,
            seed: None,
            content: None,
            replay_dir: None,
        }
    }
}

/// What a finished relay reports.
#[derive(Debug, Default)]
pub struct RelaySummary {
    /// Ticks closed. Zero if the match never started.
    pub ticks: u32,
    pub replay_path: Option<PathBuf>,
    /// Set when recording failed part-way; the file is valid up to that point.
    pub replay_error: Option<String>,
    /// First tick on which players disagreed.
    pub desync_tick: Option<u32>,
}

pub struct RelayServer {
    listener: TcpListener,
    config: RelayConfig,
}

impl RelayServer {
    pub fn bind(addr: impl ToSocketAddrs, config: RelayConfig) -> io::Result<RelayServer> {
        if config.players == 0 || config.players as usize > MAX_PLAYERS {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "players must be 1..=8"));
        }
        if config.input_delay == 0 || config.input_delay > MAX_INPUT_DELAY {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "input_delay must be 1..=50 ticks"));
        }
        if config.tick_interval.is_zero() {
            // Ticks nobody is waited for (below the input delay, all players lagging) would close in a busy loop.
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "tick_interval must be non-zero"));
        }
        Ok(RelayServer { listener: TcpListener::bind(addr)?, config })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Hosts one match on the calling thread. Returns when the last player has left a started match.
    pub fn run(self) -> io::Result<RelaySummary> {
        let (tx, rx) = mpsc::channel();
        self.run_with(tx, rx)
    }

    /// Hosts one match on background threads.
    pub fn spawn(self) -> io::Result<RelayHandle> {
        let addr = self.local_addr()?;
        let (tx, rx) = mpsc::channel();
        let hub_tx = tx.clone();
        let thread = thread::Builder::new().name("mc-relay-hub".into()).spawn(move || self.run_with(hub_tx, rx))?;
        Ok(RelayHandle { addr, tx, thread })
    }

    fn run_with(self, tx: Sender<Event>, rx: Receiver<Event>) -> io::Result<RelaySummary> {
        self.listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let accept = {
            let (listener, tx, stop) = (self.listener, tx.clone(), stop.clone());
            thread::Builder::new().name("mc-relay-accept".into()).spawn(move || accept_thread(listener, tx, stop))?
        };
        let mut hub = Hub::new(self.config, tx);
        hub.run(rx);
        stop.store(true, Ordering::SeqCst);
        let _ = accept.join();
        hub.drain_threads();
        Ok(hub.summary)
    }
}

pub struct RelayHandle {
    addr: SocketAddr,
    tx: Sender<Event>,
    thread: JoinHandle<io::Result<RelaySummary>>,
}

impl RelayHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }

    /// Ends the match now (completing the replay), disconnects everyone and waits for the threads.
    pub fn shutdown(self) -> io::Result<RelaySummary> {
        let _ = self.tx.send(Event::Shutdown);
        self.join()
    }

    /// Waits for the match to end by itself.
    pub fn join(self) -> io::Result<RelaySummary> {
        match self.thread.join() {
            Ok(result) => result,
            Err(_) => Err(io::Error::other("relay thread panicked")),
        }
    }
}

type ConnId = u64;

enum Event {
    Accepted(TcpStream),
    Message(ConnId, Message),
    Closed(ConnId, NetError),
    Shutdown,
}

fn accept_thread(listener: TcpListener, tx: Sender<Event>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if tx.send(Event::Accepted(stream)).is_err() {
                    return;
                }
            }
            // WouldBlock, or a connection that died in the backlog: neither is fatal.
            Err(_) => thread::sleep(ACCEPT_POLL),
        }
    }
}

fn reader_thread(id: ConnId, mut stream: TcpStream, tx: Sender<Event>, peer_timeout: Duration) {
    // The handshake timeout was set by the hub; it becomes the idle timeout after `Hello`.
    let mut first = true;
    loop {
        match read_frame(&mut stream) {
            Ok(msg) => {
                if first {
                    if !matches!(msg, Message::Hello(_)) {
                        let _ = tx.send(Event::Closed(id, NetError::Malformed("expected Hello")));
                        return;
                    }
                    first = false;
                    let _ = stream.set_read_timeout(Some(peer_timeout));
                }
                if tx.send(Event::Message(id, msg)).is_err() {
                    return;
                }
            }
            Err(e) => {
                let _ = tx.send(Event::Closed(id, e));
                return;
            }
        }
    }
}

enum Out {
    Frame(Arc<[u8]>),
    /// Flush what is queued, then half-close so the peer reads everything before seeing EOF.
    Close,
}

fn writer_thread(mut stream: TcpStream, rx: Receiver<Out>, queued: Arc<AtomicUsize>) {
    let mut graceful = false;
    for out in rx {
        match out {
            Out::Frame(frame) => {
                if stream.write_all(&frame).is_err() {
                    break;
                }
                queued.fetch_sub(frame.len(), Ordering::Relaxed);
            }
            Out::Close => {
                graceful = true;
                break;
            }
        }
    }
    // A failed write also wakes the reader, which reports the loss to the hub.
    let _ = stream.shutdown(if graceful { Shutdown::Write } else { Shutdown::Both });
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Connected, no `Hello` yet.
    Pending,
    Player(PlayerId),
    Observer,
}

struct Conn {
    stream: TcpStream,
    out: Sender<Out>,
    queued: Arc<AtomicUsize>,
    kind: Kind,
    /// Receives bundles as they close. False while waiting for a snapshot.
    live: bool,
}

#[derive(Default)]
struct Slot {
    occupied: bool,
    name: String,
    setup: Vec<u8>,
    token: u64,
    ready: bool,
    conn: Option<ConnId>,
    /// Commands by the tick they were stamped for. Entries at or below the closing tick are drained.
    queue: BTreeMap<u32, Vec<Vec<u8>>>,
    queue_bytes: usize,
    received_through: Option<u32>,
    lagging: bool,
    /// Has the sim state: was there at tick 0 or has been sent a snapshot.
    synced: bool,
    /// First tick this player can report a hash for.
    hash_from: u32,
}

struct SnapshotJob {
    tick: u32,
    provider: ConnId,
    deadline: Instant,
    assembler: SnapshotAssembler,
    joiners: Vec<ConnId>,
}

struct Match {
    start_frame: Arc<[u8]>,
    /// Encoded bundle frames, index == tick.
    log: Vec<Arc<[u8]>>,
    due: Instant,
    hashes: BTreeMap<u32, Vec<(PlayerId, u64)>>,
    snapshot: Option<SnapshotJob>,
    /// Joiners not yet attached to a snapshot job.
    waiting: Vec<ConnId>,
    replay: Option<ReplayWriter<BufWriter<File>>>,
}

struct Hub {
    config: RelayConfig,
    tx: Sender<Event>,
    conns: BTreeMap<ConnId, Conn>,
    next_conn: ConnId,
    slots: Vec<Slot>,
    content: Option<ContentId>,
    options: Vec<u8>,
    game: Option<Match>,
    done: bool,
    summary: RelaySummary,
    readers: Vec<JoinHandle<()>>,
    writers: Vec<JoinHandle<()>>,
}

fn frame(msg: &Message) -> Option<Arc<[u8]>> {
    // Relay-authored messages are built within the limits, so this is a bug, not bad input.
    // Even so the relay reports it and carries on rather than panic under a live match.
    match encode_frame(msg) {
        Ok(f) => Some(f.into()),
        Err(e) => {
            eprintln!("mc-relay: could not encode an outgoing message: {e}");
            None
        }
    }
}

fn random_u64() -> u64 {
    // `RandomState` is seeded from the OS; good enough for tokens and seeds without a dependency.
    let mut h = RandomState::new().build_hasher();
    h.write_u64(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64));
    h.finish()
}

impl Hub {
    fn new(config: RelayConfig, tx: Sender<Event>) -> Hub {
        let slots = (0..config.players).map(|_| Slot::default()).collect();
        Hub {
            content: config.content,
            config,
            tx,
            conns: BTreeMap::new(),
            next_conn: 0,
            slots,
            options: Vec::new(),
            game: None,
            done: false,
            summary: RelaySummary::default(),
            readers: Vec::new(),
            writers: Vec::new(),
        }
    }

    fn run(&mut self, rx: Receiver<Event>) {
        while !self.done {
            let wait = self.next_deadline().saturating_duration_since(Instant::now());
            match rx.recv_timeout(wait) {
                Ok(event) => {
                    self.handle(event);
                    // Take everything that is already here before looking at the clock.
                    while let Ok(event) = rx.try_recv() {
                        self.handle(event);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            self.on_clock(Instant::now());
        }
        self.end_match();
        let ids: Vec<ConnId> = self.conns.keys().copied().collect();
        for id in ids {
            self.close_gracefully(id);
        }
    }

    fn next_deadline(&self) -> Instant {
        let now = Instant::now();
        let mut deadline = now + Duration::from_millis(250);
        if let Some(m) = &self.game {
            let turn = if now >= m.due && !self.awaited(m.log.len() as u32).is_empty() {
                m.due + self.config.turn_timeout
            } else {
                m.due
            };
            deadline = deadline.min(turn);
            if let Some(job) = &m.snapshot {
                deadline = deadline.min(job.deadline);
            }
        }
        deadline
    }

    fn on_clock(&mut self, now: Instant) {
        if self.game.as_ref().and_then(|m| m.snapshot.as_ref()).is_some_and(|job| now >= job.deadline) {
            let failed = self.game.as_mut().and_then(|m| m.snapshot.take());
            if let (Some(job), Some(m)) = (failed, self.game.as_mut()) {
                m.waiting.extend(job.joiners);
                self.service_joiners(Some(job.provider));
            }
        }
        self.try_close_tick(now);
    }

    // ---- connections -------------------------------------------------------------------------

    fn handle(&mut self, event: Event) {
        match event {
            Event::Accepted(stream) => self.accept(stream),
            Event::Message(id, msg) => {
                if !self.conns.contains_key(&id) {
                    return; // Already closed; the reader is only draining.
                }
                if let Err(e) = self.on_message(id, msg) {
                    self.drop_conn(id, &e);
                }
            }
            Event::Closed(id, e) => {
                let pending = self.conns.get(&id).is_some_and(|c| c.kind == Kind::Pending);
                if let (true, NetError::Version { theirs }) = (pending, &e) {
                    let detail = format!("relay speaks protocol {}, client {}", crate::PROTOCOL_VERSION, theirs);
                    self.refuse(id, RefuseReason::VersionMismatch, &detail);
                } else {
                    self.drop_conn(id, &e);
                }
            }
            Event::Shutdown => self.done = true,
        }
    }

    fn accept(&mut self, stream: TcpStream) {
        self.readers.retain(|h| !h.is_finished());
        self.writers.retain(|h| !h.is_finished());
        let id = self.next_conn;
        self.next_conn += 1;
        let setup = || -> io::Result<(TcpStream, TcpStream)> {
            // Accepted sockets inherit non-blocking mode from the listener on some platforms.
            stream.set_nonblocking(false)?;
            stream.set_nodelay(true)?;
            stream.set_read_timeout(Some(self.config.handshake_timeout))?;
            stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
            Ok((stream.try_clone()?, stream.try_clone()?))
        };
        let Ok((read_half, write_half)) = setup() else { return };
        let (out, out_rx) = mpsc::channel();
        let queued = Arc::new(AtomicUsize::new(0));
        let (tx, peer_timeout, q) = (self.tx.clone(), self.config.peer_timeout, queued.clone());
        let reader = thread::Builder::new()
            .name("mc-relay-rx".into())
            .spawn(move || reader_thread(id, read_half, tx, peer_timeout));
        let writer = thread::Builder::new().name("mc-relay-tx".into()).spawn(move || writer_thread(write_half, out_rx, q));
        match (reader, writer) {
            (Ok(r), Ok(w)) => {
                self.readers.push(r);
                self.writers.push(w);
                self.conns.insert(id, Conn { stream, out, queued, kind: Kind::Pending, live: false });
            }
            // Out of threads: refuse the connection rather than take the relay down.
            _ => {
                let _ = stream.shutdown(Shutdown::Both);
            }
        }
    }

    fn send_frame(&mut self, id: ConnId, frame: &Arc<[u8]>) {
        let Some(conn) = self.conns.get(&id) else { return };
        let queued = conn.queued.fetch_add(frame.len(), Ordering::Relaxed) + frame.len();
        let sent = conn.out.send(Out::Frame(frame.clone())).is_ok();
        if !sent || queued > MAX_SNAPSHOT_LEN + OUTBOX_SLACK_BYTES {
            self.drop_conn(id, &NetError::Limit("peer is not reading fast enough"));
        }
    }

    fn send(&mut self, id: ConnId, msg: &Message) {
        if let Some(f) = frame(msg) {
            self.send_frame(id, &f);
        }
    }

    /// To everyone past the handshake.
    fn broadcast(&mut self, msg: &Message) {
        let Some(f) = frame(msg) else { return };
        let ids: Vec<ConnId> = self.conns.iter().filter(|(_, c)| c.kind != Kind::Pending).map(|(id, _)| *id).collect();
        for id in ids {
            self.send_frame(id, &f);
        }
    }

    fn refuse(&mut self, id: ConnId, reason: RefuseReason, detail: &str) {
        self.send(id, &Message::Refused { reason, detail: detail.to_owned() });
        self.close_gracefully(id);
    }

    /// For connections that did nothing wrong: they get to read what was sent before the close.
    /// Only valid for connections that hold no slot.
    fn close_gracefully(&mut self, id: ConnId) {
        if let Some(conn) = self.conns.remove(&id) {
            let _ = conn.out.send(Out::Close);
        }
    }

    /// Removes a connection and everything that hangs on it. `why` is only for the log.
    fn drop_conn(&mut self, id: ConnId, why: &NetError) {
        let Some(conn) = self.conns.remove(&id) else { return };
        let _ = conn.stream.shutdown(Shutdown::Both);
        if !matches!(why, NetError::Closed) && conn.kind != Kind::Pending {
            eprintln!("mc-relay: dropping {:?}: {why}", conn.kind);
        }
        drop(conn.out);

        if let Some(m) = &mut self.game {
            m.waiting.retain(|j| *j != id);
            let mut provider_lost = false;
            if let Some(job) = &mut m.snapshot {
                job.joiners.retain(|j| *j != id);
                provider_lost = job.provider == id;
            }
            if provider_lost {
                let job = m.snapshot.take().unwrap();
                m.waiting.extend(job.joiners);
            }
        }

        if let Kind::Player(slot) = conn.kind {
            // A reconnect may already have taken the slot over.
            if self.slots[slot.index()].conn == Some(id) {
                if self.game.is_some() {
                    self.slots[slot.index()].conn = None;
                    self.broadcast(&Message::PlayerDropped(slot));
                    if self.slots.iter().all(|s| s.conn.is_none()) {
                        self.done = true;
                        return;
                    }
                    self.evaluate_hashes();
                } else {
                    self.slots[slot.index()] = Slot::default();
                    if self.slots.iter().all(|s| !s.occupied) {
                        self.empty_lobby();
                    }
                }
            }
        }
        if self.game.is_some() {
            self.service_joiners(None);
        } else {
            self.broadcast_lobby();
        }
    }

    // ---- lobby -------------------------------------------------------------------------------

    /// The last player left before the start: the next one to arrive defines the match afresh.
    fn empty_lobby(&mut self) {
        self.content = self.config.content;
        self.options.clear();
        let observers: Vec<ConnId> =
            self.conns.iter().filter(|(_, c)| c.kind == Kind::Observer).map(|(id, _)| *id).collect();
        for id in observers {
            self.refuse(id, RefuseReason::NoHost, "every player left the lobby");
        }
    }

    fn host(&self) -> Option<PlayerId> {
        self.slots.iter().position(|s| s.occupied).map(|i| PlayerId(i as u8))
    }

    fn broadcast_lobby(&mut self) {
        let state = LobbyState {
            host: self.host(),
            players: self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.occupied)
                .map(|(i, s)| LobbyPlayer {
                    slot: PlayerId(i as u8),
                    name: s.name.clone(),
                    ready: s.ready,
                    setup: s.setup.clone(),
                })
                .collect(),
            observers: self.observer_count(),
            options: self.options.clone(),
        };
        self.broadcast(&Message::Lobby(state));
    }

    fn observer_count(&self) -> u16 {
        self.conns.values().filter(|c| c.kind == Kind::Observer).count() as u16
    }

    fn welcome(&self, slot: Option<PlayerId>, token: u64) -> Message {
        Message::Welcome(Welcome {
            slot,
            token,
            config: MatchConfig {
                max_players: self.config.players,
                input_delay: self.config.input_delay,
                tick_ms: self.config.tick_interval.as_millis() as u32,
            },
            in_progress: self.game.is_some(),
        })
    }

    fn on_hello(&mut self, id: ConnId, hello: Hello) {
        let content_ok = match self.content {
            Some(c) => c == hello.content,
            None => hello.role == Role::Player,
        };
        if !content_ok {
            return match self.content {
                Some(_) => self.refuse(id, RefuseReason::ContentMismatch, "map or blueprints differ from the match"),
                None => self.refuse(id, RefuseReason::NoHost, "no player has opened the lobby yet"),
            };
        }
        if hello.role == Role::Observer {
            if self.observer_count() >= self.config.max_observers {
                return self.refuse(id, RefuseReason::LobbyFull, "observer limit reached");
            }
            self.conns.get_mut(&id).unwrap().kind = Kind::Observer;
            let welcome = self.welcome(None, 0);
            self.send(id, &welcome);
            return self.enter(id, None);
        }

        let slot = if self.game.is_some() {
            let Some(token) = hello.token else {
                return self.refuse(id, RefuseReason::MatchInProgress, "the match has started");
            };
            let Some(i) = self.slots.iter().position(|s| s.occupied && s.token == token) else {
                return self.refuse(id, RefuseReason::BadToken, "no slot with that token");
            };
            // The old connection may be half-open and not yet timed out; the token holder wins.
            // Taking the slot first keeps `drop_conn` from seeing a match with nobody in it.
            let old = self.slots[i].conn.replace(id);
            if let Some(old) = old {
                self.drop_conn(old, &NetError::Limit("replaced by a reconnect"));
                self.broadcast(&Message::PlayerDropped(PlayerId(i as u8)));
            }
            let s = &mut self.slots[i];
            s.synced = false;
            s.lagging = true;
            s.received_through = None;
            PlayerId(i as u8)
        } else {
            let Some(i) = self.slots.iter().position(|s| !s.occupied) else {
                return self.refuse(id, RefuseReason::LobbyFull, "every player slot is taken");
            };
            self.content = Some(hello.content);
            self.slots[i] = Slot {
                occupied: true,
                name: hello.name,
                setup: hello.setup,
                token: random_u64(),
                conn: Some(id),
                ..Slot::default()
            };
            PlayerId(i as u8)
        };
        self.conns.get_mut(&id).unwrap().kind = Kind::Player(slot);
        let welcome = self.welcome(Some(slot), self.slots[slot.index()].token);
        self.send(id, &welcome);
        self.enter(id, Some(slot));
    }

    /// After `Welcome`: into the lobby, or into the running match.
    fn enter(&mut self, id: ConnId, slot: Option<PlayerId>) {
        let Some(m) = &mut self.game else {
            self.broadcast_lobby();
            return self.maybe_auto_start();
        };
        let start_frame = m.start_frame.clone();
        m.waiting.push(id);
        self.send_frame(id, &start_frame);
        let dropped: Vec<PlayerId> = (0..self.slots.len())
            .filter(|&i| self.slots[i].occupied && self.slots[i].conn.is_none())
            .map(|i| PlayerId(i as u8))
            .collect();
        for d in dropped {
            if Some(d) != slot {
                self.send(id, &Message::PlayerDropped(d));
            }
        }
        self.service_joiners(None);
    }

    fn maybe_auto_start(&mut self) {
        if self.config.auto_start && self.slots.iter().all(|s| s.occupied && s.ready) {
            self.start_match();
        }
    }

    fn start_match(&mut self) {
        let seed = self.config.seed.unwrap_or_else(random_u64);
        let start = MatchStart {
            content: self.content.unwrap_or_default(),
            seed,
            input_delay: self.config.input_delay,
            players: self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.occupied)
                .map(|(i, s)| PlayerSetup { slot: PlayerId(i as u8), name: s.name.clone(), data: s.setup.clone() })
                .collect(),
            options: self.options.clone(),
        };
        let Some(start_frame) = frame(&Message::Start(start.clone())) else { return };

        let mut replay = None;
        if let Some(dir) = &self.config.replay_dir {
            let unix = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let path = dir.join(format!("match-{unix}-{seed:016x}.{REPLAY_EXTENSION}"));
            match std::fs::create_dir_all(dir).and_then(|_| ReplayWriter::create(&path, &start)) {
                Ok(w) => {
                    replay = Some(w);
                    self.summary.replay_path = Some(path);
                }
                Err(e) => self.replay_failed(e),
            }
        }

        for s in self.slots.iter_mut().filter(|s| s.occupied) {
            s.synced = true;
        }
        for c in self.conns.values_mut().filter(|c| c.kind != Kind::Pending) {
            c.live = true;
        }
        self.game = Some(Match {
            start_frame: start_frame.clone(),
            log: Vec::new(),
            due: Instant::now() + self.config.tick_interval,
            hashes: BTreeMap::new(),
            snapshot: None,
            waiting: Vec::new(),
            replay,
        });
        let ids: Vec<ConnId> = self.conns.iter().filter(|(_, c)| c.live).map(|(id, _)| *id).collect();
        for id in ids {
            self.send_frame(id, &start_frame);
        }
    }

    fn replay_failed(&mut self, e: io::Error) {
        eprintln!("mc-relay: replay recording stopped: {e}");
        self.summary.replay_error = Some(e.to_string());
        if let Some(m) = &mut self.game {
            m.replay = None;
        }
    }

    fn end_match(&mut self) {
        let Some(mut m) = self.game.take() else { return };
        self.summary.ticks = m.log.len() as u32;
        if let Some(mut w) = m.replay.take() {
            if let Err(e) = w.finish() {
                self.replay_failed(e);
            }
        }
        self.broadcast(&Message::MatchEnd);
    }

    // ---- messages ----------------------------------------------------------------------------

    fn on_message(&mut self, id: ConnId, msg: Message) -> Result<(), NetError> {
        let kind = self.conns[&id].kind;
        let player = match kind {
            Kind::Player(slot) => Some(slot),
            _ => None,
        };
        if (kind == Kind::Pending) != matches!(msg, Message::Hello(_)) {
            return Err(NetError::Malformed("Hello must be the first message and sent once"));
        }
        let in_lobby = self.game.is_none();
        match msg {
            Message::Hello(hello) => self.on_hello(id, hello),
            Message::Ping(n) => self.send(id, &Message::Pong(n)),
            Message::Pong(_) => {}
            Message::Chat { text, .. } => self.broadcast(&Message::Chat { from: player, text }),
            Message::Leave => self.drop_conn(id, &NetError::Closed),

            // Lobby traffic can cross with `Start` on the wire; after the start it is ignored, not an offence.
            Message::Ready(ready) => {
                if let (true, Some(slot)) = (in_lobby, player) {
                    self.slots[slot.index()].ready = ready;
                    self.broadcast_lobby();
                    self.maybe_auto_start();
                }
            }
            Message::SetSetup(setup) => {
                if let (true, Some(slot)) = (in_lobby, player) {
                    self.slots[slot.index()].setup = setup;
                    self.broadcast_lobby();
                }
            }
            Message::SetOptions(options) => {
                if in_lobby && player.is_some() && player == self.host() {
                    self.options = options;
                    self.broadcast_lobby();
                }
            }
            Message::StartRequest => {
                let host = self.host();
                let others_ready = self.slots.iter().enumerate().all(|(i, s)| !s.occupied || s.ready || Some(PlayerId(i as u8)) == host);
                if in_lobby && player.is_some() && player == host && others_ready {
                    self.start_match();
                }
            }

            Message::Commands { tick, commands } => {
                let (Some(slot), Some(m)) = (player, &self.game) else {
                    return Err(NetError::Malformed("Commands outside a match or from an observer"));
                };
                let next_tick = m.log.len() as u32;
                let s = &mut self.slots[slot.index()];
                if s.received_through.is_some_and(|r| tick <= r) {
                    return Err(NetError::Malformed("Commands ticks must increase"));
                }
                // Commands answer bundle R with R + delay, and R is below the next tick to close.
                if tick >= next_tick.saturating_add(self.config.input_delay) {
                    return Err(NetError::Malformed("Commands for a tick too far ahead"));
                }
                s.received_through = Some(tick);
                if tick >= next_tick {
                    s.lagging = false;
                }
                if !commands.is_empty() {
                    s.queue_bytes += commands.iter().map(|c| command_cost(c)).sum::<usize>();
                    if s.queue_bytes > MAX_SLOT_BACKLOG_BYTES {
                        return Err(NetError::Limit("command backlog at the relay"));
                    }
                    s.queue.entry(tick).or_default().extend(commands);
                }
            }
            Message::Hash { tick, hash } => {
                let Some(m) = &mut self.game else {
                    return Err(NetError::Malformed("Hash outside a match"));
                };
                if tick as usize >= m.log.len() {
                    return Err(NetError::Malformed("Hash for a tick that has not closed"));
                }
                if let Some(slot) = player {
                    let reports = m.hashes.entry(tick).or_default();
                    let wanted = self.summary.desync_tick.is_none() && tick >= self.slots[slot.index()].hash_from;
                    if wanted && !reports.iter().any(|(s, _)| *s == slot) {
                        reports.push((slot, hash));
                    }
                    self.evaluate_hashes();
                }
            }
            Message::SnapshotChunk { tick, total_len, offset, data } => {
                let log_len = self.game.as_ref().map_or(0, |m| m.log.len());
                let job = self.game.as_mut().and_then(|m| m.snapshot.as_mut());
                let Some(job) = job.filter(|j| j.provider == id && j.tick == tick) else {
                    // A provider that was given up on may still be sending; that is not an offence.
                    return Ok(());
                };
                if tick as usize >= log_len {
                    return Err(NetError::Malformed("snapshot of a tick that has not closed"));
                }
                if let Some((tick, blob)) = job.assembler.push(tick, total_len, offset, &data)? {
                    self.deliver_snapshot(tick, &blob);
                }
            }

            Message::Welcome(_)
            | Message::Refused { .. }
            | Message::Lobby(_)
            | Message::Start(_)
            | Message::Bundle(_)
            | Message::Desync { .. }
            | Message::SnapshotRequest { .. }
            | Message::PlayerDropped(_)
            | Message::PlayerRejoined(_)
            | Message::MatchEnd => return Err(NetError::Malformed("relay-only message from a client")),
        }
        Ok(())
    }

    // ---- turns -------------------------------------------------------------------------------

    /// Slots tick `tick` has to wait for.
    fn awaited(&self, tick: u32) -> Vec<usize> {
        if tick < self.config.input_delay {
            return Vec::new();
        }
        (0..self.slots.len())
            .filter(|&i| {
                let s = &self.slots[i];
                s.conn.is_some() && s.synced && !s.lagging && s.received_through.is_none_or(|r| r < tick)
            })
            .collect()
    }

    /// Closes at most one tick; the next one is due an interval later at the earliest.
    fn try_close_tick(&mut self, now: Instant) {
        let Some(m) = &self.game else { return };
        if now < m.due {
            return;
        }
        let tick = m.log.len() as u32;
        let stragglers = self.awaited(tick);
        if !stragglers.is_empty() && now < m.due + self.config.turn_timeout {
            return;
        }
        for i in stragglers {
            self.slots[i].lagging = true;
        }

        let mut per_slot = Vec::new();
        for (i, s) in self.slots.iter_mut().enumerate() {
            let mut budget = MAX_COMMANDS_BYTES;
            let mut commands = Vec::new();
            // Late entries first, in the order they were stamped; what exceeds the budget stays queued.
            let keys: Vec<u32> = s.queue.range(..=tick).map(|(k, _)| *k).collect();
            for key in keys {
                let entry = s.queue.get_mut(&key).unwrap();
                commands.extend(take_commands(entry, &mut budget));
                if !entry.is_empty() {
                    break;
                }
                s.queue.remove(&key);
            }
            s.queue_bytes -= MAX_COMMANDS_BYTES - budget;
            per_slot.push((PlayerId(i as u8), commands));
        }
        let bundle = TickBundle::new(tick, per_slot);
        let Some(bundle_frame) = frame(&Message::Bundle(bundle.clone())) else {
            self.done = true;
            return;
        };

        let m = self.game.as_mut().unwrap();
        m.log.push(bundle_frame.clone());
        m.due = (m.due + self.config.tick_interval).max(now);
        let recorded = m.replay.as_mut().map(|w| w.bundle(&bundle));
        if let Some(Err(e)) = recorded {
            self.replay_failed(e);
        }
        let live: Vec<ConnId> = self.conns.iter().filter(|(_, c)| c.live).map(|(id, _)| *id).collect();
        for id in live {
            self.send_frame(id, &bundle_frame);
        }
    }

    // ---- desync detection --------------------------------------------------------------------

    fn evaluate_hashes(&mut self) {
        let Some(m) = &mut self.game else { return };
        if self.summary.desync_tick.is_some() {
            m.hashes.clear();
            return;
        }
        let overflow = m.hashes.len().saturating_sub(HASH_WINDOW);
        let ticks: Vec<u32> = m.hashes.keys().copied().collect();
        for (n, tick) in ticks.into_iter().enumerate() {
            let reports = &m.hashes[&tick];
            let complete = self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.conn.is_some() && s.synced && s.hash_from <= tick)
                .all(|(i, _)| reports.iter().any(|(slot, _)| slot.index() == i));
            if !complete && n >= overflow {
                continue;
            }
            let mut reports = m.hashes.remove(&tick).unwrap();
            if reports.is_empty() {
                continue;
            }
            if reports.iter().all(|(_, h)| *h == reports[0].1) {
                let recorded = m.replay.as_mut().map(|w| w.hash(tick, reports[0].1));
                if let Some(Err(e)) = recorded {
                    self.replay_failed(e);
                    return self.evaluate_hashes();
                }
            } else {
                reports.sort_by_key(|(slot, _)| *slot);
                self.summary.desync_tick = Some(tick);
                eprintln!("mc-relay: desync at tick {tick}: {reports:x?}");
                m.hashes.clear();
                self.broadcast(&Message::Desync { tick, hashes: reports });
                return;
            }
        }
    }

    // ---- join in progress --------------------------------------------------------------------

    /// Attaches waiting joiners to a snapshot job, starting one if needed.
    fn service_joiners(&mut self, avoid: Option<ConnId>) {
        let Some(m) = &mut self.game else { return };
        if m.waiting.is_empty() {
            return;
        }
        if let Some(job) = &mut m.snapshot {
            // Any snapshot still in flight serves later joiners just as well.
            job.joiners.append(&mut m.waiting);
            return;
        }
        let tick = m.log.len() as u32;
        if tick == 0 {
            // Nothing has happened yet: the joiner starts from `MatchStart` like everyone else.
            let joiners = std::mem::take(&mut m.waiting);
            return self.go_live(&joiners, 0, 0);
        }
        let candidates: Vec<(bool, ConnId)> =
            self.slots.iter().filter(|s| s.synced).filter_map(|s| s.conn.map(|c| (s.lagging, c))).collect();
        let provider = candidates
            .iter()
            .filter(|(_, c)| Some(*c) != avoid)
            .min()
            .or(candidates.iter().min())
            .map(|(_, c)| *c);
        match provider {
            Some(provider) => {
                m.snapshot = Some(SnapshotJob {
                    tick,
                    provider,
                    deadline: Instant::now() + self.config.snapshot_timeout,
                    assembler: SnapshotAssembler::new(),
                    joiners: std::mem::take(&mut m.waiting),
                });
                self.send(provider, &Message::SnapshotRequest { tick });
            }
            None => {
                // Nobody holds the state. The log from tick 0 rebuilds it, slowly but exactly.
                let joiners = std::mem::take(&mut m.waiting);
                self.go_live(&joiners, 0, 0);
            }
        }
    }

    fn deliver_snapshot(&mut self, tick: u32, blob: &[u8]) {
        let Some(job) = self.game.as_mut().and_then(|m| m.snapshot.take()) else { return };
        let Ok(chunks) = snapshot_chunks(tick, blob) else { return };
        let frames: Vec<Arc<[u8]>> = chunks.iter().filter_map(frame).collect();
        for &id in &job.joiners {
            for f in &frames {
                self.send_frame(id, f);
            }
        }
        self.go_live(&job.joiners, tick + 1, tick + 1);
        self.service_joiners(None);
    }

    /// Sends the log from `from_tick` and switches the joiners to the live stream. The hub is
    /// single-threaded, so no tick can close between the two.
    fn go_live(&mut self, joiners: &[ConnId], from_tick: u32, hash_from: u32) {
        for &id in joiners {
            let Some(m) = &self.game else { return };
            let backlog: Vec<Arc<[u8]>> = m.log.get(from_tick as usize..).unwrap_or_default().to_vec();
            for f in &backlog {
                self.send_frame(id, f);
            }
            let Some(conn) = self.conns.get_mut(&id) else { continue };
            conn.live = true;
            if let Kind::Player(slot) = conn.kind {
                let s = &mut self.slots[slot.index()];
                s.synced = true;
                s.hash_from = hash_from;
                self.broadcast(&Message::PlayerRejoined(slot));
            }
        }
    }

    // ---- exit --------------------------------------------------------------------------------

    /// Writers flush before we return (the process may exit right after). Readers get a short
    /// grace period to see the peer hang up, so our close does not reset frames still in flight.
    fn drain_threads(&mut self) {
        self.conns.clear();
        for w in self.writers.drain(..) {
            let _ = w.join();
        }
        let deadline = Instant::now() + DRAIN_GRACE;
        while self.readers.iter().any(|r| !r.is_finished()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
    }
}
