//! Rooms: hosting them, routing match connections into them, closing them, and
//! keeping subscribed players' game lists current.

use std::collections::BTreeMap;
use std::mem::discriminant;
use std::net::{IpAddr, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use mc_net::protocol::MAX_TITLE_LEN;
use mc_net::{
    ContentId, DirMessage, DirRefuseReason, Hello, NewRoom, RefuseReason, RelayConfig, Role, Room,
    RoomCode, RoomListing, RoomPhase,
};

use crate::door::ConnSlot;
use crate::replays::{prune, PRUNE_INTERVAL};
use crate::session::SessionId;
use crate::{Session, Shared, State};

/// The housekeeping thread checks for a stop this often between rounds.
const STOP_POLL: Duration = Duration::from_millis(20);
/// Tries at a fresh random code before giving up; with a few dozen rooms among
/// 887 million codes, the first try all but always does.
const CODE_TRIES: usize = 16;

pub(crate) struct Entry {
    room: Room,
    code: RoomCode,
    title: String,
    seats: u8,
    private: bool,
    /// Who created it, in lower case: the one player who may enter before a host has.
    creator: String,
    /// What the creator said it will play, listed until the host is in.
    content: ContentId,
    build: String,
    /// Creation order, which the list follows.
    seq: u64,
    /// Since when the room has had no host.
    hostless_since: Option<Instant>,
    /// The phase last logged.
    phase: RoomPhase,
}

impl Entry {
    fn listing(&self) -> RoomListing {
        let mut status = self.room.status();
        if status.seats == 0 {
            // The hub has not published its first status yet.
            status.title.clone_from(&self.title);
            status.seats = self.seats;
            status.free = 1;
        }
        let mut listing = RoomListing::new(self.code, &status, self.private);
        listing.content = listing.content.or(Some(self.content));
        if listing.build.is_empty() {
            listing.build.clone_from(&self.build);
        }
        listing
    }

    /// Ends the match now (its replay is completed) and logs how it went.
    pub(crate) fn close(self, why: &str) {
        let code = self.code;
        log::info!("room {code} closing: {why}");
        report(code, self.room.shutdown());
    }
}

fn report(code: RoomCode, result: std::io::Result<mc_net::RelaySummary>) {
    let summary = match result {
        Ok(summary) => summary,
        Err(e) => return log::error!("room {code} failed: {e}"),
    };
    let desync = summary
        .desync_tick
        .map_or(String::new(), |t| format!(", players desynced at tick {t}"));
    log::info!("room {code} ended after {} ticks{desync}", summary.ticks);
    if let Some(path) = &summary.replay_path {
        log::info!("room {code} replay: {}", path.display());
    }
    if let Some(e) = &summary.replay_error {
        log::error!("room {code} replay is incomplete: {e}");
    }
}

#[derive(Default)]
pub(crate) struct Rooms {
    by_code: BTreeMap<RoomCode, Entry>,
    next_seq: u64,
    /// What subscribers were last sent.
    listed: Vec<RoomListing>,
    stats: (u32, u32),
}

impl Rooms {
    pub(crate) fn len(&self) -> usize {
        self.by_code.len()
    }

    pub(crate) fn take_all(&mut self) -> Vec<Entry> {
        std::mem::take(&mut self.by_code).into_values().collect()
    }

    /// Public rooms still in play, oldest first.
    fn public_listing(&self) -> Vec<RoomListing> {
        let mut open: Vec<&Entry> = self.by_code.values().filter(|e| !e.private).collect();
        open.sort_by_key(|e| e.seq);
        open.iter()
            .map(|e| e.listing())
            .filter(|l| l.phase != RoomPhase::Ended)
            .collect()
    }

    /// Takes out the rooms whose match is over, and those without a host for too
    /// long; logs the others' changes of phase.
    fn sweep(&mut self, now: Instant, host_timeout: Duration) -> (Vec<Entry>, Vec<Entry>) {
        let mut over = Vec::new();
        let mut hostless = Vec::new();
        for (code, e) in &mut self.by_code {
            if e.room.is_finished() {
                over.push(*code);
                continue;
            }
            let status = e.room.status();
            if status.phase == RoomPhase::Lobby && status.host.is_empty() {
                let since = *e.hostless_since.get_or_insert(now);
                if now.duration_since(since) >= host_timeout {
                    hostless.push(*code);
                }
            } else {
                e.hostless_since = None;
            }
            if discriminant(&status.phase) != discriminant(&e.phase) {
                e.phase = status.phase;
                match status.phase {
                    RoomPhase::Loading => log::info!(
                        "room {code} started: {} ({})",
                        status.players.join(", "),
                        status.map
                    ),
                    RoomPhase::Playing { .. } => log::info!("room {code} playing"),
                    RoomPhase::Lobby | RoomPhase::Ended => {}
                }
            }
        }
        let mut take = |codes: Vec<RoomCode>| -> Vec<Entry> {
            codes
                .iter()
                .filter_map(|c| self.by_code.remove(c))
                .collect()
        };
        (take(over), take(hostless))
    }

    /// Sends subscribers the list and the counts, if either changed.
    fn publish(&mut self, sessions: &BTreeMap<SessionId, Session>) {
        let listing = self.public_listing();
        if listing != self.listed {
            push(sessions, &DirMessage::Rooms(listing.clone()));
            self.listed = listing;
        }
        let stats = (sessions.len() as u32, self.by_code.len() as u32);
        if stats != self.stats {
            let (online, rooms) = stats;
            push(sessions, &DirMessage::Stats { online, rooms });
            self.stats = stats;
        }
    }
}

fn push(sessions: &BTreeMap<SessionId, Session>, msg: &DirMessage) {
    let frame = match mc_net::directory::encode_dir_frame(msg) {
        Ok(frame) => Arc::<[u8]>::from(frame),
        Err(e) => return log::error!("could not encode the game list: {e}"),
    };
    for s in sessions.values().filter(|s| s.subscribed) {
        s.out.send_frame(&frame);
    }
}

/// A title as players will see it: no control characters, the host's name if blank.
fn clean_title(title: &str, host: &str) -> String {
    let clean: String = title
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        let mut t = format!("{host}'s game");
        t.truncate(MAX_TITLE_LEN);
        t
    } else {
        clean.to_owned()
    }
}

