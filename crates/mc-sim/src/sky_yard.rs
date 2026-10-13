//! Yards that build aloft (`mc_data::Builder::aloft`): the Regent is too big for any
//! lot, so its yard raises it in the sky over itself, at the height it cruises at and
//! under the yard's heading. The hull is no bay's product while it goes up: it hangs
//! there to be seen and shot at, and is finished by the ordinary build economy through
//! the yard's `build_target`. Builders helping it come to the yard, not up under three
//! kilometres of hull, and pour their work into the yard's intakes; the yard casts it up.
//! Finished, it gets under way like any aircraft leaving its factory (`run_produce`). If
//! the yard goes while the hull is half built, the hull comes down with it
//! (`drop_aloft_product`), breaking where it strikes, its wreck worth what was built.

use crate::tables::flag;
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::BlueprintId;

impl World {
    /// Whether the builder in `row` builds its products in the sky over it.
    pub(crate) fn builds_aloft(&self, row: usize) -> bool {
        self.bp(row).builder.as_ref().is_some_and(|b| b.aloft)
    }

    /// A unit started in the air: at its cruise height, a construction site with no lot
    /// (a warship built by engineers stands on one). A survival Gatewright's warships
    /// and a yard's hull go up this way.
    pub(crate) fn spawn_aloft(
        &mut self,
        bp: BlueprintId,
        side: u8,
        at: FxVec2,
        heading: Angle,
    ) -> Result<usize, SimError> {
        let row = self.spawn_unit(bp, side, at, heading, true)?;
        let full = self.unit_max_health(row);
        let units = &mut self.state.units;
        units.flags[row] |= flag::UNDER_CONSTRUCTION;
        units.build_progress[row] = Fx::ZERO;
        units.health[row] = full / 10;
        Ok(row)
    }

    /// The yard raising `product` in the sky, while it stands. Nothing for anything else.
    pub(crate) fn yard_of(&self, product: usize) -> Option<usize> {
        let units = &self.state.units;
        if !self.bp(product).built_aloft || !units.has_flag(product, flag::UNDER_CONSTRUCTION) {
            return None;
        }
        let id = units.id(product);
        units
            .slots
            .iter()
            .find(|&r| units.build_target[r] == id && self.builds_aloft(r))
    }

    /// Where a builder goes to work on `w`: a hull going up aloft is worked from the yard
    /// under it, anything else where it is.
    pub(crate) fn work_anchor(&self, w: usize) -> usize {
        self.yard_of(w).unwrap_or(w)
    }

    /// A hull that is built aloft and has lost its yard can never be finished: nobody
    /// else can build it.
    pub(crate) fn stranded_aloft(&self, w: usize) -> bool {
        self.bp(w).built_aloft
            && self.state.units.has_flag(w, flag::UNDER_CONSTRUCTION)
            && self.yard_of(w).is_none()
    }

    /// The yard in `row` is going: the hull it was raising comes down (`despawn_unit`
    /// lets a half-built hull built aloft fall, its wreck worth what was built of it).
    pub(crate) fn drop_aloft_product(&mut self, row: usize) {
        if !self.builds_aloft(row) {
            return;
        }
        let units = &mut self.state.units;
        if let Some(t) = units.row(units.build_target[row]) {
            if units.has_flag(t, flag::UNDER_CONSTRUCTION) {
                units.health[t] = Fx::ZERO;
            }
        }
    }

    /// The share of `row` that is built, zero to one: one for anything finished.
    pub(crate) fn built_share(&self, row: usize) -> Fx {
        let units = &self.state.units;
        if !units.has_flag(row, flag::UNDER_CONSTRUCTION) {
            return Fx::ONE;
        }
        let time = self.bp(row).build_time.max(Fx::EPSILON);
        (units.build_progress[row] / time).clamp(Fx::ZERO, Fx::ONE)
    }
}
