//! Finding games on the local network without a server.
//!
//! A game hosted from the client runs its relay on a TCP port and announces it
//! with a [`LanBeacon`]: one small UDP broadcast a second on [`LAN_PORT`]. A
//! game browser runs a [`LanScanner`] on that port and lists every game it has
//! heard from lately, at the sender's address and the relay port the beacon
//! names. Datagrams are untrusted: anything that does not decode is ignored.

use std::collections::BTreeMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use socket2::{Domain, Protocol, Socket, Type};

use crate::protocol::{ContentId, MAX_BUILD_LEN, MAX_NAME_LEN, MAX_TITLE_LEN, PROTOCOL_VERSION};
use crate::wire::{Dec, Enc, NetError, Result};

/// The UDP port beacons are broadcast to and scanners listen on.
pub const LAN_PORT: u16 = 7778;
/// How often a beacon announces its game.
const BEACON_INTERVAL: Duration = Duration::from_secs(1);
/// A game not heard from for this long has gone.
const FORGET_AFTER: Duration = Duration::from_secs(4);
/// Games one scanner keeps track of. A LAN with more at once is being flooded;
/// the stalest are forgotten first.
const MAX_LAN_GAMES: usize = 256;
const LAN_MAGIC: u32 = u32::from_le_bytes(*b"MCLN");
/// The beacon's own layout; bumped on any change to it.
const LAN_FORMAT: u16 = 1;
/// No beacon is larger; anything larger is not one.
const MAX_DATAGRAM: usize = 1024;

/// What a beacon says about its game.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LanInfo {
    pub title: String,
    pub host: String,
    pub map: String,
    pub mode: String,
    pub players: u8,
    pub seats: u8,
    /// Open seats nobody has taken.
    pub free: u8,
    pub build: String,
    pub content: ContentId,
}

/// A game a scanner has heard.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LanGame {
    /// Where its relay listens: the sender's address and the beacon's port.
    pub addr: SocketAddr,
    /// The match protocol it speaks; another version cannot be joined.
    pub protocol: u32,
    pub info: LanInfo,
    /// Since the last beacon.
    pub age: Duration,
}

fn encode(relay_port: u16, info: &LanInfo) -> Vec<u8> {
    let mut e = Enc::new();
    e.u32(LAN_MAGIC);
    e.u16(LAN_FORMAT);
    e.u32(PROTOCOL_VERSION);
    e.u16(relay_port);
    e.str(&info.title);
    e.str(&info.host);
    e.str(&info.map);
    e.str(&info.mode);
    e.u8(info.players);
    e.u8(info.seats);
    e.u8(info.free);
    e.str(&info.build);
    e.u64(info.content.map_id);
    e.u64(info.content.blueprint_hash);
    e.buf
}

/// `(relay port, protocol, info)`.
fn decode(datagram: &[u8]) -> Result<(u16, u32, LanInfo)> {
    let mut d = Dec::new(datagram);
    if d.u32()? != LAN_MAGIC || d.u16()? != LAN_FORMAT {
        return Err(NetError::Malformed("not a Meridian Conflict beacon"));
    }
    let protocol = d.u32()?;
    let port = d.u16()?;
    let info = LanInfo {
        title: d.str(MAX_TITLE_LEN)?,
        host: d.str(MAX_NAME_LEN)?,
        map: d.str(MAX_TITLE_LEN)?,
        mode: d.str(MAX_TITLE_LEN)?,
        players: d.u8()?,
        seats: d.u8()?,
        free: d.u8()?,
        build: d.str(MAX_BUILD_LEN)?,
        content: ContentId {
            map_id: d.u64()?,
            blueprint_hash: d.u64()?,
        },
    };
    d.finish()?;
    if port == 0 {
        return Err(NetError::Malformed("beacon names no port"));
    }
    Ok((port, protocol, info))
}

/// Keeps limits the decoder enforces, so a beacon always decodes.
fn check(info: &LanInfo) -> io::Result<()> {
    let texts = [
        (&info.title, MAX_TITLE_LEN),
        (&info.host, MAX_NAME_LEN),
        (&info.map, MAX_TITLE_LEN),
        (&info.mode, MAX_TITLE_LEN),
        (&info.build, MAX_BUILD_LEN),
    ];
    if texts.iter().any(|(t, max)| t.len() > *max) {
        return Err(NetError::Limit("beacon text over its limit").into());
    }
    Ok(())
}

