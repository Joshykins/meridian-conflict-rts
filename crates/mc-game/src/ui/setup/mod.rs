//! Setting up a match: skirmish or survival, on one screen. The line-up
//! (`super::lineup`) is the screen, and the multiplayer lobby draws the same
//! one; the mode is switched on the settings sheet. What is this machine's
//! own: your callsign, the sky, watching an all-AI match instead of playing,
//! and Open to Others, which takes the plan as it stands to a lobby friends
//! join (`multiplayer::share`).

use super::lineup::chat::{self, Chat, Facts, Input, Reply};
use super::lineup::roster::Control;
use super::lineup::{self, Ask, Catalog, Lineup, Mode, Table, RULE_PITCH};
use super::multiplayer::share::{self, Hosted, Share, ShareAsk};
use super::{id, palette, teams, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::settings::Settings;
use mc_data::weather::{MapConfig, SkyChoice};
use mc_map::MapFile;
use mc_sim::tables::Controller;
use mc_sim::MatchConfig;
use std::sync::Arc;

/// Image slot holding the selected map's chart.
const PREVIEW_SLOT: usize = 0;

/// Everything needed to start the match the player configured.
pub struct MatchRequest {
    pub map: Arc<MapFile>,
    pub config: MatchConfig,
    /// Player colours by player index, linear RGB.
    pub colors: crate::setup::Palette,
    /// Set for a survival match: the engine, its fronts and the rules.
    pub survival: Option<mc_sim::SurvivalConfig>,
    /// The weather and time of day the match is shown under.
    pub sky: SkyChoice,
}

pub enum SetupAction {
    Back,
    Start(Box<MatchRequest>),
    /// The set-up was opened to others: its lobby.
    Host(Box<Hosted>),
}

pub struct SetupState {
    pub catalog: Catalog,
    pub lineup: Lineup,
    /// This machine watches: every open seat, yours too, is an AI commander.
    observe: bool,
    /// Weather and time of day as last set in each mode (the line-up holds the
    /// shown mode's); left alone, the map's own.
    skies: [SkyChoice; 2],
    /// Fog of war as last set in each mode.
    fogs: [bool; 2],
    /// The mode the skies and fogs were last swapped for.
    shown: Mode,
    pub name: String,
    /// Open to Others, and the server it signs in to.
    share: Share,
    server: String,
    /// This build's unit data, which a network match must share.
    blueprint_hash: u64,
    /// What changed, as it changed; it goes with the plan to a lobby.
    pub chat: Chat,
}

fn slot(mode: Mode) -> usize {
    match mode {
        Mode::Skirmish => 0,
        Mode::Survival => 1,
    }
}

impl SetupState {
    /// Opens every map in `maps/` (skirmish maps and survival theatres) on
    /// the map picked last time, in `mode`.
    pub fn new(settings: &Settings, mode: Mode, blueprint_hash: u64) -> SetupState {
        Self::with_catalog(Catalog::load(true), settings, mode, blueprint_hash)
    }

    /// [`Self::new`] over maps already read.
    pub fn with_catalog(
        catalog: Catalog,
        settings: &Settings,
        mode: Mode,
        blueprint_hash: u64,
    ) -> SetupState {
        let find = |cards: &[super::maps::MapCard], stem: &str| {
            cards.iter().position(|m| m.stem == stem).unwrap_or(0)
        };
        let last = [
            find(&catalog.maps, &settings.skirmish_map),
            find(&catalog.theatre_cards, &settings.survival_map),
        ];
        // Skirmish: you and one AI opponent. Survival: you alone. Other zones closed.
        let (people, ai) = match mode {
            Mode::Skirmish => (1, 1),
            Mode::Survival => (1, 0),
        };
        let mode = if catalog.cards(mode).is_empty() {
            Mode::Skirmish
        } else {
            mode
        };
        let mut lineup = Lineup::new(&catalog, mode, last[slot(mode)], people, ai);
        lineup.last_map = last;
        // Rules, fog and sky start on the defaults every time; only the map is remembered.
        lineup.rules = crate::survival::env_rules();
        let fogs = [true; 2];
        lineup.fog = fogs[slot(mode)];
        let skies = [SkyChoice::default(); 2];
        lineup.sky = skies[slot(mode)];
        let mut state = SetupState {
            catalog,
            lineup,
            observe: false,
            skies,
            fogs,
            shown: mode,
            name: settings.player_name.clone(),
            share: Share::default(),
            server: settings.server.clone(),
            blueprint_hash,
            chat: Chat::default(),
        };
        state.settle_mode();
        state
    }

    /// Something else drew in the chart's image slot: draw the chart again.
    pub fn chart_lost(&mut self) {
        self.lineup.chart_lost();
    }

    /// Shows `mode`, on the map last picked in it. A mode with no maps is refused.
    pub fn set_mode(&mut self, mode: Mode) {
        if let Err(e) = self.lineup.set_mode(&self.catalog, mode, |_| false) {
            log::warn!("{e}");
        }
        self.settle_mode();
    }

    /// Opens the Open to Others sheet (the multiplayer browser's Host Game).
    pub fn open_share(&mut self) {
        self.share.open(&self.server, &self.name);
    }

    /// The mode changed on the sheet: each mode keeps its own fog and sky.
    fn settle_mode(&mut self) {
        let mode = self.lineup.mode;
        if mode != self.shown {
            self.fogs[slot(self.shown)] = self.lineup.fog;
            self.lineup.fog = self.fogs[slot(mode)];
            self.skies[slot(self.shown)] = self.lineup.sky;
            self.lineup.sky = self.skies[slot(mode)];
            self.shown = mode;
        }
    }

    pub fn mode(&self) -> Mode {
        self.lineup.mode
    }

    pub fn sky(&self) -> SkyChoice {
        self.lineup.sky
    }

    /// The chosen map's own settings: the regions the sky's rows pick a weather for.
    fn map_config(&self) -> Arc<MapConfig> {
        self.lineup
            .card(&self.catalog)
            .map(|m| m.config.clone())
            .unwrap_or_default()
    }

    pub fn observing(&self) -> bool {
        self.observe
    }

    /// Every seat filled by an AI and split into `groups` sides (headless shots).
    pub fn seat_teams_for_shot(&mut self, groups: usize) {
        let zones = self.lineup.zones(&self.catalog);
        for i in 1..self.lineup.roster.seats.len() {
            let _ = self
                .lineup
                .roster
                .set_control(i, Control::Ai, zones, |_| false);
        }
        let at = self.lineup.zone_points(&self.catalog);
        self.lineup.roster.teams_by_ground(&at, groups, 0);
    }

    /// Writes the maps picked and the callsign back to the settings, so the
    /// screen opens on them next time; true when anything changed. The match's
    /// own choices (rules, fog, sky, landing zone) are not kept. The callsign
    /// is taken only while it is not being typed.
    pub fn store(&mut self, settings: &mut Settings, typing: bool) -> bool {
        let before = settings.clone();
        let stem = |cards: &[super::maps::MapCard], i: usize| cards.get(i).map(|m| m.stem.clone());
        if let Some(s) = stem(&self.catalog.maps, self.lineup.last_map[0]) {
            settings.skirmish_map = s;
        }
        if let Some(s) = stem(&self.catalog.theatre_cards, self.lineup.last_map[1]) {
            settings.survival_map = s;
        }
        let name = crate::settings::clean_name(&self.name);
        if !typing && settings.player_name != name {
            self.name = name.clone();
            settings.player_name = name;
        }
        *settings != before
    }

    fn problem(&self) -> Option<&'static str> {
        self.lineup.problem(&self.catalog)
    }

    fn request(&self) -> MatchRequest {
        let name = self.name.clone();
        let observe = self.observe;
        let options = self
            .lineup
            .options(&self.catalog, |_| {
                if observe {
                    ("ARC AI".to_owned(), Controller::Ai)
                } else {
                    (name.clone(), Controller::Human)
                }
            })
            .expect("a request needs a map");
        let map = self
            .lineup
            .card(&self.catalog)
            .map(|m| m.map.clone())
            .expect("the options named a map");
        MatchRequest {
            map,
            config: options.config,
            colors: options.colors,
            survival: options.survival,
            sky: options.sky,
        }
    }

    fn content(&self) -> mc_net::ContentId {
        mc_net::ContentId {
            map_id: self
                .lineup
                .card(&self.catalog)
                .map_or(0, |m| m.map.content_id()),
            blueprint_hash: self.blueprint_hash,
        }
    }

    /// The sheet asked for a lobby: the plan as it stands goes to it.
    fn host(&mut self, ask: ShareAsk) -> Option<Box<Hosted>> {
        let plan = self.lineup.plan();
        let content = self.content();
        let lobby = match ask {
            ShareAsk::HereNow => {
                let title = self.share.title();
                share::host_here(&self.name, &title, &self.catalog, plan, content)
                    .map_err(|e| format!("Could not host here: {e}"))
            }
            ShareAsk::Created { code } => {
                share::lobby_on_server(&mut self.share, code, plan, content)
            }
        };
        match lobby {
            Ok(mut lobby) => {
                lobby.adopt_chat(std::mem::take(&mut self.chat));
                self.share.hosted(lobby)
            }
            Err(e) => {
                self.share.say(e);
                None
            }
        }
    }
}

