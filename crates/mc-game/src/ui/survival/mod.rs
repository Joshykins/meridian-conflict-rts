//! Survival set-up: pick a theatre and a landing zone, see where the
//! Replication Engine attacks from, and set how hard, how long and how high
//! in tech its rounds go. The chart in the middle is the siege at a glance:
//! the engine, every front flowing toward your zone in its domain's colour,
//! the harbor, the node sites. The forecast under the rules draws the rounds.

mod marks;
pub mod rules;
pub mod siege;

use super::faction::Pick;
use super::lineup::settings::{self, Sheet};
use super::maps::{self, Browser, BrowserAction, MapCard};
use super::race_picker::{race_cell, RacePicker};
use super::skirmish::{race_key, theatre_card};
use super::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::settings::Settings;
use crate::setup::{self, TEAM_COLORS};
use crate::survival::{front_counts, match_for, Setup, ENGINE_COLOR};
use glam::Vec2;
use mc_data::survival::{Domain, SurvivalLayout};
use mc_map::MapFile;
use mc_sim::{AiConfig, SurvivalRules};
use std::sync::Arc;

pub use marks::engine_mark;
use marks::{clip, domain_index, label_row};

use super::skirmish::MatchRequest;

/// Image slot holding the theatre's chart (0 is skirmish's, 1 the menu's).
const PREVIEW_SLOT: usize = 2;
const LEFT: f32 = 64.0;

pub struct Theatre {
    pub stem: String,
    pub map: Arc<MapFile>,
    pub layout: SurvivalLayout,
    pub climate: mc_data::weather::Climate,
}

pub enum SurvivalAction {
    Back,
    Start(Box<MatchRequest>),
}

pub struct SurvivalState {
    pub maps: Vec<Theatre>,
    /// The same maps as the browser shows them.
    pub cards: Vec<MapCard>,
    pub browser: Browser,
    pub race: Pick,
    /// The race picker, opened from the Faction row.
    pub races: RacePicker,
    /// The settings sheet: theatre, callsign, fog, seed and sky.
    sheet: Sheet,
    selected: usize,
    preview_of: Option<usize>,
    /// Index into the theatre's spawns.
    pub spawn: usize,
    pub rules: SurvivalRules,
    pub fog: bool,
    pub sky: mc_data::weather::SkyChoice,
    seed: u64,
    pub name: String,
    /// A fronts chip under the pointer: its lanes light up on the chart.
    hover_domain: Option<Domain>,
    /// Where the spawn markers were drawn last frame, in canvas points (tests click them).
    pub markers: Vec<Vec2>,
}

fn fresh_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    let mut z = nanos.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) & 0xFFFF_FFFF
}

/// Every map in `maps/` with a usable survival layout, smallest first, and
/// the same maps as the browser shows them.
pub fn load_theatres() -> (Vec<Theatre>, Vec<MapCard>) {
    // Only maps whose settings file has a survival block are opened.
    let mut found: Vec<(Theatre, MapCard)> = setup::list_maps()
        .into_iter()
        .filter_map(|path| {
            let config = mc_data::weather::MapConfig::for_map(&path).ok()?;
            config.survival.as_ref()?;
            let card = maps::open_card(&path, &config)?;
            let layout = crate::survival::layout_in(&card.map, config)?;
            Some((
                Theatre {
                    stem: card.stem.clone(),
                    map: card.map.clone(),
                    layout,
                    climate: card.climate,
                },
                card,
            ))
        })
        .collect();
    found.sort_by_key(|(m, _)| (m.map.info().tile_count(), m.stem.clone()));
    found.into_iter().unzip()
}

