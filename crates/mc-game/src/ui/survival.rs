//! Survival set-up: pick a theatre and a landing zone, see where the
//! Replication Engine attacks from, and set how hard, how long and how high
//! in tech its rounds go. The chart in the middle is the siege at a glance:
//! the engine, every front flowing toward your zone in its domain's colour,
//! the harbor, the node sites. The forecast under the rules draws the rounds.

use super::{id, ink, palette, preview, rgb, type_scale, ButtonKind, Color, Key, Rect, Style, Ui};
use crate::audio::Sfx;
use crate::hud::style::{AIR, LAND, NAVY};
use crate::settings::Settings;
use crate::setup::{self, TEAM_COLORS};
use crate::survival::{front_counts, match_for, Setup, ENGINE_COLOR};
use glam::Vec2;
use mc_data::survival::{Domain, SurvivalLayout};
use mc_data::IconKind;
use mc_map::MapFile;
use mc_sim::{AiConfig, SurvivalRules};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

use super::skirmish::MatchRequest;

/// Image slot holding the theatre's chart (0 is skirmish's, 1 the menu's).
const PREVIEW_SLOT: usize = 2;
const LEFT: f32 = 64.0;
/// The highest tier that has units to print; above it the engine falls back.
const UNITS_TOP: u8 = 3;

pub struct Theatre {
    pub stem: String,
    pub map: Arc<MapFile>,
    pub layout: SurvivalLayout,
}

pub enum SurvivalAction {
    Back,
    Start(MatchRequest),
}

pub struct SurvivalState {
    pub maps: Vec<Theatre>,
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

impl SurvivalState {
    /// Opens every map in `maps/` that has a survival layout.
    pub fn new(settings: &Settings) -> SurvivalState {
        let mut maps: Vec<Theatre> = setup::list_maps()
            .into_iter()
            .filter_map(|path| {
                let map = MapFile::open(&path).ok()?;
                let layout = crate::survival::layout(&map)?;
                Some(Theatre {
                    stem: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    map: Arc::new(map),
                    layout,
                })
            })
            .collect();
        maps.sort_by_key(|m| (m.map.info().tile_count(), m.stem.clone()));
        let selected = maps.iter().position(|m| m.stem == settings.survival_map).unwrap_or(0);
        let mut state = SurvivalState {
            maps,
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
        self.rules.tier_cap = self.rules.tier_cap.clamp(1, 5);
        self.rules.nodes = self.rules.nodes.min(3);
        let counts = self.counts();
        let live = Domain::ALL.iter().zip(counts).any(|(d, n)| n > 0 && self.rules.has(*d));
        if !live {
            for (d, n) in Domain::ALL.iter().zip(counts) {
                if n > 0 {
                    self.rules.fronts |= d.bit();
                }
            }
        }
    }

    /// Domains that will attack: switched on, and with lanes on this theatre.
    fn attacking(&self) -> Vec<Domain> {
        let counts = self.counts();
        Domain::ALL.into_iter().zip(counts).filter(|(d, n)| *n > 0 && self.rules.has(*d)).map(|(d, _)| d).collect()
    }

    /// Flips a domain's fronts. Refused (false) when the domain has no lanes
    /// here or it is the last one attacking.
    pub fn toggle_front(&mut self, d: Domain) -> bool {
        let counts = self.counts();
        let lanes = counts[domain_index(d)];
        if lanes == 0 {
            return false;
        }
        if self.rules.has(d) && self.attacking() == vec![d] {
            return false;
        }
        self.rules.fronts ^= d.bit();
        true
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
        });
        let mut colors = TEAM_COLORS;
        colors[1] = ENGINE_COLOR;
        Some(MatchRequest { map: t.map.clone(), config, colors, survival: Some(survival) })
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

// -- colour and glyphs -----------------------------------------------------------

fn domain_index(d: Domain) -> usize {
    match d {
        Domain::Land => 0,
        Domain::Air => 1,
        Domain::Naval => 2,
    }
}

fn domain_hex(d: Domain) -> u32 {
    match d {
        Domain::Land => LAND,
        Domain::Air => AIR,
        Domain::Naval => NAVY,
    }
}

/// A domain's colour. Navy is lifted a little on the chart, where it sits on dark sea.
fn dcol(d: Domain, a: f32) -> Color {
    rgb(if d == Domain::Naval { 0x4F86FF } else { domain_hex(d) }, a)
}

fn engine(a: f32) -> Color {
    [ENGINE_COLOR[0], ENGINE_COLOR[1], ENGINE_COLOR[2], a]
}

fn domain_icon(d: Domain) -> IconKind {
    match d {
        Domain::Land => IconKind::Tank,
        Domain::Air => IconKind::Fighter,
        Domain::Naval => IconKind::Ship,
    }
}

fn domain_glyph(ui: &mut Ui, d: Domain, c: Vec2, r: f32, color: Color) {
    crate::hud::icons::strategic(ui, domain_icon(d), 0, c, r, color, ink(0.9));
}

fn hexagon(c: Vec2, r: f32, turn: f32) -> [Vec2; 6] {
    std::array::from_fn(|k| c + Vec2::from_angle(-FRAC_PI_2 + turn + k as f32 * TAU / 6.0) * r)
}

fn fill_hex(ui: &mut Ui, c: Vec2, r: f32, turn: f32, color: Color) {
    let p = hexagon(c, r, turn);
    for k in 0..6 {
        ui.triangle(c, p[k], p[(k + 1) % 6], color);
    }
}

fn outline_hex(ui: &mut Ui, c: Vec2, r: f32, turn: f32, t: f32, color: Color) {
    let p = hexagon(c, r, turn);
    for k in 0..6 {
        ui.stroke(p[k], p[(k + 1) % 6], t, color);
    }
}

/// A ring of `n` dashes turned by `turn`.
fn dashed_ring(ui: &mut Ui, c: Vec2, r: f32, n: usize, duty: f32, turn: f32, t: f32, color: Color) {
    let step = TAU / n as f32;
    for k in 0..n {
        let a = turn + k as f32 * step;
        ui.arc(c, r, a, a + step * duty, t, color);
    }
}

/// The Replication Engine's mark: a foundry hexagon with a core, and (when
/// `veil`) its slowly turning dashed veil.
pub fn engine_mark(ui: &mut Ui, c: Vec2, r: f32, a: f32, veil: bool) {
    let pulse = 0.5 + 0.5 * (ui.time * 2.2).sin();
    if veil {
        ui.disc(c, r * 2.6, engine(0.05 * a));
        ui.disc(c, r * 1.8, engine(0.07 * a));
        dashed_ring(ui, c, r * 2.1, 18, 0.55, ui.time * 0.25, 1.3, engine(0.75 * a));
        dashed_ring(ui, c, r * 2.6, 9, 0.3, -ui.time * 0.15, 1.0, engine(0.35 * a));
    }
    fill_hex(ui, c, r, 0.0, ink(0.9 * a));
    outline_hex(ui, c, r, 0.0, (r * 0.16).max(1.4), engine(a));
    fill_hex(ui, c, r * 0.46, 0.0, engine((0.55 + 0.4 * pulse) * a));
    // Three bays: short spokes from the core to alternate corners.
    let p = hexagon(c, r, 0.0);
    for k in [1, 3, 5] {
        ui.stroke(c + (p[k] - c) * 0.5, c + (p[k] - c) * 0.82, (r * 0.12).max(1.0), engine(0.9 * a));
    }
}

fn diamond(ui: &mut Ui, c: Vec2, r: f32, fill: Color, edge: Color) {
    let (n, e, s, w) = (c - Vec2::Y * r, c + Vec2::X * r, c + Vec2::Y * r, c - Vec2::X * r);
    ui.triangle(n, e, s, fill);
    ui.triangle(n, s, w, fill);
    for (a, b) in [(n, e), (e, s), (s, w), (w, n)] {
        ui.stroke(a, b, 1.2, edge);
    }
}

fn anchor(ui: &mut Ui, c: Vec2, r: f32, color: Color) {
    let t = (r * 0.2).max(1.3);
    ui.arc(c - Vec2::Y * r * 0.72, r * 0.2, 0.0, TAU, t, color);
    ui.stroke(c - Vec2::Y * r * 0.52, c + Vec2::Y * r * 0.85, t, color);
    ui.stroke(c + Vec2::new(-r * 0.45, -r * 0.25), c + Vec2::new(r * 0.45, -r * 0.25), t, color);
    ui.arc(c + Vec2::Y * r * 0.1, r * 0.75, 0.35, std::f32::consts::PI - 0.35, t, color);
}

/// Splits `text` into lines no wider than `width`.
fn wrap(ui: &mut Ui, st: Style, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let trial = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if ui.text_width(st, &trial) > width && !line.is_empty() {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = trial;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// `text` cut short with an ellipsis to fit `width`, in `st` as given.
fn clip(ui: &mut Ui, st: Style, text: &str, width: f32) -> String {
    if ui.text_width(st, text) <= width {
        return text.to_owned();
    }
    let mut cut = text.to_owned();
    while cut.pop().is_some() {
        let candidate = format!("{}\u{2026}", cut.trim_end());
        if ui.text_width(st, &candidate) <= width {
            return candidate;
        }
    }
    String::new()
}

/// A tooltip card: a title in `accent`, then lines of text.
struct Tip {
    at: Vec2,
    title: String,
    accent: Color,
    lines: Vec<String>,
    glow: f32,
}

fn draw_tip(ui: &mut Ui, tip: &Tip, bounds: Rect) {
    const W: f32 = 270.0;
    let mut lines = Vec::new();
    for l in &tip.lines {
        lines.extend(wrap(ui, type_scale::MICRO, l, W - 24.0));
    }
    let tw = ui.text_width(type_scale::VALUE, &tip.title);
    let lw = lines.iter().map(|l| ui.text_width(type_scale::MICRO, l)).fold(tw, f32::max);
    let w = (lw + 24.0).min(W);
    let h = 34.0 + lines.len() as f32 * 16.0;
    let mut y = tip.at.y + 22.0;
    if y + h > bounds.bottom() - 4.0 {
        y = tip.at.y - 22.0 - h;
    }
    let r = Rect::new((tip.at.x - w * 0.5).clamp(bounds.x + 4.0, bounds.right() - w - 4.0), y, w, h);
    let g = tip.glow;
    ui.fill(Rect::new(r.x + 3.0, r.y + 4.0, r.w, r.h), ink(0.4 * g));
    ui.fill(r, rgb(0x0B0C0E, 0.94 * g));
    ui.frame(r, rgb(palette::LINE, 0.22 * g));
    ui.fill(Rect::new(r.x, r.y, 3.0, r.h), [tip.accent[0], tip.accent[1], tip.accent[2], g]);
    ui.text(r.x + 12.0, r.y + 16.0, type_scale::VALUE, [tip.accent[0], tip.accent[1], tip.accent[2], g], &tip.title);
    for (i, l) in lines.iter().enumerate() {
        ui.text(r.x + 12.0, r.y + 36.0 + i as f32 * 16.0, type_scale::MICRO, rgb(palette::DIM, g), l);
    }
}

/// How far along a panel is in arriving: eased, and `delay` later than the screen.
fn arrive(enter: f32, delay: f32) -> f32 {
    let k = ((enter - delay) / (1.0 - delay)).clamp(0.0, 1.0);
    1.0 - (1.0 - k) * (1.0 - k)
}

fn km(m: f32) -> String {
    format!("{:.1} km", m / 1000.0)
}

fn mass(v: i64) -> String {
    if v >= 10_000_000 {
        format!("{:.0}M", v as f32 / 1e6)
    } else if v >= 1_000_000 {
        format!("{:.1}M", v as f32 / 1e6)
    } else if v >= 10_000 {
        format!("{:.0}k", v as f32 / 1000.0)
    } else if v >= 1000 {
        format!("{:.1}k", v as f32 / 1000.0)
    } else {
        format!("{v}")
    }
}

// -- the screen ------------------------------------------------------------------

pub fn draw(ui: &mut Ui, state: &mut SurvivalState, enter: f32) -> Option<SurvivalAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.66 * enter));
    ui.scrim(Rect::new(0.0, 0.0, w, 220.0), 0.6 * enter, 0.0, false);
    ui.fade = enter;
    ui.shift = Vec2::new(0.0, 14.0 * (1.0 - enter));

