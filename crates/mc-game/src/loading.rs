//! Loading a map without freezing the window.
//!
//! A new renderer is prepared on a worker thread (`Job`) while the outgoing one
//! keeps drawing the loading screen (`Curtain`) every frame; the unit pictures
//! and the map chart are drawn on threads of their own beside it. When the
//! build is done the window changes hands in one short step, and the curtain
//! stays up over the new stage until its simulation is running and its frames
//! come steadily, then lifts off it.

use crate::hud::thumbs::{Baked, Thumbs};
use crate::ui::{self, ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Renderer, SceneDesc, Target};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

/// Moves a value that holds raw window or GPU handles to another thread. Vulkan
/// objects may be used from any thread as long as only one uses them at a time,
/// which handing them over guarantees.
pub struct Unsend<T>(T);
unsafe impl<T> Send for Unsend<T> {}

impl<T> Unsend<T> {
    pub fn new(value: T) -> Unsend<T> {
        Unsend(value)
    }

    /// A method rather than `.0`, so a closure captures the whole wrapper.
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Drops a renderer that has given up the window on a thread of its own: taking
/// a device down can take a while, and the window has frames to draw.
pub fn retire(renderer: Renderer) {
    let old = Unsend::new(renderer);
    let _ = std::thread::Builder::new()
        .name("mc-retire".into())
        .spawn(move || drop(old.into_inner()));
}

pub enum MapSource {
    Loaded(Arc<MapFile>),
    /// The front end's backdrop map, opened on the worker.
    Backdrop,
}

pub struct Order {
    pub target: Target,
    pub map: MapSource,
    pub blueprints: Arc<Blueprints>,
    pub pool: Arc<Pool>,
    pub colors: [[f32; 3]; 8],
    /// Draw the unit pictures, in this side's colours.
    pub pictures: Option<[f32; 3]>,
}

/// What the worker hands back.
pub struct Ready {
    /// Prepared but not attached: `Renderer::attach` it once the old one has let go.
    pub renderer: Renderer,
    pub map: Arc<MapFile>,
    pub chart: Vec<u8>,
    pub thumbs: Option<Baked>,
}

#[derive(Default)]
struct Shared {
    step: &'static str,
    done: f32,
    chart: Option<Vec<u8>>,
}

pub struct Job {
    shared: Arc<Mutex<Shared>>,
    result: Receiver<Result<Unsend<Ready>, String>>,
}

impl Job {
    pub fn start(order: Order) -> Job {
        let shared: Arc<Mutex<Shared>> = Arc::default();
        let (tx, result) = std::sync::mpsc::channel();
        let order = Unsend::new(order);
        let out = shared.clone();
        std::thread::Builder::new()
            .name("mc-load".into())
            .spawn(move || {
                // Loading is allowed to take longer; the frames are not.
                crate::app::set_this_thread_priority(-2);
                let started = std::time::Instant::now();
                let ready = build(order.into_inner(), &out);
                log::info!(
                    "loaded in the background in {:.0} ms",
                    started.elapsed().as_secs_f32() * 1000.0
                );
                let _ = tx.send(ready.map(Unsend::new));
            })
            .expect("could not start the loading thread");
        Job { shared, result }
    }

    /// The step under way and how far the build has got, 0..1.
    pub fn progress(&self) -> (&'static str, f32) {
        let s = self.shared.lock().unwrap();
        (s.step, s.done)
    }

    /// The map chart, once, as soon as it is drawn.
    pub fn take_chart(&self) -> Option<Vec<u8>> {
        self.shared.lock().unwrap().chart.take()
    }

