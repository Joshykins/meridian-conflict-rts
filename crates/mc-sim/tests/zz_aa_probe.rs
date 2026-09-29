//! How much damage every anti-air weapon lands on every combat aircraft. For each
//! shooter (its non-air weapons told to target nothing) a passive flight of four
//! circles it at 60% of its longest air reach, too tough to die, for 20 s after 6 s to
//! settle. Prints damage landed per second, the share of the paper figure it reached,
//! and the mass of aircraft killed per second per 1000 mass of shooter.
//!
//! `cargo test --profile gate -p mc-sim --test sim -- zz_aa_probe:: --ignored --nocapture`
//! Knob: AA_PROBE_ONLY=<shooter key> runs one shooter.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, Handle};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const SHOOTERS: &[&str] = &[
    "aster_t1_aa",
    "aster_t1_mobile_aa",
    "aster_t1_frigate",
    "aster_t1_interceptor",
    "aster_commander",
    "aster_t2_aa",
    "aster_t2_mobile_aa",
    "aster_t2_destroyer",
    "aster_t2_aa_cruiser",
    "aster_t2_interceptor",
    "aster_t3_sam",
    "aster_t3_air_superiority",
    "aster_t3_battleship",
    "aster_t3_carrier",
    "aster_t4_assault_tank",
    "aster_t5_titan",
];

const TARGETS: &[&str] = &[
    "aster_t1_interceptor",
    "aster_t1_bomber",
    "aster_t1_rotor_gunship",
    "aster_t2_gunship",
    "aster_t2_torpedo_bomber",
    "aster_t2_interceptor",
    "aster_t2_fire_bomber",
    "aster_t3_air_superiority",
    "aster_t3_strategic_bomber",
    "aster_t3_assault_aircraft",
];

const FLIGHT: i32 = 4;
const TOUGH: i32 = 10_000_000;

fn world(sea: bool) -> World {
    let mut blueprints =
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    for bp in &mut blueprints.units {
        for w in &mut bp.weapons {
            w.target_mask &= cat::AIR;
        }
    }
    let terrain = if sea {
        Heightfield::from_samples(
            1024,
            1024,
            vec![0; 1025 * 1025],
            Fx::from_int(-30),
            Fx::ONE,
            Fx::ZERO,
        )
    } else {
        Heightfield::flat(1024, 1024, Fx::from_int(20))
    };
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

/// Damage per second on paper, and the mass killed per second per 1000 shooter mass.
fn run(shooter: &str, target: &str) -> (f32, f32, f32) {
    let probe = world(false);
    let sid = probe.blueprints.id_of(shooter).unwrap();
    let sbp = probe.blueprints.unit(sid);
    let sea = sbp
        .motion
        .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval);
    let air_weapons: Vec<_> = sbp
        .weapons
        .iter()
        .filter(|w| w.target_mask & cat::AIR != 0)
        .collect();
    let reach = air_weapons
        .iter()
        .map(|w| w.range_max.to_f32())
        .fold(0.0, f32::max);
    let paper: f32 = air_weapons
        .iter()
        .map(|w| w.damage.to_f32() * w.salvo.max(1) as f32 / (w.reload_ticks.max(1) as f32 * 0.1))
        .sum();
    let smass = sbp.cost_mass.to_f32();
    let mut w = world(sea);
    let centre = FxVec2::from_ints(4000, 4000);
    let row = w.spawn_unit(sid, 0, centre, Angle::ZERO, true).unwrap();
    let sid_handle = w.state.units.id(row);
    let tid = w.blueprints.id_of(target).unwrap();
    let tbp = w.blueprints.unit(tid);
    let (thp, tmass) = (tbp.health.to_f32(), tbp.cost_mass.to_f32());
    let motion = tbp.motion.unwrap();
    let orbit = (reach * 0.6) as i32;
    let mut ids = Vec::new();
    for i in 0..FLIGHT {
        let a = i as f32 * std::f32::consts::TAU / FLIGHT as f32;
        let p = FxVec2::from_ints(
            4000 + (a.cos() * orbit as f32) as i32,
            4000 + (a.sin() * orbit as f32) as i32,
        );
        let r = w
            .spawn_unit(tid, 1, p, Angle::from_degrees(90 * i + 90), true)
            .unwrap();
        w.state.units.z[r] = Fx::from_int(20) + motion.altitude;
        w.state.units.speed[r] = motion.speed;
        w.state.units.flags[r] |= flag::PASSIVE;
        ids.push(w.state.units.id(r));
    }
    w.state.units.flags[row] |= flag::PASSIVE;
    w.tick(&[PlayerCommand {
        player: 1,
        // Guard circles at half its radius.
        command: Command::Guard {
            units: ids.clone(),
            pos: centre,
            target: Handle::NONE,
            radius: Fx::from_int(orbit * 2),
            queue: false,
        },
    }])
    .unwrap();
    let settle = 6 * TICKS_PER_SECOND;
    let secs = 20;
    let mut dealt = 0.0f32;
    for t in 0..settle + secs * TICKS_PER_SECOND {
        if t == settle {
            if let Some(r) = w.state.units.row(sid_handle) {
                w.state.units.flags[r] &= !flag::PASSIVE;
            }
        }
        for &id in &ids {
            if let Some(r) = w.state.units.row(id) {
                w.state.units.health[r] = Fx::from_int(TOUGH);
            }
        }
        w.tick(&[]).unwrap();
        if t >= settle {
            for &id in &ids {
                if let Some(r) = w.state.units.row(id) {
                    dealt += TOUGH as f32 - w.state.units.health[r].to_f32();
                }
            }
        }
    }
    let dps = dealt / secs as f32;
    (
        dps,
        100.0 * dps / paper.max(1.0),
        dps / thp * tmass / smass * 1000.0,
    )
}

#[test]
#[ignore]
fn zz_aa_probe() {
    let only = std::env::var("AA_PROBE_ONLY").ok();
    for &s in SHOOTERS {
        if only.as_deref().is_some_and(|o| o != s) {
            continue;
        }
        println!("--- {s}");
        for &t in TARGETS {
            let (dps, share, trade) = run(s, t);
            println!(
                "  vs {t:<28} {dps:>7.0}/s  {share:>4.0}% of paper  {trade:>6.2} mass/s per 1k"
            );
        }
    }
}
