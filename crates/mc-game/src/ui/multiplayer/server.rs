//! The link to a game server's directory: signing in, the list of games, and,
//! when the server cannot be reached, why, in words a player can act on.
//!
//! Connecting happens on a thread of its own, so a server that does not answer
//! never holds the screen still. A connection that fails is retried on its own,
//! waiting longer each time; the player can always try at once.

use mc_net::{DirRefuseReason, DirectoryClient, DirectoryEvent, Identity, RoomCode, RoomListing};
use std::io;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// The port a server listens on when its address does not say.
pub const DEFAULT_PORT: u16 = 7777;
/// Waits between automatic retries, growing to the last.
const BACKOFF: [u64; 5] = [3, 5, 10, 20, 30];

/// Why a server could not be reached, sorted by what the player can do about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The name did not resolve to any address.
    NotFound,
    /// A machine answered, but nothing listens on that port.
    NothingListening,
    /// Nothing answered at all: the server is off, or a router or firewall drops it.
    NoAnswer,
    /// This machine has no route out.
    NoNetwork,
    /// It was connected and then went away.
    Lost(String),
    Other(String),
}

impl Problem {
    fn of(e: &io::Error) -> Problem {
        use io::ErrorKind as K;
        match e.kind() {
            K::ConnectionRefused => Problem::NothingListening,
            K::TimedOut | K::WouldBlock => Problem::NoAnswer,
            K::NetworkUnreachable | K::HostUnreachable | K::NetworkDown | K::AddrNotAvailable => {
                Problem::NoNetwork
            }
            // Name resolution failures surface as `Other`/`Uncategorized`, with this text.
            _ if e.to_string().contains("resolve")
                || e.to_string().contains("known")
                || e.to_string().contains("nodename")
                || e.to_string().contains("No such host") =>
            {
                Problem::NotFound
            }
            K::InvalidInput => Problem::NotFound,
            _ => Problem::Other(e.to_string()),
        }
    }

    /// A headline, what it means, and what to try, for `addr`.
    pub fn explain(&self, addr: &str) -> (&'static str, String, Vec<&'static str>) {
        match self {
            Problem::NotFound => (
                "Address Not Found",
                format!("No machine goes by \u{201c}{addr}\u{201d}."),
                vec![
                    "Check the address for a typing mistake",
                    "Use the server's IP address instead of a name",
                ],
            ),
            Problem::NothingListening => (
                "Server Not Running",
                format!("{addr} answered, but no game server is listening there."),
                vec![
                    "Start meridian-server on that machine",
                    "Check the port: servers listen on 7777 unless told otherwise",
                ],
            ),
            Problem::NoAnswer => (
                "No Answer",
                format!("Nothing at {addr} answered in time."),
                vec![
                    "Is the server machine on, awake and running meridian-server?",
                    "Its router must forward TCP port 7777 to it",
                    "Its firewall must let meridian-server accept connections",
                ],
            ),
            Problem::NoNetwork => (
                "No Network",
                "This computer cannot reach the internet or that network.".to_owned(),
                vec!["Check your connection, then try again"],
            ),
            Problem::Lost(why) => (
                "Connection Lost",
                format!("The server went away ({why})."),
                vec!["It may have been stopped or restarted"],
            ),
            Problem::Other(why) => (
                "Could Not Connect",
                format!("Connecting to {addr} failed: {why}."),
                vec![],
            ),
        }
    }
}

/// Where the link to the server stands.
pub enum Status {
    /// No server address yet.
    NoAddress,
    /// Reaching the server, or signing in.
    Connecting { since: Instant },
    Online {
        name: String,
        online: u32,
        rooms: u32,
        motd: String,
    },
    /// It failed; the next try is at `retry_at`.
    Offline {
        problem: Problem,
        retry_at: Instant,
        attempts: u32,
    },
    /// The server turned us away; trying again will not help until something changes.
    Refused {
        reason: DirRefuseReason,
        detail: String,
    },
}

