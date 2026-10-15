//! The determinism matrix: one match with every domain in it (land, sea, under
//! the sea, air, a titan, a nuclear strike, a map gun, a battle scorpion's held beam and
//! curving charges, a wake tank's cone, a warp into a dampener and the stun it leaves, wrecks worn down by
//! blasts, two factories' linked batch forming up, a self-destruct counting down across
//! the snapshot, city blocks shot, set alight and brought down, a space elevator's cable
//! clamped and its first platform docked, a Fulgur raised on Repeat and its lot laid
//! again, tanks on a bridge's deck over the sea while the fleets pass under it, a
//! Regent's weapon groups turning and firing gun by gun, a second Regent's Edict
//! gathering across the snapshot on a moving hull and firing, a third raised aloft over
//! its yard across the snapshot and getting under way after it) must hash identically at
//! every worker count and after a snapshot is restored mid-match.
//! gathering across the snapshot on a moving hull and firing, a third raised aloft over
//! its yard across the snapshot and getting under way after it) must hash identically at
//! every worker count and after a snapshot is restored mid-match.
//!
//! `battle.rs` covers a land-only battle the same way; this is the one to extend
//! when a new domain or system arrives (CLAUDE.md, section 3).
//!
//! It also prints the final hash, so the Windows and Linux builds can be compared:
//! `scripts/determinism-cross.sh` runs it on both and diffs the lines.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{Deck, Heightfield};
use mc_sim::edict::EdictPhase;
use mc_sim::focus::{Focus, Priority};
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// 4 km square: land in the west, sea (20 m deep) in the east; a bridge's deck
/// (`decks.rs`) runs out from the shore over the sea across the middle, 20 m over the water.
const CELLS: u32 = 512;
const SHORE_CELL: usize = 200;
/// Long enough for the fleets to close and meet torpedoes with interceptors, which they do
/// late: the Regent overhead draws their guns first.
const TICKS: u32 = 700;
const SNAPSHOT_AT: u32 = 240;

fn terrain() -> Heightfield {
    let side = CELLS as usize + 1;
    let mut samples = vec![0u16; side * side];
    for y in 0..side {
        for x in 0..SHORE_CELL {
            samples[y * side + x] = 40;
        }
    }
    Heightfield::from_samples(CELLS, CELLS, samples, Fx::ZERO, Fx::ONE, Fx::from_int(20))
        .with_decks(vec![Deck {
            a: FxVec2::from_ints(1_500, 2_048),
            b: FxVec2::from_ints(3_000, 2_048),
            half: Fx::from_int(60),
            top: Fx::from_int(40),
        }])
}

fn world(threads: usize) -> World {
    let blueprints =
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let mut props = city();
    let salvage_prop = props.len() as u32;
    props.push(mc_map::Prop {
        kind: mc_map::PropKind::CityBillboard,
        pos: FxVec2::from_ints(100, 3650),
        heading: Angle::ZERO,
        scale_milli: 1000,
        wear_milli: 0,
    });
    let map = MapData {
        doodads: [
            (
                0,
                mc_map::placed::Doodad {
                    id: 1,
                    name: "matrix_billboard".into(),
                    prop_index: 0,
                    tint: [255; 3],
                    blocking: true,
                    health: 1400,
                    reclaim: 0,
                    footprint: Some(mc_map::placed::Footprint {
                        half: FxVec2::from_ints(12, 3),
                        height: Fx::from_int(15),
                    }),
                },
            ),
            (
                salvage_prop,
                mc_map::placed::Doodad {
                    id: 2,
                    name: "matrix_salvage".into(),
                    prop_index: salvage_prop,
                    tint: [255; 3],
                    blocking: true,
                    health: 0,
                    reclaim: 10_000,
                    footprint: Some(mc_map::placed::Footprint {
                        half: FxVec2::from_ints(8, 3),
                        height: Fx::from_int(10),
                    }),
                },
            ),
        ]
        .into_iter()
        .collect(),
        name: "determinism".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(800, 500), FxVec2::from_ints(800, 3600)],
        props,
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 1234,
        players: vec![player("south", 0), player("north", 1)],
        cheats: true,
        fog: true,
        spawn_commanders: false,
    };
    let mut world = World::with_terrain(
        terrain(),
        map,
        Arc::new(blueprints),
        Arc::new(Pool::new(threads)),
        &config,
    )
    .unwrap();
    use mc_sim::mission::{Action, Condition, Definition, Event, Objective};
    world
        .begin_mission(Definition {
            key: "determinism_mission".into(),
            placed: mc_map::placed::Placed {
                next_id: 3,
                doodads: world.map.doodads.values().cloned().collect(),
                ..Default::default()
            },
            events: vec![
                Event {
                    name: "opening".into(),
                    when: Condition::Elapsed(0),
                    actions: vec![
                        Action::Say {
                            line: "matrix.comms".into(),
                            ticks: 400,
                        },
                        Action::Objective {
                            line: "matrix.objective".into(),
                            status: Objective::Active,
                        },
                        Action::Checkpoint(1),
                    ],
                },
                Event {
                    name: "prop_destroyed".into(),
                    when: Condition::PropDead("matrix_billboard".into()),
                    actions: vec![Action::Say {
                        line: "matrix.prop".into(),
                        ticks: 10,
                    }],
                },
                Event {
                    name: "after_snapshot".into(),
                    when: Condition::All(vec![
                        Condition::After {
                            event: "opening".into(),
                            ticks: 400,
                        },
                        Condition::DialogueIdle,
                    ]),
                    actions: vec![
                        Action::Objective {
                            line: "matrix.objective".into(),
                            status: Objective::Complete,
                        },
                        Action::Checkpoint(2),
                    ],
                },
            ],
        })
        .unwrap();
    world
}

