//! Skirmish set-up: pick a map, seat the commanders, set the rules, launch.

use super::{id, ink, palette, preview, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::setup::{self, TEAM_COLORS};
use glam::Vec2;
use mc_map::MapFile;
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Difficulty, Doctrine, MatchConfig, PlayerSetup};
use std::sync::Arc;

/// Image slot holding the selected map's preview.
const PREVIEW_SLOT: usize = 0;
const LEFT: f32 = 64.0;

pub struct MapEntry {
    pub stem: String,
    pub map: Arc<MapFile>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Seat {
    /// Your seat; an AI commander takes it while you observe.
    You,
    Ai,
    Closed,
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    seat: Seat,
    team: u8,
    start: u8,
    color: u8,
    ai: AiConfig,
}

/// Everything needed to start the match the player configured.
pub struct MatchRequest {
    pub map: Arc<MapFile>,
    pub config: MatchConfig,
    /// Player colours by player index, linear RGB.
    pub colors: [[f32; 3]; 8],
    /// Set for a survival match: the engine, its fronts and the rules.
    pub survival: Option<mc_sim::SurvivalConfig>,
}

pub enum SkirmishAction {
    Back,
    Start(MatchRequest),
}

pub struct SkirmishState {
    pub maps: Vec<MapEntry>,
    selected: usize,
    /// The map whose preview is in the image slot.
    preview_of: Option<usize>,
    /// First map shown when the list is longer than its panel.
    list_top: usize,
    slots: Vec<Slot>,
    /// This machine watches: every open seat, yours too, is an AI commander.
    observe: bool,
    pub fog: bool,
    /// Weather and time of day for the match; left alone, the map's own.
    pub sky: mc_data::weather::SkyChoice,
    /// The AI row whose doctrine and preferences are open under it.
    tuning: Option<usize>,
    /// The row whose colour swatches are open under it.
    coloring: Option<usize>,
    seed: u64,
    pub name: String,
}

fn fresh_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    // SplitMix64: consecutive clock readings give unrelated seeds.
    let mut z = nanos.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) & 0xFFFF_FFFF
}

impl SkirmishState {
    /// Opens every map in `maps/` but those made for survival (they have
    /// their own screen). `preferred` is the stem of the map to select.
    pub fn new(preferred: &str, fog: bool, name: &str) -> SkirmishState {
        let mut maps: Vec<MapEntry> = setup::list_maps()
            .into_iter()
            .filter(|path| {
                mc_data::weather::MapConfig::for_map(path).map_or(true, |c| c.survival.is_none())
            })
            .filter_map(|path| match MapFile::open(&path) {
                Ok(map) => Some(MapEntry {
                    stem: path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    map: Arc::new(map),
                }),
                Err(e) => {
                    log::warn!("{}: {e}; not listed", path.display());
                    None
                }
            })
            .collect();
        // Smallest first: the quick 1v1 maps lead the list.
        maps.sort_by_key(|m| (m.map.info().tile_count(), m.stem.clone()));
        let selected = maps.iter().position(|m| m.stem == preferred).unwrap_or(0);
        let mut state = SkirmishState {
            maps,
            selected,
            preview_of: None,
            list_top: selected.saturating_sub(1),
            slots: Vec::new(),
            observe: false,
            fog,
            sky: Default::default(),
            tuning: None,
            coloring: None,
            seed: fresh_seed(),
            name: name.to_owned(),
        };
        state.seat_for_map();
        state
    }

    pub fn selected_stem(&self) -> &str {
        self.maps.get(self.selected).map_or("", |m| m.stem.as_str())
    }

    /// You, one AI opponent, and the rest of the map's start positions closed.
    fn seat_for_map(&mut self) {
        let starts = self
            .maps
            .get(self.selected)
            .map_or(0, |m| m.map.start_positions().len())
            .min(8);
        let previous = self.slots.clone();
        self.slots = (0..starts)
            .map(|i| Slot {
                seat: [Seat::You, Seat::Ai]
                    .get(i)
                    .copied()
                    .unwrap_or(Seat::Closed),
                team: i as u8,
                start: i as u8,
                color: i as u8,
                ai: previous.get(i).map_or_else(AiConfig::default, |s| s.ai),
            })
            .collect();
    }

    pub fn observing(&self) -> bool {
        self.observe
    }

    /// The slot's commander is an AI: an AI seat, or yours while you watch.
    fn is_ai(&self, slot: &Slot) -> bool {
        slot.seat == Seat::Ai || (slot.seat == Seat::You && self.observe)
    }

    /// "Arc AI n", numbered over the AI commanders in slot order.
    fn ai_name(&self, i: usize) -> String {
        let n = self.slots[..=i].iter().filter(|s| self.is_ai(s)).count();
        format!("Arc AI {n}")
    }

