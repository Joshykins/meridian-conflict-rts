//! Skirmish set-up: pick a map, seat the commanders, set the rules, launch.

use super::{ink, id, palette, preview, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::setup::{self, TEAM_COLORS};
use glam::Vec2;
use mc_map::MapFile;
use mc_sim::tables::Controller;
use mc_sim::{MatchConfig, PlayerSetup};
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
}

/// Everything needed to start the match the player configured.
pub struct MatchRequest {
    pub map: Arc<MapFile>,
    pub config: MatchConfig,
    /// Player colours by player index, linear RGB.
    pub colors: [[f32; 3]; 8],
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
    slots: Vec<Slot>,
    pub fog: bool,
    seed: u64,
    pub name: String,
}

fn fresh_seed() -> u64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    // SplitMix64: consecutive clock readings give unrelated seeds.
    let mut z = nanos.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) & 0xFFFF_FFFF
}

impl SkirmishState {
    /// Opens every map in `maps/`. `preferred` is the stem of the map to select.
    pub fn new(preferred: &str, fog: bool, name: &str) -> SkirmishState {
        let mut maps: Vec<MapEntry> = setup::list_maps()
            .into_iter()
            .filter_map(|path| match MapFile::open(&path) {
                Ok(map) => Some(MapEntry { stem: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), map: Arc::new(map) }),
                Err(e) => {
                    log::warn!("{}: {e}; not listed", path.display());
                    None
                }
            })
            .collect();
        // Smallest first: the quick 1v1 maps lead the list.
        maps.sort_by_key(|m| (m.map.info().tile_count(), m.stem.clone()));
        let selected = maps.iter().position(|m| m.stem == preferred).unwrap_or(0);
        let mut state = SkirmishState { maps, selected, preview_of: None, slots: Vec::new(), fog, seed: fresh_seed(), name: name.to_owned() };
        state.seat_for_map();
        state
    }

    pub fn selected_stem(&self) -> &str {
        self.maps.get(self.selected).map_or("", |m| m.stem.as_str())
    }

    /// You, one AI opponent, and the rest of the map's start positions closed.
    fn seat_for_map(&mut self) {
        let starts = self.maps.get(self.selected).map_or(0, |m| m.map.start_positions().len()).min(8);
        self.slots = (0..starts)
            .map(|i| Slot { seat: [Seat::You, Seat::Ai].get(i).copied().unwrap_or(Seat::Closed), team: i as u8, start: i as u8, color: i as u8 })
            .collect();
    }

    fn taken<T: PartialEq>(&self, except: usize, field: impl Fn(&Slot) -> T, value: T) -> bool {
        self.slots.iter().enumerate().any(|(i, s)| i != except && s.seat != Seat::Closed && field(s) == value)
    }

    /// Steps slot `i`'s start or colour to the next value nobody else is using.
    fn step_unique(&mut self, i: usize, by: i32, count: usize, get: impl Fn(&Slot) -> u8, set: impl Fn(&mut Slot, u8)) {
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
        let active: Vec<&Slot> = self.slots.iter().filter(|s| s.seat != Seat::Closed).collect();
        if self.maps.is_empty() {
            Some("NO MAPS FOUND - BAKE ONE WITH MC-BAKE")
        } else if active.len() < 2 {
            Some("A MATCH NEEDS AN OPPONENT")
        } else if active.iter().all(|s| s.team == active[0].team) {
            Some("EVERYONE IS ON THE SAME TEAM")
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
            let (name, controller) = match s.seat {
                Seat::You => (self.name.clone(), Controller::Human),
                _ => {
                    ai += 1;
                    (format!("ARC AI {ai}"), Controller::Ai)
                }
            };
            players.push(PlayerSetup { name, faction: "Aster".into(), team: s.team, controller, start: s.start });
        }
        MatchRequest { map: entry.map.clone(), config: MatchConfig { seed: self.seed, players, cheats: false, fog: self.fog, spawn_commanders: true }, colors }
    }
}

/// The match the set-up screen would start if opened and confirmed untouched.
pub fn default_request(settings: &crate::settings::Settings) -> Option<MatchRequest> {
    let state = SkirmishState::new(&settings.skirmish_map, settings.skirmish_fog, &settings.player_name);
    state.problem().is_none().then(|| state.request())
}