/// Something the browser asked for that has now happened.
pub enum Answer {
    Created(RoomCode),
    Refused(String),
    Found(RoomListing),
    NotFound(RoomCode),
}

pub struct Server {
    /// As typed, and as connected to (with the default port added).
    pub address: String,
    target: String,
    pub status: Status,
    client: Option<DirectoryClient>,
    connecting: Option<Receiver<io::Result<DirectoryClient>>>,
    pub rooms: Vec<RoomListing>,
    pub answers: Vec<Answer>,
    attempts: u32,
    name: String,
    identity: Option<Identity>,
}

/// `host`, `host:port`, `[v6]:port`: the port is added when it is missing.
pub fn with_port(address: &str) -> String {
    let a = address.trim();
    let has_port = match a.rsplit_once(':') {
        Some((host, port)) => {
            port.parse::<u16>().is_ok() && (!host.contains(':') || host.ends_with(']'))
        }
        None => false,
    };
    if has_port || a.is_empty() {
        a.to_owned()
    } else if a.contains(':') && !a.starts_with('[') {
        format!("[{a}]:{DEFAULT_PORT}")
    } else {
        format!("{a}:{DEFAULT_PORT}")
    }
}

impl Server {
    pub fn new(address: &str, name: &str, identity: Option<Identity>) -> Server {
        let mut s = Server {
            address: address.to_owned(),
            target: String::new(),
            status: Status::NoAddress,
            client: None,
            connecting: None,
            rooms: Vec::new(),
            answers: Vec::new(),
            attempts: 0,
            name: name.to_owned(),
            identity,
        };
        s.connect();
        s
    }

    pub fn client(&mut self) -> Option<&mut DirectoryClient> {
        self.client.as_mut()
    }

    pub fn online(&self) -> bool {
        matches!(self.status, Status::Online { .. })
    }

    /// Drops any link and connects afresh to `address` as `name`.
    pub fn reconnect(&mut self, address: &str, name: &str) {
        self.address = address.trim().to_owned();
        self.name = name.to_owned();
        self.attempts = 0;
        self.connect();
    }

    /// Try now, without waiting for the next automatic try.
    pub fn retry(&mut self) {
        self.connect();
    }

    fn connect(&mut self) {
        self.client = None;
        self.connecting = None;
        self.rooms.clear();
        if self.address.trim().is_empty() {
            self.status = Status::NoAddress;
            return;
        }
        // The server would refuse a name it cannot take; say so without asking it.
        if let Err(why) = mc_net::check_name(&self.name) {
            self.status = Status::Refused {
                reason: DirRefuseReason::BadName,
                detail: why.to_owned(),
            };
            return;
        }
        let Some(identity) = self.identity.clone() else {
            self.status = Status::Offline {
                problem: Problem::Other("this computer's key could not be made".into()),
                retry_at: Instant::now() + Duration::from_secs(3600),
                attempts: self.attempts,
            };
            return;
        };
        self.target = with_port(&self.address);
        let (tx, rx) = mpsc::channel();
        let (target, name) = (self.target.clone(), self.name.clone());
        let spawned = std::thread::Builder::new()
            .name("mc-directory-connect".into())
            .spawn(move || {
                let _ = tx.send(DirectoryClient::connect(
                    target.as_str(),
                    &name,
                    &identity,
                    crate::BUILD,
                ));
            });
        match spawned {
            Ok(_) => {
                self.connecting = Some(rx);
                self.status = Status::Connecting {
                    since: Instant::now(),
                };
            }
            Err(e) => self.failed(Problem::Other(e.to_string())),
        }
    }

    fn failed(&mut self, problem: Problem) {
        self.client = None;
        let wait = BACKOFF[(self.attempts as usize).min(BACKOFF.len() - 1)];
        self.attempts += 1;
        self.status = Status::Offline {
            problem,
            retry_at: Instant::now() + Duration::from_secs(wait),
            attempts: self.attempts,
        };
    }

