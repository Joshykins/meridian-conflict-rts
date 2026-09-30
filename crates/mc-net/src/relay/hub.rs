//! The hub: the relay's one thread of decisions. Connections, the event loop
//! and message dispatch live here; the lobby, the clock and joins have files of
//! their own.

use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{self, BufWriter};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use mc_core::PlayerId;

use super::{
    reader_thread, writer_thread, ConnId, Event, Hold, Out, RelayConfig, RelaySummary, RoomPhase,
    RoomStatus,
};
use crate::protocol::{
    encode_frame, ContentId, Message, RefuseReason, SnapshotAssembler, MAX_SNAPSHOT_LEN,
};
use crate::replay::ReplayWriter;
use crate::wire::NetError;

/// Commands a slot may have queued at the relay before it is dropped as abusive.
pub(super) const MAX_SLOT_BACKLOG_BYTES: usize = 4 << 20;
/// Bytes that may sit in a peer's outbox on top of one snapshot before the peer is dropped as too slow.
const OUTBOX_SLACK_BYTES: usize = 64 << 20;
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// How long `run` waits at exit for peers to read their last frames and hang up.
const DRAIN_GRACE: Duration = Duration::from_secs(2);
/// Round trips kept per connection.
const RTT_SAMPLES: usize = 8;
/// Chat: a message a second on average, bursts of five.
const CHAT_BURST: u32 = 5;
const CHAT_REFILL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    /// Connected, no `Hello` yet.
    Pending,
    Player(PlayerId),
    Observer,
}

/// Recent round trips of one connection, milliseconds.
#[derive(Default)]
pub(super) struct Rtt {
    samples: VecDeque<u32>,
}

impl Rtt {
    pub(super) fn push(&mut self, ms: u32) {
        if self.samples.len() == RTT_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(ms);
    }

    /// The worst of the recent round trips: what the input delay has to cover.
    pub(super) fn recent_max(&self) -> Option<u32> {
        self.samples.iter().copied().max()
    }

    /// What to show: the median of the recent round trips.
    pub(super) fn typical(&self) -> Option<u32> {
        let mut s: Vec<u32> = self.samples.iter().copied().collect();
        s.sort_unstable();
        s.get(s.len() / 2).copied()
    }
}

pub(super) struct Conn {
    pub(super) stream: TcpStream,
    pub(super) out: Sender<Out>,
    pub(super) queued: Arc<AtomicUsize>,
    pub(super) kind: Kind,
    /// Receives bundles as they close. False while waiting for a snapshot.
    pub(super) live: bool,
    /// The server checked this connection's name against its device key.
    pub(super) verified: bool,
    pub(super) name: String,
    pub(super) rtt: Rtt,
    /// Chat allowance left, and when it last grew.
    chat_tokens: u32,
    chat_refilled: Instant,
    /// From the server that routed the connection here; dropped with the connection.
    _hold: Hold,
}

#[derive(Default)]
pub(super) struct Slot {
    pub(super) occupied: bool,
    pub(super) name: String,
    pub(super) setup: Vec<u8>,
    pub(super) token: u64,
    pub(super) ready: bool,
    pub(super) verified: bool,
    pub(super) conn: Option<ConnId>,
    /// Commands by the tick they were stamped for. Entries at or below the closing tick are drained.
    pub(super) queue: BTreeMap<u32, Vec<Vec<u8>>>,
    pub(super) queue_bytes: usize,
    pub(super) received_through: Option<u32>,
    pub(super) lagging: bool,
    /// Has the sim state: was there at tick 0 or has been sent a snapshot.
    pub(super) synced: bool,
    /// First tick this player can report a hash for.
    pub(super) hash_from: u32,
    /// Has built the match from `Start` (or joined after the clock started).
    pub(super) loaded: bool,
}

pub(super) struct SnapshotJob {
    pub(super) tick: u32,
    pub(super) provider: ConnId,
    pub(super) deadline: Instant,
    pub(super) assembler: SnapshotAssembler,
    pub(super) joiners: Vec<ConnId>,
}

