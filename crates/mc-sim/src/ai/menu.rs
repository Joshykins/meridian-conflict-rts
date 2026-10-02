//! What a side can build now, from every builder and factory it has.
use super::*;

pub(super) struct Menu {
    pub ids: Vec<BlueprintId>,
}

/// An unarmed spaceship with radar and a warp drive (the Vigil).
pub(super) fn sensor_ship(bp: &UnitBlueprint) -> bool {
    bp.has(cat::SPACE)
        && bp.is_mobile()
        && bp.weapons.is_empty()
        && bp.radar > Fx::ZERO
        && bp.warp.is_some()
}

impl World {
    /// Every blueprint some builder or factory of `player` has in its menu.
    pub(super) fn side_menu(&self, player: u8) -> Menu {
        let units = &self.state.units;
        let mut ids: Vec<BlueprintId> = units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && units.is_active(r))
            .filter_map(|r| self.bp(r).builder.as_ref())
            .flat_map(|b| b.builds.iter().copied())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        Menu { ids }
    }
}
