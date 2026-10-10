//! The Storm Seeder's clouds as drawn (the Nimbus, mc-sim `cloud_seed.rs`): each seed in
//! `RenderFrame::clouds` is a thunderhead that builds behind the aircraft as the seed drifts
//! to a stop (a run lays a trail of them), and each `SimEvent::CloudBolt` a bolt out of its
//! base.
//!
//! - **Cloud**: not drawn here at all. Each is a cell of the sky's own weather (`Sky::seed`,
//!   clouds_sim.wgsl kind `disturber::SEED`): while it grows and strikes, the cover round it
//!   gathers in a circle that swells with it, so the sky's ray-marched clouds draw it in
//!   their own material and light, a heap round the height the bomber flies (the weather
//!   marks it seeded), never thinned round the selection. Already a fair cloud as it
//!   leaves the tail, it builds to some 270 m across.
//! - **Lightning inside**: strokes that flicker in it more often as it grows and all the
//!   time while it strikes, and twice or so a stroke the sky lights (`Sky::strike`) with its
//!   far thunder.
//! - **Bolt** (`CloudBolt`): a forked blue-white stroke from inside the base down to what
//!   it strikes, struck again once or twice down the same channel, forks off its upper
//!   part, the base lit round it and the touchdown flashing white. The burst where it lands
//!   is the weapon's `ion_blast` (the ordinary `Impact`).
//! - **Joining the layer**: once the sim lets the seed go it lets go of the weather: the
//!   cloud it leaves is the sky's, drifting off on the wind and healing back to the day's
//!   weather like any other.
//! - **Release** (`ShotFired`): nothing of its own, not even a muzzle flash.
//!
//! How wide it grows follows the seed's bolt `radius`, where it goes and when the seed's
//! own drift. Presentation only; the renderer's own clock.

use glam::Vec3;

use super::Renderer;
use crate::sky::SeedCell;
use mc_sim::mirror::CloudView;
use mc_sim::SimEvent;

/// Metres a grown cloud reaches from its middle at least, and as a share of the disc its
/// bolts fall in: some 270 m across, its bolts leaving from inside it. Each cloud is a
/// little bigger or smaller than that.
const LEAST_REACH: f32 = 130.0;
const REACH_PER_RADIUS: f32 = 0.9;
/// Seconds a spent cloud is kept after the sim lets it go, for the lightning in it to die.
const DYING: f32 = 3.0;
/// The cover a grown cloud gathers the weather toward at its middle. No storm, which would
/// stand it up into a tower (a trail of them rose into one towering bank): the weather
/// marks it as seeded instead, and the sky draws it round the height the Nimbus flies, so
/// the trail comes out of the bomber (`Sky::set_seeds`).
const COVER: f32 = 0.85;
/// A cloud joins the one its owner let go just before it, if that one was let go within
/// these seconds and lies within these metres: the weather fills the stretch between, so
/// a run lays one unbroken streak and not a string of dots. Nearest first, so two Nimbus
/// flying side by side each keep their own.
const LINK_SECONDS: f32 = 2.5;
const LINK_REACH: f32 = 320.0;
/// Strokes the sky lights inside one cloud (each with its thunder) before its bolts: a
/// deliberate cap, so a wing of them does not take all the sky's flashes or drown the
/// battle in thunder.
const MOST_RUMBLES: u8 = 2;
/// `push_effect` and `Lights::effect` kinds: blue-white, and white-hot.
const BLUE_FLASH: f32 = 0.0;
const WHITE_FLASH: f32 = 9.0;

/// The clouds in the weather, by the seed's serial.
#[derive(Default)]
pub(super) struct StormClouds {
    clouds: Vec<StormCloud>,
    /// The sim tick last seen: one that goes back is a new match or a reset range, whose
    /// shots' serials start again, so what was known is forgotten.
    last_tick: u32,
}

/// One seed's cloud: where it is and how grown, as last told by the sim.
#[derive(Clone, Copy)]
struct StormCloud {
    serial: u32,
    owner: u8,
    /// When it was first seen, renderer seconds, and the serial of the cloud before it
    /// in its streak (`LINK_SECONDS`).
    born: f32,
    link: Option<u32>,
    /// Its middle and how far grown (0 a seed, 1 grown), as the sim last had it.
    middle: Vec3,
    grown: f32,
    /// Metres it reaches from its middle, grown: its own, a little over or under the rest.
    reach: f32,
    /// When the sim let it go: its lightning dies away over `DYING` after, and the weather
    /// it left is the sky's.
    gone: Option<f32>,
    /// Strokes the sky has lit in it; whether one of its bolts has been seen from afar.
    rumbles: u8,
    struck: bool,
}

/// How big a cloud `f` of the way grown is, as a share of grown: already a fair cloud as
/// it leaves the tail, so the trail forms close behind the aircraft, building into its size.
fn size_at(f: f32) -> f32 {
    0.45 + 0.55 * f * f * (3.0 - 2.0 * f)
}

