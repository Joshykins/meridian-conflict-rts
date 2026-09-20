//! Radar detects without lighting the ground; unidentified contacts stay grey.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::{STATE_RADAR, STATE_UNIDENTIFIED};
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, RenderFrame, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "fog".into(),
        content_id: 1,
        deposits: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: true,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(owner: u8, key: &str, x: i32, w: &World) -> PlayerCommand {
    PlayerCommand {
        player: owner,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, 512),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    }
}

fn enemy<'a>(frame: &'a RenderFrame) -> &'a mc_sim::mirror::UnitInstance {
    frame
        .units
        .iter()
        .find(|u| u.owner_flags & 0xFF == 1)
        .expect("enemy on radar")
}

#[test]
fn radar_is_a_blip_until_vision_names_it() {
    let mut w = world();
    // Watchtower vision is 250 m; its radar is 1150 m. A tank at 800 m is a
    // contact, not a silhouette, and must not light the ground under it.
    w.tick(&[
        spawn(0, "aster_t1_radar", 512, &w),
        spawn(1, "aster_t1_tank", 1312, &w),
    ])
    .unwrap();

    let mut frame = RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    assert!(
        frame.fog.chunks_exact(2).all(|c| c[0] == 0 || c[0] == 255),
        "radar must not half-light fog"
    );
    let u = enemy(&frame);
    assert_eq!(
        u.owner_flags & (STATE_RADAR | STATE_UNIDENTIFIED),
        STATE_RADAR | STATE_UNIDENTIFIED
    );

    let row = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == 1)
        .unwrap();
    w.fog
        .identify(row, w.state.units.id(row).generation(), w.team_mask(0));
    w.write_render_frame(Some(0), &mut frame);
    let u = enemy(&frame);
    assert_eq!(
        u.owner_flags & (STATE_RADAR | STATE_UNIDENTIFIED),
        STATE_RADAR
    );
}
