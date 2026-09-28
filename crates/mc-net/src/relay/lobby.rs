//! Joining, the lobby's seats and host, the countdown, and the start and end of
//! the match.

use std::collections::BTreeMap;
use std::io;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mc_core::PlayerId;

use super::hub::{frame, random_u64, seat_mask, Hub, Kind, Match, Slot};
use super::{ConnId, RoomPhase};
use crate::protocol::{
    Hello, LobbyPlayer, LobbyState, MatchConfig, MatchStart, Message, PlayerSetup, RefuseReason,
    Role, Welcome,
};
use crate::replay::{ReplayWriter, REPLAY_EXTENSION};
use crate::wire::NetError;

impl Hub {
    pub(super) fn on_hello(&mut self, id: ConnId, hello: Hello) {
        if self.standalone() && hello.room != 0 {
            return self.refuse(
                id,
                RefuseReason::RoomNotFound,
                "this relay hosts a single match",
            );
        }
        // In the lobby the host may still change the map, so only the unit data has to agree
        // at the door (each client finds the map the host names); once the match runs, all of it.
        let content_ok = match self.content {
            Some(c) if self.game.is_some() => c == hello.content,
            Some(c) => c.blueprint_hash == hello.content.blueprint_hash,
            None => hello.role == Role::Player,
        };
        if !content_ok {
            return match self.content {
                Some(_) => self.refuse(
                    id,
                    RefuseReason::ContentMismatch,
                    "map or unit data differ from the match's",
                ),
                None => self.refuse(
                    id,
                    RefuseReason::NoHost,
                    "no player has opened the lobby yet",
                ),
            };
        }
        if let Some(build) = self.build.as_ref().filter(|b| **b != hello.build) {
            let detail = format!("the match runs {build}, you have {}", hello.build);
            return self.refuse(id, RefuseReason::BuildMismatch, &detail);
        }
        if self.banned.contains(&hello.name.to_lowercase()) {
            return self.refuse(
                id,
                RefuseReason::Kicked,
                "the host removed you from this game",
            );
        }
        if let Some(conn) = self.conns.get_mut(&id) {
            conn.name = hello.name.clone();
        }
        if hello.role == Role::Observer {
            if self.observer_count() >= self.config.max_observers {
                return self.refuse(id, RefuseReason::LobbyFull, "observer limit reached");
            }
            let Some(conn) = self.conns.get_mut(&id) else {
                return;
            };
            conn.kind = Kind::Observer;
            let welcome = self.welcome(None, 0);
            self.send(id, &welcome);
            return self.enter(id, None);
        }

        let verified = self.conns.get(&id).is_some_and(|c| c.verified);
        let slot = if self.game.is_some() {
            let Some(token) = hello.token else {
                return self.refuse(id, RefuseReason::MatchInProgress, "the match has started");
            };
            let Some(i) = self
                .slots
                .iter()
                .position(|s| s.occupied && s.token == token)
            else {
                return self.refuse(id, RefuseReason::BadToken, "no slot with that token");
            };
            // The old connection may be half-open and not yet timed out; the token holder wins.
            // Taking the slot first keeps `drop_conn` from seeing a match with nobody in it.
            let old = self.slots[i].conn.replace(id);
            if let Some(old) = old {
                self.drop_conn(old, &NetError::Limit("replaced by a reconnect"));
                self.broadcast(&Message::PlayerDropped(PlayerId(i as u8)));
            }
            let s = &mut self.slots[i];
            s.synced = false;
            s.lagging = true;
            s.received_through = None;
            PlayerId(i as u8)
        } else {
            let free = (0..self.slots.len())
                .find(|&i| !self.slots[i].occupied && self.open & (1 << i) != 0);
            let Some(i) = free else {
                return self.refuse(id, RefuseReason::LobbyFull, "every seat is taken");
            };
            if self.slots.iter().all(|s| !s.occupied) {
                // The first one in defines what the match is played with, and hosts.
                self.content = Some(hello.content);
                self.build = Some(hello.build.clone());
                self.host = Some(i);
            }
            self.slots[i] = Slot {
                occupied: true,
                name: hello.name,
                setup: hello.setup,
                token: random_u64(),
                verified,
                conn: Some(id),
                ..Slot::default()
            };
            self.cancel_countdown();
            PlayerId(i as u8)
        };
        let Some(conn) = self.conns.get_mut(&id) else {
            return;
        };
        conn.kind = Kind::Player(slot);
        let welcome = self.welcome(Some(slot), self.slots[slot.index()].token);
        self.send(id, &welcome);
        self.enter(id, Some(slot));
    }