    fn taken<T: PartialEq>(&self, except: usize, field: impl Fn(&Slot) -> T, value: T) -> bool {
        self.slots
            .iter()
            .enumerate()
            .any(|(i, s)| i != except && s.seat != Seat::Closed && field(s) == value)
    }

    /// Steps slot `i`'s start or colour to the next value nobody else is using.
    fn step_unique(
        &mut self,
        i: usize,
        by: i32,
        count: usize,
        get: impl Fn(&Slot) -> u8,
        set: impl Fn(&mut Slot, u8),
    ) {
        let mut v = get(&self.slots[i]) as i32;
        for _ in 0..count {
            v = (v + by).rem_euclid(count as i32);
            if !self.taken(i, &get, v as u8) {
                set(&mut self.slots[i], v as u8);
                return;
            }
        }
    }

    fn problem(&self) -> Option<&'static str> {
        let active: Vec<&Slot> = self
            .slots
            .iter()
            .filter(|s| s.seat != Seat::Closed)
            .collect();
        if self.maps.is_empty() {
            Some("No maps found - bake one with mc-bake")
        } else if active.len() < 2 {
            Some("A match needs an opponent")
        } else if active.iter().all(|s| s.team == active[0].team) {
            Some("Everyone is on the same team")
        } else {
            None
        }
    }

    fn request(&self) -> MatchRequest {
        let entry = &self.maps[self.selected];
        let mut colors = TEAM_COLORS;
        let mut players = Vec::new();
        let mut ai = 0;
        for s in self.slots.iter().filter(|s| s.seat != Seat::Closed) {
            colors[players.len()] = TEAM_COLORS[s.color as usize];
            let (name, controller) = if self.is_ai(s) {
                ai += 1;
                (format!("Arc AI {ai}"), Controller::Ai)
            } else {
                (self.name.clone(), Controller::Human)
            };
            players.push(PlayerSetup {
                name,
                faction: "Aster".into(),
                ai: s.ai,
                team: s.team,
                controller,
                start: s.start,
            });
        }
        MatchRequest {
            map: entry.map.clone(),
            config: MatchConfig {
                seed: self.seed,
                players,
                cheats: false,
                fog: self.fog,
                spawn_commanders: true,
            },
            colors,
            survival: None,
        }
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
    let (w, h) = (ui.size.x, ui.size.y);
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.66 * enter));
    ui.scrim(Rect::new(0.0, 0.0, w, 220.0), 0.6 * enter, 0.0, false);
    ui.fade = enter;
    ui.shift.y = 14.0 * (1.0 - enter);

    // Header.
    ui.emblem(Vec2::new(LEFT + 15.0, 84.0), 13.0, rgb(palette::TEXT, 0.9));
    let end = ui.text(
        LEFT + 50.0,
        84.0,
        type_scale::TITLE,
        rgb(0xFFFFFF, 1.0),
        "Skirmish",
    );
    ui.text(
        end + 18.0,
        90.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        if state.observing() {
            "Watch the Commanders Fight"
        } else {
            "Configure the Engagement"
        },
    );
    ui.fill(Rect::new(LEFT, 124.0, 58.0, 2.0), rgb(palette::ACCENT, 1.0));
    ui.gradient_h(
        Rect::new(LEFT + 66.0, 124.0, w - 2.0 * LEFT - 66.0, 1.0),
        rgb(palette::LINE, 0.35),
        rgb(palette::LINE, 0.04),
    );

    // Columns: maps and rules | preview | commanders.
    let (top, bottom) = (160.0, h - 172.0);
    let (left_w, right_w, gap) = (372.0, 600.0, 58.0);
    let centre = Rect::new(
        LEFT + left_w + gap,
        top,
        w - 2.0 * LEFT - left_w - right_w - 2.0 * gap,
        bottom - top,
    );
    let right = Rect::new(w - LEFT - right_w, top, right_w, bottom - top);

    // The side columns sit on glass; the chart in the middle carries its own frame.
    ui.panel(Rect::new(
        LEFT - 22.0,
        top - 20.0,
        left_w + 44.0,
        bottom - top + 40.0,
    ));
    ui.panel(Rect::new(
        right.x - 22.0,
        top - 20.0,
        right_w + 44.0,
        bottom - top + 40.0,
    ));
    map_list(ui, state, Rect::new(LEFT, top, left_w, bottom - top - RULES_H));
    rules(ui, state, Rect::new(LEFT, bottom - RULES_H, left_w, RULES_H));
    map_preview(ui, state, centre);
    commanders(ui, state, right);

    // Footer.
    let problem = state.problem();
    let back = ui.button(
        id("skirmish-back", 0),
        Rect::new(LEFT, h - 64.0 - 52.0, 200.0, 52.0),
        "Back",
        ButtonKind::Secondary,
        true,
    );
    let start_rect = Rect::new(w - LEFT - 340.0, h - 64.0 - 58.0, 340.0, 58.0);
    let launch = if state.observing() {
        "Watch Match"
    } else {
        "Start Match"
    };
    let start = ui.button(
        id("skirmish-start", 0),
        start_rect,
        launch,
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
            let seated = state
                .slots
                .iter()
                .filter(|s| s.seat != Seat::Closed)
                .count();
            ui.text_right(
                start_rect.x - 24.0,
                start_rect.mid_y(),
                type_scale::CAPTION,
                rgb(palette::DIM, 1.0),
                &if state.observing() {
                    format!("{seated} AI Commanders  \u{b7}  You Watch")
                } else {
                    format!("{seated} Commanders Ready")
                },
            );
        }
    }
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
        action = Some(SkirmishAction::Start(state.request()));
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    action
}

