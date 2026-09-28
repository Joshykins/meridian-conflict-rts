//! The front door: accepting connections, counting them per address, and
//! reading the first frame to see which protocol each one speaks.

use std::collections::BTreeMap;
use std::io::{self, Read};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use mc_net::protocol::write_frame;
use mc_net::{
    read_first, DirMessage, DirRefuseReason, FirstFrame, Message, RefuseReason, DIRECTORY_VERSION,
    PROTOCOL_VERSION,
};

use crate::{rooms, session, Shared};

const ACCEPT_POLL: Duration = Duration::from_millis(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// After a refusal, how long we wait for the client to read it and hang up.
const LINGER: Duration = Duration::from_secs(2);
/// A warning about a cap is logged at most this often, however hard it is hit.
const WARN_INTERVAL: Duration = Duration::from_secs(10);

/// Open connections, all told and per address.
pub(crate) struct Conns {
    max_total: usize,
    max_per_ip: usize,
    counts: Mutex<Counts>,
}

struct Counts {
    total: usize,
    per_ip: BTreeMap<IpAddr, usize>,
    last_warning: Option<Instant>,
}

/// One counted connection; dropping it gives the place back.
pub(crate) struct ConnSlot {
    conns: Arc<Conns>,
    ip: IpAddr,
    /// Counted against its address too; a connection over the address's cap is
    /// only let in to be told so.
    per_ip: bool,
}

impl Drop for ConnSlot {
    fn drop(&mut self) {
        let mut c = self.conns.counts();
        c.total -= 1;
        if self.per_ip {
            if let Some(n) = c.per_ip.get_mut(&self.ip) {
                *n -= 1;
                if *n == 0 {
                    c.per_ip.remove(&self.ip);
                }
            }
        }
    }
}

impl Conns {
    pub(crate) fn new(max_total: usize, max_per_ip: usize) -> Conns {
        Conns {
            max_total,
            max_per_ip,
            counts: Mutex::new(Counts {
                total: 0,
                per_ip: BTreeMap::new(),
                last_warning: None,
            }),
        }
    }

    fn counts(&self) -> MutexGuard<'_, Counts> {
        self.counts.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `None` when the server is at its connection cap.
    fn admit(self: &Arc<Self>, ip: IpAddr) -> Option<ConnSlot> {
        let mut c = self.counts();
        if c.total >= self.max_total {
            if c.last_warning
                .is_none_or(|at| at.elapsed() >= WARN_INTERVAL)
            {
                c.last_warning = Some(Instant::now());
                log::warn!(
                    "connection cap of {} reached: new connections are closed unread",
                    self.max_total
                );
            }
            return None;
        }
        c.total += 1;
        let n = c.per_ip.entry(ip).or_insert(0);
        let per_ip = *n < self.max_per_ip;
        if per_ip {
            *n += 1;
        }
        Some(ConnSlot {
            conns: self.clone(),
            ip,
            per_ip,
        })
    }
}

pub(crate) fn accept_loop(listener: TcpListener, shared: &Arc<Shared>) {
    if let Err(e) = listener.set_nonblocking(true) {
        log::error!("the front door cannot poll for connections: {e}");
        return;
    }
    while !shared.stopping() {
        let (stream, peer) = match listener.accept() {
            Ok(accepted) => accepted,
            // WouldBlock, or a connection that died in the backlog: neither is fatal.
            Err(_) => {
                thread::sleep(ACCEPT_POLL);
                continue;
            }
        };
        let ip = peer.ip().to_canonical();
        let Some(slot) = shared.conns.admit(ip) else {
            let _ = stream.shutdown(Shutdown::Both);
            continue;
        };
        let shared = shared.clone();
        let spawned = thread::Builder::new()
            .name("mc-server-conn".into())
            .spawn(move || handshake(&shared, stream, peer, slot));
        if let Err(e) = spawned {
            // The stream and its slot went with the closure; nothing is left open.
            log::warn!("no thread for a connection from {ip}: {e}");
        }
    }
}

/// Reads the first frame and passes the connection on.
fn handshake(shared: &Arc<Shared>, stream: TcpStream, peer: SocketAddr, slot: ConnSlot) {
    let setup = || -> io::Result<()> {
        // Accepted sockets inherit non-blocking mode from the listener on some platforms.
        stream.set_nonblocking(false)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(shared.config.handshake_timeout))?;
        stream.set_write_timeout(Some(WRITE_TIMEOUT))
    };
    if setup().is_err() {
        return;
    }
    let ip = peer.ip().to_canonical();
    let first = match read_first(&mut &stream) {
        Ok(first) => first,
        Err(e) => {
            log::debug!("{ip}: not a game connection: {e}");
            let _ = stream.shutdown(Shutdown::Both);
            return;
        }
    };
    let over_cap = !slot.per_ip;
    match first {
        FirstFrame::MatchVersion(theirs) => {
            let detail = format!("the server speaks protocol {PROTOCOL_VERSION}, you {theirs}");
            refuse_match(stream, RefuseReason::VersionMismatch, &detail);
        }
        FirstFrame::DirectoryVersion(theirs) => {
            let detail =
                format!("the server's directory is version {DIRECTORY_VERSION}, yours {theirs}");
            refuse_directory(stream, DirRefuseReason::VersionMismatch, &detail);
        }
        FirstFrame::Match(_) if over_cap => {
            log::warn!("{ip}: over the cap of connections per address");
            let detail = "too many connections from your address";
            refuse_match(stream, RefuseReason::ServerFull, detail);
        }
        FirstFrame::Directory(_) if over_cap => {
            log::warn!("{ip}: over the cap of connections per address");
            refuse_directory(stream, DirRefuseReason::TooManyConnections, "");
        }
        FirstFrame::Match(hello) => {
            if let Err((stream, reason, detail)) = rooms::route(shared, stream, hello, ip, slot) {
                refuse_match(stream, reason, &detail);
            }
        }
        FirstFrame::Directory(hello) => session::run(shared, stream, hello, ip, slot),
    }
}

/// Answers a match connection with `Refused` and closes it.
pub(crate) fn refuse_match(mut stream: TcpStream, reason: RefuseReason, detail: &str) {
    let msg = Message::Refused {
        reason,
        detail: detail.to_owned(),
    };
    if write_frame(&mut stream, &msg).is_ok() {
        let _ = stream.shutdown(Shutdown::Write);
        linger(&stream);
    }
}

/// Answers a directory connection with `Refused` and closes it.
fn refuse_directory(mut stream: TcpStream, reason: DirRefuseReason, detail: &str) {
    let msg = DirMessage::Refused {
        reason,
        detail: detail.to_owned(),
    };
    if mc_net::directory::write_dir_frame(&mut stream, &msg).is_ok() {
        let _ = stream.shutdown(Shutdown::Write);
        linger(&stream);
    }
}

/// After a refusal has been sent and the sending side closed: reads until the
/// client hangs up (or a short while passes). Closing outright with the
/// client's pings unread would reset the connection and could destroy the
/// refusal before the client reads it.
pub(crate) fn linger(stream: &TcpStream) {
    let _ = stream.set_read_timeout(Some(LINGER));
    let deadline = Instant::now() + LINGER;
    let mut buf = [0u8; 4096];
    let mut reader = stream;
    while Instant::now() < deadline {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
}
