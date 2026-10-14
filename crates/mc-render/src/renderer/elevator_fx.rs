//! Space elevators (`UnitBlueprint::elevator`, the ARC Skyhook): what the cable and its
//! platform do besides move. The model's jaws, cable and platform are posed by the shader
//! (entity.wgsl, `gpu_consts::elevator`) from `mc_sim::elevator::pose`; this lays what goes
//! with them, from the same pose (`UnitInstance::mount`, `deploy`), the sim's elevator
//! events and the geometry in `gpu_consts::elevator`, all read by name.
//!
//! - **Tether.** The cable's tip falls out of the sky glowing with the heat of its fall,
//!   a fading comet's tail of that glow strung up behind it and embers shed off it. Where
//!   it goes down through the cloud deck it throws a collar of vapour out round itself and
//!   shoves the cloud aside. Low over the lot its downdraft lifts dust. When it seats in
//!   the well: a white flash and a flare, a pressure ring and a wave of dust across the
//!   whole lot, the static the cable carried down out of orbit earthing in arcs to the
//!   collar, a fountain of sparks, and the coupling glowing as it cools for a while after,
//!   breathing off a little steam. The jaws swinging in vent hydraulic steam at each tower,
//!   and when they bite (`ElevatorTethered`), sparks and a white flash where each one meets
//!   the cable, a flare, and a purge of steam off every tower.
//! - **Each fall of crates.** While they move, and in the seconds before they sink, amber
//!   beacons turn on the three tower crowns, sweeping real light round the lot. The towers'
//!   floods light the pad while the crates are low or landed (and at night their beams are
//!   seen through the air). High up they are still fast, and heated: a re-entry glow under
//!   each and a glowing wake, vapour trails off their corners, white strobes on their roofs.
//!   Through the cloud deck they tear wisps off the cloud and punch a hole. Over their last
//!   third they brake: thruster plumes off the bells under each crate's corners, the pad
//!   under them heated and dust driven out from under them across the lot. Landing
//!   (`ElevatorDocked`): a pressure ring and dust across the lot, lock clamps flashing at
//!   each crate's door posts, steam purged from the doors and the towers, and the doors
//!   breathing steam while the crates stand. Sinking (`ElevatorSinking`): the empty crates
//!   go down into the ground in steam and a ring of dust.
//! - **Always.** Red obstruction lamps blink up the whole cable every `LAMP_STEP` metres,
//!   and once the cable is clamped a slow pulse of light climbs it every `PULSE_EVERY`.
//!
//! Flares and glows keep a floor size in pixels, so the lamps and pulses read from far
//! off up a cable that runs far past the clouds. Close work (dust, steam, sparks, wisps) is laid only near the eye.
//! A site, a wreck, a ghost or a radar blip has none of it.
//!
//! Budget: steady emissions share `TICK_BUDGET` puffs a tick across every elevator (a
//! deliberate cosmetic cap: past it the rest go without for that tick, so a field of
//! elevators cannot flush the shared puff ring). The one-shots (seating, clamps, docking,
//! sinking) are fixed-size bursts, each a few hundred puffs at most, once in 30 s.
//! Presentation only; the renderer's own clock.

use std::collections::BTreeMap;
use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use mc_map::BUILD_CELL_M;
use mc_sim::mirror::{SimEvent, UnitInstance, KIND_GHOST, KIND_WRECK, STATE_RADAR};

use super::capital_fx::{BEACON_TURN, PUFF_LAMP, PUFF_LAMP_CONE};
use super::lens_flare::Flare;
use super::water_fx::PUFF_STEAM;
use super::{Renderer, PUFF_CLOUD_WISP, PUFF_SHOCK_DUST, PUFF_SPARK};
use crate::camera::Camera;
use crate::gpu_consts::{elevator as geo, puff};

const GLOW: f32 = puff::WARP_GLOW as f32;
const STREAK: f32 = puff::WARP_STREAK as f32;
const ARC: f32 = puff::WARP_ARC as f32;

/// Effect kinds (sprites.wgsl `vs_effect`): an orange flash, a white-hot one that burns
/// through everything, a white rail-gun strike.
const ORANGE: f32 = 1.0;
const WHITE_HOT: f32 = 4.0;
const STRIKE: f32 = 9.0;

/// The amber of the warning beacons, the red of obstruction lamps, the floods' white.
const AMBER: Vec3 = Vec3::new(1.0, 0.45, 0.04);
const RED: Vec3 = Vec3::new(1.0, 0.04, 0.02);
const FLOOD: Vec3 = Vec3::new(1.0, 0.94, 0.82);
/// Heat: metal and air glowing from friction, orange into white.
const HEAT: Vec3 = Vec3::new(1.0, 0.42, 0.12);
const HOT: Vec3 = Vec3::new(1.0, 0.8, 0.55);
/// The cable's static earthing, and the pulse that climbs it: ARC's pale blue.
const STATIC: Vec3 = Vec3::new(0.62, 0.8, 1.0);
const PULSE: Vec3 = Vec3::new(0.45, 0.8, 1.0);
/// The thrusters' blue-white light on the deck.
const JET: Vec3 = Vec3::new(0.5, 0.72, 1.0);

