//! [`NetSession`]: the TCP client side of a relayed lockstep match.
//!
//! Two threads own the socket. The reader decodes frames, answers the ones
//! that need no game involvement, and forwards the rest as [`SessionEvent`]s
//! over a channel. The writer drains an outgoing channel. `poll` and `submit`
//! therefore never touch the socket and never block the game loop.
//!
//! Turns are cut on the reader thread: the moment bundle `R` arrives, whatever
//! the game has submitted so far goes out stamped `R + input_delay`. Doing it
//! there rather than in `poll` means a hitching game loop delays only its own
//! commands, not everybody's turn. The delay is the relay's (`Clock`); a stamp
//! is never at or below one already sent, so when the delay shrinks the
//! commands wait a tick or two rather than break their order.
//!
//! `poll` smooths playback: bundles that arrive bunched (network jitter) are
//! released at least three quarters of a tick apart, unless the queue shows the
//! game has fallen behind, when they go out as fast as the tick budget allows.

use std::io;
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use mc_core::{PlayerId, TICKS_PER_SECOND};

use crate::protocol::{
    check_commands, command_cost, encode_frame, read_frame, snapshot_chunks, take_commands,
    ContentId, Hello, Message, Role, SnapshotAssembler, MAX_BUILD_LEN, MAX_CHAT_LEN,
    MAX_COMMANDS_BYTES, MAX_NAME_LEN, MAX_OPTIONS_LEN, MAX_SECTIONS, MAX_SETUP_LEN, MAX_TITLE_LEN,
};
use crate::session::{EndReason, EventQueue, Session, SessionEvent};
use crate::wire::NetError;

/// Commands submitted but not yet sent may not exceed this. It only fills up
/// when the match has stalled, and then the player must hear about it.
pub const MAX_PENDING_BYTES: usize = 4 << 20;
/// Ticks queued beyond this mean the game is behind: stop spacing them out.
const PLAYOUT_SLACK: usize = 2;

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub name: String,
    pub role: Role,
    pub content: ContentId,
    /// The game build; the relay refuses a match's players that differ.
    pub build: String,
    /// The room on a server that hosts many; 0 for a relay that hosts one match.
    pub room: u32,
    /// From the server's sign-in, proving the name is this player's.
    pub ticket: Option<[u8; 16]>,
    /// Token from a previous `Joined` event, to reclaim that slot.
    pub token: Option<u64>,
    /// Opaque per-player setup shown in the lobby and copied into `MatchStart`.
    pub setup: Vec<u8>,
    pub connect_timeout: Duration,
    /// The relay answers a ping every `ping_interval`; silence for this long is a lost connection.
    pub peer_timeout: Duration,
    pub ping_interval: Duration,
}

impl ClientConfig {
    pub fn new(name: impl Into<String>, role: Role, content: ContentId) -> ClientConfig {
        ClientConfig {
            name: name.into(),
            role,
            content,
            build: String::new(),
            room: 0,
            ticket: None,
            token: None,
            setup: Vec::new(),
            connect_timeout: Duration::from_secs(5),
            peer_timeout: Duration::from_secs(30),
            ping_interval: Duration::from_secs(1),
        }
    }
}

enum Out {
    Frame(Vec<u8>),
    Close,
}

struct Pending {
    commands: Vec<Vec<u8>>,
    bytes: usize,
}

struct Shared {
    pending: Mutex<Pending>,
    /// Round trip in microseconds; `u64::MAX` until the first pong.
    rtt_us: AtomicU64,
    started: AtomicBool,
    /// Set when this side hangs up, so the reader does not report it as a loss.
    closing: AtomicBool,
}

pub struct NetSession {
    events: EventQueue,
    incoming: Receiver<SessionEvent>,
    out: Sender<Out>,
    shared: Arc<Shared>,
    role: Role,
    slot: Option<PlayerId>,
    token: Option<u64>,
    /// Length of a tick as the relay paces them.
    tick: Duration,
    /// The next bunched tick is not released before this.
    next_release: Instant,
}