    /// Call every frame: finishes connecting, takes the server's news, retries when due.
    pub fn pump(&mut self) {
        if let Some(rx) = &self.connecting {
            match rx.try_recv() {
                Ok(Ok(mut client)) => {
                    client.subscribe(true);
                    self.client = Some(client);
                    self.connecting = None;
                }
                Ok(Err(e)) => {
                    self.connecting = None;
                    log::info!("server {}: {e}", self.target);
                    self.failed(Problem::of(&e));
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.connecting = None;
                    self.failed(Problem::Other("the connection attempt stopped".into()));
                }
            }
        }
        let events = self.client.as_mut().map(|c| c.poll()).unwrap_or_default();
        for event in events {
            match event {
                DirectoryEvent::SignedIn {
                    name,
                    online,
                    rooms,
                    motd,
                    ..
                } => {
                    self.attempts = 0;
                    self.status = Status::Online {
                        name,
                        online,
                        rooms,
                        motd,
                    };
                }
                DirectoryEvent::Refused { reason, detail } => {
                    self.client = None;
                    self.status = Status::Refused { reason, detail };
                }
                DirectoryEvent::Rooms(rooms) => self.rooms = rooms,
                DirectoryEvent::Stats { online, rooms } => {
                    if let Status::Online {
                        online: o,
                        rooms: r,
                        ..
                    } = &mut self.status
                    {
                        (*o, *r) = (online, rooms);
                    }
                }
                DirectoryEvent::RoomCreated(code) => self.answers.push(Answer::Created(code)),
                DirectoryEvent::RoomRefused { reason, detail } => {
                    let why = if detail.is_empty() {
                        reason.describe().to_owned()
                    } else {
                        detail
                    };
                    self.answers.push(Answer::Refused(why));
                }
                DirectoryEvent::RoomFound(listing) => self.answers.push(Answer::Found(listing)),
                DirectoryEvent::RoomNotFound(code) => self.answers.push(Answer::NotFound(code)),
                DirectoryEvent::Lost(why) => {
                    log::info!("server {}: lost: {why}", self.target);
                    self.failed(Problem::Lost(why));
                }
            }
        }
        if let Status::Offline { retry_at, .. } = self.status {
            if Instant::now() >= retry_at {
                self.connect();
            }
        }
    }

    /// The address as connected to, port included.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// How long the round trip to the server takes, once measured.
    pub fn latency(&self) -> Option<Duration> {
        self.client.as_ref().and_then(|c| c.latency())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_get_the_default_port_only_when_they_lack_one() {
        assert_eq!(with_port("203.0.113.7"), "203.0.113.7:7777");
        assert_eq!(with_port(" games.example.com "), "games.example.com:7777");
        assert_eq!(
            with_port("games.example.com:9000"),
            "games.example.com:9000"
        );
        assert_eq!(with_port("::1"), "[::1]:7777");
        assert_eq!(with_port("[::1]:9000"), "[::1]:9000");
        assert_eq!(with_port(""), "");
    }

    #[test]
    fn a_closed_port_reads_as_a_server_not_running() {
        let e = io::Error::from(io::ErrorKind::ConnectionRefused);
        assert_eq!(Problem::of(&e), Problem::NothingListening);
        let e = io::Error::from(io::ErrorKind::TimedOut);
        assert_eq!(Problem::of(&e), Problem::NoAnswer);
    }

    #[test]
    fn nothing_listening_is_found_and_retried_without_blocking() {
        // A port nobody listens on: bind one, learn its number, close it.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let identity = Identity::from_bytes([7; 32]);
        let mut s = Server::new(&format!("127.0.0.1:{port}"), "Tester", Some(identity));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !matches!(s.status, Status::Offline { .. }) {
            assert!(Instant::now() < deadline, "never gave up on a closed port");
            s.pump();
            std::thread::sleep(Duration::from_millis(5));
        }
        match &s.status {
            Status::Offline {
                problem, attempts, ..
            } => {
                assert_eq!(*problem, Problem::NothingListening);
                assert_eq!(*attempts, 1);
            }
            _ => unreachable!(),
        }
    }
}
