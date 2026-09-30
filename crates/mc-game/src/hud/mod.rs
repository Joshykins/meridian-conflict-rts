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
mod groups;
pub mod icons;
mod issue_mark;
pub use issue_mark::IssueMark;
mod mine;
mod mine_coast;
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
pub(crate) use build::queue::batch_linked;
pub use selection::ordered_as;
mod match_state;
pub mod style;
pub mod thumbs;
mod titan;
mod top_bar;
mod unit_picker;
mod volatile;
pub mod warp;

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
pub const SPEEDS: [u32; 9] = [10, 25, 50, 100, 150, 200, 400, 800, 1200];

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
/// The narrowest the stall chip right of the economy gets.
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
    /// The selected factories' products form up outside and leave together (`true`), or
    /// each leaves as it is made, those waiting at once.
    Batch(bool),
    /// The selected factories' batches leave now, as many as are waiting.
    SendBatch,
    /// The selected factories' batches wait for this many (`None`: for their queues' laps).
    BatchSize(Option<u16>),
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
        to: mc_core::PlayerMask,
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
        let c = self.view.colors[owner as usize % mc_core::MAX_PLAYERS];
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
    /// The map counted for the mine survey, and each mine's coast; built on first use.
    survey: mine_coast::Survey,
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
        mine_marks(ui, s, &mut self.survey);
        self.pause_frame(ui, view.paused && !view.menu_open);
        if !view.observing && !self.free.on {
            groups::badges(ui, s);
        }
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
            // Down to the line over the deck, where chat rises from.
            let bottom = h - EDGE - DECK_H - 24.0 - 8.0 - GAP;
            let under_idle = builders::idle_cards(self, ui, s, under_economy, bottom);
            groups::card(self, ui, s, under_idle, bottom);
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
        let report_top = if !view.show_profiler {
            // Opened, it starts where it belongs; it glides only as the camera changes.
            ui.snap(id("report-top", 0), goal);
            self.issues.column = Rect::default();
            None
        } else {
            // Drawn last, over the deck and its chips, but first to the pointer.
            ui.hold(id("debug-column", 0), self.issues.column);
            if !self.free.on && self.issues.column.h > 0.0 {
                right_top = self.issues.column.bottom() + GAP;
            }
            Some(ui.ease(id("report-top", 0), goal, 9.0))
        };
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
        // Chat rises from over the deck.
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
        if let Some(top) = report_top {
            self.debug_column(ui, s, top);
        }

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

    /// The report card, then the profiler under it (it can run off the bottom),
    /// in the right column from `top`, over whatever else is there.
    fn debug_column(&mut self, ui: &mut Ui, s: &Scene, top: f32) {
        ui.release(id("debug-column", 0));
        let right = ui.size.x - EDGE;
        let card = self.issues.draw(ui, s, Vec2::new(right, top));
        self.claim(ui, card);
        if let Some(mark) = self.issues.take_fresh() {
            self.replay_bar.marked(mark);
        }
        let prof = profiler::draw(ui, s, Vec2::new(right, card.bottom() + GAP));
        self.claim(ui, prof);
        let x = card.x.min(prof.x);
        self.issues.column = Rect::new(x, card.y, right - x, prof.bottom() - card.y);
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
        // The warp order's card says what the jump takes (`warp_marks::cursor_card`).
        Mode::Normal | Mode::Target(Targeting::Warp) => return,
        Mode::Target(t) => (
            t.label().to_owned(),
            match t {
                Targeting::Attack
                | Targeting::AttackMove
                | Targeting::AttackGround
                | Targeting::Strike
                | Targeting::Bombard => style::Family::Combat.tone(),
                Targeting::Move | Targeting::Patrol => style::Family::Movement.tone(),
                Targeting::Assist => style::Family::Engineering.tone(),
                Targeting::Reclaim => MASS,
                Targeting::Guard => style::Family::Stance.tone(),
                Targeting::Land | Targeting::Unload => style::Family::Transport.tone(),
                Targeting::Nuke => silo::WARHEAD,
                Targeting::Warp => warp::WARP,
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
        Mode::Target(Targeting::Reclaim) => {
            "Click a wreck or unit  \u{b7}  Click ground to reclaim on the way  \u{b7}  Drag out an area to clear"
        }
        Mode::Target(Targeting::Assist) => {
            "Click a unit to help it  \u{b7}  Press on the ground, drag out an area to work"
        }
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
mod tests;
