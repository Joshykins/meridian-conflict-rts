//! A stream gun (`Weapon::rounds`) is drawn as several rounds per simulated
//! shot, and the rounds still in the air when the shot lands fly on.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, RenderFrame, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
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
        players: vec![player("one", 0), player("two", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let map = MapData {
        name: "flat".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(256, 512), FxVec2::from_ints(768, 512)],
        props: Vec::new(),
    };
    let terrain = Heightfield::flat(128, 128, Fx::from_int(20));
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: owner,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, 512),
            heading: if owner == 0 {
                Angle::ZERO
            } else {
                Angle::HALF_TURN
            },
            count: 1,
            flags,
            build: 1000,
        },
    }
}

#[test]
fn a_stream_gun_is_drawn_as_its_rounds() {
    let mut w = world();
    let commander = w.blueprints.id_of("aster_commander").unwrap();
    let rounds = w.blueprints.unit(commander).weapons[0].rounds as usize;
    assert!(rounds > 1, "the commander's machine gun is a stream gun");
    let setup = vec![
        spawn(&w, 0, "aster_commander", 300, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            500,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ];
    w.tick(&setup).unwrap();

    let mut frame = RenderFrame::default();
    let (mut most_drawn, mut tails, mut starting) = (0, false, false);
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        let shots = (0..w.state.projectiles.len())
            .filter(|&i| w.state.projectiles.blueprint[i] == commander)
            .count();
        let drawn = frame.projectiles.len();
        assert!(drawn <= shots * rounds + w.streams.len() * rounds + w.spent.len());
        most_drawn = most_drawn.max(drawn);
        tails |= !w.streams.is_empty();
        starting |= frame
            .projectiles
            .iter()
            .any(|p| p.color >> mc_sim::mirror::PROJECTILE_STARTS_SHIFT != 0);
    }
    assert!(most_drawn >= rounds, "drew at most {most_drawn} rounds");
    assert!(tails, "rounds in the air when a shot lands fly on");
    assert!(
        starting,
        "rounds leave the muzzle part of the way through a tick"
    );
}
