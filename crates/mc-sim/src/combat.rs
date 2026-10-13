//! Targeting, weapons, projectiles, damage and death.
//!
//! The read-only searches (target acquisition, projectile sweeps) run in
//! parallel over fixed-size row chunks and return per-chunk results that are
//! applied in chunk order. Everything that mutates state or draws random
//! numbers runs sequentially in row order.

use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::*;
use crate::target_pick::MAP_GUN_REACH;
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::{cat, Trajectory, Weapon, MAX_WEAPONS};

mod step;
mod targeting;

/// How far forward of the Thunderhead's origin its cannon's mount yaws, for a hull of
/// collision `radius`: the breech, inside the nose, 7 m on the 12 m model as authored.
/// The model's turret pivot (`models::aster::air`) matches and scales with the hull.
fn assault_gun_pivot_x(radius: Fx) -> Fx {
    radius * 7 / 12
}
const DT: i32 = TICKS_PER_SECOND as i32;
pub(crate) const CHUNK: usize = 256;
/// Where `weapon` of `row` is heard and seen charging: over the middle of the hull, or a
/// group's gun (`house`) over its own house's pivot, which may stand a kilometre off it.
fn charging_point(units: &Units, row: usize, weapon: &Weapon, house: bool) -> FxVec3 {
    let at = match weapon.pivot.filter(|_| house) {
        Some(p) => units.pos[row] + p.xy().rotate(units.heading[row]),
        None => units.pos[row],
    };
    at.extend(units.z[row] + weapon.muzzle.z)
}

/// Turret must be within this of the firing solution to shoot (~2 degrees).
const AIM_TOLERANCE: u16 = 364;
/// Where an idle land AA gun rests: raised toward the sky it guards.
const IDLE_SKY_PITCH: Angle = Angle::from_degrees(60);
/// Fixed strafing gun may correct at most eight degrees from the pitched nose.
const ASSAULT_GUN_CONE: Angle = Angle::from_degrees(8);
/// Highest over a ground mark's foot an aircraft aims (`air_strike_z`).
const AIR_STRIKE_HEIGHT: Fx = Fx::from_int(12);

/// The height an aircraft's guns and rockets aim at on a ground or sea mark standing at
/// `z`, `height` tall: its middle, but no higher than [`AIR_STRIKE_HEIGHT`] over its foot.
/// An aircraft attacks along its glide, nose down, and a strafing gun fixed to the nose
/// could never lay up onto the middle of a giant (the Behemoth's is 240 m up): it never
/// fired at one at all. It strikes the legs and the hull's foot instead.
fn air_strike_z(z: Fx, height: Fx) -> Fx {
    z + (height / 2).min(AIR_STRIKE_HEIGHT)
}
/// Steepest a warship pitches its hull to lay a spinal gun (`spinal_gun`): nose down, and up.
pub const HULL_DIVE_DEGREES: i32 = 40;
const HULL_DIVE: Angle = Angle::from_degrees(HULL_DIVE_DEGREES as _);
const HULL_CLIMB: Angle = Angle::from_degrees(15);

/// Whether weapon `w` of `bp` is a warship's spinal gun: fixed along the keel and laid by
/// turning and pitching the whole hull, so `Units::arm_pitch[_][0]` is the hull's pitch.
/// A lift ship's slot 0 is its flight pitch instead (`movement.rs`).
pub(crate) fn spinal_gun(bp: &mc_data::UnitBlueprint, w: usize, weapon: &Weapon) -> bool {
    w == 0
        && bp.is_capital_ship()
        && bp.transport.is_none()
        && weapon.turret_turn == 0
        && !weapon.guided
        && !weapon.missile
        && !weapon.vertical_launch
}

/// The hull pitch that lays a spinal gun, its bore `bore` metres above the hull's origin
/// at `from`, on the point `to`: the line of sight, tipped by the angle the bore's offset
/// subtends so the bore's own line runs through the point.
fn spinal_pitch(from: FxVec3, bore: Fx, to: FxVec3) -> Angle {
    let want = spinal_pitch_steps(from, bore, to);
    Angle(want.clamp(-(HULL_DIVE.0 as i32), HULL_CLIMB.0 as i32) as i16 as u16)
}

/// [`spinal_pitch`] before the hull's limits, in signed angle steps.
fn spinal_pitch_steps(from: FxVec3, bore: Fx, to: FxVec3) -> i32 {
    let run = (to.xy() - from.xy()).length();
    let sight = FxVec2::new(run, to.z - from.z);
    let reach = sight.length().max(Fx::ONE);
    // Radians to angle steps: 65536 / tau.
    let offset = (bore / reach * Fx::from_int(10430)).round_int();
    Angle::ZERO.delta_to(sight.angle()) as i32 - offset
}

/// Whether a spinal gun, its bore `bore` over the hull's origin at `from`, can be laid on
/// `to` at all: a mark closer in under the hull than the steepest dive (`HULL_DIVE`)
/// is in its dead zone, and left to the turrets.
fn spinal_bears(from: FxVec3, bore: Fx, to: FxVec3) -> bool {
    spinal_pitch_steps(from, bore, to) >= -(HULL_DIVE.0 as i32)
}

/// Whether a gun house with a depression limit (`Weapon::depression`) can dip to `elevation`
/// off its deck (`house_lay`): a mark in under the hull, steeper than that, is out of reach.
fn dips_to(weapon: &Weapon, elevation: Angle) -> bool {
    weapon.depression.0 == 0
        || Angle::ZERO.delta_to(elevation) as i32 >= -(weapon.depression.0 as i32)
}

/// The yaw off the nose and elevation off the deck that lay a gun house of `weapon` on
/// `to`, on a hull at `pos`, `z`, facing `heading` and pitched `hull` (a warship laying its
/// spinal gun, `hull_pitched`). The pivot rides the pitch, and the house turns and
/// elevates in the deck's frame, as the entity shader draws it: laid in the level world
/// plane instead, the guns of a hull pitched 20 degrees pointed as far off their marks.
fn house_lay(
    pos: FxVec2,
    z: Fx,
    heading: Angle,
    hull: Angle,
    weapon: &Weapon,
    to: FxVec3,
) -> (Angle, Angle) {
    let p = crate::world::pitched(
        weapon.pivot.unwrap_or(weapon.muzzle),
        Some(FxVec3::ZERO),
        hull,
    );
    let delta = to - (pos + p.xy().rotate(heading)).extend(z + p.z);
    let flat = delta.xy().rotate(Angle::ZERO - heading).extend(delta.z);
    let deck = crate::world::pitched(flat, Some(FxVec3::ZERO), Angle::ZERO - hull);
    (
        deck.xy().angle(),
        FxVec2::new(deck.xy().length(), deck.z).angle(),
    )
}

/// How far out, level, the dead zone under a gun house with a depression limit of
/// `depression` degrees reaches when its pivot is `drop` metres above the mark. The range
/// rings draw it. Presentation only, in floats.
#[expect(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    reason = "presentation: the range rings draw it"
)]
pub fn depression_dead_zone(drop: f32, depression: f32) -> f32 {
    if drop <= 0.0 || depression <= 0.0 {
        return 0.0;
    }
    drop / depression.to_radians().tan()
}

