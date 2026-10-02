//! A cone weapon's wake as it is drawn (`Weapon::cone`, the Regency Wake's projector;
//! docs/STYLE.md "The Regency suite"): the plasma the projector gathered across its mouth
//! (drawn as a Pinched charge, `regency_guns_fx`) is pinched out and rolls out over the
//! ground ahead as one tall front, as wide as the cone the sim strikes and as long as its
//! reach, at the pace the sim's front rolls (`Cone::speed`).
//!
//! - **The mouth:** a hard red flash where the charge hung, the charge collapsing into it.
//! - **The front:** a wall of plasma that arches up off the ground and curls over forward,
//!   like a breaking wave: ribs of hot filament laid across the fan, each rising from the
//!   ground behind the front, cresting and leaning out over it, lit only as the front
//!   passes so the curl reads as one wall rolling out. Across the fan it arches too:
//!   tallest down the middle, low at the flanks. It builds as it leaves the mouth and sags
//!   a little toward the end of its reach. Sparks are flung forward off its crest.
//! - **Behind it:** hot red filaments laid radially along the ground, cooling and breaking
//!   up (sprites.wgsl's plasma trail), clumps of plasma rolled along at its foot.
//! - **The ground:** glassed where it passed: a few big ragged molten pools down the fan,
//!   glowing and crusting over (`BoreFx::melt`); the trees bow as it goes by.
//! - **Its light** runs out with the crest and goes out behind it.
//! - **Where it runs out** the curl breaks: its crest flung forward in sparks.
//!
//! No mist, no rings, nothing wound round a middle: everything is hard-edged and short-lived.
//! Laid a tick at a time as the front gets there (`roll_wakes`, from `regency_trails`), so
//! only the stretch alive now is held. Presentation only; the renderer's own clock.

use super::regency_guns_fx::{BURST, GLOB, GLOW, HOT, MOTE, RED};
use super::Renderer;
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;

/// Metres between the rings the wake is laid in.
const RING: f32 = 4.0;
/// Metres apart across the fan the ribs of the front are laid, the filaments behind it,
/// the clumps rolled along its foot and the pools glassed into the ground.
const RIB_GAP: f32 = 5.0;
const FILAMENT_GAP: f32 = 8.0;
const CLUMP_GAP: f32 = 14.0;
const POOL_GAP: f32 = 45.0;
/// Most ribs a ring of the front is laid with, and filaments behind it: deliberate
/// cosmetic caps, so a wide cone costs no more than this many a ring.
const MAX_RIBS: usize = 16;
const MAX_ACROSS: usize = 14;
/// Most glassed pools a ring of them is laid with: a deliberate cosmetic cap.
const MAX_POOLS: usize = 3;
/// Wakes rolling at once that are drawn: a deliberate cosmetic cap, the oldest goes first.
const MAX_WAKES: usize = 24;
/// How far up from the ground the wake is laid, metres.
const SKIM: f32 = 0.6;
/// The crest's height over the ground, metres, for each unit of the weapon's `impact`.
const CREST: f32 = 9.0;
/// Metres out from the mouth the front takes to rise to its full height.
const BUILD: f32 = 30.0;
/// The curl in profile, through the front: (ahead of it, up), in crest heights. It rises
/// from the ground behind the front, crests and leans out over it.
const CURL: [(f32, f32); 5] = [
    (-0.9, 0.0),
    (-0.45, 0.45),
    (0.0, 0.85),
    (0.35, 1.0),
    (0.65, 0.8),
];

/// A wake rolling out, laid ring by ring as its front gets there.
#[derive(Clone, Copy)]
pub(super) struct RollingWake {
    blueprint: BlueprintId,
    weapon: u8,
    /// The muzzle as drawn, and which way over the ground it rolls.
    at: Vec3,
    ahead: Vec2,
    start: f32,
    /// Rings laid so far.
    laid: usize,
}