pub fn draw(ui: &mut Ui, state: &mut SkirmishState, enter: f32) -> Option<SkirmishAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.66 * enter));
    ui.scrim(Rect::new(0.0, 0.0, w, 220.0), 0.6 * enter, 0.0, false);
    ui.fade = enter;
    ui.shift.y = 14.0 * (1.0 - enter);

    // Header.
    ui.reticle(Vec2::new(LEFT + 15.0, 84.0), 13.0, rgb(palette::TEXT, 0.9));
    let end = ui.text(LEFT + 50.0, 84.0, type_scale::TITLE, rgb(0xFFFFFF, 1.0), "SKIRMISH");
    ui.text(end + 18.0, 90.0, type_scale::CAPTION, rgb(palette::DIM, 1.0), "CONFIGURE THE ENGAGEMENT");
    ui.fill(Rect::new(LEFT, 124.0, 58.0, 2.0), rgb(palette::ACCENT, 1.0));
    ui.gradient_h(Rect::new(LEFT + 66.0, 124.0, w - 2.0 * LEFT - 66.0, 1.0), rgb(palette::LINE, 0.35), rgb(palette::LINE, 0.04));

    // Columns: maps and rules | preview | commanders.
    let (top, bottom) = (160.0, h - 172.0);
    let (left_w, right_w, gap) = (372.0, 600.0, 58.0);
    let centre = Rect::new(LEFT + left_w + gap, top, w - 2.0 * LEFT - left_w - right_w - 2.0 * gap, bottom - top);
    let right = Rect::new(w - LEFT - right_w, top, right_w, bottom - top);

    // The side columns sit on glass; the chart in the middle carries its own frame.
    ui.panel(Rect::new(LEFT - 22.0, top - 20.0, left_w + 44.0, bottom - top + 40.0));
    ui.panel(Rect::new(right.x - 22.0, top - 20.0, right_w + 44.0, bottom - top + 40.0));
    map_list(ui, state, Rect::new(LEFT, top, left_w, bottom - top));
    rules(ui, state, Rect::new(LEFT, bottom - 196.0, left_w, 196.0));
    map_preview(ui, state, centre);
    commanders(ui, state, right);

    // Footer.
    let problem = state.problem();
    let back = ui.button(id("skirmish-back", 0), Rect::new(LEFT, h - 64.0 - 52.0, 200.0, 52.0), "BACK", ButtonKind::Secondary, true);
    let start_rect = Rect::new(w - LEFT - 340.0, h - 64.0 - 58.0, 340.0, 58.0);
    let start = ui.button(id("skirmish-start", 0), start_rect, "START MATCH", ButtonKind::Primary, problem.is_none());
    match problem {
        Some(text) => ui.text_right(start_rect.x - 24.0, start_rect.mid_y(), type_scale::CAPTION, rgb(palette::WARN, 1.0), text),
        None => {
            let seated = state.slots.iter().filter(|s| s.seat != Seat::Closed).count();
            ui.text_right(start_rect.x - 24.0, start_rect.mid_y(), type_scale::CAPTION, rgb(palette::DIM, 1.0), &format!("{seated} COMMANDERS READY"));
        }
    }
    let typing = ui.mem.editing.is_some();
    let mut action = None;
    if back || (ui.input.key(Key::Escape) && !typing && ui.interactive) {
        ui.audio.play(Sfx::Back);
        action = Some(SkirmishAction::Back);
    } else if (start || (ui.input.key(Key::Enter) && !typing && ui.interactive)) && problem.is_none() {
        ui.audio.play(Sfx::Launch);
        action = Some(SkirmishAction::Start(state.request()));
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    action
}

