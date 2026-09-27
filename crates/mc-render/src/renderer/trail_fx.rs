//! Missile smoke, white conventional wakes and blue energy-slug trails: the
//! puffs laid along a shot's path, a heavy rocket's motor flame and the plume
//! it kicks out as it lights, and the sea skimmer's and high-arc missile's
//! extras.

use super::*;

impl Renderer {
    /// Missile smoke, white conventional wakes, and blue energy-slug trails.
    /// Paths are recorded even when the camera is too far to draw them, so
    /// zooming in lights the trail already flown, not a fresh ribbon from here.
    pub(super) fn missile_trails(
        &mut self,
        projectiles: &[ProjectileInstance],
        time: f32,
        camera: &Camera,
    ) {
        let close = camera.distance <= 2200.0;
        let reach = camera.distance * 2.8 + 360.0;
        let focus = camera.focus.truncate();
        let n = if camera.distance > 900.0 { 4 } else { 6 };
        let mut was = std::mem::take(&mut self.trail_paths);
        let mut now = HashMap::with_capacity(was.len());
        for p in projectiles {
            if p.color & (PROJECTILE_BEAM | PROJECTILE_FADE_BEAM) != 0 {
                continue;
            }
            if p.color & PROJECTILE_COLD != 0 {
                continue;
            }
            let missile = p.color & PROJECTILE_MISSILE != 0;
            let arc = p.color & PROJECTILE_TRAIL != 0;
            let smoke = p.color & PROJECTILE_SMOKE != 0;
            if !missile && !arc && !smoke {
                continue;
            }
            let (from, to) = (Vec3::from(p.prev_pos), Vec3::from(p.pos));
            let ends = ((p.color >> PROJECTILE_ENDS_SHIFT) & 0xFF) as f32 / 255.0;
            let duration = if ends > 0.0 {
                ends * self.tick_seconds
            } else {
                self.tick_seconds
            };
            let mut path = was.remove(&trail_key(from)).unwrap_or_else(|| TrailPath {
                points: vec![(from, time)],
                spawned: 0,
            });
            path.points.push((to, time + duration));
            // Keep only what the ribbon still shows: older puffs have died.
            let keep = if arc || smoke {
                (p.wake + 0.55).max(1.45)
            } else {
                4.2
            };
            while path.points.len() >= 2 && time - path.points[1].1 > keep {
                path.points.remove(0);
                path.spawned = path.spawned.saturating_sub(1);
            }
            // A heavy missile's column (one with a `wake`) is seen from any height: laid only
            // while the camera was close, it went missing zoomed out and came back broken.
            let heavy = missile && p.wake > 0.0;
            let in_view = (close || heavy) && to.truncate().distance(focus) <= reach;
            let skim = missile && p.color & PROJECTILE_SKIM != 0;
            let apogee = missile && p.color & PROJECTILE_APOGEE != 0;
            // A high-arc missile burns all the way down its arc, trail and all; coming
            // down, the nose heats as well (`arc_missile_flight`).
            let falling = apogee && to.z < from.z;
            if in_view && skim {
                self.skimmer_over_sea(from, to, time, duration, p);
            }
            if in_view && apogee {
                self.arc_missile_flight(from, to, time, duration, p, falling);
            }
            if in_view {
                let fresh = p.color & PROJECTILE_FRESH != 0;
                let puffs = if fresh && path.points.len() <= 2 {
                    6
                } else {
                    n
                };
                let first = if path.spawned == 0 {
                    0
                } else {
                    path.spawned - 1
                };
                let last = path.points.len().saturating_sub(1);
                for i in first..last {
                    let (a, t_a) = path.points[i];
                    let (b, t_b) = path.points[i + 1];
                    let (seg_time, seg_dur) = if i + 1 == last {
                        (time, duration)
                    } else {
                        (t_a, (t_b - t_a).max(0.001))
                    };
                    if seg_time + keep < time {
                        continue;
                    }
                    let engine = missile && i + 1 == last;
                    self.emit_trail_segment(a, b, seg_time, seg_dur, puffs, arc, engine, p);
                }
                if heavy && !skim && !apogee && fresh && path.points.len() <= 2 {
                    self.rocket_launch_plume(from, to, time, p);
                }
                path.spawned = path.points.len();
                if arc && p.plasma > 0.0 {
                    let dir = (to - from).normalize_or_zero();
                    self.emit_plasma_head(to, dir, time + duration, p.plasma);
                }
            }
            now.insert(trail_key(to), path);
        }
        self.trail_paths = now;
    }