impl NetSession {
    /// Connects and sends `Hello`. Blocks for the TCP connect only (bounded by
    /// `connect_timeout`); the relay's answer arrives through `poll` as
    /// `Joined` or `Ended(Refused)`.
    pub fn connect(addr: impl ToSocketAddrs, config: ClientConfig) -> io::Result<NetSession> {
        let mut last_err =
            io::Error::new(io::ErrorKind::InvalidInput, "address resolved to nothing");
        let mut stream = None;
        for a in addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&a, config.connect_timeout) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(e) => last_err = e,
            }
        }
        let Some(stream) = stream else {
            return Err(last_err);
        };
        Self::over(stream, config)
    }

    /// Like `connect`, over a stream already open (a server's front door, a test).
    pub fn over(stream: TcpStream, config: ClientConfig) -> io::Result<NetSession> {
        if config.name.len() > MAX_NAME_LEN
            || config.setup.len() > MAX_SETUP_LEN
            || config.build.len() > MAX_BUILD_LEN
        {
            return Err(NetError::Limit("player name, setup blob or build too long").into());
        }
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(config.peer_timeout))?;
        stream.set_write_timeout(Some(config.peer_timeout))?;

        let shared = Arc::new(Shared {
            pending: Mutex::new(Pending {
                commands: Vec::new(),
                bytes: 0,
            }),
            rtt_us: AtomicU64::new(u64::MAX),
            started: AtomicBool::new(false),
            closing: AtomicBool::new(false),
        });
        let (out_tx, out_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        let hello = Message::Hello(Hello {
            name: config.name,
            role: config.role,
            token: config.token,
            content: config.content,
            setup: config.setup,
            build: config.build,
            room: config.room,
            ticket: config.ticket,
        });
        let _ = out_tx.send(Out::Frame(encode_frame(&hello)?));

        let epoch = Instant::now();
        let writer_stream = stream.try_clone()?;
        let ping_interval = config.ping_interval;
        thread::Builder::new()
            .name("mc-net-client-tx".into())
            .spawn(move || writer_thread(writer_stream, out_rx, epoch, ping_interval))?;
        let reader = Reader {
            shared: shared.clone(),
            events: event_tx,
            out: out_tx.clone(),
            epoch,
            role: config.role,
            input_delay: 0,
            last_stamp: None,
            next_tick: 0,
            assembler: SnapshotAssembler::new(),
        };
        thread::Builder::new()
            .name("mc-net-client-rx".into())
            .spawn(move || reader.run(stream))?;

        Ok(NetSession {
            events: EventQueue::new(),
            incoming: event_rx,
            out: out_tx,
            shared,
            role: config.role,
            slot: None,
            token: None,
            tick: Duration::from_millis(1000 / TICKS_PER_SECOND as u64),
            next_release: Instant::now(),
        })
    }

    fn send(&self, msg: &Message) -> Result<(), NetError> {
        let frame = encode_frame(msg)?;
        // A dead writer means the connection is gone; `poll` reports that.
        let _ = self.out.send(Out::Frame(frame));
        Ok(())
    }

    pub fn set_ready(&mut self, ready: bool) {
        let _ = self.send(&Message::Ready(ready));
    }

    pub fn set_setup(&mut self, setup: Vec<u8>) -> Result<(), NetError> {
        if setup.len() > MAX_SETUP_LEN {
            return Err(NetError::Limit("setup blob over MAX_SETUP_LEN"));
        }
        self.send(&Message::SetSetup(setup))
    }

    /// Host only; the relay ignores it from anyone else.
    pub fn set_match_options(&mut self, options: Vec<u8>) -> Result<(), NetError> {
        if options.len() > MAX_OPTIONS_LEN {
            return Err(NetError::Limit("match options over MAX_OPTIONS_LEN"));
        }
        self.send(&Message::SetOptions(options))
    }

    /// Host only: the seats a person may take, one bit per slot.
    pub fn set_open_seats(&mut self, mask: u8) {
        let _ = self.send(&Message::SetOpenSeats(mask));
    }

    /// In the lobby: move to that free, open seat.
    pub fn take_seat(&mut self, seat: PlayerId) {
        let _ = self.send(&Message::TakeSeat(seat));
    }

    /// Host only: remove that player from the lobby, for the life of the room.
    pub fn kick(&mut self, seat: PlayerId) {
        let _ = self.send(&Message::Kick(seat));
    }

    /// Host only, in the lobby: the match is played on this content now (another map).
    pub fn set_content(&mut self, content: ContentId) {
        let _ = self.send(&Message::SetContent(content));
    }

    /// Host only: what a game browser shows for this room.
    pub fn set_listing(&mut self, map: &str, mode: &str) -> Result<(), NetError> {
        if map.len() > MAX_TITLE_LEN || mode.len() > MAX_TITLE_LEN {
            return Err(NetError::Limit("listing over MAX_TITLE_LEN"));
        }
        self.send(&Message::Listing {
            map: map.to_owned(),
            mode: mode.to_owned(),
        })
    }

    /// Host only. Starts the countdown once every other player is ready.
    pub fn request_start(&mut self) {
        let _ = self.send(&Message::StartRequest);
    }

    /// Round trip to the relay, once measured.
    pub fn latency(&self) -> Option<Duration> {
        match self.shared.rtt_us.load(Ordering::Relaxed) {
            u64::MAX => None,
            us => Some(Duration::from_micros(us)),
        }
    }

    /// Reconnect token, once `Joined` has been polled.
    pub fn token(&self) -> Option<u64> {
        self.token
    }

    /// Leaves the match and closes the connection. Dropping does the same.
    pub fn leave(&mut self) {
        if !self.shared.closing.swap(true, Ordering::SeqCst) {
            let _ = self.send(&Message::Leave);
            let _ = self.out.send(Out::Close);
        }
    }

    /// How many ticks this `poll` may release: spaced out while in step, all it may
    /// once behind.
    fn release_now(&mut self) -> u32 {
        let queued = self.events.queued_ticks();
        let now = Instant::now();
        if queued == 0 {
            return 0;
        }
        if queued > PLAYOUT_SLACK {
            return self.events.budget();
        }
        if now < self.next_release {
            return 0;
        }
        self.next_release = now + self.tick * 3 / 4;
        1
    }
}

