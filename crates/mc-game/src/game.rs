//! A match in progress: camera, selection, orders, the HUD and the in-match menu.
//!
//! Everything here reads the published render mirror and status; it never
//! touches the simulation directly. Player intent leaves as `Command`s.

use crate::audio::{Audio, Sfx};
use crate::hud::{self, Hud, HudAction};
use crate::orders::{self, Dropped, Field, OrderMap};
use crate::pointer::Pointer;
use crate::range::{self, Range, RangeAction};
use crate::rings::{Reach, Rings};
use crate::settings::Settings;
use crate::sim_thread::{self, SceneScript, SimHandle, SimSetup, SimStatus, Watch};
use crate::ui::pause::{self, Heading, PauseAction};
use crate::ui::{self, options, palette, Ui};
use glam::{Vec2, Vec3};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, BlueprintId, Blueprints};
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::camera::MIN_DISTANCE;
use mc_render::{Camera, FrameInput, Mark, Overlay, Renderer};
use mc_sim::mirror::{UnitInstance, UnitOrders, KIND_GHOST, KIND_WRECK};
use mc_sim::tables::{flag, OrderKind};
use mc_sim::{Command, Handle, RenderFrame};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const DRAG_THRESHOLD: f32 = 6.0;
/// Distance factor of one wheel notch.
const ZOOM_STEP: f32 = 0.8;
/// How fast the camera closes on the distance the wheel asked for, per second.
const ZOOM_RATE: f32 = 16.0;
const TICK_SECONDS: f32 = 0.1;
/// Most selected units whose order queues are asked for (and drawn).
const MAX_WATCHED: usize = 64;
/// A control group's key pressed twice within this long also brings the camera.
const DOUBLE_TAP: f32 = 0.35;
/// A unit clicked twice within this long selects every one of its type on screen.
const DOUBLE_CLICK: f32 = 0.35;
/// Test range: how far east of the pad the camera looks, so the pad clears the panel.
const RANGE_LOOK_AHEAD: f32 = 60.0;

/// Everything a match starts from, whether it was set up in the front end,
/// on the command line, or in a relay's lobby.
pub struct GameStart {
    pub map: Arc<MapFile>,
    /// Player colours by player index, linear RGB.
    pub colors: [[f32; 3]; 8],
    pub session: Box<dyn mc_net::Session + Send>,
    /// Session events already polled while waiting in a lobby, `Started` included.
    pub prefetched: Vec<mc_net::SessionEvent>,
    /// The slot this machine plays.
    pub local: u8,
    /// The map start position the camera opens on.
    pub start_index: usize,
    pub scene: Option<(SceneScript, SceneScript)>,
    /// The match is the test range, with this unit on the pad.
    pub range: Option<BlueprintId>,
}

/// An order picked from the order card (or its key) that still needs a target.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Targeting {
    Move,
    Attack,
    AttackMove,
    Assist,
    Reclaim,
}

impl Targeting {
    pub fn label(self) -> &'static str {
        match self {
            Targeting::Move => "MOVE",
            Targeting::Attack => "ATTACK",
            Targeting::AttackMove => "ATTACK-MOVE",
            Targeting::Assist => "ASSIST",
            Targeting::Reclaim => "RECLAIM",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Normal,
    Target(Targeting),
    Place(BlueprintId),
    /// Test range: the next click on the ground spawns the subject there.
    Spawn,
}

pub struct View {
    /// The slot this machine plays.
    pub local: u8,
    pub frame: RenderFrame,
    pub status: SimStatus,
    /// Unit id -> index in `frame.units`.
    pub index_of: HashMap<u32, usize>,
    pub selection: Vec<u32>,
    pub mode: Mode,
    pub fps: f32,
    pub cpu_ms: f32,
    pub show_profiler: bool,
    /// The in-match menu is up: the HUD leaves the middle of the screen to it.
    pub menu_open: bool,
    /// Player colours by player index, linear RGB.
    pub colors: [[f32; 3]; 8],
    /// Game speed, percent of real time.
    pub speed: u32,
    /// The player stopped the clock (single-player only).
    pub paused: bool,
    pub shift: bool,
    /// Control groups, by their key.
    pub groups: [Vec<u32>; 10],
    /// The test range, when this match is one.
    pub range: Option<Range>,
    /// The range rings on the ground this frame, by kind: the farthest reach and its dead zone.
    pub reaches: Vec<(Reach, f32, f32)>,
}

impl View {
    pub fn new(local: u8, colors: [[f32; 3]; 8], show_profiler: bool) -> View {
        View {
            local,
            frame: RenderFrame::default(),
            status: SimStatus::default(),
            index_of: HashMap::new(),
            selection: Vec::new(),
            mode: Mode::Normal,
            fps: 0.0,
            cpu_ms: 0.0,
            show_profiler,
            menu_open: false,
            colors,
            speed: 100,
            paused: false,
            shift: false,
            groups: Default::default(),
            range: None,
            reaches: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GameEvent {
    /// Back to the front end.
    Leave,
    Quit,
    /// Test range: read `data/` again and open the range on this blueprint key.
    ReloadRange(String),
}

/// What a frame needs from the application around the match.
pub struct FrameCtx<'a> {
    pub renderer: &'a mut Renderer,
    pub overlay: &'a mut Overlay,
    pub input: &'a ui::Input,
    pub memory: &'a mut ui::Memory,
    pub audio: &'a Audio,
    pub settings: &'a mut Settings,
    /// Set when the in-match menu changed a setting.
    pub settings_changed: &'a mut bool,
    /// Set when that setting needs the window touched (full screen).
    pub display_changed: &'a mut bool,
    pub now: Instant,
    pub dt: f32,
    /// Seconds since the application started.
    pub time: f32,
}

struct Menu {
    heading: Heading,
    enter: f32,
    closing: bool,
    /// The settings screen is up over the menu; how far in it is.
    settings: Option<(f32, bool)>,
}

/// What each blueprint sounds like, as ids into the sound library in use.
struct SoundTable {
    /// The library generation the ids belong to (`Audio::library`).
    generation: u32,
    units: Vec<UnitSoundIds>,
    /// The library's `reclaim_beam` loop, `reclaim_start` and `reclaim_end`: every reclaimer shares them.
    reclaim: [Option<mc_data::SoundId>; 3],
    /// `build_beam`, `build_start`, `build_end`: every builder shares them.
    build: [Option<mc_data::SoundId>; 3],
}

struct UnitSoundIds {
    death: Option<mc_data::SoundId>,
    moving: Option<mc_data::SoundId>,
    /// A walker's footfall, and the ground one of its steps covers (half the model's stride):
    /// the same figure the vertex shader plants the feet by, so the two keep time.
    step: Option<(mc_data::SoundId, f32)>,
    select: Option<mc_data::SoundId>,
    weapons: Vec<WeaponSoundIds>,
}

struct WeaponSoundIds {
    fire: Option<mc_data::SoundId>,
    charge: Option<mc_data::SoundId>,
    impact: Option<mc_data::SoundId>,
    ground: Option<mc_data::SoundId>,
}

pub struct Game {
    map: Arc<MapFile>,
    blueprints: Arc<Blueprints>,
    /// Sound ids per blueprint; `None` until first needed.
    sounds: Option<SoundTable>,
    /// Who had a reclaim beam on last tick, and where its emitter was: a beam is heard starting and stopping.
    beaming: std::collections::HashMap<u32, [f32; 3]>,
    /// Who had a construction beam on last tick, and where it met the work.
    welding: std::collections::HashMap<u32, [f32; 3]>,
    rings: Rings,
    camera: Camera,
    sim: SimHandle,
    view: View,
    hud: Hud,
    serial: u64,
    published_at: Instant,
    cursor: Vec2,
    keys: HashSet<KeyCode>,
    shift: bool,
    ctrl: bool,
    left_down: Option<Vec2>,
    /// World site of a Place-mode press, so a drag can line buildings from there.
    place_from: Option<FxVec2>,
    /// Orders on the map: their lines, the ghosts of planned structures, the one being dragged.
    orders: OrderMap,
    /// What a click would do right now; the application shows it as the mouse cursor.
    pointer: Pointer,
    middle_down: bool,
    /// The distance the wheel asked for, while the camera is still easing to it.
    zoom_target: Option<f32>,
    /// The ground point the zoom holds still, and the pixel it was under.
    zoom_anchor: Option<(Vec3, Vec2)>,
    /// Where the last zoom step left the focus: a pan or a jump since then drops the anchor.
    zoom_focus: Vec2,
    menu: Option<Menu>,
    result_shown: bool,
    /// What the sim thread was last asked to publish orders for.
    watched: Watch,
    chart_ready: bool,
    /// The control group key pressed last, and when.
    last_group: Option<(usize, Instant)>,
    /// Last single-clicked type and owner, and when: a second click of the same
    /// kind takes every one of them on screen.
    last_pick: Option<(u32, u8, Instant)>,
    /// The selection as it was last answered with a sound, and that sound and when.
    answered: Vec<u32>,
    last_answer: Option<(mc_data::SoundId, Instant)>,
    /// Something the match wants from the application, handed over by `frame`.
    event: Option<GameEvent>,
}

impl Game {
    pub fn new(
        start: GameStart,
        blueprints: Arc<Blueprints>,
        pool: Arc<Pool>,
        viewport: Vec2,
        show_profiler: bool,
    ) -> Game {
        let sim = sim_thread::spawn(
            SimSetup {
                map: start.map.clone(),
                blueprints: blueprints.clone(),
                pool,
                prefetched: start.prefetched,
                scene: start.scene,
            },
            start.session,
        );
        let size = Vec2::from(start.map.info().size_metres().to_f32());
        let mut camera = Camera::new(size, viewport);
        if let Some(at) = start.map.start_positions().get(start.start_index) {
            camera.focus = Vec2::from(at.to_f32()).extend(0.0);
            camera.distance = 700.0;
        }
        let mut view = View::new(start.local, start.colors, show_profiler);
        if let Some(subject) = start.range {
            let pad = crate::setup::range_pad(&start.map);
            // The pad clear of the panel on the left, the firing lane running off to the right.
            camera.focus =
                Vec2::from(pad.to_f32()).extend(0.0) + Vec3::new(RANGE_LOOK_AHEAD, 0.0, 0.0);
            camera.distance = range::ZOOMS[1];
            view.range = Some(Range::new(pad, subject));
        }
        Game {
            map: start.map,
            rings: Rings::new(&blueprints),
            blueprints,
            sounds: None,
            beaming: Default::default(),
            welding: Default::default(),
            camera,
            sim,
            view,
            hud: Hud::default(),
            serial: 0,
            published_at: Instant::now(),
            cursor: Vec2::ZERO,
            keys: HashSet::new(),
            shift: false,
            ctrl: false,
            left_down: None,
            place_from: None,
            orders: OrderMap::default(),
            pointer: Pointer::Arrow,
            middle_down: false,
            zoom_target: None,
            zoom_anchor: None,
            zoom_focus: Vec2::ZERO,
            menu: None,
            result_shown: false,
            watched: Watch::default(),
            chart_ready: false,
            last_group: None,
            last_pick: None,
            answered: Vec::new(),
            last_answer: None,
            event: None,
        }
    }

    /// A failure the application ran into on the match's behalf (a reload that did not parse).
    pub fn complain(&mut self, message: &str) {
        self.hud.toast(message.to_uppercase(), palette::BAD);
    }

    pub fn pointer(&self) -> Pointer {
        self.pointer
    }

    pub fn tick(&self) -> u32 {
        self.view.status.tick
    }

    pub fn map(&self) -> &Arc<MapFile> {
        &self.map
    }

    pub fn resized(&mut self, viewport: Vec2) {
        self.camera.viewport = viewport;
    }

    fn open_menu(&mut self, heading: Heading, audio: &Audio) {
        if self.menu.is_none() {
            // Opened by a key, so there is no control to make the sound. The result has its stinger.
            if heading == Heading::Menu {
                audio.play(Sfx::Select);
            }
            self.menu = Some(Menu {
                heading,
                enter: 0.0,
                closing: false,
                settings: None,
            });
            // Held keys and drags must not carry on underneath the menu.
            self.keys.clear();
            self.left_down = None;
            self.place_from = None;
            self.orders.cancel();
            self.middle_down = false;
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent, r: &Renderer, audio: &Audio) {
        // Track modifiers and the pointer always; act on them only when the menu is down.
        match event {
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
                self.view.shift = self.shift;
                self.ctrl = m.state().control_key();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = Vec2::new(position.x as f32, position.y as f32);
                if self.middle_down && self.menu.is_none() {
                    self.camera.pan(p - self.cursor);
                }
                self.cursor = p;
            }
            _ if self.menu.is_some() => {}
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                // The wheel moves the target; `frame` eases the camera to it.
                let from = self.zoom_target.unwrap_or(self.camera.distance);
                self.zoom_target = Some(
                    (from * ZOOM_STEP.powf(lines)).clamp(MIN_DISTANCE, self.camera.max_distance()),
                );
                self.zoom_anchor = self.ground_under_cursor(r).map(|g| (g, self.cursor));
                self.zoom_focus = self.camera.focus.truncate();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.mouse_button(*button, *state == ElementState::Pressed, r, audio)
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        if self.keys.insert(code) {
                            self.key_pressed(code, audio);
                        }
                    } else {
                        self.keys.remove(&code);
                    }
                }
            }
            _ => {}
        }
    }

