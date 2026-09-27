//! A network lobby as a model: the connection to the room, what the relay says
//! about it, the host's plan for the seats, and the start.
//!
//! The host owns the plan (seat count, which seats are open to people and
//! which are AI, teams, landing zones, colours, the map, the rules) and
//! publishes it as the match options, the open-seat mask, the content and the
//! listing. Everyone else reads the options back. Each person picks their own
//! race (`SeatChoice`). An open seat nobody takes at the start is played by an AI.

use crate::match_options::{MatchOptions, SeatChoice};
use crate::setup::TEAM_COLORS;
use crate::ui::faction::{self, Pick};
use crate::ui::maps::MapCard;
use mc_core::PlayerId;
use mc_net::{
    ClientConfig, EndReason, LanBeacon, LanInfo, LobbyState, NetSession, PeerStat, RelayHandle,
    RoomCode, Session, SessionEvent,
};
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Difficulty, MatchConfig, PlayerSetup};
use std::sync::mpsc::{Receiver, TryRecvError};

/// Chat lines a lobby keeps.
const CHAT_KEPT: usize = 80;

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

/// The host's plan for one seat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeatPlan {
    /// Played by an AI; otherwise open to a person (an AI if nobody takes it).
    pub ai: bool,
    pub team: u8,
    pub start: u8,
    pub color: u8,
    pub race: Pick,
    pub difficulty: Difficulty,
}

#[derive(Clone, Debug)]
pub struct Plan {
    /// Index into the screen's map list.
    pub map: usize,
    pub seats: Vec<SeatPlan>,
    /// Seats in play, from the first; the rest do not exist.
    pub count: usize,
    pub fog: bool,
    pub seed: u64,
}

impl Plan {
    /// Two seats for people, the rest of the map's zones AI, alternating sides.
    pub fn new(map: usize, starts: usize, people: usize, seed: u64) -> Plan {
        let starts = starts.clamp(2, 8);
        Plan {
            map,
            seats: (0..8)
                .map(|i| SeatPlan {
                    ai: i >= people,
                    team: (i % 2) as u8,
                    start: i as u8,
                    color: i as u8,
                    race: Pick::default(),
                    difficulty: Difficulty::Normal,
                })
                .collect(),
            count: starts,
            fog: true,
            seed,
        }
    }

    /// The host's plan as someone else left it: when the host leaves, the next takes over.
    pub fn from_options(options: &MatchOptions, map: usize) -> Plan {
        let mut plan = Plan::new(map, options.config.players.len(), 0, options.config.seed);
        plan.count = options.config.players.len().clamp(2, 8);
        plan.fog = options.config.fog;
        for (i, p) in options.config.players.iter().enumerate() {
            let s = &mut plan.seats[i];
            s.ai = p.controller == Controller::Ai && p.name != OPEN_NAME;
            s.team = p.team;
            s.start = p.start;
            s.color = TEAM_COLORS
                .iter()
                .position(|c| *c == options.colors[i])
                .unwrap_or(i) as u8;
            s.race = faction::race_by_key(&p.faction)
                .and_then(|r| faction::races().iter().position(|x| x.key == r.key))
                .map_or(Pick::default(), |r| Pick::Race(r as u8));
            s.difficulty = p.ai.difficulty;
        }
        plan
    }

    /// Seats open to people, one bit each.
    pub fn open_mask(&self) -> u8 {
        (0..self.count)
            .filter(|&i| !self.seats[i].ai)
            .fold(0u8, |m, i| m | 1 << i)
    }

    /// The match options this plan publishes.
    pub fn options(&self, maps: &[MapCard]) -> Option<MatchOptions> {
        let card = maps.get(self.map)?;
        let mut colors = TEAM_COLORS;
        let mut ai = 0;
        let players = (0..self.count)
            .map(|i| {
                let s = &self.seats[i];
                colors[i] = TEAM_COLORS[s.color as usize % 8];
                let race = s.race.resolve(self.seed, i);
                let name = if s.ai {
                    ai += 1;
                    format!("{} AI {ai}", faction::race_of(race).abbreviation)
                } else {
                    OPEN_NAME.to_owned()
                };
                PlayerSetup {
                    name,
                    faction: faction::race_key(race),
                    ai: AiConfig {
                        difficulty: s.difficulty,
                        ..AiConfig::default()
                    },
                    team: s.team,
                    // Every seat is an AI until someone joins it (`MatchOptions::from_start`).
                    controller: Controller::Ai,
                    start: s.start,
                }
            })
            .collect();
        Some(MatchOptions {
            config: MatchConfig {
                seed: self.seed,
                players,
                cheats: false,
                fog: self.fog,
                spawn_commanders: true,
            },
            survival: None,
            colors,
            map: card.name.clone(),
            map_id: card.map.content_id(),
        })
    }
}

/// What an open seat is called in the options until someone takes it.
pub const OPEN_NAME: &str = "Open Seat";

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
    /// The host's plan, while this machine hosts.
    pub plan: Option<Plan>,
    /// The plan as last published, to publish only changes.
    published: Option<(Vec<u8>, u8, u64)>,
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
    countdown: Option<(u32, std::time::Instant)>,
}

impl Lobby {
    /// A lobby whose connection is still being made; `plan` is set when this machine hosts.
    pub fn new(
        pending: Receiver<std::io::Result<NetSession>>,
        place: Place,
        rejoin_addr: String,
        rejoin: ClientConfig,
        plan: Option<Plan>,
        title: String,
    ) -> Lobby {
        Lobby {
            session: None,
            pending: Some(pending),
            place,
            rejoin_addr,
            rejoin,
            slot: None,
            state: None,
            options: None,
            plan,
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
        }
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

    /// The map the options name, among the maps this machine has.
    pub fn map_index(&self, maps: &[MapCard]) -> Option<usize> {
        if let Some(plan) = &self.plan {
            return Some(plan.map);
        }
        let id = self.options.as_ref()?.map_id;
        maps.iter().position(|m| m.map.content_id() == id)
    }

    /// Takes the connection's news; publishes the host's plan when it changed.
    pub fn pump(&mut self, maps: &[MapCard]) {
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
                    self.countdown = (state.countdown_ms > 0)
                        .then(|| (state.countdown_ms, std::time::Instant::now()));
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
        // A host who inherited the room takes the plan over from the options.
        if self.is_host() && self.plan.is_none() {
            if let (Some(o), Some(map)) = (&self.options, self.map_index(maps)) {
                self.plan = Some(Plan::from_options(o, map));
            }
        }
        // Someone else hosts (known once the relay has said who): theirs is the plan.
        if self.state.is_some() && self.slot.is_some() && !self.is_host() {
            self.plan = None;
        }
        self.publish(maps);
    }

    /// Sends the host's plan when it differs from what was last sent.
    fn publish(&mut self, maps: &[MapCard]) {
        let (Some(plan), Some(session)) = (&self.plan, &mut self.session) else {
            return;
        };
        let Some(options) = plan.options(maps) else {
            return;
        };
        let Ok(bytes) = options.encode() else {
            return;
        };
        let now = (bytes, plan.open_mask(), options.map_id);
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
        let mode = crate::ui::teams::matchup(
            &options
                .config
                .players
                .iter()
                .map(|p| p.team)
                .collect::<Vec<_>>(),
        );
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
                seats: plan.count as u8,
                free: plan.open_mask().count_ones() as u8,
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
        maps: &[MapCard],
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
        let Some(card) = maps.iter().find(|m| m.map.content_id() == options.map_id) else {
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
