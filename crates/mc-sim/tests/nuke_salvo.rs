//! A salvo: dozens of warheads on one mark (docs/NUKES.md, "Salvos"). Every one lands and
//! hurts; a T5 takes them one after another; the sim's tick stays cheap while they fly.
//! `NUKE_SALVO=n` sets how many (60 by default); `--nocapture` prints tick times.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::SimEvent;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "salvo".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(15000, 15000)],
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
        seed: 7,
        players: vec![player("you", 0), player("them", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        Heightfield::flat(2048, 2048, Fx::from_int(20)),
        map,
        blueprints,
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

fn spawn(w: &mut World, owner: u8, key: &str, at: FxVec2) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    w.tick(&[PlayerCommand {
        player: owner,
        command: Command::DebugSpawn {
            owner,
            blueprint,
            pos: at,
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    }])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == blueprint && u.owner[r] == owner)
        .min_by_key(|&r| u.pos[r].distance_sq(at))
        .unwrap();
    u.id(row)
}

#[test]
fn a_salvo_of_warheads_all_lands_and_the_tick_stays_cheap() {
    let n: usize = std::env::var("NUKE_SALVO")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let mut w = world();
    let mark = FxVec2::from_ints(8000, 8000);
    let titan = spawn(&mut w, 1, "aster_t5_titan", mark);
    // Silos in rings round the mark, 4.5-7.5 km out, well past the titan's guns; an array
    // beside the titan.
    let mut silos = Vec::new();
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU * 3.0;
        let r = 4500.0 + 3000.0 * (i as f32 / n as f32);
        let at = FxVec2::new(
            Fx::from_f32(8000.0 + a.cos() * r),
            Fx::from_f32(8000.0 + a.sin() * r),
        );
        let id = spawn(&mut w, 0, "aster_t4_nuke_silo", at);
        w.state.strategic.launchers.entry(id).or_default().stock = 1;
        silos.push(id);
    }
    silos.sort_unstable();
    silos.dedup();
    let n = silos.len();
    // Its side has the power to run the array (and the titan's field).
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::DebugFreeBuild {
            player: 1,
            on: true,
        },
    }])
    .unwrap();
    let array = spawn(
        &mut w,
        1,
        "aster_t3_nuke_defense",
        FxVec2::from_ints(8300, 8000),
    );
    w.state.strategic.launchers.entry(array).or_default().stock = 4;
    let health = |w: &World| {
        w.state
            .units
            .row(titan)
            .map(|r| w.state.units.health[r].to_f32())
    };
    let before = health(&w).unwrap();
    let orders: Vec<_> = silos
        .iter()
        .map(|&id| PlayerCommand {
            player: 0,
            command: Command::LaunchNuke {
                units: vec![id],
                pos: mark,
            },
        })
        .collect();
    w.tick(&orders).unwrap();
    let (mut launched, mut burst, mut killed) = (0, 0, 0);
    let (mut total, mut worst, mut ticks) = (0.0f64, 0.0f64, 0);
    let mut worst_flying = 0;
    let (mut last, mut hurt) = (before, 0);
    let (mut worst_at, mut first_burst) = (0, None);
    for _ in 0..1600 {
        let clock = Instant::now();
        w.tick(&[]).unwrap();
        let ms = clock.elapsed().as_secs_f64() * 1000.0;
        total += ms;
        if ms > worst {
            worst = ms;
            worst_at = ticks;
        }
        ticks += 1;
        worst_flying = worst_flying.max(w.state.strategic.missiles.len());
        let now = health(&w);
        // Every burst after the first few (its field takes those) must hurt it.
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::NuclearDetonation { .. }))
            && now.is_none_or(|h| h < last)
        {
            hurt += 1;
        }
        last = now.unwrap_or(0.0);
        for e in &w.events {
            match e {
                SimEvent::NuclearLaunch { .. } => launched += 1,
                SimEvent::NuclearDetonation { .. } => {
                    burst += 1;
                    first_burst.get_or_insert(ticks);
                }
                SimEvent::WarheadIntercepted { killed: true, .. } => killed += 1,
                _ => {}
            }
        }
        if launched == n && burst + killed == n && w.state.strategic.detonations.is_empty() {
            break;
        }
    }
    let after = health(&w);
    println!(
        "salvo {n}: {launched} launched, {burst} burst, {killed} intercepted, {worst_flying} in flight at once; \
         {ticks} ticks, mean {:.3} ms, worst {:.3} ms at tick {worst_at} (first burst {first_burst:?}); titan {before} -> {after:?}",
        total / ticks as f64,
        worst,
    );
    assert_eq!(launched, n);
    assert_eq!(burst + killed, n);
    // Warheads land on it one after another (several may land the same tick), and past
    // what its field soaks up they keep hurting it.
    assert!(after.is_none_or(|h| h < before), "{before} -> {after:?}");
    assert!(hurt >= 10, "only {hurt} ticks of bursts hurt it");
}