/// The clamp towers' bearings off the heading (entity.wgsl turns each jaw about the
/// nearest multiple of a third of a turn), and the ways out between them.
const TOWERS: [f32; 3] = [0.0, TAU / 3.0, TAU * 2.0 / 3.0];
const EXITS: [f32; 3] = [TAU / 6.0, TAU / 2.0, TAU * 5.0 / 6.0];

/// Obstruction lamps: metres apart up the cable, starting this far up, and their blink
/// (seconds from one flash to the next, and the share of that they are lit).
const LAMP_STEP: f32 = 150.0;
const LAMP_FIRST: f32 = 300.0;
const BLINK_PERIOD: f32 = 1.5;
const BLINK_ON: f32 = 0.4;
/// The pulse up the clamped cable: one leaves the jaws every `PULSE_EVERY` seconds and
/// climbs at `PULSE_SPEED` metres a second.
const PULSE_EVERY: f32 = 5.0;
const PULSE_SPEED: f32 = 420.0;
/// Seconds the coupling glows after the cable seats.
const COUPLING_COOL: f32 = 10.0;
/// Ticks before the crates sink that the beacons start turning.
const WARN_AHEAD: u32 = 25;
/// Thrusters under the crates: one at each corner of each of the three.
const NOZZLES: usize = 12;
/// Distance from the cable's axis to a bay's middle, along its bearing.
const BAY_MID: f32 = geo::BAY_FROM + geo::BAY_LENGTH * 0.5;
/// Height (m) over the pad at which the thrusters' wash begins to reach it.
const WASH_REACH: f32 = 170.0;
/// Crate speed (m/s) at which they start to glow with heat, and the span over which
/// that glow comes to full.
const HEAT_FROM: f32 = 650.0;
const HEAT_SPAN: f32 = 1100.0;
/// Vapour trails off the crates: laid above this height, at least this fast,
/// this many metres apart, hanging this long.
const TRAIL_FLOOR: f32 = 1200.0;
const TRAIL_SPEED: f32 = 300.0;
const TRAIL_STEP: f32 = 90.0;
const TRAIL_LIFE: f32 = 14.0;
/// Puffs every elevator's steady effects together may lay in a tick (see the module doc).
const TICK_BUDGET: usize = 900;

/// What one elevator has been doing, as of the last tick it was seen.
#[derive(Clone, Copy)]
struct Track {
    /// The cable's drop and the crates' height last seen.
    drop: f32,
    car: f32,
    /// When the cable seated (render clock), and ticks the crates have stood landed.
    seated_at: Option<f32>,
    docked: Option<u32>,
    /// Height (world z) the crates' vapour trails are laid to.
    trail: Option<f32>,
    seen: f32,
}

/// One elevator this tick: what its lights need each frame.
#[derive(Clone, Copy)]
struct Lit {
    site: Site,
    pose: Pose,
    beacons: bool,
    /// How hard the floods, the falling tip's heat, the coupling's glow and the
    /// thrusters burn (0..1).
    floods: f32,
    tip_heat: f32,
    coupling: f32,
    thrust: f32,
}

#[derive(Default)]
pub(super) struct ElevatorFx {
    tracks: BTreeMap<u32, Track>,
    /// Every elevator near enough to draw this tick, by unit, for the events.
    sites: Vec<(u32, Site)>,
    lit: Vec<Lit>,
    /// Puffs left in this tick's budget.
    left: usize,
    /// The render clock at the last tick, to notice it going back.
    last: f32,
}

/// An elevator's place: its foot, its heading's forward and left, and half its lot
/// across (from its blueprint's footprint).
#[derive(Clone, Copy)]
struct Site {
    base: Vec3,
    fwd: Vec3,
    left: Vec3,
    lot: f32,
    seed: f32,
}

impl Site {
    fn new(u: &UnitInstance, lot: f32) -> Self {
        let (s, c) = u.heading.sin_cos();
        Self {
            base: Vec3::from(u.pos),
            fwd: Vec3::new(c, s, 0.0),
            left: Vec3::new(-s, c, 0.0),
            lot,
            seed: (u.unit_id % 97) as f32 * 0.613,
        }
    }

    /// The level direction `a` radians off the heading.
    fn dir(&self, a: f32) -> Vec3 {
        self.fwd * a.cos() + self.left * a.sin()
    }

