//! A network lobby as a model: the connection to the room, what the relay says
//! about it, the plan for the match, and the start.
//!
//! The plan is a line-up (`ui::lineup`), the same one skirmish sets up. The
//! host owns it (mode, map, which seats are open to people and which are AI,
//! teams, landing zones, colours, the rules) and publishes it as the match
//! options, the open-seat mask, the content and the listing. Everyone else's
//! line-up is rebuilt from the options. Each person picks their own race
//! (`SeatChoice`). An open seat nobody takes at the start is played by an AI.

use crate::match_options::{MatchOptions, SeatChoice};
use crate::ui::faction::Pick;
use crate::ui::lineup::roster::Control;
use crate::ui::lineup::{Catalog, Lineup, Mode, Occupant, OPEN_NAME};
use mc_core::PlayerId;
use mc_net::{
    ClientConfig, EndReason, LanBeacon, LanInfo, Link, LobbyState, NetSession, PeerStat,
    RelayHandle, RoomCode, Session, SessionEvent,
};
use mc_sim::tables::Controller;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// Chat lines a lobby keeps.
const CHAT_KEPT: usize = 80;
/// A lobby that has not shown its plan this long after we set off to join it
/// says so, rather than spin on.
const JOIN_TIMEOUT: Duration = Duration::from_secs(20);

/// A game hosted from this machine for its network: the relay and its beacon. The
/// game lasts as long as this does: the host leaving ends it for everyone.
pub struct LanHost {
    relay: Option<RelayHandle>,
    /// `None` when the network's announcement port was taken: joining by address still works.
    pub beacon: Option<LanBeacon>,
}

impl LanHost {
    pub fn new(relay: RelayHandle, beacon: Option<LanBeacon>) -> LanHost {
        LanHost {
            relay: Some(relay),
            beacon,
        }
    }
}

impl Drop for LanHost {
    fn drop(&mut self) {
        if let Some(relay) = self.relay.take() {
            // Its threads get a moment to say goodbye; not on the caller's frame.
            let _ = std::thread::Builder::new()
                .name("mc-relay-stop".into())
                .spawn(move || {
                    if let Err(e) = relay.shutdown() {
                        log::warn!("the hosted relay did not stop cleanly: {e}");
                    }
                });
        }
    }
}

/// Where the room is.
pub enum Place {
    /// On a server, by code.
    Server { code: RoomCode, private: bool },
    /// A relay on this network, maybe our own.
    Lan { addr: String },
}

pub struct ChatLine {
    pub from: Option<u8>,
    pub name: String,
    pub text: String,
}

/// Everything the match needs once the room starts it.
pub struct Launch {
    pub session: NetSession,
    /// `Started` and whatever came after it in the same poll.
    pub prefetched: Vec<SessionEvent>,
    pub local: u8,
    /// No seat: this machine watches.
    pub observing: bool,
    pub options: MatchOptions,
    pub map: std::sync::Arc<mc_map::MapFile>,
    pub rejoin: crate::netplay::Rejoin,
    /// What must stay alive for the match: the directory link (its ticket), our relay.
    pub keep: Vec<Box<dyn std::any::Any + Send>>,
}

pub struct Lobby {
    session: Option<NetSession>,
    pending: Option<Receiver<std::io::Result<NetSession>>>,
    pub place: Place,
    /// How to reconnect: where, and who (the token is filled in on `Joined`).
    rejoin_addr: String,
    rejoin: ClientConfig,
    pub slot: Option<u8>,
    pub state: Option<LobbyState>,
    pub options: Option<MatchOptions>,
    /// The plan: this machine's own while it hosts, else the host's as published.
    pub lineup: Option<Lineup>,
    /// This machine hosts: its line-up is the plan everyone plays.
    planning: bool,
    /// The host's map, when this machine does not have it.
    pub missing_map: Option<String>,
    /// The plan as last published, to publish only changes.
    published: Option<(Vec<u8>, mc_core::PlayerMask, u64)>,
    pub choice: SeatChoice,
    pub stats: Vec<PeerStat>,
    pub chat: Vec<ChatLine>,
    pub draft: String,
    pub error: Option<String>,
    pub title: String,
    pub hosting: Option<LanHost>,
    started: Option<Vec<SessionEvent>>,
    ready: bool,
    /// The relay's countdown as last heard, and when: the screen counts down from it.
    countdown: Option<(u32, Instant)>,
    /// When we set off to join or open it.
    opened: Instant,
}