/// City blocks (`city.rs`): houses round where the warhead lands, which it brings
/// down; worn houses and a wall among the north's buildings, which the strike
/// missiles and the map guns set alight; a row of blocks across the middle.
fn city() -> Vec<mc_map::Prop> {
    use mc_map::{Prop, PropKind};
    let at = |kind, x, y, heading: u16, wear_milli| Prop {
        kind,
        pos: FxVec2::from_ints(x, y),
        heading: Angle(heading),
        scale_milli: 1000,
        wear_milli,
    };
    let mut props = vec![
        at(PropKind::CityBillboard, 840, 3300, 0, 0),
        at(PropKind::CityHouse, 960, 3300, 0, 300),
        at(PropKind::CityTenement, 900, 3390, 0x4000, 0),
        at(PropKind::CityOffice, 900, 3200, 0x2000, 0),
        at(PropKind::CityHouse, 450, 3750, 0, 500),
        at(PropKind::CityHouse, 600, 3760, 0x1000, 500),
        at(PropKind::CityRowhouses, 850, 3700, 0, 500),
        at(PropKind::CityWall, 1100, 3600, 0, 0),
        at(PropKind::CityWallTower, 1180, 3600, 0, 200),
        // In front of the gunner's tank (`GUNNER`).
        at(PropKind::CityHouse, 380, 1000, 0, 0),
    ];
    for i in 0..8 {
        props.push(at(
            PropKind::CityApartments,
            500 + 130 * i,
            2048,
            0,
            100 * i as u16,
        ));
    }
    props
}

/// What each side fields, and where: (unit, count, x, distance from home).
const ARMY: &[(&str, u16, i32, i32)] = &[
    ("aster_t1_tank", 12, 700, 600),
    ("aster_t2_tank", 6, 900, 650),
    ("aster_t1_aa", 4, 800, 500),
    ("aster_t1_frigate", 4, 2600, 400),
    ("aster_t1_attack_boat", 6, 3000, 450),
    ("aster_t1_submarine", 3, 3300, 350),
    ("aster_t1_sonar", 1, 2800, 250),
    // Torpedo launchers: a float firing attack torpedoes at the submarines, and a seabed
    // installation that does too and meets their torpedoes with interceptors
    // (`naval_arms.rs`).
    ("aster_t1_torpedo_defense", 1, 3000, 330),
    ("aster_t3_torpedo_defense", 1, 3400, 250),
    ("aster_t1_rotor_gunship", 4, 1200, 300),
    ("aster_t1_interceptor", 4, 1400, 250),
    ("aster_t2_torpedo_bomber", 2, 1800, 200),
    // Reclaimers: hover salvagers, clearing wrecks on the attack-move.
    ("aster_t2_mobile_reclaimer", 2, 850, 450),
    // The Regency battle scorpion: a held beam that runs up (`spin`), claws whose charges
    // curve onto their marks (`curve.rs`).
    ("regency_t4_scorpion", 1, 1100, 700),
    // The Regency assault tripod: twin charging cannons taking turns (`twins.rs`), and
    // vertical-launch seekers riding its turning head.
    ("regency_t4_strider", 1, 1300, 750),
    // The Regency tech 3 fusion guns: a Pinch-fusion Howitzer's charged high lob, and a gun
    // that shoots only spacecraft (none here: it must hold through the whole battle).
    ("regency_t3_artillery", 1, 600, 350),
    ("regency_t3_mobile_aa", 1, 1500, 300),
    // Regency boats, and a heavy destroyer with a hull shield and torpedoes. Its
    // assault submarine dives with guns that work only surfaced (`Weapon::surfaced`).
    ("regency_t1_attack_boat", 4, 2800, 500),
    ("regency_t2_destroyer", 1, 3200, 600),
    ("regency_t3_submarine", 1, 3400, 650),
    // The Regency wake tank: every wake rolls out over the fan ahead of it, striking what
    // its front reaches (`wake.rs`), and is often still rolling at the mid-match snapshot.
    ("regency_t3_wake_tank", 1, 1200, 760),
    // Regency bombardment walkers: seekers lobbed high that split into sub-seekers on the
    // way down (`cluster.rs`).
    ("regency_t2_bombard", 2, 1000, 1500),
    // Regency drone carriers: Wicks built in their bays, let go at a mark, diving onto it
    // and bursting (`strike_drones.rs`), sent in over the far side's base; and the beam craft that hangs over its mark,
    // walking its beam across it (`Motion::hangs`, `Weapon::walk`).
    ("regency_t2_drone_carrier", 2, 900, 2900),
    ("regency_t3_assault_aircraft", 1, 1000, 450),
    // Regency strike drones: they circle what they fight near their reach instead of making
    // runs over it (`stand_off.rs`).
    ("regency_t2_strike_drone", 2, 1300, 350),
    // The Regency orb battery: orbs stored in batches, spent one a shot on arcs of random
    // height (`orbs.rs`); the store is mid-way at the snapshot.
    ("regency_t4_area_defense", 1, 1700, 600),
];

