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
/// Ticks without progress before the crowd stops steering a hull onto ground it cannot stand on.
const PINNED_TICKS: u16 = 20;
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
    /// The field's goal was built over: release it so the order asks again.
    repath: bool,
}

impl World {
    pub(crate) fn run_movement(&mut self) -> Result<(), SimError> {
        let rows = self.state.units.slots.rows();
        let span = mc_core::perf_span!("move.formation");
        let formation = self.formation_motion();
        drop(span);
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
        let span = mc_core::perf_span!("move.steer");
        let results: Vec<Vec<MoveOut>> = self.pool.parallel_map_chunks(rows, CHUNK, |_, range| {
            let units = &this.state.units;
            let mut out = Vec::new();
            for row in range {
                if !units.slots.is_alive(row)
                    || !units.is_active(row)
                    || this.drone_seated(row)
                    || this.warp_held(row)
                {
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
        drop(span);
        let span = mc_core::perf_span!("move.contacts");
        let rows: Vec<usize> = moves.iter().map(|m| m.row).collect();
        let mut at: Vec<FxVec2> = moves.iter().map(|m| m.pos).collect();
        self.resolve_mobile_contacts(&rows, &mut at);
        for (m, pos) in moves.iter_mut().zip(at) {
            m.pos = pos;
        }
        drop(span);
        let _span = mc_core::perf_span!("move.apply");
        // Striding walkers' ground counters before this move: their footfalls land after it.
        let striders: Vec<(usize, u32)> = moves
            .iter()
            .filter(|m| self.bp(m.row).stomp.is_some())
            .map(|m| (m.row, self.state.units.gait[m.row]))
            .collect();
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
                    .clamp(Fx::ratio(1, 20), Fx::ratio(8, 25));
                    let pass_height = ASSAULT_PASS_CLEARANCE
                        + (distance - Fx::from_int(60)).max(Fx::ZERO) * glide;
                    let ahead = m.pos + FxVec2::from_angle(m.heading) * motion.speed;
                    let surface = self.ground_surface(m.pos).max(self.ground_surface(ahead));
                    want_z =
                        (surface + pass_height).min(want_z.max(surface + ASSAULT_PASS_CLEARANCE));
                }
            }
            // A bomber on its run climbs ahead of rising ground, so a cliff top at the
            // mark does not find it still coming up off the low ground below.
            let bomb_run = !assault
                && attack_altitudes[row].is_none()
                && self.state.units.has_flag(row, flag::AIR_RUN)
                && self
                    .bp(row)
                    .motion
                    .is_some_and(|mo| mo.layer == MoveLayer::Air && !mo.hover)
                && self.bp(row).weapons.iter().any(|w| {
                    w.trajectory == mc_data::Trajectory::Ballistic && !w.missile && !w.torpedo
                })
                && !self.bp(row).weapons.first().is_some_and(|w| w.torpedo);
            if bomb_run {
                let motion = self.bp(row).motion.expect("bomber");
                let nose = FxVec2::from_angle(m.heading);
                // Far enough out to make a big rise at the run's climb rate.
                let ahead = (1..=9)
                    .map(|s| self.ground_surface(m.pos + nose * (motion.speed * s)))
                    .fold(self.ground_surface(m.pos), Fx::max);
                want_z = want_z.max(ahead + motion.altitude);
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
                Some(mo) if mo.layer == MoveLayer::Air && self.lands_on_order(row) => {
                    self.lift_vertical(row, m.pos, want_z)
                }
                Some(mo) if mo.layer == MoveLayer::Air => {
                    let height_left = self.state.units.z[row] - want_z;
                    // A lift ship comes down from the clouds and climbs back at its own rate.
                    let lift = self.bp(row).transport.map(|t| t.descent);
                    if height_left > Fx::ZERO && want_z == self.ground_surface(m.pos) {
                        // Ease the last 24 metres and cap the final step so landing
                        // never snaps to the ground. Takeoff retains its climb rate.
                        -((height_left / 2).clamp(TOUCHDOWN_SPEED, lift.unwrap_or(LANDING_SPEED))
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
                            || bomb_run
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
                        let share = (m.speed / mo.speed).clamp(Fx::ZERO, Fx::ONE);
                        -(Fx::from_int(turn * 1200 / self.air_turn_rate(row, &mo).max(1)) * share)
                            .floor_int()
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
                let slope = crate::world::pitch_to_limit(run.max(Fx::ONE), vertical_delta, 2184);
                let slope = mc_core::Angle::ZERO.delta_to(slope) as i32;
                // Lift straight up level; align to the flight path once underway.
                let share = (speed / Fx::from_int(20)).clamp(Fx::ZERO, Fx::ONE);
                let accel = ((speed - self.state.units.speed[row]) * Fx::from_int(360)).round_int();
                let clear = self.state.units.z[row] > self.ground_surface(m.pos) + Fx::from_int(12);
                let desired = if clear {
                    ((Fx::from_int(slope) * share).round_int() - accel).clamp(-2184, 2184)
                } else {
                    0
                };
                let current =
                    mc_core::Angle::ZERO.delta_to(self.state.units.arm_pitch[row][0]) as i32;
                let delta = desired - current;
                let step = if delta.abs() < 8 {
                    delta
                } else {
                    (delta / 8).clamp(-80, 80)
                };
                Some(mc_core::Angle((current + step) as u16))
            } else {
                None
            };
            let hover_flies = self.bp(row).motion.is_some_and(|mo| {
                crate::hover_flight::flies(
                    &mo,
                    self.bp(row).transport.is_some(),
                    self.bp(row).is_capital_ship(),
                )
            });
            let units = &mut self.state.units;
            if let Some(pitch) = capital_pitch {
                units.arm_pitch[row][0] = pitch;
            }
            if let Some(pitch) = assault_pitch {
                units.arm_pitch[row][0] = pitch;
            }
            match self.blueprints.unit(units.blueprint[row]).motion {
                Some(mo) if hover_flies => {
                    let before = units.air_velocity[row].xy();
                    let after = m.pos - units.pos[row];
                    crate::hover_flight::lean(units, row, &mo, m.heading, before, after);
                }
                _ => {
                    let bank = units.bank[row] as i32;
                    let delta = want_bank - bank;
                    // A fighter in a fight snaps into its bank: steering reads its
                    // turn back from the bank, so a slow roll is a slow turn.
                    let ease = if attack_altitudes[row].is_some() {
                        2
                    } else {
                        4
                    };
                    units.bank[row] = (bank
                        + if delta.abs() <= 4 {
                            delta
                        } else {
                            delta / ease
                        }) as i16;
                }
            }
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
            if m.repath && field != NO_FIELD {
                self.nav.release(field);
                let units = &mut self.state.units;
                units.field[row] = NO_FIELD;
                units.flags[row] &= !flag::HAS_FIELD;
            }
        }
        if !striders.is_empty() {
            self.run_stomps(&striders);
        }
        Ok(())
    }

    /// Lower airspeed buys a tighter fighter turn, up to 1.8 times cruise yaw.
    /// Both roll feedback and steering use the same rate so bank stays bounded.
    pub(crate) fn air_turn_rate(&self, row: usize, motion: &Motion) -> i32 {
        if self.state.units.has_flag(row, flag::AIR_RUN) && self.bp(row).has(cat::ANTI_AIR) {
            let speed = self.state.units.speed[row].max(motion.speed * Fx::ratio(5, 9));
            (Fx::from_int(motion.turn_rate as i32) * (motion.speed / speed).max(Fx::ONE))
                .floor_int()
        } else {
            motion.turn_rate as i32
        }
    }

    /// Steer round the hulls this one cannot shift, and whether it is
    /// getting out of one's way (at full speed, not a formation's crawl).
    ///
    /// A heavy on the move sweeps a lane ahead of it, two seconds of its
    /// drive. Standing in it, this one steps out to the nearer edge; beside
    /// it, it keeps out rather than cut back across. A tank ahead of its rank
    /// crawls for the block to catch up; with its rank past a Fulgur's nose
    /// it otherwise drifts back in front of it and is shoved along for good.
    /// Once the heavy has gone by, it crosses behind. A heavy across its way
    /// otherwise, within a second's drive, it aims past the near edge of, or,
    /// pressed on it square, goes along its flank toward the goal's side.
    fn round_heavies(
        &self,
        row: usize,
        pos: FxVec2,
        dir: FxVec2,
        to_goal: FxVec2,
        motion: &Motion,
    ) -> (FxVec2, bool) {
        let radius = self.bp(row).radius;
        let look = motion.speed.max(Fx::from_int(8));
        // Nearest along the way: (distance ahead, centre, combined reach).
        let mut first: Option<(Fx, FxVec2, Fx)> = None;
        // The nearest heavy whose lane this one is in or beside:
        // (distance behind it, course, signed distance off its line, lane half-width).
        let mut lane: Option<(Fx, FxVec2, Fx, Fx)> = None;
        let widest = Fx::from_int(64);
        self.index.query(
            pos,
            radius + PERSONAL_SPACE + look.max(widest),
            kind::UNIT,
            |e| {
                let other = e.row as usize;
                if other == row
                    || e.radius <= radius
                    || !self.unit_entry_is_current(e)
                    || !self.bp(other).is_mobile()
                    || self
                        .bp(other)
                        .motion
                        .is_some_and(|m| m.layer == MoveLayer::Air)
                    || self.state.units.has_flag(other, flag::IN_FACTORY)
                    || self.hulls_pass(row, other)
                    || self.give_way(row, other) <= Fx::ratio(7, 8)
                {
                    return true;
                }
                let reach = radius + e.radius + PERSONAL_SPACE;
                let rel = e.pos - pos;
                let speed = self.state.units.speed[other];
                if speed > Fx::ONE {
                    let course = FxVec2::from_angle(self.state.units.heading[other]);
                    let behind = (-rel).dot(course);
                    let off = course.cross(-rel);
                    if behind > Fx::ZERO
                        && behind < reach + speed * 2
                        && off.abs() < reach + Fx::from_int(6)
                        && lane.is_none_or(|(nearest, ..)| behind < nearest)
                    {
                        lane = Some((behind, course, off, reach + Fx::from_int(2)));
                    }
                }
                let ahead = rel.dot(dir);
                if ahead > Fx::ZERO
                    && ahead < reach + look
                    && dir.cross(rel).abs() < reach
                    && first.is_none_or(|(nearest, _, _)| ahead < nearest)
                {
                    first = Some((ahead, e.pos, reach));
                }
                true
            },
        );
        if let Some((_, course, off, half)) = lane {
            let left = off > Fx::ZERO || (off == Fx::ZERO && row.is_multiple_of(2));
            let out = if left { course.perp() } else { -course.perp() };
            if off.abs() < half {
                return (out, true);
            }
            // Beside it: nothing of the way on may lead back in.
            let inward = dir.dot(-out);
            if inward > Fx::ZERO {
                let along = dir + out * inward;
                return (
                    if along == FxVec2::ZERO {
                        course
                    } else {
                        along.normalize()
                    },
                    false,
                );
            }
        }
        let Some((_, centre, reach)) = first else {
            return (dir, false);
        };
        let rel = centre - pos;
        let lateral = dir.cross(rel);
        // Round the side it is already off to; dead ahead, the goal's side.
        let left = if lateral != Fx::ZERO {
            lateral < Fx::ZERO
        } else {
            let side = rel.cross(to_goal);
            side > Fx::ZERO || (side == Fx::ZERO && row.is_multiple_of(2))
        };
        let n = rel.normalize();
        let flank = if left { n.perp() } else { -n.perp() };
        if rel.length() <= reach {
            // Pressed on it: along the flank.
            (flank, false)
        } else {
            ((centre + flank * reach - pos).normalize(), false)
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
            repath: false,
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
                    if units.has_flag(other, flag::IN_FACTORY)
                        || self.hulls_pass(row, other)
                        || self.steps_over(row, other)
                    {
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
                        // Twice its share, as the push below is halved.
                        let share = self.give_way(row, other);
                        push += away
                            * if share == Fx::HALF {
                                overlap
                            } else {
                                overlap * share * 2
                            };
                        seen += 1;
                    }
                    seen < MAX_NEIGHBOURS
                });
        }

        // A strider stands wherever it can put its feet: structures are under it, not round it.
        let standing_on_blocked =
            !motion.stride && !self.nav.passable(motion.layer, motion.size_class, pos);
        let stranded = standing_on_blocked.then(|| self.stranded(row, pos, radius, motion));
        let moving = units.flags[row] & flag::HAS_FIELD != 0 && units.flags[row] & flag::HOLD == 0;
        let goal = formation.map(|f| f.goal).unwrap_or(units.move_goal[row]);
        let to_goal = goal - pos;
        let dist = to_goal.length();
        let transit_air = motion.layer == MoveLayer::Air
            && (motion.hover || units.flags[row] & flag::AIR_RUN == 0);
        // A lone aircraft with another waypoint queued flies through this one:
        // it neither brakes for it nor docks on it (`run_air_move` moves it on).
        let passing = transit_air && formation.is_none() && self.air_waypoint_after(row).is_some();
        let docking = transit_air
            && !passing
            && moving
            && dist <= AIR_DOCK_RADIUS
            && units.speed[row] <= (motion.speed / 3).min(AIR_DOCK_SPEED);
        let steering_goal = goal;

        let mut dir = FxVec2::ZERO;
        let mut waiting = false;
        if let Some(stranded) = stranded {
            dir = stranded.dir;
        } else if moving {
            if motion.layer == MoveLayer::Air
                || motion.stride
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
                    Steer::Rebuilt => {
                        out.repath = true;
                        waiting = true;
                    }
                    Steer::Unreachable => {
                        out.stuck = u16::MAX;
                        waiting = true;
                    }
                }
            }
        }

        let mut clearing = false;
        if dir != FxVec2::ZERO && !waiting && motion.layer != MoveLayer::Air {
            (dir, clearing) = self.round_heavies(row, pos, dir, to_goal, motion);
        }

        let max_speed = formation
            .filter(|_| !clearing)
            .map(|f| f.speed.min(motion.speed))
            .unwrap_or(motion.speed);
        let accel = motion.accel / DT;
        let mut target_speed = Fx::ZERO;
        if dir != FxVec2::ZERO && !waiting && !(transit_air && dist <= Fx::HALF) {
            let crowd = (push.length() / radius).min(Fx::ratio(3, 2));
            let mut steer = dir + push.normalize() * crowd;
            // Once it is getting nowhere, the crowd no longer turns a hull onto ground
            // it cannot stand on: pinned against a bank by a neighbour, it would face
            // the bank and never leave. (Before then the crowd steers it as ever: a
            // block marching round a slope keeps its spacing that way.)
            if steer != FxVec2::ZERO
                && motion.layer != MoveLayer::Air
                && units.stuck_ticks[row] >= PINNED_TICKS
                && [Fx::from_int(4), radius].into_iter().any(|ahead| {
                    !self.nav.passable(
                        motion.layer,
                        motion.size_class,
                        pos + steer.normalize() * ahead,
                    )
                })
            {
                steer = dir;
            }
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
                // A fighter in a fight pulls hard to put its nose on the target; a
                // gentle roll-in lets a target off the wing hold it in a circle.
                let gain = if engaged { 3 } else { 8 };
                let want_turn = (out.heading.delta_to(facing) as i32 / gain).clamp(-limit, limit);
                let old_turn = -(units.bank[row] as i32) * limit / 8192;
                let slew = (limit / if engaged { 3 } else { 5 }).max(1);
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
                    // Trade speed for turn radius while the opponent is off the
                    // nose, down to corner speed at 60 degrees off, then accelerate
                    // back onto the firing line. Off the nose is measured to the
                    // opponent too, not only to the goal: flying straight out of a
                    // turning circle it lies inside, the slower the fighter goes
                    // the sooner the target is outside the circle to turn in on.
                    let aim_off = out
                        .heading
                        .delta_to((units.air_aim[row] - pos).angle())
                        .unsigned_abs();
                    let turning = Fx::ratio(off.max(aim_off).min(0x2AAA) as i64, 0x2AAA);
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
                    && !passing
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
                // On the march a member chases a point a few lengths ahead of
                // its rank, and the anchor brakes the block into its goal. A
                // ship gets under way so slowly that braking for that point
                // would hold it well under the block's pace.
                let marching = formation.is_some_and(|f| !f.settling);
                // A ship marching in a block keeps way on through a turn and
                // carves it, with open water off its bow to carve it in. Alone,
                // picking its own way through a strait, or with its bow to the
                // shore, it swings (nearly) in place, as a tank pivots.
                let carves = motion.layer == MoveLayer::Naval
                    && marching
                    && self.nav.clear_segment(
                        motion.layer,
                        motion.size_class,
                        pos,
                        pos + FxVec2::from_angle(out.heading) * (max_speed + radius),
                    );
                target_speed = if off > 0x2000 {
                    if carves {
                        max_speed / 2
                    } else if formation.is_some() {
                        Fx::ZERO
                    } else {
                        // Swinging round onto a goal close by, the turning circle
                        // must fit inside the distance left, or the hull circles it.
                        let yaw = Fx::ratio(motion.turn_rate as i64 * DT as i64 * 355, 113 * 32768);
                        (max_speed / 5).min(dist * yaw / 2)
                    }
                } else {
                    max_speed
                };
                if moving && !standing_on_blocked && !(marching && motion.layer == MoveLayer::Naval)
                {
                    target_speed = target_speed
                        .min(dist * 2)
                        .min((motion.accel * dist * 2).sqrt());
                }
            }
        }
        // Lift clear before accelerating horizontally out of a landing site. A capital
        // ship has its own rule (`lift_speed_cap`), and glides in lower than this.
        if motion.layer == MoveLayer::Air
            && moving
            && !self.lands_on_order(row)
            && units.z[row] < self.ground_surface(pos) + Fx::from_int(16).min(motion.altitude)
        {
            target_speed = Fx::ZERO;
        }
        // A capital ship rises clear before it moves off, and brakes into its glide.
        if moving && self.lands_on_order(row) {
            if let Some(cap) = self.lift_speed_cap(row, pos, max_speed) {
                target_speed = target_speed.min(cap);
            }
        }
        out.speed = out.speed.approach(target_speed, accel);