impl Lobby {
    /// A lobby whose connection is still being made; `plan` is set when this machine hosts.
    pub fn new(
        pending: Receiver<std::io::Result<NetSession>>,
        place: Place,
        rejoin_addr: String,
        rejoin: ClientConfig,
        plan: Option<Lineup>,
        title: String,
    ) -> Lobby {
        let planning = plan.is_some();
        Lobby {
            session: None,
            pending: Some(pending),
            place,
            rejoin_addr,
            rejoin,
            slot: None,
            state: None,
            options: None,
            lineup: plan,
            planning,
            missing_map: None,
            published: None,
            choice: SeatChoice::default(),
            stats: Vec::new(),
            chat: Vec::new(),
            draft: String::new(),
            error: None,
            title,
            hosting: None,
            started: None,
            ready: false,
            countdown: None,
            opened: Instant::now(),
        }
    }

    /// This machine opened the lobby, or took it over, and its plan is the one published.
    pub fn planning(&self) -> bool {
        self.planning
    }

    pub fn connecting(&self) -> bool {
        self.pending.is_some() || (self.session.is_some() && self.slot.is_none())
    }

    pub fn is_host(&self) -> bool {
        match (&self.state, self.slot) {
            (Some(s), Some(me)) => s.host == Some(PlayerId(me)),
            _ => false,
        }
    }

    pub fn ready(&self) -> bool {
        self.ready
    }

    /// Milliseconds before the start, while the countdown runs.
    pub fn countdown_left(&self) -> Option<u32> {
        let (ms, at) = self.countdown?;
        Some(ms.saturating_sub(at.elapsed().as_millis() as u32).max(1))
    }

    /// Who sits in each seat, as the line-up shows them.
    pub fn occupants(&self) -> Vec<Option<Occupant>> {
        let seats = self.lineup.as_ref().map_or(0, |l| l.roster.seats.len());
        let mut out: Vec<Option<Occupant>> = vec![None; seats.max(mc_core::MAX_PLAYERS)];
        let Some(state) = &self.state else {
            return out;
        };
        for p in &state.players {
            let Some(slot) = out.get_mut(p.slot.index()) else {
                continue;
            };
            let host = state.host == Some(p.slot);
            let mut tags = Vec::new();
            if host {
                tags.push("Host".to_owned());
            }
            if self.slot == Some(p.slot.0) {
                tags.push("You".to_owned());
            }
            if p.verified {
                tags.push("Verified".to_owned());
            }
            // The host's Control cell is a Remove button: say here who is ready.
            if self.is_host() && !host {
                tags.push(if p.ready { "Ready" } else { "Not Ready" }.to_owned());
            }
            if let Some(st) = self.stats.iter().find(|s| s.slot == p.slot) {
                match st.link {
                    Link::Connected if st.rtt_ms > 0 => tags.push(format!("{} ms", st.rtt_ms)),
                    Link::Dropped => tags.push("Disconnected".into()),
                    _ => {}
                }
            }
            let race = SeatChoice::decode(&p.setup).and_then(|c| {
                if c.random {
                    Some(Pick::Random)
                } else {
                    crate::ui::faction::races()
                        .iter()
                        .position(|r| {
                            !c.faction.is_empty() && r.key.eq_ignore_ascii_case(&c.faction)
                        })
                        .map(|r| Pick::Race(r as u8))
                }
            });
            *slot = Some(Occupant {
                name: p.name.clone(),
                tags,
                ready: p.ready,
                host,
                race,
            });
        }
        out
    }

