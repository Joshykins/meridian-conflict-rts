//! A reactor going up (`death_blast`): a plant's held charge breaking loose, then
//! the fusion fireball. In three acts, all of it sized by the blast's radius, the
//! commander's 140 m being the reference:
//!
//! - **Breach** (a third to half a second): the core's light swells blue, arcs lash
//!   out of it to the ground all round, and sparks are drawn in to it.
//! - **Detonation**: a blue-white flash over the white one, the fireball climbing on
//!   its stalk into a cloud, a shock front across the ground, and the charge earthing:
//!   lightning running out along the ground in forks to the edge of the blast, each
//!   strike lighting the ground and spitting sparks; ionised haze in the fire.
//! - **Aftermath**: the crater crackles, arcs snapping across it, fewer and fewer,
//!   for several seconds (`ReactorFx::aftermath`, laid a tick at a time).
//!
//! Presentation only.

use super::reactor_fx::Aftermath;
use super::stun_fx::{Flash, BOLT};
use super::{
    Renderer, PUFF_BOLT, PUFF_CLOD, PUFF_DUST, PUFF_FIRE, PUFF_FIREBALL, PUFF_PLASMA, PUFF_SMOKE,
    PUFF_SPARK,
};
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;
use std::f32::consts::TAU;

/// The arcs' light: blue-white, as the lightning is.
const ARC_LIGHT: Vec3 = Vec3::new(0.5, 0.68, 1.0);
/// Metres a second the charge runs out along the ground.
const EARTHING_SPEED: f32 = 240.0;
/// The commander's blast, the size this is drawn for.
const BLAST_REF: f32 = 140.0;
/// Blasts this big (tech 2 and 3) go up as a fireball of the nuclear kind (`nuke_fx.rs`),
/// drawn at least this share of a warhead's size; a smaller one as puffs.
const VOLUME_BLAST: f32 = 50.0;
const VOLUME_SMALLEST: f32 = 0.1;

impl Renderer {
    /// A volatile plant of `blueprint` dying at `at` (`h` its height): its charge breaks
    /// loose and then it goes up, `blast` metres.
    pub(super) fn reactor_death(
        &mut self,
        blueprint: BlueprintId,
        at: Vec3,
        h: f32,
        blast: f32,
        time: f32,
    ) {
        let s = blast / BLAST_REF;
        // Where the charge was held: the model's core if it says, else mid-height.
        let (core_z, orb) = self
            .reactor_fx
            .core_of(blueprint.0 as u32)
            .unwrap_or((h * 0.5, blast * 0.03));
        let core = at + Vec3::Z * core_z;
        let breach = 0.18 + 0.3 * s.sqrt();
        self.reactor_breach(at, core, orb, blast, time, breach);
        let bang = time + breach;
        if blast >= VOLUME_BLAST {
            // A big plant goes up as a fireball of the nuclear kind, at its own size.
            self.reactor_fx.detonations.push((at, blast, bang));
        } else {
            self.fusion_fireball(at, core, blast * 0.046, blast, bang);
        }
        // A blue-white flash, and the haze of the charge in the fire.
        self.push_effect(core.to_array(), bang, 70.0 * s + 8.0, 0.3, 9.0, 0.0);
        let haze = 4 + (12.0 * s) as usize;
        for _ in 0..haze {
            let dir = self.scatter.upward(0.0);
            let vel = dir * (6.0 + 14.0 * self.scatter.unit()) * s.sqrt();
            let life = 0.7 + 0.6 * self.scatter.unit();
            let size = blast * (0.06 + 0.05 * self.scatter.unit());
            self.push_puff(PUFF_PLASMA, core, vel, bang, life, (size, size * 2.2));
        }
        self.earthing(at, core, blast, bang);
        self.reactor_fx.aftermath.push(Aftermath {
            at,
            reach: blast * 0.3,
            start: bang + 0.4,
            until: bang + 3.0 + 5.0 * s.sqrt(),
        });
    }