/// How far grown seed `v` is: 0 let go, 1 grown.
fn grown(v: &CloudView) -> f32 {
    (v.age as f32 / v.grow.max(1) as f32).clamp(0.0, 1.0)
}

impl StormCloud {
    /// What it puts into the weather, reaching back to the cloud before it in its streak
    /// among `clouds`: nothing once the sim has let it go.
    fn cell(&self, clouds: &[StormCloud]) -> Option<SeedCell> {
        let back = self
            .link
            .and_then(|serial| clouds.iter().find(|c| c.serial == serial))
            .map(|c| c.middle.truncate());
        self.gone.is_none().then(|| SeedCell {
            at: self.middle.truncate(),
            back,
            radius: self.reach * size_at(self.grown) * 0.8,
            cover: COVER,
            height: self.middle.z,
        })
    }
}

impl Renderer {
    /// The storm seeder's part of a sim event: true when it was all of it.
    pub(super) fn storm_cloud_event(&mut self, event: &SimEvent, time: f32) -> bool {
        match event {
            // Nothing at the tail: the cloud's first knot is the seed let go.
            SimEvent::ShotFired {
                blueprint, weapon, ..
            } if self.blueprints.unit(*blueprint).weapons[*weapon as usize]
                .cloud
                .is_some() =>
            {
                true
            }
            SimEvent::CloudBolt { from, to, .. } => {
                let (from, to) = (Vec3::from(from.to_f32()), Vec3::from(to.to_f32()));
                self.with_outbound(|r| r.cloud_bolt(from, to, time));
                true
            }
            _ => false,
        }
    }

    /// Runs `draw` with the effects it lays unclipped by shields and with no origin of
    /// their own: weather is not a shot leaving a gun.
    fn with_outbound(&mut self, draw: impl FnOnce(&mut Self)) {
        let previous = self.effect_origin.take();
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        draw(self);
        self.effect_outbound = outbound;
        self.effect_origin = previous;
    }

