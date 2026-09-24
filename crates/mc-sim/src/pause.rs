//! Paused work: a way to ease a stalling economy without throwing orders away.
//!
//! A paused builder, factory or upgrading structure keeps its queue exactly as it
//! was, but spends nothing: a site it is on stays half built, a factory's product
//! waits in the bay, an upgrade holds where it got to. It still walks to its next
//! site, fights and reclaims (reclaim pays). Whoever assists it stops with it, so
//! pausing a factory stops the whole line feeding it.

use crate::tables::UnitId;
use crate::World;
use mc_data::{Blueprints, UnitBlueprint};

/// Whether a unit of this kind has work that pausing stops: it builds, or it can
/// be upgraded or refitted. Pausing anything else would do nothing.
pub fn pausable(blueprints: &Blueprints, bp: &UnitBlueprint) -> bool {
    bp.builder.is_some() || bp.upgrades_to.is_some() || blueprints.refit_set(bp.id).is_some()
}

impl World {
    /// `Command::SetPaused`: the player's units among `ids` that have work to pause.
    pub(crate) fn set_paused(&mut self, player: u8, ids: &[UnitId], paused: bool) {
        for row in self.owned(player, ids, 0) {
            if pausable(&self.blueprints, self.bp(row)) {
                self.state.units.paused[row] = paused;
            }
        }
    }

    /// True when the unit in `row` may not spend on its work this tick.
    pub(crate) fn work_paused(&self, row: usize) -> bool {
        self.state.units.paused[row]
    }
}