/// One machine's table: you plan everything and command seat 0, unless you watch.
fn table_of(observe: bool, name: &str) -> Table<'_> {
    Table {
        host: true,
        me: (!observe).then_some(0),
        lobby: false,
        people: &[],
        observe,
        name,
    }
}

/// The skirmish the set-up screen would start if opened and confirmed untouched.
pub fn default_request(settings: &Settings) -> Option<MatchRequest> {
    let state = SetupState::new(settings, Mode::Skirmish, 0);
    state.problem().is_none().then(|| state.request())
}

pub fn draw(ui: &mut Ui, state: &mut SetupState, enter: f32) -> Option<SetupAction> {
    state.catalog.pump(ui, state.lineup.mode);
    // While the map browser, race picker or a sheet is open, nothing under it takes the pointer.
    let interactive = ui.interactive;
    let over = state.catalog.browsing() || state.lineup.races.is_open();
    let browsing = over || state.lineup.sheet.is_open() || state.share.is_open();
    ui.interactive = interactive && !browsing;
    let action = screen(ui, state, enter);
    ui.interactive = interactive;
    // The table reads the callsign as it was when the sheet opened this frame.
    let callsign = state.name.clone();
    let table = table_of(state.observe, &callsign);
    let name = &mut state.name;
    let mut asks: Vec<Ask> = lineup::sheet(
        ui,
        &mut state.lineup,
        &mut state.catalog,
        &table,
        !over,
        |ui, area, y| callsign_row(ui, name, area, y),
    )
    .into_iter()
    .collect();
    state.settle_mode();
    let content = state.content();
    let hosted = state
        .share
        .draw(ui, &state.lineup, &state.catalog, content, &callsign, !over)
        .and_then(|ask| state.host(ask));
    let table = table_of(state.observe, &state.name);
    asks.extend(lineup::overlays(
        ui,
        &mut state.lineup,
        &mut state.catalog,
        &table,
        enter,
        PREVIEW_SLOT,
    ));
    let facts = Facts::of(&state.lineup, &state.catalog, &table)
        .with_sky(&state.sky(), &state.map_config());
    state.chat.watch(facts);
    refused(&mut state.chat, asks);
    if let Some(hosted) = hosted {
        return Some(SetupAction::Host(hosted));
    }
    if browsing {
        None
    } else {
        action
    }
}

