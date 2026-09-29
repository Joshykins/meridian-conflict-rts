//! An EMP stun in the world (`UnitInstance::stun`; the hull's own look, dark with arcs
//! crawling over it, is emp.wgsl). Around a stunned unit near the eye, every tick:
//!
//! - arcs jump between points on its hull, blue-white and short-lived, and now and then
//!   one runs half its length and forks (the fading lightning strokes `sprites.wgsl`
//!   draws as colour 3);
//! - sparks snap off where they land;
//! - each tick's brightest arc lights the hull round it for a moment;
//! - a capital hull trails a thin grey smoke of scorched insulation.
//!
//! All of it thins out as the stun wears off, and is capped: the nearest `MAX_UNITS`
//! stunned units within `NEAR` metres of the eye, a dozen arcs each a tick. A warp
//! dampener's tether (`damper_fx.rs`) is laid through the same strokes.

use super::{Renderer, PUFF_BOLT, PUFF_SMOKE, PUFF_SPARK};
use crate::camera::Camera;
use glam::Vec3;
use mc_sim::mirror::{
    RenderFrame, UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK, PROJECTILE_FADE_BEAM,
};
use std::mem::size_of;

/// Fading-beam colour (sprites.wgsl): lightning, near white with a blue edge.
pub(super) const BOLT: u32 = 3;
/// Strokes kept at most (arcs and tethers together).
const MAX_STROKES: usize = 1600;
/// Metres from the eye's focus past which a stun is left to the shader.
const NEAR: f32 = 3500.0;
/// Stunned units given world arcs at most, the nearest first.
const MAX_UNITS: usize = 24;
/// The arcs' light.
const ARC_LIGHT: Vec3 = Vec3::new(0.45, 0.62, 1.0);

pub(super) struct Stroke {
    pub(super) from: Vec3,
    pub(super) to: Vec3,
    pub(super) start: f32,
    pub(super) life: f32,
    pub(super) width: f32,
    pub(super) color: u32,
}

/// A moment of light: an arc's, or a tether's surge.
pub(super) struct Flash {
    pub(super) pos: Vec3,
    pub(super) start: f32,
    pub(super) life: f32,
    pub(super) color: Vec3,
    pub(super) range: f32,
    /// How much it flickers, 0 steady to 1.
    pub(super) flicker: f32,
}

#[derive(Default)]
pub(super) struct EmpFx {
    pub(super) strokes: Vec<Stroke>,
    pub(super) flashes: Vec<Flash>,
    last: f32,
    /// When each dampener's edge was last laid (`damper_fx`), by unit id.
    pub(super) veils: Vec<(u32, f32)>,
}

/// A hull's frame at the end of the tick: origin, forward, left, up. A capital ship's is
/// heeled and pitched as the entity shader draws it (its list while stunned).
fn hull_frame(u: &UnitInstance, capital: bool) -> (Vec3, Vec3, Vec3, Vec3) {
    let yaw = u.heading;
    let (bank, pitch) = if capital {
        (u._pad2[1], u.arm_pitch[1])
    } else {
        (0.0, 0.0)
    };
    let horizontal = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
    let forward = horizontal * pitch.cos() + Vec3::Z * pitch.sin();
    let up = Vec3::Z * pitch.cos() - horizontal * pitch.sin();
    let left = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
    (
        Vec3::from(u.pos),
        forward,
        left * bank.cos() + up * bank.sin(),
        up * bank.cos() - left * bank.sin(),
    )
}