    /// Once a tick: the clouds into the weather, and lightning inside them.
    pub(super) fn storm_clouds_tick(&mut self, views: &[CloudView], sim_tick: u32, time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let mut clouds = std::mem::take(&mut self.storm_clouds.clouds);
        if sim_tick < std::mem::replace(&mut self.storm_clouds.last_tick, sim_tick) {
            clouds.clear();
        }
        for c in &mut clouds {
            match views.iter().find(|v| v.serial == c.serial) {
                Some(v) => {
                    c.middle = Vec3::from(v.pos);
                    c.grown = grown(v);
                }
                None if c.gone.is_none() => c.gone = Some(time),
                None => {}
            }
        }
        clouds.retain(|c| c.gone.is_none_or(|gone| time < gone + DYING));
        for v in views {
            if !clouds.iter().any(|c| c.serial == v.serial) {
                // No two alike: each its own size, so a trail reads as weather.
                let reach = (v.radius * REACH_PER_RADIUS).max(LEAST_REACH)
                    * (0.85 + 0.3 * self.scatter.unit());
                let middle = Vec3::from(v.pos);
                let link = clouds
                    .iter()
                    .filter(|c| c.owner == v.owner && time - c.born < LINK_SECONDS)
                    .map(|c| (c.middle.distance(middle), c.serial))
                    .filter(|(d, _)| *d < LINK_REACH)
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, serial)| serial);
                clouds.push(StormCloud {
                    serial: v.serial,
                    owner: v.owner,
                    born: time,
                    link,
                    middle: Vec3::from(v.pos),
                    grown: grown(v),
                    reach,
                    gone: None,
                    rumbles: 0,
                    struck: false,
                });
            }
        }
        self.with_outbound(|r| {
            for c in &mut clouds {
                r.storm_flicker(c, time, tick);
            }
        });
        self.sky
            .set_seeds(clouds.iter().filter_map(|c| c.cell(&clouds)).collect());
        self.storm_clouds.clouds = clouds;
    }

    /// This tick's lightning inside cloud `c`: now and then while it grows, more often
    /// the bigger it is, every tick while it strikes, dying away as it rains out.
    fn storm_flicker(&mut self, c: &mut StormCloud, now: f32, tick: f32) {
        let (f, middle) = (c.grown, c.middle);
        let r = c.reach * size_at(f);
        let (chance, strokes) = match c.gone {
            Some(gone) => {
                let k = 1.0 - ((now - gone) / DYING).clamp(0.0, 1.0);
                (0.6 * k * k, 1)
            }
            None if f >= 1.0 => (0.8, 2),
            None => (0.06 + 0.5 * f * f, 1),
        };
        for _ in 0..strokes {
            if self.scatter.unit() >= chance {
                continue;
            }
            let lag = self.scatter.unit() * tick;
            let a = middle
                + Vec3::new(
                    self.scatter.signed() * 0.6,
                    self.scatter.signed() * 0.6,
                    self.scatter.signed() * 0.3,
                ) * r;
            let way = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.4,
            )
            .normalize_or(Vec3::X);
            let b = a + way * r * (0.25 + 0.35 * self.scatter.unit());
            let width = (0.5 + 0.9 * self.scatter.unit()) * (0.4 + 0.6 * f);
            self.arc(a, b, 5, a.distance(b) * 0.15, now + lag, 0.14, width);
            // The water round it lights up: a flash with no sprite of its own.
            let light = 14.0 + 26.0 * f * self.scatter.unit();
            self.lights.effect(
                a.lerp(b, 0.5).to_array(),
                now + lag,
                light,
                0.16,
                BLUE_FLASH,
            );
        }
        if c.gone.is_none() && f > 0.55 && c.rumbles < MOST_RUMBLES && self.scatter.unit() < 0.05 {
            c.rumbles += 1;
            self.sky.strike(middle, now, 0.3, 1.2, false);
        }
    }

    /// A bolt from `from` (the cloud's base) down onto `to`.
    fn cloud_bolt(&mut self, from: Vec3, to: Vec3, time: f32) {
        // It leaves from inside the base, so it is seen to come out of the cloud.
        let top = from + Vec3::Z * 18.0;
        let path = self.bolt_path(top, to, 0.09);
        // The stroke, and the channel struck again a moment later, fainter.
        for (start, life, width) in [(0.0, 0.32, 7.0), (0.09, 0.18, 4.0), (0.21, 0.14, 2.6)] {
            for pair in path.windows(2) {
                self.bore_fx
                    .lightning(pair[0], pair[1], time + start, life, width);
            }
        }
        // Forks off its upper part, reaching out and down and dying in the air.
        let forks = 3 + (self.scatter.unit() * 2.0) as usize;
        for _ in 0..forks {
            let i = 1 + (self.scatter.unit() * (path.len() / 2) as f32) as usize;
            let from = path[i.min(path.len() - 2)];
            let out =
                Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0).normalize_or(Vec3::X);
            let length = 30.0 + self.scatter.unit() * 60.0;
            let end = from + (out - Vec3::Z * (0.6 + self.scatter.unit())).normalize() * length;
            let fork = self.bolt_path(from, end, 0.18);
            for pair in fork.windows(2) {
                self.bore_fx.lightning(pair[0], pair[1], time, 0.22, 2.4);
            }
        }
        // Spider strokes out along the base round where it leaves.
        for _ in 0..2 {
            let out =
                Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0).normalize_or(Vec3::Y);
            let reach = 60.0 + self.scatter.unit() * 70.0;
            self.arc(top, top + out * reach, 6, reach * 0.12, time, 0.25, 2.8);
        }
        // The base lit round it, and the touchdown flashing white.
        self.push_effect(top.to_array(), time, 42.0, 0.28, BLUE_FLASH, 0.0);
        self.lights.effect(
            (top + Vec3::Z * 30.0).to_array(),
            time,
            60.0,
            0.35,
            BLUE_FLASH,
        );
        self.push_effect(to.to_array(), time, 20.0, 0.18, WHITE_FLASH, 0.0);
        // The first bolt out of each cloud lights the sky round it, and is heard far off.
        let nearest = self
            .storm_clouds
            .clouds
            .iter_mut()
            .filter(|c| c.middle.truncate().distance(from.truncate()) < c.reach * 1.5)
            .min_by(|a, b| {
                let d = |c: &StormCloud| c.middle.distance(from);
                d(a).total_cmp(&d(b))
            });
        if let Some(c) = nearest {
            if !std::mem::replace(&mut c.struck, true) {
                self.sky.strike(top, time, 0.35, 2.0, false);
            }
        }
    }

    /// A jagged lightning path from `from` to `to`, a kink every 20 m or so, wandering up
    /// to `wander` of its length off the line (most in the middle).
    fn bolt_path(&mut self, from: Vec3, to: Vec3, wander: f32) -> Vec<Vec3> {
        let length = from.distance(to);
        let kinks = ((length / 20.0) as usize).clamp(4, 24);
        let along = (to - from).normalize_or(Vec3::NEG_Z);
        let side = along.cross(Vec3::X).normalize_or(Vec3::Y);
        let other = along.cross(side).normalize_or(Vec3::Z);
        let mut path = Vec::with_capacity(kinks + 1);
        path.push(from);
        // A walk off the line, each kink from the last, pulled back in at both ends.
        let mut off = Vec3::ZERO;
        for k in 1..kinks {
            let t = k as f32 / kinks as f32;
            let taper = (t * std::f32::consts::PI).sin().sqrt();
            off = off * 0.65
                + (side * self.scatter.signed() + other * self.scatter.signed())
                    * length
                    * wander
                    * 0.6;
            path.push(from + (to - from) * t + off * taper);
        }
        path.push(to);
        path
    }
}
