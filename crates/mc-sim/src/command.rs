//! Commands: the only way anything outside the simulation changes game state.
//!
//! A command carries no player id. The session attaches the issuing slot, and
//! the sim checks every referenced unit against it, so a client cannot order
//! units it does not own.

use crate::tables::UnitId;
use mc_core::{Angle, FxVec2};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

/// Most units one command may address. Larger selections are split by the client.
pub const MAX_COMMAND_UNITS: usize = 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Move { units: Vec<UnitId>, target: FxVec2, queue: bool },
    AttackMove { units: Vec<UnitId>, target: FxVec2, queue: bool },
    Attack { units: Vec<UnitId>, target: UnitId, queue: bool },
    Stop { units: Vec<UnitId> },
    /// `pos` is snapped to the build grid by the sim.
    Build { units: Vec<UnitId>, blueprint: BlueprintId, pos: FxVec2, heading: Angle, queue: bool },
    /// Help build, or repair, `target`. On a builder target: help whatever it builds.
    Assist { units: Vec<UnitId>, target: UnitId, queue: bool },
    ReclaimWreck { units: Vec<UnitId>, wreck: crate::tables::WreckId, queue: bool },
    /// Append `count` of `blueprint` to each factory's queue.
    Produce { factories: Vec<UnitId>, blueprint: BlueprintId, count: u8 },
    /// Drop the last queued `blueprint` from each factory.
    CancelProduce { factories: Vec<UnitId>, blueprint: BlueprintId },
    SetRepeat { factories: Vec<UnitId>, repeat: bool },
    SetRally { factories: Vec<UnitId>, pos: FxVec2 },
    Upgrade { units: Vec<UnitId> },
    SelfDestruct { units: Vec<UnitId> },
    Resign,
    /// Only honoured when the match was created with `cheats` on (test scenes, tools).
    DebugSpawn { owner: u8, blueprint: BlueprintId, pos: FxVec2, heading: Angle, count: u16 },
}

impl Command {
    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).expect("commands always serialize")
    }

    /// `None` for malformed input. A peer can send anything; never panic on it.
    pub fn decode(bytes: &[u8]) -> Option<Command> {
        use bincode::Options;
        // Bounded so a hostile length prefix cannot force a huge allocation.
        let cmd: Command = bincode::options()
            .with_fixint_encoding()
            .allow_trailing_bytes()
            .with_limit(64 * 1024)
            .deserialize(bytes)
            .ok()?;
        let units = match &cmd {
            Command::Move { units, .. }
            | Command::AttackMove { units, .. }
            | Command::Attack { units, .. }
            | Command::Stop { units }
            | Command::Build { units, .. }
            | Command::Assist { units, .. }
            | Command::ReclaimWreck { units, .. }
            | Command::Upgrade { units }
            | Command::SelfDestruct { units } => units.len(),
            Command::Produce { factories, .. }
            | Command::CancelProduce { factories, .. }
            | Command::SetRepeat { factories, .. }
            | Command::SetRally { factories, .. } => factories.len(),
            Command::Resign | Command::DebugSpawn { .. } => 0,
        };
        (units <= MAX_COMMAND_UNITS).then_some(cmd)
    }
}

/// A command with the slot that issued it, as delivered by the session for one tick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerCommand {
    pub player: u8,
    pub command: Command,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slots::Handle;

    #[test]
    fn round_trip() {
        let cmd = Command::Move { units: vec![Handle::new(3, 1), Handle::new(9, 0)], target: FxVec2::from_ints(100, -5), queue: true };
        assert_eq!(Command::decode(&cmd.encode()), Some(cmd));
    }

    #[test]
    fn garbage_is_rejected() {
        assert_eq!(Command::decode(&[0xFF; 3]), None);
        assert_eq!(Command::decode(&[]), None);
        // A Move whose length prefix claims far more units than the limit.
        let mut bytes = vec![0, 0, 0, 0];
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(Command::decode(&bytes), None);
    }
}