    // Header: the engine's mark where skirmish has the emblem.
    engine_mark(ui, Vec2::new(LEFT + 15.0, 84.0), 12.0, 1.0, false);
    let end = ui.text(LEFT + 50.0, 84.0, type_scale::TITLE, rgb(0xFFFFFF, 1.0), "Survival");
    ui.text(end + 18.0, 90.0, type_scale::CAPTION, rgb(palette::DIM, 1.0), "Hold Out Against the Replication Engine");
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
    let centre = Rect::new(left.right() + gap, top, right.x - gap - left.right() - gap, bottom - top);

    // Left column slides in from the left, the rules from the right, the chart rises.
    let k = arrive(enter, 0.0);
    ui.fade = k;
    ui.shift = Vec2::new(-24.0 * (1.0 - k), 0.0);
    ui.panel(Rect::new(left.x - 22.0, top - 20.0, left_w + 44.0, bottom - top + 40.0));
    let list_h = 28.0 + state.maps.len().clamp(1, 3) as f32 * (THEATRE_ROW + 6.0);
    theatres(ui, state, Rect::new(left.x, top, left_w, list_h));
    zones(ui, state, Rect::new(left.x, top + list_h + 12.0, left_w, left.h - RULES_H - list_h - 24.0));
    commander(ui, state, Rect::new(left.x, bottom - RULES_H, left_w, RULES_H));

    let k = arrive(enter, 0.12);
    ui.fade = k;
    ui.shift = Vec2::new(24.0 * (1.0 - k), 0.0);
    ui.panel(Rect::new(right.x - 22.0, top - 20.0, right_w + 44.0, bottom - top + 40.0));
    engagement(ui, state, right);

    let k = arrive(enter, 0.06);
    ui.fade = k;
    ui.shift = Vec2::new(0.0, 18.0 * (1.0 - k));
    chart(ui, state, centre);

