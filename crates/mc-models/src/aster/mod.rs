//! The Aster faction (Asterian Reach Command): angular white plating over
//! dark frames, near-white blue emitters, a team-colour flash on every roof.
//!
//! Nominal sizes are the blueprint `radius` / `height` values from
//! `data/factions/aster/units/*.ron`; weapon assemblies end at the blueprint
//! muzzle offsets.

mod aa;
pub(super) mod air;
mod airbase;
mod assault_tank;
mod bore_tank;
mod citadel;
mod culverin;
mod drone_port;
pub(crate) use citadel::RAIL as CITADEL_RAIL;
mod factories;
mod mechs;
pub(crate) use mechs::RAIL as COMMANDER_RAIL;
mod mine;
mod missile_defense;
mod naval;
pub(crate) use naval::NARWHAL_RAIL;
mod parts;
mod reactor;
pub(super) mod reclaim_tower;
mod reclaimers;
mod strategic;
mod structures;
pub(super) mod titan;
mod vehicles;
mod wall;
pub(super) mod zenith;

use super::library::ModelDef;

/// The reclaim tower's authored (radius, height) per tier.
const RECLAIM_TOWER: [(f32, f32); 3] = [
    (reclaim_tower::RADIUS, reclaim_tower::HEIGHT[0]),
    (reclaim_tower::RADIUS, reclaim_tower::HEIGHT[1]),
    (reclaim_tower::RADIUS, reclaim_tower::HEIGHT[2]),
];

