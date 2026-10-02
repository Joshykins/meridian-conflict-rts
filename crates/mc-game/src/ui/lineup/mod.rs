//! The line-up: setting up a match, the same on one machine and over a
//! network. The set-up screen (skirmish and survival) and the multiplayer
//! lobby all draw it: the chart under a bar that names the map and sums up the
//! rules (all of them are on the settings sheet it opens), and the commanders
//! on the right. A set-up on one machine opens to others as a lobby with the
//! same plan (`multiplayer::share`).
//!
//! `Lineup` is the plan (mode, map, seats, rules) with the screen's own state.
//! On one machine the player edits it; in a lobby the host edits theirs and
//! publishes it as match options, and everyone else's is rebuilt from the
//! options (`sync`). What differs between the two is said by a `Table`: who
//! may change the plan, which seat is yours, and who sits where.
//!
//! Two modes: skirmish (teams on any skirmish map) and survival (every seat a
//! defender, on one team, against the Progenitor on a survival map).

mod ai;
mod chart;
pub mod chat;
mod layout;
pub mod roster;
mod seats;
pub mod settings;

use super::faction::{self, Pick};
use super::maps::{self, Browser, BrowserAction, MapCard};
use super::race_picker::RacePicker;
use super::survival::Theatre;
use super::teams;
use super::{id, ink, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use crate::audio::Sfx;
use crate::match_options::MatchOptions;
use crate::setup::TEAM_COLORS;
use glam::Vec2;
use mc_data::survival::Domain;
use mc_data::weather::{MapConfig, SkyChoice};
use mc_sim::tables::Controller;
use mc_sim::{MatchConfig, PlayerSetup, SurvivalRules};
use roster::{Control, Roster, Seat};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

pub use chart::chart;
pub use layout::{columns, footer, header, margin, title_y};
pub use seats::commanders;
pub use settings::{CardAsk, Sheet};

/// What a seat open to people is called in published options until someone takes it.
pub const OPEN_NAME: &str = "Open Seat";

/// A new match's seed: consecutive clock readings give unrelated seeds.
pub fn fresh_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    // SplitMix64.
    let mut z = nanos.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) & 0xFFFF_FFFF
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Skirmish,
    /// Co-op: every seat defends against the Progenitor.
    Survival,
}

impl Mode {
    /// As a game is listed for others to join.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Skirmish => "Skirmish",
            Mode::Survival => "Co-op Survival",
        }
    }

    /// As the set-up screen names it.
    pub fn title(self) -> &'static str {
        match self {
            Mode::Skirmish => "Skirmish",
            Mode::Survival => "Survival",
        }
    }

    fn index(self) -> usize {
        match self {
            Mode::Skirmish => 0,
            Mode::Survival => 1,
        }
    }
}

/// The maps a set-up can pick from, and their browsers.
pub struct Catalog {
    /// Skirmish maps, smallest first.
    pub maps: Vec<MapCard>,
    /// Survival theatres, and the same as cards.
    pub theatres: Vec<Theatre>,
    pub theatre_cards: Vec<MapCard>,
    pub browser: Browser,
    pub theatre_browser: Browser,
}

impl Catalog {
    /// Every map in `maps/`; survival theatres too when `survival`.
    pub fn load(survival: bool) -> Catalog {
        Catalog::load_from(crate::setup::list_maps(), survival)
    }

    /// [`Self::load`] over the maps at `paths`. Each map's settings file is read
    /// once: a survival block makes it a theatre, and only theatres are opened then.
    pub(crate) fn load_from(paths: Vec<std::path::PathBuf>, survival: bool) -> Catalog {
        let mut maps: Vec<MapCard> = Vec::new();
        let mut found = Vec::new();
        for path in paths {
            let config = mc_data::weather::MapConfig::for_map(&path).unwrap_or_else(|e| {
                log::warn!("{e}");
                Default::default()
            });
            if config.survival.is_none() {
                maps.extend(maps::open_card(&path, &config));
            } else if survival {
                found.extend(super::survival::theatre_of(&path, config));
            }
        }
        // Smallest first: the quick 1v1 maps lead the list.
        maps.sort_by_key(|m| (m.map.info().tile_count(), m.stem.clone()));
        found
            .sort_by_key(|(t, _): &(Theatre, MapCard)| (t.map.info().tile_count(), t.stem.clone()));
        let (theatres, theatre_cards) = found.into_iter().unzip();
        Catalog::new(maps, theatres, theatre_cards)
    }

