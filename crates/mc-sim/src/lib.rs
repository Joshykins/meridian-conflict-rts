//! The simulation: all game state and the 10 Hz tick that advances it.
//!
//! [`World`] owns the state tables ([`State`]), the immutable inputs (map,
//! blueprints) and the derived structures (spatial index, fog, flow-field
//! cache). `World::tick` is a pure function of `(State, commands)`: no floats,
//! no clocks, no thread-count dependence. See `docs/ARCHITECTURE.md`.

// The determinism gate (clippy.toml beside Cargo.toml lists what it bans; CLAUDE.md
// section 3). tests/determinism_gate.rs fails if these lines go.
#![warn(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::float_arithmetic
)]
#![expect(
    clippy::should_implement_trait,
    reason = "every state table has `hash(&self, &mut StateHasher)`; it is not std::hash::Hash, \
              because the lockstep hash must never depend on a std Hasher impl"
)]

pub mod adjacency;
pub mod ai;
pub mod ai_config;
mod anti_missile;
pub use ai_config::{AiConfig, Difficulty, Doctrine, Skill, BUILD_RANGE, INCOME_RANGE};
mod air_pick;
mod air_support;
pub mod air_wings;
pub mod aircraft_crash;
mod approach;
mod area_work;
mod assist_follow;
mod assist_work;
mod authored;
pub mod batch;
mod body;
mod bore_lay;
pub mod city;
mod cloud_seed;
mod cluster;
pub mod combat;
pub mod command;
mod contacts;
mod curve;
pub mod dash;
mod debug;
mod decks;
mod destruct;
mod dogfight;
mod doodads;
pub mod economy;
pub mod elevator;
pub mod fed_reload;
mod fight_line;
mod flak;
pub mod focus;
pub mod fog;
mod formation_destination;
pub mod formations;
pub mod gravitic;
pub mod growth;
mod guard;
mod held;
mod hover_flight;
mod impacts;
pub mod lance;
mod launch_cells;
mod line_of_fire;
mod lots;
mod marching;
pub mod mines;
pub mod mirror;
pub mod mounts;
pub mod movement;
pub mod nav;
mod naval;
mod naval_arms;
pub mod nukes;
mod orbit;
pub mod orbs;
pub mod orders;
pub mod pause;
pub mod perf;

mod bore;
pub mod placement;
pub use mc_core::print_heads;
pub mod lens;
pub mod mission;
mod ranks;
pub mod reclaim;
mod reclaim_area;
mod reclaim_heads;
mod reclaim_props;
mod reform;
mod relocate;
pub mod repair;
mod repair_field;
mod seabed;
mod shields;
pub mod sinking;
mod site_clearing;
pub mod site_repeat;
pub mod slots;
pub mod spatial;
mod staging;
mod stand_off;
mod standing;
pub mod state_hash;
pub mod store_lights;
mod strafe;
mod stranded;
mod strike_drones;
mod succession;
pub mod surge;
pub mod survival;
pub mod tables;
mod target_pick;
pub mod titan;
pub mod transport;
pub mod trees;
mod twins;
mod validate;
pub mod veterancy;
pub mod wake;
pub mod warp;
mod well_area;
pub mod world;
mod wreck_damage;

pub use command::{Command, PlayerCommand};
pub use mirror::{Refusal, RenderFrame, SimEvent};
pub use slots::Handle;
pub use succession::FallenArmy;
pub use survival::{SurvivalConfig, SurvivalRules, SurvivalStatus};
pub use tables::{FireState, UnitId, WreckId};
pub use validate::{decode_untrusted, MAX_SNAPSHOT_BYTES};
pub use veterancy::{veterancy_health, veterancy_need, VETERANCY_MAX};
pub use world::{
    lot_covers_point, pack_structure_pad, snap_to_build_grid, snap_turned_to_build_grid,
    MatchConfig, PlayerSetup, State, TickTimings, World, PAD_GHOST, PAD_NANITE, PAD_OWNER_MASK,
};

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Table {
    Units,
    Orders,
    Projectiles,
    Wrecks,
    Pads,
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
