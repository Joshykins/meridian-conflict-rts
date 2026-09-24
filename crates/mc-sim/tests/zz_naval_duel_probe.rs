//! Probe, not a rule: fights between T1 warships on open sea, printed for balance
//! work. `cargo test -p mc-sim --release --test zz_naval_duel_probe -- --ignored --nocapture`

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn sea(seed: u64) -> World {
    let samples = vec![0u16; 257 * 257];
    let terrain = Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(20));
    let map = MapData {
        name: "sea".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(150, 300), FxVec2::from_ints(150, 1700)],
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
        players: vec![player("a", 0), player("b", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let bps = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    World::with_terrain(terrain, map, bps, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32, flags: u16) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.flags[row] |= flags;
    w.state.units.id(row)
}

fn hp(w: &World, ids: &[UnitId]) -> i32 {
    ids.iter()
        .filter_map(|&id| w.state.units.row(id))
        .map(|r| w.state.units.health[r].ceil_int())
        .sum()
}

/// `n` of `a` attack `m` of `b`; `b` fights back unless `passive`.
fn fight(a: &str, n: usize, b: &str, m: usize, passive: bool, seed: u64) -> String {
    let mut w = sea(seed);
    let us: Vec<_> = (0..n)
        .map(|i| spawn(&mut w, a, 0, 700, 800 + i as i32 * 18, 0))
        .collect();
    let flags = if passive { flag::PASSIVE } else { 0 };
    let them: Vec<_> = (0..m)
        .map(|i| spawn(&mut w, b, 1, 1150, 850 + i as i32 * 40, flags))
        .collect();
    let start = hp(&w, &them);
    let cmd = Command::Attack {
        units: us.clone(),
        target: them[0],
        queue: false,
    };
    w.tick(&[PlayerCommand {
        player: 0,
        command: cmd,
    }])
    .unwrap();
    for t in 0..3000 {
        w.tick(&[]).unwrap();
        let (ha, hb) = (hp(&w, &us), hp(&w, &them));
        if ha == 0 || hb == 0 {
            let alive = us
                .iter()
                .filter(|&&id| w.state.units.row(id).is_some())
                .count();
            return format!("{t:>5} ticks  {alive}/{n} {a} left, {b} hp {hb}/{start}");
        }
    }
    format!("timeout, {b} hp {}/{start}", hp(&w, &them))
}

#[test]
#[ignore]
fn probe() {
    for seed in [1, 2, 3] {
        println!("seed {seed}");
        for n in [1, 4, 6, 8] {
            println!(
                "  {n} skiff -> passive pike: {}",
                fight("aster_t1_attack_boat", n, "aster_t1_frigate", 1, true, seed)
            );
        }
        for n in [4, 6, 8] {
            println!(
                "  {n} skiff -> pike:         {}",
                fight(
                    "aster_t1_attack_boat",
                    n,
                    "aster_t1_frigate",
                    1,
                    false,
                    seed
                )
            );
        }
        println!(
            "  2 skiff -> barracuda(surf): {}",
            fight(
                "aster_t1_attack_boat",
                2,
                "aster_t1_submarine",
                1,
                false,
                seed
            )
        );
        println!(
            "  1 skiff -> skiff:      {}",
            fight(
                "aster_t1_attack_boat",
                1,
                "aster_t1_attack_boat",
                1,
                false,
                seed
            )
        );
    }
}