    pub fn new(maps: Vec<MapCard>, theatres: Vec<Theatre>, theatre_cards: Vec<MapCard>) -> Catalog {
        Catalog {
            browser: Browser::new(&maps),
            theatre_browser: Browser::new(&theatre_cards),
            maps,
            theatres,
            theatre_cards,
        }
    }

    pub fn cards(&self, mode: Mode) -> &[MapCard] {
        match mode {
            Mode::Skirmish => &self.maps,
            Mode::Survival => &self.theatre_cards,
        }
    }

    /// The map with this content id, and its mode.
    pub fn find(&self, map_id: u64) -> Option<(Mode, usize)> {
        let at = |cards: &[MapCard]| cards.iter().position(|m| m.map.content_id() == map_id);
        at(&self.maps)
            .map(|i| (Mode::Skirmish, i))
            .or_else(|| at(&self.theatre_cards).map(|i| (Mode::Survival, i)))
    }

    /// Keeps the thumbnails of the mode's browser flowing. The browsers share
    /// one image slot, so only the one on screen may be pumped.
    pub fn pump(&mut self, ui: &mut Ui, mode: Mode) {
        match mode {
            Mode::Skirmish => self.browser.pump(ui),
            Mode::Survival => self.theatre_browser.pump(ui),
        }
    }

    pub fn browsing(&self) -> bool {
        self.browser.is_open() || self.theatre_browser.is_open()
    }
}

/// The maps read ahead on a worker, so a screen that lists them opens at
/// once: reading every map (more so from another file system, as the Windows
/// build does from WSL) would stall the frame the screen opens on.
pub struct ReadAhead(Option<Receiver<Catalog>>);

impl ReadAhead {
    /// Starts reading every map in `maps/`, survival theatres too, and the
    /// factions the line-up offers.
    pub fn start() -> ReadAhead {
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name("map-catalog".into())
            .spawn(move || {
                faction::races();
                let _ = tx.send(Catalog::load(true));
            });
        match spawned {
            Ok(_) => ReadAhead(Some(rx)),
            Err(e) => {
                log::warn!("no thread to read the maps ahead: {e}");
                ReadAhead(None)
            }
        }
    }

    /// The maps read ahead, waiting for them if they are still being read.
    /// Another read starts at once for the next screen that asks (multiplayer
    /// reads the maps afresh on every visit).
    pub fn take(&mut self) -> Catalog {
        let read = self.0.take().and_then(|rx| rx.recv().ok());
        *self = ReadAhead::start();
        read.unwrap_or_else(|| Catalog::load(true))
    }

    /// The maps read ahead if they have been read; `None` while they are still
    /// being read.
    pub fn try_take(&mut self) -> Option<Catalog> {
        match self.0.as_ref().map(Receiver::try_recv) {
            Some(Ok(catalog)) => {
                *self = ReadAhead::start();
                Some(catalog)
            }
            Some(Err(TryRecvError::Empty)) => None,
            // No reader, or it died: read them here.
            Some(Err(TryRecvError::Disconnected)) | None => Some(self.take()),
        }
    }
}

/// Who sits in a lobby seat, as the line-up shows them.
#[derive(Clone, Debug, Default)]
pub struct Occupant {
    pub name: String,
    /// Host, You, Verified, a ping: said under the name.
    pub tags: Vec<String>,
    pub ready: bool,
    pub host: bool,
    /// Their own pick, over the host's.
    pub race: Option<Pick>,
}

/// What differs between set-ups: who may change the plan, and who sits where.
pub struct Table<'a> {
    /// May change the plan: always on one machine; a lobby's host.
    pub host: bool,
    /// The seat this machine commands; none while watching.
    pub me: Option<usize>,
    /// A network lobby: seats for people are open to whoever joins.
    pub lobby: bool,
    /// By seat; empty on one machine.
    pub people: &'a [Option<Occupant>],
    /// On one machine: every commander is an AI and you watch.
    pub observe: bool,
    /// Your callsign, for your seat on one machine.
    pub name: &'a str,
}

impl Table<'_> {
    pub fn occupant(&self, i: usize) -> Option<&Occupant> {
        self.people.get(i).and_then(Option::as_ref)
    }

    pub fn occupied(&self, i: usize) -> bool {
        self.occupant(i).is_some()
    }

    /// The seat's commander is an AI: an AI seat, or yours while you watch.
    pub fn is_ai(&self, s: &Seat) -> bool {
        s.control == Control::Ai || (s.control == Control::Person && self.observe && !self.lobby)
    }
}