fn map_list(ui: &mut Ui, state: &mut SkirmishState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Theatre");
    let mut pick = None;
    let fit = (((area.h - 28.0) / 74.0).floor().max(1.0) as usize).min(6);
    // More maps than rows: the wheel over the list scrolls it.
    let most = state.maps.len().saturating_sub(fit);
    let list = Rect::new(area.x, area.y + 28.0, area.w, fit as f32 * 74.0);
    if most > 0 && list.contains(ui.cursor - ui.shift) && ui.input.scroll != 0.0 && ui.interactive {
        state.list_top = if ui.input.scroll > 0.0 {
            state.list_top.saturating_sub(1)
        } else {
            state.list_top + 1
        };
    }
    state.list_top = state.list_top.min(most);
    let top = state.list_top;
    if most > 0 {
        // A thin bar at the list's edge: where the rows shown sit among them all.
        let track = Rect::new(area.x + area.w + 6.0, list.y, 3.0, list.h - 6.0);
        ui.fill(track, rgb(palette::LINE, 0.25));
        let n = state.maps.len() as f32;
        ui.fill(
            Rect::new(track.x, track.y + track.h * top as f32 / n, track.w, track.h * fit as f32 / n),
            rgb(palette::ACCENT, 0.8),
        );
    }
    for (i, entry) in state.maps.iter().enumerate().skip(top).take(fit) {
        let row = Rect::new(area.x, area.y + 28.0 + (i - top) as f32 * 74.0, area.w, 68.0);
        let res = ui.interact(id("map-row", i), row, true);
        let chosen = state.selected == i;
        if res.clicked && !chosen {
            pick = Some(i);
        }
        let lit = ui.ease(id("map-row-lit", i), if chosen { 1.0 } else { 0.0 }, 12.0);
        let g = lit.max(res.glow * 0.6);
        ui.fill(row, ink(0.5));
        ui.gradient_h(
            row,
            rgb(palette::ACCENT, 0.20 * g),
            rgb(palette::ACCENT, 0.01),
        );
        ui.frame(
            row,
            rgb(
                if chosen {
                    palette::ACCENT
                } else {
                    palette::LINE
                },
                0.14 + 0.4 * g,
            ),
        );
        ui.fill(
            Rect::new(row.x, row.y, 4.0, row.h),
            rgb(palette::ACCENT, lit),
        );
        let info = entry.map.info();
        let km = info.size_metres().to_f32()[0] / 1000.0;
        ui.text(
            row.x + 20.0 + 4.0 * g,
            row.y + 25.0,
            type_scale::ITEM,
            rgb(
                if chosen {
                    palette::ACCENT
                } else {
                    palette::TEXT
                },
                0.85 + 0.15 * g,
            ),
            &entry.map.name(),
        );
        ui.text(
            row.x + 21.0 + 4.0 * g,
            row.y + 48.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!(
                "{km:.0} km  \u{b7}  {} Players  \u{b7}  {} Ore Fields",
                entry.map.start_positions().len().min(8),
                entry.map.ore_regions().len()
            ),
        );
    }
    if let Some(i) = pick {
        state.selected = i;
        state.seat_for_map();
        ui.audio.play(Sfx::Select);
    }
    if state.maps.is_empty() {
        ui.text(
            area.x,
            area.y + 50.0,
            type_scale::BODY,
            rgb(palette::WARN, 1.0),
            "No maps in maps/",
        );
    }
}

/// Rules and sky: three rule rows, then the weather rows under their own heading.
const RULE_PITCH: f32 = 46.0;
const RULES_H: f32 = 26.0 + 3.0 * RULE_PITCH + 44.0 + super::sky::ROWS as f32 * RULE_PITCH;