    /// Closes a fixed share of the remaining zoom every frame, in log space so
    /// it feels the same at every height and at every frame rate.
    fn ease_zoom(&mut self, dt: f32) {
        let Some(target) = self.zoom_target else {
            return;
        };
        let target = target.clamp(MIN_DISTANCE, self.camera.max_distance());
        if self.camera.focus.truncate() != self.zoom_focus {
            self.zoom_anchor = None;
        }
        let remaining = (target / self.camera.distance).ln();
        if remaining.abs() < 1e-3 {
            self.camera
                .zoom(target / self.camera.distance, self.zoom_anchor);
            self.zoom_target = None;
            return;
        }
        let share = 1.0 - (-dt * ZOOM_RATE).exp();
        self.camera
            .zoom((remaining * share).exp(), self.zoom_anchor);
        self.zoom_focus = self.camera.focus.truncate();
    }

    fn send(&self, command: Command) {
        // The sim thread is gone only after a fatal error, which the HUD already shows.
        let _ = self.sim.commands.send(command);
    }

    fn ground_under_cursor(&self, r: &Renderer) -> Option<Vec3> {
        let (origin, dir) = self.camera.ray(self.cursor);
        r.pick_ground(origin, dir)
    }

    fn team_of(&self, owner: u8) -> u8 {
        self.view
            .status
            .players
            .get(owner as usize)
            .map_or(owner, |p| p.team)
    }

    fn is_enemy(&self, owner: u8) -> bool {
        self.team_of(owner) != self.team_of(self.view.local)
    }