/// What a frame of the line-up asks of the lobby around it.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// This machine sits in seat n.
    Sit(usize),
    /// The host removes whoever sits in seat n.
    Kick(usize),
    /// This machine picks its own race.
    MyRace(Pick),
    /// A change was refused: say why.
    Say(String),
}

/// A set-up in progress, and the screen's state for it.
pub struct Lineup {
    pub mode: Mode,
    /// Index into the mode's maps in the catalog.
    pub map: usize,
    pub roster: Roster,
    pub fog: bool,
    /// The weather and time of day the match is shown under; left alone, the map's own.
    pub sky: SkyChoice,
    pub seed: u64,
    /// Survival's rules (ignored in skirmish).
    pub rules: SurvivalRules,
    /// Each mode's map as last picked, so switching back finds it again.
    pub last_map: [usize; 2],
    pub races: RacePicker,
    /// The settings sheet: the theatre and the rules.
    pub sheet: Sheet,
    /// The AI row (by seat key) whose settings are open under it, when the
    /// rows are too many to show every AI's.
    tuning: Option<u8>,
    /// The commander (by seat key) being moved: the next landing zone clicked
    /// on the chart is theirs.
    placing: Option<u8>,
    /// The commander (by seat key) under the pointer, on the chart or in the
    /// list: the other lights up too.
    hover_seat: Option<u8>,
    /// The row (by seat key) whose colour swatches are open under it.
    coloring: Option<u8>,
    /// How far the commanders list is scrolled, in pixels, when its rows do not
    /// all fit (a 32-seat map). Eased on screen (`seats::commanders`).
    seat_scroll: f32,
    /// The team whose heading the pointer was over last frame.
    hover_team: Option<u8>,
    /// A survival fronts chip under the pointer.
    hover_domain: Option<Domain>,
    /// The chart's picture of the map.
    picture: chart::Picture,
    /// Where the chart's zone markers were drawn (tests click them).
    pub markers: Vec<Vec2>,
    /// Where each row's zone cell was drawn, by seat key (tests click them).
    pub zone_cells: Vec<(u8, Vec2)>,
}

impl Lineup {
    /// `people` seats for people (yours first), `ai` AI commanders, the rest closed.
    pub fn new(catalog: &Catalog, mode: Mode, map: usize, people: usize, ai: usize) -> Lineup {
        let mut lineup = Lineup {
            mode,
            map,
            roster: Roster::default(),
            fog: true,
            sky: SkyChoice::default(),
            seed: fresh_seed(),
            rules: SurvivalRules::default(),
            last_map: [0; 2],
            races: RacePicker::default(),
            sheet: Sheet::default(),
            tuning: None,
            placing: None,
            hover_seat: None,
            coloring: None,
            seat_scroll: 0.0,
            hover_team: None,
            hover_domain: None,
            picture: chart::Picture::default(),
            markers: Vec::new(),
            zone_cells: Vec::new(),
        };
        lineup.last_map[mode.index()] = map;
        lineup.roster = Roster::new(lineup.zones(catalog), people, ai, mode == Mode::Survival);
        lineup.settle(catalog);
        lineup
    }

    /// Something else drew in the chart's image slot: draw the chart again.
    pub fn chart_lost(&mut self) {
        self.picture.lost();
    }

    /// A commander is being moved, and Escape puts that down before it leaves the screen.
    pub fn placing(&self) -> bool {
        self.placing.is_some()
    }

    /// Landing zone `n` was clicked by whoever may plan: the commander being
    /// moved goes there (whoever held it takes theirs). With nobody being
    /// moved, a held zone picks up its commander to move, and a free one
    /// takes yours.
    fn place(&mut self, table: &Table, n: u8) -> Sfx {
        let holder = self
            .roster
            .seats
            .iter()
            .position(|s| s.open() && s.start == n);
        let moving = self.placing.and_then(|k| self.roster.index_of(k));
        match (moving, holder) {
            (Some(p), h) => {
                self.placing = None;
                if h == Some(p) {
                    return Sfx::Back;
                }
                self.roster.take_zone(p, n);
                Sfx::Tick
            }
            (None, Some(h)) => {
                self.placing = Some(self.roster.seats[h].key);
                Sfx::Select
            }
            (None, None) => match table.me {
                Some(me) => {
                    self.roster.take_zone(me, n);
                    Sfx::Tick
                }
                None => Sfx::Deny,
            },
        }
    }

