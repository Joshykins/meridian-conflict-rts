//! Bytes from another machine (commands, snapshots) are untrusted: malformed
//! input is refused with an error, never a panic, and a snapshot whose tables do
//! not fit together is refused at restore rather than crashing a later tick.

use mc_core::{Angle, Fx, FxVec2, Rng};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const CELLS: u32 = 64;

fn terrain() -> Heightfield {
    Heightfield::flat(CELLS, CELLS, Fx::from_int(20))
}

fn world() -> World {
    let blueprints =
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let map = MapData {
        name: "small".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(100, 100), FxVec2::from_ints(400, 400)],
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
        players: vec![player("a", 0), player("b", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain(), map, Arc::new(blueprints), Arc::new(Pool::new(1)), &config)
        .unwrap()
}

/// A few tanks on each side, a few ticks in, so every table has rows.
fn populated() -> World {
    let mut w = world();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let spawns: Vec<_> = [(0u8, 150), (1u8, 350)]
        .into_iter()
        .map(|(player, at)| PlayerCommand {
            player,
            command: Command::DebugSpawn {
                owner: player,
                blueprint: tank,
                pos: FxVec2::from_ints(at, at),
                heading: Angle::ZERO,
                count: 4,
                flags: 0,
                build: 1000,
            },
        })
        .collect();
    w.tick(&spawns).unwrap();
    for _ in 0..20 {
        w.tick(&[]).unwrap();
    }
    w
}

#[test]
fn command_decoding_never_panics() {
    let mut rng = Rng::new(17);
    for _ in 0..5000 {
        let len = rng.below(96) as usize;
        let noise: Vec<u8> = (0..len).map(|_| rng.below(256) as u8).collect();
        let _ = Command::decode(&noise);
    }
    let valid = Command::Move {
        units: Vec::new(),
        target: FxVec2::from_ints(10, 10),
        queue: false,
    }
    .encode();
    for _ in 0..2000 {
        let mut bad = valid.clone();
        let i = rng.below(bad.len() as u32) as usize;
        bad[i] = rng.below(256) as u8;
        let _ = Command::decode(&bad);
    }
}

#[test]
fn a_corrupted_snapshot_is_refused_not_a_panic() {
    let mut source = populated();
    let blob = source.snapshot();
    let mut rng = Rng::new(3);
    for _ in 0..300 {
        let mut bad = blob.clone();
        for _ in 0..1 + rng.below(4) {
            let i = rng.below(bad.len() as u32) as usize;
            bad[i] = rng.below(256) as u8;
        }
        bad.truncate(bad.len() - rng.below(3) as usize);
        let mut w = world();
        // Whatever the bytes, restore answers; it never takes the process down.
        let _ = w.restore(terrain(), &bad);
    }
}

#[test]
fn a_snapshot_with_a_short_column_is_refused() {
    let mut source = populated();
    source.state.units.z.pop();
    let blob = source.snapshot();
    let mut w = world();
    let err = w.restore(terrain(), &blob).expect_err("refused");
    assert!(err.to_string().contains("units"), "{err}");
}

#[test]
fn a_snapshot_naming_a_blueprint_that_does_not_exist_is_refused() {
    let mut source = populated();
    let row = source.state.units.slots.iter().next().unwrap();
    source.state.units.blueprint[row] = mc_data::BlueprintId(u16::MAX);
    let blob = source.snapshot();
    let mut w = world();
    assert!(w.restore(terrain(), &blob).is_err());
}

#[test]
fn a_good_snapshot_still_restores() {
    let mut source = populated();
    let blob = source.snapshot();
    let mut w = world();
    w.restore(terrain(), &blob).unwrap();
    assert_eq!(w.hash(), source.hash());
}
