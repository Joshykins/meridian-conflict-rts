//! [`DirectoryClient`]: the game browser's connection to `meridian-server`.
//!
//! Like [`crate::NetSession`], two threads own the socket and the game loop only
//! touches channels, so nothing here blocks a frame. The reader answers the
//! sign-in challenge itself with its copy of the [`Identity`]. Requests made
//! before the server has said `SignedIn` wait in a backlog and go out, in
//! order, the moment it has: the server reads nothing but the proof until then.

use std::io;
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use super::{
    check_name, encode_dir_frame, read_dir_frame, DirHello, DirMessage, DirRefuseReason, Identity,
    NewRoom, RoomCode, RoomListing, TICKET_LEN,
};
use crate::client::ClientConfig;
use crate::protocol::{ContentId, Role, MAX_BUILD_LEN, MAX_TITLE_LEN};
use crate::wire::NetError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// The server answers every ping; this long without a word is a lost connection.
const READ_TIMEOUT: Duration = Duration::from_secs(20);
const PING_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DirectoryEvent {
    SignedIn {
        /// The name as the server knows it.
        name: String,
        ticket: [u8; TICKET_LEN],
        online: u32,
        rooms: u32,
        motd: String,
    },
    /// Sign-in failed and the connection is closed.
    Refused {
        reason: DirRefuseReason,
        detail: String,
    },
    /// Every public room, whenever the list changes (after `subscribe(true)`).
    Rooms(Vec<RoomListing>),
    Stats {
        online: u32,
        rooms: u32,
    },
    RoomCreated(RoomCode),
    /// `create_room` was turned down; the connection stays.
    RoomRefused {
        reason: DirRefuseReason,
        detail: String,
    },
    RoomFound(RoomListing),
    RoomNotFound(RoomCode),
    /// The connection is gone; nothing follows.
    Lost(String),
}

enum Out {
    Frame(Vec<u8>),
    Close,
}

/// Requests wait here until the server has signed us in.
struct Gate {
    signed_in: bool,
    backlog: Vec<Vec<u8>>,
}

struct Shared {
    gate: Mutex<Gate>,
    out: Sender<Out>,
    /// Round trip in microseconds; `u64::MAX` until the first pong.
    rtt_us: AtomicU64,
    /// Set when this side hangs up, so the reader does not report it as a loss.
    closing: AtomicBool,
}

impl Shared {
    fn gate(&self) -> MutexGuard<'_, Gate> {
        // The gate holds plain frames; a panic elsewhere leaves it usable.
        self.gate.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sends now if signed in, else once signed in.
    fn request(&self, msg: &DirMessage) -> Result<(), NetError> {
        let frame = encode_dir_frame(msg)?;
        let mut gate = self.gate();
        if gate.signed_in {
            // A dead writer means the connection is gone; `poll` reports that.
            let _ = self.out.send(Out::Frame(frame));
        } else {
            gate.backlog.push(frame);
        }
        Ok(())
    }
}

pub struct DirectoryClient {
    incoming: Receiver<DirectoryEvent>,
    shared: Arc<Shared>,
    addr: SocketAddr,
    name: String,
    build: String,
    ticket: Option<[u8; TICKET_LEN]>,
}

