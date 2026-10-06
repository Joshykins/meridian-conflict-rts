//! The Carillon, the Regency's orb battery: it makes plasma orbs a batch at a time while
//! idle and holds up to five batches, spends one orb a shot from the top slot down when
//! something comes into reach, lobs each on an arc of its own height, and reaches land,
//! sea, structures and aircraft but not a submerged boat.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::RenderFrame;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const SPIRE: &str = "regency_t4_area_defense";
const AT: (i32, i32) = (1000, 1000);

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        name: "orbs".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(AT.0, AT.1), FxVec2::from_ints(3000, 3000)],
        props: Vec::new(),
    };
    let player = |faction: &str, team| PlayerSetup {
        name: faction.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 5,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, (x, y): (i32, i32)) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

fn stock(w: &World, spire: UnitId) -> u8 {
    w.state.units.orb_stock[w.state.units.row(spire).unwrap()]
}

#[test]
fn it_makes_a_batch_at_a_time_and_holds_five() {
    let mut w = world();
    let spire = spawn(&mut w, SPIRE, 0, AT);
    let bp = w.blueprints.id_of(SPIRE).unwrap();
    let weapon = &w.blueprints.unit(bp).weapons[0];
    let (batch, period, cap) = (weapon.orbs, weapon.orb_ticks as u32, weapon.muzzles.len());
    assert_eq!(cap, 5 * batch as usize, "it holds five batches");
    run(&mut w, period - 1);
    assert_eq!(
        stock(&w, spire),
        0,
        "nothing before the first batch is done"
    );
    run(&mut w, 1);
    assert_eq!(stock(&w, spire), batch, "the first batch comes whole");
    run(&mut w, period * 10);
    assert_eq!(
        stock(&w, spire) as usize,
        cap,
        "the store stops at five batches"
    );
    let mut frame = RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    let held = frame
        .units
        .iter()
        .find(|u| u.unit_id == spire.0)
        .map(|u| u.status[2]);
    assert_eq!(held, Some((1u32 << cap) - 1), "every slot is told held");
}

#[test]
fn a_full_store_goes_over_on_arcs_of_different_heights() {
    let mut w = world();
    let spire = spawn(&mut w, SPIRE, 0, AT);
    let period = w
        .blueprints
        .unit(w.blueprints.id_of(SPIRE).unwrap())
        .weapons[0]
        .orb_ticks;
    run(&mut w, period as u32 * 5 + 1);
    let full = stock(&w, spire);
    let tank = spawn(&mut w, "aster_t1_tank", 1, (AT.0 + 900, AT.1));
    let plane = spawn(&mut w, "aster_t1_interceptor", 1, (AT.0, AT.1 + 800));
    // Each shell's whole flight, in ticks: what it has flown and what it has left.
    let (mut fired, mut flights) = (0, std::collections::BTreeSet::new());
    for _ in 0..60 {
        w.tick(&[]).unwrap();
        fired += w
            .events
            .iter()
            .filter(|e| matches!(e, mc_sim::SimEvent::ShotFired { .. }))
            .count();
        let p = &w.state.projectiles;
        for i in 0..p.len() {
            flights.insert(p.age[i] as u32 + p.ticks_left[i] as u32);
        }
    }
    assert!(stock(&w, spire) < full, "orbs were spent");
    assert!(fired >= 8, "a salvo of orbs went up: {fired}");
    let (lo, hi) = (*flights.first().unwrap(), *flights.last().unwrap());
    assert!(hi > lo + 5, "arcs of different heights: {lo}..{hi} ticks");
    // The last shells land.
    run(&mut w, 150);
    let hurt = |id: UnitId| {
        w.state.units.row(id).is_none_or(|r| {
            w.state.units.health[r] < w.blueprints.unit(w.state.units.blueprint[r]).health
        })
    };
    assert!(hurt(tank) || hurt(plane), "something was hit");
}
