//! The Regency's direct-fire plasma guns as they are drawn (docs/STYLE.md "The Regency
//! suite"): every grade its own charge, shot and strike, none of them ARC's (no shock ring,
//! no dust or clods, no powder smoke). Picked by the weapon's data (`Weapon::plasma_grade`,
//! and a proximity fuse for flak), never by a unit:
//!
//! - **Plasmeric bolt** (the Picket's repeater): no charge. A small red bloom at the mouth
//!   for each bolt; in flight a fat glowing teardrop (sprites.wgsl, `plasma_look` 4); where
//!   it lands a small ragged splash of plasma, a few sparkles and a seared spot.
//! - **Plasmeric flak** (the Canopy's, a proximity-fused bolt): the same bolt; it bursts as
//!   a wide red bloom that throws sparkles and streaks of plasma out through the air.
//! - **Pinched-plasmeric** (the Halberd's): over its charge (`SimEvent::WeaponCharging`) a
//!   ball of red plasma gathers in front of the bore, motes and filaments drawn in to it,
//!   the air round it shimmering as the pinch takes hold, its light growing. Each bolt of
//!   the salvo leaves it with a hard red flash; the ball collapses after the last. Where a
//!   bolt lands it bursts in ragged red fronds over a white heart, molten spatter thrown
//!   out low, globs of plasma thrown out of it in place of a shock ring, and the ground seared: a
//!   small glassed scorch that glows and crusts over.
//! - **Pinch-fusion** (the Sunspear's): the charge goes much further: lightning crackles
//!   round the ball, the shimmer is wider, and over the last part it goes over to fusion,
//!   white at the heart with every colour running round its rim, its light white and
//!   strobing. It launches with a blinding white flash, globs of plasma thrown off round
//!   the bore and arcs snapping forward. Where it lands it opens white with every colour in its fringe and
//!   cools back to red, the ground melted into a wide glowing pool; a knot of fusion is
//!   left burning over the pool for seconds, slowly letting white lightning go into the
//!   ground round it while red sparkles cool and drift off the edges.
//!
//! Presentation only; the renderer's own clock. The light is plasma_puffs.wgsl's (the
//! ball, the bursts) and warp_puffs.wgsl's (motes, filaments, arcs, shimmer, flash), which
//! take any colour.

use super::{Puff, Renderer, PUFF_RING, PUFF_TREE_SMOKE};
use crate::gpu_consts::puff;
use glam::Vec3;
use mc_data::{BlueprintId, PlasmaGrade, Trajectory, Weapon};
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK};
use std::mem::size_of;

const ORB: f32 = puff::PLASMA_ORB as f32;
const BURST: f32 = puff::PLASMA_BURST as f32;
const GLOB: f32 = puff::PLASMA_GLOB as f32;
const GLOW: f32 = puff::WARP_GLOW as f32;
const STREAK: f32 = puff::WARP_STREAK as f32;
const SHIMMER: f32 = puff::WARP_RIFT as f32;
const ARC: f32 = puff::WARP_ARC as f32;
const MOTE: f32 = puff::WARP_MOTE as f32;

/// The plasma's colours, brightness in their size: red plasma, its hot pink-white heart,
/// and the white of fusion.
const RED: Vec3 = Vec3::new(1.0, 0.07, 0.04);
const HOT: Vec3 = Vec3::new(1.0, 0.55, 0.5);
const WHITE: Vec3 = Vec3::new(1.0, 0.96, 1.0);
/// Share of its birth speed a mote covers in its life of `MOTE_LIFE` seconds
/// (warp_puffs.wgsl: drag 1.8), so one aimed at the ball arrives as it dies.
const MOTE_LIFE: f32 = 0.5;
const MOTE_REACH: f32 = 0.329;
/// Seconds between the steps a charge is laid in, each on the barrel as it is then.
const STEP: f32 = 0.05;
/// Seconds a knot of fusion burns over its pool after a Pinch-fusion strike.
const KNOT: f32 = 4.0;
/// Timed lights held at most. A deliberate cosmetic cap: the oldest goes first.
const MAX_GLOWS: usize = 96;
/// Guns charging at once that are drawn. A deliberate cosmetic cap: a charge past it is
/// not drawn (its shot still is), so a wall of guns charging costs no more than this.
const MAX_CHARGES: usize = 48;

