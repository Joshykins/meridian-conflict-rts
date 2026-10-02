//! A cone weapon's wake as it is drawn (`Weapon::cone`, the Regency Wake's projector;
//! docs/STYLE.md "The Regency suite"): the plasma the projector gathered across its mouth
//! (drawn as a Pinched charge, `regency_guns_fx`) is pinched out flat and rolls out over the
//! ground ahead as a fan, as wide as the cone the sim strikes and as long as its reach.
//!
//! - **The mouth:** a hard red flash where the charge hung, the charge collapsing into it.
//! - **The front:** a hard bright edge across the fan, each short piece of it lit only as
//!   it passes, so it reads as one line running out over the ground.
//! - **The fan:** hot red filaments laid radially across the cone, each lit as the front
//!   reaches it and cooling and breaking up behind it (sprites.wgsl's plasma trail, as a
//!   Pinched bolt's trail), so the front reads as a hard bright edge running out over the
//!   ground with torn streaks behind it. Clumps of plasma roll forward along the front and
//!   are eaten through as they cool; hard red hearts break out of it here and there.
//! - **The ground:** glassed where it passed: a few big ragged molten pools down the fan,
//!   glowing and crusting over (`BoreFx::melt`).
//! - **Its light** runs out with the front and goes out behind it.
//!
//! No mist, no rings, nothing wound round a middle: everything is hard-edged and short-lived.
//! Laid out whole on the tick it fires, each piece timed to the front's arrival (the sim has
//! already struck the fan, `mc-sim` `wake.rs`). Presentation only; the renderer's own clock.

use super::regency_guns_fx::{BURST, GLOB, GLOW, HOT, MOTE, RED};
use super::Renderer;
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;

/// Metres a second the wake's front runs out over the ground.
const FRONT_SPEED: f32 = 260.0;
/// Metres between the rings of filaments laid across the fan.
const RING: f32 = 5.0;
/// Metres apart across the fan the filaments of a ring are laid, the clumps rolled along
/// its front, and the pools glassed into the ground.
const FILAMENT_GAP: f32 = 7.0;
const CLUMP_GAP: f32 = 14.0;
const POOL_GAP: f32 = 45.0;
/// Most glassed pools a ring of them is laid with: a deliberate cosmetic cap.
const MAX_POOLS: usize = 3;
/// Most filaments a ring is laid with: a deliberate cosmetic cap, so a long wide cone
/// costs no more than this many a ring.
const MAX_ACROSS: usize = 28;
/// How far up from the ground the fan is drawn, metres.
const SKIM: f32 = 0.6;