/// `CreateRoom`: a new hub, or the reason there is none.
pub(crate) fn create(shared: &Shared, id: SessionId, new: NewRoom) {
    let mut state = shared.state();
    let Some(name) = state.sessions.get(&id).map(|s| s.name.clone()) else {
        return;
    };
    let owner = name.to_ascii_lowercase();
    let config = &shared.config;
    let mine = state
        .rooms
        .by_code
        .values()
        .filter(|e| e.creator == owner)
        .count();
    let refusal = if mine >= config.rooms_per_player {
        let detail = format!("at most {} open games per player", config.rooms_per_player);
        Some((DirRefuseReason::TooManyRooms, detail))
    } else if state.rooms.len() >= config.max_rooms {
        log::warn!(
            "room cap of {} reached: {name} cannot host",
            config.max_rooms
        );
        let detail = "the server hosts as many games as it can".to_owned();
        Some((DirRefuseReason::ServerFull, detail))
    } else {
        None
    };
    if let Some((reason, detail)) = refusal {
        return refuse_room(&state, id, reason, detail);
    }
    let code = (0..CODE_TRIES)
        .filter_map(|_| RoomCode::random().ok())
        .find(|c| !state.rooms.by_code.contains_key(c));
    let Some(code) = code else {
        let detail = "no free game code".to_owned();
        return refuse_room(&state, id, DirRefuseReason::Other, detail);
    };
    let title = clean_title(&new.title, &name);
    let relay = RelayConfig {
        players: new.seats,
        title: title.clone(),
        replay_dir: Some(config.replay_dir()),
        // The host alone until the host opens more seats.
        open_seats: 0b1,
        ..config.room.clone()
    };
    let room = match Room::spawn(relay) {
        Ok(room) => room,
        Err(e) => {
            log::error!("could not start a room: {e}");
            let detail = "the server could not start the game".to_owned();
            return refuse_room(&state, id, DirRefuseReason::Other, detail);
        }
    };
    log::info!(
        "room {code} created by {name}: {title:?}, {} seats, {}",
        new.seats,
        if new.private { "private" } else { "public" }
    );
    let seq = state.rooms.next_seq;
    state.rooms.next_seq += 1;
    state.rooms.by_code.insert(
        code,
        Entry {
            room,
            code,
            title,
            seats: new.seats,
            private: new.private,
            creator: owner,
            content: new.content,
            build: new.build,
            seq,
            hostless_since: None,
            phase: RoomPhase::Lobby,
        },
    );
    if let Some(s) = state.sessions.get(&id) {
        s.out.send(&DirMessage::RoomCreated(code));
    }
}

