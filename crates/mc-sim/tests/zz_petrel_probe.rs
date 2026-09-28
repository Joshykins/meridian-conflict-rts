//! Probe: how fast T1 Petrels kill early targets. `cargo test -p mc-sim --test
//! zz_petrel_probe -- --ignored --nocapture`

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        Heightfield::flat(512, 512, Fx::from_int(20)),
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

/// `n` bombers attack targets `key` laid out at `spots`; prints each target's
/// health over time and when it dies.
fn scene(n: i32, key: &str, spots: &[(i32, i32)], attack_move: bool) {
    let mut w = world();
    let bombers: Vec<_> = (0..n)
        .map(|i| add(&mut w, "aster_t1_bomber", 0, 1300, 1980 + i * 20))
        .collect();
    let targets: Vec<_> = spots
        .iter()
        .map(|&(x, y)| add(&mut w, key, 1, x, y))
        .collect();
    for &t in &targets {
        w.state.units.flags[t] |= flag::PASSIVE;
    }
    let tids: Vec<_> = targets.iter().map(|&t| w.state.units.id(t)).collect();
    let ids: Vec<_> = bombers.iter().map(|&r| w.state.units.id(r)).collect();
    let cmds: Vec<_> = if attack_move {
        vec![PlayerCommand {
            player: 0,
            command: Command::AttackMove {
                units: ids.clone(),
                target: FxVec2::from_ints(spots[0].0 + 300, spots[0].1),
                queue: false,
            },
        }]
    } else {
        tids.iter()
            .enumerate()
            .map(|(i, &t)| PlayerCommand {
                player: 0,
                command: Command::Attack {
                    units: ids.clone(),
                    target: t,
                    queue: i > 0,
                },
            })
            .collect()
    };
    w.tick(&cmds).unwrap();
    let hp0 = w.state.units.health[targets[0]].to_f64();
    println!("== {n} Petrel(s) vs {} x {key} (hp {hp0})", spots.len());
    let mut dead = vec![false; targets.len()];
    for t in 1..=1200u32 {
        w.tick(&[]).unwrap();
        for (i, &tid) in tids.iter().enumerate() {
            if !dead[i] && w.state.units.row(tid).is_none() {
                dead[i] = true;
                println!("   target {i} dead at {:.1} s", t as f64 / 10.0);
            }
        }
        if t % 100 == 0 {
            let hp: Vec<String> = tids
                .iter()
                .map(|&id| match w.state.units.row(id) {
                    Some(r) => format!("{:.0}", w.state.units.health[r].to_f64()),
                    None => "x".into(),
                })
                .collect();
            println!("   {:>3} s: {}", t / 10, hp.join(" "));
        }
        if dead.iter().all(|&d| d) {
            break;
        }
    }
}

#[test]
#[ignore]
fn zz_petrel_probe() {
    scene(1, "aster_t1_engineer", &[(2000, 2000)], false);
    scene(
        1,
        "aster_t1_engineer",
        &[(2000, 2000), (2008, 2006), (1994, 2010)],
        false,
    );
    scene(1, "aster_t1_power", &[(2000, 2000)], false);
    scene(2, "aster_t1_power", &[(2000, 2000)], false);
    scene(1, "aster_core_mine", &[(2000, 2000)], false);
    scene(3, "aster_core_mine", &[(2000, 2000)], false);
    let column: Vec<_> = (0..6)
        .map(|i| (2000 + (i % 3) * 14, 2000 + (i / 3) * 14))
        .collect();
    scene(3, "aster_t1_tank", &column, true);
}
