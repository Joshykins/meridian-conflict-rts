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
use mc_core::{Fx, FxVec2};
use mc_data::{cat, BlueprintId, Blueprints};
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::camera::MIN_DISTANCE;
use mc_render::{Camera, FrameInput, Mark, Overlay, Renderer};
use mc_sim::command::{MAX_GUARD_RADIUS, MIN_GUARD_RADIUS};
use mc_sim::mirror::{
    ShieldInstance, UnitInstance, UnitOrders, KIND_GHOST, KIND_WRECK, STATE_UNIDENTIFIED,
};
use mc_sim::placement::Unfit;
use mc_sim::tables::{flag, OrderKind};
use mc_sim::{Command, FireState, Handle, RenderFrame};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[path = "game_ambience.rs"]
mod ambience_notes;
#[path = "game_cine.rs"]
mod cine_input;
#[path = "game_music.rs"]
mod music_notes;
#[path = "game_survival.rs"]
mod survival_notes;
pub use cine_input::CursorMode;

const DRAG_THRESHOLD: f32 = 6.0;
/// The guard area a click without a drag gives, metres of radius.
const GUARD_DEFAULT: f32 = 250.0;
/// Distance factor of one wheel notch.
const ZOOM_STEP: f32 = 0.8;
/// Stiffness of the spring that carries the camera to the distance the wheel asked
/// for, per second: critically damped, so it eases in and out and never overshoots.
const ZOOM_RATE: f32 = 12.0;
const TICK_SECONDS: f32 = 0.1;
/// Most selected units whose order queues are asked for (and drawn).
const MAX_WATCHED: usize = 64;
/// A control group's key pressed twice within this long also brings the camera.
const DOUBLE_TAP: f32 = 0.35;
/// A unit clicked twice within this long selects every one of its type on screen.
const DOUBLE_CLICK: f32 = 0.35;
/// Test range: how far east of the pad the camera looks, so the pad clears the panel.
const RANGE_LOOK_AHEAD: f32 = 60.0;
/// Radians of yaw per pixel while Alt is held.
const ORBIT_YAW: f32 = 0.0032;
/// Radians of extra tilt per pixel while Alt is held. Mouse-up looks more shallow.
const ORBIT_TILT: f32 = 0.0026;
/// How fast Alt-orbit closes on the yaw, tilt and pivot the mouse asked for, per second.
const ORBIT_RATE: f32 = 18.0;
/// How long leaving a network match waits for its surrender to be carried out.
const SURRENDER_WAIT: std::time::Duration = std::time::Duration::from_secs(3);
/// How fast the camera settles back to its old view after Alt comes up, per second.
/// Slow on purpose: the view drifts home over about a second rather than snapping.
const ORBIT_RETURN_RATE: f32 = 3.0;

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
    /// Who plays and from which start, in slot order (the loading chart marks them).
    pub roster: Vec<mc_sim::PlayerSetup>,
    /// No human seat: every commander is an AI and this machine watches.
    pub observing: bool,
    pub scene: Option<(SceneScript, SceneScript)>,
    /// The match is the test range, with this unit on the pad.
    pub range: Option<BlueprintId>,
    /// This machine records the match under this id (`issues`).
    pub record: Option<crate::issues::MatchRecord>,
    /// Records it from the sim thread, when the session does not (`recorder.rs`).
    pub recorder: Option<crate::recorder::Recorder>,
    /// Watching a replay: jump to this tick as it opens.
    pub seek: Option<u32>,
    /// A network match, and how to come back to it if the connection drops.
    pub net: Option<crate::netplay::Rejoin>,
    /// What a network match needs kept alive: the server link, a relay hosted here.
    pub keep: Vec<Box<dyn std::any::Any + Send>>,
}

/// An order picked from the order card (or its key) that still needs a target.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Targeting {
    Move,
    Attack,
    AttackMove,
    Assist,
    Reclaim,
    /// The first click sets the group patrolling out to it and back; with shift
    /// held, each further click adds a post to that loop.
    Patrol,
    AttackGround,
    /// Press on the centre, drag out the size.
    Bombard,
    /// Press on the spot to hold (or on a friendly unit to go with it), drag out the
    /// area watched around it. Aircraft circle it halfway out.
    Guard,
    /// Lift ships set down on the nearest ground that takes them and lower the ramp.
    Land,
    /// `Land`, and everything in the hold walks out.
    Unload,
    /// A nuclear silo's warhead: click anywhere on the map, or the minimap (`docs/NUKES.md`).
    Nuke,
    /// A giant's storm called down with its great bore alone (the Behemoth's Strike).
    Strike,
}

impl Targeting {
    pub fn label(self) -> &'static str {
        match self {
            Targeting::Move => "Move",
            Targeting::Attack => "Attack",
            Targeting::AttackMove => "Attack-Move",
            Targeting::Assist => "Assist",
            Targeting::Reclaim => "Reclaim",
            Targeting::Patrol => "Patrol",
            Targeting::AttackGround => "Fire on Ground",
            Targeting::Bombard => "Bombard",
            Targeting::Guard => "Guard",
            Targeting::Land => "Land",
            Targeting::Unload => "Unload",
            Targeting::Nuke => "Launch Warhead",
            Targeting::Strike => "Strike",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Normal,
    Target(Targeting),
    Place(BlueprintId),
    /// Test range: the next click copies the selection onto that ground.
    Spawn,
    /// Test range: the next click places the current subject.
    SpawnSubject,
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
    pub formation_panel: bool,
    pub formation_together: bool,
    pub formation_spacing: u8,
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
    /// Ctrl is held: a click on a tile picks rather than acts (a lift ship's hold).
    pub ctrl: bool,
    /// Control groups, by their key.
    pub groups: [Vec<u32>; 10],
    /// The test range, when this match is one.
    pub range: Option<Range>,
    /// The range weather the renderer was last given (live or from a recording),
    /// so a change is applied once.
    pub sky_shown: Option<crate::range::RangeSky>,
    /// The range rings on the ground this frame, by kind: the farthest reach and its dead zone.
    pub reaches: Vec<(Reach, u8, f32, f32)>,
    /// The patrol being laid: where its loop began, then each post clicked, exactly as sent.
    pub patrol_posts: Vec<FxVec2>,
    /// Posts put into a live patrol since shift went down, as `(after, point)`: counted
    /// in its loop until the queues show them, so quick clicks find the right leg.
    pub patrol_inserts: Vec<(FxVec2, FxVec2)>,
    /// The centre of a bombardment or a guard being dragged out.
    pub circle_from: Option<Vec2>,
    /// Warhead launches sent and not yet seen in the frame (`hud::silo::settle_sent`).
    pub nuke_sent: Vec<crate::hud::silo::SentLaunch>,
    /// Tier-5 titans: strikes charging and storms raging, sightings (`titan_marks.rs`).
    pub titans: crate::titan_marks::Titans,
    /// This machine has no slot: the commanders fight, and we watch.
    pub observing: bool,
    /// Observing: whose eyes the battlefield is seen through; `None` sees it all.
    pub perspective: Option<u8>,
    /// Where the map lets structures stand, for the placement preview. Filled
    /// in off the main thread as the match starts; empty until then.
    pub sites: Arc<std::sync::OnceLock<mc_sim::placement::SiteMap>>,
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
            formation_panel: false,
            formation_together: true,
            formation_spacing: 1,
            fps: 0.0,
            cpu_ms: 0.0,
            show_profiler,
            menu_open: false,
            colors,
            speed: 100,
            paused: false,
            shift: false,
            ctrl: false,
            groups: Default::default(),
            range: None,
            reaches: Vec::new(),
            patrol_posts: Vec::new(),
            patrol_inserts: Vec::new(),
            circle_from: None,
            nuke_sent: Vec::new(),
            titans: Default::default(),
            observing: false,
            perspective: None,
            sites: Default::default(),
            sky_shown: None,
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
    /// The loading screen, still over the match while it settles.
    pub cover: Option<&'a mut crate::loading::Curtain>,
}

struct Menu {
    heading: Heading,
    enter: f32,
    closing: bool,
    /// Opened since the last frame: the key press that opened it (Escape) is still in
    /// this frame's input and must not close it again.
    fresh: bool,
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
    /// `repair_beam`, `repair_start`, `repair_end`: every builder shares them.
    repair: [Option<mc_data::SoundId>; 3],
    /// A builder's loop, start and end, by faction: its own (`Faction::sounds.build`), or
    /// the shared `build_beam`, `build_start`, `build_end`.
    build: Vec<[Option<mc_data::SoundId>; 3]>,
    shield_hit: Option<mc_data::SoundId>,
    shield_break: Option<mc_data::SoundId>,
    /// The intercept laser, and the snap when a missile casing fails.
    intercept_laser: Option<mc_data::SoundId>,
    intercept_break: Option<mc_data::SoundId>,
    /// Pneumatic toss of a cold-launched missile, before the motor lights.
    cold_eject: Option<mc_data::SoundId>,
    /// A shot landing in the sea instead of on the ground: `shell_in_water`, and its heavy version.
    water: [Option<mc_data::SoundId>; 2],
    /// `rain_light` and `rain_heavy` loops; `thunder_near` and `thunder_far`.
    rain: [Option<mc_data::SoundId>; 2],
    thunder: [Option<mc_data::SoundId>; 2],
}

struct UnitSoundIds {
    death: Option<mc_data::SoundId>,
    moving: Option<mc_data::SoundId>,
    /// A walker's footfall, and the ground one of its steps covers (half the model's stride):
    /// the same figure the vertex shader plants the feet by, so the two keep time. A core
    /// mine's hammer blow, one per blow its `gait` counts.
    step: Option<(mc_data::SoundId, f32)>,
    select: Option<mc_data::SoundId>,
    weapons: Vec<WeaponSoundIds>,
}

struct WeaponSoundIds {
    fire: Option<mc_data::SoundId>,
    /// A held beam's loop (`Weapon::beam`), heard while it fires instead of `fire`.
    hold: Option<mc_data::SoundId>,
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
    /// Who had a repair beam on last tick, and where its emitter was.
    mending: std::collections::HashMap<u32, [f32; 3]>,
    /// Who had a construction beam on last tick, and where it met the work.
    welding: std::collections::HashMap<u32, ([f32; 3], u8)>,
    /// Capital ships' drives, heard from their motion (audio/capital.rs).
    capital_sounds: crate::audio::capital::CapitalSounds,
    /// Giants' far footfalls, the biggest guns' far hits, rotary spin-ups (audio/titan.rs).
    giant_sounds: crate::audio::titan::GiantSounds,
    /// A salvo of strategic missiles heard as one (audio/salvo.rs).
    salvo_sounds: crate::audio::salvo::SalvoAudio,
    /// The score's tension, cues and ending (game_music.rs).
    music: music_notes::MatchMusic,
    /// The local player's faction key: which battle song plays.
    music_faction: String,
    rings: Rings,
    camera: Camera,
    sim: SimHandle,
    view: View,
    hud: Hud,
    serial: u64,
    published_at: Instant,
    /// Seconds the GPU glides between the last two ticks. Late ticks stretch
    /// this to the gap they really arrived at, so units do not freeze then jump.
    interp_span: f32,
    cursor: Vec2,
    keys: HashSet<KeyCode>,
    shift: bool,
    ctrl: bool,
    alt: bool,
    /// Ctrl and Alt are both down, and the free camera has been toggled for this press.
    free_chord: bool,
    /// The free camera's flight, and its hand-back to the strategic camera (`cine.rs`).
    cine: crate::cine::Cine,
    /// Free-camera input gathered between frames (`game_cine.rs`).
    cine_look: Vec2,
    cine_orbit: Vec2,
    cine_dolly: f32,
    cine_throttle: f32,
    cine_drag: Vec2,
    /// The ground under the pointer, for F and a lock-on click.
    cine_ground: Option<Vec3>,
    /// How far between ticks the last frame was drawn, so a hand-off lands on it.
    cine_alpha: f32,
    right_down: bool,
    /// The window sends raw mouse motion: mouse-look uses it.
    raw_motion: bool,
    /// When the pointer last moved; the free camera hides it once still.
    pointer_moved: Instant,
    left_down: Option<Vec2>,
    /// World site of a Place-mode press, so a drag can line buildings from there.
    place_from: Option<FxVec2>,
    /// Orders on the map: their lines, the ghosts of planned structures, the one being dragged.
    orders: OrderMap,
    /// What a click would do right now; the application shows it as the mouse cursor.
    pointer: Pointer,
    middle_down: bool,
    /// Unit the camera is following; a pan clears it.
    track: Option<u32>,
    /// Yaw, tilt and focus as they were when Alt went down, restored when it comes up.
    orbit_saved: Option<(f32, f32, Vec3)>,
    /// Unit to swing around while Alt is held, if the orbit picked one.
    orbit_unit: Option<u32>,
    /// Yaw and tilt the orbit is easing toward while Alt is held.
    orbit_aim: Option<(f32, f32)>,
    /// Focus at the moment Alt went down; blended toward the pivot so the retarget is not a jump.
    orbit_from: Option<Vec3>,
    /// Ground point to swing around when the orbit did not pick a unit.
    orbit_pivot: Option<Vec3>,
    /// 0 when Alt went down, easing toward 1 as the focus slides onto the pivot.
    orbit_blend: f32,
    /// Yaw, tilt and focus the camera is drifting back to after Alt came up.
    orbit_return: Option<(f32, f32, Vec3)>,
    /// This frame's share of the remaining return, for the tracked-unit focus.
    orbit_return_share: f32,
    /// Where the focus was last frame while its height was eased to the ground
    /// (`ease_focus_height`): a jump from there snaps the height instead.
    focus_eased_at: Option<Vec2>,
    /// While tracking: the focus has not yet slid back onto the tracked unit.
    orbit_return_far: bool,
    /// The drift home still brings the focus back. A pan or a jump hands the focus
    /// to the player but the angle keeps easing home: stopping it there froze
    /// whatever low look the orbit had reached, and every zoom after kept it.
    orbit_return_focus: bool,
    /// The distance the wheel asked for, while the camera is still easing to it.
    zoom_target: Option<f32>,
    /// How fast the log of the distance is changing, carried across wheel notches.
    zoom_velocity: f32,
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
    /// How hard it rains where the camera looks (the renderer's weather), 0 to 1.
    rain_here: f32,
    /// The living world heard round the camera (ambience.rs).
    ambience: crate::ambience::Ambience,
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
        let watching = start.session.length();
        let sim = sim_thread::spawn(
            SimSetup {
                map: start.map.clone(),
                blueprints: blueprints.clone(),
                pool,
                prefetched: start.prefetched,
                scene: start.scene,
                net: start.net.map(|rejoin| {
                    let local = (!start.observing).then_some(start.local);
                    crate::netplay::pair(Some(rejoin), local, start.keep)
                }),
                recorder: start.recorder,
            },
            start.session,
        );
        let size = Vec2::from(start.map.info().size_metres().to_f32());
        let mut camera = Camera::new(size, viewport);
        if !start.observing {
            if let Some(at) = start.map.start_positions().get(start.start_index) {
                camera.focus = Vec2::from(at.to_f32()).extend(0.0);
                camera.distance = 700.0;
            }
        }
        let mut view = View::new(start.local, start.colors, show_profiler);
        view.observing = start.observing;
        {
            let (sites, map) = (view.sites.clone(), start.map.clone());
            std::thread::spawn(move || match mc_sim::placement::SiteMap::for_map(&map) {
                Ok(built) => {
                    let _ = sites.set(built);
                }
                Err(e) => log::warn!("no placement sites for this map: {e}"),
            });
        }
        if let Some(subject) = start.range {
            let pad = crate::setup::range_pad(&start.map);
            // The pad clear of the panel on the left, the firing lane running off to the right.
            camera.focus =
                Vec2::from(pad.to_f32()).extend(0.0) + Vec3::new(RANGE_LOOK_AHEAD, 0.0, 0.0);
            camera.distance = range::ZOOMS[1];
            view.range = Some(Range::new(pad, subject));
        }
        *sim.seek.lock().unwrap() = start.seek;
        let mut hud = Hud::default();
        if let (Some(_), Some(record)) = (watching, &start.record) {
            hud.replay_bar = crate::hud::ReplayBar::watching(record.id.clone());
        }
        hud.issues = crate::hud::IssueMark::new(start.record);
        Game {
            map: start.map,
            rings: Rings::new(&blueprints),
            blueprints,
            sounds: None,
            beaming: Default::default(),
            mending: Default::default(),
            welding: Default::default(),
            capital_sounds: Default::default(),
            giant_sounds: Default::default(),
            salvo_sounds: Default::default(),
            music: Default::default(),
            music_faction: start
                .roster
                .get(start.local as usize)
                .map(|p| p.faction.clone())
                .unwrap_or_default(),
            camera,
            sim,
            view,
            hud,
            serial: 0,
            published_at: Instant::now(),
            interp_span: TICK_SECONDS,
            cursor: Vec2::ZERO,
            keys: HashSet::new(),
            shift: false,
            ctrl: false,
            alt: false,
            free_chord: false,
            cine: Default::default(),
            cine_look: Vec2::ZERO,
            cine_orbit: Vec2::ZERO,
            cine_dolly: 0.0,
            cine_throttle: 0.0,
            cine_drag: Vec2::ZERO,
            cine_ground: None,
            cine_alpha: 1.0,
            right_down: false,
            raw_motion: false,
            pointer_moved: Instant::now(),
            left_down: None,
            place_from: None,
            orders: OrderMap::default(),
            pointer: Pointer::Arrow,
            middle_down: false,
            track: None,
            orbit_saved: None,
            orbit_unit: None,
            orbit_aim: None,
            orbit_from: None,
            orbit_pivot: None,
            orbit_blend: 0.0,
            orbit_return: None,
            orbit_return_share: 0.0,
            focus_eased_at: None,
            orbit_return_far: false,
            orbit_return_focus: false,
            zoom_target: None,
            zoom_velocity: 0.0,
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
            rain_here: 0.0,
            ambience: Default::default(),
            event: None,
        }
    }

