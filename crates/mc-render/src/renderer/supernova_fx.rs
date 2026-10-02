//! A Regency power generator's star going supernova when its field is breached (`star_core_fx.rs`). It
//! does no harm (the sim has no blast for it: the Regency's trade for tough, packable
//! power), but it is the biggest light a plant makes. One explosion, all of it sized by
//! the star, the tech 3 crown's being the reference:
//!
//! - **Break**: the cage gives way (sparks and plates, no fireball: nothing in it burns)
//!   and the freed star flares, swelling and whitening for a moment.
//! - **Supernova**: straight out of the flare, a white flash and dust driven out across the
//!   ground, a hollow shell of plasma tearing outward from the star's face (plasma_puffs.wgsl
//!   `supernova`), a second inside it, streamers flung out round its waist, up from its
//!   pole and every way (`nova_wisp`), and a spray of sparks; the light floods the ground
//!   white, then lavender. No reds or oranges anywhere: it is the star's light, not fire.
//! - **Nebula**: what lingers is a slow, faint shell torn into violet strands and a small
//!   hot core, both fading.
//!
//! The break lasts `BREAK * scale` seconds, and the death sound (`regency_supernova`,
//! data/factions/regency/sounds.ron) is written for the same beat: tech 3 at size 1,
//! tech 1 and 2 `like` it at their scale. Change one and change the other.
//!
//! Presentation only; the renderer's own clock.

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

use super::star_core_fx::{ARC, GLARE, LAVENDER, ROSE, STAR, WHITE};
use super::stun_fx::Flash;
use super::{Renderer, PUFF_DUST, PUFF_SHARD, PUFF_SPARK};
use crate::gpu_consts::puff;

const SHELL: f32 = puff::SUPERNOVA as f32;
const WISP: f32 = puff::NOVA_WISP as f32;
/// Seconds the tech 3 star flares between its cage breaking and the supernova.
const BREAK: f32 = 0.3;
/// How long the flare outlasts the bang, as the shell takes its light over.
const HANDOVER: f32 = 0.4;
/// The farthest any shell gets (metres): the tech 3 crown's.
const REACH_MAX: f32 = 75.0;

/// A star going supernova.
#[derive(Clone, Copy)]
pub(super) struct Nova {
    centre: Vec3,
    r: f32,
    seed: f32,
    /// The star's size against the tech 3 crown's (`scale`).
    s: f32,
    start: f32,
    bang: f32,
    /// When the remnant has gone out.
    until: f32,
    /// What it shines with this tick: its face's radius and its brightness.
    now_r: f32,
    now_glow: f32,
}

/// A star of face radius `r` against the tech 3 crown's: 0.585 for the cradle, 0.69 for
/// the yoke, 1 for the crown (the sizes its sound is written at).
fn scale(r: f32) -> f32 {
    (0.5 + 0.053 * r).min(1.0)
}

impl Renderer {
    /// A unit standing at `at` died: if it held a burning star, its cage breaks and the
    /// star goes supernova. Whether it did.
    pub(super) fn star_nova(&mut self, at: Vec3, time: f32) -> bool {
        let Some(b) = self
            .star_core_fx
            .burning
            .iter()
            .find(|b| b.at.truncate().distance(at.truncate()) < 1.0)
            .copied()
        else {
            return false;
        };
        let s = scale(b.r);
        let bang = time + BREAK * s;
        let nova = Nova {
            centre: b.centre,
            r: b.r,
            seed: b.seed,
            s,
            start: time,
            bang,
            until: bang + 4.0 + 5.0 * s,
            now_r: b.r,
            now_glow: GLARE,
        };
        self.star_core_fx.novae.push(nova);
        self.cage_breaks(&nova, time);
        self.supernova(&nova);
        true
    }

