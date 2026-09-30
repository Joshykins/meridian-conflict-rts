//! The relay: lobby, turn closing, desync detection, join-in-progress, replay.
//!
//! One thread (the hub) owns all state and makes every decision, so bundle
//! contents have a single author. Around it: an accept thread, and per
//! connection a reader thread (socket → hub channel) and a writer thread
//! (outbox channel → socket), so a slow peer never stalls the hub.
//!
//! **Lobby.** Whoever opens the room hosts. The host sets the match options,
//! which seats people may take, and starts the match once everyone else is
//! ready; a short countdown runs first, and anything that changes the lobby
//! cancels it.
//!
//! **Loading.** After `Start` the clock waits until every player has built the
//! match (`Loaded`), or `load_timeout` has passed; a slow loader then catches up
//! from the bundles like a lagging player.
//!
//! **Turns.** The relay is the match clock. Tick `N` is due one tick interval
//! after tick `N-1` closed. When it is due the relay waits until every
//! connected, caught-up player has sent `Commands` for `N`, but no longer than
//! `turn_timeout`; then it closes the tick with whatever it has. Whatever the
//! relay puts in the bundle *is* the tick, on every machine, so closing without
//! a straggler cannot desync anyone. The straggler is marked lagging and not
//! waited for again until one of its `Commands` arrives on time. Commands that
//! arrive for an already closed tick run in the next open tick instead; they
//! are never dropped. Ticks below the starting input delay are closed
//! unconditionally: clients send `Commands` for `R + input_delay` in response to
//! bundle `R`, so nobody can have sent anything for them.
//!
//! **Input delay.** The relay pings every connection and, with
//! `adaptive_delay`, sets the delay to cover the slowest player's round trip
//! (raised at once, lowered slowly). Clients stamp with the delay from the last
//! `Clock` and never stamp a tick at or below one they already sent, so a
//! change never breaks the order of anyone's commands.
//!
//! **Pause.** Any player may stop the clock and any player may restart it;
//! everyone is told who did.
//!
//! **Joining a running match.** The relay asks one healthy player for a
//! snapshot at the next tick to close, `S`. The request travels in front of
//! bundle `S` on the same stream, so the player is guaranteed not to have
//! stepped `S` yet. The joiner then receives the snapshot, every bundle after
//! `S` from the log, and from that point the live stream. If no player can
//! provide a snapshot the joiner gets the whole log from tick 0 instead.
//!
//! **Rooms.** [`RelayServer`] listens on its own socket and hosts one match.
//! A server hosting many matches runs each as a [`Room`] and hands it the
//! connections it has routed there, `Hello` already read.

mod hub;
mod joins;
mod lobby;
mod turns;

use std::io;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use mc_core::{MAX_PLAYERS, TICKS_PER_SECOND};

use crate::protocol::{read_frame, ContentId, Hello, Message, MAX_INPUT_DELAY};
use crate::wire::NetError;
use hub::Hub;

#[derive(Clone, Debug)]
pub struct RelayConfig {
    /// Player slots, `1..=MAX_PLAYERS`.
    pub players: u8,
    pub max_observers: u16,
    /// Ticks between a command being issued and executed, `1..=MAX_INPUT_DELAY`: the
    /// delay the match starts with. Round trips up to `input_delay` tick intervals
    /// cost no stalls.
    pub input_delay: u32,
    /// Follow the players' round trips with the input delay once the match runs.
    pub adaptive_delay: bool,
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
    /// How long the clock waits for every player to load the match.
    pub load_timeout: Duration,
    /// From the host's start to the match's.
    pub countdown: Duration,
    /// How often the relay measures every connection's round trip.
    pub ping_interval: Duration,
    /// Start as soon as every slot is filled and ready, without waiting for the host.
    pub auto_start: bool,
    /// Fixed match seed; random when `None`.
    pub seed: Option<u64>,
    /// Required content; when `None` the first player to join defines it.
    pub content: Option<ContentId>,
    /// Where to write `match-<unix time>-<seed>.mcreplay`.
    pub replay_dir: Option<PathBuf>,
    pub title: String,
    /// Seats a person may take until the host says otherwise, one bit per slot.
    pub open_seats: mc_core::PlayerMask,
}

impl Default for RelayConfig {
    fn default() -> Self {
        RelayConfig {
            players: MAX_PLAYERS as u8,
            max_observers: 16,
            input_delay: 2,
            adaptive_delay: true,
            tick_interval: Duration::from_millis(1000 / TICKS_PER_SECOND as u64),
            turn_timeout: Duration::from_millis(300),
            handshake_timeout: Duration::from_secs(5),
            peer_timeout: Duration::from_secs(30),
            snapshot_timeout: Duration::from_secs(20),
            load_timeout: Duration::from_secs(90),
            countdown: Duration::from_secs(3),
            ping_interval: Duration::from_millis(500),
            auto_start: false,
            seed: None,
            content: None,
            replay_dir: None,
            title: String::new(),
            open_seats: mc_core::PlayerMask::MAX,
        }
    }
}

