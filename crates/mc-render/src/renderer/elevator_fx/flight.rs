//! The elevator's platform in flight, docking and leaving (see `elevator_fx.rs`).

use std::f32::consts::TAU;

use glam::Vec3;

use super::{
    car_lift, smoothstep, Pose, Site, View, EXITS, HEAT, HEAT_FROM, HEAT_SPAN, HOT, NOZZLES,
    ORANGE, STRIKE, TOWERS, TRAIL_FLOOR, TRAIL_LIFE, TRAIL_SPEED, TRAIL_STEP, WASH_REACH,
};
use crate::gpu_consts::elevator as geo;
use crate::renderer::capital_fx::{PUFF_LAMP, PUFF_THRUST, PUFF_THRUST_GLOW};
use crate::renderer::nuke_fx::PUFF_STRATEGIC_TRAIL;
use crate::renderer::water_fx::PUFF_STEAM;
use crate::renderer::{Puff, Renderer, PUFF_CLOUD_WISP, PUFF_SHOCK_DUST, PUFF_SPARK};

const GLOW: f32 = super::GLOW;
const STREAK: f32 = super::STREAK;
/// Most vapour-trail steps one platform lays in a tick: a deliberate cap, as the trails
/// share the strategic missiles' reserved puff slots (`NukeFx::lay_trail`); a platform
/// faster than this many steps a tick leaves its trail in longer gaps, never cut short.
const TRAIL_STEPS: usize = 4;

impl Renderer {
    /// The platform moving up or down the cable this tick (unit `id`): its thrusters,
    /// sleeves, heat, strobes, the cloud it goes through and its vapour trails. Returns
    /// how hard its thrusters burn (0..1).
    pub(super) fn platform_flight(
        &mut self,
        id: u32,
        site: &Site,
        pose: &Pose,
        time: f32,
        near: bool,
        view: &View,
    ) -> f32 {
        let dt = self.tick_seconds.max(0.02);
        let [c0, c1] = pose.car;
        let (l0, l1) = (car_lift(c0.min(1.0)), car_lift(c1.min(1.0)));
        let v = (l1 - l0) / dt;
        // Braking over the last third of the way down; driven off the deck and clear.
        let thrust = if c1 <= 0.0 {
            0.0
        } else if v < 0.0 {
            smoothstep(0.55, 0.68, 1.0 - c1.cbrt())
        } else {
            1.0 - smoothstep(0.22, 0.4, c1.cbrt())
        };
        if thrust > 0.01 {
            self.platform_thrusters(site, l0, v, thrust, time, near, view);
        }
        if near && v.abs() > 20.0 && c1 < 1.0 {
            self.guide_sleeves(site, l0, v, thrust, time);
        }
        if c1 < 1.0 {
            self.platform_heat(site, l0, l1, v, time, view);
            self.platform_strobes(site, l0, v, time, view);
        }
        self.platform_clouds(site, l0, l1, v, time, near || view.distance < 9000.0);
        self.platform_trails(id, site, l1, v, time);
        thrust
    }