fn map_list(ui: &mut Ui, state: &mut SkirmishState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "THEATRE");
    let mut pick = None;
    for (i, entry) in state.maps.iter().enumerate().take(6) {
        let row = Rect::new(area.x, area.y + 28.0 + i as f32 * 74.0, area.w, 68.0);
        let res = ui.interact(id("map-row", i), row, true);
        let chosen = state.selected == i;
        if res.clicked && !chosen {
            pick = Some(i);
        }
        let lit = ui.ease(id("map-row-lit", i), if chosen { 1.0 } else { 0.0 }, 12.0);
        let g = lit.max(res.glow * 0.6);
        ui.fill(row, ink(0.5));
        ui.gradient_h(row, rgb(palette::ACCENT, 0.20 * g), rgb(palette::ACCENT, 0.01));
        ui.frame(row, rgb(if chosen { palette::ACCENT } else { palette::LINE }, 0.14 + 0.4 * g));
        ui.fill(Rect::new(row.x, row.y, 4.0, row.h), rgb(palette::ACCENT, lit));
        let info = entry.map.info();
        let km = info.size_metres().to_f32()[0] / 1000.0;
        ui.text(row.x + 20.0 + 4.0 * g, row.y + 25.0, type_scale::ITEM, rgb(if chosen { palette::ACCENT } else { palette::TEXT }, 0.85 + 0.15 * g), &entry.map.name().to_uppercase());
        ui.text(row.x + 21.0 + 4.0 * g, row.y + 48.0, type_scale::MICRO, rgb(palette::DIM, 1.0), &format!("{km:.0} KM  \u{b7}  {} PLAYERS  \u{b7}  {} DEPOSITS", entry.map.start_positions().len().min(8), entry.map.mass_deposits().len()));
    }
    if let Some(i) = pick {
        state.selected = i;
        state.seat_for_map();
        ui.audio.play(Sfx::Select);
    }
    if state.maps.is_empty() {
        ui.text(area.x, area.y + 50.0, type_scale::BODY, rgb(palette::WARN, 1.0), "No maps in maps/");
    }
}

fn rules(ui: &mut Ui, state: &mut SkirmishState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "RULES");
    let row = |k: f32| Rect::new(area.x, area.y + 26.0 + k * 54.0, area.w, 50.0);
    ui.toggle(id("rule-fog", 0), row(0.0), "FOG OF WAR", "", &mut state.fog);

    // Seed: shown as two groups of hex digits, with a re-roll.
    let r = row(1.0);
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    ui.text(r.x + 16.0, r.mid_y(), type_scale::BODY, rgb(palette::TEXT, 0.82), "SEED");
    let reroll = Rect::new(r.right() - 96.0, r.y + 9.0, 96.0, 32.0);
    if ui.button(id("rule-seed", 0), reroll, "ROLL", ButtonKind::Secondary, true) {
        state.seed = fresh_seed();
        ui.audio.play(Sfx::Tick);
    }
    ui.text_right(reroll.x - 14.0, r.mid_y(), type_scale::VALUE, rgb(palette::TEXT, 1.0), &format!("{:04X}-{:04X}", state.seed >> 16 & 0xFFFF, state.seed & 0xFFFF));

    let r = row(2.0);
    ui.text(r.x + 16.0, r.mid_y(), type_scale::BODY, rgb(palette::TEXT, 0.82), "CALLSIGN");
    ui.text_field(id("rule-name", 0), Rect::new(r.right() - 220.0, r.y + 8.0, 220.0, 34.0), &mut state.name, 16);
}

