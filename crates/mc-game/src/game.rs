//! A match in progress: camera, selection, orders, the HUD and the in-match menu.
//!
//! Everything here reads the published render mirror and status; it never
//! touches the simulation directly. Player intent leaves as `Command`s.

use crate::audio::{Audio, Sfx};
use crate::hud::{self, Action, Hud};
use crate::settings::Settings;
use crate::sim_thread::{self, SceneScript, SimHandle, SimSetup, SimStatus};
use crate::ui::pause::{self, Heading, PauseAction};
use crate::ui::{self, Ui};
use glam::{Vec2, Vec3};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, BlueprintId, Blueprints};
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Mark, Overlay, Renderer};
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK};
use mc_sim::tables::flag;
use mc_sim::{Command, Handle, RenderFrame};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const DRAG_THRESHOLD: f32 = 6.0;
const TICK_SECONDS: f32 = 0.1;

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
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Normal,
    AttackMove,
    Place(BlueprintId),
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameEvent {
    /// Back to the front end.
    Leave,
    Quit,
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
    pub now: Instant,
    pub dt: f32,
    /// Seconds since the application started.
    pub time: f32,
}

struct Menu {
    heading: Heading,
    enter: f32,
    closing: bool,
}

pub struct Game {
    map: Arc<MapFile>,
    blueprints: Arc<Blueprints>,
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
    middle_down: bool,
    groups: [Vec<u32>; 10],
    menu: Option<Menu>,
    result_shown: bool,
}

impl Game {
    pub fn new(start: GameStart, blueprints: Arc<Blueprints>, pool: Arc<Pool>, viewport: Vec2, show_profiler: bool) -> Game {
        let sim = sim_thread::spawn(SimSetup { map: start.map.clone(), blueprints: blueprints.clone(), pool, prefetched: start.prefetched, scene: start.scene }, start.session);
        let size = Vec2::from(start.map.info().size_metres().to_f32());
        let mut camera = Camera::new(size, viewport);
        if let Some(at) = start.map.start_positions().get(start.start_index) {
            camera.focus = Vec2::from(at.to_f32()).extend(0.0);
            camera.distance = 700.0;
        }
        Game {
            map: start.map,
            blueprints,
            camera,
            sim,
            view: View { local: start.local, frame: RenderFrame::default(), status: SimStatus::default(), index_of: HashMap::new(), selection: Vec::new(), mode: Mode::Normal, fps: 0.0, cpu_ms: 0.0, show_profiler, menu_open: false },
            hud: Hud::default(),
            serial: 0,
            published_at: Instant::now(),
            cursor: Vec2::ZERO,
            keys: HashSet::new(),
            shift: false,
            ctrl: false,
            left_down: None,
            middle_down: false,
            groups: Default::default(),
            menu: None,
            result_shown: false,
        }
    }

    pub fn tick(&self) -> u32 {
        self.view.status.tick
    }

    pub fn resized(&mut self, viewport: Vec2) {
        self.camera.viewport = viewport;
    }

