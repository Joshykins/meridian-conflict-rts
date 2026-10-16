//! A site turned while placing (`Command::Build`'s heading): the sim snaps the heading
//! to a quarter turn of `build_heading`, and the lot turns with it, sides swapped.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

/// Raised on a 28 x 13 lot, lying along it (east) unturned.
const SHIP: &str = "aster_t3_lift_ship";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        doodads: Default::default(),
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(3500, 3500)],
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
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

#[test]
fn a_turned_site_built_unit_takes_its_lot_turned() {
    let mut w = world();
    let bp = w.blueprints.id_of(SHIP).unwrap();
    let ship_bp = w.blueprints.unit(bp).clone();
    assert_eq!(ship_bp.footprint, (28, 13));
    assert_eq!(ship_bp.build_heading(), Angle::ZERO);
    let power = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t1_power").unwrap())
        .clone();
    let engineer = w
        .spawn_unit(
            w.blueprints.id_of("aster_t3_engineer").unwrap(),
            0,
            FxVec2::from_ints(1300, 1500),
            Angle::ZERO,
            true,
        )
        .unwrap();
    // Not a quarter turn: the sim snaps it to the nearest one.
    w.tick(&[cmd(Command::Build {
        units: vec![w.state.units.id(engineer)],
        blueprint: bp,
        pos: FxVec2::from_ints(1500, 1500),
        heading: Angle::from_degrees(80),
        queue: false,
    })])
    .unwrap();
    let ship = (0..900)
        .find_map(|_| {
            let found = (w.state.units.slots.iter()).find(|&r| w.state.units.blueprint[r] == bp);
            if found.is_none() {
                w.tick(&[]).unwrap();
            }
            found
        })
        .expect("the engineer never began the site");
    assert_eq!(w.state.units.heading[ship], Angle::QUARTER_TURN);
    let at = w.state.units.pos[ship];
    // The lot is 13 x 28 now: 78 m to each side across, 168 m along north and south.
    let north = at + FxVec2::from_ints(0, 130);
    let east = at + FxVec2::from_ints(130, 0);
    assert!(!w.can_place(&power, north), "the turned lot reaches north");
    assert!(w.can_place(&power, east), "and no longer east");

    let ship_id = w.state.units.id(ship);
    w.tick(&[cmd(Command::DebugFreeBuild {
        player: 0,
        on: true,
    })])
    .unwrap();
    w.tick(&[cmd(Command::DebugSetBuild {
        units: vec![ship_id],
        permille: 999,
    })])
    .unwrap();
    for _ in 0..400 {
        if !w.state.units.has_flag(ship, flag::UNDER_CONSTRUCTION) {
            break;
        }
        w.tick(&[]).unwrap();
    }
    assert!(!w.state.units.has_flag(ship, flag::UNDER_CONSTRUCTION));
    assert!(
        w.can_place(&power, north),
        "finished, it gives the turned lot back"
    );
}
