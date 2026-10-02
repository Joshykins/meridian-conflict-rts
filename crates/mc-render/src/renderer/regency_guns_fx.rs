//! The Regency's direct-fire plasma guns as they are drawn (docs/STYLE.md "The Regency
//! suite"): every grade its own charge, shot and strike, none of them ARC's (no shock ring,
//! no dust or clods, no powder smoke). Picked by the weapon's data (`Weapon::plasma_grade`,
//! and a proximity fuse for flak), never by a unit:
//!
//! - **Plasmeric bolt** (the Plasmeric Repeater: the Picket's, the Sledge's, the navy's):
//!   no charge. Each bolt is spat out hard (`plasmeric`): a white-hot snap at the mouth, a
//!   red bloom thrown forward, droplets and sparks flung after it. In flight a fat bolt of
//!   red plasma round a white-hot heart, its skin boiling and licking back off it
//!   (sprites.wgsl, `plasma_look` 4), lighting what it passes; no trail.
//!   Where it lands it dumps its heat at once: a white flash in a ragged red bloom, a knot
//!   of plasma left frying, droplets spattered out, sparks, a seared glowing spot.
//! - **Plasmeric AA repeater** (the Canopy's, a proximity-fused bolt): the same bolt; it
//!   bursts as a wide red bloom that throws sparkles and streaks of plasma out through the
//!   air.
//! - **Pinched-plasmeric** (the Halberd's): over its charge (`SimEvent::WeaponCharging`) a
//!   ball of red plasma gathers in front of the bore, motes and filaments drawn in to it
//!   and red lightning snapping into it from round it, its light growing. It pinches its
//!   plasma out as one shot, a jet a quarter of a second long (the shot's rounds,
//!   `Weapon::round_span`): a hard red flash as it opens, a pulse at the mouth for each
//!   round, the ball draining as the jet leaves it, plasma thrown down the line of fire.
//!   In flight the jet leaves a thin red trail that cools and breaks up
//!   (`regency_trails`). Where its head lands it bursts in a hard red heart over a white
//!   one that lingers as it cools, lumps bursting round it, filaments torn out of it, a spout of plasma thrown up
//!   out of it, a skirt of it rolling out low, red plasma crackling over the ground, globs
//!   and molten spatter thrown out in place of a shock ring, and the ground seared: a
//!   glassed scorch that glows and crusts over, embers rising off it for a couple of
//!   seconds. The rest of the jet pours in after it, each round a red splash
//!   (`pinched_pour`). The strikes are drawn in `strike.rs`.
//! - **Pinch-fusion** (the Sunspear's): the charge goes much further: lightning crackles
//!   round the ball and is pulled into it, and over the last part it goes over to fusion,
//!   white at the heart with the prism's pinks drifting over it, its light white and
//!   strobing. It launches with a blinding white flash, a cone of plasma thrown out down
//!   the line of fire, globs thrown off round the bore and arcs snapping forward; heat
//!   rises off the gun's back. The shot is a jet of fusion pinched out, long and fast: a
//!   white-hot core in a broad sheath of the prism, strobing, and behind it a white-hot
//!   trail that takes the prism and cools through pink and red, breaking up, shedding
//!   sparks. Where it lands it opens in a blinding flash, a hard white heart with
//!   filaments torn out of it, the prism in them, cooling back to red, a column of plasma
//!   rising out of it, a lumpy skirt of it rolling out over the ground, streaks and globs
//!   flung wide and lightning thrown into the ground round it. As it cools it slows: its
//!   red body, column, skirt and globs open at full pace and then linger, cooling slower
//!   and slower as what was thrown out drifts to a stop (`push_lingering`). The ground is
//!   melted into a wide glowing pool
//!   and a knot of fusion left burning over it for seconds, slowly letting white lightning go into the ground round it while
//!   red sparkles cool and drift off the edges.
//!
//! Nothing is wound round a middle (no spiral arms, no rings), and nothing hangs as a
//! mist: the plasma is hard-edged and goes out fast (plasma_puffs.wgsl).
//!
//! Presentation only; the renderer's own clock. The light is plasma_puffs.wgsl's (the
//! ball, the bursts, the globs, the thrown clumps), warp_puffs.wgsl's (motes, filaments,
//! arcs, flash), which take any colour, and sprites.wgsl's (the trails).