    pub fn finished(&self) -> Option<Result<Ready, String>> {
        match self.result.try_recv() {
            Ok(r) => Some(r.map(Unsend::into_inner)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("the loading thread died".into())),
        }
    }
}

fn build(order: Order, shared: &Mutex<Shared>) -> Result<Ready, String> {
    let set = |step: &'static str, done: f32| {
        let mut s = shared.lock().unwrap();
        s.step = step;
        s.done = done;
    };
    let map = match order.map {
        MapSource::Loaded(map) => map,
        MapSource::Backdrop => {
            set("Reading the map", 0.0);
            let path = crate::setup::backdrop_map().ok_or("no maps found. Bake one with: cargo run --release -p mc-map --bin mc-bake -- --layout islands --size-km 10 --seed 46 --name \"Twin Shoals\" -o maps/twin_shoals.mcmap")?;
            Arc::new(MapFile::open(&path).map_err(|e| format!("{}: {e}", path.display()))?)
        }
    };
    let blueprints = &order.blueprints;
    std::thread::scope(|s| {
        let chart = s.spawn(|| {
            crate::app::set_this_thread_priority(-2);
            let chart = ui::preview::render(&map);
            shared.lock().unwrap().chart = Some(chart.clone());
            chart
        });
        let pictures = order
            .pictures
            .map(|team| {
                s.spawn(move || {
                    crate::app::set_this_thread_priority(-2);
                    Thumbs::render(blueprints, team)
                })
            });
        let scene = SceneDesc {
            map: map.clone(),
            blueprints: blueprints.clone(),
            pool: order.pool.clone(),
            team_colors: order.colors,
        };
        let renderer = Renderer::prepare(order.target, scene, &set)
            .map_err(|e| format!("could not start the renderer: {e}"))?;
        if pictures.as_ref().is_some_and(|p| !p.is_finished()) {
            set("Drawing unit pictures", 1.0);
        }
        let thumbs = match pictures {
            Some(p) => Some(p.join().map_err(|_| "drawing the unit pictures failed")?),
            None => None,
        };
        let chart = chart.join().map_err(|_| "drawing the map chart failed")?;
        Ok(Ready {
            renderer,
            map: map.clone(),
            chart,
            thumbs,
        })
    })
}

// -- the loading screen ------------------------------------------------------------

/// The share of the bar the renderer build fills; the rest is the new stage
/// getting going under the curtain.
const BUILD_SHARE: f32 = 0.84;
const DEPLOYING: &str = "Deploying commanders";
const STEADYING: &str = "Steadying the view";
/// Frames in a row that must come on time before the curtain lifts, and the
/// most it waits for them.
const STEADY_FRAMES: u32 = 30;
const STEADY_DT: f32 = 1.0 / 40.0;
const STEADY_WAIT: f32 = 6.0;
/// Seconds the curtain takes to arrive and to lift.
const ARRIVE: f32 = 0.35;
const LIFT: f32 = 1.1;
/// The build starts once the screen has arrived and its titles have slid in.
const SETTLED_IN: f32 = 0.7;
/// Frames in a row that must come on time after the handover before the
/// screen moves again.
const RESUME_FRAMES: u32 = 4;
/// The longest the screen stays still after the handover.
const HOLD_MOST: f32 = 1.5;

/// When the screen holds still. Some steps stall the window for a frame or a
/// few (a graphics device starting, the window changing hands, a renderer's
/// first frame) and nothing can be drawn meanwhile; a stall is invisible
/// while nothing moves, so the screen eases to rest before each one and
/// picks up again after.
#[derive(PartialEq)]
enum Hold {
    /// Until the new graphics device is up.
    Start,
    /// From the end of the build until the new stage draws steadily. The bar
    /// catches up with the build first, then everything comes to rest.
    Handover { steady: u32 },
    Moving,
}

#[derive(PartialEq)]
enum Phase {
    /// The worker is building; the old renderer draws.
    Building,
    /// The new stage runs underneath.
    Settling,
    Lifting { since: f32 },
    Gone,
}

struct Step {
    name: &'static str,
    /// When it began and, once over, ended.
    began: f32,
    ended: Option<f32>,
}

