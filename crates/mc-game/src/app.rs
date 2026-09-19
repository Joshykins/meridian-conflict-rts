//! The windowed application: owns the window, the renderer, sound and settings,
//! and moves between three stages: the front end over its live backdrop, a
//! loading card while a map and renderer are swapped, and the match.
//!
//! A renderer is built for one map, so every stage change builds a fresh one.
//! That also guarantees nothing of the last battle (scorch marks, terrain
//! edits, fog) can leak into the next.

use crate::audio::{Audio, Sfx};
use crate::game::{FrameCtx, Game, GameEvent, GameStart};
use crate::settings::Settings;
use crate::setup::{self, Options, Scene};
use crate::sim_thread::{self, SimHandle, SimSetup, SimStatus};
use crate::ui::backdrop::Director;
use crate::ui::front::{self, Front, FrontEvent};
use crate::ui::menu::{Telemetry, PREVIEW_SLOT};
use crate::ui::skirmish::MatchRequest;
use crate::ui::{self, Key, Ui};
use glam::Vec2;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::RenderFrame;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Fullscreen, Window, WindowId};

const TICK_SECONDS: f32 = 0.1;

pub struct AppArgs {
    pub blueprints: Arc<Blueprints>,
    pub pool: Arc<Pool>,
    /// `--no-vsync` on the command line beats the saved setting for this run.
    pub force_no_vsync: bool,
    /// A match to go straight into (command-line play, test scenes, relay
    /// matches); `None` opens the front end.
    pub direct: Option<GameStart>,
    /// Drive the whole loop unattended: front end, a default skirmish, back to
    /// the front end, exit. Fails loudly if any stage change does.
    pub smoke: bool,
}

/// What the loading card is covering for.
enum Pending {
    Front,
    Match(Box<GameStart>),
}

enum Stage {
    Front(Box<FrontStage>),
    Loading { pending: Option<Pending>, title: String, detail: String, frames: u32 },
    Match(Box<Game>),
}

/// The front end and the battle staged behind it.
struct FrontStage {
    front: Front,
    map: Arc<MapFile>,
    sim: SimHandle,
    serial: u64,
    published_at: Instant,
    frame: RenderFrame,
    status: SimStatus,
    camera: Camera,
    /// Seconds with nothing in the air: the backdrop battle has burnt out.
    quiet: f32,
}

struct App {
    args: AppArgs,
    settings: Settings,
    audio: Audio,
    /// Declared before the window so that it is dropped first: its surface belongs to the window.
    renderer: Option<Renderer>,
    window: Option<Arc<Window>>,
    overlay: Overlay,
    input: ui::Input,
    memory: ui::Memory,
    stage: Stage,
    /// Fades every new stage up from black.
    reveal: f32,
    /// What the live renderer's swapchain was built with.
    applied_vsync: bool,
    started: Instant,
    last_frame: Instant,
    fatal: Option<String>,
    /// Smoke test: stage changes made so far, and when the current stage began.
    smoke_step: u32,
    stage_since: Instant,
}

