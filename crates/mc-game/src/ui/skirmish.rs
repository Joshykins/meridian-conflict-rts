//! Skirmish set-up: pick a map, seat the commanders, set the rules, launch.
//! The line-up (`super::lineup`) is the screen; the multiplayer lobby draws
//! the same one. What is skirmish's own: your callsign, the sky, and watching
//! an all-AI match instead of playing.

use super::lineup::roster::Control;
use super::lineup::{self, Ask, Catalog, Lineup, Mode, Table, RULE_PITCH};
use super::{id, palette, teams, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use mc_map::MapFile;
use mc_sim::tables::Controller;
use mc_sim::MatchConfig;
use std::sync::Arc;

/// Image slot holding the selected map's preview.
const PREVIEW_SLOT: usize = 0;

pub use super::faction::race_key;
pub use super::lineup::theatre_card;

/// Everything needed to start the match the player configured.
pub struct MatchRequest {
    pub map: Arc<MapFile>,
    pub config: MatchConfig,
    /// Player colours by player index, linear RGB.
    pub colors: crate::setup::Palette,
    /// Set for a survival match: the engine, its fronts and the rules.
    pub survival: Option<mc_sim::SurvivalConfig>,
}

pub enum SkirmishAction {
    Back,
    Start(Box<MatchRequest>),
}

pub struct SkirmishState {
    pub catalog: Catalog,
    pub lineup: Lineup,
    /// This machine watches: every open seat, yours too, is an AI commander.
    observe: bool,
    /// Weather and time of day for the match; left alone, the map's own.
    pub sky: mc_data::weather::SkyChoice,
    pub name: String,
}

impl SkirmishState {
    /// Opens every map in `maps/` but those made for survival (they have
    /// their own screen). `preferred` is the stem of the map to select.
    pub fn new(preferred: &str, fog: bool, name: &str) -> SkirmishState {
        Self::with_catalog(Catalog::load(false), preferred, fog, name)
    }

    fn with_catalog(catalog: Catalog, preferred: &str, fog: bool, name: &str) -> SkirmishState {
        let selected = catalog
            .maps
            .iter()
            .position(|m| m.stem == preferred)
            .unwrap_or(0);
        // You and one AI opponent; the map's other zones closed.
        let mut lineup = Lineup::new(&catalog, Mode::Skirmish, selected, 1, 1);
        lineup.fog = fog;
        SkirmishState {
            catalog,
            lineup,
            observe: false,
            sky: Default::default(),
            name: name.to_owned(),
        }
    }

    /// Something else drew in the chart's image slot: draw the chart again.
    pub fn chart_lost(&mut self) {
        self.lineup.chart_lost();
    }

    pub fn selected_stem(&self) -> &str {
        self.lineup
            .card(&self.catalog)
            .map_or("", |m| m.stem.as_str())
    }

    pub fn fog(&self) -> bool {
        self.lineup.fog
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
            .expect("a skirmish request needs a map");
        let map = self
            .lineup
            .card(&self.catalog)
            .map(|m| m.map.clone())
            .expect("the options named a map");
        MatchRequest {
            map,
            config: options.config,
            colors: options.colors,
            survival: None,
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

/// The match the set-up screen would start if opened and confirmed untouched.
pub fn default_request(settings: &crate::settings::Settings) -> Option<MatchRequest> {
    let state = SkirmishState::new(
        &settings.skirmish_map,
        settings.skirmish_fog,
        &settings.player_name,
    );
    state.problem().is_none().then(|| state.request())
}

pub fn draw(ui: &mut Ui, state: &mut SkirmishState, enter: f32) -> Option<SkirmishAction> {
    state.catalog.pump(ui, Mode::Skirmish);
    // While the map browser or race picker is open, nothing under it takes the pointer.
    let interactive = ui.interactive;
    let over = state.catalog.browsing() || state.lineup.races.is_open();
    let browsing = over || state.lineup.sheet.is_open();
    ui.interactive = interactive && !browsing;
    let action = screen(ui, state, enter);
    ui.interactive = interactive;
    // The table reads the callsign as it was when the sheet opened this frame.
    let callsign = state.name.clone();
    let table = table_of(state.observe, &callsign);
    let (name, sky) = (&mut state.name, &mut state.sky);
    lineup::sheet(
        ui,
        &mut state.lineup,
        &mut state.catalog,
        &table,
        !over,
        |ui, area, y| callsign_and_sky(ui, name, sky, area, y),
    );
    let table = table_of(state.observe, &state.name);
    lineup::overlays(
        ui,
        &mut state.lineup,
        &mut state.catalog,
        &table,
        enter,
        PREVIEW_SLOT,
    );
    if browsing {
        None
    } else {
        action
    }
}

fn screen(ui: &mut Ui, state: &mut SkirmishState, enter: f32) -> Option<SkirmishAction> {
    lineup::header(
        ui,
        "Skirmish",
        if state.observing() {
            "Watch the Commanders Fight"
        } else {
            "Configure the Engagement"
        },
        enter,
    );
    let (_, centre, right) = lineup::columns(ui, 0.0);
    let chips = [
        lineup::settings::fog_chip(state.lineup.fog),
        lineup::settings::seed_chip(state.lineup.seed),
        lineup::settings::sky_chip(&state.sky),
    ];
    let chart = lineup::bar(
        ui,
        &mut state.lineup,
        &mut state.catalog,
        true,
        &chips,
        centre,
    );
    let table = table_of(state.observe, &state.name);
    lineup::chart(
        ui,
        &mut state.lineup,
        &state.catalog,
        &table,
        PREVIEW_SLOT,
        chart,
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
    debug_assert!(asks.iter().all(|a| matches!(a, Ask::Say(_))));

    // Footer.
    let problem = state.problem();
    let line = match problem {
        Some(text) => text.to_owned(),
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
    let launch = if state.observing() {
        "Watch Match"
    } else {
        "Start Match"
    };
    let (back, start) = lineup::footer(
        ui,
        "Back",
        (launch, ButtonKind::Primary, problem.is_none()),
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
    if back || (ui.input.key(Key::Escape) && !typing && !listing && ui.interactive) {
        ui.audio.play(Sfx::Back);
        action = Some(SkirmishAction::Back);
    } else if (start || (ui.input.key(Key::Enter) && !typing && ui.interactive))
        && problem.is_none()
    {
        ui.audio.play(Sfx::Launch);
        action = Some(SkirmishAction::Start(Box::new(state.request())));
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    action
}

/// Your callsign under the shared rules, then the sky: the map's own weather
/// and time of day, or what is picked here.
fn callsign_and_sky(
    ui: &mut Ui,
    name: &mut String,
    sky: &mut mc_data::weather::SkyChoice,
    area: Rect,
    y: f32,
) {
    let r = Rect::new(area.x, y, area.w, RULE_PITCH - 4.0);
    lineup::rule_label(ui, r, "Callsign");
    ui.text_field(
        id("rule-name", 0),
        Rect::new(r.right() - 220.0, r.mid_y() - 17.0, 220.0, 34.0),
        name,
        16,
    );
    let y = y + RULE_PITCH + 20.0;
    ui.section(area.x, y, area.w, "Sky");
    let look = super::sky::Look {
        row_h: RULE_PITCH - 4.0,
        pitch: RULE_PITCH,
        value_w: 220.0,
        compact: false,
    };
    super::sky::rows(ui, 0, area.x, y + 20.0, area.w, look, sky);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::faction::Pick;
    use crate::ui::{Input, Memory};
    use glam::Vec2;
    use mc_render::Overlay;
    use mc_sim::{Difficulty, Doctrine};

    /// Set-up over the test maps (`test_maps`), not the checkout's `maps/`.
    fn state() -> SkirmishState {
        let catalog = Catalog::load_from(crate::ui::test_maps::paths(), false);
        SkirmishState::with_catalog(catalog, "", true, "Tester")
    }

    fn frame(
        state: &mut SkirmishState,
        overlay: &mut Overlay,
        memory: &mut Memory,
        input: &Input,
    ) -> Option<SkirmishAction> {
        let audio = crate::audio::Audio::silent();
        memory.begin_frame();
        let mut ui = Ui::new(
            overlay,
            input,
            memory,
            &audio,
            Vec2::new(1920.0, 1080.0),
            1.0,
            1.0,
            1.0 / 30.0,
        );
        let action = draw(&mut ui, state, 1.0);
        ui.popups();
        memory.end_frame(input);
        overlay.clear();
        action
    }

    fn click(state: &mut SkirmishState, overlay: &mut Overlay, memory: &mut Memory, at: Vec2) {
        for input in [
            Input {
                cursor: at,
                ..Default::default()
            },
            Input {
                cursor: at,
                down: true,
                pressed: true,
                ..Default::default()
            },
            Input {
                cursor: at,
                released: true,
                ..Default::default()
            },
        ] {
            assert!(
                frame(state, overlay, memory, &input).is_none(),
                "nothing starts while browsing"
            );
        }
    }

    #[test]
    fn the_map_browser_picks_a_map_and_arc_is_the_default_race() {
        let mut state = state();
        assert!(
            state.catalog.maps.len() >= 2,
            "bake two skirmish maps to test the browser"
        );
        assert!(crate::ui::faction::races()
            .iter()
            .any(|r| r.abbreviation == "ARC"));
        state.catalog.browser.wait_for_thumbs();
        let (mut overlay, mut memory) = (Overlay::default(), Memory::default());
        state.catalog.browser.open_now(state.lineup.map);
        frame(&mut state, &mut overlay, &mut memory, &Input::default());
        assert_eq!(
            state.catalog.browser.cards.len(),
            state.catalog.maps.len(),
            "every map shows with no filter"
        );
        let (other, at) = *state
            .catalog
            .browser
            .cards
            .iter()
            .find(|(i, _)| *i != state.lineup.map)
            .unwrap();
        click(&mut state, &mut overlay, &mut memory, at);
        assert!(
            state.catalog.browser.is_open(),
            "one click only picks the card"
        );
        let choose = state.catalog.browser.select_at;
        click(&mut state, &mut overlay, &mut memory, choose);
        assert!(!state.catalog.browser.is_open());
        assert_eq!(state.lineup.map, other);
        assert_eq!(
            state.lineup.roster.seats.len(),
            state.catalog.maps[other].starts.min(8),
            "the seats follow the new map"
        );
        assert_eq!(state.request().config.players[0].faction, "Aster");
    }

    #[test]
    fn the_race_picker_sets_a_seat_to_random_and_launch_deals_a_race() {
        let mut state = state();
        assert!(
            !state.catalog.maps.is_empty(),
            "bake a map so skirmish set-up can be tested"
        );
        let (mut overlay, mut memory) = (Overlay::default(), Memory::default());
        let seat = state.lineup.roster.seats[1];
        state
            .lineup
            .races
            .open_now(seat.key as usize, "ARC AI 1", seat.race);
        frame(&mut state, &mut overlay, &mut memory, &Input::default());
        let cards = state.lineup.races.cards.clone();
        assert_eq!(
            cards.len(),
            crate::ui::faction::races().len() + 1,
            "every race and Random"
        );
        let (pick, at) = *cards.last().unwrap();
        assert_eq!(pick, Pick::Random);
        click(&mut state, &mut overlay, &mut memory, at);
        assert!(
            state.lineup.races.is_open(),
            "one click only picks the card"
        );
        let choose = state.lineup.races.choose_at;
        click(&mut state, &mut overlay, &mut memory, choose);
        assert!(!state.lineup.races.is_open());
        assert_eq!(state.lineup.roster.seats[1].race, Pick::Random);
        let table = table_of(false, "Tester");
        assert!(state.lineup.ai_name(&table, 1).starts_with("Random AI"));
        let request = state.request();
        let dealt = &request.config.players[1];
        assert!(
            crate::ui::faction::race_by_key(&dealt.faction).is_some(),
            "a real race is dealt: {}",
            dealt.faction
        );
        assert_eq!(
            request.config.players[1].faction,
            state.request().config.players[1].faction,
            "the seed deals it the same way twice"
        );
    }

    #[test]
    fn two_sides_splits_a_full_map_evenly_with_you_on_team_one() {
        let mut state = state();
        let Some(big) = state.catalog.maps.iter().position(|m| m.starts == 8) else {
            return;
        };
        state
            .lineup
            .set_map(&state.catalog, big, |_| false)
            .unwrap();
        state.seat_teams_for_shot(2);
        let seated = state.lineup.roster.seated_teams();
        assert_eq!(teams::matchup(&seated), "4 v 4");
        assert_eq!(state.lineup.roster.seats[0].team, 0);
        assert!(state.lineup.roster.allied());
        let request = state.request();
        assert_eq!(
            request
                .config
                .players
                .iter()
                .filter(|p| p.team == 0)
                .count(),
            4
        );
    }

    #[test]
    fn observing_your_slot_starts_an_all_ai_match() {
        let mut state = state();
        assert!(
            !state.catalog.maps.is_empty(),
            "bake a map so skirmish set-up can be tested"
        );
        assert!(!state.observing());
        assert!(state.problem().is_none());
        let play = state.request();
        assert!(play
            .config
            .players
            .iter()
            .any(|p| p.controller == Controller::Human));

        let ai = &mut state.lineup.roster.seats[1].ai;
        ai.difficulty = Difficulty::Hard;
        ai.doctrine = Doctrine::Defensive;
        ai.domain_weights = [50, 150, 75];
        let preserved = *ai;
        let configured = state.request();
        assert_eq!(configured.config.players[1].ai, preserved);
        // Another map keeps the seats' AI set-up.
        let other = (state.lineup.map + 1) % state.catalog.maps.len();
        state
            .lineup
            .set_map(&state.catalog, other, |_| false)
            .unwrap();
        assert_eq!(state.lineup.roster.seats[1].ai, preserved);
        state.observe = true;
        assert!(state.observing());
        assert!(state.problem().is_none());
        let watch = state.request();
        assert!(watch
            .config
            .players
            .iter()
            .all(|p| p.controller == Controller::Ai));
        assert!(watch.config.players.len() >= 2);

        state.observe = false;
        assert!(!state.observing());
        assert!(state
            .request()
            .config
            .players
            .iter()
            .any(|p| p.controller == Controller::Human));
    }
}