pub struct Curtain {
    /// The screen it arrives over was black already (the front end fades out
    /// before a match), rather than a stage to fade over.
    from_black: bool,
    title: String,
    detail: String,
    map_name: String,
    size_km: Vec2,
    /// Start positions across the chart, 0..1, and which one is ours.
    starts: Vec<Vec2>,
    ours: Option<usize>,
    /// When it was first drawn, and when the chart arrived.
    born: Option<f32>,
    chart_at: Option<f32>,
    reported: f32,
    reported_at: f32,
    shown: f32,
    steps: Vec<Step>,
    phase: Phase,
    settle_since: f32,
    steady: u32,
    /// Wall-clock seconds, and the screen's own clock: it runs at `motion`
    /// speed, and every animation on the screen is driven by it.
    now: f32,
    clock: f32,
    motion: f32,
    hold: Hold,
    /// Frames drawn under it, the slowest, and how many missed 25 ms.
    frames: u32,
    worst: f32,
    late: u32,
    /// Frames when the current step began, and its slowest.
    step_frames: (u32, f32),
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Curtain {
    pub fn new(title: &str, detail: &str, from_black: bool) -> Curtain {
        Curtain {
            from_black,
            title: title.to_owned(),
            detail: detail.to_owned(),
            map_name: String::new(),
            size_km: Vec2::ZERO,
            starts: Vec::new(),
            ours: None,
            born: None,
            chart_at: None,
            reported: 0.0,
            reported_at: 0.0,
            shown: 0.0,
            steps: Vec::new(),
            phase: Phase::Building,
            settle_since: 0.0,
            steady: 0,
            now: 0.0,
            clock: 0.0,
            motion: 0.0,
            hold: Hold::Start,
            frames: 0,
            worst: 0.0,
            late: 0,
            step_frames: (0, 0.0),
        }
    }

    /// The map being loaded, and the start position that is ours (if we play).
    pub fn set_map(&mut self, map: &MapFile, ours: Option<usize>) {
        self.map_name = map.name().to_owned();
        let size = Vec2::from(map.info().size_metres().to_f32());
        self.size_km = size / 1000.0;
        // The chart is square and letterboxed: positions are over the longer side.
        let side = size.x.max(size.y).max(1.0);
        let pad = (Vec2::splat(side) - size) * 0.5;
        self.starts = map
            .start_positions()
            .iter()
            .map(|p| (Vec2::from(p.to_f32()) + pad) / side)
            .collect();
        self.ours = ours;
    }

    /// The chart is in the overlay's image slot now.
    pub fn chart_ready(&mut self) {
        self.chart_at.get_or_insert(self.clock);
    }

    /// Whether the build may start: the screen is up and still.
    pub fn ready_to_build(&self) -> bool {
        self.born.is_some_and(|b| self.now - b >= SETTLED_IN)
    }

    /// The build's own progress, 0..1 of its share of the bar.
    pub fn building(&mut self, step: &'static str, done: f32) {
        if self.hold != Hold::Start && self.hold != Hold::Moving {
            return;
        }
        // The device is up once the build is past starting it.
        if self.hold == Hold::Start
            && !matches!(step, "" | "Reading the map" | "Waking the graphics card")
        {
            self.hold = Hold::Moving;
        }
        self.report(step, done * BUILD_SHARE);
    }

    /// The build is done: come to rest for the handover.
    pub fn built(&mut self) {
        self.hold = Hold::Handover { steady: 0 };
        self.report("Taking over the display", BUILD_SHARE);
    }

    /// At rest, bar included: the window can change hands unseen.
    pub fn still(&self) -> bool {
        self.motion == 0.0 && (self.goal() - self.shown).abs() < 0.001
    }

    /// The window changed hands: the new stage now runs underneath.
    pub fn handed_over(&mut self) {
        self.phase = Phase::Settling;
        self.settle_since = self.now;
        self.report(DEPLOYING, BUILD_SHARE + 0.02);
    }

    /// Once a frame after the handover: whether the new stage's simulation has
    /// ticked yet, and how long the frame took.
    pub fn settle(&mut self, dt: f32, sim_running: bool) {
        if self.born.is_some() && self.phase != Phase::Gone {
            self.frames += 1;
            self.worst = self.worst.max(dt);
            self.step_frames.1 = self.step_frames.1.max(dt);
            self.late += (dt > 0.025) as u32;
        }
        if self.phase != Phase::Settling {
            return;
        }
        let waited = self.now - self.settle_since;
        // Still until the simulation runs and the frames come on time: its
        // first frames are the heavy ones. A simulation slow to start does not
        // keep the screen frozen long.
        if let Hold::Handover { steady } = &mut self.hold {
            *steady = if dt < STEADY_DT && sim_running { *steady + 1 } else { 0 };
            if *steady >= RESUME_FRAMES || waited > HOLD_MOST {
                self.hold = Hold::Moving;
            }
        }
        if self.current() == Some(DEPLOYING) {
            if sim_running || waited > 30.0 {
                self.report(STEADYING, 0.94);
                self.steady = 0;
            }
            return;
        }
        if self.hold != Hold::Moving {
            return;
        }
        self.steady = if dt < STEADY_DT { self.steady + 1 } else { 0 };
        if self.steady >= STEADY_FRAMES || waited > STEADY_WAIT {
            self.report("Ready", 1.0);
        }
        // The bar reaches the end before the curtain goes.
        if self.reported >= 1.0 && self.shown > 0.995 {
            self.phase = Phase::Lifting { since: self.now };
            for s in &mut self.steps {
                s.ended.get_or_insert(self.now);
            }
        }
    }

    /// Whether it still hides the stage: input is kept from it until then.
    pub fn covering(&self) -> bool {
        match self.phase {
            Phase::Lifting { since } => self.now - since < LIFT * 0.4,
            Phase::Gone => false,
            _ => true,
        }
    }

    pub fn gone(&self) -> bool {
        self.phase == Phase::Gone
    }

    fn current(&self) -> Option<&'static str> {
        self.steps.last().filter(|s| s.ended.is_none()).map(|s| s.name)
    }