impl Session for NetSession {
    fn submit(&mut self, commands: Vec<Vec<u8>>) -> Result<(), NetError> {
        if self.role == Role::Observer {
            return Ok(());
        }
        check_commands(&commands)?;
        if !self.shared.started.load(Ordering::SeqCst) {
            return Err(NetError::Limit("the match has not started"));
        }
        let bytes: usize = commands.iter().map(|c| command_cost(c)).sum();
        let mut pending = lock(&self.shared.pending);
        if pending.bytes + bytes > MAX_PENDING_BYTES {
            return Err(NetError::Limit("unsent commands over MAX_PENDING_BYTES"));
        }
        pending.bytes += bytes;
        pending.commands.extend(commands);
        Ok(())
    }

    fn poll(&mut self) -> Vec<SessionEvent> {
        while let Ok(event) = self.incoming.try_recv() {
            if let SessionEvent::Joined(w) = &event {
                self.slot = w.slot;
                self.token = Some(w.token);
                if w.config.tick_ms > 0 {
                    self.tick = Duration::from_millis(w.config.tick_ms as u64);
                }
            }
            self.events.push(event);
        }
        let ticks = self.release_now();
        self.events.drain_ticks(ticks)
    }

    fn report_hash(&mut self, tick: u32, hash: u64) {
        if self.role == Role::Player {
            let _ = self.send(&Message::Hash { tick, hash });
        }
    }

    fn provide_snapshot(&mut self, tick: u32, blob: Vec<u8>) -> Result<(), NetError> {
        for chunk in snapshot_chunks(tick, &blob)? {
            self.send(&chunk)?;
        }
        Ok(())
    }

    fn set_tick_budget(&mut self, max_ticks_per_poll: u32) {
        self.events.set_budget(max_ticks_per_poll);
    }