use super::{Puff, Renderer, PUFF_RING};
use crate::gpu_consts::{fade_beam, puff};
use glam::Vec3;
use mc_data::{BlueprintId, PlasmaGrade, Trajectory, Weapon};
use mc_sim::mirror::{
    ProjectileInstance, UnitInstance, KIND_GHOST, KIND_WRECK, PROJECTILE_ENDS_SHIFT,
    PROJECTILE_FADE_BEAM, PROJECTILE_MISSILE, PROJECTILE_STARTS_SHIFT,
};
use std::mem::size_of;

mod plasmeric;
mod strike;

const ORB: f32 = puff::PLASMA_ORB as f32;
pub(super) const BURST: f32 = puff::PLASMA_BURST as f32;
pub(super) const GLOB: f32 = puff::PLASMA_GLOB as f32;
pub(super) const WAKE: f32 = puff::PLASMA_WAKE as f32;
pub(super) const GLOW: f32 = puff::WARP_GLOW as f32;
const STREAK: f32 = puff::WARP_STREAK as f32;
const ARC: f32 = puff::WARP_ARC as f32;
pub(super) const MOTE: f32 = puff::WARP_MOTE as f32;

/// The plasma's colours, brightness in their size: red plasma, its hot pink-white heart,
/// and the white of fusion.
pub(super) const RED: Vec3 = Vec3::new(1.0, 0.07, 0.04);
pub(super) const HOT: Vec3 = Vec3::new(1.0, 0.55, 0.5);
pub(super) const WHITE: Vec3 = Vec3::new(1.0, 0.96, 1.0);
/// Share of its birth speed a mote covers in its life of `MOTE_LIFE` seconds
/// (warp_puffs.wgsl: drag 1.8), so one aimed at the ball arrives as it dies.
const MOTE_LIFE: f32 = 0.5;
const MOTE_REACH: f32 = 0.329;
/// Seconds between the steps a charge is laid in, each on the barrel as it is then.
const STEP: f32 = 0.05;
/// Seconds a knot of fusion burns over its pool after a Pinch-fusion strike.
const KNOT: f32 = 4.0;
/// Timed lights held at most. A deliberate cosmetic cap: the oldest goes first.
const MAX_GLOWS: usize = 256;
/// Guns charging at once that are drawn. A deliberate cosmetic cap: a charge past it is
/// not drawn (its shot still is), so a wall of guns charging costs no more than this.
const MAX_CHARGES: usize = 48;
/// Sparks a fusion round's trail sheds a tick at most, over every shot in flight. A
/// deliberate cosmetic cap: past it the trail is laid without them.
const MAX_TRAIL_SPARKS: usize = 200;
/// Trail pieces held at most. A deliberate cosmetic cap: the oldest goes first.
const MAX_TRAILS: usize = 2400;
/// Metres a piece of a shot's trail runs, and the seconds it glows: a Pinched bolt's, then
/// a Pinch-fusion round's.
const BOLT_TRAIL: (f32, f32) = (12.0, 0.7);
const FUSION_TRAIL: (f32, f32) = (10.0, 1.8);
/// Metres between the lights a Plasmeric bolt throws down its flight.
const PLASMERIC_LIGHT_STEP: f32 = 8.0;

/// What a direct-fire Regency plasma gun is drawn as.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Grade {
    Bolt,
    Flak,
    /// A lobbed Plasmeric shot: the Plasmeric Mortar (`plasmeric`).
    Mortar,
    Pinched,
    Fusion,
}