    fn report(&mut self, step: &'static str, done: f32) {
        if !step.is_empty() && self.steps.last().map(|s| s.name) != Some(step) {
            if let Some(last) = self.steps.last() {
                log::debug!(
                    "loading: {} {:.0} ms ({} frames, slowest {:.0} ms)",
                    last.name,
                    (self.clock - last.began) * 1000.0,
                    self.frames - self.step_frames.0,
                    self.step_frames.1 * 1000.0
                );
            }
            self.step_frames = (self.frames, 0.0);
            for s in &mut self.steps {
                s.ended.get_or_insert(self.clock);
            }
            self.steps.push(Step {
                name: step,
                began: self.clock,
                ended: (step == "Ready").then_some(self.clock),
            });
        }
        if done > self.reported {
            self.reported = done;
            self.reported_at = self.clock;
        }
    }

    /// Where the bar is heading: what was reported, crept on a little while
    /// nothing new arrives so it never sits dead still (on the screen's clock,
    /// so it rests with everything else).
    fn goal(&self) -> f32 {
        if self.reported >= 1.0 {
            return 1.0;
        }
        let creep = ((self.clock - self.reported_at) * 0.004).min(0.03);
        (self.reported + creep).min(0.99)
    }

    pub fn draw(&mut self, ui: &mut Ui) {
        self.now = ui.time;
        let born = *self.born.get_or_insert(ui.time);
        let age = ui.time - born;

        // Motion eases down to rest and back up; the screen's clock runs at its pace.
        let catching_up = matches!(self.hold, Hold::Handover { .. })
            && self.phase == Phase::Building
            && self.goal() - self.shown > 0.001;
        let moving = if self.hold == Hold::Moving || catching_up { 1.0 } else { 0.0 };
        self.motion += (moving - self.motion) * (1.0 - (-5.0 * ui.dt).exp());
        if (self.motion - moving).abs() < 0.01 {
            self.motion = moving;
        }
        self.clock += ui.dt * self.motion;

        let goal = self.goal();
        let rate = if self.reported >= 1.0 || catching_up { 7.0 } else { 3.0 };
        let step = (goal - self.shown) * (1.0 - (-rate * ui.dt * self.motion).exp());
        self.shown = (self.shown + step).max(self.shown);
        if goal - self.shown < 0.003 {
            self.shown = self.shown.max(goal);
        }

        let lift = match self.phase {
            Phase::Lifting { since } => {
                let t = (ui.time - since) / LIFT;
                if t >= 1.0 {
                    self.phase = Phase::Gone;
                    log::info!(
                        "loading screen: {:.1} s, {} frames, slowest {:.0} ms, {} over 25 ms",
                        ui.time - self.born.unwrap_or(ui.time),
                        self.frames,
                        self.worst * 1000.0,
                        self.late
                    );
                    return;
                }
                t
            }
            Phase::Gone => return,
            _ => 0.0,
        };
        let saved = (ui.fade, ui.shift, ui.interactive, ui.time);
        ui.interactive = false;
        if self.from_black && self.phase == Phase::Building {
            ui.fill(Rect::new(0.0, 0.0, ui.size.x, ui.size.y), ink(1.0));
        }
        ui.fade = saved.0 * ease_out(age / ARRIVE) * (1.0 - ease_in_out(lift));
        // The toolkit's own animations (the emblem) keep the screen's time too.
        ui.time = self.clock;
        self.scene(ui, age, lift);
        (ui.fade, ui.shift, ui.interactive, ui.time) = saved;
    }