    // Footer.
    ui.fade = enter;
    ui.shift = Vec2::new(0.0, 14.0 * (1.0 - enter));
    let problem = state.problem();
    let back = ui.button(id("survival-back", 0), Rect::new(LEFT, h - 64.0 - 52.0, 200.0, 52.0), "Back", ButtonKind::Secondary, true);
    let start_rect = Rect::new(w - LEFT - 340.0, h - 64.0 - 58.0, 340.0, 58.0);
    let start = ui.button(id("survival-start", 0), start_rect, "Begin Survival", ButtonKind::Primary, problem.is_none());
    match problem {
        Some(text) => ui.text_right(start_rect.x - 24.0, start_rect.mid_y(), type_scale::CAPTION, rgb(palette::WARN, 1.0), text),
        None => {
            let t = state.theatre().expect("no problem means a theatre");
            let zone = t.layout.spawns.get(state.spawn).map_or("", |s| s.name.as_str());
            let lanes: usize = state.attacking().iter().map(|d| state.counts()[domain_index(*d)]).sum();
            let rounds = if state.rules.rounds == 0 { "Endless".to_owned() } else { format!("{} Rounds", state.rules.rounds) };
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
    } else if (start || (ui.input.key(Key::Enter) && !typing && !listing && ui.interactive)) && problem.is_none() {
        if let Some(request) = state.request() {
            ui.audio.play(Sfx::Launch);
            action = Some(SurvivalAction::Start(request));
        }
    }
    ui.fade = 1.0;
    ui.shift = Vec2::ZERO;
    action
}

// -- left: theatres and commander ------------------------------------------------

const RULE_PITCH: f32 = 40.0;
const RULES_H: f32 = 26.0 + 3.0 * RULE_PITCH + 44.0 + super::sky::ROWS as f32 * RULE_PITCH;
const THEATRE_ROW: f32 = 64.0;

fn theatres(ui: &mut Ui, state: &mut SurvivalState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Theatre");
    let fit = ((area.h - 28.0) / (THEATRE_ROW + 6.0)).floor().max(1.0) as usize;
    let mut pick = None;
    for (i, t) in state.maps.iter().enumerate().take(fit) {
        let row = Rect::new(area.x, area.y + 28.0 + i as f32 * (THEATRE_ROW + 6.0), area.w, THEATRE_ROW);
        let res = ui.interact(id("theatre-row", i), row, true);
        let chosen = state.selected == i;
        if res.clicked && !chosen {
            pick = Some(i);
        }
        let lit = ui.ease(id("theatre-lit", i), if chosen { 1.0 } else { 0.0 }, 12.0);
        let g = lit.max(res.glow * 0.6);
        ui.fill(row, ink(0.5));
        ui.gradient_h(row, rgb(palette::ACCENT, 0.2 * g), rgb(palette::ACCENT, 0.01));
        ui.frame(row, rgb(if chosen { palette::ACCENT } else { palette::LINE }, 0.14 + 0.4 * g));
        ui.fill(Rect::new(row.x, row.y, 4.0, row.h), rgb(palette::ACCENT, lit));
        let x = row.x + 20.0 + 4.0 * g;
        ui.text_fit_left(
            x,
            row.y + 21.0,
            row.w - 160.0,
            type_scale::ITEM,
            rgb(if chosen { palette::ACCENT } else { palette::TEXT }, 0.85 + 0.15 * g),
            &t.map.name(),
        );
        let size = t.map.info().size_metres().to_f32();
        ui.text(
            x + 1.0,
            row.y + 45.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!(
                "{:.0} km  \u{b7}  {} Zones  \u{b7}  {} Node Sites",
                size[0].max(size[1]) / 1000.0,
                t.layout.spawns.len(),
                t.layout.node_sites.len()
            ),
        );
        // Fronts by domain as small chips at the row's right.
        let counts = front_counts(&t.layout);
        let mut cx = row.right() - 12.0;
        for d in Domain::ALL.into_iter().rev() {
            let n = counts[domain_index(d)];
            let chip = Rect::new(cx - 42.0, row.y + 10.0, 42.0, 22.0);
            cx -= 46.0;
            let a = if n > 0 { 1.0 } else { 0.3 };
            ui.fill(chip, dcol(d, 0.10 * a));
            ui.frame(chip, dcol(d, 0.45 * a));
            domain_glyph(ui, d, Vec2::new(chip.x + 13.0, chip.mid_y()), 6.5, dcol(d, a));
            ui.text(chip.x + 25.0, chip.mid_y(), type_scale::VALUE, rgb(palette::TEXT, 0.9 * a), &n.to_string());
        }
    }
    if let Some(i) = pick {
        state.selected = i;
        state.settle_rules();
        ui.audio.play(Sfx::Select);
    }
    if state.maps.is_empty() {
        ui.text(area.x, area.y + 50.0, type_scale::BODY, rgb(palette::WARN, 1.0), "No survival maps in maps/");
    }
}

/// The theatre's landing zones as rows: the key that picks it, its name, what holding it is like.
fn zones(ui: &mut Ui, state: &mut SurvivalState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Landing Zones");
    let Some(t) = state.theatre() else {
        return;
    };
    let spawns = t.layout.spawns.clone();
    let mine = TEAM_COLORS[0];
    let mut pick = None;
    let mut y = area.y + 26.0;
    for (i, s) in spawns.iter().enumerate() {
        let mut lines = wrap(ui, type_scale::MICRO, &s.blurb, area.w - 58.0);
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
        let lit = ui.ease(id("sv-zone-row-lit", i), if chosen { 1.0 } else { 0.0 }, 12.0);
        let g = lit.max(res.glow * 0.6);
        let c = |a: f32| [mine[0], mine[1], mine[2], a];
        ui.fill(row, ink(0.45));
        ui.gradient_h(row, c(0.16 * g), c(0.0));
        ui.frame(row, if chosen { c(0.55) } else { rgb(palette::LINE, 0.12 + 0.25 * g) });
        ui.fill(Rect::new(row.x, row.y, 3.0, row.h), c(lit));
        ui.key_cap(row.x + 14.0, row.y + 10.0, &(i + 1).to_string(), chosen);
        ui.text(row.x + 42.0 + 3.0 * g, row.y + 17.0, type_scale::VALUE, if chosen { c(1.0) } else { rgb(palette::TEXT, 0.85 + 0.15 * g) }, &s.name);
        if chosen {
            ui.text_right(row.right() - 12.0, row.y + 17.0, type_scale::MICRO, c(0.9), "Your Zone");
        }
        for (k, l) in lines.iter().take(n).enumerate() {
            ui.text(row.x + 42.0 + 3.0 * g, row.y + 35.0 + k as f32 * 14.0, type_scale::MICRO, rgb(palette::DIM, 0.85 + 0.15 * g), l);
        }
        y += h + 6.0;
    }
    if let Some(i) = pick {
        state.spawn = i;
        ui.audio.play(Sfx::Select);
    }
}

fn commander(ui: &mut Ui, state: &mut SurvivalState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Commander");
    let row = |k: f32| Rect::new(area.x, area.y + 26.0 + k * RULE_PITCH, area.w, RULE_PITCH - 4.0);
    let r = row(0.0);
    label_row(ui, r, "Callsign", "");
    ui.text_field(id("survival-name", 0), Rect::new(r.right() - 200.0, r.mid_y() - 16.0, 200.0, 32.0), &mut state.name, 16);
    ui.toggle(id("survival-fog", 0), row(1.0), "Fog of War", "", &mut state.fog);
    let r = row(2.0);
    label_row(ui, r, "Seed", "");
    let reroll = Rect::new(r.right() - 90.0, r.mid_y() - 15.0, 90.0, 30.0);
    if ui.button(id("survival-seed", 0), reroll, "Roll", ButtonKind::Secondary, true) {
        state.seed = fresh_seed();
        ui.audio.play(Sfx::Tick);
    }
    ui.text_right(
        reroll.x - 14.0,
        r.mid_y(),
        type_scale::VALUE,
        rgb(palette::TEXT, 1.0),
        &format!("{:04X}-{:04X}", state.seed >> 16 & 0xFFFF, state.seed & 0xFFFF),
    );
    let y = area.y + 26.0 + 3.0 * RULE_PITCH + 20.0;
    ui.section(area.x, y, area.w, "Sky");
    let look = super::sky::Look { row_h: RULE_PITCH - 4.0, pitch: RULE_PITCH, value_w: 200.0, compact: false };
    super::sky::rows(ui, 3, area.x, y + 20.0, area.w, look, &mut state.sky);
}

/// A settings row's label and rule, as the toolkit's rows draw them.
fn label_row(ui: &mut Ui, r: Rect, label: &str, hint: &str) {
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    let end = ui.text(r.x + 16.0, r.mid_y(), type_scale::BODY, rgb(palette::TEXT, 0.82), label);
    if !hint.is_empty() {
        ui.text(end + 12.0, r.mid_y() + 0.5, type_scale::MICRO, rgb(palette::FAINT, 1.0), hint);
    }
}

// -- centre: the tactical chart ----------------------------------------------------

/// A front as drawn: from the engine, down its path, and the last leg to your zone.
struct Route {
    pts: Vec<Vec2>,
    /// Cumulative length at each point.
    at: Vec<f32>,
    /// Index of the path's first and last points in `pts`.
    first: usize,
    last: usize,
    /// Length on the ground, path and last leg, metres.
    metres: f32,
}

impl Route {
    fn new(pts: Vec<Vec2>, first: usize, last: usize, metres: f32) -> Route {
        let mut at = vec![0.0];
        for w in pts.windows(2) {
            at.push(at.last().unwrap() + w[0].distance(w[1]));
        }
        Route { pts, at, first, last, metres }
    }

