//! Probe: what a Hellkite does to ground targets, beside the same mass of Petrels and
//! Kestrels (and a pair of Hellkites). Passive targets, parked or driving off, 60 s, three
//! seeds: damage, kills, mass killed per own mass, first hit, bombs, time to kill them all. `cargo test --profile gate -p mc-sim --test sim -- zz_hellkite_probe::
//! --ignored --nocapture`

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn world(seed: u64) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(7000, 7000)],
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
        seed,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        Heightfield::flat(1024, 1024, Fx::from_int(20)),
        map,
        blueprints,
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

#[derive(Clone, Copy)]
enum Order {
    /// Attack each target in turn.
    Attack,
    /// Attack-move through the area to the far side.
    Sweep,
}

/// Returns (damage dealt, kills, mass killed, first-hit second, salvos).
fn scene(
    seed: u64,
    bomber: &str,
    n: i32,
    key: &str,
    spots: &[(i32, i32)],
    order: Order,
    walk: Option<(i32, i32)>,
    secs: u32,
) -> (f64, usize, f64, f64, usize, f64) {
    let mut w = world(seed);
    let bombers: Vec<_> = (0..n)
        .map(|i| add(&mut w, bomber, 0, 1800, 3000 + (i - n / 2) * 25))
        .collect();
    let targets: Vec<_> = spots
        .iter()
        .map(|&(x, y)| add(&mut w, key, 1, x, y))
        .collect();
    for &t in &targets {
        w.state.units.flags[t] |= flag::PASSIVE;
    }
    let bp_mass = {
        let id = w.blueprints.id_of(key).unwrap();
        w.blueprints.unit(id).cost_mass.to_f64()
    };
    let bomber_bp = w.blueprints.id_of(bomber).unwrap();
    let tids: Vec<_> = targets.iter().map(|&t| w.state.units.id(t)).collect();
    let max: Vec<f64> = targets
        .iter()
        .map(|&t| w.state.units.health[t].to_f64())
        .collect();
    let ids: Vec<_> = bombers.iter().map(|&r| w.state.units.id(r)).collect();
    let mut cmds: Vec<_> = match order {
        Order::Sweep => vec![PlayerCommand {
            player: 0,
            command: Command::AttackMove {
                units: ids.clone(),
                target: FxVec2::from_ints(spots[0].0 + 600, spots[0].1),
                queue: false,
            },
        }],
        Order::Attack => tids
            .iter()
            .enumerate()
            .map(|(i, &t)| PlayerCommand {
                player: 0,
                command: Command::Attack {
                    units: ids.clone(),
                    target: t,
                    queue: i > 0,
                },
            })
            .collect(),
    };
    if let Some((dx, dy)) = walk {
        cmds.push(PlayerCommand {
            player: 1,
            command: Command::Move {
                units: tids.clone(),
                target: FxVec2::from_ints(spots[0].0 + dx, spots[0].1 + dy),
                queue: false,
            },
        });
    }
    w.tick(&cmds).unwrap();
    let mut first = f64::NAN;
    let mut salvos = 0;
    let mut cleared = f64::NAN;
    let mut hp_last = max.clone();
    for t in 1..=secs * 10 {
        w.tick(&[]).unwrap();
        salvos += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::ShotFired { blueprint, weapon: 0, .. } if *blueprint == bomber_bp))
            .count();
        for (i, &tid) in tids.iter().enumerate() {
            hp_last[i] = match w.state.units.row(tid) {
                Some(r) => w.state.units.health[r].to_f64(),
                None => 0.0,
            };
        }
        if first.is_nan() && hp_last.iter().zip(&max).any(|(h, m)| h < m) {
            first = t as f64 / 10.0;
        }
        if hp_last.iter().all(|&h| h <= 0.0) {
            cleared = t as f64 / 10.0;
            break;
        }
    }
    let dealt: f64 = hp_last.iter().zip(&max).map(|(h, m)| m - h).sum();
    let kills = hp_last.iter().filter(|&&h| h <= 0.0).count();
    (dealt, kills, kills as f64 * bp_mass, first, salvos, cleared)
}

