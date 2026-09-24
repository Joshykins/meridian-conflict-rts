//! The Gannet torpedo bomber: it hears dived hulls with its own sonar, comes down
//! low over the water, and its torpedoes fall in, then run and home.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// The sea: 20 m of water over a flat bed at zero, with a strip of land along the west edge.
const WATER: i32 = 20;

fn blueprints() -> Arc<Blueprints> {
    Arc::new(Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap())
}

fn sea(fog: bool) -> World {
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..40 {
            samples[y * 257 + x] = 40;
        }
    }
    let terrain =
        Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(WATER));
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
        seed: 7,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints(), Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32, flags: u16) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.flags[row] |= flags;
    w.state.units.id(row)
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

fn order(w: &mut World, player: u8, command: Command) {
    w.tick(&[PlayerCommand { player, command }]).unwrap();
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn under(w: &World, id: UnitId) -> bool {
    let r = row(w, id);
    w.state.units.z[r] + w.bp(r).height < Fx::from_int(WATER)
}

fn impacts(w: &World) -> usize {
    w.events
        .iter()
        .filter(|e| matches!(e, SimEvent::Impact { on_unit: true, .. }))
        .count()
}

/// Flies a Gannet from the south at `target` on attack-move; returns the ticks it
/// took for the mark to die (or None), the hits, whether a torpedo was ever seen
/// falling, and the lowest the bomber flew.
fn raid(w: &mut World, target: UnitId, ticks: u32) -> (Option<u32>, usize, bool, Fx) {
    let bomber = spawn(w, "aster_t2_torpedo_bomber", 0, 1000, 300, 0);
    run(w, 40);
    let goal = w.state.units.pos[row(w, target)];
    order(
        w,
        0,
        Command::AttackMove {
            units: vec![bomber],
            target: goal,
            queue: false,
        },
    );
    let (mut hits, mut fell, mut lowest) = (0, false, Fx::from_int(1000));
    for t in 0..ticks {
        w.tick(&[]).unwrap();
        hits += impacts(w);
        let p = &w.state.projectiles;
        fell |= (0..p.len()).any(|i| p.pos[i].z > Fx::from_int(WATER));
        if let Some(r) = w.state.units.row(bomber) {
            if w.state.units.flags[r] & flag::AIR_RUN != 0 {
                lowest = lowest.min(w.state.units.z[r] - Fx::from_int(WATER));
            }
        }
        if w.state.units.row(target).is_none() {
            return (Some(t), hits, fell, lowest);
        }
    }
    (None, hits, fell, lowest)
}

#[test]
fn a_gannet_hears_and_sinks_a_dived_submarine() {
    let mut w = sea(true);
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 1000, 1200, flag::PASSIVE);
    run(&mut w, 40);
    assert!(under(&w, sub));
    let (sunk, hits, fell, lowest) = raid(&mut w, sub, 1500);
    assert!(fell, "the torpedoes are dropped from the air");
    assert!(
        lowest < Fx::from_int(45),
        "it comes down low for the drop: {lowest:?}"
    );
    assert!(sunk.is_some(), "the submarine lived: {hits} hits");
    assert!(hits >= 3, "900 hit points take three torpedoes");
}

#[test]
fn a_gannet_torpedoes_a_frigate() {
    let mut w = sea(false);
    let ship = spawn(&mut w, "aster_t1_frigate", 1, 1000, 1200, flag::PASSIVE);
    let (sunk, hits, _, _) = raid(&mut w, ship, 300);
    assert!(sunk.is_none(), "2600 hit points outlast two passes");
    assert!(hits >= 4, "two passes put four torpedoes into it: {hits}");
    let (sunk, _, _, _) = raid(&mut w, ship, 1500);
    assert!(sunk.is_some(), "two Gannets finish it");
}

#[test]
fn a_gannet_leaves_land_units_alone() {
    let mut w = sea(false);
    let tank = spawn(&mut w, "aster_t1_tank", 1, 20, 1000, flag::PASSIVE);
    let bomber = spawn(&mut w, "aster_t2_torpedo_bomber", 0, 200, 1000, 0);
    run(&mut w, 300);
    let r = row(&w, tank);
    assert_eq!(w.state.units.health[r], w.bp(r).health);
    assert!(w.state.units.row(bomber).is_some());
}