    fn scene(&mut self, ui: &mut Ui, age: f32, lift: f32) {
        let (w, h) = (ui.size.x, ui.size.y);
        let margin = (w * 0.07).clamp(48.0, 160.0);
        ui.fill(Rect::new(0.0, 0.0, w, h), ink(1.0));
        self.chart(ui, w, h, lift);
        // Black from the left over the chart, so the words stand on it.
        ui.scrim(Rect::new(0.0, 0.0, w * 0.62, h), 1.0, 0.0, true);
        ui.scrim(Rect::new(0.0, h * 0.72, w, h * 0.28), 0.0, 0.92, false);
        ui.scrim(Rect::new(0.0, 0.0, w, h * 0.16), 0.7, 0.0, false);

        // Brand, top left, where the match's economy panel sits.
        let top = 56.0;
        ui.emblem(Vec2::new(margin + 12.0, top + 14.0), 11.0, rgb(palette::TEXT, 0.85));
        let x = ui.text(
            margin + 36.0,
            top + 5.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 0.9),
            "Meridian",
        );
        ui.text(
            x + 5.0,
            top + 5.0,
            ui::style(mc_render::Face::Light, 13.5, 0.4),
            rgb(palette::DIM, 0.9),
            "Conflict",
        );

        // What is loading.
        let y = h * 0.31;
        let slide = (1.0 - ease_out(age / 0.7)) * 18.0;
        ui.fill(Rect::new(margin, y - 9.0 + slide, 3.0, 18.0), rgb(palette::ACCENT, 1.0));
        ui.text(
            margin + 16.0,
            y + slide,
            type_scale::OVERLINE,
            rgb(palette::ACCENT, 1.0),
            &self.title,
        );
        let name = if self.map_name.is_empty() {
            self.detail.clone()
        } else {
            self.map_name.clone()
        };
        ui.text(
            margin - 3.0,
            y + 52.0 + slide * 1.4,
            ui::style(mc_render::Face::Light, 64.0, 0.5),
            rgb(palette::TEXT, 1.0),
            &name,
        );
        if !self.map_name.is_empty() {
            ui.text(
                margin,
                y + 104.0 + slide * 1.8,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                &self.detail,
            );
        }
        if self.size_km.x > 0.0 {
            let facts = [
                ("Area", format!("{:.0} \u{d7} {:.0} km", self.size_km.x, self.size_km.y)),
                ("Starts", format!("{}", self.starts.len())),
            ];
            let mut fx = margin;
            for (label, value) in facts {
                let fy = y + 150.0 + slide * 2.0;
                ui.text(fx, fy, type_scale::MICRO, rgb(palette::FAINT, 1.0), label);
                let end = ui.text(fx, fy + 22.0, type_scale::VALUE, rgb(palette::TEXT, 0.95), &value);
                fx = end.max(fx + 60.0) + 36.0;
            }
        }