    /// The map's picture is in the chart's image slot.
    #[cfg(test)]
    pub fn chart_shown(&self) -> bool {
        self.picture.is_shown()
    }

    /// The same plan (mode, map, seats, rules) with the screen's state fresh:
    /// what a set-up takes to the lobby it opens.
    pub fn plan(&self) -> Lineup {
        Lineup {
            mode: self.mode,
            map: self.map,
            roster: self.roster.clone(),
            fog: self.fog,
            sky: self.sky,
            seed: self.seed,
            rules: self.rules,
            last_map: self.last_map,
            races: RacePicker::default(),
            sheet: Sheet::default(),
            tuning: None,
            placing: None,
            hover_seat: None,
            coloring: None,
            seat_scroll: 0.0,
            hover_team: None,
            hover_domain: None,
            picture: chart::Picture::default(),
            markers: Vec::new(),
            zone_cells: Vec::new(),
        }
    }

    pub fn card<'a>(&self, catalog: &'a Catalog) -> Option<&'a MapCard> {
        catalog.cards(self.mode).get(self.map)
    }

    pub fn theatre<'a>(&self, catalog: &'a Catalog) -> Option<&'a Theatre> {
        match self.mode {
            Mode::Survival => catalog.theatres.get(self.map),
            Mode::Skirmish => None,
        }
    }

    /// Landing zones seats can take: the map's starts, or the theatre's spawns.
    pub fn zones(&self, catalog: &Catalog) -> usize {
        match self.mode {
            Mode::Skirmish => self.card(catalog).map_or(0, |m| m.starts),
            Mode::Survival => self.theatre(catalog).map_or(0, |t| t.layout.spawns.len()),
        }
        .min(mc_core::MAX_PLAYERS)
    }

    /// Each zone's map start position.
    fn zone_start(&self, catalog: &Catalog, zone: u8) -> u8 {
        match self.theatre(catalog) {
            Some(t) => t.layout.spawns.get(zone as usize).map_or(0, |s| s.start),
            None => zone,
        }
    }

    /// Each zone's place on the ground, metres.
    pub fn zone_points(&self, catalog: &Catalog) -> Vec<Vec2> {
        let Some(card) = self.card(catalog) else {
            return Vec::new();
        };
        let starts = card.map.start_positions();
        (0..self.zones(catalog))
            .map(|z| {
                starts
                    .get(self.zone_start(catalog, z as u8) as usize)
                    .map_or(Vec2::ZERO, |p| Vec2::from(p.to_f32()))
            })
            .collect()
    }

    /// Survival's lanes by domain on this theatre.
    fn counts(&self, catalog: &Catalog) -> [usize; 3] {
        self.theatre(catalog)
            .map_or([0; 3], |t| crate::survival::front_counts(&t.layout))
    }

    /// Keeps the rules possible on the map; survival's defenders all on one team.
    fn settle(&mut self, catalog: &Catalog) {
        if self.mode == Mode::Survival {
            for s in &mut self.roster.seats {
                s.team = 0;
            }
            let counts = self.counts(catalog);
            super::survival::rules::settle(&mut self.rules, counts);
        }
    }

    /// Another map of this mode. Refused when a seat somebody sits in would go.
    pub fn set_map(
        &mut self,
        catalog: &Catalog,
        map: usize,
        occupied: impl Fn(usize) -> bool,
    ) -> Result<(), roster::Refusal> {
        let before = (self.map, self.roster.clone());
        self.map = map;
        let zones = self.zones(catalog);
        if let Err(e) = self.roster.fit(zones, occupied) {
            (self.map, self.roster) = before;
            return Err(e);
        }
        self.last_map[self.mode.index()] = map;
        self.settle(catalog);
        Ok(())
    }

    /// Switches between skirmish and survival, on the map last picked in that mode.
    pub fn set_mode(
        &mut self,
        catalog: &Catalog,
        mode: Mode,
        occupied: impl Fn(usize) -> bool,
    ) -> Result<(), roster::Refusal> {
        if mode == self.mode {
            return Ok(());
        }
        if catalog.cards(mode).is_empty() {
            return Err(format!("No {} maps in maps/", mode.label().to_lowercase()));
        }
        let before = (self.mode, self.map, self.roster.clone());
        self.mode = mode;
        self.map = self.last_map[mode.index()].min(catalog.cards(mode).len() - 1);
        let zones = self.zones(catalog);
        if let Err(e) = self.roster.fit(zones, occupied) {
            (self.mode, self.map, self.roster) = before;
            return Err(e);
        }
        if mode == Mode::Skirmish {
            // Everyone on their own side again.
            for s in &mut self.roster.seats {
                s.team = s.key;
            }
        }
        self.settle(catalog);
        Ok(())
    }

    /// What keeps the match from starting, if anything.
    pub fn problem(&self, catalog: &Catalog) -> Option<&'static str> {
        let active = self.roster.seated_teams();
        match self.mode {
            _ if catalog.cards(self.mode).is_empty() => {
                Some("No maps found - bake one with mc-bake")
            }
            Mode::Skirmish if active.len() < 2 => Some("A match needs an opponent"),
            Mode::Skirmish if active.iter().all(|t| *t == active[0]) => {
                Some("Everyone is on the same team")
            }
            Mode::Survival if active.is_empty() => Some("Survival needs a defender"),
            Mode::Survival
                if super::survival::rules::attacking(&self.rules, self.counts(catalog))
                    .is_empty() =>
            {
                Some("No front attacks")
            }
            _ => None,
        }
    }

    /// "ARC AI n", numbered over the AI commanders in seat order.
    pub fn ai_name(&self, table: &Table, i: usize) -> String {
        let n = self.roster.seats[..=i]
            .iter()
            .filter(|s| table.is_ai(s))
            .count();
        format!("{} AI {n}", self.roster.seats[i].race.label())
    }

    /// The match this plan makes. `person(i)` names seat i's person and says who
    /// controls it (on one machine, you; in a lobby, `OPEN_NAME` for an AI to hold
    /// until someone joins). Random races are dealt from the seed.
    pub fn options(
        &self,
        catalog: &Catalog,
        person: impl Fn(usize) -> (String, Controller),
    ) -> Option<MatchOptions> {
        let card = self.card(catalog)?;
        let mut colors = TEAM_COLORS;
        let mut players = Vec::new();
        let mut ai = 0;
        for (i, s) in self
            .roster
            .seats
            .iter()
            .enumerate()
            .filter(|(_, s)| s.open())
        {
            colors[players.len()] = TEAM_COLORS[s.color as usize % TEAM_COLORS.len()];
            let race = s.race.resolve(self.seed, i);
            let (name, controller) = if s.control == Control::Ai {
                ai += 1;
                (
                    format!("{} AI {ai}", faction::race_of(race).abbreviation),
                    Controller::Ai,
                )
            } else {
                person(i)
            };
            players.push(PlayerSetup {
                name,
                faction: faction::race_key(race),
                ai: s.ai,
                team: s.team,
                controller,
                start: self.zone_start(catalog, s.start),
            });
        }
        let (config, survival) = match self.theatre(catalog) {
            Some(t) => {
                let engine = players.len();
                let (config, survival) =
                    crate::survival::siege_for(&t.layout, self.rules, self.seed, self.fog, players);
                if let Some(c) = colors.get_mut(engine) {
                    *c = crate::survival::ENGINE_COLOR;
                }
                (config, Some(survival))
            }
            None => (
                MatchConfig {
                    seed: self.seed,
                    players,
                    cheats: false,
                    fog: self.fog,
                    spawn_commanders: true,
                },
                None,
            ),
        };
        Some(MatchOptions {
            config,
            survival,
            colors,
            map: card.name.clone(),
            map_id: card.map.content_id(),
            sky: self.sky,
        })
    }

    /// Takes the plan from published options (someone else hosts, or the host
    /// left and this machine takes over), keeping the screen's own state.
    /// False when this machine does not have the map.
    pub fn sync(&mut self, catalog: &Catalog, options: &MatchOptions) -> bool {
        let Some((mode, map)) = catalog.find(options.map_id) else {
            return false;
        };
        self.mode = mode;
        self.map = map;
        self.last_map[mode.index()] = map;
        self.fog = options.config.fog;
        self.sky = options.sky;
        self.seed = options.config.seed;
        let engine = options.survival.as_ref().map(|s| s.engine_player as usize);
        if let Some(s) = &options.survival {
            self.rules = s.rules;
        }
        let spawns = self.theatre(catalog).map(|t| t.layout.spawns.clone());
        let mut seats = Vec::new();
        for (i, p) in options.config.players.iter().enumerate() {
            if Some(i) == engine || seats.len() >= mc_core::MAX_PLAYERS {
                continue;
            }
            let start = match &spawns {
                Some(spawns) => spawns.iter().position(|s| s.start == p.start).unwrap_or(0) as u8,
                None => p.start,
            };
            let race = faction::races()
                .iter()
                .position(|r| r.key.eq_ignore_ascii_case(&p.faction))
                .map_or(Pick::default(), |r| Pick::Race(r as u8));
            seats.push(Seat {
                key: seats.len() as u8,
                control: if p.name == OPEN_NAME || p.controller == Controller::Human {
                    Control::Person
                } else {
                    Control::Ai
                },
                team: p.team,
                start,
                color: TEAM_COLORS
                    .iter()
                    .position(|c| *c == options.colors[i])
                    .unwrap_or(i) as u8,
                race,
                ai: p.ai,
            });
        }
        self.roster = Roster { seats };
        let zones = self.zones(catalog);
        let _ = self
            .roster
            .fit(zones.max(self.roster.seats.len()), |_| true);
        true
    }
}