fn grid(n: i32, gap: i32) -> Vec<(i32, i32)> {
    let side = (n as f64).sqrt().ceil() as i32;
    (0..n)
        .map(|i| (3000 + (i % side) * gap, 3000 + (i / side) * gap))
        .collect()
}

fn report(
    label: &str,
    bomber: &str,
    n: i32,
    mass: f64,
    f: impl Fn(u64) -> (f64, usize, f64, f64, usize, f64),
) {
    let runs: Vec<_> = (1..=3).map(f).collect();
    let dealt = runs.iter().map(|r| r.0).sum::<f64>() / 3.0;
    let kills = runs.iter().map(|r| r.1).sum::<usize>() as f64 / 3.0;
    let killed = runs.iter().map(|r| r.2).sum::<f64>() / 3.0;
    let first = runs.iter().map(|r| r.3).sum::<f64>() / 3.0;
    // Seconds to kill every target, averaged; NaN when any run left one alive.
    let cleared = runs.iter().map(|r| r.5).sum::<f64>() / 3.0;
    let salvos = runs.iter().map(|r| r.4).sum::<usize>() as f64 / 3.0;
    println!(
        "{label:<34} {n} x {bomber:<24} dmg {dealt:>7.0}  kills {kills:>4.1}  mass killed {killed:>6.0} ({:>4.2}x own)  first hit {first:>5.1}s  bombs {salvos:>5.1}  all dead {cleared:>5.1}s",
        killed / mass
    );
}

#[test]
#[ignore]
fn zz_hellkite_probe() {
    let fleets: &[(&str, i32, f64)] = &[
        ("aster_t2_fire_bomber", 1, 250.0),
        ("aster_t1_bomber", 4, 240.0),
        ("aster_t2_gunship", 1, 220.0),
        ("aster_t2_fire_bomber", 2, 500.0),
    ];
    type Case = (
        &'static str,
        &'static str,
        Vec<(i32, i32)>,
        Order,
        Option<(i32, i32)>,
        u32,
    );
    let cases: Vec<Case> = vec![
        (
            "1 Warden, 60 s",
            "aster_t1_tank",
            grid(1, 0),
            Order::Attack,
            None,
            60,
        ),
        (
            "1 Bulwark, 60 s",
            "aster_t2_tank",
            grid(1, 0),
            Order::Attack,
            None,
            60,
        ),
        (
            "9 Wardens parked 14 m, 60 s",
            "aster_t1_tank",
            grid(9, 14),
            Order::Sweep,
            None,
            60,
        ),
        (
            "9 Wardens moving north, 60 s",
            "aster_t1_tank",
            grid(9, 14),
            Order::Sweep,
            Some((0, 1500)),
            60,
        ),
        (
            "16 Lancers parked 10 m, 60 s",
            "aster_t1_bot",
            grid(16, 10),
            Order::Sweep,
            None,
            60,
        ),
        (
            "9 Bulwarks parked 16 m, 60 s",
            "aster_t2_tank",
            grid(9, 16),
            Order::Sweep,
            None,
            60,
        ),
        (
            "6 Mason IIs 12 m, 60 s",
            "aster_t2_engineer",
            grid(6, 12),
            Order::Sweep,
            None,
            60,
        ),
        (
            "1 Reactor II, 60 s",
            "aster_t2_power",
            grid(1, 0),
            Order::Attack,
            None,
            60,
        ),
        (
            "9 Reactors 16 m, 60 s",
            "aster_t1_power",
            grid(9, 16),
            Order::Sweep,
            None,
            60,
        ),
        (
            "1 Sentinel PD, 60 s",
            "aster_t1_point_defense",
            grid(1, 0),
            Order::Attack,
            None,
            60,
        ),
    ];
    for (label, key, spots, order, walk, secs) in &cases {
        for &(bomber, n, mass) in fleets {
            report(label, bomber, n, mass, |seed| {
                scene(seed, bomber, n, key, spots, *order, *walk, *secs)
            });
        }
        println!();
    }
}
