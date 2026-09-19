//! The simulation: all game state and the 10 Hz tick that advances it.
//!
//! [`World`] owns the state tables ([`State`]), the immutable inputs (map,
//! blueprints) and the derived structures (spatial index, fog, flow-field
//! cache). `World::tick` is a pure function of `(State, commands)`: no floats,
//! no clocks, no thread-count dependence. See `docs/ARCHITECTURE.md`.

pub mod ai;
pub mod combat;
pub mod command;
pub mod economy;
pub mod fog;
pub mod mirror;
pub mod movement;
pub mod nav;
pub mod orders;
pub mod slots;
pub mod spatial;
pub mod tables;
pub mod world;

pub use command::{Command, PlayerCommand};
pub use mirror::{RenderFrame, SimEvent};
pub use slots::Handle;
pub use tables::{UnitId, WreckId};
pub use world::{MatchConfig, PlayerSetup, State, TickTimings, World};

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Table {
    Units,
    Orders,
    Projectiles,
    Wrecks,
    Stains,
    Flattens,
    FlowFields,
}

/// Errors that stop the match. Limits are never enforced by dropping things
/// quietly: the tick fails and the game reports why.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SimError {
    TableFull(Table),
    Path(String),
    Snapshot(String),
    Setup(String),
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimError::TableFull(t) => write!(f, "simulation limit reached: the {t:?} table is full"),
            SimError::Path(e) => write!(f, "pathfinding: {e}"),
            SimError::Snapshot(e) => write!(f, "snapshot: {e}"),
            SimError::Setup(e) => write!(f, "match setup: {e}"),
        }
    }
}

impl std::error::Error for SimError {}