impl SurvivalState {
    /// Opens every map in `maps/` that has a survival layout.
    pub fn new(settings: &Settings) -> SurvivalState {
        let (maps, cards) = load_theatres();
        let selected = maps
            .iter()
            .position(|m| m.stem == settings.survival_map)
            .unwrap_or(0);
        let browser = Browser::new(&cards);
        let mut state = SurvivalState {
            maps,
            cards,
            browser,
            race: Pick::default(),
            races: RacePicker::default(),
            sheet: Sheet::default(),
            selected,
            preview_of: None,
            spawn: settings.survival_spawn,
            rules: settings.survival_rules,
            fog: settings.survival_fog,
            sky: settings.survival_sky,
            seed: fresh_seed(),
            name: settings.player_name.clone(),
            hover_domain: None,
            markers: Vec::new(),
        };
        state.settle_rules();
        state
    }

    fn theatre(&self) -> Option<&Theatre> {
        self.maps.get(self.selected)
    }

    pub fn selected_stem(&self) -> &str {
        self.theatre().map_or("", |t| t.stem.as_str())
    }

    /// Lanes of each domain on the selected theatre.
    fn counts(&self) -> [usize; 3] {
        self.theatre().map_or([0; 3], |t| front_counts(&t.layout))
    }

    /// Keeps the rules and spawn possible on the selected theatre: a spawn it
    /// has, and at least one front that attacks.
    fn settle_rules(&mut self) {
        let spawns = self.theatre().map_or(1, |t| t.layout.spawns.len().max(1));
        self.spawn = self.spawn.min(spawns - 1);
        let counts = self.counts();
        rules::settle(&mut self.rules, counts);
    }

    fn attacking(&self) -> Vec<Domain> {
        rules::attacking(&self.rules, self.counts())
    }

    #[cfg(test)]
    fn toggle_front(&mut self, d: Domain) -> bool {
        let counts = self.counts();
        rules::toggle_front(&mut self.rules, counts, d)
    }

    fn problem(&self) -> Option<&'static str> {
        if self.maps.is_empty() {
            Some("No survival maps - give a map's .ron a survival block")
        } else if self.attacking().is_empty() {
            Some("No front attacks")
        } else {
            None
        }
    }

    pub fn request(&self) -> Option<MatchRequest> {
        let t = self.theatre()?;
        let (config, survival) = match_for(&Setup {
            layout: &t.layout,
            rules: self.rules,
            spawn: self.spawn,
            name: self.name.clone(),
            seed: self.seed,
            fog: self.fog,
            observe: false,
            ai: AiConfig::default(),
            faction: race_key(self.race.resolve(self.seed, 0)),
        });
        let mut colors = TEAM_COLORS;
        colors[1] = ENGINE_COLOR;
        Some(MatchRequest {
            map: t.map.clone(),
            config,
            colors,
            survival: Some(survival),
        })
    }

    /// Writes what was set up back to the settings; true when anything changed.
    /// The callsign is taken only while it is not being typed.
    pub fn store(&mut self, settings: &mut Settings, typing: bool) -> bool {
        let name = crate::settings::clean_name(&self.name);
        let before = (
            settings.survival_map.clone(),
            settings.survival_rules,
            settings.survival_spawn,
            settings.survival_fog,
            settings.survival_sky,
            settings.player_name.clone(),
        );
        settings.survival_map = self.selected_stem().to_owned();
        settings.survival_rules = self.rules;
        settings.survival_spawn = self.spawn;
        settings.survival_fog = self.fog;
        settings.survival_sky = self.sky;
        if !typing && settings.player_name != name {
            self.name = name.clone();
            settings.player_name = name;
        }
        before
            != (
                settings.survival_map.clone(),
                settings.survival_rules,
                settings.survival_spawn,
                settings.survival_fog,
                settings.survival_sky,
                settings.player_name.clone(),
            )
    }
}

fn arrive(enter: f32, delay: f32) -> f32 {
    let k = ((enter - delay) / (1.0 - delay)).clamp(0.0, 1.0);
    1.0 - (1.0 - k) * (1.0 - k)
}

// -- the screen ------------------------------------------------------------------