/// What a direct-fire Regency plasma gun is drawn as.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Grade {
    Bolt,
    Flak,
    Pinched,
    Fusion,
}

/// The grade a weapon is drawn as: a direct-fire plasma gun. None for a beam, a thrown
/// charge, a missile, or anything not plasma.
pub(super) fn grade(w: &Weapon) -> Option<Grade> {
    if w.beam || w.missile || w.curve.0 > 0 || w.trajectory == Trajectory::Ballistic {
        return None;
    }
    Some(match w.plasma_grade? {
        PlasmaGrade::Plasmeric if w.proximity.0 > 0 => Grade::Flak,
        PlasmaGrade::Plasmeric => Grade::Bolt,
        PlasmaGrade::Pinched => Grade::Pinched,
        PlasmaGrade::PinchFusion => Grade::Fusion,
    })
}

/// A squeezed gun's ball, `0..1` of its size: across, in metres, at full charge.
fn ball(w: &Weapon) -> f32 {
    (1.0 + w.damage.to_f32().max(1.0).sqrt() * 0.1) * w.flash.max(0.5)
}

/// A gun gathering its charge in front of its bore.
struct Charge {
    unit: u32,
    blueprint: BlueprintId,
    weapon: u8,
    fusion: bool,
    start: f32,
    due: f32,
    /// Time laid so far.
    laid: f32,
    /// Bolts of the salvo still to leave it; it collapses after the last.
    left: u8,
    /// Where it was last laid.
    at: Vec3,
}

/// A light the guns throw: from `start` for `life` seconds, fading out, pulsing `pulse`
/// times a second (0 steady).
#[derive(Clone, Copy)]
struct Glow {
    pos: Vec3,
    color: Vec3,
    range: f32,
    start: f32,
    life: f32,
    pulse: f32,
}

#[derive(Default)]
pub(super) struct RegencyGunFx {
    charges: Vec<Charge>,
    glows: Vec<Glow>,
}

impl RegencyGunFx {
    fn light(&mut self, glow: Glow) {
        if self.glows.len() >= MAX_GLOWS {
            self.glows.remove(0);
        }
        self.glows.push(glow);
    }
}