    /// The thrusters round the underside ring, firing down; low over the deck, the deck
    /// heated under them and dust driven out from under the rim across the lot.
    fn platform_thrusters(
        &mut self,
        site: &Site,
        l0: f32,
        v: f32,
        heat: f32,
        time: f32,
        near: bool,
        view: &View,
    ) {
        if !self.elevator_fx.spend(NOZZLES * 3 + 16) {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        let motion = Vec3::Z * v;
        let mouth_z = l0 + geo::CAR_UNDER_Z;
        let gap = (mouth_z - geo::DECK_Z).max(0.0);
        let length = (40.0 + 90.0 * heat).min(gap + 6.0);
        let bearing = |k: usize| (k as f32 + 0.5) / NOZZLES as f32 * TAU;
        for k in 0..NOZZLES {
            let mouth = site.at(bearing(k), geo::THRUSTER_R, mouth_z);
            if length > 2.0 {
                let width = view.floor(mouth, 7.5, 1.5);
                self.push_drive(
                    PUFF_THRUST,
                    mouth,
                    -Vec3::Z * length,
                    time,
                    dt * 1.5,
                    (width, width),
                    motion,
                    heat,
                );
            }
            if near {
                self.push_drive(
                    PUFF_THRUST_GLOW,
                    mouth - Vec3::Z * 1.5,
                    Vec3::ZERO,
                    time,
                    dt * 1.6,
                    (8.0 + 4.0 * heat, 10.0 + 5.0 * heat),
                    motion,
                    heat,
                );
            }
        }
        let wash = (1.0 - gap / WASH_REACH).clamp(0.0, 1.0);
        let wash = wash * wash * heat;
        if !near || wash < 0.03 {
            return;
        }
        for k in 0..NOZZLES {
            let hit = site.at(bearing(k), geo::THRUSTER_R, geo::DECK_Z + 1.5);
            let size = 14.0 + 26.0 * wash;
            self.push_drive(
                PUFF_THRUST_GLOW,
                hit,
                Vec3::ZERO,
                time,
                dt * 1.7,
                (size, size * 1.08),
                Vec3::ZERO,
                -wash,
            );
        }
        let n = (4.0 + 12.0 * wash) as usize;
        for _ in 0..n {
            let a = self.scatter.unit() * TAU;
            let mut at = site.at(a, geo::DECK_R * (1.0 + 0.08 * self.scatter.unit()), 0.0);
            at.z = self.ground_height(at.truncate()).max(site.base.z) + 2.5;
            let push = 35.0 + 75.0 * wash + 15.0 * self.scatter.unit();
            let vel = site.dir(a) * push + Vec3::Z * (1.0 + 2.0 * wash);
            let start = time + self.scatter.unit() * dt;
            let life = 3.0 + 1.5 * wash + self.scatter.unit();
            let size = (12.0 + 8.0 * wash, 40.0 + 45.0 * wash);
            self.push_puff(PUFF_SHOCK_DUST, at, vel, start, life, size);
        }
        if self.scatter.unit() < 0.2 {
            self.tree_blasts
                .record(site.base, time, 160.0 + 120.0 * wash, 0.3 * wash, false);
        }
    }

    /// The sleeves the platform rides the cable in, above and below its deck: hot from
    /// the brakes, shedding sparks.
    fn guide_sleeves(&mut self, site: &Site, l0: f32, v: f32, brake: f32, time: f32) {
        let sparks = 1 + (3.0 * brake) as usize;
        if !self.elevator_fx.spend(2 + sparks) {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        let motion = Vec3::Z * v;
        let top = site.axis(l0 + geo::DECK_Z + 12.0);
        let bottom = site.axis(l0 + geo::CAR_UNDER_Z - 4.0);
        let glow = HEAT * (0.6 + 3.5 * brake);
        let size = (geo::CABLE_R * 2.4, geo::CABLE_R * 3.0);
        for at in [top, bottom] {
            self.push_warp(GLOW, at, motion, time, dt * 2.0, size, glow, 0.0);
        }
        for _ in 0..sparks {
            let a = self.scatter.unit() * TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let vel = out * (15.0 + 35.0 * self.scatter.unit()) + motion * 0.5;
            let life = 0.5 + 0.6 * self.scatter.unit();
            self.push_puff(
                PUFF_SPARK,
                top + out * geo::CABLE_R,
                vel,
                time,
                life,
                (1.4, 0.35),
            );
        }
    }

    /// High up and fast, the platform's leading face glows with the heat of the air and
    /// leaves a glowing wake down (or up) the cable behind it.
    fn platform_heat(&mut self, site: &Site, l0: f32, l1: f32, v: f32, time: f32, view: &View) {
        let h = ((v.abs() - HEAT_FROM) / HEAT_SPAN).clamp(0.0, 1.0);
        if h <= 0.0 || !self.elevator_fx.spend(3) {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        let face_z = if v < 0.0 {
            geo::CAR_UNDER_Z - 12.0
        } else {
            geo::DECK_Z + 30.0
        };
        let face = site.axis(l1 + face_z);
        let s = view.floor(face, geo::DECK_R * (0.9 + 0.6 * h), 8.0);
        let color = HEAT.lerp(HOT, h) * (1.0 + 5.0 * h);
        self.push_warp(
            GLOW,
            face,
            Vec3::Z * v,
            time,
            dt * 2.0,
            (s, s * 1.2),
            color,
            0.0,
        );
        let core = s * 0.45;
        self.push_warp(
            GLOW,
            face,
            Vec3::Z * v,
            time,
            dt * 2.0,
            (core, core),
            HOT * 4.0 * h,
            0.0,
        );
        let behind = site.axis(l1 - (l1 - l0) * 3.0 + face_z);
        let width = view.floor(face, geo::DECK_R * 0.35, 2.0);
        self.push_warp(
            STREAK,
            behind,
            face - behind,
            time,
            0.6,
            (width, width * 1.4),
            HEAT * 2.5 * h,
            0.0,
        );
    }

    /// White strobes round the platform's rim while it flies, seen from far off.
    fn platform_strobes(&mut self, site: &Site, l0: f32, v: f32, time: f32, view: &View) {
        if !self.elevator_fx.spend(3) {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        for &a in &EXITS {
            let at = site.at(a, geo::DECK_R, l0 + geo::DECK_Z + 4.0);
            let size = view.floor(at, 6.0, 6.0);
            self.push_drive(
                PUFF_LAMP,
                at,
                Vec3::splat(12.0),
                time,
                dt * 1.6,
                (size, size),
                Vec3::Z * v,
                1.0,
            );
        }
    }

    /// Through the cloud deck: wisps torn off the cloud round the platform (the shader
    /// keeps only those born in cloud), and a hole punched where it crosses.
    fn platform_clouds(&mut self, site: &Site, l0: f32, l1: f32, v: f32, time: f32, wisps: bool) {
        let rel = site.base.z;
        let base = self.sky.cloud_base_at(site.base.truncate());
        let top = base + crate::sky::CLOUD_DECK;
        let (lo, hi) = (l0.min(l1) + rel, l0.max(l1) + rel);
        if hi < base - 80.0 || lo > top + 80.0 {
            return;
        }
        for level in [base, top] {
            if (l0 + rel - level) * (l1 + rel - level) < 0.0 {
                let at = site.axis(level - rel);
                self.sky.blast(at, 320.0, 1.3, time);
            }
        }
        if !wisps || !self.elevator_fx.spend(8) {
            return;
        }
        for _ in 0..8 {
            let z = (lo + (hi - lo) * self.scatter.unit()).clamp(base - 40.0, top + 40.0);
            let a = self.scatter.unit() * TAU;
            let r = geo::DECK_R * (0.6 + 0.6 * self.scatter.unit());
            let at = site.at(a, r, z - rel);
            let vel = site.dir(a) * (30.0 + 30.0 * self.scatter.unit()) + Vec3::Z * v * 0.08;
            let life = 5.5 + 1.5 * self.scatter.unit();
            self.push_puff(PUFF_CLOUD_WISP, at, vel, time, life, (30.0, 130.0));
        }
    }

    /// Vapour trails off the platform's rim while it is high and fast: long white streaks
    /// up the sky, laid in steps as it goes (unit `id`'s `Track::trail`).
    fn platform_trails(&mut self, id: u32, site: &Site, l1: f32, v: f32, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let z = l1 + geo::CAR_UNDER_Z;
        let Some(track) = self.elevator_fx.tracks.get_mut(&id) else {
            return;
        };
        if v.abs() < TRAIL_SPEED || l1 < TRAIL_FLOOR || l1 >= geo::SKY_Z {
            track.trail = None;
            return;
        }
        let from = *track.trail.get_or_insert(z);
        let span = z - from;
        let count = (span.abs() / TRAIL_STEP).floor() as usize;
        if count == 0 {
            return;
        }
        let step = span / count.min(TRAIL_STEPS) as f32;
        let steps = count.min(TRAIL_STEPS);
        track.trail = Some(z);
        for k in 1..=steps {
            let pz = from + step * k as f32;
            let born = time - dt * (1.0 - k as f32 / steps as f32);
            for &a in &EXITS {
                let p = site.at(a, geo::DECK_R * 0.95, pz);
                let seed = self.scatter.unit();
                self.nuke_fx.lay_trail(Puff {
                    origin: p.to_array(),
                    opacity: 1.0,
                    pos: p.to_array(),
                    start: born,
                    // A tent a step either side (puffs.wgsl `strategic_trail`): no seams.
                    vel: (Vec3::Z * step).to_array(),
                    life: TRAIL_LIFE,
                    params: [5.0, 34.0, PUFF_STRATEGIC_TRAIL, seed],
                    appearance: [-1.0, -1.0, -1.0, 0.45],
                });
            }
        }
    }

    /// The platform touching down (`ElevatorDocked`): a pressure ring and dust across the
    /// lot, the lock clamps flashing shut round the rim, steam purged.
    pub(super) fn elevator_docked(&mut self, site: &Site, time: f32) {
        let deck = site.axis(geo::DECK_Z + 2.0);
        self.push_shockwave(deck.to_array(), time, 360.0, 1.4, 0.7, ORANGE, Vec3::ZERO);
        self.lot_dust(site, 40, 70.0, 1.1, time);
        for k in 0..6 {
            let a = (k as f32 + 0.5) / 6.0 * TAU;
            let at = site.at(a, geo::DECK_R * 0.9, geo::DECK_Z + 3.0);
            let start = time + 0.15 + 0.04 * k as f32;
            self.push_effect(at.to_array(), start, 10.0, 0.2, STRIKE, 0.0);
            for _ in 0..8 {
                let vel = self.scatter.upward(0.3) * (15.0 + 30.0 * self.scatter.unit());
                let life = 0.4 + 0.5 * self.scatter.unit();
                self.push_puff(PUFF_SPARK, at, vel, start, life, (1.2, 0.3));
            }
        }
        self.vent_purge(site, time + 0.3, 12, 1.0);
    }

    /// The platform leaving (`ElevatorDeparting`): the clamps let go in steam, and the
    /// thrusters drive it off the deck in a ring of dust.
    pub(super) fn elevator_leaving(&mut self, site: &Site, time: f32) {
        let deck = site.axis(geo::DECK_Z + 2.0);
        self.push_shockwave(deck.to_array(), time, 260.0, 1.2, 0.5, ORANGE, Vec3::ZERO);
        self.lot_dust(site, 30, 50.0, 0.9, time);
        for k in 0..6 {
            let a = (k as f32 + 0.5) / 6.0 * TAU;
            let at = site.at(a, geo::DECK_R * 0.9, geo::DECK_Z + 3.0);
            for _ in 0..5 {
                let vel = (site.dir(a) * 0.5 + Vec3::Z) * (12.0 + 18.0 * self.scatter.unit());
                let life = 2.0 + self.scatter.unit();
                self.push_puff(PUFF_STEAM, at, vel, time, life, (4.0, 22.0));
            }
        }
        self.vent_purge(site, time, 8, 0.8);
    }

    /// Dust rolling out across the lot, `push` m/s, `big` times the usual size: `n` puffs
    /// from the deck's edge and as many again from out by the lot's own edge, so the wave
    /// covers the whole lot and spills past it, not only the middle the towers hide.
    pub(super) fn lot_dust(&mut self, site: &Site, n: usize, push: f32, big: f32, time: f32) {
        for band in 0..2 {
            let from = if band == 0 {
                geo::DECK_R
            } else {
                site.lot * 0.75
            };
            for k in 0..n {
                let a = (k as f32 + self.scatter.unit() * 0.7) / n as f32 * TAU;
                let mut at = site.at(a, from * (1.0 + 0.08 * self.scatter.unit()), 0.0);
                at.z = self.ground_height(at.truncate()).max(site.base.z) + 3.0;
                let speed = push * (1.0 + 0.6 * self.scatter.unit()) * (1.0 - 0.3 * band as f32);
                let vel = site.dir(a) * speed + Vec3::Z * 3.0;
                let life = 4.5 + 2.0 * self.scatter.unit();
                let start = time + 0.12 * band as f32;
                let size = (26.0 * big, 110.0 * big);
                self.push_puff(PUFF_SHOCK_DUST, at, vel, start, life, size);
            }
        }
    }

    /// Steam purged from the deck's vents at the ways out and off the towers' rams.
    fn vent_purge(&mut self, site: &Site, time: f32, n: usize, big: f32) {
        for &a in &EXITS {
            let vent = site.at(a, geo::DECK_R * 0.97, geo::DECK_Z + 1.0);
            for _ in 0..n {
                let side = site.dir(a + std::f32::consts::FRAC_PI_2) * self.scatter.signed() * 0.4;
                let push = (25.0 + 25.0 * self.scatter.unit()) * big;
                let vel = (site.dir(a) + side + Vec3::Z * 0.25) * push;
                let life = 3.5 + 1.5 * self.scatter.unit();
                self.push_puff(PUFF_STEAM, vent, vel, time, life, (8.0 * big, 42.0 * big));
            }
        }
        for &a in &TOWERS {
            for (vent, out) in super::ram_vents(site, a) {
                for _ in 0..n / 4 {
                    let vel = (out + Vec3::Z * 0.4) * (20.0 + 20.0 * self.scatter.unit());
                    let life = 3.0 + self.scatter.unit();
                    self.push_puff(PUFF_STEAM, vent, vel, time, life, (6.0, 32.0));
                }
            }
        }
    }
}