    /// The cage giving way round the star: a white blink, sparks and plates thrown off.
    /// No smoke: dark puffs would hang in front of the light.
    fn cage_breaks(&mut self, nova: &Nova, time: f32) {
        let Nova { centre, r, s, .. } = *nova;
        self.push_effect(centre.to_array(), time, r * 3.0, 0.25, 9.0, 0.0);
        for _ in 0..(20.0 + 40.0 * s) as usize {
            let vel = self.scatter.upward(0.1) * (15.0 + 25.0 * self.scatter.unit()) * s.sqrt();
            let life = 0.8 + 1.0 * self.scatter.unit();
            let size = 0.4 + 0.4 * self.scatter.unit();
            self.push_puff(PUFF_SPARK, centre, vel, time, life, (size, 0.1));
        }
        for _ in 0..(6.0 + 14.0 * s) as usize {
            let dir = self.scatter.upward(0.2);
            let vel = dir * (6.0 + 10.0 * self.scatter.unit()) * s.sqrt();
            let life = 0.9 + 0.6 * self.scatter.unit();
            let plate = r * (0.25 + 0.2 * self.scatter.unit());
            self.push_puff(
                PUFF_SHARD,
                centre + dir * r * 1.4,
                vel,
                time,
                life,
                (plate, plate * 0.55),
            );
        }
    }

    /// Once a sim tick: the swelling stars and the remnants.
    pub(super) fn novae_tick(&mut self, time: f32) {
        self.star_core_fx.novae.retain(|n| time < n.until);
        let life = self.tick_seconds.clamp(0.03, 0.25) * 2.0;
        for i in 0..self.star_core_fx.novae.len() {
            let mut n = self.star_core_fx.novae[i];
            let (r, glow, lash) = if time < n.bang {
                // Freed: flaring, swelling and whitening, shuddering, lashing out.
                let u = (time - n.start) / (n.bang - n.start);
                let shudder = 1.0 + 0.06 * u * (time * 47.0 + n.seed).sin();
                let r = n.r * (1.0 + 0.6 * u * u) * shudder;
                (r, GLARE + 6.0 * u * u, 0.5 + 0.5 * u)
            } else {
                // The flare handing its light to the shell, and under it the hot core left
                // behind, fading with the nebula.
                let e = time - n.bang;
                let flare = (1.0 - e / HANDOVER).max(0.0);
                let fade = (1.0 - e / (n.until - n.bang)).max(0.0).powf(1.5);
                let r = n.r * (0.4 + 1.4 * flare);
                let glow = (GLARE + 6.0) * flare * flare + 2.4 * fade * (1.0 - flare);
                (r, glow, 0.15 * fade)
            };
            n.now_r = r;
            n.now_glow = glow;
            self.star_core_fx.novae[i] = n;
            if glow < 0.05 {
                continue;
            }
            let across = r / 0.42;
            self.push_lit(
                STAR,
                n.centre,
                Vec3::ZERO,
                time,
                life,
                (across, across),
                Vec3::splat(glow),
                n.seed,
            );
            // Lightning lashing off it, more and more as it swells.
            let mut lashes = lash * (1.0 + 3.0 * n.s);
            while lashes > 0.0 {
                if self.scatter.unit() < lashes.min(1.0) {
                    self.nova_arc(n.centre, r, time);
                }
                lashes -= 1.0;
            }
        }
    }

    fn nova_arc(&mut self, centre: Vec3, r: f32, time: f32) {
        let out = Vec3::new(
            self.scatter.signed(),
            self.scatter.signed(),
            self.scatter.signed() * 0.7,
        )
        .normalize_or(Vec3::Z);
        let tint = WHITE.lerp(LAVENDER, self.scatter.unit());
        let length = r * (0.9 + 1.4 * self.scatter.unit());
        let lasts = 0.07 + 0.06 * self.scatter.unit();
        self.push_lit(
            ARC,
            centre + out * r * 0.9,
            out * length,
            time,
            lasts,
            (r * 0.16, r * 0.16),
            tint * 4.0,
            0.0,
        );
    }

    /// Every frame: the swelling star and the remnant light the ground round them.
    pub(super) fn novae_lights(&mut self, time: f32) {
        for n in &self.star_core_fx.novae {
            if time >= n.bang && time < n.bang + 0.4 {
                // The flash carries the light of the bang itself.
                continue;
            }
            let colour = if time < n.bang {
                WHITE.lerp(ROSE, 0.5)
            } else {
                LAVENDER
            };
            self.lights.lamp(
                n.centre,
                Vec3::NEG_Z,
                colour * (6.0 * n.now_r * n.now_glow),
                n.now_r * 8.0 + 10.0,
                180.0,
                1.0,
            );
        }
    }

