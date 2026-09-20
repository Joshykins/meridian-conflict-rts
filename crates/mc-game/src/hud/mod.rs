//! The match HUD, in the same glass-and-cyan language as the front end and
//! built from the same toolkit: economy, clock and game speed along the top;
//! minimap, selection, order card, construction and queue along the bottom.
//!
//! Immediate mode. `draw` lays everything out in points, remembers which parts
//! of the screen it covered (so the match does not take those clicks for the
//! battlefield) and returns what the player asked for as `HudAction`s. The HUD
//! never sends commands itself.

mod build;
pub mod icons;
mod minimap;
mod profiler;
mod range;
mod selection;

use crate::audio::Sfx;
use crate::game::{Mode, Targeting, View};
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Color, Id, Rect, Response, Ui};
use glam::Vec2;
use icons::Glyph;
use mc_data::{BlueprintId, Blueprints, UnitBlueprint};
use mc_map::MapFile;
use mc_render::{Camera, FrameStats};
use mc_sim::mirror::{UnitInstance, UnitOrders, KIND_WRECK, STATE_IDLE};
use mc_sim::tables::flag;
use std::f32::consts::TAU;

pub use minimap::MINIMAP_SLOT;

pub const MASS: u32 = 0x6FE39B;
pub const ENERGY: u32 = 0xF4C25E;
/// A store running low. Yellower than `ENERGY`, so it reads on that row too.
const LOW: u32 = 0xFFE23D;

/// Game speeds on offer, percent of real time.
pub const SPEEDS: [u32; 10] = [5, 10, 25, 50, 100, 150, 200, 300, 500, 1000];

/// Margin between the HUD and the window's edge, and between its panels.
const EDGE: f32 = 14.0;
const GAP: f32 = 10.0;
/// Height of the bottom row of panels.
const DECK_H: f32 = 214.0;
const MINIMAP: f32 = 276.0;

#[derive(Clone, Debug, PartialEq)]
pub enum HudAction {
    /// A construction tile: place the structure, or queue the unit.
    Build(BlueprintId),
    /// Take one of these out of the selected factories' queues.
    Cancel(BlueprintId),
    Target(Targeting),
    Stop,
    Upgrade,
    /// Take the selection's upgrade back out of its queue.
    CancelUpgrade,
    Repeat(bool),
    /// Replace the selection; `focus` also brings the camera to it.
    Select {
        units: Vec<u32>,
        focus: bool,
    },
    /// Minimap: move the camera to this ground position.
    LookAt(Vec2),
    /// Minimap, right button: the context order at this ground position.
    OrderAt(Vec2),
    /// Minimap, left button while an order is being targeted.
    TargetAt(Vec2),
    Menu,
    Pause,
    /// One step through `SPEEDS`.
    Speed(i32),
    /// The test range's panel.
    Range(crate::range::RangeAction),
}

/// What the HUD draws from. All of it is a snapshot; nothing here is the simulation.
pub struct Scene<'a> {
    pub view: &'a View,
    pub blueprints: &'a Blueprints,
    pub map: &'a MapFile,
    pub camera: &'a Camera,
    pub gpu: &'a FrameStats,
}

impl Scene<'_> {
    fn bp(&self, u: &UnitInstance) -> &UnitBlueprint {
        self.blueprints.unit(BlueprintId(u.blueprint as u16))
    }

    fn queue_of(&self, unit_id: u32) -> Option<&UnitOrders> {
        self.view
            .status
            .queues
            .iter()
            .find(|q| q.unit_id == unit_id)
    }

    fn team_color(&self, owner: u8) -> Color {
        let c = self.view.colors[owner as usize % 8];
        [c[0], c[1], c[2], 1.0]
    }
}

pub fn has_flag(u: &UnitInstance, f: u16) -> bool {
    u.owner_flags & (f as u32) << 8 != 0
}

struct Toast {
    text: String,
    color: u32,
    age: f32,
}

#[derive(Default)]
pub struct Hud {
    /// Tech tab of the construction panel, and the builder blueprint it was chosen for.
    tab: u8,
    tab_for: Option<u32>,
    /// Screen areas the HUD covered last frame, in window pixels.
    covered: Vec<Rect>,
    toasts: Vec<Toast>,
    actions: Vec<HudAction>,
    /// Test range: the build state the slider last asked for during this drag.
    range_built: Option<u16>,
}

/// What a HUD tile reports back.
#[derive(Clone, Copy, Default)]
struct Tile {
    hovered: bool,
    clicked: bool,
    right_clicked: bool,
    glow: f32,
}

