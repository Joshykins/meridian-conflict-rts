//! Real-order smoke playthrough. Requires scripts/vela.sh; never edits world state.
use mc_sim::{MatchConfig, PlayerSetup, World};
use std::{collections::BTreeSet, path::Path, sync::Arc};
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let map = mc_map::MapFile::open(root.join("maps/vela_m1/terrain.mcmap")).unwrap();
    let bp = Arc::new(mc_data::Blueprints::load(&root.join("data")).unwrap());
    let placed = mc_map::placed::Placed::read(
        &map.path().with_file_name("placed.ron"),
        map.info(),
        map.props().len(),
    )
    .unwrap();
    let cfg = MatchConfig {
        seed: 7,
        cheats: false,
        fog: true,
        spawn_commanders: false,
        players: (0..6)
            .map(|i| PlayerSetup {
                name: format!("side {i}"),
                faction: "Aster".into(),
                ai: Default::default(),
                team: if i == 1 { 1 } else { 0 },
                controller: mc_sim::tables::Controller::Human,
                start: 0,
            })
            .collect(),
    };
    let mut world = World::new(&map, bp.clone(), Arc::new(mc_jobs::Pool::new(0)), &cfg).unwrap();
    world
        .begin_mission(mc_missions::definition(placed))
        .unwrap();
    let mut seen = BTreeSet::new();
    let mut pilot = mc_missions::Pilot::default();
    let mut restored_beat = 0;
    let resume = std::env::args().any(|a| a == "--resume");
    for tick in 0..30000 {
        let beat = world.mission_status().unwrap().beat;
        if resume && beat > restored_beat {
            let hash = world.hash();
            let mut fresh =
                World::new(&map, bp.clone(), Arc::new(mc_jobs::Pool::new(0)), &cfg).unwrap();
            fresh
                .restore(mc_map::Heightfield::load(&map).unwrap(), &world.snapshot())
                .unwrap();
            assert_eq!(hash, fresh.hash());
            world = fresh;
            restored_beat = beat;
            println!("Restored checkpoint {beat} at tick {tick}");
        }
        let m = world.state.mission.as_ref().unwrap();
        for (event, at) in &m.fired {
            if seen.insert(event.clone()) && !event.starts_with("line_") {
                println!("{at:05} {event}");
            }
        }
        if let Some(win) = m.outcome {
            println!("Outcome {win}, tick {tick}, hash {:016x}", world.hash());
            assert!(win);
            return;
        }
        let commands = pilot.commands(&world);
        world.tick(&commands).unwrap();
    }
    let m = world.state.mission.as_ref().unwrap();
    let row = world.state.units.row(m.units["merrick"]).unwrap();
    panic!(
        "Mission stuck at {:?}; events {:?}",
        world.state.units.pos[row], m.fired
    );
}
