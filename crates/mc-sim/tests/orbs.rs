//! The Carillon, the Regency's orb battery: it makes plasma orbs a batch at a time while
//! idle and holds up to four batches, spends one orb a shot when something comes into
//! reach, and sends each as a fleck that homes on its mark, takes another when the mark
//! dies and flies home to the store when there is none. Missile defence can shoot the
//! flecks down.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::RenderFrame;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, UnitId, World};
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
        doodads: Default::default(),
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
fn it_makes_a_batch_at_a_time_and_holds_four() {
    let mut w = world();
    let spire = spawn(&mut w, SPIRE, 0, AT);
    let bp = w.blueprints.id_of(SPIRE).unwrap();
    let weapon = &w.blueprints.unit(bp).weapons[0];
    let (batch, period, cap) = (weapon.orbs, weapon.orb_ticks as u32, weapon.muzzles.len());
    assert_eq!(cap, 4 * batch as usize, "it holds four batches");
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
        "the store stops at four batches"
    );
    let mut frame = RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    let held = frame
        .units
        .iter()
        .find(|u| u.unit_id == spire.0)
        .map(|u| u.status[2]);
    assert_eq!(
        held,
        Some(u32::MAX >> (32 - cap)),
        "every slot is told held"
    );
}

/// Flecks in the air.
fn flecks(w: &World) -> usize {
    w.state.projectiles.len()
}

fn full_spire(w: &mut World) -> UnitId {
    let spire = spawn(w, SPIRE, 0, AT);
    let period = w
        .blueprints
        .unit(w.blueprints.id_of(SPIRE).unwrap())
        .weapons[0]
        .orb_ticks;
    run(w, period as u32 * 4 + 1);
    spire
}

#[test]
fn flecks_home_on_an_aircraft() {
    let mut w = world();
    let spire = full_spire(&mut w);
    let full = stock(&w, spire);
    let plane = spawn(&mut w, "aster_t1_interceptor", 1, (AT.0, AT.1 + 800));
    run(&mut w, 8);
    assert!(stock(&w, spire) < full, "orbs were spent");
    run(&mut w, 112);
    let hurt = w.state.units.row(plane).is_none_or(|r| {
        w.state.units.health[r] < w.blueprints.unit(w.state.units.blueprint[r]).health
    });
    assert!(hurt, "the flecks caught the aircraft");
}

#[test]
fn flecks_take_another_mark_when_theirs_dies() {
    let mut w = world();
    let spire = full_spire(&mut w);
    let first = spawn(&mut w, "aster_t1_tank", 1, (AT.0 + 900, AT.1));
    let second = spawn(&mut w, "aster_t1_tank", 1, (AT.0 + 900, AT.1 + 150));
    run(&mut w, 8);
    assert!(flecks(&w) > 0, "flecks are in the air");
    let row = w.state.units.row(first).unwrap();
    w.state.units.health[row] = Fx::ZERO;
    w.tick(&[]).unwrap();
    w.tick(&[]).unwrap();
    let on_second = (0..w.state.projectiles.len())
        .filter(|&i| w.state.projectiles.target[i] == second)
        .count();
    assert!(on_second > 0, "flecks went on to the other tank");
    assert!(w.state.units.row(spire).is_some());
}

#[test]
fn flecks_with_no_mark_left_return_to_the_spire() {
    let mut w = world();
    let spire = full_spire(&mut w);
    let full = stock(&w, spire);
    let tank = spawn(&mut w, "aster_t1_tank", 1, (AT.0 + 900, AT.1));
    run(&mut w, 8);
    let spent = stock(&w, spire);
    assert!(spent < full && flecks(&w) > 0);
    let row = w.state.units.row(tank).unwrap();
    w.state.units.health[row] = Fx::ZERO;
    // Home from 900 m out is a few seconds' flight, well short of the flecks' lifetime:
    // one swinging back and forth across the spire instead of landing would still be up.
    run(&mut w, 100);
    assert_eq!(flecks(&w), 0, "no fleck is left flying");
    assert!(
        stock(&w, spire) > spent,
        "the flecks went back into the store: {} of {full}",
        stock(&w, spire)
    );
}

#[test]
fn flecks_climb_over_a_ridge_in_their_way() {
    let mut w = world();
    // A ridge across the way to the mark, taller than the spire's flecks float.
    w.terrain
        .flatten_rect((150, 0), (155, 511), Some(Fx::from_int(120)));
    let spire = full_spire(&mut w);
    let tank = spawn(&mut w, "aster_t1_tank", 1, (AT.0 + 900, AT.1));
    run(&mut w, 150);
    assert!(stock(&w, spire) < 32, "orbs were spent");
    let row = w.state.units.row(tank);
    let hurt = row.is_none_or(|r| {
        w.state.units.health[r] < w.blueprints.unit(w.state.units.blueprint[r]).health
    });
    assert!(hurt, "the flecks got over the ridge");
}

#[test]
fn missile_defence_shoots_flecks_down() {
    let mut w = world();
    full_spire(&mut w);
    spawn(&mut w, "aster_t1_tank", 1, (AT.0 + 900, AT.1));
    spawn(
        &mut w,
        "aster_t2_missile_defense",
        1,
        (AT.0 + 960, AT.1 + 40),
    );
    let mut lased = 0;
    for _ in 0..60 {
        w.tick(&[]).unwrap();
        lased += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::MissileLased { killed: true, .. }))
            .count();
    }
    assert!(lased > 0, "the lasers burned flecks out of the air");
}