        self.step_list(ui, margin, y + 250.0);
        self.bar(ui, margin, w - margin, h - 92.0);
    }

    /// The chart of the map, drifting slowly, with its grid and the start
    /// positions pulsing on it.
    fn chart(&self, ui: &mut Ui, w: f32, h: f32, lift: f32) {
        let Some(at) = self.chart_at else { return };
        let arrive = ease_out((self.clock - at) / 1.2);
        let age = self.clock;
        let side = (h * 0.74).min(w * 0.5) * (1.0 + 0.035 * ease_in_out(age / 24.0) + 0.06 * ease_in_out(lift));
        let centre = Vec2::new(w * 0.69, h * 0.445);
        let r = Rect::new(centre.x - side * 0.5, centre.y - side * 0.5, side, side);
        let size = ui::preview::SIZE as f32;
        ui.image(
            crate::hud::MINIMAP_SLOT,
            [0.0, 0.0, size, size],
            r,
            [1.0, 1.0, 1.0, 0.5 * arrive],
        );
        // The sheet sinks into the dark at its edges.
        let (edge, cap) = (side * 0.2, side * 0.12);
        ui.scrim(Rect::new(r.x, r.y, edge, r.h), 0.95, 0.0, true);
        ui.scrim(Rect::new(r.right() - edge, r.y, edge, r.h), 0.0, 0.8, true);
        ui.scrim(Rect::new(r.x, r.y, r.w, cap), 0.8, 0.0, false);
        ui.scrim(Rect::new(r.x, r.bottom() - cap, r.w, cap), 0.0, 0.9, false);

        // A survey grid, lettered like a map sheet.
        let cells = 8;
        for i in 0..=cells {
            let k = i as f32 / cells as f32;
            let a = if i == 0 || i == cells { 0.16 } else { 0.06 } * arrive;
            ui.vline(r.x + r.w * k, r.y, r.h, rgb(palette::LINE, a));
            ui.hline(r.x, r.y + r.h * k, r.w, rgb(palette::LINE, a));
            if i < cells {
                let mid = (i as f32 + 0.5) / cells as f32;
                let letter = ((b'A' + i as u8) as char).to_string();
                ui.text_centred(r.x + r.w * mid, r.y - 12.0, type_scale::MICRO, rgb(palette::FAINT, arrive), &letter);
                ui.text_right(r.right() + 18.0, r.y + r.h * mid + 4.0, type_scale::MICRO, rgb(palette::FAINT, arrive), &format!("{}", i + 1));
            }
        }
        ui.brackets(r.inset(-10.0), 18.0, rgb(palette::LINE, 0.55 * arrive));

        // A survey line sweeping down the sheet.
        let sweep = (age * 0.11).fract();
        let sy = r.y + r.h * sweep;
        let edge = (sweep * 10.0).min((1.0 - sweep) * 10.0).min(1.0);
        ui.gradient_h(
            Rect::new(r.x, sy, r.w * 0.5, 1.0),
            rgb(palette::ACCENT, 0.0),
            rgb(palette::ACCENT, 0.5 * arrive * edge),
        );
        ui.gradient_h(
            Rect::new(r.x + r.w * 0.5, sy, r.w * 0.5, 1.0),
            rgb(palette::ACCENT, 0.5 * arrive * edge),
            rgb(palette::ACCENT, 0.0),
        );

        for (i, p) in self.starts.iter().enumerate() {
            let c = Vec2::new(r.x + r.w * p.x, r.y + r.h * p.y);
            let ours = self.ours == Some(i);
            let hue = if ours { palette::ACCENT } else { palette::TEXT };
            let pulse = (age * 0.6 + i as f32 * 0.37).fract();
            ui.arc(c, 6.0 + 22.0 * ease_out(pulse), 0.0, std::f32::consts::TAU, 1.2, rgb(hue, 0.55 * (1.0 - pulse) * arrive));
            ui.arc(c, 7.0, 0.0, std::f32::consts::TAU, 1.4, rgb(hue, 0.9 * arrive));
            ui.disc(c, 2.6, rgb(hue, arrive));
            ui.text(c.x + 13.0, c.y - 7.0, type_scale::MICRO, rgb(hue, 0.85 * arrive), &format!("{}", i + 1));
        }
    }

    /// What has been done and what is under way, newest last.
    fn step_list(&self, ui: &mut Ui, x: f32, y: f32) {
        const ROW: f32 = 30.0;
        const SHOWN: usize = 6;
        ui.section(x, y, 340.0, "Progress");
        let first = self.steps.len().saturating_sub(SHOWN);
        // Rows slide up as a new one arrives.
        let newest = self.steps.last().map_or(0.0, |s| ease_out((self.clock - s.began) / 0.35));
        let scroll = if self.steps.len() > SHOWN { 1.0 - newest } else { 0.0 };
        for (row, step) in self.steps.iter().enumerate().skip(first.saturating_sub(1)) {
            let slot = row as f32 - first as f32 + scroll;
            if slot < -0.99 {
                continue;
            }
            let ry = y + 30.0 + slot * ROW;
            let appear = ease_out((self.clock - step.began) / 0.35);
            let leave = if slot < 0.0 { 1.0 + slot * 2.5 } else { 1.0 };
            let a = appear * leave.clamp(0.0, 1.0);
            let dx = (1.0 - appear) * 14.0;
            let mark = Vec2::new(x + 6.0 + dx, ry);
            match step.ended {
                None => {
                    let t = self.clock * 5.0;
                    ui.arc(mark, 6.0, t, t + 4.2, 1.6, rgb(palette::ACCENT, a));
                    ui.text(x + 26.0 + dx, ry + 5.0, type_scale::ITEM, rgb(palette::TEXT, a), step.name);
                    // A faint count of the time it has taken so far.
                    let secs = self.clock - step.began;
                    if secs > 1.5 {
                        let end = x + 26.0 + dx + ui.text_width(type_scale::ITEM, step.name);
                        ui.text(end + 12.0, ry + 4.0, type_scale::MICRO, rgb(palette::FAINT, a), &format!("{secs:.0} s"));
                    }
                }
                Some(ended) => {
                    let done = ease_out((self.clock - ended) / 0.3);
                    let tint = rgb(palette::DIM, a * (1.0 - 0.35 * done));
                    ui.stroke(mark + Vec2::new(-4.0, 0.5), mark + Vec2::new(-1.0, 3.5), 1.6, tint);
                    ui.stroke(mark + Vec2::new(-1.0, 3.5), mark + Vec2::new(5.0, -3.5), 1.6, tint);
                    ui.text(x + 26.0 + dx, ry + 5.0, type_scale::BODY, tint, step.name);
                }
            }
        }
    }

    /// The progress bar across the foot of the screen.
    fn bar(&self, ui: &mut Ui, x0: f32, x1: f32, y: f32) {
        let w = x1 - x0;
        let p = self.shown.clamp(0.0, 1.0);
        let fill = w * p;
        let label = match self.current() {
            Some(step) => step,
            None if self.steps.is_empty() => "Preparing",
            None => "Ready",
        };
        ui.text(x0, y - 22.0, type_scale::MICRO, rgb(palette::FAINT, 1.0), "Loading");
        ui.text(x0 + 64.0, y - 22.0, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
        let pct = format!("{:.0}", p * 100.0);
        let right = ui.text_width(type_scale::CAPTION, "%");
        ui.text_right(x1, y - 16.0, type_scale::CAPTION, rgb(palette::DIM, 1.0), "%");
        ui.text_right(
            x1 - right - 3.0,
            y - 14.0,
            ui::style(mc_render::Face::Light, 36.0, 0.5),
            rgb(palette::TEXT, 1.0),
            &pct,
        );

        // Track and scale.
        ui.fill(Rect::new(x0, y, w, 3.0), rgb(palette::LINE, 0.08));
        for i in 0..=20 {
            let k = i as f32 / 20.0;
            let major = i % 5 == 0;
            let lit = k <= p;
            ui.vline(
                x0 + w * k,
                y + 8.0,
                if major { 7.0 } else { 3.0 },
                rgb(if lit { palette::ACCENT } else { palette::LINE }, if major { 0.5 } else { 0.22 }),
            );
        }
        // The fill: deep at its tail, bright toward its head, with a glint
        // running along it.
        ui.gradient_h(
            Rect::new(x0, y, fill, 3.0),
            rgb(palette::ACCENT_DEEP, 0.9),
            rgb(palette::ACCENT, 1.0),
        );
        let glint = (self.clock * 0.55).fract();
        let gw = 140.0f32.min(fill);
        let gx = x0 + (fill + gw) * glint - gw;
        let (gx0, gx1) = (gx.max(x0), (gx + gw).min(x0 + fill));
        if gx1 > gx0 {
            let mid = gx0 + (gx1 - gx0) * 0.5;
            ui.gradient_h(Rect::new(gx0, y, mid - gx0, 3.0), rgb(palette::TEXT, 0.0), rgb(palette::TEXT, 0.55));
            ui.gradient_h(Rect::new(mid, y, gx1 - mid, 3.0), rgb(palette::TEXT, 0.55), rgb(palette::TEXT, 0.0));
        }
        // The head: a soft bloom and a white tick.
        if p > 0.0 {
            let hx = x0 + fill;
            for (reach, a) in [(46.0, 0.10), (22.0, 0.18), (9.0, 0.30)] {
                ui.gradient_h(Rect::new(hx - reach, y - 3.0, reach, 9.0), rgb(palette::ACCENT, 0.0), rgb(palette::ACCENT, a));
            }
            ui.fill(Rect::new(hx - 1.0, y - 6.0, 2.0, 15.0), rgb(palette::TEXT, 0.95));
        }
    }
}