pub fn run(mut args: AppArgs) -> Result<(), String> {
    let settings = Settings::load();
    // The smoke test opens the device like any run, but is not there to be heard.
    let audio = Audio::new(if args.smoke { crate::audio::Volumes { master: 0.0, interface: 0.0, ambience: 0.0 } } else { settings.volumes() });
    let first = match args.direct.take() {
        Some(start) => Pending::Match(Box::new(start)),
        None => Pending::Front,
    };
    let now = Instant::now();
    let mut app = App {
        args,
        settings,
        audio,
        renderer: None,
        window: None,
        overlay: Overlay::default(),
        input: ui::Input::default(),
        memory: ui::Memory::default(),
        stage: Stage::Loading { pending: Some(first), title: String::new(), detail: String::new(), frames: 0 },
        reveal: 0.0,
        applied_vsync: true,
        started: now,
        last_frame: now,
        fatal: None,
        smoke_step: 0,
        stage_since: now,
    };
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.settings.save();
    match app.fatal {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// A local match from a front-end request: one human, the rest as configured.
pub fn local_start(request: MatchRequest, blueprints: &Blueprints, record: bool) -> Result<GameStart, String> {
    let MatchRequest { map, config, colors } = request;
    let content = mc_net::ContentId { map_id: map.content_id(), blueprint_hash: blueprints.content_hash() };
    let local = config.players.iter().position(|p| p.controller == mc_sim::tables::Controller::Human).unwrap_or(0);
    let start = mc_net::MatchStart {
        content,
        seed: config.seed,
        input_delay: 1,
        players: vec![mc_net::PlayerSetup { slot: mc_core::PlayerId(local as u8), name: config.players[local].name.clone(), data: Vec::new() }],
        options: bincode::serialize(&config).map_err(|e| e.to_string())?,
    };
    let mut session = mc_net::LocalSession::new(start, mc_core::PlayerId(local as u8), mc_net::session::Pacing::RealTime).map_err(|e| e.to_string())?;
    if record {
        if let Err(e) = session.record_to("last-match.mcreplay") {
            log::warn!("this match will not be recorded: {e}");
        }
    }
    let start_index = config.players[local].start as usize;
    Ok(GameStart { map, colors, session: Box::new(session), prefetched: Vec::new(), local: local as u8, start_index, scene: None })
}

/// The scripts that stage a test scene: armies on the first tick, their orders on the second.
pub fn scene_scripts(opts: &Options, map: &Arc<MapFile>, blueprints: &Arc<Blueprints>) -> (sim_thread::SceneScript, sim_thread::SceneScript) {
    let config = setup::match_config(opts, map);
    let (m, b, o) = (map.clone(), blueprints.clone(), opts.clone());
    let first: sim_thread::SceneScript = Box::new(move |_| setup::opening_commands(&o, &m, &b, &config));
    let (m, o) = (map.clone(), opts.clone());
    let second: sim_thread::SceneScript = Box::new(move |world| {
        let u = &world.state.units;
        let units: Vec<_> = u.slots.iter().map(|r| (u.owner[r], u.id(r), world.bp(r).is_mobile())).collect();
        setup::scene_orders(&o, &m, &units)
    });
    (first, second)
}

impl FrontStage {
    fn new(args: &AppArgs, settings: &Settings, viewport: Vec2) -> Result<FrontStage, String> {
        let path = setup::backdrop_map().ok_or("no maps found. Bake one with: cargo run --release -p mc-map --bin mc-bake -- --layout islands --size-km 10 --seed 46 --name \"Twin Shoals\" -o maps/twin_shoals.mcmap")?;
        let map = Arc::new(MapFile::open(&path).map_err(|e| format!("{}: {e}", path.display()))?);
        let sim = Self::stage_battle(args, &map)?;
        let size = Vec2::from(map.info().size_metres().to_f32());
        let director = Director::new(&map, settings.backdrop_auto_advance);
        Ok(FrontStage { front: Front::new(director), map, sim, serial: 0, published_at: Instant::now(), frame: RenderFrame::default(), status: SimStatus::default(), camera: Camera::new(size, viewport), quiet: 0.0 })
    }

    fn stage_battle(args: &AppArgs, map: &Arc<MapFile>) -> Result<SimHandle, String> {
        let opts = Options { map: Default::default(), scene: Scene::Backdrop, players: 2, seed: 7, army: 0, fog: false };
        let request = MatchRequest { map: map.clone(), config: setup::match_config(&opts, map), colors: setup::TEAM_COLORS };
        let start = local_start(request, &args.blueprints, false)?;
        let scene = Some(scene_scripts(&opts, map, &args.blueprints));
        Ok(sim_thread::spawn(SimSetup { map: map.clone(), blueprints: args.blueprints.clone(), pool: args.pool.clone(), prefetched: Vec::new(), scene }, start.session))
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attrs = Window::default_attributes().with_title("Meridian Conflict").with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0));
        if self.args.smoke {
            // Unattended: do not take the keyboard from whatever the person is doing.
            attrs = attrs.with_active(false);
        } else if self.settings.fullscreen {
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        match event_loop.create_window(attrs) {
            Ok(w) => self.window = Some(Arc::new(w)),
            Err(e) => self.fail(event_loop, format!("could not create a window: {e}")),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // The interface's view of the input, whatever the stage.
        match &event {
            WindowEvent::CursorMoved { position, .. } => self.input.cursor = Vec2::new(position.x as f32, position.y as f32),
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.input.down = *state == ElementState::Pressed;
                self.input.pressed |= self.input.down;
                self.input.released |= !self.input.down;
            }
            WindowEvent::KeyboardInput { event: key, .. } if key.state == ElementState::Pressed => {
                if let PhysicalKey::Code(code) = key.physical_key {
                    let mapped = match code {
                        KeyCode::ArrowUp | KeyCode::KeyW => Some(Key::Up),
                        KeyCode::ArrowDown | KeyCode::KeyS => Some(Key::Down),
                        KeyCode::ArrowLeft => Some(Key::Left),
                        KeyCode::ArrowRight => Some(Key::Right),
                        KeyCode::Enter | KeyCode::NumpadEnter => Some(Key::Enter),
                        KeyCode::Escape => Some(Key::Escape),
                        KeyCode::Backspace => Some(Key::Backspace),
                        _ => None,
                    };
                    // W and S steer menus only while nothing is being typed.
                    let letter = matches!(code, KeyCode::KeyW | KeyCode::KeyS);
                    if let Some(k) = mapped.filter(|_| !(letter && self.memory.editing.is_some())) {
                        if !key.repeat || k == Key::Backspace {
                            self.input.keys.push(k);
                        }
                    }
                }
                if let Some(text) = &key.text {
                    self.input.typed.push_str(text);
                }
            }
            _ => {}
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                let viewport = Vec2::new(size.width.max(1) as f32, size.height.max(1) as f32);
                match &mut self.stage {
                    Stage::Front(f) => f.camera.viewport = viewport,
                    Stage::Match(g) => g.resized(viewport),
                    _ => {}
                }
                if let Some(r) = &mut self.renderer {
                    if let Err(e) = r.resize(size.width, size.height) {
                        self.fail(event_loop, e.to_string());
                    }
                }
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            other => {
                if let (Stage::Match(game), Some(r)) = (&mut self.stage, &self.renderer) {
                    // The match sees the press that opened its menu, not the ones aimed at it.
                    game.window_event(&other, r, &self.audio);
                }
            }
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

    fn viewport(&self) -> Vec2 {
        let size = self.window.as_ref().map(|w| w.inner_size()).unwrap_or_default();
        Vec2::new(size.width.max(1) as f32, size.height.max(1) as f32)
    }

    /// Replaces the renderer with one for `map`. The old one goes first: two
    /// cannot share the window's surface.
    fn build_renderer(&mut self, map: &Arc<MapFile>, colors: [[f32; 3]; 8]) -> Result<(), String> {
        self.renderer = None;
        let window = self.window.as_ref().ok_or("no window")?;
        let size = window.inner_size();
        let (display, handle) = match (window.display_handle(), window.window_handle()) {
            (Ok(d), Ok(h)) => (d.as_raw(), h.as_raw()),
            _ => return Err("the window has no native handle".into()),
        };
        let scene = SceneDesc { map: map.clone(), blueprints: self.args.blueprints.clone(), pool: self.args.pool.clone(), team_colors: colors };
        let vsync = self.settings.vsync && !self.args.force_no_vsync;
        self.applied_vsync = vsync;
        let target = Target::Window { display, window: handle, width: size.width, height: size.height, vsync };
        let started = Instant::now();
        self.renderer = Some(Renderer::new(target, scene).map_err(|e| format!("could not start the renderer: {e}"))?);
        log::info!("renderer for {:?} ready in {:.0} ms", map.name(), started.elapsed().as_secs_f32() * 1000.0);
        Ok(())
    }

    fn enter(&mut self, pending: Pending) -> Result<(), String> {
        self.stage_since = Instant::now();
        self.memory = ui::Memory::default();
        self.reveal = 0.0;
        match pending {
            Pending::Front => {
                let stage = FrontStage::new(&self.args, &self.settings, self.viewport())?;
                self.build_renderer(&stage.map, setup::TEAM_COLORS)?;
                self.overlay.set_image(PREVIEW_SLOT, ui::preview::SIZE, ui::preview::SIZE, &ui::preview::render(&stage.map));
                self.audio.set_ambience(true);
                self.stage = Stage::Front(Box::new(stage));
            }
            Pending::Match(start) => {
                self.build_renderer(&start.map, start.colors)?;
                self.audio.set_ambience(false);
                let game = Game::new(*start, self.args.blueprints.clone(), self.args.pool.clone(), self.viewport(), self.settings.show_profiler);
                self.stage = Stage::Match(Box::new(game));
            }
        }
        Ok(())
    }

    fn load(&mut self, pending: Pending, title: &str, detail: &str) {
        self.stage = Stage::Loading { pending: Some(pending), title: title.to_owned(), detail: detail.to_owned(), frames: 0 };
    }

    fn apply_settings(&mut self, display: bool) -> Result<(), String> {
        self.audio.set_volumes(self.settings.volumes());
        if display {
            if let Some(w) = &self.window {
                w.set_fullscreen(self.settings.fullscreen.then_some(Fullscreen::Borderless(None)));
            }
            // Vertical sync is a property of the swapchain: rebuild the renderer around it.
            let vsync = self.settings.vsync && !self.args.force_no_vsync;
            if let (Stage::Front(f), true) = (&self.stage, vsync != self.applied_vsync) {
                let map = f.map.clone();
                self.build_renderer(&map, setup::TEAM_COLORS)?;
                if let Stage::Front(f) = &mut self.stage {
                    f.serial = 0;
                }
            }
        }
        self.settings.save();
        Ok(())
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let time = (now - self.started).as_secs_f32();
        if let Err(e) = self.run_frame(event_loop, now, dt, time) {
            self.fail(event_loop, e);
        }
        self.input.end_frame();
    }

    fn run_frame(&mut self, event_loop: &ActiveEventLoop, now: Instant, dt: f32, time: f32) -> Result<(), String> {
        let viewport = self.viewport();
        self.memory.begin_frame();
        self.reveal = (self.reveal + dt / 0.6).min(1.0);
        let veil = (1.0 - self.reveal).powi(2);

        let mut next: Option<(Pending, &str, String)> = None;
        let mut quit = false;
        let mut settings_changed = (false, false);
        let mut ready: Option<Pending> = None;
        match &mut self.stage {
            Stage::Loading { pending, title, detail, frames } => {
                // The card is drawn by the outgoing renderer (if there is one) before
                // the swap blocks this thread: two frames, so it is really on screen.
                *frames += 1;
                if let Some(r) = &mut self.renderer {
                    self.overlay.clear();
                    let mut ui = Ui::new(&mut self.overlay, &self.input, &mut self.memory, &self.audio, viewport, self.settings.ui_scale, time, dt);
                    front::loading_card(&mut ui, title, detail);
                    let camera = Camera::new(Vec2::splat(1000.0), viewport);
                    r.render(&FrameInput { camera: &camera, time, alpha: 1.0, sim: None, ghosts: &[], marks: &[], overlay: &self.overlay, build_grid: false }).map_err(|e| e.to_string())?;
                }
                if *frames >= 3 || self.renderer.is_none() {
                    ready = pending.take();
                }
            }
            Stage::Front(stage) => {
                let FrontStage { front, map, sim, serial, published_at, frame, status, camera, quiet } = &mut **stage;
                let fresh = match sim.pull(serial, frame, status) {
                    Some(at) => {
                        *published_at = at;
                        true
                    }
                    None => false,
                };
                // Restage the battle when the director cuts back to it, or when it has burnt out.
                *quiet = if frame.projectiles.is_empty() && status.tick > 300 { *quiet + dt } else { 0.0 };
                if *quiet > 12.0 {
                    front.director.restart();
                    *quiet = 0.0;
                }
                if front.director.restage {
                    *sim = FrontStage::stage_battle(&self.args, map)?;
                    *serial = 0;
                }
                let renderer = self.renderer.as_mut().ok_or("no renderer")?;
                front.director.apply(camera);
                camera.focus.z = renderer.ground_height(camera.focus.truncate()).max(map.info().water_level.to_f32());

                self.overlay.clear();
                let mut ui = Ui::new(&mut self.overlay, &self.input, &mut self.memory, &self.audio, viewport, self.settings.ui_scale, time, dt);
                let telemetry = Telemetry { map_name: map.name(), tick: status.tick, units: status.units, camera: camera.focus.truncate(), altitude: camera.eye().z - camera.focus.z, preview: true };
                let out = front.frame(&mut ui, &mut self.settings, &telemetry);
                ui.fill(ui::Rect::new(0.0, 0.0, ui.size.x, ui.size.y), ui::rgb(0x000000, veil));
                settings_changed = (out.settings_changed, out.display_changed);
                match out.event {
                    Some(FrontEvent::Launch(request)) => {
                        let detail = format!("{}   \u{b7}   {} COMMANDERS", request.map.name().to_uppercase(), request.config.players.len());
                        next = Some((Pending::Match(Box::new(local_start(request, &self.args.blueprints, true)?)), "DEPLOYING", detail));
                    }
                    Some(FrontEvent::Quit) => quit = true,
                    None => {}
                }
                let alpha = ((now - *published_at).as_secs_f32() / TICK_SECONDS).clamp(0.0, 1.0);
                renderer.render(&FrameInput { camera, time, alpha, sim: fresh.then_some(&*frame), ghosts: &[], marks: &[], overlay: &self.overlay, build_grid: false }).map_err(|e| format!("rendering failed: {e}"))?;
            }
            Stage::Match(game) => {
                let renderer = self.renderer.as_mut().ok_or("no renderer")?;
                let ctx = FrameCtx { renderer, overlay: &mut self.overlay, input: &self.input, memory: &mut self.memory, audio: &self.audio, settings: &mut self.settings, settings_changed: &mut settings_changed.0, now, dt, time };
                match game.frame(ctx)? {
                    Some(GameEvent::Leave) => {
                        self.audio.play(Sfx::Whoosh);
                        next = Some((Pending::Front, "STANDING DOWN", "RETURNING TO COMMAND".into()));
                    }
                    Some(GameEvent::Quit) => quit = true,
                    None => {}
                }
            }
        }
        self.memory.end_frame(&self.input);
        if self.args.smoke && next.is_none() {
            let waited = self.stage_since.elapsed().as_secs_f32();
            match (&self.stage, self.smoke_step) {
                (Stage::Front(_), 0) if waited > 4.0 => {
                    let request = ui::skirmish::default_request(&self.settings).ok_or("smoke: the default skirmish is not startable")?;
                    log::info!("smoke: front end is up; starting a skirmish on {:?}", request.map.name());
                    next = Some((Pending::Match(Box::new(local_start(request, &self.args.blueprints, false)?)), "DEPLOYING", "SMOKE TEST".into()));
                    self.smoke_step = 1;
                }
                (Stage::Match(game), 1) if game.tick() > 50 => {
                    log::info!("smoke: match reached tick {}; leaving", game.tick());
                    next = Some((Pending::Front, "STANDING DOWN", "SMOKE TEST".into()));
                    self.smoke_step = 2;
                }
                (Stage::Match(_), 1) if waited > 120.0 => return Err("smoke: the match never started ticking".into()),
                (Stage::Front(f), 2) if waited > 4.0 && f.status.tick > 10 => {
                    log::info!("smoke: back in the front end, backdrop at tick {}; passed", f.status.tick);
                    quit = true;
                }
                _ => {}
            }
        }
        if let Some(pending) = ready {
            self.enter(pending)?;
        }

        if settings_changed.0 || settings_changed.1 {
            self.apply_settings(settings_changed.1)?;
        }
        if let Some((pending, title, detail)) = next {
            self.load(pending, title, &detail);
        }
        if quit {
            event_loop.exit();
        }
        Ok(())
    }
}

/// Joins a relay and waits in its lobby until the match starts. Returns the
/// session, every event polled from `Started` on, and our slot.
pub fn lobby(addr: &str, name: &str, content: mc_net::ContentId, template: Vec<u8>) -> Result<(mc_net::NetSession, Vec<mc_net::SessionEvent>, u8), String> {
    use mc_net::{Session, SessionEvent};
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