    /// The unit (or wreck) drawn nearest to a pixel, if any is within reach of it.
    fn unit_at(&self, pixel: Vec2) -> Option<usize> {
        let eye = self.camera.eye();
        let scale = self.camera.projection_scale();
        let mut best: Option<(f32, usize)> = None;
        for (i, u) in self.view.frame.units.iter().enumerate() {
            if u.owner_flags & (flag::IN_FACTORY as u32) << 8 != 0 {
                continue;
            }
            let centre = Vec3::from(u.pos) + Vec3::Z * u.radius * 0.5;
            let Some(p) = self.camera.project(centre) else {
                continue;
            };
            let reach = (u.radius * scale / (centre - eye).length().max(1.0)).max(11.0);
            let d = p.distance(pixel);
            if d <= reach && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }

    fn selected_ids(&self) -> Vec<Handle> {
        self.view.selection.iter().map(|id| Handle(*id)).collect()
    }

    fn selected_units(&self) -> impl Iterator<Item = &UnitInstance> {
        self.view
            .selection
            .iter()
            .filter_map(|id| self.view.index_of.get(id))
            .map(|&i| &self.view.frame.units[i])
    }

    fn selection_has(&self, categories: u32) -> bool {
        self.selected_units().any(|u| {
            self.blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .has(categories)
        })
    }

    fn mouse_button(&mut self, button: MouseButton, pressed: bool, r: &Renderer, audio: &Audio) {
        match (button, pressed) {
            (MouseButton::Middle, p) => self.middle_down = p,
            // A press on the HUD is the HUD's: it sees it through the interface's input.
            (MouseButton::Left, true) => {
                if !self.hud.covers(self.cursor) {
                    // With shift held, a press on an order picks it up.
                    let field = Field {
                        view: &self.view,
                        blueprints: &self.blueprints,
                        map: &self.map,
                        camera: &self.camera,
                        renderer: r,
                    };
                    if self.shift && self.orders.press(&field, self.cursor) {
                        self.place_from = None;
                    } else {
                        self.left_down = Some(self.cursor);
                        if let Mode::Place(blueprint) = self.view.mode {
                            self.place_from = self.placement(blueprint, r).map(|(pos, _)| pos);
                        }
                    }
                }
            }
            (MouseButton::Left, false) => {
                match self
                    .orders
                    .release(self.view.status.tick, self.cursor, DRAG_THRESHOLD)
                {
                    Some(Dropped::Moved(command)) => {
                        audio.play(Sfx::Order);
                        self.send(command);
                    }
                    Some(Dropped::Refused) => audio.play(Sfx::Deny),
                    // The order never left its place: an ordinary click.
                    Some(Dropped::Stayed) => self.left_released(self.cursor, r, audio),
                    None => {
                        if let Some(from) = self.left_down.take() {
                            self.left_released(from, r, audio);
                        }
                    }
                }
            }
            (MouseButton::Right, true) => {
                if self.orders.dragging() {
                    self.orders.cancel();
                } else if self.hud.covers(self.cursor) {
                } else if self.view.mode != Mode::Normal {
                    self.view.mode = Mode::Normal;
                    self.left_down = None;
                    self.place_from = None;
                } else {
                    let ground = self.ground_under_cursor(r).map(|g| g.truncate());
                    self.context_order(ground, self.unit_at(self.cursor), audio);
                }
            }
            _ => {}
        }
    }

    fn left_released(&mut self, from: Vec2, r: &Renderer, audio: &Audio) {
        match self.view.mode {
            Mode::Place(blueprint) => {
                let sites =
                    self.placing_sites(blueprint, from.distance(self.cursor) >= DRAG_THRESHOLD, r);
                let valid: Vec<FxVec2> = sites
                    .into_iter()
                    .filter(|(_, ok)| *ok)
                    .map(|(pos, _)| pos)
                    .collect();
                if valid.is_empty() {
                    audio.play(Sfx::Deny);
                } else {
                    audio.play(Sfx::Order);
                    let units = self.selected_ids();
                    let heading = Angle::from_degrees(270);
                    for (i, pos) in valid.into_iter().enumerate() {
                        self.send(Command::Build {
                            units: units.clone(),
                            blueprint,
                            pos,
                            heading,
                            queue: self.shift || i > 0,
                        });
                    }
                }
                if !self.shift {
                    self.view.mode = Mode::Normal;
                }
                self.place_from = None;
            }
            Mode::Target(targeting) => {
                let ground = self.ground_under_cursor(r).map(|g| g.truncate());
                self.targeted_order(targeting, ground, self.unit_at(self.cursor), audio);
            }
            Mode::Spawn => {
                let ground = self
                    .ground_under_cursor(r)
                    .map(|g| FxVec2::new(Fx::from_f32(g.x), Fx::from_f32(g.y)));
                match (ground, &self.view.range) {
                    (Some(pos), Some(range)) => {
                        audio.play(Sfx::Order);
                        self.send(range.spawn_at(pos));
                    }
                    _ => audio.play(Sfx::Deny),
                }
                if !self.shift {
                    self.view.mode = Mode::Normal;
                }
            }
            Mode::Normal => {
                let mut picked: Vec<u32> = Vec::new();
                // On the range either side can be picked up, and whoever owns the pick is the side commanded.
                let sides: &[u8] = if self.view.range.is_none() {
                    &[self.view.local]
                } else if self.view.local == range::BLUE {
                    &[range::BLUE, range::RED]
                } else {
                    &[range::RED, range::BLUE]
                };
                let mut side = self.view.local;
                let now = Instant::now();
                if from.distance(self.cursor) < DRAG_THRESHOLD {
                    let mut click_kind = None;
                    if let Some(i) = self.unit_at(self.cursor) {
                        let u = &self.view.frame.units[i];
                        let owner = (u.owner_flags & 0xFF) as u8;
                        if u.owner_flags & KIND_WRECK == 0 && sides.contains(&owner) {
                            let kind = (u.blueprint, owner);
                            picked = if self.last_pick.is_some_and(|(bp, own, at)| {
                                bp == kind.0
                                    && own == kind.1
                                    && (now - at).as_secs_f32() < DOUBLE_CLICK
                            }) {
                                let mut all = type_on_screen(
                                    &self.view.frame.units,
                                    &self.camera,
                                    kind.0,
                                    kind.1,
                                );
                                if !all.contains(&u.unit_id) {
                                    all.push(u.unit_id);
                                }
                                all
                            } else {
                                vec![u.unit_id]
                            };
                            side = owner;
                            click_kind = Some(kind);
                        }
                    }
                    self.last_pick = click_kind.map(|(bp, own)| (bp, own, now));
                } else {
                    self.last_pick = None;
                    for &owner in sides {
                        picked = self.box_pick(from.min(self.cursor), from.max(self.cursor), owner);
                        if !picked.is_empty() {
                            side = owner;
                            break;
                        }
                    }
                }
                if side != self.view.local {
                    self.take_control(side);
                }
                if self.shift {
                    for id in picked {
                        if !self.view.selection.contains(&id) {
                            self.view.selection.push(id);
                        }
                    }
                } else {
                    self.view.selection = picked;
                }
            }
        }
    }

    /// Box select: `owner`'s units inside the rectangle. The army wins over
    /// builders (engineers and the commander), so a drag across the front never
    /// pulls them into the fight; builders win over structures.
    fn box_pick(&self, min: Vec2, max: Vec2, owner: u8) -> Vec<u32> {
        let (mut army, mut builders, mut structures) = (Vec::new(), Vec::new(), Vec::new());
        for u in &self.view.frame.units {
            if u.owner_flags & KIND_WRECK != 0
                || (u.owner_flags & 0xFF) as u8 != owner
                || u.owner_flags & (flag::IN_FACTORY as u32) << 8 != 0
            {
                continue;
            }
            let Some(p) = self.camera.project(Vec3::from(u.pos)) else {
                continue;
            };
            if p.cmpge(min).all() && p.cmple(max).all() {
                let bp = self.blueprints.unit(BlueprintId(u.blueprint as u16));
                if !bp.is_mobile() {
                    structures.push(u.unit_id);
                } else if bp.builder.is_some() {
                    builders.push(u.unit_id);
                } else {
                    army.push(u.unit_id);
                }
            }
        }
        [army, builders, structures]
            .into_iter()
            .find(|v| !v.is_empty())
            .unwrap_or_default()
    }

    /// Test range: from now on this machine's orders are `player`'s.
    fn take_control(&mut self, player: u8) {
        self.send(Command::DebugControl { player });
        self.view.local = player;
        self.view.selection.clear();
        self.view.mode = Mode::Normal;
    }

    fn range_action(&mut self, action: RangeAction, audio: &Audio) {
        let View {
            range,
            selection,
            frame,
            local,
            ..
        } = &mut self.view;
        let Some(range) = range.as_mut() else { return };
        let acted = || {
            range
                .acted(selection, &frame.units, &self.blueprints, *local)
                .ids
                .into_iter()
                .map(Handle)
                .collect::<Vec<_>>()
        };
        let mut commands = Vec::new();
        match action {
            RangeAction::Subject(step) => {
                // A new subject gets a clean range to stand on.
                range.subject = range::step_subject(&self.blueprints, range.subject, step);
                return self.range_action(RangeAction::Reset, audio);
            }
            RangeAction::Count(step) => {
                range.count =
                    (range.count as i32 + step).clamp(0, range::COUNTS.len() as i32 - 1) as usize
            }
            RangeAction::Side(side) => range.side = side,
            RangeAction::ArmSpawn => {
                self.view.mode = if self.view.mode == Mode::Spawn {
                    Mode::Normal
                } else {
                    Mode::Spawn
                }
            }
            RangeAction::Damage(permille) => commands.push(Command::DebugDamage {
                units: acted(),
                permille,
            }),
            RangeAction::Remove => commands.push(Command::DebugRemove { units: acted() }),
            RangeAction::Flag(bit, on) => commands.push(Command::DebugSetFlags {
                units: acted(),
                set: if on { bit } else { 0 },
                clear: if on { 0 } else { bit },
            }),
            RangeAction::Build(permille) => commands.push(Command::DebugSetBuild {
                units: acted(),
                permille,
            }),
            RangeAction::Scenario(scenario) => {
                match range.stage(scenario, &self.blueprints, &frame.units) {
                    Ok(staged) => commands = staged,
                    Err(why) => {
                        audio.play(Sfx::Deny);
                        self.hud.toast(why, palette::WARN);
                    }
                }
            }
            RangeAction::Reset => {
                commands = range.reset(&self.blueprints);
                selection.clear();
                *local = range::BLUE;
                self.view.mode = Mode::Normal;
                self.hud.toast("RANGE RESET", palette::ACCENT);
            }
            RangeAction::Control(player) => return self.take_control(player),
            RangeAction::FreeBuild(on) => {
                range.free_build = on;
                commands.extend(
                    [range::BLUE, range::RED].map(|player| Command::DebugFreeBuild { player, on }),
                );
            }
            RangeAction::Zoom(preset) => {
                let pad = Vec2::from(range.pad.to_f32());
                self.zoom_target =
                    Some(range::ZOOMS[preset.min(2)].min(self.camera.max_distance()));
                self.zoom_anchor = None;
                if self.view.selection.is_empty() {
                    self.camera.focus = (pad
                        + Vec2::new(
                            RANGE_LOOK_AHEAD
                                * (range::ZOOMS[preset.min(2)] / range::ZOOMS[1]).min(1.0),
                            0.0,
                        ))
                    .extend(self.camera.focus.z);
                } else {
                    self.focus_selection();
                }
            }
            RangeAction::Reload => {
                self.event = Some(GameEvent::ReloadRange(
                    self.blueprints.unit(range.subject).key.clone(),
                ))
            }
        }
        for command in commands {
            self.send(command);
        }
    }

    /// The order chosen on the order card, aimed at a unit or a point on the
    /// ground; `None` where it cannot apply. The pointer asks too, so it shows
    /// exactly what a click would do.
    fn targeted_command(
        &self,
        targeting: Targeting,
        ground: Option<Vec2>,
        unit: Option<usize>,
    ) -> Option<Command> {
        let units = self.selected_ids();
        let queue = self.shift;
        let point = ground.map(|g| FxVec2::new(Fx::from_f32(g.x), Fx::from_f32(g.y)));
        let target = unit.map(|i| self.view.frame.units[i]);
        let is_wreck = |u: &UnitInstance| u.owner_flags & KIND_WRECK != 0;
        match targeting {
            Targeting::Move => point.map(|target| Command::Move {
                units,
                target,
                queue,
            }),
            Targeting::AttackMove => point.map(|target| Command::AttackMove {
                units,
                target,
                queue,
            }),
            Targeting::Attack => match target
                .filter(|u| !is_wreck(u) && self.is_enemy((u.owner_flags & 0xFF) as u8))
            {
                Some(u) => Some(Command::Attack {
                    units,
                    target: Handle(u.unit_id),
                    queue,
                }),
                // No unit there: fight towards the point instead.
                None if target.is_none() => point.map(|target| Command::AttackMove {
                    units,
                    target,
                    queue,
                }),
                None => None,
            },
            Targeting::Assist => target
                .filter(|u| {
                    !is_wreck(u)
                        && !self.is_enemy((u.owner_flags & 0xFF) as u8)
                        && !self.view.selection.contains(&u.unit_id)
                })
                .map(|u| Command::Assist {
                    units,
                    target: Handle(u.unit_id),
                    queue,
                }),
            // A wreck, or a live unit: the player's own (not an ally's) or an enemy's.
            Targeting::Reclaim => target.and_then(|u| {
                let owner = (u.owner_flags & 0xFF) as u8;
                if is_wreck(&u) {
                    Some(Command::ReclaimWreck {
                        units,
                        wreck: Handle(u.unit_id),
                        queue,
                    })
                } else if (owner == self.view.local || self.is_enemy(owner))
                    && !self.view.selection.contains(&u.unit_id)
                {
                    Some(Command::ReclaimUnit {
                        units,
                        target: Handle(u.unit_id),
                        queue,
                    })
                } else {
                    None
                }
            }),
        }
    }

    /// An order that cannot apply where it was aimed is refused and stays armed.
    fn targeted_order(
        &mut self,
        targeting: Targeting,
        ground: Option<Vec2>,
        unit: Option<usize>,
        audio: &Audio,
    ) {
        match self.targeted_command(targeting, ground, unit) {
            Some(command) => {
                audio.play(Sfx::Order);
                self.send(command);
                if !self.shift {
                    self.view.mode = Mode::Normal;
                }
            }
            None => audio.play(Sfx::Deny),
        }
    }

    /// What a right click does: attack, reclaim, assist, or else move (a factory's
    /// rally point), depending on what is there and on who is selected. The pointer asks too.
    fn context_command(&self, ground: Option<Vec2>, unit: Option<usize>) -> Option<Command> {
        if self.view.selection.is_empty() {
            return None;
        }
        let units = self.selected_ids();
        let queue = self.shift;
        let mobile = |u: &UnitInstance| {
            self.blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .is_mobile()
        };
        let builders = self.selected_units().any(|u| {
            mobile(u)
                && self
                    .blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .builder
                    .is_some()
        });
        let armed = self.selected_units().any(|u| {
            mobile(u)
                && !self
                    .blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .weapons
                    .is_empty()
        });
        let reclaimers = self.selected_units().any(|u| {
            self.blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .reclaims()
                .is_some()
        });
        if let Some(i) = unit {
            let u = self.view.frame.units[i];
            let owner = (u.owner_flags & 0xFF) as u8;
            if u.owner_flags & KIND_WRECK != 0 {
                if reclaimers {
                    return Some(Command::ReclaimWreck {
                        units,
                        wreck: Handle(u.unit_id),
                        queue,
                    });
                }
            } else if self.is_enemy(owner) {
                if armed {
                    return Some(Command::Attack {
                        units,
                        target: Handle(u.unit_id),
                        queue,
                    });
                }
                // Nothing to shoot it with: take it apart instead.
                if reclaimers {
                    return Some(Command::ReclaimUnit {
                        units,
                        target: Handle(u.unit_id),
                        queue,
                    });
                }
            } else if builders && !self.view.selection.contains(&u.unit_id) {
                return Some(Command::Assist {
                    units,
                    target: Handle(u.unit_id),
                    queue,
                });
            }
        }
        let target = ground.map(|g| FxVec2::new(Fx::from_f32(g.x), Fx::from_f32(g.y)))?;
        if self.selection_has(cat::MOBILE) {
            Some(Command::Move {
                units,
                target,
                queue,
            })
        } else if self.selection_has(cat::FACTORY) {
            Some(Command::SetRally {
                factories: units,
                pos: target,
            })
        } else {
            None
        }
    }

    fn context_order(&mut self, ground: Option<Vec2>, unit: Option<usize>, audio: &Audio) {
        if let Some(command) = self.context_command(ground, unit) {
            audio.play(Sfx::Order);
            self.send(command);
        }
    }

    /// The pointer for this frame: what a click here would do. `can_place` is
    /// whether the structure being placed (or the drag-line of them) can go down.
    fn pointer_for(&self, renderer: &Renderer, over_ui: bool, can_place: bool) -> Pointer {
        if over_ui {
            return Pointer::Arrow;
        }
        if self.middle_down {
            return Pointer::Pan;
        }
        match self.orders.in_hand() {
            Some(true) => return Pointer::Grabbing,
            Some(false) => return Pointer::Denied,
            None if self.orders.hovering() => return Pointer::Grab,
            None => {}
        }
        let ground = || self.ground_under_cursor(renderer).map(|g| g.truncate());
        let of = |command: Option<Command>| match command {
            Some(Command::Attack { .. }) => Pointer::Attack,
            Some(Command::AttackMove { .. }) => Pointer::AttackMove,
            Some(Command::Assist { .. }) => Pointer::Assist,
            Some(Command::ReclaimWreck { .. } | Command::ReclaimUnit { .. }) => Pointer::Reclaim,
            Some(Command::Move { .. }) => Pointer::Move,
            Some(_) => Pointer::Arrow,
            None => Pointer::Denied,
        };
        match self.view.mode {
            Mode::Place(_) => {
                if can_place {
                    Pointer::Place
                } else {
                    Pointer::Denied
                }
            }
            Mode::Spawn => {
                if ground().is_some() {
                    Pointer::Place
                } else {
                    Pointer::Denied
                }
            }
            Mode::Target(targeting) => {
                of(self.targeted_command(targeting, ground(), self.unit_at(self.cursor)))
            }
            // A box is being dragged out: the pointer is a corner of it.
            Mode::Normal
                if self
                    .left_down
                    .is_some_and(|from| from.distance(self.cursor) >= DRAG_THRESHOLD) =>
            {
                Pointer::Arrow
            }
            Mode::Normal => {
                let unit = self.unit_at(self.cursor);
                match self.context_command(ground(), unit) {
                    // Open ground says nothing: a right click there has always meant "go".
                    Some(Command::Move { .. } | Command::SetRally { .. }) | None => {
                        let mine = unit.map(|i| &self.view.frame.units[i]).is_some_and(|u| {
                            let owner = (u.owner_flags & 0xFF) as u8;
                            u.owner_flags & KIND_WRECK == 0
                                && (owner == self.view.local
                                    || (self.view.range.is_some() && owner <= range::RED))
                        });
                        if mine {
                            Pointer::Select
                        } else {
                            Pointer::Arrow
                        }
                    }
                    command => of(command),
                }
            }
        }
    }

    fn hud_action(&mut self, action: HudAction, audio: &Audio) {
        match action {
            HudAction::Build(blueprint) => {
                let bp = self.blueprints.unit(blueprint);
                if bp.is_structure() {
                    self.view.mode = Mode::Place(blueprint);
                    self.place_from = None;
                    self.left_down = None;
                } else {
                    self.send(Command::Produce {
                        factories: self.selected_ids(),
                        blueprint,
                        count: if self.shift { 5 } else { 1 },
                    });
                }
            }
            HudAction::Cancel(blueprint) => {
                for _ in 0..if self.shift { 5 } else { 1 } {
                    self.send(Command::CancelProduce {
                        factories: self.selected_ids(),
                        blueprint,
                    });
                }
            }
            HudAction::Target(targeting) => {
                self.view.mode = if self.view.mode == Mode::Target(targeting) {
                    Mode::Normal
                } else {
                    Mode::Target(targeting)
                }
            }
            HudAction::Upgrade => self.send(Command::Upgrade {
                units: self.selected_ids(),
            }),
            HudAction::CancelUpgrade => self.send(Command::CancelUpgrade {
                units: self.selected_ids(),
            }),
            HudAction::Stop => self.send(Command::Stop {
                units: self.selected_ids(),
            }),
            HudAction::Repeat(on) => self.send(Command::SetRepeat {
                factories: self.selected_ids(),
                repeat: on,
            }),
            HudAction::Select { units, focus } => {
                self.view.mode = Mode::Normal;
                self.view.selection = units;
                if focus {
                    self.focus_selection();
                }
            }
            HudAction::LookAt(at) => {
                self.camera.focus = at.extend(self.camera.focus.z);
                self.camera.clamp_focus();
            }
            HudAction::OrderAt(at) => {
                if self.view.mode != Mode::Normal {
                    self.view.mode = Mode::Normal;
                } else {
                    self.context_order(Some(at), None, audio);
                }
            }
            HudAction::TargetAt(at) => {
                if let Mode::Target(targeting) = self.view.mode {
                    self.targeted_order(targeting, Some(at), None, audio);
                }
            }
            HudAction::Menu => self.open_menu(Heading::Menu, audio),
            HudAction::Pause => self.toggle_pause(),
            HudAction::Speed(step) => self.step_speed(step),
            HudAction::Range(action) => self.range_action(action, audio),
        }
    }

    fn toggle_pause(&mut self) {
        if self.view.status.owns_clock {
            self.view.paused = !self.view.paused;
        } else {
            self.hud
                .toast("A NETWORK MATCH CANNOT BE PAUSED", palette::WARN);
        }
    }

    fn step_speed(&mut self, step: i32) {
        if !self.view.status.owns_clock {
            return self
                .hud
                .toast("GAME SPEED IS FIXED IN A NETWORK MATCH", palette::WARN);
        }
        let at = hud::SPEEDS
            .iter()
            .position(|s| *s == self.view.speed)
            .or(hud::SPEEDS.iter().position(|s| *s == 100))
            .unwrap_or(0) as i32;
        self.view.speed = hud::SPEEDS[(at + step).clamp(0, hud::SPEEDS.len() as i32 - 1) as usize];
        self.sim.speed.store(self.view.speed, Ordering::Relaxed);
    }

    /// Brings the camera over the middle of the selection.
    fn focus_selection(&mut self) {
        let (mut sum, mut n) = (Vec3::ZERO, 0.0);
        for u in self.selected_units() {
            sum += Vec3::from(u.pos);
            n += 1.0;
        }
        if n > 0.0 {
            self.camera.focus = sum / n;
            self.camera.clamp_focus();
        }
    }

    /// Arms an order from its key, if the selection could carry it out.
    fn arm(&mut self, targeting: Targeting) {
        let builders = self.selected_units().any(|u| {
            let bp = self.blueprints.unit(BlueprintId(u.blueprint as u16));
            bp.is_mobile() && bp.builder.is_some()
        });
        let able = match targeting {
            Targeting::Move | Targeting::Attack | Targeting::AttackMove => {
                self.selection_has(cat::MOBILE)
            }
            Targeting::Assist => builders,
            Targeting::Reclaim => self.selected_units().any(|u| {
                self.blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .reclaims()
                    .is_some()
            }),
        };
        if able {
            self.view.mode = Mode::Target(targeting);
        }
    }

    fn key_pressed(&mut self, code: KeyCode, audio: &Audio) {
        let digit = |c: KeyCode| {
            [
                KeyCode::Digit0,
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
                KeyCode::Digit5,
                KeyCode::Digit6,
                KeyCode::Digit7,
                KeyCode::Digit8,
                KeyCode::Digit9,
            ]
            .iter()
            .position(|k| *k == c)
        };
        match code {
            KeyCode::Escape => {
                if self.orders.dragging() {
                    self.orders.cancel();
                } else if self.view.mode != Mode::Normal {
                    self.view.mode = Mode::Normal;
                    self.left_down = None;
                    self.place_from = None;
                } else if !self.view.selection.is_empty() {
                    self.view.selection.clear();
                } else {
                    self.open_menu(Heading::Menu, audio);
                }
            }
            KeyCode::F1 => self.view.show_profiler = !self.view.show_profiler,
            KeyCode::F10 => self.open_menu(Heading::Menu, audio),
            KeyCode::F5 if self.view.range.is_some() => {
                self.range_action(RangeAction::Reset, audio)
            }
            KeyCode::F9 if self.view.range.is_some() => {
                self.range_action(RangeAction::Reload, audio)
            }
            KeyCode::F2 | KeyCode::F3 | KeyCode::F4 if self.view.range.is_some() => self
                .range_action(
                    RangeAction::Zoom(
                        [KeyCode::F2, KeyCode::F3, KeyCode::F4]
                            .iter()
                            .position(|k| *k == code)
                            .unwrap_or(1),
                    ),
                    audio,
                ),
            KeyCode::KeyG if self.view.range.is_some() => {
                self.range_action(RangeAction::ArmSpawn, audio)
            }
            KeyCode::KeyM => self.arm(Targeting::Move),
            KeyCode::KeyT => self.arm(Targeting::Attack),
            KeyCode::KeyF => self.arm(Targeting::AttackMove),
            KeyCode::KeyC => self.arm(Targeting::Assist),
            KeyCode::KeyR => self.arm(Targeting::Reclaim),
            KeyCode::KeyX => self.send(Command::Stop {
                units: self.selected_ids(),
            }),
            KeyCode::KeyU => self.send(Command::Upgrade {
                units: self.selected_ids(),
            }),
            KeyCode::KeyL if self.selection_has(cat::FACTORY) => {
                let on = self
                    .selected_units()
                    .any(|u| hud::has_flag(u, flag::REPEAT));
                self.send(Command::SetRepeat {
                    factories: self.selected_ids(),
                    repeat: !on,
                });
            }
            KeyCode::KeyP | KeyCode::Pause => self.toggle_pause(),
            KeyCode::Equal | KeyCode::NumpadAdd => self.step_speed(1),
            KeyCode::Minus | KeyCode::NumpadSubtract => self.step_speed(-1),
            KeyCode::Delete if self.ctrl => self.send(Command::SelfDestruct {
                units: self.selected_ids(),
            }),
            KeyCode::Home => {
                // Jump to the commander.
                let acu = self.view.frame.units.iter().find(|u| {
                    (u.owner_flags & 0xFF) as u8 == self.view.local
                        && u.owner_flags & KIND_WRECK == 0
                        && self
                            .blueprints
                            .unit(BlueprintId(u.blueprint as u16))
                            .has(cat::COMMANDER)
                });
                if let Some(u) = acu {
                    self.camera.focus = Vec3::from(u.pos);
                    self.camera.distance = self.camera.distance.min(900.0);
                    self.zoom_target = None;
                    self.view.selection = vec![u.unit_id];
                }
            }
            c => {
                if let Some(n) = digit(c) {
                    if self.ctrl {
                        self.view.groups[n] = self.view.selection.clone();
                    } else if !self.view.groups[n].is_empty() {
                        self.view.selection = self.view.groups[n].clone();
                        let now = Instant::now();
                        if self.last_group.is_some_and(|(key, at)| {
                            key == n && (now - at).as_secs_f32() < DOUBLE_TAP
                        }) {
                            self.focus_selection();
                        }
                        self.last_group = Some((n, now));
                    }
                }
            }
        }
    }

    /// Where the structure being placed would go, and whether it looks buildable.
    /// The sim has the final say; this only drives the preview colour.
    fn placement(&self, blueprint: BlueprintId, r: &Renderer) -> Option<(FxVec2, bool)> {
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            renderer: r,
        };
        orders::site(&field, blueprint, self.ground_under_cursor(r)?, None)
    }