pub(super) struct Match {
    pub(super) start_frame: Arc<[u8]>,
    /// Encoded bundle frames, index == tick.
    pub(super) log: Vec<Arc<[u8]>>,
    pub(super) due: Instant,
    pub(super) hashes: BTreeMap<u32, Vec<(PlayerId, u64)>>,
    pub(super) snapshot: Option<SnapshotJob>,
    /// Joiners not yet attached to a snapshot job.
    pub(super) waiting: Vec<ConnId>,
    pub(super) replay: Option<ReplayWriter<BufWriter<File>>>,
    /// The input delay in `MatchStart`: no player can have commands for ticks below it.
    pub(super) start_delay: u32,
    /// Set while the clock waits for players to load: when it stops waiting.
    pub(super) loading_until: Option<Instant>,
    /// Set while paused: by whom (`None`: an observer or the relay).
    pub(super) paused: Option<Option<PlayerId>>,
    /// Slots whose desync report has been passed on.
    pub(super) desync_reported: mc_core::PlayerMask,
}

pub(super) struct Hub {
    pub(super) config: RelayConfig,
    tx: Sender<Event>,
    pub(super) conns: BTreeMap<ConnId, Conn>,
    next_conn: ConnId,
    pub(super) slots: Vec<Slot>,
    pub(super) content: Option<ContentId>,
    /// The game build the first player brought; everyone else must match it.
    pub(super) build: Option<String>,
    pub(super) options: Vec<u8>,
    pub(super) game: Option<Match>,
    pub(super) done: bool,
    pub(super) summary: RelaySummary,
    readers: Vec<JoinHandle<()>>,
    writers: Vec<JoinHandle<()>>,
    /// The seat of the player who may start the match.
    pub(super) host: Option<usize>,
    /// Seats a person may take.
    pub(super) open: mc_core::PlayerMask,
    /// When the countdown to the start runs out.
    pub(super) countdown: Option<Instant>,
    /// The host's `Listing`: map and mode.
    pub(super) listing: (String, String),
    /// Names the host removed, lower case; they may not come back to this room.
    pub(super) banned: Vec<String>,
    status: Arc<Mutex<RoomStatus>>,
    /// Listens on its own socket: a `Hello` for any room but 0 is not for it.
    standalone: bool,
    epoch: Instant,
    pub(super) next_ping: Instant,
    /// The delay commands are stamped with now.
    pub(super) input_delay: u32,
    pub(super) delay_changed: Instant,
    /// Since when a lower delay would have done.
    pub(super) lower_since: Option<Instant>,
}

pub(super) fn frame(msg: &Message) -> Option<Arc<[u8]>> {
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

pub(super) fn random_u64() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};
    // `RandomState` is seeded from the OS; good enough for tokens and seeds without a dependency.
    let mut h = RandomState::new().build_hasher();
    h.write_u64(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64),
    );
    h.finish()
}

impl Hub {
    pub(super) fn new(
        config: RelayConfig,
        tx: Sender<Event>,
        status: Arc<Mutex<RoomStatus>>,
        standalone: bool,
    ) -> Hub {
        let slots = (0..config.players).map(|_| Slot::default()).collect();
        let now = Instant::now();
        let mut hub = Hub {
            content: config.content,
            open: config.open_seats & seat_mask(config.players),
            input_delay: config.input_delay,
            config,
            tx,
            conns: BTreeMap::new(),
            next_conn: 0,
            slots,
            build: None,
            options: Vec::new(),
            game: None,
            done: false,
            summary: RelaySummary::default(),
            readers: Vec::new(),
            writers: Vec::new(),
            host: None,
            countdown: None,
            listing: (String::new(), String::new()),
            banned: Vec::new(),
            status,
            standalone,
            epoch: now,
            next_ping: now,
            delay_changed: now,
            lower_since: None,
        };
        hub.publish_status();
        hub
    }