fn setup(w: &mut World) {
    let mut spawns = Vec::new();
    for (player, home_y, dir) in [(0u8, 0, 1), (1u8, 4096, -1)] {
        let mut add = |key: &str, count: u16, x: i32, y: i32| {
            spawns.push(PlayerCommand {
                player,
                command: Command::DebugSpawn {
                    owner: player,
                    blueprint: w.blueprints.id_of(key).unwrap(),
                    pos: FxVec2::from_ints(x, home_y + dir * y),
                    heading: Angle::ZERO,
                    count,
                    flags: 0,
                    build: 1000,
                },
            });
        };
        for &(key, count, x, y) in ARMY {
            add(key, count, x, y);
        }
        if player == 0 {
            add("aster_t1_tank", 1, GUNNER.0, GUNNER.1);
            // Tanks out on the deck over the sea, the fleets fighting under it.
            add("aster_t1_tank", 3, 2_400, 2_048);
            add("aster_t3_engineer", 1, REPEAT_LOT.0 + 60, REPEAT_LOT.1);
        }
        // Room for what the Reclaimers bring in.
        add("aster_materials_storage", 1, 500, 150);
        // An Atoll at sea that builds a wing aboard and launches it (`air_wings.rs`).
        add("aster_t3_carrier", 1, 2900, 650);
        // A Roost under the aircraft (`staging.rs`).
        add("aster_t1_airbase", 1, 850, 380);
        match player {
            0 => {
                add("aster_t5_titan", 1, 1000, 900);
                add("aster_t4_nuke_silo", 1, 300, 250);
                // A Courier that warps into the north's dampener (`warp.rs`), on its own power.
                add("aster_t2_lift_ship", 1, 2000, 100);
                add("aster_t3_power", 1, 400, 150);
                // Adjacency (`adjacency.rs`): a fabricator against the reactor, saving its
                // upkeep, and one against a factory, saving its scouts' materials.
                add("aster_t2_fabricator", 1, 460, 150);
                add("aster_t2_fabricator", 1, 260, 450);
                // An Onager against the reactor's west side: its reload is fed from the grid,
                // faster and cheaper for the reactor (`fed_reload.rs`).
                add("aster_t2_artillery", 1, 340, 150);
                // Two factories linked in one batch: their scouts form up by them and join the attack
                // together (`batch.rs`), one of them waiting across the snapshot.
                add("aster_t1_land_factory", 1, 200, 450);
                add("aster_t1_land_factory", 1, 450, 450);
                // Two submarines already in the north's waters, raiding its fleet past its
                // torpedo launchers (the north's grid is paid, so they stand to).
                add("aster_t1_submarine", 2, 3000, 3350);
                // The experimental submarine: strike missiles on a high arc spread over the
                // north's buildings, deck rails that wait for the surface.
                add("aster_t4_submarine", 1, 3600, 500);
                // A space elevator: its cable comes down and is clamped before the
                // snapshot, its first platform docks after it with what was queued
                // (`elevator.rs`).
                add("aster_t5_space_elevator", 1, 250, 1600);
                // The Regency siege walker: a lance that charges and burns a salvo of ticks, twin
                // mounts turning on their own, unguided lofted rockets and a hull field.
                add("regency_t4_colossus", 1, 1500, 900);
                // Its hull field runs on power: a plant for it, or the Courier's warp has none left.
                add("aster_t3_power", 1, 600, 150);
                // A survival Shaper (`lance.rs`): its Lance charges on what comes in reach
                // and strikes; its dome and interceptors are a shield's and an array's.
                add("replication_node", 1, 1600, 1500);
                // The Regent: its Edict gathers along the hull across the snapshot while the
                // ship moves on, every other gun silent, then fires on the empty sea to the
                // east, its wave burning out after (`edict.rs`).
                add(REGENT, 1, EDICT_SHIP.0, EDICT_SHIP.1);
            }
            _ => {
                add("aster_t4_assault_tank", 2, 1000, 900);
                // Two rotary guns on one unit, each with its own spin (`Weapon::rotary`),
                // and pods that set the ground alight.
                add("aster_t4_breacher", 1, 1200, 950);
                // A storm bomber out over the south's army: it lays a trail of seeds that
                // grow and strike down bolts (`cloud_seed.rs`).
                add("aster_t4_storm_bomber", 1, 1000, 3000);
                // A map gun, and a powered radar that finds the south's silo for it.
                add("aster_t4_artillery", 1, 300, 250);
                add("aster_t3_power", 1, 700, 150);
                add("aster_t2_radar", 1, 150, 150);
                add("aster_t3_warp_damper", 1, 1300, 300);
                // An Exarch with its Nanite Repair Field by the north's AA, which start
                // half dead, mending them (`repair_field.rs`).
                add("regency_commander+nano_field", 1, 800, 540);
                // The Regency Seraph: it dashes into the middle of the south's army and lands
                // as a shockwave, its seekers going up as it lands (`dash.rs`).
                add("regency_t5_seraph", 1, 600, 1300);
                // Its upkeep has a plant of its own, or the dampener goes dark.
                add("aster_t3_power", 1, 1000, 150);
                // The Regency gravitic engine: it gathers, its rift runs across the map, and
                // its well holds over the south across the snapshot, crushing, slowing and
                // tearing aircraft down (`gravitic.rs`); a store of energy pays for it.
                // (Clear of the projector past the deck's west end, where nothing stands.)
                add("regency_t5_gravitic_engine", 1, 1000, 1900);
                // The Regency gravitic lens: it plants, its front runs out to one of the
                // south's army and a well holds on it, crushing and slowing (`lens.rs`).
                add("regency_t3_gravitic_lens", 1, 700, 1250);
                // The Regency surge artillery: it plants and lobs a shell into the south's
                // army, whose wake rolls out round the crater and burns (`surge.rs`).
                add("regency_t4_surge_artillery", 1, 300, 600);
                // The Regent's yard: the hull goes up in the sky over it across the
                // snapshot, finishes after it and gets under way (`sky_yard.rs`).
                add(YARD, 1, YARD_AT.0, YARD_AT.1);
            }
        }
    }
    // The Regent off the south's coast, nose north: two of its weapon groups, its ventral
    // lances' held beams and its keel guns' salvos, shell a point at sea ahead to starboard
    // gun by gun (`mounts.rs`), across the snapshot.
    spawns.push(PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner: 0,
            blueprint: w.blueprints.id_of(REGENT).unwrap(),
            pos: REGENT_AT,
            heading: Angle::from_degrees(90),
            count: 1,
            flags: 0,
            build: 1000,
        },
    });
    // A wreck field where the warhead lands: the blast wears it down (`wreck_damage.rs`).
    spawns.push(PlayerCommand {
        player: 0,
        command: Command::DebugWrecks {
            blueprint: w.blueprints.id_of("aster_t2_tank").unwrap(),
            pos: WRECK_FIELD,
            count: 9,
        },
    });
    spawns.push(PlayerCommand {
        player: 1,
        command: Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of("aster_t2_reclaimer").unwrap(),
            pos: FxVec2::from_ints(100, 3600),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    });
    w.tick(&spawns).unwrap();
    // The south's store pays for the Edict's gather.
    let south = &mut w.state.players[0];
    south.bonus_storage[1] += Fx::from_int(1_200_000);
    south.energy += Fx::from_int(1_200_000);
    // The north's store pays the gravitic engine's upkeep, past what its plants make.
    let north = &mut w.state.players[1];
    north.bonus_storage[1] += Fx::from_int(600_000);
    north.energy += Fx::from_int(500_000);
    // And its yard's Regent.
    north.bonus_storage[0] += Fx::from_int(60_000);
    north.materials += Fx::from_int(50_000);
    north.bonus_storage[1] += Fx::from_int(400_000);
    north.energy += Fx::from_int(400_000);
    // The north's AA start hurt, for the Exarch's repair field to mend.
    let aa = w.blueprints.id_of("aster_t1_aa").unwrap();
    for row in w.state.units.slots.iter().collect::<Vec<_>>() {
        if w.state.units.owner[row] == 1 && w.state.units.blueprint[row] == aa {
            w.state.units.health[row] = w.unit_max_health(row) / 2;
        }
    }
}