/// The match card at the top of `left`, with a chip per setting in `chips`.
/// Change Map opens the browser, Settings the sheet. Returns the y under it.
pub fn match_card(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &mut Catalog,
    host: bool,
    chips: &[String],
    left: Rect,
) -> f32 {
    let (browser, cards) = match lineup.mode {
        Mode::Skirmish => (&mut catalog.browser, &catalog.maps),
        Mode::Survival => (&mut catalog.theatre_browser, &catalog.theatre_cards),
    };
    let (ask, h) = settings::card(ui, left, browser, cards, lineup.map, chips, host);
    match ask {
        Some(CardAsk::ChangeMap) => browser.open(lineup.map),
        Some(CardAsk::Settings) => lineup.sheet.open(),
        None => {}
    }
    left.y + h
}

/// The settings sheet over the screen, when open: the theatre on the left, the
/// shared rules on the right with whatever `more` draws under them (from the
/// y it is given, returning the y under it), then the sky. A map with regions
/// has a weather row for each, and the sheet grows by them. `live` is false
/// while an overlay lies over the sheet.
pub fn sheet(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &mut Catalog,
    table: &Table,
    live: bool,
    more: impl FnOnce(&mut Ui, Rect, f32) -> f32,
) -> Option<Ask> {
    let mut sheet = std::mem::take(&mut lineup.sheet);
    let mut ask = None;
    let map = lineup
        .card(catalog)
        .map(|m| m.config.clone())
        .unwrap_or_default();
    let taller = (super::sky::row_count(&map) as f32 - 2.0) * RULE_PITCH;
    let size = settings::SHEET + Vec2::new(0.0, taller);
    sheet.draw(ui, "Match Settings", size, live, |ui, body| {
        let (left, right) = settings::sheet_columns(body);
        theatre(ui, lineup, catalog, table.host, left);
        let (y, a) = rules(ui, lineup, catalog, table, right);
        ask = a;
        let y = more(ui, right, y);
        sky(ui, &mut lineup.sky, &map, table.host, right, y);
    });
    lineup.sheet = sheet;
    ask
}

