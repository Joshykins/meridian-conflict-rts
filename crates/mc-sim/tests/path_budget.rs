//! More units heading for different places than pathing has field slots: the
//! match goes on, the overflow waits for a slot and then moves.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        name: "path budget".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(200, 200), FxVec2::from_ints(3800, 3800)],
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
        players: vec![player("you", 0), player("other", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(2)), &config).unwrap()
}

#[test]
fn running_out_of_field_slots_makes_units_wait_not_the_match_end() {
    let mut w = world();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let count = mc_path::NavConfig::default().max_fields + 40;
    let side = (count as f64).sqrt().ceil() as usize;
    let mut orders = Vec::new();
    let mut goals = Vec::new();
    for i in 0..count {
        let at = FxVec2::from_ints(300 + (i % side) as i32 * 100, 300 + (i / side) as i32 * 100);
        let row = w.spawn_unit(tank, 0, at, Angle::ZERO, true).unwrap();
        // Every goal its own cell, so no two units can share a field.
        let goal = at + FxVec2::from_ints(40, 0);
        goals.push((w.state.units.id(row), goal));
        orders.push(PlayerCommand {
            player: 0,
            command: Command::Move {
                units: vec![w.state.units.id(row)],
                target: goal,
                queue: false,
            },
        });
    }
    w.tick(&orders)
        .expect("a full field table must not end the match");
    for _ in 0..300 {
        w.tick(&[])
            .expect("a full field table must not end the match");
    }
    let near = |id| {
        let row = w.state.units.row(id).expect("nothing here can die");
        let goal = goals.iter().find(|g| g.0 == id).unwrap().1;
        w.state.units.pos[row].distance(goal) < Fx::from_int(12)
    };
    let arrived = goals.iter().filter(|g| near(g.0)).count();
    assert_eq!(
        arrived, count,
        "units left waiting for a field slot never moved"
    );
}
