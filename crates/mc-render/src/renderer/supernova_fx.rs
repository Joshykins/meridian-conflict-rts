//! A Regency power generator's star going supernova when its field is breached (`star_core_fx.rs`). It
//! does no harm (the sim has no blast for it: the Regency's trade for tough, packable
//! power), but it is the biggest light a plant makes. In three acts, all of it sized by
//! the star, the tech 3 crown's being the reference:
//!
//! - **Swell**: the cage breaks (sparks and plates, no fireball: nothing in it burns),
//!   and freed of it the star swells to several times its size, shuddering,
//!   brightening, lightning lashing off it ever more often.
//! - **Collapse**: in the last moment it falls in on itself to a white point.
//! - **Supernova**: a white flash and dust driven out across the ground, then a shell of
//!   plasma tearing outward in the prism's pinks (plasma_puffs.wgsl `supernova`), a
//!   second slower one inside it, a bright ring thrown out round its waist, a jet up
//!   from its pole and globs of plasma flung off; the light floods the ground rose
//!   and white. What is left is a small pulsing remnant that beats and fades.
//!
//! The swell lasts `SWELL * scale` seconds, and the death sound (`regency_supernova`,
//! data/factions/regency/sounds.ron) is written for the same beat: tech 3 at size 1,
//! tech 1 and 2 `like` it at their scale. Change one and change the other.
//!
//! Presentation only; the renderer's own clock.

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

use super::star_core_fx::{ARC, GLARE, LAVENDER, ROSE, STAR, WHITE};
use super::stun_fx::Flash;
use super::{Renderer, PUFF_DUST, PUFF_SHARD, PUFF_SMOKE, PUFF_SPARK};
use crate::gpu_consts::puff;