/// The sky, from `y`: the map's own weather (each region's, on a map with
/// regions) and time of day, or what the host picks here. Everyone sees it;
/// only the host changes it.
fn sky(ui: &mut Ui, sky: &mut SkyChoice, map: &MapConfig, host: bool, area: Rect, y: f32) {
    ui.section(area.x, y, area.w, "Sky");
    // The rows at the rules' pitch; closer together only where the screen is too
    // short for the sheet to grow by all of them. (The last row has always run a
    // little into the foot's margin.)
    let room = area.bottom() + 14.0 - (y + 20.0);
    let pitch = (room / super::sky::row_count(map) as f32).clamp(30.0, RULE_PITCH);
    let look = super::sky::Look {
        row_h: pitch - 4.0,
        pitch,
        value_w: 220.0,
        compact: false,
        enabled: host,
    };
    super::sky::rows(ui, 0, area.x, y + 20.0, area.w, look, map, sky);
}

/// The chosen map as a card; the host clicks it to open the browser.
fn theatre(ui: &mut Ui, lineup: &Lineup, catalog: &mut Catalog, host: bool, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Theatre");
    let mode = lineup.mode;
    if catalog.cards(mode).is_empty() {
        ui.text(
            area.x,
            area.y + 50.0,
            type_scale::BODY,
            rgb(palette::WARN, 1.0),
            "No maps in maps/",
        );
        return;
    }
    // As big as the column allows: the chart is what picks a map at a glance.
    let card = Rect::new(
        area.x,
        area.y + 28.0,
        area.w,
        (area.h - 48.0).clamp(THEATRE_CARD_H, 250.0),
    );
    let live = ui.interactive;
    ui.interactive = live && host;
    let (browser, cards) = match mode {
        Mode::Skirmish => (&mut catalog.browser, &catalog.maps),
        Mode::Survival => (&mut catalog.theatre_browser, &catalog.theatre_cards),
    };
    if theatre_card(ui, browser, cards, lineup.map, card) && host {
        browser.open(lineup.map);
    }
    ui.interactive = live;
}