    fn welcome(&self, slot: Option<PlayerId>, token: u64) -> Message {
        Message::Welcome(Welcome {
            slot,
            token,
            config: MatchConfig {
                max_players: self.config.players,
                input_delay: self.input_delay,
                tick_ms: self.config.tick_interval.as_millis() as u32,
            },
            in_progress: self.game.is_some(),
        })
    }

    /// After `Welcome`: into the lobby, or into the running match.
    fn enter(&mut self, id: ConnId, slot: Option<PlayerId>) {
        let Some(m) = &mut self.game else {
            self.broadcast_lobby();
            return self.maybe_auto_start();
        };
        let start_frame = m.start_frame.clone();
        m.waiting.push(id);
        self.send_frame(id, &start_frame);
        let dropped: Vec<PlayerId> = (0..self.slots.len())
            .filter(|&i| self.slots[i].occupied && self.slots[i].conn.is_none())
            .map(|i| PlayerId(i as u8))
            .collect();
        for d in dropped {
            if Some(d) != slot {
                self.send(id, &Message::PlayerDropped(d));
            }
        }
        self.send_clock(Some(id));
        self.service_joiners(None);
    }

    /// A player left the lobby: its seat is free, and the host passes on if it was theirs.
    pub(super) fn vacate(&mut self, seat: usize) {
        self.slots[seat] = Slot::default();
        if self.slots.iter().all(|s| !s.occupied) {
            return self.empty_lobby();
        }
        if self.host == Some(seat) {
            self.host = self.slots.iter().position(|s| s.occupied);
        }
    }

