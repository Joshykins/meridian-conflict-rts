//! The Storm Seeder's clouds as drawn (the Nimbus, mc-sim `cloud_seed.rs`): each seed in
//! `RenderFrame::clouds` is a thunderhead that grows out of a knot of vapour at the tail
//! as it drifts to a stop behind the aircraft (a run lays a trail of them), and each
//! `SimEvent::CloudBolt` a bolt out of its base.
//!
//! - **Release** (`ShotFired`): nothing of its own, not even a muzzle flash: the cloud's
//!   first knot is the seed being let go.
//! - **Cloud** (once, when a seed is first seen): its lumps are laid at once
//!   (storm_puffs.wgsl), lowest first, and carried and grown on the GPU along the seed's
//!   own eased drift. They live in slots of their own at the end of the puff buffer
//!   (`STORM_PUFF_SLOTS`), written again every tick, so a battle's smoke and sparks going
//!   round the ring can never overwrite a cloud halfway through its life. A small dense
//!   knot at first, it builds slowly into a dark thunderhead some 200 m across, grey-blue on its crown and near black under its base. Lit from inside
//!   by lightning that flickers more often as it grows and all the time while it strikes,
//!   and twice or so a stroke the sky lights (`Sky::strike`) with its far thunder.
//! - **Bolt** (`CloudBolt`): a forked blue-white stroke from inside the base down to what
//!   it strikes, struck again once or twice down the same channel, forks off its upper
//!   part, the base lit round it and the touchdown flashing white. The burst where it lands
//!   is the weapon's `ion_blast` (the ordinary `Impact`).
//! - **Joining the layer**: once the sim lets the seed go the cloud rises over
//!   `STORM_LINGER` seconds to the base of the sky's cloud layer, spreading, lightening
//!   from storm grey to an ordinary cloud's and thinning away as it goes, its lightning
//!   dying; meanwhile its water is handed to the weather there (`Sky::seed`), so the
//!   layer over a run's track thickens and drifts off with the rest.
//!
//! The drawn cloud follows the data: how wide it grows from the seed's bolt `radius`, and
//! where it goes and when from the seed's own drift. Presentation only; the renderer's
//! own clock.

use glam::Vec3;
use std::f32::consts::TAU;

use super::{Puff, Renderer, GROUND_FIRE_SLOTS, PUFF_RING};
use crate::gpu_consts::puff;
use bytemuck::Zeroable;
use mc_sim::mirror::CloudView;
use mc_sim::SimEvent;

const PUFF_STORM: f32 = puff::STORM_CLOUD as f32;
/// Seconds a cloud rains itself out after the sim lets it go (storm_puffs.wgsl).
const LINGER: f32 = puff::STORM_LINGER;
/// Lumps a cloud is built of: a deliberate cosmetic cap on what one cloud costs (a run
/// keeps eight or so in the air at once, a seed a second, each lingering as it rains out).
const LUMPS: usize = 40;
/// Metres a grown cloud reaches from its middle at least, and as a share of the disc its
/// bolts fall in: some 200 m across, its bolts leaving from inside it. Each cloud is a little bigger or smaller than that (`lay_storm_cloud`).
const LEAST_REACH: f32 = 95.0;
const REACH_PER_RADIUS: f32 = 0.65;
/// Puff slots kept for storm clouds at the end of the puff buffer, after the ring, the
/// ground fires' and the missile trails' (renderer.rs `MAX_PUFFS`): room for about 150
/// clouds, six or so Nimbus runs at once (a cloud lives some 23 s, a seed a second). A deliberate cosmetic cap: past it the
/// oldest clouds are let go first.
pub(super) const STORM_PUFF_SLOTS: usize = 6144;
/// Strokes the sky lights inside one cloud (each with its thunder) before its bolts: a
/// deliberate cap, so a wing of them does not take all the sky's flashes or drown the
/// battle in thunder.
const MOST_RUMBLES: u8 = 2;
/// `push_effect` and `Lights::effect` kinds: blue-white, and white-hot.
const BLUE_FLASH: f32 = 0.0;
const WHITE_FLASH: f32 = 9.0;