    /// Takes the connection's news; publishes the host's plan when it changed.
    pub fn pump(&mut self, catalog: &Catalog) {
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(Ok(session)) => {
                    self.session = Some(session);
                    self.pending = None;
                }
                Ok(Err(e)) => {
                    self.pending = None;
                    self.error = Some(format!("Could not reach the game: {e}"));
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.error = Some("The connection attempt stopped.".into());
                }
            }
        }
        let Some(session) = &mut self.session else {
            return;
        };
        let mut events = session.poll().into_iter();
        while let Some(event) = events.next() {
            match event {
                SessionEvent::Joined(w) => {
                    self.slot = w.slot.map(|s| s.0);
                    self.rejoin.token = Some(w.token);
                }
                SessionEvent::Lobby(state) => {
                    self.options = MatchOptions::decode(&state.options).ok();
                    if !self.planning {
                        self.follow(catalog);
                    }
                    self.countdown =
                        (state.countdown_ms > 0).then(|| (state.countdown_ms, Instant::now()));
                    self.state = Some(state);
                }
                SessionEvent::Chat {
                    from, name, text, ..
                } => {
                    self.chat.push(ChatLine {
                        from: from.map(|p| p.0),
                        name,
                        text,
                    });
                    if self.chat.len() > CHAT_KEPT {
                        self.chat.remove(0);
                    }
                }
                SessionEvent::NetStats(stats) => self.stats = stats,
                SessionEvent::Started(start) => {
                    let mut rest = vec![SessionEvent::Started(start)];
                    rest.extend(events.by_ref());
                    self.started = Some(rest);
                    return;
                }
                SessionEvent::Ended(reason) => {
                    self.error = Some(match reason {
                        EndReason::Refused { reason, detail } => {
                            let what = reason.describe();
                            let mut s = what[..1].to_uppercase() + &what[1..];
                            if !detail.is_empty() && detail != what {
                                s.push_str(&format!(" ({detail})"));
                            }
                            s
                        }
                        EndReason::ConnectionLost(e) => format!("The connection was lost: {e}"),
                        EndReason::Finished => "The room was closed.".into(),
                    });
                    self.session = None;
                    return;
                }
                _ => {}
            }
        }
        // We came in to join and found the lobby empty, so the relay made us its host:
        // there is no plan to follow and none of ours to publish.
        if self.is_host() && !self.planning && self.options.is_none() {
            return self.give_up("Nobody is hosting this game any more.");
        }
        if self.lineup.is_none() && self.opened.elapsed() >= JOIN_TIMEOUT {
            return self.give_up("The game did not answer. It may have closed.");
        }
        // A host who inherited the room takes the plan over, as the options left it.
        if self.is_host() && !self.planning && self.lineup.is_some() && self.missing_map.is_none() {
            self.planning = true;
            self.published = None;
        }
        // Someone else hosts (known once the relay has said who): theirs is the plan.
        if self.state.is_some() && self.slot.is_some() && !self.is_host() && self.planning {
            self.planning = false;
            self.follow(catalog);
        }
        self.publish(catalog);
    }

    /// Leaves the room, saying why.
    fn give_up(&mut self, why: &str) {
        self.error = Some(why.to_owned());
        self.session = None;
    }

    /// Rebuilds this machine's line-up from the host's options.
    fn follow(&mut self, catalog: &Catalog) {
        let Some(options) = &self.options else {
            return;
        };
        let lineup = self.lineup.get_or_insert_with(|| {
            let mode = if options.survival.is_some() {
                Mode::Survival
            } else {
                Mode::Skirmish
            };
            Lineup::new(catalog, mode, 0, 0, 0)
        });
        self.missing_map = (!lineup.sync(catalog, options)).then(|| options.map.clone());
    }

    /// Sends the host's plan when it differs from what was last sent.
    fn publish(&mut self, catalog: &Catalog) {
        if !self.planning {
            return;
        }
        let (Some(lineup), Some(session)) = (&self.lineup, &mut self.session) else {
            return;
        };
        let Some(options) = lineup.options(catalog, |_| (OPEN_NAME.to_owned(), Controller::Ai))
        else {
            return;
        };
        let Ok(bytes) = options.encode() else {
            return;
        };
        // Seats open to people, one bit each: the relay seats them there.
        let open = (0..lineup.roster.in_play())
            .filter(|&i| lineup.roster.seats[i].control == Control::Person)
            .fold(0, |m, i| m | mc_core::player_bit(i as u8));
        let now = (bytes, open, options.map_id);
        if self.published.as_ref() == Some(&now) {
            return;
        }
        let content = mc_net::ContentId {
            map_id: options.map_id,
            blueprint_hash: self.rejoin.content.blueprint_hash,
        };
        session.set_content(content);
        self.rejoin.content = content;
        let _ = session.set_match_options(now.0.clone());
        session.set_open_seats(now.1);
        let mode = match lineup.mode {
            Mode::Skirmish => crate::ui::teams::matchup(&lineup.roster.seated_teams()),
            Mode::Survival => Mode::Survival.label().to_owned(),
        };
        let _ = session.set_listing(&options.map, &mode);
        if let Some(beacon) = self.hosting.as_ref().and_then(|h| h.beacon.as_ref()) {
            let _ = beacon.update(LanInfo {
                title: self.title.clone(),
                host: self
                    .state
                    .as_ref()
                    .and_then(|s| s.players.first())
                    .map_or_else(String::new, |p| p.name.clone()),
                map: options.map.clone(),
                mode,
                players: self.state.as_ref().map_or(1, |s| s.players.len() as u8),
                seats: lineup.roster.in_play() as u8,
                free: open.count_ones() as u8,
                build: crate::BUILD.to_owned(),
                content,
            });
        }
        self.published = Some(now);
    }

    pub fn set_ready(&mut self, ready: bool) {
        if let Some(s) = &mut self.session {
            s.set_ready(ready);
            self.ready = ready;
        }
    }

    pub fn set_choice(&mut self, choice: SeatChoice) {
        if let Some(s) = &mut self.session {
            let _ = s.set_setup(choice.encode());
        }
        self.choice = choice;
    }

    pub fn take_seat(&mut self, seat: u8) {
        if let Some(s) = &mut self.session {
            s.take_seat(PlayerId(seat));
            self.ready = false;
        }
    }

    pub fn kick(&mut self, seat: u8) {
        if let Some(s) = &mut self.session {
            s.kick(PlayerId(seat));
        }
    }

    pub fn start(&mut self) {
        if let Some(s) = &mut self.session {
            s.request_start();
        }
    }

    pub fn send_chat(&mut self) {
        let text = self.draft.trim().to_owned();
        self.draft.clear();
        if text.is_empty() {
            return;
        }
        if let Some(s) = &mut self.session {
            if let Err(e) = s.chat(&text, 0) {
                log::warn!("lobby chat not sent: {e}");
            }
        }
    }

    /// Once the room has started: what the match needs. `keep` holds what must outlive
    /// the screen (the server link).
    pub fn launch(
        &mut self,
        catalog: &Catalog,
        mut keep: Vec<Box<dyn std::any::Any + Send>>,
    ) -> Option<Result<Launch, String>> {
        let prefetched = self.started.take()?;
        let session = self.session.take()?;
        let start = prefetched.iter().find_map(|e| match e {
            SessionEvent::Started(s) => Some(s.clone()),
            _ => None,
        })?;
        let options = match MatchOptions::from_start(&start) {
            Ok(o) => o,
            Err(e) => return Some(Err(e)),
        };
        let Some(card) = catalog
            .find(options.map_id)
            .and_then(|(mode, i)| catalog.cards(mode).get(i))
        else {
            return Some(Err(format!(
                "The host started on {}, which this game does not have.",
                options.map
            )));
        };
        if let Some(h) = self.hosting.take() {
            keep.push(Box::new(h));
        }
        Some(Ok(Launch {
            session,
            prefetched,
            local: self.slot.unwrap_or(0),
            observing: self.slot.is_none(),
            map: card.map.clone(),
            rejoin: crate::netplay::Rejoin {
                addr: self.rejoin_addr.clone(),
                config: self.rejoin.clone(),
            },
            options,
            keep,
        }))
    }
}
