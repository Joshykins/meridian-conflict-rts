//! The Argon Electric Bore's discharge (docs/STYLE.md "The electric bore").
//!
//! The tracer round is an ordinary shot. When it lands the sim reports
//! `SimEvent::BoreDischarge` (right after the tracer's `Impact`), and this draws what
//! follows: a straight plasma column down the ionised channel, with branching
//! lightning re-igniting around it in a sustained train of return strokes; a flash at each kink that lights the ground; and,
//! where the channel ran low over the ground, a molten track that glows white, then
//! orange, then dull red as it crusts over and cools. The sim's scorch stays under it.
//!
//! Presentation only. The molten track is ground stains with `STAIN_MOLTEN` set, whose
//! low byte is the heat left (0 to 255), rewritten every frame as the track cools, and
//! uploaded after the wreck craters.

use super::{Renderer, PUFF_BOLT, PUFF_SPARK, PUFF_TREE_SMOKE};
use glam::{Vec2, Vec3};
use mc_sim::mirror::{ProjectileInstance, StainInstance, PROJECTILE_FADE_BEAM};
use std::mem::size_of;

/// `strength_seed` bit that makes a stain molten ground (ground.wgsl `molten`).
const STAIN_MOLTEN: u32 = 1 << 30;
/// Molten patches kept, at most; the oldest go first.
const MAX_MOLTEN: usize = 4096;
/// Colour byte of a lightning stroke among the fading beams (sprites.wgsl).
const BOLT: u32 = 3;
/// Straight plasma column inside the surrounding electrical arcs.
const PLASMA_COLUMN: u32 = 4;
/// Bolt strokes kept, at most.
const MAX_STROKES: usize = 4096;
/// Overlapping return strokes keep the channel alive while its branching shape changes.
/// (delay, lifetime, relative width), in seconds from the tracer impact.
const DISCHARGE_STROKES: [(f32, f32, f32); 5] = [
    (0.0, 0.24, 1.25),
    (0.12, 0.42, 1.0),
    (0.31, 0.48, 1.15),
    (0.55, 0.46, 0.85),
    (0.80, 0.48, 0.65),
];

struct Molten {
    pos: Vec2,
    radius: f32,
    seed: u32,
    start: f32,
    cool: f32,
}

struct Stroke {
    from: Vec3,
    to: Vec3,
    start: f32,
    life: f32,
    width: f32,
    color: u32,
}

#[derive(Default)]
pub(super) struct BoreFx {
    molten: Vec<Molten>,
    next: usize,
    strokes: Vec<Stroke>,
    /// This frame's molten stains, heat filled in.
    stains: Vec<StainInstance>,
    seed: u32,
}

impl BoreFx {
    pub(super) fn clear(&mut self) {
        *self = BoreFx::default();
    }

    fn push_molten(&mut self, m: Molten) {
        if self.molten.len() < MAX_MOLTEN {
            self.molten.push(m);
        } else {
            self.molten[self.next] = m;
            self.next = (self.next + 1) % MAX_MOLTEN;
        }
    }

    /// The molten stains as they stand at `time`: heat in the low byte.
    pub(super) fn molten_stains(&mut self, time: f32) -> &[StainInstance] {
        self.stains.clear();
        for m in &self.molten {
            let age = (time - m.start) / m.cool.max(0.01);
            if !(0.0..1.0).contains(&age) {
                continue;
            }
            let heat = ((1.0 - age) * 255.0).round() as u32;
            self.stains.push(StainInstance {
                pos: m.pos.to_array(),
                radius: m.radius,
                strength_seed: STAIN_MOLTEN | heat.min(255) | (m.seed & 0x7FFF) << 8,
            });
        }
        &self.stains
    }
}

