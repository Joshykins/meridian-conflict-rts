//! The match HUD, in the same glass-and-cyan language as the front end and
//! built from the same toolkit: economy, clock and game speed along the top;
//! minimap, selection, order card, construction and queue along the bottom.
//!
//! Immediate mode. `draw` lays everything out in points, remembers which parts
//! of the screen it covered (so the match does not take those clicks for the
//! battlefield) and returns what the player asked for as `HudAction`s. The HUD
//! never sends commands itself.

mod armament;
mod build;
mod builders;
pub mod cargo;
mod economy;
mod economy_panel;
mod focus;
pub mod free_camera;
pub mod icons;
mod issue_mark;
pub use issue_mark::IssueMark;
mod mine;
mod mine_marks;
mod minimap;
mod net_cards;
mod netplay;
pub mod notices;
mod observer;
mod pause;
mod profiler;
mod range;
mod reclaim;
mod replay_bar;
pub use replay_bar::{clock as replay_clock, ReplayBar};
mod refit;
mod selection;
pub mod silo;
pub mod survival;
pub use selection::ordered_as;
mod match_state;
pub mod style;
pub mod thumbs;
mod titan;
mod top_bar;
mod unit_picker;
mod volatile;

use crate::audio::Sfx;
use crate::game::{Mode, Targeting, View};
use crate::ui::{id, ink, palette, rgb, type_scale, Color, Id, Rect, Response, Ui};
use glam::Vec2;
use mc_data::{BlueprintId, Blueprints, UnitBlueprint};
use mc_map::MapFile;
use mc_render::{Camera, FrameStats};
use mc_sim::mirror::{
    UnitInstance, UnitOrders, KIND_GHOST, KIND_PROP, KIND_WRECK, STATE_UNIDENTIFIED,
};
use mc_sim::tables::flag;

pub use mine_marks::ore_tapped;
use mine_marks::{mine_marks, mines_in_sight, territory, TERRITORY_SEGMENTS};
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
    if pct.is_multiple_of(100) {
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
/// The economy panel's width, top left.
const ECONOMY_W: f32 = 292.0 * 2.0 + 46.0;
/// The economy panel's height: its figures, and the focus switches under them.
const ECONOMY_H: f32 = 68.0 + focus::FOCUS_H;
/// The narrowest the stall chip right of the economy gets; the pause strip keeps clear of it.
const STALL_CHIP_W: f32 = 236.0;
/// The top bar's width: clock, speed, pause, menu.
const TOP_BAR_W: f32 = 146.0 + 50.0 + SPEED_W + 10.0 + 50.0 + 98.0;

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
    /// What the side's economy builds first when it stalls.
    Focus(mc_sim::focus::Focus),
    /// Selected lift ships set down where they stand and let their holds out.
    UnloadHere,
    /// Selected lift ships set down where they stand.
    LandHere,
    /// Selected lift ships that are down raise the ramp and climb back to the clouds.
    TakeOff,
    /// These units, riding in a lift ship's hold, walk out of it (`Command::Unload`).
    UnloadUnits(Vec<u32>),
    /// Test range: clear it and line up one of each of these units.
    LineUp(Vec<BlueprintId>),

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
    /// A command as it stands (a panel that builds its own).
    Send(mc_sim::Command),
    /// Watching a replay: jump to this tick.
    Seek(u32),
    /// Say `text` in a network match, to the slots in `to` (0: everyone).
    Chat {
        text: String,
        to: u8,
    },
    /// Leave the match for the front end, now (a network match that cannot go on).
    Leave,
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
    /// A network match's link as it stands; `None` on one machine.
    pub net: Option<&'a crate::netplay::NetLink>,
    /// What the session had to say since the last frame.
    pub net_notices: &'a [crate::netplay::NetNotice],
}

impl Scene<'_> {
    fn bp(&self, u: &UnitInstance) -> &UnitBlueprint {
        self.blueprints.unit(BlueprintId(u.blueprint as u16))
    }

    /// A unit's orders and reports. Wrecks, ghosts and props have none: their
    /// ids come from tables of their own and can match a live unit's.
    fn queue_of(&self, u: &UnitInstance) -> Option<&UnitOrders> {
        if u.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP) != 0 {
            return None;
        }
        self.view
            .status
            .queues
            .iter()
            .find(|q| q.unit_id == u.unit_id)
    }

    fn team_color(&self, owner: u8) -> Color {
        let c = self.view.colors[owner as usize % 8];
        [c[0], c[1], c[2], 1.0]
    }
}

pub fn has_flag(u: &UnitInstance, f: u16) -> bool {
    u.owner_flags & (f as u32) << 8 != 0
}

