//! A hull standing where it cannot: a structure put down on it, or a ship set
//! down on a headland or in water too shallow or narrow for its size class.
//! It makes for the nearest point its hull can stand on, straight there, and
//! never along its heading: a ship that did drove on across the land until it
//! happened on water, a landlocked pond as likely as the sea beside it.

use crate::spatial::kind;
use crate::World;
use mc_core::FxVec2;
use mc_data::{Motion, MoveLayer};

/// Where a stranded hull goes, and which way it heads this tick.
#[derive(Clone, Copy)]
pub(crate) struct Stranded {
    /// The nearest point its hull can stand on; `None` when there is none in reach.
    pub(crate) to: Option<FxVec2>,
    pub(crate) dir: FxVec2,
}

impl World {
    /// The way out for the hull at `row`, standing at `pos` on ground it cannot.
    pub(crate) fn stranded(
        &self,
        row: usize,
        pos: FxVec2,
        radius: mc_core::Fx,
        motion: &Motion,
    ) -> Stranded {
        if let Some(to) = self
            .nav
            .nearest_passable(motion.layer, motion.size_class, pos)
        {
            return Stranded {
                to: Some(to),
                dir: (to - pos).normalize(),
            };
        }
        // Nothing to stand on in reach. A ship stays put rather than drive
        // across land; anything else walks out of the structure the short way.
        let mut dir = FxVec2::ZERO;
        if motion.layer != MoveLayer::Naval {
            if let Some(s) = self.index.nearest(pos, radius, kind::UNIT, |e| {
                self.unit_entry_is_current(e) && self.bp(e.row as usize).is_structure()
            }) {
                dir = (pos - s.pos).normalize();
            }
            if dir == FxVec2::ZERO {
                dir = FxVec2::from_angle(self.state.units.heading[row]);
            }
        }
        Stranded { to: None, dir }
    }
}

impl Stranded {
    /// Whether a stranded hull at `from` may step onto `to`, which its hull
    /// cannot stand on either: only closer to the way out (the crowd's push
    /// included), never further over the land. A ship with no way out stays put.
    pub(crate) fn may_step(&self, motion: &Motion, from: FxVec2, to: FxVec2) -> bool {
        match self.to {
            Some(out) => to.distance_sq(out) < from.distance_sq(out),
            None => motion.layer != MoveLayer::Naval,
        }
    }
}