/// `1234.5` as `1,234`.
fn whole(v: f32) -> String {
    let n = v.max(0.0).round() as u64;
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn clock(seconds: f32) -> String {
    let s = seconds.max(0.0) as u32;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

impl Hud {
    /// Whether the HUD had something under this window pixel last frame.
    pub fn covers(&self, p: Vec2) -> bool {
        self.covered.iter().any(|r| r.contains(p))
    }

    pub fn toast(&mut self, text: impl Into<String>, color: u32) {
        let text = text.into();
        // The same complaint over and over is one complaint.
        if let Some(t) = self.toasts.iter_mut().find(|t| t.text == text) {
            t.age = t.age.min(0.3);
            return;
        }
        self.toasts.push(Toast {
            text,
            color,
            age: 0.0,
        });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    fn claim(&mut self, ui: &Ui, r: Rect) {
        self.covered
            .push(Rect::new(r.x * ui.s, r.y * ui.s, r.w * ui.s, r.h * ui.s));
    }

    /// A glass panel that also keeps clicks from reaching the battlefield.
    fn glass(&mut self, ui: &mut Ui, r: Rect) {
        self.claim(ui, r);
        // Denser than the front end's panels: these sit on sunlit ground, not a dimmed backdrop.
        ui.fill(r, ink(0.55));
        ui.panel(r);
    }

    /// The square button every HUD grid is made of. Draws the background; the
    /// caller draws what is on it. `lit` marks the active choice.
    fn tile(&mut self, ui: &mut Ui, id: Id, r: Rect, lit: bool, enabled: bool) -> Tile {
        let res: Response = ui.interact(id, r, enabled);
        let live = if enabled { 1.0 } else { 0.4 };
        let lit_k = ui.ease(id ^ 7, if lit { 1.0 } else { 0.0 }, 16.0);
        let sink = if res.held { 1.0 } else { 0.0 };
        let r = Rect::new(r.x, r.y + sink, r.w, r.h);
        ui.fill(r, ink(0.62));
        ui.gradient_v(
            r,
            rgb(palette::ACCENT, 0.05 + 0.16 * res.glow + 0.20 * lit_k),
            rgb(palette::ACCENT_DEEP, 0.10 * lit_k),
        );
        ui.frame(
            r,
            rgb(
                if lit { palette::ACCENT } else { palette::LINE },
                (0.16 + 0.45 * res.glow + 0.6 * lit_k).min(1.0) * live,
            ),
        );
        let bar = r.w * (0.25 + 0.75 * res.glow.max(lit_k));
        ui.fill(
            Rect::new(r.x + (r.w - bar) * 0.5, r.bottom() - 2.0, bar, 2.0),
            rgb(palette::ACCENT, (0.25 + 0.75 * res.glow.max(lit_k)) * live),
        );
        Tile {
            hovered: res.hovered,
            clicked: res.clicked,
            right_clicked: res.hovered && ui.input.right_pressed,
            glow: res.glow.max(lit_k),
        }
    }

    /// A small chip: `LABEL  value`. Returns whether it was clicked, and its width.
    #[allow(clippy::too_many_arguments)]
    fn chip(
        &mut self,
        ui: &mut Ui,
        id: Id,
        x: f32,
        y: f32,
        label: &str,
        value: &str,
        tone: u32,
    ) -> (bool, f32) {
        let w = ui.text_width(type_scale::MICRO, label)
            + ui.text_width(type_scale::VALUE, value)
            + 34.0;
        let r = Rect::new(x, y, w, 24.0);
        self.claim(ui, r);
        let res = ui.interact(id, r, true);
        ui.fill(r, ink(0.7));
        ui.gradient_h(r, rgb(tone, 0.10 + 0.18 * res.glow), rgb(tone, 0.0));
        ui.frame(r, rgb(palette::LINE, 0.18 + 0.4 * res.glow));
        ui.fill(
            Rect::new(r.x, r.y, 2.0, r.h),
            rgb(tone, 0.6 + 0.4 * res.glow),
        );
        let end = ui.text(
            r.x + 11.0,
            r.mid_y(),
            type_scale::MICRO,
            rgb(palette::DIM, 0.85 + 0.15 * res.glow),
            label,
        );
        ui.text(
            end + 8.0,
            r.mid_y(),
            type_scale::VALUE,
            rgb(tone, 1.0),
            value,
        );
        (res.clicked, w)
    }

    pub fn draw(&mut self, ui: &mut Ui, s: &Scene, dt: f32) -> Vec<HudAction> {
        self.covered.clear();
        self.actions.clear();
        let (w, h) = (ui.size.x, ui.size.y);
        let view = s.view;

        let under_economy = self.economy(ui, s);
        if let Some(r) = &view.range {
            range::draw(self, ui, s, r, under_economy);
        }
        deposits(ui, s);
        let under_top = self.top_bar(ui, s);
        if view.show_profiler {
            let r = profiler::draw(ui, s, Vec2::new(w - EDGE, under_top + GAP));
            self.claim(ui, r);
        }

        // The bottom deck. The minimap is always there; the rest follows the selection.
        let map_rect = Rect::new(EDGE, h - EDGE - MINIMAP, MINIMAP, MINIMAP);
        self.idle_chips(ui, s, map_rect.y - 24.0 - 8.0);
        minimap::draw(self, ui, s, map_rect);

        let units: Vec<&UnitInstance> = view
            .selection
            .iter()
            .filter_map(|id| view.index_of.get(id))
            .map(|&i| &view.frame.units[i])
            .collect();
        let deck_y = h - EDGE - DECK_H;
        let mut x = map_rect.right() + GAP;
        self.group_chips(ui, s, x, deck_y - 24.0 - 8.0);
        self.reach_key(ui, s, deck_y - 24.0 - 8.0);
        if !units.is_empty() {
            let info = Rect::new(x, deck_y, 336.0, DECK_H);
            selection::info(self, ui, s, &units, info);
            x = info.right() + GAP;
            let orders = Rect::new(
                x,
                deck_y,
                3.0 * selection::ORDER_W + 2.0 * selection::ORDER_GAP + 28.0,
                DECK_H,
            );
            selection::orders(self, ui, s, &units, orders);
            x = orders.right() + GAP;
            build::draw(
                self,
                ui,
                s,
                &units,
                Rect::new(x, deck_y, (w - EDGE - x).max(0.0), DECK_H),
            );
        }

        self.toasts(ui, dt);
        self.match_state(ui, s);
        std::mem::take(&mut self.actions)
    }

    /// Returns the y below it.
    fn economy(&mut self, ui: &mut Ui, s: &Scene) -> f32 {
        let block = 292.0;
        let r = Rect::new(EDGE, EDGE, block * 2.0 + 46.0, 68.0);
        let Some(p) = s.view.status.players.get(s.view.local as usize) else {
            return r.bottom();
        };
        self.glass(ui, r);
        // The test range's free build spends nothing, whatever the builders ask for.
        let free = s.view.range.as_ref().is_some_and(|range| range.free_build);
        let rows = [
            (
                "MASS",
                p.mass,
                p.mass_capacity,
                p.mass_income,
                p.mass_demand,
                MASS,
            ),
            (
                "ENERGY",
                p.energy,
                p.energy_capacity,
                p.energy_income,
                p.energy_demand,
                ENERGY,
            ),
        ];
        for (i, (name, have, cap, income, demand, tone)) in rows.into_iter().enumerate() {
            let x = r.x + 16.0 + i as f32 * (block + 14.0);
            if i > 0 {
                ui.vline(x - 8.0, r.y + 10.0, r.h - 20.0, rgb(palette::LINE, 0.14));
            }
            // What the builders are asking for, not what a stall lets them have: the
            // sum is how far short the income falls.
            let spend = demand;
            let net = income - spend;
            // The store itself only drains by what is really being spent.
            let drain = income - demand * p.efficiency;
            let empty = !free && have < 1.0 && net < -0.05;
            // Dry within fifteen seconds, or nearly there already.
            let low = !free
                && !empty
                && drain < -0.05
                && (have + drain * 15.0 <= 0.0 || have < cap * 0.15);
            // Empty is a hard red blink; low a slower yellow swell.
            let alert = if empty {
                Some((
                    palette::BAD,
                    if (ui.time * 3.0).fract() < 0.5 {
                        1.0
                    } else {
                        0.2
                    },
                ))
            } else if low {
                Some((LOW, 0.5 + 0.5 * (ui.time * 5.0).sin()))
            } else {
                None
            };
            if let Some((color, k)) = alert {
                let wash = Rect::new(x - 7.0, r.y + 5.0, block + 2.0, r.h - 10.0);
                let strength = if empty { 1.6 } else { 1.0 };
                ui.gradient_v(
                    wash,
                    rgb(color, (0.05 + 0.15 * k) * strength),
                    rgb(color, (0.02 + 0.06 * k) * strength),
                );
                ui.frame(wash, rgb(color, 0.25 + 0.7 * k));
            }
            ui.fill(Rect::new(x, r.y + 12.0, 3.0, 10.0), rgb(tone, 1.0));
            let end = ui.text(
                x + 10.0,
                r.y + 17.0,
                type_scale::CAPTION,
                rgb(tone, 1.0),
                name,
            );
            let have_color = alert.map_or(rgb(palette::TEXT, 1.0), |(color, k)| {
                rgb(color, 0.7 + 0.3 * k)
            });
            let end = ui.text(
                end + 12.0,
                r.y + 17.0,
                type_scale::VALUE,
                have_color,
                &whole(have),
            );
            let net_text = if net.abs() >= 100.0 {
                format!("{}{}", if net < 0.0 { "-" } else { "+" }, whole(net.abs()))
            } else {
                format!("{net:+.1}")
            };
            let net_w = ui.text_width(type_scale::BUTTON, &net_text);
            ui.text_right(
                x + block - 12.0,
                r.y + 17.0,
                type_scale::BUTTON,
                rgb(if net < -0.05 { palette::BAD } else { tone }, 1.0),
                &net_text,
            );
            // The capacity gives way when the numbers get long.
            let cap_text = format!("/ {}", whole(cap));
            if end + 5.0 + ui.text_width(type_scale::MICRO, &cap_text)
                < x + block - 12.0 - net_w - 8.0
            {
                ui.text(
                    end + 5.0,
                    r.y + 17.5,
                    type_scale::MICRO,
                    rgb(palette::FAINT, 1.0),
                    &cap_text,
                );
            }

            let track = Rect::new(x, r.y + 32.0, block - 12.0, 6.0);
            ui.fill(
                track,
                alert.map_or(rgb(palette::LINE, 0.13), |(color, k)| {
                    rgb(color, 0.12 + 0.3 * k)
                }),
            );
            let fill = (have / cap.max(1.0)).clamp(0.0, 1.0);
            let bar = alert.map_or(tone, |(color, _)| color);
            ui.gradient_h(
                Rect::new(track.x, track.y, track.w * fill, track.h),
                rgb(bar, 0.55),
                rgb(bar, 1.0),
            );
            ui.fill(
                Rect::new(
                    track.x + track.w * fill - 1.0,
                    track.y - 2.0,
                    2.0,
                    track.h + 4.0,
                ),
                rgb(0xFFFFFF, if fill > 0.002 { 0.9 } else { 0.0 }),
            );
            for k in 1..4 {
                ui.vline(
                    track.x + track.w * k as f32 / 4.0,
                    track.bottom() + 2.0,
                    3.0,
                    rgb(palette::LINE, 0.25),
                );
            }
            ui.text(
                x,
                r.y + 54.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &format!("INCOME  +{income:.1}"),
            );
            ui.text_right(
                x + block - 12.0,
                r.y + 54.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &format!("SPEND  -{spend:.1}"),
            );
        }
        if p.efficiency < 0.999 {
            let pulse = 0.65 + 0.35 * (ui.time * 5.0).sin().abs();
            let chip = Rect::new(r.right() + GAP, r.y, 170.0, 28.0);
            ui.fill(chip, ink(0.7));
            ui.frame(chip, rgb(palette::BAD, 0.7 * pulse));
            ui.fill(
                Rect::new(chip.x, chip.y, 3.0, chip.h),
                rgb(palette::BAD, pulse),
            );
            ui.text(
                chip.x + 14.0,
                chip.mid_y(),
                type_scale::CAPTION,
                rgb(palette::BAD, pulse),
                &format!("STALLING  {:.0}%", p.efficiency * 100.0),
            );
        }
        r.bottom()
    }

    /// Clock, game speed, pause and menu; the commanders under them. Returns the y below it all.
    fn top_bar(&mut self, ui: &mut Ui, s: &Scene) -> f32 {
        let view = s.view;
        let owns_clock = view.status.owns_clock;
        let r = Rect::new(ui.size.x - EDGE - 476.0, EDGE, 476.0, 44.0);
        self.glass(ui, r);
        let mid = r.mid_y();
        ui.text(
            r.x + 16.0,
            mid - 8.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "MISSION TIME",
        );
        ui.text(
            r.x + 16.0,
            mid + 8.0,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &clock(view.status.tick as f32 * 0.1),
        );
        ui.vline(r.x + 132.0, r.y + 9.0, r.h - 18.0, rgb(palette::LINE, 0.14));

        let speed = format!("{}\u{d7}", view.speed as f32 / 100.0);
        let tone = if view.speed == 100 {
            rgb(palette::TEXT, 1.0)
        } else {
            rgb(palette::ACCENT, 1.0)
        };
        ui.text(
            r.x + 146.0,
            mid,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "SPEED",
        );
        let step = ui.stepper(
            id("hud-speed", 0),
            Rect::new(r.x + 196.0, r.y + 7.0, 122.0, 30.0),
            &speed,
            tone,
            owns_clock,
        );
        if step != 0 {
            self.actions.push(HudAction::Speed(step));
        }

        let pause = Rect::new(r.x + 328.0, r.y + 7.0, 40.0, 30.0);
        let t = self.tile(ui, id("hud-pause", 0), pause, view.paused, owns_clock);
        let tone = rgb(
            if view.paused {
                palette::ACCENT
            } else {
                palette::TEXT
            },
            if owns_clock {
                0.75 + 0.25 * t.glow
            } else {
                0.3
            },
        );
        icons::glyph(
            ui,
            if view.paused {
                Glyph::Play
            } else {
                Glyph::Pause
            },
            Vec2::new(pause.x + pause.w * 0.5, pause.mid_y()),
            7.0,
            tone,
        );
        if t.clicked {
            ui.audio.play(Sfx::Select);
            self.actions.push(HudAction::Pause);
        }
        if ui.button(
            id("hud-menu", 0),
            Rect::new(r.x + 378.0, r.y + 7.0, 88.0, 30.0),
            "MENU",
            ButtonKind::Secondary,
            true,
        ) {
            self.actions.push(HudAction::Menu);
        }

        // The commanders in this match.
        let players = &view.status.players;
        let mut y = r.bottom() + GAP;
        if players.len() > 1 {
            let list = Rect::new(
                r.right() - 250.0,
                y,
                250.0,
                players.len() as f32 * 22.0 + 12.0,
            );
            self.claim(ui, list);
            ui.fill(list, ink(0.6));
            ui.frame(list, rgb(palette::LINE, 0.12));
            for (i, p) in players.iter().enumerate() {
                let ry = list.y + 17.0 + i as f32 * 22.0;
                let alive = if p.defeated { 0.35 } else { 1.0 };
                let mut c = s.team_color(i as u8);
                c[3] = alive;
                ui.fill(Rect::new(list.x + 12.0, ry - 4.0, 8.0, 8.0), c);
                if i == view.local as usize {
                    ui.frame(
                        Rect::new(list.x + 9.0, ry - 7.0, 14.0, 14.0),
                        rgb(palette::ACCENT, 0.8),
                    );
                }
                ui.text(
                    list.x + 32.0,
                    ry,
                    type_scale::CAPTION,
                    rgb(palette::TEXT, 0.9 * alive),
                    &p.name.to_uppercase(),
                );
                let (tag, tone) = if p.defeated {
                    ("DEFEATED".to_owned(), palette::BAD)
                } else {
                    (format!("TEAM {}", p.team + 1), palette::FAINT)
                };
                ui.text_right(
                    list.right() - 12.0,
                    ry,
                    type_scale::MICRO,
                    rgb(tone, 1.0),
                    &tag,
                );
            }
            y = list.bottom();
        }
        y
    }

    /// Idle engineers and factories, over the minimap.
    fn idle_chips(&mut self, ui: &mut Ui, s: &Scene, y: f32) {
        use mc_data::cat;
        let (mut engineers, mut factories) = (Vec::new(), Vec::new());
        for u in &s.view.frame.units {
            let mine =
                (u.owner_flags & 0xFF) as u8 == s.view.local && u.owner_flags & KIND_WRECK == 0;
            if !mine
                || u.owner_flags & STATE_IDLE == 0
                || has_flag(u, flag::UNDER_CONSTRUCTION | flag::IN_FACTORY)
            {
                continue;
            }
            let bp = s.bp(u);
            if bp.has(cat::ENGINEER) && !bp.has(cat::COMMANDER) {
                engineers.push(u.unit_id);
            } else if bp.has(cat::FACTORY) {
                factories.push(u.unit_id);
            }
        }
        let mut x = EDGE;
        for (key, label, units) in [
            ("idle-eng", "IDLE ENGINEERS", engineers),
            ("idle-fac", "IDLE FACTORIES", factories),
        ] {
            if units.is_empty() {
                continue;
            }
            let (clicked, w) = self.chip(
                ui,
                id(key, 0),
                x,
                y,
                label,
                &units.len().to_string(),
                palette::WARN,
            );
            if clicked {
                ui.audio.play(Sfx::Select);
                self.actions.push(HudAction::Select { units, focus: true });
            }
            x += w + 6.0;
        }
    }

    /// The control groups that hold something, over the selection panel.
    fn group_chips(&mut self, ui: &mut Ui, s: &Scene, mut x: f32, y: f32) {
        // In keyboard order: 1..9, then 0.
        for n in (1..10).chain([0]) {
            let members = &s.view.groups[n];
            if members.is_empty() {
                continue;
            }
            let (clicked, w) = self.chip(
                ui,
                id("group", n),
                x,
                y,
                &format!("GROUP {n}"),
                &members.len().to_string(),
                palette::ACCENT,
            );
            if clicked {
                ui.audio.play(Sfx::Select);
                self.actions.push(HudAction::Select {
                    units: members.clone(),
                    focus: false,
                });
            }
            x += w + 6.0;
        }
    }

    /// What the colours of the range rings on the ground mean, from the right edge inward.
    fn reach_key(&mut self, ui: &mut Ui, s: &Scene, y: f32) {
        let mut right = ui.size.x - EDGE;
        for (reach, dead, far) in s.view.reaches.iter().rev() {
            let value = if *dead > 0.0 {
                format!("{dead:.0}\u{2013}{far:.0} M")
            } else {
                format!("{far:.0} M")
            };
            let w = ui.text_width(type_scale::MICRO, reach.label())
                + ui.text_width(type_scale::VALUE, &value)
                + 52.0;
            let r = Rect::new(right - w, y, w, 24.0);
            ui.fill(r, ink(0.7));
            ui.frame(r, rgb(palette::LINE, 0.18));
            // A piece of the ring itself.
            ui.fill(
                Rect::new(r.x + 10.0, r.mid_y() - 1.0, 16.0, 2.0),
                rgb(reach.tone(), 1.0),
            );
            let end = ui.text(
                r.x + 34.0,
                r.mid_y(),
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                reach.label(),
            );
            ui.text(
                end + 8.0,
                r.mid_y(),
                type_scale::VALUE,
                rgb(palette::TEXT, 1.0),
                &value,
            );
            right = r.x - 6.0;
        }
    }

    fn toasts(&mut self, ui: &mut Ui, dt: f32) {
        self.toasts.retain_mut(|t| {
            t.age += dt;
            t.age < 4.0
        });
        let mut y = 104.0;
        for t in &self.toasts {
            let k = (t.age / 0.18).min(1.0) * ((4.0 - t.age) / 0.6).clamp(0.0, 1.0);
            let w = ui.text_width(type_scale::CAPTION, &t.text) + 44.0;
            let r = Rect::new((ui.size.x - w) * 0.5, y - 6.0 * (1.0 - k), w, 30.0);
            ui.fill(r, ink(0.72 * k));
            ui.frame(r, rgb(t.color, 0.5 * k));
            ui.fill(Rect::new(r.x, r.y, 3.0, r.h), rgb(t.color, k));
            ui.text(
                r.x + 22.0,
                r.mid_y(),
                type_scale::CAPTION,
                rgb(t.color, k),
                &t.text,
            );
            y += 36.0;
        }
    }

    /// Loading, a fatal error, and the pause card. The result of the match is the menu's to show.
    fn match_state(&mut self, ui: &mut Ui, s: &Scene) {
        let view = s.view;
        let (w, h) = (ui.size.x, ui.size.y);
        if let Some(e) = &view.status.error {
            let tw = ui.text_width(type_scale::BODY, e) + 80.0;
            let r = Rect::new((w - tw) * 0.5, h * 0.38, tw, 84.0);
            ui.panel(r);
            ui.text_centred(
                w * 0.5,
                r.y + 28.0,
                type_scale::CAPTION,
                rgb(palette::BAD, 1.0),
                "THE MATCH CANNOT CONTINUE",
            );
            ui.text_centred(
                w * 0.5,
                r.y + 56.0,
                type_scale::BODY,
                rgb(palette::TEXT, 1.0),
                e,
            );
        } else if view.status.tick == 0 && view.status.winner.is_none() {
            ui.text_centred(
                w * 0.5,
                h * 0.46,
                type_scale::TITLE,
                rgb(palette::TEXT, 0.9),
                "ESTABLISHING UPLINK",
            );
            let k = (ui.time * 0.8).fract();
            ui.fill(
                Rect::new(w * 0.5 - 120.0 + 200.0 * k, h * 0.46 + 34.0, 40.0, 2.0),
                rgb(palette::ACCENT, 1.0),
            );
            ui.fill(
                Rect::new(w * 0.5 - 120.0, h * 0.46 + 34.0, 240.0, 1.0),
                rgb(palette::LINE, 0.25),
            );
        } else if view.paused && !view.menu_open {
            self.pause_card(ui);
        }
    }

    /// The battlefield held still: a band across the middle, and a way back.
    fn pause_card(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.size.x, ui.size.y);
        let k = ui.ease(id("pause-card", 0), 1.0, 9.0);
        ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.28 * k));
        let band = Rect::new(0.0, h * 0.5 - 92.0, w, 184.0);
        self.claim(ui, band);
        ui.scrim(Rect::new(0.0, band.y, w * 0.5, band.h), 0.0, 0.78 * k, true);
        ui.scrim(
            Rect::new(w * 0.5, band.y, w * 0.5, band.h),
            0.78 * k,
            0.0,
            true,
        );
        for y in [band.y, band.bottom()] {
            ui.gradient_h(
                Rect::new(w * 0.2, y, w * 0.3, 1.0),
                rgb(palette::LINE, 0.0),
                rgb(palette::LINE, 0.5 * k),
            );
            ui.gradient_h(
                Rect::new(w * 0.5, y, w * 0.3, 1.0),
                rgb(palette::LINE, 0.5 * k),
                rgb(palette::LINE, 0.0),
            );
        }
        let c = Vec2::new(w * 0.5, band.y + 62.0);
        let tw = ui.text_width(type_scale::DISPLAY, "PAUSED");
        ui.text_centred(
            c.x + 12.0,
            c.y,
            type_scale::DISPLAY,
            rgb(0xFFFFFF, k),
            "PAUSED",
        );
        // Instrument marks either side of the word: broken rings turning against each other.
        for side in [-1.0f32, 1.0] {
            let rc = Vec2::new(c.x + side * (tw * 0.5 + 64.0), c.y);
            let turn = ui.time * 0.5 * side;
            for i in 0..3 {
                let a = turn + i as f32 * std::f32::consts::TAU / 3.0;
                ui.arc(rc, 22.0, a, a + 1.5, 1.4, rgb(palette::ACCENT, 0.9 * k));
                ui.arc(rc, 30.0, -a, -a + 0.7, 1.0, rgb(palette::LINE, 0.5 * k));
            }
            ui.disc(rc, 2.0, rgb(palette::ACCENT, k));
            ui.hline(
                rc.x + side * 40.0 - if side < 0.0 { 60.0 } else { 0.0 },
                c.y,
                60.0,
                rgb(palette::LINE, 0.3 * k),
            );
        }
        ui.text_centred(
            c.x,
            c.y + 40.0,
            type_scale::CAPTION,
            rgb(palette::DIM, k),
            "THE BATTLEFIELD IS HOLDING  \u{b7}  ORDERS GIVEN NOW ARE CARRIED OUT ON RESUME",
        );
        if ui.button(
            id("pause-card-resume", 0),
            Rect::new(c.x - 130.0, c.y + 66.0, 260.0, 44.0),
            "RESUME",
            ButtonKind::Primary,
            true,
        ) {
            ui.audio.play(Sfx::Back);
            self.actions.push(HudAction::Pause);
        }
    }
}