/// The grade a weapon is drawn as: a plasma gun firing straight, laid flat with its shot
/// arcing a little (`Weapon::flat_fire`, the Sunspear's), or lobbing it high (the
/// Pinch-fusion Howitzer's; a lobbed Plasmeric shot is a Plasmeric Mortar), or a Gravitic
/// Seeker (a guided plasma `missile`) with a proximity fuse, which leaves its cradle and
/// bursts as flak does. None for a beam, a thrown charge, any other missile, or anything
/// not plasma.
pub(super) fn grade(w: &Weapon) -> Option<Grade> {
    let lobbed = w.trajectory == Trajectory::Ballistic && !w.flat_fire;
    if w.beam || w.curve.0 > 0 {
        return None;
    }
    let grade = match w.plasma_shot()? {
        PlasmaGrade::Plasmeric if lobbed && !w.missile => Grade::Mortar,
        PlasmaGrade::Plasmeric if w.proximity.0 > 0 => Grade::Flak,
        PlasmaGrade::Plasmeric => Grade::Bolt,
        PlasmaGrade::Pinched => Grade::Pinched,
        PlasmaGrade::PinchFusion => Grade::Fusion,
        PlasmaGrade::Gravitic => return None,
    };
    (!w.missile || grade == Grade::Flak).then_some(grade)
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

/// A piece of a shot's trail: from `start` for `life` seconds, cooling as it goes.
#[derive(Clone, Copy)]
struct Trail {
    from: Vec3,
    to: Vec3,
    start: f32,
    life: f32,
    width: f32,
    fusion: bool,
}

#[derive(Default)]
pub(super) struct RegencyGunFx {
    charges: Vec<Charge>,
    glows: Vec<Glow>,
    trails: Vec<Trail>,
}

impl RegencyGunFx {
    fn trail(&mut self, trail: Trail) {
        self.trails.push(trail);
    }

    fn light(&mut self, glow: Glow) {
        if self.glows.len() >= MAX_GLOWS {
            self.glows.remove(0);
        }
        self.glows.push(glow);
    }

    /// A white-hot streak from `from` to `to`, the Sunspear round's trail (it takes the
    /// prism's pinks, cools to red and breaks up over `life` seconds): behind a Regency
    /// strategic missile (`nuke_fx::nova`).
    pub(super) fn streak(&mut self, from: Vec3, to: Vec3, start: f32, life: f32, width: f32) {
        self.trail(Trail {
            from,
            to,
            start,
            life,
            width,
            fusion: true,
        });
    }

    /// A steady red light at `pos` from `start` for `life` seconds, fading out.
    pub(super) fn flare(&mut self, pos: Vec3, color: Vec3, range: f32, start: f32, life: f32) {
        self.light(Glow {
            pos,
            color,
            range,
            start,
            life,
            pulse: 0.0,
        });
    }
}

/// What a trail piece is drawn as (sprites.wgsl, `aim.w`).
fn trail_kind(t: &Trail) -> f32 {
    if t.fusion {
        fade_beam::PLASMA_TRAIL_FUSION
    } else {
        0.0
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

    /// The trails Pinched bolts and Pinch-fusion rounds leave behind them (from
    /// `upload_sim`, once a tick, after this tick's shots are written): a hot filament laid
    /// down the stretch each shot flies this tick, each piece lit as the shot passes it,
    /// cooling and breaking up as it hangs (sprites.wgsl `FADE_BEAM_PLASMA_TRAIL`). A
    /// fusion round's is white-hot, takes the prism and cools through pink and red,
    /// shedding sparks; a bolt's is a thin red one. No puffs: a wake of them reads as mist.
    /// A Plasmeric bolt leaves no trail (at its speed even a short-lived one is a beam):
    /// it lights the ground and hulls red down its flight as it passes.
    pub(super) fn regency_trails(&mut self, projectiles: &[ProjectileInstance], time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let mut sparks = 0;
        for p in projectiles {
            let look = drawn_look(p);
            let (fusion, plasmeric) = match look {
                1 => (false, false),
                2 => (true, false),
                4 => (false, true),
                _ => continue,
            };
            // A round of a jet that leaves the muzzle part of the way through the tick
            // (`PROJECTILE_STARTS_SHIFT`) has `prev_pos` behind the gun: its trail starts
            // where it leaves, as sprites.wgsl `shot_muzzle` draws it.
            let ends = ((p.color >> PROJECTILE_ENDS_SHIFT) & 0xFF) as f32 / 255.0;
            let span = if ends > 0.0 { ends } else { 1.0 };
            let starts = (p.color >> PROJECTILE_STARTS_SHIFT) as f32 / 255.0;
            let to = Vec3::from(p.pos);
            let from = Vec3::from(p.prev_pos).lerp(to, (starts / span).min(1.0));
            if plasmeric {
                // A deliberate cosmetic cap on one bolt's lights a tick: a tick's flight
                // is far under this many.
                let n = ((from.distance(to) / PLASMERIC_LIGHT_STEP).ceil() as usize).clamp(1, 16);
                for k in 1..=n {
                    let f = k as f32 / n as f32;
                    self.plasma_fx.guns.light(Glow {
                        pos: from.lerp(to, f),
                        color: RED * 45.0 * p.size,
                        range: 9.0,
                        start: time + tick * (starts + (span - starts).max(0.0) * f),
                        life: tick * 0.4,
                        pulse: 0.0,
                    });
                }
                continue;
            }
            let (step, life) = if fusion { FUSION_TRAIL } else { BOLT_TRAIL };
            // A deliberate cosmetic cap on one shot's stretch: a tick's flight is far
            // under this many pieces.
            let n = ((from.distance(to) / step).ceil() as usize).clamp(1, 16);
            for k in 0..n {
                let (f0, f1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
                let (a, b) = (from.lerp(to, f0), from.lerp(to, f1));
                let when = time + tick * (starts + (span - starts).max(0.0) * f1);
                let fx = &mut self.plasma_fx.guns;
                // The thread where it passed, and the sheath round it that goes out sooner.
                fx.trail(Trail {
                    from: a,
                    to: b,
                    start: when,
                    life,
                    width: p.size * if fusion { 0.32 } else { 0.22 },
                    fusion,
                });
                fx.trail(Trail {
                    from: a,
                    to: b,
                    start: when,
                    life: life * 0.35,
                    width: p.size * if fusion { 0.9 } else { 0.5 },
                    fusion,
                });
                // A fusion round sheds sparks that fall away from its trail. A deliberate
                // cosmetic cap on them a tick.
                if fusion && sparks < MAX_TRAIL_SPARKS && self.scatter.unit() < 0.6 {
                    sparks += 1;
                    let at = a.lerp(b, self.scatter.unit());
                    let drift = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    );
                    let dot = 0.3 + 0.3 * self.scatter.unit();
                    let roll0 = self.scatter.unit();
                    let roll1 = self.scatter.unit();
                    self.push_lit(
                        MOTE,
                        at,
                        drift * 8.0 - Vec3::Z * 3.0,
                        when,
                        0.6 + 0.6 * roll0,
                        (dot, dot * 0.4),
                        WHITE.lerp(HOT, roll1) * 5.0,
                        0.0,
                    );
                }
            }
        }
        self.roll_wakes(time);
        self.write_regency_trails(time);
    }

    /// This tick's trails among the fading beams.
    fn write_regency_trails(&mut self, time: f32) {
        let fx = &mut self.plasma_fx.guns;
        fx.trails.retain(|t| time < t.start + t.life);
        if fx.trails.len() > MAX_TRAILS {
            let extra = fx.trails.len() - MAX_TRAILS;
            fx.trails.drain(..extra);
        }
        let out: Vec<ProjectileInstance> = fx
            .trails
            .iter()
            .map(|t| ProjectileInstance {
                prev_pos: t.from.to_array(),
                color: PROJECTILE_FADE_BEAM | fade_beam::PLASMA_TRAIL,
                pos: t.to.to_array(),
                size: t.width,
                wake: t.start,
                plasma: t.life,
                _pad: [0.0; 2],
                aim: [0.0, 0.0, 0.0, trail_kind(t)],
                prev_aim: [0.0; 4],
            })
            .collect();
        self.push_projectiles(&out);
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
        // A soft halo round it as the pinch takes hold.
        if f > 0.2 {
            let s = size * if fusion { 3.0 } else { 2.2 } * (0.6 + 0.4 * f);
            self.push_lit(
                GLOW,
                at,
                Vec3::ZERO,
                when,
                life,
                (s, s),
                RED.lerp(WHITE, over * 0.5) * 0.5 * f,
                0.0,
            );
        }
        // Lightning snapping into the ball from round it: red, then white in fusion, more
        // often the fuller it gets.
        let snap = if fusion {
            0.25 + f * 1.1
        } else {
            0.1 + f * 0.6
        };
        if f > 0.15 && self.scatter.unit() < snap {
            let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
            let up = side.cross(dir);
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let out = (side * a.cos() + up * a.sin() + dir * self.scatter.signed() * 0.4)
                .normalize_or(side);
            let from = at + out * size * (1.4 + 0.8 * self.scatter.unit());
            let tint = if fusion {
                WHITE.lerp(
                    Vec3::new(1.0, 0.55, 0.8),
                    self.scatter.unit() * (1.0 - over),
                )
            } else {
                RED.lerp(HOT, 0.4 + 0.4 * self.scatter.unit())
            };
            let roll0 = self.scatter.unit();
            self.push_lit(
                ARC,
                from,
                at - from,
                when,
                0.07 + 0.06 * roll0,
                (size * 0.14, size * 0.14),
                tint * (4.0 + 5.0 * f),
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
            let tint = WHITE.lerp(Vec3::new(1.0, 0.6, 0.85), self.scatter.unit());
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
        // The gun's length from its trunnion to its muzzle.
        let reach = (Vec3::from(w.muzzle.to_f32())
            - w.pivot.map_or(Vec3::ZERO, |p| Vec3::from(p.to_f32())))
        .length();
        match grade {
            Grade::Bolt | Grade::Flak => self.bolt_fired(at, dir, rounds, flash, round_gap, time),
            Grade::Mortar => self.mortar_fired(at, dir, flash, time),
            Grade::Pinched => {
                // The ball pinches its plasma out as one jet (`Weapon::round_span`): a hard
                // red flash where it hangs, a pulse at the mouth as each round of the jet
                // leaves, the ball draining as it goes, and plasma thrown out down the line
                // of fire after it.
                let span = rounds as f32 * round_gap;
                self.charge_spent(blueprint, weapon, at, span.max(0.18), time);
                for k in 1..rounds {
                    let fall = 1.0 - k as f32 / rounds as f32;
                    let s = size * (0.5 + 0.7 * fall);
                    self.push_lit(
                        GLOW,
                        at + dir * size * 0.4,
                        Vec3::ZERO,
                        time + k as f32 * round_gap,
                        round_gap * 1.8,
                        (s * 0.8, s * 1.2),
                        HOT * (1.5 + 2.0 * fall),
                        0.0,
                    );
                }
                self.push_lit(
                    BURST,
                    at,
                    Vec3::ZERO,
                    time,
                    0.2,
                    (size * 1.0, size * 3.0),
                    RED * 4.0,
                    0.0,
                );
                self.push_lit(
                    GLOW,
                    at,
                    Vec3::ZERO,
                    time,
                    0.12,
                    (size * 1.3, size * 1.8),
                    HOT * 3.5,
                    0.0,
                );
                self.plasma_jet(at, dir, size, false, time);
                self.plasma_fx.guns.light(Glow {
                    pos: at,
                    color: RED * 220.0 * size,
                    range: size * 12.0,
                    start: time,
                    life: span.max(0.2),
                    pulse: 0.0,
                });
            }
            Grade::Fusion => {
                self.charge_spent(blueprint, weapon, at, 0.18, time);
                let s = size * 1.5;
                // Launch: a blinding white flash, a cone of fusion thrown out down the line
                // of fire, arcs snapping forward along it.
                self.push_lit(
                    GLOW,
                    at,
                    Vec3::ZERO,
                    time,
                    0.3,
                    (s * 2.8, s * 4.8),
                    WHITE * 12.0,
                    0.0,
                );
                self.push_lit(
                    BURST,
                    at,
                    Vec3::ZERO,
                    time,
                    0.35,
                    (s * 0.8, s * 3.0),
                    WHITE * 3.0,
                    1.0,
                );
                self.plasma_jet(at, dir, s, true, time);
                // What was left of the cage thrown off round the bore as globs of plasma.
                let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
                let up = side.cross(dir);
                for k in 0..14 {
                    let a = std::f32::consts::TAU * (k as f32 + self.scatter.unit() * 0.8) / 14.0;
                    let out = (side * a.cos() + up * a.sin() + dir * 0.4).normalize_or(dir);
                    let blob = s * (0.1 + 0.06 * self.scatter.unit());
                    let tint = WHITE.lerp(RED, self.scatter.unit() * 0.7);
                    let roll0 = self.scatter.unit();
                    self.push_lit(
                        GLOB,
                        at,
                        out * s * (4.0 + 3.0 * roll0),
                        time,
                        0.7,
                        (blob, blob * 0.3),
                        tint * 4.0,
                        0.0,
                    );
                }
                for _ in 0..7 {
                    let spread = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    ) * 0.45;
                    let reach = s * (3.5 + 5.0 * self.scatter.unit());
                    let late = self.scatter.unit() * 0.1;
                    self.push_lit(
                        ARC,
                        at,
                        (dir + spread).normalize_or(dir) * reach,
                        time + late,
                        0.14,
                        (s * 0.28, s * 0.28),
                        WHITE * 6.0,
                        0.0,
                    );
                }
                // Heat rising off the gun's back, behind its trunnion, either side.
                let flat = dir.with_z(0.0).normalize_or(Vec3::X);
                let across = flat.cross(Vec3::Z);
                let back = at - flat * reach * 1.25 - Vec3::Z * reach * 0.06;
                for k in 0..16 {
                    let side = if k % 2 == 0 { 1.0 } else { -1.0 };
                    let from = back
                        + across * side * reach * 0.19
                        + flat * self.scatter.signed() * reach * 0.1;
                    let when = time + 0.1 + k as f32 * 0.16 + self.scatter.unit() * 0.1;
                    let puff = reach * (0.05 + 0.03 * self.scatter.unit());
                    let roll0 = self.scatter.unit();
                    self.push_lit(
                        WAKE,
                        from,
                        Vec3::Z * (3.0 + 3.0 * roll0) + across * side * 1.5,
                        when,
                        0.9,
                        (puff, puff * 1.6),
                        RED.lerp(HOT, 0.3) * 1.6 * (1.0 - k as f32 / 20.0),
                        0.0,
                    );
                }
                self.plasma_fx.guns.light(Glow {
                    pos: at,
                    color: WHITE * 700.0 * s,
                    range: s * 18.0,
                    start: time,
                    life: 0.5,
                    pulse: 0.0,
                });
            }
        }
        true
    }

    /// The plasma a squeezed charge throws out down the line of fire `dir` as it lets a
    /// shot go: a cone of wake puffs driven forward, slowing and spreading, red for a
    /// Pinched bolt, white going pink for fusion.
    pub(super) fn plasma_jet(&mut self, at: Vec3, dir: Vec3, size: f32, fusion: bool, time: f32) {
        let (count, speed, life) = if fusion {
            (14, size * 9.0, 0.6)
        } else {
            (7, size * 8.0, 0.35)
        };
        for _ in 0..count {
            let spread = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 0.28;
            let v = (dir + spread).normalize_or(dir) * speed * (0.4 + 0.8 * self.scatter.unit());
            let puff = size * (0.25 + 0.2 * self.scatter.unit());
            let rgb = if fusion {
                WHITE.lerp(HOT, self.scatter.unit() * 0.6) * 3.0
            } else {
                RED.lerp(HOT, self.scatter.unit() * 0.4) * 3.0
            };
            let roll0 = self.scatter.unit();
            let roll1 = self.scatter.unit();
            self.push_lit(
                WAKE,
                at + dir * size * 0.5,
                v,
                time + roll0 * 0.05,
                life * (0.7 + 0.6 * roll1),
                (puff, puff * 1.5),
                rgb,
                if fusion { 0.8 } else { 0.0 },
            );
        }
    }

    /// A bolt has left a charge near `at`: when it was the salvo's last, the ball collapses
    /// over `life` seconds.
    pub(super) fn charge_spent(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        life: f32,
        time: f32,
    ) {
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
            life,
            (size, size * 0.15),
            rgb,
            if fusion { 1.0 } else { 0.0 },
        );
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
    pub(super) fn push_lit(
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
        self.push_lingering(kind, pos, vel, start, life, size, rgb, w, 0);
    }

    /// `push_lit` for a plasma puff (plasma_puffs.wgsl) that opens over `life` as it
    /// would and then lingers: it lives `1 + linger` times as long, its cooling slowing
    /// toward the end (the whole part of its seed). A lingering wake is also braked harder
    /// by the air, so it is thrown out faster to cover about the same ground.
    pub(super) fn push_lingering(
        &mut self,
        kind: f32,
        pos: Vec3,
        vel: Vec3,
        start: f32,
        life: f32,
        size: (f32, f32),
        rgb: Vec3,
        w: f32,
        linger: u8,
    ) {
        let p = Puff {
            appearance: [rgb.x, rgb.y, rgb.z, w],
            origin: pos.to_array(),
            opacity: 1.0,
            pos: pos.to_array(),
            start,
            vel: vel.to_array(),
            life: life * (1.0 + f32::from(linger)),
            params: [
                size.0,
                size.1,
                kind,
                self.scatter.unit().min(0.999) + f32::from(linger),
            ],
        };
        self.puffs.write(
            (self.puff_cursor * size_of::<Puff>()) as u64,
            bytemuck::bytes_of(&p),
        );
        self.puff_cursor = (self.puff_cursor + 1) % PUFF_RING;
    }
}

/// A Regency plasma shot's look as the mirror packs it for the sprite (`mirror::plasma_look`,
/// twice over in `_pad[0]` past one and its redness; sprites.wgsl `plasma_look`). Zero for
/// any other shot.
pub(super) fn drawn_look(p: &ProjectileInstance) -> u32 {
    if p._pad[0] < 2.5 || p.color & PROJECTILE_MISSILE != 0 {
        return 0;
    }
    ((p._pad[0] - 1.0) * 0.5).floor() as u32
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