/// How far out, level, a spinal gun's dead zone reaches when its hull's origin is `drop`
/// metres above the mark and its bore `bore` above the origin (`spinal_bears`): the range
/// rings draw it. Presentation only, in floats.
#[expect(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    reason = "presentation: the range rings draw it"
)]
pub fn spinal_dead_zone(drop: f32, bore: f32) -> f32 {
    if drop <= 0.0 {
        return 0.0;
    }
    let dive = (HULL_DIVE_DEGREES as f32).to_radians();
    let mut run = drop / dive.tan();
    for _ in 0..4 {
        let tip = (bore / run.hypot(drop)).min(dive * 0.9);
        run = drop / (dive - tip).tan();
    }
    run
}
/// Ballistic gravity, metres per tick squared. Stronger than Earth's so shells arc visibly.
pub(crate) const GRAVITY: Fx = Fx::ratio(40, (DT * DT) as i64);

/// A shot's aim error, angle steps across and along (or up): a point drawn evenly from a
/// disc of radius `spread`, so the misses scatter in a circle round the aim point rather
/// than along a line or over a square.
fn aim_error(rng: &mut mc_core::Rng, spread: u16) -> (i32, i32) {
    let s = spread as i32;
    loop {
        let x = rng.below(s as u32 * 2 + 1) as i32 - s;
        let y = rng.below(s as u32 * 2 + 1) as i32 - s;
        if x * x + y * y <= s * s {
            return (x, y);
        }
    }
}

/// A storm seed drifts on, while it grows, as far as the aircraft flew in this share of
/// that time (`cloud_seed.rs`): one third, so it is left behind and slows to a stop.
const CLOUD_CARRY: i32 = 3;

/// Ticks a ballistic shell is given past its flight to its mark (`Projectiles::ticks_left`),
/// so one that falls short of the ground there still comes down: a shell's `ticks_left`
/// less this is the ticks before it lands.
pub(crate) const BALLISTIC_OVERRUN: i32 = 20;

/// Ticks a ballistic shell spends in the air to cover `dist`, including `loft`.
fn ballistic_ticks(dist: Fx, weapon: &Weapon) -> i32 {
    let step = weapon.projectile_speed / DT;
    (dist / step).ceil_int().max(1) + weapon.loft_ticks as i32
}

/// Lead airborne targets using their actual three-dimensional travel. Six
/// refinements account for the extra distance to the intercept, including altitude.
fn direct_air_aim(
    origin: FxVec3,
    target: FxVec3,
    velocity: FxVec3,
    turn: Angle,
    step: Fx,
) -> FxVec3 {
    let mut aim = target;
    for _ in 0..6 {
        let ticks = (aim - origin).length() / step.max(Fx::EPSILON);
        // A shot is first swept on the tick it is fired, before the target has
        // moved again, and the target stands still through each sweep: one that
        // arrives in its nth sweep meets the target n - 1 moves on. Leading by
        // the unrounded flight time instead puts fast jets half a tick of their
        // own flight off the line, wider than a light aircraft.
        let moves = Fx::from_int(ticks.ceil_int() - 1).max(Fx::ZERO);
        aim = target + turning_lead(velocity, turn, moves);
    }
    aim
}

/// Where a target flying `velocity` a tick, and turning `turn` a tick, is `ticks` on.
/// A jet circling at speed curves off a straight lead by more than its own size
/// before a gun's rounds arrive.
fn turning_lead(velocity: FxVec3, turn: Angle, ticks: Fx) -> FxVec3 {
    if turn == Angle::ZERO {
        return velocity * ticks;
    }
    let whole = ticks.floor_int().clamp(0, 64);
    let mut v = velocity.xy();
    let mut d = FxVec2::ZERO;
    for _ in 0..whole {
        v = v.rotate(turn);
        d += v;
    }
    d += v.rotate(turn) * (ticks - Fx::from_int(whole)).max(Fx::ZERO);
    d.extend(velocity.z * ticks)
}

/// World-space launch pitch that lands a ballistic shell on a target `rise` higher, `dist` away.
fn ballistic_launch_pitch(dist: Fx, rise: Fx, weapon: &Weapon) -> Angle {
    let n = ballistic_ticks(dist, weapon);
    let drop = GRAVITY * (n * (n + 1) / 2);
    crate::world::pitch_to_limit(dist, rise + drop, crate::world::BALLISTIC_PITCH_LIMIT)
}

/// Vertical speed of a cold launch, metres per tick. Constant gravity for the
/// whole toss: it leaves the tube, slows evenly, crests, and is already
/// falling when the motor lights. No coast and no hover.
fn cold_lob_speed(age: u16, cold: u16) -> Fx {
    // Twice a shell's pull, so a short vertical toss still crests in view.
    let g = GRAVITY * 2;
    let coast = cold.saturating_sub(3).max(1);
    g * (coast as i32 - age as i32)
}

/// Extra rack pitch from a flat rest pose up to a steep loft.
fn missile_rack_pitch(weapon: &Weapon) -> Angle {
    // Raised to its launch angle (`rake`) where the data gives one.
    let loft = if weapon.rake.0 > 0 {
        weapon.rake
    } else {
        Angle::from_degrees(50)
    };
    let Some(pivot) = weapon.pivot else {
        return loft;
    };
    let rest = crate::world::pitch_to_limit(
        weapon.muzzle.x - pivot.x,
        weapon.muzzle.z - pivot.z,
        crate::world::BALLISTIC_PITCH_LIMIT,
    );
    let extra = rest.delta_to(loft);
    Angle(extra.clamp(
        -crate::world::BALLISTIC_PITCH_LIMIT,
        crate::world::BALLISTIC_PITCH_LIMIT,
    ) as u16)
}

/// Extra tube pitch (from the rest pose) that matches the lob from the muzzle at that pitch.
fn ballistic_tube_pitch(dist: Fx, aim_z: Fx, unit_z: Fx, weapon: &Weapon, pivot: FxVec3) -> Angle {
    let limit = crate::world::BALLISTIC_PITCH_LIMIT;
    let rest =
        crate::world::pitch_to_limit(weapon.muzzle.x - pivot.x, weapon.muzzle.z - pivot.z, limit);
    let mut extra = Angle::ZERO;
    for _ in 0..6 {
        let at = crate::world::pitched(weapon.muzzle, Some(pivot), extra);
        let world = ballistic_launch_pitch(dist, aim_z - (unit_z + at.z), weapon);
        extra = Angle(rest.delta_to(world).clamp(-limit, limit) as u16);
    }
    extra
}

/// How fast slot 0 of `arm_pitch` comes round for weapon `w`: a spinal gun's is the whole
/// hull's pitch, which swings at the ship's turn rate; anything else at half its gun's.
fn hull_pitch_rate(bp: &mc_data::UnitBlueprint, w: usize, weapon: &Weapon) -> u16 {
    match bp.motion {
        Some(m) if spinal_gun(bp, w, weapon) => m.turn_rate,
        _ => weapon.turret_turn / 2,
    }
}

/// Whether `bp`'s hull pitches to lay a spinal gun (its first weapon is one).
pub(crate) fn hull_pitched(bp: &mc_data::UnitBlueprint) -> bool {
    bp.weapons.first().is_some_and(|w| spinal_gun(bp, 0, w))
}