fn map_preview(ui: &mut Ui, state: &mut SkirmishState, area: Rect) {
    let Some(entry) = state.maps.get(state.selected) else { return };
    if state.preview_of != Some(state.selected) {
        ui.o.set_image(PREVIEW_SLOT, preview::SIZE, preview::SIZE, &preview::render(&entry.map));
        state.preview_of = Some(state.selected);
    }
    let map = entry.map.clone();
    let side = area.w.min(area.h - 64.0);
    let frame = Rect::new(area.x + (area.w - side) * 0.5, area.y, side, side);
    // The chart fades up when the map changes.
    let shown = ui.ease(id("preview-shown", state.selected), 1.0, 5.0);
    ui.fill(frame, ink(0.85));
    ui.image(PREVIEW_SLOT, [0.0, 0.0, preview::SIZE as f32, preview::SIZE as f32], frame, [shown, shown, shown, 1.0]);
    // Chart furniture: grid, frame, brackets, a scale bar.
    for k in 1..4 {
        let t = k as f32 / 4.0;
        ui.vline(frame.x + frame.w * t, frame.y, frame.h, rgb(palette::LINE, 0.07));
        ui.hline(frame.x, frame.y + frame.h * t, frame.w, rgb(palette::LINE, 0.07));
    }
    ui.frame(frame, rgb(palette::LINE, 0.25));
    ui.brackets(frame.inset(-6.0), 14.0, rgb(palette::ACCENT, 0.7));
    let size_m = map.info().size_metres().to_f32();
    let km_pts = frame.w / (size_m[0].max(size_m[1]) / 1000.0);
    let bar_km = if size_m[0] > 30_000.0 { 10.0 } else { 2.0 };
    ui.fill(Rect::new(frame.x + 14.0, frame.bottom() - 16.0, km_pts * bar_km, 2.0), rgb(palette::TEXT, 0.8));
    ui.text(frame.x + 14.0, frame.bottom() - 28.0, type_scale::MICRO, rgb(palette::TEXT, 0.8), &format!("{bar_km:.0} KM"));
    ui.text_right(frame.right() - 12.0, frame.y + 16.0, type_scale::MICRO, rgb(palette::TEXT, 0.6), "N");
    ui.stroke(Vec2::new(frame.right() - 16.0, frame.y + 44.0), Vec2::new(frame.right() - 16.0, frame.y + 26.0), 1.2, rgb(palette::TEXT, 0.6));

    // Start positions. Click one to deploy there; whoever held it takes yours.
    let mut take = None;
    for (n, start) in map.start_positions().iter().enumerate().take(8) {
        let p = Vec2::new(frame.x, frame.y) + preview::locate(&map, start.to_f32(), side);
        let holder = state.slots.iter().position(|s| s.seat != Seat::Closed && s.start as usize == n);
        let res = ui.interact(id("start-marker", n), Rect::new(p.x - 17.0, p.y - 17.0, 34.0, 34.0), true);
        if res.clicked && holder != Some(0) {
            take = Some(n as u8);
        }
        let color = holder.map_or(rgb(palette::DIM, 0.9), |i| {
            let c = TEAM_COLORS[state.slots[i].color as usize];
            [c[0], c[1], c[2], 1.0]
        });
        ui.disc(p, 13.0 + 2.0 * res.glow, ink(0.85));
        ui.arc(p, 13.0 + 2.0 * res.glow, 0.0, std::f32::consts::TAU, if holder.is_some() { 2.2 } else { 1.2 }, color);
        if holder == Some(0) {
            let pulse = (ui.time * 0.8).fract();
            ui.arc(p, 14.0 + 16.0 * pulse, 0.0, std::f32::consts::TAU, 1.4, [color[0], color[1], color[2], 0.8 * (1.0 - pulse)]);
        }
        ui.text_centred(p.x + 1.0, p.y, type_scale::VALUE, rgb(palette::TEXT, 1.0), &format!("{}", n + 1));
        if res.glow > 0.05 {
            let tip = match holder {
                Some(0) => "YOUR LANDING ZONE",
                Some(_) => "CLICK TO SWAP PLACES",
                None => "CLICK TO DEPLOY HERE",
            };
            let tw = ui.text_width(type_scale::MICRO, tip);
            let tag = Rect::new((p.x - tw * 0.5 - 8.0).clamp(frame.x, frame.right() - tw - 16.0), p.y + 22.0, tw + 16.0, 20.0);
            ui.fill(tag, ink(0.9 * res.glow));
            ui.text(tag.x + 8.0, tag.mid_y(), type_scale::MICRO, rgb(palette::TEXT, res.glow), tip);
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
    let end = ui.text(frame.x, y, type_scale::ITEM, rgb(palette::TEXT, 1.0), &map.name().to_uppercase());
    ui.text(end + 18.0, y + 1.0, type_scale::MICRO, rgb(palette::DIM, 1.0), &format!("{:.1} \u{d7} {:.1} KM", size_m[0] / 1000.0, size_m[1] / 1000.0));
    ui.text_right(frame.right(), y + 1.0, type_scale::MICRO, rgb(palette::DIM, 1.0), "AMBER: MASS DEPOSITS    RINGS: LANDING ZONES");
}

fn commanders(ui: &mut Ui, state: &mut SkirmishState, area: Rect) {
    ui.section(area.x, area.y + 6.0, area.w, "COMMANDERS");
    let head = area.y + 34.0;
    let (seat_x, team_x, start_x) = (area.right() - 392.0, area.right() - 236.0, area.right() - 104.0);
    for (x, label) in [(area.x + 58.0, "CALLSIGN"), (seat_x, "CONTROL"), (team_x, "TEAM"), (start_x, "ZONE")] {
        ui.text(x, head, type_scale::MICRO, rgb(palette::DIM, 0.8), label);
    }
    let starts = state.slots.len();
    for i in 0..starts {
        let slot = state.slots[i];
        let row = Rect::new(area.x, head + 16.0 + i as f32 * 60.0, area.w, 54.0);
        let open = slot.seat != Seat::Closed;
        let live = if open { 1.0 } else { 0.4 };
        ui.fill(row, ink(0.5));
        ui.frame(row, rgb(palette::LINE, 0.12));

        // Colour chip: click to take the next free colour.
        let c = TEAM_COLORS[slot.color as usize];
        let chip = Rect::new(row.x, row.y, 14.0, row.h);
        let res = ui.interact(id("slot-color", i), Rect::new(row.x, row.y, 46.0, row.h), open);
        ui.fill(Rect::new(chip.x, chip.y, chip.w + 4.0 * res.glow, chip.h), [c[0], c[1], c[2], live]);
        if res.clicked {
            state.step_unique(i, 1, TEAM_COLORS.len(), |s| s.color, |s, v| s.color = v);
            ui.audio.play(Sfx::Tick);
        }
        ui.text(row.x + 26.0, row.mid_y(), type_scale::VALUE, rgb(palette::FAINT, 1.0), &format!("{:02}", i + 1));
        let name = match slot.seat {
            Seat::You => state.name.to_uppercase(),
            Seat::Ai => format!("ARC AI {}", state.slots[..=i].iter().filter(|s| s.seat == Seat::Ai).count()),
            Seat::Closed => "- - -".into(),
        };
        ui.text(row.x + 58.0, row.mid_y(), type_scale::BODY, rgb(palette::TEXT, live), &name);

        let field = |x: f32, w: f32| Rect::new(x, row.y + 10.0, w, row.h - 20.0);
        // Seat: yours is fixed; the others open and close.
        let (label, ink) = match slot.seat {
            Seat::You => ("YOU", rgb(palette::ACCENT, 1.0)),
            Seat::Ai => ("AI", rgb(palette::TEXT, 1.0)),
            Seat::Closed => ("CLOSED", rgb(palette::FAINT, 1.0)),
        };
        if ui.stepper(id("slot-seat", i), field(seat_x, 140.0), label, ink, i != 0) != 0 {
            if open {
                state.slots[i].seat = Seat::Closed;
            } else {
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
        let step = ui.stepper(id("slot-team", i), field(team_x, 116.0), &format!("TEAM {}", slot.team + 1), rgb(palette::TEXT, 1.0), open);
        if step != 0 {
            state.slots[i].team = (slot.team as i32 + step).rem_euclid(starts as i32) as u8;
        }
        let step = ui.stepper(id("slot-start", i), field(start_x, 104.0), &format!("{}", slot.start + 1), rgb(palette::TEXT, 1.0), open && starts > 1);
        if step != 0 {
            state.step_unique(i, step, starts, |s| s.start, |s, v| s.start = v);
        }
    }

    let y = head + 16.0 + starts as f32 * 60.0 + 22.0;
    if y + 60.0 < area.bottom() {
        ui.section(area.x, y, area.w, "BRIEFING");
        for (k, line) in ["Destroy every enemy commander to win.", "Commanders and engineers are amphibious: islands are", "reached on foot along the sea bed, or by hover tank."].into_iter().enumerate() {
            ui.text(area.x + 12.0, y + 28.0 + k as f32 * 22.0, super::style(mc_render::Face::Medium, 14.5, 1.2), rgb(palette::DIM, 1.0), line);
        }
    }
}
