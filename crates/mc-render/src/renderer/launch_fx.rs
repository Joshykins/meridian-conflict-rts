//! A hot launch out of a deck cell (`Weapon::vertical_launch` with no cold
//! ejection): the Swordfish's cruise missiles and the Manta's SAMs. The motor
//! lights in the cell, so the lid flashes, a jet of flame follows the missile out
//! along the cell, the exhaust the cell and its deflectors cannot take rolls out
//! across the deck in a ring of smoke, a pillar of smoke is left standing on the
//! launch line, and the pressure knocks a ring off the deck.

use super::{Renderer, PUFF_FIRE, PUFF_FIREBALL, PUFF_SMOKE, PUFF_SPARK};
use glam::Vec3;

impl Renderer {
    /// A missile leaving cell `at` along `dir`; `power` is the square root of its
    /// damage, which scales it (a SAM smaller than a cruise missile).
    pub(super) fn cell_launch(&mut self, at: Vec3, dir: Vec3, power: f32, time: f32) {
        let k = (power / 22.0).clamp(0.6, 1.5);
        self.push_effect(at.to_array(), time, 3.6 * k, 0.16, 1.0, 0.0);
        self.push_effect((at + dir * 2.0).to_array(), time, 2.0 * k, 0.1, 1.0, 0.5);
        // The flame chasing the missile out of the cell.
        for i in 0..6 {
            let f = i as f32;
            let jitter = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 2.0;
            self.push_puff(
                PUFF_FIRE,
                at + dir * (0.6 * f),
                dir * (22.0 + 9.0 * f) + jitter,
                time + 0.018 * f,
                0.16 + 0.03 * f,
                (0.55 * k, 1.5 * k),
            );
        }
        self.push_puff(
            PUFF_FIREBALL,
            at + dir * 0.5,
            dir * 5.0,
            time,
            0.32,
            (1.0 * k, 2.8 * k),
        );
        // The exhaust rolling out of the cell across the deck.
        let phase = self.scatter.unit() * std::f32::consts::TAU;
        for s in 0..10 {
            let a = phase + s as f32 * std::f32::consts::TAU / 10.0;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let speed = 8.0 + 6.0 * self.scatter.unit();
            let lift = 1.2 + self.scatter.unit() * 1.5;
            let start = time + 0.02 + 0.03 * self.scatter.unit();
            let life = 2.0 + self.scatter.unit();
            self.push_puff(
                PUFF_SMOKE,
                at + out * 0.7,
                out * speed * k + Vec3::Z * lift,
                start,
                life,
                (0.7 * k, 3.4 * k),
            );
        }
        // The smoke left standing on the launch line.
        for i in 0..6 {
            let f = i as f32;
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.3) * 1.5;
            self.push_puff(
                PUFF_SMOKE,
                at + dir * (1.5 + 3.2 * f),
                dir * (7.0 - 0.8 * f) + drift,
                time + 0.04 * f,
                2.6 + 0.35 * f,
                (0.9 * k, (3.6 + 0.4 * f) * k),
            );
        }
        for _ in 0..10 {
            let spray = (dir * 1.2
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.9)
                .normalize_or_zero();
            let speed = 26.0 + 40.0 * self.scatter.unit();
            self.push_puff(PUFF_SPARK, at, spray * speed, time, 0.5, (0.24, 0.08));
        }
        self.push_shockwave(at.to_array(), time, 14.0 * k, 0.4, 0.45, 1.0, dir);
    }
}
