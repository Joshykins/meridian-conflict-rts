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

use super::star_core_fx::{LAVENDER, ROSE, WHITE};
use crate::gpu_consts::{fade_beam, puff, stain};
use crate::models::Crawl;
use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, PlasmaGrade, Trajectory, Weapon};
use mc_sim::mirror::{ProjectileInstance, StainInstance, UnitInstance, PROJECTILE_FADE_BEAM};
use std::mem::size_of;

use super::water_fx::PUFF_STEAM;
use super::{beam_score, rail_fx, Renderer, MAX_PROJECTILES, PUFF_SPARK, PUFF_TREE_SMOKE};

/// Fade-beam colour of a held beam and of a charge (sprites.wgsl).
pub(super) const HELD_BEAM: u32 = 7;
const CHARGE: u32 = 8;
/// Seconds a beam that stops being fed takes to go out.
const CUT: f32 = 0.12;
/// Metres apart the pools a beam glasses the ground with are laid, at the least, and as a
/// share of a pool's radius: a wide pool every couple of metres heats the same ground
/// many times over for nothing.
const GLASS_STEP: f32 = 2.4;
const GLASS_STEP_SHARE: f32 = 0.2;
/// Radius of the pools a beam glasses the ground with, in beam widths.
const GLASS_RADIUS: f32 = 2.8;
/// Furthest a beam's strike may have moved since it last glassed the ground for the
/// stretch between to be seared too: past it the beam jumped to a new mark.
const RAKE_GAP: f32 = 40.0;
/// Radius, in its pools' radii, of the plasma spreading over the ground where an Edict
/// Lance strikes.
const SPILL_RADIUS: f32 = 2.2;
/// How hot an Edict Lance runs the ground (`GroundMelt::melt`'s `peak`): molten red-orange,
/// not white-hot, under its red column.
const EDICT_MELT_PEAK: f32 = 0.65;
/// Seconds a glassed pool takes to crust over and cool.
const GLASS_COOL: f32 = 9.0;
/// The red filament an Edict Lance leaves along its line when it goes out: its width
/// against the column's, and the seconds it burns as it cools and breaks up.
const THREAD: f32 = 0.22;
const FILAMENT: f32 = 4.0;

/// An Edict Lance's filament: where its line ran, its width and when it went out.
struct Filament {
    from: Vec3,
    to: Vec3,
    width: f32,
    start: f32,
}

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
    /// The radius of the pools it glasses the ground with. An Edict Lance's is its
    /// `splash`, the ground whose trees it burns, not its column's width: the column is
    /// drawn far wider than what it burns.
    melt: f32,
    fusion: bool,
    /// Drawn as an Edict Lance's column, and melts the ground without digging
    /// (`Weapon::edict_beam`).
    edict: bool,
    /// When it was last fed, whether this tick's strike has come in, and whether it had
    /// struck anything before (a new beam starts at its first strike, not from nowhere).
    last: f32,
    struck: bool,
    new: bool,
    /// Where it last glassed the ground.
    glassed: Option<Vec2>,
    /// It last struck the sea: no plasma spills over the water.
    boiling: bool,
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
    /// The plasma spread over the ground at each Edict Lance's strike, drawn as stains.
    pub(super) spills: Vec<StainInstance>,
    /// The filaments Edict Lances left as they went out.
    filaments: Vec<Filament>,
    charges: Vec<Charge>,
    /// The direct-fire guns' charges and lights (`regency_guns_fx`).
    pub(super) guns: super::regency_guns_fx::RegencyGunFx,
    /// The Gravitic Seekers' lights, and a gravity crush's (`gravitic_fx`, `crush_fx`).
    pub(super) seekers: super::gravitic_fx::GraviticFx,
    /// Gravitic shock shells' trails, lights and lenses (`gravitic_shell_fx`).
    pub(super) shells: super::gravitic_shell_fx::GraviticShellFx,
    /// Cone weapons' wakes rolling out (`wake_fx`).
    pub(super) wakes: Vec<super::wake_fx::RollingWake>,
    /// Edict Lances' cores: the charge, the star and the light (`edict_lance_fx`).
    pub(super) cores: super::edict_lance_fx::EdictLances,
}

