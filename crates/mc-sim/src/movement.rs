//! Movement: flow-field following, crowd avoidance, arrival.
//!
//! Each unit's new transform is computed from last tick's state only (its own
//! row, its neighbours through the spatial index, the flow field), so rows can
//! be processed in parallel in any order and the result is the same.

use crate::nav::Steer;
use crate::spatial::kind;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::{Motion, MoveLayer};

const DT: i32 = TICKS_PER_SECOND as i32;
const CHUNK: usize = 128;
/// Inside this distance a unit heads straight for its own slot instead of the shared field.
const DIRECT_RADIUS: Fx = Fx::from_int(72);
/// Neighbours considered for avoidance; the nearest cells are visited first.
const MAX_NEIGHBOURS: usize = 12;
/// Breathing room kept between hulls.
const PERSONAL_SPACE: Fx = Fx::from_int(2);
/// Largest shove per tick from overlapping neighbours, metres.
const MAX_PUSH: Fx = Fx::from_int(2);
/// A unit that has made no headway for this long gives up on its order.
const GIVE_UP_TICKS: u16 = 300;

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
        let this = &*self;
        let results: Vec<Vec<MoveOut>> = self.pool.parallel_map_chunks(rows, CHUNK, |_, range| {
            let units = &this.state.units;
            let mut out = Vec::new();
            for row in range {
                if !units.slots.is_alive(row) || !units.is_active(row) {
                    continue;
                }
                if let Some(motion) = this.bp(row).motion {
                    out.push(this.move_unit(row, &motion));
                }
            }
            out
        });

        for m in results.into_iter().flatten() {
            let units = &mut self.state.units;
            let row = m.row;
            if m.pos != units.pos[row] {
                units.flags[row] |= flag::MOVING;
                units.pos[row] = m.pos;
            }
            units.heading[row] = m.heading;
            units.speed[row] = m.speed;
            units.stuck_ticks[row] = m.stuck;
            let ground = self.terrain.height_at(m.pos);
            let layer = self.blueprints.unit(units.blueprint[row]).motion.map(|mo| mo.layer);
            units.z[row] = match layer {
                Some(MoveLayer::Hover) | Some(MoveLayer::Naval) => ground.max(self.terrain.water_level()),
                _ => ground,
            };
            if m.extend {
                self.nav.extend(units.field[row], m.pos)?;
            }
        }
        Ok(())
    }

    fn move_unit(&self, row: usize, motion: &Motion) -> MoveOut {
        let units = &self.state.units;
        let pos = units.pos[row];
        let radius = self.bp(row).radius;
        let mut out = MoveOut { row, pos, heading: units.heading[row], speed: units.speed[row], stuck: units.stuck_ticks[row], extend: false };

        // Shove from overlapping neighbours, and the nearest structure if we are inside one.
        let mut push = FxVec2::ZERO;
        let mut seen = 0;
        self.index.query(pos, radius + PERSONAL_SPACE, kind::UNIT, |e| {
            let other = e.row as usize;
            if other == row || !self.unit_entry_is_current(e) || !self.bp(other).is_mobile() {
                return true;
            }
            let d = pos - e.pos;
            let dist = d.length();
            let overlap = radius + e.radius + PERSONAL_SPACE - dist;
            if overlap > Fx::ZERO {
                // Coincident units separate along a direction fixed by their rows.
                let away = if dist > Fx::ZERO { d * (Fx::ONE / dist) } else { FxVec2::from_angle(mc_core::Angle((row as u16).wrapping_mul(9973))) };
                push += away * overlap;
                seen += 1;
            }
            seen < MAX_NEIGHBOURS
        });

        let standing_on_blocked = !self.nav.passable(motion.layer, motion.size_class, pos);
        let moving = units.flags[row] & flag::HAS_FIELD != 0 && units.flags[row] & flag::HOLD == 0;
        let goal = units.move_goal[row];
        let to_goal = goal - pos;
        let dist = to_goal.length();

        let mut dir = FxVec2::ZERO;
        let mut waiting = false;
        if standing_on_blocked {
            // A structure was placed on top of us: walk out the short way.
            if let Some(s) = self.index.nearest(pos, radius, kind::UNIT, |e| self.unit_entry_is_current(e) && self.bp(e.row as usize).is_structure()) {
                dir = (pos - s.pos).normalize();
            }
            if dir == FxVec2::ZERO {
                dir = FxVec2::from_angle(units.heading[row]);
            }
        } else if moving {
            if dist <= DIRECT_RADIUS {
                dir = to_goal.normalize();
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

        let max_speed = motion.speed;
        let accel = motion.accel / DT;
        let mut target_speed = Fx::ZERO;
        if dir != FxVec2::ZERO && !waiting {
            let crowd = (push.length() / radius).min(Fx::ratio(3, 2));
            let steer = dir + push.normalize() * crowd;
            let want = if steer == FxVec2::ZERO { dir.angle() } else { steer.angle() };
            out.heading = out.heading.turn_toward(want, motion.turn_rate);
            let off = out.heading.delta_to(want).unsigned_abs();
            // Tracked hulls pivot before they drive; brake into the destination.
            target_speed = if off > 0x2AAA { max_speed / 5 } else { max_speed };
            if moving && !standing_on_blocked {
                target_speed = target_speed.min((dist * 2).max(max_speed / 5));
            }
        }
        out.speed = out.speed.approach(target_speed, accel);

        let mut step = FxVec2::from_angle(out.heading) * (out.speed / DT);
        if push != FxVec2::ZERO {
            step += push.clamp_length(MAX_PUSH) * Fx::HALF;
        }
        if step != FxVec2::ZERO {
            let size = self.terrain.size_metres();
            let clamp = |p: FxVec2| FxVec2::new(p.x.clamp(Fx::ONE, size.x - Fx::ONE), p.y.clamp(Fx::ONE, size.y - Fx::ONE));
            let ok = |p: FxVec2| standing_on_blocked || self.nav.passable(motion.layer, motion.size_class, p);
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

        if moving && !waiting && out.stuck != u16::MAX {
            let progress = dist - (goal - out.pos).length();
            out.stuck = if progress < Fx::ratio(1, 20) { (out.stuck + 1).min(GIVE_UP_TICKS) } else { out.stuck.saturating_sub(2) };
            if out.stuck >= GIVE_UP_TICKS {
                out.stuck = u16::MAX;
            }
        }
        out
    }
}
