//! Paused work: a way to ease a stalling economy without throwing orders away.
//!
//! A paused builder, factory or upgrading structure keeps its queue exactly as it
//! was, but spends nothing: a site it is on stays half built, a factory's product
//! waits in the bay, an upgrade holds where it got to. It still walks to its next
//! site, fights and reclaims (reclaim pays). Whoever assists it stops with it, so
//! pausing a factory stops the whole line feeding it.
//!
//! A paused shield, radar, sonar, warp dampener or material fabricator powers down
//! instead: the system goes off (and any missile defence on the same unit with it),
//! a fabricator makes nothing, and its energy upkeep stops until it is resumed.

use crate::tables::UnitId;
use crate::World;
use mc_core::Fx;
use mc_data::{Blueprints, UnitBlueprint};

/// Whether a unit of this kind has something pausing stops: it builds, it can be
/// upgraded or refitted, or it runs a powered system. Pausing anything else would
/// do nothing.
pub fn pausable(blueprints: &Blueprints, bp: &UnitBlueprint) -> bool {
    bp.builder.is_some()
        || bp.upgrades_to.is_some()
        || bp.strategic.is_some()
        || blueprints.refit_set(bp.id).is_some()
        || powers_down(bp)
}

/// Whether pausing a unit of this kind switches a powered system off, and its
/// energy upkeep with it: a shield, radar, sonar, warp dampener or material fabricator
/// that draws energy.
/// A builder's pause holds its work only, so pausing the commander's
/// building never drops its Personal Shield.
pub fn powers_down(bp: &UnitBlueprint) -> bool {
    bp.builder.is_none()
        && bp.economy.energy_upkeep > Fx::ZERO
        && (bp.shield.is_some()
            || bp.radar > Fx::ZERO
            || bp.sonar > Fx::ZERO
            || bp.warp_damper.is_some()
            || bp.fabricator.is_some())
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

    /// True when the unit in `row` is paused with its powered system off: no
    /// shield, radar, sonar or missile defence, and no upkeep paid.
    pub(crate) fn powered_down(&self, row: usize) -> bool {
        self.state.units.paused[row] && powers_down(self.bp(row))
    }
}