/// What the cursor is about to do, next to it. `placing` is how many structures
/// a place-drag would put down (one when the pointer has not moved).
pub fn cursor_hint(ui: &mut Ui, view: &View, blueprints: &Blueprints, placing: usize) {
    let (text, tone) = match view.mode {
        Mode::Normal => return,
        Mode::Target(t) => (
            t.label().to_owned(),
            if t == Targeting::Attack || t == Targeting::AttackMove {
                palette::BAD
            } else {
                palette::ACCENT
            },
        ),
        Mode::Place(b) => {
            let name = blueprints.unit(b).name.to_uppercase();
            let text = if placing > 1 {
                format!("PLACE {placing} {name}")
            } else {
                format!("PLACE {name}")
            };
            (text, palette::ACCENT)
        }
        Mode::Spawn => match &view.range {
            Some(r) => (
                format!(
                    "SPAWN {} {}  \u{d7} {}",
                    r.side.label(),
                    blueprints.unit(r.subject).name.to_uppercase(),
                    r.count()
                ),
                if r.side == crate::range::Side::Blue {
                    palette::ACCENT
                } else {
                    palette::BAD
                },
            ),
            None => return,
        },
    };
    let hint = match view.mode {
        Mode::Place(_) if placing > 1 => "SHIFT QUEUES  \u{b7}  RMB CANCELS",
        Mode::Place(_) | Mode::Spawn => {
            "CLICK OR DRAG  \u{b7}  SHIFT KEEPS PLACING  \u{b7}  RMB CANCELS"
        }
        _ => "LMB CONFIRMS  \u{b7}  RMB CANCELS",
    };
    let p = ui.cursor + Vec2::new(20.0, 22.0);
    let w = ui
        .text_width(type_scale::CAPTION, &text)
        .max(ui.text_width(type_scale::MICRO, hint))
        + 26.0;
    let r = Rect::new(
        p.x.min(ui.size.x - w - 4.0),
        p.y.min(ui.size.y - 50.0),
        w,
        44.0,
    );
    ui.fill(r, ink(0.74));
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(tone, 1.0));
    ui.text(
        r.x + 13.0,
        r.y + 14.0,
        type_scale::CAPTION,
        rgb(tone, 1.0),
        &text,
    );
    ui.text(
        r.x + 13.0,
        r.y + 31.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        hint,
    );
}

