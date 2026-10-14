use super::*;
use crate::{tables::Controller, world::MapData, MatchConfig, PlayerSetup};
use mc_core::{Angle, FxVec2};
use mc_map::{
    placed::{PlacedUnit, Region},
    Heightfield,
};
use std::{path::Path, sync::Arc};

fn terrain() -> Heightfield {
    Heightfield::from_samples(256, 256, vec![20; 257 * 257], Fx::ZERO, Fx::ONE, Fx::ZERO)
}
fn world(workers: usize) -> World {
    let bp = mc_data::Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
        .unwrap();
    World::with_terrain(
        terrain(),
        MapData {
            name: "mission test".into(),
            content_id: 1,
            ore: vec![],
            starts: vec![],
            props: vec![],
        },
        Arc::new(bp),
        Arc::new(mc_jobs::Pool::new(workers)),
        &MatchConfig {
            seed: 7,
            cheats: true,
            fog: false,
            spawn_commanders: false,
            players: (0..3)
                .map(|i| PlayerSetup {
                    name: format!("side {i}"),
                    faction: "Aster".into(),
                    ai: Default::default(),
                    team: 0,
                    controller: Controller::Human,
                    start: 0,
                })
                .collect(),
        },
    )
    .unwrap()
}
fn definition() -> Definition {
    let placed = Placed {
        next_id: 5,
        units: (0..3)
            .map(|i| PlacedUnit {
                id: i as u64 + 1,
                name: ["hero", "pdf", "holdout"][i].into(),
                group: ["company", "pdf", "holdouts"][i].into(),
                blueprint: "aster_t1_tank".into(),
                owner: i as u8,
                at: FxVec2::from_ints(300 + i as i32 * 600, 300),
                heading: Angle::ZERO,
            })
            .collect(),
        regions: vec![Region {
            id: 4,
            name: "square".into(),
            min: FxVec2::from_ints(500, 500),
            max: FxVec2::from_ints(700, 700),
        }],
        ..Default::default()
    };
    Definition {
        key: "test".into(),
        placed,
        events: vec![
            Event {
                name: "opening".into(),
                when: Condition::Elapsed(0),
                actions: vec![
                    Action::Spawn("company".into()),
                    Action::Spawn("pdf".into()),
                    Action::Spawn("holdouts".into()),
                    Action::Say {
                        line: "welcome".into(),
                        ticks: 30,
                    },
                ],
            },
            Event {
                name: "betrayal".into(),
                when: Condition::Inside {
                    unit: "hero".into(),
                    region: "square".into(),
                },
                actions: vec![
                    Action::Team { player: 1, team: 1 },
                    Action::Objective {
                        line: "escape".into(),
                        status: Objective::Active,
                    },
                    Action::Checkpoint(1),
                ],
            },
            Event {
                name: "followup".into(),
                when: Condition::After {
                    event: "betrayal".into(),
                    ticks: 10,
                },
                actions: vec![
                    Action::Say {
                        line: "arc".into(),
                        ticks: 20,
                    },
                    Action::Checkpoint(2),
                ],
            },
            Event {
                name: "failure".into(),
                when: Condition::DeadUnit("hero".into()),
                actions: vec![Action::Lose],
            },
        ],
    }
}
fn arrive(world: &mut World) {
    let id = world.state.mission.as_ref().unwrap().units["hero"];
    let row = world.state.units.row(id).unwrap();
    world.state.units.pos[row] = FxVec2::from_ints(600, 600);
}
#[test]
fn betrayal_changes_only_the_pdf_and_death_loses() {
    let mut world = world(0);
    world.begin_mission(definition()).unwrap();
    assert!(!world.are_enemies(0, 1));
    arrive(&mut world);
    world.tick(&[]).unwrap();
    assert!(world.are_enemies(0, 1));
    assert!(!world.are_enemies(0, 2));
    assert_eq!(world.mission_status().unwrap().beat, 1);
    let id = world.state.mission.as_ref().unwrap().units["hero"];
    world
        .remove_unit_row(world.state.units.row(id).unwrap(), false)
        .unwrap();
    world.tick(&[]).unwrap();
    assert_eq!(world.mission_status().unwrap().outcome, Some(false));
}
#[test]
fn worker_counts_and_checkpoint_preserve_pending_waits_and_dialogue() {
    let mut expected = None;
    for workers in [0, 1, 3, 8] {
        let mut live = world(workers);
        live.begin_mission(definition()).unwrap();
        arrive(&mut live);
        for _ in 0..5 {
            live.tick(&[]).unwrap();
        }
        let bytes = live.snapshot();
        let mut resumed = world(workers);
        resumed.restore(terrain(), &bytes).unwrap();
        assert_eq!(live.hash(), resumed.hash());
        for _ in 0..35 {
            assert_eq!(live.tick(&[]).unwrap(), resumed.tick(&[]).unwrap());
        }
        assert_eq!(live.state.mission.as_ref().unwrap().fired.len(), 3);
        assert_eq!(live.mission_status().unwrap().beat, 2);
        match expected {
            Some(hash) => assert_eq!(hash, live.hash()),
            None => expected = Some(live.hash()),
        }
    }
}
#[test]
fn restored_mission_definitions_are_validated_before_spawn() {
    let mut source = world(0);
    source.begin_mission(definition()).unwrap();
    source
        .state
        .mission
        .as_mut()
        .unwrap()
        .definition
        .placed
        .units[0]
        .owner = 255;
    let bytes = source.snapshot();
    let mut target = world(0);
    assert!(target.restore(terrain(), &bytes).is_err());
    assert!(target.state.mission.is_none());
}
#[test]
fn bad_references_do_not_start_a_partial_mission() {
    let mut w = world(0);
    let mut d = definition();
    d.events[0].actions.push(Action::Spawn("typo".into()));
    assert!(w.begin_mission(d).is_err());
    assert!(w.state.mission.is_none());
    assert_eq!(w.state.units.slots.iter().count(), 0);
}