    fn total(&self) -> f32 {
        *self.at.last().unwrap_or(&0.0)
    }

    /// Point and heading `s` along.
    fn sample(&self, s: f32) -> (Vec2, Vec2) {
        let i = self.at.partition_point(|&a| a <= s).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let len = self.at[i] - self.at[i - 1];
        let t = if len > 0.0 { (s - self.at[i - 1]) / len } else { 0.0 };
        (a + (b - a) * t.clamp(0.0, 1.0), (b - a).normalize_or_zero())
    }

    fn distance(&self, p: Vec2) -> f32 {
        self.pts
            .windows(2)
            .map(|w| {
                let e = w[1] - w[0];
                let t = ((p - w[0]).dot(e) / e.length_squared().max(1e-4)).clamp(0.0, 1.0);
                p.distance(w[0] + e * t)
            })
            .fold(f32::MAX, f32::min)
    }
}

fn dashed(ui: &mut Ui, a: Vec2, b: Vec2, dash: f32, gap: f32, phase: f32, t: f32, color: Color) {
    let len = a.distance(b);
    if len < 0.5 {
        return;
    }
    let d = (b - a) / len;
    let mut s = phase.rem_euclid(dash + gap) - (dash + gap);
    while s < len {
        let (s0, s1) = (s.max(0.0), (s + dash).min(len));
        if s1 > s0 {
            ui.stroke(a + d * s0, a + d * s1, t, color);
        }
        s += dash + gap;
    }
}

fn chart(ui: &mut Ui, state: &mut SurvivalState, area: Rect) {
    let Some(t) = state.maps.get(state.selected) else {
        return;
    };
    if state.preview_of != Some(state.selected) {
        ui.o.set_image(PREVIEW_SLOT, preview::SIZE, preview::SIZE, &preview::render(&t.map));
        state.preview_of = Some(state.selected);
    }
    let map = t.map.clone();
    let layout = t.layout.clone();
    let side = area.w.min(area.h - 84.0);
    let frame = Rect::new(area.x + (area.w - side) * 0.5, area.y, side, side);
    let origin = Vec2::new(frame.x, frame.y);
    let place = |p: (f32, f32)| origin + preview::locate(&map, [p.0, p.1], side);
    let shown = ui.ease(id("survival-chart-shown", state.selected), 1.0, 5.0);
    ui.fill(frame, ink(0.85));
    // A little darker than skirmish's chart: the fronts are what should read.
    let tone = 0.42 * shown;
    ui.image(PREVIEW_SLOT, [0.0, 0.0, preview::SIZE as f32, preview::SIZE as f32], frame, [tone, tone, tone, 1.0]);
    ui.fill(frame, ink(0.38));
    for k in 1..4 {
        let f = k as f32 / 4.0;
        ui.vline(frame.x + frame.w * f, frame.y, frame.h, rgb(palette::LINE, 0.06));
        ui.hline(frame.x, frame.y + frame.h * f, frame.w, rgb(palette::LINE, 0.06));
    }
    ui.frame(frame, rgb(palette::LINE, 0.25));
    ui.brackets(frame.inset(-6.0), 14.0, rgb(palette::ACCENT, 0.7));
    let size_m = map.info().size_metres().to_f32();
    let km_pts = frame.w / (size_m[0].max(size_m[1]) / 1000.0);
    let bar_km = if size_m[0] > 30_000.0 { 10.0 } else { 2.0 };
    ui.fill(Rect::new(frame.x + 14.0, frame.bottom() - 16.0, km_pts * bar_km, 2.0), rgb(palette::TEXT, 0.8));
    ui.text(frame.x + 14.0, frame.bottom() - 28.0, type_scale::MICRO, rgb(palette::TEXT, 0.8), &format!("{bar_km:.0} km"));
    ui.text_right(frame.right() - 12.0, frame.y + 16.0, type_scale::MICRO, rgb(palette::TEXT, 0.6), "N");
    ui.stroke(
        Vec2::new(frame.right() - 16.0, frame.y + 44.0),
        Vec2::new(frame.right() - 16.0, frame.y + 26.0),
        1.2,
        rgb(palette::TEXT, 0.6),
    );

    let starts = map.start_positions();
    let zone_world = |i: usize| -> Option<(f32, f32)> {
        let s = layout.spawns.get(i)?;
        let p = starts.get(s.start as usize)?.to_f32();
        Some((p[0], p[1]))
    };
    let you = zone_world(state.spawn).unwrap_or(layout.engine);
    let eng = place(layout.engine);

    // Pointer handling first, drawing after: markers sit on top of the fronts
    // and take the pointer before them.
    let mut tip: Option<Tip> = None;
    let mut spawn_res = Vec::new();
    let mut pick_spawn = None;
    state.markers.clear();
    for i in 0..layout.spawns.len() {
        let Some(w) = zone_world(i) else {
            spawn_res.push(Default::default());
            state.markers.push(Vec2::ZERO);
            continue;
        };
        let p = place(w);
        state.markers.push(p + ui.shift);
        let res = ui.interact(id("survival-zone", i), Rect::new(p.x - 18.0, p.y - 18.0, 36.0, 36.0), true);
        if res.clicked && i != state.spawn {
            pick_spawn = Some(i);
        }
        spawn_res.push(res);
    }
    let engine_res = ui.interact_with(id("survival-engine", 0), Rect::new(eng.x - 20.0, eng.y - 20.0, 40.0, 40.0), true, false);
    let harbor_res = layout
        .harbor
        .map(|h| ui.interact_with(id("survival-harbor", 0), Rect::new(place(h).x - 12.0, place(h).y - 12.0, 24.0, 24.0), true, false));
    let site_res: Vec<_> = layout
        .node_sites
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let p = place(n.at);
            ui.interact_with(id("survival-site", i), Rect::new(p.x - 9.0, p.y - 9.0, 18.0, 18.0), true, false)
        })
        .collect();

    // Keys 1-9 pick a zone.
    if ui.mem.editing.is_none() && ui.mem.popup.is_none() && ui.interactive {
        if let Some(d) = ui.input.typed.chars().filter_map(|c| c.to_digit(10)).next() {
            let i = d as usize;
            if (1..=layout.spawns.len()).contains(&i) && i - 1 != state.spawn {
                pick_spawn = Some(i - 1);
            }
        }
    }