/// The Regent, and where its Edict strikes: the open sea on the east edge, the fleets
/// clear of it when it fires.
const REGENT: &str = "regency_t5_super_destroyer";
const EDICT_MARK: (i32, i32) = (4050, 1400);
/// When it is ordered: its 15 s gather spans the snapshot.
const EDICT_AT: u32 = 110;

fn edict_regent(w: &World) -> Vec<UnitId> {
    let bp = w.blueprints.id_of(REGENT).unwrap();
    let u = &w.state.units;
    u.slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp && u.owner[r] == 0 && !gunnery_regent(u.pos[r]))
        .map(|r| u.id(r))
        .collect()
}

/// Whether a Regent of the south's at `pos` is the one shelling `REGENT_MARK` (the other
/// gathers the Edict, far from it, so their guns' shots can be told apart).
fn gunnery_regent(pos: FxVec2) -> bool {
    (pos - REGENT_AT).length_sq() < Fx::from_int(EDICT_APART) * Fx::from_int(EDICT_APART)
}

/// Where the Edict's Regent starts (south's frame); how far from the gunnery Regent a
/// Regent counts as the other one; and how far from its middle its own guns' shots leave
/// (its farthest muzzle is ~1.7 km out; the gunnery Regent's are over 2 km away).
const EDICT_SHIP: (i32, i32) = (300, 3000);
const EDICT_APART: i32 = 2000;
const EDICT_GUNS: i32 = 1850;

/// A tank of the south's that, from tick 3, shoots at the ground behind a house.
const GUNNER: (i32, i32) = (300, 1000);

/// Where the south's engineer raises a Fulgur on Repeat (`site_repeat.rs`).
const REPEAT_LOT: (i32, i32) = (180, 800);
const FULGUR: &str = "aster_t4_assault_tank";

/// The Regent, where it hangs, and the point at sea ahead to starboard it shells, holding
/// its fire for anything else (its groups' guns that take the ground and bear on it).
const REGENT_AT: FxVec2 = FxVec2::from_ints(3600, 1700);
const REGENT_MARK: FxVec2 = FxVec2::from_ints(3950, 2800);

/// The Regent's orders, after the tick-1 attack-move: hold fire, and shell `REGENT_MARK`.
fn regent_on(w: &World) -> Vec<PlayerCommand> {
    let bp = w.blueprints.id_of(REGENT).unwrap();
    let u = &w.state.units;
    let units: Vec<UnitId> = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp && u.owner[r] == 0 && gunnery_regent(u.pos[r]))
        .map(|r| u.id(r))
        .collect();
    // The Edict's Regent holds its fire too: its guns would otherwise take the north's
    // base apart before the Edict is ordered.
    let mut holding = units.clone();
    holding.extend(edict_regent(w));
    vec![
        PlayerCommand {
            player: 0,
            command: Command::SetFireState {
                units: holding,
                state: mc_sim::FireState::HoldFire,
            },
        },
        PlayerCommand {
            player: 0,
            command: Command::AttackGround {
                units,
                pos: REGENT_MARK,
                queue: false,
            },
        },
    ]
}

/// The north's Regent yard: x, and distance from home.
const YARD: &str = "regency_t5_sky_yard";
const YARD_AT: (i32, i32) = (350, 1700);
/// When the Regent over it is all but finished: it finishes after the snapshot.
const REGENT_NEARLY: u32 = 150;

/// The Regent of the north's, raised over its yard.
fn north_regent(w: &World) -> Option<UnitId> {
    let bp = w.blueprints.id_of(REGENT).unwrap();
    let u = &w.state.units;
    u.slots
        .iter()
        .find(|&r| u.blueprint[r] == bp && u.owner[r] == 1)
        .map(|r| u.id(r))
}

/// Where the warhead lands, and a field of wrecks lies.
const WRECK_FIELD: FxVec2 = FxVec2::from_ints(900, 3300);

fn units_of(w: &World, player: u8) -> Vec<UnitId> {
    let u = &w.state.units;
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == player)
        .map(|r| u.id(r))
        .collect()
}

/// The south's factories, their standing orders the tick-1 attack-move: linked in one
/// batch of five, three scouts a lap each, repeating, one paid first and one last.
fn batch_on(w: &World) -> Vec<PlayerCommand> {
    let factories = factories(w);
    [
        // The materials storage's worth in hand to build them with.
        Command::DebugStock {
            player: 0,
            materials: Some(1000),
            energy: None,
        },
        Command::SetBatch {
            factories: factories.clone(),
            batch: true,
        },
        // The two linked in one batch that leaves at five, short of their laps' six.
        Command::SetBatchSize {
            factories: factories.clone(),
            size: 5,
        },
        Command::SetRepeat {
            factories: factories.clone(),
            repeat: true,
        },
        // One paid first in a stall, the other last (`focus.rs`): held per unit, and restored.
        Command::SetPriority {
            units: factories[..1].to_vec(),
            priority: Priority::First,
        },
        Command::SetPriority {
            units: factories[1..].to_vec(),
            priority: Priority::Last,
        },
        Command::Produce {
            factories,
            blueprint: w.blueprints.id_of("aster_t1_scout").unwrap(),
            count: 3,
        },
    ]
    .into_iter()
    .map(|command| PlayerCommand { player: 0, command })
    .collect()
}