/// The drag-selection rectangle. Window pixels in, like the pointer.
pub fn drag_box(ui: &mut Ui, from: Vec2, to: Vec2) {
    let (min, max) = (from.min(to) / ui.s, from.max(to) / ui.s);
    let r = Rect::new(min.x, min.y, max.x - min.x, max.y - min.y);
    ui.fill(r, rgb(palette::ACCENT, 0.07));
    ui.frame(r, rgb(palette::ACCENT, 0.85));
    ui.brackets(r, 7.0, rgb(0xFFFFFF, 0.9));
}

/// Screen-space marks over every mass deposit, so the cracks stay readable
/// from orbit. Close in they fade and the ground does the talking; while
/// placing an extractor they stay up.
fn deposits(ui: &mut Ui, s: &Scene) {
    let placing = matches!(s.view.mode, Mode::Place(bp) if s.blueprints.unit(bp).needs_deposit);
    let fade = ((s.camera.distance - 240.0) / 360.0).clamp(0.0, 1.0);
    let alpha = if placing { fade.max(0.88) } else { fade };
    if alpha < 0.05 {
        return;
    }
    let viewport = s.camera.viewport;
    for d in s.map.mass_deposits() {
        let xy = Vec2::from(d.to_f32());
        let z = overview_height(s.map, xy);
        let Some(p) = s.camera.project(xy.extend(z + 1.2)) else {
            continue;
        };
        if p.x < -30.0 || p.y < -30.0 || p.x > viewport.x + 30.0 || p.y > viewport.y + 30.0 {
            continue;
        }
        let c = p / ui.s;
        let taken = deposit_taken(s, xy);
        let tone = if placing && taken { palette::BAD } else { MASS };
        let a = if taken && !placing {
            alpha * 0.35
        } else {
            alpha
        };
        let r = if taken { 7.0 } else { 9.5 };
        ui.arc(c, r, 0.0, TAU, 1.7, rgb(tone, 0.85 * a));
        ui.arc(c, r * 0.55, 0.0, TAU, 1.2, rgb(tone, 0.55 * a));
        // The extractor mark: a disc with a cross cut, same picture as the icon.
        ui.stroke(
            c - Vec2::new(r * 0.42, 0.0),
            c + Vec2::new(r * 0.42, 0.0),
            1.5,
            rgb(tone, 0.95 * a),
        );
        ui.stroke(
            c - Vec2::new(0.0, r * 0.42),
            c + Vec2::new(0.0, r * 0.42),
            1.5,
            rgb(tone, 0.95 * a),
        );
    }
}