pub(super) const MODELS: &[ModelDef] = &[
    // Command and construction.
    ModelDef::new("commander", 6.5, 15.0, mechs::commander),
    ModelDef::new("engineer", 3.6, 3.4, vehicles::engineer),
    // Land units.
    ModelDef::new("scout", 2.4, 1.8, vehicles::scout),
    ModelDef::new("bot_light", 2.6, 5.2, mechs::bot_light),
    ModelDef::new("tank_light", 4.6, 3.4, vehicles::tank_light),
    ModelDef::new("artillery_light", 4.2, 3.2, vehicles::artillery_light),
    ModelDef::new("tank_heavy", 6.2, 4.4, vehicles::tank_heavy),
    ModelDef::new("hover_tank", 5.4, 3.2, vehicles::hover_tank),
    ModelDef::new("missile_launcher", 5.2, 4.0, vehicles::missile_launcher),
    ModelDef::new("assault_bot", 6.8, 12.0, mechs::assault_bot),
    ModelDef::new("artillery_heavy", 7.0, 5.0, vehicles::artillery_heavy),
    ModelDef::new("bore_tank", 8.2, 4.8, bore_tank::bore_tank),
    ModelDef::new("assault_tank", 19.0, 15.0, assault_tank::assault_tank),
    ModelDef::new("titan", 40.0, 120.0, titan::titan),
    // Land reclaimers, as design variants until the user picks (CLAUDE.md section 9).
    ModelDef::new("gleaner~a", 4.6, 3.4, reclaimers::gleaner_a),
    ModelDef::new("gleaner~b", 4.6, 3.4, reclaimers::gleaner_b),
    ModelDef::new("gleaner~c", 4.6, 3.4, reclaimers::gleaner_c),
    ModelDef::new("thresher~a", 6.2, 4.4, reclaimers::thresher_a),
    ModelDef::new("thresher~b", 6.2, 4.4, reclaimers::thresher_b),
    ModelDef::new("thresher~c", 6.2, 4.4, reclaimers::thresher_c),
    ModelDef::new("titan_sabot", 7.5, 6.0, titan::sabot),
    // Air units.
    ModelDef::new("interceptor", 3.6, 1.8, air::interceptor),
    ModelDef::new("bomber", 5.6, 2.6, air::bomber),
    ModelDef::new("air_scout", 3.5, 1.7, air::scout_air),
    ModelDef::new("rotor_gunship", 5.5, 3.2, air::rotor_gunship),
    ModelDef::new("support_air", 8.0, 3.5, air::support),
    ModelDef::new("support_air~a", 9.2, 4.0, air::support_a),
    ModelDef::new("support_air~b", 9.2, 4.0, air::support_b),
    ModelDef::new("support_air~c", 9.2, 4.0, air::support_c),
    ModelDef::new("reclaim_carrier", 10.0, 4.5, air::carrier),
    ModelDef::new("light_transport", 58.0, 38.0, air::light_transport),
    ModelDef::new("lift_ship", 160.0, 95.0, air::lift_ship),
    ModelDef::new("space_frigate", 150.0, 70.0, air::space_frigate),
    ModelDef::new("reclaim_drone", 1.8, 1.2, air::drone),
    ModelDef::new("gunship", 7.5, 3.5, air::gunship),
    ModelDef::new("fire_bomber", 13.0, 5.0, air::fortress),
    ModelDef::new("torpedo_bomber", 7.2, 3.0, air::torpedo_bomber),
    ModelDef::new("interceptor_t2", 5.0, 2.4, air::interceptor_t2),
    ModelDef::new("superiority", 7.0, 3.2, air::superiority),
    ModelDef::new("strategic_bomber", 16.0, 4.5, air::strategic),
    ModelDef::new("assault_air", 12.0, 5.5, air::assault),
    ModelDef::new("aa_gun", 6.0, 7.0, aa::gun),
    ModelDef::new("flak_battery", 10.0, 8.0, aa::flak_battery),
    ModelDef::new("aa_sam", 12.0, 14.0, aa::sam),
    ModelDef::tiered(
        "mobile_aa",
        // Two tiers: tech 3 has no tracked AA of its own and is drawn as the Squall.
        [(4.0, 4.5), (5.5, 5.5), (5.5, 5.5)],
        aa::mobile,
    ),
    // Naval units: a hull's origin is its waterline, `height` what stands above it.
    ModelDef::new("attack_boat", 6.0, 4.0, naval::attack_boat),
    ModelDef::new("frigate", 15.0, 10.0, naval::frigate),
    ModelDef::new("submarine", 10.0, 3.6, naval::submarine),
    ModelDef::tiered(
        "sonar",
        [(6.0, 12.0), (6.0, 15.0), (6.0, 18.0)],
        naval::sonar,
    ),
    ModelDef::new("salvage_boat", 8.0, 6.0, naval::salvage_boat),
    // The reclaim boat's open designs, until the user picks one (CLAUDE.md section 9).
    ModelDef::new("reclaim_boat~a", 8.0, 6.0, naval::reclaim_boat_a),
    ModelDef::new("reclaim_boat~b", 8.0, 6.0, naval::reclaim_boat_b),
    ModelDef::new("reclaim_boat~c", 8.0, 6.0, naval::reclaim_boat_c),
    ModelDef::new("destroyer", 22.0, 12.0, naval::destroyer),
    ModelDef::new("aa_cruiser", 22.0, 14.0, naval::aa_cruiser),
    ModelDef::new("missile_ship", 20.0, 10.0, naval::missile_ship),
    ModelDef::new("submarine_hunter", 14.0, 4.2, naval::submarine_hunter),
    ModelDef::new("shield_boat", 16.0, 12.0, naval::shield_boat),
    ModelDef::new("battleship", 72.0, 38.0, naval::battleship),
    ModelDef::new("carrier", 60.0, 24.0, naval::carrier),
    ModelDef::new("rail_trimaran", 84.0, 34.0, naval::rail_trimaran),
    ModelDef::new("submarine_strategic", 30.0, 5.0, naval::submarine_strategic),
    // Structures.
    ModelDef::tiered(
        "factory_land",
        [(46.0, 28.0), (46.0, 34.0), (46.0, 42.0)],
        factories::factory_land,
    ),
    ModelDef::tiered(
        "factory_air",
        [(46.0, 26.0), (46.0, 34.0), (46.0, 42.0)],
        factories::factory_air,
    ),
    ModelDef::tiered(
        "factory_naval",
        [(46.0, 26.0), (46.0, 34.0), (46.0, 42.0)],
        factories::factory_naval,
    ),
    ModelDef::tiered(
        "extractor",
        [(14.0, 9.0), (14.0, 11.0), (14.0, 13.0)],
        structures::extractor,
    ),
    ModelDef::tiered(
        "core_mine",
        [(40.5, 40.0), (40.5, 66.0), (40.5, 70.0)],
        mine::core_mine,
    )
    .with_tier_4(),
    ModelDef::tiered(
        "power",
        [(10.5, 10.0), (22.5, 22.0), (46.0, 38.0)],
        reactor::power,
    ),
    ModelDef::tiered(
        "storage_mass",
        [(16.5, 8.0), (16.5, 13.0), (16.5, 19.0)],
        structures::storage_mass,
    ),
    ModelDef::new("storage_energy", 14.0, 10.0, structures::storage_energy),
    ModelDef::new("turret", 7.0, 9.0, structures::turret),
    ModelDef::new("turret_heavy", 14.0, 13.0, structures::turret_heavy),
    ModelDef::new("artillery_static", 14.0, 12.0, structures::artillery_static),
    // The tech 3 rail point defence, at blueprint scale (`citadel::TRUNNION` is its pivot).
    ModelDef::new("citadel", 20.0, 17.0, citadel::citadel),
    ModelDef::new("citadel_casing", 2.2, 1.3, citadel::casing),
    ModelDef::new(
        "missile_defense",
        6.0,
        13.0,
        missile_defense::missile_defense,
    ),
    ModelDef::tiered(
        "radar",
        [(6.0, 20.0), (6.0, 24.0), (6.0, 28.0)],
        structures::radar,
    ),
    ModelDef::tiered(
        "reclaimer",
        [(11.0, 12.0), (11.0, 12.0), (11.0, 14.0)],
        structures::reclaimer,
    ),
    // The reclaim tower (a 4x4 lot, `reclaim_tower`): design variants until the user picks
    // (CLAUDE.md section 9). All three share the head rig (`reclaim_tower::PIVOT_Z`, `EMIT_X`).
    ModelDef::tiered("reclaim_tower~a", RECLAIM_TOWER, reclaim_tower::derrick),
    ModelDef::tiered("reclaim_tower~b", RECLAIM_TOWER, reclaim_tower::spire),
    ModelDef::tiered("reclaim_tower~c", RECLAIM_TOWER, reclaim_tower::legs),
    ModelDef::tiered(
        "shield",
        [(16.5, 40.0), (16.5, 40.0), (16.5, 52.0)],
        structures::shield,
    ),
    // Authored at blueprint scale: a 12 m cell, pieces for each quarter (`wall`).
    ModelDef::new("wall", 6.0, 4.5, wall::wall),
    ModelDef::tiered(
        "airbase",
        [(34.0, 9.0), (34.0, 10.0), (34.0, 12.0)],
        airbase::airbase,
    ),
    // Strategic weapons: the nuclear silo (tech 4, a look of its own) and the interceptor array.
    ModelDef::new("nuke_silo", 42.5, 26.0, strategic::nuke_silo).with_tier_4(),
    ModelDef::new("nuke_defense", 18.75, 20.0, strategic::nuke_defense),
    // The tech 4 map gun, at blueprint scale (`culverin::TRUNNION` is its pivot).
    ModelDef::new("culverin", 30.0, 27.0, culverin::culverin).with_tier_4(),
    // The tech 4 anti-ship rail cannon (`zenith::ZENITH_RAIL` holds its barrel anchors).
    ModelDef::new("anti_ship_rail", 46.0, 86.0, zenith::zenith),
];
