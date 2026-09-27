//! The server end to end: real sockets on loopback, a server on port 0 with a
//! data directory of its own per test, directory clients and match sessions.
//!
//! Nothing sleeps to synchronise: every wait polls a condition under a
//! generous deadline. Rooms tick every millisecond.

mod rooms;
mod sign_in;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use mc_net::{
    ContentId, DirectoryClient, DirectoryEvent, EndReason, Identity, LobbyState, NetSession,
    RelayConfig, Role, RoomCode, RoomListing, Session, SessionEvent, Welcome,
};
use mc_server::{ServerConfig, ServerHandle};

const DEADLINE: Duration = Duration::from_secs(30);
const BUILD: &str = "0.1.0+test";
const CONTENT: ContentId = ContentId {
    map_id: 0x5E4F_E400,
    blueprint_hash: 0xB1E0_0002,
};

/// A server with its own data directory, removed when dropped.
struct TestServer {
    handle: Option<ServerHandle>,
    addr: SocketAddr,
    dir: PathBuf,
}

fn data_dir() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("mc-server-test-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

impl TestServer {
    fn start(tweak: impl FnOnce(&mut ServerConfig)) -> TestServer {
        TestServer::start_in(data_dir(), tweak)
    }

    fn start_in(dir: PathBuf, tweak: impl FnOnce(&mut ServerConfig)) -> TestServer {
        let mut config = ServerConfig {
            bind: "127.0.0.1:0".into(),
            data_dir: dir.clone(),
            motd: "Welcome to the test server.".into(),
            room: RelayConfig {
                tick_interval: Duration::from_millis(1),
                countdown: Duration::ZERO,
                adaptive_delay: false,
                ..RelayConfig::default()
            },
            ..ServerConfig::default()
        };
        tweak(&mut config);
        let handle = mc_server::start(config).unwrap();
        TestServer {
            addr: handle.local_addr(),
            handle: Some(handle),
            dir,
        }
    }

    /// Stops the server and keeps its data directory, to start another on it.
    fn stop(mut self) -> PathBuf {
        if let Some(h) = self.handle.take() {
            h.shutdown();
        }
        std::mem::take(&mut self.dir)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.shutdown();
        }
        if !self.dir.as_os_str().is_empty() {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + DEADLINE;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(2));
    }
}

/// A directory client and everything it has heard.
struct Dir {
    client: DirectoryClient,
    events: Vec<DirectoryEvent>,
    /// The last `Rooms`.
    rooms: Option<Vec<RoomListing>>,
}

impl Dir {
    fn connect(server: &TestServer, name: &str, identity: &Identity) -> Dir {
        Dir {
            client: DirectoryClient::connect(server.addr, name, identity, BUILD).unwrap(),
            events: Vec::new(),
            rooms: None,
        }
    }

    /// Connects and waits for `SignedIn`.
    fn sign_in(server: &TestServer, name: &str, identity: &Identity) -> Dir {
        let mut d = Dir::connect(server, name, identity);
        d.until("sign-in", |d| d.signed_in().then_some(()));
        d
    }

    fn pump(&mut self) {
        for e in self.client.poll() {
            if let DirectoryEvent::Rooms(r) = &e {
                self.rooms = Some(r.clone());
            }
            self.events.push(e);
        }
    }

    fn until<T>(&mut self, what: &str, mut f: impl FnMut(&mut Dir) -> Option<T>) -> T {
        let deadline = Instant::now() + DEADLINE;
        loop {
            self.pump();
            if let Some(t) = f(self) {
                return t;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}; heard {:?}",
                self.events
            );
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn signed_in(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, DirectoryEvent::SignedIn { .. }))
    }

    fn refusal(&mut self) -> mc_net::DirRefuseReason {
        self.until("a refusal", |d| {
            d.events.iter().find_map(|e| match e {
                DirectoryEvent::Refused { reason, .. } => Some(*reason),
                _ => None,
            })
        })
    }

    fn create(&mut self, title: &str, seats: u8, private: bool) -> RoomCode {
        let seen = self.events.len();
        self.client
            .create_room(title, seats, private, CONTENT)
            .unwrap();
        self.until("the room", |d| {
            d.events[seen..].iter().find_map(|e| match e {
                DirectoryEvent::RoomCreated(code) => Some(*code),
                _ => None,
            })
        })
    }

    /// Waits for a game list that satisfies `f`.
    fn listed(
        &mut self,
        what: &str,
        mut f: impl FnMut(&[RoomListing]) -> bool,
    ) -> Vec<RoomListing> {
        self.until(what, |d| d.rooms.clone().filter(|r| f(r)))
    }

    /// Asks for `code` and returns the answer: the listing, or `None` for not found.
    fn find(&mut self, code: RoomCode) -> Option<RoomListing> {
        let seen = self.events.len();
        self.client.find_room(code);
        self.until("the lookup", |d| {
            d.events[seen..].iter().find_map(|e| match e {
                DirectoryEvent::RoomFound(l) if l.code == code => Some(Some(l.clone())),
                DirectoryEvent::RoomNotFound(c) if *c == code => Some(None),
                _ => None,
            })
        })
    }

    /// A match session into `code`, as this signed-in player.
    fn join(&self, code: RoomCode, role: Role) -> Match {
        let config = self.client.join_config(code, role, CONTENT).unwrap();
        Match::connect(self.client.server_addr(), config)
    }
}

/// A match connection, pumped by hand.
struct Match {
    session: NetSession,
    welcome: Option<Welcome>,
    lobby: Option<LobbyState>,
    ticks: u32,
    ended: Option<EndReason>,
}

impl Match {
    fn connect(addr: SocketAddr, config: mc_net::ClientConfig) -> Match {
        let mut session = NetSession::connect(addr, config).unwrap();
        session.set_tick_budget(10_000);
        Match {
            session,
            welcome: None,
            lobby: None,
            ticks: 0,
            ended: None,
        }
    }

    fn pump(&mut self) {
        for event in self.session.poll() {
            match event {
                SessionEvent::Joined(w) => self.welcome = Some(w),
                SessionEvent::Lobby(l) => self.lobby = Some(l),
                SessionEvent::Started(_) => self.session.loaded(),
                SessionEvent::TickReady(_) => {
                    self.ticks += 1;
                    self.session.credit_tick();
                }
                SessionEvent::Ended(end) => self.ended = Some(end),
                _ => {}
            }
        }
    }

    fn until(&mut self, what: &str, mut f: impl FnMut(&Match) -> bool) {
        let deadline = Instant::now() + DEADLINE;
        loop {
            self.pump();
            if f(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}; ended {:?}",
                self.ended
            );
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn refused(&mut self) -> mc_net::RefuseReason {
        self.until("a refusal", |m| m.ended.is_some());
        match &self.ended {
            Some(EndReason::Refused { reason, .. }) => *reason,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
