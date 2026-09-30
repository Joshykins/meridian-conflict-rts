//! Economy focus: what a stalling side builds first, and what it builds last.
//!
//! When a side cannot pay for everything, everything slows by the same ratio
//! (`economy.rs`). Its player can set new mines and new power apart: paid in full
//! before the rest, or only out of what the rest leaves over. The rest, upkeep and
//! the mines' own draw included, is paid in between. Reclaimers go first with the
//! mines, but are never held back with them: only First changes when they are paid.
//! A refit that makes materials or energy goes First with either resource it makes,
//! and is likewise never held back.

use crate::World;
use mc_data::{cat, Blueprints, UnitBlueprint};
use serde::{Deserialize, Serialize};

/// When a kind of construction is paid in a stall. The numbers cross the wire in
/// `Command::SetFocus`; never renumber them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Priority {
    /// With the rest: slows alike.
    #[default]
    Even = 0,
    /// Paid in full before the rest.
    First = 1,
    /// Paid only out of what the rest leaves over.
    Last = 2,
}

impl Priority {
    /// The tier it is paid in, in paying order (`economy.rs`).
    pub fn tier(self) -> usize {
        match self {
            Priority::First => 0,
            Priority::Even => 1,
            Priority::Last => 2,
        }
    }
}

/// How a side pays for new mines and new power in a stall.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Focus {
    pub mines: Priority,
    pub power: Priority,
}

impl Focus {
    /// When building or upgrading a unit of this kind is paid under this focus.
    pub fn priority(self, bp: &UnitBlueprint, blueprints: &Blueprints) -> Priority {
        if bp.categories & cat::EXTRACTOR != 0 {
            self.mines
        } else if bp.categories & cat::POWER != 0 {
            self.power
        } else if self.mines == Priority::First && reclaims(bp, blueprints) {
            Priority::First
        } else if let Some((set, slot, module)) = blueprints.kit(bp.id) {
            // A refit that makes materials or energy (the Material Formation Engine) goes
            // first when what it makes does. It is never held back to Last: it is also
            // the commander's, and stays with the rest.
            let m = set.module(slot, module);
            let first = (m.makes_mass && self.mines == Priority::First)
                || (m.makes_energy && self.power == Priority::First);
            if first {
                Priority::First
            } else {
                Priority::Even
            }
        } else {
            Priority::Even
        }
    }

    /// Packed for the state hash.
    pub(crate) fn bits(self) -> u64 {
        self.mines as u64 | (self.power as u64) << 2
    }
}

/// A unit whose work is reclaiming: a scavenger tower, a salvage unit, a salvage
/// drone or the carrier of them. Builders reclaim too, but building is their work.
fn reclaims(bp: &UnitBlueprint, blueprints: &Blueprints) -> bool {
    bp.builder.is_none()
        && (bp.reclaimer.is_some()
            || bp
                .drone
                .is_some_and(|d| blueprints.unit(d).reclaimer.is_some()))
}

impl World {
    /// `Command::SetFocus`.
    pub(crate) fn set_focus(&mut self, player: u8, focus: Focus) {
        if let Some(pl) = self.state.players.get_mut(player as usize) {
            pl.focus = focus;
        }
    }
}