pub fn draw(ui: &mut Ui, state: &mut SurvivalState, enter: f32) -> Option<SurvivalAction> {
    state.browser.pump(ui);
    // While the map browser is open, nothing under it takes the pointer.
    let interactive = ui.interactive;
    let over = state.browser.is_open() || state.races.is_open();
    let browsing = over || state.sheet.is_open();
    ui.interactive = interactive && !browsing;
    let action = screen(ui, state, enter);
    ui.interactive = interactive;
    let mut sheet = std::mem::take(&mut state.sheet);
    sheet.draw(ui, "Match Settings", settings::SHEET, !over, |ui, body| {
        let (left, right) = settings::sheet_columns(body);
        theatres(ui, state, left);
        match_rows(ui, state, right);
    });
    state.sheet = sheet;
    let (fade, shift) = (ui.fade, ui.shift);
    ui.fade = enter;
    ui.shift = Vec2::ZERO;
    if let Some(chosen) = state.races.draw(ui) {
        state.race = chosen.pick;
    }
    if let Some(BrowserAction::Pick(i)) =
        state
            .browser
            .draw(ui, &state.cards, "Choose a Survival Map", PREVIEW_SLOT)
    {
        if i != state.selected {
            state.selected = i;
            state.settle_rules();
        }
    }
    // The browser lent our chart's slot to its detail pane: draw ours again.
    if state.browser.release_slot() {
        state.preview_of = None;
    }
    ui.fade = fade;
    ui.shift = shift;
    if browsing {
        None
    } else {
        action
    }
}