#[derive(Default)]
pub struct Hud {
    /// Survival: names of the map's node sites, by site index.
    pub survival_sites: Vec<String>,
    /// Survival: the map's fronts (domain, path), read once with the sites.
    survival_fronts: Vec<(mc_data::survival::Domain, Vec<Vec2>)>,
    survival_read: bool,
    /// Survival's card this frame, which the range key keeps left of.
    survival_card: Option<Rect>,
    /// Where toasts start: under survival's panel when there is one.
    toast_top: f32,
    /// Tech tab of the construction panel, and the builder blueprint it was chosen for.
    tab: u8,
    tab_for: Option<u32>,
    /// Screen areas the HUD covered last frame, in window pixels.
    covered: Vec<Rect>,
    /// Toasts, launch warnings and event notes, merged (`notices`).
    notices: notices::Notices,
    /// The economy's build-first switches and stall notes (`focus`).
    focus_ui: focus::FocusUi,
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
    /// The unit browser's filters, kept between openings.
    unit_picker_filters: unit_picker::Filters,
    /// Whether the reclaim survey was up last frame, so the cue plays on the edge.
    reclaim_seen: bool,
    reclaim_open: bool,
    /// 0..1 fade of the survey, so it eases in and out after the key.
    reclaim_vis: f32,
    /// Pictures of the units, for tiles.
    pub thumbs: thumbs::Thumbs,
    /// The profiler's report card: match id, note, Mark Issue.
    pub issues: issue_mark::IssueMark,
    /// Watching a replay: its timeline.
    pub replay_bar: replay_bar::ReplayBar,
    /// How far along the construction strip is scrolled, in points: where it is
    /// headed, and where it is on screen (easing after it).
    build_scroll: f32,
    build_shown: f32,
    /// The lore-and-weapons card over the unit panel is open.
    pub details_open: bool,
    /// The rings (bits by `rings::projections` index) whose weapon card or Reach row the
    /// pointer was on last frame: the details card lights them and dims the rest.
    pub details_focus: u64,
    /// The same rings for the ground, with their blueprint: taken each frame into
    /// `Rings::focus`, so they go out when the card is not drawn.
    pub reach_focus: Option<(u32, u64)>,
    pub minimap_hidden: bool,
    /// The commander's card and the idle engineer and factory cards under it.
    builders: builders::Builders,
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
    /// Launch warnings and what came of them (`silo::alerts`).
    nuke_alerts: silo::Alerts,
    /// Titan calls and strike cards (`titan::alerts`).
    titan: titan::Alerts,
    /// Ctrl+Alt: the panels folded away for the camera (`free_camera.rs`).
    pub free: free_camera::FreeCamera,
    /// Drawing a folded region: its panels do not keep the pointer from the battlefield.
    unclaimed: bool,
    /// Chat and the link's news in a network match (`netplay.rs`).
    pub net: netplay::NetHud,
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
pub(super) fn whole(v: f32) -> String {
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
        if let Some(t) = range::Tab::ALL
            .into_iter()
            .find(|t| t.label().eq_ignore_ascii_case(name))
        {
            self.range_tab = t;
            self.range_page = 1.0;
        }
    }

    pub fn browse_range_subject(&mut self) {
        self.unit_picker = Some(unit_picker::Picker::new());
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
        // The same complaint over and over is one complaint, with a count.
        let text = text.into();
        self.notices
            .note(text.clone(), text, color, notices::Glyph::Bar, None);
    }

