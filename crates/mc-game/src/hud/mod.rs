//! The match HUD, in the same glass-and-cyan language as the front end and
//! built from the same toolkit: economy, clock and game speed along the top;
//! minimap, selection, order card, construction and queue along the bottom.
//!
//! Immediate mode. `draw` lays everything out in points, remembers which parts
//! of the screen it covered (so the match does not take those clicks for the
//! battlefield) and returns what the player asked for as `HudAction`s. The HUD
//! never sends commands itself.

mod build;
pub mod cargo;
mod economy;
mod hangar;
pub mod icons;
mod mine;
mod minimap;
mod observer;
mod profiler;
mod range;
mod reclaim;
mod refit;
mod selection;
pub mod survival;
pub use selection::ordered_as;
pub mod style;
pub mod thumbs;
mod unit_picker;
mod volatile;

use crate::audio::Sfx;
use crate::game::{Mode, Targeting, View};
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Color, Id, Rect, Response, Ui};
use glam::{Vec2, Vec3};
use icons::Glyph;
use mc_data::{BlueprintId, Blueprints, UnitBlueprint};
use mc_map::MapFile;
use mc_render::{Camera, FrameStats};
use mc_sim::mirror::{UnitInstance, UnitOrders, KIND_WRECK, STATE_IDLE, STATE_UNIDENTIFIED};
use mc_sim::tables::flag;
use std::f32::consts::TAU;

pub use minimap::MINIMAP_SLOT;

/// Materials (the sim's `mass`): red-orange.
pub const MASS: u32 = 0xFF6B3D;
/// Healthy units: the green that used to be the materials colour.
pub const HEALTHY: u32 = 0x6FE39B;
pub const ENERGY: u32 = 0xF4C25E;
/// A store running low. Yellower than `ENERGY`, so it reads on that row too.
const LOW: u32 = 0xFFE23D;

/// Game speeds on offer, percent of real time.
pub const SPEEDS: [u32; 8] = [10, 25, 50, 100, 200, 400, 800, 1200];

/// A speed as the player reads it: `.25\u{d7}`, `1\u{d7}`, `12\u{d7}`.
pub fn speed_label(pct: u32) -> String {
    if pct % 100 == 0 {
        format!("{}\u{d7}", pct / 100)
    } else {
        let s = format!("{}", pct as f32 / 100.0);
        format!("{}\u{d7}", s.trim_start_matches('0'))
    }
}

/// Margin between the HUD and the window's edge, and between its panels.
const EDGE: f32 = 14.0;
const GAP: f32 = 10.0;
/// Height of the bottom row of panels.
const DECK_H: f32 = 232.0;
const MINIMAP: f32 = 250.0;
/// The commander's own card, under the economy.
const COMMANDER_W: f32 = 250.0;
const COMMANDER_H: f32 = 90.0;
/// The speed control: two arrows and the speed between them.
const SPEED_W: f32 = 150.0;

#[derive(Clone, Debug, PartialEq)]
pub enum HudAction {
    /// A construction tile: place the structure, or queue the unit.
    Build(BlueprintId),
    /// Take one of these out of the selected factories' queues.
    Cancel(BlueprintId),
    Target(Targeting),
    Stop,
    FormationPanel,
    FormationTogether(bool),
    FormationSpacing(u8),
    FormUp,
    /// Queue the selection's next tier, after any tiers already queued.
    Upgrade,
    /// Queue fitting these kits' modules on the selection, in order.
    Refit(Vec<BlueprintId>),
    /// Take the refit to this kit, or the upgrade to this tier, out of the selection's queues.
    CancelRefit(BlueprintId),
    Repeat(bool),
    /// Take one queued order out of the selection's queues: the one of `kind` at `pos`.
    CancelOrder {
        kind: mc_sim::tables::OrderKind,
        pos: mc_core::FxVec2,
    },
    FireState(mc_sim::FireState),
    /// Submarines in the selection dive (`true`) or surface.
    Dive(bool),
    /// Builders, factories and upgrading structures in the selection pause (`true`)
    /// or resume their work, keeping their queues.
    PauseWork(bool),
    /// Aircraft out of the selected airbases: `blueprint` only, if given, and at most
    /// `count` from each (zero: all of them).
    Launch {
        blueprint: Option<BlueprintId>,
        count: u16,
    },
    /// Selected airbases take idle aircraft by themselves (`true`) or only when sent.
    AutoLand(bool),
    /// These aircraft, stored below an airbase, are fired out.
    LaunchUnits(Vec<u32>),
    /// Selected lift ships set down where they stand and let their holds out.
    UnloadHere,
    /// Selected lift ships set down where they stand.
    LandHere,
    /// Selected lift ships that are down raise the ramp and climb back to the clouds.
    TakeOff,
    /// These units, riding in a lift ship's hold, walk out of it (`Command::Unload`).
    UnloadUnits(Vec<u32>),

    /// Replace the selection; `focus` also brings the camera to it.
    Select {
        units: Vec<u32>,
        focus: bool,
    },
    /// Minimap: move the camera to this ground position.
    LookAt(Vec2),
    /// Jump the camera to this slot's commander.
    FocusPlayer(u8),
    /// Observing: see through this slot's eyes, or (`None`) everyone's.
    Vision(Option<u8>),
    /// Minimap, right button: the context order at this ground position.
    OrderAt(Vec2),
    /// Minimap, left button while an order is being targeted.
    TargetAt(Vec2),
    Menu,
    Pause,
    /// One of `SPEEDS`, percent.
    SetSpeed(u32),
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
    /// Live unit under the pointer, when it can be read.
    pub hover: Option<u32>,
    /// Control is held: survey reclaimable mass on the battlefield.
    pub show_reclaim: bool,
    /// While placing: the site under the pointer.
    pub placing: Option<mc_core::FxVec2>,
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
    /// Survival: names of the map's node sites, by site index.
    pub survival_sites: Vec<String>,
    /// Survival: the map's fronts (domain, path), read once with the sites.
    survival_fronts: Vec<(mc_data::survival::Domain, Vec<Vec2>)>,
    survival_read: bool,
    /// Where toasts start: under survival's panel when there is one.
    toast_top: f32,
    /// Tech tab of the construction panel, and the builder blueprint it was chosen for.
    tab: u8,
    tab_for: Option<u32>,
    /// Screen areas the HUD covered last frame, in window pixels.
    covered: Vec<Rect>,
    toasts: Vec<Toast>,
    actions: Vec<HudAction>,
    /// Test range: the build state the slider last asked for during this drag.
    range_built: Option<u16>,
    /// Test range: the weather being set up on the panel, not yet applied.
    range_sky: Option<crate::range::RangeSky>,
    /// Test range: the panel's open tab, and whose economy its Economy tab shows.
    range_tab: range::Tab,
    range_econ: u8,
    /// Test range: the panel's eased height, and how far the open tab's page has faded in.
    range_tall: f32,
    range_page: f32,
    unit_picker: Option<unit_picker::Picker>,
    /// Whether the reclaim survey was up last frame, so the cue plays on the edge.
    reclaim_seen: bool,
    reclaim_open: bool,
    /// 0..1 fade of the survey, so it eases in and out after the key.
    reclaim_vis: f32,
    /// Pictures of the units, for tiles.
    pub thumbs: thumbs::Thumbs,
    /// How far along the construction strip is scrolled, in points: where it is
    /// headed, and where it is on screen (easing after it).
    build_scroll: f32,
    build_shown: f32,
    /// The lore-and-weapons card over the unit panel is open.
    pub details_open: bool,
    pub minimap_hidden: bool,
    /// Which idle engineer and factory a click on their chip goes to next.
    idle_next: [usize; 2],
    /// 1 when the commander was hit, fading.
    commander_hit: f32,
    /// The list of every game speed is open under the speed control.
    speed_open: bool,
    speed_anchor: Rect,
    /// What the deck last showed, so it can slide away showing it.
    deck_units: Vec<u32>,
    deck_kinds: Vec<u32>,
    deck_inspect: bool,
    /// What the deck showed when the details card was last looked at: another
    /// selection (or coming back to the same one) closes the card.
    details_for: Option<Vec<u32>>,
    /// The construction keys are live (B): digits pick a tier, Q..Y a shelf, A..L an item on it.
    pub build_keys: bool,
    /// A construction key pressed since the last frame, for the panel to act on.
    pub build_key: Option<char>,
    /// The shelf the item keys pick from.
    shelf: Option<style::Purpose>,
    /// A refit that would take a module off, waiting for the player to confirm it.
    refit_prompt: Option<refit::Prompt>,
    /// Open the construction panel on its upgrade/refit tab next frame.
    want_refit_tab: bool,
    /// The map's ore fields, counted for the mine placement preview; built on first use.
    ore: Option<mc_sim::mines::OreGrid>,
    /// Observing: every commander's income and army over the last minutes.
    observed: observer::History,
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
    /// Opens the range panel on the tab called `name`, if there is one.
    pub fn open_range_tab(&mut self, name: &str) {
        if let Some(t) = range::Tab::ALL.into_iter().find(|t| t.label().eq_ignore_ascii_case(name)) {
            self.range_tab = t;
            self.range_page = 1.0;
        }
    }

    pub fn browse_range_subject(&mut self) {
        self.unit_picker = Some(unit_picker::Picker::new(unit_picker::Target::Subject));
    }

    /// Opens the construction panel on the selection's upgrade or refit tab.
    pub fn open_refit_tab(&mut self) {
        self.want_refit_tab = true;
    }

