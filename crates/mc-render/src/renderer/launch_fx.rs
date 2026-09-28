//! A hot launch out of a deck cell (`Weapon::vertical_launch` with no cold
//! ejection): the Swordfish's cruise missiles and the Manta's SAMs. The motor
//! lights in the cell, so the lid flashes, a jet of flame follows the missile out
//! along the cell, the exhaust the cell and its deflectors cannot take rolls out
//! across the deck in a ring of smoke, a pillar of smoke is left standing on the
//! launch line, and the pressure knocks a ring off the deck.
//!
//! A boosted SAM (`Weapon::boost_ticks`) leaves its cell the same way; its booster
//! spent, it coasts while its thrusters swing it over (`cell_thrusters`): short white
//! jets off the nose and tail, pushing it round, until the motor lights.
//!
//! A missile or rocket fired from a tube or rail (`tube_launch`): a flash at the
//! mouth, flame chasing it out, the backblast thrown out behind the tube, and
//! smoke that billows round the launcher and hangs there.

use super::{Renderer, PUFF_FIRE, PUFF_FIREBALL, PUFF_SMOKE, PUFF_SPARK};
use glam::Vec3;
use mc_sim::mirror::ProjectileInstance;

impl Renderer {
    /// A missile leaving cell `at` along `dir`; `power` is the square root of its
    /// damage, which scales it (a SAM smaller than a cruise missile).
    pub(super) fn cell_launch(&mut self, at: Vec3, dir: Vec3, power: f32, time: f32) {
        let k = (power / 15.0).clamp(0.8, 2.0);
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
            let life = 2.6 + self.scatter.unit() * 1.2;
            self.push_puff(
                PUFF_SMOKE,
                at + out * 0.7,
                out * speed * k + Vec3::Z * lift,
                start,
                life,
                (0.9 * k, 4.6 * k),
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
                (1.1 * k, (4.8 + 0.5 * f) * k),
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

    /// A missile leaving a tube or rail at `at` along `dir`, its motor lighting as it
    /// goes; `power` is the square root of its damage, `width` the body across in metres
    /// (`Weapon::caliber`, zero for a small one).
    pub(super) fn tube_launch(&mut self, at: Vec3, dir: Vec3, power: f32, width: f32, time: f32) {
        let k = (power / 12.0).clamp(0.6, 1.6) * (width / 0.35).clamp(1.0, 2.2).sqrt();
        self.push_effect(at.to_array(), time, 2.6 * k, 0.12, 1.0, 0.0);
        // The flame chasing the missile out of the tube.
        for i in 0..4 {
            let f = i as f32;
            self.push_puff(
                PUFF_FIRE,
                at + dir * (0.5 * f),
                dir * (16.0 + 8.0 * f),
                time + 0.015 * f,
                0.12 + 0.03 * f,
                (0.4 * k, 1.1 * k),
            );
        }
        self.push_puff(
            PUFF_FIREBALL,
            at - dir * 0.6,
            -dir * 6.0,
            time,
            0.26,
            (0.7 * k, 2.0 * k),
        );
        // The backblast: flame and smoke thrown out behind the tube, fanning as it slows.
        for i in 0..2 {
            self.push_puff(
                PUFF_FIRE,
                at - dir * (1.0 + i as f32),
                -dir * 18.0,
                time,
                0.14,
                (0.5 * k, 1.4 * k),
            );
        }
        for i in 0..6 {
            let fan = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit() * 0.6,
            );
            let speed = 9.0 + 6.0 * self.scatter.unit();
            let life = 2.2 + self.scatter.unit() * 1.2;
            self.push_puff(
                PUFF_SMOKE,
                at - dir * (1.5 + 0.6 * i as f32),
                (-dir + fan * 0.55) * speed * k,
                time + 0.02 * i as f32,
                life,
                (0.8 * k, 4.2 * k),
            );
        }
        // Smoke left hanging where the motor lit, drifting after the missile a little.
        for i in 0..3 {
            let f = i as f32;
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.4);
            self.push_puff(
                PUFF_SMOKE,
                at + dir * (1.0 + 2.5 * f),
                dir * (5.0 - f) + drift,
                time + 0.03 * f,
                1.8 + 0.3 * f,
                (0.7 * k, (3.2 + 0.4 * f) * k),
            );
        }
        for _ in 0..6 {
            let spray = (dir * 1.3
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.8)
                .normalize_or_zero();
            let speed = 20.0 + 30.0 * self.scatter.unit();
            self.push_puff(PUFF_SPARK, at, spray * speed, time, 0.4, (0.2, 0.07));
        }
    }

    /// A boosted missile coasting between its booster and its motor (a cold body with
    /// `_pad[1]` below zero, `mirror::wings_or_turn`): the booster's last flame and smoke
    /// as it cuts out, then the thrusters' jets over this tick. The nose thrusters fire
    /// against the way the nose is swinging and the tail's with it, so the jets push it
    /// round; a missile already pointing its way trims with small jets either side.
    pub(super) fn cell_thrusters(&mut self, p: &ProjectileInstance, time: f32) {
        let turn = -p._pad[1];
        let (from, to) = (Vec3::from(p.prev_pos), Vec3::from(p.pos));
        let nose = Vec3::from_slice(&p.aim[..3]).normalize_or(Vec3::Z);
        let was = Vec3::from_slice(&p.prev_aim[..3]).normalize_or(nose);
        let half = if p.aim[3] > 0.0 { p.aim[3] / 0.28 } else { 2.0 };
        let width = p.aim[3].max(0.4);
        if turn <= 0.12 {
            // The booster cuts out: a last gout of flame and a knot of smoke at the tail.
            let tail = from - was * half;
            self.push_puff(PUFF_FIRE, tail, -was * 12.0, time, 0.18, (0.5, 1.4));
            for i in 0..3 {
                let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0);
                self.push_puff(
                    PUFF_SMOKE,
                    tail - was * i as f32,
                    -was * 4.0 + drift,
                    time + 0.02 * i as f32,
                    1.6,
                    (0.8, 2.6),
                );
            }
        }
        // Across the body, the way the nose is swinging this tick.
        let swing = nose - was;
        let side = (swing - nose * swing.dot(nose)).normalize_or_zero();
        let side = if side == Vec3::ZERO {
            let across = nose.cross(Vec3::X).normalize_or(Vec3::Y);
            if self.scatter.unit() < 0.5 {
                across
            } else {
                -across
            }
        } else {
            side
        };
        let pulses = 3;
        let step = self.tick_seconds / pulses as f32;
        for k in 0..pulses {
            let f = (k as f32 + 0.5) / pulses as f32;
            let at = from.lerp(to, f);
            let dir = was.lerp(nose, f).normalize_or(nose);
            let start = time + step * k as f32;
            let jitter = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 0.15;
            // Nose jets out against the swing, tail jets with it.
            for (along, out) in [(0.7, -side), (-0.8, side)] {
                let port = at + dir * (half * along) + out * width * 0.5;
                self.push_puff(
                    PUFF_SMOKE,
                    port,
                    (out + jitter) * 22.0,
                    start,
                    0.45,
                    (0.18, 1.1),
                );
            }
            self.push_puff(
                PUFF_SPARK,
                at + dir * (half * 0.7) - side * width * 0.5,
                -side * 14.0,
                start,
                0.08,
                (0.14, 0.05),
            );
        }
    }
}
