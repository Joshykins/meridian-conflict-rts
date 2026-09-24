//! Trees in the way: cleared off a lot before building, knocked over by big walkers.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{Heightfield, Prop, PropKind};
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn tree(x: i32, y: i32) -> Prop {
    Prop {
        kind: PropKind::TreeConifer,
        pos: FxVec2::from_ints(x, y),
        heading: Angle::ZERO,
        scale_milli: 1000,
    }
}

fn world(props: Vec<Prop>) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "trees".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
        props,
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

fn spawn(w: &mut World, key: &str, x: i32, y: i32) -> UnitId {
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, y),
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build: 1000,
    })])
    .unwrap();
    let u = &w.state.units;
    u.id(u.slots.iter().last().unwrap())
}

#[test]
fn a_lot_is_cleared_of_trees_before_the_structure_goes_down() {
    // A factory lot (96 m) at 600,516 full of trees, one just off it, one far away.
    let mut props: Vec<Prop> = (0..8)
        .flat_map(|i| (0..8).map(move |j| tree(560 + i * 11, 478 + j * 11)))
        .collect();
    // Just past the lot's edge (48 m), under where the crown reaches.
    props.push(tree(600, 565));
    props.push(tree(800, 800));
    let lot = props.len() - 2;
    let mut w = world(props);
    let commander = spawn(&mut w, "aster_commander", 600, 420);
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    w.tick(&[cmd(Command::Build {
        units: vec![commander],
        blueprint: factory,
        pos: FxVec2::from_ints(600, 516),
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();

    let mass = w.state.players[0].reclaimed_mass;
    // Tick each wave came, and the wave each tree went in.
    let mut waves: Vec<(u8, u32)> = Vec::new();
    let mut went = vec![None; lot + 2];
    let mut site_at = None;
    for t in 0..600u32 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::LotClearing { wave, pos, .. } => {
                    assert_eq!(*pos, FxVec2::from_ints(600, 516));
                    waves.push((*wave, t));
                }
                SimEvent::TreeVaporized { prop, .. } => {
                    assert!(went[*prop as usize].is_none(), "a tree goes once");
                    went[*prop as usize] = waves.last().map(|w| w.0);
                }
                _ => {}
            }
        }
        if waves
            .last()
            .is_some_and(|w| w.0 < mc_sim::trees::CLEAR_WAVES)
        {
            assert_eq!(
                w.reclaims.len(),
                1,
                "one beam feeds the field, not one per tree"
            );
        }
        let standing = (0..=lot).filter(|&p| w.is_prop_alive(p)).count();
        if w.state
            .units
            .slots
            .iter()
            .any(|r| w.state.units.blueprint[r] == factory)
        {
            assert_eq!(
                standing, 0,
                "no tree is left standing when the site goes down"
            );
            site_at = Some(t);
            break;
        }
    }
    let site = site_at.expect("the factory was started");
    let last = mc_sim::trees::CLEAR_WAVES;
    let steps: Vec<u8> = waves.iter().map(|w| w.0).collect();
    assert_eq!(
        steps,
        (0..=last).collect::<Vec<_>>(),
        "the field goes up, then every wave in turn"
    );
    for pair in waves.windows(2) {
        let gap = pair[1].1 - pair[0].1;
        assert!(
            gap >= 7 && gap <= 8,
            "waves come steadily, not all at once ({gap} ticks)"
        );
    }
    assert!(
        site - waves.last().unwrap().1 <= 3,
        "the site follows the last wave"
    );
    // Waves run out from the middle: the middle trees go first, the corners last.
    let middle = went[4 * 8 + 3].expect("the middle tree went");
    let corner = went[0].expect("the corner tree went");
    assert!(
        middle < corner,
        "wave {middle} took the middle, {corner} the corner"
    );
    assert_eq!(
        went[lot],
        Some(last),
        "the overhanging tree went with the last wave"
    );
    assert!(went[..=lot].iter().all(Option::is_some));
    assert!(w.is_prop_alive(lot + 1), "trees elsewhere stay");
    assert_eq!(
        w.state.players[0].reclaimed_mass, mass,
        "trees are worth nothing"
    );
}

#[test]
fn a_commander_knocks_over_the_trees_it_walks_through() {
    let props: Vec<Prop> = (0..10).map(|i| tree(600 + i * 12, 512)).collect();
    let mut w = world(props);
    let commander = spawn(&mut w, "aster_commander", 560, 512);
    w.tick(&[cmd(Command::Move {
        units: vec![commander],
        target: FxVec2::from_ints(760, 512),
        queue: false,
    })])
    .unwrap();
    let mut trampled = 0;
    for _ in 0..300 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::TreeTrampled { prop, motion, .. } = e {
                assert!(!w.is_prop_alive(*prop as usize));
                assert!(motion.x > Fx::ZERO, "it falls the way the commander walks");
                trampled += 1;
            }
        }
    }
    assert_eq!(trampled, 10, "every tree along its path went down");
}

#[test]
fn a_tank_leaves_trees_standing() {
    let props: Vec<Prop> = (0..10).map(|i| tree(600 + i * 12, 512)).collect();
    let mut w = world(props);
    let tank = spawn(&mut w, "aster_t1_tank", 560, 512);
    w.tick(&[cmd(Command::Move {
        units: vec![tank],
        target: FxVec2::from_ints(760, 512),
        queue: false,
    })])
    .unwrap();
    for _ in 0..300 {
        w.tick(&[]).unwrap();
        assert!(!w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::TreeTrampled { .. })));
    }
    assert!((0..10).all(|p| w.is_prop_alive(p)));
}
