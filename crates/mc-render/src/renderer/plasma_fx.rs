//! The Regency's plasma weapons as they are drawn (docs/STYLE.md "The Regency suite"):
//!
//! - **A held beam** (`Weapon::beam`). The sim fires it every tick it bears, a hitscan shot
//!   each time; this draws one steady stream from the muzzle to what it strikes instead of
//!   a flash a tick, its two ends gliding over each tick as the units at either end are
//!   drawn to (sprites.wgsl fade beam 7), and glasses the ground where it lands: molten
//!   pools that glow and crust over (`BoreFx::melt`).
//! - **A thrown plasma charge charging** (`SimEvent::WeaponCharging` on a thrown plasma
//!   weapon, the battle scorpion's Gravitic Bombs): a red-white charge swelling at the
//!   muzzle over the charge, carried with the unit and held between the claw's fingers as
//!   the claw is drawn (fade beam 8, a point). Between the throws of a salvo it forms again.
//! - **A thrown plasma charge landing**: the cage lets go at once, a white-hot flash over
//!   the gun's own blast, molten spatter, and a glassed scorch that glows and cools.
//!
//! The direct-fire plasma guns (Plasmeric bolts and flak, Pinched-plasmeric,
//! Pinch-fusion) charge, fire and strike in `regency_guns_fx.rs`; every plasma shot in
//! flight is drawn by sprites.wgsl (`plasma_look`).
//!
//! Presentation only; the renderer's own clock.

use crate::models::Crawl;
use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, Trajectory, Weapon};
use mc_sim::mirror::{ProjectileInstance, UnitInstance, PROJECTILE_FADE_BEAM};
use std::mem::size_of;

use super::{beam_score, rail_fx, Renderer, MAX_PROJECTILES, PUFF_SPARK, PUFF_TREE_SMOKE};

/// Fade-beam colour of a held beam and of a charge (sprites.wgsl).
pub(super) const HELD_BEAM: u32 = 7;
const CHARGE: u32 = 8;
/// Seconds a beam that stops being fed takes to go out.
const CUT: f32 = 0.12;
/// Metres apart the pools a beam glasses the ground with are laid.
const GLASS_STEP: f32 = 2.4;
/// Seconds a glassed pool takes to crust over and cool.
const GLASS_COOL: f32 = 9.0;

/// One held beam: the same gun (owner, blueprint, weapon) from one tick to the next.
struct Held {
    owner: u8,
    blueprint: BlueprintId,
    weapon: u8,
    /// The muzzle as drawn at the start of the tick, and where it gets to by its end.
    from: [Vec3; 2],
    /// Where it strikes, the same.
    to: [Vec3; 2],
    /// Down the barrel, and its reach: where it runs to on a tick it strikes nothing.
    dir: Vec3,
    range: f32,
    width: f32,
    /// When it was last fed, whether this tick's strike has come in, and whether it had
    /// struck anything before (a new beam starts at its first strike, not from nowhere).
    last: f32,
    struck: bool,
    new: bool,
    /// Where it last glassed the ground.
    glassed: Option<Vec2>,
}

/// A thrown charge swelling at its muzzle.
struct Charge {
    unit: u32,
    owner: u8,
    blueprint: BlueprintId,
    weapon: u8,
    start: f32,
    due: f32,
    /// Throws left in its salvo: it forms again between them.
    left: u8,
    /// Where it was last drawn, to tell whose throw a shot is.
    at: Vec3,
}

#[derive(Default)]
pub(super) struct PlasmaFx {
    held: Vec<Held>,
    charges: Vec<Charge>,
    /// The direct-fire guns' charges and lights (`regency_guns_fx`).
    pub(super) guns: super::regency_guns_fx::RegencyGunFx,
}

impl PlasmaFx {
    pub(super) fn clear(&mut self) {
        *self = PlasmaFx::default();
    }
}

