//! The windowed application: owns the window, the renderer, sound and settings,
//! and moves between three stages: the front end over its live backdrop, a
//! loading card while a map and renderer are swapped, and the match.
//!
//! A renderer is built for one map, so every stage change builds a fresh one.
//! That also guarantees nothing of the last battle (scorch marks, terrain
//! edits, fog) can leak into the next.

use crate::audio::Audio;
use crate::game::{FrameCtx, Game, GameEvent, GameStart};
use crate::loading::{self, Curtain, Job, MapSource, Order};
use crate::settings::Settings;
use crate::setup::{self, Options, Scene};
use crate::sim_thread::{self, SimHandle, SimSetup, SimStatus};
use crate::ui::backdrop::Director;
use crate::ui::front::{Front, FrontEvent};
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
    /// The sound library, handed to the mixer at start-up.
    pub sounds: mc_data::SoundLibrary,
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
    Loading {
        pending: Option<Pending>,
        /// The background build, started once there is a window to build for,
        /// and what it built, waiting for the loading screen to come to rest.
        job: Option<Job>,
        built: Option<loading::Ready>,
        /// The outgoing renderer has been turned down to draw the loading
        /// screen cheaply: the scene behind it is covered anyway.
        eased: bool,
        /// The outgoing stage's view: the old renderer keeps it while it draws
        /// the loading screen, so it has no new ground to stream in.
        camera: Option<Camera>,
    },
    Match(Box<Game>),
}

/// The front end and the battle staged behind it.
struct FrontStage {
    front: Front,
    map: Arc<MapFile>,
    sim: SimHandle,
    serial: u64,
    published_at: Instant,
    interp_span: f32,
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
    /// The mouse pointers, and the one the window shows.
    cursors: crate::pointer::Cursors,
    stage: Stage,
    /// The loading screen: over the loading stage, then over the new stage
    /// until it runs steadily.
    curtain: Option<Curtain>,
    /// The front end's map, kept open through matches: going back needs no reading.
    backdrop: Option<Arc<MapFile>>,
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
    /// The match being loaded is survival: its sky is the survival screen's.
    survival_launch: bool,
}

pub fn run(mut args: AppArgs) -> Result<(), String> {
    raise_this_thread();
    let settings = Settings::load();
    // The smoke test opens the device like any run, but is not there to be heard.
    let audio = Audio::new(
        if args.smoke {
            crate::audio::Volumes {
                master: 0.0,
                interface: 0.0,
                effects: 0.0,
                weather: 0.0,
            }
        } else {
            settings.volumes()
        },
        std::mem::take(&mut args.sounds),
    );
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
        cursors: Default::default(),
        stage: Stage::Loading {
            pending: Some(first),
            job: None,
            built: None,
            eased: false,
            camera: None,
        },
        curtain: None,
        backdrop: None,
        reveal: 0.0,
        applied_vsync: true,
        started: now,
        last_frame: now,
        fatal: None,
        smoke_step: 0,
        stage_since: now,
        survival_launch: false,
    };
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.settings.save();
    match app.fatal {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// The window's thread draws every frame; the simulation, the job pool and the
/// loading threads can keep every core busy, and when they do the scheduler
/// makes the window wait its turn and frames come late. It goes first instead.
fn raise_this_thread() {
    set_this_thread_priority(1);
}

/// Win32 thread priority, -2 (lowest) to 2 (highest); nothing elsewhere.
pub fn set_this_thread_priority(priority: i32) {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentThread() -> isize;
            fn SetThreadPriority(thread: isize, priority: i32) -> i32;
        }
        if unsafe { SetThreadPriority(GetCurrentThread(), priority) } == 0 {
            log::warn!("could not set a thread's priority");
        }
    }
    #[cfg(not(windows))]
    let _ = priority;
}