    fn claim(&mut self, ui: &Ui, r: Rect) {
        if self.unclaimed {
            return;
        }
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
        ui.bevel(
            r,
            5.0,
            (0.45 + 0.4 * res.glow + 0.6 * lit_k).min(1.0) * live,
        );
        if lit_k > 0.01 {
            ui.outline_cut(
                r,
                5.0,
                rgb(0xFFFFFF, 0.55 * lit_k * live),
                rgb(0xFFFFFF, 0.55 * lit_k * live),
            );
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
        if self.free.on {
            self.speed_open = false;
            self.build_keys = false;
            ui.mem.popup = None;
        }
        self.net_news(ui, s);
        // The mine survey lies on the world, under every panel.
        mine_marks(ui, s, &mut self.ore);
        let fold = self.fold_begin(ui, free_camera::Part::Top);
        let speed_hits = self.speed_hits(ui);
        self.fold_end(ui, fold);
        let fold = self.fold_begin(ui, free_camera::Part::Left);
        let mut under_economy = self.economy(ui, s, dt);
        if !view.observing {
            let card = Rect::new(EDGE, under_economy + GAP, COMMANDER_W, COMMANDER_H);
            if builders::commander_card(self, ui, s, card, dt) {
                under_economy = card.bottom();
            }
        }
        if let Some(r) = &view.range {
            range::draw(self, ui, s, r, under_economy);
            under_economy += GAP + self.range_tall;
        }
        if !view.observing {
            // Down to the chip line over the deck.
            let bottom = h - EDGE - DECK_H - 24.0 - 8.0 - GAP;
            builders::idle_cards(self, ui, s, under_economy, bottom);
        }
        self.fold_end(ui, fold);
        if self.reclaim_seen && s.show_reclaim != self.reclaim_open {
            reclaim::cue(ui, s.show_reclaim);
        }
        self.reclaim_seen = true;
        self.reclaim_open = s.show_reclaim;
        let goal = if s.show_reclaim { 1.0 } else { 0.0 };
        self.reclaim_vis += (goal - self.reclaim_vis) * (1.0 - (-dt * 9.0).exp());
        reclaim::draw(ui, s, self.reclaim_vis);
        let fold = self.fold_begin(ui, free_camera::Part::Top);
        let under_top = self.top_bar(ui, s);
        self.fold_end(ui, fold);
        // With the panels away, notices rise to the top edge.
        self.toast_top = 96.0 - 68.0 * self.free.part(free_camera::Part::Top);
        let fold = self.fold_begin(ui, free_camera::Part::Right);
        // The right column under the minimap (or its tab): the report card and
        // the profiler, then survival's card.
        let mut right_top =
            under_top + 2.0 * GAP + if self.minimap_hidden { 26.0 } else { MINIMAP };
        // F1 does not fold with the column: an issue seen through the free camera
        // gets marked from it. The panel rises to the corner, under any cinema bars.
        self.fold_end(ui, fold);
        let goal = if self.free.on {
            EDGE + self.bars_goal(ui)
        } else {
            right_top
        };
        if !view.show_profiler {
            // Opened, it starts where it belongs; it glides only as the camera changes.
            ui.snap(id("report-top", 0), goal);
        } else {
            let top = ui.ease(id("report-top", 0), goal, 9.0);
            // The report card first: the profiler can run off the bottom.
            let r = self.issues.draw(ui, s, Vec2::new(w - EDGE, top));
            self.claim(ui, r);
            if let Some(mark) = self.issues.take_fresh() {
                self.replay_bar.marked(mark);
            }
            let r = profiler::draw(ui, s, Vec2::new(w - EDGE, r.bottom() + GAP));
            self.claim(ui, r);
            if !self.free.on {
                right_top = r.bottom() + GAP;
            }
        }
        let fold = self.fold_begin(ui, free_camera::Part::Right);

        // The minimap sits under the top bar on the right, and folds away.
        let map_rect = Rect::new(w - EDGE - MINIMAP, under_top + GAP, MINIMAP, MINIMAP);
        if self.minimap_hidden {
            let tab = Rect::new(w - EDGE - 96.0, under_top + GAP, 96.0, 26.0);
            let t = self.tile(ui, id("minimap-show", 0), tab, false, true);
            self.claim(ui, tab);
            ui.text_centred(
                tab.x + tab.w * 0.5,
                tab.mid_y(),
                type_scale::MICRO,
                rgb(palette::TEXT, 0.8 + 0.2 * t.glow),
                "Map  +",
            );
            if t.clicked {
                ui.audio.play(Sfx::Tick);
                self.minimap_hidden = false;
            }
        }
        // The map folds up into its tab and unfolds from it.
        let map_k = ui.ease(
            id("minimap-open", 0),
            if self.minimap_hidden { 0.0 } else { 1.0 },
            16.0,
        );
        if map_k > 0.01 {
            let (fade, shift, live) = (ui.fade, ui.shift, ui.interactive);
            ui.fade *= map_k;
            ui.shift.y -= (1.0 - map_k) * 40.0;
            ui.interactive &= !self.minimap_hidden;
            minimap::draw(self, ui, s, map_rect);
            (ui.fade, ui.shift, ui.interactive) = (fade, shift, live);
        }
        // Survival's rounds and Shapers, under the map.
        self.survival_card = survival::draw(
            self,
            ui,
            s,
            w - EDGE,
            right_top,
            MINIMAP,
            h - EDGE - DECK_H - GAP,
        );
        self.fold_end(ui, fold);
        let fold = self.fold_begin(ui, free_camera::Part::Deck);

        // The bottom deck: whatever the selection is.
        let deck_y = h - EDGE - DECK_H;
        // Chat rises from over the idle chips.
        self.net_chat(ui, s, deck_y - 24.0 - 8.0 - 12.0, dt);

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
            self.group_chips(ui, s, EDGE, deck_y - 24.0 - 8.0);
        }
        // Above where a queue strip would be, so the two never overlap.
        self.reach_key(ui, s, deck_y - 62.0 - GAP - 24.0 - 8.0);
        if !units.is_empty() {
            let info = Rect::new(x, deck_y, 336.0, DECK_H);
            let strip = !view.observing && !inspect_only;
            selection::info(self, ui, s, &units, info, strip);
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
                // A nuclear silo's or an interceptor array's rounds and launch (`silo.rs`).
                if let Some((launcher, l)) = silo::launcher_of(s, &units) {
                    let pw = silo::WIDTH.min(w - EDGE - x);
                    if pw > 200.0 {
                        silo::panel(self, ui, s, launcher, &l, Rect::new(x, deck_y, pw, DECK_H));
                        x += pw + GAP;
                    }
                }
                // A tier-5 titan's great bore and its strike (`titan.rs`).
                if let Some((titan, count)) = titan::titan_of(s, &units) {
                    let pw = titan::WIDTH.min(w - EDGE - x);
                    if pw > 220.0 {
                        titan::panel(self, ui, s, titan, count, Rect::new(x, deck_y, pw, DECK_H));
                        x += pw + GAP;
                    }
                }
                // A lift ship's hold, where a builder's construction panel goes.
                if let Some((ship, view)) = cargo::ship_of(s, &units) {
                    let hold_w = cargo::width(w - EDGE - x);
                    if hold_w > 0.0 {
                        cargo::panel(
                            self,
                            ui,
                            s,
                            ship.unit_id,
                            &view,
                            Rect::new(x, deck_y, hold_w, DECK_H),
                        );
                        x += hold_w + GAP;
                    }
                }
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
        // A replay's timeline, clear of the selection panel.
        let side = EDGE + 336.0 + GAP;
        if let Some(r) = self
            .replay_bar
            .draw(ui, s, side, w - side, h - EDGE, &mut self.actions)
        {
            self.claim(ui, r);
        }
        self.fold_end(ui, fold);

        silo::alerts(self, s);
        titan::alerts(self, ui, s, dt);
        notices::draw(self, ui, self.toast_top, dt);
        self.free_camera_guide(ui, dt);
        self.match_state(ui, s);
        ui.interactive = interactive;
        let fold = self.fold_begin(ui, free_camera::Part::Top);
        self.speed_list(ui, s, &speed_hits);
        self.fold_end(ui, fold);
        unit_picker::draw(self, ui, s);
        // An open dropdown's list goes over every panel, and while it is open
        // the whole screen is the HUD's: a click off the list only closes it.
        if ui.mem.popup.is_some() {
            self.claim(ui, Rect::new(0.0, 0.0, w, h));
        }
        ui.popups();
        std::mem::take(&mut self.actions)
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
        // Beside survival's card when it reaches down this far.
        if let Some(card) = self.survival_card.filter(|c| c.bottom() > y) {
            right = right.min(card.x - GAP);
        }
        for &(reach, rank, dead, far) in s.view.reaches.iter().rev() {
            let value = if dead > 0.0 {
                format!("{dead:.0}\u{2013}{far:.0} m")
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
                | Targeting::Strike
                | Targeting::Bombard => style::Family::Combat.tone(),
                Targeting::Move | Targeting::Patrol => style::Family::Movement.tone(),
                Targeting::Assist | Targeting::Reclaim => style::Family::Engineering.tone(),
                Targeting::Guard => style::Family::Stance.tone(),
                Targeting::Land | Targeting::Unload => style::Family::Transport.tone(),
                Targeting::Nuke => silo::WARHEAD,
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
        Mode::Target(Targeting::Guard) => {
            "Press on the spot to hold (or a friendly unit to go with), drag out the area to guard"
        }
        Mode::Target(Targeting::Nuke) => {
            "Click anywhere on the map or the minimap  \u{b7}  RMB cancels"
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Input, Memory};
    use mc_render::Overlay;
    use mc_sim::mirror::{QueuedOrder, UnitOrders, STATE_IDLE};
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
                packed: 0,
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
                status: [0; 3],
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
                net: None,
                net_notices: &[],
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
        Vec2::new(
            build_x(families) + 14.0 + 30.0 + 40.0,
            DECK_Y + 44.0 + 32.0 + 40.0,
        )
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
        1920.0 - EDGE - TOP_BAR_W
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
        Vec2::new(
            top_bar() + 196.0 + SPEED_W * 0.5,
            EDGE + 44.0 + 4.0 + 4.0 + i as f32 * 28.0 + 14.0,
        )
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
        assert!(rig
            .click(Vec2::new(253.0, 279.0 + focus::FOCUS_H))
            .is_empty());
        assert!(rig
            .click(Vec2::new(238.0, 320.0 + focus::FOCUS_H))
            .is_empty());
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
        assert_eq!(
            rig.hud.range_sky.unwrap().choice.time,
            Some(TimeOfDay::Dawn)
        );
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
        assert!(rig
            .click(Vec2::new(180.0, 152.0 + focus::FOCUS_H))
            .is_empty());
        assert!(rig.hud.unit_picker_open());
        assert!(rig.hud.covers(Vec2::new(1800.0, 700.0)));
        // A covered battlefield/reset click cannot produce a range action.
        assert!(rig
            .click(Vec2::new(184.0, 615.0 + focus::FOCUS_H))
            .is_empty());
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

        rig.click(Vec2::new(180.0, 152.0 + focus::FOCUS_H));
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
    fn range_browser_picks_t4_t5_and_space() {
        use crate::range::{Range, RangeAction};
        use crate::ui::Key;
        let mut rig = Rig::new("aster_t1_tank");
        let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
        rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
        for (filter, key) in [
            // The first tech 4 unit listed.
            (Vec2::new(758.0, 283.0), "aster_t4_artillery"),
            (Vec2::new(844.0, 283.0), "replication_engine"),
            (Vec2::new(1128.0, 242.0), "aster_t2_lift_ship"),
        ] {
            // The browser keeps its filters between openings; start each pick clean.
            rig.hud.unit_picker_filters = Default::default();
            rig.hud.unit_picker = Some(unit_picker::Picker::new());
            assert!(rig.click(filter).is_empty());
            let chosen = rig.blueprints.id_of(key).unwrap();
            let asked = rig.frame(&Input {
                keys: vec![Key::Enter],
                ..Default::default()
            });
            assert_eq!(
                asked,
                vec![HudAction::Range(RangeAction::PickSubject(chosen))],
                "{key}"
            );
            assert!(!rig.hud.unit_picker_open());
        }
        // Space and tech intersect; clearing an empty combination restores the catalog.
        rig.hud.unit_picker = Some(unit_picker::Picker::new());
        rig.click(Vec2::new(1128.0, 242.0));
        rig.click(Vec2::new(844.0, 283.0));
        assert!(rig
            .frame(&Input {
                keys: vec![Key::Enter],
                ..Default::default()
            })
            .is_empty());
        assert!(rig.hud.unit_picker_open());
        rig.click(Vec2::new(1450.0, 189.0));
        rig.frame(&Input {
            typed: "space".into(),
            ..Default::default()
        });
        assert_eq!(
            rig.frame(&Input {
                keys: vec![Key::Enter],
                ..Default::default()
            }),
            vec![HudAction::Range(RangeAction::PickSubject(
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
        rig.click(Vec2::new(180.0, 152.0 + focus::FOCUS_H));
        assert_eq!(
            rig.click(Vec2::new(450.0, 355.0)),
            vec![HudAction::Range(RangeAction::PickSubject(first))]
        );
        rig.click(Vec2::new(180.0, 152.0 + focus::FOCUS_H));
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
            !rig.hud.covers(Vec2::new(180.0, 400.0 + focus::FOCUS_H)),
            "a match that is not the range has no range panel"
        );
        let tank = rig.blueprints.id_of("aster_t1_tank").unwrap();
        rig.view.range = Some(Range::new(mc_core::FxVec2::from_ints(4000, 4000), tank));
        // Where things are at 1920x1080 (see `range::draw`), less the economy's focus strip
        // over it: the panel's top is at 94, its tab strip at 266..292, and the open tab's
        // page starts at 306.
        let range = |a| vec![HudAction::Range(a)];
        assert_eq!(
            rig.click(Vec2::new(170.0, 206.0 + focus::FOCUS_H)),
            range(RangeAction::Side(crate::range::Side::Blue)),
            "the duplicate's team"
        );
        assert_eq!(
            rig.click(Vec2::new(320.0, 116.0 + focus::FOCUS_H)),
            range(RangeAction::Control(RED)),
            "the side commanded sits by the title"
        );
        assert_eq!(
            rig.click(Vec2::new(333.0, 152.0 + focus::FOCUS_H)),
            range(RangeAction::Subject(1))
        );
        assert_eq!(
            rig.click(Vec2::new(270.0, 238.0 + focus::FOCUS_H)),
            range(RangeAction::ArmSpawn)
        );

        // The Unit tab is open first.
        assert_eq!(
            rig.click(Vec2::new(308.0, 336.0 + focus::FOCUS_H)),
            range(RangeAction::Damage(1000)),
            "Kill"
        );
        assert_eq!(
            rig.click(Vec2::new(295.0, 368.0 + focus::FOCUS_H)),
            range(RangeAction::Flag(flag::INVULNERABLE, true))
        );
        // Holding the build track half way along asks for a half-built unit.
        let half = Vec2::new(80.0 + 216.0 * 0.5, 404.0 + focus::FOCUS_H);
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
        assert_eq!(
            rig.click(Vec2::new(184.0, 449.0 + focus::FOCUS_H)),
            range(RangeAction::Reset)
        );
        assert!(
            rig.hud.covers(Vec2::new(180.0, 400.0 + focus::FOCUS_H)),
            "a click on the panel must not reach the battlefield"
        );

        // With nothing selected it acts on every unit of the subject's type; with none of those, on nothing.
        rig.view.selection.clear();
        assert_eq!(
            rig.click(Vec2::new(308.0, 336.0 + focus::FOCUS_H)),
            range(RangeAction::Damage(1000))
        );
        let units = std::mem::take(&mut rig.view.frame.units);
        let index = std::mem::take(&mut rig.view.index_of);
        assert_eq!(rig.click(Vec2::new(308.0, 336.0 + focus::FOCUS_H)), vec![]);
        (rig.view.frame.units, rig.view.index_of) = (units, index);

        // Stage: scenarios around the subject, then what it does itself.
        assert!(
            rig.click(Vec2::new(127.0, 279.0 + focus::FOCUS_H))
                .is_empty(),
            "a tab is not an order"
        );
        assert_eq!(
            rig.click(Vec2::new(230.0, 322.0 + focus::FOCUS_H)),
            range(RangeAction::Scenario(Scenario::Targets))
        );
        assert_eq!(
            rig.click(Vec2::new(230.0, 358.0 + focus::FOCUS_H)),
            range(RangeAction::Scenario(Scenario::March)),
            "the second row: what the subject does itself"
        );
        // The panel is shorter on this tab, and Reset came up with it.
        assert_eq!(
            rig.click(Vec2::new(184.0, 405.0 + focus::FOCUS_H)),
            range(RangeAction::Reset)
        );

        // Range: the camera presets.
        assert!(rig
            .click(Vec2::new(316.0, 279.0 + focus::FOCUS_H))
            .is_empty());
        assert_eq!(
            rig.click(Vec2::new(300.0, 336.0 + focus::FOCUS_H)),
            range(RangeAction::Zoom(2))
        );
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
        assert!(
            rig.click(Vec2::new(190.0, 279.0 + focus::FOCUS_H))
                .is_empty(),
            "the Economy tab"
        );
        assert_eq!(
            rig.click(Vec2::new(160.0, 368.0 + focus::FOCUS_H)),
            range(RangeAction::Stock {
                player: BLUE,
                mass: Some(1000),
                energy: None
            }),
            "fill the materials"
        );
        assert_eq!(
            rig.click(Vec2::new(270.0, 368.0 + focus::FOCUS_H)),
            range(RangeAction::Income {
                player: BLUE,
                resource: 0,
                step: 1
            })
        );
        // Red's economy is its own.
        assert!(rig
            .click(Vec2::new(122.0, 318.0 + focus::FOCUS_H))
            .is_empty());
        assert_eq!(
            rig.click(Vec2::new(57.0, 428.0 + focus::FOCUS_H)),
            range(RangeAction::Stock {
                player: RED,
                mass: None,
                energy: Some(0)
            }),
            "empty red's energy"
        );
        // A power shortage in one click: free build off, a quarter of the income, the store dry.
        assert_eq!(
            rig.click(Vec2::new(150.0, 466.0 + focus::FOCUS_H)),
            vec![
                HudAction::Range(RangeAction::FreeBuild(false)),
                HudAction::Range(RangeAction::SetIncome {
                    player: RED,
                    resource: 1,
                    index: 2
                }),
                HudAction::Range(RangeAction::Stock {
                    player: RED,
                    mass: None,
                    energy: Some(0)
                }),
            ]
        );
        assert_eq!(
            rig.click(Vec2::new(71.0, 466.0 + focus::FOCUS_H)),
            range(RangeAction::FreeBuild(false))
        );
        let normal = rig.click(Vec2::new(308.0, 466.0 + focus::FOCUS_H));
        assert!(normal.contains(&HudAction::Range(RangeAction::SetIncome {
            player: RED,
            resource: 1,
            index: INCOME_NORMAL
        })));
        assert_eq!(
            rig.click(Vec2::new(100.0, 500.0 + focus::FOCUS_H)),
            range(RangeAction::Wrecks)
        );
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
    fn the_free_camera_folds_the_panels_away_and_gives_them_back() {
        let mut rig = Rig::new("aster_t1_tank");
        rig.settle();
        assert!(
            rig.hud.covers(speed_faster()),
            "the top bar keeps the pointer"
        );
        rig.hud.free.set(true);
        rig.settle();
        // Folded: nothing it held takes a click, and the battlefield gets the pointer.
        assert_eq!(rig.click(speed_faster()), vec![]);
        assert!(!rig.hud.covers(speed_faster()));
        assert!(!rig.hud.covers(chart()));
        // The unit panel, bottom left.
        assert!(!rig.hud.covers(Vec2::new(100.0, VIEWPORT.y - 100.0)));
        // Only the guide at the foot of the screen is left, and it keeps its own clicks.
        assert!(rig
            .hud
            .covers(Vec2::new(VIEWPORT.x * 0.5, VIEWPORT.y - 40.0)));
        rig.hud.free.set(false);
        rig.settle();
        assert_eq!(rig.click(speed_faster()), vec![HudAction::SetSpeed(200)]);
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
        assert_eq!(
            rig.click(speed_pick(3)),
            vec![],
            "the speed in force is no change"
        );
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
        assert!(
            !rig.hud.covers(chart),
            "a folded map leaves the battlefield clear"
        );
        rig.click(Vec2::new(1920.0 - EDGE - 48.0, fold.y));
        assert!(!rig.hud.minimap_hidden);
        // A network match owns no clock: the speed and pause controls are dead.
        rig.view.status.owns_clock = false;
        assert_eq!(rig.click(pause_button()), vec![]);
        assert_eq!(rig.click(speed_faster()), vec![]);
    }

    #[test]
    fn the_pause_strip_leaves_the_battlefield_clear() {
        let mut rig = Rig::new("aster_t1_tank");
        rig.view.paused = true;
        rig.settle();
        for p in [Vec2::new(960.0, 540.0), Vec2::new(960.0, 300.0)] {
            assert!(!rig.hud.covers(p), "the pause card covers {p}");
        }
        // Its Resume button sits at the top, between the economy and the clock.
        let resume = (700..1400)
            .step_by(8)
            .map(|x| Vec2::new(x as f32, EDGE + 22.0))
            .find(|&p| rig.click(p) == [HudAction::Pause]);
        assert!(resume.is_some(), "no Resume on the pause strip");
    }

    #[test]
    fn the_order_card_offers_what_the_selection_can_do_by_family() {
        let mut rig = Rig::new("aster_t1_tank");
        assert_eq!(
            rig.click(order_slot(0, 0)),
            vec![HudAction::Target(Targeting::Move)]
        );
        assert_eq!(
            rig.click(order_slot(0, 1)),
            vec![HudAction::Target(Targeting::Patrol)]
        );
        assert_eq!(
            rig.click(order_slot(1, 0)),
            vec![HudAction::Target(Targeting::Attack)]
        );
        assert_eq!(
            rig.click(order_slot(1, 1)),
            vec![HudAction::Target(Targeting::AttackMove)]
        );
        assert_eq!(
            rig.click(order_slot(1, 2)),
            vec![HudAction::Target(Targeting::AttackGround)]
        );
        assert_eq!(
            rig.click(order_slot(1, 3)),
            vec![HudAction::Target(Targeting::Bombard)]
        );
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
        assert_eq!(
            rig.click(order_slot(0, 0)),
            vec![HudAction::Target(Targeting::Move)]
        );
        assert_eq!(
            rig.click(order_slot(0, 1)),
            vec![HudAction::Target(Targeting::Patrol)]
        );
        assert_eq!(
            rig.click(order_slot(3, 0)),
            vec![HudAction::Target(Targeting::Assist)]
        );
        assert_eq!(
            rig.click(order_slot(3, 1)),
            vec![HudAction::PauseWork(true)]
        );
        assert_eq!(rig.click(order_slot(3, 2)), vec![HudAction::Stop]);
        let repeat = Vec2::new(1920.0 - EDGE - 12.0 - 48.0, DECK_Y - GAP - 31.0);
        assert_eq!(rig.click(repeat), vec![HudAction::Repeat(true)]);
        // Pause sits beside it on the strip.
        let pause = Vec2::new(repeat.x - 48.0 - 10.0 - 48.0, repeat.y);
        assert_eq!(rig.click(pause), vec![HudAction::PauseWork(true)]);
    }

    #[test]
    fn the_economy_focus_switches_turn_one_on_or_neither() {
        use mc_sim::focus::Focus;
        let mut rig = Rig::new("aster_t1_tank");
        // Mines first under materials, Power first under energy, in the strip under the figures.
        let y = EDGE + ECONOMY_H - focus::FOCUS_H + 19.0;
        let mines = Vec2::new(EDGE + 60.0, y);
        let power = Vec2::new(EDGE + 16.0 + 306.0 + 60.0, y);
        assert_eq!(rig.click(power), vec![HudAction::Focus(Focus::Power)]);
        assert_eq!(rig.click(mines), vec![HudAction::Focus(Focus::Materials)]);
        rig.view.status.players[0].focus = Focus::Materials;
        rig.frame(&Input::default());
        assert_eq!(rig.click(mines), vec![HudAction::Focus(Focus::Neither)]);
        assert!(rig.hud.covers(power));
    }

    #[test]
    fn paused_work_offers_resume_on_the_card_and_the_strip() {
        let mut rig = Rig::new("aster_t1_land_factory");
        rig.view.frame.units[0].status[0] |= mc_sim::mirror::UNIT_PAUSED;
        assert_eq!(
            rig.click(order_slot(3, 1)),
            vec![HudAction::PauseWork(false)]
        );
        let repeat = Vec2::new(1920.0 - EDGE - 12.0 - 48.0, DECK_Y - GAP - 31.0);
        let resume = Vec2::new(repeat.x - 48.0 - 10.0 - 48.0, repeat.y);
        assert_eq!(rig.click(resume), vec![HudAction::PauseWork(false)]);
        // A tank has no work to pause: its card has no such order.
        let mut rig = Rig::new("aster_t1_tank");
        rig.view.frame.units[0].status[0] |= mc_sim::mirror::UNIT_PAUSED;
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
            hits.contains(&HudAction::CancelOrder {
                kind: OrderKind::Build,
                pos: at
            }),
            "{hits:?}"
        );
    }

    #[test]
    fn the_commander_card_is_always_there_and_selects_it() {
        let mut rig = Rig::new("aster_commander");
        rig.view.selection.clear();
        let card = Vec2::new(
            EDGE + COMMANDER_W * 0.5,
            EDGE + ECONOMY_H + GAP + COMMANDER_H * 0.5,
        );
        assert_eq!(
            rig.click(card),
            vec![HudAction::Select {
                units: vec![7],
                focus: true
            }]
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
            let at = size
                * Vec2::new(
                    0.1 + 0.8 * (k % 8) as f32 / 7.0,
                    0.1 + 0.8 * (k / 8) as f32 / 5.0,
                );
            let pos = [at.x, at.y, 0.0];
            let id = 100 + k;
            rig.view.index_of.insert(id, rig.view.frame.units.len());
            rig.view.frame.units.push(UnitInstance {
                pos,
                prev_pos: pos,
                unit_id: id,
                ..template
            });
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
        let card = Vec2::new(
            EDGE + COMMANDER_W * 0.5,
            EDGE + ECONOMY_H + GAP + COMMANDER_H * 0.5,
        );
        assert_eq!(
            rig.click(card),
            vec![HudAction::Select {
                units: vec![8],
                focus: true
            }]
        );
    }

    #[test]
    fn a_builders_queued_mines_bring_up_their_territories_once_each() {
        let mut rig = Rig::new("aster_t1_engineer");
        rig.camera.focus = glam::Vec3::new(4000.0, 4000.0, 0.0);
        rig.camera.distance = 3000.0;
        let mine = rig.blueprints.id_of("aster_core_mine").expect("core mine");
        let order = |x: i32| QueuedOrder {
            formation: 0,
            offset: [0.0; 2],
            moving_slot: None,
            formation_phase: 0,
            kind: OrderKind::Build,
            pos: [x as f32, 4000.0],
            at: mc_core::FxVec2::from_ints(x, 4000),
            blueprint: mine,
            radius: 0.0,
        };
        let drawn = |rig: &mut Rig, queues: Vec<UnitOrders>| {
            rig.view.status.queues = queues;
            rig.settle();
            rig.overlay.vertices.len()
        };
        let none = drawn(&mut rig, Vec::new());
        let queued = drawn(
            &mut rig,
            vec![UnitOrders {
                unit_id: 7,
                orders: vec![order(4000), order(4300)],
                ..Default::default()
            }],
        );
        assert!(queued > none, "{queued} vertices, {none} with none queued");
        // A second builder carrying the same order draws nothing more.
        let shared = drawn(
            &mut rig,
            vec![
                UnitOrders {
                    unit_id: 7,
                    orders: vec![order(4000), order(4300)],
                    ..Default::default()
                },
                UnitOrders {
                    unit_id: 9,
                    orders: vec![order(4300)],
                    ..Default::default()
                },
            ],
        );
        assert_eq!(shared, queued);
    }

    #[test]
    fn an_idle_engineer_tile_steps_through_them_and_shift_takes_them_all() {
        let mut rig = Rig::new("aster_t1_engineer");
        rig.view.selection.clear();
        rig.view.frame.units[0].owner_flags |= STATE_IDLE;
        let mut second = rig.view.frame.units[0];
        second.unit_id = 9;
        rig.view.frame.units.push(second);
        rig.view.index_of.insert(9, 1);
        // No commander: the card sits right under the economy, its first tile right
        // of the title block.
        let card_y = EDGE + ECONOMY_H + GAP;
        let tile = Vec2::new(EDGE + 8.0 + 72.0 + 8.0 + 18.0, card_y + 8.0 + 18.0);
        let one = |id| {
            vec![HudAction::Select {
                units: vec![id],
                focus: true,
            }]
        };
        assert_eq!(rig.click(tile), one(7));
        assert_eq!(rig.click(tile), one(9));
        assert_eq!(rig.click(tile), one(7), "and round again");
        rig.view.shift = true;
        assert_eq!(
            rig.click(tile),
            vec![HudAction::Select {
                units: vec![7, 9],
                focus: false
            }]
        );
        rig.view.selection = vec![9, 7];
        assert_eq!(
            rig.click(tile),
            vec![HudAction::Select {
                units: vec![7, 9],
                focus: true
            }],
            "a second shift-click finds them"
        );
        rig.view.shift = false;
    }

    #[test]
    fn a_structure_that_upgrades_offers_it_on_its_next_tier() {
        let mut rig = Rig::new("aster_t1_radar");
        rig.frame(&Input::default());
        let radar = rig
            .blueprints
            .unit(rig.blueprints.id_of("aster_t1_radar").unwrap());
        assert!(
            radar.upgrades_to.is_some(),
            "the test needs an upgradable structure"
        );
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
            build_x(4)
                + 14.0
                + 104.0
                + n as f32 * 172.0
                + ors as f32 * 34.0
                + arrows as f32 * 26.0
                + 86.0,
            DECK_Y + 44.0 + row as f32 * (row_h + 4.0) + row_h * 0.5,
        )
    }

    #[test]
    fn refits_queue_their_earlier_tiers_and_ask_before_replacing() {
        let mut rig = Rig::new("aster_commander+mfe");
        let bps = rig.blueprints.clone();
        let set = bps
            .refit_set(bps.id_of("aster_commander").unwrap())
            .unwrap();
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
            vec![HudAction::Refit(vec![
                kit("gun", "cannon"),
                kit("gun", "railgun")
            ])]
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
        assert_eq!(
            rig.click(yes),
            vec![HudAction::Refit(vec![kit("back", "shield")])]
        );
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
        assert_eq!(
            rig.click(order_slot(0, 0)),
            vec![],
            "watching has no order card"
        );
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
            rig.click(Vec2::new(
                EDGE + 18.0 + 52.0 + 6.0 + 34.0 + 4.0 + 17.0,
                EDGE + 47.0
            )),
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
        assert_eq!(
            rig.frame(&Input::default()),
            vec![HudAction::Build(items[1].id)]
        );
    }

    #[test]
    fn details_opens_a_card_of_lore_and_weapons_over_the_panel() {
        let mut rig = Rig::new("aster_t1_tank");
        let details = Vec2::new(INFO_X + 336.0 - 16.0 - 37.0, DECK_Y + 26.0);
        rig.click(details);
        assert!(rig.hud.details_open);
        rig.frame(&Input::default());
        assert!(
            rig.hud.covers(Vec2::new(INFO_X + 200.0, DECK_Y - 40.0)),
            "the card sits over the deck"
        );
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
        assert!(
            !rig.hud.details_open,
            "coming back to the unit finds the card closed"
        );
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
        assert!(
            !later.is_empty() && later != first,
            "{first:?} then {later:?}"
        );
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
        let rider = |unit_id, blueprint| CargoUnit {
            unit_id,
            blueprint,
            health: 1.0,
            room: 2,
        };
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
        // The hold is a panel right of the order card, a card per kind aboard: the
        // first card is the tanks, and a click lets one of them out.
        let mut first = None;
        'scan: for y in (0..12).map(|i| DECK_Y + 50.0 + i as f32 * 4.0) {
            for x in (0..140).map(|i| ORDERS_X + 100.0 + i as f32 * 8.0) {
                let got = rig.click(Vec2::new(x, y));
                if got.iter().any(|a| matches!(a, HudAction::UnloadUnits(_))) {
                    assert_eq!(
                        got,
                        vec![HudAction::UnloadUnits(vec![21])],
                        "one click lets one unit out"
                    );
                    first = Some(Vec2::new(x, y));
                    break 'scan;
                }
            }
        }
        let first = first.expect("no hold card to click right of the order card");
        // Shift-click: every unit of that kind.
        rig.view.shift = true;
        assert_eq!(rig.click(first), vec![HudAction::UnloadUnits(vec![21, 23])]);
        rig.view.shift = false;
        // Ctrl-click: pick that kind alongside the ship instead.
        rig.view.ctrl = true;
        assert_eq!(
            rig.click(first),
            vec![HudAction::Select {
                units: vec![7, 21, 23],
                focus: false
            }]
        );
        rig.view.ctrl = false;
        // The order card has a Transport column: down, it offers Take Off; aloft, Land Here.
        let card = |rig: &mut Rig, row: usize| -> Vec<HudAction> {
            (0..6)
                .flat_map(|col| {
                    let x = ORDERS_X
                        + 14.0
                        + col as f32 * (selection::ORDER_W + selection::ORDER_GAP)
                        + 50.0;
                    let y = DECK_Y
                        + 36.0
                        + row as f32 * (selection::ORDER_H + selection::ORDER_GAP)
                        + 20.0;
                    rig.click(Vec2::new(x, y))
                })
                .collect()
        };
        assert!(
            card(&mut rig, 3).contains(&HudAction::TakeOff),
            "no Take Off on the card"
        );
        assert!(
            card(&mut rig, 2).contains(&HudAction::UnloadHere),
            "no Unload Here on the card"
        );
        assert!(card(&mut rig, 0).contains(&HudAction::Target(crate::game::Targeting::Land)));
        assert!(card(&mut rig, 1).contains(&HudAction::Target(crate::game::Targeting::Unload)));
        rig.view.status.queues[0].cargo = Some(cargo(LiftPhase::InFlight));
        assert!(
            card(&mut rig, 3).contains(&HudAction::LandHere),
            "no Land Here while aloft"
        );
        assert_eq!(
            super::cargo::status(&cargo(LiftPhase::RampOpening)).0,
            "Ramp opening"
        );
        let mut out = cargo(LiftPhase::Unloading);
        out.to_unload = 2;
        assert_eq!(super::cargo::status(&out).0, "Unloading \u{b7} 2 left");
    }
}
