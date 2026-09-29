//! The determinism matrix: one match with every domain in it (land, sea, under
//! the sea, air, a titan, a nuclear strike, a map gun, a battle scorpion's held beam and
//! curving charges, a warp into a dampener and the stun it leaves, wrecks worn down by
//! blasts) must hash identically at every worker count and after a snapshot is
//! restored mid-match.
//!
//! `battle.rs` covers a land-only battle the same way; this is the one to extend
//! when a new domain or system arrives (CLAUDE.md, section 3).
//!
//! It also prints the final hash, so the Windows and Linux builds can be compared:
//! `scripts/determinism-cross.sh` runs it on both and diffs the lines.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::focus::{Focus, Priority};
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// 4 km square: land in the west, sea (20 m deep) in the east.
const CELLS: u32 = 512;
const SHORE_CELL: usize = 200;
const TICKS: u32 = 500;
const SNAPSHOT_AT: u32 = 250;

fn terrain() -> Heightfield {
    let side = CELLS as usize + 1;
    let mut samples = vec![0u16; side * side];
    for y in 0..side {
        for x in 0..SHORE_CELL {
            samples[y * side + x] = 40;
        }
    }
    Heightfield::from_samples(CELLS, CELLS, samples, Fx::ZERO, Fx::ONE, Fx::from_int(20))
}

fn world(threads: usize) -> World {
    let blueprints =
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let map = MapData {
        name: "determinism".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(800, 500), FxVec2::from_ints(800, 3600)],
        props: Vec::new(),
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
    World::with_terrain(
        terrain(),
        map,
        Arc::new(blueprints),
        Arc::new(Pool::new(threads)),
        &config,
    )
    .unwrap()
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
    ("aster_t1_rotor_gunship", 4, 1200, 300),
    ("aster_t1_interceptor", 4, 1400, 250),
    ("aster_t2_torpedo_bomber", 2, 1800, 200),
    // Salvage carriers: three reclaim heads each, clearing wrecks on the attack-move.
    ("aster_t2_land_reclaimer", 2, 850, 450),
    // The Naga battle scorpion: a held beam that runs up (`spin`), claws whose charges
    // curve onto their marks (`curve.rs`).
    ("naga_t3_scorpion", 1, 1100, 700),
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
        // Room for what the salvage carriers bring in.
        add("aster_mass_storage", 1, 500, 150);
        match player {
            0 => {
                add("aster_t5_titan", 1, 1000, 900);
                add("aster_t4_nuke_silo", 1, 300, 250);
                // A Courier that warps into the north's dampener (`warp.rs`), on its own power.
                add("aster_t1_lift_ship", 1, 2000, 100);
                add("aster_t3_power", 1, 400, 150);
            }
            _ => {
                add("aster_t4_assault_tank", 2, 1000, 900);
                // A map gun, and a powered radar that finds the south's silo for it.
                add("aster_t4_artillery", 1, 300, 250);
                add("aster_t3_power", 1, 700, 150);
                add("aster_t2_radar", 1, 150, 150);
                add("aster_t2_warp_damper", 1, 1300, 300);
            }
        }
    }
    // A wreck field where the warhead lands: the blast wears it down (`wreck_damage.rs`).
    spawns.push(PlayerCommand {
        player: 0,
        command: Command::DebugWrecks {
            blueprint: w.blueprints.id_of("aster_t2_tank").unwrap(),
            pos: WRECK_FIELD,
            count: 9,
        },
    });
    w.tick(&spawns).unwrap();
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

/// The commands given on `tick`, the same in every run.
fn script(w: &mut World, tick: u32) -> Vec<PlayerCommand> {
    match tick {
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
            let courier = w.blueprints.id_of("aster_t1_lift_ship").unwrap();
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
    let hashes = play(&mut w, 1);
    // The salvage carriers' heads were at work in it, on the move.
    let reclaimed = w.state.players.iter().map(|p| p.reclaimed_mass).max();
    assert!(
        reclaimed.is_some_and(|m| m > mc_core::Fx::ZERO),
        "nothing was reclaimed in the match"
    );
    // And the warhead wore down every wreck of the field it landed on.
    let wr = &w.state.wrecks;
    let intact = wr.slots.iter().filter(|&r| {
        wr.pos[r].distance(WRECK_FIELD) < Fx::from_int(60) && wr.mass[r] >= wr.mass_max[r]
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