/// Height of `theatre_card`.
pub const THEATRE_CARD_H: f32 = 206.0;

/// The theatre the set-up is on: its chart, name, summary and tags over a
/// Change Map button. True when the player asks to change it.
pub fn theatre_card(
    ui: &mut Ui,
    browser: &mut Browser,
    maps: &[MapCard],
    selected: usize,
    r: Rect,
) -> bool {
    let Some(m) = maps.get(selected) else {
        return false;
    };
    let top = Rect::new(r.x, r.y, r.w, r.h - 50.0);
    let res = ui.interact(id("theatre-card", 0), top, true);
    ui.fill(top, ink(0.5));
    ui.gradient_h(
        top,
        rgb(palette::ACCENT, 0.14 + 0.08 * res.glow),
        rgb(palette::ACCENT, 0.01),
    );
    ui.frame(top, rgb(palette::ACCENT, 0.4 + 0.3 * res.glow));
    ui.fill(
        Rect::new(top.x, top.y, 4.0, top.h),
        rgb(palette::ACCENT, 1.0),
    );
    let side = top.h - 20.0;
    browser.thumb(
        ui,
        selected,
        Rect::new(top.x + 14.0, top.y + 10.0, side, side),
        0.9 + 0.1 * res.glow,
    );
    let x = top.x + side + 30.0;
    let w = top.right() - x - 10.0;
    ui.text_fit_left(
        x,
        top.y + 26.0,
        w,
        type_scale::ITEM,
        rgb(palette::ACCENT, 1.0),
        &m.name,
    );
    let lines = [
        format!("{:.0} km  \u{b7}  {}", m.km, m.size_class().label()),
        format!("{} Players  \u{b7}  {}", m.starts, m.style.label()),
        format!("{}  \u{b7}  {} Ore Fields", m.biome.label(), m.ores),
    ];
    // The lines share what height the card has under the name.
    let pitch = ((top.h - 58.0) / 3.0).min(20.0);
    for (k, l) in lines.iter().enumerate() {
        ui.text_fit_left(
            x + 1.0,
            top.y + 50.0 + k as f32 * pitch,
            w,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            l,
        );
    }
    if top.h >= 150.0 {
        let count = format!(
            "{} Map{}",
            maps.len(),
            if maps.len() == 1 { "" } else { "s" }
        );
        ui.text_right(
            top.right() - 10.0,
            top.bottom() - 14.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &count,
        );
    }
    let change = ui.button(
        id("theatre-change", 0),
        Rect::new(r.x, r.bottom() - 40.0, r.w, 40.0),
        "Change Map",
        ButtonKind::Secondary,
        true,
    );
    if res.clicked || change {
        ui.audio.play(Sfx::Select);
        return true;
    }
    false
}

/// Pitch of the rule rows under the theatre.
pub const RULE_PITCH: f32 = 46.0;

/// A rule row's label and rule line, as the toolkit's rows draw them.
pub fn rule_label(ui: &mut Ui, r: Rect, label: &str) {
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    ui.text(
        r.x + 16.0,
        r.mid_y(),
        type_scale::BODY,
        rgb(palette::TEXT, 0.82),
        label,
    );
}