    /// The point `r` metres out on bearing `a`, `z` metres up (model space).
    fn at(&self, a: f32, r: f32, z: f32) -> Vec3 {
        self.base + self.dir(a) * r + Vec3::Z * z
    }

    /// The cable's axis, `z` metres up.
    fn axis(&self, z: f32) -> Vec3 {
        self.base + Vec3::Z * z
    }

    /// A point in bay `k`'s frame: `along` its bearing from the cable's axis, `across` it
    /// (left positive), `z` up.
    fn bay(&self, k: usize, along: f32, across: f32, z: f32) -> Vec3 {
        let a = EXITS[k];
        self.base
            + self.dir(a) * along
            + self.dir(a + std::f32::consts::FRAC_PI_2) * across
            + Vec3::Z * z
    }
}

/// The elevator's pose last tick and this (`mc_sim::elevator::pose`): the cable's drop,
/// the jaws' grip, the crates' height up the cable (1 at the top, 0 landed, below 0 open
/// and sinking, over 1 away).
#[derive(Clone, Copy)]
struct Pose {
    drop: [f32; 2],
    grip: [f32; 2],
    car: [f32; 2],
}

impl Pose {
    fn of(u: &UnitInstance) -> Self {
        Self {
            drop: [u.mount[0], u.mount[1]],
            grip: [u.mount[2], u.mount[3]],
            car: [u.prev_deploy, u.deploy],
        }
    }
}

/// Height of the cable's lowest point over the foot at `drop` (as entity.wgsl lifts it).
fn tip_z(drop: f32) -> f32 {
    geo::CABLE_FOOT + (1.0 - drop) * (geo::SKY_Z - geo::CABLE_FOOT)
}