/// Ticks a bomb let go `height` over its mark takes to get there.
fn bomb_fall(height: Fx) -> Fx {
    let height = height.max(Fx::ONE);
    // Gravity is applied before each position step.
    (((Fx::ONE + height * 8 / GRAVITY).sqrt() - Fx::ONE) / 2).max(Fx::ONE)
}

/// Whether `weapon` can be fired at a point on the ground (`AttackGround`, `Bombard`).
/// A guided missile off a rail flies straight at the point with nothing to home on; one
/// launched upright (vertical or cold) needs a unit to turn it over, so it cannot,
/// unless it flies a high arc (`Weapon::apogee`) or skims (`Weapon::skim`) and turns
/// over onto the point by itself (`naval_arms.rs`). Interceptor tubes shoot only at
/// torpedoes.
pub(crate) fn hits_ground(weapon: &Weapon) -> bool {
    !weapon.intercepts
        && !(weapon.guided
            && (weapon.vertical_launch || weapon.cold_launch_ticks > 0)
            && weapon.apogee <= Fx::ZERO
            && weapon.skim <= Fx::ZERO)
        && weapon.target_mask & (cat::LAND | cat::NAVAL | cat::STRUCTURE) != 0
}

/// Whether weapon `weapon` of a unit of `bp` shells the ground its orders name. On a
/// `Strike` only a giant's storm bore fires there (a unit without one fires everything,
/// as on `AttackGround`); its other guns keep to their own targets.
pub(crate) fn takes_ground(bp: &mc_data::UnitBlueprint, weapon: &Weapon, strike: bool) -> bool {
    let storm = |w: &Weapon| w.bore.is_some_and(|b| b.storm.is_some());
    hits_ground(weapon) && (!strike || storm(weapon) || !bp.weapons.iter().any(storm))
}

/// What a weapon is aiming at: a unit, or a point on the ground.
#[derive(Clone, Copy)]
struct Mark {
    unit: Option<usize>,
    pos: FxVec2,
    /// Bottom of the unit, or the ground.
    z: Fx,
    height: Fx,
    radius: Fx,
    air: bool,
    /// Last tick's displacement, climb included.
    moved: FxVec3,
    /// Last tick's turn, for a fixed-wing aircraft whose path follows its nose.
    turn: Angle,
    /// Heading times speed, per tick: how a shell leads a ground target.
    lead: FxVec2,
    /// `Bombard`: shots fall anywhere within this of `pos`.
    scatter: Fx,
}

/// Angle steps a rotary gun's barrels turn in a tick at full spin: a turn and a half a second.
pub(crate) const SPIN_TOP: u16 = 9830;

/// Signed angle steps from `turn` back to the nearest turn with one of `barrels` barrels
/// at the top of its cluster. The model sets its barrels half a spacing off the top at
/// rest, so the tops fall on odd multiples of half the spacing.
pub(crate) fn barrel_off_top(barrels: u8, turn: u16) -> i32 {
    let spacing = 65536 / barrels.max(1) as i32;
    let phase = (turn as i32 - spacing / 2).rem_euclid(65536) % spacing;
    if phase > spacing / 2 {
        phase - spacing
    } else {
        phase
    }
}

/// How far a barrel-timed rotary gun (`Weapon::barrels`) turns this tick, spun up
/// `speed` ticks from `turn`: at full spin a barrel spacing for every reload, so a barrel
/// comes to the top each time it fires, and slower as it spins up. On the tick a barrel
/// comes within half a step of the top the step is trimmed to land it there, so at full
/// spin the cluster locks with a barrel at the top on every shot.
pub(crate) fn barrel_step(weapon: &Weapon, speed: u16, turn: u16) -> u16 {
    let spacing = 65536 / weapon.barrels.max(1) as u32;
    let full = spacing * weapon.laps.max(1) as u32 / weapon.reload_ticks.max(1) as u32;
    let step =
        (full * speed.min(weapon.spin_ticks) as u32 / weapon.spin_ticks.max(1) as u32) as i32;
    if step == 0 {
        return 0;
    }
    let off = barrel_off_top(weapon.barrels, turn.wrapping_add(step as u16));
    // Pulled toward a top only on the tick a barrel comes up to it: a slow gun, several
    // ticks a barrel, would otherwise be tugged about between tops.
    // Then the whole way: a barrel is within half a step of the top, and lands on it.
    if off.abs() > step / 2 {
        return step as u16;
    }
    (step - off) as u16
}

/// Whether one of `barrels` barrels came up to the top of its cluster on the last tick's
/// turn of `step` angle steps, to `turn`.
pub(crate) fn barrel_topped(barrels: u8, turn: u16, step: u16) -> bool {
    if step == 0 {
        return false;
    }
    let spacing = 65536 / barrels.max(1) as i32;
    let before = barrel_off_top(barrels, turn.wrapping_sub(step)).rem_euclid(spacing);
    let to_next = if before == 0 {
        spacing
    } else {
        spacing - before
    };
    barrel_off_top(barrels, turn) == 0 || step as i32 >= to_next
}

/// Which `arm_pitch` slot weapon `w` pitches: its own for a gun house of its own or a
/// torso gun that sways (`Weapon::sway`), else the torso's arm (0).
pub(crate) fn pitch_slot(weapon: &Weapon, w: usize) -> usize {
    if weapon.mount || (weapon.sway.0 > 0 && w != 0) {
        2 + w
    } else {
        0
    }
}

/// `yaw` held within `sway` of `torso`.
fn within_sway(torso: Angle, yaw: Angle, sway: Angle) -> Angle {
    let limit = sway.0.min(i16::MAX as u16) as i16;
    torso + Angle(torso.delta_to(yaw).clamp(-limit, limit) as u16)
}

/// Whether weapon `w` rides the torso with the first weapon: on the same elbow and not a
/// turret of its own. Guns on one torso share its yaw (`weapon_yaw[..][0]`) and the arm's
/// pitch; whichever of them leads (`World::torso_lead`) turns it.
/// A gun alone on its elbow is just a turret, and this is false for it.
fn on_torso(weapons: &[Weapon], w: usize) -> bool {
    let arm = weapons[0].pivot;
    let rides = |i: usize| {
        !weapons[i].mount && weapons[i].turret_turn > 0 && arm.is_some() && weapons[i].pivot == arm
    };
    rides(w) && (0..weapons.len()).filter(|&i| rides(i)).nth(1).is_some()
}

/// The index kinds a search for what a weapon hitting `mask` can strike reads:
/// only aircraft keep the air category (`target_categories`), so a search for
/// air targets alone passes over the armies under them.
pub(crate) fn target_kinds(mask: u32) -> u8 {
    if mask & !cat::AIR == 0 {
        kind::AIRCRAFT
    } else {
        kind::UNIT
    }
}

impl World {
    /// Whether `row` is wholly under water: no gun aims at it, only torpedoes and blasts
    /// (`blast_reaches`) reach it.
    pub(crate) fn submerged(&self, row: usize) -> bool {
        self.state.units.z[row] + self.bp(row).height < self.terrain.water_level()
    }

    /// Whether a weapon that hits `mask` can strike `target`: the right kind, and not under water.
    pub(crate) fn hittable(&self, target: usize, mask: u32) -> bool {
        self.target_layers(target) & mask != 0 && !self.submerged(target)
    }