fn screen(ui: &mut Ui, state: &mut SurvivalState, enter: f32) -> Option<SurvivalAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.66 * enter));
    ui.scrim(Rect::new(0.0, 0.0, w, 220.0), 0.6 * enter, 0.0, false);
    ui.fade = enter;
    ui.shift = Vec2::new(0.0, 14.0 * (1.0 - enter));

    // Header: the engine's mark where skirmish has the emblem.
    engine_mark(ui, Vec2::new(LEFT + 15.0, 84.0), 12.0, 1.0, false);
    let end = ui.text(
        LEFT + 50.0,
        84.0,
        type_scale::TITLE,
        rgb(0xFFFFFF, 1.0),
        "Survival",
    );
    ui.text(
        end + 18.0,
        90.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        "Hold Out Against the Progenitor",
    );
    ui.fill(Rect::new(LEFT, 124.0, 58.0, 2.0), rgb(palette::ACCENT, 1.0));
    ui.gradient_h(
        Rect::new(LEFT + 66.0, 124.0, w - 2.0 * LEFT - 66.0, 1.0),
        rgb(palette::LINE, 0.35),
        rgb(palette::LINE, 0.04),
    );

    let (top, bottom) = (160.0, h - 172.0);
    let (left_w, right_w, gap) = (340.0, 540.0, 50.0);
    let left = Rect::new(LEFT, top, left_w, bottom - top);
    let right = Rect::new(w - LEFT - right_w, top, right_w, bottom - top);
    let centre = Rect::new(
        left.right() + gap,
        top,
        right.x - gap - left.right() - gap,
        bottom - top,
    );

    // Left column slides in from the left, the rules from the right, the chart rises.
    let k = arrive(enter, 0.0);
    ui.fade = k;
    ui.shift = Vec2::new(-24.0 * (1.0 - k), 0.0);
    ui.panel(Rect::new(
        left.x - 22.0,
        top - 20.0,
        left_w + 44.0,
        bottom - top + 40.0,
    ));
    let end = zones(
        ui,
        state,
        Rect::new(left.x, top, left_w, left.h - COMMANDER_H - 12.0),
    );
    commander(
        ui,
        state,
        Rect::new(left.x, end + 12.0, left_w, COMMANDER_H),
    );

    let k = arrive(enter, 0.12);
    ui.fade = k;
    ui.shift = Vec2::new(24.0 * (1.0 - k), 0.0);
    ui.panel(Rect::new(
        right.x - 22.0,
        top - 20.0,
        right_w + 44.0,
        bottom - top + 40.0,
    ));
    let counts = state.counts();
    rules::engagement(ui, &mut state.rules, counts, &mut state.hover_domain, right);

    let k = arrive(enter, 0.06);
    ui.fade = k;
    ui.shift = Vec2::new(0.0, 18.0 * (1.0 - k));
    let (bar, centre) = settings::over_chart(centre, 84.0);
    let chips = [
        settings::fog_chip(state.fog),
        settings::seed_chip(state.seed),
        settings::sky_chip(&state.sky),
    ];
    match settings::bar(
        ui,
        bar,
        &mut state.browser,
        &state.cards,
        state.selected,
        &chips,
        true,
    ) {
        Some(settings::BarAsk::ChangeMap) => state.browser.open(state.selected),
        Some(settings::BarAsk::Settings) => state.sheet.open(),
        None => {}
    }
    if let Some(t) = state.maps.get(state.selected) {
        let holders = [siege::Holder {
            spawn: state.spawn,
            color: TEAM_COLORS[0],
            you: true,
            name: state.name.clone(),
        }];
        let view = siege::Siege {
            theatre: t,
            key: state.selected,
            rules: &state.rules,
            hover_domain: state.hover_domain,
            holders: &holders,
            can_pick: true,
            slot: PREVIEW_SLOT,
        };
        if let Some(i) = siege::chart(ui, &view, &mut state.preview_of, &mut state.markers, centre)
        {
            state.spawn = i;
        }
    }

    // Footer.
    ui.fade = enter;
    ui.shift = Vec2::new(0.0, 14.0 * (1.0 - enter));
    let problem = state.problem();
    let back = ui.button(
        id("survival-back", 0),
        Rect::new(LEFT, h - 64.0 - 52.0, 200.0, 52.0),
        "Back",
        ButtonKind::Secondary,
        true,
    );
    let start_rect = Rect::new(w - LEFT - 340.0, h - 64.0 - 58.0, 340.0, 58.0);
    let start = ui.button(
        id("survival-start", 0),
        start_rect,
        "Begin Survival",
        ButtonKind::Primary,
        problem.is_none(),
    );
    match problem {
        Some(text) => ui.text_right(
            start_rect.x - 24.0,
            start_rect.mid_y(),
            type_scale::CAPTION,
            rgb(palette::WARN, 1.0),
            text,
        ),
        None => {
            let t = state.theatre().expect("no problem means a theatre");
            let zone = t
                .layout
                .spawns
                .get(state.spawn)
                .map_or("", |s| s.name.as_str());
            let lanes: usize = state
                .attacking()
                .iter()
                .map(|d| state.counts()[domain_index(*d)])
                .sum();
            let rounds = rules::rounds_label(&state.rules);
            ui.text_right(
                start_rect.x - 24.0,
                start_rect.mid_y(),
                type_scale::CAPTION,
                rgb(palette::DIM, 1.0),
                &format!("Deploying at {zone}  \u{b7}  {lanes} Fronts  \u{b7}  {rounds}"),
            );
        }
    }

    let typing = ui.mem.editing.is_some();
    let listing = ui.mem.popup.is_some();
    let mut action = None;
    if back || (ui.input.key(Key::Escape) && !typing && !listing && ui.interactive) {
        ui.audio.play(Sfx::Back);
        action = Some(SurvivalAction::Back);
    } else if (start || (ui.input.key(Key::Enter) && !typing && !listing && ui.interactive))
        && problem.is_none()
    {
        if let Some(request) = state.request() {
            ui.audio.play(Sfx::Launch);
            action = Some(SurvivalAction::Start(Box::new(request)));
        }
    }
    ui.fade = 1.0;
    ui.shift = Vec2::ZERO;
    action
}

// -- left: landing zones and faction; the sheet: theatre and match rows ---------

const RULE_PITCH: f32 = 46.0;
/// The commander block under the landing zones: a heading and the Faction row.
const COMMANDER_H: f32 = 26.0 + RULE_PITCH;

