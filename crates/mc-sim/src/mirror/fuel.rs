//! What the presentation sees of aircraft fuel (`crate::staging`).

use crate::World;
use mc_core::Fx;

/// An aircraft's tank, for the line under its health bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuelView {
    pub unit_id: u32,
    /// Zero (dry) to one (full).
    pub fill: f32,
}

impl World {
    /// Every tank on `viewer`'s side and its allies' (everyone's with no viewer): an enemy
    /// is not told how far its aircraft can still go.
    pub(super) fn write_fuel(&self, viewer: Option<u8>, out: &mut Vec<FuelView>) {
        out.clear();
        let units = &self.state.units;
        for row in units.slots.iter() {
            let Some(tank) = self.bp(row).fuel else {
                continue;
            };
            if viewer.is_some_and(|v| self.are_enemies(v, units.owner[row])) {
                continue;
            }
            out.push(FuelView {
                unit_id: units.id(row).0,
                fill: (units.fuel[row] / tank.range.max(Fx::ONE)).to_f32(),
            });
        }
    }
}