    /// Sites a click or a place-drag would put down. `drag` is the pointer having
    /// moved a footprint's worth; without it the ghost stays under the cursor.
    fn placing_sites(
        &self,
        blueprint: BlueprintId,
        drag: bool,
        r: &Renderer,
    ) -> Vec<(FxVec2, bool)> {
        let Some(ground) = self.ground_under_cursor(r) else {
            return Vec::new();
        };
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            renderer: r,
        };
        let Some((to, _)) = orders::site(&field, blueprint, ground, None) else {
            return Vec::new();
        };
        match (drag, self.place_from) {
            (true, Some(from)) => orders::drag_sites(&field, blueprint, from, to),
            _ => orders::site(&field, blueprint, ground, None)
                .into_iter()
                .collect(),
        }
    }

    fn pull_sim(&mut self) -> bool {
        let Some(at) = self.sim.pull(
            &mut self.serial,
            &mut self.view.frame,
            &mut self.view.status,
        ) else {
            return false;
        };
        self.published_at = at;
        self.view.index_of.clear();
        for (i, u) in self.view.frame.units.iter().enumerate() {
            if u.owner_flags & KIND_WRECK == 0 {
                self.view.index_of.insert(u.unit_id, i);
            }
        }
        let index_of = &self.view.index_of;
        self.view.selection.retain(|id| index_of.contains_key(id));
        // A death is not a selection: nobody answers for it.
        self.answered.retain(|id| index_of.contains_key(id));
        for group in &mut self.view.groups {
            group.retain(|id| index_of.contains_key(id));
        }
        true
    }

    /// How loud and where between the speakers something at `pos` is, from
    /// where the camera looks. Loudest at the middle of the view, fading with
    /// distance from it measured against how much ground is on screen, and the
    /// whole battle quieter from orbit than from beside the tracks.
    fn hear(&self, pos: Vec3) -> (f32, f32) {
        let camera = &self.camera;
        let offset = (pos - camera.focus).truncate();
        let view = camera.distance * 0.9 + 80.0;
        let near = 1.0 / (1.0 + (offset.length() / view).powi(2) * 1.5);
        let height = (260.0 / (260.0 + camera.distance)).sqrt();
        let right = glam::Vec2::new(camera.yaw.cos(), -camera.yaw.sin());
        (near * height, (offset.dot(right) / view).clamp(-0.85, 0.85))
    }

    /// The sound ids of every blueprint, from the names in the unit files and
    /// the library's defaults. Looked up again whenever the library is replaced.
    fn sound_table(&mut self, audio: &Audio) {
        let (library, generation) = audio.library();
        if self
            .sounds
            .as_ref()
            .is_some_and(|t| t.generation == generation)
        {
            return;
        }
        let id = |named: &Option<String>, otherwise: &Option<String>| {
            named
                .as_ref()
                .or(otherwise.as_ref())
                .and_then(|n| library.id_of(n))
        };
        let d = &library.defaults;
        let units = self
            .blueprints
            .units
            .iter()
            .map(|u| UnitSoundIds {
                death: id(&u.sounds.death, &d.death),
                moving: id(&u.sounds.moving, &None),
                step: id(&u.sounds.step, &None).and_then(|sound| {
                    let model = mc_render::models::build_model_scaled(
                        &u.visual.mesh,
                        u.radius.to_f32(),
                        u.height.to_f32(),
                        u.tech,
                    )?;
                    model.legs.map(|legs| (sound, legs.stride * 0.5))
                }),
                select: u
                    .sounds
                    .select
                    .as_ref()
                    .or(d.select.get(&u.visual.icon))
                    .and_then(|n| library.id_of(n)),
                weapons: u
                    .weapons
                    .iter()
                    .map(|w| WeaponSoundIds {
                        fire: id(&w.sounds.fire, &d.fire),
                        charge: id(&w.sounds.charge, &None),
                        impact: id(&w.sounds.impact, &d.impact),
                        // A weapon that names its impact but not its ground hit sounds the same on both.
                        ground: id(
                            &w.sounds.ground,
                            if w.sounds.impact.is_some() {
                                &w.sounds.impact
                            } else {
                                &d.ground
                            },
                        ),
                    })
                    .collect(),
            })
            .collect();
        self.sounds = Some(SoundTable {
            generation,
            units,
            reclaim: ["reclaim_beam", "reclaim_start", "reclaim_end"]
                .map(|name| library.id_of(name)),
            build: ["build_beam", "build_start", "build_end"].map(|name| library.id_of(name)),
        });
    }

    /// The battle sounds of the last tick. Which sound is the unit files'
    /// business; this places them, and lets only the loudest few of each kind
    /// through, so a hundred tanks firing at once is a barrage and not a wall of noise.
    fn battle_sounds(&mut self, audio: &Audio) {
        self.sound_table(audio);
        let was_beaming = std::mem::take(&mut self.beaming);
        let was_welding = std::mem::take(&mut self.welding);
        let beaming: std::collections::HashMap<u32, [f32; 3]> = self
            .view
            .frame
            .beam_sources
            .iter()
            .copied()
            .zip(self.view.frame.beams.iter().map(|b| b.from))
            .collect();
        let welding: std::collections::HashMap<u32, [f32; 3]> =
            self.view.frame.build_sources.iter().copied().collect();
        let Some(table) = &self.sounds else { return };
        let bps = &self.blueprints;
        // (sound, gain, pan, pitch) per kind: shots, impacts, deaths, charging, beams starting and stopping.
        let mut heard: [Vec<(mc_data::SoundId, f32, f32, f32)>; 5] = Default::default();
        let switched = beaming
            .iter()
            .filter(|(id, _)| !was_beaming.contains_key(id))
            .map(|(_, at)| (table.reclaim[1], at));
        let switched = switched.chain(
            was_beaming
                .iter()
                .filter(|(id, _)| !beaming.contains_key(id))
                .map(|(_, at)| (table.reclaim[2], at)),
        );
        let switched = switched.chain(
            welding
                .iter()
                .filter(|(id, _)| !was_welding.contains_key(id))
                .map(|(_, at)| (table.build[1], at)),
        );
        let switched = switched.chain(
            was_welding
                .iter()
                .filter(|(id, _)| !welding.contains_key(id))
                .map(|(_, at)| (table.build[2], at)),
        );
        for (sound, at) in switched {
            let Some(sound) = sound else { continue };
            let (gain, pan) = self.hear(Vec3::from(*at));
            let jitter = ((at[0] * 12.9898 + at[1] * 78.233).sin() * 43_758.547)
                .fract()
                .abs();
            heard[4].push((sound, gain, pan, 0.96 + jitter * 0.08));
        }
        for event in &self.view.frame.events {
            let (kind, sound, pos, weight) = match event {
                mc_sim::SimEvent::ShotFired {
                    pos,
                    blueprint,
                    weapon,
                    ..
                } => (
                    0,
                    table.units[blueprint.index()].weapons[*weapon as usize].fire,
                    pos.to_f32(),
                    bps.unit(*blueprint).weapons[*weapon as usize]
                        .damage
                        .to_f32(),
                ),
                mc_sim::SimEvent::Impact {
                    pos,
                    on_unit,
                    blueprint,
                    weapon,
                    ..
                } => {
                    let ids = &table.units[blueprint.index()].weapons[*weapon as usize];
                    (
                        1,
                        if *on_unit { ids.impact } else { ids.ground },
                        pos.to_f32(),
                        bps.unit(*blueprint).weapons[*weapon as usize]
                            .damage
                            .to_f32(),
                    )
                }
                mc_sim::SimEvent::UnitDied { pos, blueprint, .. } => (
                    2,
                    table.units[blueprint.index()].death,
                    pos.to_f32(),
                    bps.unit(*blueprint).health.to_f32() * 0.2,
                ),
                mc_sim::SimEvent::WeaponCharging {
                    pos,
                    blueprint,
                    weapon,
                    ..
                } => (
                    3,
                    table.units[blueprint.index()].weapons[*weapon as usize].charge,
                    pos.to_f32(),
                    26.0,
                ),
                _ => continue,
            };
            let Some(sound) = sound else { continue };
            let (mut gain, pan) = self.hear(Vec3::from(pos));
            if matches!(event, mc_sim::SimEvent::UnitDied { blueprint, .. } if bps.unit(*blueprint).has(mc_data::cat::COMMANDER))
            {
                // A commander's reactor is heard wherever the camera is, from orbit too.
                gain = gain.max(0.8);
            }
            // The sound says how big the weapon is; this only leans on it a little, and no two shots are pitched quite alike.
            let size: f32 = (weight.max(1.0) / 26.0).ln();
            let jitter = ((pos[0] * 12.9898 + pos[1] * 78.233 + self.view.frame.tick as f32 * 3.7)
                .sin()
                * 43_758.547)
                .fract()
                .abs();
            heard[kind].push((
                sound,
                gain * (0.85 + size * 0.1).clamp(0.6, 1.3),
                pan,
                0.95 + jitter * 0.1,
            ));
        }
        for (kind, most) in [(0, 5), (1, 4), (2, 3), (3, 2), (4, 3)] {
            heard[kind].sort_by(|a, b| b.1.total_cmp(&a.1));
            for (i, (sound, gain, pan, pitch)) in heard[kind].iter().take(most).enumerate() {
                // Those that lose out still lend the loudest ones a little weight.
                let crowd = if i == 0 {
                    1.0 + (heard[kind].len().saturating_sub(most) as f32 * 0.04).min(0.3)
                } else {
                    1.0
                };
                audio.play_world(*sound, gain * crowd, *pan, *pitch);
            }
        }

        // Footfalls. A foot comes down each time the ground a walker has covered passes another
        // step's worth, part of the way through the tick: the sound waits for that moment.
        let mut steps: Vec<(mc_data::SoundId, f32, f32, f32, f32)> = Vec::new();
        for u in &self.view.frame.units {
            let Some((sound, pace)) = table.units.get(u.blueprint as usize).and_then(|t| t.step)
            else {
                continue;
            };
            let [ground, moved, _] = u.gait;
            if moved <= 0.0 || u.owner_flags & KIND_WRECK != 0 {
                continue;
            }
            let landing = (ground / pace).floor() * pace;
            if landing <= ground - moved {
                continue;
            }
            let (gain, pan) = self.hear(Vec3::from(u.pos));
            let after = (landing - (ground - moved)) / moved * 10.0 / self.view.speed.max(5) as f32;
            // Left and right feet are not quite alike.
            let pitch = if ((ground / pace).floor() as i64) % 2 == 0 {
                1.0
            } else {
                0.94
            };
            steps.push((sound, gain, pan, pitch, after));
        }
        steps.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (sound, gain, pan, pitch, after) in steps.into_iter().take(6) {
            audio.play_world_after(sound, gain, pan, pitch, after);
        }

        // Running gear: a few loops per sound, split across the view and
        // slightly detuned, so a column stays a column and not one machine.
        const MOVING: u32 = (mc_sim::tables::flag::MOVING as u32) << 8;
        let mut movers: Vec<(mc_data::SoundId, f32, f32)> = Vec::new();
        for u in &self.view.frame.units {
            if u.owner_flags & MOVING == 0 || u.owner_flags & KIND_WRECK != 0 {
                continue;
            }
            let Some(sound) = table.units.get(u.blueprint as usize).and_then(|t| t.moving) else {
                continue;
            };
            let (gain, pan) = self.hear(Vec3::from(u.pos));
            movers.push((sound, gain, pan));
        }
        let mut loops = crate::audio::mix_moving(&movers);
        // Reclaim beams: one loop for all of them, heard from where they bite.
        if let Some(sound) = table.reclaim[0] {
            let mut beams = (0.0, 0.0);
            for b in &self.view.frame.beams {
                let (gain, pan) = self.hear(Vec3::from(b.to));
                beams = (beams.0 + gain * gain, beams.1 + gain * gain * pan);
            }
            if beams.0 > 0.0 {
                loops.push((
                    sound,
                    (beams.0.sqrt() * 0.5).min(0.9),
                    beams.1 / beams.0.max(1e-9),
                    1.0,
                ));
            }
        }
        // Construction beams: one loop, heard from the weld.
        if let Some(sound) = table.build[0] {
            let mut beams = (0.0, 0.0);
            for (_, at) in &self.view.frame.build_sources {
                let (gain, pan) = self.hear(Vec3::from(*at));
                beams = (beams.0 + gain * gain, beams.1 + gain * gain * pan);
            }
            if beams.0 > 0.0 {
                loops.push((
                    sound,
                    (beams.0.sqrt() * 0.5).min(0.9),
                    beams.1 / beams.0.max(1e-9),
                    1.0,
                ));
            }
        }
        audio.set_loops(&loops);
        self.beaming = beaming;
        self.welding = welding;
    }

    /// The player changed the selection: the units new to it answer (all of it,
    /// when it only got smaller), in the voice of the kind there is most of.
    /// Which sound is the unit files' and the library's business.
    fn answer_selection(&mut self, audio: &Audio, now: Instant) {
        if self.view.selection == self.answered {
            return;
        }
        let before: HashSet<u32> = self.answered.iter().copied().collect();
        self.answered.clone_from(&self.view.selection);
        self.sound_table(audio);
        let Some(table) = &self.sounds else { return };
        let new = self.view.selection.iter().any(|id| !before.contains(id));
        // (sound, how many make it, the dearest of them)
        let mut voices: Vec<(mc_data::SoundId, u32, f32)> = Vec::new();
        for u in self
            .selected_units()
            .filter(|u| !new || !before.contains(&u.unit_id))
        {
            let Some(sound) = table.units.get(u.blueprint as usize).and_then(|t| t.select) else {
                continue;
            };
            let cost = self
                .blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .cost_mass
                .to_f32();
            match voices.iter_mut().find(|v| v.0 == sound) {
                Some(v) => (v.1, v.2) = (v.1 + 1, v.2.max(cost)),
                None => voices.push((sound, 1, cost)),
            }
        }
        let Some(&(sound, _, _)) = voices
            .iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)))
        else {
            return;
        };
        // A double click selects twice in a moment; one answer will do.
        if self
            .last_answer
            .is_some_and(|(last, at)| last == sound && (now - at).as_secs_f32() < 0.35)
        {
            return;
        }
        self.last_answer = Some((sound, now));
        audio.play_response(sound, 1.0);
    }

    /// What the last tick reported that the player should hear about.
    fn note_events(&mut self, audio: &Audio) {
        self.battle_sounds(audio);
        for event in &self.view.frame.events {
            match event {
                mc_sim::SimEvent::BuildRejected { player } if *player == self.view.local => {
                    audio.play(Sfx::Deny);
                    self.hud.toast("CANNOT BUILD THERE", palette::BAD);
                }
                mc_sim::SimEvent::PlayerDefeated { player } => {
                    let name = self
                        .view
                        .status
                        .players
                        .get(*player as usize)
                        .map_or("A COMMANDER", |p| p.name.as_str())
                        .to_uppercase();
                    self.hud.toast(
                        format!("{name} HAS BEEN DEFEATED"),
                        if *player == self.view.local {
                            palette::BAD
                        } else {
                            palette::ACCENT
                        },
                    );
                }
                _ => {}
            }
        }
    }

    pub fn frame(&mut self, ctx: FrameCtx) -> Result<Option<GameEvent>, String> {
        let FrameCtx {
            renderer,
            overlay,
            input,
            memory,
            audio,
            settings,
            settings_changed,
            display_changed,
            now,
            dt,
            time,
        } = ctx;
        if !self.chart_ready {
            overlay.set_image(
                hud::MINIMAP_SLOT,
                ui::preview::SIZE,
                ui::preview::SIZE,
                &ui::preview::render(&self.map),
            );
            self.chart_ready = true;
        }
        self.view.fps = self.view.fps * 0.95 + (1.0 / dt.max(1e-4)) * 0.05;
        // F1 toggles the profiler; the choice is remembered.
        if settings.show_profiler != self.view.show_profiler {
            if self.keys.contains(&KeyCode::F1) {
                settings.show_profiler = self.view.show_profiler;
                *settings_changed = true;
            } else {
                self.view.show_profiler = settings.show_profiler;
            }
        }

        // Keyboard camera.
        let mut pan = Vec2::ZERO;
        for (key, d) in [
            (KeyCode::KeyW, Vec2::Y),
            (KeyCode::ArrowUp, Vec2::Y),
            (KeyCode::KeyS, -Vec2::Y),
            (KeyCode::ArrowDown, -Vec2::Y),
            (KeyCode::KeyA, Vec2::X),
            (KeyCode::ArrowLeft, Vec2::X),
            (KeyCode::KeyD, -Vec2::X),
            (KeyCode::ArrowRight, -Vec2::X),
        ] {
            if self.keys.contains(&key) {
                pan += d;
            }
        }
        if pan != Vec2::ZERO {
            self.camera.pan(pan * dt * 900.0);
        }
        if self.keys.contains(&KeyCode::KeyQ) {
            self.camera.yaw -= dt * 1.4;
        }
        if self.keys.contains(&KeyCode::KeyE) {
            self.camera.yaw += dt * 1.4;
        }
        if self.keys.contains(&KeyCode::PageUp) {
            self.camera.tilt = (self.camera.tilt + dt * 0.8).min(0.5);
        }
        if self.keys.contains(&KeyCode::PageDown) {
            self.camera.tilt = (self.camera.tilt - dt * 0.8).max(-0.6);
        }
        self.ease_zoom(dt);

        let fresh = self.pull_sim();
        self.camera.focus.z = renderer.ground_height(self.camera.focus.truncate());
        if fresh {
            self.note_events(audio);
            // A scenario's builder has arrived: give it the order it was spawned for.
            let owed = self
                .view
                .range
                .as_mut()
                .map(|r| r.resolve_pending(&self.view.frame.units))
                .unwrap_or_default();
            if !owed.is_empty() {
                self.view.local = range::BLUE;
            }
            for command in owed {
                self.send(command);
            }
        }

        // The sim publishes the order queues of what is selected and, while shift is held
        // or an order is in hand, of the whole side.
        let everyone = self.shift || self.orders.dragging();
        if !self
            .view
            .selection
            .iter()
            .take(MAX_WATCHED)
            .eq(self.watched.units.iter())
            || self.watched.side != self.view.local
            || self.watched.everyone != everyone
        {
            self.watched = Watch {
                units: self
                    .view
                    .selection
                    .iter()
                    .take(MAX_WATCHED)
                    .copied()
                    .collect(),
                side: self.view.local,
                everyone,
            };
            self.sim.watch.lock().unwrap().clone_from(&self.watched);
        }
        self.answer_selection(audio, now);

        // The result, once: a stinger and the menu with the verdict on it.
        if let (Some(team), false) = (self.view.status.winner, self.result_shown) {
            self.result_shown = true;
            let won = self
                .view
                .status
                .players
                .get(self.view.local as usize)
                .is_some_and(|p| p.team == team);
            audio.play(if won { Sfx::Victory } else { Sfx::Defeat });
            self.menu = None;
            self.open_menu(
                if won {
                    Heading::Victory
                } else {
                    Heading::Defeat
                },
                audio,
            );
        }

        // Selection and hover marks.
        let over_ui = self.menu.is_some() || self.hud.covers(self.cursor);
        let mut marks: Vec<Mark> = self
            .view
            .selection
            .iter()
            .filter_map(|id| self.view.index_of.get(id).copied())
            .map(|i| {
                let u = &self.view.frame.units[i];
                Mark {
                    unit_index: i as u32,
                    kind: 0,
                    work: unit_bar_work(u, &self.view.status.queues),
                    _pad: 0,
                }
            })
            .collect();
        if self.view.mode == Mode::Normal && !over_ui {
            if let Some(i) = self.unit_at(self.cursor) {
                if self.view.frame.units[i].owner_flags & KIND_WRECK == 0 {
                    marks.push(Mark {
                        unit_index: i as u32,
                        kind: 1,
                        work: -1.0,
                        _pad: 0,
                    });
                }
            }
        }

        // Placement preview: one ghost under the pointer, or a line of them while dragging.
        let mut ghosts: Vec<UnitInstance> = Vec::new();
        let sites = match self.view.mode {
            Mode::Place(blueprint) => self.placing_sites(
                blueprint,
                self.left_down
                    .is_some_and(|from| from.distance(self.cursor) >= DRAG_THRESHOLD),
                renderer,
            ),
            _ => Vec::new(),
        };
        if let Mode::Place(blueprint) = self.view.mode {
            let heading = Angle::from_degrees(270).to_radians_f32();
            let radius = self.blueprints.unit(blueprint).radius.to_f32();
            for &(pos, valid) in &sites {
                let xy = pos.to_f32();
                let p = [xy[0], xy[1], renderer.ground_height(Vec2::from(xy))];
                ghosts.push(UnitInstance {
                    prev_pos: p,
                    prev_heading: heading,
                    pos: p,
                    heading,
                    blueprint: blueprint.0 as u32,
                    owner_flags: self.view.local as u32 | KIND_GHOST,
                    health: if valid { 1.0 } else { 0.0 },
                    build: 1.0,
                    turret_yaw: 0.0,
                    radius,
                    unit_id: u32::MAX,
                    _pad: 0,
                    gait: [0.0; 3],
                    upgrade: 0.0,
                    arm_pitch: [0.0; 4],
                    prev_turret_yaw: 0.0,
                    weld: [0.0; 3],
                });
            }
        }

        if let (Mode::Spawn, Some(range), false) = (self.view.mode, &self.view.range, over_ui) {
            if let Some(g) = self.ground_under_cursor(renderer) {
                let owner = if range.side == range::Side::Blue {
                    range::BLUE
                } else {
                    range::RED
                };
                let heading = if owner == range::BLUE {
                    0.0
                } else {
                    (Vec2::from(range.pad.to_f32()) - g.truncate()).to_angle()
                };
                ghosts.push(UnitInstance {
                    prev_pos: g.into(),
                    prev_heading: heading,
                    pos: g.into(),
                    heading,
                    blueprint: range.subject.0 as u32,
                    owner_flags: owner as u32 | KIND_GHOST,
                    health: 1.0,
                    build: 1.0,
                    turret_yaw: 0.0,
                    radius: self.blueprints.unit(range.subject).radius.to_f32(),
                    unit_id: u32::MAX,
                    _pad: 0,
                    gait: [0.0; 3],
                    upgrade: 0.0,
                    arm_pitch: [0.0; 4],
                    prev_turret_yaw: 0.0,
                    weld: [0.0; 3],
                });
            }
        }

        // Behind those: the structures that are planned, and the order the pointer has hold of.
        let placing = ghosts.len();
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            renderer,
        };
        self.orders.update(&field, self.cursor, over_ui);
        self.orders.ghosts(&field, &mut ghosts);
        self.pointer = self.pointer_for(renderer, over_ui, sites.iter().any(|(_, ok)| *ok));

        // Ticks arrive faster at a higher game speed; interpolate over the shorter gap.
        let tick_seconds = TICK_SECONDS * 100.0 / self.view.speed.max(1) as f32;
        let alpha = ((now - self.published_at).as_secs_f32() / tick_seconds).clamp(0.0, 1.0);

        // Range rings: what the selection reaches, and what the thing being placed would.
        let selected = self
            .view
            .selection
            .iter()
            .filter_map(|id| self.view.index_of.get(id))
            .map(|&i| &self.view.frame.units[i]);
        let (ranges, ranges_drawn) = self.rings.collect(
            ghosts[..placing]
                .iter()
                .chain(self.orders.ghost_in_hand(&ghosts[placing..]))
                .chain(selected),
            alpha,
            fresh,
        );
        self.view.reaches = Rings::key(&ranges);

        // A single-player match holds its clock under the menu and on the player's
        // pause; a network match plays on.
        let menu_holds = self
            .menu
            .as_ref()
            .is_some_and(|m| m.heading == Heading::Menu && !m.closing);
        self.sim.paused.store(
            self.view.status.owns_clock && (menu_holds || self.view.paused),
            Ordering::Relaxed,
        );

        // HUD.
        overlay.clear();
        self.view.menu_open = self.menu.is_some();
        let viewport = self.camera.viewport;
        let mut ui = Ui::new(
            overlay,
            input,
            memory,
            audio,
            viewport,
            settings.ui_scale,
            time,
            dt,
        );
        ui.interactive = self.menu.is_none();
        self.orders.draw(
            &mut ui,
            &Field {
                view: &self.view,
                blueprints: &self.blueprints,
                map: &self.map,
                camera: &self.camera,
                renderer,
            },
        );
        if let Some(from) = self.left_down {
            if self.view.mode == Mode::Normal && from.distance(self.cursor) >= DRAG_THRESHOLD {
                hud::drag_box(&mut ui, from, self.cursor);
            }
        }
        let scene = hud::Scene {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            gpu: &renderer.stats,
        };
        let actions = self.hud.draw(&mut ui, &scene, dt);
        if !over_ui {
            hud::cursor_hint(&mut ui, &self.view, &self.blueprints, sites.len());
        }

        // The in-match menu, over everything; the settings screen over that.
        let mut event = None;
        if let Some(menu) = &mut self.menu {
            menu.enter =
                (menu.enter + if menu.closing { -dt / 0.14 } else { dt / 0.28 }).clamp(0.0, 1.0);
            let in_settings = menu.settings.is_some();
            ui.interactive = !menu.closing && !in_settings;
            let eased = 1.0 - (1.0 - menu.enter).powi(3);
            let out = pause::draw(
                &mut ui,
                menu.heading,
                self.view.status.owns_clock,
                settings,
                eased,
            );
            *settings_changed |= out.settings_changed;
            match out.action {
                Some(PauseAction::Resume) => menu.closing = true,
                Some(PauseAction::Settings) => menu.settings = Some((0.0, false)),
                Some(PauseAction::Leave) => event = Some(GameEvent::Leave),
                Some(PauseAction::Quit) => event = Some(GameEvent::Quit),
                None => {}
            }
            if let (Some((enter, closing)), true) = (&mut menu.settings, in_settings) {
                *enter = (*enter + if *closing { -dt / 0.14 } else { dt / 0.24 }).clamp(0.0, 1.0);
                ui.interactive = !*closing;
                let out = options::draw(&mut ui, settings, 1.0 - (1.0 - *enter).powi(3));
                *settings_changed |= out.changed;
                *display_changed |= out.display_changed;
                *closing |= out.back;
                if *closing && *enter <= 0.0 {
                    menu.settings = None;
                }
            }
            if menu.closing && menu.enter <= 0.0 {
                self.menu = None;
            }
        }
        for action in actions {
            self.hud_action(action, audio);
        }

        let frame = FrameInput {
            camera: &self.camera,
            time,
            alpha,
            sim: fresh.then_some(&self.view.frame),
            ghosts: &ghosts,
            marks: &marks,
            ranges: &ranges,
            ranges_drawn,
            overlay,
            build_grid: matches!(self.view.mode, Mode::Place(_)),
        };
        renderer
            .render(&frame)
            .map_err(|e| format!("rendering failed: {e}"))?;
        self.view.cpu_ms = self.view.cpu_ms * 0.9 + now.elapsed().as_secs_f32() * 100.0;
        Ok(event.or(self.event.take()))
    }
}

