//! Real-order smoke playthrough. Requires scripts/vela.sh; never edits world state.
use mc_core::Fx;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
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
    let mut world = World::new(&map, bp, Arc::new(mc_jobs::Pool::new(0)), &cfg).unwrap();
    world
        .begin_mission(mc_missions::definition(placed))
        .unwrap();
    let mut seen = BTreeSet::new();
    let mut target = String::new();
    for tick in 0..30000 {
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
        let next = route(m);
        let mut commands = Vec::new();
        if m.controls && (next != target || tick % 200 == 0) {
            target = next.into();
            let region = m
                .definition
                .placed
                .regions
                .iter()
                .find(|r| r.name == next)
                .unwrap();
            let mut at = (region.min + region.max) * Fx::HALF;
            if next == "ridge" && m.fired.contains_key("ridge") {
                let guns = m
                    .definition
                    .placed
                    .units
                    .iter()
                    .filter(|u| u.group == "bridge_guns")
                    .collect::<Vec<_>>();
                if let Some(gun) = guns.first() {
                    at = gun.at;
                }
            }
            let ids = m.groups["company"]
                .iter()
                .copied()
                .filter(|id| world.state.units.row(*id).is_some())
                .collect();
            commands.push(PlayerCommand {
                player: 0,
                command: Command::AttackMove {
                    units: ids,
                    target: at,
                    queue: false,
                },
            });
            if tick % 1000 == 0 {
                let row = world.state.units.row(m.units["merrick"]).unwrap();
                println!(
                    "tick {tick} -> {next}, Merrick {:?}, health {:?}",
                    world.state.units.pos[row], world.state.units.health[row]
                );
            }
        }
        world.tick(&commands).unwrap();
    }
    let m = world.state.mission.as_ref().unwrap();
    let row = world.state.units.row(m.units["merrick"]).unwrap();
    panic!(
        "Mission stuck at {target}: {:?}; events {:?}",
        world.state.units.pos[row], m.fired
    );
}
fn route(m: &mc_sim::mission::Mission) -> &'static str {
    for (event, region) in [
        ("roadblock_clear", "first_contact"),
        ("pdf_linkup", "junction"),
        ("bridge_clear", "ridge"),
        ("miners_freed", "camp"),
        ("betrayal", "square"),
        ("breakout", "breakout"),
        ("holdouts_rescued", "warehouse"),
        ("flank", "harbour"),
        ("base_destroyed", "spaceport"),
    ] {
        if !m.fired.contains_key(event) {
            return region;
        }
    }
    "spaceport"
}