    fn local_player(&self) -> Option<PlayerId> {
        self.slot
    }

    /// Asks the relay; the `Clock` event says when it has happened, and who did it.
    fn set_paused(&mut self, paused: bool) -> bool {
        self.role == Role::Player && self.send(&Message::Pause(paused)).is_ok()
    }

    fn loaded(&mut self) {
        if self.role == Role::Player {
            let _ = self.send(&Message::Loaded);
        }
    }

    /// `to`: the slots it is for, one bit each (the relay adds the sender); 0 is everyone.
    fn chat(&mut self, text: &str, to: u8) -> Result<(), NetError> {
        if text.len() > MAX_CHAT_LEN {
            return Err(NetError::Limit("chat message over MAX_CHAT_LEN"));
        }
        self.send(&Message::Chat {
            from: None,
            name: String::new(),
            to,
            text: text.to_owned(),
        })
    }

    fn report_desync(&mut self, tick: u32, sections: &[u64]) {
        if self.role == Role::Player && sections.len() <= MAX_SECTIONS {
            let _ = self.send(&Message::DesyncReport {
                tick,
                sections: sections.to_vec(),
            });
        }
    }
}

impl Drop for NetSession {
    fn drop(&mut self) {
        self.leave();
    }
}

fn writer_thread(
    mut stream: TcpStream,
    rx: Receiver<Out>,
    epoch: Instant,
    ping_interval: Duration,
) {
    use std::io::Write;
    // `Hello` is already queued and must be the first frame out; the first ping follows shortly.
    let mut next_ping = Instant::now() + ping_interval.min(Duration::from_millis(100));
    loop {
        let now = Instant::now();
        if now >= next_ping {
            next_ping = now + ping_interval;
            let stamp = epoch.elapsed().as_micros() as u32;
            let Ok(frame) = encode_frame(&Message::Ping(stamp)) else {
                break;
            };
            if stream.write_all(&frame).is_err() {
                break;
            }
        }
        match rx.recv_timeout(next_ping.saturating_duration_since(now)) {
            Ok(Out::Frame(frame)) => {
                if stream.write_all(&frame).is_err() {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Ok(Out::Close) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    // Wakes the reader too.
    let _ = stream.shutdown(Shutdown::Both);
}

struct Reader {
    shared: Arc<Shared>,
    events: Sender<SessionEvent>,
    out: Sender<Out>,
    epoch: Instant,
    role: Role,
    input_delay: u32,
    /// The last tick our commands were stamped for.
    last_stamp: Option<u32>,
    /// The only bundle tick we will accept next.
    next_tick: u32,
    assembler: SnapshotAssembler,
}

impl Reader {
    fn run(mut self, mut stream: TcpStream) {
        let end = loop {
            match read_frame(&mut stream) {
                Ok(msg) => match self.handle(msg) {
                    Ok(None) => {}
                    Ok(Some(end)) => break end,
                    Err(e) => break EndReason::ConnectionLost(e.to_string()),
                },
                Err(e) => break EndReason::ConnectionLost(e.to_string()),
            }
        };
        if !self.shared.closing.swap(true, Ordering::SeqCst) {
            let _ = self.events.send(SessionEvent::Ended(end));
        }
        let _ = self.out.send(Out::Close);
    }

    fn emit(&self, event: SessionEvent) {
        let _ = self.events.send(event);
    }

    fn handle(&mut self, msg: Message) -> Result<Option<EndReason>, NetError> {
        match msg {
            Message::Welcome(w) => {
                self.input_delay = w.config.input_delay;
                self.emit(SessionEvent::Joined(w));
            }
            Message::Refused { reason, detail } => {
                return Ok(Some(EndReason::Refused { reason, detail }))
            }
            Message::Lobby(l) => self.emit(SessionEvent::Lobby(l)),
            Message::Start(start) => {
                self.input_delay = start.input_delay;
                self.next_tick = 0;
                self.last_stamp = None;
                self.shared.started.store(true, Ordering::SeqCst);
                self.emit(SessionEvent::Started(start));
            }
            Message::Bundle(bundle) => {
                if bundle.tick != self.next_tick {
                    return Err(NetError::Malformed("bundle out of sequence"));
                }
                self.next_tick += 1;
                if self.role == Role::Player {
                    self.cut_turn(bundle.tick + self.input_delay)?;
                }
                self.emit(SessionEvent::TickReady(bundle));
            }
            Message::SnapshotChunk {
                tick,
                total_len,
                offset,
                data,
            } => {
                if let Some((tick, blob)) = self.assembler.push(tick, total_len, offset, &data)? {
                    self.next_tick = tick + 1;
                    self.emit(SessionEvent::SnapshotLoaded { tick, blob });
                }
            }
            Message::SnapshotRequest { tick } => self.emit(SessionEvent::SnapshotWanted { tick }),
            Message::Desync { tick, hashes } => self.emit(SessionEvent::Desync { tick, hashes }),
            Message::DesyncDetail {
                tick,
                slot,
                sections,
            } => self.emit(SessionEvent::DesyncDetail {
                tick,
                slot,
                sections,
            }),
            Message::PlayerDropped(s) => self.emit(SessionEvent::PlayerDropped(s)),
            Message::PlayerRejoined(s) => self.emit(SessionEvent::PlayerRejoined(s)),
            Message::Chat {
                from,
                name,
                to,
                text,
            } => self.emit(SessionEvent::Chat {
                from,
                name,
                to,
                text,
            }),
            Message::Loading { loaded } => self.emit(SessionEvent::Loading { loaded }),
            Message::Clock {
                paused,
                by,
                input_delay,
            } => {
                self.input_delay = input_delay;
                self.emit(SessionEvent::Clock {
                    paused,
                    by,
                    input_delay,
                });
            }
            Message::NetStats(stats) => self.emit(SessionEvent::NetStats(stats)),
            Message::Ping(n) => {
                let _ = self.out.send(Out::Frame(encode_frame(&Message::Pong(n))?));
            }
            Message::Pong(stamp) => {
                let now = self.epoch.elapsed().as_micros() as u32;
                self.shared
                    .rtt_us
                    .store(now.wrapping_sub(stamp) as u64, Ordering::Relaxed);
            }
            Message::MatchEnd => return Ok(Some(EndReason::Finished)),
            Message::Hello(_)
            | Message::Ready(_)
            | Message::SetSetup(_)
            | Message::SetOptions(_)
            | Message::StartRequest
            | Message::Commands { .. }
            | Message::Hash { .. }
            | Message::Leave
            | Message::Loaded
            | Message::DesyncReport { .. }
            | Message::Pause(_)
            | Message::SetOpenSeats(_)
            | Message::TakeSeat(_)
            | Message::Kick(_)
            | Message::Listing { .. }
            | Message::SetContent(_) => {
                return Err(NetError::Malformed("client-only message from the relay"))
            }
        }
        Ok(None)
    }

    /// Sends this turn's commands, empty or not: the relay closes a tick when
    /// it has heard from everyone. What does not fit the budget waits a tick.
    /// A stamp at or below the last one (the delay just shrank) sends nothing:
    /// the relay has already heard from us for that tick.
    fn cut_turn(&mut self, tick: u32) -> Result<(), NetError> {
        if self.last_stamp.is_some_and(|last| tick <= last) {
            return Ok(());
        }
        self.last_stamp = Some(tick);
        let commands = {
            let mut pending = lock(&self.shared.pending);
            let mut budget = MAX_COMMANDS_BYTES;
            let taken = take_commands(&mut pending.commands, &mut budget);
            pending.bytes -= MAX_COMMANDS_BYTES - budget;
            taken
        };
        let _ = self.out.send(Out::Frame(encode_frame(&Message::Commands {
            tick,
            commands,
        })?));
        Ok(())
    }
}

/// Locks, carrying on through a poisoned mutex: the data behind it (queued
/// commands) stays valid even if a thread panicked while holding it.
fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