fn theatres(ui: &mut Ui, state: &mut SurvivalState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Theatre");
    if state.maps.is_empty() {
        ui.text(
            area.x,
            area.y + 50.0,
            type_scale::BODY,
            rgb(palette::WARN, 1.0),
            "No survival maps in maps/",
        );
        return;
    }
    let card = Rect::new(
        area.x,
        area.y + 28.0,
        area.w,
        (area.h - 48.0).clamp(super::lineup::THEATRE_CARD_H, 250.0),
    );
    if theatre_card(ui, &mut state.browser, &state.cards, state.selected, card) {
        state.browser.open(state.selected);
    }
}

/// The theatre's landing zones as rows: the key that picks it, its name, what
/// holding it is like. Returns the y under the last row.
fn zones(ui: &mut Ui, state: &mut SurvivalState, area: Rect) -> f32 {
    ui.section(area.x, area.y + 6.0, area.w, "Landing Zones");
    let Some(t) = state.theatre() else {
        return area.y + 26.0;
    };
    let spawns = t.layout.spawns.clone();
    let mine = TEAM_COLORS[0];
    let mut pick = None;
    let mut y = area.y + 26.0;
    for (i, s) in spawns.iter().enumerate() {
        let mut lines = ui.wrap(type_scale::MICRO, &s.blurb, area.w - 58.0);
        if lines.len() > 2 {
            let rest = lines[1..].join(" ");
            lines.truncate(1);
            lines.push(clip(ui, type_scale::MICRO, &rest, area.w - 58.0));
        }
        let n = lines.len().min(2);
        let h = 34.0 + n as f32 * 14.0;
        if y + h > area.bottom() {
            break;
        }
        let row = Rect::new(area.x, y, area.w, h);
        let res = ui.interact(id("sv-zone-row", i), row, true);
        let chosen = i == state.spawn;
        if res.clicked && !chosen {
            pick = Some(i);
        }
        let lit = ui.ease(
            id("sv-zone-row-lit", i),
            if chosen { 1.0 } else { 0.0 },
            12.0,
        );
        let g = lit.max(res.glow * 0.6);
        let c = |a: f32| [mine[0], mine[1], mine[2], a];
        ui.fill(row, ink(0.45));
        ui.gradient_h(row, c(0.16 * g), c(0.0));
        ui.frame(
            row,
            if chosen {
                c(0.55)
            } else {
                rgb(palette::LINE, 0.12 + 0.25 * g)
            },
        );
        ui.fill(Rect::new(row.x, row.y, 3.0, row.h), c(lit));
        ui.key_cap(row.x + 14.0, row.y + 10.0, &(i + 1).to_string(), chosen);
        ui.text(
            row.x + 42.0 + 3.0 * g,
            row.y + 17.0,
            type_scale::VALUE,
            if chosen {
                c(1.0)
            } else {
                rgb(palette::TEXT, 0.85 + 0.15 * g)
            },
            &s.name,
        );
        if chosen {
            ui.text_right(
                row.right() - 12.0,
                row.y + 17.0,
                type_scale::MICRO,
                c(0.9),
                "Your Zone",
            );
        }
        for (k, l) in lines.iter().take(n).enumerate() {
            ui.text(
                row.x + 42.0 + 3.0 * g,
                row.y + 35.0 + k as f32 * 14.0,
                type_scale::MICRO,
                rgb(palette::DIM, 0.85 + 0.15 * g),
                l,
            );
        }
        y += h + 6.0;
    }
    if let Some(i) = pick {
        state.spawn = i;
        ui.audio.play(Sfx::Select);
    }
    y
}

fn commander(ui: &mut Ui, state: &mut SurvivalState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Commander");
    let r = Rect::new(area.x, area.y + 26.0, area.w, RULE_PITCH - 4.0);
    let about = match state.race.race() {
        Some(race) => race
            .borrowed_roster()
            .map(|roster| format!("{roster} units"))
            .unwrap_or_default(),
        None => "Drawn at launch".to_owned(),
    };
    label_row(ui, r, "Faction", &about);
    if race_cell(
        ui,
        id("survival-race", 0),
        Rect::new(r.right() - 130.0, r.mid_y() - 16.0, 130.0, 32.0),
        state.race,
        true,
    ) {
        let who = state.name.clone();
        state.races.open(0, &who, state.race);
    }
}

