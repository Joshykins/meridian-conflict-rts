//! `meridian-server`: the public game server.
//!
//! One TCP port does everything. The front door ([`door`]) reads each
//! connection's first frame and hands it on:
//!
//! * A directory connection (`DirHello`) becomes a session ([`session`]): the
//!   player signs in with their device key, hosts rooms, finds them by code and
//!   watches the list of open games.
//! * A match connection (`Hello` with a room code) goes to that room's hub
//!   ([`rooms`]), with the name the player's sign-in ticket proves.
//!
//! The server has no game data and runs no simulation. Each room is an
//! `mc_net::Room`, the same lockstep hub a game hosts for LAN play. What the
//! server keeps on disk is in its data directory: `names.json` (which device
//! key owns which name) and `replays/` (every match it hosted).

mod door;
mod names;
mod replays;
mod rooms;
mod session;

use std::collections::BTreeMap;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use mc_net::directory::{MAX_LISTED_ROOMS, MAX_MOTD_LEN, TICKET_LEN};
use mc_net::RelayConfig;

use door::Conns;
use names::Names;
use rooms::Rooms;
use session::{Outbox, SessionId};

/// Where the server's files go, and its limits.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// `ADDR:PORT` to listen on; port 0 picks a free one.
    pub bind: String,
    /// `names.json` and `replays/` live here.
    pub data_dir: PathBuf,
    /// Rooms open at once, at most [`MAX_LISTED_ROOMS`].
    pub max_rooms: usize,
    /// Rooms one player may have open at once.
    pub rooms_per_player: usize,
    /// Connections from one address at once (directory and match together).
    pub max_per_ip: usize,
    /// Connections at once, all told.
    pub max_connections: usize,
    /// Shown to every player on sign-in.
    pub motd: String,
    /// Replays older than this many days are deleted at start and daily; 0 keeps them all.
    pub replay_days: u32,
    /// A new connection must say which protocol it speaks, and a player must answer
    /// the sign-in challenge, within this.
    pub handshake_timeout: Duration,
    /// A signed-in directory connection silent this long is dropped. Clients ping every 2 s.
    pub idle_timeout: Duration,
    /// A room its creator has not come into this long after creating it is closed.
    /// (A room whose lobby everyone has left closes at once.)
    pub host_timeout: Duration,
    /// A ticket stays good this long after its directory connection closes.
    pub ticket_grace: Duration,
    /// How often room states are read and the game list pushed out when it changed.
    pub list_interval: Duration,
    /// Every room's relay settings; the seat count, title, replay directory and
    /// open seats are set per room.
    pub room: RelayConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            bind: "0.0.0.0:7777".into(),
            data_dir: PathBuf::from("meridian-data"),
            max_rooms: 64,
            rooms_per_player: 2,
            max_per_ip: 16,
            max_connections: 1024,
            motd: String::new(),
            replay_days: 14,
            handshake_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(30),
            host_timeout: Duration::from_secs(60),
            ticket_grace: Duration::from_secs(10 * 60),
            list_interval: Duration::from_millis(500),
            room: RelayConfig::default(),
        }
    }
}

impl ServerConfig {
    fn check(&self) -> io::Result<()> {
        let invalid = |what: &str| Err(io::Error::new(io::ErrorKind::InvalidInput, what));
        if self.max_rooms == 0 || self.max_rooms > MAX_LISTED_ROOMS {
            return invalid("max rooms must be 1..=512");
        }
        if self.rooms_per_player == 0 || self.max_per_ip == 0 || self.max_connections == 0 {
            return invalid(
                "rooms per player, connections per address and connections must be at least 1",
            );
        }
        if self.motd.len() > MAX_MOTD_LEN {
            return invalid("the message of the day is over 1024 bytes");
        }
        if self.list_interval.is_zero() {
            return invalid("list interval must be non-zero");
        }
        Ok(())
    }

    fn names_path(&self) -> PathBuf {
        self.data_dir.join("names.json")
    }

    fn replay_dir(&self) -> PathBuf {
        self.data_dir.join("replays")
    }
}

/// A signed-in player's pass into match connections.
struct Ticket {
    /// The verified name, as the player last signed in with it.
    name: String,
    /// The name in lower case: whose it is.
    owner: String,
    /// The directory session it came from, while that is open.
    session: Option<SessionId>,
    /// Set once the session has closed.
    expires: Option<Instant>,
}

impl Ticket {
    fn valid(&self, now: Instant) -> bool {
        self.expires.is_none_or(|at| now < at)
    }
}

/// A signed-in directory connection.
struct Session {
    name: String,
    out: Outbox,
    /// Receives `Rooms` and `Stats` as they change.
    subscribed: bool,
}

/// Everything the server decides with, behind one lock.
struct State {
    names: Names,
    tickets: BTreeMap<[u8; TICKET_LEN], Ticket>,
    sessions: BTreeMap<SessionId, Session>,
    next_session: SessionId,
    rooms: Rooms,
}

struct Shared {
    config: ServerConfig,
    state: Mutex<State>,
    conns: Arc<Conns>,
    stop: AtomicBool,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        // Every change to the state is completed under the lock or not begun, so a
        // thread that panicked holding it left it consistent.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
}

/// A running server. [`ServerHandle::shutdown`] stops it cleanly.
pub struct ServerHandle {
    addr: SocketAddr,
    shared: Arc<Shared>,
    door: JoinHandle<()>,
    housekeeping: JoinHandle<()>,
}

/// Loads the data directory, binds the port and starts serving on background threads.
pub fn start(config: ServerConfig) -> io::Result<ServerHandle> {
    config.check()?;
    std::fs::create_dir_all(config.replay_dir())?;
    let names = Names::load(&config.names_path())?;
    log::info!(
        "{} names on record in {}",
        names.len(),
        config.names_path().display()
    );
    replays::prune(&config.replay_dir(), config.replay_days);
    let listener = TcpListener::bind(&config.bind)?;
    let addr = listener.local_addr()?;
    let shared = Arc::new(Shared {
        conns: Arc::new(Conns::new(config.max_connections, config.max_per_ip)),
        config,
        state: Mutex::new(State {
            names,
            tickets: BTreeMap::new(),
            sessions: BTreeMap::new(),
            next_session: 0,
            rooms: Rooms::default(),
        }),
        stop: AtomicBool::new(false),
    });
    let door = {
        let shared = shared.clone();
        thread::Builder::new()
            .name("mc-server-door".into())
            .spawn(move || door::accept_loop(listener, &shared))?
    };
    let housekeeping = {
        let shared = shared.clone();
        thread::Builder::new()
            .name("mc-server-rooms".into())
            .spawn(move || rooms::housekeeping(&shared))?
    };
    log::info!("listening on {addr}");
    Ok(ServerHandle {
        addr,
        shared,
        door,
        housekeeping,
    })
}

impl ServerHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stops taking connections, ends every room (their replays are completed),
    /// signs everyone out and saves the names.
    pub fn shutdown(self) {
        log::info!("shutting down");
        self.shared.stop.store(true, Ordering::SeqCst);
        let _ = self.door.join();
        let _ = self.housekeeping.join();
        let rooms = {
            let mut state = self.shared.state();
            for session in state.sessions.values() {
                session.out.close();
            }
            if let Err(e) = state.names.save() {
                log::error!("could not save the names: {e}");
            }
            state.rooms.take_all()
        };
        // Side by side: each room waits a moment for its players to hang up.
        thread::scope(|scope| {
            for room in rooms {
                scope.spawn(|| room.close("the server is shutting down"));
            }
        });
        log::info!("stopped");
    }
}
