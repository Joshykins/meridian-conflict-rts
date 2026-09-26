//! Radar detects without lighting the ground; unidentified contacts stay grey.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::{STATE_RADAR, STATE_UNIDENTIFIED, STATE_UNPOWERED};
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
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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

fn enemy(frame: &RenderFrame) -> &mc_sim::mirror::UnitInstance {
    frame
        .units
        .iter()
        .find(|u| u.owner_flags & 0xFF == 1)
        .expect("enemy on radar")
}

#[test]
fn radar_is_a_blip_until_vision_names_it() {
    let mut w = world();
    w.state.players[0].free_build = true;
    // Watchtower vision is 250 m; its radar is 2000 m. A tank at 800 m is a
    // contact, not a silhouette, and must not light the ground under it.
    w.tick(&[
        spawn(0, "aster_t1_radar", 512, &w),
        spawn(1, "aster_t1_tank", 1312, &w),
    ])
    .unwrap();

    let mut frame = RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    assert!(
        frame
            .fog
            .as_chunks::<2>()
            .0
            .iter()
            .all(|c| c[0] == 0 || c[0] == 255),
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

#[test]
fn a_radar_goes_dark_when_energy_stalls() {
    let mut w = world();
    w.state.players[0].free_build = true;
    w.tick(&[
        spawn(0, "aster_t1_radar", 512, &w),
        spawn(1, "aster_t1_tank", 1312, &w),
    ])
    .unwrap();

    let mut frame = RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    enemy(&frame);
    let tower = frame
        .units
        .iter()
        .find(|u| u.owner_flags & 0xFF == 0)
        .expect("watchtower");
    assert_eq!(tower.owner_flags & STATE_UNPOWERED, 0);

    w.state.players[0].free_build = false;
    w.tick(&[]).unwrap();
    w.write_render_frame(Some(0), &mut frame);
    assert!(
        frame.units.iter().all(|u| u.owner_flags & 0xFF != 1),
        "a dark tower must not paint the tank"
    );
    let tower = frame
        .units
        .iter()
        .find(|u| u.owner_flags & 0xFF == 0)
        .expect("watchtower");
    assert_ne!(
        tower.owner_flags & STATE_UNPOWERED,
        0,
        "the dish should stop while the grid is dry"
    );
}

fn wrecks(frame: &RenderFrame) -> Vec<f32> {
    frame
        .units
        .iter()
        .filter(|u| u.owner_flags & mc_sim::mirror::KIND_WRECK != 0)
        .map(|u| u.pos[0])
        .collect()
}

#[test]
fn wrecks_stay_on_the_map_under_explored_fog() {
    let mut w = world();
    w.state.players[0].free_build = true;
    // One wreck in ground the watchtower has seen, one far off in ground it never has.
    w.tick(&[
        spawn(0, "aster_t1_radar", 512, &w),
        spawn(0, "aster_t1_tank", 700, &w),
        spawn(1, "aster_t1_tank", 1800, &w),
    ])
    .unwrap();
    let ids: Vec<_> = w
        .state
        .units
        .slots
        .iter()
        .map(|r| (w.state.units.owner[r], w.state.units.id(r)))
        .collect();
    let tanks = |owner: u8| {
        ids.iter()
            .filter(|(o, _)| *o == owner)
            .map(|&(_, id)| id)
            .collect::<Vec<_>>()
    };
    let tank0: Vec<_> = tanks(0)
        .into_iter()
        .filter(|&id| {
            let r = w.state.units.row(id).unwrap();
            w.blueprints.unit(w.state.units.blueprint[r]).key == "aster_t1_tank"
        })
        .collect();
    w.tick(&[
        PlayerCommand {
            player: 0,
            command: Command::SelfDestruct { units: tank0 },
        },
        PlayerCommand {
            player: 1,
            command: Command::SelfDestruct { units: tanks(1) },
        },
    ])
    .unwrap();
    for _ in 0..5 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(w.state.wrecks.slots.live(), 2, "both tanks leave wrecks");

    // Blind the viewer: every eye it had is gone, the ground it saw stays explored.
    let own: Vec<_> = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.owner[r] == 0)
        .map(|r| w.state.units.id(r))
        .collect();
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::SelfDestruct { units: own },
    }])
    .unwrap();
    for _ in 0..5 {
        w.tick(&[]).unwrap();
    }
    let mut frame = RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    let seen = wrecks(&frame);
    assert!(
        seen.iter().any(|&x| (x - 700.0).abs() < 1.0),
        "explored wreck shows in fog: {seen:?}"
    );
    assert!(
        seen.iter().all(|&x| x < 1200.0),
        "unexplored wreck stays hidden: {seen:?}"
    );
}