const SHELL: f32 = puff::SUPERNOVA as f32;
const WAKE: f32 = puff::PLASMA_WAKE as f32;
const GLOB: f32 = puff::PLASMA_GLOB as f32;
/// Seconds the tech 3 star swells before it goes.
const SWELL: f32 = 2.2;
/// The share of the swell it spends growing; the rest it falls in.
const GROWING: f32 = 0.85;

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
        let bang = time + SWELL * s;
        let nova = Nova {
            centre: b.centre,
            r: b.r,
            seed: b.seed,
            s,
            start: time,
            bang,
            until: bang + 3.0 + 5.0 * s,
            now_r: b.r,
            now_glow: GLARE,
        };
        self.star_core_fx.novae.push(nova);
        self.cage_breaks(at, &nova, time);
        self.supernova(&nova);
        true
    }

    /// The cage giving way round the star: a white blink, sparks and plates thrown off,
    /// a little smoke from its footing.
    fn cage_breaks(&mut self, at: Vec3, nova: &Nova, time: f32) {
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
        for _ in 0..(4.0 + 6.0 * s) as usize {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 1.5;
            let vel = Vec3::Z * (2.0 + 2.0 * self.scatter.unit());
            let life = 2.5 + 1.5 * self.scatter.unit();
            self.push_puff(
                PUFF_SMOKE,
                at + off + Vec3::Z * 1.0,
                vel,
                time,
                life,
                (r * 0.6, r * 2.0),
            );
        }
    }

    /// Once a sim tick: the swelling stars and the remnants.
    pub(super) fn novae_tick(&mut self, time: f32) {
        self.star_core_fx.novae.retain(|n| time < n.until);
        let life = self.tick_seconds.clamp(0.03, 0.25) * 2.0;
        for i in 0..self.star_core_fx.novae.len() {
            let mut n = self.star_core_fx.novae[i];
            let grown = n.r * (2.0 + 2.2 * n.s);
            let (r, glow, lash) = if time < n.bang {
                let u = (time - n.start) / (n.bang - n.start);
                if u < GROWING {
                    // Swelling, shuddering harder as it goes.
                    let e = u / GROWING;
                    let shudder = 1.0 + 0.05 * e * (time * 41.0 + n.seed).sin();
                    let r = (n.r + (grown - n.r) * e.powf(1.6)) * shudder;
                    (r, GLARE + 3.0 * e * e, 0.25 + 0.75 * e)
                } else {
                    // Falling in on itself, whiter as it goes.
                    let c = (u - GROWING) / (1.0 - GROWING);
                    let r = n.r * 0.2 + (grown - n.r * 0.2) * (1.0 - c) * (1.0 - c);
                    (r, GLARE + 3.0 + 6.0 * c, 0.0)
                }
            } else {
                // The remnant: a small hot core beating fast, fading.
                let k = (time - n.bang) / (n.until - n.bang);
                // Beating 2.4 times a second at the crown's size from the bang, as its sound does.
                let pulse = (-((time - n.bang) * 2.4 / n.s).fract() * 7.0).exp();
                let fade = (1.0 - k).powf(1.5);
                let r = n.r * 0.3 * (1.0 + 0.5 * pulse);
                (r, (2.0 + 5.0 * pulse) * fade, 0.12 * fade)
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
            let colour = if time < n.bang { ROSE } else { LAVENDER };
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
        // How far the shell gets: the crown's about 130 m.
        let reach = r * (6.0 + 8.0 * s);
        // The flash: white beyond the screen, then a broader, slower one.
        self.push_effect(centre.to_array(), bang, r * 10.0, 0.4, 9.0, 0.0);
        self.push_effect(centre.to_array(), bang + 0.04, r * 6.0, 0.8, 9.0, 0.0);
        // How fast the shock drives the dust out across the ground.
        let shock = 40.0 + 200.0 * s * s;
        // The light of it on everything round: white, then rose as the shell spreads.
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
            color: ROSE * (15.0 + 50.0 * s),
            range: (reach * 1.5).min(900.0),
            flicker: 0.3,
        });
        // The shell, and a slower one inside it.
        let life = 2.4 + 3.6 * s;
        self.push_lit(
            SHELL,
            centre,
            Vec3::ZERO,
            bang,
            life,
            (r * 0.8, reach / 0.78),
            Vec3::splat(3.2),
            seed,
        );
        self.push_lit(
            SHELL,
            centre,
            Vec3::ZERO,
            bang + 0.12,
            life * 1.3,
            (r * 0.5, reach * 0.62 / 0.78),
            Vec3::splat(2.2),
            seed + 3.7,
        );
        // The ring thrown out round its waist, tilted a little its own way.
        let tilt = 0.45 * self.scatter.signed();
        let (ts, tc) = tilt.sin_cos();
        let yaw = self.scatter.unit() * TAU;
        let around = (40.0 + 100.0 * s) as usize;
        for i in 0..around {
            let a = (i as f32 + 0.5 * self.scatter.unit()) * TAU / around as f32;
            let flat = Vec2::from_angle(a + yaw);
            let dir = Vec3::new(flat.x, flat.y * tc, flat.y * ts);
            // A wake drifts `vel / 1.6` in all (plasma_puffs.wgsl).
            let speed = reach * (0.8 + 0.2 * self.scatter.unit()) * 1.6;
            let puff = r * (0.6 + 0.3 * self.scatter.unit());
            let tint = WHITE.lerp(ROSE, 0.3 * self.scatter.unit());
            let (roll0, roll1) = (self.scatter.unit(), self.scatter.unit());
            self.push_lit(
                WAKE,
                centre + dir * r,
                dir * speed,
                bang + 0.03 * roll0,
                1.8 + 1.6 * s + 0.6 * roll1,
                (puff, puff * 3.5),
                tint * 3.0,
                1.0,
            );
        }
        // A jet up from its pole: a column laid over half a second, the first highest.
        let jet = (24.0 + 30.0 * s) as usize;
        for i in 0..jet {
            let k = i as f32 / jet as f32;
            let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.06;
            let dir = (Vec3::Z + lean).normalize();
            let speed = reach * (1.1 + 0.2 * self.scatter.unit()) * 1.6;
            let puff = r * (0.5 + 0.3 * self.scatter.unit());
            self.push_lit(
                WAKE,
                centre + dir * r,
                dir * speed,
                bang + 0.05 + 0.5 * k,
                1.6 + 1.4 * s,
                (puff, puff * 3.0),
                WHITE.lerp(LAVENDER, k) * 2.6,
                1.0,
            );
        }
        // Globs of the star flung off every way.
        for _ in 0..(16.0 + 34.0 * s) as usize {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.8 + 0.2,
            )
            .normalize_or(Vec3::Z);
            // A glob travels `vel / 3` before the air stops it.
            let speed = reach * (0.5 + 0.6 * self.scatter.unit()) * 3.0;
            let blob = r * (0.18 + 0.14 * self.scatter.unit());
            let tint = WHITE.lerp(ROSE, self.scatter.unit());
            self.push_lit(
                GLOB,
                centre + dir * r * 0.5,
                dir * speed,
                bang,
                1.0 + 0.8 * s,
                (blob, blob * 0.3),
                tint * 4.0,
                0.0,
            );
        }
        // Dust driven out flat across the ground ahead of the shock.
        let dust = (16.0 + 28.0 * s) as usize;
        for i in 0..dust {
            let a = (i as f32 + self.scatter.unit()) * TAU / dust as f32;
            let out = Vec3::new(a.cos(), a.sin(), 0.04);
            let from = r * 2.0;
            let size = r * (0.8 + 0.6 * self.scatter.unit());
            let push = out * shock * (0.5 + 0.2 * self.scatter.unit());
            self.push_puff(
                PUFF_DUST,
                ground + out * from + Vec3::Z * 0.6,
                push,
                bang + 0.08,
                1.8 + 1.4 * s,
                (size, size * 4.0),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::scale;

    /// Each Regency power generator's death sound is written at its star's scale, so the bang in the
    /// sound lands when the star goes.
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
        assert_eq!(seen, 3);
    }
}