fn rules(ui: &mut Ui, state: &mut SkirmishState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Rules");
    let row = |k: f32| Rect::new(area.x, area.y + 26.0 + k * RULE_PITCH, area.w, RULE_PITCH - 4.0);
    ui.toggle(
        id("rule-fog", 0),
        row(0.0),
        "Fog of War",
        "",
        &mut state.fog,
    );

    // Seed: shown as two groups of hex digits, with a re-roll.
    let r = row(1.0);
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    ui.text(
        r.x + 16.0,
        r.mid_y(),
        type_scale::BODY,
        rgb(palette::TEXT, 0.82),
        "Seed",
    );
    let reroll = Rect::new(r.right() - 96.0, r.mid_y() - 16.0, 96.0, 32.0);
    if ui.button(
        id("rule-seed", 0),
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

    let r = row(2.0);
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    ui.text(
        r.x + 16.0,
        r.mid_y(),
        type_scale::BODY,
        rgb(palette::TEXT, 0.82),
        "Callsign",
    );
    ui.text_field(
        id("rule-name", 0),
        Rect::new(r.right() - 220.0, r.mid_y() - 17.0, 220.0, 34.0),
        &mut state.name,
        16,
    );

    // The sky: the map's own weather and time of day, or what is picked here.
    let y = area.y + 26.0 + 3.0 * RULE_PITCH + 20.0;
    ui.section(area.x, y, area.w, "Sky");
    let look = super::sky::Look { row_h: RULE_PITCH - 4.0, pitch: RULE_PITCH, value_w: 220.0, compact: false };
    super::sky::rows(ui, 0, area.x, y + 20.0, area.w, look, &mut state.sky);
}

fn map_preview(ui: &mut Ui, state: &mut SkirmishState, area: Rect) {
    let Some(entry) = state.maps.get(state.selected) else {
        return;
    };
    if state.preview_of != Some(state.selected) {
        ui.o.set_image(
            PREVIEW_SLOT,
            preview::SIZE,
            preview::SIZE,
            &preview::render(&entry.map),
        );
        state.preview_of = Some(state.selected);
    }
    let map = entry.map.clone();
    let side = area.w.min(area.h - 64.0);
    let frame = Rect::new(area.x + (area.w - side) * 0.5, area.y, side, side);
    // The chart fades up when the map changes.
    let shown = ui.ease(id("preview-shown", state.selected), 1.0, 5.0);
    ui.fill(frame, ink(0.85));
    ui.image(
        PREVIEW_SLOT,
        [0.0, 0.0, preview::SIZE as f32, preview::SIZE as f32],
        frame,
        [shown, shown, shown, 1.0],
    );
    // Chart furniture: grid, frame, brackets, a scale bar.
    for k in 1..4 {
        let t = k as f32 / 4.0;
        ui.vline(
            frame.x + frame.w * t,
            frame.y,
            frame.h,
            rgb(palette::LINE, 0.07),
        );
        ui.hline(
            frame.x,
            frame.y + frame.h * t,
            frame.w,
            rgb(palette::LINE, 0.07),
        );
    }
    ui.frame(frame, rgb(palette::LINE, 0.25));
    ui.brackets(frame.inset(-6.0), 14.0, rgb(palette::ACCENT, 0.7));
    let size_m = map.info().size_metres().to_f32();
    let km_pts = frame.w / (size_m[0].max(size_m[1]) / 1000.0);
    let bar_km = if size_m[0] > 30_000.0 { 10.0 } else { 2.0 };
    ui.fill(
        Rect::new(frame.x + 14.0, frame.bottom() - 16.0, km_pts * bar_km, 2.0),
        rgb(palette::TEXT, 0.8),
    );
    ui.text(
        frame.x + 14.0,
        frame.bottom() - 28.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.8),
        &format!("{bar_km:.0} km"),
    );
    ui.text_right(
        frame.right() - 12.0,
        frame.y + 16.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.6),
        "N",
    );
    ui.stroke(
        Vec2::new(frame.right() - 16.0, frame.y + 44.0),
        Vec2::new(frame.right() - 16.0, frame.y + 26.0),
        1.2,
        rgb(palette::TEXT, 0.6),
    );

    // Start positions. Click one to deploy there; whoever held it takes yours.
    let mut take = None;
    for (n, start) in map.start_positions().iter().enumerate().take(8) {
        let p = Vec2::new(frame.x, frame.y) + preview::locate(&map, start.to_f32(), side);
        let holder = state
            .slots
            .iter()
            .position(|s| s.seat != Seat::Closed && s.start as usize == n);
        let res = ui.interact(
            id("start-marker", n),
            Rect::new(p.x - 17.0, p.y - 17.0, 34.0, 34.0),
            true,
        );
        if res.clicked && holder != Some(0) && !state.observing() {
            take = Some(n as u8);
        }
        let color = holder.map_or(rgb(palette::DIM, 0.9), |i| {
            let c = TEAM_COLORS[state.slots[i].color as usize];
            [c[0], c[1], c[2], 1.0]
        });
        ui.disc(p, 13.0 + 2.0 * res.glow, ink(0.85));
        ui.arc(
            p,
            13.0 + 2.0 * res.glow,
            0.0,
            std::f32::consts::TAU,
            if holder.is_some() { 2.2 } else { 1.2 },
            color,
        );
        if holder == Some(0) && !state.observing() {
            let pulse = (ui.time * 0.8).fract();
            ui.arc(
                p,
                14.0 + 16.0 * pulse,
                0.0,
                std::f32::consts::TAU,
                1.4,
                [color[0], color[1], color[2], 0.8 * (1.0 - pulse)],
            );
        }
        ui.text_centred(
            p.x + 1.0,
            p.y,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &format!("{}", n + 1),
        );
        if res.glow > 0.05 {
            let tip = match holder {
                Some(0) if state.observing() => "AI Landing Zone",
                Some(0) => "Your Landing Zone",
                Some(_) => "Click to Swap Places",
                None if state.observing() => "Unoccupied",
                None => "Click to Deploy Here",
            };
            let tw = ui.text_width(type_scale::MICRO, tip);
            let tag = Rect::new(
                (p.x - tw * 0.5 - 8.0).clamp(frame.x, frame.right() - tw - 16.0),
                p.y + 22.0,
                tw + 16.0,
                20.0,
            );
            ui.fill(tag, ink(0.9 * res.glow));
            ui.text(
                tag.x + 8.0,
                tag.mid_y(),
                type_scale::MICRO,
                rgb(palette::TEXT, res.glow),
                tip,
            );
        }
    }
    if let Some(n) = take {
        let mine = state.slots[0].start;
        if let Some(other) = state.slots.iter_mut().skip(1).find(|s| s.start == n) {
            other.start = mine;
        }
        state.slots[0].start = n;
        ui.audio.play(Sfx::Select);
    }

    // Caption under the chart.
    let y = frame.bottom() + 30.0;
    let end = ui.text(
        frame.x,
        y,
        type_scale::ITEM,
        rgb(palette::TEXT, 1.0),
        &map.name(),
    );
    ui.text(
        end + 18.0,
        y + 1.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!(
            "{:.1} \u{d7} {:.1} Km",
            size_m[0] / 1000.0,
            size_m[1] / 1000.0
        ),
    );
    ui.text_right(
        frame.right(),
        y + 1.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Red: Ore fields    rings: Landing zones",
    );
}