/// The south's elevator: tanks, a Fulgur and fighters queued for its first platform,
/// more than fits on it.
fn elevator_on(w: &World) -> Vec<PlayerCommand> {
    let bp = w.blueprints.id_of("aster_t5_space_elevator").unwrap();
    let u = &w.state.units;
    let factories: Vec<UnitId> = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp)
        .map(|r| u.id(r))
        .collect();
    [
        ("aster_t4_assault_tank", 2),
        ("aster_t1_tank", 20),
        ("aster_t1_interceptor", 6),
        ("aster_t4_assault_tank", 4),
    ]
    .into_iter()
    .map(|(key, count)| PlayerCommand {
        player: 0,
        command: Command::Produce {
            factories: factories.clone(),
            blueprint: w.blueprints.id_of(key).unwrap(),
            count,
        },
    })
    .collect()
}

/// The north's yard starts a Regent.
fn yard_on(w: &World) -> Vec<PlayerCommand> {
    let bp = w.blueprints.id_of(YARD).unwrap();
    let u = &w.state.units;
    let factories: Vec<UnitId> = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp)
        .map(|r| u.id(r))
        .collect();
    vec![PlayerCommand {
        player: 1,
        command: Command::Produce {
            factories,
            blueprint: w.blueprints.id_of("regency_t5_super_destroyer").unwrap(),
            count: 1,
        },
    }]
}

/// The south's two factories.
fn factories(w: &World) -> Vec<UnitId> {
    let bp = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let u = &w.state.units;
    u.slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp)
        .map(|r| u.id(r))
        .collect()
}

/// The south's Fulgur site while it is not yet on Repeat.
fn unrepeated_fulgur(w: &World) -> Option<UnitId> {
    let bp = w.blueprints.id_of(FULGUR).unwrap();
    let u = &w.state.units;
    u.slots
        .iter()
        .find(|&r| {
            u.owner[r] == 0
                && u.blueprint[r] == bp
                && u.has_flag(r, flag::UNDER_CONSTRUCTION)
                && !u.has_flag(r, flag::REPEAT)
        })
        .map(|r| u.id(r))
}