    // The fronts, and which one the pointer is on.
    let routes: Vec<Route> = layout
        .fronts
        .iter()
        .map(|f| {
            let mut pts = vec![eng];
            if f.domain == Domain::Naval {
                if let Some(h) = layout.harbor {
                    if place(h).distance(place(f.path[0])) > 1.0 {
                        pts.push(place(h));
                    }
                }
            }
            let first = pts.len();
            pts.extend(f.path.iter().map(|&p| place(p)));
            let last = pts.len() - 1;
            pts.push(place(you));
            let mut metres = 0.0;
            let world: Vec<Vec2> = f.path.iter().map(|&(x, y)| Vec2::new(x, y)).chain([Vec2::new(you.0, you.1)]).collect();
            for w in world.windows(2) {
                metres += w[0].distance(w[1]);
            }
            Route::new(pts, first, last, metres)
        })
        .collect();
    let over_marker = spawn_res.iter().any(|r| r.hovered) || engine_res.hovered || harbor_res.is_some_and(|r| r.hovered) || site_res.iter().any(|r| r.hovered);
    let cursor = ui.cursor - ui.shift;
    let hovered_front = (ui.interactive && ui.mem.popup.is_none() && !over_marker && frame.contains(cursor))
        .then(|| {
            routes
                .iter()
                .enumerate()
                .map(|(i, r)| (i, r.distance(cursor)))
                .filter(|(_, d)| *d < 8.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        })
        .flatten();

    let tick = ui.time;
    for (i, (f, route)) in layout.fronts.iter().zip(&routes).enumerate() {
        let on = state.rules.has(f.domain);
        let on_k = ui.ease(id("survival-front-on", i), if on { 1.0 } else { 0.0 }, 8.0);
        let lit = hovered_front == Some(i) || state.hover_domain == Some(f.domain);
        let lit_k = ui.ease(id("survival-front-lit", i), if lit { 1.0 } else { 0.0 }, 14.0);
        let base = 0.16 + 0.34 * on_k + 0.4 * lit_k;
        let width = 1.3 + 1.2 * lit_k;
        if lit_k > 0.01 {
            for w in route.pts.windows(2) {
                ui.stroke(w[0], w[1], 8.0, dcol(f.domain, 0.12 * lit_k));
            }
        }
        for (k, w) in route.pts.windows(2).enumerate() {
            let leg = k >= route.last;
            let stub = k < route.first;
            let a = if stub { base * 0.5 } else { base };
            let c = dcol(f.domain, a);
            let phase = if on { tick * 8.0 } else { 0.0 };
            match (f.domain, leg) {
                (_, true) => dashed(ui, w[0], w[1], 5.0, 5.0, phase, width, c),
                (Domain::Air, false) => dashed(ui, w[0], w[1], 12.0, 6.0, phase, width, c),
                (Domain::Naval, false) => {
                    let n = (w[1] - w[0]).normalize_or_zero().perp() * 1.8;
                    ui.stroke(w[0] + n, w[1] + n, width * 0.8, c);
                    ui.stroke(w[0] - n, w[1] - n, width * 0.8, c);
                }
                (Domain::Land, false) => ui.stroke(w[0], w[1], width, c),
            }
        }
        // Chevrons flowing from the engine toward your zone.
        let total = route.total();
        let spacing = 26.0;
        let speed = match f.domain {
            Domain::Land => 16.0,
            Domain::Air => 30.0,
            Domain::Naval => 11.0,
        };
        let offset = if on { (tick * speed).rem_euclid(spacing) } else { spacing * 0.5 };
        let mut s = offset;
        while s < total {
            let (p, d) = route.sample(s);
            let n = d.perp();
            let fade = (s / 30.0).min(1.0) * ((total - s) / 30.0).clamp(0.0, 1.0);
            let a = if on { (0.55 + 0.45 * lit_k) * fade } else { 0.14 * fade } * (0.25 + 0.75 * on_k.max(0.3));
            let size = 4.0 + 1.5 * lit_k;
            ui.stroke(p - d * size + n * size, p, 1.6, dcol(f.domain, a));
            ui.stroke(p - d * size - n * size, p, 1.6, dcol(f.domain, a));
            s += spacing;
        }
        // A tag half way down the path: the domain glyph and the front's name.
        let mid_s = (route.at[route.first] + route.at[route.last]) * 0.5;
        let (p, _) = route.sample(mid_s);
        let ta = 0.45 + 0.4 * on_k + 0.15 * lit_k;
        ui.disc(p, 9.0 + 1.5 * lit_k, ink(0.85));
        ui.arc(p, 9.0 + 1.5 * lit_k, 0.0, TAU, 1.2, dcol(f.domain, ta));
        domain_glyph(ui, f.domain, p, 5.0, dcol(f.domain, ta));
        let (st, name) = ui.fitted(type_scale::MICRO, &f.name, 130.0);
        let nw = ui.text_width(st, &name);
        let lx = if p.x + 14.0 + nw > frame.right() - 6.0 { p.x - 14.0 - nw } else { p.x + 14.0 };
        ui.fill(Rect::new(lx - 4.0, p.y - 8.0, nw + 8.0, 16.0), ink(0.55 * ta));
        ui.text(lx, p.y, st, rgb(palette::TEXT, ta), &name);
        if hovered_front == Some(i) {
            let zone = layout.spawns.get(state.spawn).map_or("your zone", |s| s.name.as_str());
            let what = match f.domain {
                Domain::Land => "Ground forces march this road",
                Domain::Air => "Aircraft fly this corridor",
                Domain::Naval => "Ships sail this lane out of the harbor",
            };
            tip = Some(Tip {
                at: cursor,
                title: f.name.clone(),
                accent: dcol(f.domain, 1.0),
                lines: vec![
                    format!("{} front  \u{b7}  {} to {zone}", f.domain.label(), km(route.metres)),
                    if on { what.to_owned() } else { format!("Off: no {} attacks this match", f.domain.label().to_lowercase()) },
                ],
                glow: lit_k.max(0.6),
            });
        }
    }

    // Node sites.
    let nodes_on = state.rules.nodes > 0;
    let nodes_k = ui.ease(id("survival-nodes-on", 0), if nodes_on { 1.0 } else { 0.0 }, 8.0);
    for (i, (n, res)) in layout.node_sites.iter().zip(&site_res).enumerate() {
        let p = place(n.at);
        let a = 0.35 + 0.55 * nodes_k + 0.1 * res.glow;
        let fill = if n.domain == Domain::Naval { dcol(Domain::Naval, 0.75 * a) } else { engine(0.35 * a) };
        diamond(ui, p, 5.0 + 1.5 * res.glow, fill, engine(a));
        if res.hovered {
            tip = Some(Tip {
                at: p,
                title: n.name.clone(),
                accent: engine(1.0),
                lines: vec![
                    format!("{} node site", n.domain.label()),
                    if nodes_on {
                        "The engine may raise a replication node here".to_owned()
                    } else {
                        "Replication nodes are off".to_owned()
                    },
                ],
                glow: res.glow,
            });
            let _ = i;
        }
    }

    // Harbor.
    if let (Some(h), Some(res)) = (layout.harbor, harbor_res) {
        let p = place(h);
        let a = if state.rules.has(Domain::Naval) { 0.95 } else { 0.4 };
        ui.disc(p, 9.0, ink(0.85));
        ui.arc(p, 9.0, 0.0, TAU, 1.2, dcol(Domain::Naval, a));
        anchor(ui, p, 5.5, dcol(Domain::Naval, a));
        if res.hovered {
            tip = Some(Tip {
                at: p,
                title: "Harbor".into(),
                accent: dcol(Domain::Naval, 1.0),
                lines: vec!["The engine's slipways: every ship it prints launches here".into()],
                glow: res.glow,
            });
        }
    }

    // The engine.
    engine_mark(ui, eng, 11.0 + 1.5 * engine_res.glow, 1.0, true);
    let (lx, ly) = (eng.x, eng.y - 40.0);
    let label = "Replication Engine";
    let lw = ui.text_width(type_scale::MICRO, label);
    let lx = (lx - lw * 0.5).clamp(frame.x + 6.0, frame.right() - lw - 6.0);
    let ly = if ly < frame.y + 12.0 { eng.y + 42.0 } else { ly };
    ui.fill(Rect::new(lx - 5.0, ly - 8.0, lw + 10.0, 16.0), ink(0.7));
    ui.text(lx, ly, type_scale::MICRO, engine(1.0), label);
    if engine_res.hovered {
        tip = Some(Tip {
            at: eng,
            title: "Replication Engine".into(),
            accent: engine(1.0),
            lines: vec!["Prints every round's attack in its bays and sends it down the fronts. It cannot be destroyed: outlast it.".into()],
            glow: engine_res.glow,
        });
    }

    // Landing zones.
    let mine = TEAM_COLORS[0];
    for (i, res) in spawn_res.iter().enumerate() {
        let Some(w) = zone_world(i) else { continue };
        let p = place(w);
        let chosen = i == state.spawn;
        let sel = ui.ease(id("survival-zone-sel", i), if chosen { 1.0 } else { 0.0 }, 10.0);
        let color = if chosen { [mine[0], mine[1], mine[2], 1.0] } else { rgb(palette::TEXT, 0.75 + 0.25 * res.glow) };
        let r = 13.0 + 2.0 * res.glow + 2.0 * sel;
        ui.disc(p, r, ink(0.85));
        ui.arc(p, r, 0.0, TAU, 1.2 + 1.2 * sel, color);
        if chosen {
            let pulse = (ui.time * 0.8).fract();
            ui.arc(p, r + 1.0 + 18.0 * pulse, 0.0, TAU, 1.4, [mine[0], mine[1], mine[2], 0.8 * (1.0 - pulse)]);
            // Corner ticks: the zone you hold.
            ui.brackets(Rect::new(p.x - r - 7.0, p.y - r - 7.0, 2.0 * r + 14.0, 2.0 * r + 14.0), 5.0, [mine[0], mine[1], mine[2], 0.9 * sel]);
        }
        ui.text_centred(p.x + 0.5, p.y, type_scale::VALUE, rgb(palette::TEXT, 1.0), &format!("{}", i + 1));
        let name = &layout.spawns[i].name;
        let nw = ui.text_width(type_scale::CAPTION, name);
        let ny = p.y - r - 14.0;
        ui.fill(Rect::new(p.x - nw * 0.5 - 5.0, ny - 8.0, nw + 10.0, 16.0), ink(0.6));
        ui.text_centred(p.x, ny, type_scale::CAPTION, if chosen { color } else { rgb(palette::TEXT, 0.8) }, name);
        if res.glow > 0.05 {
            tip = Some(Tip {
                at: p,
                title: name.clone(),
                accent: if chosen { [mine[0], mine[1], mine[2], 1.0] } else { rgb(palette::ACCENT, 1.0) },
                lines: vec![
                    layout.spawns[i].blurb.clone(),
                    if chosen { "Your landing zone".into() } else { format!("Click or press {} to deploy here", i + 1) },
                ],
                glow: res.glow,
            });
        }
    }
    if let Some(i) = pick_spawn {
        state.spawn = i;
        ui.audio.play(Sfx::Select);
    }
    if let Some(t) = &tip {
        draw_tip(ui, t, frame);
    }

    // Under the chart: the theatre, where you deploy, and the legend.
    let y = frame.bottom() + 26.0;
    let end = ui.text(frame.x, y, type_scale::ITEM, rgb(palette::TEXT, 1.0), &map.name());
    let end = ui.text(end + 14.0, y + 1.0, type_scale::MICRO, rgb(palette::DIM, 1.0), &format!("{:.1} \u{d7} {:.1} km", size_m[0] / 1000.0, size_m[1] / 1000.0));
    if let Some(s) = layout.spawns.get(state.spawn) {
        let x = end + 22.0;
        let w = frame.right() - x;
        let head = format!("Deploying at {}  \u{b7}  ", s.name);
        let hw = ui.text_width(type_scale::MICRO, &head);
        ui.text(x, y + 1.0, type_scale::MICRO, [mine[0], mine[1], mine[2], 1.0], &head);
        ui.text_fit_left(x + hw, y + 1.0, (w - hw).max(0.0), type_scale::MICRO, rgb(palette::DIM, 1.0), &s.blurb);
    }
    legend(ui, frame.x, y + 30.0, frame.w);
}

fn legend(ui: &mut Ui, x: f32, y: f32, w: f32) {
    let mut cx = x;
    let item = |ui: &mut Ui, cx: &mut f32, label: &str, draw: &dyn Fn(&mut Ui, Vec2)| {
        draw(ui, Vec2::new(*cx + 8.0, y));
        *cx = ui.text(*cx + 22.0, y, type_scale::MICRO, rgb(palette::DIM, 1.0), label) + 20.0;
    };
    item(ui, &mut cx, "Engine", &|ui, c| engine_mark(ui, c, 6.0, 1.0, false));
    for d in Domain::ALL {
        let label = format!("{} Front", d.label());
        item(ui, &mut cx, &label, &|ui, c| {
            ui.stroke(c - Vec2::X * 9.0, c + Vec2::X * 9.0, 1.4, dcol(d, 0.5));
            ui.stroke(c + Vec2::new(-1.0, -4.0), c + Vec2::new(3.0, 0.0), 1.6, dcol(d, 1.0));
            ui.stroke(c + Vec2::new(-1.0, 4.0), c + Vec2::new(3.0, 0.0), 1.6, dcol(d, 1.0));
        });
    }
    item(ui, &mut cx, "Node Site", &|ui, c| diamond(ui, c, 5.0, engine(0.35), engine(0.9)));
    item(ui, &mut cx, "Harbor", &|ui, c| anchor(ui, c, 6.0, dcol(Domain::Naval, 1.0)));
    let mine = TEAM_COLORS[0];
    item(ui, &mut cx, "Landing Zone", &|ui, c| ui.arc(c, 6.0, 0.0, TAU, 1.8, [mine[0], mine[1], mine[2], 1.0]));
    let _ = w;
}

// -- right: engagement and forecast -------------------------------------------------

const ROUNDS: [u16; 7] = [5, 10, 15, 20, 30, 50, 0];
const ROUND_LABELS: [&str; 7] = ["5", "10", "15", "20", "30", "50", "Endless"];
const INTENSITY: [(&str, u16); 5] = [("Skirmish", 600), ("Standard", 1000), ("Siege", 1500), ("Onslaught", 2200), ("Annihilation", 3200)];
const GRACE: [(&str, u16); 4] = [("2 min", 120), ("4 min", 240), ("6 min", 360), ("8 min", 480)];
const INTERVAL: [(&str, u16); 3] = [("1.5 min", 90), ("2.5 min", 150), ("4 min", 240)];
const NODES: [&str; 4] = ["Off", "Rare", "Regular", "Frequent"];
const TIER_NAMES: [&str; 5] = ["Light", "Main Line", "Heavy", "Experimental", "Apex"];

fn nearest(values: impl Iterator<Item = u16>, v: u16) -> usize {
    values
        .enumerate()
        .min_by_key(|(_, x)| (*x as i32 - v as i32).abs())
        .map_or(0, |(i, _)| i)
}

const VALUE_W: f32 = 200.0;
const PITCH: f32 = 40.0;

fn pick_row(ui: &mut Ui, key: &str, r: Rect, label: &str, hint: &str, options: &[&str], selected: usize) -> Option<usize> {
    label_row(ui, r, label, hint);
    ui.dropdown(id(key, 0), Rect::new(r.right() - VALUE_W, r.mid_y() - 15.0, VALUE_W, 30.0), options, selected, true)
}

fn engagement(ui: &mut Ui, state: &mut SurvivalState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "Engagement");
    let row = |k: f32| Rect::new(area.x, area.y + 22.0 + k * PITCH, area.w, PITCH - 4.0);
    let rules = &mut state.rules;