/// A thrown plasma charge (`Weapon::plasma_grade` on a lobbed or curving gun): it charges at
/// its muzzle, and lands the way the cage letting go does. A gun laid flat
/// (`Weapon::flat_fire`) is a direct-fire gun whose shot arcs (`regency_guns_fx`).
fn thrown_plasma(weapon: &Weapon) -> bool {
    weapon.plasma_grade.is_some()
        && ((weapon.trajectory == Trajectory::Ballistic && !weapon.flat_fire) || weapon.curve.0 > 0)
        && !weapon.missile
}

/// Where a charge held in a pincer is drawn, in the unit's frame: `local` (the weapon's
/// muzzle, between the fingers as the model stands) carried as `entity.wgsl` `claw_pose`
/// carries the claw while the unit is busy (raised and turned in about its shoulder), and
/// down with the body as it sets itself (`crawl_set`). `t`: 0 at the tick's start, 1 its end.
fn held_in_claw(crawl: &Crawl, u: &UnitInstance, local: Vec3, t: f32) -> Vec3 {
    let Some([shoulder, _]) = crawl.claw else {
        return local;
    };
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    let brace = lerp(u.prev_recoil, u.recoil);
    // As `entity.wgsl` `crawl_busy`.
    let busy = (lerp(u.arm_pitch[0], u.arm_pitch[1]).abs() * 6.0
        + lerp(u.arm_pitch[2], u.arm_pitch[3]).abs() * 6.0
        + brace * 4.0
        + lerp(u.prev_deploy, u.deploy).clamp(0.0, 1.0) * 4.0)
        .clamp(0.0, 1.0);
    let side = if local.y > 0.0 { 1.0 } else { -1.0 };
    let shoulder = Vec3::from(shoulder) * Vec3::new(1.0, side, 1.0);
    let (pitch, yaw) = (0.16 * busy, -0.1 * busy * side);
    let q = local - shoulder;
    let (s, c) = pitch.sin_cos();
    let q = Vec3::new(q.x * c - q.z * s, q.y, q.x * s + q.z * c);
    let (s, c) = yaw.sin_cos();
    let q = Vec3::new(q.x * c - q.y * s, q.x * s + q.y * c, q.z);
    let sink = crawl.joints[0][0][2] * 0.0625 * brace.clamp(0.0, 1.0);
    q + shoulder - Vec3::Z * sink
}

impl Renderer {
    /// A held beam fired this tick (`ShotFired` of a `beam` weapon): `muzzle` is the sim's,
    /// `travel` the gun's own way this tick (it is drawn that far short at the tick's start).
    pub(super) fn beam_fired(
        &mut self,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
        muzzle: Vec3,
        travel: Vec3,
        dir: Vec3,
        time: f32,
    ) {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let range = w.range_max.to_f32();
        let width = 0.5 + w.damage.to_f32().max(1.0).sqrt() * 0.08;
        let start = muzzle - travel;
        let reach = travel.length() + 6.0;
        let fx = &mut self.plasma_fx;
        let same = fx.held.iter_mut().find(|h| {
            h.owner == owner
                && h.blueprint == blueprint
                && h.weapon == weapon
                && h.from[1].distance(start) < reach
        });
        match same {
            Some(h) => {
                h.from = [start, muzzle];
                h.to[0] = h.to[1];
                h.dir = dir;
                h.last = time;
                h.struck = false;
                h.new = false;
            }
            None => fx.held.push(Held {
                owner,
                blueprint,
                weapon,
                from: [start, muzzle],
                to: [muzzle + dir * range; 2],
                dir,
                range,
                width,
                last: time,
                struck: false,
                new: true,
                glassed: None,
            }),
        }
    }