/// The commands given on `tick`, the same in every run.
fn script(w: &mut World, tick: u32) -> Vec<PlayerCommand> {
    match tick {
        // The south's engineer lays a Fulgur's lot; once the site stands it is put on
        // Repeat and all but finished, so it drives off and the lot is laid again.
        7 => {
            let engineer = w.blueprints.id_of("aster_t3_engineer").unwrap();
            let fulgur = w.blueprints.id_of(FULGUR).unwrap();
            let u = &w.state.units;
            let units = u
                .slots
                .iter()
                .filter(|&r| u.owner[r] == 0 && u.blueprint[r] == engineer)
                .map(|r| u.id(r))
                .collect();
            vec![PlayerCommand {
                player: 0,
                command: Command::Build {
                    units,
                    blueprint: fulgur,
                    pos: FxVec2::from_ints(REPEAT_LOT.0, REPEAT_LOT.1),
                    heading: w.blueprints.unit(fulgur).build_heading(),
                    queue: false,
                },
            }]
        }
        t if t > 7 && t < SNAPSHOT_AT && unrepeated_fulgur(w).is_some() => {
            let site = vec![unrepeated_fulgur(w).unwrap()];
            [
                Command::SetRepeat {
                    factories: site.clone(),
                    repeat: true,
                },
                Command::DebugSetBuild {
                    units: site,
                    permille: 997,
                },
            ]
            .into_iter()
            .map(|command| PlayerCommand { player: 0, command })
            .collect()
        }
        1 => [(0u8, 3600), (1u8, 500)]
            .into_iter()
            .map(|(player, y)| PlayerCommand {
                player,
                command: Command::AttackMove {
                    units: units_of(w, player),
                    target: FxVec2::from_ints(1600, y),
                    queue: false,
                },
            })
            .chain(batch_on(w))
            .chain(elevator_on(w))
            .chain(regent_on(w))
            .chain(yard_on(w))
            // The economy's focus (`focus.rs`): held in each side's state, and restored.
            .chain([PlayerCommand {
                player: 1,
                command: Command::SetFocus {
                    focus: Focus {
                        mines: Priority::Last,
                        power: Priority::First,
                    },
                },
            }])
            .collect(),
        // The north's Reclaimers clear the wreck field in a circle (`reclaim_area.rs`).
        2 => {
            let reclaimer = w.blueprints.id_of("aster_t2_mobile_reclaimer").unwrap();
            let u = &w.state.units;
            let units = u
                .slots
                .iter()
                .filter(|&r| u.owner[r] == 1 && u.blueprint[r] == reclaimer)
                .map(|r| u.id(r))
                .collect();
            vec![PlayerCommand {
                player: 1,
                command: Command::ReclaimArea {
                    units,
                    pos: WRECK_FIELD,
                    radius: Fx::from_int(120),
                    queue: false,
                },
            }]
        }
        40 => {
            let silo_bp = w.blueprints.id_of("aster_t4_nuke_silo").unwrap();
            let u = &w.state.units;
            let silo = u
                .slots
                .iter()
                .find(|&r| u.blueprint[r] == silo_bp)
                .map(|r| u.id(r))
                .unwrap();
            w.state.strategic.launchers.entry(silo).or_default().stock = 1;
            vec![PlayerCommand {
                player: 0,
                command: Command::LaunchNuke {
                    units: vec![silo],
                    pos: WRECK_FIELD,
                },
            }]
        }
        // Charged on a strained grid by tick ~150, dragged through the dampener's field, and stunned across
        // the snapshot at `SNAPSHOT_AT`.
        60 => {
            let courier = w.blueprints.id_of("aster_t2_lift_ship").unwrap();
            let u = &w.state.units;
            let ship = u
                .slots
                .iter()
                .find(|&r| u.blueprint[r] == courier)
                .map(|r| u.id(r))
                .unwrap();
            vec![PlayerCommand {
                player: 0,
                command: Command::Warp {
                    units: vec![ship],
                    pos: FxVec2::from_ints(2000, 3700),
                    queue: false,
                },
            }]
        }
        // The gunner fires on the ground beyond its house: its shells meet the house.
        3 => {
            let u = &w.state.units;
            let spot = FxVec2::from_ints(GUNNER.0, GUNNER.1);
            let gunner = u
                .slots
                .iter()
                .find(|&r| u.owner[r] == 0 && u.pos[r].distance(spot) < Fx::from_int(30))
                .map(|r| u.id(r))
                .unwrap();
            vec![PlayerCommand {
                player: 0,
                command: Command::AttackGround {
                    units: vec![gunner],
                    pos: FxVec2::from_ints(460, 1000),
                    queue: false,
                },
            }]
        }
        4 => {
            let engine = w.blueprints.id_of("regency_t5_gravitic_engine").unwrap();
            let u = &w.state.units;
            let units = u
                .slots
                .iter()
                .filter(|&r| u.blueprint[r] == engine)
                .map(|r| u.id(r))
                .collect();
            vec![PlayerCommand {
                player: 1,
                command: Command::Fracture {
                    units,
                    pos: FxVec2::from_ints(1450, 1500),
                },
            }]
        }
        // Each side's Atoll builds a torpedo wing, holding its own fire so its missiles
        // leave the rest of the match be.
        6 => (0u8..2)
            .flat_map(|player| {
                let atoll = w.blueprints.id_of("aster_t3_carrier").unwrap();
                let u = &w.state.units;
                let units = u
                    .slots
                    .iter()
                    .filter(|&r| u.owner[r] == player && u.blueprint[r] == atoll)
                    .map(|r| u.id(r))
                    .collect();
                let units: Vec<UnitId> = units;
                [
                    PlayerCommand {
                        player,
                        command: Command::SetFireState {
                            units: units.clone(),
                            state: mc_sim::FireState::HoldFire,
                        },
                    },
                    PlayerCommand {
                        player,
                        command: Command::QueueWing {
                            units,
                            wing: 2,
                            count: 1,
                        },
                    },
                ]
            })
            .collect(),
        // The south's gunships go down on their Roost (`staging.rs`), and are fired back
        // out of its tunnels.
        5 => {
            let (gunship, roost) = (
                w.blueprints.id_of("aster_t1_rotor_gunship").unwrap(),
                w.blueprints.id_of("aster_t1_airbase").unwrap(),
            );
            let u = &w.state.units;
            let of = |bp| {
                u.slots
                    .iter()
                    .filter(move |&r| u.owner[r] == 0 && u.blueprint[r] == bp)
            };
            let base = of(roost).map(|r| u.id(r)).next().unwrap();
            vec![PlayerCommand {
                player: 0,
                command: Command::Stage {
                    units: of(gunship).map(|r| u.id(r)).collect(),
                    base,
                    queue: false,
                },
            }]
        }
        // The Regent all but finished, with a few hundred ticks of work left on it.
        REGENT_NEARLY => vec![PlayerCommand {
            player: 1,
            command: Command::DebugSetBuild {
                units: north_regent(w).into_iter().collect(),
                permille: 975,
            },
        }],
        // The Regent's Edict, then a move: the hull, and its firing point, go on moving
        // while it gathers.
        EDICT_AT => vec![PlayerCommand {
            player: 0,
            command: Command::Edict {
                units: edict_regent(w),
                pos: FxVec2::from_ints(EDICT_MARK.0, EDICT_MARK.1),
                queue: false,
            },
        }],
        t if t == EDICT_AT + 1 => vec![PlayerCommand {
            player: 0,
            command: Command::Move {
                units: edict_regent(w),
                target: FxVec2::from_ints(EDICT_SHIP.0 - 200, EDICT_SHIP.1 - 200),
                queue: false,
            },
        }],
        // A timed self-destruct still counting down when the snapshot is taken.
        t if t == SNAPSHOT_AT - 20 => vec![PlayerCommand {
            player: 1,
            command: Command::SelfDestruct {
                units: units_of(w, 1).into_iter().take(1).collect(),
                timed: true,
            },
        }],
        _ => Vec::new(),
    }
}

/// The state hash after every tick, from `from` up to `TICKS`.
fn play(w: &mut World, from: u32) -> Vec<u64> {
    (from..TICKS)
        .map(|t| {
            let commands = script(w, t);
            w.tick(&commands).unwrap()
        })
        .collect()
}

