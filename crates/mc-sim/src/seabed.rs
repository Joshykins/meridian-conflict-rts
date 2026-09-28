//! Walkers on the seabed (docs/STYLE.md "The navy").
//!
//! An amphibious walker keeps to the bottom wherever it goes. In water shallower than it
//! stands tall it wades, seen and shot at like anything ashore; once the sea closes over
//! its top it is under water with the dived hulls: only sonar finds it and only a torpedo
//! reaches it. Its guns fire only while their muzzles are out of the water, and its
//! torpedo tubes only while theirs are under it.

use mc_core::Fx;
use mc_data::{MoveLayer, Weapon};

use crate::world::World;

/// How far under the surface a torpedo tube must be before it will launch: a run started
/// right at the surface would broach.
const TUBE_DEPTH: Fx = Fx::ONE;

impl World {
    /// Whether `row` is an amphibious walker wholly under water, on the bed.
    pub(crate) fn on_seabed(&self, row: usize) -> bool {
        self.bp(row)
            .motion
            .is_some_and(|m| m.layer == MoveLayer::Amphibious)
            && self.submerged(row)
    }

    /// Whether a torpedo can take `row` for its mark: a hull in the water, or a walker on
    /// the bed under it.
    pub(crate) fn torpedo_can_mark(&self, row: usize) -> bool {
        self.in_water(row) || self.on_seabed(row)
    }

    /// Whether weapon `weapon` of amphibious walker `row` is kept silent by the water: a
    /// gun whose muzzle is under the surface, or a torpedo tube whose muzzle is not.
    /// Shoulder guns have their own rule (`drowned` in `combat.rs`).
    pub(crate) fn drowned_on_seabed(&self, row: usize, weapon: &Weapon) -> bool {
        if weapon.mount
            || self
                .bp(row)
                .motion
                .is_none_or(|m| m.layer != MoveLayer::Amphibious)
        {
            return false;
        }
        let muzzle = self.state.units.z[row] + weapon.muzzle.z;
        let water = self.terrain.water_level();
        if weapon.torpedo {
            muzzle > water - TUBE_DEPTH
        } else {
            muzzle < water
        }
    }
}