/// A local match from a front-end request. No human seat means this machine
/// watches: every commander is an AI, and the camera opens on the whole map.
pub fn local_start(
    request: MatchRequest,
    blueprints: &Blueprints,
    record: bool,
) -> Result<GameStart, String> {
    let MatchRequest {
        map,
        config,
        colors,
        survival,
    } = request;
    let content = mc_net::ContentId {
        map_id: map.content_id(),
        blueprint_hash: blueprints.content_hash(),
    };
    let human = config
        .players
        .iter()
        .position(|p| p.controller == mc_sim::tables::Controller::Human);
    let observing = human.is_none();
    let local = human.unwrap_or(0);
    let humans = if observing {
        Vec::new()
    } else {
        vec![mc_net::PlayerSetup {
            slot: mc_core::PlayerId(local as u8),
            name: config.players[local].name.clone(),
            data: Vec::new(),
        }]
    };
    let start = mc_net::MatchStart {
        content,
        seed: config.seed,
        input_delay: 1,
        players: humans,
        options: crate::survival::encode_options(&config, survival.as_ref())?,
    };
    let mut session = if observing {
        mc_net::LocalSession::observer(start, mc_net::session::Pacing::RealTime)
    } else {
        mc_net::LocalSession::new(
            start,
            mc_core::PlayerId(local as u8),
            mc_net::session::Pacing::RealTime,
        )
    }
    .map_err(|e| e.to_string())?;
    if record {
        if let Err(e) = session.record_to("last-match.mcreplay") {
            log::warn!("this match will not be recorded: {e}");
        }
    }
    let start_index = config
        .players
        .get(local)
        .map(|p| p.start as usize)
        .unwrap_or(0);
    Ok(GameStart {
        map,
        colors,
        session: Box::new(session),
        prefetched: Vec::new(),
        local: local as u8,
        start_index,
        observing,
        scene: None,
        range: None,
    })
}

/// The test range on `map` with `subject` (a blueprint key) on the pad. An
/// unknown key opens it on the default subject instead: after a reload the
/// unit that was there may be gone.
pub fn range_start(
    map: &Arc<MapFile>,
    blueprints: &Arc<Blueprints>,
    subject: &str,
    scenario: Option<crate::range::Scenario>,
) -> Result<GameStart, String> {
    let key = if blueprints.id_of(subject).is_some() {
        subject
    } else {
        crate::range::DEFAULT_SUBJECT
    };
    let id = blueprints
        .id_of(key)
        .ok_or(format!("there is no unit {key:?} to put on the range"))?;
    let opts = Options {
        scene: Scene::Range,
        fog: false,
        subject: key.to_owned(),
        scenario,
        ..Default::default()
    };
    let request = MatchRequest {
        map: map.clone(),
        config: setup::match_config(&opts, map),
        colors: setup::TEAM_COLORS,
        survival: None,
    };
    let mut start = local_start(request, blueprints, false)?;
    start.scene = Some(scene_scripts(&opts, map, blueprints));
    start.range = Some(id);
    Ok(start)
}

/// The scripts that stage a test scene: armies on the first tick, their orders on the second.
pub fn scene_scripts(
    opts: &Options,
    map: &Arc<MapFile>,
    blueprints: &Arc<Blueprints>,
) -> (sim_thread::SceneScript, sim_thread::SceneScript) {
    let config = setup::match_config(opts, map);
    let (m, b, o) = (map.clone(), blueprints.clone(), opts.clone());
    let first: sim_thread::SceneScript =
        Box::new(move |_| setup::opening_commands(&o, &m, &b, &config));
    let (m, b, o) = (map.clone(), blueprints.clone(), opts.clone());
    let second: sim_thread::SceneScript =
        Box::new(move |world| setup::scene_orders(&o, &m, &b, world));
    (first, second)
}

impl FrontStage {
    /// Over `map`, the backdrop map the loading thread opened.
    fn new(
        args: &AppArgs,
        settings: &Settings,
        viewport: Vec2,
        map: Arc<MapFile>,
    ) -> Result<FrontStage, String> {
        let sim = Self::stage_battle(args, &map)?;
        let size = Vec2::from(map.info().size_metres().to_f32());
        let director = Director::new(&map, settings.backdrop_auto_advance);
        Ok(FrontStage {
            front: Front::new(director),
            map,
            sim,
            serial: 0,
            published_at: Instant::now(),
            interp_span: TICK_SECONDS,
            frame: RenderFrame::default(),
            status: SimStatus::default(),
            camera: Camera::new(size, viewport),
            quiet: 0.0,
        })
    }