fn deposit_taken(s: &Scene, at: Vec2) -> bool {
    s.view
        .frame
        .units
        .iter()
        .any(|u| s.bp(u).needs_deposit && Vec2::new(u.pos[0], u.pos[1]).distance(at) < 12.0)
}

fn overview_height(map: &MapFile, xy: Vec2) -> f32 {
    let info = map.info();
    let (ow, oh) = map.overview_dims();
    let size = info.size_metres().to_f32();
    let u = (xy.x / size[0] * (ow - 1) as f32).clamp(0.0, (ow - 1) as f32);
    let v = (xy.y / size[1] * (oh - 1) as f32).clamp(0.0, (oh - 1) as f32);
    let (x0, y0) = (u.floor() as u32, v.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(ow - 1), (y0 + 1).min(oh - 1));
    let (fx, fy) = (u - x0 as f32, v - y0 as f32);
    let o = map.overview();
    let w = ow as usize;
    let z = |x: u32, y: u32| {
        info.sample_to_height(o[y as usize * w + x as usize])
            .to_f32()
    };
    let a = z(x0, y0) * (1.0 - fx) + z(x1, y0) * fx;
    let b = z(x0, y1) * (1.0 - fx) + z(x1, y1) * fx;
    a * (1.0 - fy) + b * fy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Input, Memory};
    use mc_render::Overlay;
    use mc_sim::mirror::{QueuedOrder, UnitOrders};
    use mc_sim::tables::OrderKind;
    use std::sync::Arc;

    /// A match with one finished unit of `key` selected, and the HUD that draws it.
    struct Rig {
        hud: Hud,
        view: View,
        blueprints: Arc<Blueprints>,
        map: MapFile,
        camera: Camera,
        overlay: Overlay,
        memory: Memory,
    }

    const VIEWPORT: Vec2 = Vec2::new(1920.0, 1080.0);

    impl Rig {
        fn new(key: &str) -> Rig {
            let blueprints = Arc::new(
                Blueprints::load(&Blueprints::locate_data_dir().expect("data dir"))
                    .expect("blueprints"),
            );
            let map =
                MapFile::open(crate::setup::find_map(None).expect("a map")).expect("map opens");
            let camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), VIEWPORT);
            let mut view = View::new(0, crate::setup::TEAM_COLORS, false);
            let blueprint = blueprints.id_of(key).expect("blueprint exists");
            let pos = [4000.0, 4000.0, 0.0];
            view.frame.units.push(UnitInstance {
                prev_pos: pos,
                prev_heading: 0.0,
                pos,
                heading: 0.0,
                blueprint: blueprint.0 as u32,
                owner_flags: 0,
                health: 1.0,
                build: 1.0,
                turret_yaw: 0.0,
                radius: 4.0,
                unit_id: 7,
                _pad: 0,
                gait: [0.0; 3],
                upgrade: 0.0,
                arm_pitch: [0.0; 4],
                prev_turret_yaw: 0.0,
                weld: [0.0; 3],
            });
            view.index_of.insert(7, 0);
            view.selection = vec![7];
            view.status.owns_clock = true;
            view.status.tick = 50;
            view.status.players = vec![Default::default(), Default::default()];
            Rig {
                hud: Hud::default(),
                view,
                blueprints,
                map,
                camera,
                overlay: Overlay::default(),
                memory: Memory::default(),
            }
        }

        fn frame(&mut self, input: &Input) -> Vec<HudAction> {
            let audio = crate::audio::Audio::silent();
            let stats = FrameStats::default();
            self.overlay.clear();
            self.memory.begin_frame();
            let mut ui = Ui::new(
                &mut self.overlay,
                input,
                &mut self.memory,
                &audio,
                VIEWPORT,
                1.0,
                1.0,
                0.016,
            );
            let scene = Scene {
                view: &self.view,
                blueprints: &self.blueprints,
                map: &self.map,
                camera: &self.camera,
                gpu: &stats,
            };
            let actions = self.hud.draw(&mut ui, &scene, 0.016);
            self.memory.end_frame(input);
            assert!(!self.overlay.overflowed, "the HUD overflowed the overlay");
            actions
        }

        /// Hover, press, release at `at`; returns what the release produced.
        fn click(&mut self, at: Vec2) -> Vec<HudAction> {
            self.frame(&Input {
                cursor: at,
                ..Default::default()
            });
            self.frame(&Input {
                cursor: at,
                down: true,
                pressed: true,
                ..Default::default()
            });
            self.frame(&Input {
                cursor: at,
                released: true,
                ..Default::default()
            })
        }

        fn right_click(&mut self, at: Vec2) -> Vec<HudAction> {
            self.frame(&Input {
                cursor: at,
                ..Default::default()
            });
            self.frame(&Input {
                cursor: at,
                right_pressed: true,
                ..Default::default()
            })
        }
    }

    // Where things are at 1920x1080 and interface scale 1: see `Hud::draw`.
    const DECK_Y: f32 = 1080.0 - EDGE - DECK_H;
    const BUILD_X: f32 = EDGE
        + MINIMAP
        + GAP
        + 336.0
        + GAP
        + (3.0 * selection::ORDER_W + 2.0 * selection::ORDER_GAP + 28.0)
        + GAP;
    const FIRST_TILE: Vec2 = Vec2::new(BUILD_X + 14.0 + 40.0, DECK_Y + 46.0 + 30.0);

    #[test]
    fn the_range_panel_reports_what_was_asked() {
        use crate::range::{Range, RangeAction, Scenario, RED};
        let mut rig = Rig::new("aster_t1_tank");
        assert!(
            !rig.hud.covers(Vec2::new(180.0, 400.0)),
            "a match that is not the range has no range panel"
        );
        let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
        rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
        // Where things are at 1920x1080: see `range::draw`.
        let range = |a| vec![HudAction::Range(a)];
        assert_eq!(
            rig.click(Vec2::new(305.0, 330.0)),
            range(RangeAction::Damage(1000)),
            "KILL"
        );
        assert_eq!(
            rig.click(Vec2::new(186.0, 465.0)),
            range(RangeAction::Scenario(Scenario::Targets))
        );
        assert_eq!(
            rig.click(Vec2::new(186.0, 501.0)),
            range(RangeAction::Scenario(Scenario::March)),
            "the second row: what the subject does itself"
        );
        assert_eq!(
            rig.click(Vec2::new(184.0, 541.0)),
            range(RangeAction::Reset)
        );
        assert_eq!(
            rig.click(Vec2::new(186.0, 609.0)),
            range(RangeAction::Control(RED))
        );
        assert_eq!(
            rig.click(Vec2::new(327.0, 172.0)),
            range(RangeAction::Subject(1))
        );
        assert_eq!(
            rig.click(Vec2::new(290.0, 362.0)),
            range(RangeAction::Flag(flag::INVULNERABLE, true))
        );
        // Holding the build track half way along asks for a half-built unit.
        let half = Vec2::new(80.0 + (340.0 - 28.0 - 52.0 - 44.0) * 0.5, 398.0);
        rig.frame(&Input {
            cursor: half,
            ..Default::default()
        });
        assert_eq!(
            rig.frame(&Input {
                cursor: half,
                down: true,
                pressed: true,
                ..Default::default()
            }),
            range(RangeAction::Build(500))
        );
        assert!(
            rig.hud.covers(Vec2::new(180.0, 400.0)),
            "a click on the panel must not reach the battlefield"
        );

        // With nothing selected it acts on every unit of the subject's type; with none of those, on nothing.
        rig.view.selection.clear();
        assert_eq!(
            rig.click(Vec2::new(305.0, 330.0)),
            range(RangeAction::Damage(1000))
        );
        rig.view.frame.units.clear();
        rig.view.index_of.clear();
        assert_eq!(rig.click(Vec2::new(305.0, 330.0)), vec![]);
    }

    #[test]
    fn a_construction_tile_builds_and_the_hud_keeps_the_click() {
        let mut rig = Rig::new("aster_t1_land_factory");
        let first = rig
            .blueprints
            .unit(rig.blueprints.id_of("aster_t1_land_factory").unwrap())
            .builder
            .as_ref()
            .unwrap()
            .builds[0];
        assert_eq!(rig.click(FIRST_TILE), vec![HudAction::Build(first)]);
        assert!(
            rig.hud.covers(FIRST_TILE),
            "a click on a tile must not reach the battlefield"
        );
        assert!(
            !rig.hud.covers(Vec2::new(960.0, 400.0)),
            "the middle of the screen is the battlefield's"
        );
        assert_eq!(rig.right_click(FIRST_TILE), vec![HudAction::Cancel(first)]);
    }

    #[test]
    fn tech_tabs_switch_what_is_offered() {
        let mut rig = Rig::new("aster_t2_engineer");
        let builds = rig
            .blueprints
            .unit(rig.blueprints.id_of("aster_t2_engineer").unwrap())
            .builder
            .as_ref()
            .unwrap()
            .builds
            .clone();
        let tier = |rig: &Rig, t: u8| {
            builds
                .iter()
                .copied()
                .find(|b| rig.blueprints.unit(*b).tech == t)
                .unwrap()
        };
        // A tech 2 engineer opens on its own tier; the T1 tab brings the basics back.
        assert_eq!(rig.click(FIRST_TILE), vec![HudAction::Build(tier(&rig, 2))]);
        assert_eq!(
            rig.click(Vec2::new(BUILD_X + 14.0 + 29.0, DECK_Y + 23.0)),
            vec![]
        );
        assert_eq!(rig.click(FIRST_TILE), vec![HudAction::Build(tier(&rig, 1))]);
    }

    #[test]
    fn the_queue_strip_lists_production_and_cancels_from_it() {
        let mut rig = Rig::new("aster_t1_land_factory");
        let builds = rig
            .blueprints
            .unit(rig.blueprints.id_of("aster_t1_land_factory").unwrap())
            .builder
            .as_ref()
            .unwrap()
            .builds
            .clone();
        let order = |b| QueuedOrder {
            kind: OrderKind::Produce,
            pos: [0.0, 0.0],
            at: mc_core::FxVec2::ZERO,
            blueprint: b,
        };
        rig.view.status.queues = vec![UnitOrders {
            unit_id: 7,
            orders: vec![order(builds[0]), order(builds[0]), order(builds[1])],
            progress: 0.4,
        }];
        rig.frame(&Input::default());
        let strip = Vec2::new(BUILD_X + 200.0, DECK_Y - GAP - 30.0);
        assert!(
            rig.hud.covers(strip),
            "the queue strip appears above the construction panel"
        );
        // Two stacks: 2 of the first product, then 1 of the second. The second stack starts one tile along.
        let second = Vec2::new(BUILD_X + 14.0, DECK_Y - GAP - 31.0) + Vec2::new(0.0, 0.0);
        let hits: Vec<HudAction> = (0..40)
            .flat_map(|i| rig.right_click(second + Vec2::X * (150.0 + i as f32 * 6.0)))
            .collect();
        assert!(
            hits.contains(&HudAction::Cancel(builds[0]))
                && hits.contains(&HudAction::Cancel(builds[1])),
            "{hits:?}"
        );
    }

    #[test]
    fn the_top_bar_and_the_minimap_report_what_was_asked() {
        let mut rig = Rig::new("aster_t1_tank");
        let bar = 1920.0 - EDGE - 476.0;
        assert_eq!(
            rig.click(Vec2::new(bar + 196.0 + 122.0 - 10.0, EDGE + 22.0)),
            vec![HudAction::Speed(1)]
        );
        assert_eq!(
            rig.click(Vec2::new(bar + 348.0, EDGE + 22.0)),
            vec![HudAction::Pause]
        );
        assert_eq!(
            rig.click(Vec2::new(bar + 420.0, EDGE + 22.0)),
            vec![HudAction::Menu]
        );
        // Holding the button on the chart looks there; the right button orders there.
        let chart = Vec2::new(EDGE + MINIMAP * 0.5, 1080.0 - EDGE - MINIMAP * 0.5);
        rig.frame(&Input {
            cursor: chart,
            ..Default::default()
        });
        let held = rig.frame(&Input {
            cursor: chart,
            down: true,
            pressed: true,
            ..Default::default()
        });
        assert!(matches!(held[..], [HudAction::LookAt(_)]), "{held:?}");
        rig.frame(&Input {
            cursor: chart,
            released: true,
            ..Default::default()
        });
        assert!(matches!(
            rig.right_click(chart)[..],
            [HudAction::OrderAt(_)]
        ));
        // A network match owns no clock: the speed and pause controls are dead.
        rig.view.status.owns_clock = false;
        assert_eq!(rig.click(Vec2::new(bar + 348.0, EDGE + 22.0)), vec![]);
    }

    #[test]
    fn the_order_card_arms_orders_the_selection_can_carry_out() {
        let mut rig = Rig::new("aster_t1_tank");
        let card = Vec2::new(EDGE + MINIMAP + GAP + 336.0 + GAP + 14.0, DECK_Y + 36.0);
        let slot = |i: usize| {
            card + Vec2::new(
                (i % 3) as f32 * (selection::ORDER_W + selection::ORDER_GAP) + 30.0,
                (i / 3) as f32 * (selection::ORDER_H + selection::ORDER_GAP) + 25.0,
            )
        };
        assert_eq!(rig.click(slot(0)), vec![HudAction::Target(Targeting::Move)]);
        assert_eq!(
            rig.click(slot(2)),
            vec![HudAction::Target(Targeting::AttackMove)]
        );
        assert_eq!(rig.click(slot(3)), vec![HudAction::Stop]);
        // A tank cannot reclaim, and is not a factory.
        assert_eq!(rig.click(slot(5)), vec![]);
        assert_eq!(rig.click(slot(7)), vec![]);
    }
}