    pub fn unit_picker_open(&self) -> bool {
        self.unit_picker.is_some()
    }

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
        ui.fill_cut(r, 5.0, ink(0.62));
        ui.fill_cut(r, 5.0, rgb(0xFFFFFF, 0.04 * res.glow + 0.09 * lit_k));
        ui.bevel(r, 5.0, (0.45 + 0.4 * res.glow + 0.6 * lit_k).min(1.0) * live);
        if lit_k > 0.01 {
            ui.outline_cut(r, 5.0, rgb(0xFFFFFF, 0.55 * lit_k * live), rgb(0xFFFFFF, 0.55 * lit_k * live));
        }
        // The bar along the foot stays between the cut corners.
        let bar = (r.w - 12.0).max(0.0) * (0.25 + 0.75 * res.glow.max(lit_k));
        ui.fill(
            Rect::new(r.x + (r.w - bar) * 0.5, r.bottom() - 2.0, bar, 2.0),
            rgb(0xFFFFFF, (0.2 + 0.7 * res.glow.max(lit_k)) * live),
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
    pub(crate) fn chip(
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

        let interactive = ui.interactive;
        if self.unit_picker_open() {
            ui.interactive = false;
        }
        // The mine survey lies on the world, under every panel.
        mine_marks(ui, s, &mut self.ore);
        let speed_hits = self.speed_hits(ui);
        let mut under_economy = self.economy(ui, s);
        if !view.observing {
            let card = Rect::new(EDGE, under_economy + GAP, COMMANDER_W, COMMANDER_H);
            if self.commander_card(ui, s, card, dt) {
                under_economy = card.bottom();
            }
        }
        if let Some(r) = &view.range {
            range::draw(self, ui, s, r, under_economy);
        }
        if self.reclaim_seen && s.show_reclaim != self.reclaim_open {
            reclaim::cue(ui, s.show_reclaim);
        }
        self.reclaim_seen = true;
        self.reclaim_open = s.show_reclaim;
        let goal = if s.show_reclaim { 1.0 } else { 0.0 };
        self.reclaim_vis += (goal - self.reclaim_vis) * (1.0 - (-dt * 9.0).exp());
        reclaim::draw(ui, s, self.reclaim_vis);
        let under_top = self.top_bar(ui, s);
        // Survival's rounds and nodes, top centre; toasts go under them.
        self.toast_top = survival::draw(self, ui, s, EDGE).unwrap_or(104.0).max(104.0);
        if view.show_profiler {
            let below = if self.minimap_hidden { 26.0 } else { MINIMAP };
            let r = profiler::draw(ui, s, Vec2::new(w - EDGE, under_top + 2.0 * GAP + below));
            self.claim(ui, r);
        }

        // The minimap sits under the top bar on the right, and folds away.
        let map_rect = Rect::new(w - EDGE - MINIMAP, under_top + GAP, MINIMAP, MINIMAP);
        if self.minimap_hidden {
            let tab = Rect::new(w - EDGE - 96.0, under_top + GAP, 96.0, 26.0);
            let t = self.tile(ui, id("minimap-show", 0), tab, false, true);
            self.claim(ui, tab);
            ui.text_centred(tab.x + tab.w * 0.5, tab.mid_y(), type_scale::MICRO, rgb(palette::TEXT, 0.8 + 0.2 * t.glow), "Map  +");
            if t.clicked {
                ui.audio.play(Sfx::Tick);
                self.minimap_hidden = false;
            }
        }
        // The map folds up into its tab and unfolds from it.
        let map_k = ui.ease(id("minimap-open", 0), if self.minimap_hidden { 0.0 } else { 1.0 }, 16.0);
        if map_k > 0.01 {
            let (fade, shift, live) = (ui.fade, ui.shift, ui.interactive);
            ui.fade *= map_k;
            ui.shift.y -= (1.0 - map_k) * 40.0;
            ui.interactive &= !self.minimap_hidden;
            minimap::draw(self, ui, s, map_rect);
            (ui.fade, ui.shift, ui.interactive) = (fade, shift, live);
        }

        // The bottom deck: whatever the selection is.
        let deck_y = h - EDGE - DECK_H;
        let chips_end = if view.observing {
            EDGE
        } else {
            self.idle_chips(ui, s, deck_y - 24.0 - 8.0)
        };

        let selected: Vec<&UnitInstance> = view
            .selection
            .iter()
            .filter_map(|id| view.index_of.get(id))
            .map(|&i| &view.frame.units[i])
            .collect();
        let hovered = s.hover.and_then(|id| {
            view.index_of
                .get(&id)
                .map(|&i| &view.frame.units[i])
                .filter(|u| u.owner_flags & (KIND_WRECK | STATE_UNIDENTIFIED) == 0)
        });
        let ours = selected
            .iter()
            .any(|u| (u.owner_flags & 0xFF) as u8 == view.local);
        let units: Vec<&UnitInstance> = if !selected.is_empty() {
            selected.clone()
        } else if let Some(u) = hovered {
            vec![u]
        } else {
            Vec::new()
        };
        let mut inspect_only = selected.is_empty() || !ours;
        let mut x = EDGE;
        // The deck slides up and fades in when something is picked, back down when
        // nothing is, and dips briefly when the selection becomes something else.
        // By unit, not loadout: a finished refit is still the same selection.
        let mut kinds: Vec<u32> = units
            .iter()
            .map(|u| s.blueprints.base_of(BlueprintId(u.blueprint as u16)).0 as u32)
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        let deck_id = id("hud-deck", 0);
        let showing: Vec<u32> = units.iter().map(|u| u.unit_id).collect();
        if self.details_for.as_ref().is_some_and(|was| *was != showing) {
            self.details_open = false;
            ui.snap(id("unit-details-card", 0), 0.0);
        }
        self.details_for = Some(showing);
        if !units.is_empty() {
            if !self.deck_units.is_empty() && kinds != self.deck_kinds {
                let now = ui.ease(deck_id, 1.0, 0.0);
                ui.snap(deck_id, now.min(0.85));
            }
            self.deck_units = units.iter().map(|u| u.unit_id).collect();
            self.deck_kinds = kinds;
            self.deck_inspect = inspect_only;
        }
        // Quick to come up, so a big selection's panels are there at once; slower to go.
        let deck_k = if units.is_empty() {
            ui.ease(deck_id, 0.0, 12.0)
        } else {
            ui.ease(deck_id, 1.0, 36.0)
        };
        let closing = units.is_empty();
        let units: Vec<&UnitInstance> = if closing && deck_k > 0.01 {
            inspect_only = self.deck_inspect;
            self.deck_units
                .iter()
                .filter_map(|id| view.index_of.get(id))
                .map(|&i| &view.frame.units[i])
                .collect()
        } else {
            if closing {
                self.deck_units.clear();
            }
            units
        };
        let (fade, shift, live) = (ui.fade, ui.shift, ui.interactive);
        ui.fade *= deck_k;
        ui.shift.y += (1.0 - deck_k) * 36.0;
        ui.interactive &= !closing;
        if !view.observing {
            self.group_chips(ui, s, chips_end + 6.0, deck_y - 24.0 - 8.0);
        }
        // Above where a queue strip would be, so the two never overlap.
        self.reach_key(ui, s, deck_y - 62.0 - GAP - 24.0 - 8.0);
        if !units.is_empty() {
            let info = Rect::new(x, deck_y, 336.0, DECK_H);
            selection::info(self, ui, s, &units, info);
            x = info.right() + GAP;
            if !view.observing && !inspect_only {
                let orders = Rect::new(
                    x,
                    deck_y,
                    selection::orders_width(selection::order_families(s, &units)),
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
        }
        (ui.fade, ui.shift, ui.interactive) = (fade, shift, live);
        if let Some(u) = hovered {
            if !selected.iter().any(|s| s.unit_id == u.unit_id) && !selected.is_empty() {
                let card = selection::hover_card(
                    self,
                    ui,
                    s,
                    u,
                    Rect::new(EDGE, deck_y - 40.0, 336.0, 0.0),
                );
                self.claim(ui, card);
            }
        }

        self.toasts(ui, dt);
        self.match_state(ui, s);
        ui.interactive = interactive;
        self.speed_list(ui, s, &speed_hits);
        unit_picker::draw(self, ui, s);
        // An open dropdown's list goes over every panel, and while it is open
        // the whole screen is the HUD's: a click off the list only closes it.
        if ui.mem.popup.is_some() {
            self.claim(ui, Rect::new(0.0, 0.0, w, h));
        }
        ui.popups();
        std::mem::take(&mut self.actions)
    }

    /// Returns the y below it.
    fn economy(&mut self, ui: &mut Ui, s: &Scene) -> f32 {
        if s.view.observing {
            // Down to the idle-unit chips' line above the deck.
            let bottom = ui.size.y - EDGE - DECK_H - GAP;
            return self.observer_panel(ui, s, bottom);
        }
        let block = 292.0;
        let r = Rect::new(EDGE, EDGE, block * 2.0 + 46.0, 68.0);
        let Some(p) = s.view.status.players.get(s.view.local as usize) else {
            return r.bottom();
        };
        self.glass(ui, r);
        // The test range's free build spends nothing, whatever the builders ask for.
        let free = s.view.range.as_ref().is_some_and(|range| range.free_build);
        // Materials come in from the mines and from reclaim; the reclaim's share is shown too.
        let rows = [
            (
                "Materials",
                p.mass,
                p.mass_capacity,
                p.mass_income + p.reclaim_income,
                p.mass_demand,
                MASS,
            ),
            (
                "Energy",
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
            let end = ui.text(
                x,
                r.y + 54.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &format!("Income  +{income:.1}"),
            );
            if i == 0 && p.reclaim_income > 0.05 {
                let share = p.reclaim_income / income.max(0.01) * 100.0;
                ui.text(
                    end + 8.0,
                    r.y + 54.0,
                    type_scale::MICRO,
                    rgb(MASS, 0.95),
                    &format!("{share:.0}% reclaim"),
                );
            }
            ui.text_right(
                x + block - 12.0,
                r.y + 54.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &format!("Spend  -{spend:.1}"),
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
                &format!("Stalling  {:.0}%", p.efficiency * 100.0),
            );
        }
        r.bottom()
    }

    /// Clock, game speed, pause and menu; the commanders under them. Returns the y below it all.
    fn top_bar(&mut self, ui: &mut Ui, s: &Scene) -> f32 {
        let view = s.view;
        let owns_clock = view.status.owns_clock;
        let wide = 146.0 + 50.0 + SPEED_W + 10.0 + 50.0 + 98.0;
        let r = Rect::new(ui.size.x - EDGE - wide, EDGE, wide, 44.0);
        self.glass(ui, r);
        let mid = r.mid_y();
        ui.text(
            r.x + 16.0,
            mid - 8.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Mission Time",
        );
        ui.text(
            r.x + 16.0,
            mid + 8.0,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &clock(view.status.tick as f32 * 0.1),
        );
        ui.vline(r.x + 132.0, r.y + 9.0, r.h - 18.0, rgb(palette::LINE, 0.14));

        // Slower and faster either side; the middle opens the list of every speed.
        ui.text(
            r.x + 146.0,
            mid,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Speed",
        );
        let at = SPEEDS.iter().position(|p| *p == view.speed);
        let sx = r.x + 196.0;
        let arrows = [
            (Rect::new(sx, r.y + 7.0, 30.0, 30.0), -1i32),
            (Rect::new(sx + SPEED_W - 30.0, r.y + 7.0, 30.0, 30.0), 1),
        ];
        for (i, (ar, step)) in arrows.into_iter().enumerate() {
            let next = match at {
                Some(a) => (a as i32 + step).clamp(0, SPEEDS.len() as i32 - 1) as usize,
                None => SPEEDS.iter().position(|p| *p == 100).unwrap_or(0),
            };
            let able = owns_clock && Some(next) != at;
            let t = self.tile(ui, id("hud-speed-step", i), ar, false, able);
            let tone = rgb(palette::TEXT, if able { 0.7 + 0.3 * t.glow } else { 0.25 });
            let c = Vec2::new(ar.x + ar.w * 0.5, ar.mid_y());
            let d = step as f32;
            ui.triangle(
                c + Vec2::new(d * 4.0, 0.0),
                c + Vec2::new(-d * 3.0, -5.0),
                c + Vec2::new(-d * 3.0, 5.0),
                tone,
            );
            if t.clicked && able {
                ui.audio.play(Sfx::Tick);
                self.actions.push(HudAction::SetSpeed(SPEEDS[next]));
            }
        }
        let centre = Rect::new(sx + 34.0, r.y + 7.0, SPEED_W - 68.0, 30.0);
        let t = self.tile(ui, id("hud-speed", 0), centre, self.speed_open, owns_clock);
        ui.text_centred(
            centre.x + centre.w * 0.5 - 6.0,
            centre.mid_y(),
            type_scale::VALUE,
            rgb(palette::TEXT, if owns_clock { 0.85 + 0.15 * t.glow } else { 0.3 }),
            &speed_label(view.speed),
        );
        let caret = Vec2::new(centre.right() - 10.0, centre.mid_y());
        ui.triangle(
            caret + Vec2::new(-3.5, -2.0),
            caret + Vec2::new(3.5, -2.0),
            caret + Vec2::new(0.0, 2.5),
            rgb(palette::DIM, if owns_clock { 1.0 } else { 0.3 }),
        );
        if t.clicked && owns_clock {
            ui.audio.play(Sfx::Tick);
            self.speed_open = !self.speed_open;
        }
        if !owns_clock {
            self.speed_open = false;
        }
        // The list of speeds is drawn last, over everything: see `speed_list`.
        self.speed_anchor = centre;
        let after = sx + SPEED_W + 10.0;

        let pause = Rect::new(after, r.y + 7.0, 40.0, 30.0);
        let t = self.tile(ui, id("hud-pause", 0), pause, view.paused, owns_clock);
        let tone = rgb(
            if view.paused {
                palette::TEXT
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
            Rect::new(after + 50.0, r.y + 7.0, 88.0, 30.0),
            "Menu",
            ButtonKind::Secondary,
            true,
        ) {
            self.actions.push(HudAction::Menu);
        }

        // The commanders in this match.
        let players = &view.status.players;
        let mut y = r.bottom() + GAP;
        // An observer has them all down the left instead.
        if players.len() > 1 && !view.observing {
            let (wide, row_h) = (250.0, 22.0);
            let list = Rect::new(
                r.right() - wide,
                y,
                wide,
                players.len() as f32 * row_h + 12.0,
            );
            self.claim(ui, list);
            ui.fill(list, ink(0.6));
            ui.frame(list, rgb(palette::LINE, 0.12));
            for (i, p) in players.iter().enumerate() {
                let row = Rect::new(list.x, list.y + 6.0 + i as f32 * row_h, list.w, row_h);
                let res = ui.interact(id("hud-player", i), row, !p.defeated);
                if res.clicked {
                    ui.audio.play(Sfx::Select);
                    self.actions.push(HudAction::FocusPlayer(i as u8));
                }
                if res.glow > 0.02 {
                    ui.fill(row, rgb(palette::TEXT, 0.10 * res.glow));
                }
                let ry = row.mid_y();
                let alive = if p.defeated { 0.35 } else { 1.0 };
                let mut c = s.team_color(i as u8);
                c[3] = alive;
                ui.fill(Rect::new(list.x + 12.0, ry - 4.0, 8.0, 8.0), c);
                if i == view.local as usize {
                    ui.frame(
                        Rect::new(list.x + 9.0, ry - 7.0, 14.0, 14.0),
                        rgb(palette::TEXT, 0.8),
                    );
                }
                ui.text(
                    list.x + 32.0,
                    ry,
                    type_scale::CAPTION,
                    rgb(palette::TEXT, 0.9 * alive),
                    &p.name,
                );
                let (tag, tone) = if p.defeated {
                    ("Defeated".to_owned(), palette::BAD)
                } else {
                    (format!("Team {}", p.team + 1), palette::FAINT)
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

    /// Where the rows of the speed list are, as far as it has unrolled.
    fn speed_rows(&self, k: f32) -> (Rect, Vec<Rect>) {
        let centre = self.speed_anchor;
        let row = 28.0;
        let full = SPEEDS.len() as f32 * row + 8.0;
        let list = Rect::new(centre.x, centre.bottom() + 11.0, centre.w, full * k);
        let rows = (0..SPEEDS.len())
            .map(|i| Rect::new(list.x + 4.0, list.y + 4.0 + i as f32 * row, list.w - 8.0, row))
            .take_while(|rr| rr.bottom() <= list.bottom() + 0.5)
            .collect();
        (list, rows)
    }

    /// The speed list takes the pointer before anything under it can: called
    /// first thing in the frame, while `speed_list` draws it last.
    fn speed_hits(&mut self, ui: &mut Ui) -> Vec<Response> {
        if !self.speed_open {
            return Vec::new();
        }
        let (list, rows) = self.speed_rows(1.0);
        self.claim(ui, list);
        let hits = rows
            .iter()
            .enumerate()
            .map(|(i, rr)| ui.interact(id("hud-speed-pick", i), *rr, true))
            .collect();
        // The empty edges of the list are the list's too.
        ui.interact_with(id("hud-speed-list-bg", 0), list, true, false);
        hits
    }

    /// Every game speed, dropped down from the speed control; it unrolls and rolls back up.
    fn speed_list(&mut self, ui: &mut Ui, s: &Scene, hits: &[Response]) {
        let k = ui.ease(id("hud-speed-list", 0), if self.speed_open { 1.0 } else { 0.0 }, 18.0);
        if k < 0.01 {
            return;
        }
        let (list, rows) = self.speed_rows(k);
        let fade = ui.fade;
        ui.fade *= k;
        ui.panel(list);
        for (i, rr) in rows.iter().enumerate() {
            let pct = SPEEDS[i];
            let on = pct == s.view.speed;
            let res = hits.get(i).copied().unwrap_or_default();
            ui.fill(*rr, rgb(0xFFFFFF, 0.06 * res.glow + if on { 0.1 } else { 0.0 }));
            if on {
                ui.fill(Rect::new(rr.x + 2.0, rr.y + 6.0, 2.0, rr.h - 12.0), rgb(palette::ACCENT, 1.0));
            }
            ui.text_centred(
                rr.x + rr.w * 0.5,
                rr.mid_y(),
                type_scale::VALUE,
                rgb(if on { 0xFFFFFF } else { palette::DIM }, 0.85 + 0.15 * res.glow),
                &speed_label(pct),
            );
            if res.clicked {
                ui.audio.play(Sfx::Tick);
                self.speed_open = false;
                if !on {
                    self.actions.push(HudAction::SetSpeed(pct));
                }
            }
        }
        ui.fade = fade;
        // A click anywhere else closes it.
        let centre = self.speed_anchor;
        if self.speed_open && ui.input.pressed && !list.contains(ui.cursor) && !centre.contains(ui.cursor) {
            self.speed_open = false;
        }
    }

    /// The player's commander, always on show: its picture, health, what it is
    /// doing. It flashes when hit and pulses when idle; a click selects it and
    /// brings the camera to it.
    fn commander_card(&mut self, ui: &mut Ui, s: &Scene, r: Rect, dt: f32) -> bool {
        use mc_data::cat;
        let Some(u) = s.view.frame.units.iter().find(|u| {
            (u.owner_flags & 0xFF) as u8 == s.view.local
                && u.owner_flags & KIND_WRECK == 0
                && s.bp(u).has(cat::COMMANDER)
        }) else {
            return false;
        };
        let bp = s.bp(u);
        self.claim(ui, r);
        if has_flag(u, flag::HURT) {
            self.commander_hit = 1.0;
        }
        self.commander_hit = (self.commander_hit - dt * 1.4).max(0.0);
        let hit = self.commander_hit;
        let res = ui.interact(id("commander-card", 0), r, true);
        ui.panel(r);
        if hit > 0.0 {
            let blink = 0.5 + 0.5 * (ui.time * 18.0).sin();
            ui.fill_cut(r, 10.0, rgb(palette::BAD, 0.25 * hit * blink));
            ui.bevel(r, 10.0, hit);
        }
        ui.fill_cut(r, 10.0, rgb(0xFFFFFF, 0.05 * res.glow));
        if res.clicked {
            ui.audio.play(Sfx::Select);
            self.actions.push(HudAction::Select {
                units: vec![u.unit_id],
                focus: true,
            });
        }
        // Its picture on the left.
        let pic = Rect::new(r.x + 8.0, r.y + 8.0, r.h - 16.0, r.h - 16.0);
        style::domain_wash(ui, pic, style::Domain::of(bp), 0.2 + 0.3 * res.glow);
        if !self.thumbs.draw(ui, bp.id, pic, 1.0) {
            icons::strategic(ui, bp.visual.icon, bp.tech, Vec2::new(pic.x + pic.w * 0.5, pic.mid_y()), 14.0, s.team_color(s.view.local), ink(0.9));
        }
        let x = pic.right() + 12.0;
        let cw = r.right() - 12.0 - x;
        let level = u.veterancy_level();
        ui.text_fit_left(x, r.y + 17.0, cw - 64.0, type_scale::CAPTION, rgb(0xFFFFFF, 1.0), &bp.name);
        if level > 0 {
            selection::chevrons(ui, Vec2::new(r.right() - 58.0, r.y + 17.0), level);
        }
        let idle = u.owner_flags & STATE_IDLE != 0;
        let status = if hit > 0.0 {
            ("Under Fire".to_owned(), palette::BAD)
        } else if idle {
            ("Idle".to_owned(), palette::WARN)
        } else {
            let doing = s
                .queue_of(u.unit_id)
                .and_then(|q| q.orders.first())
                .map_or("Working", |o| selection::activity(o.kind));
            (doing.to_owned(), palette::DIM)
        };
        let pulse = if idle && hit == 0.0 { 0.55 + 0.45 * (ui.time * 3.0).sin().abs() } else { 1.0 };
        ui.fill(Rect::new(x, r.y + 33.0, 5.0, 5.0), rgb(status.1, pulse));
        ui.text_fit_left(x + 12.0, r.y + 36.0, cw - 70.0, type_scale::MICRO, rgb(status.1, pulse), &status.0);
        let hp = mc_sim::veterancy_health(bp.health, level).to_f32();
        let tone = if u.health > 0.6 { HEALTHY } else if u.health > 0.3 { palette::WARN } else { palette::BAD };
        ui.text_right(x + cw, r.y + 36.0, type_scale::VALUE, rgb(tone, 1.0), &whole(u.health * hp));
        let track = Rect::new(x, r.y + 50.0, cw, 5.0);
        ui.fill(track, rgb(palette::LINE, 0.12));
        ui.fill(Rect::new(track.x, track.y, track.w * u.health.clamp(0.0, 1.0), track.h), rgb(tone, 1.0));
        // What it has fitted, as icons; the list shows over them.
        if refit::icon_row(ui, s.blueprints, bp.id, x, r.y + 72.0, 17.0) == 0.0 {
            ui.text(x, r.y + 72.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), "No refits");
        }
        if res.hovered {
            build::tip(ui, r.x, r.bottom() + 6.0, "Click selects and finds your commander  \u{b7}  Home");
        }
        true
    }

    /// Idle engineers and factories, over the commander's card.
    /// Returns the x after the last chip.
    fn idle_chips(&mut self, ui: &mut Ui, s: &Scene, y: f32) -> f32 {
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
        for (n, (key, label, units)) in [
            ("idle-eng", "Idle Engineers", engineers),
            ("idle-fac", "Idle Factories", factories),
        ]
        .into_iter()
        .enumerate()
        {
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
                // A click goes to the next one and brings the camera to it; shift takes them all.
                let pick = if s.view.shift {
                    units
                } else {
                    let i = self.idle_next[n] % units.len();
                    self.idle_next[n] = i + 1;
                    vec![units[i]]
                };
                self.actions.push(HudAction::Select { units: pick, focus: true });
            }
            x += w + 6.0;
        }
        x
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
                &format!("Group {n}"),
                &members.len().to_string(),
                palette::TEXT,
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
        for &(reach, rank, dead, far) in s.view.reaches.iter().rev() {
            let value = if dead > 0.0 {
                format!("{dead:.0}\u{2013}{far:.0} M")
            } else {
                format!("{far:.0} m")
            };
            let w = ui.text_width(type_scale::MICRO, reach.label())
                + ui.text_width(type_scale::VALUE, &value)
                + 52.0;
            let r = Rect::new(right - w, y, w, 24.0);
            ui.fill(r, ink(0.7));
            ui.frame(r, rgb(palette::LINE, 0.18));
            // A piece of the ring itself.
            selection::ring_swatch(ui, r.x + 10.0, r.mid_y(), 16.0, reach, rank, dead > 0.0);
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
        let mut y = self.toast_top;
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
                "The Match Cannot Continue",
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
                "Establishing Uplink",
            );
            let k = (ui.time * 0.8).fract();
            ui.fill(
                Rect::new(w * 0.5 - 120.0 + 200.0 * k, h * 0.46 + 34.0, 40.0, 2.0),
                rgb(palette::TEXT, 1.0),
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
        let tw = ui.text_width(type_scale::DISPLAY, "Paused");
        ui.text_centred(
            c.x + 12.0,
            c.y,
            type_scale::DISPLAY,
            rgb(0xFFFFFF, k),
            "Paused",
        );
        // Instrument marks either side of the word: broken rings turning against each other.
        for side in [-1.0f32, 1.0] {
            let rc = Vec2::new(c.x + side * (tw * 0.5 + 64.0), c.y);
            let turn = ui.time * 0.5 * side;
            for i in 0..3 {
                let a = turn + i as f32 * std::f32::consts::TAU / 3.0;
                ui.arc(rc, 22.0, a, a + 1.5, 1.4, rgb(palette::TEXT, 0.9 * k));
                ui.arc(rc, 30.0, -a, -a + 0.7, 1.0, rgb(palette::LINE, 0.5 * k));
            }
            ui.disc(rc, 2.0, rgb(palette::TEXT, k));
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
            "The battlefield is holding  \u{b7}  Orders given now are carried out on resume",
        );
        if ui.button(
            id("pause-card-resume", 0),
            Rect::new(c.x - 130.0, c.y + 66.0, 260.0, 44.0),
            "Resume",
            ButtonKind::Primary,
            true,
        ) {
            ui.audio.play(Sfx::Back);
            self.actions.push(HudAction::Pause);
        }
    }
}

/// What the cursor is about to do, next to it. `sites` are where a place-drag
/// would put structures down (one when the pointer has not moved), and whether
/// each can go there; when none can, the hint says why.
pub fn cursor_hint(
    ui: &mut Ui,
    view: &View,
    blueprints: &Blueprints,
    sites: &[(mc_core::FxVec2, Result<(), mc_sim::placement::Unfit>)],
) {
    let placing = sites.len();
    let fit = sites.iter().filter(|(_, f)| f.is_ok()).count();
    let (text, tone) = match view.mode {
        Mode::Normal => return,
        Mode::Target(t) => (
            t.label().to_owned(),
            match t {
                Targeting::Attack
                | Targeting::AttackMove
                | Targeting::AttackGround
                | Targeting::Bombard => style::Family::Combat.tone(),
                Targeting::Move | Targeting::Patrol | Targeting::Orbit => {
                    style::Family::Movement.tone()
                }
                Targeting::Assist | Targeting::Reclaim => style::Family::Engineering.tone(),
                Targeting::Guard => style::Family::Stance.tone(),
                Targeting::Land | Targeting::Unload => style::Family::Transport.tone(),
            },
        ),
        Mode::Place(b) => {
            let name = blueprints.unit(b).name.clone();
            let why = sites.iter().find_map(|(_, f)| f.err());
            match why {
                Some(why) if fit == 0 => (format!("{name}  \u{b7}  {}", why.label()), palette::BAD),
                _ if fit < placing => (format!("Place {fit} of {placing} {name}"), palette::WARN),
                _ if placing > 1 => (format!("Place {placing} {name}"), palette::TEXT),
                _ => (format!("Place {name}"), palette::TEXT),
            }
        }
        Mode::Spawn => match &view.range {
            Some(r) => (
                format!("Duplicate {} Selection", r.side.label()),
                if r.side == crate::range::Side::Blue {
                    palette::TEXT
                } else {
                    palette::BAD
                },
            ),
            None => return,
        },
        Mode::SpawnSubject => match &view.range {
            Some(r) => {
                let subject = blueprints.unit(r.subject);
                (
                    format!(
                        "Spawn {} {}  \u{d7} {}",
                        r.side.label(),
                        subject.name,
                        if subject.is_mobile() { r.count() } else { 1 }
                    ),
                    if r.side == crate::range::Side::Blue {
                        palette::TEXT
                    } else {
                        palette::BAD
                    },
                )
            }
            None => return,
        },
    };
    let hint = match view.mode {
        Mode::Place(_) if placing > 1 => "Shift Queues  \u{b7}  RMB Cancels",
        Mode::Place(_) | Mode::Spawn | Mode::SpawnSubject => {
            "Click or drag  \u{b7}  Shift keeps placing  \u{b7}  RMB cancels"
        }
        Mode::Target(Targeting::Patrol)
            if view.shift && !crate::orders::patrol_legs(view).is_empty() =>
        {
            "Click to add a post to the nearest leg  \u{b7}  Release shift to finish"
        }
        Mode::Target(Targeting::Patrol) if view.patrol_posts.is_empty() => {
            "Click to patrol  \u{b7}  Shift adds posts  \u{b7}  RMB cancels"
        }
        Mode::Target(Targeting::Patrol) => "Click the next post  \u{b7}  Release shift to finish",
        Mode::Target(Targeting::Bombard) => "Press on the centre, drag out its size",
        Mode::Target(Targeting::Guard) => "Press on the spot to hold, drag out the area to guard",
        _ => "LMB Confirms  \u{b7}  RMB Cancels",
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
    ui.frost(r, 0.74);
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
    ui.fill(r, rgb(palette::TEXT, 0.07));
    ui.frame(r, rgb(palette::TEXT, 0.85));
    ui.brackets(r, 7.0, rgb(0xFFFFFF, 0.9));
}

/// The middle of every ore field (the mean of its corners), in map order.
pub fn ore_centres(map: &MapFile) -> Vec<Vec2> {
    map.ore_regions()
        .iter()
        .map(|r| {
            let sum: Vec2 = r.points.iter().map(|p| Vec2::from(p.to_f32())).sum();
            sum / r.points.len().max(1) as f32
        })
        .collect()
}

/// Core mines in sight (or remembered): position, reach and id. Anyone's; a
/// mine's territory does not care whose the next one is.
fn mines_in_sight(blueprints: &Blueprints, units: &[UnitInstance]) -> Vec<(Vec2, f32, u32)> {
    units
        .iter()
        .filter(|u| u.owner_flags & (KIND_WRECK | mc_sim::mirror::KIND_GHOST) == 0)
        .filter_map(|u| {
            let m = blueprints.unit(BlueprintId(u.blueprint as u16)).mine?;
            Some((Vec2::new(u.pos[0], u.pos[1]), m.reach.to_f32(), u.unit_id))
        })
        .collect()
}

/// Which ore fields, in map order, a mine the viewer has seen is working:
/// its middle lies in some mine's reach.
pub fn ore_tapped(map: &MapFile, blueprints: &Blueprints, units: &[UnitInstance]) -> Vec<bool> {
    let mines = mines_in_sight(blueprints, units);
    ore_centres(map)
        .into_iter()
        .map(|c| mines.iter().any(|(p, r, _)| p.distance(c) < *r))
        .collect()
}

/// Whether `at` falls to the mine at `centre` with `reach` among `all`
/// (itself included): the least `distance^2 - reach^2`, as the sim divides ground.
fn owns(at: Vec2, centre: Vec2, reach: f32, all: &[(Vec2, f32)]) -> bool {
    let mine = at.distance_squared(centre) - reach * reach;
    mine <= 0.0
        && all
            .iter()
            .all(|(p, r)| at.distance_squared(*p) - r * r >= mine - 1e-3)
}

const TERRITORY_SEGMENTS: usize = 240;

/// Overlay vertices the mine survey may use; the rest are the panels'.
const SURVEY_BUDGET: usize = mc_render::overlay::MAX_OVERLAY_VERTICES / 2;

/// A mine's territory: its circle, cut by a straight line toward every
/// neighbour it overlaps (the line through the two points where the circles
/// cross). Points round it on the ground, closed.
fn territory(centre: Vec2, reach: f32, others: &[(Vec2, f32)]) -> Vec<Vec2> {
    (0..=TERRITORY_SEGMENTS)
        .map(|i| {
            let a = i as f32 / TERRITORY_SEGMENTS as f32 * TAU;
            let u = Vec2::new(a.cos(), a.sin());
            let mut t = reach;
            for &(c, r) in others {
                let d = c - centre;
                let k = u.dot(d);
                if k <= 1e-4 || d.length_squared() < 1e-3 {
                    continue;
                }
                let limit = (d.length_squared() - r * r + reach * reach) / (2.0 * k);
                t = t.min(limit.max(0.0));
            }
            centre + u * t
        })
        .collect()
}

/// How a mine's territory reads at its tier, so a glance tells them apart:
/// short sparse dashes at T1, long ones at T2, a solid double edge at T3 and
/// the same heavier at T4, the fill deepening and the orange running hotter
/// as they climb.
struct TierLook {
    width: f32,
    /// Dash length and period, points; `None` is a solid line.
    dash: Option<(f32, f32)>,
    /// A second, thinner edge just inside the first.
    inner: bool,
    /// Times the base fill.
    fill: f32,
    /// Sonar rings sweeping out at once.
    rings: usize,
    /// How far the orange runs toward white.
    heat: f32,
}

fn tier_look(tier: u8) -> TierLook {
    match tier {
        0 | 1 => TierLook { width: 1.1, dash: Some((5.0, 13.0)), inner: false, fill: 0.5, rings: 1, heat: 0.0 },
        2 => TierLook { width: 1.7, dash: Some((16.0, 21.0)), inner: false, fill: 1.1, rings: 2, heat: 0.18 },
        3 => TierLook { width: 2.3, dash: None, inner: true, fill: 1.7, rings: 2, heat: 0.36 },
        _ => TierLook { width: 3.0, dash: None, inner: true, fill: 2.4, rings: 3, heat: 0.52 },
    }
}

/// `tone` run `k` of the way toward white.
fn heat(tone: u32, k: f32) -> u32 {
    let ch = |shift: u32| {
        let c = ((tone >> shift) & 0xFF) as f32;
        ((c + (255.0 - c) * k).round() as u32) << shift
    };
    ch(16) | ch(8) | ch(0)
}

/// Where a point `depth` metres under the ground at `xy` sits in the world.
fn underground(s: &Scene, xy: Vec2, depth: f32) -> Vec3 {
    xy.extend(overview_height(s.map, xy) - depth)
}

/// Pixels per metre at a point of the world, in interface points.
fn px_per_metre(s: &Scene, scale: f32, world: Vec3) -> f32 {
    s.camera.projection_scale() / s.camera.eye().distance(world).max(1.0) / scale
}

/// A shaded tube through the world from `a` to `b`, radius in metres at each
/// end: a dark translucent sheath, a bright core and a thin glint, so it reads
/// as something round and solid. Never thinner than `min` points.
fn tube(
    ui: &mut Ui,
    s: &Scene,
    (a, ra): (Vec3, f32),
    (b, rb): (Vec3, f32),
    min: f32,
    tone: u32,
    alpha: f32,
) {
    let scale = ui.s;
    let (Some(pa), Some(pb)) = (s.camera.project(a), s.camera.project(b)) else {
        return;
    };
    let (pa, pb) = (pa / scale, pb / scale);
    let d = (pb - pa).normalize_or_zero();
    if d == Vec2::ZERO {
        return;
    }
    let wa = (ra * px_per_metre(s, scale, a)).max(min * 0.5);
    let wb = (rb * px_per_metre(s, scale, b)).max(min * 0.5);
    // Running off the screen: only the part on it, as thick as it is there,
    // and no rounded end where it was cut.
    let (lo, hi) = screen_box(ui, s);
    let Some((ca, cb)) = clip(pa, pb, lo, hi) else {
        return;
    };
    let length = pa.distance(pb);
    let (cut_a, cut_b) = (ca != pa, cb != pb);
    let width_at = |p: Vec2| wa + (wb - wa) * (pa.distance(p) / length).clamp(0.0, 1.0);
    let (wa, wb, pa, pb) = (width_at(ca), width_at(cb), ca, cb);
    // Shaded across like a lit cylinder: a bright core easing out through
    // the sheath to a soft rim a point wide, never a hard step.
    let c = |a: f32| rgb(tone, alpha * a);
    let clear = rgb(tone, 0.0);
    let body = [(0.0, 0.0, c(0.9)), (0.4, 0.0, c(0.82)), (0.62, 0.0, c(0.38)), (1.0, -0.5, c(0.22)), (1.0, 0.5, clear)];
    let mut across: Vec<(f32, f32, crate::ui::Color)> = body.iter().rev().map(|&(k, px, col)| (-k, -px, col)).collect();
    across.extend_from_slice(&body[1..]);
    ui.ribbon(pa, pb, wa, wb, &across);
    // Round ends, so a shaft meets its drifts in a knuckle rather than a corner.
    if !cut_a {
        ui.ribbon_cap(pa, -d, wa, &body);
    }
    if !cut_b {
        ui.ribbon_cap(pb, d, wb, &body);
    }
    // The glint, soft on both sides.
    let glint = rgb(0xFFFFFF, alpha * 0.35);
    let edge = rgb(0xFFFFFF, 0.0);
    ui.ribbon(pa, pb, wa, wb, &[(0.18, -0.5, edge), (0.26, 0.0, glint), (0.38, 0.0, glint), (0.46, 0.5, edge)]);
}

/// A dashed line through the world, the dashes marching along with time.
/// The part of the segment `a`-`b` inside the rectangle `lo`..`hi`, if any
/// (Liang-Barsky). World marks near the camera project far off screen; drawn
/// whole they would cost the overlay its vertex budget and the frame its time.
fn clip(a: Vec2, b: Vec2, lo: Vec2, hi: Vec2) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - lo.x),
        (d.x, hi.x - a.x),
        (-d.y, a.y - lo.y),
        (d.y, hi.y - a.y),
    ] {
        if p.abs() < 1e-6 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((a + d * t0, a + d * t1))
}

/// The screen, a little larger, in interface points.
fn screen_box(ui: &Ui, s: &Scene) -> (Vec2, Vec2) {
    let size = s.camera.viewport / ui.s;
    (Vec2::splat(-40.0), size + 40.0)
}

/// A straight stroke between two interface points, cut to the screen.
fn seg(ui: &mut Ui, s: &Scene, a: Vec2, b: Vec2, width: f32, color: crate::ui::Color) {
    let (lo, hi) = screen_box(ui, s);
    if let Some((a, b)) = clip(a, b, lo, hi) {
        ui.stroke(a, b, width, color);
    }
}

/// A line through interface points, cut to the screen, with mitred joins:
/// a gap (`None`) or a cut at the screen's edge starts a new run.
fn smooth(ui: &mut Ui, s: &Scene, points: &[Option<Vec2>], width: f32, color: crate::ui::Color) {
    let (lo, hi) = screen_box(ui, s);
    let mut run: Vec<Vec2> = Vec::new();
    let closed = points.len() > 2
        && matches!((points[0], points[points.len() - 1]), (Some(a), Some(b)) if a.distance(b) < 0.01);
    let mut whole = true;
    for pair in points.windows(2) {
        let cut = match (pair[0], pair[1]) {
            (Some(a), Some(b)) => clip(a, b, lo, hi).map(|(p, q)| (p, q, p != a, q != b)),
            _ => None,
        };
        match cut {
            Some((p, q, cut_start, cut_end)) => {
                if cut_start || run.is_empty() {
                    if run.len() > 1 {
                        ui.polyline(&run, width, color, false);
                    }
                    run.clear();
                    run.push(p);
                }
                run.push(q);
                if cut_end {
                    whole = false;
                    ui.polyline(&run, width, color, false);
                    run.clear();
                }
                whole &= !cut_start;
            }
            None => {
                whole = false;
                if run.len() > 1 {
                    ui.polyline(&run, width, color, false);
                }
                run.clear();
            }
        }
    }
    if run.len() > 1 {
        ui.polyline(&run, width, color, closed && whole);
    }
}

/// What a mine has still to build: light grey.
const PLANNED: u32 = 0xC8C8C4;

/// A mine's unbuilt workings: grey dashes marching, every fourth one in the
/// materials red-orange, so the plan reads as a plan and not as the real thing.
fn planned(ui: &mut Ui, s: &Scene, points: &[Vec3], width: f32, alpha: f32, phase: f32) {
    dashed(ui, s, points, width, rgb(PLANNED, alpha), phase, Some(rgb(MASS, alpha.max(0.6))));
}

fn dashed(
    ui: &mut Ui,
    s: &Scene,
    points: &[Vec3],
    width: f32,
    color: crate::ui::Color,
    phase: f32,
    accent: Option<crate::ui::Color>,
) {
    dashed_by(ui, s, points, width, color, phase, accent, 8.0, 14.0);
}

/// `dashed` with dashes `on` points long every `period` points.
#[allow(clippy::too_many_arguments)]
fn dashed_by(
    ui: &mut Ui,
    s: &Scene,
    points: &[Vec3],
    width: f32,
    color: crate::ui::Color,
    phase: f32,
    accent: Option<crate::ui::Color>,
    on: f32,
    period: f32,
) {
    let scale = ui.s;
    let mut run = 0.0f32;
    let mut last = points.first().and_then(|&p| s.camera.project(p)).map(|p| p / scale);
    for &p in &points[1..] {
        let next = s.camera.project(p).map(|q| q / scale);
        if let (Some(a0), Some(b0)) = (last, next) {
            let full = a0.distance(b0);
            let (lo, hi) = screen_box(ui, s);
            let Some((a, b)) = clip(a0, b0, lo, hi) else {
                run += full;
                last = next;
                continue;
            };
            let skipped = a0.distance(a);
            let len = a.distance(b);
            // Dashes `on` points of every `period`, measured from the unclipped start.
            let mut t = 0.0;
            let run = run + skipped;
            let mut dashes = 0;
            let dash_on = on;
            while t < len && dashes < 400 {
                dashes += 1;
                let at = (run + t + phase) % period;
                let on = (dash_on - at).max(0.0).min(len - t);
                if at < dash_on && on > 0.0 {
                    let dash = ((run + t + phase) / period).floor() as i64;
                    let tone = match accent {
                        Some(c) if dash.rem_euclid(4) == 0 => c,
                        _ => color,
                    };
                    ui.stroke(a.lerp(b, t / len), a.lerp(b, (t + on) / len), width, tone);
                }
                t += if at < dash_on { on.max(0.5) } else { (period - at).max(0.5) };
            }
        }
        if let (Some(a), Some(b)) = (last, next) {
            run += a.distance(b);
        }
        last = next;
    }
}

/// The mine survey: while a core mine is placed or selected, or Ctrl is held.
/// The ore shows as veins deep underground; every mine in sight shows its
/// territory (a dim fill with sonar pulses and a marching edge, cut straight
/// where it meets a neighbour) and its workings: a main shaft straight down,
/// thicker with the tier, and a drift out to each field at that field's
/// depth, dug over time, the ore flowing back once it arrives. A mine being
/// placed shows the workings it would dig, and what it would make.
fn mine_marks(
    ui: &mut Ui,
    s: &Scene,
    ore: &mut Option<mc_sim::mines::OreGrid>,
) {
    let placing = match s.view.mode {
        Mode::Place(bp) => s.blueprints.unit(bp).mine.map(|m| (bp, m)),
        _ => None,
    };
    let selected: Vec<u32> = s
        .view
        .frame
        .units
        .iter()
        .filter(|u| s.view.selection.contains(&u.unit_id) && s.bp(u).mine.is_some())
        .map(|u| u.unit_id)
        .collect();
    if placing.is_none() && selected.is_empty() && !s.show_reclaim {
        return;
    }
    let time = ui.time;
    let far = ((s.camera.distance - 1200.0) / 5000.0).clamp(0.0, 1.0);
    let scale = ui.s;
    let viewport = s.camera.viewport / scale;
    let on_screen = |p: Vec2, margin: f32| {
        p.x > -margin && p.y > -margin && p.x < viewport.x + margin && p.y < viewport.y + margin
    };
    let regions = s.map.ore_regions();
    let fields: Vec<(Vec2, f32)> = regions
        .iter()
        .map(|r| (Vec2::from(r.centre().to_f32()), r.depth().to_f32()))
        .collect();

    // Mines in sight, and the one being placed.
    struct Site {
        at: Vec2,
        reach: f32,
        id: u32,
        tier: u8,
        /// Seconds it has been digging; unknown (anyone else's) counts as long done.
        age: Option<f32>,
        /// Metres out the land it works reaches so far.
        spread: f32,
    }
    let mut mines: Vec<Site> = s
        .view
        .frame
        .units
        .iter()
        .filter(|u| u.owner_flags & (KIND_WRECK | mc_sim::mirror::KIND_GHOST) == 0)
        .filter_map(|u| {
            let bp = s.bp(u);
            let m = bp.mine?;
            Some(Site {
                at: Vec2::new(u.pos[0], u.pos[1]),
                reach: m.reach.to_f32(),
                id: u.unit_id,
                tier: bp.tech,
                age: Some(s.queue_of(u.unit_id).and_then(|q| q.mine).map_or(1.0e9, |v| v.age)),
                spread: s.queue_of(u.unit_id).and_then(|q| q.mine).map_or(1.0e9, |v| v.spread),
            })
        })
        .collect();
    let ghost = placing.and_then(|(bp, m)| {
        Some(Site {
            at: Vec2::from(s.placing?.to_f32()),
            reach: m.reach.to_f32(),
            id: u32::MAX,
            tier: s.blueprints.unit(bp).tech,
            age: None,
            spread: 0.0,
        })
    });
    let ghost_at = ghost.as_ref().map(|g| g.at);
    mines.extend(ghost);
    let all: Vec<(Vec2, f32)> = mines.iter().map(|m| (m.at, m.reach)).collect();
    let ground = |xy: Vec2| {
        s.camera
            .project(xy.extend(overview_height(s.map, xy) + 1.5))
            .map(|p| p / scale)
    };

    // Late in a match there are dozens of mines, each with thousands of
    // vertices of survey, more than the overlay can spare. Which ones get
    // the full survey (sonar rings, workings) is settled up front from what
    // each would cost, which only the camera changes: the lit ones first,
    // then outward from the middle of the screen. The rest keep their fill
    // and tier edge, and only past even that do the farthest drop out. A
    // cut decided by the vertices actually drawn moved every frame with the
    // rings and dashes, and the mines at the end of the list flickered.
    struct Plan {
        i: usize,
        lit: bool,
        /// The territory, every few points of screen.
        outline: Vec<Vec2>,
        /// The same, coarser, for the rings and the inner edge.
        coarse: Vec<Vec2>,
        full: bool,
    }
    let middle = viewport * 0.5;
    let mut plans: Vec<(Plan, f32, usize, usize)> = Vec::new();
    for (i, site) in mines.iter().enumerate() {
        // Off screen: nothing of it shows (its cards are placed separately).
        let centre = site.at.extend(overview_height(s.map, site.at));
        let reach_px = site.reach * px_per_metre(s, scale, centre);
        let c = match s.camera.project(centre).map(|p| p / scale) {
            Some(c) if on_screen(c, reach_px * 1.5 + 200.0) => c,
            _ => continue,
        };
        let others: Vec<(Vec2, f32)> = all
            .iter()
            .enumerate()
            .filter(|&(j, &(p, r))| j != i && p.distance(site.at) < site.reach + r)
            .map(|(_, &o)| o)
            .collect();
        // Zoomed out a territory is small on screen: keep its segments a few
        // points long rather than drawing 240 of them. Every stride divides
        // 240, so the stepped outline still closes.
        let per_segment = TAU * reach_px / TERRITORY_SEGMENTS as f32;
        let strides = [24, 20, 16, 15, 12, 10, 8, 6, 5, 4, 3, 2, 1];
        let stride = strides.into_iter().find(|&k| per_segment * k as f32 <= 6.0).unwrap_or(1);
        let coarse_stride = strides
            .into_iter()
            .rev()
            .find(|&k| k % stride == 0 && per_segment * k as f32 >= 14.0)
            .unwrap_or(24);
        // Each edge a few points inside its own side, so where two meet both
        // lines show side by side, each in its own tier's look.
        let inset = 3.0 * site.reach / reach_px.max(1.0);
        let whole: Vec<Vec2> = territory(site.at, site.reach, &others)
            .into_iter()
            .map(|p| {
                let d = p - site.at;
                site.at + d.normalize_or_zero() * (d.length() - inset).max(0.0)
            })
            .collect();
        let outline: Vec<Vec2> = whole.iter().copied().step_by(stride).collect();
        let coarse: Vec<Vec2> = whole.into_iter().step_by(coarse_stride).collect();
        let look = tier_look(site.tier);
        let edge = outline.len() * 24 + if look.inner { coarse.len() * 18 } else { 0 };
        let extra = look.rings * coarse.len() * 18
            + if site.spread < site.reach { outline.len() * 18 } else { 0 }
            + 3000;
        let lit = site.id == u32::MAX || selected.contains(&site.id);
        let off_middle = if lit { -1.0 } else { c.distance(middle) };
        plans.push((Plan { i, lit, outline, coarse, full: false }, off_middle, edge, extra));
    }
    plans.sort_by(|a, b| a.1.total_cmp(&b.1).then(mines[a.0.i].id.cmp(&mines[b.0.i].id)));
    // Every mine's edge first, then as many full surveys as still fit. A
    // fixed allowance, not what the marks drawn before happen to leave: those
    // move with the units, and the cut would move with them.
    let mut left = SURVEY_BUDGET;
    let mut kept = 0;
    for (_, _, edge, _) in &plans {
        if *edge > left {
            break;
        }
        left -= edge;
        kept += 1;
    }
    plans.truncate(kept);
    for (plan, _, _, extra) in &mut plans {
        if *extra <= left {
            left = left.saturating_sub(*extra);
            plan.full = true;
        }
    }

    for (plan, _, _, _) in &plans {
        // Only if the estimates were far out: the panels drawn next need room.
        if ui.o.vertices.len() > mc_render::overlay::MAX_OVERLAY_VERTICES * 3 / 4 {
            break;
        }
        let Plan { i, lit, ref outline, ref coarse, full } = *plan;
        let site = &mines[i];
        let look = tier_look(site.tier);
        let tone = if site.id == u32::MAX { PLANNED } else { heat(MASS, look.heat) };
        let strength = if lit { 1.0 } else { 0.6 };

        // The territory: a dim fill, then sonar rings sweeping out from the
        // mine to its edge, then the edge itself in its tier's line.
        // Close in the territory is bigger than the screen: only the edge.
        if far > 0.15 {
            if let Some(c) = ground(site.at) {
                let pts: Vec<Option<Vec2>> = outline.iter().map(|&p| ground(p)).collect();
                let shade = rgb(tone, 0.035 * look.fill * strength * (1.0 + far));
                let near = |p: Vec2| p.x.abs() < viewport.x * 3.0 && p.y.abs() < viewport.y * 3.0;
                if near(c) && pts.iter().all(|p| p.is_some_and(near)) {
                    for pair in pts.windows(2) {
                        if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                            ui.triangle(c, a, b, shade);
                        }
                    }
                }
            }
        }
        // The land it works so far: its territory cut to the spread, a solid line.
        let worked_r = site.spread.min(site.reach);
        if full && worked_r < site.reach {
            let worked: Vec<Vec3> = outline
                .iter()
                .map(|&edge| {
                    let d = edge - site.at;
                    let p = site.at + d.normalize_or_zero() * d.length().min(worked_r);
                    p.extend(overview_height(s.map, p) + 1.5)
                })
                .collect();
            let pts: Vec<Option<Vec2>> = worked.iter().map(|&p| s.camera.project(p).map(|q| q / scale)).collect();
            smooth(ui, s, &pts, 1.8 + far * 1.6, rgb(tone, 0.8 * strength));
        }
        for k in 0..if full { look.rings } else { 0 } {
            let t = (time / 5.0 + k as f32 / look.rings as f32 + (site.id % 97) as f32 * 0.17).fract();
            let r = worked_r * t;
            let ring: Vec<Option<Vec2>> = coarse
                .iter()
                .map(|&edge| {
                    let d = edge - site.at;
                    (d.length() > r).then(|| ground(site.at + d.normalize_or_zero() * r)).flatten()
                })
                .collect();
            let a = 0.35 * strength * (1.0 - t) * t * 4.0;
            smooth(ui, s, &ring, 1.2 + far * 1.5, rgb(tone, a));
        }
        let lift = |p: Vec2| p.extend(overview_height(s.map, p) + 1.5);
        let edge: Vec<Vec3> = outline.iter().map(|&p| lift(p)).collect();
        let width = look.width + far * 1.6;
        if site.spread < site.reach {
            planned(ui, s, &edge, width, 0.6 * strength, time * 18.0);
        } else if let Some((on, period)) = look.dash {
            dashed_by(ui, s, &edge, width, rgb(tone, 0.55 * strength), time * 18.0, None, on, period);
        } else {
            let pts: Vec<Option<Vec2>> = edge.iter().map(|&p| s.camera.project(p).map(|q| q / scale)).collect();
            smooth(ui, s, &pts, width, rgb(tone, 0.6 * strength));
        }
        // The top tiers' second, inner edge: a double border reads as rank.
        if look.inner {
            let inset: Vec<Option<Vec2>> = coarse
                .iter()
                .map(|&p| s.camera.project(lift(site.at + (p - site.at) * 0.94)).map(|q| q / scale))
                .collect();
            smooth(ui, s, &inset, 1.0 + far, rgb(tone, 0.35 * strength));
        }
        if !full {
            continue;
        }

        // The workings: a main shaft down, drifts out at each field's depth.
        let owned: Vec<usize> = fields
            .iter()
            .enumerate()
            .filter(|&(_, &(c, _))| owns(c, site.at, site.reach, &all))
            .map(|(f, _)| f)
            .collect();
        let bottom = owned.iter().map(|&f| fields[f].1).fold(0.0f32, f32::max);
        if bottom <= 0.0 {
            continue;
        }
        let shaft_r = [5.0, 8.0, 12.0, 16.0][(site.tier.clamp(1, 4) - 1) as usize];
        let top = underground(s, site.at, 0.0);
        let shaft_speed = mc_sim::mines::SHAFT_SPEED as f32;
        let drift_speed = mc_sim::mines::DRIFT_SPEED as f32;
        let dug = site.age.map_or(0.0, |age| (age * shaft_speed).min(bottom));
        let alpha = 0.9 * strength;
        // What is still to dig: a dashed plan.
        if dug < bottom {
            planned(ui, s, &[underground(s, site.at, dug), underground(s, site.at, bottom)], 1.4, 0.6 * strength, time * 12.0);
        }
        if dug > 0.0 {
            let foot = underground(s, site.at, dug);
            tube(ui, s, (top, shaft_r), (foot, shaft_r), 2.5 + site.tier as f32, MASS, alpha);
            if dug < bottom {
                let pulse = 0.5 + 0.5 * (time * 6.0).sin();
                if let Some(p) = s.camera.project(foot) {
                    ui.disc(p / scale, 3.0 + 3.0 * pulse, rgb(MASS, 0.9));
                    ui.arc(p / scale, 8.0 + 6.0 * pulse, 0.0, TAU, 1.2, rgb(MASS, 0.6 * (1.0 - pulse)));
                }
            }
        }
        for &f in &owned {
            let (c, depth) = fields[f];
            let from = underground(s, site.at, depth);
            let to = underground(s, c, depth);
            let length = site.at.distance(c);
            // Seconds since the shaft reached this depth, as far as we know.
            let driven = site.age.map_or(0.0, |age| ((age - depth / shaft_speed) * drift_speed).clamp(0.0, length));
            let drift_r = shaft_r * 0.55;
            if driven < length {
                let head = from.lerp(to, if length > 0.0 { driven / length } else { 1.0 });
                planned(ui, s, &[head, to], 1.2, 0.55 * strength, time * 12.0);
                if driven > 0.0 {
                    tube(ui, s, (from, drift_r), (head, drift_r), 2.0, MASS, alpha);
                    let pulse = 0.5 + 0.5 * (time * 6.0 + f as f32).sin();
                    if let Some(p) = s.camera.project(head) {
                        ui.disc(p / scale, 2.5 + 2.5 * pulse, rgb(MASS, 0.9));
                    }
                }
            } else {
                tube(ui, s, (from, drift_r), (to, drift_r), 2.0, MASS, alpha);
                // Reached: ore running back along the drift and up the shaft.
                let path = [to, from, top];
                let legs = [length.max(1.0), depth.max(1.0)];
                let total = legs[0] + legs[1];
                for k in 0..6 {
                    let t = ((time * 60.0 / total) + k as f32 / 6.0 + f as f32 * 0.13).fract() * total;
                    let (a, b, u) = if t < legs[0] {
                        (path[0], path[1], t / legs[0])
                    } else {
                        (path[1], path[2], (t - legs[0]) / legs[1])
                    };
                    if let Some(p) = s.camera.project(a.lerp(b, u)) {
                        ui.disc(p / scale, 2.2 + far, rgb(0xFFFFFF, 0.8));
                    }
                }
            }
        }
    }

    // The deposits themselves are real geometry the renderer draws through
    // the ground (`ore_vein_mesh`, `fs_vein`) while the survey is up.

    // The viewer's mines: a selected one gets a card of its own and one on
    // every ore field it is going for; the rest a small readout. Cards make
    // room for each other, the mines' first.
    let mut taken: Vec<Rect> = Vec::new();
    let mut deposits: Vec<(Vec2, Option<Vec2>, mc_sim::mirror::MineVein)> = Vec::new();
    for u in &s.view.frame.units {
        let Some(q) = s.queue_of(u.unit_id) else {
            continue;
        };
        let Some(view) = q.mine else {
            continue;
        };
        let at = Vec2::new(u.pos[0], u.pos[1]);
        let Some(p) = ground(at) else {
            continue;
        };
        let bp = s.bp(u);
        if !selected.contains(&u.unit_id) {
            if on_screen(p, 60.0) {
                mine_readout(ui, p, &view);
            }
            continue;
        }
        for vein in &q.mine_veins {
            let Some(&(c, depth)) = fields.get(vein.field as usize) else {
                continue;
            };
            let Some(anchor) = ground(c) else {
                continue;
            };
            if !on_screen(anchor, 80.0) {
                continue;
            }
            let heart = s.camera.project(underground(s, c, depth)).map(|p| p / scale);
            deposits.push((anchor, heart, *vein));
        }
        if on_screen(p, 120.0) {
            mine_card(ui, p, bp, &view, &q.mine_veins, time, &mut taken);
        }
    }
    let soonest = deposits
        .iter()
        .map(|d| d.2.eta)
        .filter(|&eta| eta > 0.0)
        .fold(f32::INFINITY, f32::min);
    for (anchor, heart, vein) in deposits {
        let next = vein.eta == soonest;
        deposit_card(ui, anchor, heart, &vein, next, time, &mut taken);
    }

    // What the mine under the pointer would make there.
    let (Some((bp, mine)), Some(fx), Some(site)) = (placing, s.placing, ghost_at) else {
        return;
    };
    let grid = ore.get_or_insert_with(|| {
        let water = s.map.info().water_level.to_f32();
        mc_sim::mines::OreGrid::new(s.map.ore_regions(), s.map.info().size_metres(), |p| {
            overview_height(s.map, Vec2::from(p.to_f32())) > water
        })
    });
    let others: Vec<(mc_core::FxVec2, mc_core::Fx)> = mines
        .iter()
        .filter(|m| m.id != u32::MAX)
        .map(|m| (m.at, m.reach))
        .filter(|&(p, r)| p.distance(site) < mine.reach.to_f32() + r)
        .map(|(p, r)| {
            (
                mc_core::FxVec2::new(mc_core::Fx::from_f32(p.x), mc_core::Fx::from_f32(p.y)),
                mc_core::Fx::from_f32(r),
            )
        })
        .collect();
    let share = grid.share(fx, mine.reach, &others);
    let rate = share.rate(&mine).to_f32();
    let efficiency = share.efficiency(&mine).to_f32();
    let cost = s.blueprints.unit(bp).cost_mass.to_f32();
    let z = overview_height(s.map, site);
    let Some(p) = s.camera.project(site.extend(z)) else {
        return;
    };
    let c = p / ui.s;
    let land = if share.ore > mc_core::Fx::ZERO {
        format!("{:.0} ha of land  \u{b7}  {:.1} ha of ore", share.ground.to_f32(), share.ore.to_f32())
    } else {
        format!("{:.0} ha of land  \u{b7}  no ore", share.ground.to_f32())
    };
    let tone = if efficiency >= 0.9 {
        palette::TEXT
    } else if efficiency >= 0.6 {
        palette::WARN
    } else {
        palette::BAD
    };
    let payback = if rate > 0.0 {
        format!("Pays back in {}", mine::duration(cost / rate))
    } else {
        "Never pays back".to_owned()
    };
    let lines = [
        (format!("{rate:.1} materials/s"), MASS),
        (format!("Efficiency {:.0}%  \u{b7}  {payback}", efficiency * 100.0), tone),
        (land, palette::DIM),
    ];
    let w = 236.0;
    let r = Rect::new(c.x + 40.0, c.y - 28.0, w, 56.0);
    ui.frost(r, 0.9);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(MASS, 1.0));
    for (i, (text, tone)) in lines.iter().enumerate() {
        ui.text(r.x + 10.0, r.y + 5.0 + i as f32 * 16.0, type_scale::CAPTION, rgb(*tone, 1.0), text);
    }
}

/// A mine's small readout over it: what it makes, and how much of its reach it has.
fn mine_readout(ui: &mut Ui, p: Vec2, view: &mc_sim::mirror::MineView) {
    let tone = share_tone(view.share);
    let rate = format!("{:.1}/s", view.rate);
    let share = if view.rate + 0.05 < view.full {
        format!("{:.0}%  \u{b7}  growing", view.share * 100.0)
    } else {
        format!("{:.0}%", view.share * 100.0)
    };
    let w = 26.0 + ui.text_width(type_scale::VALUE, &rate) + ui.text_width(type_scale::MICRO, &share);
    let r = Rect::new(p.x - w * 0.5, p.y - 44.0, w, 22.0);
    ui.frost(r, 0.9);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(MASS, 1.0));
    let after = r.x + 8.0 + ui.text_width(type_scale::VALUE, &rate) + 6.0;
    ui.text(r.x + 8.0, r.y + 4.0, type_scale::VALUE, rgb(MASS, 1.0), &rate);
    ui.text(after, r.y + 6.0, type_scale::MICRO, rgb(tone, 1.0), &share);
    ui.stroke(Vec2::new(p.x, r.bottom()), p, 1.4, rgb(MASS, 0.8));
}

fn share_tone(share: f32) -> u32 {
    if share >= 0.9 {
        palette::TEXT
    } else if share >= 0.6 {
        palette::WARN
    } else {
        palette::BAD
    }
}

/// `r` moved up, then down, until it overlaps none of `taken`; then taken.
fn make_room(mut r: Rect, taken: &mut Vec<Rect>) -> Rect {
    let hits = |r: &Rect, taken: &[Rect]| {
        taken.iter().find(|t| {
            r.x < t.right() + 4.0 && t.x < r.right() + 4.0 && r.y < t.bottom() + 4.0 && t.y < r.bottom() + 4.0
        }).copied()
    };
    let start = r;
    for step in 0..12 {
        match hits(&r, taken) {
            None => break,
            Some(t) => {
                r = if step < 6 {
                    Rect::new(r.x, t.y - r.h - 6.0, r.w, r.h)
                } else {
                    Rect::new(start.x, t.bottom() + 6.0, r.w, r.h)
                };
            }
        }
    }
    taken.push(r);
    r
}

/// A thin bar: `k` of it filled in `fill`, the rest in the planned grey.
fn progress(ui: &mut Ui, r: Rect, k: f32, fill: u32) {
    ui.fill(r, rgb(PLANNED, 0.22));
    ui.fill(Rect::new(r.x, r.y, r.w * k.clamp(0.0, 1.0), r.h), rgb(fill, 0.95));
}

/// A selected mine's card, just over it: what it makes now and, while it is
/// still growing (its land spreading, drifts on their way to the ore), what
/// it will make and when, with a bar for how far along it is. Efficiency only
/// shows when a neighbour is taking some of its reach.
fn mine_card(
    ui: &mut Ui,
    p: Vec2,
    bp: &UnitBlueprint,
    view: &mc_sim::mirror::MineView,
    veins: &[mc_sim::mirror::MineVein],
    time: f32,
    taken: &mut Vec<Rect>,
) {
    let reach = bp.mine.map_or(1.0, |m| m.reach.to_f32());
    let spread = view.spread.min(reach);
    // Grown once the land is all worked and the last drift is in.
    let land_left = (reach - spread) / mc_sim::mines::SPREAD_SPEED as f32;
    let left = veins.iter().map(|v| v.eta).fold(land_left, f32::max).max(0.0);
    let growing = left > 0.5 && view.rate + 0.05 < view.full;
    let shared = view.share < 0.9;
    let h = 38.0 + if growing { 8.0 } else { 0.0 } + if shared { 14.0 } else { 0.0 };
    let w = 200.0;
    // The reticle keeps its own room, so no field's chip lands on the mine.
    taken.push(Rect::new(p.x - 24.0, p.y - 24.0, 48.0, 48.0));
    let r = make_room(Rect::new(p.x - w * 0.5, p.y - 34.0 - h, w, h), taken);
    // A reticle on the mine, turning slowly.
    for k in 0..4 {
        let a = time * 0.6 + k as f32 * std::f32::consts::FRAC_PI_2;
        let d = Vec2::new(a.cos(), a.sin());
        ui.stroke(p + d * 14.0, p + d * 22.0, 2.0, rgb(MASS, 0.9));
    }
    ui.arc(p, 17.0, 0.0, TAU, 1.2, rgb(MASS, 0.5));
    ui.stroke(Vec2::new(r.x + r.w * 0.5, r.bottom()), p - Vec2::Y * 22.0, 1.2, rgb(MASS, 0.6));

    ui.frost(r, 0.92);
    ui.fill(Rect::new(r.x, r.y, 3.0, r.h), rgb(MASS, 1.0));
    let x = r.x + 12.0;
    let right = r.right() - 10.0;
    ui.text(x, r.y + 8.0, type_scale::CAPTION, rgb(palette::TEXT, 1.0), &bp.name);
    ui.text_right(right, r.y + 7.0, type_scale::VALUE, rgb(MASS, 1.0), &format!("{:.1}/s", view.rate));
    let mut y = r.y + 26.0;
    if growing {
        let after = ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Grows to ");
        ui.text(after, y, type_scale::MICRO, rgb(MASS, 1.0), &format!("{:.1}/s", view.full));
        ui.text_right(right, y, type_scale::MICRO, rgb(palette::TEXT, 0.9), &mine::duration(left));
        let done = view.age / (view.age + left).max(1.0);
        progress(ui, Rect::new(x, y + 10.0, right - x, 3.0), done, MASS);
        y += 22.0;
    } else {
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Fully grown");
        y += 14.0;
    }
    if shared {
        ui.text(
            x,
            y,
            type_scale::MICRO,
            rgb(share_tone(view.share), 1.0),
            &format!("Shares its land  \u{b7}  {:.0}% efficient", view.share * 100.0),
        );
    }
}

/// An ore field a selected mine is going for: a ring on the field that fills
/// as the drift comes, and beside it a chip of what the field adds and when
/// it starts; the soonest one brightest. Once reached, the ring is solid and
/// the chip says so.
fn deposit_card(
    ui: &mut Ui,
    p: Vec2,
    heart: Option<Vec2>,
    vein: &mc_sim::mirror::MineVein,
    next: bool,
    time: f32,
    taken: &mut Vec<Rect>,
) {
    let reached = vein.eta <= 0.0;
    let tone = if reached { MASS } else { PLANNED };
    if let Some(h) = heart {
        ui.stroke(p, h, 1.0, rgb(tone, 0.35));
    }
    let start = -std::f32::consts::FRAC_PI_2;
    if reached {
        let pulse = 0.5 + 0.5 * (time * 3.0).sin();
        ui.disc(p, 3.0, rgb(MASS, 1.0));
        ui.arc(p, 7.0, 0.0, TAU, 2.0, rgb(MASS, 1.0));
        ui.arc(p, 10.0 + 4.0 * pulse, 0.0, TAU, 1.0, rgb(MASS, 0.5 * (1.0 - pulse)));
    } else {
        let done = if vein.dig > 0.0 { (1.0 - vein.eta / vein.dig).clamp(0.0, 1.0) } else { 1.0 };
        ui.arc(p, 7.0, 0.0, TAU, 2.0, rgb(PLANNED, 0.45));
        if done > 0.0 {
            ui.arc(p, 7.0, start, start + TAU * done, 2.0, rgb(MASS, 1.0));
        }
    }

    let rate = format!("+{:.1}/s", vein.rate);
    let note = if reached { "mining".to_owned() } else { mine::duration(vein.eta) };
    let w = 22.0 + ui.text_width(type_scale::VALUE, &rate) + ui.text_width(type_scale::MICRO, &note);
    let h = 20.0;
    // Right of the ring, else left of it, else wherever there is room.
    let beside = [
        Rect::new(p.x + 13.0, p.y - h * 0.5, w, h),
        Rect::new(p.x - 13.0 - w, p.y - h * 0.5, w, h),
    ];
    let clear = |r: &Rect| {
        !taken.iter().any(|t| r.x < t.right() + 4.0 && t.x < r.right() + 4.0 && r.y < t.bottom() + 4.0 && t.y < r.bottom() + 4.0)
    };
    let r = match beside.iter().find(|r| clear(r)) {
        Some(&r) => {
            taken.push(r);
            r
        }
        None => {
            let r = make_room(beside[0], taken);
            let end = Vec2::new(r.x, r.y + r.h * 0.5);
            ui.stroke(p + (end - p).normalize_or_zero() * 8.0, end, 1.0, rgb(tone, 0.6));
            r
        }
    };
    let strong = reached || next;
    ui.frost(r, if strong { 0.9 } else { 0.75 });
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(tone, 1.0));
    let after = ui.text(r.x + 8.0, r.y + 10.0, type_scale::VALUE, rgb(MASS, if strong { 1.0 } else { 0.7 }), &rate);
    let note_tone = if reached {
        rgb(MASS, 1.0)
    } else if next {
        rgb(palette::TEXT, 0.95)
    } else {
        rgb(palette::DIM, 1.0)
    };
    ui.text(after + 6.0, r.y + 10.0, type_scale::MICRO, note_tone, &note);
}

/// A circle of `radius` metres round `centre`, laid on the ground as a line,
/// dashed when `dashed`.
fn ground_circle(
    ui: &mut Ui,
    s: &Scene,
    centre: Vec2,
    radius: f32,
    width: f32,
    color: crate::ui::Color,
    dashed: bool,
) {
    const SEGMENTS: usize = 96;
    let scale = ui.s;
    let point = |i: usize| {
        let a = i as f32 / SEGMENTS as f32 * TAU;
        let xy = centre + Vec2::new(a.cos(), a.sin()) * radius;
        s.camera
            .project(xy.extend(overview_height(s.map, xy) + 1.0))
            .map(|p| p / scale)
    };
    if !dashed {
        let pts: Vec<Option<Vec2>> = (0..=SEGMENTS).map(point).collect();
        smooth(ui, s, &pts, width, color);
        return;
    }
    let mut last = point(0);
    for i in 1..=SEGMENTS {
        let next = point(i);
        if let (Some(a), Some(b)) = (last, next) {
            if i % 2 == 0 {
                ui.stroke(a, b, width, color);
            }
        }
        last = next;
    }
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
        hover: Option<u32>,
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
                recoil: 0.0,
                prev_recoil: 0.0,
                weld_first: 0,
                weld_count: 0,
                deploy: 0.0,
                prev_deploy: 0.0,
                _pad2: [0.0; 2],
                refit_modules: 0,
                _pad3: [0; 3],
                mount: [0.0; 4],
                spin_recoil: [0.0; 4],
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
                hover: None,
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
                hover: self.hover,
                show_reclaim: false,
                placing: None,
            };
            let actions = self.hud.draw(&mut ui, &scene, 0.016);
            self.memory.end_frame(input);
            assert!(!self.overlay.overflowed, "the HUD overflowed the overlay");
            actions
        }

        /// Frames enough for every panel to finish sliding in or out.
        fn settle(&mut self) {
            for _ in 0..60 {
                self.frame(&Input::default());
            }
        }

        /// Hover, press, release at `at`; returns what the release produced.
        fn click(&mut self, at: Vec2) -> Vec<HudAction> {
            self.settle();
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
            self.settle();
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
    const INFO_X: f32 = EDGE;
    const ORDERS_X: f32 = INFO_X + 336.0 + GAP;
    /// The construction panel's left edge, after an order card of `families` columns.
    fn build_x(families: usize) -> f32 {
        ORDERS_X + selection::orders_width(families) + GAP
    }
    fn first_tile(families: usize) -> Vec2 {
        // Inside the first tile whether or not the strip has its end arrows.
        Vec2::new(build_x(families) + 14.0 + 30.0 + 40.0, DECK_Y + 44.0 + 32.0 + 40.0)
    }
    /// The middle of order `row` in family column `col`.
    fn order_slot(col: usize, row: usize) -> Vec2 {
        Vec2::new(
            ORDERS_X + 14.0 + col as f32 * (selection::ORDER_W + selection::ORDER_GAP) + 30.0,
            DECK_Y + 36.0 + row as f32 * (selection::ORDER_H + selection::ORDER_GAP) + 20.0,
        )
    }
    /// The top bar: its left edge, the speed control's parts, pause.
    fn top_bar() -> f32 {
        1920.0 - EDGE - (146.0 + 50.0 + SPEED_W + 10.0 + 50.0 + 98.0)
    }
    fn speed_slower() -> Vec2 {
        Vec2::new(top_bar() + 196.0 + 15.0, EDGE + 22.0)
    }
    fn speed_faster() -> Vec2 {
        Vec2::new(top_bar() + 196.0 + SPEED_W - 15.0, EDGE + 22.0)
    }
    fn speed_centre() -> Vec2 {
        Vec2::new(top_bar() + 196.0 + SPEED_W * 0.5, EDGE + 22.0)
    }
    /// Row `i` of the open speed list.
    fn speed_pick(i: usize) -> Vec2 {
        Vec2::new(top_bar() + 196.0 + SPEED_W * 0.5, EDGE + 44.0 + 4.0 + 4.0 + i as f32 * 28.0 + 14.0)
    }
    fn pause_button() -> Vec2 {
        Vec2::new(top_bar() + 196.0 + SPEED_W + 10.0 + 20.0, EDGE + 22.0)
    }
    /// Middle of the minimap's chart: under the top bar and the two-player roster.
    fn chart() -> Vec2 {
        let top = EDGE + 44.0 + GAP + 2.0 * 22.0 + 12.0 + GAP;
        Vec2::new(1920.0 - EDGE - MINIMAP * 0.5, top + 11.0 + MINIMAP * 0.5)
    }
    /// What the first tile of tier `tech` offers: the tier's builds on shelves by purpose.
    fn first_on_shelves(rig: &Rig, key: &str, tech: u8) -> BlueprintId {
        let mut items: Vec<&UnitBlueprint> = rig
            .blueprints
            .unit(rig.blueprints.id_of(key).unwrap())
            .builder
            .as_ref()
            .unwrap()
            .builds
            .iter()
            .map(|b| rig.blueprints.unit(*b))
            .filter(|b| b.tech == tech)
            .collect();
        items.sort_by_key(|b| style::Purpose::of(b));
        items[0].id
    }

    /// Tank families: movement, combat, fire state, control.
    const TANK_FAMILIES: usize = 4;
    /// A factory only has STOP.
    /// A land factory offers what its units can do: movement, combat, stance, engineering.
    const FACTORY_FAMILIES: usize = 4;

    #[test]
    fn range_weather_is_picked_from_a_list_and_waits_for_apply() {
        use crate::range::{Range, RangeAction};
        use mc_data::weather::{TimeOfDay, WeatherPreset};
        let mut rig = Rig::new("aster_t1_tank");
        let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
        rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
        rig.view.range.as_mut().unwrap().sky = Some(Default::default());
        // The Sky tab; the middle of the Weather value opens its list, it does not step.
        assert!(rig.click(Vec2::new(253.0, 279.0)).is_empty());
        assert!(rig.click(Vec2::new(238.0, 320.0)).is_empty());
        let popup = rig.memory.popup.as_ref().expect("the weather list is open");
        let anchor = popup.anchor();
        let stormy = popup.row_centre(4, VIEWPORT);
        // While it is open the whole screen belongs to it.
        assert!(rig.hud.covers(Vec2::new(1800.0, 700.0)));
        // Picking a row changes the draft, not the range's weather.
        assert!(rig.click(stormy).is_empty());
        assert!(rig.memory.popup.is_none());
        rig.frame(&Input::default());
        let draft = rig.hud.range_sky.expect("a draft waits for Apply");
        assert_eq!(draft.choice.preset, Some(WeatherPreset::Stormy));
        // The arrows still step: the right one on the Time of Day row goes to Dawn.
        let time_arrow = Vec2::new(anchor.right() - 12.0, anchor.mid_y() + 32.0);
        assert!(rig.click(time_arrow).is_empty());
        assert_eq!(rig.hud.range_sky.unwrap().choice.time, Some(TimeOfDay::Dawn));
        // Apply sits right of Storm Overhead, under the two rows.
        let apply = Vec2::new(anchor.right() - 40.0, anchor.mid_y() + 2.0 * 32.0);
        let asked = rig.click(apply);
        let Some(HudAction::Range(RangeAction::Sky(sky))) = asked.first() else {
            panic!("Apply asked for {asked:?}");
        };
        assert_eq!(sky.choice.preset, Some(WeatherPreset::Stormy));
        assert_eq!(sky.choice.time, Some(TimeOfDay::Dawn));
    }

    #[test]
    fn range_browser_search_pick_cancel_and_background_capture() {
        use crate::range::{Range, RangeAction};
        use crate::ui::Key;
        let mut rig = Rig::new("aster_t1_tank");
        let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
        rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
        assert!(rig.click(Vec2::new(180.0, 152.0)).is_empty());
        assert!(rig.hud.unit_picker_open());
        assert!(rig.hud.covers(Vec2::new(1800.0, 700.0)));
        // A covered battlefield/reset click cannot produce a range action.
        assert!(rig.click(Vec2::new(184.0, 615.0)).is_empty());
        rig.frame(&Input {
            typed: "wArDeN".into(),
            ..Default::default()
        });
        let asked = rig.frame(&Input {
            keys: vec![Key::Enter],
            ..Default::default()
        });
        assert_eq!(
            asked,
            vec![HudAction::Range(RangeAction::PickSubject(tank))]
        );
        assert!(!rig.hud.unit_picker_open());
        assert!(rig.memory.editing.is_none());

        rig.click(Vec2::new(180.0, 152.0));
        rig.frame(&Input {
            typed: "no such unit".into(),
            ..Default::default()
        });
        assert!(rig
            .frame(&Input {
                keys: vec![Key::Enter],
                ..Default::default()
            })
            .is_empty());
        assert!(rig.hud.unit_picker_open());
        assert!(rig
            .frame(&Input {
                keys: vec![Key::Escape],
                ..Default::default()
            })
            .is_empty());
        assert!(!rig.hud.unit_picker_open());
        assert!(rig.memory.editing.is_none());
    }

    #[test]
    fn range_browser_picks_t4_t5_and_space_for_both_controls() {
        use crate::range::{Range, RangeAction};
        use crate::ui::Key;
        let mut rig = Rig::new("aster_t1_tank");
        let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
        rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
        for target in [unit_picker::Target::Subject, unit_picker::Target::Spawn] {
            for (filter, key) in [
                (Vec2::new(758.0, 283.0), "aster_t4_assault_tank"),
                (Vec2::new(844.0, 283.0), "replication_engine"),
                (Vec2::new(1128.0, 242.0), "aster_t2_lift_ship"),
            ] {
                rig.hud.unit_picker = Some(unit_picker::Picker::new(target));
                assert!(rig.click(filter).is_empty());
                let chosen = rig.blueprints.id_of(key).unwrap();
                let asked = rig.frame(&Input { keys: vec![Key::Enter], ..Default::default() });
                let expected = match target {
                    unit_picker::Target::Subject => RangeAction::PickSubject(chosen),
                    unit_picker::Target::Spawn => RangeAction::PickSpawn(chosen),
                };
                assert_eq!(asked, vec![HudAction::Range(expected)], "{key}");
                assert!(!rig.hud.unit_picker_open());
            }
        }
        // Space and tech intersect; clearing an empty combination restores the catalog.
        rig.hud.unit_picker = Some(unit_picker::Picker::new(unit_picker::Target::Spawn));
        rig.click(Vec2::new(1128.0, 242.0));
        rig.click(Vec2::new(844.0, 283.0));
        assert!(rig.frame(&Input { keys: vec![Key::Enter], ..Default::default() }).is_empty());
        assert!(rig.hud.unit_picker_open());
        rig.click(Vec2::new(1450.0, 189.0));
        rig.frame(&Input { typed: "space".into(), ..Default::default() });
        assert_eq!(
            rig.frame(&Input { keys: vec![Key::Enter], ..Default::default() }),
            vec![HudAction::Range(RangeAction::PickSpawn(
                rig.blueprints.id_of("aster_t2_lift_ship").unwrap()
            ))]
        );
    }

    #[test]
    fn range_browser_card_click_and_paging_reach_the_catalog() {
        use crate::range::{Range, RangeAction};
        let mut rig = Rig::new("aster_t1_tank");
        let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
        rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
        let mut sorted: Vec<_> = rig
            .blueprints
            .units
            .iter()
            .filter(|b| rig.blueprints.is_listed(b.id))
            .collect();
        sorted.sort_by(|a, b| (a.tech, &a.name, &a.key).cmp(&(b.tech, &b.name, &b.key)));
        let first = sorted[0].id;
        let last = sorted.last().unwrap().id;
        rig.click(Vec2::new(180.0, 152.0));
        assert_eq!(
            rig.click(Vec2::new(450.0, 355.0)),
            vec![HudAction::Range(RangeAction::PickSubject(first))]
        );
        rig.click(Vec2::new(180.0, 152.0));
        for _ in 0..rig.blueprints.units.len() {
            rig.frame(&Input {
                scroll: -1.0,
                ..Default::default()
            });
        }
        assert_eq!(
            rig.frame(&Input {
                keys: vec![crate::ui::Key::Enter],
                ..Default::default()
            }),
            vec![HudAction::Range(RangeAction::PickSubject(last))]
        );
    }

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
        // Where things are at 1920x1080: see `range::draw`. The panel's top is at 94,
        // its tab strip at 266..292, and the open tab's page starts at 306.
        let range = |a| vec![HudAction::Range(a)];
        assert_eq!(
            rig.click(Vec2::new(170.0, 206.0)),
            range(RangeAction::Side(crate::range::Side::Blue)),
            "the duplicate's team"
        );
        assert_eq!(
            rig.click(Vec2::new(320.0, 116.0)),
            range(RangeAction::Control(RED)),
            "the side commanded sits by the title"
        );
        assert_eq!(rig.click(Vec2::new(333.0, 152.0)), range(RangeAction::Subject(1)));
        assert_eq!(rig.click(Vec2::new(270.0, 238.0)), range(RangeAction::ArmSpawn));

        // The Unit tab is open first.
        assert_eq!(
            rig.click(Vec2::new(308.0, 336.0)),
            range(RangeAction::Damage(1000)),
            "Kill"
        );
        assert_eq!(
            rig.click(Vec2::new(295.0, 368.0)),
            range(RangeAction::Flag(flag::INVULNERABLE, true))
        );
        // Holding the build track half way along asks for a half-built unit.
        let half = Vec2::new(80.0 + 216.0 * 0.5, 404.0);
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
        rig.frame(&Input::default());
        // Reset is always under the page.
        assert_eq!(rig.click(Vec2::new(184.0, 449.0)), range(RangeAction::Reset));
        assert!(
            rig.hud.covers(Vec2::new(180.0, 400.0)),
            "a click on the panel must not reach the battlefield"
        );

        // With nothing selected it acts on every unit of the subject's type; with none of those, on nothing.
        rig.view.selection.clear();
        assert_eq!(
            rig.click(Vec2::new(308.0, 336.0)),
            range(RangeAction::Damage(1000))
        );
        let units = std::mem::take(&mut rig.view.frame.units);
        let index = std::mem::take(&mut rig.view.index_of);
        assert_eq!(rig.click(Vec2::new(308.0, 336.0)), vec![]);
        (rig.view.frame.units, rig.view.index_of) = (units, index);

        // Stage: scenarios around the subject, then what it does itself.
        assert!(rig.click(Vec2::new(127.0, 279.0)).is_empty(), "a tab is not an order");
        assert_eq!(
            rig.click(Vec2::new(230.0, 322.0)),
            range(RangeAction::Scenario(Scenario::Targets))
        );
        assert_eq!(
            rig.click(Vec2::new(230.0, 358.0)),
            range(RangeAction::Scenario(Scenario::March)),
            "the second row: what the subject does itself"
        );
        // The panel is shorter on this tab, and Reset came up with it.
        assert_eq!(rig.click(Vec2::new(184.0, 405.0)), range(RangeAction::Reset));

        // Range: the camera presets.
        assert!(rig.click(Vec2::new(316.0, 279.0)).is_empty());
        assert_eq!(rig.click(Vec2::new(300.0, 336.0)), range(RangeAction::Zoom(2)));
    }

    #[test]
    fn the_range_economy_tab_fills_starves_and_scatters_wrecks() {
        use crate::range::{Range, RangeAction, BLUE, INCOME_NORMAL, RED};
        let mut rig = Rig::new("aster_t1_tank");
        let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
        rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
        for p in &mut rig.view.status.players {
            (p.mass_capacity, p.energy_capacity) = (1000.0, 5000.0);
        }
        let range = |a| vec![HudAction::Range(a)];
        assert!(rig.click(Vec2::new(190.0, 279.0)).is_empty(), "the Economy tab");
        assert_eq!(
            rig.click(Vec2::new(160.0, 368.0)),
            range(RangeAction::Stock { player: BLUE, mass: Some(1000), energy: None }),
            "fill the materials"
        );
        assert_eq!(
            rig.click(Vec2::new(270.0, 368.0)),
            range(RangeAction::Income { player: BLUE, resource: 0, step: 1 })
        );
        // Red's economy is its own.
        assert!(rig.click(Vec2::new(122.0, 318.0)).is_empty());
        assert_eq!(
            rig.click(Vec2::new(57.0, 428.0)),
            range(RangeAction::Stock { player: RED, mass: None, energy: Some(0) }),
            "empty red's energy"
        );
        // A power shortage in one click: free build off, a quarter of the income, the store dry.
        assert_eq!(
            rig.click(Vec2::new(150.0, 466.0)),
            vec![
                HudAction::Range(RangeAction::FreeBuild(false)),
                HudAction::Range(RangeAction::SetIncome { player: RED, resource: 1, index: 2 }),
                HudAction::Range(RangeAction::Stock { player: RED, mass: None, energy: Some(0) }),
            ]
        );
        assert_eq!(rig.click(Vec2::new(71.0, 466.0)), range(RangeAction::FreeBuild(false)));
        let normal = rig.click(Vec2::new(308.0, 466.0));
        assert!(normal.contains(&HudAction::Range(RangeAction::SetIncome {
            player: RED,
            resource: 1,
            index: INCOME_NORMAL
        })));
        assert_eq!(rig.click(Vec2::new(100.0, 500.0)), range(RangeAction::Wrecks));
    }

    #[test]
    fn formation_controls_emit_actions_and_capture_their_clicks() {
        let mut rig = Rig::new("aster_t1_tank");
        // Formation is for more than one unit.
        let mut second = rig.view.frame.units[0];
        second.unit_id = 9;
        rig.view.frame.units.push(second);
        rig.view.index_of.insert(9, 1);
        rig.view.selection.push(9);
        assert_eq!(rig.click(order_slot(0, 2)), vec![HudAction::FormationPanel]);
        rig.view.formation_panel = true;
        let w = selection::orders_width(TANK_FAMILIES);
        let (px, py) = (ORDERS_X, DECK_Y - 154.0);
        let cw = w - 24.0;
        assert_eq!(
            rig.click(Vec2::new(px + 12.0 + cw * 0.75, py + 42.0)),
            vec![HudAction::FormationTogether(false)]
        );
        assert_eq!(
            rig.click(Vec2::new(px + 12.0 + cw * 0.25, py + 42.0)),
            vec![HudAction::FormationTogether(true)]
        );
        assert_eq!(
            rig.click(Vec2::new(px + 12.0 + cw * 0.85, py + 74.0)),
            vec![HudAction::FormationSpacing(2)]
        );
        let up = Vec2::new(px + w * 0.5, py + 107.0);
        assert_eq!(rig.click(up), vec![HudAction::FormUp]);
        assert!(
            rig.hud.covers(up),
            "formation control click leaked onto the battlefield"
        );
    }

    #[test]
    fn a_construction_tile_builds_and_the_hud_keeps_the_click() {
        let mut rig = Rig::new("aster_t1_land_factory");
        let first = first_on_shelves(&rig, "aster_t1_land_factory", 1);
        let tile = first_tile(FACTORY_FAMILIES);
        assert_eq!(rig.click(tile), vec![HudAction::Build(first)]);
        assert!(
            rig.hud.covers(tile),
            "a click on a tile must not reach the battlefield"
        );
        assert!(
            !rig.hud.covers(Vec2::new(960.0, 400.0)),
            "the middle of the screen is the battlefield's"
        );
        assert_eq!(rig.right_click(tile), vec![HudAction::Cancel(first)]);
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
        let _ = builds;
        let tier = |rig: &Rig, t: u8| first_on_shelves(rig, "aster_t2_engineer", t);
        // Movement, and work (assist, reclaim, stop).
        let families = 2;
        let tile = first_tile(families);
        // A tech 2 engineer opens on its own tier; the T1 tab brings the basics back.
        assert_eq!(rig.click(tile), vec![HudAction::Build(tier(&rig, 2))]);
        assert_eq!(
            rig.click(Vec2::new(build_x(families) + 14.0 + 29.0, DECK_Y + 23.0)),
            vec![]
        );
        assert_eq!(rig.click(tile), vec![HudAction::Build(tier(&rig, 1))]);
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
            formation: 0,
            offset: [0.0; 2],
            moving_slot: None,
            formation_phase: 0,
            kind: OrderKind::Produce,
            pos: [0.0, 0.0],
            at: mc_core::FxVec2::ZERO,
            blueprint: b,
            radius: 0.0,
        };
        rig.view.status.queues = vec![UnitOrders {
            unit_id: 7,
            orders: vec![order(builds[0]), order(builds[0]), order(builds[1])],
            progress: 0.4,
            ..Default::default()
        }];
        rig.frame(&Input::default());
        let strip = Vec2::new(build_x(FACTORY_FAMILIES) + 200.0, DECK_Y - GAP - 30.0);
        assert!(
            rig.hud.covers(strip),
            "the queue strip appears above the construction panel"
        );
        // Two stacks: 2 of the first product, then 1 of the second. The second stack starts one tile along.
        let second = Vec2::new(build_x(FACTORY_FAMILIES) + 14.0, DECK_Y - GAP - 31.0);
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
        // Arrows step; the middle opens every speed, and a pick closes it.
        assert_eq!(rig.click(speed_faster()), vec![HudAction::SetSpeed(200)]);
        assert_eq!(rig.click(speed_slower()), vec![HudAction::SetSpeed(50)]);
        assert_eq!(rig.click(speed_centre()), vec![]);
        assert!(rig.hud.speed_open);
        assert_eq!(rig.click(speed_pick(7)), vec![HudAction::SetSpeed(1200)]);
        assert!(!rig.hud.speed_open);
        rig.click(speed_centre());
        assert_eq!(rig.click(speed_pick(3)), vec![], "the speed in force is no change");
        rig.click(speed_centre());
        rig.click(Vec2::new(900.0, 500.0));
        assert!(!rig.hud.speed_open, "a click elsewhere closes the list");
        assert_eq!(rig.click(pause_button()), vec![HudAction::Pause]);
        assert_eq!(
            rig.click(pause_button() + Vec2::new(74.0, 0.0)),
            vec![HudAction::Menu]
        );
        // Holding the button on the chart looks there; the right button orders there.
        let chart = chart();
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
        // The map folds away, and comes back from its tab.
        let fold = Vec2::new(1920.0 - EDGE - 16.0, chart.y - MINIMAP * 0.5 - 11.0 + 12.0);
        rig.click(fold);
        assert!(rig.hud.minimap_hidden);
        rig.settle();
        assert!(!rig.hud.covers(chart), "a folded map leaves the battlefield clear");
        rig.click(Vec2::new(1920.0 - EDGE - 48.0, fold.y));
        assert!(!rig.hud.minimap_hidden);
        // A network match owns no clock: the speed and pause controls are dead.
        rig.view.status.owns_clock = false;
        assert_eq!(rig.click(pause_button()), vec![]);
        assert_eq!(rig.click(speed_faster()), vec![]);
    }

