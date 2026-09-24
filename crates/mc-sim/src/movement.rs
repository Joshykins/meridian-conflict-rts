//! Movement: flow-field following, crowd avoidance, arrival.
//!
//! Each unit's new transform is computed from last tick's state only (its own
//! row, its neighbours through the spatial index, the flow field), so rows can
//! be processed in parallel in any order and the result is the same. Aircraft
//! bank through combat turns and brake before landing when idle.

use crate::nav::Steer;
use crate::spatial::kind;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::{cat, Motion, MoveLayer};

const DT: i32 = TICKS_PER_SECOND as i32;
const CHUNK: usize = 128;
/// Inside this distance a unit heads straight for its own slot instead of the shared field.
const DIRECT_RADIUS: Fx = Fx::from_int(72);
/// Neighbours considered for avoidance; the nearest cells are visited first.
const MAX_NEIGHBOURS: usize = 48;
/// Breathing room kept between hulls.
const PERSONAL_SPACE: Fx = Fx::from_int(2);
/// Largest shove per tick from overlapping neighbours, metres.
const MAX_PUSH: Fx = Fx::from_int(2);
/// A unit that has made no headway for this long gives up on its order.
const GIVE_UP_TICKS: u16 = 300;
/// Landing is slower than climb/cruise terrain following, with a soft final flare.
const LANDING_SPEED: Fx = Fx::from_int(12);
const TOUCHDOWN_SPEED: Fx = Fx::from_int(2);
const AIR_DOCK_RADIUS: Fx = Fx::from_int(24);
/// Fastest an aircraft slides into its slot. A third of top speed, but no
/// more than this: a fast jet docking at a third of its speed overshoots
/// the radius in a few ticks and wobbles round the slot.
const AIR_DOCK_SPEED: Fx = Fx::from_int(46);
/// Fighters may pursue low targets, but must remain clear of terrain and water.
pub(crate) const FIGHTER_ATTACK_CLEARANCE: Fx = Fx::from_int(24);
/// Low point of a Thunderhead strafing pass, above terrain/water.
pub(crate) const ASSAULT_PASS_CLEARANCE: Fx = Fx::from_int(32);
/// Height over the water a torpedo bomber comes down to on its run in.
pub(crate) const TORPEDO_RUN_HEIGHT: Fx = Fx::from_int(30);

#[derive(Clone, Copy)]
pub(crate) struct FormationMotion {
    pub(crate) goal: FxVec2,
    pub(crate) speed: Fx,
    pub(crate) facing: mc_core::Angle,
    /// The goal is a point flown toward, well ahead, not a slot to stop in:
    /// a flight on patrol sweeps through its posts at speed.
    pub(crate) cruise: bool,
    /// The block has arrived and this member is closing on its own slot.
    pub(crate) settling: bool,
}

/// Formation phase of a flight on patrol that has begun its turn onto the next
/// leg, short of the post; its members take that leg at once.
pub(crate) const PHASE_NEXT_LEG: u8 = 4;

struct MoveOut {
    row: usize,
    pos: FxVec2,
    heading: mc_core::Angle,
    speed: Fx,
    stuck: u16,
    extend: bool,
}