impl Renderer {
    /// The charge struck down a bore's channel from `from` to `to`, landing `after` of
    /// a tick into it. `width`: how far either side it sears (zero for the tracer's
    /// strike alone). `splash` and `cool` come from the weapon.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bore_discharge(
        &mut self,
        from: Vec3,
        to: Vec3,
        width: f32,
        splash: f32,
        cool: f32,
        after: f32,
        time: f32,
    ) {
        let start = time + after * self.tick_seconds;
        let length = from.distance(to);
        if length < 1.0 {
            return;
        }
        let along = (to - from) / length;
        let side = along.cross(Vec3::Z).normalize_or_zero();
        let up = side.cross(along).normalize_or_zero();
        // The bolt wanders off the channel by a share of its length (heavier guns more),
        // in kinks of uneven length, and throws short forks off the first stroke.
        let wander = (length * 0.033 + width * 0.35).clamp(2.0, 16.0);
        let core = 0.9 + width * 0.13;
        // The tracer ignites a straight, sustained plasma column. Only the arcs
        // around it wander; the hot central channel stays locked muzzle-to-impact.
        self.bore_fx.strokes.push(Stroke {
            from, to, start: start + 0.02, life: 1.26,
            width: core * 3.6, color: PLASMA_COLUMN,
        });
        // Successive paths re-ignite the surrounding lightning.
        for (stroke, (delay, life, thick)) in DISCHARGE_STROKES.into_iter().enumerate() {
            let kinks = ((length / 15.0) as usize).clamp(7, 44);
            let mut last = from;
            let mut t = 0.0;
            for k in 1..=kinks {
                let even = 1.0 / kinks as f32;
                t = if k == kinks { 1.0 } else { (t + even * (0.45 + self.scatter.unit() * 1.1)).min(1.0 - even * 0.3) };
                // Pinned at both ends, widest in the middle.
                let taper = (t * std::f32::consts::PI).sin().sqrt();
                let jitter = if k == kinks {
                    Vec3::ZERO
                } else {
                    (side * (self.scatter.unit() - 0.5) + up * (self.scatter.unit() - 0.5) * 0.6)
                        * 2.0 * wander * taper
                };
                let mut next = from + (to - from) * t + jitter;
                // Keep low shots connected above the terrain; a downward kink must
                // not disappear through the ground while the channel is still live.
                if k < kinks {
                    next.z = next.z.max(self.ground_height(next.truncate()) + 0.35);
                }
                self.bore_fx.strokes.push(Stroke { from: last, to: next, start: start + delay, life, width: core * thick, color: BOLT });
                // A fork: two or three short kinks off to one side, thinner, gone sooner.
                if k < kinks && self.scatter.unit() < if stroke % 2 == 0 { 0.25 } else { 0.14 } {
                    let mut tip = next;
                    let away = (side * (self.scatter.unit() - 0.5) * 2.0 + up * (self.scatter.unit() * 0.6 - 0.1))
                        .normalize_or_zero();
                    let reach = length * (0.03 + self.scatter.unit() * 0.05);
                    let bits = 2 + (self.scatter.unit() * 2.0) as usize;
                    for _ in 0..bits {
                        let step = (along * 0.6 + away) .normalize_or_zero() * (reach / bits as f32)
                            + (side * (self.scatter.unit() - 0.5) + up * (self.scatter.unit() - 0.5)) * wander * 0.5;
                        let mut end = tip + step;
                        end.z = end.z.max(self.ground_height(end.truncate()) + 0.35);
                        self.bore_fx.strokes.push(Stroke { from: tip, to: end, start: start + delay, life: life * 0.65, width: core * 0.32 * thick, color: BOLT });
                        tip = end;
                    }
                }
                last = next;
            }
        }
        if self.bore_fx.strokes.len() > MAX_STROKES {
            let extra = self.bore_fx.strokes.len() - MAX_STROKES;
            self.bore_fx.strokes.drain(..extra);
        }

        // Blue-white light pulses along the whole channel with each return stroke.
        let lamps = ((length / 60.0) as usize).clamp(2, 12);
        for (delay, life, thick) in DISCHARGE_STROKES {
            for k in 0..=lamps {
                let at = from + (to - from) * (k as f32 / lamps as f32);
                self.push_effect(at.to_array(), start + delay, (2.6 + width * 0.45) * thick, life * 0.8, 0.0, 0.0);
            }
            // A contained contact flare lets the branching channel remain readable.
            self.push_effect(to.to_array(), start + delay, (splash * 0.45).max(4.0) * thick, life * 0.75, 0.0, 0.0);
        }
        for _ in 0..(10.0 + width * 2.0) as usize {
            let dir = Vec3::new(
                self.scatter.unit() - 0.5,
                self.scatter.unit() - 0.5,
                self.scatter.unit() * 0.8,
            )
            .normalize_or_zero();
            let speed = 20.0 + self.scatter.unit() * 45.0;
            let life = 0.25 + self.scatter.unit() * 0.2;
            self.push_puff(PUFF_BOLT, to, dir * speed, start, life, (1.4, 0.4));
        }

        let water = self.map_info.water_level.to_f32();
        // What the tracer's strike alone melts: a small pool where it landed.
        let pool = (splash * 0.7).max(2.0);
        if to.z - self.ground_height(to.truncate()) < pool && self.ground_height(to.truncate()) >= water {
            let seed = self.bore_fx.bump();
            self.bore_fx.push_molten(Molten { pos: to.truncate(), radius: pool, seed, start, cool });
        }
        if width <= 0.0 {
            return;
        }
        // The channel's molten track, where it ran low over dry ground, and smoke
        // coming off it as it cools.
        let wind = self.sky.wind_heading();
        // Pools close enough to run together into one gouge.
        let step = (width * 0.3).max(1.5);
        let n = ((length / step) as usize).min(600);
        for k in 0..=n {
            let at = from + (to - from) * (k as f32 / n.max(1) as f32);
            let ground = self.ground_height(at.truncate());
            if ground < water || at.z - ground > width * 2.0 {
                continue;
            }
            let seed = self.bore_fx.bump();
            let wobble = 0.75 + (seed % 97) as f32 / 97.0 * 0.5;
            self.bore_fx.push_molten(Molten {
                pos: at.truncate(),
                radius: width * 0.6 * wobble,
                seed,
                start,
                cool,
            });
            let ground_at = at.truncate().extend(ground + 0.5);
            if k % 5 == 0 {
                let spark = Vec3::new(self.scatter.unit() - 0.5, self.scatter.unit() - 0.5, 1.0) * 14.0;
                self.push_puff(PUFF_SPARK, ground_at, spark, start, 0.6, (0.5, 0.2));
            }
            // A thin wisp now and then off the cooling track, carried off on the wind.
            if k % 20 == 7 {
                let drift = (wind * (2.0 + self.scatter.unit() * 2.0)).extend(0.0);
                let life = 4.0 + self.scatter.unit() * 3.0;
                self.push_puff(PUFF_TREE_SMOKE, ground_at, drift, start + 0.5, life, (width * 0.15, width * 0.6));
            }
        }
    }

    /// Writes this frame's lightning strokes after the fading beams, into the
    /// projectile buffer, as fading beams of their own colour.
    pub(super) fn write_bore_strokes(&mut self, time: f32) {
        self.bore_fx.strokes.retain(|s| time < s.start + s.life);
        let size = size_of::<ProjectileInstance>();
        for s in &self.bore_fx.strokes {
            if time < s.start { continue; }
            let i = self.projectile_count as usize;
            if i >= super::MAX_PROJECTILES {
                break;
            }
            let inst = ProjectileInstance {
                prev_pos: s.from.to_array(),
                color: PROJECTILE_FADE_BEAM | s.color,
                pos: s.to.to_array(),
                size: s.width,
                wake: s.start,
                plasma: s.life,
                _pad: [0.0; 2],
                aim: [0.0; 4],
                prev_aim: [0.0; 4],
            };
            self.projectiles.write((i * size) as u64, bytemuck::bytes_of(&inst));
            self.projectile_count += 1;
        }
    }
}

impl BoreFx {
    fn bump(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.seed >> 8
    }
}