fn screen(ui: &mut Ui, state: &mut SetupState, enter: f32) -> Option<SetupAction> {
    let mode = state.lineup.mode;
    // Escape puts down a commander being moved before it leaves the screen.
    let placing = state.lineup.placing();
    let defenders = state.lineup.roster.seated_teams().len();
    let caption = match mode {
        Mode::Skirmish if state.observing() => "Watch the Commanders Fight",
        Mode::Skirmish => "Configure the Engagement",
        Mode::Survival if defenders > 1 => "Hold Out Together Against the Progenitor",
        Mode::Survival => "Hold Out Against the Progenitor",
    };
    lineup::header(ui, Some(mode), caption, enter);
    let (left, centre, right) = lineup::columns(ui);
    let chips = [
        lineup::settings::fog_chip(state.lineup.fog),
        lineup::settings::seed_chip(state.lineup.seed),
        super::sky::summary(&state.sky(), &state.map_config()),
    ];
    let below = lineup::match_card(
        ui,
        &mut state.lineup,
        &mut state.catalog,
        true,
        &chips,
        left,
    );
    let log = Rect::new(left.x, below + 22.0, left.w, left.bottom() - below - 22.0);
    let hint = "Open to Others to chat with friends";
    if let Some(Reply::Open) = chat::draw(ui, &mut state.chat, log, Input::Closed(hint)) {
        ui.audio.play(Sfx::Select);
        state.open_share();
    }
    let table = table_of(state.observe, &state.name);
    lineup::chart(
        ui,
        &mut state.lineup,
        &state.catalog,
        &table,
        PREVIEW_SLOT,
        centre,
    );
    let mut observe = state.observe;
    let asks = lineup::commanders(
        ui,
        &mut state.lineup,
        &mut state.catalog,
        &table,
        Some(&mut observe),
        right,
    );
    state.observe = observe;
    refused(&mut state.chat, asks);

    // Footer.
    let problem = state.problem();
    let line = match problem {
        Some(text) => text.to_owned(),
        None if mode == Mode::Survival => lineup::summary(&state.lineup, &state.catalog),
        None => {
            let seated = state.lineup.roster.seated_teams();
            let matchup = teams::matchup(&seated);
            if state.observing() {
                format!(
                    "{matchup}  \u{b7}  {} AI Commanders  \u{b7}  You Watch",
                    seated.len()
                )
            } else {
                format!("{matchup}  \u{b7}  {} Commanders Ready", seated.len())
            }
        }
    };
    let launch = match mode {
        Mode::Skirmish if state.observing() => "Watch Match",
        Mode::Skirmish => "Start Match",
        Mode::Survival => "Begin Survival",
    };
    let clicked = lineup::footer(
        ui,
        "Back",
        (launch, ButtonKind::Primary, problem.is_none()),
        Some(("Open to Others", problem.is_none())),
        &line,
        if problem.is_some() {
            palette::WARN
        } else {
            palette::DIM
        },
        None,
    );
    let typing = ui.mem.editing.is_some();
    let mut action = None;
    let listing = ui.mem.popup.is_some();
    let escape = ui.input.key(Key::Escape) && !typing && !listing && !placing;
    if clicked.back || (escape && ui.interactive) {
        ui.audio.play(Sfx::Back);
        action = Some(SetupAction::Back);
    } else if clicked.beside {
        ui.audio.play(Sfx::Select);
        state.open_share();
    } else if (clicked.launch
        || (ui.input.key(Key::Enter) && !typing && !listing && ui.interactive))
        && problem.is_none()
    {
        ui.audio.play(Sfx::Launch);
        action = Some(SetupAction::Start(Box::new(state.request())));
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    action
}

/// On one machine the line-up asks only to say why a change was refused: the chat says it.
fn refused(chat: &mut Chat, asks: Vec<Ask>) {
    for ask in asks {
        match ask {
            Ask::Say(text) => chat.warn(text),
            other => log::warn!("a set-up on one machine was asked {other:?}"),
        }
    }
}

/// Your callsign under the shared rules; returns the y under it.
fn callsign_row(ui: &mut Ui, name: &mut String, area: Rect, y: f32) -> f32 {
    let r = Rect::new(area.x, y, area.w, RULE_PITCH - 4.0);
    lineup::rule_label(ui, r, "Callsign");
    ui.text_field(
        id("rule-name", 0),
        Rect::new(r.right() - 220.0, r.mid_y() - 17.0, 220.0, 34.0),
        name,
        16,
    );
    y + RULE_PITCH + 20.0
}

#[cfg(test)]
mod tests;
