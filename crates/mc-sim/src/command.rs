//! Commands: the only way anything outside the simulation changes game state.
//!
//! A command carries no player id. The session attaches the issuing slot, and
//! the sim checks every referenced unit against it, so a client cannot order
//! units it does not own.

use crate::tables::{OrderKind, UnitId};
use mc_core::{Angle, FxVec2};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

/// Most units one command may address. Larger selections are split by the client.
pub const MAX_COMMAND_UNITS: usize = 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Move {
        units: Vec<UnitId>,
        target: FxVec2,
        queue: bool,
    },
    AttackMove {
        units: Vec<UnitId>,
        target: FxVec2,
        queue: bool,
    },
    Attack {
        units: Vec<UnitId>,
        target: UnitId,
        queue: bool,
    },
    Stop {
        units: Vec<UnitId>,
    },
    /// `pos` is snapped to the build grid by the sim.
    Build {
        units: Vec<UnitId>,
        blueprint: BlueprintId,
        pos: FxVec2,
        heading: Angle,
        queue: bool,
    },
    /// Help build, or repair, `target`. On a builder target: help whatever it builds.
    Assist {
        units: Vec<UnitId>,
        target: UnitId,
        queue: bool,
    },
    ReclaimWreck {
        units: Vec<UnitId>,
        wreck: crate::tables::WreckId,
        queue: bool,
    },
    /// Take a live unit apart for mass: one of the player's own, or an enemy's. It loses
    /// health as it goes, and what is reclaimed to nothing leaves no blast and no wreck.
    ReclaimUnit {
        units: Vec<UnitId>,
        target: UnitId,
        queue: bool,
    },
    /// Append `count` of `blueprint` to each factory's queue.
    Produce {
        factories: Vec<UnitId>,
        blueprint: BlueprintId,
        count: u8,
    },
    /// Drop the last queued `blueprint` from each factory.
    CancelProduce {
        factories: Vec<UnitId>,
        blueprint: BlueprintId,
    },
    SetRepeat {
        factories: Vec<UnitId>,
        repeat: bool,
    },
    SetRally {
        factories: Vec<UnitId>,
        pos: FxVec2,
    },
    /// Queues each unit's upgrade (`upgrades_to`) behind the orders it already has.
    Upgrade {
        units: Vec<UnitId>,
    },
    /// Takes the upgrade back out of each unit's queue; one already under way is scrapped.
    CancelUpgrade {
        units: Vec<UnitId>,
    },
    SelfDestruct {
        units: Vec<UnitId>,
    },
    Resign,
    /// Moves the `kind` orders (`Move`, `AttackMove` or `Build`) these units hold at exactly
    /// `from` to `to`. A `Build` is snapped to the build grid and has to fit where it lands.
    RelocateOrder {
        units: Vec<UnitId>,
        kind: OrderKind,
        from: FxVec2,
        to: FxVec2,
    },
    /// Only honoured when the match was created with `cheats` on (test scenes, tools),
    /// like every `Debug*` command below. `flags` takes `flag::DEBUG` bits; `build` is
    /// how complete the unit is in thousandths, a construction site below 1000.
    DebugSpawn {
        owner: u8,
        blueprint: BlueprintId,
        pos: FxVec2,
        heading: Angle,
        count: u16,
        flags: u16,
        build: u16,
    },
    /// Takes `permille` thousandths of each unit's full health away (negative: gives
    /// it back), whoever owns it and whether or not it is invulnerable.
    DebugDamage {
        units: Vec<UnitId>,
        permille: i16,
    },
    /// Takes units off the map with no death, wreck or scorch mark.
    DebugRemove {
        units: Vec<UnitId>,
    },
    /// Sets and clears `flag::DEBUG` bits: hold fire, invulnerable.
    DebugSetFlags {
        units: Vec<UnitId>,
        set: u16,
        clear: u16,
    },
    /// Makes each unit a construction site that is `permille` thousandths built; 1000 completes it.
    DebugSetBuild {
        units: Vec<UnitId>,
        permille: u16,
    },
    /// Empties the map: units, wrecks, shots in the air and scorch marks.
    DebugClear,
    /// From now on the issuing slot's commands are applied as `player`'s.
    DebugControl {
        player: u8,
    },
    /// Building costs `player` nothing and never stalls.
    DebugFreeBuild {
        player: u8,
        on: bool,
    },
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
            | Command::ReclaimUnit { units, .. }
            | Command::Upgrade { units }
            | Command::CancelUpgrade { units }
            | Command::SelfDestruct { units }
            | Command::RelocateOrder { units, .. }
            | Command::DebugDamage { units, .. }
            | Command::DebugRemove { units }
            | Command::DebugSetFlags { units, .. }
            | Command::DebugSetBuild { units, .. } => units.len(),
            Command::Produce { factories, .. }
            | Command::CancelProduce { factories, .. }
            | Command::SetRepeat { factories, .. }
            | Command::SetRally { factories, .. } => factories.len(),
            Command::Resign
            | Command::DebugSpawn { .. }
            | Command::DebugClear
            | Command::DebugControl { .. }
            | Command::DebugFreeBuild { .. } => 0,
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
        let cmd = Command::Move {
            units: vec![Handle::new(3, 1), Handle::new(9, 0)],
            target: FxVec2::from_ints(100, -5),
            queue: true,
        };
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
