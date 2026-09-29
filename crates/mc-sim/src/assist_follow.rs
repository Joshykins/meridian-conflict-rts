//! An engineer assisting a unit that walks off follows it closely, a little behind
//! and to one side, so it is at hand when the unit starts its next build. Its path is
//! kept while the unit stays near where it leads, and only its end moves with the
//! unit: a new path every tick (what walking at the unit itself asked for) never got
//! the engineer under way.

use crate::orders::CHASE_REPATH_DISTANCE;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2};

/// Gap an assisting engineer keeps from the unit it follows, hull to hull, metres.
const FOLLOW_GAP: i32 = 6;

impl World {
    /// One tick of `row` keeping close behind `t`, which it assists.
    pub(crate) fn follow_assisted(&mut self, row: usize, t: usize) -> Result<(), SimError> {
        let units = &self.state.units;
        let gap = self.bp(row).radius + self.bp(t).radius + Fx::from_int(FOLLOW_GAP);
        // Behind it, fanned out a little by row so several helpers do not pile on one spot.
        let fan = Angle::from_degrees(((row % 5) as i32 - 2) * 25);
        let behind = units.heading[t] + Angle::from_degrees(180) + fan;
        let spot = self.clamp_to_map(units.pos[t] + FxVec2::from_angle(behind) * gap);
        let near = self.bp(row).radius + Fx::from_int(4);
        if units.pos[row].distance(spot) <= near && !units.has_flag(t, flag::MOVING) {
            let moving = units.has_flag(row, flag::HAS_FIELD);
            self.state.units.flags[row] |= flag::HOLD;
            if moving {
                self.stop_moving(row);
            }
            return Ok(());
        }
        let goal = if units.has_flag(row, flag::HAS_FIELD)
            && units.field_goal[row].distance(spot) <= CHASE_REPATH_DISTANCE
        {
            units.field_goal[row]
        } else {
            spot
        };
        self.ensure_moving(row, goal, spot)
    }
}