/// The rules every set-up shares, from `y`: the mode, fog, and the
/// seed (one machine's; a network match's seed is the relay's). Only the host
/// changes them. Returns the y under the last row.
fn rules(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &Catalog,
    table: &Table,
    area: Rect,
) -> (f32, Option<Ask>) {
    ui.section(area.x, area.y + 6.0, area.w, "Rules");
    let mut y = area.y + 26.0;
    let row = |y: f32| Rect::new(area.x, y, area.w, RULE_PITCH - 4.0);
    let mut ask = None;
    {
        let r = row(y);
        rule_label(ui, r, "Mode");
        let modes = [Mode::Skirmish, Mode::Survival];
        let tw = 118.0;
        for (k, mode) in modes.into_iter().enumerate() {
            let t = Rect::new(
                r.right() - (2 - k) as f32 * (tw + 4.0) + 4.0,
                r.mid_y() - 16.0,
                tw,
                32.0,
            );
            let on = lineup.mode == mode;
            let res = ui.tile(id("rule-mode", k), t, on, table.host || on);
            ui.text_centred(
                t.x + t.w * 0.5,
                t.mid_y(),
                type_scale::BUTTON,
                rgb(
                    if on { 0xFFFFFF } else { palette::DIM },
                    0.85 + 0.15 * res.glow,
                ),
                match mode {
                    Mode::Skirmish => "Skirmish",
                    Mode::Survival => "Survival",
                },
            );
            if res.clicked && !on && table.host {
                match lineup.set_mode(catalog, mode, |i| table.occupied(i)) {
                    Ok(()) => ui.audio.play(Sfx::Select),
                    Err(e) => {
                        ui.audio.play(Sfx::Deny);
                        ask = Some(Ask::Say(e));
                    }
                }
            }
        }
        y += RULE_PITCH;
    }
    let r = row(y);
    let live = ui.interactive;
    ui.interactive = live && table.host;
    ui.toggle(id("rule-fog", 0), r, "Fog of War", "", &mut lineup.fog);
    ui.interactive = live;
    y += RULE_PITCH;
    if !table.lobby {
        // Seed: shown as two groups of hex digits, with a re-roll.
        let r = row(y);
        rule_label(ui, r, "Seed");
        let reroll = Rect::new(r.right() - 96.0, r.mid_y() - 16.0, 96.0, 32.0);
        if ui.button(
            id("rule-seed", 0),
            reroll,
            "Roll",
            ButtonKind::Secondary,
            true,
        ) {
            lineup.seed = fresh_seed();
            ui.audio.play(Sfx::Tick);
        }
        ui.text_right(
            reroll.x - 14.0,
            r.mid_y(),
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &format!(
                "{:04X}-{:04X}",
                lineup.seed >> 16 & 0xFFFF,
                lineup.seed & 0xFFFF
            ),
        );
        y += RULE_PITCH;
    }
    (y, ask)
}

/// The race picker and the map browser over the screen, when open. `slot` is the
/// image slot the browser may borrow for its detail pane.
pub fn overlays(
    ui: &mut Ui,
    lineup: &mut Lineup,
    catalog: &mut Catalog,
    table: &Table,
    enter: f32,
    slot: usize,
) -> Vec<Ask> {
    let mut asks = Vec::new();
    let (fade, shift) = (ui.fade, ui.shift);
    ui.fade = enter;
    ui.shift = Vec2::ZERO;
    if let Some(chosen) = lineup.races.draw(ui) {
        match lineup.roster.index_of(chosen.seat as u8) {
            Some(i) if table.lobby && table.me == Some(i) => asks.push(Ask::MyRace(chosen.pick)),
            Some(i) if table.host => lineup.roster.seats[i].race = chosen.pick,
            _ => {}
        }
    }
    let title = match lineup.mode {
        Mode::Skirmish => "Choose a Map",
        Mode::Survival => "Choose a Survival Map",
    };
    let (browser, cards) = match lineup.mode {
        Mode::Skirmish => (&mut catalog.browser, &catalog.maps),
        Mode::Survival => (&mut catalog.theatre_browser, &catalog.theatre_cards),
    };
    let picked = browser.draw(ui, cards, title, slot);
    // The browser lent the chart's slot to its detail pane: draw ours again.
    if browser.release_slot() {
        lineup.picture.lost();
    }
    if let Some(BrowserAction::Pick(i)) = picked {
        if i != lineup.map && table.host {
            if let Err(e) = lineup.set_map(catalog, i, |i| table.occupied(i)) {
                asks.push(Ask::Say(e));
            }
        }
    }
    ui.fade = fade;
    ui.shift = shift;
    asks
}

/// The line beside the launch: the matchup and who plays, or the problem.
pub fn summary(lineup: &Lineup, catalog: &Catalog) -> String {
    let seated = lineup.roster.seated_teams();
    match lineup.mode {
        Mode::Skirmish => teams::matchup(&seated),
        Mode::Survival => {
            let zone = lineup
                .theatre(catalog)
                .and_then(|t| {
                    let first = lineup.roster.seats.first()?;
                    Some(t.layout.spawns.get(first.start as usize)?.name.clone())
                })
                .unwrap_or_default();
            format!(
                "{} Defender{}  \u{b7}  {}  \u{b7}  from {zone}",
                seated.len(),
                if seated.len() == 1 { "" } else { "s" },
                super::survival::rules::rounds_label(&lineup.rules)
            )
        }
    }
}