impl World {
    pub(crate) fn run_movement(&mut self) -> Result<(), SimError> {
        let rows = self.state.units.slots.rows();
        let formation = self.formation_motion();
        // Resolve the same visible engagement as the order controller before
        // applying any moves, so altitude pursuit cannot depend on row order.
        let attack_altitudes: Vec<_> = (0..rows)
            .map(|row| {
                let units = &self.state.units;
                if !units.slots.is_alive(row)
                    || !units.is_active(row)
                    || !units.has_flag(row, flag::AIR_RUN)
                    || !self.bp(row).has(cat::ANTI_AIR)
                    || !self
                        .bp(row)
                        .motion
                        .is_some_and(|m| m.layer == MoveLayer::Air && !m.hover)
                {
                    return None;
                }
                self.air_engage_target(row)
                    .map(|target| units.prev_z[target])
            })
            .collect();
        let this = &*self;
        let results: Vec<Vec<MoveOut>> = self.pool.parallel_map_chunks(rows, CHUNK, |_, range| {
            let units = &this.state.units;
            let mut out = Vec::new();
            for row in range {
                if !units.slots.is_alive(row) || !units.is_active(row) {
                    continue;
                }
                if let Some(motion) = this.bp(row).motion {
                    let engaged = attack_altitudes[row].is_some();
                    out.push(this.move_unit(row, &motion, formation[row], engaged));
                }
            }
            out
        });

        let mut moves: Vec<_> = results.into_iter().flatten().collect();
        self.resolve_mobile_contacts(&mut moves);
        for m in moves {
            let row = m.row;
            // A stride's worth of ground: the way made, plus the feet shuffling round in a turn.
            let radius = self.blueprints.unit(self.state.units.blueprint[row]).radius;
            let turned = self.state.units.heading[row]
                .delta_to(m.heading)
                .unsigned_abs() as i32;
            let ground = m.pos.distance(self.state.units.pos[row])
                + (radius * turned).mul_div(355, 113 * 0x10000);
            let step = (ground * 256).floor_int().clamp(0, u16::MAX as i32) as u16;
            let mut want_z = self.stand_z(row, m.pos);
            let assault = self.bp(row).visual.mesh == "assault_air";
            if let Some(target_z) = attack_altitudes[row] {
                let ahead = m.pos + FxVec2::from_angle(m.heading) * m.speed;
                let surface = self.ground_surface(m.pos).max(self.ground_surface(ahead));
                want_z = target_z.max(surface + FIGHTER_ATTACK_CLEARANCE);
            }
            if assault && self.state.units.has_flag(row, flag::AIR_RUN) {
                let units = &self.state.units;
                let motion = self.bp(row).motion.expect("assault aircraft");
                if m.speed >= motion.speed / 2 {
                    // Descend on ingress, level over the target, climb on egress.
                    // Keep the recorded attack point outside the gun's arc/range.
                    // Altitude eases over twelve ticks below. Aim that far
                    // along the flight path so the descent lines the fixed gun
                    // up on ingress instead of lagging until almost overhead.
                    let lookahead = m.speed * 12 / DT;
                    let pass_ahead = m.pos + FxVec2::from_angle(m.heading) * lookahead;
                    let distance = pass_ahead.distance(units.air_aim[row]);
                    // The glide leaves cruise height about where the cannon comes into
                    // range, so the nose is already down on the target when it does.
                    let reach = self
                        .bp(row)
                        .weapons
                        .first()
                        .map_or(Fx::from_int(360), |w| w.range_max);
                    let glide = ((motion.altitude - ASSAULT_PASS_CLEARANCE)
                        / (reach - lookahead - Fx::from_int(60)).max(Fx::from_int(100)))
                    .clamp(Fx::ratio(3, 20), Fx::ratio(8, 25));
                    let pass_height = ASSAULT_PASS_CLEARANCE
                        + (distance - Fx::from_int(60)).max(Fx::ZERO) * glide;
                    let ahead = m.pos + FxVec2::from_angle(m.heading) * motion.speed;
                    let surface = self.ground_surface(m.pos).max(self.ground_surface(ahead));
                    want_z =
                        (surface + pass_height).min(want_z.max(surface + ASSAULT_PASS_CLEARANCE));
                }
            }
            // A torpedo bomber comes down to the wave tops on its run in, reaching them
            // by its drop range, holds there until it is past the mark,
            // then climbs back to cruise on the way out.
            let torpedo_run = self.state.units.has_flag(row, flag::AIR_RUN)
                && self
                    .bp(row)
                    .motion
                    .is_some_and(|mo| mo.layer == MoveLayer::Air && !mo.hover)
                && self.bp(row).weapons.first().is_some_and(|w| w.torpedo);
            if torpedo_run {
                let motion = self.bp(row).motion.expect("torpedo bomber");
                if m.speed >= motion.speed / 2 {
                    let reach = self.bp(row).weapons[0].range_max;
                    let distance = m.pos.distance(self.state.units.air_aim[row]);
                    let pass_height =
                        TORPEDO_RUN_HEIGHT + (distance - reach).max(Fx::ZERO) * Fx::ratio(7, 20);
                    let ahead = m.pos + FxVec2::from_angle(m.heading) * motion.speed;
                    let surface = self.ground_surface(m.pos).max(self.ground_surface(ahead));
                    want_z = (surface + pass_height).min(want_z.max(surface + TORPEDO_RUN_HEIGHT));
                }
            }
            let vertical_delta = match self.bp(row).motion {
                // A lift ship eases up and down at its own rate (`transport.rs`).
                Some(mo) if mo.layer == MoveLayer::Air && self.bp(row).transport.is_some() => {
                    self.lift_vertical(row, m.pos, want_z)
                }
                Some(mo) if mo.layer == MoveLayer::Air => {
                    let height_left = self.state.units.z[row] - want_z;
                    // A lift ship comes down from the clouds and climbs back at its own rate.
                    let lift = self.bp(row).transport.map(|t| t.descent);
                    if height_left > Fx::ZERO && want_z == self.ground_surface(m.pos) {
                        // Ease the last 24 metres and cap the final step so landing
                        // never snaps to the ground. Takeoff retains its climb rate.
                        -((height_left / 2)
                            .clamp(TOUCHDOWN_SPEED, lift.unwrap_or(LANDING_SPEED))
                            / DT)
                            .min(height_left)
                    } else {
                        // Ease toward cruise altitude and limit changes in vertical
                        // velocity, so terrain steps do not jerk the airframe.
                        let previous = self.state.units.air_velocity[row].z;
                        let takeoff =
                            self.state.units.z[row] < self.ground_surface(m.pos) + mo.altitude / 2;
                        let limit = if let Some(rate) = lift {
                            rate / DT
                        } else if takeoff {
                            mo.speed / 2 / DT
                        } else if attack_altitudes[row].is_some()
                            || (assault && self.state.units.has_flag(row, flag::AIR_RUN))
                            || torpedo_run
                        {
                            mo.speed * Fx::ratio(7, 20) / DT
                        } else {
                            (mo.speed / 4).clamp(Fx::from_int(6), Fx::from_int(18)) / DT
                        };
                        let remaining = want_z - self.state.units.z[row];
                        // Start braking before a fast pursuit descent reaches its
                        // altitude; damping alone can overshoot low targets.
                        let limit = if attack_altitudes[row].is_some() {
                            limit.min((remaining.abs() * Fx::ratio(3, 10)).sqrt() * Fx::ratio(4, 5))
                        } else {
                            limit
                        };
                        let desired = (remaining / 12).clamp(-limit, limit);
                        // A strafing run at speed has to tip into its dive quickly, or the
                        // fixed nose gun comes down on the target too shallow to bear.
                        let ease = if torpedo_run
                            || (assault && self.state.units.has_flag(row, flag::AIR_RUN))
                        {
                            Fx::ratio(3, 10)
                        } else {
                            Fx::ratio(3, 20)
                        };
                        let velocity = previous.approach(desired, ease);
                        if remaining.abs() < Fx::ratio(1, 100) && velocity.abs() < Fx::ratio(1, 100)
                        {
                            remaining
                        } else {
                            velocity
                        }
                    }
                }
                _ => want_z - self.state.units.z[row],
            };
            let want_bank = match self.bp(row).motion {
                Some(mo) if mo.layer == MoveLayer::Air => {
                    let turn = self.state.units.heading[row].delta_to(m.heading) as i32;
                    if self.bp(row).transport.is_some() {
                        let share = (m.speed / mo.speed).clamp(Fx::ZERO,Fx::ONE);
                        -(Fx::from_int(turn * 1200 / self.air_turn_rate(row,&mo).max(1)) * share).floor_int()
                    } else if mo.hover {
                        0
                    } else {
                        let speed_share = if m.speed > Fx::ONE { Fx::ONE } else { Fx::ZERO };
                        // Left turn lowers the left wing: negative local-X roll.
                        -(Fx::from_int(turn * 8192 / self.air_turn_rate(row, &mo).max(1))
                            * speed_share)
                            .floor_int()
                    }
                }
                _ => 0,
            };
            let assault_pitch = if assault {
                // Hull, muzzle and exhaust follow the actual flight slope.
                Some(crate::world::pitch_to(
                    m.pos
                        .distance(self.state.units.pos[row])
                        .max(Fx::from_int(2)),
                    vertical_delta,
                ))
            } else {
                None
            };
            let capital_pitch = if self.bp(row).transport.is_some() {
                let speed = m.speed;
                let run = m.pos.distance(self.state.units.pos[row]);
                let slope = crate::world::pitch_to_limit(run.max(Fx::ONE),vertical_delta,2184);
                let slope = mc_core::Angle::ZERO.delta_to(slope) as i32;
                // Lift straight up level; align to the flight path once underway.
                let share = (speed / Fx::from_int(20)).clamp(Fx::ZERO,Fx::ONE);
                let accel = ((speed-self.state.units.speed[row])*Fx::from_int(360)).round_int();
                let clear = self.state.units.z[row] > self.ground_surface(m.pos)+Fx::from_int(12);
                let desired = if clear { ((Fx::from_int(slope)*share).round_int()-accel).clamp(-2184,2184) } else { 0 };
                let current = mc_core::Angle::ZERO.delta_to(self.state.units.arm_pitch[row][0]) as i32;
                let delta = desired-current;
                let step = if delta.abs()<8 { delta } else { (delta/8).clamp(-80,80) };
                Some(mc_core::Angle((current+step) as u16))
            } else { None };
            let units = &mut self.state.units;
            if let Some(pitch) = capital_pitch { units.arm_pitch[row][0] = pitch; }
            if let Some(pitch) = assault_pitch {
                units.arm_pitch[row][0] = pitch;
            }
            let bank = units.bank[row] as i32;
            let delta = want_bank - bank;
            units.bank[row] = (bank + if delta.abs() <= 4 { delta } else { delta / 4 }) as i16;
            units.gait[row] = units.gait[row].wrapping_add(step as u32);
            units.gait_step[row] = [step, units.gait_step[row][0]];
            units.air_velocity[row] = (m.pos - units.pos[row]).extend(vertical_delta);
            if m.pos != units.pos[row] {
                units.flags[row] |= flag::MOVING;
                units.pos[row] = m.pos;
            }
            units.heading[row] = m.heading;
            units.speed[row] = m.speed;
            units.stuck_ticks[row] = m.stuck;
            units.z[row] += vertical_delta;
            let field = units.field[row];
            if m.extend && field != NO_FIELD {
                self.nav.extend(field, m.pos)?;
            }
        }
        Ok(())
    }