/// Construction fill for the unit bar, or a negative if the unit is not building.
pub(crate) fn unit_bar_work(u: &UnitInstance, queues: &[UnitOrders]) -> f32 {
    const UNDER: u32 = (flag::UNDER_CONSTRUCTION as u32) << 8;
    if u.owner_flags & KIND_WRECK != 0 {
        return -1.0;
    }
    if u.owner_flags & UNDER != 0 {
        return u.build.clamp(0.0, 1.0);
    }
    if u.upgrade > 0.0 {
        return u.upgrade.clamp(0.0, 1.0);
    }
    let Some(q) = queues.iter().find(|q| q.unit_id == u.unit_id) else {
        return -1.0;
    };
    let making = q.orders.first().is_some_and(|o| {
        matches!(
            o.kind,
            OrderKind::Build | OrderKind::Produce | OrderKind::Upgrade
        )
    });
    if making {
        q.progress.clamp(0.0, 1.0)
    } else {
        -1.0
    }
}

/// `owner`'s units of this blueprint whose position is on the screen.
fn type_on_screen(units: &[UnitInstance], camera: &Camera, blueprint: u32, owner: u8) -> Vec<u32> {
    let viewport = camera.viewport;
    let mut out = Vec::new();
    for u in units {
        if u.blueprint != blueprint
            || (u.owner_flags & 0xFF) as u8 != owner
            || u.owner_flags & KIND_WRECK != 0
            || u.owner_flags & (flag::IN_FACTORY as u32) << 8 != 0
        {
            continue;
        }
        let Some(p) = camera.project(Vec3::from(u.pos)) else {
            continue;
        };
        if p.cmpge(Vec2::ZERO).all() && p.cmple(viewport).all() {
            out.push(u.unit_id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy(id: u32, blueprint: u32, owner: u8, pos: [f32; 3], flags: u32) -> UnitInstance {
        UnitInstance {
            prev_pos: pos,
            prev_heading: 0.0,
            pos,
            heading: 0.0,
            blueprint,
            owner_flags: owner as u32 | flags,
            health: 1.0,
            build: 1.0,
            turret_yaw: 0.0,
            radius: 2.0,
            unit_id: id,
            _pad: 0,
            gait: [0.0; 3],
            upgrade: 0.0,
            arm_pitch: [0.0; 4],
            prev_turret_yaw: 0.0,
            weld: [0.0; 3],
        }
    }

    fn camera_over(at: Vec2) -> Camera {
        let mut camera = Camera::new(Vec2::splat(16_384.0), Vec2::new(1280.0, 720.0));
        camera.focus = at.extend(0.0);
        camera.distance = 400.0;
        camera
    }

    #[test]
    fn unit_bar_work_follows_construction() {
        let mut site = dummy(1, 1, 0, [0.0; 3], (flag::UNDER_CONSTRUCTION as u32) << 8);
        site.build = 0.4;
        assert!((unit_bar_work(&site, &[]) - 0.4).abs() < 1e-6);

        let idle = dummy(2, 1, 0, [0.0; 3], 0);
        assert!(unit_bar_work(&idle, &[]) < 0.0);

        let factory = dummy(3, 1, 0, [0.0; 3], 0);
        let queues = [UnitOrders {
            unit_id: 3,
            orders: vec![mc_sim::mirror::QueuedOrder {
                kind: OrderKind::Produce,
                pos: [0.0; 2],
                at: FxVec2::ZERO,
                blueprint: BlueprintId(0),
            }],
            progress: 0.55,
        }];
        assert!((unit_bar_work(&factory, &queues) - 0.55).abs() < 1e-6);

        let mut refit = dummy(4, 1, 0, [0.0; 3], 0);
        refit.upgrade = 0.2;
        assert!((unit_bar_work(&refit, &[]) - 0.2).abs() < 1e-6);

        let wreck = dummy(5, 1, 0, [0.0; 3], KIND_WRECK);
        assert!(unit_bar_work(&wreck, &[]) < 0.0);
    }

    #[test]
    fn double_click_takes_the_type_on_screen() {
        let camera = camera_over(Vec2::new(1000.0, 1000.0));
        let units = [
            dummy(1, 7, 0, [1000.0, 1000.0, 0.0], 0),
            dummy(2, 7, 0, [1020.0, 1000.0, 0.0], 0),
            dummy(3, 7, 0, [8000.0, 8000.0, 0.0], 0),
            dummy(4, 8, 0, [1000.0, 1020.0, 0.0], 0),
            dummy(5, 7, 1, [1010.0, 1010.0, 0.0], 0),
            dummy(6, 7, 0, [1015.0, 1000.0, 0.0], KIND_WRECK),
            dummy(
                7,
                7,
                0,
                [1005.0, 1005.0, 0.0],
                (flag::IN_FACTORY as u32) << 8,
            ),
        ];
        let mut ids = type_on_screen(&units, &camera, 7, 0);
        ids.sort();
        assert_eq!(ids, vec![1, 2]);
        assert!(camera
            .project(Vec3::from(units[0].pos))
            .is_some_and(|p| p.cmpge(Vec2::ZERO).all() && p.cmple(camera.viewport).all()));
        assert!(camera
            .project(Vec3::from(units[2].pos))
            .is_some_and(|p| !p.cmpge(Vec2::ZERO).all() || !p.cmple(camera.viewport).all()));
    }
}