impl PlasmaFx {
    /// The blueprints. beam cores (`edict_lance_fx`), per blueprint in id order.
    pub(super) fn new(cores: Vec<Option<[f32; 4]>>) -> Self {
        PlasmaFx {
            cores: super::edict_lance_fx::EdictLances::new(cores),
            ..PlasmaFx::default()
        }
    }

    /// Forgets everything in flight; the blueprints' cores stay.
    pub(super) fn clear(&mut self) {
        let mut cores = std::mem::take(&mut self.cores);
        cores.clear();
        *self = PlasmaFx {
            cores,
            ..PlasmaFx::default()
        };
    }
}

/// A thrown plasma charge (`Weapon::plasma_grade` on a lobbed or curving gun): it charges at
/// its muzzle, and lands the way the cage letting go does. A gun laid flat
/// (`Weapon::flat_fire`) is a direct-fire gun whose shot arcs (`regency_guns_fx`).
fn thrown_plasma(weapon: &Weapon) -> bool {
    weapon.plasma_shot().is_some()
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
    let (pitch, yaw) = (0.035 * busy, -0.025 * busy * side);
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
        // Tracer width also applies to a held stream, alongside its existing flash scale.
        let width =
            (0.5 + w.damage.to_f32().max(1.0).sqrt() * 0.08) * w.flash.max(1.0) * w.tracer.max(0.1);
        let fusion = w.plasma_grade == Some(PlasmaGrade::PinchFusion);
        let edict = fusion && w.edict_beam;
        let melt = if edict {
            w.splash.to_f32().max(1.0)
        } else {
            width * GLASS_RADIUS
        };
        let start = muzzle - travel;
        let reach = travel.length() + 6.0;
        let fx = &mut self.plasma_fx;
        // The same gun's beam last tick, the one that left from nearest where this one
        // starts and not already fed this tick: a gun with twin beams (a strafe) fires
        // both each tick, one from each muzzle.
        let same = fx
            .held
            .iter_mut()
            .filter(|h| {
                h.owner == owner
                    && h.blueprint == blueprint
                    && h.weapon == weapon
                    && h.last < time
                    && h.from[1].distance(start) < reach
            })
            .min_by(|a, b| {
                a.from[1]
                    .distance(start)
                    .total_cmp(&b.from[1].distance(start))
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
                melt,
                fusion,
                edict,
                last: time,
                struck: false,
                new: true,
                glassed: None,
                boiling: false,
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
        // Into the sea it strikes the water, not the seabed: it ends at the surface and
        // boils it, and melts nothing under it.
        let muzzle = fx.held[i].from[1];
        let sea = (!on_unit).then(|| self.at_sea(at, 1.0)).flatten();
        let at = match sea {
            Some(water) if muzzle.z > water => muzzle.lerp(
                at,
                ((muzzle.z - water) / (muzzle.z - at.z).max(0.01)).min(1.0),
            ),
            _ => at,
        };
        let h = &mut self.plasma_fx.held[i];
        h.to[1] = at;
        if h.new {
            h.to[0] = at;
        }
        h.struck = true;
        let width = h.width;
        let melt = h.melt;
        let fusion = h.fusion;
        let edict = h.edict;
        // It glasses the ground where it lands, a pool at a time as the strike walks. A
        // beam raking fast over the ground (a strafe) lays the stretch it crossed since
        // its last pool too, so it leaves an unbroken line.
        let last = h.glassed;
        h.boiling = sea.is_some();
        if h.boiling {
            h.glassed = None;
            // An Edict Lance boils what it would melt on land, not its column's width.
            self.beam_boils(at, if edict { melt } else { width }, edict, time);
            return;
        }
        let step = GLASS_STEP.max(melt * GLASS_STEP_SHARE);
        let glass = !on_unit && last.is_none_or(|g| g.distance(at.truncate()) > step);
        if glass {
            h.glassed = Some(at.truncate());
        }
        let from = last.filter(|g| g.distance(at.truncate()) < RAKE_GAP);
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let burn = w.burn_ticks as f32 / mc_core::TICKS_PER_SECOND as f32;
        let bite = on_unit && super::regency_guns_fx::is_lance(w);
        if glass && burn > 0.0 && w.plasma_grade.is_some() {
            self.plasma_left_burning(from, at, width, burn, time);
        }
        if glass {
            // An Edict Lance runs the ground molten at once; any other glows it red at
            // first, and melts it as it drags on over ground it has already heated. One
            // that leaves it burning only sears it: the burning plasma is its mark.
            let peak = if edict {
                EDICT_MELT_PEAK
            } else if burn > 0.0 {
                (0.12 + 0.08 * width).min(0.5)
            } else {
                (0.25 + 0.12 * width).min(0.9)
            };
            if let Some(g) = from {
                let steps = (g.distance(at.truncate()) / step).floor() as usize;
                for k in 1..steps {
                    let p = g.lerp(at.truncate(), k as f32 / steps as f32);
                    self.ground_melt.melt(p, melt, peak, time, GLASS_COOL);
                }
            }
            self.ground_melt
                .melt(at.truncate(), melt, peak, time, GLASS_COOL);
            // The excavation lance digs as it glasses; the Edict Lance only melts.
            if fusion && !edict {
                self.impact_craters.dig(at.truncate(), width * 2.2, time);
            }
            self.push_puff(
                PUFF_TREE_SMOKE,
                at + Vec3::Z * 0.8,
                Vec3::Z * 2.5,
                time,
                2.2,
                (melt * 0.43, melt * 1.25),
            );
        }
        if edict {
            self.edict_outflow(at, melt, on_unit, time);
        }
        // A fusion lance biting into a hull (`regency_guns_fx/lance.rs`).
        if bite {
            let dir = (at - muzzle).normalize_or(Vec3::Z);
            self.lance_bite(at, dir, width, time);
        }
        // A fusion lance's bite on a hull is its own (`lance_bite`).
        if fusion && !edict && !bite {
            self.push_lit(
                puff::WARP_GLOW as f32,
                at + Vec3::Z * width,
                Vec3::ZERO,
                time,
                0.18,
                (width * 2.0, width * 3.4),
                ROSE * 8.0,
                0.0,
            );
            for _ in 0..4 {
                let spray = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    0.5 + self.scatter.unit(),
                )
                .normalize_or(Vec3::Z);
                self.push_lit(
                    puff::WARP_STREAK as f32,
                    at,
                    spray * 36.0,
                    time,
                    0.35,
                    (width * 0.25, width * 0.08),
                    LAVENDER * 5.0,
                    0.0,
                );
            }
            self.push_lit(
                puff::WARP_GLOW as f32,
                at,
                Vec3::ZERO,
                time,
                0.1,
                (width, width * 1.6),
                WHITE * 9.0,
                0.0,
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

    /// Where a held beam strikes the sea: the water flashes to steam round it, white puffs
    /// rolling up and out off the surface and a ring running out over the water. Every
    /// tick the beam holds.
    fn beam_boils(&mut self, at: Vec3, width: f32, edict: bool, time: f32) {
        let s = if edict { 1.6 } else { 1.0 };
        // An Edict Lance's strike is red-hot, as on the ground (`edict_outflow`).
        let hot = if edict {
            Vec3::new(1.0, 0.32, 0.18)
        } else {
            Vec3::new(1.0, 0.92, 0.85)
        };
        self.push_lit(
            puff::WARP_GLOW as f32,
            at + Vec3::Z * width * 0.3,
            Vec3::ZERO,
            time,
            0.12,
            (width * 1.1 * s, width * 1.5 * s),
            hot * 5.0,
            0.0,
        );
        for _ in 0..2 {
            let out =
                Vec3::new(self.scatter.signed(), self.scatter.signed(), 1.6).normalize_or_zero();
            let speed = 6.0 + self.scatter.unit() * 8.0;
            self.push_puff(
                PUFF_STEAM,
                at,
                out * speed * s,
                time,
                2.5,
                (width * 1.2 * s, width * 4.0 * s),
            );
        }
        if self.scatter.unit() < 0.25 {
            self.push_ripple(at, time, width * 6.0 * s, 1.6, 0.0, 0.6);
        }
    }

    /// Where an Edict Lance strikes: a tight red-hot spot with a deeper red bloom under it,
    /// and a crown of molten droplets thrown up and out in arcs that fall back and cool.
    /// Nothing hazy. Every tick the beam holds. `melt` is the radius it glasses.
    fn edict_outflow(&mut self, at: Vec3, melt: f32, on_unit: bool, time: f32) {
        self.push_lit(
            puff::WARP_GLOW as f32,
            at + Vec3::Z * melt * 0.3,
            Vec3::ZERO,
            time,
            0.12,
            (melt * 1.1, melt * 1.5),
            Vec3::new(1.0, 0.32, 0.18) * 5.0,
            0.0,
        );
        self.push_lit(
            puff::WARP_GLOW as f32,
            at + Vec3::Z * melt * 0.2,
            Vec3::ZERO,
            time,
            0.2,
            (melt * 2.2, melt * 2.6),
            Vec3::new(1.0, 0.14, 0.08) * 2.5,
            0.0,
        );
        // Droplets of melt: out in a ring and up, falling back under gravity.
        let up = if on_unit { 0.6 } else { 1.0 };
        for _ in 0..5 {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let out = 7.0 + self.scatter.unit() * 13.0;
            let vel = Vec3::new(
                a.cos() * out,
                a.sin() * out,
                (10.0 + self.scatter.unit() * 16.0) * up,
            );
            self.push_puff(PUFF_SPARK, at + Vec3::Z * 0.4, vel, time, 0.9, (0.34, 0.07));
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
            self.ground_melt
                .melt(at.truncate(), splash * 0.55, 0.6, start, GLASS_COOL);
        }
    }

    /// Writes this tick's held beams and charges among the fading beams, after
    /// `write_fade_beams`. `units`: this tick's, to carry each charge with its unit.
    pub(super) fn write_plasma_fx(&mut self, units: &[UnitInstance], time: f32) {
        let tick = self.tick_seconds.max(0.02);
        let fx = &mut self.plasma_fx;
        // An Edict Lance going out leaves its filament along the line it last held.
        let gone = |h: &Held| time >= h.last + tick * 1.5 + CUT;
        for h in fx.held.iter().filter(|h| h.edict && !h.new && gone(h)) {
            fx.filaments.push(Filament {
                from: h.from[1],
                to: h.to[1],
                width: h.width,
                start: h.last + tick * 1.5,
            });
        }
        fx.held.retain(|h| !gone(h));
        fx.filaments.retain(|f| time < f.start + FILAMENT);
        // One that still has throws to come waits a little longer for its shot.
        fx.charges
            .retain(|c| time < c.due + tick * if c.left > 1 { 3.0 } else { 1.0 });
        let mut out: Vec<ProjectileInstance> = Vec::new();
        fx.spills.clear();
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
            let mut beam = held_instance(HELD_BEAM, h.from, h.to, h.width, start, life);
            if h.edict {
                beam.aim[3] = fade_beam::HELD_EDICT;
            } else if h.fusion {
                beam.aim[3] = fade_beam::HELD_FUSION;
            }
            out.push(beam);
            if h.edict && h.struck && !h.boiling {
                // Plasma spreading out over the ground from the strike (ground.wgsl).
                let fade = 1.0 - ((time - h.last - tick * 1.5) / CUT).clamp(0.0, 1.0);
                fx.spills.push(StainInstance {
                    pos: h.to[1].truncate().to_array(),
                    radius: h.melt * SPILL_RADIUS,
                    strength_seed: (fade * 255.0) as u32 | stain::SPILL,
                });
            }
        }
        // A Pinched bolt's trail (`aim.w` 0): red, cooling and breaking up.
        out.extend(fx.filaments.iter().map(|f| ProjectileInstance {
            prev_pos: f.from.to_array(),
            color: PROJECTILE_FADE_BEAM | fade_beam::PLASMA_TRAIL,
            pos: f.to.to_array(),
            size: f.width * THREAD,
            wake: f.start,
            plasma: FILAMENT,
            _pad: [0.0; 2],
            aim: [0.0; 4],
            prev_aim: [0.0; 4],
        }));
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