    fn open_menu(&mut self, heading: Heading, audio: &Audio) {
        if self.menu.is_none() {
            audio.play(Sfx::Whoosh);
            self.menu = Some(Menu { heading, enter: 0.0, closing: false });
            // Held keys and drags must not carry on underneath the menu.
            self.keys.clear();
            self.left_down = None;
            self.middle_down = false;
            // A single-player match stops its clock; a network match plays on.
            self.sim.paused.store(heading == Heading::Menu, Ordering::Relaxed);
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent, r: &Renderer, audio: &Audio) {
        // Track modifiers and the pointer always; act on them only when the menu is down.
        match event {
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
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
                let anchor = self.ground_under_cursor(r);
                self.camera.zoom(0.86f32.powf(lines), anchor);
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_button(*button, *state == ElementState::Pressed, r, audio),
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

    fn send(&self, command: Command) {
        // The sim thread is gone only after a fatal error, which the HUD already shows.
        let _ = self.sim.commands.send(command);
    }

    fn ground_under_cursor(&self, r: &Renderer) -> Option<Vec3> {
        let (origin, dir) = self.camera.ray(self.cursor);
        r.pick_ground(origin, dir)
    }

    fn team_of(&self, owner: u8) -> u8 {
        self.view.status.players.get(owner as usize).map_or(owner, |p| p.team)
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
            let Some(p) = self.camera.project(centre) else { continue };
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
        self.view.selection.iter().filter_map(|id| self.view.index_of.get(id)).map(|&i| &self.view.frame.units[i])
    }

    fn selection_has(&self, categories: u32) -> bool {
        self.selected_units().any(|u| self.blueprints.unit(BlueprintId(u.blueprint as u16)).has(categories))
    }

    fn mouse_button(&mut self, button: MouseButton, pressed: bool, r: &Renderer, audio: &Audio) {
        match (button, pressed) {
            (MouseButton::Middle, p) => self.middle_down = p,
            (MouseButton::Left, true) => {
                if let Some(action) = self.hud.hit(self.cursor) {
                    audio.play_at(Sfx::Select, 0.6);
                    self.hud_action(action);
                } else {
                    self.left_down = Some(self.cursor);
                }
            }
            (MouseButton::Left, false) => {
                if let Some(from) = self.left_down.take() {
                    self.left_released(from, r, audio);
                }
            }
            (MouseButton::Right, true) => {
                if self.view.mode != Mode::Normal {
                    self.view.mode = Mode::Normal;
                } else if !self.hud.covers(self.cursor) {
                    self.context_order(r, audio);
                }
            }
            _ => {}
        }
    }

    fn left_released(&mut self, from: Vec2, r: &Renderer, audio: &Audio) {
        match self.view.mode {
            Mode::Place(blueprint) => {
                if let Some((pos, valid)) = self.placement(blueprint, r) {
                    if valid {
                        audio.play(Sfx::Order);
                        self.send(Command::Build { units: self.selected_ids(), blueprint, pos, heading: Angle::from_degrees(270), queue: self.shift });
                    } else {
                        audio.play(Sfx::Deny);
                    }
                }
                if !self.shift {
                    self.view.mode = Mode::Normal;
                }
            }
            Mode::AttackMove => {
                if let Some(g) = self.ground_under_cursor(r) {
                    audio.play(Sfx::Order);
                    self.send(Command::AttackMove { units: self.selected_ids(), target: FxVec2::new(Fx::from_f32(g.x), Fx::from_f32(g.y)), queue: self.shift });
                }
                if !self.shift {
                    self.view.mode = Mode::Normal;
                }
            }
            Mode::Normal => {
                let mut picked: Vec<u32> = Vec::new();
                if from.distance(self.cursor) < DRAG_THRESHOLD {
                    if let Some(i) = self.unit_at(self.cursor) {
                        let u = &self.view.frame.units[i];
                        if u.owner_flags & KIND_WRECK == 0 && (u.owner_flags & 0xFF) as u8 == self.view.local {
                            picked.push(u.unit_id);
                        }
                    }
                } else {
                    // Box select: own units inside the rectangle; mobile units win over structures.
                    let (min, max) = (from.min(self.cursor), from.max(self.cursor));
                    let mut structures = Vec::new();
                    for u in &self.view.frame.units {
                        if u.owner_flags & KIND_WRECK != 0 || (u.owner_flags & 0xFF) as u8 != self.view.local || u.owner_flags & (flag::IN_FACTORY as u32) << 8 != 0 {
                            continue;
                        }
                        let Some(p) = self.camera.project(Vec3::from(u.pos)) else { continue };
                        if p.cmpge(min).all() && p.cmple(max).all() {
                            if self.blueprints.unit(BlueprintId(u.blueprint as u16)).is_mobile() { picked.push(u.unit_id) } else { structures.push(u.unit_id) }
                        }
                    }
                    if picked.is_empty() {
                        picked = structures;
                    }
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

    /// Right click: attack, assist, reclaim or move, depending on what is under the cursor.
    fn context_order(&mut self, r: &Renderer, audio: &Audio) {
        if self.view.selection.is_empty() {
            return;
        }
        let units = self.selected_ids();
        let queue = self.shift;
        audio.play(Sfx::Order);
        let builders = self.selection_has(cat::ENGINEER) || self.selection_has(cat::COMMANDER);
        if let Some(i) = self.unit_at(self.cursor) {
            let u = self.view.frame.units[i];
            let owner = (u.owner_flags & 0xFF) as u8;
            if u.owner_flags & KIND_WRECK != 0 {
                if builders {
                    return self.send(Command::ReclaimWreck { units, wreck: Handle(u.unit_id), queue });
                }
            } else if self.is_enemy(owner) {
                return self.send(Command::Attack { units, target: Handle(u.unit_id), queue });
            } else if builders && !self.view.selection.contains(&u.unit_id) {
                return self.send(Command::Assist { units, target: Handle(u.unit_id), queue });
            }
        }
        if let Some(g) = self.ground_under_cursor(r) {
            let target = FxVec2::new(Fx::from_f32(g.x), Fx::from_f32(g.y));
            if self.selection_has(cat::FACTORY) && !self.selection_has(cat::MOBILE) {
                self.send(Command::SetRally { factories: units, pos: target });
            } else {
                self.send(Command::Move { units, target, queue });
            }
        }
    }

    fn hud_action(&mut self, action: Action) {
        match action {
            Action::Build(blueprint) => {
                let bp = self.blueprints.unit(blueprint);
                if bp.is_structure() {
                    self.view.mode = Mode::Place(blueprint);
                } else {
                    self.send(Command::Produce { factories: self.selected_ids(), blueprint, count: if self.shift { 5 } else { 1 } });
                }
            }
            Action::Upgrade => self.send(Command::Upgrade { units: self.selected_ids() }),
            Action::Stop => self.send(Command::Stop { units: self.selected_ids() }),
            Action::Repeat(on) => self.send(Command::SetRepeat { factories: self.selected_ids(), repeat: on }),
        }
    }

    fn key_pressed(&mut self, code: KeyCode, audio: &Audio) {
        let digit = |c: KeyCode| {
            [KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9]
                .iter()
                .position(|k| *k == c)
        };
        match code {
            KeyCode::Escape => {
                if self.view.mode != Mode::Normal {
                    self.view.mode = Mode::Normal;
                } else if !self.view.selection.is_empty() {
                    self.view.selection.clear();
                } else {
                    self.open_menu(Heading::Menu, audio);
                }
            }
            KeyCode::F1 => self.view.show_profiler = !self.view.show_profiler,
            KeyCode::F10 => self.open_menu(Heading::Menu, audio),
            KeyCode::KeyF if !self.view.selection.is_empty() => self.view.mode = Mode::AttackMove,
            KeyCode::KeyX => self.send(Command::Stop { units: self.selected_ids() }),
            KeyCode::KeyU => self.send(Command::Upgrade { units: self.selected_ids() }),
            KeyCode::Delete if self.ctrl => self.send(Command::SelfDestruct { units: self.selected_ids() }),
            KeyCode::Home => {
                // Jump to the commander.
                let acu = self.view.frame.units.iter().find(|u| (u.owner_flags & 0xFF) as u8 == self.view.local && u.owner_flags & KIND_WRECK == 0 && self.blueprints.unit(BlueprintId(u.blueprint as u16)).has(cat::COMMANDER));
                if let Some(u) = acu {
                    self.camera.focus = Vec3::from(u.pos);
                    self.camera.distance = self.camera.distance.min(900.0);
                    self.view.selection = vec![u.unit_id];
                }
            }
            c => {
                if let Some(n) = digit(c) {
                    if self.ctrl {
                        self.groups[n] = self.view.selection.clone();
                    } else if !self.groups[n].is_empty() {
                        self.view.selection = self.groups[n].clone();
                    }
                }
            }
        }
    }

    /// Where the structure being placed would go, and whether it looks buildable.
    /// The sim has the final say; this only drives the preview colour.
    fn placement(&self, blueprint: BlueprintId, r: &Renderer) -> Option<(FxVec2, bool)> {
        let bp = self.blueprints.unit(blueprint);
        let g = self.ground_under_cursor(r)?;
        let mut pos = FxVec2::new(Fx::from_f32(g.x), Fx::from_f32(g.y));
        let mut valid = true;
        if bp.needs_deposit {
            let nearest = self.map.mass_deposits().iter().min_by_key(|d| d.distance_sq(pos))?;
            valid = nearest.distance(pos) < Fx::from_int(60);
            if valid {
                pos = *nearest;
            }
        }
        let pos = mc_sim::world::snap_to_build_grid(bp, pos);
        let half = Vec2::new(bp.footprint.0 as f32, bp.footprint.1 as f32) * 8.0;
        let p = Vec2::from(pos.to_f32());
        for u in &self.view.frame.units {
            let other = self.blueprints.unit(BlueprintId(u.blueprint as u16));
            if u.owner_flags & KIND_WRECK != 0 || !other.is_structure() {
                continue;
            }
            let other_half = Vec2::new(other.footprint.0 as f32, other.footprint.1 as f32) * 8.0;
            let d = (Vec2::new(u.pos[0], u.pos[1]) - p).abs();
            if d.x < half.x + other_half.x && d.y < half.y + other_half.y {
                valid = false;
            }
        }
        let water = self.map.info().water_level.to_f32();
        if g.z < water {
            valid = false;
        }
        Some((pos, valid))
    }

    fn pull_sim(&mut self) -> bool {
        let Some(at) = self.sim.pull(&mut self.serial, &mut self.view.frame, &mut self.view.status) else { return false };
        self.published_at = at;
        self.view.index_of.clear();
        for (i, u) in self.view.frame.units.iter().enumerate() {
            if u.owner_flags & KIND_WRECK == 0 {
                self.view.index_of.insert(u.unit_id, i);
            }
        }
        let index_of = &self.view.index_of;
        self.view.selection.retain(|id| index_of.contains_key(id));
        true
    }

    pub fn frame(&mut self, ctx: FrameCtx) -> Result<Option<GameEvent>, String> {
        let FrameCtx { renderer, overlay, input, memory, audio, settings, settings_changed, now, dt, time } = ctx;
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
        for (key, d) in [(KeyCode::KeyW, Vec2::Y), (KeyCode::ArrowUp, Vec2::Y), (KeyCode::KeyS, -Vec2::Y), (KeyCode::ArrowDown, -Vec2::Y), (KeyCode::KeyA, Vec2::X), (KeyCode::ArrowLeft, Vec2::X), (KeyCode::KeyD, -Vec2::X), (KeyCode::ArrowRight, -Vec2::X)] {
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

        let fresh = self.pull_sim();
        self.camera.focus.z = renderer.ground_height(self.camera.focus.truncate());

        // The result, once: a stinger and the menu with the verdict on it.
        if let (Some(team), false) = (self.view.status.winner, self.result_shown) {
            self.result_shown = true;
            let won = self.view.status.players.get(self.view.local as usize).is_some_and(|p| p.team == team);
            audio.play(if won { Sfx::Victory } else { Sfx::Defeat });
            self.menu = None;
            self.open_menu(if won { Heading::Victory } else { Heading::Defeat }, audio);
        }

        // Selection and hover marks.
        let over_ui = self.menu.is_some() || self.hud.covers(self.cursor);
        let mut marks: Vec<Mark> = self.view.selection.iter().filter_map(|id| self.view.index_of.get(id)).map(|&i| Mark { unit_index: i as u32, kind: 0 }).collect();
        if self.view.mode == Mode::Normal && !over_ui {
            if let Some(i) = self.unit_at(self.cursor) {
                if self.view.frame.units[i].owner_flags & KIND_WRECK == 0 {
                    marks.push(Mark { unit_index: i as u32, kind: 1 });
                }
            }
        }

        // Placement preview.
        let mut ghosts: Vec<UnitInstance> = Vec::new();
        if let Mode::Place(blueprint) = self.view.mode {
            if let Some((pos, valid)) = self.placement(blueprint, renderer) {
                let xy = pos.to_f32();
                let p = [xy[0], xy[1], renderer.ground_height(Vec2::from(xy))];
                let heading = Angle::from_degrees(270).to_radians_f32();
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
                    radius: self.blueprints.unit(blueprint).radius.to_f32(),
                    unit_id: u32::MAX,
                    _pad: 0,
                });
            }
        }

        // HUD.
        overlay.clear();
        self.view.menu_open = self.menu.is_some();
        self.hud.draw(overlay, &self.view, &self.blueprints, self.camera.viewport, &renderer.stats);
        if let Some(from) = self.left_down {
            if self.view.mode == Mode::Normal && from.distance(self.cursor) >= DRAG_THRESHOLD {
                let (min, max) = (from.min(self.cursor), from.max(self.cursor));
                overlay.rect(min.x, min.y, max.x - min.x, max.y - min.y, [0.3, 1.0, 0.4, 0.12]);
                overlay.frame(min.x, min.y, max.x - min.x, max.y - min.y, 1.0, [0.4, 1.0, 0.5, 0.9]);
            }
        }
        hud::cursor_hint(overlay, &self.view, &self.blueprints, self.cursor);

        // The in-match menu, over everything.
        let mut event = None;
        if let Some(menu) = &mut self.menu {
            menu.enter = (menu.enter + if menu.closing { -dt / 0.14 } else { dt / 0.28 }).clamp(0.0, 1.0);
            let mut ui = Ui::new(overlay, input, memory, audio, self.camera.viewport, settings.ui_scale, time, dt);
            ui.interactive = !menu.closing;
            let eased = 1.0 - (1.0 - menu.enter).powi(3);
            let out = pause::draw(&mut ui, menu.heading, settings, eased);
            *settings_changed |= out.settings_changed;
            match out.action {
                Some(PauseAction::Resume) => menu.closing = true,
                Some(PauseAction::Leave) => event = Some(GameEvent::Leave),
                Some(PauseAction::Quit) => event = Some(GameEvent::Quit),
                None => {}
            }
            if menu.closing && menu.enter <= 0.0 {
                self.menu = None;
                self.sim.paused.store(false, Ordering::Relaxed);
            }
        }

        let alpha = ((now - self.published_at).as_secs_f32() / TICK_SECONDS).clamp(0.0, 1.0);
        let frame = FrameInput { camera: &self.camera, time, alpha, sim: fresh.then_some(&self.view.frame), ghosts: &ghosts, marks: &marks, overlay, build_grid: matches!(self.view.mode, Mode::Place(_)) };
        renderer.render(&frame).map_err(|e| format!("rendering failed: {e}"))?;
        self.view.cpu_ms = self.view.cpu_ms * 0.9 + now.elapsed().as_secs_f32() * 100.0;
        Ok(event)
    }
}