impl DirectoryClient {
    /// Connects and starts signing in as `name`. Blocks for the TCP connect only
    /// (a few seconds at most); the outcome arrives through `poll` as `SignedIn`
    /// or `Refused`. A name that breaks [`check_name`] is refused here, before
    /// anything is sent.
    pub fn connect(
        addr: impl ToSocketAddrs,
        name: &str,
        identity: &Identity,
        build: &str,
    ) -> io::Result<DirectoryClient> {
        check_name(name).map_err(|why| io::Error::new(io::ErrorKind::InvalidInput, why))?;
        if build.len() > MAX_BUILD_LEN {
            return Err(NetError::Limit("build over MAX_BUILD_LEN").into());
        }
        let mut last_err =
            io::Error::new(io::ErrorKind::InvalidInput, "address resolved to nothing");
        let mut stream = None;
        for a in addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&a, CONNECT_TIMEOUT) {
                Ok(s) => {
                    stream = Some((s, a));
                    break;
                }
                Err(e) => last_err = e,
            }
        }
        let Some((stream, addr)) = stream else {
            return Err(last_err);
        };
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(READ_TIMEOUT))?;
        stream.set_write_timeout(Some(READ_TIMEOUT))?;

        let (out_tx, out_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let hello = DirMessage::Hello(DirHello {
            name: name.to_owned(),
            public_key: identity.public_key(),
            build: build.to_owned(),
        });
        let _ = out_tx.send(Out::Frame(encode_dir_frame(&hello)?));
        let shared = Arc::new(Shared {
            gate: Mutex::new(Gate {
                signed_in: false,
                backlog: Vec::new(),
            }),
            out: out_tx,
            rtt_us: AtomicU64::new(u64::MAX),
            closing: AtomicBool::new(false),
        });

        let epoch = Instant::now();
        let writer_stream = stream.try_clone()?;
        let writer_shared = shared.clone();
        thread::Builder::new()
            .name("mc-dir-client-tx".into())
            .spawn(move || writer_thread(writer_stream, out_rx, &writer_shared, epoch))?;
        let reader = Reader {
            shared: shared.clone(),
            events: event_tx,
            identity: identity.clone(),
            epoch,
            challenged: false,
        };
        thread::Builder::new()
            .name("mc-dir-client-rx".into())
            .spawn(move || reader.run(stream))?;

        Ok(DirectoryClient {
            incoming: event_rx,
            shared,
            addr,
            name: name.to_owned(),
            build: build.to_owned(),
            ticket: None,
        })
    }

    /// Everything that arrived since the last call.
    pub fn poll(&mut self) -> Vec<DirectoryEvent> {
        let events: Vec<DirectoryEvent> = self.incoming.try_iter().collect();
        for event in &events {
            if let DirectoryEvent::SignedIn { name, ticket, .. } = event {
                self.name.clone_from(name);
                self.ticket = Some(*ticket);
            }
        }
        events
    }

    /// Asks for a room; `RoomCreated` or `RoomRefused` answers. An empty title
    /// lets the server name it after the host.
    pub fn create_room(
        &mut self,
        title: &str,
        seats: u8,
        private: bool,
        content: ContentId,
    ) -> Result<(), NetError> {
        if title.len() > MAX_TITLE_LEN {
            return Err(NetError::Limit("title over MAX_TITLE_LEN"));
        }
        if seats == 0 || seats as usize > mc_core::MAX_PLAYERS {
            return Err(NetError::Limit("seats must be 1..=MAX_PLAYERS"));
        }
        self.shared.request(&DirMessage::CreateRoom(NewRoom {
            title: title.to_owned(),
            seats,
            private,
            content,
            build: self.build.clone(),
        }))
    }

    /// Looks a room up by its code, listed or private: `RoomFound` or `RoomNotFound` answers.
    pub fn find_room(&mut self, code: RoomCode) {
        let _ = self.shared.request(&DirMessage::FindRoom(code));
    }

    /// Start or stop receiving `Rooms` and `Stats` as they change. Starting sends
    /// the current list at once.
    pub fn subscribe(&mut self, on: bool) {
        let _ = self.shared.request(&DirMessage::Subscribe(on));
    }

    /// The sign-in ticket, once `SignedIn` has been polled.
    pub fn ticket(&self) -> Option<[u8; TICKET_LEN]> {
        self.ticket
    }

    /// The name asked for, and once signed in the name as the server knows it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where match connections to this server's rooms go.
    pub fn server_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Round trip to the server, once measured.
    pub fn latency(&self) -> Option<Duration> {
        match self.shared.rtt_us.load(Ordering::Relaxed) {
            u64::MAX => None,
            us => Some(Duration::from_micros(us)),
        }
    }

    /// How to open the match connection to room `code` as the signed-in player;
    /// `None` until signed in. Connect it to [`Self::server_addr`].
    pub fn join_config(
        &self,
        code: RoomCode,
        role: Role,
        content: ContentId,
    ) -> Option<ClientConfig> {
        let ticket = self.ticket?;
        let mut config = ClientConfig::new(self.name.clone(), role, content);
        config.build.clone_from(&self.build);
        config.room = code.to_u32();
        config.ticket = Some(ticket);
        Some(config)
    }

    /// Signs out and closes the connection. Dropping does the same. The ticket
    /// stays good for the server's grace period, so a match joined with it
    /// carries on.
    pub fn leave(&mut self) {
        if !self.shared.closing.swap(true, Ordering::SeqCst) {
            let _ = self.shared.request(&DirMessage::Leave);
            let _ = self.shared.out.send(Out::Close);
        }
    }
}

impl Drop for DirectoryClient {
    fn drop(&mut self) {
        self.leave();
    }
}