    /// Resolve overlaps against proposed positions, including different ground
    /// locomotion types. Steering alone is not a collision constraint.
    fn resolve_mobile_contacts(&self, moves: &mut [MoveOut]) {
        let mut index = vec![usize::MAX; self.state.units.slots.rows()];
        for (i, m) in moves.iter().enumerate() {
            index[m.row] = i;
        }
        let mut pairs = Vec::new();
        for (i, m) in moves.iter().enumerate() {
            let row = m.row;
            // Flight spacing belongs to formations, not ground hull contacts.
            if self.bp(row).motion.unwrap().layer == MoveLayer::Air {
                continue;
            }
            self.index.query(
                self.state.units.pos[row],
                self.bp(row).radius + Fx::from_int(32),
                kind::UNIT,
                |e| {
                    let other = e.row as usize;
                    if other <= row || !self.unit_entry_is_current(e) || index[other] == usize::MAX
                    {
                        return true;
                    }
                    // A dived submarine slips under a floating hull, and it over it.
                    if self.bp(other).motion.unwrap().layer == MoveLayer::Air
                        || self.hulls_pass(row, other)
                    {
                        return true;
                    }
                    pairs.push((i, index[other]));
                    true
                },
            );
        }
        // A crowd pressed against a slope needs more passes to spread; stop
        // as soon as a pass finds nothing to push apart.
        for _ in 0..8 {
            let mut pushed_any = false;
            for &(i, j) in &pairs {
                let (a, b) = (moves[i].row, moves[j].row);
                let delta = moves[i].pos - moves[j].pos;
                let dist = delta.length();
                let ra = self.bp(a).radius;
                let rb = self.bp(b).radius;
                let overlap = ra + rb + Fx::ONE - dist;
                if overlap <= Fx::ZERO {
                    continue;
                }
                let dir = if dist > Fx::EPSILON {
                    delta.normalize()
                } else {
                    FxVec2::from_angle(mc_core::Angle((a as u16).wrapping_mul(9973)))
                };
                pushed_any = true;
                let correction = overlap.min(Fx::from_int(6));
                // Where `k` stands pushed `amount` along `direction`, if it can stand there.
                let pushed = |k: usize, direction: FxVec2, amount: Fx| {
                    let motion = self.bp(moves[k].row).motion.unwrap();
                    let candidate = self.clamp_to_map(moves[k].pos + direction * amount);
                    self.nav
                        .passable(motion.layer, motion.size_class, candidate)
                        .then_some(candidate)
                };
                let a = pushed(i, dir, correction * (rb / (ra + rb)));
                let b = pushed(j, -dir, correction * (ra / (ra + rb)));
                // A hull pressed against a slope cannot give way; the other
                // takes the whole push, or two hulls stay sunk into each other.
                let (a, b) = match (a, b) {
                    (None, Some(_)) => (None, pushed(j, -dir, correction).or(b)),
                    (Some(_), None) => (pushed(i, dir, correction).or(a), None),
                    both => both,
                };
                if let Some(p) = a {
                    moves[i].pos = p;
                }
                if let Some(p) = b {
                    moves[j].pos = p;
                }
            }
            if !pushed_any {
                break;
            }
        }
    }