    pub(super) fn run(&mut self, rx: Receiver<Event>) {
        while !self.done {
            let wait = self
                .next_deadline()
                .saturating_duration_since(Instant::now());
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
    }

    /// Ends the match, closes every connection and waits for the threads.
    pub(super) fn finish(mut self) -> RelaySummary {
        self.end_match();
        let ids: Vec<ConnId> = self.conns.keys().copied().collect();
        for id in ids {
            self.close_gracefully(id);
        }
        self.set_phase(RoomPhase::Ended);
        self.drain_threads();
        self.summary
    }

    fn next_deadline(&self) -> Instant {
        let now = Instant::now();
        let mut deadline = (now + Duration::from_millis(250)).min(self.next_ping);
        if let Some(at) = self.countdown {
            deadline = deadline.min(at);
        }
        if let Some(m) = &self.game {
            if let Some(until) = m.loading_until {
                deadline = deadline.min(until);
            } else if m.paused.is_none() {
                let turn = if now >= m.due && !self.awaited(m.log.len() as u32).is_empty() {
                    m.due + self.config.turn_timeout
                } else {
                    m.due
                };
                deadline = deadline.min(turn);
            }
            if let Some(job) = &m.snapshot {
                deadline = deadline.min(job.deadline);
            }
        }
        deadline
    }

    fn on_clock(&mut self, now: Instant) {
        if self.countdown.is_some_and(|at| now >= at) {
            self.countdown = None;
            self.start_match();
        }
        if self
            .game
            .as_ref()
            .and_then(|m| m.loading_until)
            .is_some_and(|until| now >= until)
        {
            self.end_loading();
        }
        if self
            .game
            .as_ref()
            .and_then(|m| m.snapshot.as_ref())
            .is_some_and(|job| now >= job.deadline)
        {
            let failed = self.game.as_mut().and_then(|m| m.snapshot.take());
            if let (Some(job), Some(m)) = (failed, self.game.as_mut()) {
                m.waiting.extend(job.joiners);
                self.service_joiners(Some(job.provider));
            }
        }
        if now >= self.next_ping {
            self.next_ping = now + self.config.ping_interval;
            self.ping_all(now);
            self.adapt_delay(now);
            self.send_net_stats();
            self.publish_status();
        }
        self.try_close_tick(now);
    }

    /// Microseconds since the hub started, as the relay's pings carry them.
    pub(super) fn stamp(&self, now: Instant) -> u32 {
        now.duration_since(self.epoch).as_micros() as u32
    }

    // ---- connections -------------------------------------------------------------------------

    fn handle(&mut self, event: Event) {
        match event {
            Event::Accepted(stream) => {
                self.accept(stream, true, Box::new(()));
            }
            Event::Adopted(a) => {
                if let Some(id) = self.accept(a.stream, false, a.hold) {
                    if let Some(conn) = self.conns.get_mut(&id) {
                        conn.verified = a.verified;
                    }
                    self.on_hello(id, *a.hello);
                }
            }
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
                    let detail = format!(
                        "relay speaks protocol {}, client {}",
                        crate::PROTOCOL_VERSION,
                        theirs
                    );
                    self.refuse(id, RefuseReason::VersionMismatch, &detail);
                } else {
                    self.drop_conn(id, &e);
                }
            }
            Event::Shutdown => self.done = true,
        }
    }

    /// Takes a connection on. `hello_first`: it still has to introduce itself.
    fn accept(&mut self, stream: TcpStream, hello_first: bool, hold: Hold) -> Option<ConnId> {
        self.readers.retain(|h| !h.is_finished());
        self.writers.retain(|h| !h.is_finished());
        let id = self.next_conn;
        self.next_conn += 1;
        let timeout = if hello_first {
            self.config.handshake_timeout
        } else {
            self.config.peer_timeout
        };
        let setup = || -> io::Result<(TcpStream, TcpStream)> {
            // Accepted sockets inherit non-blocking mode from the listener on some platforms.
            stream.set_nonblocking(false)?;
            stream.set_nodelay(true)?;
            stream.set_read_timeout(Some(timeout))?;
            stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
            Ok((stream.try_clone()?, stream.try_clone()?))
        };
        let Ok((read_half, write_half)) = setup() else {
            return None;
        };
        let (out, out_rx) = mpsc::channel();
        let queued = Arc::new(AtomicUsize::new(0));
        let (tx, peer_timeout, q) = (self.tx.clone(), self.config.peer_timeout, queued.clone());
        let reader = thread::Builder::new()
            .name("mc-relay-rx".into())
            .spawn(move || reader_thread(id, read_half, tx, peer_timeout, hello_first));
        let writer = thread::Builder::new()
            .name("mc-relay-tx".into())
            .spawn(move || writer_thread(write_half, out_rx, q));
        match (reader, writer) {
            (Ok(r), Ok(w)) => {
                self.readers.push(r);
                self.writers.push(w);
                self.conns.insert(
                    id,
                    Conn {
                        stream,
                        out,
                        queued,
                        kind: Kind::Pending,
                        live: false,
                        verified: false,
                        name: String::new(),
                        rtt: Rtt::default(),
                        chat_tokens: CHAT_BURST,
                        chat_refilled: Instant::now(),
                        _hold: hold,
                    },
                );
                Some(id)
            }
            // Out of threads: refuse the connection rather than take the relay down.
            _ => {
                let _ = stream.shutdown(Shutdown::Both);
                None
            }
        }
    }

    pub(super) fn send_frame(&mut self, id: ConnId, frame: &Arc<[u8]>) {
        let Some(conn) = self.conns.get(&id) else {
            return;
        };
        let queued = conn.queued.fetch_add(frame.len(), Ordering::Relaxed) + frame.len();
        let sent = conn.out.send(Out::Frame(frame.clone())).is_ok();
        if !sent || queued > MAX_SNAPSHOT_LEN + OUTBOX_SLACK_BYTES {
            self.drop_conn(id, &NetError::Limit("peer is not reading fast enough"));
        }
    }

    pub(super) fn send(&mut self, id: ConnId, msg: &Message) {
        if let Some(f) = frame(msg) {
            self.send_frame(id, &f);
        }
    }

    /// To everyone past the handshake.
    pub(super) fn broadcast(&mut self, msg: &Message) {
        let Some(f) = frame(msg) else { return };
        let ids: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.kind != Kind::Pending)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.send_frame(id, &f);
        }
    }

    pub(super) fn refuse(&mut self, id: ConnId, reason: RefuseReason, detail: &str) {
        self.send(
            id,
            &Message::Refused {
                reason,
                detail: detail.to_owned(),
            },
        );
        self.close_gracefully(id);
    }

    /// For connections that did nothing wrong: they get to read what was sent before the close.
    /// Only valid for connections that hold no slot.
    pub(super) fn close_gracefully(&mut self, id: ConnId) {
        if let Some(conn) = self.conns.remove(&id) {
            let _ = conn.out.send(Out::Close);
        }
    }

    /// Removes a connection and everything that hangs on it. `why` is only for the log.
    pub(super) fn drop_conn(&mut self, id: ConnId, why: &NetError) {
        let Some(conn) = self.conns.remove(&id) else {
            return;
        };
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
            if let Some(job) = m.snapshot.take_if(|_| provider_lost) {
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
                    self.check_loaded();
                } else {
                    self.vacate(slot.index());
                }
            }
        }
        if self.game.is_some() {
            self.service_joiners(None);
        } else {
            self.cancel_countdown();
            self.broadcast_lobby();
        }
    }

    // ---- messages ----------------------------------------------------------------------------

    fn on_message(&mut self, id: ConnId, msg: Message) -> Result<(), NetError> {
        let kind = self.conns[&id].kind;
        let player = match kind {
            Kind::Player(slot) => Some(slot),
            _ => None,
        };
        if (kind == Kind::Pending) != matches!(msg, Message::Hello(_)) {
            return Err(NetError::Malformed(
                "Hello must be the first message and sent once",
            ));
        }
        let in_lobby = self.game.is_none();
        let is_host = player.is_some_and(|p| self.host == Some(p.index()));
        match msg {
            Message::Hello(hello) => self.on_hello(id, hello),
            Message::Ping(n) => self.send(id, &Message::Pong(n)),
            Message::Pong(stamp) => self.on_pong(id, stamp),
            Message::Chat { to, text, .. } => self.on_chat(id, player, to, text),
            Message::Leave => self.drop_conn(id, &NetError::Closed),

            // Lobby traffic can cross with `Start` on the wire; after the start it is ignored, not an offence.
            Message::Ready(ready) => {
                if let (true, Some(slot)) = (in_lobby, player) {
                    self.slots[slot.index()].ready = ready;
                    if !ready {
                        self.cancel_countdown();
                    }
                    self.broadcast_lobby();
                    self.maybe_auto_start();
                }
            }
            Message::SetSetup(setup) => {
                if let (true, Some(slot)) = (in_lobby, player) {
                    self.slots[slot.index()].setup = setup;
                    self.cancel_countdown();
                    self.broadcast_lobby();
                }
            }
            Message::SetOptions(options) => {
                if in_lobby && is_host {
                    self.options = options;
                    self.cancel_countdown();
                    self.broadcast_lobby();
                }
            }
            Message::SetContent(content) => {
                // Unit data is what the door checked; only the map may change.
                let same_units = self
                    .content
                    .is_some_and(|c| c.blueprint_hash == content.blueprint_hash);
                if in_lobby && is_host && same_units {
                    self.content = Some(content);
                    self.cancel_countdown();
                    self.broadcast_lobby();
                }
            }
            Message::Listing { map, mode } => {
                if is_host {
                    self.listing = (map, mode);
                    self.publish_status();
                }
            }
            Message::StartRequest => {
                if in_lobby && is_host {
                    self.request_start();
                }
            }
            Message::SetOpenSeats(mask) => {
                if in_lobby && is_host {
                    self.set_open(mask);
                }
            }
            Message::TakeSeat(seat) => {
                if let (true, Some(slot)) = (in_lobby, player) {
                    self.take_seat(id, slot, seat);
                }
            }
            Message::Kick(seat) => {
                if in_lobby && is_host {
                    self.kick(seat);
                }
            }

            Message::Commands { tick, commands } => {
                let Some(slot) = player else {
                    return Err(NetError::Malformed("Commands from an observer"));
                };
                self.on_commands(slot, tick, commands)?;
            }
            Message::Hash { tick, hash } => self.on_hash(player, tick, hash)?,
            Message::Loaded => {
                if let Some(slot) = player {
                    self.on_loaded(slot);
                }
            }
            Message::Pause(paused) => {
                if let Some(slot) = player {
                    self.set_paused(paused, Some(slot));
                }
            }
            Message::DesyncReport { tick, sections } => {
                if let Some(slot) = player {
                    self.on_desync_report(slot, tick, sections);
                }
            }
            Message::SnapshotChunk {
                tick,
                total_len,
                offset,
                data,
            } => self.on_snapshot_chunk(id, tick, total_len, offset, &data)?,

            Message::Welcome(_)
            | Message::Refused { .. }
            | Message::Lobby(_)
            | Message::Start(_)
            | Message::Bundle(_)
            | Message::Desync { .. }
            | Message::SnapshotRequest { .. }
            | Message::PlayerDropped(_)
            | Message::PlayerRejoined(_)
            | Message::MatchEnd
            | Message::Loading { .. }
            | Message::DesyncDetail { .. }
            | Message::Clock { .. }
            | Message::NetStats(_) => {
                return Err(NetError::Malformed("relay-only message from a client"))
            }
        }
        Ok(())
    }

    fn on_chat(
        &mut self,
        id: ConnId,
        from: Option<PlayerId>,
        to: mc_core::PlayerMask,
        text: String,
    ) {
        let now = Instant::now();
        let Some(conn) = self.conns.get_mut(&id) else {
            return;
        };
        let refills =
            (now.duration_since(conn.chat_refilled).as_millis() / CHAT_REFILL.as_millis()) as u32;
        if refills > 0 {
            conn.chat_tokens = (conn.chat_tokens + refills).min(CHAT_BURST);
            conn.chat_refilled = now;
        }
        if conn.chat_tokens == 0 {
            let notice = Message::Chat {
                from: None,
                name: String::new(),
                to: 0,
                text: "Slow down: chat allows about one message a second.".into(),
            };
            return self.send(id, &notice);
        }
        conn.chat_tokens -= 1;
        let msg = Message::Chat {
            from,
            name: conn.name.clone(),
            to,
            text,
        };
        if to == 0 || from.is_none() {
            return self.broadcast(&msg);
        }
        let Some(f) = frame(&msg) else { return };
        let ids: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(cid, c)| match c.kind {
                Kind::Player(s) => **cid == id || to & mc_core::player_bit(s.0) != 0,
                _ => false,
            })
            .map(|(cid, _)| *cid)
            .collect();
        for cid in ids {
            self.send_frame(cid, &f);
        }
    }

    fn on_snapshot_chunk(
        &mut self,
        id: ConnId,
        tick: u32,
        total_len: u32,
        offset: u32,
        data: &[u8],
    ) -> Result<(), NetError> {
        let log_len = self.game.as_ref().map_or(0, |m| m.log.len());
        let job = self.game.as_mut().and_then(|m| m.snapshot.as_mut());
        let Some(job) = job.filter(|j| j.provider == id && j.tick == tick) else {
            // A provider that was given up on may still be sending; that is not an offence.
            return Ok(());
        };
        if tick as usize >= log_len {
            return Err(NetError::Malformed(
                "snapshot of a tick that has not closed",
            ));
        }
        if let Some((tick, blob)) = job.assembler.push(tick, total_len, offset, data)? {
            self.deliver_snapshot(tick, &blob);
        }
        Ok(())
    }

    // ---- status ------------------------------------------------------------------------------

    pub(super) fn set_phase(&mut self, phase: RoomPhase) {
        if let Ok(mut s) = self.status.lock() {
            s.phase = phase;
        }
    }

    /// Refreshes what a game browser sees of this room.
    pub(super) fn publish_status(&mut self) {
        let phase = match &self.game {
            None => RoomPhase::Lobby,
            Some(m) if m.loading_until.is_some() => RoomPhase::Loading,
            Some(m) => RoomPhase::Playing {
                tick: m.log.len() as u32,
            },
        };
        let seated: Vec<&Slot> = self.slots.iter().filter(|s| s.occupied).collect();
        let free = (0..self.slots.len())
            .filter(|&i| self.open & mc_core::player_bit(i as u8) != 0 && !self.slots[i].occupied)
            .count() as u8;
        let status = RoomStatus {
            title: self.config.title.clone(),
            host: self
                .host
                .and_then(|h| self.slots.get(h))
                .map_or(String::new(), |s| s.name.clone()),
            players: seated.iter().map(|s| s.name.clone()).collect(),
            seats: self.slots.len() as u8,
            free,
            observers: self.observer_count(),
            phase,
            content: self.content,
            build: self.build.clone().unwrap_or_default(),
            map: self.listing.0.clone(),
            mode: self.listing.1.clone(),
        };
        if let Ok(mut s) = self.status.lock() {
            *s = status;
        }
    }

    pub(super) fn observer_count(&self) -> u16 {
        self.conns
            .values()
            .filter(|c| c.kind == Kind::Observer)
            .count() as u16
    }

    pub(super) fn standalone(&self) -> bool {
        self.standalone
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

/// One bit for each of the first `seats` slots.
pub(super) fn seat_mask(seats: u8) -> mc_core::PlayerMask {
    mc_core::PlayerMask::MAX
        .checked_shr(mc_core::PlayerMask::BITS - u32::from(seats))
        .unwrap_or(0)
}