fn refuse_room(state: &State, id: SessionId, reason: DirRefuseReason, detail: String) {
    if let Some(s) = state.sessions.get(&id) {
        s.out.send(&DirMessage::RoomRefused { reason, detail });
    }
}

/// `FindRoom`: listed or private.
pub(crate) fn find(shared: &Shared, id: SessionId, code: RoomCode) {
    let state = shared.state();
    let found = state.rooms.by_code.get(&code).map(Entry::listing);
    if let Some(s) = state.sessions.get(&id) {
        s.out.send(&match found {
            Some(listing) => DirMessage::RoomFound(listing),
            None => DirMessage::RoomNotFound(code),
        });
    }
}

/// `Subscribe`: turning it on sends the list and the counts at once.
pub(crate) fn subscribe(shared: &Shared, id: SessionId, on: bool) {
    let mut state = shared.state();
    let listing = state.rooms.public_listing();
    let stats = DirMessage::Stats {
        online: state.sessions.len() as u32,
        rooms: state.rooms.len() as u32,
    };
    let Some(s) = state.sessions.get_mut(&id) else {
        return;
    };
    s.subscribed = on;
    if on {
        s.out.send(&DirMessage::Rooms(listing));
        s.out.send(&stats);
    }
}

type RouteError = (TcpStream, RefuseReason, String);

/// A match connection: into its room under the name its ticket proves, or the
/// stream back with the reason it may not enter.
pub(crate) fn route(
    shared: &Shared,
    stream: TcpStream,
    mut hello: Hello,
    ip: IpAddr,
    slot: ConnSlot,
) -> Result<(), RouteError> {
    let state = shared.state();
    let refuse = |stream, reason, detail: &str| {
        log::info!(
            "{ip}: match connection to room {} refused: {}",
            hello.room,
            RefuseReason::describe(reason)
        );
        Err((stream, reason, detail.to_owned()))
    };
    let entry = RoomCode::from_u32(hello.room).and_then(|c| state.rooms.by_code.get(&c));
    let Some(entry) = entry.filter(|e| !e.room.is_finished()) else {
        return refuse(stream, RefuseReason::RoomNotFound, "no game with that code");
    };
    let now = Instant::now();
    let ticket = hello.ticket.and_then(|t| state.tickets.get(&t));
    let Some(ticket) = ticket.filter(|t| t.valid(now)) else {
        return refuse(
            stream,
            RefuseReason::BadTicket,
            "sign in to the server again",
        );
    };
    if entry.room.status().host.is_empty() && ticket.owner != entry.creator {
        return refuse(stream, RefuseReason::NoHost, "the host is still setting up");
    }
    hello.name.clone_from(&ticket.name);
    let role = match hello.role {
        Role::Player => "player",
        Role::Observer => "observer",
    };
    log::info!(
        "{ip}: {} enters room {} as a {role}",
        hello.name,
        entry.code
    );
    if !entry.room.adopt(stream, hello, true, Box::new(slot)) {
        log::info!("room {} closed as {ip} came in", entry.code);
    }
    Ok(())
}

/// Every `list_interval`: close finished and abandoned rooms, expire tickets,
/// push the game list, save names; prune replays daily. Runs until the server stops.
pub(crate) fn housekeeping(shared: &Shared) {
    let config = &shared.config;
    let mut next_round = Instant::now();
    let mut next_prune = Instant::now() + PRUNE_INTERVAL;
    while !shared.stopping() {
        let now = Instant::now();
        if now < next_round {
            thread::sleep(STOP_POLL.min(next_round - now));
            continue;
        }
        next_round = now + config.list_interval;
        let (over, hostless) = {
            let mut state = shared.state();
            let State {
                names,
                tickets,
                sessions,
                rooms,
                ..
            } = &mut *state;
            tickets.retain(|_, t| t.valid(now));
            names.save_if_due();
            let swept = rooms.sweep(now, config.host_timeout);
            rooms.publish(sessions);
            swept
        };
        for e in over {
            report(e.code, e.room.join());
        }
        for e in hostless {
            e.close("nobody hosted it");
        }
        if now >= next_prune {
            next_prune = now + PRUNE_INTERVAL;
            prune(&config.replay_dir(), config.replay_days);
        }
    }
}