fn reference() -> Vec<u64> {
    let mut w = world(0);
    setup(&mut w);
    let mut intercepted = 0;
    let mut struck = 0;
    let mut split = 0;
    let titan_sub = w.blueprints.id_of("aster_t4_submarine").unwrap();
    let quiver = w.blueprints.id_of("regency_t2_drone_carrier").unwrap();
    let mut burst = 0;
    let fid = factories(&w)[0];
    let (mut last_held, mut batches_sent, mut held_at_snapshot) = (0, 0, 0);
    let courier = w.blueprints.id_of("aster_t2_lift_ship").unwrap();
    let mut stunned_at_snapshot = false;
    let mut well_at_snapshot = false;
    let (mut on_blocks, mut alight, mut down) = (0, 0, 0);
    let mut platforms = 0;
    let mut dashes = 0;
    let mut bolts = 0;
    let mut staged = false;
    let mut wing_out = false;
    let mut lens_strikes = 0;
    let mut surging = 0;
    let mut lanced = 0;
    let regent_bp = w.blueprints.id_of(REGENT).unwrap();
    let (mut gathering_at_snapshot, mut edicts_fired, mut wave_out) = (false, 0, false);
    let mut shots_in_gather = 0;
    let fulgur = w.blueprints.id_of(FULGUR).unwrap();
    let mut relaid = false;
    let regent = w.blueprints.id_of(REGENT).unwrap();
    let mut regent_guns = std::collections::BTreeSet::new();
    let mut regent_weapons = std::collections::BTreeSet::new();
    let mut regent_guns_at_snapshot = 0;
    let mut regent_shots_after = 0;
    let (mut aloft_at_snapshot, mut regent_done) = (false, false);
    let hashes = (1..TICKS)
        .map(|t| {
            let commands = script(&mut w, t);
            let hash = w.tick(&commands).unwrap();
            struck += w
                .events
                .iter()
                .filter(|e| {
                    matches!(e, mc_sim::SimEvent::ShotFired { blueprint, weapon: 1, .. }
                        if *blueprint == titan_sub)
                })
                .count();
            let held = w
                .state
                .batches
                .values()
                .find(|b| b.factories.contains(&fid))
                .map_or(0, |b| b.held.len());
            // Four waiting, then fewer: the fifth came out and the batch left (the other
            // factory's scout out that same tick may already wait in the next).
            if last_held >= 4 && held < last_held {
                batches_sent += 1;
            }
            last_held = held;
            let u = &w.state.units;
            staged |= u
                .slots
                .iter()
                .any(|r| u.hangar[r] != mc_sim::tables::Handle::NONE && u.blueprint[r] != courier);
            wing_out |= w.state.wings.bays.values().any(|b| !b.launching.is_empty());
            if let Some(r) = north_regent(&w).and_then(|id| w.state.units.row(id)) {
                let u = &w.state.units;
                regent_done |= !u.has_flag(r, flag::UNDER_CONSTRUCTION)
                    && w.state.orders.front(u, r).is_some();
                if t == SNAPSHOT_AT {
                    aloft_at_snapshot = u.has_flag(r, flag::UNDER_CONSTRUCTION)
                        && !u.has_flag(r, flag::IN_FACTORY)
                        && u.z[r] > Fx::from_int(1000);
                }
            }
            if t == SNAPSHOT_AT {
                held_at_snapshot = held;
                let u = &w.state.units;
                stunned_at_snapshot = u
                    .slots
                    .iter()
                    .any(|r| u.blueprint[r] == courier && u.stun[r][0] > 0);
                well_at_snapshot = w
                    .state
                    .wells
                    .iter()
                    .any(|x| matches!(x.phase, mc_sim::gravitic::WellPhase::Holding { .. }));
                gathering_at_snapshot = w
                    .state
                    .edicts
                    .iter()
                    .any(|e| matches!(e.phase, EdictPhase::Gathering { .. }));
            }
            let gathering = w
                .state
                .edicts
                .iter()
                .any(|e| matches!(e.phase, EdictPhase::Gathering { .. }));
            if gathering {
                // The Edict's own ship: its guns' shots leave within its reach of its middle.
                let near = |at: mc_core::FxVec3| {
                    edict_regent(&w).iter().filter_map(|&id| w.state.units.row(id)).any(|r| {
                        (at.xy() - w.state.units.pos[r]).length_sq()
                            < Fx::from_int(EDICT_GUNS) * Fx::from_int(EDICT_GUNS)
                    })
                };
                shots_in_gather += w
                    .events
                    .iter()
                    .filter(|e| {
                        matches!(e, mc_sim::SimEvent::ShotFired { blueprint, pos, .. }
                            if *blueprint == regent_bp && near(*pos))
                    })
                    .count();
            }
            let roll = w.blueprints.unit(regent_bp).edict.as_ref().unwrap().wave.roll_ticks;
            wave_out |= w
                .state
                .edicts
                .iter()
                .any(|e| matches!(e.phase, EdictPhase::Fired { age } if age > roll));
            intercepted += w
                .events
                .iter()
                .filter(|e| matches!(e, mc_sim::SimEvent::TorpedoIntercepted { .. }))
                .count();
            burst += w
                .events
                .iter()
                .filter(|e| matches!(e, mc_sim::SimEvent::Impact { blueprint, .. } if *blueprint == quiver))
                .count();
            for e in &w.events {
                match e {
                    mc_sim::SimEvent::Impact {
                        on_structure: Some(_),
                        ..
                    } => on_blocks += 1,
                    mc_sim::SimEvent::StructureAlight { .. } => alight += 1,
                    mc_sim::SimEvent::StructureCollapsed { .. } => down += 1,
                    mc_sim::SimEvent::ElevatorDocked { units, .. } if *units > 0 => {
                        platforms += 1
                    }
                    mc_sim::SimEvent::DashLanded { .. } => dashes += 1,
                    mc_sim::SimEvent::CloudBolt { .. } => bolts += 1,
                    mc_sim::SimEvent::LensStruck { .. } => lens_strikes += 1,
                    mc_sim::SimEvent::LanceStrike { .. } => lanced += 1,
                    mc_sim::SimEvent::EdictFired { .. } => edicts_fired += 1,
                    _ => {}
                }
            }
            surging += usize::from(!w.state.surges.is_empty());
            for e in &w.events {
                if let mc_sim::SimEvent::ShotFired {
                    blueprint,
                    weapon,
                    gun,
                    ..
                } = e
                {
                    if *blueprint == regent {
                        regent_guns.insert(*gun);
                        regent_weapons.insert(*weapon);
                        regent_shots_after += usize::from(t > SNAPSHOT_AT);
                    }
                }
            }
            if t == SNAPSHOT_AT {
                regent_guns_at_snapshot = regent_guns.len();
            }
            // A finished Fulgur of the south's, and its lot laid again on Repeat.
            let u = &w.state.units;
            let of_south = |r: usize| u.owner[r] == 0 && u.blueprint[r] == fulgur;
            relaid |= u
                .slots
                .iter()
                .any(|r| of_south(r) && !u.has_flag(r, flag::UNDER_CONSTRUCTION))
                && u.slots.iter().any(|r| {
                    of_south(r)
                        && u.has_flag(r, flag::UNDER_CONSTRUCTION)
                        && u.has_flag(r, flag::REPEAT)
                });
            split += w
                .events
                .iter()
                .filter(|e| matches!(e, mc_sim::SimEvent::ClusterSplit { .. }))
                .count();
            hash
        })
        .collect();
    eprintln!("determinism: city {on_blocks} shots on blocks, {alight} alight, {down} down");
    // The Regent went up aloft over its yard across the snapshot, and got under way.
    assert!(
        aloft_at_snapshot,
        "no Regent was going up aloft at the snapshot"
    );
    assert!(regent_done, "the Regent was never finished and sent off");
    // The gravitic engine's well struck before the snapshot and holds across it.
    assert!(well_at_snapshot, "no gravity well held at the snapshot");
    // The Regent's Edict gathered across the snapshot with its guns silent, fired, and its
    // wave rolled out to its full reach.
    assert!(
        gathering_at_snapshot,
        "the Edict was not gathering at the snapshot"
    );
    assert_eq!(edicts_fired, 1, "the Edict did not fire once");
    assert!(wave_out, "the Edict's wave never rolled out");
    assert_eq!(
        shots_in_gather, 0,
        "the Regent's guns fired while it gathered"
    );
    // The storm bomber's clouds struck down.
    assert!(bolts > 0, "no storm cloud let a bolt fall");
    // A carrier's wing came aboard and went off its deck.
    assert!(wing_out, "no air wing was built");
    // The gunships went down on their Roost.
    assert!(staged, "no aircraft went down on a staging base");
    // The Seraph dashed into the south's army.
    assert!(dashes > 0, "the Seraph never dashed");
    // The lens's front struck and its well opened.
    assert!(lens_strikes > 0, "the gravitic lens never struck");
    // The Fulgur raised on Repeat drove off and its lot was laid again.
    assert!(relaid, "no lot was laid again on Repeat");
    // The Shaper's Lance struck.
    assert!(lanced > 0, "the Lance never struck");
    // The Regent's groups fired gun by gun on both sides of the snapshot.
    assert!(
        regent_weapons.len() >= 2 && regent_guns_at_snapshot >= 8,
        "the Regent's groups: weapons {regent_weapons:?}, {regent_guns_at_snapshot} guns by the snapshot"
    );
    assert!(
        regent_shots_after > 0,
        "the Regent's guns never fired after the snapshot: {regent_guns:?}"
    );
    // A surge shell landed and its wake rolled out.
    assert!(surging > 0, "no surge shell's wake rolled out");
    // The elevator's first platform brought its queue down after the snapshot.
    assert!(platforms > 0, "no elevator platform docked with units");
    // City blocks took shots, caught fire and came down.
    assert!(on_blocks > 0, "no shot met a city block");
    assert!(alight > 0, "no city block caught fire");
    assert!(down > 0, "no city block came down");
    assert!(
        !w.is_prop_alive(0),
        "the authored billboard survived the warhead"
    );
    let salvage = w.state.city.salvage.last().unwrap();
    assert!(
        *salvage < Fx::from_int(10_000),
        "authored salvage was never collected"
    );
    // The factory's scouts formed up, waited across the snapshot and left together.
    assert!(held_at_snapshot > 0, "no batch was waiting at the snapshot");
    assert!(batches_sent > 0, "no batch filled and left");
    // The Courier charged its jump (priced by its distance), was dragged and stunned.
    assert!(
        stunned_at_snapshot,
        "the Courier was not stunned across the snapshot"
    );
    // Interceptors met torpedoes: the Atoll's, as the fleets close late in the match (the
    // Atoll keeps behind its fleet's firing line, `fight_line.rs`, so it is late to them).
    assert!(intercepted > 0, "no torpedo was intercepted in the match");
    // The experimental submarine's strike missiles went up on their high arc.
    assert!(struck > 0, "no strike missile was launched in the match");
    // The bombardment walkers' seekers broke into their sub-seekers.
    assert!(split > 0, "no cluster shot split in the match");
    // The drone carriers' Wicks went off on their marks.
    assert!(burst > 0, "no Wick burst in the match");
    // The Reclaimers' heads were at work in it, on the move.
    let reclaimed = w.state.players.iter().map(|p| p.reclaimed_materials).max();
    assert!(
        reclaimed.is_some_and(|m| m > mc_core::Fx::ZERO),
        "nothing was reclaimed in the match"
    );
    // And the warhead wore down every wreck of the field it landed on.
    let wr = &w.state.wrecks;
    let intact = wr.slots.iter().filter(|&r| {
        wr.pos[r].distance(WRECK_FIELD) < Fx::from_int(60) && wr.materials[r] >= wr.materials_max[r]
    });
    assert_eq!(intact.count(), 0, "the blast left wrecks untouched");
    hashes
}