/// Announces a game hosted on this machine until dropped.
pub struct LanBeacon {
    info: Arc<Mutex<LanInfo>>,
    /// Dropping it wakes the thread and stops it.
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl LanBeacon {
    /// Broadcasts `info` and `relay_port` (where the game's relay listens) once a
    /// second to the whole local network.
    pub fn start(relay_port: u16, info: LanInfo) -> io::Result<LanBeacon> {
        let to = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::BROADCAST, LAN_PORT));
        LanBeacon::start_to(relay_port, info, to)
    }

    /// `start`, announcing to `to` instead of the broadcast address.
    pub(crate) fn start_to(
        relay_port: u16,
        info: LanInfo,
        to: SocketAddr,
    ) -> io::Result<LanBeacon> {
        check(&info)?;
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_broadcast(true)?;
        let info = Arc::new(Mutex::new(info));
        let (stop, stopped) = mpsc::channel::<()>();
        let shared = info.clone();
        let thread = thread::Builder::new()
            .name("mc-lan-beacon".into())
            .spawn(move || loop {
                let datagram = encode(
                    relay_port,
                    &shared.lock().unwrap_or_else(PoisonError::into_inner),
                );
                // A network that is down now may be up in a second; the next beacon tries again.
                let _ = socket.send_to(&datagram, to);
                match stopped.recv_timeout(BEACON_INTERVAL) {
                    Err(RecvTimeoutError::Timeout) => {}
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
                }
            })?;
        Ok(LanBeacon {
            info,
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// What the next beacons say.
    pub fn update(&self, info: LanInfo) -> io::Result<()> {
        check(&info)?;
        *self.info.lock().unwrap_or_else(PoisonError::into_inner) = info;
        Ok(())
    }
}

impl Drop for LanBeacon {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Listens for beacons.
pub struct LanScanner {
    socket: UdpSocket,
    games: BTreeMap<SocketAddr, (u32, LanInfo, Instant)>,
}

impl LanScanner {
    /// Listens on [`LAN_PORT`], sharing it with any other game on this machine.
    pub fn start() -> io::Result<LanScanner> {
        LanScanner::bind(LAN_PORT)
    }

    /// `start`, on another port (0: any free one).
    pub(crate) fn bind(port: u16) -> io::Result<LanScanner> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        // Several games on one machine each scan: every one of them gets every broadcast.
        socket.set_reuse_address(true)?;
        #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
        socket.set_reuse_port(true)?;
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port));
        socket.bind(&addr.into())?;
        socket.set_nonblocking(true)?;
        Ok(LanScanner {
            socket: socket.into(),
            games: BTreeMap::new(),
        })
    }

    #[cfg(test)]
    fn local_port(&self) -> io::Result<u16> {
        Ok(self.socket.local_addr()?.port())
    }

    /// Takes in every beacon that has arrived and returns the games heard from
    /// in the last few seconds.
    pub fn poll(&mut self) -> Vec<LanGame> {
        let now = Instant::now();
        let mut buf = [0u8; MAX_DATAGRAM];
        // Stops at `WouldBlock`, and at any other error: the next poll tries again.
        while let Ok((len, from)) = self.socket.recv_from(&mut buf) {
            if let Ok((port, protocol, info)) = decode(&buf[..len]) {
                let addr = SocketAddr::new(from.ip(), port);
                self.games.insert(addr, (protocol, info, now));
            }
        }
        self.games
            .retain(|_, (_, _, heard)| now.duration_since(*heard) < FORGET_AFTER);
        while self.games.len() > MAX_LAN_GAMES {
            let stalest = self
                .games
                .iter()
                .min_by_key(|(_, (_, _, heard))| *heard)
                .map(|(addr, _)| *addr);
            if let Some(addr) = stalest {
                self.games.remove(&addr);
            }
        }
        self.games
            .iter()
            .map(|(addr, (protocol, info, heard))| LanGame {
                addr: *addr,
                protocol: *protocol,
                info: info.clone(),
                age: now.duration_since(*heard),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::Rng;

    fn info(title: &str) -> LanInfo {
        LanInfo {
            title: title.into(),
            host: "Ada".into(),
            map: "Twin Shoals".into(),
            mode: "1 v 1".into(),
            players: 1,
            seats: 2,
            free: 1,
            build: "0.1.0+abc".into(),
            content: ContentId {
                map_id: 5,
                blueprint_hash: 6,
            },
        }
    }

    #[test]
    fn beacons_round_trip_and_noise_is_ignored() {
        let datagram = encode(7777, &info("Friday"));
        assert_eq!(
            decode(&datagram).unwrap(),
            (7777, PROTOCOL_VERSION, info("Friday"))
        );
        let mut rng = Rng::new(11);
        for _ in 0..2000 {
            let mut bad = datagram.clone();
            let i = rng.below(bad.len() as u32) as usize;
            bad[i] = rng.below(256) as u8;
            bad.truncate(bad.len() - rng.below(3) as usize);
            let _ = decode(&bad);
            let noise: Vec<u8> = (0..rng.below(80)).map(|_| rng.below(256) as u8).collect();
            let _ = decode(&noise);
        }
        let mut long = info("x");
        long.title = "x".repeat(MAX_TITLE_LEN + 1);
        assert!(LanBeacon::start(7777, long).is_err());
    }

    #[test]
    fn a_scanner_hears_a_beacon_and_forgets_it() {
        let mut scanner = LanScanner::bind(0).unwrap();
        let port = scanner.local_port().unwrap();
        let to = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        // Garbage on the port first: ignored.
        let junk = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        junk.send_to(b"MCLN but not really", to).unwrap();

        let beacon = LanBeacon::start_to(40_000, info("Friday"), to).unwrap();
        let heard = |scanner: &mut LanScanner, title: &str| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let games = scanner.poll();
                if let Some(g) = games.iter().find(|g| g.info.title == title) {
                    return (g.clone(), games.len());
                }
                assert!(Instant::now() < deadline, "never heard {title}");
                thread::sleep(Duration::from_millis(5));
            }
        };
        let (game, count) = heard(&mut scanner, "Friday");
        assert_eq!(count, 1);
        assert_eq!(game.addr, SocketAddr::from((Ipv4Addr::LOCALHOST, 40_000)));
        assert_eq!(game.protocol, PROTOCOL_VERSION);
        assert!(game.age < FORGET_AFTER);

        beacon.update(info("Saturday")).unwrap();
        heard(&mut scanner, "Saturday");
        drop(beacon);
        let deadline = Instant::now() + FORGET_AFTER + Duration::from_secs(10);
        while !scanner.poll().is_empty() {
            assert!(Instant::now() < deadline, "the game was never forgotten");
            thread::sleep(Duration::from_millis(50));
        }
    }
}
