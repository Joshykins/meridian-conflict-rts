//! A mass extractor is refitted in place, like the commander: the next kit is
//! built onto the wellhead, and the unit stays the same.

use mc_core::{Angle, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, mc_core::Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        deposits: vec![FxVec2::from_ints(512, 512)],
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
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn(w: &World, key: &str) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(512, 512),
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build: 1000,
    })
}

fn with_extractor() -> (World, UnitId) {
    let mut w = world();
    w.tick(&[
        spawn(&w, "aster_t1_extractor"),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let id = w.state.units.id(w.state.units.slots.iter().next().unwrap());
    (w, id)
}

#[test]
fn an_extractor_pours_a_well_pad() {
    let (w, _) = with_extractor();
    assert_eq!(w.state.pads.len(), 1);
    assert_ne!(
        w.state.pads.packed[0] & mc_sim::PAD_WELL,
        0,
        "the lot is a well, not a solid slab"
    );
}

#[test]
fn an_extractor_is_refitted_in_place_and_stays_the_same_unit() {
    let (mut w, mex) = with_extractor();
    let (t1, t2) = (
        w.blueprints.id_of("aster_t1_extractor").unwrap(),
        w.blueprints.id_of("aster_t2_extractor").unwrap(),
    );
    let row = w.state.units.row(mex).unwrap();

    w.tick(&[cmd(Command::Upgrade { units: vec![mex] })])
        .unwrap();
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(OrderKind::Upgrade),
        "the refit is an order in its queue"
    );

    let mut shown = 0.0f32;
    let mut frame = mc_sim::RenderFrame::default();
    let done = (0..2000).any(|_| {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        assert_eq!(
            frame.units.len(),
            1,
            "the refit is never drawn as a second unit"
        );
        let u = &frame.units[0];
        assert_eq!(
            u.owner_flags & (flag::UNDER_CONSTRUCTION as u32) << 8,
            0,
            "the wellhead is not rebuilt from the weld"
        );
        shown = shown.max(u.upgrade);
        w.state.units.blueprint[row] == t2
    });
    assert!(done, "the refit never finished");
    assert!(
        shown > 0.9,
        "the mirror reported the refit's progress ({shown})"
    );
    assert_eq!(w.state.units.row(mex), Some(row), "same unit, same id");
    assert_eq!(w.state.units.blueprint[row], t2, "it is now the next tier");
    assert_eq!(
        w.state.units.slots.iter().count(),
        1,
        "nothing is left behind"
    );
    w.write_render_frame(None, &mut frame);
    assert_eq!(frame.units[0].upgrade, 0.0);
    assert_eq!(w.blueprints.unit(t1).visual.mesh, "extractor");
}