    /// Where this tick's shot of a held beam struck (its `Impact`): `at` where the sim has
    /// it, `on_unit` whether that was a hull rather than the ground.
    pub(super) fn beam_struck(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        on_unit: bool,
        time: f32,
    ) {
        let fx = &mut self.plasma_fx;
        let best = fx
            .held
            .iter()
            .enumerate()
            .filter(|(_, h)| h.blueprint == blueprint && h.weapon == weapon && !h.struck)
            .filter_map(|(i, h)| beam_score(h.from[1], h.dir, h.range, at).map(|s| (i, s)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, _)) = best else {
            return;
        };
        let h = &mut fx.held[i];
        h.to[1] = at;
        if h.new {
            h.to[0] = at;
        }
        h.struck = true;
        let width = h.width;
        // It glasses the ground where it lands, a pool at a time as the strike walks.
        let glass = !on_unit
            && h.glassed
                .is_none_or(|g| g.distance(at.truncate()) > GLASS_STEP);
        if glass {
            h.glassed = Some(at.truncate());
        }
        if glass {
            self.bore_fx
                .melt(at.truncate(), width * 2.2, time, GLASS_COOL);
            self.push_puff(
                PUFF_TREE_SMOKE,
                at + Vec3::Z * 0.8,
                Vec3::Z * 2.5,
                time,
                2.2,
                (width * 1.2, width * 3.5),
            );
        }
        // Molten spatter thrown up where it bites, every tick.
        for _ in 0..2 {
            let spray = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.6 + self.scatter.unit(),
            )
            .normalize_or_zero();
            let speed = 14.0 + self.scatter.unit() * 18.0;
            self.push_puff(PUFF_SPARK, at, spray * speed, time, 0.5, (0.22, 0.08));
        }
    }

    /// A lobbed plasma charge began charging (`WeaponCharging`).
    pub(super) fn plasma_charging(
        &mut self,
        unit: u32,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
        pos: Vec3,
        time: f32,
    ) {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        if !thrown_plasma(w) || w.charge_ticks == 0 {
            return;
        }
        let due = time + w.charge_ticks as f32 * self.tick_seconds;
        let left = w.salvo.max(1);
        self.plasma_fx
            .charges
            .retain(|c| !(c.unit == unit && c.blueprint == blueprint && c.weapon == weapon));
        self.plasma_fx.charges.push(Charge {
            unit,
            owner,
            blueprint,
            weapon,
            start: time,
            due,
            left,
            at: pos,
        });
    }

    /// A thrown plasma charge left its muzzle (`ShotFired` at `at`): the charge it was is
    /// spent, and forms again for the next throw of the salvo, if there is one. A small hard
    /// ring where the cage snaps shut round it.
    pub(super) fn plasma_thrown(
        &mut self,
        owner: u8,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        time: f32,
    ) {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        if !thrown_plasma(w) {
            return;
        }
        let gap = w.salvo_delay_ticks.max(1) as f32 * self.tick_seconds;
        let size = w.splash.to_f32().max(4.0);
        let fx = &mut self.plasma_fx;
        let mine = fx
            .charges
            .iter()
            .enumerate()
            .filter(|(_, c)| c.owner == owner && c.blueprint == blueprint && c.weapon == weapon)
            .map(|(i, c)| (i, c.at.distance(at)))
            .filter(|(_, d)| *d < 20.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = mine {
            let c = &mut fx.charges[i];
            c.left = c.left.saturating_sub(1);
            if c.left == 0 {
                fx.charges.swap_remove(i);
            } else {
                c.start = time;
                c.due = time + gap;
            }
        }
        self.push_shockwave(at.to_array(), time, size * 0.5, 0.2, 0.35, 1.0, Vec3::ZERO);
    }

    /// A thrown plasma charge landed (its `Impact`): the gun's own blast is drawn as any
    /// shell's; this adds the cage letting go. A direct-fire plasma gun's strike is all its
    /// own (`regency_guns_fx`).
    pub(super) fn plasma_landed(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        on_unit: bool,
        start: f32,
    ) {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        if !thrown_plasma(w) {
            return;
        }
        let splash = w.splash.to_f32().max(2.0);
        // The bind breaking: a sharp white-hot flash, over at once.
        self.push_effect(
            at.to_array(),
            start,
            splash * 0.9,
            0.12,
            rail_fx::RAIL_FLASH,
            0.9,
        );
        for _ in 0..10 {
            let spray = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.4 + self.scatter.unit(),
            )
            .normalize_or_zero();
            let speed = 20.0 + self.scatter.unit() * 30.0;
            self.push_puff(PUFF_SPARK, at, spray * speed, start, 0.8, (0.3, 0.1));
        }
        if !on_unit {
            self.bore_fx
                .melt(at.truncate(), splash * 0.55, start, GLASS_COOL);
        }
    }

    /// Writes this tick's held beams and charges among the fading beams, after
    /// `write_fade_beams`. `units`: this tick's, to carry each charge with its unit.
    pub(super) fn write_plasma_fx(&mut self, units: &[UnitInstance], time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let fx = &mut self.plasma_fx;
        fx.held.retain(|h| time < h.last + tick * 1.5 + CUT);
        // One that still has throws to come waits a little longer for its shot.
        fx.charges
            .retain(|c| time < c.due + tick * if c.left > 1 { 3.0 } else { 1.0 });
        let mut out: Vec<ProjectileInstance> = Vec::new();
        for h in &mut fx.held {
            let fed = time - h.last < tick * 0.5;
            if fed && !h.struck {
                // Nothing struck this tick: it runs out to its reach.
                h.to[1] = h.from[1] + h.dir * h.range;
                if h.new {
                    h.to[0] = h.to[1];
                }
            }
            // A live beam never fades; one no longer fed goes out quickly.
            let (start, life) = if time - h.last < tick * 1.5 {
                (time, 0.3)
            } else {
                (h.last + tick * 1.5, CUT)
            };
            out.push(held_instance(HELD_BEAM, h.from, h.to, h.width, start, life));
        }
        let legs = &self.legs;
        for c in &mut fx.charges {
            let Some(u) = units.iter().find(|u| u.unit_id == c.unit) else {
                continue;
            };
            let w = &self.blueprints.unit(c.blueprint).weapons[c.weapon as usize];
            let muzzle = Vec3::from(w.muzzle.to_f32());
            let crawl = legs
                .get(c.blueprint.0 as usize)
                .copied()
                .flatten()
                .and_then(|l| l.crawl);
            let at = |pos: [f32; 3], heading: f32, t: f32| {
                let local = crawl.map_or(muzzle, |cr| held_in_claw(&cr, u, muzzle, t));
                let (s, co) = heading.sin_cos();
                Vec3::from(pos)
                    + Vec3::new(
                        local.x * co - local.y * s,
                        local.x * s + local.y * co,
                        local.z,
                    )
            };
            let (then, now) = (
                at(u.prev_pos, u.prev_heading, 0.0),
                at(u.pos, u.heading, 1.0),
            );
            c.at = now;
            // Swells over the charge; the flicker is the shader's.
            let grown = ((time - c.start) / (c.due - c.start).max(0.01)).clamp(0.0, 1.0);
            let size = w.splash.to_f32().max(4.0) * (0.1 + 0.25 * grown);
            out.push(held_instance(
                CHARGE,
                [then, now],
                [then, now],
                size,
                time,
                0.3,
            ));
        }
        self.push_projectiles(&out);
    }

    /// Appends drawn-only projectiles (held beams, charges) after this tick's own, as far
    /// as the buffer holds.
    pub(super) fn push_projectiles(&mut self, out: &[ProjectileInstance]) {
        let bytes = size_of::<ProjectileInstance>();
        for inst in out {
            let i = self.projectile_count as usize;
            if i >= MAX_PROJECTILES {
                break;
            }
            self.projectiles
                .write((i * bytes) as u64, bytemuck::bytes_of(inst));
            self.projectile_count += 1;
        }
    }
}

/// A fading beam whose ends glide over the tick (sprites.wgsl): `prev_pos` and `pos` where
/// its tail and head are drawn at the tick's start, `aim` and `prev_aim` where they get to.
pub(super) fn held_instance(
    kind: u32,
    from: [Vec3; 2],
    to: [Vec3; 2],
    width: f32,
    start: f32,
    life: f32,
) -> ProjectileInstance {
    ProjectileInstance {
        prev_pos: from[0].to_array(),
        color: PROJECTILE_FADE_BEAM | kind,
        pos: to[0].to_array(),
        size: width,
        wake: start,
        plasma: life,
        _pad: [0.0; 2],
        aim: from[1].extend(0.0).to_array(),
        prev_aim: to[1].extend(0.0).to_array(),
    }
}
