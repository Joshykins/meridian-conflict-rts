//! The Aster faction (Asterian Reach Command): angular white plating over
//! dark frames, near-white blue emitters, a team-colour flash on every roof.
//!
//! Nominal sizes are the blueprint `radius` / `height` values from
//! `data/factions/aster/units/*.ron`; weapon assemblies end at the blueprint
//! muzzle offsets.

mod mechs;
mod parts;
mod structures;
mod vehicles;

use super::library::ModelDef;

pub(super) const MODELS: &[ModelDef] = &[
    // Command and construction.
    ModelDef::new("commander", 6.5, 15.0, mechs::commander),
    ModelDef::new("engineer", 3.6, 3.4, vehicles::engineer),
    // Land units.
    ModelDef::new("scout", 2.4, 1.8, vehicles::scout),
    ModelDef::new("bot_light", 2.6, 5.2, mechs::bot_light),
    ModelDef::new("tank_medium", 4.6, 3.4, vehicles::tank_medium),
    ModelDef::new("artillery_light", 4.2, 3.2, vehicles::artillery_light),
    ModelDef::new("tank_heavy", 6.2, 4.4, vehicles::tank_heavy),
    ModelDef::new("hover_tank", 5.4, 3.2, vehicles::hover_tank),
    ModelDef::new("missile_launcher", 5.2, 4.0, vehicles::missile_launcher),
    ModelDef::new("assault_bot", 6.8, 12.0, mechs::assault_bot),
    ModelDef::new("artillery_heavy", 7.0, 5.0, vehicles::artillery_heavy),
    // Structures.
    ModelDef::tiered("factory_land", [(46.0, 22.0), (46.0, 26.0), (46.0, 30.0)], structures::factory_land),
    ModelDef::tiered("extractor", [(14.0, 9.0), (14.0, 11.0), (14.0, 13.0)], structures::extractor),
    ModelDef::new("power", 14.0, 12.0, structures::power),
    ModelDef::new("storage_mass", 14.0, 8.0, structures::storage_mass),
    ModelDef::new("storage_energy", 14.0, 10.0, structures::storage_energy),
    ModelDef::new("turret", 7.0, 9.0, structures::turret),
    ModelDef::new("turret_heavy", 14.0, 13.0, structures::turret_heavy),
    ModelDef::new("artillery_static", 14.0, 12.0, structures::artillery_static),
    ModelDef::new("radar", 6.0, 20.0, structures::radar),
    ModelDef::new("wall", 8.0, 6.0, structures::wall),
];
