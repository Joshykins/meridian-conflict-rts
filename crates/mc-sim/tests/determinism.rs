//! The determinism matrix: one match with every domain in it (land, sea, under
//! the sea, air, a titan, a nuclear strike, a map gun, a battle scorpion's held beam and
//! curving charges, a wake tank's cone, a warp into a dampener and the stun it leaves, wrecks worn down by
//! blasts, two factories' linked batch forming up, a self-destruct counting down across
//! the snapshot) must hash identically at every worker count and after a snapshot is
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
    // Torpedo launchers: a float firing attack torpedoes at the submarines, and a seabed
    // installation that does too and meets their torpedoes with interceptors
    // (`naval_arms.rs`).
    ("aster_t1_torpedo_defense", 1, 3000, 330),
    ("aster_t3_torpedo_defense", 1, 3400, 250),
    ("aster_t1_rotor_gunship", 4, 1200, 300),
    ("aster_t1_interceptor", 4, 1400, 250),
    ("aster_t2_torpedo_bomber", 2, 1800, 200),
    // Salvage carriers: three reclaim heads each, clearing wrecks on the attack-move.
    ("aster_t2_land_reclaimer", 2, 850, 450),
    // The Regency battle scorpion: a held beam that runs up (`spin`), claws whose charges
    // curve onto their marks (`curve.rs`).
    ("regency_t4_scorpion", 1, 1100, 700),
    // The Regency tech 3 fusion guns: a Pinch-fusion Howitzer's charged high lob, and a gun
    // that shoots only spacecraft (none here: it must hold through the whole battle).
    ("regency_t3_artillery", 1, 600, 350),
    ("regency_t3_mobile_aa", 1, 1500, 300),
    // Regency boats lying dived in ambush (`Dive::ambush`): they surface for a mark and
    // go back down without one. A diving heavy destroyer with a hull shield and torpedoes
    // dived, guns only up.
    ("regency_t1_attack_boat", 4, 2800, 500),
    ("regency_t2_destroyer", 1, 3200, 600),
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
                add("aster_t2_lift_ship", 1, 2000, 100);
                add("aster_t3_power", 1, 400, 150);
                // Adjacency (`adjacency.rs`): a fabricator against the reactor, saving its
                // upkeep, and one against a factory, saving its scouts' materials.
                add("aster_t2_fabricator", 1, 460, 150);
                add("aster_t2_fabricator", 1, 260, 450);
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

/// The south's factories, their standing orders the tick-1 attack-move: linked in one
/// batch of five, three scouts a lap each, repeating.
fn batch_on(w: &World) -> Vec<PlayerCommand> {
    let factories = factories(w);
    [
        // The mass storage's worth in hand to build them with.
        Command::DebugStock {
            player: 0,
            mass: Some(1000),
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
            .chain(batch_on(w))
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
        // The north's salvage carriers clear the wreck field in a circle (`reclaim_area.rs`).
        2 => {
            let carrier = w.blueprints.id_of("aster_t2_land_reclaimer").unwrap();
            let u = &w.state.units;
            let units = u
                .slots
                .iter()
                .filter(|&r| u.owner[r] == 1 && u.blueprint[r] == carrier)
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
            if t == SNAPSHOT_AT {
                held_at_snapshot = held;
                let u = &w.state.units;
                stunned_at_snapshot = u
                    .slots
                    .iter()
                    .any(|r| u.blueprint[r] == courier && u.stun[r][0] > 0);
            }
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
            split += w
                .events
                .iter()
                .filter(|e| matches!(e, mc_sim::SimEvent::ClusterSplit { .. }))
                .count();
            hash
        })
        .collect();
    // The factory's scouts formed up, waited across the snapshot and left together.
    assert!(held_at_snapshot > 0, "no batch was waiting at the snapshot");
    assert!(batches_sent > 0, "no batch filled and left");
    // The Courier charged its jump (priced by its distance), was dragged and stunned.
    assert!(
        stunned_at_snapshot,
        "the Courier was not stunned across the snapshot"
    );
    // The seabed installation's interceptors met the submarines' torpedoes.
    assert!(intercepted > 0, "no torpedo was intercepted in the match");
    // The experimental submarine's strike missiles went up on their high arc.
    assert!(struck > 0, "no strike missile was launched in the match");
    // The bombardment walkers' seekers broke into their sub-seekers.
    assert!(split > 0, "no cluster shot split in the match");
    // The drone carriers' Wicks went off on their marks.
    assert!(burst > 0, "no Wick burst in the match");
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