        let flies = crate::hover_flight::flies(
            motion,
            self.bp(row).transport.is_some(),
            self.bp(row).is_capital_ship(),
        );
        let mut step = if motion.hover {
            // VTOL translation is independent of its target-facing fuselage, but only
            // ahead is fast: it drifts sideways or astern slowly (`hover_flight`).
            let previous = units.air_velocity[row].xy();
            let mut desired = dir * (out.speed / DT).min(dist);
            if flies {
                desired = crate::hover_flight::hold_to_airframe(desired, out.heading, motion);
            }
            let step = previous + (desired - previous).clamp_length(accel / DT);
            if flies {
                out.speed = out.speed.min(step.length() * DT + accel);
            }
            step
        } else if docking {
            to_goal.normalize() * (out.speed / DT).min(dist)
        } else if let Some(out_to) = stranded.and_then(|s| s.to) {
            // Straight for the way out, whatever the hull is facing.
            dir * (out.speed / DT).min(pos.distance(out_to))
        } else {
            FxVec2::from_angle(out.heading) * (out.speed / DT)
        };
        if transit_air && dist <= Fx::HALF && moving {
            out.speed = Fx::ZERO;
            step = FxVec2::ZERO;
        }
        // The hull's own drive, before the crowd's push goes on top.
        let drive = step;
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
                if motion.stride {
                    return self.stride_footing(p);
                }
                if self.nav.passable(motion.layer, motion.size_class, p) {
                    return true;
                }
                stranded.is_some_and(|s| s.may_step(motion, pos, p))
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
            } else if drive != step && drive != FxVec2::ZERO && ok(clamp(pos + drive)) {
                // The crowd shoves it onto ground it cannot stand on: two big hulls
                // overlapping by a bank push each other into it, and with the push
                // in every step neither could ever gather way and drive clear.
                out.pos = clamp(pos + drive);
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
    let half = FxVec2::from_angle(mc_core::Angle(corner / 2));
    turn_radius * half.y / half.x.max(Fx::HALF)
}