    let at = ROUNDS.iter().position(|r| *r == rules.rounds).unwrap_or_else(|| nearest(ROUNDS[..6].iter().copied(), rules.rounds));
    let hint = if rules.rounds == 0 { "Until you fall" } else { "Survive them all to win" };
    if let Some(i) = pick_row(ui, "sv-rounds", row(0.0), "Rounds", hint, &ROUND_LABELS, at) {
        rules.rounds = ROUNDS[i];
    }
    let at = nearest(INTENSITY.iter().map(|l| l.1), rules.intensity);
    let hint = format!("\u{d7}{:.1} round size", rules.intensity as f32 / 1000.0);
    let labels = INTENSITY.map(|l| l.0);
    if let Some(i) = pick_row(ui, "sv-intensity", row(1.0), "Intensity", &hint, &labels, at) {
        rules.intensity = INTENSITY[i].1;
    }
    let at = nearest(GRACE.iter().map(|l| l.1), rules.grace_secs);
    let labels = GRACE.map(|l| l.0);
    if let Some(i) = pick_row(ui, "sv-grace", row(2.0), "First Contact", "Time to build before round 1", &labels, at) {
        rules.grace_secs = GRACE[i].1;
    }
    let at = nearest(INTERVAL.iter().map(|l| l.1), rules.interval_secs);
    let labels = INTERVAL.map(|l| l.0);
    if let Some(i) = pick_row(ui, "sv-interval", row(3.0), "Between Rounds", "", &labels, at) {
        rules.interval_secs = INTERVAL[i].1;
    }
    // The economy, said plainly: the waves are the income.
    let r = row(4.0);
    ui.fill(Rect::new(r.x, r.y + 4.0, 2.0, r.h - 8.0), rgb(crate::hud::MASS, 0.8));
    let note = "No mass is handed out: every unit the engine sends leaves a wreck worth most of its cost - reclaim the field.";
    for (k, l) in wrap(ui, type_scale::MICRO, note, r.w - 24.0).iter().take(2).enumerate() {
        ui.text(r.x + 14.0, r.y + 11.0 + k as f32 * 15.0, type_scale::MICRO, rgb(palette::DIM, 1.0), l);
    }

