//! The rift: the blast at the engine's foot as it tears out, the fissure racing across
//! the ground with a white head, slabs thrown up either side and lightning crawling along
//! it; and the scar it leaves, lit while the well holds (`ground.rs` pulses light along it
//! toward the well), dark once let go.

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

use super::engine::{fx_lamp, BOLT};
use super::slabs::{shard_model, slab_model, Motion, Piece};
use super::{Engine, LILAC, VIOLET, WHITE};
use crate::camera::Camera;
use crate::gpu_consts::{effect, puff, rift};
use crate::renderer::lens_flare::Flare;
use crate::renderer::stun_fx::Flash;
use crate::renderer::{Renderer, PUFF_CLOD, PUFF_DUST, PUFF_SHOCK_DUST};
use mc_sim::mirror::WellView;

const GLOW: f32 = puff::WARP_GLOW as f32;
/// How fast the rift runs, m/s, as the effects time it; the sim's front corrects it each
/// tick (`RiftGround::line_cap`).
pub(super) const SPEED: f32 = 900.0;
/// Half the width of the band of broken ground either side of the fissure, metres.
pub(super) const WIDTH: f32 = 26.0;
/// Metres between the slabs thrown up along it, each side, at least; a long rift spaces
/// them out so it never has more than `MOST_ALONG` a side (a deliberate cosmetic cap).
const SLAB_STEP: f32 = 13.0;
const MOST_ALONG: f32 = 220.0;

impl Renderer {
    /// The rift tears out of the ground under the engine (`from`) toward `mark`.
    pub(super) fn rift_launched(&mut self, unit: u32, from: Vec3, mark: Vec3, time: f32) {
        let Some(e) = self
            .gravitic_engine_fx
            .engines
            .iter()
            .find(|e| e.id == unit && e.shown)
            .copied()
        else {
            // The engine not drawn (out of sight): the ground still breaks.
            self.gravitic_engine_fx.ground.line(
                unit,
                from.truncate(),
                mark.truncate(),
                SPEED,
                WIDTH,
                time,
            );
            return;
        };
        self.gravitic_engine_fx.ground.line(
            unit,
            from.truncate(),
            mark.truncate(),
            SPEED,
            WIDTH,
            time,
        );
        let toward = (mark - from).truncate().normalize_or(Vec2::X);
        let r = e.radius;
        // The blast down at the foot: a hard flash in the crown and where the rift leaves
        // the ring, a pressure ring, dust driven out all round.
        let gate = e.base.truncate() + toward * r;
        let gate = gate.extend(self.ground_height(gate));
        self.push_effect(
            e.core.to_array(),
            time,
            r * 0.5,
            0.35,
            effect::GRAVITIC as f32,
            0.0,
        );
        self.push_effect(
            (gate + Vec3::Z * 10.0).to_array(),
            time,
            r * 0.45,
            0.5,
            effect::GRAVITIC as f32,
            0.0,
        );
        self.lens_flares.flash(
            e.core,
            Flare {
                color: LILAC * 30.0,
                size: r * 1.6,
            },
            time,
            0.6,
        );
        self.emp_fx.flashes.push(Flash {
            pos: e.core,
            start: time,
            life: 1.2,
            color: LILAC * 40_000.0,
            range: r * 5.0,
            flicker: 0.3,
        });
        self.gravity_wave(e.base + Vec3::Z * 4.0, time, r * 3.2, 1.6, 1.0);
        for i in 0..40 {
            let a = TAU * (i as f32 + self.scatter.unit() * 0.5) / 40.0;
            let dir = Vec3::new(a.cos(), a.sin(), 0.06);
            let at = e.base + dir * r * 1.05;
            let at = at.truncate().extend(self.ground_height(at.truncate()));
            let speed = r * (0.5 + 0.4 * self.scatter.unit());
            self.push_puff(
                PUFF_SHOCK_DUST,
                at,
                dir * speed,
                time,
                3.5,
                (r * 0.08, r * 0.22),
            );
        }
        // Lightning down out of the crown, out along the way the rift goes.
        for _ in 0..8 {
            let to =
                gate.truncate() + Vec2::new(self.scatter.signed(), self.scatter.signed()) * r * 0.4;
            let to = to.extend(self.ground_height(to));
            let side = toward.extend(0.0).cross(Vec3::Z);
            let start = time + 0.08 * self.scatter.unit();
            self.emp_kinks(
                e.core,
                to,
                r * 0.12,
                side,
                Vec3::Z,
                start,
                0.25,
                r * 0.03,
                10,
                BOLT,
            );
        }
        // Clods and rock thrown up out of the gate.
        for _ in 0..30 {
            let vel = (toward.extend(0.0) * (0.3 + self.scatter.unit())
                + Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.5
                + Vec3::Z * (0.8 + self.scatter.unit()))
                * (25.0 + 30.0 * self.scatter.unit());
            self.push_puff(PUFF_CLOD, gate, vel, time, 3.0, (1.6, 0.8));
        }
    }