fn writer_thread(mut stream: TcpStream, rx: Receiver<Out>, shared: &Shared, epoch: Instant) {
    use std::io::Write;
    let mut next_ping = Instant::now() + PING_INTERVAL;
    loop {
        let now = Instant::now();
        // The server reads nothing but the proof until it has signed us in.
        if now >= next_ping && shared.gate().signed_in {
            next_ping = now + PING_INTERVAL;
            let stamp = epoch.elapsed().as_micros() as u32;
            let Ok(frame) = encode_dir_frame(&DirMessage::Ping(stamp)) else {
                break;
            };
            if stream.write_all(&frame).is_err() {
                break;
            }
        }
        let wait = next_ping
            .saturating_duration_since(now)
            .max(Duration::from_millis(50));
        match rx.recv_timeout(wait) {
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
    events: Sender<DirectoryEvent>,
    identity: Identity,
    epoch: Instant,
    challenged: bool,
}

impl Reader {
    fn run(mut self, mut stream: TcpStream) {
        let lost = loop {
            let msg = match read_dir_frame(&mut stream) {
                Ok(msg) => msg,
                Err(e) => break Some(e.to_string()),
            };
            match self.handle(msg) {
                Ok(true) => {}
                Ok(false) => break None,
                Err(e) => break Some(e.to_string()),
            }
        };
        if !self.shared.closing.swap(true, Ordering::SeqCst) {
            if let Some(why) = lost {
                let _ = self.events.send(DirectoryEvent::Lost(why));
            }
        }
        let _ = self.shared.out.send(Out::Close);
    }

    fn emit(&self, event: DirectoryEvent) {
        let _ = self.events.send(event);
    }

    /// False once the server has closed the session on purpose.
    fn handle(&mut self, msg: DirMessage) -> Result<bool, NetError> {
        match msg {
            DirMessage::Challenge { nonce } => {
                if std::mem::replace(&mut self.challenged, true) {
                    return Err(NetError::Malformed("challenged twice"));
                }
                let proof = DirMessage::Proof {
                    signature: self.identity.sign(&nonce),
                };
                // Straight out, past the gate: it is the one thing the server waits for.
                let _ = self.shared.out.send(Out::Frame(encode_dir_frame(&proof)?));
            }
            DirMessage::SignedIn {
                name,
                ticket,
                online,
                rooms,
                motd,
            } => {
                let mut gate = self.shared.gate();
                if gate.signed_in {
                    return Err(NetError::Malformed("signed in twice"));
                }
                gate.signed_in = true;
                for frame in gate.backlog.drain(..) {
                    let _ = self.shared.out.send(Out::Frame(frame));
                }
                drop(gate);
                self.emit(DirectoryEvent::SignedIn {
                    name,
                    ticket,
                    online,
                    rooms,
                    motd,
                });
            }
            DirMessage::Refused { reason, detail } => {
                // The server closes after this: an end, not a loss.
                self.emit(DirectoryEvent::Refused { reason, detail });
                return Ok(false);
            }
            DirMessage::Rooms(rooms) => self.emit(DirectoryEvent::Rooms(rooms)),
            DirMessage::Stats { online, rooms } => {
                self.emit(DirectoryEvent::Stats { online, rooms })
            }
            DirMessage::RoomCreated(code) => self.emit(DirectoryEvent::RoomCreated(code)),
            DirMessage::RoomRefused { reason, detail } => {
                self.emit(DirectoryEvent::RoomRefused { reason, detail })
            }
            DirMessage::RoomFound(listing) => self.emit(DirectoryEvent::RoomFound(listing)),
            DirMessage::RoomNotFound(code) => self.emit(DirectoryEvent::RoomNotFound(code)),
            DirMessage::Ping(n) => {
                let _ = self
                    .shared
                    .out
                    .send(Out::Frame(encode_dir_frame(&DirMessage::Pong(n))?));
            }
            DirMessage::Pong(stamp) => {
                let now = self.epoch.elapsed().as_micros() as u32;
                self.shared
                    .rtt_us
                    .store(now.wrapping_sub(stamp) as u64, Ordering::Relaxed);
            }
            DirMessage::Hello(_)
            | DirMessage::Proof { .. }
            | DirMessage::CreateRoom(_)
            | DirMessage::FindRoom(_)
            | DirMessage::Subscribe(_)
            | DirMessage::Leave => {
                return Err(NetError::Malformed("client-only message from the server"))
            }
        }
        Ok(true)
    }
}
