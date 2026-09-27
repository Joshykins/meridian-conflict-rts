//! Economy focus: what a stalling side builds first.
//!
//! When a side cannot pay for everything, everything slows by the same ratio
//! (`economy.rs`). Its player can pick one kind of construction to be paid in
//! full before the rest: new power, or new mines. The rest, upkeep and the mines'
//! own draw included, shares what that leaves.

use crate::World;
use mc_data::{cat, UnitBlueprint};
use serde::{Deserialize, Serialize};

/// What a side's economy pays first. The numbers cross the wire in
/// `Command::SetFocus`; never renumber them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Focus {
    /// Nothing first: every build slows alike.
    #[default]
    Neither = 0,
    /// Power plants being built or upgraded are paid first.
    Power = 1,
    /// Mines being built or upgraded are paid first.
    Materials = 2,
}

impl Focus {
    /// Whether building or upgrading a unit of this kind is paid first under this focus.
    pub fn covers(self, bp: &UnitBlueprint) -> bool {
        match self {
            Focus::Neither => false,
            Focus::Power => bp.categories & cat::POWER != 0,
            Focus::Materials => bp.categories & cat::EXTRACTOR != 0,
        }
    }
}

impl World {
    /// `Command::SetFocus`.
    pub(crate) fn set_focus(&mut self, player: u8, focus: Focus) {
        if let Some(pl) = self.state.players.get_mut(player as usize) {
            pl.focus = focus;
        }
    }
}
