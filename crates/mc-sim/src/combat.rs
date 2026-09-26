//! Targeting, weapons, projectiles, damage and death.
//!
//! The read-only searches (target acquisition, projectile sweeps) run in
//! parallel over fixed-size row chunks and return per-chunk results that are
//! applied in chunk order. Everything that mutates state or draws random
//! numbers runs sequentially in row order.

use crate::mirror::SimEvent;
use crate::shields::{in_dome, ray_dome};
use crate::spatial::kind;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::{cat, TargetPick, Trajectory, Weapon, MAX_WEAPONS};

/// Metres forward of the Thunderhead's origin where its cannon's mount yaws: the
/// breech, inside the nose. The model's turret pivot (`models::aster::air`) matches.
pub(crate) const ASSAULT_GUN_PIVOT_X: Fx = Fx::from_int(7);
const DT: i32 = TICKS_PER_SECOND as i32;
const CHUNK: usize = 256;
/// Turret must be within this of the firing solution to shoot (~2 degrees).
const AIM_TOLERANCE: u16 = 364;
/// Where an idle land AA gun rests: raised toward the sky it guards.
const IDLE_SKY_PITCH: Angle = Angle::from_degrees(60);
/// Fixed strafing gun may correct at most eight degrees from the pitched nose.
const ASSAULT_GUN_CONE: Angle = Angle::from_degrees(8);
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
/// Hit points an intercept laser burns off a missile each tick.
const LASER_BITE: Fx = Fx::from_int(10);
/// Ticks after a kill before the laser acquires another missile.
const LASER_GAP: u16 = 3;
/// Longest a ready broadside battery (`Weapon::volley`) holds for the others: 4 s.
const VOLLEY_HOLD: u16 = 40;
/// `Units::volley` busy bit for a broadside hull still turning onto its lay.
const VOLLEY_HULL: u16 = 1 << 15;

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
    let loft = Angle::from_degrees(50);
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

struct Hit {
    projectile: usize,
    point: FxVec3,
    unit: Option<usize>,
    /// A dome that stopped the shot. Mutually exclusive with `unit`.
    shield: Option<usize>,
    /// Share of this tick's step flown before the hit.
    after: Fx,
    /// Where the hit shows: `point` can lie deep inside a unit (the sweep finds
    /// the closest pass to its centre), so this is backed out to about its skin.
    /// Presentation only; damage and blasts use `point`.
    seen: FxVec3,
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
    let full = spacing / weapon.reload_ticks.max(1) as u32;
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

impl World {
    /// Whether `row` is wholly under water, below the reach of every weapon.
    pub(crate) fn submerged(&self, row: usize) -> bool {
        self.state.units.z[row] + self.bp(row).height < self.terrain.water_level()
    }

    /// Whether a weapon that hits `mask` can strike `target`: the right kind, and not under water.
    pub(crate) fn hittable(&self, target: usize, mask: u32) -> bool {
        self.target_layers(target) & mask != 0 && !self.submerged(target)
    }