    #[test]
    fn the_order_card_offers_what_the_selection_can_do_by_family() {
        let mut rig = Rig::new("aster_t1_tank");
        assert_eq!(rig.click(order_slot(0, 0)), vec![HudAction::Target(Targeting::Move)]);
        assert_eq!(rig.click(order_slot(0, 1)), vec![HudAction::Target(Targeting::Patrol)]);
        assert_eq!(rig.click(order_slot(1, 0)), vec![HudAction::Target(Targeting::Attack)]);
        assert_eq!(
            rig.click(order_slot(1, 1)),
            vec![HudAction::Target(Targeting::AttackMove)]
        );
        assert_eq!(
            rig.click(order_slot(1, 2)),
            vec![HudAction::Target(Targeting::AttackGround)]
        );
        assert_eq!(rig.click(order_slot(1, 3)), vec![HudAction::Target(Targeting::Bombard)]);
        assert_eq!(
            rig.click(order_slot(2, 0)),
            vec![HudAction::FireState(mc_sim::FireState::FireAtWill)]
        );
        assert_eq!(
            rig.click(order_slot(2, 1)),
            vec![HudAction::FireState(mc_sim::FireState::HoldPosition)]
        );
        assert_eq!(
            rig.click(order_slot(2, 2)),
            vec![HudAction::FireState(mc_sim::FireState::HoldFire)]
        );
        assert_eq!(rig.click(order_slot(3, 0)), vec![HudAction::Stop]);
        // One tank has no formation, and a tank cannot reclaim: nothing more on the card.
        assert_eq!(rig.click(order_slot(0, 2)), vec![]);
        assert_eq!(rig.click(order_slot(4, 0)), vec![]);
    }

