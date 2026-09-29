//! A lot being built on is cleared of the builder's own side's units: those standing
//! on it (assisting engineers most of all, gathered round the builder) step off it
//! to the nearest edge, and go back to what they were doing from there.

use crate::orders::order;
use crate::spatial::kind;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2};
use mc_data::BlueprintId;

/// Metres past the lot's edge a unit steps to.
const STEP_CLEAR: i32 = 3;

impl World {
    /// Sends `builder`'s side's units standing on the lot of a `bp` at `pos` off it.
    /// Units already on their way somewhere are left to go.
    pub(crate) fn send_off_site(
        &mut self,
        bp: &mc_data::UnitBlueprint,
        pos: FxVec2,
        builder: usize,
        blueprint: BlueprintId,
    ) -> Result<(), SimError> {
        let half = FxVec2::from_ints(
            bp.footprint.0 as i32 * mc_map::BUILD_CELL_M / 2,
            bp.footprint.1 as i32 * mc_map::BUILD_CELL_M / 2,
        );
        let owner = self.state.units.owner[builder];
        let mut off = Vec::new();
        self.index.query(pos, half.x.max(half.y), kind::UNIT, |e| {
            let r = e.row as usize;
            let units = &self.state.units;
            let d = e.pos - pos;
            if r != builder
                && self.unit_entry_is_current(e)
                && units.owner[r] == owner
                && self.bp(r).is_mobile()
                && !self.building_this_site(r, blueprint, pos)
                && d.x.abs() < half.x
                && d.y.abs() < half.y
                && self
                    .state
                    .orders
                    .front(units, r)
                    .is_none_or(|o| o.kind != OrderKind::Move)
            {
                // Out across the nearer edge.
                let gap = self.bp(r).radius + Fx::from_int(STEP_CLEAR);
                let (sx, sy) = (sign(d.x), sign(d.y));
                let exit = if half.x - d.x.abs() <= half.y - d.y.abs() {
                    FxVec2::new(pos.x + (half.x + gap) * sx, e.pos.y)
                } else {
                    FxVec2::new(e.pos.x, pos.y + (half.y + gap) * sy)
                };
                off.push((r, exit));
            }
            true
        });
        for (r, exit) in off {
            let exit = self.clamp_to_map(exit);
            self.state.orders.push_front(
                &mut self.state.units,
                r,
                order(OrderKind::Move, exit, Handle::NONE),
            )?;
            self.state.units.stuck_ticks[r] = 0;
        }
        Ok(())
    }
}

/// -1 or 1: which way from the middle; the middle itself goes the positive way.
fn sign(v: Fx) -> i32 {
    if v < Fx::ZERO {
        -1
    } else {
        1
    }
}
