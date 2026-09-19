//! The windowed game: input, camera, selection, orders and the frame loop.
//!
//! Everything here reads the published render mirror and status; it never
//! touches the simulation directly. Player intent leaves as `Command`s.

use crate::hud::{self, Action, Hud};
use crate::setup::{self, Options};
use crate::sim_thread::{self, SimHandle, SimSetup, SimStatus};
use glam::{Vec2, Vec3};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, BlueprintId, Blueprints};
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Mark, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK};
use mc_sim::tables::flag;
use mc_sim::{Command, Handle, RenderFrame};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};

const DRAG_THRESHOLD: f32 = 6.0;
const TICK_SECONDS: f32 = 0.1;

pub struct GameArgs {
    /// Relay address for a network match; `None` plays locally.
    pub connect: Option<String>,
    pub name: String,
    pub opts: Options,
    pub map: Arc<MapFile>,
    pub blueprints: Arc<Blueprints>,
    pub pool: Arc<Pool>,
    pub vsync: bool,
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
}

struct App {
    args: GameArgs,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    camera: Camera,
    sim: SimHandle,
    view: View,
    hud: Hud,
    overlay: Overlay,
    serial: u64,
    published_at: Instant,
    started: Instant,
    last_frame: Instant,
    cursor: Vec2,
    keys: HashSet<KeyCode>,
    shift: bool,
    ctrl: bool,
    left_down: Option<Vec2>,
    middle_down: bool,
    groups: [Vec<u32>; 10],
    fatal: Option<String>,
}