    #[test]
    fn a_factory_card_orders_its_units_and_its_queue_holds_repeat() {
        let mut rig = Rig::new("aster_t1_land_factory");
        // What it makes takes these: moves, patrols, attacks, an engineer's assist.
        assert_eq!(rig.click(order_slot(0, 0)), vec![HudAction::Target(Targeting::Move)]);
        assert_eq!(rig.click(order_slot(0, 1)), vec![HudAction::Target(Targeting::Patrol)]);
        assert_eq!(rig.click(order_slot(3, 0)), vec![HudAction::Target(Targeting::Assist)]);
        assert_eq!(rig.click(order_slot(3, 1)), vec![HudAction::PauseWork(true)]);
        assert_eq!(rig.click(order_slot(3, 2)), vec![HudAction::Stop]);
        let repeat = Vec2::new(1920.0 - EDGE - 12.0 - 48.0, DECK_Y - GAP - 31.0);
        assert_eq!(rig.click(repeat), vec![HudAction::Repeat(true)]);
        // Pause sits beside it on the strip.
        let pause = Vec2::new(repeat.x - 48.0 - 10.0 - 48.0, repeat.y);
        assert_eq!(rig.click(pause), vec![HudAction::PauseWork(true)]);
    }

    #[test]
    fn paused_work_offers_resume_on_the_card_and_the_strip() {
        let mut rig = Rig::new("aster_t1_land_factory");
        rig.view.frame.units[0]._pad3[0] |= mc_sim::mirror::UNIT_PAUSED;
        assert_eq!(rig.click(order_slot(3, 1)), vec![HudAction::PauseWork(false)]);
        let repeat = Vec2::new(1920.0 - EDGE - 12.0 - 48.0, DECK_Y - GAP - 31.0);
        let resume = Vec2::new(repeat.x - 48.0 - 10.0 - 48.0, repeat.y);
        assert_eq!(rig.click(resume), vec![HudAction::PauseWork(false)]);
        // A tank has no work to pause: its card has no such order.
        let mut rig = Rig::new("aster_t1_tank");
        rig.view.frame.units[0]._pad3[0] |= mc_sim::mirror::UNIT_PAUSED;
        assert_eq!(rig.click(order_slot(3, 0)), vec![HudAction::Stop]);
    }