    /// One shell of plasma `radius` metres out from `centre` (from, to), as its two halves
    /// (plasma_puffs.wgsl `supernova_vertex`: `vel.x` 1 is the far half).
    fn nova_shell(
        &mut self,
        centre: Vec3,
        start: f32,
        life: f32,
        radius: (f32, f32),
        glow: f32,
        seed: f32,
    ) {
        for half in [Vec3::ZERO, Vec3::X] {
            self.push_lit(
                SHELL,
                centre,
                half,
                start,
                life,
                radius,
                Vec3::splat(glow),
                seed,
            );
        }
    }

    /// The supernova itself, laid at once to go off at `nova.bang`.
    fn supernova(&mut self, nova: &Nova) {
        let Nova {
            centre,
            r,
            seed,
            s,
            bang,
            ..
        } = *nova;
        let ground_xy = centre.truncate();
        let ground = ground_xy.extend(self.ground_height(ground_xy));
        // How far the shell gets: the crown's held to 75 m, so it stays a plant's death and does
        // not swallow the base round it.
        let reach = (r * (6.0 + 8.0 * s)).min(REACH_MAX);
        // The flare's size at the bang: the shell starts at the star's face.
        let face = r * 1.6;
        // The flash: white beyond the screen, then a broader, slower one.
        self.push_effect(centre.to_array(), bang, r * 5.0, 0.35, 9.0, 0.0);
        self.push_effect(centre.to_array(), bang + 0.04, r * 3.5, 0.7, 9.0, 0.0);
        // How fast the shock drives the dust out across the ground.
        let shock = 40.0 + 200.0 * s * s;
        // The light of it on everything round: white, then lavender as the shell spreads.
        self.emp_fx.flashes.push(Flash {
            pos: centre,
            start: bang,
            life: 0.9 + 0.9 * s,
            color: WHITE * (40.0 + 140.0 * s),
            range: (reach * 2.0).min(900.0),
            flicker: 0.2,
        });
        self.emp_fx.flashes.push(Flash {
            pos: centre,
            start: bang + 0.2,
            life: 1.5 + 2.0 * s,
            color: LAVENDER * (12.0 + 40.0 * s),
            range: (reach * 1.5).min(900.0),
            flicker: 0.3,
        });
        // The shell, a slower one inside it, and the nebula: a faint one that lingers.
        let life = 2.8 + 3.6 * s;
        self.nova_shell(centre, bang, life, (face, reach), 1.6, seed);
        self.nova_shell(
            centre,
            bang + 0.1,
            life * 1.3,
            (face * 0.8, reach * 0.62),
            1.0,
            seed + 3.7,
        );
        self.nova_shell(
            centre,
            bang + 0.3,
            life * 2.0,
            (face, reach * 0.85),
            0.6,
            seed + 6.1,
        );
        // Streamers thrown out round its waist, the ring tilted a little its own way.
        let tilt = 0.45 * self.scatter.signed();
        let (ts, tc) = tilt.sin_cos();
        let yaw = self.scatter.unit() * TAU;
        // A loose band, not a flat disc: each thrown a little off it, none past the shell.
        let around = (20.0 + 36.0 * s) as usize;
        for i in 0..around {
            let a = (i as f32 + 0.5 * self.scatter.unit()) * TAU / around as f32;
            let flat = Vec2::from_angle(a + yaw);
            let off = 0.35 * self.scatter.signed();
            let dir = Vec3::new(flat.x, flat.y * tc, flat.y * ts + off).normalize();
            let go = reach * (0.5 + 0.3 * self.scatter.unit());
            let size = r * (0.5 + 0.4 * self.scatter.unit());
            let (roll0, roll1) = (self.scatter.unit(), self.scatter.unit());
            self.push_lit(
                WISP,
                centre + dir * face,
                dir * go * puff::NOVA_WISP_DRAG,
                bang + 0.03 * roll0,
                2.4 + 2.0 * s + 0.6 * roll1,
                (size, size * 3.2),
                Vec3::splat(1.3),
                0.0,
            );
        }
        // A jet up from its pole: a column laid over a third of a second, the first highest.
        let jet = (14.0 + 20.0 * s) as usize;
        for i in 0..jet {
            let k = i as f32 / jet as f32;
            let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.06;
            let dir = (Vec3::Z + lean).normalize();
            let go = reach * (1.0 - 0.4 * k) * (0.9 + 0.2 * self.scatter.unit());
            let size = r * (0.45 + 0.3 * self.scatter.unit());
            self.push_lit(
                WISP,
                centre + dir * face,
                dir * go * puff::NOVA_WISP_DRAG,
                bang + 0.35 * k,
                2.0 + 1.6 * s,
                (size, size * 2.6),
                Vec3::splat(2.4),
                0.0,
            );
        }
        // Streamers flung every way, thinner and quicker to go out.
        for _ in 0..(20.0 + 40.0 * s) as usize {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.8 + 0.3,
            )
            .normalize_or(Vec3::Z);
            let go = reach * (0.4 + 0.6 * self.scatter.unit());
            let size = r * (0.22 + 0.16 * self.scatter.unit());
            self.push_lit(
                WISP,
                centre + dir * face * 0.8,
                dir * go * puff::NOVA_WISP_DRAG,
                bang,
                1.4 + 1.2 * s,
                (size, size * 2.4),
                Vec3::splat(2.8),
                0.0,
            );
        }
        // Sparks sprayed out of the bang.
        for _ in 0..(60.0 + 120.0 * s) as usize {
            let vel = self.scatter.upward(0.0) * (30.0 + 60.0 * self.scatter.unit()) * s.sqrt();
            let life = 1.2 + 1.2 * self.scatter.unit();
            let size = 0.5 + 0.5 * self.scatter.unit();
            self.push_puff(PUFF_SPARK, centre, vel, bang, life, (size, 0.1));
        }
        // Dust driven out flat across the ground ahead of the shock.
        let dust = (16.0 + 28.0 * s) as usize;
        for i in 0..dust {
            let a = (i as f32 + self.scatter.unit()) * TAU / dust as f32;
            let out = Vec3::new(a.cos(), a.sin(), 0.04);
            let from = r * 2.0;
            let size = r * (0.4 + 0.3 * self.scatter.unit());
            let push = out * shock * (0.35 + 0.15 * self.scatter.unit());
            self.push_puff(
                PUFF_DUST,
                ground + out * from + Vec3::Z * 0.6,
                push,
                bang + 0.08,
                1.4 + 1.0 * s,
                (size, size * 2.5),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::scale;

    /// Each Regency star's death sound (the power generators', the Wards') is written at its
    /// star's scale, so the bang in the sound lands when the star goes.
    #[test]
    fn each_power_generator_sounds_at_its_stars_scale() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let blueprints = mc_data::Blueprints::load(&root.join("data")).unwrap();
        let sounds = mc_data::sounds::SoundLibrary::load(&root.join("data")).unwrap();
        let mut seen = 0;
        for u in &blueprints.units {
            let model = crate::models::build_model_scaled(
                &u.visual.mesh,
                u.radius.to_f32(),
                u.height.to_f32(),
                u.tech,
            );
            let Some(star) = model.and_then(|m| m.star_core) else {
                continue;
            };
            seen += 1;
            let name = u.sounds.death.as_deref().expect(&u.key);
            let sound = &sounds.sounds[sounds.id_of(name).unwrap().0 as usize];
            let size = sound.like.as_ref().map_or(1.0, |(_, size)| *size);
            assert!(
                (size - scale(star[3])).abs() < 0.01,
                "{}: {name} at size {size}, its star at {}",
                u.key,
                scale(star[3])
            );
        }
        // Three power generators and the two Wards.
        assert_eq!(seen, 5);
    }
}