    // Fronts: a chip per domain, with the theatre's lanes of it.
    let y = area.y + 22.0 + 5.0 * PITCH + 14.0;
    ui.text(area.x + 2.0, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Attack Fronts");
    ui.text_right(area.right(), y, type_scale::MICRO, rgb(palette::FAINT, 1.0), "Hover to see the lanes");
    let counts = state.counts();
    let gap = 10.0;
    let cw = (area.w - 2.0 * gap) / 3.0;
    state.hover_domain = None;
    for (k, d) in Domain::ALL.into_iter().enumerate() {
        let r = Rect::new(area.x + k as f32 * (cw + gap), y + 12.0, cw, 50.0);
        let lanes = counts[k];
        let on = state.rules.has(d) && lanes > 0;
        let res = ui.tile(id("sv-front", k), r, on, lanes > 0);
        if res.hovered {
            state.hover_domain = Some(d);
        }
        if res.clicked {
            if state.toggle_front(d) {
                ui.audio.play(if state.rules.has(d) { Sfx::ToggleOn } else { Sfx::ToggleOff });
            } else {
                ui.audio.play(Sfx::Deny);
            }
        }
        let a = if lanes == 0 { 0.3 } else if on { 1.0 } else { 0.5 };
        ui.fill(Rect::new(r.x + 1.0, r.y + 7.0, 2.0, r.h - 14.0), dcol(d, a));
        ui.gradient_h(Rect::new(r.x + 3.0, r.y + 3.0, r.w * 0.6, r.h - 6.0), dcol(d, 0.16 * a * res.glow.max(on as u8 as f32)), dcol(d, 0.0));
        domain_glyph(ui, d, Vec2::new(r.x + 24.0, r.mid_y() - 2.0), 9.0, dcol(d, a));
        ui.text(r.x + 44.0, r.y + 18.0, type_scale::ITEM, rgb(palette::TEXT, 0.35 + 0.65 * a), d.label());
        let sub = match (lanes, on) {
            (0, _) => "No lanes here".to_owned(),
            (n, true) => format!("{n} lane{}", if n == 1 { "" } else { "s" }),
            (_, false) => "Off".to_owned(),
        };
        ui.text(r.x + 44.0, r.y + 36.0, type_scale::MICRO, rgb(if on { palette::DIM } else { palette::FAINT }, 1.0), &sub);
        // A check box in the corner.
        let b = Rect::new(r.right() - 22.0, r.y + 10.0, 12.0, 12.0);
        ui.frame(b, rgb(palette::LINE, 0.4 * a));
        if on {
            ui.fill(b.inset(3.0), dcol(d, 1.0));
        }
    }

    // Tech ceiling: five tiers, planned beyond what has units.
    let y = y + 12.0 + 50.0 + 22.0;
    ui.text(area.x + 2.0, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Tech Ceiling");
    let cap = state.rules.tier_cap.clamp(1, 5);
    let note = if cap > UNITS_TOP {
        format!("Beyond T{UNITS_TOP} the engine prints its best T{UNITS_TOP} until more exist")
    } else {
        "The engine climbs to it over the rounds".to_owned()
    };
    ui.text_right(area.right(), y, type_scale::MICRO, rgb(if cap > UNITS_TOP { palette::WARN } else { palette::FAINT }, 1.0), &note);
    let gap = 8.0;
    let tw = (area.w - 4.0 * gap) / 5.0;
    for k in 0..5u8 {
        let tier = k + 1;
        let r = Rect::new(area.x + k as f32 * (tw + gap), y + 12.0, tw, 56.0);
        let reached = tier <= cap;
        let res = ui.tile(id("sv-tier", k as usize), r, tier == cap, true);
        if res.clicked && tier != cap {
            state.rules.tier_cap = tier;
            ui.audio.play(Sfx::Tick);
        }
        let empty = tier > UNITS_TOP;
        let tone = if reached { palette::ACCENT } else { palette::FAINT };
        // The ladder: every tier up to the ceiling is lit.
        ui.fill(Rect::new(r.x + 6.0, r.y + 4.0, r.w - 12.0, 2.0), rgb(tone, if reached { 0.85 } else { 0.3 }));
        ui.text(r.x + 12.0, r.y + 22.0, type_scale::ITEM, rgb(if reached { 0xFFFFFF } else { palette::DIM }, 0.85 + 0.15 * res.glow), &format!("T{tier}"));
        for p in 0..tier {
            ui.fill(Rect::new(r.right() - 12.0 - (tier - p) as f32 * 6.0, r.y + 18.0, 4.0, 8.0), rgb(tone, if reached { 0.9 } else { 0.35 }));
        }
        let (label, c) = if empty { ("No units yet", rgb(palette::WARN, if reached { 0.9 } else { 0.5 })) } else { (TIER_NAMES[k as usize], rgb(palette::DIM, 1.0)) };
        ui.text_fit_left(r.x + 12.0, r.y + 41.0, r.w - 18.0, type_scale::MICRO, c, label);
    }

    // Replication nodes.
    let y = y + 12.0 + 56.0 + 12.0;
    let r = Rect::new(area.x, y, area.w, PITCH - 4.0);
    let hint = match state.rules.nodes {
        0 => "None".to_owned(),
        n => format!("Up to {} standing", state.rules.node_limit().max(n as usize)),
    };
    if let Some(i) = pick_row(ui, "sv-nodes", r, "Replication Nodes", &hint, &NODES, state.rules.nodes.min(3) as usize) {
        state.rules.nodes = i as u8;
    }
    ui.text_fit_left(
        area.x + 16.0,
        r.bottom() + 14.0,
        area.w - 16.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "The engine raises nodes that print one unit type each - a destroyed node leaves a rich wreck to reclaim",
    );

    let y = r.bottom() + 44.0;
    forecast(ui, state, Rect::new(area.x, y, area.w, area.bottom() - y));
}

/// One bar per round (sampled when there are many): the engine's budget split
/// over the attacking domains, tier bands behind, a diamond where a node rises.
fn forecast(ui: &mut Ui, state: &mut SurvivalState, area: Rect) {
    ui.section(area.x, area.y, area.w, "Forecast");
    let rules = state.rules;
    let endless = rules.rounds == 0;
    let total = if endless { 30 } else { rules.rounds };
    let n = total.min(30) as usize;
    let round_of = |i: usize| -> u16 {
        if n <= 1 || total as usize == n {
            i as u16 + 1
        } else {
            1 + ((i as f32 * (total - 1) as f32 / (n - 1) as f32).round() as u16)
        }
    };
    let attacking = state.attacking();
    let weight = |d: Domain| match d {
        Domain::Land => 60,
        Domain::Air => 25,
        Domain::Naval => 20,
    };
    let wsum: i64 = attacking.iter().map(|d| weight(*d)).sum::<i64>().max(1);
    let peak = (0..n).map(|i| rules.budget(round_of(i))).max().unwrap_or(1).max(1);

    let plot = Rect::new(area.x + 34.0, area.y + 50.0, area.w - 34.0, area.h - 50.0 - 44.0);
    // Axis and grid.
    for k in 0..=3 {
        let y = plot.bottom() - 0.9 * plot.h * k as f32 / 3.0;
        ui.hline(plot.x, y, plot.w, rgb(palette::LINE, if k == 0 { 0.3 } else { 0.06 }));
        if k > 0 {
            ui.text_right(plot.x - 6.0, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), &mass(peak * k / 3));
        }
    }
    let slot = plot.w / n as f32;
    let bw = (slot * 0.68).max(2.0);

    // Tier bands behind the bars.
    let mut i0 = 0;
    while i0 < n {
        let tier = rules.tier_at(round_of(i0));
        let mut i1 = i0;
        while i1 + 1 < n && rules.tier_at(round_of(i1 + 1)) == tier {
            i1 += 1;
        }
        let band = Rect::new(plot.x + i0 as f32 * slot, plot.y - 18.0, (i1 - i0 + 1) as f32 * slot, plot.h + 18.0);
        ui.fill(band, rgb(0xFFFFFF, 0.012 + 0.018 * tier as f32));
        if tier > UNITS_TOP {
            // Hatched: planned, but printed at the best tier that has units.
            let mut x = band.x - band.h;
            while x < band.right() {
                let (a, b) = (Vec2::new(x, band.bottom()), Vec2::new(x + band.h, band.y));
                let clip = |p: Vec2, q: Vec2| -> Option<(Vec2, Vec2)> {
                    let d = q - p;
                    let t0 = ((band.x - p.x) / d.x).max(0.0);
                    let t1 = ((band.right() - p.x) / d.x).min(1.0);
                    (t1 > t0).then(|| (p + d * t0, p + d * t1))
                };
                if let Some((a, b)) = clip(a, b) {
                    ui.stroke(a, b, 1.0, rgb(palette::WARN, 0.07));
                }
                x += 9.0;
            }
        }
        if i0 > 0 {
            ui.vline(band.x, band.y, band.h, rgb(palette::LINE, 0.18));
        }
        let label = if tier > UNITS_TOP { format!("T{tier} \u{b7} as T{UNITS_TOP}") } else { format!("T{tier}") };
        ui.text_fit_left(
            band.x + 5.0,
            band.y + 8.0,
            band.w - 8.0,
            type_scale::MICRO,
            rgb(if tier > UNITS_TOP { palette::WARN } else { palette::TEXT }, 0.75),
            &label,
        );
        i0 = i1 + 1;
    }

    // Bars.
    let cursor = ui.cursor - ui.shift;
    let over = (ui.interactive && ui.mem.popup.is_none() && Rect::new(plot.x, plot.y - 18.0, plot.w, plot.h + 18.0).contains(cursor))
        .then(|| (((cursor.x - plot.x) / slot) as usize).min(n - 1));
    for i in 0..n {
        let round = round_of(i);
        let budget = rules.budget(round);
        let target = 0.9 * budget as f32 / peak as f32;
        let k = ui.ease(id("sv-bar", i), target, 9.0);
        let x = plot.x + i as f32 * slot + (slot - bw) * 0.5;
        let hot = over == Some(i);
        if hot {
            ui.fill(Rect::new(plot.x + i as f32 * slot, plot.y - 18.0, slot, plot.h + 18.0), rgb(palette::ACCENT, 0.08));
        }
        let mut y = plot.bottom();
        let h_total = plot.h * k;
        for d in &attacking {
            let h = h_total * weight(*d) as f32 / wsum as f32;
            ui.fill(Rect::new(x, y - h, bw, h), dcol(*d, if hot { 0.95 } else { 0.72 }));
            y -= h;
        }
        ui.fill(Rect::new(x, y - 1.0, bw, 1.5), rgb(0xFFFFFF, if hot { 0.9 } else { 0.45 }));
        if rules.raises_node(round) || (total as usize != n && (round_of(i.saturating_sub(1))..=round).skip(1).any(|r| rules.raises_node(r))) {
            let s = (bw * 0.35).clamp(2.5, 4.5);
            diamond(ui, Vec2::new(x + bw * 0.5, y - 6.0 - s), s, engine(0.9), engine(1.0));
        }
    }
    // Round numbers under the axis.
    let label_every = if n > 20 { 5 } else if n > 10 { 2 } else { 1 };
    for i in 0..n {
        let r = round_of(i);
        if i == 0 || i == n - 1 || (r as usize) % label_every == 0 && i != n - 2 {
            ui.text_centred(plot.x + (i as f32 + 0.5) * slot, plot.bottom() + 11.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), &r.to_string());
        }
    }

