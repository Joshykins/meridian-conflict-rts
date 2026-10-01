//! Walking a builder into reach of its work. Work standing on ground no hull can
//! enter (a structure, or a product in a factory's bay) is walked up to from the side
//! the builder comes from, never toward its middle: the open cell nearest a
//! structure's middle can be a pocket sealed in among its hull pieces, and a builder
//! sent there finds no way in and stands where it is.

use crate::tables::flag;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2};

impl World {
    /// Walks a builder into range of `pos`. True once it is close enough to work.
    pub(crate) fn approach(
        &mut self,
        row: usize,
        pos: FxVec2,
        target_radius: Fx,
    ) -> Result<bool, SimError> {
        let range = self.work_range(row) + target_radius;
        if self.state.units.pos[row].distance(pos) <= range {
            self.state.units.flags[row] |= flag::HOLD;
            if self.state.units.has_flag(row, flag::HAS_FIELD) {
                self.stop_moving(row);
            }
            return Ok(true);
        }
        // A structure cannot walk over: what is out of its reach is given up.
        if self.bp(row).motion.is_none() {
            self.state.units.stuck_ticks[row] = u16::MAX;
        }
        if self.state.units.stuck_ticks[row] == u16::MAX {
            return Ok(false);
        }
        let goal = self.approach_goal(row, pos, target_radius);
        self.ensure_moving(row, goal, goal)?;
        Ok(false)
    }

    /// Where a builder walks to reach work at `pos`: the work itself when its hull can
    /// stand there, else a point three quarters of its reach out from the work's edge,
    /// on the side it comes from. The side is one of eight compass points, so builders
    /// coming from about the same way share a field.
    fn approach_goal(&self, row: usize, pos: FxVec2, target_radius: Fx) -> FxVec2 {
        let Some(motion) = self.bp(row).motion else {
            return pos;
        };
        if self.nav.passable(motion.layer, motion.size_class, pos) {
            return pos;
        }
        let away = self.state.units.pos[row] - pos;
        // A side counts when it is within 22.5 degrees of the bearing (tan 22.5 ~ 0.41).
        let side = |a: Fx, b: Fx| {
            if a.abs() * 12 < b.abs() * 5 {
                0
            } else if a < Fx::ZERO {
                -1
            } else {
                1
            }
        };
        let (sx, sy) = (side(away.x, away.y), side(away.y, away.x));
        let out = target_radius + self.work_range(row) * 3 / 4;
        let along = if sx != 0 && sy != 0 {
            out * Fx::ratio(7071, 10000)
        } else {
            out
        };
        pos + FxVec2::new(along * sx, along * sy)
    }
}