/// Choices in an open seat's Control list; an AI seat's difficulty is picked here too.
const CONTROL: [&str; 4] = ["Closed", "AI \u{b7} Easy", "AI \u{b7} Normal", "AI \u{b7} Hard"];
const DIFFICULTIES: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];
const ROW_H: f32 = 54.0;
const ROW_PITCH: f32 = 60.0;
/// The doctrine strip that opens under an AI row.
const TUNE_H: f32 = 74.0;
const FORCE_PRESETS: [[u8; 3]; 4] = [[100, 100, 100], [160, 60, 60], [60, 160, 60], [60, 60, 160]];
const FORCE_LABELS: [&str; 4] = ["Balanced", "Land", "Air", "Naval"];

fn commanders(ui: &mut Ui, state: &mut SkirmishState, area: Rect) {
    // Heading, and whether you command or watch.
    let switch_w = 236.0;
    ui.section(area.x, area.y + 6.0, area.w - switch_w - 16.0, "Commanders");
    for (k, (label, observe)) in [("Command", false), ("Observe", true)].into_iter().enumerate() {
        let r = Rect::new(
            area.right() - switch_w + k as f32 * (switch_w * 0.5 + 2.0),
            area.y - 10.0,
            switch_w * 0.5 - 2.0,
            32.0,
        );
        let on = state.observe == observe;
        let res = ui.tile(id("seat-mode", k), r, on, true);
        ui.text_centred(
            r.x + r.w * 0.5,
            r.mid_y(),
            type_scale::BUTTON,
            rgb(if on { 0xFFFFFF } else { palette::DIM }, 0.85 + 0.15 * res.glow),
            label,
        );
        if res.clicked && !on {
            state.observe = observe;
            ui.audio.play(Sfx::Select);
        }
    }

    let head = area.y + 44.0;
    let (seat_x, team_x, start_x) = (
        area.right() - 404.0,
        area.right() - 222.0,
        area.right() - 96.0,
    );
    for (x, label) in [
        (area.x + 58.0, "Commander"),
        (seat_x, "Control"),
        (team_x, "Team"),
        (start_x, "Zone"),
    ] {
        ui.text(x, head, type_scale::MICRO, rgb(palette::DIM, 0.8), label);
    }
    let starts = state.slots.len();
    if state.tuning.is_some_and(|i| !state.slots.get(i).is_some_and(|s| state.is_ai(s))) {
        state.tuning = None;
    }
    if state.coloring.is_some_and(|i| state.slots.get(i).is_none_or(|s| s.seat == Seat::Closed)) {
        state.coloring = None;
    }
    let mut y = head + 16.0;
    for i in 0..starts {
        let slot = state.slots[i];
        let row = Rect::new(area.x, y, area.w, ROW_H);
        let open = slot.seat != Seat::Closed;
        let ai = state.is_ai(&slot);
        let tuning = state.tuning == Some(i);
        let live = if open { 1.0 } else { 0.4 };
        ui.fill(row, ink(0.5));
        ui.frame(row, rgb(if tuning { palette::ACCENT } else { palette::LINE }, if tuning { 0.5 } else { 0.12 }));

        // Colour swatch: opens every colour under the row.
        let coloring = state.coloring == Some(i);
        let c = TEAM_COLORS[slot.color as usize];
        let hit = Rect::new(row.x, row.y, 48.0, row.h);
        let res = ui.interact(id("slot-color", i), hit, open);
        let swatch = Rect::new(row.x + 8.0, row.mid_y() - 13.0, 26.0, 26.0).inset(-2.0 * res.glow);
        ui.fill(Rect::new(row.x, row.y, 3.0, row.h), [c[0], c[1], c[2], live]);
        ui.fill(swatch, [c[0], c[1], c[2], live]);
        ui.frame(swatch, rgb(0xFFFFFF, if coloring { 0.9 } else { 0.15 + 0.6 * res.glow }));
        if open {
            // A caret in the corner says it opens.
            let k = Vec2::new(swatch.right() - 5.0, swatch.bottom() - 5.0);
            ui.triangle(k + Vec2::new(-4.0, 0.0), k + Vec2::new(0.0, 0.0), k + Vec2::new(0.0, -4.0), ink(0.8));
        }
        if res.clicked {
            state.coloring = if coloring { None } else { Some(i) };
            state.tuning = None;
            ui.audio.play(Sfx::Select);
        }

        // Who: you, or an AI with its doctrine under its name; that line opens the tuning.
        let name = match slot.seat {
            Seat::Closed => "Open Slot".to_owned(),
            _ if ai => state.ai_name(i),
            _ => state.name.clone(),
        };
        let name_w = seat_x - row.x - 70.0;
        let name_y = if open { row.y + 18.0 } else { row.mid_y() };
        ui.text_fit_left(row.x + 58.0, name_y, name_w, type_scale::BODY, rgb(palette::TEXT, live), &name);
        if ai {
            let tune = Rect::new(row.x + 48.0, row.y + 28.0, name_w + 10.0, 22.0);
            let res = ui.interact(id("slot-ai-tune", i), tune, true);
            let summary = format!(
                "{}  \u{b7}  {}",
                doctrine_label(slot.ai.doctrine),
                force_label(slot.ai.domain_weights),
            );
            let tone = rgb(if tuning || res.glow > 0.3 { palette::ACCENT } else { palette::DIM }, 1.0);
            ui.text_fit_left(row.x + 58.0, row.y + 38.0, name_w - 16.0, type_scale::MICRO, tone, &summary);
            // A caret: down to open the tuning, up to close it.
            let tw = ui.text_width(type_scale::MICRO, &summary).min(name_w - 16.0);
            let c = Vec2::new(row.x + 58.0 + tw + 9.0, row.y + 38.0);
            let d = if tuning { -1.0 } else { 1.0 };
            ui.triangle(
                c + Vec2::new(-3.5, -2.0 * d),
                c + Vec2::new(3.5, -2.0 * d),
                c + Vec2::new(0.0, 2.5 * d),
                tone,
            );
            if res.clicked {
                state.tuning = if tuning { None } else { Some(i) };
                state.coloring = None;
                ui.audio.play(Sfx::Select);
            }
        } else if slot.seat == Seat::You {
            ui.text(row.x + 58.0, row.y + 38.0, type_scale::MICRO, rgb(palette::ACCENT, 1.0), "You Command");
        }

        let field = |x: f32, w: f32| Rect::new(x, row.y + 10.0, w, row.h - 20.0);
        if slot.seat == Seat::You && !state.observe {
            let f = field(seat_x, 168.0);
            ui.text(f.x + 12.0, f.mid_y(), type_scale::VALUE, rgb(palette::ACCENT, 1.0), "Player");
        } else {
            let at = if ai {
                1 + DIFFICULTIES.iter().position(|d| *d == slot.ai.difficulty).unwrap_or(1)
            } else {
                0
            };
            // Your seat cannot be closed: while you watch, an AI always holds it.
            let options: &[&str] = if slot.seat == Seat::You { &CONTROL[1..] } else { &CONTROL };
            let shown = if slot.seat == Seat::You { at - 1 } else { at };
            if let Some(pick) = ui.dropdown(id("slot-seat", i), field(seat_x, 168.0), options, shown, true) {
                let pick = if slot.seat == Seat::You { pick + 1 } else { pick };
                if pick == 0 {
                    state.slots[i].seat = Seat::Closed;
                } else {
                    state.slots[i].ai.difficulty = DIFFICULTIES[pick - 1];
                    if slot.seat == Seat::Closed {
                        state.slots[i].seat = Seat::Ai;
                        // Take whatever zone and colour are free.
                        if state.taken(i, |s| s.start, slot.start) {
                            state.step_unique(i, 1, starts, |s| s.start, |s, v| s.start = v);
                        }
                        if state.taken(i, |s| s.color, slot.color) {
                            state.step_unique(i, 1, TEAM_COLORS.len(), |s| s.color, |s, v| s.color = v);
                        }
                    }
                }
            }
        }
        let step = ui.stepper(
            id("slot-team", i),
            field(team_x, 112.0),
            &format!("Team {}", slot.team + 1),
            rgb(palette::TEXT, 1.0),
            open,
        );
        if step != 0 {
            state.slots[i].team = (slot.team as i32 + step).rem_euclid(starts as i32) as u8;
        }
        let step = ui.stepper(
            id("slot-start", i),
            field(start_x, 96.0),
            &format!("{}", slot.start + 1),
            rgb(palette::TEXT, 1.0),
            open && starts > 1,
        );
        if step != 0 {
            state.step_unique(i, step, starts, |s| s.start, |s, v| s.start = v);
        }
        y = row.y + ROW_PITCH;
        if tuning {
            ai_tuning(ui, state, i, Rect::new(row.x + 14.0, row.bottom(), row.w - 14.0, TUNE_H));
            y += TUNE_H;
        }
        if state.coloring == Some(i) {
            colour_picker(ui, state, i, Rect::new(row.x + 14.0, row.bottom(), row.w - 14.0, SWATCH_H));
            y += SWATCH_H;
        }
    }

    // Quick team layouts.
    let y = y + 8.0;
    if y + 34.0 < area.bottom() {
        ui.text(area.x, y + 16.0, type_scale::MICRO, rgb(palette::DIM, 1.0), "Teams");
        let open: Vec<usize> = (0..starts).filter(|&i| state.slots[i].seat != Seat::Closed).collect();
        let layouts: [(&str, fn(usize) -> u8); 2] = [
            ("Free for All", |k| k as u8),
            ("Two Sides", |k| (k % 2) as u8),
        ];
        for (n, (label, team)) in layouts.into_iter().enumerate() {
            let r = Rect::new(area.x + 64.0 + n as f32 * 150.0, y, 142.0, 32.0);
            if ui.button(id("team-layout", n), r, label, ButtonKind::Secondary, open.len() > 2) {
                for (k, &i) in open.iter().enumerate() {
                    state.slots[i].team = team(k);
                }
                ui.audio.play(Sfx::Tick);
            }
        }
        if y + 60.0 < area.bottom() {
            ui.text(
                area.x,
                y + 56.0,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                "AI commanders get no resource bonuses. Difficulty sets reaction speed and memory.",
            );
        }
    }
}