    #[test]
    fn an_engineers_queue_takes_a_right_click() {
        let mut rig = Rig::new("aster_t1_engineer");
        let build = rig.blueprints.id_of("aster_t1_power").unwrap_or_else(|| {
            rig.blueprints
                .unit(rig.blueprints.id_of("aster_t1_engineer").unwrap())
                .builder
                .as_ref()
                .unwrap()
                .builds[0]
        });
        let at = mc_core::FxVec2::new(mc_core::Fx::from_int(100), mc_core::Fx::from_int(200));
        rig.view.status.queues = vec![UnitOrders {
            unit_id: 7,
            orders: vec![QueuedOrder {
                formation: 0,
                offset: [0.0; 2],
                moving_slot: None,
                formation_phase: 0,
                kind: OrderKind::Build,
                pos: [100.0, 200.0],
                at,
                blueprint: build,
                radius: 0.0,
            }],
            progress: 0.0,
            ..Default::default()
        }];
        let families = 2;
        rig.frame(&Input::default());
        let hits: Vec<HudAction> = (0..40)
            .flat_map(|i| {
                rig.right_click(Vec2::new(
                    build_x(families) + 150.0 + i as f32 * 6.0,
                    DECK_Y - GAP - 31.0,
                ))
            })
            .collect();
        assert!(
            hits.contains(&HudAction::CancelOrder { kind: OrderKind::Build, pos: at }),
            "{hits:?}"
        );
    }