    /// A sea skimmer (`PROJECTILE_SKIM`) running in low: within a few metres of the sea,
    /// its exhaust tears a line of spray off the water under it.
    fn skimmer_over_sea(
        &mut self,
        from: Vec3,
        to: Vec3,
        time: f32,
        duration: f32,
        p: &ProjectileInstance,
    ) {
        let Some(water) = self.at_sea(to, 6.0) else {
            return;
        };
        let dir = (to - from).normalize_or_zero();
        let side = Vec3::new(-dir.y, dir.x, 0.0);
        let low = (1.0 - (to.z - water) / 6.0).clamp(0.2, 1.0);
        let s = (0.5 + p.size * 0.3) * low;
        for i in 0..3 {
            let along = (i as f32 + 0.5) / 3.0;
            let at = from.lerp(to, along);
            let at = Vec3::new(at.x, at.y, water + 0.25);
            let start = time + along * duration;
            self.push_puff(
                water_fx::PUFF_SPRAY,
                at,
                dir * 3.0 + Vec3::Z * 0.8,
                start,
                0.55,
                (s, s * 2.6),
            );
            for sign in [-1.0f32, 1.0] {
                let vel = side * sign * (3.0 + self.scatter.unit() * 3.0)
                    + dir * 4.0
                    + Vec3::Z * (1.5 + self.scatter.unit() * 2.0) * low;
                self.push_puff(
                    water_fx::PUFF_DROPLET,
                    at + side * sign * 0.6,
                    vel,
                    start,
                    1.2,
                    (0.14, 0.32),
                );
            }
        }
    }

    /// A high-arc missile (`PROJECTILE_APOGEE`): a long white-hot exhaust behind it the
    /// whole flight (the trail's column is `emit_trail_segment`'s); coming down, the air
    /// it falls through heats the nose too, brighter the lower it gets, streaking fire
    /// back off it.
    fn arc_missile_flight(
        &mut self,
        from: Vec3,
        to: Vec3,
        time: f32,
        duration: f32,
        p: &ProjectileInstance,
        falling: bool,
    ) {
        let dir = (to - from).normalize_or_zero();
        let half = missile_half_length(p.size, p.aim[3]);
        let when = time + duration;
        // Fire is additive and whites out when it stacks, so the flame is small and short
        // and the body of the exhaust is the fireball behind it.
        let tail = to - dir * half;
        self.push_puff(
            PUFF_FIRE,
            tail,
            -dir * 22.0,
            when,
            0.18,
            (0.5 + p.size * 0.12, 1.1 + p.size * 0.25),
        );
        self.push_puff(
            PUFF_FIREBALL,
            tail - dir * 2.0,
            -dir * 9.0,
            when,
            0.3,
            (0.8 + p.size * 0.2, 2.0 + p.size * 0.4),
        );
        self.push_effect(
            (tail - dir).to_array(),
            when,
            1.6 + p.size * 0.5,
            0.25,
            1.0,
            0.0,
        );
        if !falling {
            return;
        }
        let floor = self.ground_height(to.truncate()).max(self.sea_level());
        let heat = (1.0 - (to.z - floor) / 700.0).clamp(0.15, 1.0);
        let nose = to + dir * half * 0.8;
        self.push_effect(
            nose.to_array(),
            when,
            (1.0 + p.size * 0.5) * (0.5 + 1.5 * heat),
            0.3,
            6.0,
            0.0,
        );
        for i in 0..3 {
            let back = (i as f32 + 0.5) * (1.5 + heat * 2.5);
            let off = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 0.3;
            let s = 0.4 + heat * 0.7;
            self.push_puff(
                PUFF_FIRE,
                nose - dir * back + off,
                -dir * 12.0 * heat,
                when,
                0.22,
                (s, s * (2.0 + heat * 1.5)),
            );
        }
    }

