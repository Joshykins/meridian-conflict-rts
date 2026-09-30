//! Two big hulls sunk into each other by a bank (replay 20260929-215246 mark 2:
//! a carrier in a corner of the shallows and a rail trimaran in a notch below
//! it). Each was pushed away from the other onto ground it cannot stand on, so
//! every step either tried went nowhere: the crowd's push turned the carrier's
//! bow into the bank, and the push in every step it took kept it from ever
//! gathering way. Ordered off, both have to sail clear.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{Blueprints, MoveLayer};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const WATER: i32 = 20;
const CARRIER: &str = "aster_t3_carrier";
const TRIMARAN: &str = "aster_t3_rail_trimaran";

/// Land at 40 m. Under 20 m of water: a basin open to the east, with a corner
/// at its north-west, and a notch running south from its floor near the west end.
fn shallows() -> World {
    let mut samples = vec![40u16; 257 * 257];
    for y in 0..257 {
        for x in 0..257 {
            let (mx, my) = (x as i32 * 8, y as i32 * 8);
            let basin = mx >= 480 && (1030..=1120).contains(&my);
            let notch = (960..1030).contains(&my) && (mx - 530).abs() * 3 <= (my - 960) * 2;
            if basin || notch {
                samples[y * 257 + x] = 0;
            }
        }
    }
    let terrain =
        Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(WATER));
    let map = MapData {
        name: "shallows".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(150, 300), FxVec2::from_ints(150, 1700)],
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
        seed: 7,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn afloat(w: &World, key: &str, pos: FxVec2) -> bool {
    let bp = w.blueprints.id_of(key).unwrap();
    let class = w.blueprints.unit(bp).motion.unwrap().size_class;
    w.nav.passable(MoveLayer::Naval, class, pos)
}

/// From `pos`, as far along each of `ways` in turn (a metre at a time) as the hull still floats.
fn to_the_edge(w: &World, key: &str, mut pos: FxVec2, ways: &[FxVec2]) -> FxVec2 {
    for _ in 0..4 {
        for &way in ways {
            while afloat(w, key, pos + way) {
                pos = pos + way;
            }
        }
    }
    pos
}

fn spawn(w: &mut World, key: &str, pos: FxVec2) -> UnitId {
    let before: Vec<_> = w.state.units.slots.iter().collect();
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos,
        heading: Angle(0x4000),
        count: 1,
        flags: flag::PASSIVE,
        build: 1000,
    })])
    .unwrap();
    let row = w
        .state
        .units
        .slots
        .iter()
        .find(|r| !before.contains(r))
        .unwrap();
    w.state.units.id(row)
}

fn pos_of(w: &World, id: UnitId) -> FxVec2 {
    w.state.units.pos[w.state.units.row(id).unwrap()]
}

#[test]
fn hulls_pinned_against_a_bank_by_each_other_sail_clear() {
    let mut w = shallows();
    // The carrier in the basin's north-west corner, the trimaran down the
    // notch below and a little east of it, as far from the carrier as it floats.
    let carrier_at = to_the_edge(
        &w,
        CARRIER,
        FxVec2::from_ints(700, 1080),
        &[FxVec2::from_ints(-1, 0), FxVec2::from_ints(0, 1)],
    );
    let away = FxVec2::new(Fx::ratio(34, 100), Fx::ratio(-94, 100));
    let trimaran_at = to_the_edge(
        &w,
        TRIMARAN,
        FxVec2::from_ints(530, 1040),
        &[FxVec2::from_ints(0, -1), away],
    );
    let carrier = spawn(&mut w, CARRIER, carrier_at);
    let trimaran = spawn(&mut w, TRIMARAN, trimaran_at);
    for _ in 0..30 {
        w.tick(&[]).unwrap();
    }
    let (c, t) = (pos_of(&w, carrier), pos_of(&w, trimaran));
    let radii =
        [CARRIER, TRIMARAN].map(|k| w.blueprints.unit(w.blueprints.id_of(k).unwrap()).radius);
    assert!(
        c.distance(t) < radii[0] + radii[1],
        "the hulls should start sunk into each other: {c:?} {t:?}"
    );

    let goal = FxVec2::from_ints(1700, 1075);
    w.tick(&[cmd(Command::Move {
        units: vec![carrier, trimaran],
        target: goal,
        queue: false,
    })])
    .unwrap();
    for _ in 0..1200 {
        w.tick(&[]).unwrap();
    }
    for (name, id) in [("carrier", carrier), ("trimaran", trimaran)] {
        let at = pos_of(&w, id);
        assert!(
            at.distance(goal) < Fx::from_int(300),
            "the {name} never sailed clear: still at {at:?}, the goal at {goal:?}"
        );
    }
}