    /// Advance a real group anchor, then steer every member to its moving slot.
    /// Members converge during travel; a disordered group never waits to assemble.
    fn formation_motion(&mut self) -> Vec<Option<FormationMotion>> {
        let units = &self.state.units;
        let mut out = vec![None; units.slots.rows()];
        let mut active = std::collections::BTreeMap::<u64, Vec<usize>>::new();
        let mut circling = std::collections::BTreeMap::<u64, Vec<usize>>::new();
        let mut live = std::collections::BTreeSet::new();
        for row in units.slots.iter() {
            for o in self.state.orders.iter(units, row) {
                if o.formation != 0 {
                    live.insert(o.formation);
                }
            }
            if !units.is_active(row) || units.flags[row] & (flag::AIR_RUN | flag::HOLD) != 0 {
                continue;
            }
            if let Some(o) = self.state.orders.front(units, row) {
                if o.formation != 0 && o.kind == OrderKind::Orbit {
                    circling.entry(o.formation).or_default().push(row);
                } else if o.formation != 0 {
                    active.entry(o.formation).or_default().push(row);
                }
            }
        }
        self.state.formations.retain(|id, _| live.contains(id));
        self.orbit_formation_motion(circling, &mut out);
        for (id, rows) in active {
            let Some(mut group) = self.state.formations.get(&id).cloned() else {
                continue;
            };
            let first = *self.state.orders.front(&self.state.units, rows[0]).unwrap();
            let target = first.pos;
            let air = self.bp(rows[0]).motion.unwrap().layer == MoveLayer::Air;
            // A flight on patrol sweeps round its loop like one aircraft: it
            // never stops on a post, and turns no faster than its wings follow.
            let sweep =
                air && first.kind == OrderKind::Patrol && !self.bp(rows[0]).motion.unwrap().hover;
            let mut mean = FxVec2::ZERO;
            let mut pace = Fx::MAX;
            let mut accel = Fx::MAX;
            let mut radius = Fx::ZERO;
            let mut yaw = i32::MAX;
            for &row in &rows {
                let o = self.state.orders.front(&self.state.units, row).unwrap();
                mean += self.state.units.pos[row] - o.offset;
                let m = self.bp(row).motion.unwrap();
                pace = pace.min(m.speed);
                accel = accel.min(m.accel);
                yaw = yaw.min(m.turn_rate as i32);
                radius = radius.max(self.bp(row).radius);
            }
            mean = FxVec2::new(mean.x / rows.len() as i32, mean.y / rows.len() as i32);
            // The anchor turns at half the slowest wing's rate, so the outside
            // of the flight has room to keep station through the turn.
            let yaw = (yaw / 2).max(1);
            if group.phase == 0 && sweep {
                // Carry on the way the flight is already going, at its airspeed:
                // the slots do not swing round to the new leg, and nobody brakes.
                let units = &self.state.units;
                let (mut dir, mut speed) = (FxVec2::ZERO, Fx::ZERO);
                for &row in &rows {
                    dir += FxVec2::from_angle(units.heading[row]);
                    speed += units.speed[row];
                }
                group.heading = if dir == FxVec2::ZERO {
                    first.heading
                } else {
                    dir.angle()
                };
                let mut anchor = FxVec2::ZERO;
                for &row in &rows {
                    let o = self.state.orders.front(units, row).unwrap();
                    anchor += units.pos[row] - o.offset.rotate(group.heading - o.heading);
                }
                let n = rows.len() as i32;
                group.anchor = FxVec2::new(anchor.x / n, anchor.y / n);
                group.speed = (speed / n).min(pace);
                group.phase = 1;
            }
            if group.phase == 0 {
                group.anchor = mean;
                group.heading = first.heading;
                group.phase = 1;
            }
            let delta = target - group.anchor;
            let m = self.bp(rows[0]).motion.unwrap();
            let probe = group.anchor + delta.clamp_length(Fx::from_int(128));
            // Straight on only while the block fits that way, not just its
            // middle: a gap the anchor alone threads is for the route to find.
            let clear_ahead = air
                || self
                    .nav
                    .clear_segment(m.layer, m.size_class, group.anchor, probe)
                    && {
                        let shut = rows
                            .iter()
                            .filter(|&&row| {
                                let o = self.state.orders.front(&self.state.units, row).unwrap();
                                let slot =
                                    group.anchor + o.offset.rotate(group.heading - o.heading);
                                let mo = self.bp(row).motion.unwrap();
                                !self.nav.clear_segment(
                                    mo.layer,
                                    mo.size_class,
                                    slot,
                                    slot + (probe - group.anchor),
                                )
                            })
                            .count();
                        shut * 3 <= rows.len()
                    };
            let mut on_field = false;
            let route = if sweep {
                if delta != FxVec2::ZERO {
                    group.heading = group.heading.turn_toward(delta.angle(), yaw as u16);
                }
                FxVec2::from_angle(group.heading)
            } else {
                // Straight at the goal while the way is open and the field agrees
                // with it; the probe sees only so far, and open ground that ends
                // in a ridge's pocket is for the field to lead the block round.
                let straight = delta.normalize();
                match self
                    .nav
                    .sample(self.state.units.field[rows[0]], group.anchor)
                {
                    Steer::Direction(d) if !(clear_ahead && d.dot(straight) > Fx::HALF) => {
                        on_field = true;
                        d
                    }
                    _ => straight,
                }
            };
            // Slots turn with the route, returning to the requested facing on arrival.
            let facing = if delta.length() < pace {
                first.heading
            } else if on_field && !air {
                // A block faces where its way goes over the next stretch, not
                // each kink the field makes under its anchor.
                self.way_ahead(
                    self.state.units.field[rows[0]],
                    group.anchor,
                    route,
                    delta.length(),
                )
                .angle()
            } else {
                route.angle()
            };
            if !sweep && (group.phase == 1 || group.phase == 2) {
                // A block wheels no faster than its outside ranks can keep up
                // with speed in hand, or its inside ranks crowd together.
                let wheel = if air {
                    m.turn_rate / 2
                } else {
                    let mut reach = Fx::ONE;
                    for &row in &rows {
                        reach = reach.max(
                            self.state
                                .orders
                                .front(&self.state.units, row)
                                .unwrap()
                                .offset
                                .length(),
                        );
                    }
                    let spare = pace * Fx::ratio(3, 10);
                    let cap = (spare * 10430 / (reach * DT))
                        .floor_int()
                        .clamp(1, (m.turn_rate / 2) as i32) as u16;
                    // Beside a slope it turns with its way, to keep its ranks
                    // off it. Only worth asking when the cap holds it back.
                    let look = route
                        * (pace * 3)
                            .clamp(Fx::from_int(24), Fx::from_int(64))
                            .min(delta.length());
                    let hemmed = || {
                        rows.iter().any(|&row| {
                            let o = self.state.orders.front(&self.state.units, row).unwrap();
                            let slot = group.anchor + o.offset.rotate(group.heading - o.heading);
                            let mo = self.bp(row).motion.unwrap();
                            !self
                                .nav
                                .clear_segment(mo.layer, mo.size_class, slot, slot + look)
                        })
                    };
                    if group.heading.delta_to(facing).unsigned_abs() > cap && hemmed() {
                        m.turn_rate / 2
                    } else {
                        cap
                    }
                };
                group.heading = group.heading.turn_toward(facing, wheel);
            }
            // Out of order after a pass, or wheeling through a turn: hand slots
            // to whoever is nearest them instead of swinging the whole block.
            if !air
                && (group.phase == 1
                    || (group.phase == 2 && (self.state.tick as u64 + id) % 4 == 0))
            {
                self.regroup_ranks(&rows, group.anchor, group.heading);
            }
            let offset = |o: &Order| o.offset.rotate(group.heading - o.heading);
            // Whether a member's rank, moved from `from` to `to` with the anchor, crosses ground it cannot.
            let cut_off = |row: usize, from: FxVec2, to: FxVec2| {
                let m = self.bp(row).motion.unwrap();
                !self.nav.clear_segment(m.layer, m.size_class, from, to)
            };
            let n = rows.len();
            // A spur or a boulder may cut off a few hulls: they go round it on
            // their own and the block marches on. It files through only where
            // most of its ranks cannot pass.
            let crowded = |cut: usize| cut * 3 > n;
            let mut worst = Fx::ZERO;
            let mut cut = 0;
            let mut astray = 0;
            for &row in &rows {
                let o = self.state.orders.front(&self.state.units, row).unwrap();
                let slot = group.anchor + offset(o);
                let pos = self.state.units.pos[row];
                astray += (pos.distance(slot) > pace * 2) as usize;
                if cut_off(row, pos, slot) {
                    cut += 1;
                } else {
                    // The march paces itself on the members that can reach their ranks.
                    worst = worst.max(pos.distance(slot));
                }
            }
            // Clear of a pass once the ranks fit again and the way on is open,
            // straight at the goal or along the route the next stretch.
            let reopened = || {
                let look = route
                    * (pace * 3)
                        .clamp(Fx::from_int(24), Fx::from_int(64))
                        .min(delta.length());
                let ahead = rows
                    .iter()
                    .filter(|&&row| {
                        let o = self.state.orders.front(&self.state.units, row).unwrap();
                        let slot = group.anchor + offset(o);
                        cut_off(row, slot, slot + look)
                    })
                    .count();
                cut * 4 <= n && (clear_ahead || ahead * 4 <= n)
            };
            if crowded(cut) || (group.phase == 3 && !reopened()) {
                // A narrow passage may not fit the whole block. Let the existing
                // per-hull navigation cross it, then recover ranks while moving on open ground.
                group.anchor = mean;
                group.phase = 3;
                group.speed = Fx::ZERO;
                self.state.formations.insert(id, group);
                continue;
            }
            if group.phase == 3 {
                group.phase = 1;
                group.anchor = mean;
            }
            if air && astray * 2 > rows.len() {
                // A flight coming off its attack runs is strewn over the sky
                // round an anchor nobody was moving. Form up where the aircraft
                // are and on the way, not back at slots left behind.
                group.anchor = mean;
            }
            if group.phase == 1 && worst <= radius.max(Fx::from_int(5)) {
                group.phase = 2;
            }
            // Leave immediately, keeping speed in reserve for members catching up.
            // Formation error can slow the march, but must never create a rally pause.
            // A flight on patrol holds its pace: its wings have speed in hand
            // to close up, and an anchor that slows leaves them ahead of it.
            let share = if sweep || worst < radius * 2 + Fx::from_int(14) {
                Fx::ratio(7, 10)
            } else {
                Fx::ratio(2, 5)
            };
            let speed = if sweep {
                pace * share
            } else {
                (pace * share).min(delta.length() * 2)
            };
            group.speed = group.speed.approach(speed, accel / DT);
            let advance = if sweep {
                route * (group.speed / DT)
            } else {
                route * (group.speed / DT).min(delta.length())
            };
            let mut candidate = group.anchor + advance;
            let side = route.perp();
            let blocked_by = |to: FxVec2| {
                rows.iter()
                    .filter(|&&row| {
                        let o = self.state.orders.front(&self.state.units, row).unwrap();
                        cut_off(row, group.anchor + offset(o), to + offset(o))
                    })
                    .count()
            };
            let mut cut = blocked_by(candidate);
            if !air {
                // Ranks the next stretch runs into ground: side the block away from
                // them, so it skirts a mountain's flank a rank's width off instead
                // of scraping along it. A pass pinching from both sides nets out.
                // Ground past the goal is none of the block's business.
                let look = route
                    * (pace * 3)
                        .clamp(Fx::from_int(24), Fx::from_int(64))
                        .min(delta.length());
                let mut lean = 0i32;
                let mut facing_ground = false;
                let mut width = Fx::ZERO;
                for &row in &rows {
                    let o = self.state.orders.front(&self.state.units, row).unwrap();
                    let slot = group.anchor + offset(o);
                    let across = offset(o).dot(side);
                    width = width.max(across.abs());
                    if cut_off(row, slot, slot + look) {
                        facing_ground = true;
                        if across.abs() > radius {
                            lean -= across.signum() as i32;
                        }
                    }
                }
                let mut dodge = false;
                let far = route
                    * (pace * 8)
                        .clamp(Fx::from_int(48), Fx::from_int(128))
                        .min(delta.length());
                if lean == 0 && !facing_ground {
                    // Look further down the road for ground square across the front.
                    facing_ground = rows.iter().all(|&row| {
                        let o = self.state.orders.front(&self.state.units, row).unwrap();
                        let slot = group.anchor + offset(o);
                        cut_off(row, slot, slot + far)
                    });
                }
                if lean == 0 && facing_ground {
                    // Ground square across the whole front, a mountain dead ahead:
                    // the block goes round it on one side rather than splitting
                    // either side of it. Take the side with more open ground,
                    // then the one the route already bends to.
                    let shut = |shift: FxVec2| {
                        rows.iter()
                            .filter(|&&row| {
                                let o = self.state.orders.front(&self.state.units, row).unwrap();
                                let slot = group.anchor + shift + offset(o);
                                !self.nav.passable(m.layer, m.size_class, slot + far)
                            })
                            .count()
                    };
                    let step = side * (width + radius * 2);
                    let left = shut(step) + shut(step * Fx::from_int(2));
                    let right = shut(-step) + shut(-step * Fx::from_int(2));
                    let bend = delta.normalize().cross(route);
                    lean = match left.cmp(&right) {
                        std::cmp::Ordering::Less => 1,
                        std::cmp::Ordering::Greater => -1,
                        _ if bend < Fx::ZERO => -1,
                        _ => 1,
                    };
                    dodge = true;
                }
                if lean != 0 {
                    // Edge off only as fast as the ranks can follow.
                    let rate = if dodge {
                        pace / DT
                    } else if worst < radius * 2 + Fx::from_int(14) {
                        pace / DT / 2
                    } else {
                        pace / DT / 4
                    };
                    let slid = candidate + if lean > 0 { side } else { -side } * rate;
                    let slid_cut = blocked_by(slid);
                    if slid_cut <= cut && self.nav.passable(m.layer, m.size_class, slid) {
                        candidate = slid;
                        cut = slid_cut;
                    }
                }
            }
            // The anchor keeps to ground the block can drive, so a ridge too
            // thin to cut off many ranks at once is not walked straight over.
            let anchor_clear = !self.nav.passable(m.layer, m.size_class, group.anchor)
                || self
                    .nav
                    .clear_segment(m.layer, m.size_class, group.anchor, candidate);
            if anchor_clear && !crowded(cut) {
                group.anchor = candidate;
            } else {
                group.phase = 3;
                group.speed = Fx::ZERO;
                group.anchor = mean;
                self.state.formations.insert(id, group);
                continue;
            }
            if sweep {
                if group.phase != 3 && self.turn_onto_next_leg(rows[0], &group, yaw) {
                    group.phase = PHASE_NEXT_LEG;
                }
            } else if group.anchor.distance(target) <= Fx::HALF {
                group.anchor = target;
                group.heading = first.heading;
                group.speed = Fx::ZERO;
            }
            for &row in &rows {
                let o = self.state.orders.front(&self.state.units, row).unwrap();
                let slot = group.anchor + o.offset.rotate(group.heading - o.heading);
                let own_speed = self.bp(row).motion.unwrap().speed;
                let settling = !sweep && group.anchor.distance(target) <= pace;
                let (goal, speed) = if !settling {
                    let error = slot - self.state.units.pos[row];
                    let lag = error.dot(route);
                    let look = (pace / 2).max(Fx::from_int(12));
                    // Look ahead of each hull: even members ahead of their rank
                    // move forward as they ease into it, rather than reversing
                    // toward a slot behind them.
                    let mo = self.bp(row).motion.unwrap();
                    let forward = if mo.layer == MoveLayer::Air && !mo.hover {
                        // A wing takes seconds to roll into a turn. Chasing a
                        // point a few lengths ahead, it is still rolling when
                        // the point has crossed its nose, and it weaves all the
                        // way. Two turn radii out it eases onto its station.
                        let turn_radius = mo
                            .speed
                            .mul_div(10430, (mo.turn_rate as i64 * DT as i64).max(1));
                        (turn_radius * 2).max(look * 2)
                    } else {
                        (look + lag).clamp(look / 4, look * 2)
                    };
                    let lateral = (error - route * lag).clamp_length(forward * Fx::ratio(3, 4));
                    let pos = self.state.units.pos[row];
                    let spacing = radius * 2 + Fx::from_int(6);
                    // Far from its rank, a hull makes for it by the way from where it
                    // is: the anchor's heading may lead it into a slope it is beside.
                    let astray = !air && error.length() > spacing * 3;
                    let mut goal = if astray {
                        slot
                    } else {
                        pos + route * forward + lateral
                    };
                    // Only close by: a member walled off from a rank far away takes
                    // the field round, not a sidestep along the wrong side of a ridge.
                    if !air && !astray && cut_off(row, pos, goal) {
                        // A knoll stands in this rank's way. Tuck in toward the
                        // middle of the block, which has found its way past,
                        // rather than going round on a route of its own.
                        let inward = side * (group.anchor - pos).dot(side);
                        // Failing that, edge across behind it before going round alone.
                        if let Some(tucked) = [2, 1, 0]
                            .into_iter()
                            .flat_map(|k| {
                                (1..=4).map(move |j| {
                                    pos + route * forward * Fx::ratio(k, 2)
                                        + inward * Fx::ratio(j, 4)
                                })
                            })
                            .find(|&g| g != pos && !cut_off(row, pos, g))
                        {
                            goal = tucked;
                        }
                    }
                    // A wing on patrol eases back onto station instead of
                    // throttling down to a crawl; a stall reads as a fault.
                    let floor = if sweep { pace / 2 } else { pace / 5 };
                    let speed = (group.speed + lag * 2).clamp(floor, own_speed);
                    (goal, speed)
                } else {
                    (slot, own_speed)
                };
                out[row] = Some(FormationMotion {
                    goal,
                    speed,
                    facing: group.heading,
                    cruise: sweep,
                    settling,
                });
            }
            self.state.formations.insert(id, group);
        }
        out
    }