fn force_label(weights: [u8; 3]) -> &'static str {
    FORCE_PRESETS
        .iter()
        .position(|w| *w == weights)
        .map_or("Custom", |i| FORCE_LABELS[i])
}

/// The colour strip that opens under a row.
const SWATCH_H: f32 = 52.0;

/// Every colour for slot `i`; one another commander wears shows their number,
/// and picking it swaps the two.
fn colour_picker(ui: &mut Ui, state: &mut SkirmishState, i: usize, area: Rect) {
    ui.fill(area, ink(0.35));
    ui.fill(Rect::new(area.x, area.y, 2.0, area.h), rgb(palette::ACCENT, 0.6));
    ui.text(area.x + 12.0, area.mid_y(), type_scale::MICRO, rgb(palette::DIM, 1.0), "Colour");
    let size = 34.0;
    let mut pick = None;
    for (n, c) in TEAM_COLORS.iter().enumerate() {
        let r = Rect::new(area.x + 72.0 + n as f32 * (size + 10.0), area.mid_y() - size * 0.5, size, size);
        let mine = state.slots[i].color as usize == n;
        let holder = (0..state.slots.len())
            .find(|&k| k != i && state.slots[k].seat != Seat::Closed && state.slots[k].color as usize == n);
        let res = ui.interact(id("slot-swatch", i * 16 + n), r, !mine);
        let grow = r.inset(-2.0 * res.glow);
        ui.fill(grow, [c[0], c[1], c[2], if holder.is_some() { 0.45 } else { 1.0 }]);
        ui.frame(
            grow.inset(-3.0),
            rgb(0xFFFFFF, if mine { 0.95 } else { 0.5 * res.glow }),
        );
        if let Some(k) = holder {
            ui.text_centred(r.x + r.w * 0.5, r.mid_y(), type_scale::VALUE, rgb(0xFFFFFF, 0.95), &format!("{}", k + 1));
        }
        if res.glow > 0.3 && holder.is_some() {
            ui.text_right(area.right() - 12.0, area.mid_y(), type_scale::MICRO, rgb(palette::DIM, 1.0), "Taken \u{b7} Click to Swap");
        }
        if res.clicked {
            pick = Some((n as u8, holder));
        }
    }
    if let Some((n, holder)) = pick {
        if let Some(k) = holder {
            state.slots[k].color = state.slots[i].color;
        }
        state.slots[i].color = n;
        state.coloring = None;
        ui.audio.play(Sfx::Tick);
    }
}