/// The clouds being drawn, by the seed's serial.
#[derive(Default)]
pub(super) struct StormClouds {
    clouds: Vec<StormCloud>,
    /// The sim tick last seen: one that goes back is a new match or a reset range, whose
    /// shots' serials start again, so what was known is forgotten.
    last_tick: u32,
}

/// One seed's cloud: its lumps, and what they were laid with, so the lightning finds it
/// where the GPU draws it.
#[derive(Clone)]
struct StormCloud {
    /// Its lumps as laid, written to the storm slots every tick until they are spent.
    lumps: Vec<Puff>,
    serial: u32,
    /// When it was laid, its middle as drawn then, the drift it had left then, the seconds
    /// it had left to grow (and drift) then, and how far grown it was then (0 a seed, 1
    /// grown).
    laid: f32,
    origin: Vec3,
    drift: Vec3,
    grow_seconds: f32,
    grown_then: f32,
    /// Metres it reaches from its middle, grown.
    reach: f32,
    /// When the sim let it go: it rains out until `LINGER` after.
    gone: Option<f32>,
    /// Strokes the sky has lit in it; whether one of its bolts has been seen from afar.
    rumbles: u8,
    struck: bool,
}

impl StormCloud {
    /// How far grown it is at `now`, and where its middle is (storm_puffs.wgsl
    /// `storm_growth`).
    fn at(&self, now: f32) -> (f32, Vec3) {
        let u = if self.grow_seconds > 0.0 {
            ((now - self.laid) / self.grow_seconds).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let f = self.grown_then + (1.0 - self.grown_then) * u;
        let eased = 1.0 - (1.0 - u) * (1.0 - u);
        (f, self.origin + self.drift * eased)
    }
}

/// How big a cloud `f` of the way grown is, as a share of grown (storm_puffs.wgsl): a small
/// knot at first, building slowly into its size.
fn size_at(f: f32) -> f32 {
    0.06 + 0.94 * f * f * (3.0 - 2.0 * f)
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

    /// Once a tick: lays the clouds of seeds seen for the first time, and lights them all
    /// from inside.
    pub(super) fn storm_clouds_tick(&mut self, views: &[CloudView], sim_tick: u32, time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let mut clouds = std::mem::take(&mut self.storm_clouds.clouds);
        if sim_tick < std::mem::replace(&mut self.storm_clouds.last_tick, sim_tick) {
            clouds.clear();
        }
        for c in &mut clouds {
            if c.gone.is_none() && !views.iter().any(|v| v.serial == c.serial) {
                c.gone = Some(time);
                // Spent, it rises into the cloud layer and its water joins the sky's.
                let (_, middle) = c.at(time);
                self.sky.seed(middle.truncate(), c.reach * 1.6, time);
            }
        }
        clouds.retain(|c| c.gone.is_none_or(|gone| time < gone + LINGER));
        // Oldest first: past the slots kept for them, the oldest clouds go first.
        let mut held = clouds.iter().map(|c| c.lumps.len()).sum::<usize>();
        while held > STORM_PUFF_SLOTS && !clouds.is_empty() {
            held -= clouds.remove(0).lumps.len();
        }
        for v in views {
            if !clouds.iter().any(|c| c.serial == v.serial) {
                let cloud = self.lay_storm_cloud(v, time, tick);
                clouds.push(cloud);
            }
        }
        self.with_outbound(|r| {
            for c in &mut clouds {
                r.storm_flicker(c, time, tick);
            }
        });
        let mut slots = vec![Puff::zeroed(); STORM_PUFF_SLOTS];
        for (slot, lump) in slots
            .iter_mut()
            .zip(clouds.iter().flat_map(|c| c.lumps.iter()))
        {
            *slot = *lump;
        }
        self.puffs.write(
            ((PUFF_RING + GROUND_FIRE_SLOTS + super::nuke_fx::NUKE_PUFF_SLOTS) * size_of::<Puff>())
                as u64,
            bytemuck::cast_slice(&slots),
        );
        self.storm_clouds.clouds = clouds;
    }

    /// Lays all of seed `v`'s cloud at `time`, carried along the rest of its drift.
    fn lay_storm_cloud(&mut self, v: &CloudView, time: f32, tick: f32) -> StormCloud {
        let pos = Vec3::from(v.pos);
        let step = Vec3::from(v.vel);
        // Shots are drawn a tick behind the sim, gliding toward where it has them.
        let drawn = pos - step;
        // The drift eases out as a square (mc-sim `cloud_seed::path`): each step is shorter
        // than the last by the same amount, so from `pos`, `left` steps from the end, the
        // rest is `step * left² / (2 left + 1)`; and the tail of such an ease is one too.
        // It grows over the same time, drifting or not (one let go from a hovering
        // aircraft does not drift at all).
        let left = v.grow.saturating_sub(v.age) as f32;
        let (drift, grow_seconds) = if v.age <= v.grow {
            (
                step * (1.0 + left * left / (2.0 * left + 1.0)),
                (left + 1.0) * tick,
            )
        } else {
            (Vec3::ZERO, 0.0)
        };
        let grown_then = ((v.age as f32 - 1.0) / v.grow.max(1) as f32).clamp(0.0, 1.0);
        // No two alike: each is its own size, height of crown, lean, stretch and shade of
        // grey, so a trail of them reads as weather and not one cloud stamped down a line.
        let reach =
            (v.radius * REACH_PER_RADIUS).max(LEAST_REACH) * (0.85 + 0.3 * self.scatter.unit());
        let tower = 0.7 + 0.6 * self.scatter.unit();
        let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.25;
        let turn = self.scatter.unit() * TAU;
        let (stretch, sin, cos) = (0.75 + 0.5 * self.scatter.unit(), turn.sin(), turn.cos());
        let tone = 0.85 + 0.3 * self.scatter.unit();
        let life = (v.life.saturating_sub(v.age) as f32 + 1.0) * tick + LINGER;
        // A heap: a flat base a third of its reach under its middle, a crown that rises
        // highest in the middle; the inner lumps show first (the seed's knot), the outer
        // as it grows.
        let mut lumps: Vec<(Vec3, f32, f32)> = (0..LUMPS)
            .map(|_| {
                let a = self.scatter.unit() * TAU;
                let r = self.scatter.unit().sqrt();
                let crown = 1.0 - 0.6 * r * r;
                let z = -0.3 + self.scatter.unit().powf(0.8) * (0.3 + 0.72 * crown) * tower;
                let (x, y) = (a.cos() * r * 0.7 * stretch, a.sin() * r * 0.7 / stretch);
                let flat = Vec3::new(x * cos - y * sin, x * sin + y * cos, 0.0);
                // The crown leans off the base.
                let offset = (flat + lean * (z + 0.3) + Vec3::Z * z) * reach;
                let size = reach * (0.6 + 0.25 * self.scatter.unit()) * (1.0 - 0.2 * r);
                let shows = (r * 0.85 + self.scatter.unit() * 0.25 - 0.2).clamp(0.0, 0.85);
                (offset, size, shows)
            })
            .collect();
        lumps.sort_by(|a, b| a.0.z.total_cmp(&b.0.z));
        let lumps = lumps
            .into_iter()
            .map(|(offset, size, shows)| Puff {
                appearance: [grow_seconds, grown_then, reach, shows],
                origin: drawn.to_array(),
                opacity: 1.0,
                pos: (drawn + offset).to_array(),
                start: time,
                vel: drift.to_array(),
                life: life + self.scatter.unit() * 0.8,
                params: [size, tone, PUFF_STORM, self.scatter.unit()],
            })
            .collect();
        StormCloud {
            lumps,
            serial: v.serial,
            laid: time,
            origin: drawn,
            drift,
            grow_seconds,
            grown_then,
            reach,
            gone: None,
            rumbles: 0,
            struck: false,
        }
    }

    /// This tick's lightning inside cloud `c`: now and then while it grows, more often
    /// the bigger it is, every tick while it strikes, dying away as it rains out.
    fn storm_flicker(&mut self, c: &mut StormCloud, now: f32, tick: f32) {
        let (f, middle) = c.at(now);
        let r = c.reach * size_at(f);
        let (chance, strokes) = match c.gone {
            Some(gone) => {
                let k = 1.0 - ((now - gone) / LINGER).clamp(0.0, 1.0);
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
            .filter(|c| c.at(time).1.truncate().distance(from.truncate()) < c.reach * 1.5)
            .min_by(|a, b| {
                let d = |c: &StormCloud| c.at(time).1.distance(from);
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