/// `v` turned `a` radians about the upright.
fn rot_z(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

/// `v` pitched `a` radians up about the sideways axis.
fn rot_xz(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
}

/// An angle `f` of the way from `a` to `b` the short way round.
fn lerp_angle(a: f32, b: f32, f: f32) -> f32 {
    a + ((b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI)
        * f
}

impl Renderer {
    /// Where `w`'s muzzle (the middle of its charge) is drawn on `u`, `f` of the way through
    /// the tick, and the way its bore points: the turret's yaw and the gun's pitch about its
    /// trunnion, as entity.wgsl draws them.
    fn regency_muzzle(&self, u: &UnitInstance, w: &Weapon, f: f32) -> (Vec3, Vec3) {
        let pos = Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), f);
        let heading = lerp_angle(u.prev_heading, u.heading, f);
        let muzzle = Vec3::from(w.muzzle.to_f32());
        let (local, dir) = match w.pivot {
            Some(p) if w.turret_turn > 0 => {
                let pivot = Vec3::from(p.to_f32());
                let yaw = lerp_angle(u.prev_turret_yaw, u.turret_yaw, f);
                let pitch = u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * f;
                let bore = (muzzle - pivot).normalize_or(Vec3::X);
                (
                    rot_z(pivot + rot_xz(muzzle - pivot, pitch), yaw),
                    rot_z(rot_xz(bore, pitch), yaw),
                )
            }
            _ => (muzzle, Vec3::X),
        };
        (pos + rot_z(local, heading), rot_z(dir, heading))
    }

    /// A gun began charging (`WeaponCharging`). True when it is a squeezed plasma gun,
    /// whose charge this draws (and nothing else should).
    pub(super) fn regency_charging(
        &mut self,
        unit: u32,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        time: f32,
    ) -> bool {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let fusion = match grade(w) {
            Some(Grade::Pinched) => false,
            Some(Grade::Fusion) => true,
            _ => return false,
        };
        let due = time + w.charge_ticks as f32 * self.tick_seconds;
        let left = w.salvo.max(1);
        let fx = &mut self.plasma_fx.guns;
        fx.charges
            .retain(|c| !(c.unit == unit && c.blueprint == blueprint && c.weapon == weapon));
        if fx.charges.len() < MAX_CHARGES {
            fx.charges.push(Charge {
                unit,
                blueprint,
                weapon,
                fusion,
                start: time,
                due,
                laid: time,
                left,
                at,
            });
        }
        true
    }

    /// The next tick of every charge, on its barrel as it is drawn (from `upload_sim`).
    pub(super) fn regency_guns_tick(&mut self, units: &[UnitInstance], time: f32) {
        let tick = self.tick_seconds.max(0.02);
        // A charge whose salvo never came (the target died) goes out a little after it was due.
        self.plasma_fx
            .guns
            .charges
            .retain(|c| time < c.due + tick * 4.0 + 0.1 * c.left as f32);
        for i in 0..self.plasma_fx.guns.charges.len() {
            let c = &self.plasma_fx.guns.charges[i];
            let (unit, blueprint, weapon, laid, due) =
                (c.unit, c.blueprint, c.weapon, c.laid, c.due);
            // Held until the salvo is spent, past the charge's own end.
            let until = time + tick;
            if laid >= until {
                continue;
            }
            let live = |u: &&UnitInstance| {
                u.unit_id == unit && u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0
            };
            let Some(u) = units.iter().find(live).copied() else {
                continue;
            };
            let w = self.blueprints.unit(blueprint).weapons[weapon as usize].clone();
            let from = laid.max(time);
            let steps = ((until - from) / STEP).ceil().max(1.0) as usize;
            for k in 0..steps {
                let when = from + (until - from) * k as f32 / steps as f32;
                let (at, dir) = self.regency_muzzle(&u, &w, ((when - time) / tick).clamp(0.0, 1.0));
                let c = &self.plasma_fx.guns.charges[i];
                let f = ((when - c.start) / (due - c.start).max(0.01)).clamp(0.0, 1.0);
                let fusion = c.fusion;
                self.regency_charge_step(
                    at,
                    dir,
                    &w,
                    f,
                    fusion,
                    when,
                    (until - from) / steps as f32,
                );
                self.plasma_fx.guns.charges[i].at = at;
            }
            self.plasma_fx.guns.charges[i].laid = until;
        }
    }

    /// `span` seconds of a charge `f` of the way done, at `at`.
    fn regency_charge_step(
        &mut self,
        at: Vec3,
        dir: Vec3,
        w: &Weapon,
        f: f32,
        fusion: bool,
        when: f32,
        span: f32,
    ) {
        let size = ball(w) * if fusion { 1.5 } else { 1.0 };
        let life = span * 2.0;
        // Fusion takes over the last part of it: white at the heart, colour round the rim.
        let over = if fusion { smooth(0.35, 0.92, f) } else { 0.0 };
        let across = size * 1.8 * (0.25 + 0.75 * f.sqrt());
        let rgb =
            RED.lerp(WHITE, over * 0.7) * (1.6 + 4.5 * f * f) * if fusion { 1.4 } else { 1.0 };
        self.push_lit(
            ORB,
            at,
            Vec3::ZERO,
            when,
            life,
            (across, across * 1.04),
            rgb,
            over,
        );
        // Motes of plasma drawn in from round the bore.
        let motes = (1.0 + f * if fusion { 4.0 } else { 2.5 } + self.scatter.unit()) as usize;
        for _ in 0..motes {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            )
            .normalize_or(Vec3::Z);
            let from = at + (out + dir * 0.6) * size * (1.6 + self.scatter.unit() * 1.4);
            let rgb = RED.lerp(HOT, self.scatter.unit() * 0.5 + over * 0.5) * (3.0 + 4.0 * f);
            let dot = size * 0.08 * (1.0 + over);
            let late = self.scatter.unit() * span;
            self.push_lit(
                MOTE,
                from,
                (at - from) / MOTE_REACH,
                when + late,
                MOTE_LIFE,
                (dot, dot * 0.5),
                rgb,
                0.0,
            );
        }
        // Filaments of it pulled in, now and then.
        if self.scatter.unit() < 0.25 + f * 0.6 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.6,
            )
            .normalize_or(Vec3::Z);
            let from = at + out * size * (1.1 + 0.6 * self.scatter.unit());
            self.push_lit(
                STREAK,
                from,
                at - from,
                when,
                0.22,
                (size * 0.04, size * 0.03),
                RED.lerp(WHITE, over) * (2.0 + 3.0 * f),
                0.0,
            );
        }
        // The pinch taking hold: the air round the ball bends.
        let shimmer_from = if fusion { 0.15 } else { 0.4 };
        if f > shimmer_from {
            let s = size * if fusion { 3.4 } else { 2.4 } * (0.7 + 0.3 * f);
            let k = (f - shimmer_from) / (1.0 - shimmer_from);
            self.push_lit(
                SHIMMER,
                at,
                Vec3::ZERO,
                when,
                life,
                (s, s),
                RED.lerp(WHITE, over) * 0.18 * k,
                0.0,
            );
        }
        if fusion && f > 0.3 && self.scatter.unit() < 0.35 + f * 0.9 {
            // Lightning crackling off the ball.
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            )
            .normalize_or(Vec3::Z);
            let reach = size * (0.9 + 1.4 * self.scatter.unit()) * (0.5 + f);
            let life = 0.08 + 0.06 * self.scatter.unit();
            let tint = WHITE.lerp(Vec3::new(0.75, 0.6, 1.0), self.scatter.unit());
            self.push_lit(
                ARC,
                at + out * size * 0.35,
                out * reach,
                when,
                life,
                (size * 0.18, size * 0.18),
                tint * (3.0 + 4.0 * f),
                0.0,
            );
        }
        let color = RED.lerp(WHITE, over) * (5.0 + 70.0 * f * f) * size;
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color,
            range: size * (2.5 + 3.5 * f),
            start: when,
            life: life * 1.2,
            pulse: if fusion { 4.0 + 18.0 * over } else { 0.0 },
        });
    }

    /// A Regency plasma gun fired (`ShotFired`): `at` its muzzle as drawn, `dir` down the
    /// bore, `round_gap` seconds between the rounds a shot is drawn as. True when this
    /// drew the firing (and an ordinary gun's flash and smoke should not be).
    pub(super) fn regency_fired(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        dir: Vec3,
        round_gap: f32,
        time: f32,
    ) -> bool {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let Some(grade) = grade(w) else {
            return false;
        };
        let (rounds, flash, size) = (w.rounds.max(1), w.flash.max(0.3), ball(w));
        match grade {
            Grade::Bolt | Grade::Flak => {
                for k in 0..rounds {
                    let when = time + k as f32 * round_gap;
                    let mouth = at + dir * 0.4;
                    let s = 1.6 * flash;
                    self.push_lit(
                        BURST,
                        mouth,
                        Vec3::ZERO,
                        when,
                        0.11,
                        (s * 0.4, s),
                        RED * 3.5,
                        0.0,
                    );
                    self.push_lit(
                        GLOW,
                        mouth,
                        Vec3::ZERO,
                        when,
                        0.08,
                        (s * 0.5, s * 0.7),
                        HOT * 2.0,
                        0.0,
                    );
                    self.plasma_fx.guns.light(Glow {
                        pos: mouth,
                        color: RED * 60.0 * flash,
                        range: 12.0,
                        start: when,
                        life: 0.1,
                        pulse: 0.0,
                    });
                }
            }
            Grade::Pinched => {
                // The ball gives up a bolt: a hard red flash where it hangs.
                self.charge_spent(blueprint, weapon, at, time);
                self.push_lit(
                    BURST,
                    at,
                    Vec3::ZERO,
                    time,
                    0.16,
                    (size * 0.8, size * 2.2),
                    RED * 4.0,
                    0.0,
                );
                self.push_lit(
                    GLOW,
                    at,
                    Vec3::ZERO,
                    time,
                    0.1,
                    (size, size * 1.3),
                    HOT * 3.0,
                    0.0,
                );
                self.plasma_fx.guns.light(Glow {
                    pos: at,
                    color: RED * 150.0 * size,
                    range: size * 9.0,
                    start: time,
                    life: 0.15,
                    pulse: 0.0,
                });
            }
            Grade::Fusion => {
                self.charge_spent(blueprint, weapon, at, time);
                let s = size * 1.5;
                // Launch: a blinding white flash, arcs snapping forward along the bore.
                self.push_lit(
                    GLOW,
                    at,
                    Vec3::ZERO,
                    time,
                    0.22,
                    (s * 2.0, s * 3.2),
                    WHITE * 6.0,
                    0.0,
                );
                // What was left of the cage thrown off round the bore as globs of plasma.
                let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
                let up = side.cross(dir);
                for k in 0..10 {
                    let a = std::f32::consts::TAU * (k as f32 + self.scatter.unit() * 0.5) / 10.0;
                    let out = (side * a.cos() + up * a.sin() + dir * 0.3).normalize_or(dir);
                    let blob = s * (0.1 + 0.05 * self.scatter.unit());
                    let tint = WHITE.lerp(RED, self.scatter.unit() * 0.7);
                    self.push_lit(
                        GLOB,
                        at,
                        out * s * 5.0,
                        time,
                        0.5,
                        (blob, blob * 0.3),
                        tint * 4.0,
                        0.0,
                    );
                }
                for _ in 0..5 {
                    let spread = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    ) * 0.5;
                    let reach = s * (3.0 + 4.0 * self.scatter.unit());
                    let late = self.scatter.unit() * 0.08;
                    self.push_lit(
                        ARC,
                        at,
                        (dir + spread).normalize_or(dir) * reach,
                        time + late,
                        0.12,
                        (s * 0.25, s * 0.25),
                        WHITE * 6.0,
                        0.0,
                    );
                }
                self.plasma_fx.guns.light(Glow {
                    pos: at,
                    color: WHITE * 500.0 * s,
                    range: s * 14.0,
                    start: time,
                    life: 0.4,
                    pulse: 0.0,
                });
            }
        }
        true
    }

    /// A bolt has left a charge near `at`: when it was the salvo's last, the ball collapses.
    fn charge_spent(&mut self, blueprint: BlueprintId, weapon: u8, at: Vec3, time: f32) {
        let fx = &mut self.plasma_fx.guns;
        let mine = fx
            .charges
            .iter()
            .enumerate()
            .filter(|(_, c)| c.blueprint == blueprint && c.weapon == weapon)
            .map(|(i, c)| (i, c.at.distance(at)))
            .filter(|(_, d)| *d < 30.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, _)) = mine else {
            return;
        };
        let c = &mut fx.charges[i];
        c.left = c.left.saturating_sub(1);
        if c.left > 0 {
            return;
        }
        let (fusion, at) = (c.fusion, c.at);
        fx.charges.swap_remove(i);
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let size = ball(w) * if fusion { 1.5 } else { 1.0 };
        let rgb = if fusion { WHITE * 4.0 } else { RED * 4.0 };
        self.push_lit(
            ORB,
            at,
            Vec3::ZERO,
            time,
            0.18,
            (size, size * 0.15),
            rgb,
            if fusion { 1.0 } else { 0.0 },
        );
    }

    /// A Regency plasma shot struck (`Impact`, not on a shield). True when this drew the
    /// strike (and a shell's blast should not be).
    pub(super) fn regency_landed(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        on_unit: bool,
        start: f32,
    ) -> bool {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let Some(grade) = grade(w) else {
            return false;
        };
        let (impact, splash, size) = (w.impact.max(0.3), w.splash.to_f32(), ball(w));
        // A squeezed strike on a hull standing on the ground sears the ground under it too.
        let height = at.z - self.ground_height(at.truncate());
        let ground = height < 1.5 && !on_unit;
        let under = height < 4.0;
        match grade {
            Grade::Bolt => self.bolt_splash(at, impact, ground, start),
            Grade::Flak => self.plasma_flak_burst(at, impact, splash, ground, start),
            Grade::Pinched => self.pinched_burst(at, impact, size, under, start),
            Grade::Fusion => self.fusion_burst(at, impact, size * 1.5, splash, under, start),
        }
        true
    }

    /// A Plasmeric bolt splashing: a small ragged bloom, a few sparkles, a seared spot.
    fn bolt_splash(&mut self, at: Vec3, impact: f32, ground: bool, start: f32) {
        let s = 2.6 * impact;
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.32,
            (s * 0.3, s),
            RED * 2.6,
            0.0,
        );
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.1,
            (s * 0.4, s * 0.6),
            HOT * 2.5,
            0.0,
        );
        for _ in 0..4 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.3 + self.scatter.unit(),
            )
            .normalize_or(Vec3::Z);
            let dot = 0.18 + 0.12 * self.scatter.unit();
            let speed = 8.0 + 10.0 * self.scatter.unit();
            self.push_lit(
                MOTE,
                at,
                out * speed,
                start,
                0.45,
                (dot, dot * 0.4),
                RED * 4.0,
                0.0,
            );
        }
        if ground {
            self.bore_fx.melt(at.truncate(), 0.7 * impact, start, 2.5);
        }
        self.plasma_fx.guns.light(Glow {
            pos: at + Vec3::Z,
            color: RED * 60.0 * impact,
            range: 10.0 * impact,
            start,
            life: 0.25,
            pulse: 0.0,
        });
    }

    /// A plasma flak bolt bursting: a wide red bloom throwing sparkles and streaks of
    /// plasma out through the air.
    fn plasma_flak_burst(&mut self, at: Vec3, impact: f32, splash: f32, ground: bool, start: f32) {
        let s = (splash * 1.4).max(4.0) * impact;
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.45,
            (s * 0.25, s),
            RED * 3.2,
            0.0,
        );
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.12,
            (s * 0.3, s * 0.45),
            HOT * 3.0,
            0.0,
        );
        for _ in 0..7 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            )
            .normalize_or(Vec3::Z);
            let reach = s * (0.4 + 0.4 * self.scatter.unit());
            self.push_lit(
                STREAK,
                at,
                out * reach,
                start,
                0.3,
                (0.12, 0.08),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        for _ in 0..8 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            )
            .normalize_or(Vec3::Z);
            let dot = 0.25 + 0.2 * self.scatter.unit();
            let when = start + self.scatter.unit() * 0.1;
            self.push_lit(
                MOTE,
                at,
                out * s * 2.2,
                when,
                0.8,
                (dot, dot * 0.3),
                RED * 4.0,
                0.0,
            );
        }
        if ground {
            self.bore_fx.melt(at.truncate(), 0.25 * s, start, 2.5);
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: RED * 100.0 * impact,
            range: s * 3.0,
            start,
            life: 0.35,
            pulse: 0.0,
        });
    }

    /// A Pinched-plasmeric bolt bursting: ragged red fronds over a white heart, molten
    /// spatter thrown out low, globs of plasma in place of a shock ring, the ground seared.
    fn pinched_burst(&mut self, at: Vec3, impact: f32, size: f32, ground: bool, start: f32) {
        let s = size * 1.6 * impact;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.1,
            (s * 0.3, s * 0.5),
            WHITE * 1.5,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.55,
            (s * 0.3, s * 1.6),
            RED * 2.5,
            0.0,
        );
        // Globs of plasma thrown out of it as the bind breaks, in place of a shock ring.
        for _ in 0..7 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.1 + self.scatter.unit() * 0.6,
            )
            .normalize_or(Vec3::Z);
            let speed = s * (2.5 + 2.5 * self.scatter.unit());
            let blob = s * (0.1 + 0.06 * self.scatter.unit());
            self.push_lit(
                GLOB,
                at,
                out * speed,
                start,
                0.6,
                (blob, blob * 0.4),
                RED * 3.5,
                0.0,
            );
        }
        for _ in 0..12 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.15 + self.scatter.unit() * 0.5,
            )
            .normalize_or(Vec3::Z);
            let speed = 14.0 + self.scatter.unit() * 22.0;
            let life = 0.45 + self.scatter.unit() * 0.4;
            self.push_lit(
                MOTE,
                at + Vec3::Z * 0.3,
                out * speed,
                start,
                life,
                (0.28, 0.1),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        for _ in 0..6 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.2 + self.scatter.unit(),
            )
            .normalize_or(Vec3::Z);
            let dot = 0.3 + 0.2 * self.scatter.unit();
            let when = start + self.scatter.unit() * 0.15;
            self.push_lit(
                MOTE,
                at,
                out * s * 1.4,
                when,
                1.1,
                (dot, dot * 0.3),
                RED * 3.5,
                0.0,
            );
        }
        if ground {
            // The ground seared a little: a glassed scorch that glows red and crusts over.
            self.bore_fx.melt(at.truncate(), s * 0.35, start, 7.0);
            for k in 0..2 {
                self.push_puff(
                    PUFF_TREE_SMOKE,
                    at + Vec3::Z * 0.8,
                    Vec3::Z * (2.0 + k as f32),
                    start + 0.2 + k as f32 * 0.3,
                    2.2,
                    (s * 0.25, s * 0.7),
                );
            }
        }
        self.plasma_fx.guns.light(Glow {
            pos: at + Vec3::Z * 1.5,
            color: RED * 35.0 * impact * size,
            range: s * 1.6,
            start,
            life: 0.5,
            pulse: 0.0,
        });
    }

    /// A Pinch-fusion shot letting go: it opens white with every colour in its fringe and
    /// cools to red, globs of it thrown out, the ground melted into a wide pool, and a knot
    /// of fusion left burning over it (`fusion_knot`).
    fn fusion_burst(
        &mut self,
        at: Vec3,
        impact: f32,
        size: f32,
        splash: f32,
        ground: bool,
        start: f32,
    ) {
        let s = (size * 1.6).max(splash * 1.3) * impact;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.3,
            (s * 0.8, s * 1.4),
            WHITE * 8.0,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.45,
            (s * 0.4, s * 2.0),
            WHITE * 3.2,
            1.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start + 0.15,
            1.4,
            (s * 0.6, s * 2.6),
            RED * 2.6,
            0.0,
        );
        // Globs of fusion thrown out of it, in place of a shock ring.
        for _ in 0..12 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.1 + self.scatter.unit() * 0.8,
            )
            .normalize_or(Vec3::Z);
            let speed = s * (2.0 + 2.5 * self.scatter.unit());
            let blob = s * (0.06 + 0.05 * self.scatter.unit());
            let tint = HOT.lerp(RED, self.scatter.unit());
            let when = start + self.scatter.unit() * 0.1;
            self.push_lit(
                GLOB,
                at,
                out * speed,
                when,
                0.9,
                (blob, blob * 0.4),
                tint * 4.0,
                0.0,
            );
        }
        for _ in 0..24 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.2 + self.scatter.unit() * 0.7,
            )
            .normalize_or(Vec3::Z);
            let speed = 20.0 + self.scatter.unit() * 40.0;
            let life = 0.6 + self.scatter.unit() * 0.6;
            let tint = WHITE.lerp(RED, self.scatter.unit());
            self.push_lit(
                MOTE,
                at + Vec3::Z * 0.5,
                out * speed,
                start,
                life,
                (0.36, 0.12),
                tint * 4.0,
                0.0,
            );
        }
        if ground {
            self.bore_fx.melt(at.truncate(), s * 0.6, start, 16.0);
        }
        self.plasma_fx.guns.light(Glow {
            pos: at + Vec3::Z * 3.0,
            color: WHITE * 900.0 * impact,
            range: s * 6.0,
            start,
            life: 0.6,
            pulse: 0.0,
        });
        self.fusion_knot(at, s, impact, start);
    }

    /// The knot of fusion a Pinch-fusion strike leaves burning over its pool (`s` the
    /// strike's size) for `KNOT` seconds: shrinking as it slowly lets white lightning go
    /// into the ground round it, red sparkles cooling and drifting off the edges.
    fn fusion_knot(&mut self, at: Vec3, s: f32, impact: f32, start: f32) {
        let knot = at + Vec3::Z * (1.0 + s * 0.12);
        let pieces = (KNOT / 0.5) as usize;
        for k in 0..pieces {
            let f = k as f32 / pieces as f32;
            let across = s * 0.75 * (1.0 - 0.6 * f);
            let rgb = WHITE.lerp(RED, f * 0.6) * (4.0 - 2.4 * f);
            self.push_lit(
                ORB,
                knot,
                Vec3::ZERO,
                start + 0.3 + k as f32 * 0.5,
                1.0,
                (across, across * 0.92),
                rgb,
                1.0 - f * 0.5,
            );
        }
        // White lightning let go slowly into the ground round it.
        let strokes = 16;
        for k in 0..strokes {
            let when = start + 0.4 + KNOT * (k as f32 + self.scatter.unit()) / strokes as f32;
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let r = s * (0.5 + 0.8 * self.scatter.unit());
            let to = at + Vec3::new(a.cos() * r, a.sin() * r, 0.0);
            let to = to.with_z(self.ground_height(to.truncate()) + 0.2);
            let life = 0.14 + 0.1 * self.scatter.unit();
            let tint = WHITE.lerp(Vec3::new(0.7, 0.75, 1.0), self.scatter.unit());
            self.push_lit(
                ARC,
                knot,
                to - knot,
                when,
                life,
                (s * 0.12, s * 0.12),
                tint * 7.0,
                0.0,
            );
        }
        // Red sparkles cooling and drifting off the edges.
        for _ in 0..45 {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let r = s * (0.6 + 0.6 * self.scatter.unit());
            let from = at + Vec3::new(a.cos() * r, a.sin() * r, 0.4 + self.scatter.unit() * 2.0);
            let drift = Vec3::new(a.cos(), a.sin(), 1.5 + self.scatter.unit())
                * (1.0 + self.scatter.unit() * 2.0);
            let when = start + 0.3 + self.scatter.unit() * KNOT;
            let dot = 0.45 + 0.45 * self.scatter.unit();
            let life = 1.2 + self.scatter.unit();
            self.push_lit(
                MOTE,
                from,
                drift,
                when,
                life,
                (dot, dot * 0.5),
                RED * 5.0,
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: knot,
            color: WHITE.lerp(RED, 0.4) * 150.0 * impact,
            range: s * 3.0,
            start: start + 0.3,
            life: KNOT,
            pulse: 7.0,
        });
    }

    /// Every gun's light this frame (from `upload_lights`).
    pub(super) fn regency_guns_lights(&mut self, time: f32) {
        self.plasma_fx
            .guns
            .glows
            .retain(|g| time < g.start + g.life);
        for g in &self.plasma_fx.guns.glows {
            let age = (time - g.start) / g.life.max(0.01);
            if age < 0.0 {
                continue;
            }
            let fade = (1.0 - age).clamp(0.0, 1.0);
            let beat = if g.pulse > 0.0 {
                0.75 + 0.25 * (time * g.pulse * std::f32::consts::TAU).sin()
            } else {
                1.0
            };
            self.lights.lamp(
                g.pos,
                Vec3::NEG_Z,
                g.color * fade * beat,
                g.range,
                180.0,
                1.0,
            );
        }
    }

    /// One puff of light (plasma_puffs.wgsl, warp_puffs.wgsl): `rgb` its colour and
    /// brightness, `w` its `appearance.w` (how far a plasma puff has gone over to fusion).
    fn push_lit(
        &mut self,
        kind: f32,
        pos: Vec3,
        vel: Vec3,
        start: f32,
        life: f32,
        size: (f32, f32),
        rgb: Vec3,
        w: f32,
    ) {
        let p = Puff {
            appearance: [rgb.x, rgb.y, rgb.z, w],
            origin: pos.to_array(),
            opacity: 1.0,
            pos: pos.to_array(),
            start,
            vel: vel.to_array(),
            life,
            params: [size.0, size.1, kind, self.scatter.unit()],
        };
        self.puffs.write(
            (self.puff_cursor * size_of::<Puff>()) as u64,
            bytemuck::bytes_of(&p),
        );
        self.puff_cursor = (self.puff_cursor + 1) % PUFF_RING;
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