    // The summary, or the hovered round.
    let y = area.bottom() - 12.0;
    let (max_tier, nodes) = if endless {
        (rules.tier_cap.min(5), (1..=30).filter(|r| rules.raises_node(*r)).count())
    } else {
        ((1..=total).map(|r| rules.tier_at(r)).max().unwrap_or(1), (1..=total).filter(|r| rules.raises_node(*r)).count())
    };
    let text = match over {
        Some(i) => {
            let round = round_of(i);
            let budget = rules.budget(round);
            let parts: Vec<String> = attacking.iter().map(|d| format!("{} {}", d.label(), mass(budget * weight(*d) / wsum))).collect();
            format!(
                "Round {round}  \u{b7}  T{}  \u{b7}  {} mass  \u{b7}  {}{}",
                rules.tier_at(round),
                mass(budget),
                parts.join("  "),
                if rules.raises_node(round) { "  \u{b7}  Node rises" } else { "" }
            )
        }
        None if endless => format!(
            "Endless  \u{b7}  a tier every 6 rounds  \u{b7}  T{} by round {}  \u{b7}  {} nodes in 30 rounds",
            rules.tier_cap.clamp(1, 5),
            1 + 6 * (rules.tier_cap.clamp(1, 5) as u32 - 1),
            nodes
        ),
        None => {
            let secs = rules.grace_secs as u32 + total as u32 * rules.interval_secs as u32;
            format!(
                "{total} rounds  \u{b7}  ~{} min  \u{b7}  tops out at T{max_tier}  \u{b7}  {nodes} node{}  \u{b7}  last wave {} mass",
                (secs + 30) / 60,
                if nodes == 1 { "" } else { "s" },
                mass(rules.budget(total))
            )
        }
    };
    let tone = if over.is_some() { rgb(palette::TEXT, 1.0) } else { rgb(palette::DIM, 1.0) };
    ui.text_fit_left(area.x, y, area.w, type_scale::CAPTION, tone, &text);
    // Key for the stack colours.
    let mut x = area.right();
    for d in attacking.iter().rev() {
        let w = ui.text_width(type_scale::MICRO, d.label());
        x -= w;
        ui.text(x, area.y + 22.0, type_scale::MICRO, rgb(palette::DIM, 1.0), d.label());
        ui.fill(Rect::new(x - 12.0, area.y + 18.0, 8.0, 8.0), dcol(*d, 0.85));
        x -= 26.0;
    }
    let _ = max_tier;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Input, Memory};
    use mc_render::Overlay;

    fn state() -> SurvivalState {
        let state = SurvivalState::new(&Settings::default());
        assert!(!state.maps.is_empty(), "bake a survival map (crucible) so survival set-up can be tested");
        state
    }

    fn frame(state: &mut SurvivalState, overlay: &mut Overlay, memory: &mut Memory, input: &Input) -> Option<SurvivalAction> {
        let audio = crate::audio::Audio::silent();
        memory.begin_frame();
        let mut ui = Ui::new(overlay, input, memory, &audio, Vec2::new(1920.0, 1080.0), 1.0, 1.0, 1.0 / 30.0);
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
            Input { cursor: at, ..Default::default() },
            Input { cursor: at, down: true, pressed: true, ..Default::default() },
            Input { cursor: at, released: true, ..Default::default() },
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
        let present: Vec<Domain> = Domain::ALL.into_iter().filter(|d| counts[domain_index(*d)] > 0).collect();
        for d in &present[..present.len() - 1] {
            if state.rules.has(*d) {
                assert!(state.toggle_front(*d));
            }
        }
        let last = *present.last().unwrap();
        assert_eq!(state.attacking(), vec![last]);
        assert!(!state.toggle_front(last), "turning off the last front is refused");
        assert_eq!(state.attacking(), vec![last]);
        assert!(state.problem().is_none());
    }
}
