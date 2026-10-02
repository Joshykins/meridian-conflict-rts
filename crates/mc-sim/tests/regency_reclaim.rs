//! The Regency take apart with nanites as they build (docs/STYLE.md "The Regency suite"):
//! their reclaim beams are written as the nanite stream's kind, ARC's as the salvage beam.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::reclaim::{is_reclaim, BEAM_NANITE_RECLAIM, BEAM_RECLAIM};
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, RenderFrame, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const BREAKER: &str = "regency_t1_land_reclaimer";
const GLEANER: &str = "aster_t1_land_reclaimer";
const TANK: &str = "aster_t1_tank";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "regency reclaim".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![
            FxVec2::from_ints(512, 512),
            FxVec2::from_ints(512, 1500),
            FxVec2::from_ints(1500, 1500),
        ],
        props: Vec::new(),
    };
    let player = |faction: &str, team, start| PlayerSetup {
        name: faction.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start,
    };
    let config = MatchConfig {
        seed: 3,
        players: vec![
            player("Regency", 0, 0),
            player("Aster", 0, 1),
            player("Aster", 1, 2),
        ],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, y: i32, flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            count: 1,
            flags,
            build: 1000,
        },
    }
}

fn ids(w: &World, owner: u8, key: &str) -> Vec<UnitId> {
    let (u, bp) = (&w.state.units, w.blueprints.id_of(key).unwrap());
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == owner && u.blueprint[r] == bp)
        .map(|r| u.id(r))
        .collect()
}

#[test]
fn a_regency_salvager_takes_a_wreck_apart_with_a_nanite_stream() {
    let mut w = world();
    // Somewhere for each side to put the mass, a salvager each, and a wreck in front of
    // each salvager.
    w.tick(&[
        spawn(&w, 0, "regency_storage", 300, 512, 0),
        spawn(&w, 1, "aster_mass_storage", 300, 1500, 0),
        spawn(&w, 0, BREAKER, 500, 512, 0),
        spawn(&w, 1, GLEANER, 500, 1500, 0),
        spawn(&w, 2, TANK, 560, 512, flag::PASSIVE),
        spawn(&w, 2, TANK, 560, 1500, flag::PASSIVE),
    ])
    .unwrap();
    let tanks = ids(&w, 2, TANK);
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::DebugDamage {
            units: tanks,
            permille: 1000,
        },
    }])
    .unwrap();
    let mut frame = RenderFrame::default();
    let mut seen = Vec::new();
    for _ in 0..60 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        seen = frame.beams.iter().map(|b| b.kind).collect::<Vec<_>>();
        seen.sort_unstable();
        if seen.len() == 2 {
            break;
        }
    }
    assert_eq!(seen, vec![BEAM_RECLAIM, BEAM_NANITE_RECLAIM]);
    assert!(seen.iter().all(|&k| is_reclaim(k)));
    assert!(
        w.state.players[0].reclaimed_mass > Fx::ZERO,
        "the Breaker is paid"
    );
}