    fn stage_battle(args: &AppArgs, map: &Arc<MapFile>) -> Result<SimHandle, String> {
        let opts = Options {
            scene: Scene::Backdrop,
            seed: 7,
            army: 0,
            fog: false,
            ..Default::default()
        };
        let request = MatchRequest {
            map: map.clone(),
            config: setup::match_config(&opts, map),
            colors: setup::TEAM_COLORS,
            survival: None,
        };
        let start = local_start(request, &args.blueprints, false)?;
        let scene = Some(scene_scripts(&opts, map, &args.blueprints));
        Ok(sim_thread::spawn(
            SimSetup {
                map: map.clone(),
                blueprints: args.blueprints.clone(),
                pool: args.pool.clone(),
                prefetched: Vec::new(),
                scene,
            },
            start.session,
        ))
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title("Meridian Conflict")
            .with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0));
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
            WindowEvent::MouseWheel { delta, .. } => {
                self.input.scroll += match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.cursor = Vec2::new(position.x as f32, position.y as f32)
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.input.down = *state == ElementState::Pressed;
                self.input.pressed |= self.input.down;
                self.input.released |= !self.input.down;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => self.input.right_pressed = true,
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
                let covered = self.curtain.as_ref().is_some_and(|c| c.covering());
                if let (Stage::Match(game), Some(r), false) = (&mut self.stage, &self.renderer, covered) {
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
        let size = self
            .window
            .as_ref()
            .map(|w| w.inner_size())
            .unwrap_or_default();
        Vec2::new(size.width.max(1) as f32, size.height.max(1) as f32)
    }

    /// Replaces the renderer with one for `map`, blocking this thread. The old
    /// one goes first: two cannot share the window's surface. Only for a
    /// change of vertical sync; loads go through `load` and never block.
    fn build_renderer(&mut self, map: &Arc<MapFile>, colors: [[f32; 3]; 8]) -> Result<(), String> {
        self.renderer = None;
        let target = self.window_target()?;
        let scene = SceneDesc {
            map: map.clone(),
            blueprints: self.args.blueprints.clone(),
            pool: self.args.pool.clone(),
            team_colors: colors,
        };
        let started = Instant::now();
        self.renderer = Some(
            Renderer::new(target, scene)
                .map_err(|e| format!("could not start the renderer: {e}"))?,
        );
        self.apply_render_quality()?;
        log::info!(
            "renderer for {:?} ready in {:.0} ms",
            map.name(),
            started.elapsed().as_secs_f32() * 1000.0
        );
        Ok(())
    }

    /// The window as a renderer's target, with the vertical sync wanted now.
    fn window_target(&mut self) -> Result<Target, String> {
        let window = self.window.as_ref().ok_or("no window")?;
        let size = window.inner_size();
        let (display, handle) = match (window.display_handle(), window.window_handle()) {
            (Ok(d), Ok(h)) => (d.as_raw(), h.as_raw()),
            _ => return Err("the window has no native handle".into()),
        };
        let vsync = self.settings.vsync && !self.args.force_no_vsync;
        self.applied_vsync = vsync;
        Ok(Target::Window {
            display,
            window: handle,
            width: size.width,
            height: size.height,
            vsync,
        })
    }

    /// Starts the background build for `pending` once there is a window.
    fn start_job(&mut self) -> Result<(), String> {
        if self.window.is_none() || self.curtain.as_ref().is_some_and(|c| !c.ready_to_build()) {
            return Ok(());
        }
        let Stage::Loading { pending: Some(pending), job: None, .. } = &self.stage else {
            return Ok(());
        };
        let (map, colors, pictures) = match pending {
            Pending::Front => (
                match &self.backdrop {
                    Some(map) => MapSource::Loaded(map.clone()),
                    None => MapSource::Backdrop,
                },
                setup::TEAM_COLORS,
                None,
            ),
            Pending::Match(start) => (
                MapSource::Loaded(start.map.clone()),
                start.colors,
                Some(Game::picture_team(start)),
            ),
        };
        let target = self.window_target()?;
        let order = Order {
            target,
            map,
            blueprints: self.args.blueprints.clone(),
            pool: self.args.pool.clone(),
            colors,
            pictures,
        };
        if let Stage::Loading { job, .. } = &mut self.stage {
            *job = Some(Job::start(order));
        }
        Ok(())
    }

    /// The build is done: the window changes hands and the new stage starts
    /// under the curtain.
    fn enter(&mut self, pending: Pending, ready: loading::Ready) -> Result<(), String> {
        self.stage_since = Instant::now();
        self.memory = ui::Memory::default();
        // The curtain lifts off the new stage; without one it comes up from black.
        self.reveal = if self.curtain.is_some() { 1.0 } else { 0.0 };
        let loading::Ready {
            mut renderer,
            map,
            chart,
            thumbs,
        } = ready;
        let handover = Instant::now();
        if let Some(mut old) = self.renderer.take() {
            old.release_window();
            loading::retire(old);
        }
        renderer
            .attach()
            .map_err(|e| format!("could not start the renderer: {e}"))?;
        // The window may have changed size while the build ran.
        let size = self.viewport();
        renderer
            .resize(size.x as u32, size.y as u32)
            .map_err(|e| e.to_string())?;
        self.renderer = Some(renderer);
        self.apply_render_quality()?;
        log::info!(
            "renderer for {:?} took the window in {:.0} ms",
            map.name(),
            handover.elapsed().as_secs_f32() * 1000.0
        );
        if let Some(c) = &mut self.curtain {
            c.handed_over();
        }
        match pending {
            Pending::Front => {
                self.backdrop = Some(map.clone());
                let stage = FrontStage::new(&self.args, &self.settings, self.viewport(), map)?;
                self.overlay
                    .set_image(PREVIEW_SLOT, ui::preview::SIZE, ui::preview::SIZE, &chart);
                self.stage = Stage::Front(Box::new(stage));
            }
            Pending::Match(start) => {
                // The map's own weather and time of day, unless skirmish set-up picked others.
                let config = setup::map_config(&start.map);
                let sky = if std::mem::take(&mut self.survival_launch) {
                    self.settings.survival_sky
                } else {
                    self.settings.skirmish_sky
                };
                if let Some(r) = &mut self.renderer {
                    r.set_weather(sky.weather(&config));
                    r.set_hour(sky.hour(&config));
                }
                let mut game = Game::new(
                    *start,
                    self.args.blueprints.clone(),
                    self.args.pool.clone(),
                    self.viewport(),
                    self.settings.show_profiler,
                );
                if let Some(thumbs) = thumbs {
                    game.preload(&mut self.overlay, &chart, thumbs);
                }
                self.stage = Stage::Match(Box::new(game));
            }
        }
        Ok(())
    }

    /// Leaves the current stage for `pending`, behind the loading screen when
    /// there is a `title` (the first stage of a run just comes up from black).
    fn load(&mut self, pending: Pending, title: &str, detail: &str) {
        self.curtain = (!title.is_empty()).then(|| {
            let from_black = matches!(self.stage, Stage::Front(_));
            let mut curtain = Curtain::new(title, detail, from_black);
            if let Pending::Match(start) = &pending {
                curtain.set_map(&start.map, (!start.observing).then_some(start.start_index));
            }
            curtain
        });
        if matches!(self.stage, Stage::Match(_)) {
            self.audio.release_world();
        }
        let camera = match &self.stage {
            Stage::Front(f) => Some(f.camera.clone()),
            Stage::Match(game) => Some(game.camera().clone()),
            Stage::Loading { camera, .. } => camera.clone(),
        };
        self.stage = Stage::Loading {
            pending: Some(pending),
            job: None,
            built: None,
            eased: false,
            camera,
        };
    }

    fn apply_render_quality(&mut self) -> Result<(), String> {
        if let Some(r) = &mut self.renderer {
            r.set_render_quality(self.settings.render_scale, self.settings.fxaa)
                .map_err(|e| format!("could not change the render scale: {e}"))?;
        }
        Ok(())
    }

    fn apply_settings(&mut self, display: bool) -> Result<(), String> {
        self.audio.set_volumes(self.settings.volumes());
        if display {
            if let Some(w) = &self.window {
                w.set_fullscreen(
                    self.settings
                        .fullscreen
                        .then_some(Fullscreen::Borderless(None)),
                );
            }
            self.apply_render_quality()?;
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
        self.audio.follow_device();
        if let Err(e) = self.run_frame(event_loop, now, dt, time) {
            self.fail(event_loop, e);
        }
        self.input.end_frame();
    }

    fn run_frame(
        &mut self,
        event_loop: &ActiveEventLoop,
        now: Instant,
        dt: f32,
        time: f32,
    ) -> Result<(), String> {
        let viewport = self.viewport();
        self.memory.begin_frame();
        self.reveal = (self.reveal + dt / 0.6).min(1.0);
        let veil = (1.0 - self.reveal).powi(2);
        // Nothing under the loading screen hears a click or a key.
        if self.curtain.as_ref().is_some_and(|c| c.covering()) {
            self.input.end_frame();
        }
        self.start_job()?;

        let mut next: Option<(Pending, &str, String)> = None;
        let mut quit = false;
        let mut settings_changed = (false, false);
        let mut ready: Option<(Pending, loading::Ready)> = None;
        match &mut self.stage {
            Stage::Loading {
                pending,
                job,
                built,
                eased,
                camera,
            } => {
                // Its first frame here the old stage's last picture is still on
                // screen: a stall to rebuild at a lower scale goes unseen, and
                // this frame draws nothing so the screen's arrival starts after it.
                let mut drawn = true;
                if let (Some(r), false) = (&mut self.renderer, *eased) {
                    *eased = true;
                    drawn = false;
                    r.set_render_quality(0.5, false)
                        .map_err(|e| format!("could not change the render scale: {e}"))?;
                }
                if let Some(job) = job {
                    if let Some(chart) = job.take_chart() {
                        self.overlay.set_image(
                            crate::hud::MINIMAP_SLOT,
                            ui::preview::SIZE,
                            ui::preview::SIZE,
                            &chart,
                        );
                        if let Some(c) = &mut self.curtain {
                            c.chart_ready();
                        }
                    }
                    let (step, done) = job.progress();
                    if let Some(c) = &mut self.curtain {
                        c.building(step, done);
                    }
                    if built.is_none() {
                        if let Some(result) = job.finished() {
                            *built = Some(result?);
                            if let Some(c) = &mut self.curtain {
                                c.built();
                            }
                        }
                    }
                }
                // The window changes hands while the loading screen is at rest.
                if built.is_some() && self.curtain.as_ref().map_or(true, |c| c.still()) {
                    let pending = pending.take().ok_or("nothing was loading")?;
                    ready = built.take().map(|b| (pending, b));
                }
                // The outgoing renderer draws the loading screen while the new
                // one is built; the first load of a run has none, and waits.
                if let (Some(r), true) = (&mut self.renderer, drawn) {
                    self.overlay.clear();
                    let mut ui = Ui::new(
                        &mut self.overlay,
                        &self.input,
                        &mut self.memory,
                        &self.audio,
                        viewport,
                        self.settings.ui_scale,
                        time,
                        dt,
                    );
                    match &mut self.curtain {
                        Some(c) => c.draw(&mut ui),
                        None => ui.fill(ui::Rect::new(0.0, 0.0, ui.size.x, ui.size.y), ui::ink(1.0)),
                    }
                    let camera = match camera {
                        Some(c) => {
                            c.viewport = viewport;
                            c.clone()
                        }
                        None => Camera::new(Vec2::splat(1000.0), viewport),
                    };
                    r.render(&FrameInput {
                        camera: &camera,
                        time,
                        alpha: 1.0,
                        sim: None,
                        ghosts: &[],
                        marks: &[],
                        ranges: &[],
                        ranges_drawn: 0,
                        overlay: &self.overlay,
                        build_grid: false,
                    })
                    .map_err(|e| e.to_string())?;
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(4));
                }
            }
            Stage::Front(stage) => {
                let FrontStage {
                    front,
                    map,
                    sim,
                    serial,
                    published_at,
                    interp_span,
                    frame,
                    status,
                    camera,
                    quiet,
                } = &mut **stage;
                let fresh = match sim.pull(serial, frame, status) {
                    Some(at) => {
                        if at != *published_at {
                            let gap = at.duration_since(*published_at).as_secs_f32();
                            *interp_span = gap.clamp(TICK_SECONDS, TICK_SECONDS * 8.0);
                            *published_at = at;
                        }
                        true
                    }
                    None => false,
                };
                // Restage the battle when the director cuts back to it, or when it has burnt out.
                *quiet = if frame.projectiles.is_empty() && status.tick > 300 {
                    *quiet + dt
                } else {
                    0.0
                };
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
                camera.focus.z = renderer
                    .ground_height(camera.focus.truncate())
                    .max(map.info().water_level.to_f32());

                self.overlay.clear();
                let mut ui = Ui::new(
                    &mut self.overlay,
                    &self.input,
                    &mut self.memory,
                    &self.audio,
                    viewport,
                    self.settings.ui_scale,
                    time,
                    dt,
                );
                let telemetry = Telemetry {
                    map_name: map.name(),
                    tick: status.tick,
                    units: status.units,
                    camera: camera.focus.truncate(),
                    altitude: camera.eye().z - camera.focus.z,
                    preview: true,
                };
                let out = front.frame(&mut ui, &mut self.settings, &telemetry);
                ui.fill(
                    ui::Rect::new(0.0, 0.0, ui.size.x, ui.size.y),
                    ui::rgb(0x000000, veil),
                );
                if let Some(c) = &mut self.curtain {
                    c.draw(&mut ui);
                }
                settings_changed = (out.settings_changed, out.display_changed);
                match out.event {
                    Some(FrontEvent::Launch(request)) => {
                        self.survival_launch = request.survival.is_some();
                        let detail = match &request.survival {
                            Some(s) if s.rules.rounds == 0 => "Survival   \u{b7}   Endless".to_owned(),
                            Some(s) => format!("Survival   \u{b7}   {} rounds", s.rules.rounds),
                            None => format!(
                                "Skirmish   \u{b7}   {} commanders",
                                request.config.players.len()
                            ),
                        };
                        next = Some((
                            Pending::Match(Box::new(local_start(
                                request,
                                &self.args.blueprints,
                                true,
                            )?)),
                            "Deploying",
                            detail,
                        ));
                    }
                    Some(FrontEvent::Range) => {
                        let path = setup::find_map(None)?;
                        let map = Arc::new(
                            MapFile::open(&path).map_err(|e| format!("{}: {e}", path.display()))?,
                        );
                        let start = range_start(
                            &map,
                            &self.args.blueprints,
                            crate::range::DEFAULT_SUBJECT,
                            None,
                        )?;
                        next = Some((
                            Pending::Match(Box::new(start)),
                            "Test Range",
                            "Weapons and units on the pad".to_owned(),
                        ));
                    }
                    Some(FrontEvent::Quit) => quit = true,
                    None => {}
                }
                let alpha = ((now - *published_at).as_secs_f32() / *interp_span).clamp(0.0, 1.0);
                renderer
                    .render(&FrameInput {
                        camera,
                        time,
                        alpha,
                        sim: fresh.then_some(&*frame),
                        ghosts: &[],
                        marks: &[],
                        ranges: &[],
                        ranges_drawn: 0,
                        overlay: &self.overlay,
                        build_grid: false,
                    })
                    .map_err(|e| format!("rendering failed: {e}"))?;
            }
            Stage::Match(game) => {
                let renderer = self.renderer.as_mut().ok_or("no renderer")?;
                let ctx = FrameCtx {
                    renderer,
                    overlay: &mut self.overlay,
                    input: &self.input,
                    memory: &mut self.memory,
                    audio: &self.audio,
                    settings: &mut self.settings,
                    settings_changed: &mut settings_changed.0,
                    display_changed: &mut settings_changed.1,
                    now,
                    dt,
                    time,
                    cover: self.curtain.as_mut(),
                };
                match game.frame(ctx)? {
                    Some(GameEvent::Leave) => {
                        next = Some((
                            Pending::Front,
                            "Standing Down",
                            "Returning to Command".into(),
                        ));
                    }
                    Some(GameEvent::Quit) => quit = true,
                    // Stats, weapons and costs are data: read them again and stand the range back up.
                    Some(GameEvent::ReloadRange(subject)) => {
                        // Units, and the sounds they name: both are read again, and have to agree.
                        let loaded = Blueprints::locate_data_dir()
                            .ok_or("the data/ directory is gone".to_owned())
                            .and_then(|dir| {
                                let blueprints =
                                    Blueprints::load(&dir).map_err(|e| e.to_string())?;
                                let sounds =
                                    mc_data::SoundLibrary::load(&dir).map_err(|e| e.to_string())?;
                                sounds.check(&blueprints).map_err(|e| e.to_string())?;
                                Ok((blueprints, sounds))
                            });
                        match loaded {
                            Ok((blueprints, sounds)) => {
                                self.audio.set_library(sounds);
                                self.args.blueprints = Arc::new(blueprints);
                                let start =
                                    range_start(game.map(), &self.args.blueprints, &subject, None)?;
                                next = Some((
                                    Pending::Match(Box::new(start)),
                                    "Reloading",
                                    "Data/ Read Again".into(),
                                ));
                            }
                            Err(e) => {
                                log::error!("reload failed: {e}");
                                game.complain(&format!("data/ did not load: {e}"));
                            }
                        }
                    }
                    None => {}
                }
            }
        }
        // The pointer says what a click would do; outside a match it is the plain arrow.
        let pointer = match &self.stage {
            Stage::Match(game) => game.pointer(),
            _ => crate::pointer::Pointer::Arrow,
        };
        if let Some(window) = &self.window {
            self.cursors.show(
                event_loop,
                window,
                pointer,
                viewport.y / ui::CANVAS_H * self.settings.ui_scale,
            );
        }
        self.memory.end_frame(&self.input);
        if self.args.smoke && next.is_none() {
            let waited = self.stage_since.elapsed().as_secs_f32();
            match (&self.stage, self.smoke_step) {
                (Stage::Front(_), 0) if waited > 4.0 => {
                    let request = ui::skirmish::default_request(&self.settings)
                        .ok_or("smoke: the default skirmish is not startable")?;
                    log::info!(
                        "smoke: front end is up; starting a skirmish on {:?}",
                        request.map.name()
                    );
                    next = Some((
                        Pending::Match(Box::new(local_start(
                            request,
                            &self.args.blueprints,
                            false,
                        )?)),
                        "Deploying",
                        "Smoke Test".into(),
                    ));
                    self.smoke_step = 1;
                }
                (Stage::Match(game), 1) if game.tick() > 50 => {
                    log::info!("smoke: match reached tick {}; leaving", game.tick());
                    next = Some((Pending::Front, "Standing Down", "Smoke Test".into()));
                    self.smoke_step = 2;
                }
                (Stage::Match(_), 1) if waited > 120.0 => {
                    return Err("smoke: the match never started ticking".into())
                }
                (Stage::Front(f), 2) if waited > 4.0 && f.status.tick > 10 => {
                    log::info!(
                        "smoke: back in the front end, backdrop at tick {}; passed",
                        f.status.tick
                    );
                    quit = true;
                }
                _ => {}
            }
        }
        if let Some(c) = &mut self.curtain {
            let running = match &self.stage {
                Stage::Match(game) => game.tick() >= 1,
                Stage::Front(f) => f.status.tick >= 1,
                Stage::Loading { .. } => false,
            };
            c.settle(dt, running);
            if c.gone() {
                self.curtain = None;
            }
        }
        if let Some((pending, loaded)) = ready {
            self.enter(pending, loaded)?;
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
pub fn lobby(
    addr: &str,
    name: &str,
    content: mc_net::ContentId,
    template: Vec<u8>,
) -> Result<(mc_net::NetSession, Vec<mc_net::SessionEvent>, u8), String> {
    use mc_net::{Session, SessionEvent};
    let config = mc_net::ClientConfig::new(name.to_owned(), mc_net::Role::Player, content);
    let mut session = mc_net::NetSession::connect(addr, config)
        .map_err(|e| format!("could not reach the relay at {addr}: {e}"))?;
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
                        session
                            .set_match_options(template.clone())
                            .map_err(|e| e.to_string())?;
                    }
                    session.set_ready(true);
                    if welcome.in_progress {
                        log::info!("match already running; joining from a snapshot");
                    }
                }
                SessionEvent::Lobby(state) => {
                    log::info!("lobby: {} of the players are in", state.players.len())
                }
                SessionEvent::Started(start) => {
                    let mut rest = vec![SessionEvent::Started(start)];
                    rest.extend(events);
                    return Ok((session, rest, slot.unwrap_or(0)));
                }
                SessionEvent::Ended(reason) => {
                    return Err(format!("the relay closed the session: {reason:?}"))
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