/// How far the crates are raised over where they land at height `car` (none once landed).
fn car_lift(car: f32) -> f32 {
    car.max(0.0) * (geo::SKY_Z - geo::CAR_DOCK)
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where the eye is, and how big a metre is from it.
#[derive(Clone, Copy)]
struct View {
    eye: Vec3,
    focus: Vec2,
    distance: f32,
    scale: f32,
    dark: f32,
}

impl View {
    /// `metres`, or as many metres as `px` pixels cover at `p` if that is more: a floor
    /// that keeps a flare or a glow readable from far off.
    fn floor(&self, p: Vec3, metres: f32, px: f32) -> f32 {
        metres.max(self.eye.distance(p) / self.scale.max(1.0) * px)
    }

    /// The level direction from `p` toward the eye.
    fn toward(&self, p: Vec3) -> Vec3 {
        (self.eye - p).truncate().normalize_or(Vec2::X).extend(0.0)
    }
}

impl ElevatorFx {
    /// Takes `n` puffs from this tick's budget, if they are there.
    fn spend(&mut self, n: usize) -> bool {
        let ok = self.left >= n;
        if ok {
            self.left -= n;
        }
        ok
    }
}

impl Renderer {
    /// Once a sim tick: every elevator's steady effects, and the events' one-shots.
    pub(super) fn elevator_tick(
        &mut self,
        units: &[UnitInstance],
        events: &[SimEvent],
        time: f32,
        camera: &Camera,
    ) {
        let fx = &mut self.elevator_fx;
        if time + 1.0 < fx.last {
            // The clock went back (a restaged backdrop): nothing of the old world is left.
            fx.tracks.clear();
        }
        fx.last = time;
        fx.left = TICK_BUDGET;
        fx.lit.clear();
        fx.sites.clear();
        let view = View {
            eye: camera.eye(),
            focus: camera.focus.truncate(),
            distance: camera.distance,
            scale: camera.projection_scale(),
            dark: self.sky.darkness(),
        };
        let hidden = KIND_WRECK
            | KIND_GHOST
            | STATE_RADAR
            | (mc_sim::tables::flag::UNDER_CONSTRUCTION as u32) << 8;
        let saved = (self.effect_origin, self.effect_settings);
        let rows = self.unit_kinds.rows_where(|b| {
            self.blueprints
                .units
                .get(b as usize)
                .is_some_and(|bp| bp.elevator.is_some())
        });
        for u in rows.iter().map(|&i| &units[i]) {
            if u.owner_flags & hidden != 0 {
                continue;
            }
            let found = self
                .blueprints
                .units
                .get(u.blueprint as usize)
                .filter(|bp| bp.elevator.is_some())
                .map(|bp| (bp.visual.effects, bp.footprint));
            let Some((effects, footprint)) = found else {
                continue;
            };
            let lot = footprint.0.min(footprint.1) as f32 * BUILD_CELL_M as f32 * 0.5;
            self.effect_origin = Some(Vec3::from(u.pos));
            self.effect_settings = effects;
            self.elevator_unit(u, lot, time, &view);
        }
        self.elevator_fx
            .tracks
            .retain(|_, t| time - t.seen < 5.0 && t.seen <= time);
        for event in events {
            let (unit, cue): (u32, fn(&mut Renderer, &Site, f32)) = match event {
                SimEvent::ElevatorTethered { unit, .. } => (unit.0, Renderer::elevator_clamped),
                SimEvent::ElevatorDocked { unit, .. } => (unit.0, Renderer::elevator_docked),
                SimEvent::ElevatorSinking { unit, .. } => (unit.0, Renderer::elevator_sinking),
                _ => continue,
            };
            let found = self.elevator_fx.sites.iter().find(|(id, _)| *id == unit);
            if let Some(&(_, site)) = found {
                self.effect_origin = Some(site.base);
                cue(self, &site, time);
            }
        }
        (self.effect_origin, self.effect_settings) = saved;
    }

    /// One elevator's tick.
    fn elevator_unit(&mut self, u: &UnitInstance, lot: f32, time: f32, view: &View) {
        let site = Site::new(u, lot);
        let pose = Pose::of(u);
        let d = site.base.truncate().distance(view.focus);
        if d > view.distance * 3.0 + 9000.0 {
            return;
        }
        let near = d < view.distance * 1.5 + 1200.0 && view.distance < 6000.0;
        self.elevator_fx.sites.push((u.unit_id, site));
        let fx = &mut self.elevator_fx;
        let track = fx.tracks.entry(u.unit_id).or_insert(Track {
            drop: pose.drop[0],
            car: pose.car[0],
            seated_at: None,
            docked: None,
            trail: None,
            seen: time,
        });
        let seated = pose.drop[1] >= 1.0 && track.drop < 1.0;
        if seated {
            track.seated_at = Some(time);
        }
        track.docked = match track.docked {
            _ if pose.car[1] > 0.0 => None,
            Some(n) => Some(n + 1),
            None if track.car > 0.0 => Some(0),
            None => None,
        };
        track.drop = pose.drop[1];
        track.car = pose.car[1];
        track.seen = time;
        let cooling = track
            .seated_at
            .map_or(0.0, |t| 1.0 - ((time - t) / COUPLING_COOL).clamp(0.0, 1.0));
        let docked_for = track.docked;

        let mut lit = Lit {
            site,
            pose,
            beacons: false,
            floods: 0.0,
            tip_heat: 0.0,
            coupling: cooling,
            thrust: 0.0,
        };
        if pose.drop[1] > 0.0 && pose.drop[1] < 1.0 {
            lit.tip_heat = self.tether_fall(&site, &pose, time, near, view);
        }
        if seated {
            self.tether_seat(&site, time, near);
        }
        if pose.grip[1] > pose.grip[0] && near {
            self.clamps_closing(&site, &pose, time);
        }
        if pose.drop[1] > 0.0 {
            self.cable_lamps(&site, &pose, time, view);
        }
        if pose.grip[1] >= 1.0 {
            self.cable_pulse(&site, time, view);
        }
        if cooling > 0.0 {
            self.coupling_glow(&site, cooling, time, near, view);
        }
        if pose.drop[1] >= 1.0 {
            self.platform(&site, &pose, u, docked_for, time, near, view, &mut lit);
        }
        self.elevator_fx.lit.push(lit);
    }

    /// The crates' tick: beacons, floods, and whatever their fall throws off.
    fn platform(
        &mut self,
        site: &Site,
        pose: &Pose,
        u: &UnitInstance,
        docked_for: Option<u32>,
        time: f32,
        near: bool,
        view: &View,
        lit: &mut Lit,
    ) {
        let [c0, c1] = pose.car;
        let moving = (c1 - c0).abs() > 1e-7 && c0.min(c1) < 1.0 && c0.abs().max(c1.abs()) < 2.0;
        // Landed, the beacons start turning a moment before they sink.
        let leaving_soon = match (docked_for, self.dock_ticks(u)) {
            (Some(n), Some(dock)) => n + WARN_AHEAD >= dock,
            _ => false,
        };
        lit.beacons = moving || leaving_soon;
        // The floods light the pad while the crates are low or landed, and at night the
        // pad stays lit at a working level.
        let low = 1.0 - smoothstep(0.03, 0.09, c1.min(1.0));
        lit.floods = low.max(view.dark * 0.45);
        if lit.beacons {
            self.tower_beacons(site, time, view);
        }
        if lit.floods > 0.01 && near {
            self.deck_floods(site, lit.floods, time, view);
        }
        if c0.max(c1) > 0.0 {
            lit.thrust = self.platform_flight(u.unit_id, site, pose, time, near, view);
        } else if c1 <= 0.0
            && c1 > -1.0
            && near
            && self.scatter.unit() < 0.35
            && self.elevator_fx.spend(1)
        {
            // Landed: the doors breathe a little steam while the crates stand.
            let k = (self.scatter.unit() * 3.0) as usize % 3;
            let at = site.bay(
                k,
                geo::BAY_FROM + geo::BAY_LENGTH,
                self.scatter.signed() * geo::BAY_WIDTH * 0.4,
                geo::DECK_Z + 1.0,
            );
            let vel = site.dir(EXITS[k]) * 4.0 + Vec3::Z * 3.0;
            self.push_puff(PUFF_STEAM, at, vel, time, 3.5, (3.0, 14.0));
        }
    }

    /// Ticks an elevator's crates stand landed before they sink.
    fn dock_ticks(&self, u: &UnitInstance) -> Option<u32> {
        let bp = self.blueprints.units.get(u.blueprint as usize)?;
        Some(bp.elevator?.dock_ticks)
    }

    /// The cable's tip coming down: its glow and tail, embers, the cloud deck it goes
    /// through, its downdraft over the lot. Returns how hot the tip is (0..1).
    fn tether_fall(&mut self, site: &Site, pose: &Pose, time: f32, near: bool, view: &View) -> f32 {
        let dt = self.tick_seconds.max(0.02);
        let (z0, z1) = (tip_z(pose.drop[0]), tip_z(pose.drop[1]));
        let speed = (z0 - z1).max(0.0) / dt;
        let heat = (speed / 520.0).clamp(0.0, 1.0);
        let tip = site.axis(z1);
        let motion = Vec3::Z * (z1 - z0) / dt;
        if !self.elevator_fx.spend(6) {
            return heat;
        }
        // The tip's own glow, and a fading tail of it strung up the cable behind.
        let size = view.floor(tip, geo::CABLE_R * (2.5 + 3.5 * heat), 6.0);
        self.push_warp(
            GLOW,
            tip,
            motion,
            time,
            dt * 2.0,
            (size, size * 1.15),
            HOT.lerp(HEAT, 1.0 - heat) * (1.2 + 4.0 * heat),
            0.0,
        );
        for k in 0..3 {
            let f = (k as f32 + 1.0) / 3.0;
            let at = site.axis(z0 + (z1 - z0) * f + geo::CABLE_R);
            let s = view.floor(at, geo::CABLE_R * (2.0 + 2.5 * heat), 4.0);
            self.push_warp(
                GLOW,
                at,
                Vec3::ZERO,
                time + f * dt,
                0.5 + 0.6 * heat,
                (s, s * 0.6),
                HEAT * (0.5 + 2.5 * heat),
                0.0,
            );
        }
        if near && heat > 0.25 && self.elevator_fx.spend(2) {
            for _ in 0..2 {
                let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0);
                let vel = motion * 0.6 + out * 14.0 + Vec3::Z * 25.0 * self.scatter.unit();
                let life = 0.8 + 0.8 * self.scatter.unit();
                self.push_puff(PUFF_SPARK, tip, vel, time, life, (1.6, 0.4));
            }
        }
        // Down through the cloud deck: a collar of vapour thrown out, the cloud shoved.
        let base = self.sky.cloud_base_at(tip.truncate());
        for level in [base + crate::sky::CLOUD_DECK, base] {
            let rel = site.base.z;
            if z0 + rel > level && z1 + rel <= level {
                let at = Vec3::new(tip.x, tip.y, level);
                self.sky.blast(at, 420.0, 1.6, time);
                if near || view.distance < 9000.0 {
                    self.vapour_collar(at, time);
                }
            }
        }
        // Low over the lot its downdraft lifts the dust.
        let low = 1.0 - smoothstep(60.0, 420.0, z1);
        if near && low > 0.05 && self.elevator_fx.spend(3) {
            for _ in 0..3 {
                let a = self.scatter.unit() * TAU;
                let r = geo::DECK_R * (0.8 + 0.5 * self.scatter.unit());
                let mut at = site.at(a, r, 0.0);
                at.z = self.ground_height(at.truncate()).max(site.base.z) + 2.0;
                let vel = site.dir(a) * (20.0 + 40.0 * low) + Vec3::Z * 2.0;
                let life = 3.0 + self.scatter.unit();
                self.push_puff(
                    PUFF_SHOCK_DUST,
                    at,
                    vel,
                    time,
                    life,
                    (14.0, 40.0 + 30.0 * low),
                );
            }
        }
        heat
    }

    /// A collar of the cloud's own vapour thrown out round something going down through
    /// the deck at `at`: cloud wisps, which the shader keeps only where there is cloud, so
    /// a clear sky shows none.
    fn vapour_collar(&mut self, at: Vec3, time: f32) {
        for k in 0..40 {
            let a = (k as f32 + self.scatter.unit() * 0.6) / 40.0 * TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let rise = Vec3::Z * self.scatter.signed() * 30.0;
            let at = at + out * 25.0 + rise;
            let vel =
                out * (160.0 + 90.0 * self.scatter.unit()) + Vec3::Z * self.scatter.signed() * 10.0;
            let life = 5.0 + 2.0 * self.scatter.unit();
            self.push_puff(PUFF_CLOUD_WISP, at, vel, time, life, (40.0, 170.0));
        }
    }

    /// The cable seating in the well.
    fn tether_seat(&mut self, site: &Site, time: f32, near: bool) {
        let coupling = site.axis(geo::CABLE_FOOT + 6.0);
        self.push_effect(coupling.to_array(), time, 70.0, 0.4, WHITE_HOT, 0.0);
        self.push_effect(coupling.to_array(), time, 160.0, 0.7, ORANGE, 1.0);
        self.lens_flares.flash(
            coupling,
            Flare {
                color: Vec3::new(9.0, 7.5, 6.0),
                size: 170.0,
            },
            time,
            0.55,
        );
        let ground = site.axis(geo::DECK_Z + 2.0);
        self.push_shockwave(
            ground.to_array(),
            time,
            470.0,
            1.8,
            0.85,
            ORANGE,
            Vec3::ZERO,
        );
        if !near {
            return;
        }
        // A wave of dust rolling out across the whole lot and past it.
        self.lot_dust(site, 56, 95.0, 1.3, time + 0.05);
        // The static it brought down out of orbit earthing to the collar.
        for k in 0..12 {
            let a = self.scatter.unit() * TAU;
            let from = site.axis(geo::CABLE_FOOT + 10.0 + 110.0 * self.scatter.unit());
            let to = site.at(
                a,
                geo::CABLE_R * 2.0 + 60.0 * self.scatter.unit(),
                geo::DECK_Z,
            );
            let life = 0.12 + 0.18 * self.scatter.unit();
            let start = time + k as f32 * 0.05;
            self.push_warp(
                ARC,
                from,
                to - from,
                start,
                life,
                (10.0, 10.0),
                STATIC * 6.0,
                0.0,
            );
        }
        // Sparks thrown up and out of the well.
        for _ in 0..70 {
            let vel = self.scatter.upward(0.35) * (30.0 + 60.0 * self.scatter.unit());
            let life = 0.8 + 0.8 * self.scatter.unit();
            self.push_puff(PUFF_SPARK, coupling, vel, time, life, (1.8, 0.4));
        }
    }

    /// The seated coupling cooling: a fading glow, a light (`elevator_lights`), steam.
    fn coupling_glow(&mut self, site: &Site, k: f32, time: f32, near: bool, view: &View) {
        if !self.elevator_fx.spend(2) {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        let at = site.axis(geo::CABLE_FOOT + 8.0) + view.toward(site.base) * geo::CABLE_R;
        let size = geo::CABLE_R * 3.5;
        self.push_warp(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            dt * 3.0,
            (size, size),
            HEAT * 1.4 * k,
            0.0,
        );
        if near && self.scatter.unit() < 0.6 * k {
            let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 4.0;
            self.push_puff(PUFF_STEAM, at, out + Vec3::Z * 6.0, time, 3.0, (4.0, 18.0));
        }
    }

    /// The jaws swinging in: hydraulic steam off each tower, a burst as they start.
    fn clamps_closing(&mut self, site: &Site, pose: &Pose, time: f32) {
        let start = pose.grip[0] <= 0.0;
        let n = if start { 6 } else { 1 };
        if !start && !self.elevator_fx.spend(n * 9) {
            return;
        }
        let (push, size, life) = if start {
            (40.0, (8.0, 48.0), 3.5)
        } else {
            (20.0, (4.0, 24.0), 2.2)
        };
        for &a in &TOWERS {
            for (vent, out) in ram_vents(site, a) {
                for _ in 0..n {
                    let jitter = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.3;
                    let vel =
                        (out + jitter + Vec3::Z * 0.35) * push * (0.6 + 0.6 * self.scatter.unit());
                    let life = life + self.scatter.unit();
                    self.push_puff(PUFF_STEAM, vent, vel, time, life, size);
                }
            }
        }
    }

    /// The jaws biting on the cable (`ElevatorTethered`).
    fn elevator_clamped(&mut self, site: &Site, time: f32) {
        let middle = site.axis(geo::GRIP_Z);
        self.push_effect(middle.to_array(), time, 55.0, 0.35, WHITE_HOT, 0.0);
        self.lens_flares.flash(
            middle,
            Flare {
                color: Vec3::new(8.0, 7.6, 7.0),
                size: 130.0,
            },
            time,
            0.4,
        );
        for &a in &TOWERS {
            let bite = site.at(a, geo::CABLE_R, geo::GRIP_Z);
            self.push_effect(bite.to_array(), time, 22.0, 0.25, STRIKE, 0.6);
            for _ in 0..30 {
                let spray = (site.dir(a) * 0.4 + self.scatter.upward(0.0)).normalize_or(Vec3::Z);
                let vel = spray * (25.0 + 55.0 * self.scatter.unit());
                let life = 0.6 + 0.9 * self.scatter.unit();
                self.push_puff(PUFF_SPARK, bite, vel, time, life, (1.5, 0.35));
            }
            // Every tower purges its rams.
            for (vent, out) in ram_vents(site, a) {
                for _ in 0..5 {
                    let vel = (out + Vec3::Z * (0.2 + 0.6 * self.scatter.unit()))
                        * (30.0 + 30.0 * self.scatter.unit());
                    let life = 3.0 + 1.5 * self.scatter.unit();
                    self.push_puff(PUFF_STEAM, vent, vel, time + 0.05, life, (10.0, 52.0));
                }
            }
        }
    }

    /// Red obstruction lamps up the cable, blinking together.
    fn cable_lamps(&mut self, site: &Site, pose: &Pose, time: f32, view: &View) {
        let phase = ((time + site.seed) / BLINK_PERIOD).fract();
        if phase > BLINK_ON {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        let shift = tip_z(pose.drop[1]) - geo::CABLE_FOOT;
        let toward = view.toward(site.base) * geo::CABLE_R * 1.15;
        let color = RED * (5.0 + 7.0 * view.dark);
        let mut h = LAMP_FIRST;
        while h + shift < geo::SKY_Z - 30.0 {
            if !self.elevator_fx.spend(1) {
                return;
            }
            let at = site.axis(h + shift) + toward;
            let size = view.floor(at, 4.0, 4.5);
            self.push_drive(
                PUFF_LAMP,
                at,
                color,
                time,
                dt * 1.6,
                (size, size),
                Vec3::ZERO,
                0.0,
            );
            h += LAMP_STEP;
        }
        // Above the sky the cable runs on far out of sight: lamps thin out up it, each
        // a little further than the last, so its length reads from any height.
        let mut h = geo::SKY_Z;
        while h < geo::TOP_Z * 0.8 {
            h *= 1.3;
            if !self.elevator_fx.spend(1) {
                return;
            }
            let at = site.axis(h) + toward;
            let size = view.floor(at, 4.0, 4.5);
            self.push_drive(
                PUFF_LAMP,
                at,
                color,
                time,
                dt * 1.6,
                (size, size),
                Vec3::ZERO,
                0.0,
            );
        }
    }

    /// The slow pulses of light climbing the clamped cable.
    fn cable_pulse(&mut self, site: &Site, time: f32, view: &View) {
        let dt = self.tick_seconds.max(0.02);
        let run = geo::SKY_Z - geo::GRIP_Z;
        let climb = run / PULSE_SPEED;
        let toward = view.toward(site.base) * geo::CABLE_R * 1.05;
        let lead = (time + site.seed * 7.0).rem_euclid(PULSE_EVERY);
        let mut age = lead;
        while age < climb {
            if !self.elevator_fx.spend(2) {
                return;
            }
            let at = site.axis(geo::GRIP_Z + age * PULSE_SPEED) + toward;
            let fade = smoothstep(0.0, 0.6, age) * (1.0 - smoothstep(climb * 0.6, climb, age));
            let size = view.floor(at, geo::CABLE_R * 2.6, 5.0);
            self.push_warp(
                GLOW,
                at,
                Vec3::Z * PULSE_SPEED,
                time,
                dt * 2.5,
                (size, size * 1.3),
                PULSE * 3.0 * fade,
                0.0,
            );
            // Its tail, drawn on behind it.
            let tail = PULSE_SPEED * dt * 2.5;
            let width = view.floor(at, geo::CABLE_R * 0.8, 1.5);
            self.push_warp(
                STREAK,
                at - Vec3::Z * tail,
                Vec3::Z * tail,
                time,
                dt * 3.0,
                (width, width),
                PULSE * 1.6 * fade,
                0.0,
            );
            age += PULSE_EVERY;
        }
    }

    /// The amber beacons on the tower crowns, turning (puffs.wgsl `lamp_flare`).
    fn tower_beacons(&mut self, site: &Site, time: f32, view: &View) {
        if !self.elevator_fx.spend(3) {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        for (i, &a) in TOWERS.iter().enumerate() {
            let at = site.at(a, geo::BEACON_R, geo::BEACON_Z);
            let size = view.floor(at, 7.0, 6.0);
            let turn = 2.0 + i as f32 / 3.0;
            self.push_drive(
                PUFF_LAMP,
                at,
                AMBER * 10.0,
                time,
                dt * 1.6,
                (size, size),
                Vec3::ZERO,
                turn,
            );
        }
    }

    /// The towers' floods on the deck: their lenses, and at night their beams in the air.
    fn deck_floods(&mut self, site: &Site, k: f32, time: f32, view: &View) {
        if !self.elevator_fx.spend(6) {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        for &a in &TOWERS {
            let (lamp, aim) = flood(site, a);
            let size = view.floor(lamp, 5.0, 4.0);
            self.push_drive(
                PUFF_LAMP,
                lamp,
                FLOOD * 8.0 * k,
                time,
                dt * 1.6,
                (size, size),
                Vec3::ZERO,
                0.0,
            );
            if view.dark > 0.1 {
                let width = aim.length() * 0.42;
                let strength = view.dark * k * 0.8;
                self.push_drive(
                    PUFF_LAMP_CONE,
                    lamp,
                    aim,
                    time,
                    dt * 1.6,
                    (width, width),
                    Vec3::ZERO,
                    strength,
                );
            }
        }
    }

    /// Every frame, `alpha` of the way through the tick: the beacons sweeping the lot, the
    /// floods on the deck, the falling tip's heat, the cooling coupling, the thrusters.
    pub(super) fn elevator_lights(&mut self, time: f32, alpha: f32) {
        let lit = std::mem::take(&mut self.elevator_fx.lit);
        for l in &lit {
            let s = &l.site;
            if l.beacons {
                for (i, &a) in TOWERS.iter().enumerate() {
                    let at = s.at(a, geo::BEACON_R, geo::BEACON_Z);
                    // As puffs.wgsl `lamp_flare` turns the flare, each a third of a turn on.
                    let turn = time * BEACON_TURN + i as f32 / 3.0 * TAU;
                    let aim = Vec3::new(turn.cos(), turn.sin(), -0.3);
                    self.lights.lamp(at, aim, AMBER * 16000.0, 220.0, 18.0, 0.1);
                }
            }
            if l.floods > 0.01 {
                for &a in &TOWERS {
                    let (lamp, aim) = flood(s, a);
                    let reach = aim.length();
                    let color = FLOOD * 10.0 * (reach + 30.0).powi(2) * l.floods;
                    let range = (reach * 1.35 + 40.0).min(560.0);
                    self.lights.lamp(lamp, aim, color, range, 30.0, 0.04);
                }
            }
            if l.tip_heat > 0.0 {
                let drop = l.pose.drop[0] + (l.pose.drop[1] - l.pose.drop[0]) * alpha;
                let at = s.axis(tip_z(drop));
                let color = HEAT * 5000.0 * l.tip_heat;
                self.lights.lamp(at, Vec3::NEG_Z, color, 160.0, 180.0, 1.0);
            }
            if l.coupling > 0.0 {
                let at = s.axis(geo::CABLE_FOOT + 8.0);
                self.lights.lamp(
                    at,
                    Vec3::NEG_Z,
                    HEAT * 1800.0 * l.coupling,
                    70.0,
                    180.0,
                    1.0,
                );
            }
            if l.thrust > 0.0 {
                let car = l.pose.car[0] + (l.pose.car[1] - l.pose.car[0]) * alpha;
                let mouth = car_lift(car.min(1.0)) + geo::CAR_UNDER_Z;
                let reach = (mouth - 60.0).max(geo::DECK_Z);
                for k in 0..EXITS.len() {
                    let from = s.bay(k, BAY_MID, 0.0, mouth - 4.0);
                    let to = s.bay(k, BAY_MID, 0.0, reach);
                    self.lights.beam(from, to, JET * 4000.0 * l.thrust, 150.0);
                }
            }
        }
        self.elevator_fx.lit = lit;
    }
}

/// A tower's ram vents by its jaw's hinge, and which way each blows: out of both flanks,
/// and in over the well.
fn ram_vents(site: &Site, a: f32) -> [(Vec3, Vec3); 3] {
    let side = site.dir(a + std::f32::consts::FRAC_PI_2);
    let at = |r: f32, s: f32| site.at(a, r, geo::HINGE_Z) + side * s;
    [
        (at(geo::HINGE_R, 22.0), side),
        (at(geo::HINGE_R, -22.0), -side),
        (at(geo::HINGE_R - 10.0, 0.0), -site.dir(a)),
    ]
}

/// A tower's flood: the lamp under its crown, and the way to the deck it lights.
fn flood(site: &Site, a: f32) -> (Vec3, Vec3) {
    let lamp = site.at(a, geo::BEACON_R - 10.0, geo::BEACON_Z - 16.0);
    let target = site.at(a, geo::DECK_R * 0.2, geo::DECK_Z);
    (lamp, target - lamp)
}

mod flight;