    /// Whether a flight on patrol, its anchor where `group` has it, should
    /// begin its turn onto the leg after its post now. It turns in early by
    /// the lead its turn radius needs for the corner, so it rolls out on the
    /// new leg instead of overshooting the post and weaving back; a post it
    /// has already passed abeam is taken as reached.
    fn turn_onto_next_leg(&self, row: usize, group: &crate::formations::Group, yaw: i32) -> bool {
        let units = &self.state.units;
        let mut posts = self
            .state
            .orders
            .iter(units, row)
            .filter(|o| o.kind == OrderKind::Patrol)
            .map(|o| o.pos);
        let (Some(post), Some(next)) = (posts.next(), posts.next()) else {
            return false;
        };
        let to_post = post - group.anchor;
        let dist = to_post.length();
        let turn_radius = group.speed.mul_div(10430, (yaw as i64 * DT as i64).max(1));
        if to_post.dot(FxVec2::from_angle(group.heading)) <= Fx::ZERO && dist <= turn_radius * 2 {
            return true;
        }
        dist <= patrol_lead(group.heading, post, next, turn_radius).max(Fx::from_int(4))
    }

    /// Lower airspeed buys a tighter fighter turn, up to 1.8 times cruise yaw.
    /// Both roll feedback and steering use the same rate so bank stays bounded.
    fn air_turn_rate(&self, row: usize, motion: &Motion) -> i32 {
        if self.state.units.has_flag(row, flag::AIR_RUN) && self.bp(row).has(cat::ANTI_AIR) {
            let speed = self.state.units.speed[row].max(motion.speed * Fx::ratio(5, 9));
            (Fx::from_int(motion.turn_rate as i32) * (motion.speed / speed).max(Fx::ONE))
                .floor_int()
        } else {
            motion.turn_rate as i32
        }
    }