impl Renderer {
    /// A cone weapon fired (`ShotFired` of a `cone` weapon): `at` its muzzle as drawn, `dir`
    /// down the bore. The mouth flashes, and the wake starts rolling (`roll_wakes`).
    pub(super) fn wake_fired(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        dir: Vec3,
        time: f32,
    ) {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        if w.cone.is_none() {
            return;
        }
        let flash = w.flash.max(0.3);

        // The mouth: the charge collapses into a hard red flash as the plasma goes out.
        self.charge_spent(blueprint, weapon, at, 0.16, time);
        let s = 2.2 * flash;
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            time,
            0.22,
            (s * 0.6, s * 2.4),
            RED * 4.0,
            0.0,
        );
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            0.12,
            (s * 0.8, s * 1.2),
            HOT * 3.0,
            0.0,
        );
        self.plasma_fx
            .guns
            .flare(at, RED * 260.0 * flash, s * 14.0, time, 0.3);

        let wakes = &mut self.plasma_fx.wakes;
        if wakes.len() >= MAX_WAKES {
            wakes.remove(0);
        }
        wakes.push(RollingWake {
            blueprint,
            weapon,
            at,
            ahead: dir.truncate().normalize_or(Vec2::X),
            start: time,
            laid: 0,
        });
    }

    /// Every rolling wake laid out as far as its front gets by the next tick; one that
    /// has run out to its reach breaks and is let go.
    pub(super) fn roll_wakes(&mut self, time: f32) {
        let until = time + self.tick_seconds.max(0.02);
        let mut i = 0;
        while i < self.plasma_fx.wakes.len() {
            let mut wake = self.plasma_fx.wakes[i];
            let w = &self.blueprints.unit(wake.blueprint).weapons[wake.weapon as usize];
            let Some(cone) = w.cone else {
                self.plasma_fx.wakes.remove(i);
                continue;
            };
            let range = w.range_max.to_f32();
            let speed = cone.speed.to_f32().max(1.0);
            let half = cone.half.0 as f32 / 65536.0 * std::f32::consts::TAU;
            let impact = w.impact.max(0.3);
            let front = ((until - wake.start) * speed).min(range);
            while ring_at(wake.laid) <= front {
                self.lay_ring(&wake, wake.laid, range, speed, half, impact);
                wake.laid += 1;
            }
            if ring_at(wake.laid) > range {
                self.wake_breaks(&wake, range, speed, half, impact);
                self.plasma_fx.wakes.remove(i);
                continue;
            }
            self.plasma_fx.wakes[i] = wake;
            i += 1;
        }
    }

    /// Where on the fan a point `r` out at `a` radians off its middle stands, on the ground
    /// (or the water) under it; close to the mouth it leaves the projector's height.
    fn wake_ground(&self, wake: &RollingWake, r: f32, a: f32) -> Vec3 {
        let xy = wake.at.truncate() + turned(wake.ahead, a) * r;
        let water = self.map_info.water_level.to_f32();
        let floor = self.ground_height(xy).max(water) + SKIM;
        let lift = (1.0 - r / 14.0).clamp(0.0, 1.0);
        xy.extend(floor + (wake.at.z - floor).max(0.0) * lift * lift)
    }

    /// The crest's height `r` out, `a` radians off the fan's middle: it builds as it leaves
    /// the mouth, arches across the fan and sags a little toward the end of its reach.
    fn crest(r: f32, range: f32, a: f32, half: f32, impact: f32) -> f32 {
        let build = (r / BUILD).clamp(0.0, 1.0).sqrt();
        let arch = 0.3 + 0.7 * (a / half.max(1e-3) * std::f32::consts::FRAC_PI_2).cos();
        CREST * impact * build * arch * (1.0 - 0.3 * r / range)
    }

    /// Ring `k` of `wake`: its front, the filaments behind it and the ground it passes.
    fn lay_ring(
        &mut self,
        wake: &RollingWake,
        k: usize,
        range: f32,
        speed: f32,
        half: f32,
        impact: f32,
    ) {
        let r = ring_at(k);
        let out = r / range;
        let when = wake.start + r / speed;
        let width = 2.0 * r * half.tan();
        let water = self.map_info.water_level.to_f32();

        // The front: ribs across the fan, each a curl rising off the ground behind it,
        // cresting and leaning out over it. Ragged: each rib a little ahead of or behind
        // the line, here and there one missing.
        let ribs = ((width / RIB_GAP).ceil() as usize).clamp(2, MAX_RIBS);
        for i in 0..ribs {
            if self.scatter.unit() < 0.12 {
                continue;
            }
            let a =
                -half + 2.0 * half * (i as f32 + 0.5 + 0.4 * self.scatter.signed()) / ribs as f32;
            let h = Self::crest(r, range, a, half, impact) * (0.85 + 0.3 * self.scatter.unit());
            let shift = self.scatter.signed() * RING * 0.4;
            let lean = self.scatter.signed() * 0.06;
            let mut prev = None;
            for (j, &(fore, up)) in CURL.iter().enumerate() {
                let foot =
                    self.wake_ground(wake, (r + shift + fore * h * 0.6).max(1.0), a + lean * up);
                let p = foot + Vec3::Z * up * h;
                if let Some(from) = prev {
                    // Thick and lasting at its foot, thin and quick at the crest.
                    let rise = j as f32 / (CURL.len() - 1) as f32;
                    let life = (0.24 - 0.12 * rise) * (1.0 + 0.3 * self.scatter.unit());
                    let w = (1.0 - 0.55 * rise) * (1.0 - 0.35 * out) * 0.8 * impact;
                    self.plasma_fx.guns.filament(from, p, when, life, w);
                }
                prev = Some(p);
            }
            // Now and then a spark flung forward off the crest.
            if self.scatter.unit() < 0.15 {
                let tip = prev.unwrap_or(wake.at);
                let v = (turned(wake.ahead, a) * speed * (0.5 + 0.4 * self.scatter.unit()))
                    .extend(4.0 + 6.0 * self.scatter.unit());
                let life = 0.35 + 0.3 * self.scatter.unit();
                self.push_lit(
                    MOTE,
                    tip,
                    v,
                    when,
                    life,
                    (0.35, 0.12),
                    RED.lerp(HOT, 0.4) * 4.0,
                    0.0,
                );
            }
        }

        // Filaments laid radially along the ground behind the front, cooling and breaking up.
        let across = ((width / FILAMENT_GAP).ceil() as usize).clamp(2, MAX_ACROSS);
        for i in 0..across {
            let a = -half + 2.0 * half * (i as f32 + self.scatter.unit()) / across as f32;
            // Started anywhere in the ring and torn to its own length and slant, so the
            // rings never line up into a lattice.
            let r0 = r - RING * (0.5 + self.scatter.unit());
            let len = RING * (0.8 + 1.4 * self.scatter.unit());
            let from = self.wake_ground(wake, r0.max(1.0), a);
            let bend = self.scatter.signed() * 0.05;
            let to = self.wake_ground(wake, (r0 + len).min(range), a + bend);
            let life = (0.35 + 0.35 * self.scatter.unit()) * (1.0 - 0.35 * out);
            let w = (0.3 + 0.35 * self.scatter.unit()) * (1.2 - 0.5 * out) * impact;
            self.plasma_fx.guns.filament(from, to, when, life, w);
        }

        // Clumps rolled along the front's foot, cooling as they go.
        if k.is_multiple_of(3) {
            let clumps = ((width / CLUMP_GAP).ceil() as usize).clamp(1, 8);
            for i in 0..clumps {
                let a = -half + 2.0 * half * (i as f32 + self.scatter.unit()) / clumps as f32;
                let p = self.wake_ground(wake, r, a);
                let v =
                    (turned(wake.ahead, a) * speed * 0.6).extend(1.0 + 2.0 * self.scatter.unit());
                let blob = (0.7 + 0.5 * self.scatter.unit()) * impact;
                let tint = RED.lerp(HOT, self.scatter.unit() * 0.2) * 1.8;
                self.push_lit(GLOB, p, v, when, 0.45, (blob, blob * 0.35), tint, 0.0);
            }
        }

        // Its light runs with the crest; the trees bow as it goes by.
        if k % 3 == 1 {
            let h = Self::crest(r, range, 0.0, half, impact);
            let mid = self.wake_ground(wake, r, 0.0) + Vec3::Z * h * 0.7;
            self.plasma_fx.guns.flare(
                mid,
                RED * 120.0 * impact,
                width * 0.6 + h * 2.0 + 8.0,
                when,
                0.2,
            );
        }
        if k % 6 == 2 {
            let mid = self.wake_ground(wake, r, 0.0);
            self.tree_blasts
                .record(mid, when, width * 0.5 + 12.0, 0.8, true);
        }

        // The ground glassed where it passed: a few big pools, each a cluster of
        // overlapping ones of different sizes so its edge is ragged, not a ring.
        if k % 8 == 3 {
            let pools = ((width / POOL_GAP).ceil() as usize).clamp(1, MAX_POOLS);
            for i in 0..pools {
                let a = -half + 2.0 * half * (i as f32 + self.scatter.unit()) / pools as f32;
                let shift = self.scatter.signed() * RING * 1.5;
                let p = self.wake_ground(wake, r + shift, a);
                if p.z - SKIM <= water + 0.1 {
                    continue;
                }
                let size = (4.0 + 4.0 * self.scatter.unit()) * (1.1 - 0.3 * out);
                for _ in 0..3 {
                    let lobe = Vec2::new(self.scatter.signed(), self.scatter.signed()) * size * 0.6;
                    let radius = size * (0.45 + 0.4 * self.scatter.unit());
                    self.ground_melt
                        .melt(p.truncate() + lobe, radius, when, 8.0);
                }
            }
        }
    }

    /// Where it runs out: the curl breaks, its crest flung forward in sparks.
    fn wake_breaks(&mut self, wake: &RollingWake, range: f32, speed: f32, half: f32, impact: f32) {
        let end = wake.start + range / speed;
        for _ in 0..18 {
            let a = self.scatter.signed() * half;
            let h = Self::crest(range, range, a, half, impact);
            let p =
                self.wake_ground(wake, range, a) + Vec3::Z * h * (0.4 + 0.6 * self.scatter.unit());
            let fling = (turned(wake.ahead, a) * (0.6 + 0.4 * self.scatter.unit()))
                .extend(0.2 + 0.6 * self.scatter.unit())
                .normalize_or(Vec3::Z);
            let (pace, life) = (
                14.0 + 16.0 * self.scatter.unit(),
                0.4 + 0.4 * self.scatter.unit(),
            );
            self.push_lit(
                MOTE,
                p,
                fling * pace,
                end,
                life,
                (0.4, 0.14),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
    }
}

/// How far out ring `k` of a wake is laid, metres.
fn ring_at(k: usize) -> f32 {
    2.0 + k as f32 * RING
}

/// `v` turned `a` radians about the upright.
fn turned(v: Vec2, a: f32) -> Vec2 {
    let (sin, cos) = a.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}