    /// Whether a blast that hits `mask` can strike `target`: the right kind, over the water
    /// or under it. A burst is not aimed, so it takes a dived hull within its reach too.
    pub(crate) fn blast_hittable(&self, target: usize, mask: u32) -> bool {
        self.target_layers(target) & mask != 0
    }

    /// Whether a blast at `point` from `weapon` reaches `row`. A burst on the water carries
    /// straight down through it onto a unit walking the bed, measured across only. A hull
    /// afloat or dived is measured to the nearest point between its keel and its top, so a
    /// burst on the water over a dived submarine reaches down the few metres to it: no gun
    /// aims at a dived hull, but a blast is not aimed. A torpedo still takes only what
    /// `weapon_reaches`. A bridge's deck between the burst and the unit takes it
    /// (`deck_shields`).
    pub(crate) fn blast_reaches(&self, point: FxVec3, row: usize, weapon: &Weapon) -> bool {
        if self.deck_shields(point, row) {
            return false;
        }
        let units = &self.state.units;
        let bp = self.bp(row);
        let reach = weapon.splash + bp.radius;
        let afloat = self.in_water(row);
        if self.submerged(row) && !afloat {
            return !weapon.torpedo
                && point.z <= self.terrain.water_level() + Fx::ONE
                && units.pos[row].distance(point.xy()) <= reach;
        }
        let kind = if weapon.torpedo {
            self.weapon_reaches(row, weapon)
        } else {
            self.blast_hittable(row, weapon.target_mask)
        };
        if !kind {
            return false;
        }
        let z = if afloat {
            point.z.clamp(
                units.z[row] - bp.height * crate::naval::KEEL,
                units.z[row] + bp.height,
            )
        } else {
            units.z[row] + bp.height / 2
        };
        (units.pos[row].extend(z) - point).length() <= reach
    }

    /// Whether anything weapon `w` of `row` may shoot is inside its sweep cone
    /// (`Weapon::sweep`), either side of where the gun points.
    fn target_in_cone(&self, row: usize, w: usize, weapon: &Weapon) -> bool {
        let units = &self.state.units;
        let facing = units.heading[row] + units.weapon_yaw[row][w];
        let mut found = false;
        let friends = self.team_mask(units.owner[row]);
        let reach = weapon.range_max + self.gun_offset(row, weapon);
        self.index
            .query_foes(units.pos[row], reach, kind::UNIT, friends, |e| {
                let t = e.row as usize;
                found = self.unit_entry_is_current(e)
                    && self.is_valid_target(row, t, weapon)
                    && facing
                        .delta_to((units.pos[t] - units.pos[row]).angle())
                        .unsigned_abs()
                        <= weapon.sweep;
                !found
            });
        found
    }

    /// How far out `weapon` of `shooter` reaches a mark whose middle stands at
    /// `mark_z` and which moves `drift` metres a tick. A level bomb is let go a
    /// fall's flight short of its mark: far below the aircraft (a canyon floor)
    /// that line lies beyond `range_max`. The sight leads a moving mark by what it
    /// drives while the bombs fall, so one driving head-on at the bomber is that
    /// much further off when the bay opens: without it, every head-on pass flew
    /// over with the bay shut.
    fn reach_onto(&self, shooter: usize, weapon: &Weapon, mark_z: Fx, drift: Fx) -> Fx {
        let units = &self.state.units;
        let Some(motion) = self
            .bp(shooter)
            .motion
            .filter(|m| m.layer == mc_data::MoveLayer::Air)
        else {
            return weapon.range_max;
        };
        if weapon.trajectory != Trajectory::Ballistic || weapon.missile {
            return weapon.range_max;
        }
        let half_salvo = Fx::ratio(
            (weapon.salvo.saturating_sub(1) / weapon.salvo_batch) as i64
                * weapon.salvo_delay_ticks as i64,
            2,
        );
        let fall = bomb_fall(units.z[shooter] + weapon.muzzle.z - mark_z);
        // A tick past the line, as the sight allows.
        let travel = motion.speed / DT;
        weapon
            .range_max
            .max((travel + drift) * (fall + half_salvo + Fx::ONE))
    }

    pub(crate) fn is_valid_target(&self, shooter: usize, target: usize, weapon: &Weapon) -> bool {
        // Interceptor tubes never take a unit (`naval_arms.rs`); a deck gun that only
        // works surfaced holds nothing while its hull is under.
        if weapon.intercepts || self.held_dived(shooter, weapon) {
            return false;
        }
        let units = &self.state.units;
        if !units.slots.is_alive(target) || units.has_flag(target, flag::IN_FACTORY) {
            return false;
        }
        let owner = units.owner[shooter];
        if !self.are_enemies(owner, units.owner[target]) || !self.weapon_reaches(target, weapon) {
            return false;
        }
        let gap = self.hull_gap(self.gun_origin(shooter, weapon), target);
        let bp = self.bp(shooter);
        if weapon.depression.0 > 0 {
            let to = units.pos[target].extend(units.z[target] + self.bp(target).height / 2);
            let (_, elevation) = house_lay(
                units.pos[shooter],
                units.z[shooter],
                units.heading[shooter],
                self.hull_pitch(shooter),
                weapon,
                to,
            );
            if !dips_to(weapon, elevation) {
                return false;
            }
        }
        if bp
            .weapons
            .first()
            .is_some_and(|first| std::ptr::eq(first, weapon))
            && spinal_gun(bp, 0, weapon)
        {
            // Set down, the hull can neither turn nor pitch to lay it.
            if self.grounded_hull(shooter) {
                return false;
            }
            let to = units.pos[target].extend(units.z[target] + self.bp(target).height / 2);
            if !spinal_bears(
                units.pos[shooter].extend(units.z[shooter]),
                weapon.muzzle.z,
                to,
            ) {
                return false;
            }
        }
        gap <= self.reach_onto(
            shooter,
            weapon,
            units.z[target] + self.bp(target).height / 2,
            units.speed[target].abs() / DT,
        ) && gap >= weapon.range_min - self.hull_radius(target) * 2
            && self.slant_reaches(shooter, target, weapon, gap)
            && self.in_arc(shooter, target, weapon)
            && self.detects(owner, target)
    }

    /// How far `row`'s hull is pitched: a warship laying its spinal gun (`hull_pitched`).
    fn hull_pitch(&self, row: usize) -> Angle {
        if hull_pitched(self.bp(row)) {
            self.state.units.arm_pitch[row][0]
        } else {
            Angle::ZERO
        }
    }

    /// Whether `target` lies inside the arc of `weapon` on `shooter`, seen from the
    /// weapon's pivot. A gun house with a limited arc takes nothing outside it, unless
    /// it is on a ship: a ship turns its hull to bring its guns to bear.
    pub(crate) fn in_arc(&self, shooter: usize, target: usize, weapon: &Weapon) -> bool {
        self.bears_on(shooter, self.state.units.pos[target], weapon)
    }