/// Doctrine, force preference and adaptation for AI slot `i`, opened under its row.
fn ai_tuning(ui: &mut Ui, state: &mut SkirmishState, i: usize, area: Rect) {
    ui.fill(area, ink(0.35));
    ui.fill(Rect::new(area.x, area.y, 2.0, area.h), rgb(palette::ACCENT, 0.6));
    let ai = &mut state.slots[i].ai;
    let gap = 14.0;
    let w = (area.w - 24.0 - 2.0 * gap) / 3.0;
    let field = |k: f32| Rect::new(area.x + 12.0 + k * (w + gap), area.y + 30.0, w, 32.0);
    for (k, label) in ["Doctrine", "Force Preference", "Adaptation"].into_iter().enumerate() {
        let f = field(k as f32);
        ui.text(f.x, f.y - 12.0, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
    }
    const DOCTRINES: [Doctrine; 4] = [
        Doctrine::Adaptive,
        Doctrine::Aggressive,
        Doctrine::Economic,
        Doctrine::Defensive,
    ];
    let at = DOCTRINES.iter().position(|d| *d == ai.doctrine).unwrap_or(0);
    let labels = DOCTRINES.map(doctrine_label);
    if let Some(pick) = ui.dropdown(id("ai-doctrine", i), field(0.0), &labels, at, true) {
        ai.doctrine = DOCTRINES[pick];
    }
    // A custom mix (from a saved config) shows as its nearest preset until changed.
    let at = FORCE_PRESETS.iter().position(|w| *w == ai.domain_weights).unwrap_or(0);
    if let Some(pick) = ui.dropdown(id("ai-domain", i), field(1.0), &FORCE_LABELS, at, true) {
        ai.domain_weights = FORCE_PRESETS[pick];
    }
    let step = ui.stepper(
        id("ai-adaptation", i),
        field(2.0),
        &format!("{}%", ai.adaptation),
        rgb(palette::TEXT, 1.0),
        true,
    );
    if step != 0 {
        ai.adaptation = ((ai.adaptation as i32 / 25 + step).rem_euclid(5) * 25) as u8;
    }
}

fn doctrine_label(d: Doctrine) -> &'static str {
    match d {
        Doctrine::Adaptive => "Adaptive",
        Doctrine::Aggressive => "Aggressive",
        Doctrine::Economic => "Economic",
        Doctrine::Defensive => "Defensive",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observing_your_slot_starts_an_all_ai_match() {
        let mut state = SkirmishState::new("", true, "Tester");
        assert!(
            !state.maps.is_empty(),
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

        state.slots[1].ai.difficulty = Difficulty::Hard;
        state.slots[1].ai.doctrine = Doctrine::Defensive;
        state.slots[1].ai.domain_weights = [50, 150, 75];
        let configured = state.request();
        assert_eq!(configured.config.players[1].ai, state.slots[1].ai);
        let preserved = state.slots[1].ai;
        state.seat_for_map();
        assert_eq!(state.slots[1].ai, preserved);
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
