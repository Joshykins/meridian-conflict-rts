//! The engine itself charging and holding: the column of light between its shards, the
//! lightning off it, the cracks and the heaving ground round its foot, the dust and the
//! rock lifted off the ground (the machinery's turn is `GraviticEngineFx::patch`).

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

use super::slabs::{shard_model, slab_model, Motion, Piece};
use super::{Engine, LILAC, VIOLET, WHITE};
use crate::gpu_consts::{fade_beam, puff, rift};
use crate::renderer::stun_fx::Flash;
use crate::renderer::{Renderer, PUFF_DUST, PUFF_SHOCK_DUST};
use mc_sim::mirror::WellStage;

const VORTEX: f32 = puff::GRAVITY_VORTEX as f32;
const MOTE: f32 = puff::WARP_MOTE as f32;
/// The engine's lightning (sprites.wgsl).
pub(super) const BOLT: u32 = fade_beam::RIFT_BOLT;
/// Slabs heaved round the foot and shards lifted round it, at most an engine, at full
/// charge: a deliberate cosmetic cap.
const FOOT_SLABS: f32 = 44.0;
const FOOT_SHARDS: f32 = 30.0;

/// The column of light: its foot and how tall it stands, for a charge.
pub(super) fn column(e: &Engine) -> (Vec3, f32) {
    let c = e.charge;
    let below = e.height * (0.16 + 0.06 * c);
    (
        e.core - Vec3::Z * below,
        below + e.height * (0.22 + 0.2 * c),
    )
}

/// A point on the inner faces of the shards round the core, `k` of the way round (0..1)
/// at `z` over the ground: the crown narrows as it rises (`gravitic_engine::crown`).
fn shard_face(e: &Engine, k: f32, z: f32) -> Vec3 {
    let up = (z / e.height).clamp(0.0, 1.0);
    let r = e.radius * (0.5 - 0.36 * up);
    let a = TAU * k;
    e.base + Vec3::new(a.cos() * r, a.sin() * r, z)
}