    fn emit_trail_segment(
        &mut self,
        from: Vec3,
        to: Vec3,
        time: f32,
        duration: f32,
        n: u32,
        arc: bool,
        engine: bool,
        p: &ProjectileInstance,
    ) {
        let dir = (to - from).normalize_or_zero();
        let step = (to - from).length();
        let tail = if p.color & PROJECTILE_MISSILE != 0 {
            missile_half_length(p.size, p.aim[3])
        } else {
            0.12
        };
        // A heavy missile trail (one with a `wake`) is a solid column, not a dotted line.
        let heavy = p.color & PROJECTILE_MISSILE != 0 && p.wake > 0.0;
        let skim = p.color & PROJECTILE_SKIM != 0;
        let n = if heavy { n.max(1) * 3 } else { n.max(1) };
        for i in 0..n {
            let along = (i as f32 + 0.5) / n as f32;
            let at = from.lerp(to, along) - dir * tail;
            // Tangent for the ribbon; the puff hangs where it was born.
            // Light it only once the shot has passed this stretch, or the
            // wake pops in a whole tick ahead of the interpolating slug.
            let vel = dir * (step / n as f32);
            let start = time + ((i as f32 + 1.0) / n as f32) * duration;
            if p.color & (PROJECTILE_SMOKE | PROJECTILE_MISSILE) != 0 {
                let life = if p.wake > 0.0 { p.wake } else { 1.2 };
                // A missile given a `wake` hangs a heavy, billowing column that long; a
                // booster climbing to its apogee a bigger one still.
                let boost = p.color & PROJECTILE_APOGEE != 0;
                let (size, grow) = if heavy && skim {
                    // A cruise missile lays a long, low smoke line that spreads as it hangs.
                    ((0.8 + p.size * 0.3).min(2.0), 4.5)
                } else if heavy && boost {
                    ((1.2 + p.size * 0.4).min(2.8), 3.5)
                } else if heavy {
                    // A giant's rockets (a very big tracer) trail a column to match.
                    ((1.3 + p.size * 0.45).min(3.2_f32.max(p.size * 0.6)), 4.0)
                } else {
                    ((0.35 + p.size * 0.2).min(0.65), 1.5)
                };
                self.push_puff(PUFF_BOMB_TRAIL, at, vel, start, life, (size, size * grow));
            } else if arc {
                let life = if p.wake > 0.0 {
                    p.wake + self.scatter.unit() * 0.08
                } else {
                    0.85 + self.scatter.unit() * 0.25
                };
                // A small direct-fire slug (the Bulwark) keeps a thin, dim thread.
                let faint = p.wake > 0.0 && p.size < 1.0;
                let s = if faint {
                    (0.08 + p.size * 0.05).min(0.16)
                } else {
                    (0.62 + p.size * 0.34).min(1.55)
                };
                let end = if faint { -s * 1.4 } else { s * 2.4 };
                self.push_puff(PUFF_ARC, at, vel, start, life, (s, end));
                if p.plasma > 0.0 {
                    self.emit_plasma_around(at, dir, vel, start, p.plasma);
                }
            } else {
                let life = 3.4 + self.scatter.unit() * 0.6;
                self.push_puff(PUFF_TRAIL, at, vel, start, life, (0.36, 1.05));
            }
        }
        if engine {
            // A cruise missile's motor burns a long tongue of flame.
            let (life, flame) = if heavy && skim {
                (0.34, (0.45, 1.3))
            } else if heavy && p.color & PROJECTILE_APOGEE == 0 {
                // A heavy rocket's motor: a flame you can pick out from the air.
                let s = (0.5 + p.size * 0.15).min(1.2);
                (0.36, (s, s * 2.6))
            } else {
                (0.28, (0.22, 0.55))
            };
            self.push_puff(
                PUFF_FIRE,
                to - dir * (tail + 0.35),
                -dir * 5.0,
                time + duration,
                life,
                flame,
            );
        }
    }

    /// The cloud a heavy rocket kicks out as its motor lights: a burst of flame at the rail
    /// and a knot of smoke that billows round the launch point, so a launch reads from afar.
    fn rocket_launch_plume(&mut self, from: Vec3, to: Vec3, time: f32, p: &ProjectileInstance) {
        let dir = (to - from).normalize_or_zero();
        let size = (1.3 + p.size * 0.45).min(3.2_f32.max(p.size * 0.6));
        self.push_puff(
            PUFF_FIRE,
            from,
            dir * 4.0,
            time,
            0.22,
            (size * 0.5, size * 1.4),
        );
        for _ in 0..6 {
            let spray = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.6,
            );
            let vel = -dir * (6.0 + self.scatter.unit() * 8.0) + spray * 5.0;
            let life = p.wake * (0.6 + self.scatter.unit() * 0.3);
            let start = time + self.scatter.unit() * 0.06;
            self.push_puff(
                PUFF_BOMB_TRAIL,
                from - dir * 1.5,
                vel,
                start,
                life,
                (size * 1.1, size * 5.0),
            );
        }
    }
}