pub fn run(args: GameArgs) -> Result<(), String> {
    let config = setup::match_config(&args.opts, &args.map);
    let template = bincode::serialize(&config).map_err(|e| e.to_string())?;
    let content = mc_net::ContentId { map_id: args.map.content_id(), blueprint_hash: args.blueprints.content_hash() };

    let (session, prefetched, local): (Box<dyn mc_net::Session + Send>, Vec<mc_net::SessionEvent>, u8) = match &args.connect {
        Some(addr) => {
            let (session, events, slot) = lobby(addr, &args.name, content, template)?;
            (Box::new(session), events, slot)
        }
        None => {
            let start = mc_net::MatchStart {
                content,
                seed: config.seed,
                input_delay: 1,
                players: vec![mc_net::PlayerSetup { slot: mc_core::PlayerId(0), name: args.name.clone(), data: Vec::new() }],
                options: template,
            };
            let mut session = mc_net::LocalSession::new(start, mc_core::PlayerId(0), mc_net::session::Pacing::RealTime).map_err(|e| e.to_string())?;
            // A replay is the start message plus the command log; scripted scenes
            // inject commands outside the session, so only real matches are recorded.
            if args.opts.scene == setup::Scene::Skirmish {
                if let Err(e) = session.record_to("last-match.mcreplay") {
                    log::warn!("this match will not be recorded: {e}");
                }
            }
            (Box::new(session), Vec::new(), 0)
        }
    };

    // Test scenes script their armies locally; that only works on one machine.
    let scene = (args.connect.is_none() && args.opts.scene != setup::Scene::Skirmish).then(|| {
        let (map, blueprints, config) = (args.map.clone(), args.blueprints.clone(), config.clone());
        let opts = args.opts.clone();
        let first: sim_thread::SceneScript = Box::new(move |_| setup::opening_commands(&opts, &map, &blueprints, &config));
        let (map, opts) = (args.map.clone(), args.opts.clone());
        let second: sim_thread::SceneScript = Box::new(move |world| {
            let u = &world.state.units;
            let units: Vec<_> = u.slots.iter().map(|r| (u.owner[r], u.id(r), world.bp(r).is_mobile())).collect();
            setup::scene_orders(&opts, &map, &units)
        });
        (first, second)
    });
    let sim = sim_thread::spawn(SimSetup { map: args.map.clone(), blueprints: args.blueprints.clone(), pool: args.pool.clone(), prefetched, scene }, session);

    let size = Vec2::from(args.map.info().size_metres().to_f32());
    let mut camera = Camera::new(size, Vec2::new(1600.0, 900.0));
    if let Some(start) = args.map.start_positions().get(local as usize) {
        camera.focus = Vec2::from(start.to_f32()).extend(0.0);
        camera.distance = 700.0;
    }
    let now = Instant::now();
    let mut app = App {
        args,
        window: None,
        renderer: None,
        camera,
        sim,
        view: View { local, frame: RenderFrame::default(), status: SimStatus::default(), index_of: HashMap::new(), selection: Vec::new(), mode: Mode::Normal, fps: 0.0, cpu_ms: 0.0, show_profiler: true },
        hud: Hud::default(),
        overlay: Overlay::default(),
        serial: 0,
        published_at: now,
        started: now,
        last_frame: now,
        cursor: Vec2::ZERO,
        keys: HashSet::new(),
        shift: false,
        ctrl: false,
        left_down: None,
        middle_down: false,
        groups: Default::default(),
        fatal: None,
    };
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    match app.fatal {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("Meridian Conflict").with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => return self.fail(event_loop, format!("could not create a window: {e}")),
        };
        let size = window.inner_size();
        let (display, handle) = match (window.display_handle(), window.window_handle()) {
            (Ok(d), Ok(h)) => (d.as_raw(), h.as_raw()),
            _ => return self.fail(event_loop, "the window has no native handle".into()),
        };
        let scene = SceneDesc { map: self.args.map.clone(), blueprints: self.args.blueprints.clone(), pool: self.args.pool.clone(), team_colors: setup::TEAM_COLORS };
        let target = Target::Window { display, window: handle, width: size.width, height: size.height, vsync: self.args.vsync };
        match Renderer::new(target, scene) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => return self.fail(event_loop, format!("could not start the renderer: {e}")),
        }
        self.camera.viewport = Vec2::new(size.width as f32, size.height as f32);
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.camera.viewport = Vec2::new(size.width.max(1) as f32, size.height.max(1) as f32);
                if let Some(r) = &mut self.renderer {
                    if let Err(e) = r.resize(size.width, size.height) {
                        self.fail(event_loop, e.to_string());
                    }
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
                self.ctrl = m.state().control_key();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = Vec2::new(position.x as f32, position.y as f32);
                if self.middle_down {
                    self.camera.pan(p - self.cursor);
                }
                self.cursor = p;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                let anchor = self.ground_under_cursor();
                self.camera.zoom(0.86f32.powf(lines), anchor);
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse_button(button, state == ElementState::Pressed),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        if self.keys.insert(code) {
                            self.key_pressed(code, event_loop);
                        }
                    } else {
                        self.keys.remove(&code);
                    }
                }
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, message: String) {
        self.fatal = Some(message);
        event_loop.exit();
    }

    fn send(&self, command: Command) {
        // The sim thread is gone only after a fatal error, which the HUD already shows.
        let _ = self.sim.commands.send(command);
    }

    fn ground_under_cursor(&self) -> Option<Vec3> {
        let (origin, dir) = self.camera.ray(self.cursor);
        self.renderer.as_ref()?.pick_ground(origin, dir)
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
        self.selected_units().any(|u| self.args.blueprints.unit(BlueprintId(u.blueprint as u16)).has(categories))
    }

    fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
        match (button, pressed) {
            (MouseButton::Middle, p) => self.middle_down = p,
            (MouseButton::Left, true) => {
                if let Some(action) = self.hud.hit(self.cursor) {
                    self.hud_action(action);
                } else {
                    self.left_down = Some(self.cursor);
                }
            }
            (MouseButton::Left, false) => {
                if let Some(from) = self.left_down.take() {
                    self.left_released(from);
                }
            }
            (MouseButton::Right, true) => {
                if self.view.mode != Mode::Normal {
                    self.view.mode = Mode::Normal;
                } else if !self.hud.covers(self.cursor) {
                    self.context_order();
                }
            }
            _ => {}
        }
    }

    fn left_released(&mut self, from: Vec2) {
        match self.view.mode {
            Mode::Place(blueprint) => {
                if let Some((pos, valid)) = self.placement(blueprint) {
                    if valid {
                        self.send(Command::Build { units: self.selected_ids(), blueprint, pos, heading: Angle::from_degrees(270), queue: self.shift });
                    }
                }
                if !self.shift {
                    self.view.mode = Mode::Normal;
                }
            }
            Mode::AttackMove => {
                if let Some(g) = self.ground_under_cursor() {
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
                            if self.args.blueprints.unit(BlueprintId(u.blueprint as u16)).is_mobile() { picked.push(u.unit_id) } else { structures.push(u.unit_id) }
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
    fn context_order(&mut self) {
        if self.view.selection.is_empty() {
            return;
        }
        let units = self.selected_ids();
        let queue = self.shift;
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
        if let Some(g) = self.ground_under_cursor() {
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
                let bp = self.args.blueprints.unit(blueprint);
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

    fn key_pressed(&mut self, code: KeyCode, event_loop: &ActiveEventLoop) {
        let digit = |c: KeyCode| {
            [KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9]
                .iter()
                .position(|k| *k == c)
        };
        match code {
            KeyCode::Escape => {
                if self.view.mode != Mode::Normal {
                    self.view.mode = Mode::Normal;
                } else {
                    self.view.selection.clear();
                }
            }
            KeyCode::F1 => self.view.show_profiler = !self.view.show_profiler,
            KeyCode::F10 => event_loop.exit(),
            KeyCode::KeyF if !self.view.selection.is_empty() => self.view.mode = Mode::AttackMove,
            KeyCode::KeyX => self.send(Command::Stop { units: self.selected_ids() }),
            KeyCode::KeyU => self.send(Command::Upgrade { units: self.selected_ids() }),
            KeyCode::Delete if self.ctrl => self.send(Command::SelfDestruct { units: self.selected_ids() }),
            KeyCode::Home => {
                // Jump to the commander.
                let acu = self.view.frame.units.iter().find(|u| (u.owner_flags & 0xFF) as u8 == self.view.local && u.owner_flags & KIND_WRECK == 0 && self.args.blueprints.unit(BlueprintId(u.blueprint as u16)).has(cat::COMMANDER));
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
    fn placement(&self, blueprint: BlueprintId) -> Option<(FxVec2, bool)> {
        let bp = self.args.blueprints.unit(blueprint);
        let g = self.ground_under_cursor()?;
        let mut pos = FxVec2::new(Fx::from_f32(g.x), Fx::from_f32(g.y));
        let mut valid = true;
        if bp.needs_deposit {
            let nearest = self.args.map.mass_deposits().iter().min_by_key(|d| d.distance_sq(pos))?;
            valid = nearest.distance(pos) < Fx::from_int(60);
            if valid {
                pos = *nearest;
            }
        }
        let pos = mc_sim::world::snap_to_build_grid(bp, pos);
        let half = Vec2::new(bp.footprint.0 as f32, bp.footprint.1 as f32) * 8.0;
        let p = Vec2::from(pos.to_f32());
        for u in &self.view.frame.units {
            let other = self.args.blueprints.unit(BlueprintId(u.blueprint as u16));
            if u.owner_flags & KIND_WRECK != 0 || !other.is_structure() {
                continue;
            }
            let other_half = Vec2::new(other.footprint.0 as f32, other.footprint.1 as f32) * 8.0;
            let d = (Vec2::new(u.pos[0], u.pos[1]) - p).abs();
            if d.x < half.x + other_half.x && d.y < half.y + other_half.y {
                valid = false;
            }
        }
        let water = self.args.map.info().water_level.to_f32();
        if g.z < water {
            valid = false;
        }
        Some((pos, valid))
    }

    fn pull_sim(&mut self) -> bool {
        let mut p = self.sim.shared.lock().unwrap();
        if p.serial == self.serial {
            return false;
        }
        self.serial = p.serial;
        self.published_at = p.published_at;
        self.view.frame.clone_from(&p.frame);
        // Events are consumed exactly once.
        p.frame.events.clear();
        self.view.status = p.status.clone();
        drop(p);
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

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.view.fps = self.view.fps * 0.95 + (1.0 / dt.max(1e-4)) * 0.05;

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
        if let Some(r) = &self.renderer {
            self.camera.focus.z = r.ground_height(self.camera.focus.truncate());
        }

        // Selection and hover marks.
        let mut marks: Vec<Mark> = self.view.selection.iter().filter_map(|id| self.view.index_of.get(id)).map(|&i| Mark { unit_index: i as u32, kind: 0 }).collect();
        if self.view.mode == Mode::Normal && !self.hud.covers(self.cursor) {
            if let Some(i) = self.unit_at(self.cursor) {
                if self.view.frame.units[i].owner_flags & KIND_WRECK == 0 {
                    marks.push(Mark { unit_index: i as u32, kind: 1 });
                }
            }
        }

        // Placement preview.
        let mut ghosts: Vec<UnitInstance> = Vec::new();
        if let Mode::Place(blueprint) = self.view.mode {
            if let (Some((pos, valid)), Some(r)) = (self.placement(blueprint), &self.renderer) {
                let xy = pos.to_f32();
                let p = [xy[0], xy[1], r.ground_height(Vec2::from(xy))];
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
                    radius: self.args.blueprints.unit(blueprint).radius.to_f32(),
                    unit_id: u32::MAX,
                    _pad: 0,
                });
            }
        }

        // HUD.
        self.overlay.clear();
        let gpu = self.renderer.as_ref().map(|r| r.stats.clone()).unwrap_or_default();
        self.hud.draw(&mut self.overlay, &self.view, &self.args.blueprints, self.camera.viewport, &gpu);
        if let Some(from) = self.left_down {
            if self.view.mode == Mode::Normal && from.distance(self.cursor) >= DRAG_THRESHOLD {
                let (min, max) = (from.min(self.cursor), from.max(self.cursor));
                self.overlay.rect(min.x, min.y, max.x - min.x, max.y - min.y, [0.3, 1.0, 0.4, 0.12]);
                self.overlay.frame(min.x, min.y, max.x - min.x, max.y - min.y, 1.0, [0.4, 1.0, 0.5, 0.9]);
            }
        }
        hud::cursor_hint(&mut self.overlay, &self.view, &self.args.blueprints, self.cursor);

        let alpha = ((now - self.published_at).as_secs_f32() / TICK_SECONDS).clamp(0.0, 1.0);
        let input = FrameInput {
            camera: &self.camera,
            time: (now - self.started).as_secs_f32(),
            alpha,
            sim: fresh.then_some(&self.view.frame),
            ghosts: &ghosts,
            marks: &marks,
            overlay: &self.overlay,
            build_grid: matches!(self.view.mode, Mode::Place(_)),
        };
        if let Some(r) = &mut self.renderer {
            if let Err(e) = r.render(&input) {
                return self.fail(event_loop, format!("rendering failed: {e}"));
            }
        }
        self.view.cpu_ms = self.view.cpu_ms * 0.9 + now.elapsed().as_secs_f32() * 100.0;
    }
}

/// Joins a relay and waits in its lobby until the match starts. Returns the
/// session, every event polled from `Started` on, and our slot.
fn lobby(addr: &str, name: &str, content: mc_net::ContentId, template: Vec<u8>) -> Result<(mc_net::NetSession, Vec<mc_net::SessionEvent>, u8), String> {
    use mc_net::{SessionEvent, Session};
    let config = mc_net::ClientConfig::new(name.to_owned(), mc_net::Role::Player, content);
    let mut session = mc_net::NetSession::connect(addr, config).map_err(|e| format!("could not reach the relay at {addr}: {e}"))?;
    let mut slot = None;
    log::info!("connected to {addr}; waiting for the other players");
    loop {
        let mut events = session.poll().into_iter();
        while let Some(event) = events.next() {
            match event {
                SessionEvent::Joined(welcome) => {
                    slot = welcome.slot.map(|s| s.0);
                    // The lowest slot hosts: its options define the match for everyone.
                    if slot == Some(0) {
                        session.set_match_options(template.clone()).map_err(|e| e.to_string())?;
                    }
                    session.set_ready(true);
                    if welcome.in_progress {
                        log::info!("match already running; joining from a snapshot");
                    }
                }
                SessionEvent::Lobby(state) => log::info!("lobby: {} of the players are in", state.players.len()),
                SessionEvent::Started(start) => {
                    let mut rest = vec![SessionEvent::Started(start)];
                    rest.extend(events);
                    return Ok((session, rest, slot.unwrap_or(0)));
                }
                SessionEvent::Ended(reason) => return Err(format!("the relay closed the session: {reason:?}")),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