impl Renderer {
    /// One tick of the engine's own light: whatever it is doing, by how much it holds.
    pub(super) fn engine_charge(&mut self, e: &Engine, time: f32, near: bool) {
        let c = e.charge;
        let gathering = e.view.filter(|v| v.stage == WellStage::Gathering);
        let p = gathering.map_or(0.0, |v| v.progress.clamp(0.0, 1.0));
        let fx = &mut self.gravitic_engine_fx;
        fx.slabs
            .set_shake(e.id, if gathering.is_some() { p } else { 0.0 });
        // The cracks round its foot: out from the lot as it gathers, lit while it works.
        let inner = e.radius * 0.97;
        if let Some(v) = e.view {
            let grown = if v.stage == WellStage::Gathering {
                p.powf(0.7)
            } else {
                1.0
            };
            let reach = inner + e.radius * 1.5 * grown;
            fx.ground
                .foot(e.id, e.base.truncate(), inner, reach.max(inner + 1.0), time);
            let glow = if v.stage == WellStage::Gathering {
                0.25 + 0.95 * p.powf(1.5)
            } else {
                0.6
            };
            let dim = if v.powered { 1.0 } else { 0.25 };
            fx.ground.feed(
                e.id,
                rift::FOOT,
                [glow * dim, 0.0, p, 0.0, 0.3 + 0.7 * grown],
            );
        }
        if c < 0.01 {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        let flicker = if e.powered() {
            1.0
        } else {
            0.3 + 0.7 * self.scatter.unit().powi(3)
        };
        // Its light on the shards and the works round it, and on the ground under them.
        let lamp = VIOLET.lerp(LILAC, c) * 9000.0 * c * c * flicker * (e.height / 240.0).powi(2);
        fx_lamp(self, e.core, lamp, e.radius * 2.6);
        fx_lamp(
            self,
            e.base + Vec3::Z * e.height * 0.08,
            VIOLET * 2500.0 * c * flicker,
            e.radius * 2.2,
        );
        // The column of light, two ticks long, laid every tick.
        let (foot, tall) = column(e);
        let half = e.radius * (0.05 + 0.1 * c);
        self.push_lit(
            VORTEX,
            foot,
            Vec3::Z * tall,
            time,
            dt * 2.0,
            (half, half),
            VIOLET.lerp(LILAC, 0.3 * c) * (0.6 + 3.4 * c * c) * flicker,
            (0.15 + 0.85 * c).min(1.0),
        );
        // A second, wider sheath round it, fainter and wound slower: the energy in layers,
        // filling the room between the shards so it shows through their gaps.
        let sheath = e.radius * (0.1 + 0.16 * c);
        self.push_lit(
            VORTEX,
            foot + Vec3::Z * tall * 0.08,
            Vec3::Z * tall * 0.84,
            time,
            dt * 2.0,
            (sheath, sheath),
            VIOLET * (0.3 + 1.2 * c * c) * flicker,
            0.35 * c,
        );
        if !near {
            return;
        }
        self.engine_lightning(e, c, flicker, time);
        if let Some(v) = gathering {
            self.foot_breaking(e, p, v.seconds, time);
        }
    }

    /// Lightning off the column to the shards, and, as it fills, from the shards down to
    /// the ground round the foot.
    fn engine_lightning(&mut self, e: &Engine, c: f32, flicker: f32, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (foot, tall) = column(e);
        // A deliberate cap (cosmetic): at most nine arcs a tick, far fewer at a low charge.
        let arcs = ((0.6 + 8.0 * c * c) * flicker * (0.6 + 0.8 * self.scatter.unit())) as usize;
        for _ in 0..arcs.min(9) {
            let from = foot + Vec3::Z * tall * (0.15 + 0.75 * self.scatter.unit());
            let k = self.scatter.unit();
            let z =
                (from.z - e.base.z + self.scatter.signed() * e.height * 0.15).max(e.height * 0.1);
            let to = shard_face(e, k, z);
            let start = time + self.scatter.unit() * dt;
            let life = 0.07 + 0.12 * self.scatter.unit();
            let width = e.radius * (0.006 + 0.012 * c);
            let span = from.distance(to);
            let side = (to - from).cross(Vec3::Z).normalize_or(Vec3::X);
            self.emp_kinks(
                from,
                to,
                span * 0.12,
                side,
                Vec3::Z,
                start,
                life,
                width,
                7,
                BOLT,
            );
            if self.scatter.unit() < 0.35 {
                self.emp_fx.flashes.push(Flash {
                    pos: to,
                    start,
                    life: life * 1.5,
                    color: LILAC * 900.0 * c,
                    range: e.radius * 0.7,
                    flicker: 0.9,
                });
            }
        }
        // Down from the shards' feet to the ground outside the ring.
        if c > 0.35 && self.scatter.unit() < (c - 0.35) * 1.6 * flicker {
            let k = self.scatter.unit();
            let a = TAU * k;
            let from = shard_face(e, k, e.height * (0.05 + 0.2 * self.scatter.unit()));
            let out = e.radius * (1.05 + 0.6 * self.scatter.unit());
            let to = e.base + Vec3::new(a.cos() * out, a.sin() * out, 0.0);
            let to = to
                .truncate()
                .extend(self.ground_height(to.truncate()) + 0.5);
            let start = time + self.scatter.unit() * dt;
            let side = (to - from).cross(Vec3::Z).normalize_or(Vec3::X);
            let span = from.distance(to);
            self.emp_kinks(
                from,
                to,
                span * 0.1,
                side,
                Vec3::Z,
                start,
                0.16,
                e.radius * 0.02,
                9,
                BOLT,
            );
            self.emp_fx.flashes.push(Flash {
                pos: to + Vec3::Z * 4.0,
                start,
                life: 0.2,
                color: LILAC * 2500.0 * c,
                range: e.radius * 0.8,
                flicker: 1.0,
            });
            self.push_puff(
                PUFF_DUST,
                to,
                Vec3::Z * 6.0,
                start,
                2.5,
                (e.radius * 0.06, e.radius * 0.16),
            );
        }
    }

    /// The ground round the foot breaking as it gathers (`p` of the way, `seconds` in):
    /// slabs heaving, dust shaken up, shards of rock lifted off and drawn round.
    fn foot_breaking(&mut self, e: &Engine, p: f32, seconds: f32, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let inner = e.radius * 0.97;
        let reach = inner + e.radius * 1.5 * p.powf(0.7);
        // Slabs heave as the cracks reach them.
        let want = (FOOT_SLABS * p.powf(0.8)) as usize;
        let have = self.gravitic_engine_fx.slabs.count(e.id, false);
        for _ in have..want.min(have + 3) {
            let a = TAU * self.scatter.unit();
            let r = inner * 1.04 + (reach - inner * 1.04) * self.scatter.unit().sqrt();
            let scale = 1.5 + 2.4 * self.scatter.unit() * (1.0 - 0.4 * (r - inner) / e.radius);
            let piece = Piece {
                ground: e.base.z,
                engine: e.id,
                model: slab_model(self.scatter.unit()),
                at: e.base.truncate() + Vec2::new(a.cos(), a.sin()) * r,
                heading: TAU * self.scatter.unit(),
                scale,
                born: time + self.scatter.unit() * dt,
                rise: 0.5 + self.scatter.unit(),
                lift: (0.2 + 1.6 * self.scatter.unit()) * scale,
                pitch: 0.12 + 0.35 * self.scatter.signed(),
                roll: 0.3 * self.scatter.signed(),
                motion: Motion::Heave,
                seed: self.scatter.unit(),
            };
            self.gravitic_engine_fx.slabs.add(piece);
        }
        // Shards of rock lifted off the ground, drawn round the engine and rising.
        let want = (FOOT_SHARDS * ((p - 0.15) / 0.85).clamp(0.0, 1.0)) as usize;
        let have = self.gravitic_engine_fx.slabs.count(e.id, true);
        for _ in have..want.min(have + 2) {
            let a = TAU * self.scatter.unit();
            let r = e.radius * (1.05 + 0.9 * self.scatter.unit());
            let at = e.base.truncate() + Vec2::new(a.cos(), a.sin()) * r;
            let piece = Piece {
                ground: e.base.z,
                engine: e.id,
                model: shard_model(self.scatter.unit()),
                at,
                heading: TAU * self.scatter.unit(),
                scale: 0.5 + 1.8 * self.scatter.unit().powi(2),
                born: time,
                rise: 4.0 + 6.0 * self.scatter.unit(),
                lift: e.height * (0.03 + 0.25 * self.scatter.unit()),
                pitch: self.scatter.signed(),
                roll: self.scatter.signed(),
                motion: Motion::Orbit {
                    centre: e.base.truncate(),
                    rate: (0.04 + 0.08 * self.scatter.unit()) * if a > 3.0 { 1.0 } else { 0.8 },
                },
                seed: self.scatter.unit(),
            };
            self.gravitic_engine_fx.slabs.add(piece);
        }
        // Dust shaken up off the breaking ground, more as it gathers.
        let puffs = (1.0 + 7.0 * p) as usize;
        for _ in 0..puffs {
            let a = TAU * self.scatter.unit();
            let r = inner + (reach - inner) * self.scatter.unit();
            let at = e.base.truncate() + Vec2::new(a.cos(), a.sin()) * r;
            let at = at.extend(self.ground_height(at));
            let kind = if self.scatter.unit() < 0.3 {
                PUFF_SHOCK_DUST
            } else {
                PUFF_DUST
            };
            let (born, life) = (
                time + self.scatter.unit() * dt,
                3.0 + 2.0 * self.scatter.unit(),
            );
            self.push_puff(
                kind,
                at,
                Vec3::new(a.cos(), a.sin(), 0.0) * 2.0 + Vec3::Z * (2.0 + 6.0 * p),
                born,
                life,
                (e.radius * 0.04, e.radius * (0.1 + 0.08 * p)),
            );
        }
        // Motes of violet light drawn up off the cracks toward the column.
        if p > 0.3 {
            let (foot, tall) = column(e);
            for _ in 0..(2.0 + 6.0 * p) as usize {
                let a = TAU * self.scatter.unit();
                let r = inner + (reach - inner) * self.scatter.unit();
                let from = e.base + Vec3::new(a.cos() * r, a.sin() * r, 1.0);
                let to = foot + Vec3::Z * tall * self.scatter.unit();
                let life = 1.2 + self.scatter.unit();
                let born = time + self.scatter.unit() * dt;
                self.push_lit(
                    MOTE,
                    from,
                    (to - from) / life * 2.2,
                    born,
                    life,
                    (e.radius * 0.006, e.radius * 0.003),
                    VIOLET.lerp(WHITE, 0.4 * p) * (2.0 + 4.0 * p),
                    0.0,
                );
            }
        }
        // The ground jolts now and then, harder toward the launch.
        let beat = (seconds * (0.8 + 1.6 * p)).fract();
        if beat < dt * (0.8 + 1.6 * p) {
            let at = e.base;
            self.gravity_wave(
                at + Vec3::Z * 2.0,
                time,
                e.radius * (1.6 + 1.2 * p),
                1.1,
                0.25 + 0.5 * p,
            );
        }
    }
}

/// A light this tick (`GraviticEngineFx::lamps`).
pub(super) fn fx_lamp(r: &mut Renderer, pos: Vec3, color: Vec3, range: f32) {
    r.gravitic_engine_fx.lamps.push((pos, color, range));
}