/// The sheet's rows: callsign, fog and seed under Rules, then the sky.
fn match_rows(ui: &mut Ui, state: &mut SurvivalState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Rules");
    let row = |k: f32| {
        Rect::new(
            area.x,
            area.y + 26.0 + k * RULE_PITCH,
            area.w,
            RULE_PITCH - 4.0,
        )
    };
    let r = row(0.0);
    label_row(ui, r, "Callsign", "");
    ui.text_field(
        id("survival-name", 0),
        Rect::new(r.right() - 220.0, r.mid_y() - 17.0, 220.0, 34.0),
        &mut state.name,
        16,
    );
    ui.toggle(
        id("survival-fog", 0),
        row(1.0),
        "Fog of War",
        "",
        &mut state.fog,
    );
    let r = row(2.0);
    label_row(ui, r, "Seed", "");
    let reroll = Rect::new(r.right() - 96.0, r.mid_y() - 16.0, 96.0, 32.0);
    if ui.button(
        id("survival-seed", 0),
        reroll,
        "Roll",
        ButtonKind::Secondary,
        true,
    ) {
        state.seed = fresh_seed();
        ui.audio.play(Sfx::Tick);
    }
    ui.text_right(
        reroll.x - 14.0,
        r.mid_y(),
        type_scale::VALUE,
        rgb(palette::TEXT, 1.0),
        &format!(
            "{:04X}-{:04X}",
            state.seed >> 16 & 0xFFFF,
            state.seed & 0xFFFF
        ),
    );
    let y = area.y + 26.0 + 3.0 * RULE_PITCH + 20.0;
    ui.section(area.x, y, area.w, "Sky");
    let look = super::sky::Look {
        row_h: RULE_PITCH - 4.0,
        pitch: RULE_PITCH,
        value_w: 220.0,
        compact: false,
    };
    super::sky::rows(ui, 3, area.x, y + 20.0, area.w, look, &mut state.sky);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Input, Memory};
    use mc_render::Overlay;

    fn state() -> SurvivalState {
        let state = SurvivalState::new(&Settings::default());
        assert!(
            !state.maps.is_empty(),
            "bake the survival map (threshold) so survival set-up can be tested"
        );
        state
    }

    fn frame(
        state: &mut SurvivalState,
        overlay: &mut Overlay,
        memory: &mut Memory,
        input: &Input,
    ) -> Option<SurvivalAction> {
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

    #[test]
    fn clicking_a_zone_marker_deploys_there() {
        let mut state = state();
        let (mut overlay, mut memory) = (Overlay::default(), Memory::default());
        state.spawn = 0;
        frame(&mut state, &mut overlay, &mut memory, &Input::default());
        assert!(state.markers.len() >= 2, "the theatre needs two zones");
        let at = state.markers[1];
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
            frame(&mut state, &mut overlay, &mut memory, &input);
        }
        assert_eq!(state.spawn, 1);
        let request = state.request().expect("a request");
        let spawn_start = state.maps[state.selected].layout.spawns[1].start;
        assert_eq!(request.config.players[0].start, spawn_start);
        assert!(request.survival.is_some());
        assert_eq!(request.colors[1], ENGINE_COLOR);
    }

    #[test]
    fn the_last_attacking_front_cannot_be_turned_off() {
        let mut state = state();
        let counts = state.counts();
        let present: Vec<Domain> = Domain::ALL
            .into_iter()
            .filter(|d| counts[domain_index(*d)] > 0)
            .collect();
        for d in &present[..present.len() - 1] {
            if state.rules.has(*d) {
                assert!(state.toggle_front(*d));
            }
        }
        let last = *present.last().unwrap();
        assert_eq!(state.attacking(), vec![last]);
        assert!(
            !state.toggle_front(last),
            "turning off the last front is refused"
        );
        assert_eq!(state.attacking(), vec![last]);
        assert!(state.problem().is_none());
    }
}