    fn ground_surface(&self, pos: FxVec2) -> Fx {
        self.terrain.height_at(pos).max(self.terrain.water_level())
    }

    fn move_unit(
        &self,
        row: usize,
        motion: &Motion,
        formation: Option<FormationMotion>,
        engaged: bool,
    ) -> MoveOut {
        let units = &self.state.units;
        let pos = units.pos[row];
        let radius = self.bp(row).radius;
        let mut out = MoveOut {
            row,
            pos,
            heading: units.heading[row],
            speed: units.speed[row],
            stuck: units.stuck_ticks[row],
            extend: false,
        };

        // Ground crowd separation must never displace or steer an aircraft.
        let mut push = FxVec2::ZERO;
        let mut seen = 0;
        if motion.layer != MoveLayer::Air {
            self.index
                .query(pos, radius + PERSONAL_SPACE, kind::UNIT, |e| {
                    let other = e.row as usize;
                    if other == row || !self.unit_entry_is_current(e) || !self.bp(other).is_mobile()
                    {
                        return true;
                    }
                    let other_air = self
                        .bp(other)
                        .motion
                        .is_some_and(|m| m.layer == MoveLayer::Air);
                    if other_air != (motion.layer == MoveLayer::Air) {
                        return true;
                    }
                    if units.has_flag(other, flag::IN_FACTORY) || self.hulls_pass(row, other) {
                        return true;
                    }
                    let d = pos - e.pos;
                    let dist = d.length();
                    let overlap = radius + e.radius + PERSONAL_SPACE - dist;
                    if overlap > Fx::ZERO {
                        // Coincident units separate along a direction fixed by their rows.
                        let away = if dist > Fx::ZERO {
                            d * (Fx::ONE / dist)
                        } else {
                            {
                                let axis = FxVec2::from_angle(mc_core::Angle(
                                    (row.min(other) as u16).wrapping_mul(9973),
                                ));
                                if row < other {
                                    axis
                                } else {
                                    -axis
                                }
                            }
                        };
                        push += away * overlap;
                        seen += 1;
                    }
                    seen < MAX_NEIGHBOURS
                });
        }

        let standing_on_blocked = !self.nav.passable(motion.layer, motion.size_class, pos);
        let moving = units.flags[row] & flag::HAS_FIELD != 0 && units.flags[row] & flag::HOLD == 0;
        let goal = formation.map(|f| f.goal).unwrap_or(units.move_goal[row]);
        let to_goal = goal - pos;
        let dist = to_goal.length();
        let transit_air = motion.layer == MoveLayer::Air
            && (motion.hover || units.flags[row] & flag::AIR_RUN == 0);
        let docking = transit_air
            && moving
            && dist <= AIR_DOCK_RADIUS
            && units.speed[row] <= (motion.speed / 3).min(AIR_DOCK_SPEED);
        let steering_goal = goal;

        let mut dir = FxVec2::ZERO;
        let mut waiting = false;
        if standing_on_blocked {
            // A structure was placed on top of us: walk out the short way.
            if let Some(s) = self.index.nearest(pos, radius, kind::UNIT, |e| {
                self.unit_entry_is_current(e) && self.bp(e.row as usize).is_structure()
            }) {
                dir = (pos - s.pos).normalize();
            }
            if dir == FxVec2::ZERO {
                dir = FxVec2::from_angle(units.heading[row]);
            }
        } else if moving {
            if motion.layer == MoveLayer::Air
                || ((formation.is_some() || dist <= DIRECT_RADIUS)
                    && self
                        .nav
                        .clear_segment(motion.layer, motion.size_class, pos, goal))
            {
                dir = (steering_goal - pos).normalize();
            } else {
                match self.nav.sample(units.field[row], pos) {
                    Steer::Direction(d) => dir = d,
                    Steer::Arrived => dir = to_goal.normalize(),
                    Steer::Pending => waiting = true,
                    Steer::NeedsExtend => {
                        waiting = true;
                        out.extend = true;
                    }
                    Steer::Unreachable => {
                        out.stuck = u16::MAX;
                        waiting = true;
                    }
                }
            }
        }

        let max_speed = formation
            .map(|f| f.speed.min(motion.speed))
            .unwrap_or(motion.speed);
        let accel = motion.accel / DT;
        let mut target_speed = Fx::ZERO;
        if dir != FxVec2::ZERO && !waiting && !(transit_air && dist <= Fx::HALF) {
            let crowd = (push.length() / radius).min(Fx::ratio(3, 2));
            let steer = dir + push.normalize() * crowd;
            let want = if steer == FxVec2::ZERO {
                dir.angle()
            } else {
                steer.angle()
            };
            // A capital ship never turns its hull on what it shoots: its turrets cover every side.
            let hover_combat = motion.hover
                && units.has_flag(row, flag::AIR_RUN)
                && !self.bp(row).is_capital_ship();
            let facing = if hover_combat {
                (units.air_aim[row] - pos).angle()
            } else if formation.is_some_and(|f| {
                // Settle into the rank facing the group's way, unless the rank
                // lies off to the side: a hull that cannot drive sideways
                // would face away from it and never close the last metre.
                dist < Fx::from_int(4) && f.facing.delta_to(want).unsigned_abs() <= 0x2000
            }) {
                formation.unwrap().facing
            } else if docking && dist < Fx::from_int(12) {
                self.state
                    .orders
                    .front(units, row)
                    .map(|o| o.heading)
                    .unwrap_or(want)
            } else {
                want
            };
            if motion.layer == MoveLayer::Air && !motion.hover && !docking {
                // Roll into the turn over time instead of instantly applying
                // maximum yaw. Bank is deterministic, serialized flight state.
                let limit = self.air_turn_rate(row, motion);
                let want_turn = (out.heading.delta_to(facing) as i32 / 8).clamp(-limit, limit);
                let old_turn = -(units.bank[row] as i32) * limit / 8192;
                let slew = (limit / 5).max(1);
                let turn = old_turn + (want_turn - old_turn).clamp(-slew, slew);
                out.heading = mc_core::Angle(out.heading.0.wrapping_add(turn as i16 as u16));
            } else {
                out.heading = out.heading.turn_toward(facing, motion.turn_rate);
            }
            let off = out.heading.delta_to(want).unsigned_abs();
            // Aircraft keep flying through the turn. On a transit they brake
            // into a hover; a combat run (`AIR_RUN`) does not slow for the egress.
            // Ground hulls pivot before they drive, then brake into the destination.
            if motion.layer == MoveLayer::Air {
                target_speed = max_speed;
                if engaged {
                    // Trade speed for turn radius when the opponent is off the
                    // nose, then accelerate back onto the firing line.
                    let turning = Fx::ratio(off.min(0x4000) as i64, 0x4000);
                    target_speed = max_speed * (Fx::ONE - turning * Fx::ratio(9, 20));
                    // Close pursuit also sheds speed to avoid endlessly passing
                    // through a slower opponent at full throttle.
                    if off < 0x2000 {
                        let closing = (dist / max_speed).clamp(Fx::ratio(3, 5), Fx::ONE);
                        target_speed = target_speed.min(max_speed * closing);
                    }
                }
                let cruising = formation.is_some_and(|f| f.cruise);
                if moving
                    && !standing_on_blocked
                    && !cruising
                    && (motion.hover || units.flags[row] & flag::AIR_RUN == 0)
                {
                    // Begin braking by stopping distance; the final hover
                    // controller can translate sideways without orbiting a slot.
                    target_speed = target_speed
                        .min((motion.accel * dist * 2).sqrt())
                        .min(dist * 2);
                    if docking {
                        // Stay in the low-speed docking envelope until captured.
                        target_speed = target_speed.min((motion.speed / 3).min(AIR_DOCK_SPEED));
                    } else if !motion.hover && off > 0x3000 {
                        // A fixed speed floor can produce a stable orbit outside
                        // docking range. Keep the turn radius below the remaining
                        // distance, even for slow-turning bombers (v = radius * yaw).
                        let yaw = Fx::ratio(motion.turn_rate as i64 * DT as i64 * 355, 113 * 32768);
                        target_speed = target_speed.min(max_speed / 3).min(dist * yaw / 2);
                    }
                }
            } else {
                target_speed = if off > 0x2000 {
                    if formation.is_some() {
                        Fx::ZERO
                    } else {
                        max_speed / 5
                    }
                } else {
                    max_speed
                };
                if moving && !standing_on_blocked {
                    target_speed = target_speed
                        .min(dist * 2)
                        .min((motion.accel * dist * 2).sqrt());
                }
            }
        }
        // Lift clear before accelerating horizontally out of a landing site.
        if motion.layer == MoveLayer::Air
            && moving
            && units.z[row] < self.ground_surface(pos) + Fx::from_int(16).min(motion.altitude)
            && !self.just_launched(row)
        {
            target_speed = Fx::ZERO;
        }
        // A lift ship rises clear before it moves off, and brakes into its glide.
        if moving && self.bp(row).transport.is_some() {
            if let Some(cap) = self.lift_speed_cap(row, pos, max_speed) {
                target_speed = target_speed.min(cap);
            }
        }
        out.speed = out.speed.approach(target_speed, accel);

        let mut step = if motion.hover {
            // VTOL translation is independent of its target-facing fuselage.
            let previous = units.air_velocity[row].xy();
            let desired = dir * (out.speed / DT).min(dist);
            previous + (desired - previous).clamp_length(accel / DT)
        } else if docking {
            to_goal.normalize() * (out.speed / DT).min(dist)
        } else {
            FxVec2::from_angle(out.heading) * (out.speed / DT)
        };
        if transit_air && dist <= Fx::HALF && moving {
            out.speed = Fx::ZERO;
            step = FxVec2::ZERO;
        }
        if push != FxVec2::ZERO {
            step += push.clamp_length(MAX_PUSH) * Fx::HALF;
        }
        if step != FxVec2::ZERO {
            let size = self.terrain.size_metres();
            let clamp = |p: FxVec2| {
                FxVec2::new(
                    p.x.clamp(Fx::ONE, size.x - Fx::ONE),
                    p.y.clamp(Fx::ONE, size.y - Fx::ONE),
                )
            };
            let ok = |p: FxVec2| {
                standing_on_blocked || self.nav.passable(motion.layer, motion.size_class, p)
            };
            let full = clamp(pos + step);
            let slide_x = clamp(pos + FxVec2::new(step.x, Fx::ZERO));
            let slide_y = clamp(pos + FxVec2::new(Fx::ZERO, step.y));
            if ok(full) {
                out.pos = full;
            } else if step.x != Fx::ZERO && ok(slide_x) {
                out.pos = slide_x;
            } else if step.y != Fx::ZERO && ok(slide_y) {
                out.pos = slide_y;
            } else {
                out.speed = Fx::ZERO;
            }
        }

        // A member settling into its slot counts too: another block sent to
        // the same spot overlaps its ranks, and the two shove each other round
        // for ever, a few metres short. The crowd pushes it back after every
        // step it makes, so within a rank's width of the slot every tick
        // counts, and a settling member's count only climbs.
        let settling = formation.is_some_and(|f| f.settling);
        if moving && !waiting && (formation.is_none() || settling) && out.stuck != u16::MAX {
            let progress = dist - (goal - out.pos).length();
            let near_slot = settling && dist <= radius * 2 + Fx::from_int(6);
            out.stuck = if near_slot || progress < Fx::ratio(1, 20) {
                (out.stuck + 1).min(GIVE_UP_TICKS)
            } else if settling {
                out.stuck
            } else {
                out.stuck.saturating_sub(2)
            };
            if out.stuck >= GIVE_UP_TICKS {
                out.stuck = u16::MAX;
            }
        }
        out
    }
}

/// How far short of `post` an aircraft flying `heading` with this turn radius
/// starts its turn onto the leg from `post` to `next`: the radius times the
/// tangent of half the corner, the corner capped at 120 degrees.
pub(crate) fn patrol_lead(
    heading: mc_core::Angle,
    post: FxVec2,
    next: FxVec2,
    turn_radius: Fx,
) -> Fx {
    if post == next {
        return Fx::ZERO;
    }
    let corner = heading
        .delta_to((next - post).angle())
        .unsigned_abs()
        .min(0x5555);
    let half = FxVec2::from_angle(mc_core::Angle(corner as u16 / 2));
    turn_radius * half.y / half.x.max(Fx::HALF)
}