    #[test]
    fn the_commander_card_is_always_there_and_selects_it() {
        let mut rig = Rig::new("aster_commander");
        rig.view.selection.clear();
        let card = Vec2::new(EDGE + COMMANDER_W * 0.5, EDGE + 68.0 + GAP + COMMANDER_H * 0.5);
        assert_eq!(
            rig.click(card),
            vec![HudAction::Select { units: vec![7], focus: true }]
        );
    }

    #[test]
    fn a_late_match_mine_survey_leaves_the_panels_room() {
        // Zoomed out over dozens of mines with one selected: the survey drew
        // first and filled the overlay, and every panel after it vanished.
        let mut rig = Rig::new("aster_core_mine");
        let size = Vec2::from(rig.map.info().size_metres().to_f32());
        let template = rig.view.frame.units[0];
        for k in 0..48u32 {
            let at = size * Vec2::new(0.1 + 0.8 * (k % 8) as f32 / 7.0, 0.1 + 0.8 * (k / 8) as f32 / 5.0);
            let pos = [at.x, at.y, 0.0];
            let id = 100 + k;
            rig.view.index_of.insert(id, rig.view.frame.units.len());
            rig.view.frame.units.push(UnitInstance { pos, prev_pos: pos, unit_id: id, ..template });
        }
        rig.settle();
        assert!(rig.overlay.vertices.len() < mc_render::overlay::MAX_OVERLAY_VERTICES);
        let commander = rig.blueprints.id_of("aster_commander").expect("commander");
        let pos = [4100.0, 4000.0, 0.0];
        rig.view.index_of.insert(8, rig.view.frame.units.len());
        rig.view.frame.units.push(UnitInstance {
            pos,
            prev_pos: pos,
            unit_id: 8,
            blueprint: commander.0 as u32,
            ..template
        });
        let card = Vec2::new(EDGE + COMMANDER_W * 0.5, EDGE + 68.0 + GAP + COMMANDER_H * 0.5);
        assert_eq!(
            rig.click(card),
            vec![HudAction::Select { units: vec![8], focus: true }]
        );
    }