impl RelayConfig {
    fn check(&self) -> io::Result<()> {
        let invalid = |what| Err(io::Error::new(io::ErrorKind::InvalidInput, what));
        if self.players == 0 || self.players as usize > MAX_PLAYERS {
            return invalid("players must be 1..=MAX_PLAYERS");
        }
        if self.input_delay == 0 || self.input_delay > MAX_INPUT_DELAY {
            return invalid("input_delay must be 1..=50 ticks");
        }
        if self.tick_interval.is_zero() || self.ping_interval.is_zero() {
            // Ticks nobody is waited for (below the input delay, all players lagging) would close in a busy loop.
            return invalid("tick_interval and ping_interval must be non-zero");
        }
        Ok(())
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RoomPhase {
    #[default]
    Lobby,
    Loading,
    Playing {
        tick: u32,
    },
    Ended,
}

/// A room as a game browser lists it. The hub keeps it current.
#[derive(Clone, Debug, Default)]
pub struct RoomStatus {
    pub title: String,
    /// The host's name; empty while nobody is in.
    pub host: String,
    /// The seated players' names, in seat order.
    pub players: Vec<String>,
    pub seats: u8,
    /// Open seats nobody has taken.
    pub free: u8,
    pub observers: u16,
    pub phase: RoomPhase,
    pub content: Option<ContentId>,
    pub build: String,
    /// What the host says about the match (`Listing`).
    pub map: String,
    pub mode: String,
}

pub struct RelayServer {
    listener: TcpListener,
    config: RelayConfig,
}

impl RelayServer {
    pub fn bind(addr: impl ToSocketAddrs, config: RelayConfig) -> io::Result<RelayServer> {
        config.check()?;
        Ok(RelayServer {
            listener: TcpListener::bind(addr)?,
            config,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Hosts one match on the calling thread. Returns when the last player has left a started match.
    pub fn run(self) -> io::Result<RelaySummary> {
        let (tx, rx) = mpsc::channel();
        let status = Arc::default();
        self.run_with(tx, rx, status)
    }

    /// Hosts one match on background threads.
    pub fn spawn(self) -> io::Result<RelayHandle> {
        let addr = self.local_addr()?;
        let (tx, rx) = mpsc::channel();
        let hub_tx = tx.clone();
        let status: Arc<Mutex<RoomStatus>> = Arc::default();
        let hub_status = status.clone();
        let thread = thread::Builder::new()
            .name("mc-relay-hub".into())
            .spawn(move || self.run_with(hub_tx, rx, hub_status))?;
        Ok(RelayHandle {
            addr,
            room: Room { tx, thread, status },
        })
    }

    fn run_with(
        self,
        tx: Sender<Event>,
        rx: Receiver<Event>,
        status: Arc<Mutex<RoomStatus>>,
    ) -> io::Result<RelaySummary> {
        self.listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let accept = {
            let (listener, tx, stop) = (self.listener, tx.clone(), stop.clone());
            thread::Builder::new()
                .name("mc-relay-accept".into())
                .spawn(move || accept_thread(listener, tx, stop))?
        };
        let mut hub = Hub::new(self.config, tx, status, true);
        hub.run(rx);
        stop.store(true, Ordering::SeqCst);
        let _ = accept.join();
        Ok(hub.finish())
    }
}

pub struct RelayHandle {
    addr: SocketAddr,
    room: Room,
}

impl RelayHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn is_finished(&self) -> bool {
        self.room.is_finished()
    }

    pub fn status(&self) -> RoomStatus {
        self.room.status()
    }

    /// Ends the match now (completing the replay), disconnects everyone and waits for the threads.
    pub fn shutdown(self) -> io::Result<RelaySummary> {
        self.room.shutdown()
    }

    /// Waits for the match to end by itself.
    pub fn join(self) -> io::Result<RelaySummary> {
        self.room.join()
    }
}

/// One match inside a server that hosts many. It has no socket of its own: the
/// server routes connections to it with [`Room::adopt`].
pub struct Room {
    tx: Sender<Event>,
    thread: JoinHandle<io::Result<RelaySummary>>,
    status: Arc<Mutex<RoomStatus>>,
}

impl Room {
    pub fn spawn(config: RelayConfig) -> io::Result<Room> {
        config.check()?;
        let (tx, rx) = mpsc::channel();
        let hub_tx = tx.clone();
        let status: Arc<Mutex<RoomStatus>> = Arc::default();
        let hub_status = status.clone();
        let thread = thread::Builder::new()
            .name("mc-room-hub".into())
            .spawn(move || {
                let mut hub = Hub::new(config, hub_tx, hub_status, false);
                hub.run(rx);
                Ok(hub.finish())
            })?;
        Ok(Room { tx, thread, status })
    }

    /// Hands over a connection whose `Hello` the server has read (and whose name it
    /// has checked when `verified`). The room keeps `hold` for as long as it keeps
    /// the connection and then drops it, so a server can count the connections
    /// it has handed out. False if the room has closed; the stream and `hold`
    /// went with the call either way.
    pub fn adopt(&self, stream: TcpStream, hello: Hello, verified: bool, hold: Hold) -> bool {
        self.tx
            .send(Event::Adopted(Adoption {
                stream,
                hello: Box::new(hello),
                verified,
                hold,
            }))
            .is_ok()
    }

    pub fn status(&self) -> RoomStatus {
        match self.status.lock() {
            Ok(s) => s.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
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

/// Anything a server wants dropped when a room lets go of a connection.
pub type Hold = Box<dyn Send>;

/// A connection the server routed to a room; its `Hello` has been read.
struct Adoption {
    stream: TcpStream,
    hello: Box<Hello>,
    verified: bool,
    hold: Hold,
}

enum Event {
    Accepted(TcpStream),
    Adopted(Adoption),
    Message(ConnId, Message),
    Closed(ConnId, NetError),
    Shutdown,
}

const ACCEPT_POLL: Duration = Duration::from_millis(10);

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

/// `hello_first`: the connection has not introduced itself yet, and must with `Hello`.
fn reader_thread(
    id: ConnId,
    mut stream: TcpStream,
    tx: Sender<Event>,
    peer_timeout: Duration,
    hello_first: bool,
) {
    // The handshake timeout was set by the hub; it becomes the idle timeout after `Hello`.
    let mut first = hello_first;
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
    use std::io::Write;
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
    let _ = stream.shutdown(if graceful {
        Shutdown::Write
    } else {
        Shutdown::Both
    });
}
