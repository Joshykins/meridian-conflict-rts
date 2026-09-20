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
//! commands, not everybody's turn.

use std::io;
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use mc_core::PlayerId;

use crate::protocol::{
    check_commands, command_cost, encode_frame, read_frame, snapshot_chunks, take_commands,
    ContentId, Hello, Message, Role, SnapshotAssembler, MAX_CHAT_LEN, MAX_COMMANDS_BYTES,
    MAX_NAME_LEN, MAX_OPTIONS_LEN, MAX_SETUP_LEN,
};
use crate::session::{EndReason, EventQueue, Session, SessionEvent};
use crate::wire::NetError;

/// Commands submitted but not yet sent may not exceed this. It only fills up
/// when the match has stalled, and then the player must hear about it.
pub const MAX_PENDING_BYTES: usize = 4 << 20;

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub name: String,
    pub role: Role,
    pub content: ContentId,
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
}

impl NetSession {
    /// Connects and sends `Hello`. Blocks for the TCP connect only (bounded by
    /// `connect_timeout`); the relay's answer arrives through `poll` as
    /// `Joined` or `Ended(Refused)`.
    pub fn connect(addr: impl ToSocketAddrs, config: ClientConfig) -> io::Result<NetSession> {
        if config.name.len() > MAX_NAME_LEN || config.setup.len() > MAX_SETUP_LEN {
            return Err(NetError::Limit("player name or setup blob too long").into());
        }
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

    /// Host only. Takes effect once every other player is ready.
    pub fn request_start(&mut self) {
        let _ = self.send(&Message::StartRequest);
    }

    pub fn chat(&mut self, text: &str) -> Result<(), NetError> {
        if text.len() > MAX_CHAT_LEN {
            return Err(NetError::Limit("chat message over MAX_CHAT_LEN"));
        }
        self.send(&Message::Chat {
            from: None,
            text: text.to_owned(),
        })
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
        let mut pending = self.shared.pending.lock().unwrap();
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
            }
            self.events.push(event);
        }
        self.events.drain()
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
            Message::PlayerDropped(s) => self.emit(SessionEvent::PlayerDropped(s)),
            Message::PlayerRejoined(s) => self.emit(SessionEvent::PlayerRejoined(s)),
            Message::Chat { from, text } => self.emit(SessionEvent::Chat { from, text }),
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
            | Message::Leave => {
                return Err(NetError::Malformed("client-only message from the relay"))
            }
        }
        Ok(None)
    }

    /// Sends this turn's commands, empty or not: the relay closes a tick when
    /// it has heard from everyone. What does not fit the budget waits a tick.
    fn cut_turn(&mut self, tick: u32) -> Result<(), NetError> {
        let commands = {
            let mut pending = self.shared.pending.lock().unwrap();
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