    #[test]
    fn the_idle_engineer_chip_steps_through_them_one_at_a_time() {
        let mut rig = Rig::new("aster_t1_engineer");
        rig.view.selection.clear();
        rig.view.frame.units[0].owner_flags |= STATE_IDLE;
        let mut second = rig.view.frame.units[0];
        second.unit_id = 9;
        rig.view.frame.units.push(second);
        rig.view.index_of.insert(9, 1);
        let chip = Vec2::new(EDGE + 30.0, DECK_Y - 24.0 - 8.0 + 12.0);
        let first = rig.click(chip);
        let next = rig.click(chip);
        let again = rig.click(chip);
        assert_eq!(first, vec![HudAction::Select { units: vec![7], focus: true }]);
        assert_eq!(next, vec![HudAction::Select { units: vec![9], focus: true }]);
        assert_eq!(again, first, "and round again");
    }

    #[test]
    fn a_structure_that_upgrades_offers_it_on_its_next_tier() {
        let mut rig = Rig::new("aster_t1_radar");
        rig.frame(&Input::default());
        let radar = rig.blueprints.unit(rig.blueprints.id_of("aster_t1_radar").unwrap());
        assert!(radar.upgrades_to.is_some(), "the test needs an upgradable structure");
        // No builds, so the panel opens on T2, whose first tile is the upgrade.
        // Its order card is Stop alone: one family.
        assert_eq!(rig.click(first_tile(1)), vec![HudAction::Upgrade]);
    }