    /// A tick of the rift running: its front from where it was to where it is, slabs and
    /// clods thrown up as it passes, dust, lightning crawling behind the head.
    pub(super) fn rift_running(&mut self, e: &Engine, v: &WellView, time: f32, camera: &Camera) {
        let from = Vec3::from(v.from).truncate();
        let front = Vec3::from(v.front).truncate();
        let mark = Vec3::from(v.mark).truncate();
        let run = front.distance(from);
        let length = mark.distance(from).max(1.0);
        let dt = self.tick_seconds.max(0.02);
        let fx = &mut self.gravitic_engine_fx;
        // The front may get a tick ahead of where the sim has it, no further.
        fx.ground
            .line_cap(e.id, (run + SPEED * dt * 1.2).min(length));
        let dim = if v.powered { 1.0 } else { 0.3 };
        fx.ground
            .feed(e.id, rift::LINE, [0.6 * dim, 0.0, 0.5, 1.0, 1.0]);
        let along = (mark - from) / length;
        let side = Vec2::new(-along.y, along.x);
        let was = e.run.min(run);
        let reach = camera.distance * 2.5 + 1500.0;
        let focus = camera.focus.truncate();
        // The head: white-hot light where the ground is tearing now.
        let head = front.extend(self.ground_height(front) + 6.0);
        self.push_lit(
            GLOW,
            head,
            along.extend(0.0) * SPEED * 0.5,
            time,
            dt * 1.6,
            (WIDTH * 1.2, WIDTH * 0.8),
            WHITE.lerp(LILAC, 0.4) * 6.0,
            0.0,
        );
        fx_lamp(self, head + Vec3::Z * 20.0, LILAC * 30_000.0 * dim, 260.0);
        // Slabs thrown up either side as the front passes, timed to it, all the way along
        // (they lie as the scar), and the close-up work near the eye only.
        let step = SLAB_STEP.max(length / MOST_ALONG);
        let first = (was / step).ceil() as i32;
        let last = (run / step).floor() as i32;
        for k in first..=last {
            let s = k as f32 * step;
            let at = from + along * s;
            let start = time + (s - was).max(0.0) / SPEED;
            let near = at.distance(focus) < reach;
            for sign in [-1.0f32, 1.0] {
                // Uneven: here a gap, there a slab heaved far out, never a kerb.
                if self.scatter.unit() < 0.3 {
                    continue;
                }
                let out = WIDTH * (0.3 + 1.6 * self.scatter.unit().powi(2));
                let p = at + side * sign * out + along * self.scatter.signed() * step * 0.4;
                let scale = 1.0 + 1.6 * self.scatter.unit();
                let throw = 6.0 + 16.0 * self.scatter.unit();
                let piece = Piece {
                    ground: self.ground_height(p),
                    engine: e.id,
                    model: if self.scatter.unit() < 0.8 {
                        slab_model(self.scatter.unit())
                    } else {
                        shard_model(self.scatter.unit())
                    },
                    at: p,
                    heading: along.y.atan2(along.x) + 0.4 * self.scatter.signed(),
                    scale,
                    born: start,
                    rise: 0.1,
                    lift: 0.0,
                    // Tipped up toward the fissure as it lands on its far edge.
                    pitch: 0.15 * self.scatter.signed(),
                    roll: sign * (0.2 + 0.9 * self.scatter.unit()),
                    motion: Motion::Thrown {
                        vel: (side * sign * (2.0 + 6.0 * self.scatter.unit())).extend(throw),
                    },
                    seed: self.scatter.unit(),
                };
                self.gravitic_engine_fx.slabs.add(piece);
                if !near {
                    continue;
                }
                let ground = p.extend(self.ground_height(p));
                let life = 4.0 + 2.0 * self.scatter.unit();
                self.push_puff(
                    PUFF_DUST,
                    ground,
                    (side * sign * 4.0).extend(5.0),
                    start,
                    life,
                    (WIDTH * 0.3, WIDTH * 0.9),
                );
                for _ in 0..2 {
                    let vel = (side * sign * (4.0 + 10.0 * self.scatter.unit())
                        + along * 6.0 * self.scatter.signed())
                    .extend(14.0 + 18.0 * self.scatter.unit());
                    let size = 0.9 + 0.8 * self.scatter.unit();
                    self.push_puff(PUFF_CLOD, ground, vel, start, 2.5, (size, 0.6));
                }
            }
            if !near {
                continue;
            }
            // Lightning crawling along the fissure just behind the head.
            if k % 5 == 0 {
                let a = (at - along * step * 2.0).extend(0.0);
                let b = (at + along * step * (1.0 + self.scatter.unit())).extend(0.0);
                let a = a.truncate().extend(self.ground_height(a.truncate()) + 1.5);
                let b = b.truncate().extend(self.ground_height(b.truncate()) + 1.5);
                self.emp_kinks(
                    a,
                    b,
                    4.0,
                    side.extend(0.0),
                    Vec3::Z,
                    start,
                    0.18,
                    1.4,
                    6,
                    BOLT,
                );
            }
            if k % 3 == 0 {
                self.emp_fx.flashes.push(Flash {
                    pos: (at.extend(self.ground_height(at) + 8.0)),
                    start,
                    life: 0.5,
                    color: VIOLET * 4000.0,
                    range: 120.0,
                    flicker: 0.6,
                });
            }
        }
    }
}