impl Renderer {
    /// A cone weapon fired (`ShotFired` of a `cone` weapon): `at` its muzzle as drawn, `dir`
    /// down the bore. Lays the whole wake out, each piece timed to the front.
    pub(super) fn wake_fired(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        dir: Vec3,
        time: f32,
    ) {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let Some(cone) = w.cone else {
            return;
        };
        let range = w.range_max.to_f32();
        let half = cone.half.0 as f32 / 65536.0 * std::f32::consts::TAU;
        let (flash, impact) = (w.flash.max(0.3), w.impact.max(0.3));
        let ahead = dir.truncate().normalize_or(Vec2::X);
        let water = self.map_info.water_level.to_f32();

        // The mouth: the charge collapses into a hard red flash as the plasma goes out flat.
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

        // Where on the fan a point `r` out at `a` radians off its middle stands, on the ground
        // (or the water) under it; close to the mouth it leaves the projector's height.
        let ground = |me: &Renderer, r: f32, a: f32| -> Vec3 {
            let (sin, cos) = a.sin_cos();
            let xy = at.truncate()
                + Vec2::new(ahead.x * cos - ahead.y * sin, ahead.x * sin + ahead.y * cos) * r;
            let floor = me.ground_height(xy).max(water) + SKIM;
            let lift = (1.0 - r / 14.0).clamp(0.0, 1.0);
            xy.extend(floor + (at.z - floor).max(0.0) * lift * lift)
        };

        let rings = (range / RING).ceil() as usize;
        for k in 0..rings {
            let r = 2.0 + k as f32 * RING;
            if r > range {
                break;
            }
            let out = r / range;
            let when = time + r / FRONT_SPEED;
            let width = 2.0 * r * half.tan();
            // Filaments across the fan, lit as the front passes: thicker and longer-lived
            // close in, where the wake is densest.
            let across = ((width / FILAMENT_GAP).ceil() as usize).clamp(2, MAX_ACROSS);
            for i in 0..across {
                let a = -half + 2.0 * half * (i as f32 + self.scatter.unit()) / across as f32;
                // Started anywhere in the ring and torn to its own length and slant, so
                // the rings never line up into a lattice.
                let r0 = r + self.scatter.signed() * RING * 0.5;
                let len = RING * (0.6 + 1.4 * self.scatter.unit());
                let from = ground(self, r0.max(1.0), a);
                let bend = self.scatter.signed() * 0.08;
                let to = ground(self, (r0 + len).min(range), a + bend);
                let life = (0.3 + 0.3 * self.scatter.unit()) * (1.0 - 0.35 * out);
                let width = (0.3 + 0.35 * self.scatter.unit()) * (1.2 - 0.5 * out) * impact;
                self.plasma_fx.guns.filament(from, to, when, life, width);
            }
            // The front itself: a hard bright edge across the fan, short pieces of it lit
            // only as it passes, so it reads as one line running out over the ground.
            // Ragged: each piece a little ahead of or behind the line, its ends off it, and
            // here and there a gap.
            let pieces = ((width / 3.0).ceil() as usize).clamp(2, MAX_ACROSS);
            for i in 0..pieces {
                if self.scatter.unit() < 0.2 {
                    continue;
                }
                let a0 =
                    -half + 2.0 * half * (i as f32 + 0.3 * self.scatter.unit()) / pieces as f32;
                let a1 = -half
                    + 2.0 * half * (i as f32 + 0.8 + 0.4 * self.scatter.unit()) / pieces as f32;
                let (d0, d1) = (self.scatter.signed() * 1.6, self.scatter.signed() * 1.6);
                let (from, to) = (ground(self, r + d0, a0), ground(self, r + d1, a1.min(half)));
                let w = (0.5 + 0.4 * self.scatter.unit()) * (1.0 - 0.4 * out) * impact;
                self.plasma_fx.guns.filament(from, to, when, 0.14, w);
            }
            // Clumps rolling forward along the front every other ring, cooling as they go.
            if k % 3 == 0 {
                let clumps = ((width / CLUMP_GAP).ceil() as usize).clamp(1, 12);
                for i in 0..clumps {
                    let a = -half + 2.0 * half * (i as f32 + self.scatter.unit()) / clumps as f32;
                    let p = ground(self, r, a);
                    let (sin, cos) = a.sin_cos();
                    let radial =
                        Vec2::new(ahead.x * cos - ahead.y * sin, ahead.x * sin + ahead.y * cos);
                    let v = (radial * FRONT_SPEED * 0.1).extend(1.5 + 3.0 * self.scatter.unit());
                    let blob = (0.7 + 0.5 * self.scatter.unit()) * impact;
                    let tint = RED.lerp(HOT, self.scatter.unit() * 0.2) * 1.8;
                    self.push_lit(GLOB, p, v, when, 0.5, (blob, blob * 0.35), tint, 0.0);
                }
            }
            // Hard hearts breaking out of the front, and its light running with it.
            if k % 4 == 1 {
                let off = self.scatter.signed() * half * 0.8;
                let p = ground(self, r, off);
                let h = (1.4 + 0.8 * self.scatter.unit()) * impact;
                self.push_lit(
                    BURST,
                    p,
                    Vec3::ZERO,
                    when,
                    0.28,
                    (h * 0.5, h * 2.0),
                    RED * 3.0,
                    0.0,
                );
                let mid = ground(self, r, 0.0) + Vec3::Z * 2.0;
                self.plasma_fx
                    .guns
                    .flare(mid, RED * 90.0 * impact, width * 0.6 + 8.0, when, 0.25);
            }
            // The ground glassed where it passed.
            // A few big pools, each a cluster of overlapping ones of different sizes so
            // its edge is ragged, not a ring.
            if k % 6 == 3 {
                let pools = ((width / POOL_GAP).ceil() as usize).clamp(1, MAX_POOLS);
                for i in 0..pools {
                    let a = -half + 2.0 * half * (i as f32 + self.scatter.unit()) / pools as f32;
                    let shift = self.scatter.signed() * RING * 1.5;
                    let p = ground(self, r + shift, a);
                    if p.z - SKIM <= water + 0.1 {
                        continue;
                    }
                    let size = (4.0 + 4.0 * self.scatter.unit()) * (1.1 - 0.3 * out);
                    for _ in 0..3 {
                        let lobe =
                            Vec2::new(self.scatter.signed(), self.scatter.signed()) * size * 0.6;
                        let radius = size * (0.45 + 0.4 * self.scatter.unit());
                        self.bore_fx.melt(p.truncate() + lobe, radius, when, 8.0);
                    }
                }
            }
        }
        // Where it runs out: the front breaks, flinging sparks up off its edge.
        let end = time + range / FRONT_SPEED;
        for _ in 0..14 {
            let a = self.scatter.signed() * half;
            let p = ground(self, range, a);
            let up = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.6 + self.scatter.unit(),
            )
            .normalize_or(Vec3::Z);
            let life = 0.4 + 0.4 * self.scatter.unit();
            let speed = 10.0 + 14.0 * self.scatter.unit();
            self.push_lit(
                MOTE,
                p,
                up * speed,
                end,
                life,
                (0.4, 0.14),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
    }
}