    #[test]
    fn a_factory_queues_its_upgrade_and_the_units_it_opens_on_the_tier_tab() {
        let mut rig = Rig::new("aster_t1_land_factory");
        rig.frame(&Input::default());
        let (t2, tank) = (
            rig.blueprints.id_of("aster_t2_land_factory").unwrap(),
            rig.blueprints.id_of("aster_t2_tank").unwrap(),
        );
        // The factory's card has four order families; its T2 tab is the second.
        let families = 4;
        let t2_tab = Vec2::new(build_x(families) + 14.0 + 62.0 + 29.0, DECK_Y + 23.0);
        let hits = rig.click(t2_tab);
        assert!(hits.is_empty(), "{hits:?}");
        let upgrade = first_tile(families);
        // The upgrade tile, then the gap before the first shelf.
        let unit = upgrade + Vec2::new(96.0 + 22.0, 0.0);
        assert_eq!(rig.click(upgrade), vec![HudAction::Upgrade]);
        // A T2 unit is locked until the upgrade is queued...
        assert_eq!(rig.click(unit), vec![]);
        let queued = |kind, blueprint| QueuedOrder {
            formation: 0,
            offset: [0.0; 2],
            moving_slot: None,
            formation_phase: 0,
            kind,
            pos: [0.0, 0.0],
            at: mc_core::FxVec2::ZERO,
            blueprint,
            radius: 0.0,
        };
        rig.view.status.queues = vec![UnitOrders {
            unit_id: 7,
            orders: vec![queued(OrderKind::Upgrade, t2)],
            progress: 0.0,
            ..Default::default()
        }];
        // ...then takes orders, and the upgrade's right-click takes it back out.
        let first_t2 = rig.click(unit);
        assert!(
            matches!(first_t2.as_slice(), [HudAction::Build(b)] if rig.blueprints.unit(*b).tech == 2),
            "{first_t2:?} (a T2 tank is {tank:?})"
        );
        assert_eq!(rig.click(upgrade), vec![], "already queued");
        assert_eq!(rig.right_click(upgrade), vec![HudAction::CancelRefit(t2)]);
    }

    /// A commander's refit tab: row `row` (slot), tile `n` along it counting `or`
    /// gaps after `ors` alternatives and `arrows` tier arrows.
    fn refit_tile(row: usize, n: usize, ors: usize, arrows: usize) -> Vec2 {
        let row_h = ((DECK_H - 52.0 - 12.0) / 4.0).clamp(26.0, 44.0);
        Vec2::new(
            build_x(4) + 14.0 + 104.0 + n as f32 * 172.0 + ors as f32 * 34.0 + arrows as f32 * 26.0 + 86.0,
            DECK_Y + 44.0 + row as f32 * (row_h + 4.0) + row_h * 0.5,
        )
    }

    #[test]
    fn refits_queue_their_earlier_tiers_and_ask_before_replacing() {
        let mut rig = Rig::new("aster_commander+mfe");
        let bps = rig.blueprints.clone();
        let set = bps.refit_set(bps.id_of("aster_commander").unwrap()).unwrap();
        let kit = |slot: &str, key: &str| {
            set.slots
                .iter()
                .find(|s| s.key == slot)
                .unwrap()
                .modules
                .iter()
                .find(|m| m.key == key)
                .unwrap()
                .kit
        };
        rig.hud.open_refit_tab();
        // The rail cannon goes over the cannon: one click queues both, in order.
        assert_eq!(
            rig.click(refit_tile(1, 1, 0, 1)),
            vec![HudAction::Refit(vec![kit("gun", "cannon"), kit("gun", "railgun")])]
        );
        // The shield would take the formation engine off: nothing is sent until the player says so.
        let shield = refit_tile(2, 1, 1, 0);
        assert_eq!(rig.click(shield), vec![]);
        assert!(rig.hud.refit_prompt.is_some(), "a replacement asks first");
        // The card stands over the panel: its buttons along its foot.
        let yes = Vec2::new(
            (shield.x - 210.0).clamp(14.0, 1920.0 - 420.0 - 14.0) + 93.0,
            DECK_Y - GAP - 27.0,
        );
        assert_eq!(rig.click(yes), vec![HudAction::Refit(vec![kit("back", "shield")])]);
        assert!(rig.hud.refit_prompt.is_none());
        // Asked again and kept: nothing happens.
        rig.click(shield);
        let keep = Vec2::new(yes.x + 160.0, yes.y);
        assert_eq!(rig.click(keep), vec![]);
        assert!(rig.hud.refit_prompt.is_none());
    }

    fn observer_card_y(i: usize) -> f32 {
        EDGE + observer::HEADER_H + GAP + i as f32 * (observer::CARD_FULL + 6.0)
    }

    #[test]
    fn an_observer_cannot_order_the_selection() {
        let mut rig = Rig::new("aster_t1_tank");
        rig.view.observing = true;
        assert_eq!(rig.click(order_slot(0, 0)), vec![], "watching has no order card");
        assert_eq!(
            rig.click(first_tile(TANK_FAMILIES)),
            vec![],
            "watching has no construction panel"
        );
        // A commander's card looks through their eyes; its Find button goes to them.
        let first_card = observer_card_y(0);
        assert_eq!(
            rig.click(Vec2::new(EDGE + 120.0, first_card + 60.0)),
            vec![HudAction::Vision(Some(1 - 1))]
        );
        rig.view.perspective = Some(0);
        assert_eq!(
            rig.click(Vec2::new(EDGE + 120.0, first_card + 60.0)),
            vec![HudAction::Vision(None)],
            "the card being looked through goes back to everyone's eyes"
        );
        assert_eq!(
            rig.click(Vec2::new(EDGE + observer::WIDTH - 37.0, first_card + 16.0)),
            vec![HudAction::FocusPlayer(0)]
        );
        assert_eq!(
            rig.click(Vec2::new(EDGE + 18.0 + 52.0 + 6.0 + 34.0 + 4.0 + 17.0, EDGE + 47.0)),
            vec![HudAction::Vision(Some(1))],
            "the vision chips pick a side"
        );
    }

    #[test]
    fn hovering_a_unit_fills_the_info_panel_when_nothing_is_selected() {
        let mut rig = Rig::new("aster_t1_tank");
        rig.view.selection.clear();
        let info = Vec2::new(INFO_X + 24.0, DECK_Y + 40.0);
        rig.settle();
        assert!(
            !rig.hud.covers(info),
            "an empty selection leaves the info panel off"
        );
        rig.hover = Some(7);
        rig.settle();
        assert!(
            rig.hud.covers(info),
            "hovering a unit should open its dossier"
        );
        assert_eq!(
            rig.click(order_slot(0, 0)),
            vec![],
            "a hover inspect has no order card"
        );
    }

    #[test]
    fn an_enemy_selection_shows_details_without_orders() {
        let mut rig = Rig::new("aster_t1_tank");
        rig.view.frame.units[0].owner_flags = 1;
        let info = Vec2::new(INFO_X + 24.0, DECK_Y + 40.0);
        rig.frame(&Input::default());
        assert!(
            rig.hud.covers(info),
            "inspecting an enemy still has a dossier"
        );
        assert_eq!(
            rig.click(order_slot(0, 0)),
            vec![],
            "an enemy cannot be ordered"
        );
    }

    #[test]
    fn hovering_an_enemy_while_selected_opens_a_hover_card() {
        let mut rig = Rig::new("aster_t1_tank");
        let mut enemy = rig.view.frame.units[0];
        enemy.unit_id = 8;
        enemy.owner_flags = 1;
        enemy.blueprint = rig.blueprints.id_of("aster_t1_scout").unwrap().0 as u32;
        rig.view.frame.units.push(enemy);
        rig.view.index_of.insert(8, 1);
        rig.hover = Some(8);
        rig.frame(&Input::default());
        assert!(
            rig.hud.covers(Vec2::new(INFO_X + 24.0, DECK_Y - 80.0)),
            "the hover card sits above the deck"
        );
        assert_eq!(
            rig.click(order_slot(0, 0)),
            vec![HudAction::Target(crate::game::Targeting::Move)],
            "the army's order card stays"
        );
    }

    #[test]
    fn construction_keys_pick_a_tier_a_shelf_and_an_item() {
        let mut rig = Rig::new("aster_t1_engineer");
        rig.hud.build_keys = true;
        rig.frame(&Input::default());
        let blueprints = rig.blueprints.clone();
        let items: Vec<&UnitBlueprint> = blueprints
            .unit(blueprints.id_of("aster_t1_engineer").unwrap())
            .builder
            .as_ref()
            .unwrap()
            .builds
            .iter()
            .map(|b| blueprints.unit(*b))
            .filter(|b| b.tech == 1 && style::Purpose::of(b) == style::Purpose::Economy)
            .collect();
        rig.hud.build_key = Some('1');
        rig.frame(&Input::default());
        rig.hud.build_key = Some('Q');
        rig.frame(&Input::default());
        rig.hud.build_key = Some('S');
        assert_eq!(rig.frame(&Input::default()), vec![HudAction::Build(items[1].id)]);
    }

    #[test]
    fn details_opens_a_card_of_lore_and_weapons_over_the_panel() {
        let mut rig = Rig::new("aster_t1_tank");
        let details = Vec2::new(INFO_X + 336.0 - 16.0 - 37.0, DECK_Y + 26.0);
        rig.click(details);
        assert!(rig.hud.details_open);
        rig.frame(&Input::default());
        assert!(rig.hud.covers(Vec2::new(INFO_X + 200.0, DECK_Y - 40.0)), "the card sits over the deck");
        rig.click(details);
        assert!(!rig.hud.details_open);
    }

    #[test]
    fn details_close_when_the_selection_changes_or_comes_back() {
        let mut rig = Rig::new("aster_t1_tank");
        let details = Vec2::new(INFO_X + 336.0 - 16.0 - 37.0, DECK_Y + 26.0);
        rig.click(details);
        assert!(rig.hud.details_open);
        rig.view.selection.clear();
        rig.frame(&Input::default());
        assert!(!rig.hud.details_open, "deselecting closes the card");

        rig.view.selection = vec![7];
        rig.settle();
        rig.click(details);
        assert!(rig.hud.details_open);
        rig.view.selection.clear();
        rig.frame(&Input::default());
        rig.view.selection = vec![7];
        rig.frame(&Input::default());
        assert!(!rig.hud.details_open, "coming back to the unit finds the card closed");
    }

    #[test]
    fn the_construction_strip_scrolls_sideways() {
        // A commander's order card leaves its strip too short for all of tier 1.
        let mut rig = Rig::new("aster_commander");
        let tile = first_tile(4);
        let first = rig.click(tile);
        assert_eq!(rig.hud.build_scroll, 0.0);
        rig.frame(&Input {
            cursor: tile,
            scroll: -1.0,
            ..Default::default()
        });
        assert!(rig.hud.build_scroll > 0.0, "the wheel runs the strip along");
        let later = rig.click(tile);
        assert!(!later.is_empty() && later != first, "{first:?} then {later:?}");
        // A shelf's key runs it to that shelf; the left arrow pages back to the start.
        rig.hud.build_keys = true;
        rig.hud.build_key = Some('Q');
        rig.frame(&Input::default());
        assert_eq!(rig.hud.build_scroll, 0.0, "economy is the first shelf");
        rig.hud.build_scroll = 400.0;
        rig.click(Vec2::new(build_x(4) + 14.0 + 12.0, tile.y));
        assert_eq!(rig.hud.build_scroll, 0.0);
    }

    #[test]
    fn a_lift_ship_hold_lets_out_what_is_clicked_and_its_card_lands_and_takes_off() {
        use mc_sim::mirror::{CargoUnit, CargoView, LiftPhase};
        let mut rig = Rig::new("aster_t2_lift_ship");
        let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
        let bot = rig.blueprints.id_of("aster_t1_engineer").unwrap();
        let rider = |unit_id, blueprint| CargoUnit { unit_id, blueprint, health: 1.0, room: 2 };
        let cargo = |phase| CargoView {
            capacity: 96,
            used: 6,
            stored: vec![rider(21, tank), rider(22, bot), rider(23, tank)],
            boarding: 0,
            ramp_down: phase == LiftPhase::Ready,
            unloading: false,
            phase,
            to_unload: 0,
        };
        rig.view.status.queues = vec![UnitOrders {
            unit_id: 7,
            cargo: Some(cargo(LiftPhase::Ready)),
            ..Default::default()
        }];
        rig.settle();
        // Find the first tile of the hold in the unit panel: a click there lets that tank out.
        let mut first = None;
        'scan: for y in (0..50).map(|i| DECK_Y + 60.0 + i as f32 * 4.0) {
            for x in (0..6).map(|i| INFO_X + 10.0 + i as f32 * 4.0) {
                let got = rig.click(Vec2::new(x, y));
                if got.iter().any(|a| matches!(a, HudAction::UnloadUnits(_))) {
                    assert_eq!(got, vec![HudAction::UnloadUnits(vec![21])], "one click lets one unit out");
                    first = Some(Vec2::new(x, y));
                    break 'scan;
                }
            }
        }
        let first = first.expect("no hold tile to click in the unit panel");
        // Shift-click: every unit of that kind.
        rig.view.shift = true;
        assert_eq!(rig.click(first), vec![HudAction::UnloadUnits(vec![21, 23])]);
        rig.view.shift = false;
        // Ctrl-click: pick it alongside the ship instead.
        rig.view.ctrl = true;
        assert_eq!(rig.click(first), vec![HudAction::Select { units: vec![7, 21], focus: false }]);
        rig.view.ctrl = false;
        // The order card has a Transport column: down, it offers Take Off; aloft, Land Here.
        let card = |rig: &mut Rig, row: usize| -> Vec<HudAction> {
            (0..6)
                .flat_map(|col| {
                    let x = ORDERS_X + 14.0 + col as f32 * (selection::ORDER_W + selection::ORDER_GAP) + 50.0;
                    let y = DECK_Y + 36.0 + row as f32 * (selection::ORDER_H + selection::ORDER_GAP) + 20.0;
                    rig.click(Vec2::new(x, y))
                })
                .collect()
        };
        assert!(card(&mut rig, 3).contains(&HudAction::TakeOff), "no Take Off on the card");
        assert!(card(&mut rig, 2).contains(&HudAction::UnloadHere), "no Unload Here on the card");
        assert!(card(&mut rig, 0).contains(&HudAction::Target(crate::game::Targeting::Land)));
        assert!(card(&mut rig, 1).contains(&HudAction::Target(crate::game::Targeting::Unload)));
        rig.view.status.queues[0].cargo = Some(cargo(LiftPhase::InFlight));
        assert!(card(&mut rig, 3).contains(&HudAction::LandHere), "no Land Here while aloft");
        assert_eq!(super::cargo::status(&cargo(LiftPhase::RampOpening)).0, "Ramp opening");
        let mut out = cargo(LiftPhase::Unloading);
        out.to_unload = 2;
        assert_eq!(super::cargo::status(&out).0, "Unloading \u{b7} 2 left");
    }
}