    /// Whether the point `at` lies inside the arc of `weapon` on `shooter` ([`Self::in_arc`]):
    /// a unit or a point on the ground its orders name alike.
    fn bears_on(&self, shooter: usize, at: FxVec2, weapon: &Weapon) -> bool {
        if weapon.half_arc >= 0x8000 || !weapon.mount {
            return true;
        }
        let bp = self.bp(shooter);
        if bp
            .motion
            .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval)
        {
            return true;
        }
        let units = &self.state.units;
        let heading = units.heading[shooter];
        let from = units.pos[shooter]
            + weapon
                .pivot
                .map_or(FxVec2::ZERO, |p| p.xy())
                .rotate(heading);
        let to = at - from;
        if to == FxVec2::ZERO {
            return true;
        }
        (heading + weapon.facing)
            .delta_to(to.angle())
            .unsigned_abs()
            <= weapon.half_arc
    }

    /// Whether `row`'s fire state lets it pick targets by itself, with no order naming them.
    pub(crate) fn fires_at_will(&self, row: usize) -> bool {
        self.state.units.fire_state[row] != FireState::HoldFire
    }

    /// Whether `row` may leave its spot, or its route, to go after a target no order
    /// names: only an aircraft at fire at will. Land and naval units hold where the
    /// player put them and shoot what comes in range.
    pub(crate) fn chases(&self, row: usize) -> bool {
        self.fires_at_will(row) && self.is_air(row)
    }

    /// The point (and scatter) `row`'s orders tell its ground weapons to shell, if any.
    pub(crate) fn ground_mark(&self, row: usize) -> Option<(FxVec2, Fx)> {
        let o = self.state.orders.front(&self.state.units, row)?;
        match o.kind {
            OrderKind::AttackGround | OrderKind::Strike => Some((o.pos, Fx::ZERO)),
            OrderKind::Bombard => Some((o.pos, o.radius)),
            _ => None,
        }
    }

    /// Whether `row`'s orders call a storm down (`OrderKind::Strike`).
    pub(crate) fn striking(&self, row: usize) -> bool {
        self.state
            .orders
            .front(&self.state.units, row)
            .is_some_and(|o| o.kind == OrderKind::Strike)
    }

    /// What weapon `w` of `row` aims at this tick: its target, or else the ground its orders name.
    fn weapon_mark(&self, row: usize, w: usize, weapon: &Weapon) -> Option<Mark> {
        let units = &self.state.units;
        // A giant bore feeds the storm it raised for as long as it lasts: it holds on it.
        if weapon.bore.is_some_and(|b| b.storm.is_some()) {
            if let Some(storm) = self.storm_of(row) {
                return Some(Mark {
                    unit: None,
                    pos: storm.pos,
                    z: storm.z,
                    height: Fx::ZERO,
                    radius: Fx::ZERO,
                    air: false,
                    moved: FxVec3::ZERO,
                    turn: Angle::ZERO,
                    lead: FxVec2::ZERO,
                    scatter: Fx::ZERO,
                });
            }
        }
        if let Some(t) = units.row(units.weapon_target[row][w]) {
            let bp = self.bp(t);
            return Some(Mark {
                unit: Some(t),
                pos: units.pos[t],
                z: units.z[t],
                height: bp.height,
                radius: bp.radius,
                air: bp.has(cat::AIR),
                moved: (units.pos[t] - units.prev_pos[t]).extend(units.z[t] - units.prev_z[t]),
                turn: if bp
                    .motion
                    .is_some_and(|m| m.layer == mc_data::MoveLayer::Air && !m.hover)
                {
                    Angle(units.prev_heading[t].delta_to(units.heading[t]) as u16)
                } else {
                    Angle::ZERO
                },
                lead: FxVec2::from_angle(units.heading[t]) * (units.speed[t] / DT),
                scatter: Fx::ZERO,
            });
        }
        let (centre, scatter) = self
            .ground_mark(row)
            .filter(|_| takes_ground(self.bp(row), weapon, self.striking(row)))?;
        // Bombarding: the point this gun is on, picked after its last salvo. A bomber
        // flies one run at a time, so every bay lays on its lead weapon's point.
        let pos = if scatter > Fx::ZERO {
            units.ground_aim[row][self.bombard_slot(row, w)]
        } else {
            centre
        };
        // A gun house takes a point on the ground only inside its arc, as it does a unit
        // (`is_valid_target`): one on a capital ship's far flank would shoot through its
        // own hull.
        if !self.bears_on(row, pos, weapon) {
            return None;
        }
        Some(Mark {
            unit: None,
            pos,
            z: self.terrain.height_at(pos).max(self.terrain.water_level()),
            height: Fx::ZERO,
            radius: Fx::ZERO,
            air: false,
            moved: FxVec3::ZERO,
            turn: Angle::ZERO,
            lead: FxVec2::ZERO,
            scatter,
        })
    }

    /// Damage on its way to each unit row: the shots in flight that were fired at it.
    fn incoming_damage(&self, rows: usize) -> Vec<Fx> {
        let units = &self.state.units;
        let shots = &self.state.projectiles;
        let mut incoming = vec![Fx::ZERO; rows];
        for i in 0..shots.len() {
            if let Some(t) = units.row(shots.target[i]) {
                let bp = self.blueprints.unit(shots.blueprint[i]);
                if let Some(weapon) = bp.weapons.get(shots.weapon[i] as usize) {
                    incoming[t] += weapon.damage;
                }
            }
        }
        incoming
    }

    /// A giant bore that changes its mark part way through a charge (its target died, or
    /// an order moved it) says where it will now land: the strike warning follows it.
    fn storm_retargets(&mut self, row: usize, before: [UnitId; MAX_WEAPONS]) {
        if self.storm_of(row).is_some() {
            return;
        }
        let units = &self.state.units;
        let bp = self.bp(row);
        let mut heard = Vec::new();
        for (w, weapon) in bp.weapons.iter().enumerate() {
            let Some(storm) = weapon.bore.and_then(|b| b.storm) else {
                continue;
            };
            let left = units.weapon_cooldown[row][w];
            if units.weapon_target[row][w] == before[w] || left == 0 || left > weapon.charge_ticks {
                continue;
            }
            let mark = self.weapon_mark(row, w, weapon);
            heard.push(SimEvent::StormRetargeted {
                unit: units.id(row),
                target: mark.as_ref().map(|m| m.pos.extend(m.z)),
                on: mark.and_then(|m| m.unit).map(|t| units.id(t)),
                left,
                radius: storm.radius,
                ticks: weapon.charge_ticks,
                owner: units.owner[row],
                blueprint: units.blueprint[row],
                weapon: w as u8,
            });
        }
        self.events.extend(heard);
    }

    pub(crate) fn run_weapons(&mut self) -> Result<(), SimError> {
        let rows = self.state.units.slots.rows();
        for row in 0..rows {
            if !self.state.units.slots.is_alive(row)
                || !self.state.units.is_active(row)
                || self.stunned(row)
            {
                continue;
            }
            // A build arm that is not at work folds back to rest. A two-bone arm
            // straightens the forearm first, then raises the boom. An unarmed
            // builder's turret comes home too; an armed one lets the guns do that.
            if let Some(arm) = self.bp(row).builder.as_ref().and_then(|b| b.arm) {
                let unarmed = self.bp(row).weapons.is_empty();
                let units = &mut self.state.units;
                if !units.has_flag(row, flag::WORKING) {
                    if arm.shoulder.is_some() {
                        units.arm_pitch[row][1] =
                            units.arm_pitch[row][1].turn_toward(Angle::ZERO, arm.turn / 2);
                        if units.arm_pitch[row][1].delta_to(Angle::ZERO).unsigned_abs() <= 728 {
                            units.arm_pitch[row][0] =
                                units.arm_pitch[row][0].turn_toward(arm.rest, arm.turn / 2);
                        }
                    } else {
                        units.arm_pitch[row][1] =
                            units.arm_pitch[row][1].turn_toward(arm.rest, arm.turn / 2);
                    }
                    if unarmed {
                        units.weapon_yaw[row][0] =
                            units.weapon_yaw[row][0].turn_toward(Angle::ZERO, arm.turn / 2);
                    }
                }
            }
            let weapon_count = self.bp(row).weapons.len();
            for w in 0..weapon_count {
                self.step_weapon(row, w)?;
            }
        }
        Ok(())
    }

    /// Plants a siege gun before it can fire, and packs it before it can move.
    pub(crate) fn run_deploy(&mut self) {
        let rows = self.state.units.slots.rows();
        for row in 0..rows {
            if !self.state.units.slots.is_alive(row) || !self.state.units.is_active(row) {
                continue;
            }
            let Some(motion) = self.bp(row).motion else {
                self.step_hatches(row);
                self.step_orbs(row);
                continue;
            };
            if self.bp(row).transport.is_some() {
                // A lift ship's ramp: `run_transports`.
                continue;
            }
            if crate::launch_cells::hatch_ticks(self.bp(row)).is_some() {
                // A ship's cell hatches, as a site's.
                self.step_hatches(row);
                continue;
            }
            if motion.deploy_ticks == 0 {
                // A builder's folding gear comes out while it builds, and goes away after.
                let unfold = self.bp(row).builder.as_ref().map_or(0, |b| b.unfold_ticks);
                if unfold > 0 {
                    // Not for its own refit: that is assembled on it, not by the gear.
                    let refitting = self
                        .state
                        .orders
                        .front(&self.state.units, row)
                        .is_some_and(|o| o.kind == crate::tables::OrderKind::Upgrade);
                    let units = &mut self.state.units;
                    if units.has_flag(row, flag::BUILDING)
                        && !units.has_flag(row, flag::REPAIRING)
                        && !refitting
                    {
                        units.deploy[row] = (units.deploy[row] + 1).min(unfold);
                    } else {
                        units.deploy[row] = units.deploy[row].saturating_sub(1).min(unfold);
                    }
                }
                continue;
            }
            let shelling = self.ground_mark(row).is_some();
            let units = &mut self.state.units;
            let wants_move =
                units.flags[row] & flag::HAS_FIELD != 0 && units.flags[row] & flag::HOLD == 0;
            let has_target = shelling
                || units.weapon_target[row]
                    .iter()
                    .any(|&t| units.row(t).is_some());
            if wants_move {
                if units.deploy[row] > 0 {
                    units.flags[row] |= flag::HOLD;
                    units.deploy[row] -= 1;
                }
            } else if (has_target || motion.deploy_idle) && units.deploy[row] < motion.deploy_ticks
            {
                units.deploy[row] += 1;
            }
        }
    }

    /// Whose `ground_aim` weapon `w` of a bombarding `row` fires at. A bomber's bays
    /// all lay on its lead's point; a gun with no turret of its own (the model turns
    /// one turret, by weapon 0's yaw) lays on weapon 0's, so a Paladin's two arms
    /// fire where the torso points rather than each at a point of its own.
    pub(crate) fn bombard_slot(&self, row: usize, w: usize) -> usize {
        if let Some(lead) = self.bombard_lead(row) {
            return lead;
        }
        let weapons = &self.bp(row).weapons;
        let turreted = |i: usize| !weapons[i].mount && weapons[i].turret_turn > 0;
        if w != 0
            && turreted(w)
            && turreted(0)
            && takes_ground(self.bp(row), &weapons[0], self.striking(row))
        {
            0
        } else {
            w
        }
    }

    /// For a fixed-wing aircraft, the weapon whose `ground_aim` its bombing runs
    /// fly at. `None` for anything else: each gun there picks its own points.
    pub(crate) fn bombard_lead(&self, row: usize) -> Option<usize> {
        let bp = self.bp(row);
        bp.motion
            .filter(|m| m.layer == mc_data::MoveLayer::Air && !m.hover)?;
        bp.weapons.iter().position(hits_ground)
    }

    /// A new point for weapon `w` of a bombarding `row`, evenly over the circle's area.
    pub(crate) fn pick_bombard_aim(&mut self, row: usize, w: usize) {
        let Some((centre, radius)) = self.ground_mark(row) else {
            return;
        };
        let bearing = Angle(self.state.rng.below(65536) as u16);
        let reach = radius * Fx::ratio(self.state.rng.below(1025) as i64, 1024).sqrt();
        let spot = centre + FxVec2::from_angle(bearing) * reach;
        let size = self.terrain.size_metres();
        self.state.units.ground_aim[row][w] = FxVec2::new(
            spot.x.clamp(Fx::ZERO, size.x - Fx::ONE),
            spot.y.clamp(Fx::ZERO, size.y - Fx::ONE),
        );
    }

    pub(crate) fn guide_and_intercept_missiles(&mut self) {
        let mut killed = Vec::new();
        // Only a round with a casing to burn through can be lased: a battle's
        // shells are passed over once here, not once per laser.
        let casings: Vec<usize> = {
            let p = &self.state.projectiles;
            (0..p.len())
                .filter(|&i| {
                    self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize].casing_hp()
                        > Fx::ZERO
                })
                .collect()
        };
        self.lase_missiles(&casings, &mut killed);
        killed.sort_unstable();
        for i in killed.into_iter().rev() {
            self.state.projectiles.swap_remove(i);
        }
        // A cruise missile whose mark is gone finds another near it (`naval_arms.rs`).
        self.retarget_cruise_missiles();
        for i in 0..self.state.projectiles.len() {
            let p = &self.state.projectiles;
            let w = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            if !w.guided {
                continue;
            }
            let age = p.age[i];
            let cold = w.cold_launch_ticks;
            let step = w.projectile_speed / DT;
            let previous = p.vel[i].normalize();
            let nose = if p.aim[i].length_sq() > Fx::ZERO {
                p.aim[i]
            } else {
                previous
            };
            let desired = self
                .state
                .units
                .row(p.target[i])
                .filter(|&t| self.state.units.health[t] > Fx::ZERO)
                .map(|t| {
                    let aim = self.state.units.pos[t]
                        .extend(self.state.units.z[t] + self.bp(t).height / 2);
                    let target_velocity = (self.state.units.pos[t] - self.state.units.prev_pos[t])
                        .extend(self.state.units.z[t] - self.state.units.prev_z[t]);
                    let lead = ((aim - p.pos[i]).length() / step).min(Fx::from_int(20));
                    (aim + target_velocity * lead - p.pos[i]).normalize()
                })
                .unwrap_or(if cold > 0 && age < cold {
                    nose
                } else {
                    previous
                });
            // Sea skimmers and high arcs fly their own paths onto the mark.
            let desired = self.naval_guidance(i, w, desired);
            // A boosted SAM climbs out of its cell and turns over before its motor lights.
            if let Some((aim, vel)) = crate::launch_cells::boosted(w, age, desired, step) {
                self.state.projectiles.aim[i] = aim;
                self.state.projectiles.vel[i] = vel;
                continue;
            }
            // A cold SAM is tossed straight up under gravity. The nose eases
            // onto the intercept across the whole lob. It does not translate
            // toward the target until the motor lights. Hot missiles arc immediately.
            if cold > 0 && age < cold {
                if age >= 2 {
                    let span = cold.saturating_sub(2).max(1);
                    let along = age.saturating_sub(2).min(span);
                    let u = Fx::ratio(along as i64, span as i64);
                    let ease = u * u * (Fx::from_int(3) - u * Fx::from_int(2));
                    let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
                    let pitched = (up * (Fx::ONE - ease) + desired * ease).normalize();
                    self.state.projectiles.aim[i] = if pitched.length_sq() > Fx::ZERO {
                        pitched
                    } else {
                        desired
                    };
                }
                self.state.projectiles.vel[i] =
                    FxVec3::new(Fx::ZERO, Fx::ZERO, cold_lob_speed(age, cold));
                continue;
            }
            let steering_ramp =
                Fx::ratio(age.saturating_sub(cold).min(DT as u16) as i64, DT as i64);
            let turn = Fx::ratio(1, 5) + steering_ramp * Fx::ratio(7, 15);
            // Ignition commits the hang attitude. Until then the velocity is
            // still the vertical lob, which must not steer the first burn.
            let flight = if cold > 0 && age == cold {
                nose
            } else {
                previous
            };
            let dir = (flight * (Fx::ONE - turn) + desired * turn).normalize();
            // Ramp from 15% to cruise over one second, even if the target dies.
            let powered_age = age.saturating_sub(cold) as i32;
            let ramp = Fx::ratio(powered_age.min(DT) as i64, DT as i64);
            let mut speed = step * (Fx::ratio(3, 20) + ramp * Fx::ratio(17, 20));
            if cold == 0 {
                // Never slower than it left the rail (a rocket off a fast aircraft).
                speed = speed.max(p.vel[i].length()).min(step);
            }
            if cold > 0 && age == cold {
                self.events.push(SimEvent::MissileIgnited {
                    pos: p.pos[i],
                    vel: dir * speed,
                    blueprint: p.blueprint[i],
                    weapon: p.weapon[i],
                });
            }
            self.state.projectiles.aim[i] = dir;
            self.state.projectiles.vel[i] = dir * speed;
        }
    }

    pub(crate) fn damage_unit(&mut self, row: usize, damage: Fx, by: u8, source: UnitId) {
        let units = &mut self.state.units;
        if units.health[row] <= Fx::ZERO || units.has_flag(row, flag::INVULNERABLE) {
            return;
        }
        let applied = damage.min(units.health[row]);
        units.health[row] -= applied;
        units.flags[row] |= flag::HURT;
        let killed = units.health[row] <= Fx::ZERO;
        self.record_damage(row, source, applied);
        if killed {
            self.settle_kill(row, source, by);
        }
    }

    /// Adds a ground stain, or deepens one that is already there.
    pub(crate) fn add_stain(&mut self, pos: FxVec2, radius: Fx, strength: u8) {
        // Water leaves no scorch.
        if self.terrain.height_at(pos) < self.terrain.water_level() {
            return;
        }
        let mut existing = None;
        self.index.query(pos, Fx::ZERO, kind::STAIN, |e| {
            if e.pos.distance_sq(pos) <= (e.radius / 2) * (e.radius / 2) && e.radius >= radius / 2 {
                existing = Some(e.row as usize);
                return false;
            }
            true
        });
        let stains = &mut self.state.stains;
        match existing {
            Some(i) if i < stains.len() => {
                stains.strength[i] = stains.strength[i].saturating_add(strength / 2);
                stains.radius[i] = stains.radius[i].max(radius);
            }
            _ => {
                let seed = self.state.rng.next_u32() as u16;
                stains.push(pos, radius, strength, seed);
            }
        }
    }

    pub(crate) fn reap_dead(&mut self) -> Result<(), SimError> {
        let mut dead = std::mem::take(&mut self.scratch.dead);
        dead.clear();
        // A defeated side's units go whatever their health says: zeroing it is not
        // enough, since a build, repair, refit or promotion can lift it off zero
        // again before the reap and leave a husk standing on 0 hp.
        let lost = |s: &crate::world::State, row: usize| {
            s.units.health[row] <= Fx::ZERO || s.players[s.units.owner[row] as usize].defeated
        };
        dead.extend(
            self.state
                .units
                .slots
                .iter()
                .filter(|&row| lost(&self.state, row)),
        );
        let defeated_before = self.state.players.iter().filter(|p| p.defeated).count();
        for &row in &dead {
            // A commander's blast or a defeat can take out rows later in this list.
            if self.state.units.slots.is_alive(row) {
                self.despawn_unit(row, true)?;
            }
        }
        // A commander that fell in this reap takes its whole side with it now,
        // not a tick later.
        if self.state.players.iter().filter(|p| p.defeated).count() != defeated_before {
            dead.clear();
            let s = &self.state;
            dead.extend(
                s.units
                    .slots
                    .iter()
                    .filter(|&row| s.players[s.units.owner[row] as usize].defeated),
            );
            for &row in &dead {
                if self.state.units.slots.is_alive(row) {
                    self.despawn_unit(row, true)?;
                }
            }
        }
        self.scratch.dead = dead;
        Ok(())
    }

    /// Kills a unit: effects, wreck, scorch mark, and defeat if it was a commander.
    pub(crate) fn despawn_unit(&mut self, row: usize, leave_wreck: bool) -> Result<(), SimError> {
        let units = &self.state.units;
        let bp = self.blueprints.unit(units.blueprint[row]).clone();
        let (pos, z, heading, owner, turret) = (
            units.pos[row],
            units.z[row],
            units.heading[row],
            units.owner[row],
            units.weapon_yaw[row][0],
        );
        // A unit stowed in a hold is hidden like one still in its factory, but a
        // commander there dies in the open when its ship goes down: its reactor
        // goes up and its side is lost.
        let stowed_commander = units.hangar[row] != Handle::NONE && bp.has(cat::COMMANDER);
        let visible = !units.has_flag(row, flag::IN_FACTORY) || stowed_commander;
        let complete = !units.has_flag(row, flag::UNDER_CONSTRUCTION);
        // A lift ship's wreck holds what its hold did: the cargo dies unseen with it
        // (`lose_orphaned_cargo`), so its salvage goes into the ship's.
        let cargo_materials = if bp.transport.is_some() {
            let id = units.id(row);
            units
                .slots
                .iter()
                .filter(|&r| units.hangar[r] == id)
                .map(|r| {
                    let c = self.bp(r);
                    c.cost_materials * c.wreck_fraction
                })
                .fold(Fx::ZERO, |a, m| a + m)
        } else {
            Fx::ZERO
        };
        // A hull raised aloft by its yard (`sky_yard.rs`) falls half built if it dies so,
        // its wreck worth what was built of it; any other unfinished hull in the air
        // (a Gatewright's print) is gone, as a site is.
        let built = self.built_share(row);
        let falls_unbuilt = !complete && bp.built_aloft;
        let wreck_materials = (bp.cost_materials * bp.wreck_fraction + cargo_materials) * built;
        // Taken apart to the last plate: nothing is left to blow up or to lie about.
        // A commander's reactor goes up all the same.
        let reclaimed = units.has_flag(row, flag::RECLAIMED) && !bp.has(cat::COMMANDER);
        let surface = self.terrain.height_at(pos).max(self.terrain.water_level());
        let airborne = visible
            && (complete || falls_unbuilt)
            && leave_wreck
            && !reclaimed
            && bp.motion.map(|m| m.layer) == Some(mc_data::MoveLayer::Air)
            && z > surface + Fx::ONE;
        let crash = airborne.then(|| crate::aircraft_crash::AircraftCrash {
            blueprint: bp.id,
            unit_id: units.id(row).0,
            pos: pos.extend(z),
            prev_pos: pos.extend(z),
            velocity: (pos - units.prev_pos[row]).extend((z - units.prev_z[row]).min(Fx::ZERO)),
            heading,
            bank: units.bank[row],
            age: 0,
            materials: wreck_materials,
            splashed: 0,
            floor: Fx::ZERO,
            pitch: 0,
            prev_pitch: 0,
            roll: units.bank[row],
            prev_roll: units.bank[row],
            built,
        });
        // A ship goes down slowly and settles on the seabed (`sinking.rs`).
        let sinking = (visible
            && complete
            && leave_wreck
            && !reclaimed
            && bp.motion.map(|m| m.layer) == Some(mc_data::MoveLayer::Naval))
        .then(|| {
            crate::sinking::SinkingHull::new(
                bp.id,
                units.id(row).0,
                pos,
                z,
                heading,
                self.terrain.height_at(pos),
                bp.height,
                wreck_materials,
            )
        });
        // An AI side remembers where its buildings were shot down and keeps
        // its builders off that ground for a while (`ai/danger.rs`).
        let player = &self.state.players[owner as usize];
        if bp.is_structure()
            && visible
            && leave_wreck
            && !reclaimed
            && !player.defeated
            && player.controller == crate::tables::Controller::Ai
        {
            let tick = self.state.tick;
            self.state.ai[owner as usize].note_loss(pos, tick);
        }
        self.remove_unit_row(row, false)?;

        if visible {
            self.events.push(if reclaimed {
                SimEvent::Reclaimed {
                    pos: pos.extend(z),
                    blueprint: bp.id,
                    wreck: false,
                }
            } else {
                SimEvent::UnitDied {
                    pos: pos.extend(z),
                    blueprint: bp.id,
                    owner,
                    airborne,
                    complete,
                }
            });
            self.state.players[owner as usize].units_lost += 1;
            let falling = self.state.aircraft_crashes.len() + self.state.sinking.len();
            if (crash.is_some() || sinking.is_some())
                && falling + self.state.wrecks.slots.live() >= crate::tables::MAX_WRECKS
            {
                return Err(SimError::TableFull(crate::Table::Wrecks));
            }
            if let Some(crash) = crash {
                self.state.aircraft_crashes.push(crash);
            } else if let Some(hull) = sinking {
                self.state.sinking.push(hull);
            } else if leave_wreck && !reclaimed {
                self.add_stain(pos, bp.radius * Fx::ratio(3, 2), 72);
                let materials = wreck_materials;
                if complete && materials > Fx::ZERO {
                    let wreck_z = if bp.motion.map(|m| m.layer) == Some(mc_data::MoveLayer::Air) {
                        let ground = self.terrain.height_at(pos);
                        ground.max(self.terrain.water_level())
                    } else {
                        z
                    };
                    let tick = self.state.tick;
                    let wrecks = &mut self.state.wrecks;
                    let row = wrecks.spawn(bp.id, pos, wreck_z, heading, materials, tick)?;
                    wrecks.turret[row] = turret;
                }
                // The lot was poured; death does not take it up. The scorch
                // stain sits on top of it.
                self.remember_structure_pad(&bp, pos, heading, owner)?;
            } else if reclaimed {
                // Unbuilt, not destroyed: the pour goes with the building.
                self.state.pads.remove_at(pos);
            }
        }
        // A volatile unit goes up and takes its neighbours with it, friend or foe.
        // Anything it kills goes on the next reap, so a packed row of them
        // detonates one after another rather than all in one tick.
        if let Some(db) = bp.death_blast.filter(|_| complete && visible && !reclaimed) {
            let afloat = bp.motion.map(|m| m.layer) == Some(mc_data::MoveLayer::Naval)
                || (bp.water_build && self.terrain.height_at(pos) < self.terrain.water_level());
            self.death_blast(pos, db, owner, afloat);
        }
        if complete && visible && bp.has(cat::COMMANDER) {
            // A commander's reactor goes up as a small nuclear blast (`nukes.rs`).
            let ground = self.terrain.height_at(pos).max(self.terrain.water_level());
            self.detonate(
                pos.extend(ground + Fx::from_int(8)),
                crate::nukes::COMMANDER_BLAST,
                owner,
                Handle::NONE,
                true,
                self.strategic_look(bp.id),
            )?;
            if self.state.players[owner as usize].commander.index() == row {
                self.defeat_player(owner);
            }
        }
        Ok(())
    }

    /// Frees a unit row and everything hanging off it, with no death effects.
    pub(crate) fn remove_unit_row(
        &mut self,
        row: usize,
        keep_blocked: bool,
    ) -> Result<(), SimError> {
        // Whatever it was assembling dies with it; what a yard was raising in the sky
        // comes down.
        let units = &self.state.units;
        if let Some(t) = units.row(units.build_target[row]) {
            if units.has_flag(t, flag::IN_FACTORY) {
                self.remove_unit_row(t, true)?;
            }
        }
        self.drop_aloft_product(row);
        self.state.orders.clear(&mut self.state.units, row);
        self.stop_moving(row);
        let release = {
            let units = &self.state.units;
            let bp = self.blueprints.unit(units.blueprint[row]);
            // A half-built experimental's lot is freed with it (`is_site_built_unit`).
            let site = bp.is_site_built_unit() && units.has_flag(row, flag::UNDER_CONSTRUCTION);
            if site || (bp.is_structure() && !keep_blocked && !units.has_flag(row, flag::UPGRADE)) {
                Some((bp.clone(), units.pos[row], units.heading[row]))
            } else {
                None
            }
        };
        if let Some((bp, pos, heading)) = release {
            self.release_lot(row, &bp, pos, heading);
        }
        self.state.units.slots.free(row);
        Ok(())
    }

    /// Assassination rules: lose the commander, lose everything.
    pub(crate) fn defeat_player(&mut self, player: u8) {
        let p = &mut self.state.players[player as usize];
        if p.defeated {
            return;
        }
        p.defeated = true;
        self.events.push(SimEvent::PlayerDefeated { player });
        let units = &mut self.state.units;
        for row in 0..units.slots.rows() {
            if units.slots.is_alive(row) && units.owner[row] == player {
                units.health[row] = Fx::ZERO;
            }
        }
    }
}