/// How far a stun of `stun` (0 to 1) still throws arcs: all of it while it holds, dying
/// away over the reboot.
fn arc_strength(stun: f32) -> f32 {
    let x = (stun / 0.35).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Renderer {
    /// Once a tick: arcs, sparks and smoke about the stunned units near the eye, and every
    /// stroke written for the GPU.
    pub(super) fn emp_tick(
        &mut self,
        frame: &RenderFrame,
        units: &[UnitInstance],
        time: f32,
        camera: &Camera,
    ) {
        if time + 1.0 < self.emp_fx.last {
            // The clock went back (a restaged backdrop): nothing of the old world is left.
            self.emp_fx = EmpFx::default();
        }
        self.emp_fx.last = time;
        let focus = camera.focus.truncate();
        let hidden = KIND_WRECK | KIND_PROP | KIND_GHOST;
        let mut near: Vec<(f32, &UnitInstance)> = units
            .iter()
            .filter(|u| u.owner_flags & hidden == 0 && !u.in_warp() && u.stun(1.0) > 0.0)
            .map(|u| (Vec3::from(u.pos).truncate().distance(focus), u))
            .filter(|(d, _)| *d < NEAR)
            .collect();
        // Nearest first; a deliberate cap (cosmetic): past it, the shader's arcs carry it.
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.truncate(MAX_UNITS);
        let near: Vec<UnitInstance> = near.into_iter().map(|(_, u)| *u).collect();
        for u in &near {
            self.stun_arcs(u, time);
        }
        self.dampers_tick(frame, time, camera);
        self.write_emp_strokes(time);
    }

    /// One tick of arcs over a stunned hull.
    fn stun_arcs(&mut self, u: &UnitInstance, time: f32) {
        let bp = self
            .blueprints
            .unit(mc_data::BlueprintId(u.blueprint as u16));
        let capital = bp.is_capital_ship();
        let radius = bp.radius.to_f32().max(1.0);
        let height = bp.height.to_f32().max(1.0);
        let strength = arc_strength(u.stun(1.0));
        if strength <= 0.0 {
            return;
        }
        let f = hull_frame(u, capital);
        let half = if capital {
            (radius * 0.9, radius * 0.32)
        } else {
            (radius * 0.8, radius * 0.55)
        };
        let k = (radius / 40.0).clamp(0.25, 4.0);
        let dt = self.tick_seconds.max(0.02);
        let place = |p: Vec3| f.0 + f.1 * p.x + f.2 * p.y + f.3 * p.z;
        // A point on the hull's skin, near `x` along it: over the top or down a flank,
        // narrowing toward the ends.
        let skin = |s: &mut super::Scatter, x: f32| {
            let x = x.clamp(-half.0, half.0);
            let taper = 1.0 - 0.55 * (x / half.0).powi(2);
            if s.unit() < 0.6 {
                Vec3::new(
                    x,
                    s.signed() * half.1 * taper * 0.85,
                    height * (0.55 + 0.35 * s.unit()),
                )
            } else {
                let side = if s.unit() < 0.5 { -1.0 } else { 1.0 };
                Vec3::new(x, side * half.1 * taper, height * (0.2 + 0.5 * s.unit()))
            }
        };
        let width = (0.2 + 0.3 * k.sqrt()) * (0.6 + 0.4 * strength);
        // A deliberate cap (cosmetic): a dozen arcs a tick on the biggest hull.
        let arcs =
            (((1.5 + 3.0 * k) * strength * (0.6 + 0.8 * self.scatter.unit())) as usize).min(12);
        let mut brightest: Option<Vec3> = None;
        for _ in 0..arcs {
            let x = self.scatter.signed() * half.0;
            let a = skin(&mut self.scatter, x);
            let bx = x + self.scatter.signed() * half.0 * 0.3;
            let b = skin(&mut self.scatter, bx);
            let (pa, pb) = (place(a), place(b));
            let reach = pa.distance(pb);
            // Bowed off the skin, away from the hull's middle.
            let mid = (a + b) * 0.5;
            let out = Vec3::new(0.0, mid.y, mid.z - height * 0.4).normalize_or(Vec3::Z);
            let bow = place(mid + out * reach * (0.12 + 0.2 * self.scatter.unit()));
            let start = time + self.scatter.unit() * dt;
            let life = 0.06 + self.scatter.unit() * 0.1;
            let wander = reach * 0.1;
            self.emp_bolt(pa, bow, wander, f.2, f.3, start, life, width, BOLT);
            self.emp_bolt(bow, pb, wander, f.2, f.3, start, life, width, BOLT);
            for end in [pa, pb] {
                if self.scatter.unit() < 0.6 {
                    let vel = (f.2 * self.scatter.signed() + f.3 * self.scatter.unit())
                        * (5.0 + 8.0 * k.sqrt());
                    let kind = if self.scatter.unit() < 0.5 {
                        PUFF_SPARK
                    } else {
                        PUFF_BOLT
                    };
                    let spark = 0.35 * k.sqrt();
                    let life = 0.25 + self.scatter.unit() * 0.4;
                    self.push_puff(kind, end, vel, start, life, (spark, 0.08));
                }
            }
            brightest.get_or_insert(bow);
        }
        // Now and then an arc runs half the hull and forks.
        if self.scatter.unit() < 0.3 * strength * k.min(2.0) {
            let x0 = self.scatter.signed() * half.0 * 0.5;
            let a = place(skin(&mut self.scatter, x0 - half.0 * 0.45));
            let b = place(skin(&mut self.scatter, x0 + half.0 * 0.45));
            let start = time + self.scatter.unit() * dt;
            let wander = a.distance(b) * 0.06;
            let kinks = ((a.distance(b) / (8.0 * k)) as usize).clamp(6, 16);
            self.emp_kinks(
                a,
                b,
                wander,
                f.2,
                f.3,
                start,
                0.12,
                width * 1.4,
                kinks,
                BOLT,
            );
            for _ in 0..2 {
                let root = a.lerp(b, 0.2 + 0.6 * self.scatter.unit());
                let tip =
                    root + (f.2 * self.scatter.signed() + f.3 * self.scatter.unit()) * wander * 5.0;
                self.emp_kinks(
                    root,
                    tip,
                    wander * 0.6,
                    f.2,
                    f.3,
                    start,
                    0.09,
                    width * 0.7,
                    3,
                    BOLT,
                );
            }
            brightest = Some(a.lerp(b, 0.5));
        }
        if let Some(at) = brightest {
            self.emp_fx.flashes.push(Flash {
                pos: at,
                start: time + self.scatter.unit() * dt,
                life: 0.12,
                color: ARC_LIGHT * 2600.0 * k * k * strength,
                range: 40.0 * k,
                flicker: 1.0,
            });
        }
        // Scorched insulation smoking off a capital hull, thin and grey.
        if capital && self.scatter.unit() < 0.5 * strength {
            let x = self.scatter.signed() * half.0 * 0.7;
            let at = place(skin(&mut self.scatter, x));
            let breeze = (self.sky.wind_heading() * 3.0).extend(2.5);
            self.push_puff(PUFF_SMOKE, at, breeze, time, 4.5, (3.0 * k, 16.0 * k));
        }
    }

    /// A jagged stroke from `from` to `to`, wandering `wander` metres off the line.
    #[expect(
        clippy::too_many_arguments,
        reason = "a stroke's whole shape, as heavy_rail_fx lays it"
    )]
    pub(super) fn emp_bolt(
        &mut self,
        from: Vec3,
        to: Vec3,
        wander: f32,
        side: Vec3,
        up: Vec3,
        start: f32,
        life: f32,
        width: f32,
        color: u32,
    ) {
        self.emp_kinks(from, to, wander, side, up, start, life, width, 4, color);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a stroke's whole shape, as heavy_rail_fx lays it"
    )]
    pub(super) fn emp_kinks(
        &mut self,
        from: Vec3,
        to: Vec3,
        wander: f32,
        side: Vec3,
        up: Vec3,
        start: f32,
        life: f32,
        width: f32,
        kinks: usize,
        color: u32,
    ) {
        let mut last = from;
        for k in 1..=kinks {
            let t = k as f32 / kinks as f32;
            let taper = (t * std::f32::consts::PI).sin().sqrt();
            let jitter = if k == kinks {
                Vec3::ZERO
            } else {
                (side * self.scatter.signed() + up * self.scatter.signed()) * wander * taper
            };
            let next = from.lerp(to, t) + jitter;
            self.emp_fx.strokes.push(Stroke {
                from: last,
                to: next,
                start,
                life,
                width,
                color,
            });
            last = next;
        }
    }

    /// Every live stroke into this tick's projectile buffer.
    fn write_emp_strokes(&mut self, time: f32) {
        let fx = &mut self.emp_fx;
        fx.strokes.retain(|s| time < s.start + s.life);
        fx.flashes.retain(|f| time < f.start + f.life);
        if fx.strokes.len() > MAX_STROKES {
            // A deliberate cap (cosmetic): the oldest strokes go first.
            let extra = fx.strokes.len() - MAX_STROKES;
            fx.strokes.drain(..extra);
        }
        let size = size_of::<mc_sim::mirror::ProjectileInstance>();
        for s in &self.emp_fx.strokes {
            let i = self.projectile_count as usize;
            if i >= super::MAX_PROJECTILES {
                // A deliberate cap: the projectile buffer is full this tick.
                break;
            }
            let inst = mc_sim::mirror::ProjectileInstance {
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
            self.projectiles
                .write((i * size) as u64, bytemuck::bytes_of(&inst));
            self.projectile_count += 1;
        }
    }

    /// Every frame: the arcs' and the tethers' light.
    pub(super) fn emp_lights(&mut self, time: f32) {
        for f in &self.emp_fx.flashes {
            let age = (time - f.start) / f.life.max(0.01);
            if !(0.0..1.0).contains(&age) {
                continue;
            }
            let flicker = 1.0 - f.flicker * 0.4 * (1.0 - ((time - f.start) * 83.0).sin().abs());
            self.lights.lamp(
                f.pos,
                Vec3::Z,
                f.color * (1.0 - age) * flicker,
                f.range,
                180.0,
                1.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::arc_strength;

    #[test]
    fn arcs_hold_through_the_stun_and_die_away_as_it_wears_off() {
        assert_eq!(arc_strength(1.0), 1.0);
        assert_eq!(arc_strength(0.5), 1.0);
        assert!(arc_strength(0.2) > 0.0 && arc_strength(0.2) < 1.0);
        assert_eq!(arc_strength(0.0), 0.0);
    }
}