    /// The last player left before the start. A server's room is over: its host
    /// chose it, and listing it on would lead players into a lobby nobody plans.
    /// A relay of its own waits for the next one to arrive and define the match afresh.
    fn empty_lobby(&mut self) {
        if !self.standalone() {
            self.done = true;
            return;
        }
        self.content = self.config.content;
        self.build = None;
        self.host = None;
        self.options.clear();
        self.open = self.config.open_seats & seat_mask(self.config.players);
        let observers: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.kind == Kind::Observer)
            .map(|(id, _)| *id)
            .collect();
        for id in observers {
            self.refuse(id, RefuseReason::NoHost, "every player left the lobby");
        }
    }

    pub(super) fn broadcast_lobby(&mut self) {
        let countdown_ms = self.countdown.map_or(0, |at| {
            at.saturating_duration_since(Instant::now()).as_millis() as u32
        });
        let state = LobbyState {
            host: self.host.map(|h| PlayerId(h as u8)),
            players: self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.occupied)
                .map(|(i, s)| LobbyPlayer {
                    slot: PlayerId(i as u8),
                    name: s.name.clone(),
                    ready: s.ready,
                    setup: s.setup.clone(),
                    verified: s.verified,
                })
                .collect(),
            observers: self.observer_count(),
            options: self.options.clone(),
            open: self.open,
            // A countdown that is due now still says so, so the screen shows it.
            countdown_ms: if self.countdown.is_some() {
                countdown_ms.max(1)
            } else {
                0
            },
            title: self.config.title.clone(),
        };
        self.broadcast(&Message::Lobby(state));
        self.publish_status();
    }

    pub(super) fn maybe_auto_start(&mut self) {
        let open_all_taken =
            (0..self.slots.len()).all(|i| self.slots[i].occupied || self.open & (1 << i) == 0);
        let all_ready = self.slots.iter().all(|s| !s.occupied || s.ready);
        let anyone = self.slots.iter().any(|s| s.occupied);
        if self.config.auto_start && open_all_taken && all_ready && anyone {
            self.start_match();
        }
    }

    /// The host asked to start: the countdown runs once everyone else is ready.
    pub(super) fn request_start(&mut self) {
        let host = self.host;
        let others_ready = self
            .slots
            .iter()
            .enumerate()
            .all(|(i, s)| !s.occupied || s.ready || Some(i) == host);
        if !others_ready || self.countdown.is_some() {
            return;
        }
        if self.config.countdown.is_zero() {
            return self.start_match();
        }
        self.countdown = Some(Instant::now() + self.config.countdown);
        self.broadcast_lobby();
    }

    /// Anything that changes the lobby stops a countdown under way.
    pub(super) fn cancel_countdown(&mut self) {
        if self.countdown.take().is_some() {
            self.broadcast_lobby();
        }
    }

    pub(super) fn set_open(&mut self, mask: u8) {
        // A seat somebody sits in stays open: closing it is the kick's job.
        let occupied = (0..self.slots.len())
            .filter(|&i| self.slots[i].occupied)
            .fold(0u8, |m, i| m | 1 << i);
        self.open = (mask & seat_mask(self.config.players)) | occupied;
        self.cancel_countdown();
        self.broadcast_lobby();
    }

    pub(super) fn take_seat(&mut self, id: ConnId, from: PlayerId, to: PlayerId) {
        let (f, t) = (from.index(), to.index());
        if t >= self.slots.len() || self.slots[t].occupied || self.open & (1 << t) == 0 {
            return;
        }
        let mut moved = std::mem::take(&mut self.slots[f]);
        moved.ready = false;
        self.slots[t] = moved;
        if self.host == Some(f) {
            self.host = Some(t);
        }
        if let Some(conn) = self.conns.get_mut(&id) {
            conn.kind = Kind::Player(to);
        }
        // The seat is part of the welcome: the client learns its new slot from it.
        let welcome = self.welcome(Some(to), self.slots[t].token);
        self.send(id, &welcome);
        self.cancel_countdown();
        self.broadcast_lobby();
    }

    pub(super) fn kick(&mut self, seat: PlayerId) {
        let i = seat.index();
        if i >= self.slots.len() || !self.slots[i].occupied || self.host == Some(i) {
            return;
        }
        self.banned.push(self.slots[i].name.to_lowercase());
        if let Some(conn) = self.slots[i].conn {
            if let Some(c) = self.conns.get_mut(&conn) {
                // Refused like a stranger: the seat is freed here, not by `drop_conn`.
                c.kind = Kind::Pending;
            }
            self.refuse(
                conn,
                RefuseReason::Kicked,
                "the host removed you from this game",
            );
        }
        self.vacate(i);
        self.cancel_countdown();
        self.broadcast_lobby();
    }

    pub(super) fn start_match(&mut self) {
        let seed = self.config.seed.unwrap_or_else(random_u64);
        let start = MatchStart {
            content: self.content.unwrap_or_default(),
            seed,
            input_delay: self.input_delay,
            players: self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.occupied)
                .map(|(i, s)| PlayerSetup {
                    slot: PlayerId(i as u8),
                    name: s.name.clone(),
                    data: s.setup.clone(),
                })
                .collect(),
            options: self.options.clone(),
        };
        let Some(start_frame) = frame(&Message::Start(start.clone())) else {
            return;
        };

        let mut replay = None;
        if let Some(dir) = &self.config.replay_dir {
            let unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let path = dir.join(format!("match-{unix}-{seed:016x}.{REPLAY_EXTENSION}"));
            match std::fs::create_dir_all(dir).and_then(|_| ReplayWriter::create(&path, &start)) {
                Ok(w) => {
                    replay = Some(w);
                    self.summary.replay_path = Some(path);
                }
                Err(e) => self.replay_failed(e),
            }
        }

        for s in self.slots.iter_mut().filter(|s| s.occupied) {
            s.synced = true;
        }
        for c in self.conns.values_mut().filter(|c| c.kind != Kind::Pending) {
            c.live = true;
        }
        let now = Instant::now();
        self.game = Some(Match {
            start_frame: start_frame.clone(),
            log: Vec::new(),
            due: now + self.config.tick_interval,
            hashes: BTreeMap::new(),
            snapshot: None,
            waiting: Vec::new(),
            replay,
            start_delay: self.input_delay,
            loading_until: Some(now + self.config.load_timeout),
            paused: None,
            desync_reported: 0,
        });
        let ids: Vec<ConnId> = self
            .conns
            .iter()
            .filter(|(_, c)| c.live)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.send_frame(id, &start_frame);
        }
        self.send_loading();
        self.publish_status();
    }

    pub(super) fn replay_failed(&mut self, e: io::Error) {
        eprintln!("mc-relay: replay recording stopped: {e}");
        self.summary.replay_error = Some(e.to_string());
        if let Some(m) = &mut self.game {
            m.replay = None;
        }
    }

    pub(super) fn end_match(&mut self) {
        let Some(mut m) = self.game.take() else {
            return;
        };
        self.summary.ticks = m.log.len() as u32;
        if let Some(mut w) = m.replay.take() {
            if let Err(e) = w.finish() {
                self.replay_failed(e);
            }
        }
        self.broadcast(&Message::MatchEnd);
        self.set_phase(RoomPhase::Ended);
    }
}