    /// Whether a blast at `point` from `weapon` reaches `row`. A burst on the water carries
    /// straight down through it onto a unit walking the bed, measured across only. A hull
    /// afloat or dived is only reached by what `weapon_reaches` it with (a dived one by a
    /// torpedo alone), measured to the nearest point between its keel and its top.
    fn blast_reaches(&self, point: FxVec3, row: usize, weapon: &Weapon) -> bool {
        let units = &self.state.units;
        let bp = self.bp(row);
        let reach = weapon.splash + bp.radius;
        let afloat = self.in_water(row);
        if self.submerged(row) && !afloat {
            return !weapon.torpedo
                && point.z <= self.terrain.water_level() + Fx::ONE
                && units.pos[row].distance(point.xy()) <= reach;
        }
        if !self.weapon_reaches(row, weapon) {
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
        self.index
            .query(units.pos[row], weapon.range_max, kind::UNIT, |e| {
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
    /// `mark_z`. A level bomb is let go a fall's flight short of its mark: far
    /// below the aircraft (a canyon floor) that line lies beyond `range_max`.
    fn reach_onto(&self, shooter: usize, weapon: &Weapon, mark_z: Fx) -> Fx {
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
        weapon.range_max.max(travel * (fall + half_salvo + Fx::ONE))
    }

    pub(crate) fn is_valid_target(&self, shooter: usize, target: usize, weapon: &Weapon) -> bool {
        // Interceptor tubes never take a unit (`naval_arms.rs`); a deck gun that only
        // works surfaced holds nothing while its hull is under.
        if weapon.intercepts || (weapon.surfaced && self.submerged(shooter)) {
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
        let gap =
            self.gun_origin(shooter, weapon).distance(units.pos[target]) - self.bp(target).radius;
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
        ) && gap >= weapon.range_min - self.bp(target).radius * 2
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
        let to = units.pos[target] - from;
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

    /// Whether `row` may leave its spot, or its route, to go after a target no order names.
    pub(crate) fn chases(&self, row: usize) -> bool {
        self.state.units.fire_state[row] == FireState::FireAtWill
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

    pub(crate) fn run_targeting(&mut self) {
        let rows = self.state.units.slots.rows();
        let this = &*self;
        let picks: Vec<Vec<(usize, [UnitId; MAX_WEAPONS], u8)>> =
            self.pool.parallel_map_chunks(rows, CHUNK, |_, range| {
                let units = &this.state.units;
                let mut out = Vec::new();
                for row in range {
                    if !units.slots.is_alive(row) || !units.is_active(row) {
                        continue;
                    }
                    let bp = this.bp(row);
                    if bp.weapons.is_empty() {
                        continue;
                    }
                    // Holding fire: let go of whatever it was aiming at.
                    if units.has_flag(row, flag::PASSIVE) {
                        if units.weapon_target[row] != [Handle::NONE; MAX_WEAPONS] {
                            out.push((row, [Handle::NONE; MAX_WEAPONS], 0));
                        }
                        continue;
                    }
                    let ordered = this
                        .state
                        .orders
                        .front(units, row)
                        .filter(|o| {
                            o.kind == OrderKind::Attack
                                // Attack-move aircraft retain a flight target through
                                // their return leg. Aim the weapon at that same target;
                                // a nearer enemy can otherwise steal the bomb bay while
                                // the pilot keeps lining up on the original target.
                                || (matches!(o.kind, OrderKind::AttackMove | OrderKind::Patrol)
                                    && units.has_flag(row, flag::AIR_RUN)
                                    && bp
                                        .motion
                                        .is_some_and(|m| m.layer == mc_data::MoveLayer::Air))
                        })
                        .and_then(|o| units.row(o.target));
                    let ground = this.ground_mark(row).is_some();
                    let strike = this.striking(row);
                    let mut targets = units.weapon_target[row];
                    for (w, weapon) in bp.weapons.iter().enumerate() {
                        if let Some(t) = ordered.filter(|t| this.is_valid_target(row, *t, weapon)) {
                            targets[w] = units.id(t);
                            continue;
                        }
                        // Ordered onto the ground: the weapons phase aims these at it.
                        if ground && takes_ground(bp, weapon, strike) {
                            targets[w] = Handle::NONE;
                            continue;
                        }
                        // Stick with the current target while it lives and stays in
                        // range. A closer enemy is not a reason to turn the gun.
                        let current = units.row(targets[w]).filter(|t| {
                            this.is_valid_target(row, *t, weapon) && this.fires_at_will(row)
                        });
                        let in_grid = |prefer: u32| {
                            this.index
                                .nearest(
                                    units.pos[row],
                                    weapon.range_max + this.gun_offset(row, weapon),
                                    kind::UNIT,
                                    |e| {
                                        this.unit_entry_is_current(e)
                                            && this.is_valid_target(row, e.row as usize, weapon)
                                            && (prefer == 0
                                                || this.hittable(e.row as usize, prefer))
                                    },
                                )
                                .map(|e| e.row as usize)
                        };
                        let nearest = |prefer: u32| match weapon.pick {
                            TargetPick::Nearest => in_grid(prefer),
                            TargetPick::Costliest => this.costliest_target(row, weapon, prefer),
                        };
                        // A weapon with a preference (`Weapon::prefer_mask`) leaves what it
                        // is on for one of those as soon as one is in range.
                        let preferred = (weapon.prefer_mask != 0 && this.fires_at_will(row))
                            .then(|| nearest(weapon.prefer_mask))
                            .flatten();
                        if let Some(t) = current {
                            match preferred {
                                Some(p) if !this.hittable(t, weapon.prefer_mask) => {
                                    targets[w] = units.id(p);
                                }
                                _ => {}
                            }
                            continue;
                        }
                        targets[w] = if this.fires_at_will(row) {
                            preferred
                                .or_else(|| nearest(0))
                                .map_or(Handle::NONE, |t| units.id(t))
                        } else {
                            Handle::NONE
                        };
                    }
                    // Line of fire (`line_of_fire.rs`): looked at on the unit's turn, and
                    // at once for a gun on a new target. A gun that cannot see its target
                    // turns to one it can, unless an order names the one it is on.
                    let due = this.line_check_due(row);
                    let mut blocked = units.shot_blocked[row];
                    for (w, weapon) in bp.weapons.iter().enumerate() {
                        let bit = 1u8 << w;
                        let Some(t) = units
                            .row(targets[w])
                            .filter(|_| this.needs_line(row, weapon))
                        else {
                            blocked &= !bit;
                            continue;
                        };
                        if !due && targets[w] == units.weapon_target[row][w] {
                            continue;
                        }
                        if this.clear_shot(row, weapon, t) {
                            blocked &= !bit;
                            continue;
                        }
                        blocked |= bit;
                        if ordered != Some(t) && this.fires_at_will(row) {
                            if let Some(seen) = this.visible_alternative(row, weapon, t) {
                                targets[w] = units.id(seen);
                                blocked &= !bit;
                            }
                        }
                    }
                    if targets != units.weapon_target[row] || blocked != units.shot_blocked[row] {
                        out.push((row, targets, blocked));
                    }
                }
                out
            });
        for (row, targets, blocked) in picks.into_iter().flatten() {
            let before = self.state.units.weapon_target[row];
            self.state.units.weapon_target[row] = targets;
            self.state.units.shot_blocked[row] = blocked;
            self.storm_retargets(row, before);
        }
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
            if !self.state.units.slots.is_alive(row) || !self.state.units.is_active(row) {
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
            self.volley_turn(row);
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
                continue;
            };
            if self.bp(row).transport.is_some() {
                // A lift ship's ramp: `run_transports`.
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
            if self.bp(row).drone.is_some() {
                let need = motion.deploy_ticks;
                let wants_move = self
                    .state
                    .orders
                    .front(&self.state.units, row)
                    .is_some_and(|o| {
                        matches!(
                            o.kind,
                            crate::tables::OrderKind::Move | crate::tables::OrderKind::AttackMove
                        )
                    });
                let home = self.carrier_drones_home(row);
                let working = self.carrier_has_work(row) || self.carrier_reclaim_ordered(row);
                let units = &mut self.state.units;
                if wants_move && (!home || units.deploy[row] > 0) {
                    // Wait out drones that are still out, then shut the bays.
                    // Drones not built yet do not count.
                    units.flags[row] |= flag::HOLD;
                    if home && units.deploy[row] > 0 {
                        units.deploy[row] -= 1;
                    }
                } else if !wants_move && working && units.deploy[row] < need {
                    units.deploy[row] += 1;
                } else if !wants_move && home && units.deploy[row] > 0 {
                    units.deploy[row] -= 1;
                }
                continue;
            }
            let shelling = self.ground_mark(row).is_some();
            // A salvage boat plants at its work (`World::reclaim_ready`).
            let salvaging =
                self.bp(row).reclaimer.is_some() && self.state.units.has_flag(row, flag::WORKING);
            let units = &mut self.state.units;
            let wants_move =
                units.flags[row] & flag::HAS_FIELD != 0 && units.flags[row] & flag::HOLD == 0;
            let has_target = shelling
                || salvaging
                || units.weapon_target[row]
                    .iter()
                    .any(|&t| units.row(t).is_some());
            if wants_move {
                if units.deploy[row] > 0 {
                    units.flags[row] |= flag::HOLD;
                    units.deploy[row] -= 1;
                }
            } else if has_target && units.deploy[row] < motion.deploy_ticks {
                units.deploy[row] += 1;
            }
        }
    }

    /// The torso gun that turns the torso this tick: the hardest-hitting one with something
    /// to shoot, so a main gun is aimed with the whole body and the others fire where it
    /// points. `None`: none of them has anything, and the torso comes home.
    fn torso_lead(&self, row: usize) -> Option<usize> {
        let weapons = &self.bp(row).weapons;
        (0..weapons.len())
            .filter(|&i| on_torso(weapons, i) && self.weapon_mark(row, i, &weapons[i]).is_some())
            .max_by_key(|&i| (weapons[i].damage, std::cmp::Reverse(i)))
    }

    /// Decides whether this tick is a broadside (`Weapon::volley`): the batteries that
    /// were ready last tick fire once none of the others that bear is still turning or
    /// reloading, or once they have held `VOLLEY_HOLD` ticks. Then clears the tick's record.
    fn volley_turn(&mut self, row: usize) {
        let v = &mut self.state.units.volley[row];
        let (ready, busy) = (v[0], v[1]);
        let go = ready != 0 && (busy == 0 || v[3] >= VOLLEY_HOLD);
        v[3] = if ready != 0 && !go {
            v[3].saturating_add(1)
        } else {
            0
        };
        v[4] = go as u16;
        v[0] = 0;
        v[1] = 0;
    }

    fn step_weapon(&mut self, row: usize, w: usize) -> Result<(), SimError> {
        let bp = self.blueprints.clone();
        let weapon = &bp.unit(self.state.units.blueprint[row]).weapons[w];
        // Interceptor tubes are fired and reloaded by `run_torpedo_defence`.
        if weapon.intercepts {
            return Ok(());
        }
        if let Some((centre, radius)) = self.ground_mark(row).filter(|m| m.1 > Fx::ZERO) {
            // First shot of a bombardment, or the circle has moved: choose a point in it.
            // A point this gun cannot reach from here is given up for another.
            // A bomber's point is where it flies to, so reach does not matter.
            // A gun riding another's turret lays on that gun's point, which it picks.
            let aim = self.state.units.ground_aim[row][w];
            let gap = aim.distance(self.state.units.pos[row]);
            let bomber = self.bombard_lead(row).is_some();
            if takes_ground(self.bp(row), weapon, false)
                && self.bombard_slot(row, w) == w
                && (aim.distance(centre) > radius
                    || (!bomber && (gap > weapon.range_max || gap < weapon.range_min)))
            {
                self.pick_bombard_aim(row, w);
            }
        }
        let mut mark = self.weapon_mark(row, w, weapon);
        // A mark in a spinal gun's dead zone, too far in under the hull to dive onto, is
        // not its to take: the hull stays level and the turrets have it.
        if spinal_gun(bp.unit(self.state.units.blueprint[row]), w, weapon) {
            let from = self.state.units.pos[row].extend(self.state.units.z[row]);
            mark = mark
                .filter(|t| spinal_bears(from, weapon.muzzle.z, t.pos.extend(t.z + t.height / 2)));
        }
        if weapon.depression.0 > 0 {
            let u = &self.state.units;
            let hull = self.hull_pitch(row);
            mark = mark.filter(|t| {
                let to = t.pos.extend(t.z + t.height / 2);
                let (_, elevation) =
                    house_lay(u.pos[row], u.z[row], u.heading[row], hull, weapon, to);
                dips_to(weapon, elevation)
            });
        }
        let on_body = on_torso(&bp.unit(self.state.units.blueprint[row]).weapons, w);
        let lead = if on_body { self.torso_lead(row) } else { None };
        // Whether this gun turns the torso and pitches the arm, or rides where they point.
        let drives = if weapon.mount {
            true
        } else if on_body {
            lead == Some(w)
        } else {
            w == 0
        };
        // A shoulder gun under water is silent; so is every gun of a unit that is building,
        // except one on a turret of its own.
        let drowned = (weapon.mount
            && self.state.units.z[row] + weapon.pivot.unwrap_or(weapon.muzzle).z
                < self.terrain.water_level())
            || self.drowned_on_seabed(row, weapon);
        let units = &mut self.state.units;
        let held = drowned || (units.has_flag(row, flag::WORKING) && !weapon.mount);
        // A rotary gun spins up while it has something to shoot, and down again after.
        if weapon.spin_ticks > 0 {
            let spin = &mut units.spin[row];
            spin[0] = if mark.is_some() && !held {
                (spin[0] + 1).min(weapon.spin_ticks)
            } else {
                spin[0].saturating_sub(1)
            };
            let step = if weapon.barrels > 0 {
                barrel_step(weapon, spin[0], spin[1])
            } else {
                (SPIN_TOP as u32 * spin[0] as u32 / weapon.spin_ticks as u32) as u16
            };
            spin[1] = spin[1].wrapping_add(step);
            spin[2] = step;
        }
        // The stream ends once the barrels have run down, or at once for a gun without any.
        if weapon.sweep > 0
            && (mark.is_none() || held)
            && (weapon.spin_ticks == 0 || units.spin[row][0] == 0)
        {
            units.streaming[row] = false;
        }
        // True if this tick used up a countdown (reload or a first-shot charge).
        // A weapon that sat ready (cooldown already 0) still has to charge before it fires.
        let cooling = units.weapon_cooldown[row][w] > 0;
        if cooling {
            units.weapon_cooldown[row][w] -= 1;
        }
        if held && weapon.mount {
            units.weapon_salvo_left[row][w] = 0;
            units.arm_pitch[row][2 + w] =
                units.arm_pitch[row][2 + w].turn_toward(Angle::ZERO, weapon.turret_turn / 2);
            return Ok(());
        }
        if held {
            // The torso is turned to its work: the orders phase aims it, and the guns wait.
            units.weapon_salvo_left[row][w] = 0;
            // The Thunderhead's slot 0 is its hull's flight pitch (`movement.rs`), not a gun's.
            if w == 0 && bp.unit(units.blueprint[row]).visual.mesh != "assault_air" {
                let rate = hull_pitch_rate(bp.unit(units.blueprint[row]), w, weapon);
                units.arm_pitch[row][0] = units.arm_pitch[row][0].turn_toward(Angle::ZERO, rate);
            }
            return Ok(());
        }
        // A siege gun only slews once the spade is down. While it is packing,
        // rolling, or still planting, the turret and tube come back to rest.
        // A lift ship's `deploy` is its ramp, not a spade.
        let planted = bp.unit(units.blueprint[row]).transport.is_some()
            || bp
                .unit(units.blueprint[row])
                .motion
                .is_none_or(|m| m.deploy_ticks == 0 || units.deploy[row] >= m.deploy_ticks);
        let Some(t) = mark.filter(|_| planted) else {
            // A battery with nothing to shoot is no longer primed for a broadside.
            units.volley[row][2] &= !(1u16 << w);
            // Nothing to shoot, or the gun is not planted: turrets drift back
            // to centre, arms come level.
            // A shoulder gun faces where the torso does.
            // A torso gun with nothing leaves the torso to the one that has something. One
            // that sways comes back in line with it, and level.
            if on_body && w != 0 {
                let torso = units.weapon_yaw[row][0];
                units.weapon_yaw[row][w] = if weapon.sway.0 > 0 {
                    let rate = hull_pitch_rate(bp.unit(units.blueprint[row]), w, weapon);
                    units.arm_pitch[row][2 + w] =
                        units.arm_pitch[row][2 + w].turn_toward(Angle::ZERO, rate);
                    within_sway(
                        torso,
                        units.weapon_yaw[row][w].turn_toward(torso, weapon.turret_turn),
                        weapon.sway,
                    )
                } else {
                    torso
                };
                units.weapon_salvo_left[row][w] = 0;
                return Ok(());
            }
            if (on_body && lead.is_some()) || weapon.keeps_aim {
                // A gun that keeps its aim (`Weapon::keeps_aim`) stays laid where it was.
                units.weapon_salvo_left[row][w] = 0;
                return Ok(());
            }
            // A ship's mount stands on the hull, not on another turret: it comes home to the bow.
            // So does a gun house on a land hull built like one (`hull_mounts`).
            let naval = bp
                .unit(units.blueprint[row])
                .motion
                .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval)
                || (bp.unit(units.blueprint[row]).hull_mounts && weapon.mount);
            // A mount that faces off the nose (`facing`, or aft for `rear`) rests that way.
            let home = if weapon.mount && !naval {
                units.weapon_yaw[row][0]
            } else if weapon.mount {
                weapon.facing
            } else {
                Angle::ZERO
            };
            units.weapon_yaw[row][w] =
                units.weapon_yaw[row][w].turn_toward(home, weapon.turret_turn / 2);
            if weapon.mount
                || (w == 0 && bp.unit(units.blueprint[row]).visual.mesh != "assault_air")
            {
                let slot = pitch_slot(weapon, w);
                // A land AA gun waits pointed at the sky, not at the horizon.
                let sky_gun = !weapon.mount
                    && !weapon.missile
                    && weapon.pivot.is_some()
                    && weapon.target_mask & !cat::AIR == 0
                    && bp
                        .unit(units.blueprint[row])
                        .motion
                        .is_some_and(|m| m.layer == mc_data::MoveLayer::Land);
                let rest = if sky_gun { IDLE_SKY_PITCH } else { Angle::ZERO };
                let rate = hull_pitch_rate(bp.unit(units.blueprint[row]), w, weapon);
                units.arm_pitch[row][slot] = units.arm_pitch[row][slot].turn_toward(rest, rate);
            }
            units.weapon_salvo_left[row][w] = 0;
            return Ok(());
        };

        // Heard charging a fixed time before the salvo is due. During a reload that
        // is presentation only: the countdown is already the wait. The first shot,
        // and a shot after sitting idle, start a real charge below once the tube is on.
        // Skip the tick that just began an idle charge (`!cooling` and cooldown
        // already set would be a second start, not this path).
        if weapon.charge_ticks > 0
            && cooling
            && units.weapon_cooldown[row][w] == weapon.charge_ticks
            && units.weapon_salvo_left[row][w] == 0
        {
            let at = units.pos[row].extend(units.z[row] + weapon.muzzle.z);
            self.events.push(SimEvent::WeaponCharging {
                unit: units.id(row),
                pos: at,
                owner: units.owner[row],
                blueprint: units.blueprint[row],
                weapon: w as u8,
            });
            if let (Some(storm), Some(m)) = (weapon.bore.and_then(|b| b.storm), mark.as_ref()) {
                self.events.push(SimEvent::StormCharging {
                    unit: units.id(row),
                    muzzle: at,
                    target: m.pos.extend(m.z),
                    on: m.unit.map(|t| units.id(t)),
                    radius: storm.radius,
                    ticks: weapon.charge_ticks,
                    owner: units.owner[row],
                    blueprint: units.blueprint[row],
                    weapon: w as u8,
                });
            }
        }

        let pos = units.pos[row];
        // A ship's turrets stand off the hull's origin, on the deck: each traverses about
        // its own pivot, carried round by the hull alone, and aims from there. A land hull
        // with gun houses (`hull_mounts`) carries them the same way.
        let naval = bp
            .unit(units.blueprint[row])
            .motion
            .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval)
            || (bp.unit(units.blueprint[row]).hull_mounts && weapon.mount);
        // A turret that swings about a point off the middle (`turret_at`, a scorpion's
        // tail root) aims from there. So does a shoulder gun, from its trunnion on the
        // torso: well off the middle on a giant, it would lay wide of its own shells.
        let yaw_origin = match (weapon.pivot, bp.unit(units.blueprint[row]).turret_at) {
            (Some(p), _) if naval => pos + p.xy().rotate(units.heading[row]),
            (Some(p), _) if weapon.mount => {
                pos + p.xy().rotate(units.heading[row] + units.weapon_yaw[row][0])
            }
            (_, Some(at)) if w == 0 => pos + at.rotate(units.heading[row]),
            _ => pos,
        };
        let aircraft = bp
            .unit(units.blueprint[row])
            .motion
            .filter(|m| m.layer == mc_data::MoveLayer::Air);
        let bomb =
            aircraft.is_some() && weapon.trajectory == Trajectory::Ballistic && !weapon.missile;
        // The bomb bursts part-way through its last tick: the sight works from
        // the unrounded fall, or every stick lands most of a tick short.
        let fall = if bomb {
            bomb_fall(units.z[row] + weapon.muzzle.z - t.z - t.height / 2)
        } else {
            Fx::ZERO
        };
        let fall_ticks = fall.ceil_int();
        if let Some(motion) = aircraft {
            // No firing from a parked aircraft or during a low-speed launch.
            if (!motion.hover && !units.has_flag(row, flag::AIR_RUN) && weapon.half_arc < 0x8000)
                || (!motion.hover
                    && weapon.half_arc < 0x8000
                    && units.speed[row]
                        < motion.speed * if bomb { Fx::ratio(3, 4) } else { Fx::HALF })
                || (bomb
                    && (!units.has_flag(row, flag::AIR_RUN)
                        || units.speed[row] < motion.speed * Fx::ratio(3, 4)))
                || units.z[row]
                    < self.terrain.height_at(pos)
                        + if bp.unit(units.blueprint[row]).is_capital_ship() {
                            // A capital ship's guns fire all the way down, and from the ground.
                            -Fx::ONE
                        } else if bp.unit(units.blueprint[row]).visual.mesh == "assault_air" {
                            // Keep the strafing cannon live below cruise altitude.
                            crate::movement::ASSAULT_PASS_CLEARANCE / 2
                        } else if weapon.torpedo {
                            // A torpedo is let go low over the water, on the run in.
                            crate::movement::TORPEDO_RUN_HEIGHT / 2
                        } else if bomb {
                            // The sight works from the true fall, and `AIR_RUN` at speed
                            // already rules out a launch: a run in off low ground onto a
                            // cliff top, still climbing, drops rather than going round.
                            motion.altitude / 4
                        } else if !motion.hover && bp.unit(units.blueprint[row]).has(cat::ANTI_AIR)
                        {
                            // Pursuit can take fighters well below their cruise band.
                            crate::movement::FIGHTER_ATTACK_CLEARANCE / 2
                        } else {
                            motion.altitude * Fx::ratio(4, 5)
                        }
                // Nor from high up: it would fall too far and go in anyhow.
                || (weapon.torpedo
                    && units.z[row]
                        > self.terrain.height_at(pos).max(self.terrain.water_level())
                            + crate::movement::TORPEDO_RUN_HEIGHT * 2)
            {
                units.weapon_salvo_left[row][w] = 0;
                return Ok(());
            }
        }
        let step = weapon.projectile_speed / DT;
        let target_center = t.pos.extend(t.z + t.height / 2);
        let direct_air =
            weapon.trajectory == Trajectory::Direct && !weapon.missile && !weapon.guided && t.air;
        let target_velocity = t.moved;
        let air_aim = direct_air.then(|| {
            direct_air_aim(
                pos.extend(units.z[row] + weapon.pivot.unwrap_or(weapon.muzzle).z),
                target_center,
                target_velocity,
                t.turn,
                step,
            )
        });
        let aim_z = air_aim.map_or(target_center.z, |aim| aim.z);
        // Rockets aim at the current position; guided missiles update their intercept in flight.
        let aim = if let Some(aim) = air_aim {
            aim.xy()
        } else if bomb {
            let half_salvo = ((weapon.salvo.saturating_sub(1) / weapon.salvo_batch) as i32
                * weapon.salvo_delay_ticks as i32)
                / 2;
            t.pos + t.moved.xy() * Fx::from_int(fall_ticks + half_salvo)
        } else if weapon.missile {
            t.pos
        } else {
            let flight_ticks = match weapon.trajectory {
                Trajectory::Direct => pos.distance(t.pos) / step,
                Trajectory::Ballistic => Fx::from_int(ballistic_ticks(pos.distance(t.pos), weapon)),
            };
            t.pos + t.lead * flight_ticks
        };
        // A gun house on a pitched hull turns and elevates in the deck's frame.
        let deck = (weapon.mount && hull_pitched(bp.unit(units.blueprint[row]))).then(|| {
            house_lay(
                pos,
                units.z[row],
                units.heading[row],
                units.arm_pitch[row][0],
                weapon,
                aim.extend(aim_z),
            )
        });
        let bearing = match deck {
            Some((yaw, _)) => units.heading[row] + yaw,
            None => (aim - yaw_origin).angle(),
        };

        // An arm points up or down at its target as well as round to it.
        // Howitzers elevate to the lob, not the line of sight, and wait until the tube is there.
        let mut pitched_on = true;
        let slot = pitch_slot(weapon, w);
        // A torso gun that sways lays its own yaw and pitch within its sway of the torso.
        let swaying = on_body && w != 0 && weapon.sway.0 > 0;
        if let (true, Some(pivot)) = (drives || swaying, weapon.pivot) {
            let want = if weapon.missile {
                missile_rack_pitch(weapon)
            } else if weapon.trajectory == Trajectory::Ballistic {
                ballistic_tube_pitch(
                    yaw_origin.distance(aim),
                    t.z + t.height / 2,
                    units.z[row],
                    weapon,
                    pivot,
                )
            } else if weapon.bore.is_some() && aircraft.is_none() {
                // Elevation is relative to the tilted hull, while the target is in world
                // space. The central turret yaws about the hull, independent houses
                // about their own pivots. Use the same terrain basis as the muzzle.
                let facing = units.heading[row] + units.weapon_yaw[row][w];
                let pivot_yaw = if weapon.mount {
                    units.heading[row]
                } else {
                    facing
                };
                let local_pivot = pivot.xy().rotate(pivot_yaw).extend(pivot.z);
                let lean = |v| {
                    crate::world::leaned(
                        &self.terrain,
                        pos,
                        bp.unit(units.blueprint[row]).radius,
                        units.heading[row],
                        v,
                    )
                };
                let forward = lean(FxVec2::from_angle(facing).extend(Fx::ZERO));
                let up = lean(FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE));
                let delta = (aim - pos).extend(aim_z - units.z[row]) - lean(local_pivot);
                crate::world::pitch_to(delta.dot(forward), delta.dot(up))
            } else {
                let rise = aim_z - (units.z[row] + pivot.z);
                if (direct_air && aircraft.is_none())
                    || bp.unit(units.blueprint[row]).is_capital_ship()
                {
                    // AA mounts track overhead aircraft beyond a working arm's 35-degree limit,
                    // and a capital ship's turrets look straight down at the ground.
                    let world = FxVec2::new(yaw_origin.distance(aim), rise).angle();
                    if let Some((_, elevation)) = deck {
                        // The house rides a pitched hull: it elevates off the deck.
                        let lay = Angle::ZERO.delta_to(elevation) as i32;
                        // No lower than the house may dip before its rails meet the deck.
                        let floor = if weapon.depression.0 > 0 {
                            -(weapon.depression.0 as i32)
                        } else {
                            i32::MIN
                        };
                        Angle(lay.max(floor) as i16 as u16)
                    } else {
                        world
                    }
                } else {
                    crate::world::pitch_to(yaw_origin.distance(aim), rise)
                }
            };
            let rate = if weapon.trajectory == Trajectory::Ballistic || weapon.missile {
                weapon.turret_turn
            } else {
                weapon.turret_turn / 2
            };
            units.arm_pitch[row][slot] = units.arm_pitch[row][slot].turn_toward(want, rate);
            if weapon.trajectory == Trajectory::Ballistic || weapon.missile {
                pitched_on = units.arm_pitch[row][slot] == want;
            } else if (direct_air && aircraft.is_none()) || weapon.bore.is_some() {
                pitched_on =
                    units.arm_pitch[row][slot].delta_to(want).unsigned_abs() <= AIM_TOLERANCE;
            }
        }

        let aligned = if bomb {
            let nose = FxVec2::from_angle(units.heading[row]);
            let to = aim - pos;
            let half_salvo = Fx::ratio(
                (weapon.salvo.saturating_sub(1) / weapon.salvo_batch) as i64
                    * weapon.salvo_delay_ticks as i64,
                2,
            );
            let travel = units.speed[row] / DT;
            let release = travel * (fall + half_salvo);
            let along = to.dot(nose);
            let cross = to.cross(nose).abs();
            // Open the bay on the tick nearest the release line, so the centre
            // of the carpet lands on the target instead of two ticks short of
            // it. A bay that was not ready on the line may still open just past.
            // Once pickled, finish the rack while flying on past the release line.
            units.weapon_salvo_left[row][w] > 0
                || (along > Fx::ZERO
                    && along - release <= travel / 2
                    && along - release >= -travel * 2
                    && cross <= t.radius + weapon.splash.max(Fx::from_int(4)))
        } else if weapon.turret_turn == 0
            && (aircraft.is_none() || bp.unit(units.blueprint[row]).is_capital_ship())
            && (weapon.guided || weapon.vertical_launch)
        {
            // Launch cells on a hull (a capital ship's too), or a missile that homes: no
            // need to point the hull.
            true
        } else if weapon.turret_turn == 0 {
            // Hull-mounted: the unit turns itself when it is not driving somewhere.
            if units.flags[row] & flag::MOVING == 0 {
                if let Some(m) = bp.unit(units.blueprint[row]).motion {
                    units.heading[row] = units.heading[row].turn_toward(bearing, m.turn_rate);
                }
            }
            // A warship's spinal gun lies along the keel: the hull pitches onto the mark as
            // well, nose down onto the ground, and fires only once the bore is on it.
            if spinal_gun(bp.unit(units.blueprint[row]), w, weapon) {
                let want =
                    spinal_pitch(pos.extend(units.z[row]), weapon.muzzle.z, aim.extend(aim_z));
                let rate = hull_pitch_rate(bp.unit(units.blueprint[row]), w, weapon);
                units.arm_pitch[row][0] = units.arm_pitch[row][0].turn_toward(want, rate);
                pitched_on = units.arm_pitch[row][0].delta_to(want).unsigned_abs() <= AIM_TOLERANCE;
            }
            // A started salvo stays pickled through a bombing run as the target
            // slides aft; the first drop still needs a tight heading.
            // A torpedo steers itself onto the mark once it is out of the tube.
            let slack = if weapon.torpedo {
                Angle::from_degrees(40).0
            } else if units.weapon_salvo_left[row][w] > 0 {
                AIM_TOLERANCE * 16
            } else {
                AIM_TOLERANCE * 4
            };
            units.heading[row].delta_to(bearing).unsigned_abs() <= weapon.half_arc.min(slack)
        } else {
            // The arc is centred where the weapon faces (`facing`; aft for `rear`).
            let base = weapon.facing;
            let mut want = bearing - units.heading[row] - base;
            // A main turret that reaches only so far across the nose (`Motion::aim_arc`).
            let body_arc = bp
                .unit(units.blueprint[row])
                .motion
                .map_or(0x8000, |m| m.aim_arc);
            let half_arc = if w == 0 {
                weapon.half_arc.min(body_arc)
            } else {
                weapon.half_arc
            };
            // Whether the mark is inside the arc at all: at the end of its traverse a gun
            // is not on a mark that lies beyond it.
            let mut in_arc = true;
            if half_arc < 0x8000 {
                let d = Angle::ZERO.delta_to(want);
                in_arc = d.unsigned_abs() <= half_arc;
                // A ship's main gun cannot bear astern: a stopped ship turns its hull to bring
                // the mark into the arc. Its other mounts keep tracking by themselves.
                // A ship that fights broadside on (`Motion::broadside`) lays its beam to the
                // mark instead, on whichever side is nearer, so every battery bears.
                // A land unit with an `aim_arc` turns its body the same way.
                if (naval || body_arc < 0x8000) && w == 0 && units.flags[row] & flag::MOVING == 0 {
                    if let Some(m) = bp.unit(units.blueprint[row]).motion {
                        if m.broadside.0 > 0 {
                            let to = (t.pos - pos).angle();
                            let lay = if units.heading[row].delta_to(to) >= 0 {
                                to - m.broadside
                            } else {
                                to + m.broadside
                            };
                            units.heading[row] = units.heading[row].turn_toward(lay, m.turn_rate);
                            // Still coming round: the broadside waits for the hull too.
                            if units.heading[row] != lay {
                                units.volley[row][1] |= VOLLEY_HULL;
                            }
                        } else if d.unsigned_abs() > half_arc || body_arc < 0x8000 {
                            // A land unit with an `aim_arc` squares up to what it fights,
                            // not just until the mark is at the edge of its reach: the tail
                            // aims the rest of the way while the body comes round.
                            units.heading[row] =
                                units.heading[row].turn_toward(bearing - base, m.turn_rate);
                        }
                    }
                }
                want = Angle(d.clamp(-(half_arc as i32) as i16, half_arc as i16) as u16);
            }
            // Guns sharing a torso (`on_torso`) turn it together: the lead turns it, the
            // others ride where it points, or swing onto their own marks within their sway.
            let torso = if on_body && drives {
                units.weapon_yaw[row][0].turn_toward(want + base, weapon.turret_turn)
            } else {
                units.weapon_yaw[row][0]
            };
            let yaw = if swaying {
                let lay = within_sway(torso, want + base, weapon.sway);
                within_sway(
                    torso,
                    units.weapon_yaw[row][w].turn_toward(lay, weapon.turret_turn),
                    weapon.sway,
                )
            } else if on_body {
                torso
            } else {
                units.weapon_yaw[row][w].turn_toward(want + base, weapon.turret_turn)
            };
            if on_body {
                units.weapon_yaw[row][0] = torso;
            }
            units.weapon_yaw[row][w] = yaw;
            if weapon.sweep > 0 && !weapon.missile {
                // A sweeping gun fires while its mark is anywhere in the cone.
                (units.heading[row] + yaw).delta_to(bearing).unsigned_abs() <= weapon.sweep
            } else if weapon.missile {
                let off = (bearing - units.heading[row] - base)
                    .delta_to(Angle::ZERO)
                    .unsigned_abs();
                // An unguided rocket flies where its rack points, its arc solved along the
                // rack, so the rack (or the torso it rides) waits until it is on the mark:
                // fired while still coming round, the rockets flew off to the side and short,
                // a walker's down at its own feet.
                let on_mark =
                    (units.heading[row] + yaw).delta_to(bearing).unsigned_abs() <= AIM_TOLERANCE;
                if aircraft.is_none() {
                    weapon.guided || weapon.vertical_launch || on_mark
                } else {
                    off <= weapon.half_arc && (weapon.guided || on_mark)
                }
            } else if weapon.trajectory == Trajectory::Ballistic {
                // `want` is off the weapon's facing; the turret's yaw is off the nose.
                in_arc && yaw == want + base
            } else {
                (units.heading[row] + yaw).delta_to(bearing).unsigned_abs() <= AIM_TOLERANCE
            }
        };

        if w == 0 && bp.unit(units.blueprint[row]).visual.mesh == "assault_air" {
            // The rotary cannon is fixed to the nose. Heading alone permits
            // near-vertical shots as the aircraft passes over a ground target.
            // Match the rendered muzzle's bank and hull pitch before measuring
            // the complete firing solution, and require an inbound flight path.
            let local = weapon.muzzle;
            let rolled = crate::world::pitched(
                FxVec3::new(local.y, local.x, local.z),
                Some(FxVec3::ZERO),
                Angle(units.bank[row] as u16),
            );
            let offset = crate::world::pitched(
                FxVec3::new(rolled.y, rolled.x, rolled.z),
                Some(FxVec3::ZERO),
                units.arm_pitch[row][0],
            );
            // The gun swivels a little in its mount (`turret_turn`, `arc`), about its breech.
            let facing = units.heading[row] + units.weapon_yaw[row][0];
            let breech = FxVec2::new(ASSAULT_GUN_PIVOT_X, Fx::ZERO);
            let muzzle =
                (pos + breech.rotate(units.heading[row]) + (offset.xy() - breech).rotate(facing))
                    .extend(units.z[row] + offset.z);
            let elevation = FxVec2::from_angle(units.arm_pitch[row][0]);
            let gun = (FxVec2::from_angle(facing) * elevation.x).extend(elevation.y);
            let solution = aim.extend(t.z + t.height / 2) - muzzle;
            if (t.pos - pos).dot(units.air_velocity[row].xy()) <= Fx::ZERO
                || gun.dot(solution.normalize()) < FxVec2::from_angle(ASSAULT_GUN_CONE).x
            {
                return Ok(());
            }
        }

        // A sweeping gun fires while anything it may shoot is in the cone ahead of it,
        // its mark or not. Once it is streaming it keeps firing as it swings onto a new
        // mark outside the cone, walking the rounds across to it.
        let aligned = aligned
            || (weapon.sweep > 0
                && !weapon.missile
                && (self.state.units.streaming[row] || self.target_in_cone(row, w, weapon)));
        // The ground is in the way: keep the gun on it, and wait for a shot.
        let hidden = t.unit.is_some() && self.shot_blocked(row, w);
        // A lift ship's gun reaches the ground only along the line of sight (`Weapon::slant`).
        let origin = self.gun_origin(row, weapon);
        let slant_out = t.unit.is_some_and(|u| {
            !self.slant_reaches(row, u, weapon, origin.distance(t.pos) - t.radius)
        });
        let gap = origin.distance(t.pos) - t.radius;
        let in_reach = gap <= self.reach_onto(row, weapon, t.z + t.height / 2)
            && gap >= weapon.range_min - t.radius * 2;
        let units = &mut self.state.units;
        // A broadside battery that bears on its mark but is not ready holds up the others.
        let bears = weapon.volley
            && in_reach
            && !slant_out
            && (weapon.half_arc >= 0x8000
                || (bearing - units.heading[row] - weapon.facing)
                    .delta_to(Angle::ZERO)
                    .unsigned_abs()
                    <= weapon.half_arc);
        if !aligned || !pitched_on || hidden || units.weapon_cooldown[row][w] > 0 {
            if bears {
                units.volley[row][1] |= 1 << w;
            }
            return Ok(());
        }
        let spun = if weapon.spin_ramp > 0 {
            weapon.spin_ticks.div_ceil(3)
        } else {
            weapon.spin_ticks
        };
        if weapon.spin_ticks > 0 && units.spin[row][0] < spun {
            return Ok(());
        }
        // A barrel-timed rotary gun fires only as a barrel comes up to the top.
        if weapon.barrels > 0
            && !barrel_topped(weapon.barrels, units.spin[row][1], units.spin[row][2])
        {
            return Ok(());
        }
        if let Some(motion) = bp.unit(units.blueprint[row]).motion {
            if motion.deploy_ticks > 0
                && units.deploy[row] < motion.deploy_ticks
                && bp.unit(units.blueprint[row]).transport.is_none()
            {
                return Ok(());
            }
        }
        if !in_reach || slant_out {
            return Ok(());
        }
        // Broadside fire: on its mark and ready, it holds until the turn's `go`. Held on the
        // tick its countdown ran out, it is primed and fires without a fresh charge.
        let cooling = if weapon.volley && units.weapon_salvo_left[row][w] == 0 {
            let bit = 1u16 << w;
            let v = &mut units.volley[row];
            if v[4] == 0 {
                v[0] |= bit;
                if cooling {
                    v[2] |= bit;
                }
                return Ok(());
            }
            let primed = v[2] & bit != 0;
            v[2] &= !bit;
            cooling || primed
        } else {
            cooling
        };
        // On target and ready, but it has not just finished a countdown: charge first.
        if weapon.charge_ticks > 0 && !cooling && units.weapon_salvo_left[row][w] == 0 {
            units.weapon_cooldown[row][w] = weapon.charge_ticks;
            let at = units.pos[row].extend(units.z[row] + weapon.muzzle.z);
            self.events.push(SimEvent::WeaponCharging {
                unit: units.id(row),
                pos: at,
                owner: units.owner[row],
                blueprint: units.blueprint[row],
                weapon: w as u8,
            });
            if let (Some(storm), Some(m)) = (weapon.bore.and_then(|b| b.storm), mark.as_ref()) {
                self.events.push(SimEvent::StormCharging {
                    unit: units.id(row),
                    muzzle: at,
                    target: m.pos.extend(m.z),
                    on: m.unit.map(|t| units.id(t)),
                    radius: storm.radius,
                    ticks: weapon.charge_ticks,
                    owner: units.owner[row],
                    blueprint: units.blueprint[row],
                    weapon: w as u8,
                });
            }
            return Ok(());
        }

        if units.weapon_salvo_left[row][w] == 0 {
            units.weapon_salvo_left[row][w] = weapon.salvo;
        }
        if weapon.sweep > 0 && !weapon.missile {
            units.streaming[row] = true;
        }
        // Zero delay dumps the rest of the salvo on this tick.
        let volley = if weapon.salvo_delay_ticks == 0 {
            units.weapon_salvo_left[row][w]
        } else {
            weapon.salvo_batch.min(units.weapon_salvo_left[row][w])
        };
        units.weapon_salvo_left[row][w] -= volley;
        units.weapon_cooldown[row][w] = if units.weapon_salvo_left[row][w] > 0 {
            weapon.salvo_delay_ticks.max(1) as u16
        } else if weapon.spin_ramp > 100 && weapon.spin_ticks > 0 {
            // Slower while it is still spinning up: `spin_ramp` times at a standstill, its
            // own reload at full spin.
            let (full, spin) = (
                weapon.spin_ticks as u32,
                units.spin[row][0].min(weapon.spin_ticks) as u32,
            );
            let stretch = 100 * full + (weapon.spin_ramp as u32 - 100) * (full - spin);
            ((weapon.reload_ticks as u32 * stretch).div_ceil(100 * full)).clamp(1, u16::MAX as u32)
                as u16
        } else {
            weapon.reload_ticks
        };
        // Twin barrels of the same gun take turns. The later one waits half a
        // reload whenever the earlier one fires and it is ready, so they do
        // not dump both shots on the same tick after sitting idle.
        if units.weapon_salvo_left[row][w] == 0 {
            let stagger = weapon.reload_ticks / 2 + 1;
            if stagger > 1 {
                let twin = weapon.name.clone();
                let later: Vec<usize> = bp
                    .unit(units.blueprint[row])
                    .weapons
                    .iter()
                    .enumerate()
                    .skip(w + 1)
                    .filter(|(_, other)| other.name == twin)
                    .map(|(i, _)| i)
                    .collect();
                for i in later {
                    if units.weapon_cooldown[row][i] == 0 && units.weapon_salvo_left[row][i] == 0 {
                        units.weapon_cooldown[row][i] = stagger;
                    }
                }
            }
        }

        let facing = units.heading[row]
            + if weapon.vertical_launch {
                Angle::ZERO
            } else {
                units.weapon_yaw[row][w]
            };
        // A sweeping gun fires down its barrel, not at its target: the stream walks onto
        // what is in the cone as the torso comes round.
        let aim = if weapon.sweep > 0 && !weapon.missile {
            pos + FxVec2::from_angle(facing) * pos.distance(aim)
        } else {
            aim
        };
        let unit_z = units.z[row];
        let travel = (units.pos[row] - units.prev_pos[row]).extend(unit_z - units.prev_z[row]);
        let arm_pitch = units.arm_pitch[row][slot];
        let torso = units.heading[row] + units.weapon_yaw[row][0];
        let owner = units.owner[row];
        let id = units.id(row);
        let blueprint = units.blueprint[row];
        let left = units.weapon_salvo_left[row][w];
        let first_tube =
            weapon.salvo.saturating_sub(1) as usize - left as usize - (volley as usize - 1);
        // Weapons on the first weapon's arm (the same elbow) are carried up and down with it.
        let arm = bp
            .unit(blueprint)
            .weapons
            .first()
            .and_then(|first| first.pivot);
        let pivot = weapon
            .pivot
            .filter(|p| w == 0 || weapon.mount || Some(*p) == arm);

        for i in 0..volley {
            let tube = first_tube + i as usize;
            // A salvo longer than the rack goes round its tubes again.
            let local = if weapon.muzzles.is_empty() {
                weapon.muzzle
            } else {
                weapon.muzzles[tube % weapon.muzzles.len()]
            };
            let local = if weapon.rear {
                FxVec3::new(-local.x, -local.y, local.z)
            } else {
                local
            };
            let at = crate::world::pitched(local, pivot, arm_pitch);
            // Aircraft turrets yaw around their chin mount, not the hull origin.
            let yaw_pivot = if bp.unit(blueprint).visual.mesh == "assault_air" && w == 0 {
                // The Thunderhead's cannon swivels about its breech (`ASSAULT_GUN_PIVOT_X`).
                Some(FxVec3::new(ASSAULT_GUN_PIVOT_X, Fx::ZERO, Fx::ZERO))
            } else if aircraft.is_some() || naval {
                // A ship's turret or mount turns about its own pivot on the deck.
                pivot
            } else {
                None
            };
            let muzzle_xy = if let Some(p) = yaw_pivot {
                pos + p.xy().rotate(units.heading[row]) + (at.xy() - p.xy()).rotate(facing)
            } else if let (true, Some(p)) = (weapon.mount, pivot) {
                // A shoulder gun: its trunnion rides the torso, the barrel turns about it.
                pos + p.xy().rotate(torso) + (at.xy() - p.xy()).rotate(facing)
            } else {
                pos + bp.unit(blueprint).turret_point(
                    FxVec2::new(at.x, at.y),
                    units.heading[row],
                    facing,
                )
            };
            let muzzle = if aircraft.is_some() {
                let offset =
                    (muzzle_xy - pos).rotate(Angle(0u16.wrapping_sub(units.heading[row].0)));
                let bank = Angle(units.bank[row] as u16);
                let rolled = crate::world::pitched(
                    FxVec3::new(offset.y, offset.x, at.z),
                    Some(FxVec3::ZERO),
                    bank,
                );
                let hull_pitch = if bp.unit(blueprint).visual.mesh == "assault_air" {
                    arm_pitch
                } else if hull_pitched(bp.unit(blueprint)) {
                    // A warship laying its spinal gun pitches the whole hull (`spinal_gun`).
                    units.arm_pitch[row][0]
                } else if aircraft.is_some_and(|m| m.hover) {
                    // Hovering hulls lean with forward travel rather than yaw.
                    let forward_step = (units.pos[row] - units.prev_pos[row])
                        .dot(FxVec2::from_angle(units.heading[row]));
                    Angle((-forward_step * 365).floor_int().clamp(-1252, 1252) as i16 as u16)
                } else {
                    crate::world::pitch_to(
                        (units.pos[row] - units.prev_pos[row])
                            .length()
                            .max(Fx::from_int(2)),
                        units.z[row] - units.prev_z[row],
                    )
                };
                let local = crate::world::pitched(
                    FxVec3::new(rolled.y, rolled.x, rolled.z),
                    Some(FxVec3::ZERO),
                    hull_pitch,
                );
                (pos + local.xy().rotate(units.heading[row])).extend(unit_z + local.z)
            } else if bp.unit(blueprint).is_mobile() {
                // The hull leans with the ground under it, and the turret with it.
                let offset = crate::world::leaned(
                    &self.terrain,
                    pos,
                    bp.unit(blueprint).radius,
                    units.heading[row],
                    (muzzle_xy - pos).extend(at.z),
                );
                (pos + offset.xy()).extend(unit_z + offset.z)
            } else {
                muzzle_xy.extend(unit_z + at.z)
            };
            // Use the same pitched muzzle for the cannon's launch solution
            // that was checked against its firing cone above.
            let muzzle_xy = if aircraft.is_none() || bp.unit(blueprint).visual.mesh == "assault_air"
            {
                muzzle.xy()
            } else {
                muzzle_xy
            };

            // Refine from the actual barrel tip after yaw and elevation are applied.
            let launch_aim = if direct_air {
                direct_air_aim(muzzle, target_center, target_velocity, t.turn, step)
            } else {
                aim.extend(aim_z)
            };
            let aim = launch_aim.xy();
            let aim_z = launch_aim.z;
            let (vel, ticks) = if bomb {
                // A dropped bomb inherits forward velocity. It never steers
                // sideways or launches upward to solve an artillery trajectory.
                let velocity = FxVec2::from_angle(units.heading[row]) * (units.speed[row] / DT);
                let velocity = if weapon.burn_ticks > 0 && weapon.spread > 0 {
                    let yaw = self.state.rng.below(weapon.spread as u32 * 2 + 1) as i32
                        - weapon.spread as i32;
                    velocity.rotate(Angle(yaw as i16 as u16))
                        * (Fx::ratio(85, 100) + Fx::ratio(self.state.rng.below(31) as i64, 100))
                } else {
                    velocity
                };
                (velocity.extend(Fx::ZERO), fall_ticks + DT * 3)
            } else if weapon.guided {
                let dir = if weapon.vertical_launch {
                    // Out along the cell: straight up, or leaning toward the bow (`cant`).
                    let lean = FxVec2::from_angle(Angle::from_degrees(90) - weapon.cant);
                    (FxVec2::from_angle(units.heading[row]) * lean.x).extend(lean.y)
                } else if weapon.skim > Fx::ZERO {
                    // A sea skimmer leaves level and settles to its height (`naval_arms.rs`).
                    (aim - muzzle_xy).extend(Fx::ZERO).normalize()
                } else {
                    (aim - muzzle_xy).extend(aim_z - muzzle.z).normalize()
                };
                let kick = if weapon.cold_launch_ticks > 0 {
                    cold_lob_speed(0, weapon.cold_launch_ticks)
                } else if aircraft.is_some() {
                    // Off a rail under a wing it leaves at the aircraft's own speed, or the
                    // aircraft would fly into its own rockets.
                    (step * Fx::ratio(3, 20)).max(units.speed[row] / DT)
                } else {
                    step * Fx::ratio(3, 20)
                };
                (
                    dir * kick,
                    (weapon.range_max * 3 / step).ceil_int() + DT * 3,
                )
            } else if weapon.missile && weapon.trajectory == Trajectory::Ballistic {
                // Out the elevated tubes, along the hull. Speed is chosen so that
                // parabola comes down at the target's range; the shot does not turn
                // toward the aim point.
                let mut ahead = FxVec2::from_angle(facing);
                let mut range = (aim - muzzle_xy).dot(ahead).max(Fx::ONE);
                if weapon.spread > 0 {
                    let (yaw, along) = aim_error(&mut self.state.rng, weapon.spread);
                    ahead = ahead.rotate(Angle(yaw as i16 as u16));
                    // Short or long by the same angular budget as a shell.
                    range *= Fx::ONE + Angle(along as i16 as u16).sin();
                }
                let rise = aim_z - muzzle.z;
                let tan_rake = (weapon.rake.0 > 0)
                    .then(|| {
                        let r = FxVec2::from_angle(weapon.rake);
                        r.y / r.x.max(Fx::EPSILON)
                    })
                    .or(pivot.map(|p| {
                        let along = FxVec2::new(at.x - p.x, at.z - p.z);
                        along.y / along.x.max(Fx::ONE)
                    }))
                    .unwrap_or(Fx::ratio(1192, 1000));
                let need = (range * tan_rake - rise).max(GRAVITY);
                // Half of sqrt(1 + 8 need / g) - 1, rounded up, in fixed point (never negative).
                let v = (Fx::ONE + need * 8 / GRAVITY).sqrt() - Fx::ONE;
                let n = (v.raw().max(0) as u64).div_ceil(2 * Fx::ONE.raw() as u64);
                let n = (n as i32).max(1);
                let horiz = range / n;
                let vz = horiz * tan_rake;
                ((ahead * horiz).extend(vz), n + 20)
            } else {
                let mut delta = aim - muzzle_xy;
                let mut aim_z = aim_z;
                if weapon.spread > 0 {
                    let (yaw, off) = aim_error(&mut self.state.rng, weapon.spread);
                    let off = Angle(off as i16 as u16).sin();
                    delta = delta.rotate(Angle(yaw as i16 as u16));
                    match weapon.trajectory {
                        // A direct shot misses high or low as well as to the side. A bore's
                        // tracer strays only to the side: its channel still runs past the mark.
                        Trajectory::Direct if weapon.bore.is_some() => {}
                        Trajectory::Direct => {
                            aim_z += delta.extend(aim_z - muzzle.z).length() * off;
                        }
                        // A shell misses short or long: the same angular budget laid
                        // along the ground.
                        Trajectory::Ballistic => delta = delta * (Fx::ONE + off),
                    }
                }
                let dist = delta.length().max(Fx::ONE);
                match weapon.trajectory {
                    Trajectory::Direct => {
                        let dir = delta.extend(aim_z - muzzle.z).normalize();
                        (
                            dir * step,
                            (delta
                                .extend(aim_z - muzzle.z)
                                .length()
                                .max(weapon.range_max)
                                * Fx::ratio(13, 10)
                                / step)
                                .ceil_int()
                                + 2,
                        )
                    }
                    Trajectory::Ballistic => {
                        let n = ballistic_ticks(dist, weapon);
                        let drop = GRAVITY * (n * (n + 1) / 2);
                        let vz = (aim_z - muzzle.z + drop) / n;
                        (FxVec2::new(delta.x / n, delta.y / n).extend(vz), n + 20)
                    }
                }
            };
            let (muzzle, vel, ticks) = if weapon.torpedo {
                // Dropped from an aircraft, it keeps the aircraft's way on it as it falls.
                let carried = if aircraft.is_some() {
                    FxVec2::from_angle(units.heading[row]) * (units.speed[row] / DT)
                } else {
                    FxVec2::ZERO
                };
                crate::naval::torpedo_launch(
                    muzzle,
                    facing,
                    weapon,
                    self.terrain.water_level(),
                    t.unit.is_none().then_some(aim),
                    carried,
                )
            } else if bomb && weapon.salvo == 1 {
                // The bay opens on the tick nearest the release line, up to half a
                // tick of flight either side of it; a lone bomb is let go from where
                // the line was crossed, or a fast bomber's bomb lands that much off.
                let nose = FxVec2::from_angle(units.heading[row]);
                let travel = units.speed[row] / DT;
                let off = ((aim - pos).dot(nose) - travel * fall).clamp(-travel, travel);
                (muzzle + (nose * off).extend(Fx::ZERO), vel, ticks)
            } else {
                (muzzle, vel, ticks)
            };
            self.state.projectiles.spawn(
                muzzle,
                vel,
                owner,
                id,
                blueprint,
                w as u8,
                ticks.clamp(1, u16::MAX as i32) as u16,
            )?;
            let shot = self.state.projectiles.len() - 1;
            self.state.projectiles.target[shot] = t.unit.map_or(Handle::NONE, |t| units.id(t));
            self.state.projectiles.mark[shot] = aim.extend(aim_z);
            if weapon.missile {
                self.state.projectiles.hp[shot] = weapon.casing_hp();
            }
            // A missile out of a dived hull boils the surface and paints the boat.
            if weapon.missile && unit_z + bp.unit(blueprint).height < self.terrain.water_level() {
                units.revealed[row] = crate::naval_arms::LAUNCH_REVEAL;
                self.events.push(SimEvent::DivedLaunch {
                    pos: muzzle,
                    blueprint,
                    weapon: w as u8,
                });
            }
            self.muzzles.push(muzzle);
            self.events.push(SimEvent::ShotFired {
                pos: muzzle,
                vel,
                travel,
                color: weapon.color,
                owner,
                blueprint,
                weapon: w as u8,
            });
            if let Some(sabot) = weapon.sabot {
                // The spent casing leaves by the gun's port (`titan.rs`): the port and the
                // throw are in the gun's frame, so they turn, pitch and lean with it the way
                // the muzzle does, and the unit's own way is carried into the throw.
                let unit = bp.unit(blueprint);
                let gun = |v: FxVec3| {
                    let v = crate::world::pitched(v, Some(FxVec3::ZERO), arm_pitch);
                    let v = v.xy().rotate(facing).extend(v.z);
                    if aircraft.is_none() && unit.is_mobile() {
                        crate::world::leaned(&self.terrain, pos, unit.radius, units.heading[row], v)
                    } else {
                        v
                    }
                };
                let seed = self.state.tick.wrapping_mul(2_654_435_761)
                    ^ id.0.wrapping_mul(40_503)
                    ^ w as u32;
                // Out of the port along the gun's `throw`, every case alike: they part
                // only in the air (`FallingSabot::drift`, `tumble`).
                let thrown = crate::titan::FallingSabot::thrown(
                    muzzle + gun(sabot.port - local),
                    gun(sabot.throw) * sabot.kick,
                    travel * Fx::from_int(DT),
                    facing,
                    owner,
                    id,
                    blueprint,
                    w as u8,
                    seed,
                );
                self.events.push(SimEvent::SabotThrown {
                    from: thrown.from,
                    vel: thrown.vel,
                    blueprint,
                    weapon: w as u8,
                });
                self.state.sabots.push(thrown);
            }
        }
        if t.scatter > Fx::ZERO && self.state.units.weapon_salvo_left[row][w] == 0 {
            // The salvo is away: the gun slews to its next point before it fires again,
            // and a bomber comes round for its next run somewhere else in the circle.
            self.pick_bombard_aim(row, self.bombard_slot(row, w));
        }
        Ok(())
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

    pub(crate) fn run_projectiles(&mut self) -> Result<(), SimError> {
        self.guide_and_intercept_missiles();
        self.steer_torpedoes();
        self.steer_interceptors();
        let count = self.state.projectiles.len();
        let this = &*self;
        let hits: Vec<Vec<Hit>> = self.pool.parallel_map_chunks(count, CHUNK, |_, range| {
            let mut out = Vec::new();
            for i in range {
                if let Some(hit) = this.sweep_projectile(i) {
                    out.push(hit);
                }
            }
            out
        });
        let hits: Vec<Hit> = hits.into_iter().flatten().collect();

        let p = &mut self.state.projectiles;
        for i in 0..count {
            if self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize].trajectory
                == Trajectory::Ballistic
            {
                p.vel[i].z -= GRAVITY;
            }
            p.age[i] = p.age[i].saturating_add(1);
            p.pos[i] += p.vel[i];
            p.ticks_left[i] = p.ticks_left[i].saturating_sub(1);
        }

        let mut remove: Vec<usize> = Vec::with_capacity(hits.len());
        for hit in &hits {
            let i = hit.projectile;
            let p = &self.state.projectiles;
            let weapon = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            #[expect(
                clippy::float_arithmetic,
                clippy::disallowed_types,
                clippy::disallowed_methods,
                reason = "presentation: spent shots and stream tails are drawn by the render mirror only"
            )]
            {
                let travel = self.unit_travel(p.source[i]);
                let back = travel.map(|t| -t);
                self.spent.push(crate::mirror::SpentShot {
                    cold: weapon.cold_launch_ticks > 0 && p.age[i] <= weapon.cold_launch_ticks,
                    from: p.prev_pos[i],
                    to: hit.seen,
                    after: hit.after,
                    lead: crate::mirror::launch_shift(back, p.age[i] as f32 - 1.0),
                    blueprint: p.blueprint[i],
                    weapon: p.weapon[i],
                });
                if weapon.rounds > 1 {
                    let lands = p.age[i] as f32 - 1.0 + hit.after.to_f32();
                    self.streams.push(crate::mirror::StreamTail::of(
                        p, i, weapon, lands, true, travel,
                    ));
                }
            }
            self.state.projectiles.pos[i] = hit.point;
            self.apply_impact(i, hit)?;
            remove.push(i);
        }
        // A bore's tracer that met nothing still carries the charge: it strikes where it ends.
        let spent: Vec<usize> = (0..count)
            .filter(|&i| {
                let p = &self.state.projectiles;
                p.ticks_left[i] == 0
                    && !remove.contains(&i)
                    && self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize]
                        .bore
                        .is_some()
            })
            .collect();
        for i in spent {
            let to = self.state.projectiles.pos[i];
            self.bore_discharge(i, to, None, Fx::ONE)?;
        }
        let p = &self.state.projectiles;
        for i in (0..count).filter(|&i| p.ticks_left[i] == 0 && !remove.contains(&i)) {
            let weapon = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            #[expect(
                clippy::disallowed_types,
                reason = "presentation: stream tails are drawn by the render mirror only"
            )]
            if weapon.rounds > 1 {
                let lands = p.age[i] as f32;
                let travel = self.unit_travel(p.source[i]);
                self.streams.push(crate::mirror::StreamTail::of(
                    p, i, weapon, lands, false, travel,
                ));
            }
        }
        remove.extend((0..count).filter(|&i| p.ticks_left[i] == 0));
        remove.sort_unstable();
        remove.dedup();
        for &i in remove.iter().rev() {
            self.state.projectiles.swap_remove(i);
        }
        Ok(())
    }

    fn guide_and_intercept_missiles(&mut self) {
        let mut killed = Vec::new();
        let defenders: Vec<_> = self
            .state
            .units
            .slots
            .iter()
            .filter(|&r| {
                self.state.units.is_active(r)
                    && self.state.units.health[r] > Fx::ZERO
                    && self.bp(r).anti_missile > Fx::ZERO
                    && !self.state.units.has_flag(r, flag::PASSIVE)
                    && self.state.units.fire_state[r] != FireState::HoldFire
            })
            .collect();
        for r in defenders {
            if self.state.units.intercept_cooldown[r] > 0 {
                self.state.units.intercept_cooldown[r] -= 1;
                continue;
            }
            let center = self.state.units.pos[r].extend(self.state.units.z[r]);
            let owner = self.state.units.owner[r];
            let radius = self.bp(r).anti_missile;
            if self.state.players[owner as usize].efficiency <= Fx::ZERO {
                continue;
            }
            // Each laser (`anti_missile_lasers`) takes a missile of its own from a mount
            // of its own.
            let mut lased: Vec<usize> = Vec::new();
            let mut used: Vec<FxVec3> = Vec::new();
            for _ in 0..self.bp(r).anti_missile_lasers {
                // Stay on a casing already burning. Otherwise the nearest full one.
                let mut best: Option<(usize, bool, Fx, Fx)> = None;
                for i in 0..self.state.projectiles.len() {
                    if killed.contains(&i) || lased.contains(&i) {
                        continue;
                    }
                    let p = &self.state.projectiles;
                    let w = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
                    if !w.missile || !self.are_enemies(owner, p.owner[i]) {
                        continue;
                    }
                    let d2 = (p.pos[i] - center).length_sq();
                    if d2 > radius * radius {
                        continue;
                    }
                    let max = w.casing_hp();
                    let hp = if p.hp[i] > Fx::ZERO { p.hp[i] } else { max };
                    let burning = hp < max;
                    let take = match best {
                        None => true,
                        Some((_, was, best_hp, best_d)) => {
                            (burning && !was)
                                || (burning == was
                                    && (hp < best_hp || (hp == best_hp && d2 < best_d)))
                        }
                    };
                    if take {
                        best = Some((i, burning, hp, d2));
                    }
                }
                let Some((i, _, hp, _)) = best else {
                    break;
                };
                lased.push(i);
                let left = hp - LASER_BITE;
                let dead = left <= Fx::ZERO;
                let to = self.state.projectiles.pos[i];
                // A hull with emitters of its own fires from the one nearest the missile.
                let heading = self.state.units.heading[r];
                let from = self
                    .bp(r)
                    .anti_missile_mounts
                    .iter()
                    .map(|m| center + m.xy().rotate(heading).extend(m.z))
                    .filter(|at| !used.contains(at))
                    .min_by_key(|at| (*at - to).length_sq())
                    .unwrap_or(center);
                used.push(from);
                self.events.push(SimEvent::MissileLased {
                    from,
                    to,
                    killed: dead,
                });
                if dead {
                    killed.push(i);
                    self.state.units.intercept_cooldown[r] = LASER_GAP;
                } else {
                    self.state.projectiles.hp[i] = left;
                }
            }
        }
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

    /// Finds what projectile `i` runs into during this tick's step, if anything.
    fn sweep_projectile(&self, i: usize) -> Option<Hit> {
        let p = &self.state.projectiles;
        let weapon = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
        let mut vel = p.vel[i];
        if weapon.trajectory == Trajectory::Ballistic {
            vel.z -= GRAVITY;
        }
        let from = p.pos[i];
        let to = from + vel;
        let mut best_t = Fx::MAX;
        let mut best = None;
        // A torpedo with nothing left to run at bursts where it is (`naval.rs`).
        if let Some(t) = self.torpedo_burst(i, weapon) {
            let point = from + vel * t;
            best_t = t;
            best = Some(Hit {
                projectile: i,
                point,
                unit: None,
                shield: None,
                after: t,
                seen: point,
            });
        }
        // A fast slug can cover more than one raycast's reach in a tick.
        let ground = self.terrain.raycast_split(from, to).map(|point| {
            let t = (point.xy() - from.xy()).length() / vel.xy().length().max(Fx::EPSILON);
            (point, t)
        });
        if let Some((point, t)) = ground.filter(|g| g.1 < best_t) {
            best_t = t;
            best = Some(Hit {
                projectile: i,
                point,
                unit: None,
                shield: None,
                after: best_t.clamp(Fx::ZERO, Fx::ONE),
                seen: point,
            });
        }
        // Shots stop on the water; nothing carries on down to the bed. An air-dropped
        // torpedo goes in and runs on (`naval.rs`).
        let water = self.terrain.water_level();
        if from.z > water && to.z <= water && !weapon.torpedo {
            let t = (from.z - water) / (from.z - to.z);
            if t < best_t {
                best_t = t;
                let point = from + vel * t;
                best = Some(Hit {
                    projectile: i,
                    point,
                    unit: None,
                    shield: None,
                    after: t.clamp(Fx::ZERO, Fx::ONE),
                    seen: point,
                });
            }
        }
        // Lobbed shells pass over things on the way up.
        if weapon.trajectory == Trajectory::Direct || vel.z < Fx::ZERO {
            let seg = vel.xy();
            let len_sq = seg.length_sq().max(Fx::EPSILON);
            let mid = from.xy() + seg * Fx::HALF;
            self.index
                .query(mid, seg.length() / 2 + weapon.proximity, kind::UNIT, |e| {
                    let row = e.row as usize;
                    let units = &self.state.units;
                    // An interceptor runs at torpedoes, never at hulls.
                    if !self.unit_entry_is_current(e)
                        || weapon.intercepts
                        || !self.are_enemies(p.owner[i], units.owner[row])
                        || !self.weapon_reaches(row, weapon)
                        || crate::bore::passes_through(weapon, p.target[i], units.id(row))
                    {
                        return true;
                    }
                    let height = self.bp(row).height;
                    // A torpedo meets a hull below its waterline, down to the keel.
                    let keel = if weapon.torpedo {
                        height * crate::naval::KEEL
                    } else {
                        Fx::ZERO
                    };
                    let low = units.z[row] - keel - Fx::ONE - weapon.proximity;
                    let high = units.z[row] + height + Fx::ONE + weapon.proximity;
                    // Restrict the closest horizontal pass to the part of the
                    // segment inside the hull's vertical slab. A steep AA shot
                    // can cross the hull before its closest horizontal pass.
                    let (enter, leave) = if vel.z.abs() > Fx::EPSILON {
                        let a = (low - from.z) / vel.z;
                        let b = (high - from.z) / vel.z;
                        (a.min(b).max(Fx::ZERO), a.max(b).min(Fx::ONE))
                    } else {
                        if from.z < low || from.z > high {
                            return true;
                        }
                        (Fx::ZERO, Fx::ONE)
                    };
                    if enter > leave {
                        return true;
                    }
                    let t = ((e.pos - from.xy()).dot(seg) / len_sq).clamp(enter, leave);
                    let point = from + vel * t;
                    let inside = point.xy().distance_sq(e.pos)
                        <= (e.radius + weapon.proximity) * (e.radius + weapon.proximity);
                    if inside && t < best_t {
                        best_t = t;
                        let depth = (e.radius * e.radius - point.xy().distance_sq(e.pos))
                            .max(Fx::ZERO)
                            .sqrt()
                            * Fx::ratio(8, 10);
                        let back = (depth / vel.length().max(Fx::EPSILON)).min(t);
                        // A hull field is the unit: shots that would land on the
                        // armour land on the wrap instead. A torpedo runs under it.
                        let on_hull = !weapon.torpedo
                            && self.shield_blocking(row)
                            && self.bp(row).shield.is_some_and(|s| s.is_hull());
                        best = Some(Hit {
                            projectile: i,
                            point,
                            unit: if on_hull { None } else { Some(row) },
                            shield: if on_hull { Some(row) } else { None },
                            after: t - back,
                            seen: from + vel * (t - back),
                        });
                    }
                    true
                });
        }
        self.sweep_shields(i, from, vel, &mut best_t, &mut best);
        best
    }

    /// Incoming fire hits the first enemy dome along the step. Shots that
    /// already started inside a bubble pass through it.
    fn sweep_shields(
        &self,
        projectile: usize,
        from: FxVec3,
        vel: FxVec3,
        best_t: &mut Fx,
        best: &mut Option<Hit>,
    ) {
        let owner = self.state.projectiles.owner[projectile];
        // A torpedo in the water runs under the skirt of every dome. One still
        // falling from an aircraft breaks on it like anything else.
        let p = &self.state.projectiles;
        if self.blueprints.unit(p.blueprint[projectile]).weapons[p.weapon[projectile] as usize]
            .torpedo
            && from.z <= self.terrain.water_level()
        {
            return;
        }
        for row in self.state.units.slots.iter() {
            if !self.shield_blocking(row) || !self.are_enemies(owner, self.state.units.owner[row]) {
                continue;
            }
            let spec = self.bp(row).shield.unwrap();
            // Hull fields are the unit they wrap; the unit sweep already
            // charges them. A dome is the only thing that stops a shot early.
            if spec.is_hull() {
                continue;
            }
            let radius = self.dome_radius(row);
            let center = self.state.units.pos[row].extend(self.state.units.z[row]);
            let floor = self.dome_floor();
            if in_dome(from, center, radius, floor) {
                continue;
            }
            if let Some(t) = ray_dome(from, vel, center, radius, floor) {
                if t < *best_t {
                    *best_t = t;
                    let point = from + vel * t;
                    *best = Some(Hit {
                        projectile,
                        point,
                        unit: None,
                        shield: Some(row),
                        after: t,
                        seen: point,
                    });
                }
            }
        }
    }

    fn apply_impact(&mut self, projectile: usize, hit: &Hit) -> Result<(), SimError> {
        let p = &self.state.projectiles;
        let (owner, source) = (p.owner[projectile], p.source[projectile]);
        let blueprints = self.blueprints.clone();
        let weapon =
            &blueprints.unit(p.blueprint[projectile]).weapons[p.weapon[projectile] as usize];
        let target_motion = hit.unit.map_or(FxVec3::ZERO, |row| {
            let u = &self.state.units;
            (u.pos[row] - u.prev_pos[row]).extend(u.z[row] - u.prev_z[row])
        });
        self.events.push(SimEvent::Impact {
            pos: hit.seen,
            target_motion,
            splash: weapon.splash,
            color: weapon.color,
            after: hit.after,
            on_unit: hit.unit.is_some(),
            on_shield: hit.shield.is_some(),
            blueprint: p.blueprint[projectile],
            weapon: p.weapon[projectile],
        });
        if let Some(bore) = weapon.bore {
            self.bore_discharge(projectile, hit.point, hit.unit.or(hit.shield), hit.after)?;
            if let Some(storm) = bore.storm {
                // What is left of the charge spreads out from the hit (`titan.rs`).
                self.state.storms.push(crate::titan::DischargeStorm::struck(
                    hit.point,
                    storm,
                    owner,
                    source,
                    weapon.target_mask,
                ));
            }
        }
        if weapon.discharge > 0.0 {
            // A charged shell: its charge strikes back up the last of its flight.
            #[expect(
                clippy::disallowed_methods,
                reason = "presentation: the discharge arc only goes into a SimEvent the game draws"
            )]
            let back =
                self.state.projectiles.vel[projectile].normalize() * Fx::from_f32(weapon.discharge);
            self.events.push(SimEvent::ShellDischarge {
                from: hit.seen - back,
                to: hit.seen,
                after: hit.after,
                blueprint: self.state.projectiles.blueprint[projectile],
                weapon: self.state.projectiles.weapon[projectile],
            });
        }
        if let Some(row) = hit.shield {
            self.damage_shield(row, weapon.damage);
            return Ok(());
        }
        if weapon.splash > Fx::ZERO {
            let victims: Vec<_> = self
                .state
                .units
                .slots
                .iter()
                .filter(|&r| {
                    // A half-built site stands in the open and takes the blast too;
                    // only what is still inside a factory is out of reach.
                    !self.state.units.has_flag(r, flag::IN_FACTORY)
                        && self.are_enemies(owner, self.state.units.owner[r])
                        && (self.bp(r).target_categories() & weapon.target_mask != 0
                            || (weapon.torpedo && self.on_seabed(r)))
                        && (hit.unit == Some(r) || self.blast_reaches(hit.point, r, weapon))
                })
                .collect();
            // Snapshot all protection before charging any field. A blast that
            // exhausts a shield is still absorbed for every victim of that blast.
            let victims: Vec<_> = victims
                .into_iter()
                .map(|r| {
                    let target = self.state.units.pos[r]
                        .extend(self.state.units.z[r] + self.bp(r).height / 2);
                    // A torpedo bursts under the skirt: no shield takes it.
                    let blocker = if weapon.torpedo {
                        None
                    } else {
                        self.blast_blocker(hit.point, target, Some(r))
                    };
                    (r, blocker)
                })
                .collect();
            let mut felled = Vec::new();
            // A torpedo bursts under water: it fells no trees and scorches nothing.
            let on_land = weapon.target_mask & mc_data::cat::AIR == 0 && !weapon.torpedo;
            if on_land {
                self.prop_index
                    .query(hit.point.xy(), weapon.splash, kind::PROP, |e| {
                        let prop = e.row as usize;
                        let xy = self.map.props[prop].pos;
                        if self
                            .blast_blocker(
                                hit.point,
                                xy.extend(self.terrain.height_at(xy) + Fx::ONE),
                                None,
                            )
                            .is_none()
                        {
                            felled.push(prop);
                        }
                        true
                    });
            }
            let mut charged = Vec::new();
            for (r, blocker) in victims {
                if let Some(shield) = blocker {
                    if !charged.contains(&shield) {
                        self.damage_shield(shield, weapon.damage);
                        charged.push(shield);
                    }
                    continue;
                }
                if !weapon.torpedo
                    && self.shield_blocking(r)
                    && self.bp(r).shield.is_some_and(|s| s.is_hull())
                {
                    self.damage_shield(r, weapon.damage);
                } else {
                    self.damage_unit(r, weapon.damage, owner, source);
                }
            }
            if weapon.burn_ticks > 0 {
                // The patch stays where the bomb landed. Units burn by standing in it.
                let radius =
                    (Fx::from_int(4) + weapon.splash * Fx::ratio(11, 20)).min(Fx::from_int(16));
                self.state.fires.push(
                    hit.point.xy(),
                    hit.point.z,
                    radius,
                    weapon.burn_ticks,
                    owner,
                    source,
                    weapon.target_mask,
                )?;
            }
            if on_land {
                for prop in felled {
                    if self.map.props[prop].kind.is_tree() {
                        self.state.props_dead[prop / 64] |= 1 << (prop % 64);
                    }
                }
                self.add_stain(hit.point.xy(), weapon.splash, 96)?;
            }
        } else {
            if let Some(row) = hit.unit {
                self.damage_unit(row, weapon.damage, owner, source);
            } else {
                self.add_stain(
                    hit.point.xy(),
                    Fx::from_int(2) + weapon.damage.sqrt() / 4,
                    40,
                )?;
            }
        }
        Ok(())
    }

    /// First live membrane crossed by blast propagation, regardless of team.
    /// The source and victim can share a dome: no membrane lies between them.
    pub(crate) fn blast_blocker(
        &self,
        from: FxVec3,
        to: FxVec3,
        victim: Option<usize>,
    ) -> Option<usize> {
        let mut best = None;
        let mut first = Fx::MAX;
        for row in self.state.units.slots.iter() {
            if !self.shield_blocking(row) {
                continue;
            }
            let spec = self.bp(row).shield.unwrap();
            if spec.is_hull() {
                if victim == Some(row) && best.is_none() {
                    best = Some(row);
                }
                continue;
            }
            let center = self.state.units.pos[row].extend(self.state.units.z[row]);
            let a = from - center;
            let radius = self.dome_radius(row);
            let floor = self.dome_floor();
            if in_dome(from, center, radius, floor) && in_dome(to, center, radius, floor) {
                continue;
            }
            if let Some(t) = ray_dome(from, to - from, center, radius, floor) {
                // On-membrane impacts may throw sparks back away from the dome.
                if t <= Fx::EPSILON && (to - from).dot(a) >= Fx::ZERO {
                    continue;
                }
                if t < first {
                    first = t;
                    best = Some(row);
                }
            }
        }
        best
    }

    /// A volatile unit's blast: every unit in reach, its owner's included, takes
    /// the blast's damage, less toward the edge. Shields between take it instead.
    /// `afloat`: it went up on the water (a ship, or a structure built on the sea),
    /// so the blast starts at the surface, not on the seabed under it.
    fn death_blast(
        &mut self,
        center: FxVec2,
        db: mc_data::DeathBlast,
        owner: u8,
        afloat: bool,
    ) -> Result<(), SimError> {
        let mut victims = Vec::new();
        self.index.query(center, db.radius, kind::UNIT, |e| {
            let row = e.row as usize;
            if self.unit_entry_is_current(e) && !self.state.units.has_flag(row, flag::IN_FACTORY) {
                victims.push(row);
            }
            true
        });
        let ground = self.terrain.height_at(center);
        let base = if afloat {
            ground.max(self.terrain.water_level())
        } else {
            ground
        };
        let origin = center.extend(base + Fx::ONE);
        let mut charged = Vec::new();
        for row in victims {
            let bp = self.bp(row);
            let (radius, height) = (bp.radius, bp.height);
            let reach = (self.state.units.pos[row].distance(center) - radius).max(Fx::ZERO);
            if reach > db.radius {
                continue;
            }
            let damage = db.damage * db.falloff(reach);
            let target = self.state.units.pos[row].extend(self.state.units.z[row] + height / 2);
            if let Some(shield) = self.blast_blocker(origin, target, Some(row)) {
                if !charged.contains(&shield) {
                    charged.push(shield);
                    self.damage_shield(shield, damage);
                }
                continue;
            }
            // Kills among the owner's own go uncredited.
            let by = if self.are_enemies(owner, self.state.units.owner[row]) {
                owner
            } else {
                u8::MAX
            };
            self.damage_unit(row, damage, by, Handle::NONE);
        }
        let mut felled = Vec::new();
        self.prop_index
            .query(center, db.radius * Fx::ratio(3, 4), kind::PROP, |e| {
                felled.push(e.row as usize);
                true
            });
        for prop in felled {
            if self.map.props[prop].kind.is_tree() {
                self.state.props_dead[prop / 64] |= 1 << (prop % 64);
            }
        }
        self.add_stain(center, db.radius * Fx::ratio(3, 4), 110)
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
    pub(crate) fn add_stain(
        &mut self,
        pos: FxVec2,
        radius: Fx,
        strength: u8,
    ) -> Result<(), SimError> {
        // Water leaves no scorch.
        if self.terrain.height_at(pos) < self.terrain.water_level() {
            return Ok(());
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
                stains.push(pos, radius, strength, seed)?;
            }
        }
        Ok(())
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
        let visible = !units.has_flag(row, flag::IN_FACTORY);
        let complete = !units.has_flag(row, flag::UNDER_CONSTRUCTION);
        // A lift ship's wreck holds what its hold did: the cargo dies unseen with it
        // (`lose_orphaned_cargo`), so its salvage goes into the ship's.
        let cargo_mass = if bp.transport.is_some() {
            let id = units.id(row);
            units
                .slots
                .iter()
                .filter(|&r| units.hangar[r] == id)
                .map(|r| {
                    let c = self.bp(r);
                    c.cost_mass * c.wreck_fraction
                })
                .fold(Fx::ZERO, |a, m| a + m)
        } else {
            Fx::ZERO
        };
        let wreck_mass = bp.cost_mass * bp.wreck_fraction + cargo_mass;
        // Taken apart to the last plate: nothing is left to blow up or to lie about.
        // A commander's reactor goes up all the same.
        let reclaimed = units.has_flag(row, flag::RECLAIMED) && !bp.has(cat::COMMANDER);
        let surface = self.terrain.height_at(pos).max(self.terrain.water_level());
        let airborne = visible
            && complete
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
            mass: wreck_mass,
            splashed: 0,
            floor: Fx::ZERO,
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
                wreck_mass,
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
                self.add_stain(pos, bp.radius * Fx::ratio(3, 2), 72)?;
                let mass = wreck_mass;
                if complete && mass > Fx::ZERO {
                    let wreck_z = if bp.motion.map(|m| m.layer) == Some(mc_data::MoveLayer::Air) {
                        let ground = self.terrain.height_at(pos);
                        ground.max(self.terrain.water_level())
                    } else {
                        z
                    };
                    let tick = self.state.tick;
                    let wrecks = &mut self.state.wrecks;
                    let row = wrecks.spawn(bp.id, pos, wreck_z, heading, mass, tick)?;
                    wrecks.turret[row] = turret;
                }
                // The lot was poured; death does not take it up. The scorch
                // stain sits on top of it.
                self.remember_structure_pad(&bp, pos, owner)?;
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
            self.death_blast(pos, db, owner, afloat)?;
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
        // Whatever it was assembling dies with it.
        let units = &self.state.units;
        if let Some(t) = units.row(units.build_target[row]) {
            if units.has_flag(t, flag::IN_FACTORY) {
                self.remove_unit_row(t, true)?;
            }
        }
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
