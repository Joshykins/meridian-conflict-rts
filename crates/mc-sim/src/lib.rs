//! The simulation: all game state and the 10 Hz tick that advances it.
//!
//! [`World`] owns the state tables ([`State`]), the immutable inputs (map,
//! blueprints) and the derived structures (spatial index, fog, flow-field
//! cache). `World::tick` is a pure function of `(State, commands)`: no floats,
//! no clocks, no thread-count dependence. See `docs/ARCHITECTURE.md`.

pub mod ai;
pub mod ai_config;
pub use ai_config::{AiConfig, Difficulty, Doctrine, Skill};
mod air_support;
pub mod airbase;
pub mod aircraft_crash;
pub mod combat;
pub mod command;
mod debug;
pub mod economy;
pub mod fog;
mod formations;
mod guard;
mod line_of_fire;
pub mod mines;
pub mod mirror;
pub mod movement;
pub mod nav;
mod naval;
mod naval_arms;
mod orbit;
pub mod orders;
pub mod pause;

pub mod placement;
pub mod print_heads;
mod ranks;
pub mod reclaim;
mod reform;
mod seabed;
mod bore;
pub mod repair;
mod shields;
pub mod sinking;
pub mod transport;
pub mod slots;
pub mod spatial;
mod standing;
pub mod survival;
pub mod tables;
pub mod trees;
pub mod veterancy;
pub mod world;

pub use command::{Command, PlayerCommand};
pub use mirror::{RenderFrame, SimEvent};
pub use slots::Handle;
pub use survival::{SurvivalConfig, SurvivalRules, SurvivalStatus};
pub use tables::{FireState, UnitId, WreckId};
pub use veterancy::{veterancy_health, veterancy_need, VETERANCY_MAX};
pub use world::{
    footprint_cells, lot_covers_point, pack_structure_pad, snap_to_build_grid, MatchConfig,
    PlayerSetup, State, TickTimings, World, PAD_WELL,
};

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Table {
    Units,
    Orders,
    Projectiles,
    Wrecks,
    Stains,
    Pads,
    Fires,
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
            SimError::TableFull(t) => {
                write!(f, "simulation limit reached: the {t:?} table is full")
            }
            SimError::Path(e) => write!(f, "pathfinding: {e}"),
            SimError::Snapshot(e) => write!(f, "snapshot: {e}"),
            SimError::Setup(e) => write!(f, "match setup: {e}"),
        }
    }
}

impl std::error::Error for SimError {}