    /// The breach: the core swells, arcs lash the ground, sparks are drawn in.
    fn reactor_breach(
        &mut self,
        at: Vec3,
        core: Vec3,
        orb: f32,
        blast: f32,
        time: f32,
        breach: f32,
    ) {
        let s = blast / BLAST_REF;
        self.push_effect(
            core.to_array(),
            time,
            orb * 5.0 + 4.0,
            breach + 0.12,
            0.0,
            1.5,
        );
        self.emp_fx.flashes.push(Flash {
            pos: core,
            start: time,
            life: breach + 0.1,
            color: ARC_LIGHT * (30.0 + 120.0 * s),
            range: blast * 0.8,
            flicker: 1.0,
        });
        let lashes = 6 + (34.0 * s) as usize;
        for i in 0..lashes {
            let a = self.scatter.unit() * TAU;
            let out = Vec2::new(a.cos(), a.sin());
            let reach = orb * 1.5 + blast * (0.08 + 0.32 * self.scatter.unit());
            let xy = at.truncate() + out * reach;
            let to = xy.extend(self.ground_height(xy) + 0.3);
            let dir = (to - core).normalize_or(Vec3::NEG_Z);
            let from = core + dir * orb;
            let side = dir.cross(Vec3::Z).normalize_or(Vec3::X);
            let up = side.cross(dir).normalize_or(Vec3::Z);
            // Later in the breach, more of them and fiercer.
            let k = (i as f32 + self.scatter.unit()) / lashes as f32;
            let start = time + breach * k.sqrt();
            let life = 0.08 + 0.1 * self.scatter.unit();
            let len = from.distance(to);
            let width = (0.25 + orb * 0.06) * (0.6 + k);
            self.emp_kinks(from, to, len * 0.16, side, up, start, life, width, 7, BOLT);
            self.push_effect(to.to_array(), start, 1.5 + orb * 0.5, 0.2, 9.0, 0.0);
        }
        for _ in 0..(10 + (40.0 * s) as usize) {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            )
            .normalize_or(Vec3::Z);
            let from = core + dir * (orb * 2.5 + blast * 0.06);
            let life = breach * (0.6 + 0.4 * self.scatter.unit());
            let vel = -dir * (from.distance(core) - orb) / life;
            let start = time + (breach - life) * self.scatter.unit();
            self.push_puff(PUFF_BOLT, from, vel, start, life, (0.3 + orb * 0.05, 0.08));
        }
    }

    /// The charge earthing: forks of lightning running out along the ground from under
    /// the blast to its edge, each leg striking as the front reaches it.
    fn earthing(&mut self, at: Vec3, core: Vec3, blast: f32, bang: f32) {
        let s = blast / BLAST_REF;
        let forks = 6 + (18.0 * s) as usize;
        for f in 0..forks {
            let a = (f as f32 + self.scatter.unit() * 0.7) * TAU / forks as f32;
            let mut dir = Vec2::new(a.cos(), a.sin());
            let end = blast * (0.55 + 0.45 * self.scatter.unit());
            let legs = 4 + (3.0 * s) as usize;
            let mut last = core;
            let mut run = 0.0;
            for leg in 0..legs {
                let reach = end * (leg + 1) as f32 / legs as f32;
                // Wander a little off the line as it goes, like a crack.
                let turn = self.scatter.signed() * 0.45;
                dir = Vec2::new(
                    dir.x * turn.cos() - dir.y * turn.sin(),
                    dir.x * turn.sin() + dir.y * turn.cos(),
                );
                let xy = at.truncate() + dir * reach;
                let next = xy.extend(self.ground_height(xy) + 0.4);
                let side = (next - last).cross(Vec3::Z).normalize_or(Vec3::X);
                let len = last.distance(next);
                run += len;
                let start = bang + run / EARTHING_SPEED;
                let width = (0.5 + 1.4 * s) * (1.0 - 0.6 * leg as f32 / legs as f32);
                self.emp_kinks(
                    last,
                    next,
                    len * 0.12,
                    side,
                    Vec3::Z,
                    start,
                    0.32,
                    width,
                    5,
                    BOLT,
                );
                // Re-strikes along the channel a beat later, thinner.
                self.emp_kinks(
                    last,
                    next,
                    len * 0.18,
                    side,
                    Vec3::Z,
                    start + 0.12,
                    0.2,
                    width * 0.6,
                    5,
                    BOLT,
                );
                if leg % 2 == 1 || leg + 1 == legs {
                    self.push_effect(next.to_array(), start, 3.0 + 4.0 * s, 0.3, 9.0, 0.0);
                    self.emp_fx.flashes.push(Flash {
                        pos: next + Vec3::Z * 2.0,
                        start,
                        life: 0.35,
                        color: ARC_LIGHT * (10.0 + 25.0 * s),
                        range: 10.0 + 25.0 * s,
                        flicker: 1.0,
                    });
                    for _ in 0..3 {
                        let vel = self.scatter.upward(0.3) * (4.0 + 8.0 * self.scatter.unit());
                        let life = 0.3 + 0.4 * self.scatter.unit();
                        self.push_puff(PUFF_BOLT, next, vel, start, life, (0.35, 0.08));
                    }
                    self.push_puff(
                        PUFF_DUST,
                        next,
                        Vec3::Z * 2.0,
                        start,
                        1.4,
                        (1.5 + 3.0 * s, 4.0 + 6.0 * s),
                    );
                }
                last = next;
            }
        }
    }

    /// This tick's crackle over the craters of plants gone up.
    pub(super) fn reactor_aftermath(&mut self, time: f32) {
        let due: Vec<(Vec3, f32, f32)> = self
            .reactor_fx
            .detonations
            .iter()
            .copied()
            .filter(|d| d.2 <= time)
            .collect();
        self.reactor_fx.detonations.retain(|d| d.2 > time);
        for (at, blast, when) in due {
            self.nuclear_detonation(at, blast, true, VOLUME_SMALLEST, when);
        }
        let dt = self.tick_seconds.max(0.02);
        self.reactor_fx.aftermath.retain(|a| time < a.until);
        let live: Vec<Aftermath> = self.reactor_fx.aftermath.clone();
        for a in live {
            if time < a.start {
                continue;
            }
            // Dying away: often at first, then now and then.
            let left = 1.0 - (time - a.start) / (a.until - a.start);
            let chance = left * left * (1.0 + a.reach / 20.0) * dt * 6.0;
            let mut n = chance.floor() as usize;
            if self.scatter.unit() < chance.fract() {
                n += 1;
            }
            for _ in 0..n.min(4) {
                let pick = |r: &mut Self| {
                    let a2 = r.scatter.unit() * TAU;
                    let d = a.reach * r.scatter.unit().sqrt();
                    let xy = a.at.truncate() + Vec2::new(a2.cos(), a2.sin()) * d;
                    xy.extend(r.ground_height(xy) + 0.3)
                };
                let (p, q) = (pick(self), pick(self));
                let mid = p.lerp(q, 0.5) + Vec3::Z * (1.0 + p.distance(q) * 0.25);
                let side = (q - p).cross(Vec3::Z).normalize_or(Vec3::X);
                let start = time + self.scatter.unit() * dt;
                let width = 0.2 + 0.5 * left;
                let len = p.distance(q);
                self.emp_kinks(
                    p,
                    mid,
                    len * 0.1,
                    side,
                    Vec3::Z,
                    start,
                    0.12,
                    width,
                    3,
                    BOLT,
                );
                self.emp_kinks(
                    mid,
                    q,
                    len * 0.1,
                    side,
                    Vec3::Z,
                    start,
                    0.12,
                    width,
                    3,
                    BOLT,
                );
                self.emp_fx.flashes.push(Flash {
                    pos: mid,
                    start,
                    life: 0.15,
                    color: ARC_LIGHT * 8.0 * left,
                    range: 6.0 + len,
                    flicker: 1.0,
                });
                let vel = self.scatter.upward(0.3) * 5.0;
                self.push_puff(PUFF_BOLT, q, vel, start, 0.35, (0.3, 0.08));
            }
        }
    }

    fn fusion_fireball(&mut self, at: Vec3, core: Vec3, r: f32, blast: f32, time: f32) {
        const BLAST_REF: f32 = 140.0;
        let s = blast / BLAST_REF;
        // Fewer, not only smaller, puffs for a small plant: a row of them going up
        // must not eat the puff budget.
        let n = |count: u32| ((count as f32 * s.sqrt().clamp(0.35, 1.0)).ceil()) as u32;
        // The flash: twice, the second broader and slower, so it blinds and then lingers.
        self.push_effect(core.to_array(), time, 200.0 * s, 0.4, 4.0, 0.0);
        self.push_effect(core.to_array(), time + 0.05, 120.0 * s, 1.2, 4.0, 0.0);
        // The shock front along the ground, and a second behind it.
        self.push_effect(
            (at + Vec3::Z * 2.0).to_array(),
            time + 0.05,
            blast * 1.25,
            1.1,
            2.0,
            1.0,
        );
        self.push_effect(
            (at + Vec3::Z * 2.0).to_array(),
            time + 0.35,
            blast * 0.9,
            1.4,
            2.0,
            1.0,
        );
        // The fireball, rolling upward.
        for i in 0..7 {
            let rise = i as f32 * 7.0 * s;
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 1.2
                + Vec3::Z * rise;
            self.push_effect(
                (core + off).to_array(),
                time + 0.12 + 0.16 * i as f32,
                r * (7.0 - i as f32 * 0.5),
                1.6 + 0.2 * i as f32,
                2.0,
                0.0,
            );
        }
        for i in 0..n(26) {
            let off = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit(),
            ) * r
                * 2.2;
            let vel = self.scatter.upward(0.3) * (14.0 + self.scatter.unit() * 22.0) * s.sqrt();
            let life = 2.2 + self.scatter.unit() * 1.6;
            self.push_puff(
                PUFF_FIREBALL,
                core + off,
                vel,
                time + 0.04 * i as f32,
                life,
                (r * 1.6, r * 4.5),
            );
        }
        // The stalk, growing at forty metres a second, and the cloud it carries up.
        let stalk = n(22);
        for i in 0..stalk {
            let z = (4.0 + i as f32 * 3.2 * 22.0 / stalk as f32) * s;
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.7
                + Vec3::Z * z;
            let kind = if i % 3 == 0 {
                PUFF_FIREBALL
            } else {
                PUFF_SMOKE
            };
            let life = 5.0 + self.scatter.unit() * 3.0;
            let climb = Vec3::Z * (6.0 + self.scatter.unit() * 5.0) * s;
            self.push_puff(
                kind,
                at + off,
                climb,
                time + 0.3 + z / 40.0,
                life,
                (r * 1.4, r * 3.2),
            );
        }
        let cap = n(28);
        for i in 0..cap {
            let a = (i as f32 + self.scatter.unit()) * TAU / cap as f32;
            let ring = Vec3::new(a.cos(), a.sin(), 0.0) * r * (2.0 + self.scatter.unit() * 2.4);
            let top = at + ring + Vec3::Z * (72.0 + self.scatter.signed() * 7.0) * s;
            let roll = (ring.normalize_or_zero() * (5.0 + self.scatter.unit() * 5.0)
                + Vec3::Z * (2.0 + self.scatter.unit() * 3.0))
                * s;
            let kind = if i % 4 == 0 {
                PUFF_FIREBALL
            } else {
                PUFF_SMOKE
            };
            let life = 6.0 + self.scatter.unit() * 3.5;
            let start = time + 2.0 * s.sqrt() + self.scatter.unit() * 0.5;
            self.push_puff(kind, top, roll, start, life, (r * 2.4, r * 6.0));
        }
        // Dust driven out flat ahead of the shock, all the way to the edge of the blast.
        for ring in 0..3 {
            let around = n(30);
            for i in 0..around {
                let a = (i as f32 + self.scatter.unit()) * TAU / around as f32;
                let out = Vec3::new(a.cos(), a.sin(), 0.03);
                let from = blast * (0.12 + 0.3 * ring as f32);
                let life = 2.2 + self.scatter.unit() * 1.6;
                let push = out * (60.0 + self.scatter.unit() * 30.0) * s.sqrt();
                self.push_puff(
                    PUFF_DUST,
                    at + out * from + Vec3::Z * 0.6,
                    push,
                    time + 0.05 + from / 170.0,
                    life,
                    (r * 1.2, r * 4.2),
                );
            }
        }
        // Burning fragments thrown a long way, and earth with them.
        for _ in 0..n(110) {
            let vel = self.scatter.upward(0.1) * (25.0 + self.scatter.unit() * 70.0) * s.sqrt();
            let life = 1.0 + self.scatter.unit() * 2.2;
            let (start, size) = (
                time + self.scatter.unit() * 0.1,
                0.5 + self.scatter.unit() * 0.5,
            );
            self.push_puff(PUFF_SPARK, core, vel, start, life, (size, 0.1));
        }
        for _ in 0..n(40) {
            let vel = self.scatter.upward(0.3) * (18.0 + self.scatter.unit() * 35.0) * s.sqrt();
            let life = 1.6 + self.scatter.unit() * 1.4;
            let size = 0.5 + self.scatter.unit() * 0.6;
            self.push_puff(PUFF_CLOD, core, vel, time + 0.05, life, (size, 0.3));
        }
        // Fire in the crater and smoke over it, for a long while after.
        for i in 0..30 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 2.5
                + Vec3::Z * 1.5;
            let vel = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                3.0 + self.scatter.unit() * 2.5,
            );
            let start = time + 1.2 + i as f32 * 0.35 + self.scatter.unit() * 0.2;
            let kind = if i % 2 == 0 { PUFF_FIRE } else { PUFF_SMOKE };
            let life = 2.5 + self.scatter.unit() * 2.0;
            self.push_puff(kind, at + off, vel, start, life, (r * 0.8, r * 2.6));
        }
    }
}