/// `--loading SECONDS --screenshot FILE`: the loading screen for a match on
/// the backdrop map, that long after it came up, with the build going at the
/// pace it goes on the RTX 3080 Ti.
pub fn screenshot(
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    shot: &crate::headless::Shot,
    at: f32,
) -> Result<(), String> {
    use mc_render::{Camera, FrameInput, Overlay};
    let path = crate::setup::backdrop_map().ok_or("no maps found")?;
    let map = Arc::new(MapFile::open(&path).map_err(|e| format!("{}: {e}", path.display()))?);
    let scene = SceneDesc {
        map: map.clone(),
        blueprints,
        pool,
        team_colors: crate::setup::TEAM_COLORS,
    };
    let mut renderer = Renderer::new(
        Target::Headless {
            width: shot.width,
            height: shot.height,
        },
        scene,
    )
    .map_err(|e| e.to_string())?;
    let viewport = Vec2::new(shot.width as f32, shot.height as f32);
    let camera = Camera::new(Vec2::splat(1000.0), viewport);
    let audio = crate::audio::Audio::silent();
    let (mut overlay, mut memory) = (Overlay::default(), ui::Memory::default());
    let input = ui::Input::default();
    let mut curtain = Curtain::new(
        "Deploying",
        "Skirmish   \u{b7}   2 commanders",
        true,
    );
    curtain.set_map(&map, Some(0));
    // The build's steps and when each began, in seconds after the build started.
    const STEPS: [(f32, &str, f32, f32); 9] = [
        (0.0, "Waking the graphics card", 0.0, 0.0),
        (0.07, "Compiling shaders", 0.08, 0.08),
        (0.12, "Building unit models", 0.2, 0.5),
        (0.55, "Shaping terrain props", 0.5, 0.5),
        (0.68, "Scattering the map's props", 0.6, 0.6),
        (0.75, "Weaving terrain textures", 0.7, 0.7),
        (1.35, "Laying ground cover", 0.8, 0.8),
        (1.85, "Forming the sky", 0.88, 0.88),
        (1.9, "Drawing unit pictures", 1.0, 1.0),
    ];
    let dt = 1.0 / 60.0;
    let frames = (at / dt).round() as usize;
    let mut built_at: Option<f32> = None;
    let (mut built, mut handed) = (false, false);
    for i in 0..=frames {
        let time = 10.0 + i as f32 * dt;
        let t = i as f32 * dt;
        if built && !handed && curtain.still() {
            handed = true;
            curtain.handed_over();
        }
        if curtain.ready_to_build() && !built {
            let since = *built_at.get_or_insert(t);
            let b = t - since;
            if let Some(k) = STEPS.iter().rposition(|s| b >= s.0) {
                let (start, name, from, to) = STEPS[k];
                let end = STEPS.get(k + 1).map_or(start + 1.0, |n| n.0);
                let done = from + (to - from) * ((b - start) / (end - start)).clamp(0.0, 1.0);
                curtain.building(name, done);
            }
            if b > 2.1 {
                built = true;
                curtain.built();
            }
        }
        if i == 2 {
            overlay.set_image(
                crate::hud::MINIMAP_SLOT,
                ui::preview::SIZE,
                ui::preview::SIZE,
                &ui::preview::render(&map),
            );
            curtain.chart_ready();
        }
        overlay.clear();
        memory.begin_frame();
        let mut ui = Ui::new(&mut overlay, &input, &mut memory, &audio, viewport, 1.0, time, dt);
        curtain.draw(&mut ui);
        curtain.settle(dt, handed);
        memory.end_frame(&input);
    }
    renderer
        .render(&FrameInput {
            camera: &camera,
            time: 10.0 + at,
            alpha: 1.0,
            sim: None,
            ghosts: &[],
            marks: &[],
            ranges: &[],
            ranges_drawn: 0,
            overlay: &overlay,
            build_grid: false,
        })
        .map_err(|e| e.to_string())?;
    let pixels = renderer
        .read_pixels()
        .ok_or("no pixels from a headless target")?;
    crate::headless::write_png(std::path::Path::new(&shot.path), shot.width, shot.height, &pixels)
}