    /// The minimap chart and the unit pictures, drawn ahead on the loading
    /// thread, so the first frame of the match has nothing heavy left to do.
    pub fn preload(&mut self, overlay: &mut Overlay, chart: &[u8], thumbs: hud::thumbs::Baked) {
        overlay.set_image(
            hud::MINIMAP_SLOT,
            ui::preview::SIZE,
            ui::preview::SIZE,
            chart,
        );
        self.chart_ready = true;
        self.hud.thumbs.install(overlay, thumbs);
    }

    /// The side whose colours the unit pictures are drawn in.
    pub fn picture_team(start: &GameStart) -> [f32; 3] {
        start.colors[start.local as usize % 8]
    }

    /// A failure the application ran into on the match's behalf (a reload that did not parse).
    pub fn complain(&mut self, message: &str) {
        self.hud.toast(message, palette::BAD);
    }

    pub fn pointer(&self) -> Pointer {
        self.pointer
    }

    pub fn tick(&self) -> u32 {
        self.view.status.tick
    }

    pub fn camera(&self) -> &Camera {
        &self.camera
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
                fresh: true,
                settings: None,
            });
            // Held keys and drags must not carry on underneath the menu.
            self.keys.clear();
            self.left_down = None;
            self.place_from = None;
            self.orders.cancel();
            self.middle_down = false;
            self.end_orbit();
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent, r: &Renderer, audio: &Audio) {
        // Keys typed into the issue note are not orders.
        if self.hud.issues.typing() && matches!(event, WindowEvent::KeyboardInput { .. }) {
            self.keys.clear();
            return;
        }
        // The browser owns input, including keys that normally issue orders.
        if self.hud.unit_picker_open() {
            self.keys.clear();
            self.middle_down = false;
            self.end_orbit();
            // Still track the pointer and modifiers so closing the browser cannot
            // leave Shift held or the battlefield cursor at an old position.
            if !matches!(
                event,
                WindowEvent::CursorMoved { .. } | WindowEvent::ModifiersChanged(_)
            ) {
                return;
            }
        }
        // Track modifiers and the pointer always; act on them only when the menu is down.
        match event {
            // Keys let go in another window never send their release here: a held
            // Page Up would keep tilting the view down to the horizon.
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.middle_down = false;
                self.alt = false;
                self.free_chord = false;
                self.right_down = false;
                self.end_orbit();
            }
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
                self.view.shift = self.shift;
                // A patrol is laid post by post while shift is down; letting go finishes it.
                if !self.shift
                    && self.view.mode == Mode::Target(Targeting::Patrol)
                    && !(self.view.patrol_posts.is_empty() && self.view.patrol_inserts.is_empty())
                {
                    self.view.patrol_posts.clear();
                    self.view.patrol_inserts.clear();
                    self.view.mode = Mode::Normal;
                }
                self.ctrl = m.state().control_key();
                self.view.ctrl = self.ctrl;
                let alt = m.state().alt_key();
                // Ctrl+Alt together frees the camera or gives the panels back; that
                // press does not orbit, whichever key went down first.
                let chord = self.ctrl && alt;
                if chord && !self.free_chord && self.menu.is_none() && !self.hud.unit_picker_open()
                {
                    self.set_free_camera(!self.hud.free.on, r, audio);
                }
                self.free_chord = chord;
                if alt != self.alt {
                    self.alt = alt;
                    if self.menu.is_some() || self.hud.unit_picker_open() {
                        if !alt {
                            self.end_orbit();
                        }
                    } else if alt {
                        if !chord && !self.hud.free.on {
                            self.begin_orbit(r);
                        }
                    } else {
                        self.end_orbit();
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = Vec2::new(position.x as f32, position.y as f32);
                let delta = p - self.cursor;
                if delta != Vec2::ZERO {
                    self.pointer_moved = Instant::now();
                }
                if self.hud.free.on {
                    if self.menu.is_none() {
                        self.free_camera_motion(delta, false);
                    }
                } else if self.menu.is_none() && !self.hud.unit_picker_open() {
                    if let Some((yaw, tilt)) = &mut self.orbit_aim {
                        *yaw += delta.x * ORBIT_YAW;
                        let (lo, hi) = self.camera.tilt_limits();
                        *tilt = (*tilt - delta.y * ORBIT_TILT).clamp(lo, hi);
                    } else if self.middle_down {
                        self.pan_camera(delta);
                    }
                }
                self.cursor = p;
            }
            _ if self.menu.is_some() => {}
            // Over the HUD the wheel scrolls the HUD, which reads it from the interface's input.
            WindowEvent::MouseWheel { .. } if self.hud.covers(self.cursor) => {}
            WindowEvent::MouseWheel { delta, .. } if self.hud.free.on => {
                self.free_camera_wheel(match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                });
            }
            WindowEvent::MouseInput { state, button, .. } if self.hud.free.on => {
                self.pointer_moved = Instant::now();
                self.free_camera_button(*button, *state == ElementState::Pressed, audio)
            }
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
                self.zoom_anchor = if self.orbit_saved.is_some()
                    || self.orbit_return.is_some()
                    || self.track.is_some()
                {
                    None
                } else {
                    self.ground_under_cursor(r).map(|g| (g, self.cursor))
                };
                self.zoom_focus = self.camera.focus.truncate();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.mouse_button(*button, *state == ElementState::Pressed, r, audio)
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if event.state == ElementState::Pressed {
                        if self.keys.insert(code) {
                            self.key_pressed(code, r, audio);
                        }
                    } else {
                        self.keys.remove(&code);
                    }
                }
            }
            _ => {}
        }
    }

    /// Carries the camera to the asked distance on a critically damped spring, in
    /// log space so it feels the same at every height. The spring's velocity
    /// survives a new wheel notch, so a run of notches is one smooth glide. The
    /// step is the exact solution, so it is the same at every frame rate.
    fn ease_zoom(&mut self, dt: f32) {
        let Some(target) = self.zoom_target else {
            self.zoom_velocity = 0.0;
            return;
        };
        let target = target.clamp(MIN_DISTANCE, self.camera.max_distance());
        if self.camera.focus.truncate() != self.zoom_focus {
            self.zoom_anchor = None;
        }
        let offset = (self.camera.distance / target).ln();
        if offset.abs() < 1e-3 && self.zoom_velocity.abs() < 1e-2 {
            self.camera
                .zoom(target / self.camera.distance, self.zoom_anchor);
            self.zoom_target = None;
            self.zoom_velocity = 0.0;
            return;
        }
        let decay = (-ZOOM_RATE * dt).exp();
        let push = (self.zoom_velocity + ZOOM_RATE * offset) * dt;
        self.zoom_velocity = (self.zoom_velocity - ZOOM_RATE * push) * decay;
        let next = (offset + push) * decay;
        self.camera.zoom((next - offset).exp(), self.zoom_anchor);
        // Held at a clamp, the spring must not keep pushing into it.
        if ((self.camera.distance / target).ln() - next).abs() > 1e-4 {
            self.zoom_velocity = 0.0;
        }
        self.zoom_focus = self.camera.focus.truncate();
    }

    fn send(&self, command: Command) {
        if self.view.observing {
            return;
        }
        // The sim thread is gone only after a fatal error, which the HUD already shows.
        let command = match command {
            Command::Move {
                units,
                target,
                queue,
            } => Command::FormationMove {
                units,
                target,
                queue,
                attack_move: false,
                together: self.view.formation_together,
                spacing: self.view.formation_spacing,
            },
            Command::AttackMove {
                units,
                target,
                queue,
            } => Command::FormationMove {
                units,
                target,
                queue,
                attack_move: true,
                together: self.view.formation_together,
                spacing: self.view.formation_spacing,
            },
            other => other,
        };
        // Guards (aircraft circling) and patrols are laid out together at standard
        // spacing; other settings are applied to them straight after.
        let reform = match &command {
            Command::Guard { units, .. } | Command::Patrol { units, .. }
                if !self.view.formation_together || self.view.formation_spacing != 1 =>
            {
                Some(Command::Reform {
                    units: units.clone(),
                    together: self.view.formation_together,
                    spacing: self.view.formation_spacing,
                })
            }
            _ => None,
        };
        // A new order takes its units out of the groups they were in; one given to a group
        // makes a new group, which the badges pick up from the queues. A queued order
        // leaves them where they are until it comes up.
        match &command {
            Command::FormationMove {
                units,
                queue: false,
                ..
            }
            | Command::Move {
                units,
                queue: false,
                ..
            }
            | Command::AttackMove {
                units,
                queue: false,
                ..
            }
            | Command::Attack {
                units,
                queue: false,
                ..
            }
            | Command::Stop { units }
            | Command::Build {
                units,
                queue: false,
                ..
            }
            | Command::Assist {
                units,
                queue: false,
                ..
            }
            | Command::ReclaimWreck { units, .. }
            | Command::ReclaimUnit { units, .. }
            | Command::AttackGround {
                units,
                queue: false,
                ..
            }
            | Command::Strike {
                units,
                queue: false,
                ..
            }
            | Command::Bombard {
                units,
                queue: false,
                ..
            }
            | Command::Patrol {
                units,
                queue: false,
                ..
            }
            | Command::Guard {
                units,
                queue: false,
                ..
            } => self.orders.ordered(units),
            _ => {}
        }
        let _ = self.sim.commands.send(command);
        if let Some(reform) = reform {
            let _ = self.sim.commands.send(reform);
        }
    }

    fn ground_under_cursor(&self, r: &Renderer) -> Option<Vec3> {
        let (origin, dir) = self.camera.ray(self.cursor);
        r.pick_surface(origin, dir)
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

    /// A live, identified unit the player may select to read, including an enemy's.
    fn can_inspect(&self, u: &UnitInstance) -> bool {
        u.owner_flags & KIND_WRECK == 0
            && u.owner_flags & STATE_UNIDENTIFIED == 0
            && self
                .blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .visual
                .mesh
                != "reclaim_drone"
    }

    /// The unit (or wreck) drawn nearest to a pixel, its body or its icon (`pick.rs`).
    fn unit_at(&self, pixel: Vec2) -> Option<usize> {
        crate::pick::unit_at(
            &self.view.frame.units,
            &self.blueprints,
            &self.camera,
            pixel,
        )
    }

    fn unit_mark(&self, i: usize, hover: bool) -> Mark {
        let u = &self.view.frame.units[i];
        let unknown = u.owner_flags & STATE_UNIDENTIFIED != 0;
        Mark {
            unit_index: i as u32,
            kind: mark_kind(hover, self.is_enemy((u.owner_flags & 0xFF) as u8)),
            // Like a factory's, an enemy launcher's assembly line is not shown.
            work: if unknown
                || (self.is_enemy((u.owner_flags & 0xFF) as u8)
                    && crate::hud::silo::Launcher::of(u).is_some())
            {
                -1.0
            } else {
                unit_bar_work(u, &self.view.status.queues)
            },
            shield: if unknown {
                -1.0
            } else {
                unit_bar_shield(u.unit_id, &self.view.frame.shields)
            },
        }
    }

    /// Who orders go to.
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

    /// Everyone who would carry out an order given to the selection: the units, and the
    /// units the selected factories make, who take their factory's orders.
    fn selection_takers(&self) -> impl Iterator<Item = &mc_data::UnitBlueprint> {
        self.selected_units().flat_map(|u| {
            hud::ordered_as(
                &self.blueprints,
                self.blueprints.unit(BlueprintId(u.blueprint as u16)),
            )
        })
    }

    /// A lift ship (`transport`) is among the selection.
    fn selection_lifts(&self) -> bool {
        self.selection_takers().any(|b| b.transport.is_some())
    }

    /// Whether the selection holds a capital ship, which sets down only when told to.
    fn selection_lands(&self) -> bool {
        self.selection_takers().any(|b| b.is_capital_ship())
    }

    /// Selected silos of the player's own holding a warhead no launch has spoken for.
    fn armed_silos(&self) -> Vec<u32> {
        crate::hud::silo::armed_silos(&self.view, &self.blueprints)
    }

    /// Over one of the player's own lift ships with land units selected: what boarding
    /// would take, and what will not fit (`hud::cargo::board_hint`).
    fn board_hint(&self, ui: &mut crate::ui::Ui) {
        let Some(ship) = self.unit_at(self.cursor).map(|i| &self.view.frame.units[i]) else {
            return;
        };
        let riders: Vec<BlueprintId> = self
            .selected_units()
            .filter(|u| u.unit_id != ship.unit_id)
            .map(|u| BlueprintId(u.blueprint as u16))
            .collect();
        let free = self
            .view
            .status
            .queues
            .iter()
            .find(|q| q.unit_id == ship.unit_id)
            .and_then(|q| q.cargo.as_ref())
            .map(|c| c.capacity.saturating_sub(c.used));
        let bp = self.blueprints.unit(BlueprintId(ship.blueprint as u16));
        hud::cargo::board_hint(ui, &self.blueprints, bp, &riders, free);
    }

    /// The selected lift ships.
    fn selected_lifts(&self) -> Vec<Handle> {
        self.selected_units()
            .filter(|u| {
                self.blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .is_capital_ship()
            })
            .map(|u| Handle(u.unit_id))
            .collect()
    }

    /// Selected capital ships set down where they are (`unload`: and lift ships let
    /// their holds out).
    fn lift_here(&mut self, unload: bool) {
        let ships: Vec<(Handle, FxVec2)> = self
            .selected_units()
            .filter(|u| {
                self.blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .is_capital_ship()
            })
            .map(|u| {
                (
                    Handle(u.unit_id),
                    FxVec2::new(Fx::from_f32(u.pos[0]), Fx::from_f32(u.pos[1])),
                )
            })
            .collect();
        for (ship, pos) in ships {
            self.send(Command::Land {
                units: vec![ship],
                pos,
                unload,
                queue: false,
            });
        }
    }

    /// Shift+L: selected capital ships that are down (or coming down) take off; if none
    /// is, they all set down where they are.
    fn lift_toggle(&mut self) {
        let down = self.selected_units().any(|u| {
            u.set_down()
                || self
                    .view
                    .status
                    .queues
                    .iter()
                    .find(|q| q.unit_id == u.unit_id)
                    .and_then(|q| q.cargo.as_ref())
                    .is_some_and(|c| {
                        use mc_sim::mirror::LiftPhase;
                        matches!(
                            c.phase,
                            LiftPhase::RampOpening | LiftPhase::Ready | LiftPhase::Unloading
                        )
                    })
        });
        if down {
            let ships = self.selected_lifts();
            self.send(Command::TakeOff { units: ships });
        } else {
            self.lift_here(false);
        }
    }

    /// Something in the selection moves, or makes units that do.
    fn selection_goes(&self) -> bool {
        self.selection_has(cat::MOBILE) || self.selection_has(cat::FACTORY)
    }

    fn selection_has(&self, categories: u32) -> bool {
        self.selected_units().any(|u| {
            self.blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .has(categories)
        })
    }

    fn mouse_button(&mut self, button: MouseButton, pressed: bool, r: &Renderer, audio: &Audio) {
        if self.orbit_saved.is_some() {
            return;
        }
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
                        if matches!(
                            self.view.mode,
                            Mode::Target(Targeting::Bombard | Targeting::Guard)
                        ) {
                            self.view.circle_from =
                                self.ground_under_cursor(r).map(|g| g.truncate());
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
                } else if let Some(pos) = (self.view.mode == Mode::Normal)
                    .then(|| {
                        self.orders.patrol_post_at(
                            &Field {
                                view: &self.view,
                                blueprints: &self.blueprints,
                                map: &self.map,
                                camera: &self.camera,
                                renderer: r,
                            },
                            self.cursor,
                        )
                    })
                    .flatten()
                {
                    // A right-click on one of the selection's patrol posts takes it out of the route.
                    audio.play(Sfx::Back);
                    self.send(Command::CancelOrder {
                        units: self.selected_ids(),
                        kind: OrderKind::Patrol,
                        pos,
                    });
                } else if self.view.mode != Mode::Normal {
                    self.view.patrol_posts.clear();
                    self.view.patrol_inserts.clear();
                    self.view.circle_from = None;
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
                    .filter(|(_, fit)| fit.is_ok())
                    .map(|(pos, _)| pos)
                    .collect();
                if valid.is_empty() {
                    audio.play(Sfx::Deny);
                } else {
                    audio.play(Sfx::Order);
                    let units = self.selected_ids();
                    let heading = self.blueprints.unit(blueprint).build_heading();
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
            Mode::Target(Targeting::Patrol) => match self.ground_under_cursor(r) {
                Some(g) => self.lay_patrol_post(g.truncate(), audio),
                None => audio.play(Sfx::Deny),
            },
            Mode::Target(Targeting::Guard) => {
                // Centred, and going with the friendly unit there, where the press landed.
                let centre = self.view.circle_from.take();
                let edge = self.ground_under_cursor(r).map(|g| g.truncate());
                let mut command =
                    self.targeted_command(Targeting::Guard, centre, self.unit_at(from));
                // A click keeps the area the selection has; a drag sets it.
                if let (Some(Command::Guard { radius, .. }), Some(centre), Some(edge)) =
                    (&mut command, centre, edge)
                {
                    if from.distance(self.cursor) >= DRAG_THRESHOLD {
                        *radius = Fx::from_f32(
                            centre
                                .distance(edge)
                                .clamp(MIN_GUARD_RADIUS.to_f32(), MAX_GUARD_RADIUS.to_f32()),
                        );
                    }
                }
                match command {
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
            Mode::Target(Targeting::Bombard) => {
                let ground = self.ground_under_cursor(r).map(|g| g.truncate());
                if let (Some(centre), Some(edge)) = (self.view.circle_from.take(), ground) {
                    let radius = crate::orders::bombard_radius(
                        self.selection_takers(),
                        centre.distance(edge),
                    );
                    audio.play(Sfx::Order);
                    self.send(Command::Bombard {
                        units: self.selected_ids(),
                        pos: FxVec2::new(Fx::from_f32(centre.x), Fx::from_f32(centre.y)),
                        radius: Fx::from_f32(radius),
                        queue: self.shift,
                    });
                    if !self.shift {
                        self.view.mode = Mode::Normal;
                    }
                } else {
                    audio.play(Sfx::Deny);
                }
            }
            Mode::Target(targeting) => {
                let ground = self.ground_under_cursor(r).map(|g| g.truncate());
                self.targeted_order(targeting, ground, self.unit_at(self.cursor), audio);
            }
            Mode::Spawn | Mode::SpawnSubject => {
                let ground = self
                    .ground_under_cursor(r)
                    .map(|g| FxVec2::new(Fx::from_f32(g.x), Fx::from_f32(g.y)));
                match (ground, &self.view.range) {
                    (Some(pos), Some(range)) => {
                        let copies = if self.view.mode == Mode::SpawnSubject {
                            vec![range.spawn_at(pos, &self.blueprints)]
                        } else {
                            range.duplicate_at(
                                pos,
                                &self.view.frame.units,
                                &self.view.selection,
                                &self.blueprints,
                            )
                        };
                        if copies.is_empty() {
                            audio.play(Sfx::Deny);
                        } else {
                            audio.play(Sfx::Order);
                            for command in copies {
                                self.send(command);
                            }
                        }
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
                // Watching: any commander's units, none of them ours to order.
                let sides: Vec<u8> = if self.view.observing {
                    (0..self.view.status.players.len().max(1) as u8).collect()
                } else if self.view.range.is_none() {
                    vec![self.view.local]
                } else if self.view.local == range::BLUE {
                    vec![range::BLUE, range::RED]
                } else {
                    vec![range::RED, range::BLUE]
                };
                let mut side = self.view.local;
                let now = Instant::now();
                let badge = (from.distance(self.cursor) < DRAG_THRESHOLD)
                    .then(|| {
                        self.orders.group_at(
                            &Field {
                                view: &self.view,
                                blueprints: &self.blueprints,
                                map: &self.map,
                                camera: &self.camera,
                                renderer: r,
                            },
                            self.cursor,
                        )
                    })
                    .flatten();
                if let Some(members) = badge {
                    // A group's badge: the units walking under it, together again.
                    audio.play(Sfx::Tick);
                    self.last_pick = None;
                    picked = members;
                } else if from.distance(self.cursor) < DRAG_THRESHOLD {
                    let mut click_kind = None;
                    let mut inspect = false;
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
                        } else if self.can_inspect(u) {
                            // A live enemy: look at it. Shift does not fold it into the army.
                            picked = vec![u.unit_id];
                            inspect = true;
                        }
                    }
                    self.last_pick = click_kind.map(|(bp, own)| (bp, own, now));
                    if inspect {
                        self.last_pick = None;
                    }
                } else {
                    self.last_pick = None;
                    for &owner in &sides {
                        picked = self.box_pick(from.min(self.cursor), from.max(self.cursor), owner);
                        if !picked.is_empty() {
                            side = owner;
                            break;
                        }
                    }
                }
                if !self.view.observing && side != self.view.local {
                    self.take_control(side);
                }
                if self.view.observing {
                    self.view.local = side;
                }
                let inspecting = picked.iter().any(|id| {
                    self.view
                        .index_of
                        .get(id)
                        .and_then(|&i| self.view.frame.units.get(i))
                        .is_some_and(|u| self.is_enemy((u.owner_flags & 0xFF) as u8))
                });
                if inspecting {
                    self.view.selection = picked;
                } else if self.shift {
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
            if !self.can_inspect(u)
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

    /// Test range: clear it and line up one of each of `units`, then look at them.
    fn range_line_up(&mut self, units: &[mc_data::BlueprintId], audio: &Audio) {
        let Some(range) = self.view.range.as_mut() else {
            return;
        };
        if units.is_empty() {
            audio.play(Sfx::Deny);
            return;
        }
        let bps = &self.blueprints;
        let wet = units.iter().any(|&id| {
            let bp = bps.unit(id);
            bp.water_only()
                || bp
                    .motion
                    .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval)
        });
        let sea = wet
            .then(|| crate::setup::sea_near(&self.map, range.pad))
            .flatten();
        let lined = range.line_up(bps, units, sea, self.map.info().size_metres());
        for command in lined.commands {
            self.send(command);
        }
        self.view.selection.clear();
        self.view.local = range::BLUE;
        self.view.mode = Mode::Normal;
        self.zoom_target = Some(
            (lined.span * 0.9)
                .clamp(range::ZOOMS[1], range::ZOOMS[2])
                .min(self.camera.max_distance()),
        );
        self.zoom_anchor = None;
        self.track = None;
        self.cancel_orbit_return();
        self.camera.focus = Vec2::from(lined.centre.to_f32()).extend(self.camera.focus.z);
        let placed = units.len() - lined.stranded;
        let mut note = format!("Lined Up {placed} Units");
        if lined.at_sea > 0 {
            note += &format!("  \u{b7}  {} At Sea", lined.at_sea);
        }
        if lined.stranded > 0 {
            note += &format!("  \u{b7}  {} Left Out: No Open Sea", lined.stranded);
        }
        self.hud.toast(&note, palette::ACCENT);
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
                range.subject = range::step_subject(&self.blueprints, range.subject, step);
                range.spawn = range.subject;
            }
            RangeAction::PickSubject(id) => {
                range.subject = id;
                range.spawn = id;
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
            RangeAction::ArmSubject => {
                self.view.mode = if self.view.mode == Mode::SpawnSubject {
                    Mode::Normal
                } else {
                    Mode::SpawnSubject
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
                self.hud.toast("Range Reset", palette::ACCENT);
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
                self.track = None;
                self.cancel_orbit_return();
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
            RangeAction::Sky(sky) => range.sky = Some(sky),
            RangeAction::Stock {
                player,
                mass,
                energy,
            } => commands.push(Command::DebugStock {
                player,
                mass,
                energy,
            }),
            RangeAction::Income {
                player,
                resource,
                step,
            } => {
                let at = &mut range.income[player.min(1) as usize][resource.min(1)];
                *at = (*at as i32 + step).clamp(0, range::INCOME_STEPS.len() as i32 - 1) as usize;
                commands.push(range.income_command(player));
            }
            RangeAction::SetIncome {
                player,
                resource,
                index,
            } => {
                range.income[player.min(1) as usize][resource.min(1)] =
                    index.min(range::INCOME_STEPS.len() - 1);
                commands.push(range.income_command(player));
            }
            RangeAction::Storage { player, step } => {
                let at = &mut range.storage[player.min(1) as usize];
                *at = (*at as i32 + step).clamp(0, range::STORAGE_STEPS.len() as i32 - 1) as usize;
                commands.push(range.storage_command(player));
            }
            RangeAction::Wrecks => match range.wrecks(&self.blueprints) {
                Some(c) => commands.push(c),
                None => audio.play(Sfx::Deny),
            },
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
            Targeting::Attack => match target.filter(|u| {
                !is_wreck(u)
                    && self.is_enemy((u.owner_flags & 0xFF) as u8)
                    && self.selection_can_attack(u)
            }) {
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
            Targeting::Patrol => point.map(|p| Command::Patrol {
                units,
                points: vec![p],
                queue,
            }),
            Targeting::AttackGround => point.map(|pos| Command::AttackGround { units, pos, queue }),
            Targeting::Strike => point.map(|pos| Command::Strike { units, pos, queue }),
            // One warhead a click; the sim gives it to the silo with the most free.
            Targeting::Nuke => point.and_then(|pos| {
                let silos = self.armed_silos();
                (!silos.is_empty()).then(|| Command::LaunchNuke {
                    units: silos.into_iter().map(Handle).collect(),
                    pos,
                })
            }),
            Targeting::Land | Targeting::Unload => point.map(|pos| Command::Land {
                units,
                pos,
                unload: targeting == Targeting::Unload,
                queue,
            }),
            Targeting::Guard => point.map(|pos| Command::Guard {
                units,
                pos,
                target: target
                    .filter(|u| !is_wreck(u) && !self.is_enemy((u.owner_flags & 0xFF) as u8))
                    .map_or(Handle::NONE, |u| Handle(u.unit_id)),
                radius: Fx::from_f32(self.guard_radius()),
                queue,
            }),
            Targeting::Bombard => point.map(|pos| Command::Bombard {
                units,
                pos,
                radius: Fx::from_f32(crate::orders::BOMBARD_MIN),
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
                if let (Command::LaunchNuke { pos, .. }, Some(g)) = (&command, ground) {
                    // Counted against the silo it should come from until the frame shows it.
                    let at = Vec2::new(pos.x.to_f32(), pos.y.to_f32());
                    if let Some(silo) = crate::hud::silo::next_silo(&self.view, &self.blueprints, g)
                    {
                        let sent = crate::hud::silo::SentLaunch {
                            silo: silo.unit_id,
                            at,
                            tick: self.view.frame.tick,
                        };
                        self.view.nuke_sent.push(sent);
                    }
                }
                self.send(command);
                // Shift keeps aiming while warheads are left to give out.
                let spent = targeting == Targeting::Nuke && self.armed_silos().is_empty();
                if !self.shift || spent {
                    self.view.mode = Mode::Normal;
                }
            }
            None => audio.play(Sfx::Deny),
        }
    }

    /// What a right click does: attack, reclaim, assist, copy another factory's orders,
    /// or else move (a factory's rally point), depending on what is there and on who is selected. The pointer asks too.
    fn selection_can_attack(&self, target: &UnitInstance) -> bool {
        let target = self.blueprints.unit(BlueprintId(target.blueprint as u16));
        self.selection_takers().any(|b| b.can_attack(target))
    }

    fn context_command(&self, ground: Option<Vec2>, unit: Option<usize>) -> Option<Command> {
        if self.view.selection.is_empty() {
            return None;
        }
        let units = self.selected_ids();
        let queue = self.shift;
        let builders = self
            .selection_takers()
            .any(|b| b.is_mobile() && b.builder.is_some());
        let armed = self.selection_takers().any(|b| !b.weapons.is_empty());
        let reclaimers = self.selected_units().any(|u| {
            self.blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .sends_reclaimers()
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
                if armed && self.selection_can_attack(&u) {
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
            } else if builders
                && hud::has_flag(&u, flag::UNDER_CONSTRUCTION)
                && !self.view.selection.contains(&u.unit_id)
            {
                // A site still going up: finish it, whatever it will be.
                return Some(Command::Assist {
                    units,
                    target: Handle(u.unit_id),
                    queue,
                });
            } else if owner == self.view.local
                && self.selection_has(cat::FACTORY)
                && !self.selection_has(cat::MOBILE)
                && !self.view.selection.contains(&u.unit_id)
                && self
                    .blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .has(cat::FACTORY)
            {
                // Factory on factory: take its orders for what this one makes.
                return Some(Command::CopyFactoryOrders {
                    factories: units,
                    from: Handle(u.unit_id),
                });
            } else if owner == self.view.local
                && self
                    .blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .transport
                    .is_some()
                && !self.view.selection.contains(&u.unit_id)
                && self.selection_takers().any(|b| b.cargo_room().is_some())
            {
                // Land units onto their own lift ship: up the ramp into the hold.
                return Some(Command::Board {
                    units,
                    carrier: Handle(u.unit_id),
                    queue,
                });
            } else if builders && !self.view.selection.contains(&u.unit_id) {
                return Some(Command::Assist {
                    units,
                    target: Handle(u.unit_id),
                    queue,
                });
            }
        }
        let target = ground.map(|g| FxVec2::new(Fx::from_f32(g.x), Fx::from_f32(g.y)))?;
        // A factory hands the move to what it makes: its rally point, and shift queues more.
        self.selection_goes().then_some(Command::Move {
            units,
            target,
            queue,
        })
    }

    fn context_order(&mut self, ground: Option<Vec2>, unit: Option<usize>, audio: &Audio) {
        if self.view.observing {
            return;
        }
        if let Some(command) = self.context_command(ground, unit) {
            audio.play(Sfx::Order);
            self.send(command);
        }
    }

    /// The pointer for this frame: what a click here would do. `can_place` is
    /// whether the structure being placed (or the drag-line of them) can go down.
    fn pointer_for(&self, renderer: &Renderer, over_ui: bool, can_place: bool) -> Pointer {
        if self.orbit_saved.is_some() {
            return Pointer::Pan;
        }
        if over_ui {
            return Pointer::Arrow;
        }
        if self.middle_down {
            return Pointer::Pan;
        }
        if self.view.observing {
            let live = self
                .unit_at(self.cursor)
                .is_some_and(|i| self.view.frame.units[i].owner_flags & KIND_WRECK == 0);
            return if live {
                Pointer::Select
            } else {
                Pointer::Arrow
            };
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
            Some(Command::FormationMove {
                attack_move: true, ..
            }) => Pointer::AttackMove,
            Some(Command::FormationMove { .. }) => Pointer::Move,
            Some(Command::Assist { .. } | Command::CopyFactoryOrders { .. }) => Pointer::Assist,
            Some(Command::ReclaimWreck { .. } | Command::ReclaimUnit { .. }) => Pointer::Reclaim,
            Some(Command::Move { .. } | Command::Land { .. }) => Pointer::Move,
            Some(Command::Board { .. }) => Pointer::Board,
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
            Mode::Spawn | Mode::SpawnSubject => {
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
                        let inspectable =
                            unit.map(|i| &self.view.frame.units[i]).is_some_and(|u| {
                                self.can_inspect(u)
                                    && (self.is_enemy((u.owner_flags & 0xFF) as u8)
                                        || (u.owner_flags & 0xFF) as u8 == self.view.local
                                        || (self.view.range.is_some()
                                            && (u.owner_flags & 0xFF) as u8 <= range::RED))
                            });
                        if inspectable {
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

    /// The formation panel is for a selection of two or more with something that moves.
    fn formation_available(&self) -> bool {
        self.selected_units().count() > 1 && self.selection_has(cat::MOBILE)
    }

    fn toggle_formation_panel(&mut self, audio: &Audio) {
        if self.view.formation_panel || self.formation_available() {
            self.view.formation_panel = !self.view.formation_panel;
        } else {
            audio.play(Sfx::Deny);
        }
    }

    /// The formation settings changed: the selection's moves, patrols and orbits
    /// take them at once, not only the next orders.
    fn reform_selection(&self) {
        let units: Vec<_> = self
            .selected_units()
            .filter(|u| {
                self.blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .is_mobile()
            })
            .map(|u| Handle(u.unit_id))
            .collect();
        if !units.is_empty() {
            self.send(Command::Reform {
                units,
                together: self.view.formation_together,
                spacing: self.view.formation_spacing,
            });
        }
    }

    fn hud_action(&mut self, action: HudAction, audio: &Audio) {
        // Anything else done on the HUD puts the formation panel away.
        if !matches!(
            action,
            HudAction::FormationPanel
                | HudAction::FormationTogether(_)
                | HudAction::FormationSpacing(_)
                | HudAction::FormUp
        ) {
            self.view.formation_panel = false;
        }
        match action {
            HudAction::FormationPanel => self.toggle_formation_panel(audio),
            HudAction::FormationTogether(together) => {
                self.view.formation_together = together;
                self.reform_selection();
            }
            HudAction::FormationSpacing(spacing) => {
                self.view.formation_spacing = spacing.min(2);
                self.reform_selection();
            }
            HudAction::FormUp => {
                let selected: Vec<_> = self
                    .selected_units()
                    .filter(|u| {
                        self.blueprints
                            .unit(BlueprintId(u.blueprint as u16))
                            .is_mobile()
                    })
                    .collect();
                if !selected.is_empty() {
                    let sum = selected
                        .iter()
                        .fold(Vec2::ZERO, |a, u| a + Vec2::new(u.pos[0], u.pos[1]));
                    let center = sum / selected.len() as f32;
                    self.send(Command::FormationMove {
                        units: selected.iter().map(|u| Handle(u.unit_id)).collect(),
                        target: FxVec2::new(Fx::from_f32(center.x), Fx::from_f32(center.y)),
                        queue: false,
                        attack_move: false,
                        together: true,
                        spacing: self.view.formation_spacing,
                    });
                    self.hud
                        .toast("FORMING UP · AIR Vs / LAND BLOCKS", hud::HEALTHY);
                }
            }
            HudAction::Build(blueprint) => {
                let bp = self.blueprints.unit(blueprint);
                // An experimental is placed on a lot like a structure (`built_on_site`).
                if bp.built_on_site() {
                    self.hud.build_keys = false;
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
                self.view.patrol_posts.clear();
                self.view.patrol_inserts.clear();
                self.view.circle_from = None;
                self.view.mode = if self.view.mode == Mode::Target(targeting) {
                    Mode::Normal
                } else {
                    Mode::Target(targeting)
                }
            }
            HudAction::Upgrade => self.send(Command::Upgrade {
                units: self.selected_ids(),
            }),
            HudAction::Refit(kits) => {
                for kit in kits {
                    self.send(Command::Refit {
                        units: self.selected_ids(),
                        kit,
                    });
                }
            }
            HudAction::CancelRefit(kit) => self.send(Command::CancelRefit {
                units: self.selected_ids(),
                kit,
            }),
            HudAction::Stop => self.send(Command::Stop {
                units: self.selected_ids(),
            }),
            HudAction::Repeat(on) => self.send(Command::SetRepeat {
                factories: self.selected_ids(),
                repeat: on,
            }),
            HudAction::CancelOrder { kind, pos } => self.send(Command::CancelOrder {
                units: self.selected_ids(),
                kind,
                pos,
            }),
            HudAction::FireState(state) => self.set_fire_state(state),
            HudAction::Dive(dive) => self.set_dive(dive),
            HudAction::PauseWork(paused) => self.set_paused(paused),
            HudAction::Focus(focus) => self.send(Command::SetFocus { focus }),
            HudAction::UnloadHere => self.lift_here(true),
            HudAction::LandHere => self.lift_here(false),
            HudAction::TakeOff => {
                let ships = self.selected_lifts();
                if !ships.is_empty() {
                    self.send(Command::TakeOff { units: ships });
                }
            }
            HudAction::UnloadUnits(ids) => {
                self.send(Command::Unload {
                    units: ids.into_iter().map(Handle).collect(),
                });
            }
            HudAction::Send(command) => self.send(command),
            // Taken by `frame`, which owns the way out.
            HudAction::Leave => {}
            HudAction::Chat { text, to } => {
                if let Some(net) = &self.sim.net {
                    net.request(crate::netplay::NetRequest::Chat { text, to });
                }
            }
            HudAction::SetSpeed(pct) => self.set_speed(pct),
            HudAction::Select { units, focus } => {
                self.view.mode = Mode::Normal;
                self.view.selection = units;
                if focus {
                    self.focus_selection();
                }
            }
            HudAction::LookAt(at) => {
                self.track = None;
                self.cancel_orbit_return();
                self.camera.focus = at.extend(self.camera.focus.z);
                self.camera.clamp_focus();
            }
            HudAction::FocusPlayer(player) => {
                self.look_at_commander(player);
            }
            HudAction::Vision(side) => self.set_vision(side, audio),
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
            HudAction::Seek(tick) => *self.sim.seek.lock().unwrap() = Some(tick),
            HudAction::Range(action) => self.range_action(action, audio),
            HudAction::LineUp(units) => self.range_line_up(&units, audio),
        }
    }

    /// This machine's commander has fallen (or it has surrendered).
    fn defeated(&self) -> bool {
        self.view
            .status
            .players
            .get(self.view.local as usize)
            .is_some_and(|p| p.defeated)
    }

    fn toggle_pause(&mut self) {
        if self.view.status.owns_clock {
            self.view.paused = !self.view.paused;
        } else if let (Some(net), false) = (&self.sim.net, self.view.observing) {
            // Everyone's clock: the relay stops it and tells every player who did.
            net.request(crate::netplay::NetRequest::Pause(!self.view.paused));
        } else {
            self.hud
                .toast("Only the players can pause this match", palette::WARN);
        }
    }

    /// A click in patrol mode. The first sends the selection patrolling at once, out to
    /// the post and back to where it is, or, queued, to where its queue leaves it. Each
    /// click after that, shift still held, puts one more post into that loop, after the
    /// last. Without shift the patrol is laid in one click. With shift held over a
    /// selection that already patrols, a click puts a post into the leg nearest it.
    fn lay_patrol_post(&mut self, ground: Vec2, audio: &Audio) {
        let size = self.map.info().size_metres();
        let margin = Fx::from_int(4);
        // Clamped as the sim clamps it, so the next post can name this one exactly.
        let point = FxVec2::new(
            Fx::from_f32(ground.x).clamp(margin, size.x - margin),
            Fx::from_f32(ground.y).clamp(margin, size.y - margin),
        );
        audio.play(Sfx::Order);
        let units = self.selected_ids();
        let leg = self
            .shift
            .then(|| crate::orders::patrol_insert_leg(&self.view, ground))
            .flatten();
        if let Some((after, _)) = leg {
            self.send(Command::PatrolInsert {
                units,
                after,
                point,
            });
            self.view.patrol_inserts.push((after, point));
            return;
        }
        match self.view.patrol_posts.last() {
            // The patrol just given is not in the queues yet: the post goes after the last.
            Some(&after) => {
                self.send(Command::PatrolInsert {
                    units,
                    after,
                    point,
                });
                self.view.patrol_inserts.push((after, point));
            }
            None => {
                let start = crate::orders::patrol_start(&self.view).unwrap_or(ground);
                self.send(Command::Patrol {
                    units,
                    points: vec![point],
                    queue: self.shift,
                });
                self.view
                    .patrol_posts
                    .push(FxVec2::new(Fx::from_f32(start.x), Fx::from_f32(start.y)));
            }
        }
        self.view.patrol_posts.push(point);
        if !self.shift {
            self.view.patrol_posts.clear();
            self.view.patrol_inserts.clear();
            self.view.mode = Mode::Normal;
        }
    }

    /// Armed, or (a factory) makes something armed, which takes its stance.
    fn takes_stance(&self, u: &UnitInstance) -> bool {
        hud::ordered_as(
            &self.blueprints,
            self.blueprints.unit(BlueprintId(u.blueprint as u16)),
        )
        .any(|b| !b.weapons.is_empty())
    }

    fn set_fire_state(&mut self, state: FireState) {
        let armed: Vec<Handle> = self
            .selected_units()
            .filter(|u| self.takes_stance(u))
            .map(|u| Handle(u.unit_id))
            .collect();
        if !armed.is_empty() {
            self.send(Command::SetFireState {
                units: armed,
                state,
            });
        }
    }

    /// Dives (or surfaces) the submarines in the selection.
    fn set_dive(&mut self, dive: bool) {
        let subs: Vec<Handle> = self
            .selected_units()
            .filter(|u| {
                self.blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .dive
                    .is_some()
            })
            .map(|u| Handle(u.unit_id))
            .collect();
        if !subs.is_empty() {
            self.send(Command::SetDive { units: subs, dive });
            self.hud.toast(
                if dive { "Dive" } else { "Surface" },
                hud::style::Family::Control.tone(),
            );
        }
    }

    /// The guard area a click with no drag gives.
    fn guard_radius(&self) -> f32 {
        GUARD_DEFAULT
    }

    /// Pauses (or resumes) the work of whatever in the selection has work to pause.
    fn set_paused(&mut self, paused: bool) {
        let units: Vec<Handle> = self
            .selected_units()
            .filter(|u| {
                mc_sim::pause::pausable(
                    &self.blueprints,
                    self.blueprints.unit(BlueprintId(u.blueprint as u16)),
                )
            })
            .map(|u| Handle(u.unit_id))
            .collect();
        if !units.is_empty() {
            self.send(Command::SetPaused { units, paused });
            self.hud.toast(
                if paused {
                    "Work paused"
                } else {
                    "Work resumed"
                },
                hud::style::Family::Engineering.tone(),
            );
        }
    }

    /// Z: resumes the selection's work if all of it is paused, pauses all of it otherwise
    /// (the Pause buttons say which).
    fn toggle_paused(&mut self) {
        let (mut paused, mut workers) = (0, 0);
        for u in self.selected_units() {
            if mc_sim::pause::pausable(
                &self.blueprints,
                self.blueprints.unit(BlueprintId(u.blueprint as u16)),
            ) {
                workers += 1;
                paused += usize::from(u.paused());
            }
        }
        if workers > 0 {
            self.set_paused(paused < workers);
        }
    }

    /// V: surfaces the selection's submarines if most are down, dives them otherwise.
    fn toggle_dive(&mut self) {
        let (mut down, mut subs) = (0, 0);
        for u in self.selected_units() {
            if self
                .blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .dive
                .is_some()
            {
                subs += 1;
                down += usize::from(u.dive_goal());
            }
        }
        if subs > 0 {
            self.set_dive(down * 2 <= subs);
        }
    }

    /// H (hold position) and Y (hold fire): puts the armed selection in that
    /// stance, or back to engaging if most of it is in it already.
    fn toggle_stance(&mut self, stance: FireState) {
        let (mut in_it, mut armed) = (0, 0);
        for u in self.selected_units() {
            if self.takes_stance(u) {
                armed += 1;
                in_it += usize::from(u.fire_state() == stance);
            }
        }
        if armed == 0 {
            return;
        }
        let next = if in_it * 2 > armed {
            FireState::FireAtWill
        } else {
            stance
        };
        self.set_fire_state(next);
        self.hud.toast(
            match next {
                FireState::HoldPosition => "Hold Position",
                FireState::HoldFire => "Hold Fire",
                FireState::FireAtWill => "Engage",
            },
            hud::style::Family::Stance.tone(),
        );
    }

    fn set_speed(&mut self, pct: u32) {
        if !self.view.status.owns_clock {
            return self
                .hud
                .toast("Game speed is fixed in a network match", palette::WARN);
        }
        self.view.speed = pct;
        self.sim.speed.store(self.view.speed, Ordering::Relaxed);
    }

    fn step_speed(&mut self, step: i32) {
        if !self.view.status.owns_clock {
            return self
                .hud
                .toast("Game speed is fixed in a network match", palette::WARN);
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
            self.track = None;
            self.cancel_orbit_return();
            self.camera.focus = sum / n;
            self.camera.clamp_focus();
        }
    }

    /// Looks at `player`'s commander, if they still have one.
    fn look_at_commander(&mut self, player: u8) -> bool {
        let acu = self.view.frame.units.iter().find(|u| {
            (u.owner_flags & 0xFF) as u8 == player
                && u.owner_flags & KIND_WRECK == 0
                && self
                    .blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .has(cat::COMMANDER)
        });
        if let Some(u) = acu {
            self.track = None;
            self.orbit_return_focus = false;
            self.camera.focus = Vec3::from(u.pos);
            self.camera.distance = self.camera.distance.min(900.0);
            self.zoom_target = None;
            if self.view.observing || player == self.view.local {
                self.view.selection = vec![u.unit_id];
            }
            if self.view.observing {
                self.view.local = player;
            }
            true
        } else {
            false
        }
    }

    fn pan_camera(&mut self, pixels: Vec2) {
        if pixels != Vec2::ZERO {
            self.track = None;
            self.cancel_orbit_return();
            self.camera.pan(pixels);
        }
    }

    /// Hold Alt: swing around the unit under the pointer, or the tracked unit,
    /// or the ground under the pointer. Releasing Alt puts the camera back.
    fn begin_orbit(&mut self, r: &Renderer) {
        if self.orbit_saved.is_some() {
            return;
        }
        // Alt again while still drifting home: home stays the view from before the first orbit.
        let home = match self.orbit_return.take() {
            Some((yaw, tilt, _)) if self.track.is_some() || !self.orbit_return_focus => {
                (yaw, tilt, self.camera.focus)
            }
            Some(home) => home,
            None => (self.camera.yaw, self.camera.tilt, self.camera.focus),
        };
        self.orbit_saved = Some(home);
        self.orbit_aim = Some((self.camera.yaw, self.camera.tilt));
        self.orbit_from = Some(self.camera.focus);
        self.orbit_blend = 0.0;
        self.orbit_unit = self
            .unit_at(self.cursor)
            .map(|i| &self.view.frame.units[i])
            .filter(|u| u.owner_flags & KIND_WRECK == 0)
            .map(|u| u.unit_id)
            .or(self.track);
        self.orbit_pivot = if self.orbit_unit.is_some() {
            None
        } else {
            self.ground_under_cursor(r)
        };
    }

    fn end_orbit(&mut self) {
        self.orbit_unit = None;
        self.orbit_aim = None;
        self.orbit_from = None;
        self.orbit_pivot = None;
        self.orbit_blend = 0.0;
        if let Some((yaw, tilt, focus)) = self.orbit_saved.take() {
            // Drift back the short way round, however many turns the orbit wound up.
            let turn = std::f32::consts::TAU;
            let yaw = self.camera.yaw + (yaw - self.camera.yaw + turn * 0.5).rem_euclid(turn)
                - turn * 0.5;
            self.orbit_return = Some((yaw, tilt, focus));
            self.orbit_return_far = true;
            self.orbit_return_focus = true;
        }
    }

    /// Hands the focus back to the player after a pan or a jump; the yaw and tilt
    /// still ease home, so the orbit's angle never sticks.
    fn cancel_orbit_return(&mut self) {
        self.orbit_return_focus = false;
    }

    /// Closes a fixed share of the remaining orbit every frame, so mouse motion
    /// is smooth at every frame rate and does not jump when events bunch up.
    fn ease_orbit(&mut self, dt: f32) {
        self.orbit_return_share = 0.0;
        if let Some((yaw, tilt, focus)) = self.orbit_return {
            let share = 1.0 - (-dt * ORBIT_RETURN_RATE).exp();
            self.orbit_return_share = share;
            self.camera.yaw += (yaw - self.camera.yaw) * share;
            self.camera.tilt += (tilt - self.camera.tilt) * share;
            // Tracking brings the focus home to the unit in `follow_camera`.
            let mut settled =
                (yaw - self.camera.yaw).abs() < 1e-3 && (tilt - self.camera.tilt).abs() < 1e-3;
            if self.track.is_some() {
                settled &= !self.orbit_return_far;
            } else if self.orbit_return_focus {
                self.camera.focus = self.camera.focus.lerp(focus, share);
                self.camera.clamp_focus();
                settled &= self.camera.focus.truncate().distance(focus.truncate()) < 0.05;
            }
            if settled {
                self.camera.yaw = yaw;
                self.camera.tilt = tilt;
                if self.track.is_none() && self.orbit_return_focus {
                    self.camera.focus = focus;
                    self.camera.clamp_focus();
                }
                self.orbit_return = None;
            }
        }
        if self.orbit_saved.is_none() {
            return;
        }
        let share = 1.0 - (-dt * ORBIT_RATE).exp();
        if let Some((yaw, tilt)) = &mut self.orbit_aim {
            let (lo, hi) = self.camera.tilt_limits();
            *tilt = tilt.clamp(lo, hi);
            self.camera.yaw += (*yaw - self.camera.yaw) * share;
            self.camera.tilt += (*tilt - self.camera.tilt) * share;
        }
        self.orbit_blend += (1.0 - self.orbit_blend) * share;
    }

    fn start_track(&mut self) {
        self.track = track_target(
            self.unit_at(self.cursor)
                .map(|i| &self.view.frame.units[i])
                .filter(|u| u.owner_flags & KIND_WRECK == 0)
                .map(|u| u.unit_id),
            &self.view.selection,
        );
    }

    /// Puts the focus on `id`'s interpolated position. `false` if it is gone.
    fn stick_camera_to(&mut self, id: u32, alpha: f32) -> bool {
        let Some(&i) = self.view.index_of.get(&id) else {
            return false;
        };
        let u = &self.view.frame.units[i];
        let pos = Vec3::from(u.prev_pos) + (Vec3::from(u.pos) - Vec3::from(u.prev_pos)) * alpha;
        self.camera.focus = pos;
        self.camera.clamp_focus();
        true
    }

    /// The focus's height follows the ground, averaged over a patch that grows
    /// with the zoom and eased in. Snapped every frame to the ground under the
    /// middle of the screen, every hill and cliff the view scrolled over bobbed
    /// the eye up and down by as much, and the clouds, far nearer the eye than
    /// the ground is, swelled and shrank with it: they jittered as it panned.
    /// A jump (minimap, a unit, a new match) takes the new height at once.
    fn ease_focus_height(&mut self, renderer: &Renderer, dt: f32) {
        let at = self.camera.focus.truncate();
        let reach = (self.camera.distance * 0.12).clamp(20.0, 1500.0);
        let mut sum = renderer.ground_height(at) * 2.0;
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            sum += renderer.ground_height(at + Vec2::from_angle(a) * reach);
        }
        let ground = sum / 10.0;
        let jumped = self
            .focus_eased_at
            .is_none_or(|last| last.distance(at) > self.camera.distance);
        let k = if jumped { 1.0 } else { 1.0 - (-dt / 0.3).exp() };
        self.camera.focus.z += (ground - self.camera.focus.z) * k;
        self.focus_eased_at = Some(at);
    }

    fn follow_camera(&mut self, alpha: f32) {
        if self.orbit_saved.is_some() {
            let desired = if let Some(id) = self.orbit_unit {
                if !self.stick_camera_to(id, alpha) {
                    self.orbit_unit = None;
                    self.orbit_pivot
                } else {
                    Some(self.camera.focus)
                }
            } else {
                self.orbit_pivot
            };
            if let (Some(from), Some(to)) = (self.orbit_from, desired) {
                let k = self.orbit_blend;
                self.camera.focus = from.lerp(to, k);
                self.camera.clamp_focus();
            }
            return;
        }
        let Some(id) = self.track else {
            return;
        };
        let before = self.camera.focus;
        if !self.stick_camera_to(id, alpha) {
            self.track = None;
        } else if self.orbit_return.is_some() {
            // Slide back onto the tracked unit rather than cutting to it.
            let on_unit = self.camera.focus;
            self.camera.focus = before.lerp(on_unit, self.orbit_return_share);
            self.orbit_return_far = before.truncate().distance(on_unit.truncate()) > 0.05;
        }
    }

    /// Arms an order from its key, if the selection could carry it out.
    fn arm(&mut self, targeting: Targeting) {
        let builders = self
            .selection_takers()
            .any(|b| b.is_mobile() && b.builder.is_some());
        let able = match targeting {
            Targeting::Move | Targeting::Attack | Targeting::AttackMove | Targeting::Patrol => {
                self.selection_goes()
            }
            Targeting::Assist => builders,
            Targeting::Strike => self.selection_takers().any(|b| {
                b.weapons
                    .iter()
                    .any(|w| w.bore.is_some_and(|b| b.storm.is_some()))
            }),
            Targeting::AttackGround | Targeting::Bombard => self.selection_takers().any(|b| {
                b.weapons
                    .iter()
                    .any(|w| w.target_mask & (cat::LAND | cat::NAVAL | cat::STRUCTURE) != 0)
            }),
            Targeting::Reclaim => self.selected_units().any(|u| {
                self.blueprints
                    .unit(BlueprintId(u.blueprint as u16))
                    .sends_reclaimers()
            }),
            // Aircraft circle on guard, armed or not: a scout or radar plane watches.
            Targeting::Guard => self.selection_takers().any(|b| {
                b.is_mobile()
                    && (!b.weapons.is_empty()
                        || b.motion.is_some_and(|m| m.layer == mc_data::MoveLayer::Air))
            }),
            Targeting::Land => self.selection_lands(),
            Targeting::Unload => self.selection_lifts(),
            Targeting::Nuke => !self.armed_silos().is_empty(),
        };
        if able {
            self.view.patrol_posts.clear();
            self.view.patrol_inserts.clear();
            self.view.circle_from = None;
            self.view.mode = Mode::Target(targeting);
        }
    }

    /// Observing: see the battlefield as `side` sees it, or (`None`) all of it.
    fn set_vision(&mut self, side: Option<u8>, audio: &Audio) {
        if !self.view.observing {
            return;
        }
        let side = side.filter(|p| (*p as usize) < self.view.status.players.len());
        if side == self.view.perspective {
            return;
        }
        audio.play(Sfx::Select);
        self.view.perspective = side;
        if let Some(p) = side {
            // Friend and foe are then that side's.
            self.view.local = p;
        }
    }

    fn key_pressed(&mut self, code: KeyCode, r: &Renderer, audio: &Audio) {
        // The chat line has the keyboard: its letters are words, not orders.
        if self.hud.net.typing() {
            return;
        }
        if self.hud.free.on && self.free_camera_key(code, r, audio) {
            return;
        }
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
        // B arms the construction keys; while they are live, tiers, shelves and items take the letters.
        if self.hud.build_keys {
            let key = match code {
                KeyCode::Digit1 => Some('1'),
                KeyCode::Digit2 => Some('2'),
                KeyCode::Digit3 => Some('3'),
                KeyCode::Digit4 => Some('4'),
                KeyCode::Digit5 => Some('5'),
                KeyCode::KeyU => Some('U'),
                KeyCode::KeyQ => Some('Q'),
                KeyCode::KeyW => Some('W'),
                KeyCode::KeyE => Some('E'),
                KeyCode::KeyR => Some('R'),
                KeyCode::KeyT => Some('T'),
                KeyCode::KeyY => Some('Y'),
                KeyCode::KeyI => Some('I'),
                KeyCode::KeyA => Some('A'),
                KeyCode::KeyS => Some('S'),
                KeyCode::KeyD => Some('D'),
                KeyCode::KeyF => Some('F'),
                KeyCode::KeyG => Some('G'),
                KeyCode::KeyH => Some('H'),
                KeyCode::KeyJ => Some('J'),
                KeyCode::KeyK => Some('K'),
                KeyCode::KeyL => Some('L'),
                _ => None,
            };
            match (code, key) {
                (KeyCode::Escape | KeyCode::KeyB, _) => {
                    self.hud.build_keys = false;
                    return;
                }
                (_, Some(k)) => {
                    self.hud.build_key = Some(k);
                    return;
                }
                _ => {}
            }
        }
        // Another order key puts the formation panel away.
        if matches!(
            code,
            KeyCode::KeyB
                | KeyCode::KeyM
                | KeyCode::KeyA
                | KeyCode::KeyF
                | KeyCode::KeyC
                | KeyCode::KeyR
                | KeyCode::KeyP
                | KeyCode::KeyJ
                | KeyCode::KeyK
                | KeyCode::KeyH
                | KeyCode::KeyY
                | KeyCode::KeyV
                | KeyCode::KeyZ
                | KeyCode::KeyX
        ) {
            self.view.formation_panel = false;
        }
        match code {
            KeyCode::KeyB => {
                let builds = self.selected_units().any(|u| {
                    let bp = self.blueprints.unit(BlueprintId(u.blueprint as u16));
                    bp.builder.as_ref().is_some_and(|b| !b.builds.is_empty())
                        || bp.upgrades_to.is_some()
                });
                if builds {
                    self.hud.build_keys = true;
                    audio.play(Sfx::Tick);
                } else {
                    audio.play(Sfx::Deny);
                }
            }
            KeyCode::Escape => {
                if self.orders.dragging() {
                    self.orders.cancel();
                } else if self.view.formation_panel {
                    self.view.formation_panel = false;
                } else if self.view.mode != Mode::Normal {
                    self.view.mode = Mode::Normal;
                    self.view.patrol_posts.clear();
                    self.view.patrol_inserts.clear();
                    self.view.circle_from = None;
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
            KeyCode::KeyG if self.view.range.is_some() => self.range_action(
                if self.shift {
                    RangeAction::ArmSpawn
                } else {
                    RangeAction::ArmSubject
                },
                audio,
            ),
            KeyCode::KeyG if self.ctrl => self.arm(Targeting::Guard),
            KeyCode::KeyG => self.toggle_formation_panel(audio),
            KeyCode::KeyM => self.arm(Targeting::Move),
            KeyCode::KeyA => self.arm(Targeting::Attack),
            KeyCode::KeyT => self.start_track(),
            KeyCode::KeyF => self.arm(Targeting::AttackMove),
            KeyCode::KeyC => self.arm(Targeting::Assist),
            KeyCode::KeyR => self.arm(Targeting::Reclaim),
            KeyCode::KeyP => self.arm(Targeting::Patrol),
            KeyCode::KeyJ => self.arm(Targeting::AttackGround),
            KeyCode::KeyK => self.arm(Targeting::Bombard),
            KeyCode::KeyH => self.toggle_stance(FireState::HoldPosition),
            KeyCode::KeyY => self.toggle_stance(FireState::HoldFire),
            KeyCode::KeyV => self.toggle_dive(),
            KeyCode::KeyZ => self.toggle_paused(),
            // With a silo picked, N arms the launch; otherwise it hides the minimap.
            KeyCode::KeyN
                if self
                    .selected_units()
                    .any(|u| crate::hud::silo::is_silo(&self.blueprints, u)) =>
            {
                if self.armed_silos().is_empty() {
                    audio.play(Sfx::Deny);
                } else {
                    audio.play(Sfx::Tick);
                    self.arm(Targeting::Nuke);
                }
            }
            KeyCode::KeyN => self.hud.minimap_hidden = !self.hud.minimap_hidden,
            KeyCode::KeyI => self.hud.details_open = !self.hud.details_open,
            KeyCode::KeyX => self.send(Command::Stop {
                units: self.selected_ids(),
            }),
            // A unit with refit slots has a choice to make: U opens it rather than picking.
            KeyCode::KeyU
                if self.selected_units().any(|u| {
                    self.blueprints
                        .refit_set(mc_data::BlueprintId(u.blueprint as u16))
                        .is_some()
                }) =>
            {
                self.hud.open_refit_tab()
            }
            KeyCode::KeyU if self.selection_lifts() && self.shift => self.lift_here(true),
            KeyCode::KeyU if self.selection_lifts() => self.arm(Targeting::Unload),
            KeyCode::KeyU => self.send(Command::Upgrade {
                units: self.selected_ids(),
            }),
            KeyCode::KeyL if self.selection_lands() && self.shift => self.lift_toggle(),
            KeyCode::KeyL if self.selection_lands() => self.arm(Targeting::Land),
            // As the Repeat button: off if every factory repeats, all on otherwise.
            KeyCode::KeyL if self.selection_has(cat::FACTORY) => {
                let all = self
                    .selected_units()
                    .filter(|u| {
                        self.blueprints
                            .unit(BlueprintId(u.blueprint as u16))
                            .has(cat::FACTORY)
                    })
                    .all(|u| hud::has_flag(u, flag::REPEAT));
                self.send(Command::SetRepeat {
                    factories: self.selected_ids(),
                    repeat: !all,
                });
            }
            KeyCode::Pause => self.toggle_pause(),
            KeyCode::Equal | KeyCode::NumpadAdd => self.step_speed(1),
            KeyCode::Minus | KeyCode::NumpadSubtract => self.step_speed(-1),
            KeyCode::Delete if self.ctrl => self.send(Command::SelfDestruct {
                units: self.selected_ids(),
            }),
            KeyCode::Home => {
                let player = if self.view.observing {
                    self.view
                        .status
                        .players
                        .iter()
                        .enumerate()
                        .find(|(_, p)| !p.defeated)
                        .map(|(i, _)| i as u8)
                        .unwrap_or(self.view.local)
                } else {
                    self.view.local
                };
                self.look_at_commander(player);
            }
            c => {
                if let Some(n) = digit(c) {
                    if self.view.observing && !self.ctrl {
                        // Observers have no groups to call: the digits are whose eyes to use.
                        self.set_vision(n.checked_sub(1).map(|p| p as u8), audio);
                    } else if self.ctrl {
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
        orders::site(
            &field,
            blueprint,
            orders::surface_under(&field, self.cursor)?,
            None,
        )
    }

    /// Sites a click or a place-drag would put down. `drag` is the pointer having
    /// moved a footprint's worth; without it the ghost stays under the cursor.
    fn placing_sites(
        &self,
        blueprint: BlueprintId,
        drag: bool,
        r: &Renderer,
    ) -> Vec<(FxVec2, Result<(), Unfit>)> {
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            renderer: r,
        };
        let Some(ground) = orders::surface_under(&field, self.cursor) else {
            return Vec::new();
        };
        let Some((to, _)) = orders::site(&field, blueprint, ground, None) else {
            return Vec::new();
        };
        match (drag, self.place_from) {
            (true, Some(from)) => orders::drag_sites(&field, blueprint, from, to),
            _ => orders::site_verdict(&field, blueprint, ground, None, &[])
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
        if at != self.published_at {
            let gap = at.duration_since(self.published_at).as_secs_f32();
            let expected = TICK_SECONDS * 100.0 / self.view.speed.max(1) as f32;
            self.interp_span = gap.clamp(expected, expected * 8.0);
            self.published_at = at;
        }
        crate::hud::silo::settle_sent(&mut self.view);
        crate::titan_marks::observe(&mut self.view, &self.blueprints);
        self.view.index_of.clear();
        for (i, u) in self.view.frame.units.iter().enumerate() {
            if u.owner_flags & KIND_WRECK == 0 {
                self.view.index_of.insert(u.unit_id, i);
            }
        }
        let index_of = &self.view.index_of;
        let selectable: std::collections::HashSet<_> = index_of
            .iter()
            .filter(|(_, &i)| {
                self.blueprints
                    .unit(BlueprintId(self.view.frame.units[i].blueprint as u16))
                    .visual
                    .mesh
                    != "reclaim_drone"
            })
            .map(|(&id, _)| id)
            .collect();
        self.view.selection.retain(|id| selectable.contains(id));
        // A death is not a selection: nobody answers for it.
        self.answered.retain(|id| index_of.contains_key(id));
        for group in &mut self.view.groups {
            group.retain(|id| selectable.contains(id));
        }
        true
    }

    /// How loud and where between the speakers something at `pos` is, from
    /// where the camera looks or, for the free camera, from its eye
    /// (`audio/listener.rs`).
    fn hear(&self, pos: Vec3) -> (f32, f32) {
        self.ear().hear(pos)
    }

    /// `hear`, for the small sounds of work (construction beams): `Ear::hear_work`.
    fn hear_work(&self, pos: Vec3) -> (f32, f32) {
        self.ear().hear_work(pos)
    }

    /// Where the battle is heard from this frame: the free camera's eye while it flies.
    fn ear(&self) -> crate::audio::listener::Ear<'_> {
        crate::audio::listener::Ear {
            camera: &self.camera,
            free: self.hud.free.on,
        }
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
                    // A core mine's gait counts its hammer's blows (`mines::hammer_gait`): one a step.
                    model
                        .legs
                        .map(|legs| (sound, legs.stride * 0.5))
                        .or(u.mine.as_ref().map(|_| (sound, 1.0)))
                }),
                // Its own answer, else its faction's for its kind, else the shared one.
                select: u
                    .sounds
                    .select
                    .as_ref()
                    .or(self.blueprints.factions[u.faction.0 as usize]
                        .sounds
                        .select
                        .get(&u.visual.icon))
                    .or(d.select.get(&u.visual.icon))
                    .and_then(|n| library.id_of(n)),
                weapons: u
                    .weapons
                    .iter()
                    .map(|w| WeaponSoundIds {
                        fire: id(&w.sounds.fire, &d.fire),
                        hold: id(&w.sounds.hold, &None),
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
            repair: ["repair_beam", "repair_start", "repair_end"].map(|name| library.id_of(name)),
            build: self
                .blueprints
                .factions
                .iter()
                .map(|f| match &f.sounds.build {
                    Some(b) => [&b.beam, &b.start, &b.end].map(|name| library.id_of(name)),
                    None => {
                        ["build_beam", "build_start", "build_end"].map(|name| library.id_of(name))
                    }
                })
                .collect(),
            shield_hit: library.id_of("shield_hit"),
            shield_break: library.id_of("shield_break"),
            intercept_laser: library.id_of("intercept_laser"),
            intercept_break: library.id_of("intercept_break"),
            cold_eject: library.id_of("cold_eject"),
            water: ["shell_in_water", "shell_in_water_heavy"].map(|name| library.id_of(name)),
            rain: ["rain_light", "rain_heavy"].map(|name| library.id_of(name)),
            thunder: ["thunder_near", "thunder_far"].map(|name| library.id_of(name)),
        });
    }

    /// The battle sounds of the last tick. Which sound is the unit files'
    /// business; this places them, and lets only the loudest few of each kind
    /// through, so a hundred tanks firing at once is a barrage and not a wall of noise.
    fn battle_sounds(&mut self, audio: &Audio) {
        self.sound_table(audio);
        let was_beaming = std::mem::take(&mut self.beaming);
        let was_mending = std::mem::take(&mut self.mending);
        let was_welding = std::mem::take(&mut self.welding);
        let beaming: std::collections::HashMap<u32, [f32; 3]> = self
            .view
            .frame
            .beam_sources
            .iter()
            .copied()
            .zip(self.view.frame.beams.iter())
            .filter(|(_, b)| b.kind == mc_sim::reclaim::BEAM_RECLAIM)
            .map(|(id, b)| (id, b.from))
            .collect();
        let mending: std::collections::HashMap<u32, [f32; 3]> = self
            .view
            .frame
            .beam_sources
            .iter()
            .copied()
            .zip(self.view.frame.beams.iter())
            .filter(|(_, b)| b.kind == mc_sim::repair::BEAM_REPAIR)
            .map(|(id, b)| (id, b.from))
            .collect();
        let welding: std::collections::HashMap<u32, ([f32; 3], u8)> = self
            .view
            .frame
            .build_sources
            .iter()
            .map(|b| (b.unit, (b.at, b.faction)))
            .collect();
        let Some(table) = &self.sounds else { return };
        let bps = &self.blueprints;
        // (sound, gain, pan, pitch, delay) per kind: shots, impacts, deaths, charging, beams starting and stopping.
        type Heard = (mc_data::SoundId, f32, f32, f32, f32);
        let mut heard: [Vec<Heard>; 5] = Default::default();
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
            mending
                .iter()
                .filter(|(id, _)| !was_mending.contains_key(id))
                .map(|(_, at)| (table.repair[1], at)),
        );
        let switched = switched.chain(
            was_mending
                .iter()
                .filter(|(id, _)| !mending.contains_key(id))
                .map(|(_, at)| (table.repair[2], at)),
        );
        let build_of = |faction: u8| {
            table
                .build
                .get(faction as usize)
                .copied()
                .unwrap_or_default()
        };
        let switched = switched.chain(
            welding
                .iter()
                .filter(|(id, _)| !was_welding.contains_key(id))
                .map(|(_, (at, f))| (build_of(*f)[1], at)),
        );
        let switched = switched.chain(
            was_welding
                .iter()
                .filter(|(id, _)| !welding.contains_key(id))
                .map(|(_, (at, f))| (build_of(*f)[2], at)),
        );
        let is_weld = |sound: mc_data::SoundId| {
            table
                .build
                .iter()
                .any(|b| b[1] == Some(sound) || b[2] == Some(sound))
        };
        for (sound, at) in switched {
            let Some(sound) = sound else { continue };
            let (gain, pan) = if is_weld(sound) {
                self.hear_work(Vec3::from(*at))
            } else {
                self.hear(Vec3::from(*at))
            };
            if gain < 0.02 {
                continue;
            }
            let jitter = ((at[0] * 12.9898 + at[1] * 78.233).sin() * 43_758.547)
                .fract()
                .abs();
            heard[4].push((sound, gain, pan, 0.96 + jitter * 0.08, 0.0));
        }
        let water = self.map.info().water_level;
        for event in &self.view.frame.events {
            if crate::audio::beams::is_held_beam(event, bps) {
                // Heard as its loop (`held_beam_loops`), not a shot and a strike a tick.
                continue;
            }
            let (kind, sound, pos, weight, delay) = match event {
                mc_sim::SimEvent::ShotFired {
                    pos,
                    blueprint,
                    weapon,
                    ..
                } if bps.unit(*blueprint).weapons[*weapon as usize].cold_launch_ticks == 0 => (
                    0,
                    table.units[blueprint.index()].weapons[*weapon as usize].fire,
                    pos.to_f32(),
                    bps.unit(*blueprint).weapons[*weapon as usize]
                        .damage
                        .to_f32(),
                    0.0,
                ),
                mc_sim::SimEvent::ShotFired {
                    pos,
                    blueprint,
                    weapon,
                    ..
                } if bps.unit(*blueprint).weapons[*weapon as usize].cold_launch_ticks > 0 => {
                    (0, table.cold_eject, pos.to_f32(), 36.0, 0.0)
                }
                mc_sim::SimEvent::MissileIgnited {
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
                    0.0,
                ),
                mc_sim::SimEvent::Impact {
                    pos,
                    on_unit,
                    on_shield,
                    after,
                    blueprint,
                    weapon,
                    ..
                } => {
                    let ids = &table.units[blueprint.index()].weapons[*weapon as usize];
                    let w = &bps.unit(*blueprint).weapons[*weapon as usize];
                    // Shots stop on the sea's surface, so one that ends exactly there went in the water.
                    let in_water = (pos.z - water).abs() < mc_core::Fx::milli(50);
                    let sound = if *on_shield {
                        table.shield_hit
                    } else if *on_unit {
                        ids.impact
                    } else if in_water && !w.torpedo {
                        let heavy = w.damage.to_f32() >= 60.0 || w.splash.to_f32() >= 6.0;
                        table.water[usize::from(heavy)].or(ids.ground)
                    } else {
                        ids.ground
                    };
                    (
                        1,
                        sound,
                        pos.to_f32(),
                        bps.unit(*blueprint).weapons[*weapon as usize]
                            .damage
                            .to_f32(),
                        after.to_f32() * TICK_SECONDS,
                    )
                }
                mc_sim::SimEvent::ShieldBroken { pos, radius, .. } => {
                    // Deaths, not impacts: a barrage of shield hits must not
                    // steal the four impact slots the tick the dome goes.
                    (2, table.shield_break, pos.to_f32(), radius.to_f32(), 0.0)
                }
                mc_sim::SimEvent::UnitDied { pos, blueprint, .. }
                | mc_sim::SimEvent::AircraftCrashed { pos, blueprint } => (
                    2,
                    table.units[blueprint.index()].death,
                    pos.to_f32(),
                    bps.unit(*blueprint).health.to_f32() * 0.2,
                    0.0,
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
                    0.0,
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
            if matches!(event, mc_sim::SimEvent::WeaponCharging { .. }) {
                // A wind-up is felt beside the gun; from strategic zoom it should not fill the speakers.
                gain *= self.ear().closeness(Vec3::from(pos)).sqrt();
            }
            if matches!(
                event,
                mc_sim::SimEvent::Impact {
                    on_shield: true,
                    ..
                }
            ) {
                // A dome hit must not lose the mix to an armour crack from a miss nearby.
                gain *= 1.15;
            }
            if matches!(event, mc_sim::SimEvent::ShieldBroken { .. }) {
                // A collapse is the event; keep it above the barrage that caused it.
                gain = (gain * 1.55).max(0.42);
            }
            let ignition = matches!(event, mc_sim::SimEvent::MissileIgnited { .. });
            // The sound says how big the weapon is; this only leans on it a little, and no two shots are pitched quite alike.
            let size: f32 = (weight.max(1.0) / 26.0).ln();
            let jitter = ((pos[0] * 12.9898 + pos[1] * 78.233 + self.view.frame.tick as f32 * 3.7)
                .sin()
                * 43_758.547)
                .fract()
                .abs();
            let mut loud = (0.85 + size * 0.1).clamp(0.6, 1.3);
            let mut tone = 0.95 + jitter * 0.1;
            if ignition {
                // A motor lighting in the open, above the launcher: heavier than a tube launch.
                loud = (loud * 1.9).min(2.4);
                tone *= 0.78;
            }
            // Beside the guns, 1; from strategic zoom (or a free eye far off), near 0.
            let close = self.ear().closeness(Vec3::from(pos));
            if let mc_sim::SimEvent::ShotFired {
                blueprint, weapon, ..
            } = event
            {
                // Some guns are meant to be heard over the battle (`WeaponSounds::volume`),
                // but only up close: from orbit they take their place in the mix.
                let volume = bps.unit(*blueprint).weapons[*weapon as usize].sounds.volume as f32;
                if volume > 0.0 {
                    loud *= 1.0 + (volume - 1.0) * close;
                }
            }
            if let mc_sim::SimEvent::ShotFired {
                blueprint, weapon, ..
            }
            | mc_sim::SimEvent::Impact {
                blueprint, weapon, ..
            } = event
            {
                // A stream gun sounds every tick, shots and hits both; from orbit that
                // rattle would bury everything else, so it thins out as the camera climbs.
                if bps.unit(*blueprint).weapons[*weapon as usize].rounds > 1 {
                    loud *= close.sqrt();
                }
            }
            heard[kind].push((sound, gain * loud, pan, tone, delay));
        }
        for (kind, most) in [(0, 5), (1, 4), (2, 3), (3, 2), (4, 3)] {
            heard[kind].sort_by(|a, b| b.1.total_cmp(&a.1));
            for (i, (sound, gain, pan, pitch, delay)) in heard[kind].iter().take(most).enumerate() {
                // Those that lose out still lend the loudest ones a little weight.
                let crowd = if i == 0 {
                    1.0 + (heard[kind].len().saturating_sub(most) as f32 * 0.04).min(0.3)
                } else {
                    1.0
                };
                audio.play_world_after(*sound, gain * crowd, *pan, *pitch, *delay);
            }
        }

        // The intercept laser is its own voice: a low steady hum while any laser holds
        // on a missile (a loop, below, not a sound per tick of burn), and a muffled pop
        // at the missile when the casing fails.
        let mut lasers = (0.0f32, 0.0f32);
        let mut snaps: Vec<(f32, f32, f32)> = Vec::new();
        for event in &self.view.frame.events {
            let mc_sim::SimEvent::MissileLased { from, to, killed } = event else {
                continue;
            };
            let (gain, pan) = self.hear(Vec3::from(from.to_f32()));
            lasers = (lasers.0 + gain * gain, lasers.1 + gain * gain * pan);
            if *killed {
                let jitter = ((to.x.to_f32() * 12.9898 + to.y.to_f32() * 78.233).sin()
                    * 43_758.547)
                    .fract()
                    .abs();
                let (gain, pan) = self.hear(Vec3::from(to.to_f32()));
                snaps.push((gain * 0.8, pan, 0.92 + jitter * 0.12));
            }
        }
        snaps.sort_by(|a, b| b.0.total_cmp(&a.0));
        if let Some(sound) = table.intercept_break {
            for (gain, pan, pitch) in snaps.into_iter().take(2) {
                audio.play_world(sound, gain, pan, pitch);
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
            if crate::audio::capital::is_capital(u, bps) {
                continue;
            }
            let Some(sound) = table.units.get(u.blueprint as usize).and_then(|t| t.moving) else {
                continue;
            };
            // Heard only when it is really under way: ground covered on this tick and the one
            // before (`gait`, metres). A hull the crowd nudges for one tick sets MOVING but is
            // not driving, and used to blip its running gear on and off while it stood.
            let [_, now, before] = u.gait;
            if now.min(before) < 0.12 {
                continue;
            }
            // Louder as it works harder: half level at a crawl, full at its road speed.
            let top = bps
                .units
                .get(u.blueprint as usize)
                .and_then(|bp| bp.motion)
                .map_or(20.0, |m| m.speed.to_f32())
                .max(1.0);
            let pace = (now.max(before) * mc_core::TICKS_PER_SECOND as f32 / top).min(1.0);
            let (gain, pan) = self.hear(Vec3::from(u.pos));
            movers.push((sound, gain * (0.5 + 0.5 * pace), pan));
        }
        let mut loops = crate::audio::mix_moving(&movers);
        // Capital ships' drives (audio/capital.rs): heard from how they move, not as movers.
        let mut capital = std::mem::take(&mut self.capital_sounds);
        loops.extend(
            capital.tick(&self.view.frame.units, &self.blueprints, audio, |p| {
                self.hear(p)
            }),
        );
        self.capital_sounds = capital;
        // Giant rotary guns' barrels turning (audio/titan.rs).
        loops.extend(self.giant_sounds.loops());
        // Reclaim beams: one loop for all of them, heard from where they bite.
        if let Some(sound) = table.reclaim[0] {
            let mut beams = (0.0, 0.0);
            for b in &self.view.frame.beams {
                if b.kind != mc_sim::reclaim::BEAM_RECLAIM {
                    continue;
                }
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
        // Held beams (the Naga's): one loop per sound, heard from where they fire.
        loops.extend(crate::audio::beams::held_beam_loops(
            &self.view.frame.events,
            bps,
            |b, w| table.units[b.index()].weapons[w as usize].hold,
            |p| self.hear(p),
        ));
        // Repair beams: one loop, heard from the hull they are patching.
        if let Some(sound) = table.repair[0] {
            let mut beams = (0.0, 0.0);
            for b in &self.view.frame.beams {
                if b.kind != mc_sim::repair::BEAM_REPAIR {
                    continue;
                }
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
        loops.extend(self.survival_loops(audio));
        // Construction beams: one loop per faction's building sound, each heard from its
        // nearest few welds only, so factories printing all over the map do not add up to
        // a hum everywhere.
        let mut build_loops: Vec<mc_data::SoundId> =
            table.build.iter().filter_map(|b| b[0]).collect();
        build_loops.sort_by_key(|s| s.0);
        build_loops.dedup();
        for sound in build_loops {
            let mut welds: Vec<(f32, f32)> = self
                .view
                .frame
                .build_sources
                .iter()
                .filter(|b| {
                    table
                        .build
                        .get(b.faction as usize)
                        .is_some_and(|l| l[0] == Some(sound))
                })
                .map(|b| self.hear_work(Vec3::from(b.at)))
                .filter(|(gain, _)| *gain > 0.01)
                .collect();
            welds.sort_by(|a, b| b.0.total_cmp(&a.0));
            let mut beams = (0.0, 0.0);
            for &(gain, pan) in welds.iter().take(3) {
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
        // Lasers holding on missiles: one hum, from where they are.
        if let (Some(sound), true) = (table.intercept_laser, lasers.0 > 0.0) {
            loops.push((
                sound,
                (lasers.0.sqrt() * 0.35).min(0.5),
                lasers.1 / lasers.0.max(1e-9),
                1.0,
            ));
        }
        loops.extend(self.warhead_loops(audio));
        // Missiles heard as they fly (audio/flight.rs).
        let (library, _) = audio.library();
        loops.extend(crate::audio::flight::loops(
            &self.view.frame.flights,
            &self.blueprints,
            &library,
            |pos| self.hear(pos),
        ));
        audio.set_loops(&loops);
        // Rain: a light and a heavy loop crossfaded by how hard it falls where the
        // camera looks, loudest down among the units, spread across both ears. On
        // the weather volume, and kept under the battle.
        let mut rain = Vec::new();
        if self.rain_here > 0.01 {
            let near = 1.0 / (1.0 + self.camera.distance / 900.0);
            let gain = self.rain_here.sqrt() * (0.35 + 0.65 * near);
            let heavy = ((self.rain_here - 0.3) / 0.5).clamp(0.0, 1.0);
            for (sound, share) in [(table.rain[0], 1.0 - heavy * 0.7), (table.rain[1], heavy)] {
                if let Some(sound) = sound {
                    if share > 0.01 {
                        rain.push((sound, gain * share * 0.3, -0.55, 1.0));
                        rain.push((sound, gain * share * 0.3, 0.55, 1.02));
                    }
                }
            }
        }
        // The ambient beds share the weather loops (ambience.rs).
        rain.extend_from_slice(self.ambience.loops());
        audio.set_weather_loops(&rain);
        self.beaming = beaming;
        self.mending = mending;
        self.welding = welding;
    }

    /// The player changed the selection: the units new to it answer (all of it,
    /// when it only got smaller), in the voice of the kind there is most of.
    /// Which sound is the unit files' and the library's business.
    fn answer_selection(&mut self, audio: &Audio, now: Instant) {
        if self.view.selection == self.answered {
            return;
        }
        // The formation panel belongs to the selection it was opened for.
        self.view.formation_panel = false;
        let before: HashSet<u32> = self.answered.iter().copied().collect();
        self.answered.clone_from(&self.view.selection);
        self.sound_table(audio);
        let Some(table) = &self.sounds else { return };
        let new = self.view.selection.iter().any(|id| !before.contains(id));
        // (sound, how many make it, the dearest of them)
        let mut voices: Vec<(mc_data::SoundId, u32, f32)> = Vec::new();
        for u in self.selected_units().filter(|u| {
            (!new || !before.contains(&u.unit_id)) && !self.is_enemy((u.owner_flags & 0xFF) as u8)
        }) {
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

    /// Thunder after lightning, as late as its sound takes to arrive: a crack
    /// for a stroke close by, a long roll for anything further off.
    fn thunder(&mut self, flashes: Vec<mc_render::sky::Thunder>, audio: &Audio) {
        if flashes.is_empty() {
            return;
        }
        self.sound_table(audio);
        let Some(table) = &self.sounds else { return };
        let eye = self.camera.eye();
        for f in flashes {
            let distance = f.pos.distance(eye);
            let gain = f.strength * 0.5 / (1.0 + (distance / 2500.0).powf(1.5));
            if gain < 0.017 {
                continue;
            }
            let near = f.bolt && distance < 3000.0;
            let Some(sound) = table.thunder[if near { 0 } else { 1 }] else {
                continue;
            };
            let (_, pan) = self.hear(f.pos);
            let pitch = 0.9 + (f.pos.x * 0.0137 + f.pos.y * 0.0071).fract().abs() * 0.2;
            audio.play_weather_after(
                sound,
                gain.min(1.0),
                pan,
                pitch,
                (distance / 343.0).min(12.0),
            );
        }
    }

    /// Warheads in flight (`docs/NUKES.md`): the motor's roar while it climbs, the rush
    /// of air as it comes down, louder the nearer it is to landing. Heard a little from
    /// anywhere, like the launch. A salvo is at most three voices (audio/salvo.rs): one
    /// climbing, the warhead nearest to landing, and the rest of the fall merged.
    fn warhead_loops(&self, audio: &Audio) -> Vec<(mc_data::SoundId, f32, f32, f32)> {
        use crate::audio::salvo;
        let (library, _) = audio.library();
        let (Some(flight), Some(fall)) = (
            library.id_of("warhead_flight"),
            library.id_of("warhead_fall"),
        ) else {
            return Vec::new();
        };
        let mut climbing = Vec::new();
        let mut falling = Vec::new();
        for m in &self.view.frame.strategic {
            if m.kind != mc_sim::mirror::STRATEGIC_WARHEAD {
                continue;
            }
            let (gain, pan) = self.hear(Vec3::from(m.pos));
            if m.pos[2] < m.prev_pos[2] - 0.05 {
                let near = (1.0 - m.eta / 15.0).clamp(0.0, 1.0);
                falling.push((gain.max(0.12 + 0.45 * near * near).min(1.0), pan, m.eta));
            } else {
                // Coming up to full roar as it clears the tube.
                let lit = (m.age / 1.2).clamp(0.0, 1.0);
                climbing.push((gain.max(0.18) * lit, pan));
            }
        }
        let mut out = Vec::new();
        if let Some((gain, pan)) = salvo::merge(climbing, 1.0) {
            out.push((flight, gain, pan, 1.0));
        }
        out.extend(
            salvo::falls(falling)
                .into_iter()
                .map(|(gain, pan)| (fall, gain, pan, 1.0)),
        );
        out
    }

    /// Strategic missiles (`docs/NUKES.md`): a warhead's launch is heard by everyone the
    /// same way, whoever fired it, and its detonation from anywhere on the map, at once (no
    /// delay for the sound's travel). Looked up by name: these are rare. A salvo of dozens
    /// coalesces (audio/salvo.rs): one alarm, one deeper roar, a budget of detonations,
    /// the small sounds rate-limited.
    fn nuke_sounds(&mut self, audio: &Audio) {
        use crate::audio::salvo::Small;
        use mc_sim::SimEvent;
        let (library, _) = audio.library();
        let local = self.view.local;
        let mut launches = 0u32;
        let mut bursts = Vec::new();
        let mut small: [Vec<(f32, f32)>; 3] = Default::default();
        for event in &self.view.frame.events {
            let (kind, pos, floor) = match event {
                SimEvent::NuclearDetonation {
                    pos,
                    commander: false,
                    ..
                } => {
                    bursts.push(self.hear(Vec3::from(pos.to_f32())));
                    continue;
                }
                SimEvent::NuclearLaunch { .. } => {
                    launches += 1;
                    continue;
                }
                SimEvent::InterceptorLaunch { from, .. } => (Small::InterceptorLaunch, from, 0.0),
                SimEvent::WarheadIntercepted {
                    pos, killed: true, ..
                } => (Small::Intercepted, pos, 0.5),
                SimEvent::SiloOpening { pos, .. } => (Small::SiloDoors, pos, 0.0),
                SimEvent::RoundReady {
                    owner,
                    warhead: true,
                    ..
                } if *owner == local && !self.view.observing => {
                    if let Some(ready) = library.id_of("warhead_ready") {
                        audio.play_response(ready, 0.6);
                    }
                    continue;
                }
                _ => continue,
            };
            let (gain, pan) = self.hear(Vec3::from(pos.to_f32()));
            small[kind as usize].push((gain.max(floor), pan));
        }
        if launches == 0 && bursts.is_empty() && small.iter().all(Vec::is_empty) {
            return;
        }
        let salvo = &mut self.salvo_sounds;
        let t = salvo.now();
        // The same alarm and roar for everyone, the launcher included, at one level and
        // from nowhere in particular: the sound never tells whose warhead it is, so
        // players have to look.
        if let Some(alarm) = library.id_of("nuke_alarm") {
            let length = library.sound(alarm).length;
            if salvo.alarm(t, launches, length) {
                audio.play_response(alarm, 0.8);
            }
        }
        if let Some(roar) = library.id_of("nuke_launch") {
            for p in salvo.roar(t, launches) {
                audio.play_world_after(roar, p.gain, p.pan, p.pitch, p.delay);
            }
        }
        // Heard the moment it happens, however far: the user wants the blast and its
        // sound together, not the real lag of sound through the air.
        if let (Some(sound), Some(p)) = (
            library.id_of("nuke_detonation"),
            salvo.detonation(t, &bursts),
        ) {
            audio.play_world_after(sound, p.gain, p.pan, p.pitch, 0.0);
        }
        for (kind, name) in [
            (Small::InterceptorLaunch, "interceptor_launch"),
            (Small::Intercepted, "warhead_intercepted"),
            (Small::SiloDoors, "silo_doors"),
        ] {
            if let (Some(sound), Some(p)) = (
                library.id_of(name),
                salvo.small(t, kind, &small[kind as usize]),
            ) {
                audio.play_world_after(sound, p.gain, p.pan, p.pitch, 0.0);
            }
        }
    }

    /// What the last tick reported that the player should hear about.
    fn note_events(&mut self, audio: &Audio) {
        self.battle_sounds(audio);
        let mut giants = std::mem::take(&mut self.giant_sounds);
        giants.tick(
            &self.view.frame.units,
            &self.view.frame.events,
            &self.blueprints,
            audio,
            self.camera.focus,
            10.0 / self.view.speed.max(5) as f32,
            |p| self.hear(p),
        );
        self.giant_sounds = giants;
        self.nuke_sounds(audio);
        let survival: Vec<mc_sim::SimEvent> = self
            .view
            .frame
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    mc_sim::SimEvent::RoundPrinting { .. }
                        | mc_sim::SimEvent::RoundLaunched { .. }
                        | mc_sim::SimEvent::NodeRaising { .. }
                        | mc_sim::SimEvent::NodeOnline { .. }
                        | mc_sim::SimEvent::NodeDestroyed { .. }
                        | mc_sim::SimEvent::SurvivalWon { .. }
                )
            })
            .cloned()
            .collect();
        for e in &survival {
            self.note_survival(e, audio);
        }
        for event in &self.view.frame.events {
            match event {
                mc_sim::SimEvent::BuildRejected { player } if *player == self.view.local => {
                    audio.play(Sfx::Deny);
                    self.hud.toast("Cannot Build There", palette::BAD);
                }
                mc_sim::SimEvent::CommandRefused { player, reason }
                    if *player == self.view.local =>
                {
                    audio.play(Sfx::Deny);
                    self.hud.toast(reason.message(), palette::BAD);
                }
                mc_sim::SimEvent::PlayerDefeated { player } => {
                    let name = self
                        .view
                        .status
                        .players
                        .get(*player as usize)
                        .map_or("A Commander", |p| p.name.as_str());
                    self.hud.toast(
                        format!("{name} Has Been Defeated"),
                        if !self.view.observing && *player == self.view.local {
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
            cover,
        } = ctx;
        if !self.chart_ready {
            overlay.set_image(
                hud::MINIMAP_SLOT,
                ui::preview::SIZE,
                ui::preview::SIZE,
                &ui::preview::render(&self.map, crate::setup::map_config(&self.map).climate),
            );
            self.chart_ready = true;
        }
        if !self.hud.thumbs.baked() {
            let team = self.view.colors[self.view.local as usize % 8];
            self.hud.thumbs.bake(overlay, &self.blueprints, team);
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

        // Keyboard camera. Alt owns the view: the usual keys must not pan underneath an orbit.
        let cine = self.hud.free.on;
        self.cine.hand_back_lift(&mut self.camera);
        if self.orbit_saved.is_none()
            && !self.hud.unit_picker_open()
            && !self.hud.net.typing()
            && !cine
        {
            let mut pan = Vec2::ZERO;
            // A is also the Attack order: once it has armed Attack it aims, it does not pan.
            let a_pans = self.view.mode != Mode::Target(Targeting::Attack);
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
                if self.keys.contains(&key) && (key != KeyCode::KeyA || a_pans) {
                    pan += d;
                }
            }
            if pan != Vec2::ZERO {
                self.pan_camera(pan * dt * 900.0);
            }
            let tilt_up = self.keys.contains(&KeyCode::PageUp);
            let tilt_down = self.keys.contains(&KeyCode::PageDown);
            if tilt_up
                || tilt_down
                || self.keys.contains(&KeyCode::KeyQ)
                || self.keys.contains(&KeyCode::KeyE)
            {
                // Turning or tilting by hand is the player's angle: the drift would fight it.
                self.orbit_return = None;
            }
            if self.keys.contains(&KeyCode::KeyQ) {
                self.camera.yaw -= dt * 1.4;
            }
            if self.keys.contains(&KeyCode::KeyE) {
                self.camera.yaw += dt * 1.4;
            }
            if tilt_up {
                self.camera.orbit(0.0, dt * 0.8);
            }
            if tilt_down {
                self.camera.orbit(0.0, -dt * 0.8);
            }
        }
        if !cine {
            self.ease_zoom(dt);
            self.ease_orbit(dt);
        }

        // The range's weather: read from the settings once, then applied and
        // remembered whenever the panel changes it, and kept in the recording.
        // A recorded range being watched shows the weather it was recorded with.
        let sky = match self.view.range.as_mut() {
            Some(range) => Some(*range.sky.get_or_insert(settings.range_sky)),
            None => self.view.status.replay.and_then(|r| r.range_sky),
        };
        if let Some(sky) = sky.filter(|&s| self.view.sky_shown != Some(s)) {
            sky.show(renderer, &self.map);
            self.view.sky_shown = Some(sky);
            if self.view.range.is_some() {
                // The sim thread is gone only after a fatal error, which the HUD already shows.
                let _ = self.sim.notes.send(crate::recorder::Note::RangeSky(sky));
                if settings.range_sky != sky {
                    settings.range_sky = sky;
                    *settings_changed = true;
                }
            }
        }
        let fresh = self.pull_sim();
        self.rain_here = renderer.rain_here();
        self.thunder(renderer.take_thunder(), audio);
        self.ambience_frame(renderer, audio, dt, fresh);
        let alpha = ((now - self.published_at).as_secs_f32() / self.interp_span).clamp(0.0, 1.0);
        self.cine_alpha = alpha;
        if cine {
            self.cine_frame(renderer, dt, alpha);
            self.focus_eased_at = None;
        } else {
            self.follow_camera(alpha);
            if self.track.is_none() && self.orbit_unit.is_none() {
                self.ease_focus_height(renderer, dt);
            } else {
                self.focus_eased_at = None;
            }
        }
        self.cine.hand_back_apply(&mut self.camera, dt);
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

        // The sim publishes the order queues of the whole side: every command group wears
        // its count on the map. Only the selection's lines show without shift.
        let everyone = true;
        if !self
            .view
            .selection
            .iter()
            .take(MAX_WATCHED)
            .eq(self.watched.units.iter())
            || self.watched.side != self.view.local
            || self.watched.everyone != everyone
            || self.watched.perspective != self.view.perspective
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
                perspective: self.view.perspective,
            };
            self.sim.watch.lock().unwrap().clone_from(&self.watched);
        }
        self.answer_selection(audio, now);

        // The result, once: a stinger and the menu with the verdict on it.
        if let (Some(team), false) = (self.view.status.winner, self.result_shown) {
            self.result_shown = true;
            let heading = if self.view.observing {
                Heading::Complete
            } else if self
                .view
                .status
                .players
                .get(self.view.local as usize)
                .is_some_and(|p| p.team == team)
            {
                Heading::Victory
            } else {
                Heading::Defeat
            };
            // No stinger: a commander's end is its own detonation, and a ringing chord
            // over it cut across the blast (user ask, 2026-09-24).
            self.menu = None;
            self.open_menu(heading, audio);
        }

        // Selection and hover marks.
        let over_ui = self.menu.is_some() || self.hud.covers(self.cursor);
        let hover_unit = if self.view.mode == Mode::Normal && !over_ui {
            self.unit_at(self.cursor).and_then(|i| {
                let u = &self.view.frame.units[i];
                (u.owner_flags & KIND_WRECK == 0).then_some(i)
            })
        } else {
            None
        };
        let mut marks: Vec<Mark> = self
            .view
            .selection
            .iter()
            .filter_map(|id| self.view.index_of.get(id).copied())
            .map(|i| self.unit_mark(i, false))
            .collect();
        if let Some(i) = hover_unit {
            if !self
                .view
                .selection
                .contains(&self.view.frame.units[i].unit_id)
            {
                marks.push(self.unit_mark(i, true));
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
            let heading = self
                .blueprints
                .unit(blueprint)
                .build_heading()
                .to_radians_f32();
            let radius = self.blueprints.unit(blueprint).radius.to_f32();
            for &(pos, fit) in &sites {
                let valid = fit.is_ok();
                let xy = pos.to_f32();
                let z = renderer.surface_height(Vec2::from(xy));
                let p = [xy[0], xy[1], z];
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
            }
        }

        if let (Mode::Spawn | Mode::SpawnSubject, Some(range), false) =
            (self.view.mode, &self.view.range, over_ui)
        {
            if let Some(g) = self.ground_under_cursor(renderer) {
                let owner = if range.side == range::Side::Blue {
                    range::BLUE
                } else {
                    range::RED
                };
                let mobile_heading = if owner == range::BLUE {
                    0.0
                } else {
                    (Vec2::from(range.pad.to_f32()) - g.truncate()).to_angle()
                };
                let picked: Vec<&UnitInstance> = self
                    .view
                    .selection
                    .iter()
                    .filter_map(|id| self.view.index_of.get(id))
                    .map(|&i| &self.view.frame.units[i])
                    .filter(|u| u.owner_flags & KIND_WRECK == 0)
                    .collect();
                let n = picked.len().max(1) as f32;
                let cx = picked.iter().map(|u| u.pos[0]).sum::<f32>() / n;
                let cy = picked.iter().map(|u| u.pos[1]).sum::<f32>() / n;
                let ghosts_of: Vec<(u32, f32, [f32; 2])> =
                    if self.view.mode == Mode::SpawnSubject || picked.is_empty() {
                        vec![(
                            range.subject.0 as u32,
                            self.blueprints.unit(range.subject).radius.to_f32(),
                            [0.0, 0.0],
                        )]
                    } else {
                        picked
                            .iter()
                            .map(|u| (u.blueprint, u.radius, [u.pos[0] - cx, u.pos[1] - cy]))
                            .collect()
                    };
                let origin: [f32; 3] = g.into();
                for (blueprint, radius, offset) in ghosts_of {
                    let pos = [origin[0] + offset[0], origin[1] + offset[1], origin[2]];
                    let bp = self.blueprints.unit(BlueprintId(blueprint as u16));
                    let heading = if bp.built_on_site() {
                        bp.build_heading().to_radians_f32()
                    } else {
                        mobile_heading
                    };
                    ghosts.push(UnitInstance {
                        prev_pos: pos,
                        prev_heading: heading,
                        pos,
                        heading,
                        blueprint,
                        owner_flags: owner as u32 | KIND_GHOST,
                        health: 1.0,
                        build: 1.0,
                        turret_yaw: 0.0,
                        radius,
                        unit_id: u32::MAX,
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
                }
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
        let outlined = self.orders.ghosts(&field, &mut ghosts);
        // Walls in the preview join each other and the walls already standing.
        mc_sim::mirror::join_walls(&self.blueprints, &mut ghosts, &self.view.frame.units);
        self.pointer = if self.hud.free.on {
            self.free_camera_pointer()
        } else {
            self.pointer_for(renderer, over_ui, sites.iter().any(|(_, fit)| fit.is_ok()))
        };

        // Range rings: what the selection reaches, and what the thing being placed would.
        // Units in a lift ship's hold reach nothing until they are out.
        let selected = self
            .view
            .selection
            .iter()
            .filter_map(|id| self.view.index_of.get(id))
            .map(|&i| &self.view.frame.units[i])
            .filter(|u| !u.stored());
        self.rings.focus = self.hud.reach_focus.take();
        let (ranges, ranges_drawn) = self.rings.collect(
            ghosts[..placing]
                .iter()
                .chain(self.orders.ghost_in_hand(&ghosts[placing..]))
                .chain(selected),
            alpha,
            fresh,
            &|p| renderer.ground_height(Vec2::from(p)),
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
        self.music_frame(audio, dt, fresh);

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
        // The ground shocks of the giants shake the view (audio/titan.rs). What is drawn on
        // the ground (warhead and AEB marks, orders) shakes with it, or it would slide over
        // the ground; the panels and the minimap's footprint hold still.
        let shaken = self.giant_sounds.shaken(&self.camera);
        let field = Field {
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &shaken,
            renderer,
        };
        // The free camera's picture is clean: no orders, marks or reticles over it.
        if !self.hud.free.on {
            self.orders.draw(&mut ui, &field, alpha);
            crate::line_of_fire::draw_hidden(&mut ui, &field, alpha);
            // Warheads in flight, where they will land, and a launch being aimed (`nuke_marks.rs`).
            let placing = match self.view.mode {
                Mode::Place(bp) => sites
                    .last()
                    .map(|s| (bp, Vec2::new(s.0.x.to_f32(), s.0.y.to_f32()))),
                _ => None,
            };
            crate::nuke_marks::draw(
                &mut ui,
                &field,
                alpha,
                self.ground_under_cursor(renderer),
                placing,
            );
            crate::titan_marks::draw(&mut ui, &field, alpha, self.ground_under_cursor(renderer));
            if self.pointer == Pointer::Attack {
                if let Some(target) = self.unit_at(self.cursor) {
                    crate::line_of_fire::draw_hover(&mut ui, &field, target);
                }
            }
            orders::ghost_footprints(&mut ui, &field, &ghosts[..outlined]);
            self.orders.draw_pending(&mut ui, &field, self.cursor);
            if let Some(from) = self.left_down {
                if self.view.mode == Mode::Normal && from.distance(self.cursor) >= DRAG_THRESHOLD {
                    hud::drag_box(&mut ui, from, self.cursor);
                }
            }
        }
        let link = self.sim.net.as_ref().map(|n| n.link());
        let net_notices = self.sim.net.as_ref().map_or_else(Vec::new, |n| n.notices());
        if let Some(link) = &link {
            // A network match's pause is the relay's, whoever asked for it.
            self.view.paused = link.paused_by.is_some();
        }
        let scene = hud::Scene {
            net: link.as_ref(),
            net_notices: &net_notices,
            view: &self.view,
            blueprints: &self.blueprints,
            map: &self.map,
            camera: &self.camera,
            gpu: &renderer.stats,
            show_reclaim: self.ctrl && !self.alt && self.menu.is_none() && !self.hud.free.on,
            placing: sites.last().map(|s| s.0),
            hover: hover_unit
                .filter(|&i| self.can_inspect(&self.view.frame.units[i]))
                .map(|i| self.view.frame.units[i].unit_id),
        };
        let actions = self.hud.draw(&mut ui, &scene, dt);
        if !over_ui && !self.hud.free.on {
            hud::cursor_hint(&mut ui, &self.view, &self.blueprints, &sites);
            if self.pointer == Pointer::Board {
                self.board_hint(&mut ui);
            }
        }

        // The in-match menu, over everything; the settings screen over that.
        let mut event = None;
        // A match that is already over for this machine (out of step, link lost) is left, not given up.
        let surrender = link
            .as_ref()
            .is_some_and(|l| l.desync.is_none() && l.rejoining.is_none())
            && !self.view.observing
            && !self.defeated();
        if let Some(menu) = &mut self.menu {
            menu.enter =
                (menu.enter + if menu.closing { -dt / 0.14 } else { dt / 0.28 }).clamp(0.0, 1.0);
            let in_settings = menu.settings.is_some();
            ui.interactive = !menu.closing && !in_settings && !menu.fresh;
            menu.fresh = false;
            let eased = 1.0 - (1.0 - menu.enter).powi(3);
            let out = pause::draw(
                &mut ui,
                menu.heading,
                self.view.status.owns_clock,
                surrender,
                settings,
                eased,
            );
            *settings_changed |= out.settings_changed;
            match out.action {
                Some(PauseAction::Resume) => menu.closing = true,
                Some(PauseAction::Settings) => menu.settings = Some((0.0, false)),
                Some(PauseAction::Leave) if surrender => {
                    // Leaving a network match gives it up first: the side is defeated on
                    // every machine rather than left standing idle.
                    if let Some(net) = &self.sim.net {
                        net.request(crate::netplay::NetRequest::Surrender);
                    }
                    menu.closing = true;
                }
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
        // A surrender goes out with the next turn; the match is left once it has been
        // carried out, so every machine sees this side fall rather than stand idle.
        if link
            .as_ref()
            .and_then(|l| l.surrendering)
            .is_some_and(|since| self.defeated() || since.elapsed() > SURRENDER_WAIT)
        {
            event = Some(GameEvent::Leave);
        }
        if let Some(cover) = cover {
            cover.draw(&mut ui);
        }
        for action in actions {
            if action == HudAction::Leave {
                event = Some(GameEvent::Leave);
                continue;
            }
            self.hud_action(action, audio);
        }

        // Ore shows its veins while a mine is placed or selected, or with Ctrl.
        let placing_mine =
            matches!(self.view.mode, Mode::Place(bp) if self.blueprints.unit(bp).mine.is_some());
        let mine_selected = self.selected_units().any(|u| {
            self.blueprints
                .unit(BlueprintId(u.blueprint as u16))
                .mine
                .is_some()
        });
        let survey = !self.hud.free.on
            && (placing_mine || mine_selected || (self.ctrl && !self.alt && self.menu.is_none()));
        renderer.set_ore_highlight(if survey { 1.0 } else { 0.0 });
        renderer.set_ore_tapped(&hud::ore_tapped(
            &self.map,
            &self.blueprints,
            &self.view.frame.units,
        ));
        let build_grid = matches!(self.view.mode, Mode::Place(_)) || self.orders.dragging_plan();
        if build_grid {
            let field = Field {
                view: &self.view,
                blueprints: &self.blueprints,
                map: &self.map,
                camera: &self.camera,
                renderer,
            };
            let focus = orders::build_grid_focus(&field, self.cursor, self.orders.plan_in_hand());
            let (centre, radius, lots) = focus.unwrap_or((Vec2::ZERO, 0.0, Vec::new()));
            renderer.set_build_grid(centre, radius, &lots);
        }
        let clean = self.hud.free.on;
        // The world is drawn on the match's clock, not the wall's: the moment the units
        // are drawn at (between the last two ticks). Every effect ages by it, so a blast
        // or a beam lasts as long in game time at any game speed, and holds while paused.
        let world_time = ((self.view.frame.tick as f32 - 1.0 + alpha) * TICK_SECONDS).max(0.0);
        let frame = FrameInput {
            camera: &shaken,
            time: world_time,
            alpha,
            sim: fresh.then_some(&self.view.frame),
            ghosts: if clean { &[] } else { &ghosts },
            marks: if clean { &[] } else { &marks },
            ranges: if clean { &[] } else { &ranges },
            ranges_drawn: if clean { 0 } else { ranges_drawn },
            overlay,
            build_grid,
        };
        self.hud.issues.pump_capture(renderer);
        renderer
            .render(&frame)
            .map_err(|e| format!("rendering failed: {e}"))?;
        self.view.cpu_ms = self.view.cpu_ms * 0.9 + now.elapsed().as_secs_f32() * 100.0;
        Ok(event.or(self.event.take()))
    }
}

/// Hovered mark; selected is this bit off.
const MARK_HOVER: u32 = 1;
/// Hostile mark: the ground brackets go red.
const MARK_ENEMY: u32 = 2;

fn mark_kind(hover: bool, enemy: bool) -> u32 {
    (if hover { MARK_HOVER } else { 0 }) | (if enemy { MARK_ENEMY } else { 0 })
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
    // A strategic launcher assembles its rounds like a factory: its bar follows the
    // round on the line.
    if let Some(l) = crate::hud::silo::Launcher::of(u) {
        return if l.assembling() { l.progress } else { -1.0 };
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

/// Shield fill for the unit bar, or a negative if the unit has no bubble.
pub(crate) fn unit_bar_shield(unit_id: u32, shields: &[ShieldInstance]) -> f32 {
    shields
        .iter()
        .find(|s| s.unit_id == unit_id)
        .map_or(-1.0, |s| s.health.clamp(0.0, 1.0))
}

/// Unit under the cursor, else the first of the selection.
fn track_target(cursor: Option<u32>, selection: &[u32]) -> Option<u32> {
    cursor.or_else(|| selection.first().copied())
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
                formation: 0,
                offset: [0.0; 2],
                moving_slot: None,
                formation_phase: 0,
                kind: OrderKind::Produce,
                pos: [0.0; 2],
                at: FxVec2::ZERO,
                blueprint: BlueprintId(0),
                radius: 0.0,
            }],
            progress: 0.55,
            ..Default::default()
        }];
        assert!((unit_bar_work(&factory, &queues) - 0.55).abs() < 1e-6);

        let mut refit = dummy(4, 1, 0, [0.0; 3], 0);
        refit.upgrade = 0.2;
        assert!((unit_bar_work(&refit, &[]) - 0.2).abs() < 1e-6);

        let wreck = dummy(5, 1, 0, [0.0; 3], KIND_WRECK);
        assert!(unit_bar_work(&wreck, &[]) < 0.0);

        // A launcher assembling its second round shows it; a full one shows nothing.
        use mc_sim::nukes::{LAUNCHER_CAPACITY_SHIFT, LAUNCHER_MARK, LAUNCHER_PROGRESS_SHIFT};
        let mut silo = dummy(6, 1, 0, [0.0; 3], 0);
        silo.status[2] =
            LAUNCHER_MARK | 1 | 2 << LAUNCHER_CAPACITY_SHIFT | 128 << LAUNCHER_PROGRESS_SHIFT;
        assert!((unit_bar_work(&silo, &[]) - 128.0 / 255.0).abs() < 1e-6);
        silo.status[2] = LAUNCHER_MARK | 2 | 2 << LAUNCHER_CAPACITY_SHIFT;
        assert!(unit_bar_work(&silo, &[]) < 0.0);
    }

    #[test]
    fn unit_bar_shield_follows_the_bubble() {
        let shields = [ShieldInstance {
            pos: [0.0; 3],
            radius: 100.0,
            prev_open: 1.0,
            open: 1.0,
            health: 0.4,
            packed: 0,
            unit_id: 3,
            projector: 0.0,
            height: 0.0,
            prev_radius: 0.0,
        }];
        assert!((unit_bar_shield(3, &shields) - 0.4).abs() < 1e-6);
        assert!(unit_bar_shield(9, &shields) < 0.0);
        let empty = [ShieldInstance {
            health: 0.0,
            ..shields[0]
        }];
        assert_eq!(unit_bar_shield(3, &empty), 0.0);
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

    #[test]
    fn t_tracks_the_unit_under_the_cursor_else_the_selection() {
        assert_eq!(track_target(Some(7), &[1, 2]), Some(7));
        assert_eq!(track_target(None, &[1, 2]), Some(1));
        assert_eq!(track_target(None, &[]), None);
    }

    #[test]
    fn a_mark_names_hover_and_hostility() {
        assert_eq!(mark_kind(false, false), 0);
        assert_eq!(mark_kind(true, false), MARK_HOVER);
        assert_eq!(mark_kind(false, true), MARK_ENEMY);
        assert_eq!(mark_kind(true, true), MARK_HOVER | MARK_ENEMY);
    }
}
