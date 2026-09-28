//! How hard the Behemoth's Tempest rotary cannon hits what it shoots at, alone (the pods
//! and the bore are told to target nothing). Prints shells fired and the damage they did
//! to a T3 bot company, a T4 tank and a T3 factory block at a few ranges (made too tough
//! to die, so every shell counts). Knobs: TEMPEST_SPREAD, TEMPEST_SPLASH, TEMPEST_ARC=1.
//!
//! `cargo test --release -p mc-sim --test sim -- zz_tempest_probe:: --ignored --nocapture`

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const TITAN: &str = "aster_t5_titan";

fn world() -> World {
    let mut blueprints =
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let titan = blueprints.id_of(TITAN).unwrap();
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok());
    for w in &mut blueprints.units[titan.0 as usize].weapons {
        if w.spin_ramp == 0 {
            w.target_mask = 0;
            continue;
        }
        // Try-outs: TEMPEST_SPREAD (degrees), TEMPEST_SPLASH (m), TEMPEST_ARC=1 (ballistic).
        if let Some(s) = env("TEMPEST_SPREAD") {
            w.spread = (s * 65536.0 / 360.0).round() as u16;
        }
        if let Some(s) = env("TEMPEST_SPLASH") {
            w.splash = Fx::from_f32(s as f32);
        }
        if env("TEMPEST_ARC") == Some(1.0) {
            w.trajectory = mc_data::Trajectory::Ballistic;
        }
    }
    let terrain = Heightfield::flat(1024, 1024, Fx::from_int(20));
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
        seed: 5,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        terrain,
        map,
        Arc::new(blueprints),
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

fn run(target: &str, count: i32, gap: i32, range: i32) {
    let mut w = world();
    let bp = w.blueprints.id_of(TITAN).unwrap();
    let row = w
        .spawn_unit(bp, 0, FxVec2::from_ints(1000, 3000), Angle::ZERO, true)
        .unwrap();
    let _ = row;
    let tid = w.blueprints.id_of(target).unwrap();
    let mut rows = Vec::new();
    for i in 0..count {
        let x = 1000 + range + (i % 3) * gap;
        let y = 3000 + (i / 3 - count / 6) * gap;
        let r = w
            .spawn_unit(
                tid,
                1,
                FxVec2::from_ints(x, y),
                Angle::from_degrees(180),
                true,
            )
            .unwrap();
        w.state.units.fire_state[r] = FireState::HoldFire;
        // Too tough to die in the window, so every shell's share can be counted.
        w.state.units.health[r] = Fx::from_int(10_000_000);
        rows.push(w.state.units.id(r));
    }
    let hp0: f32 = rows
        .iter()
        .map(|&id| w.state.units.health[w.state.units.row(id).unwrap()].to_f32())
        .sum();
    let mut shots = 0;
    let secs = 20;
    for _ in 0..secs * TICKS_PER_SECOND {
        w.tick(&[]).unwrap();
        shots += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == bp))
            .count();
    }
    let hp1: f32 = rows
        .iter()
        .map(|&id| {
            w.state
                .units
                .row(id)
                .map_or(0.0, |r| w.state.units.health[r].to_f32())
        })
        .sum();
    let dmg = w
        .blueprints
        .unit(bp)
        .weapons
        .iter()
        .find(|w| w.spin_ramp > 0)
        .unwrap()
        .damage
        .to_f32();
    println!(
        "{target:>24} x{count:<2} at {range:>4} m: {shots:>3} shells in {secs} s, {:>8.0} dealt ({:>3.0}% of full hits, {:>5.0}/s)",
        hp0 - hp1,
        100.0 * (hp0 - hp1) / dmg / shots.max(1) as f32,
        (hp0 - hp1) / secs as f32,
    );
}

#[test]
#[ignore]
fn zz_tempest_probe() {
    for range in [1100, 1700, 2400] {
        run("aster_t3_assault_bot", 9, 60, range);
        run("aster_t4_assault_tank", 1, 0, range);
        run("aster_t3_land_factory", 3, 120, range);
    }
}