#[test]
fn every_domain_hashes_the_same_at_any_worker_count() {
    let reference = reference();
    for threads in [1, 3, 8] {
        let mut w = world(threads);
        setup(&mut w);
        let hashes = play(&mut w, 1);
        let first = reference.iter().zip(&hashes).position(|(a, b)| a != b);
        assert_eq!(
            first, None,
            "desync with {threads} workers at tick {first:?}"
        );
    }
    eprintln!(
        "determinism: every_domain final {:016x}",
        reference.last().unwrap()
    );
}

#[test]
fn every_domain_restores_to_the_same_future() {
    let reference = reference();
    let mut a = world(2);
    setup(&mut a);
    for t in 1..SNAPSHOT_AT {
        let commands = script(&mut a, t);
        a.tick(&commands).unwrap();
    }
    let blob = a.snapshot();
    let mut b = world(3);
    b.restore(terrain(), &blob).unwrap();
    assert_eq!(a.hash(), b.hash(), "the restored state hashes differently");
    let after = play(&mut b, SNAPSHOT_AT);
    let tail = &reference[(SNAPSHOT_AT - 1) as usize..];
    let first = tail.iter().zip(&after).position(|(a, b)| a != b);
    assert_eq!(first, None, "diverged {first:?} ticks after the snapshot");
}
