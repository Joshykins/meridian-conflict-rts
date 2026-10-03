//! Area reclaim: sent to a point, a reclaimer clears the wrecks it passes; given a
//! circle, it clears every wreck inside it and leaves those outside.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const ENGINEER: &str = "aster_t1_engineer";
const TANK: &str = "aster_t1_tank";
const RECLAIMER: &str = "aster_t1_mobile_reclaimer";

fn world() -> World {
    world_on(Heightfield::flat(256, 256, Fx::from_int(20)))
}

fn world_on(terrain: Heightfield) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "reclaim_area".into(),
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
        seed: 5,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // Somewhere to put the mass: plenty of it.
    w.tick(&[cmd(Command::DebugStorage {
        player: 0,
        mass: 100_000,
        energy: 0,
    })])
    .unwrap();
    w
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn engineers(w: &mut World, at: &[(i32, i32)]) -> Vec<UnitId> {
    spawn(w, ENGINEER, at)
}

fn spawn(w: &mut World, key: &str, at: &[(i32, i32)]) -> Vec<UnitId> {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let spawns: Vec<PlayerCommand> = at
        .iter()
        .map(|&(x, y)| {
            cmd(Command::DebugSpawn {
                owner: 0,
                blueprint,
                pos: FxVec2::from_ints(x, y),
                heading: Angle::ZERO,
                count: 1,
                flags: 0,
                build: 1000,
            })
        })
        .collect();
    w.tick(&spawns).unwrap();
    let u = &w.state.units;
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == 0 && u.blueprint[r] == blueprint)
        .map(|r| u.id(r))
        .collect()
}

/// `count` tank wrecks in a loose block about `(x, y)`.
fn wrecks(w: &mut World, x: i32, y: i32, count: u16) {
    let blueprint = w.blueprints.id_of(TANK).unwrap();
    w.tick(&[cmd(Command::DebugWrecks {
        blueprint,
        pos: FxVec2::from_ints(x, y),
        count,
    })])
    .unwrap();
}

/// Where the live wrecks lie.
fn wrecks_left(w: &World) -> Vec<FxVec2> {
    let wr = &w.state.wrecks;
    wr.slots.iter().map(|r| wr.pos[r]).collect()
}

/// Ticks until none of `units` has an order left. Panics when that never happens.
fn run_until_idle(w: &mut World, units: &[UnitId], most: u32) {
    for _ in 0..most {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        if units.iter().all(|&id| {
            u.row(id)
                .is_none_or(|r| w.state.orders.front(u, r).is_none())
        }) {
            return;
        }
    }
    panic!("the reclaim order was still going after {most} ticks");
}

#[test]
fn a_point_order_reclaims_the_wrecks_along_the_way_and_stops_there() {
    let mut w = world();
    let units = engineers(&mut w, &[(300, 512)]);
    // One just off the way, one well away from it.
    wrecks(&mut w, 600, 530, 1);
    wrecks(&mut w, 600, 850, 1);
    w.tick(&[cmd(Command::ReclaimArea {
        units: units.clone(),
        pos: FxVec2::from_ints(900, 512),
        radius: Fx::ZERO,
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &units, 4000);

    let left = wrecks_left(&w);
    assert_eq!(left.len(), 1, "only the far wreck is left: {left:?}");
    assert!(left[0].y > Fx::from_int(800));
    let row = w.state.units.row(units[0]).unwrap();
    let end = w.state.units.pos[row];
    assert!(
        end.distance(FxVec2::from_ints(900, 512)) < Fx::from_int(20),
        "it went on to the point: {end:?}"
    );
}

#[test]
fn a_circle_is_cleared_and_what_lies_outside_it_is_left() {
    let mut w = world();
    let units = engineers(&mut w, &[(300, 500), (300, 540), (300, 580)]);
    let centre = FxVec2::from_ints(900, 700);
    let radius = Fx::from_int(120);
    wrecks(&mut w, 900, 700, 9);
    wrecks(&mut w, 900, 1000, 1);
    let inside = wrecks_left(&w)
        .iter()
        .filter(|p| p.distance(centre) <= radius)
        .count();
    assert_eq!(inside, 9, "the block lies inside the circle");
    w.tick(&[cmd(Command::ReclaimArea {
        units: units.clone(),
        pos: centre,
        radius,
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &units, 6000);

    let left = wrecks_left(&w);
    assert_eq!(left.len(), 1, "every wreck in the circle is gone: {left:?}");
    assert!(left[0].distance(centre) > radius);
}

/// Replay 20261001-202046 mark 1: a mobile reclaimer given a circle whose middle lies on a
/// mesa it cannot climb went for the wrecks below it, but was given up on each at
/// once: the walk to the middle had failed, and the failure stuck to the reclaim that
/// was put in front of it. It sat still, swapping between two wrecks for ever.
#[test]
fn a_circle_whose_middle_cannot_be_reached_is_still_cleared() {
    // Flat ground at 20 m, and a sheer-sided mesa 180 m above it, 800-1120 m.
    let mut samples = vec![20u16; 257 * 257];
    for y in 100..=140 {
        for x in 100..=140 {
            samples[y * 257 + x] = 200;
        }
    }
    let terrain = Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::ZERO);
    let mut w = world_on(terrain);
    let units = spawn(&mut w, RECLAIMER, &[(300, 960)]);
    let centre = FxVec2::from_ints(960, 960);
    // Inside the circle, past the Gleaner's 550 m reach.
    wrecks(&mut w, 400, 1650, 3);
    // It has already found there is no way up.
    w.tick(&[cmd(Command::Move {
        units: units.clone(),
        target: centre,
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &units, 1000);
    w.tick(&[cmd(Command::ReclaimArea {
        units: units.clone(),
        pos: centre,
        radius: Fx::from_int(1000),
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &units, 4000);

    let left = wrecks_left(&w);
    assert!(
        left.is_empty(),
        "the wrecks below the mesa are taken: {left:?}"
    );
}

#[test]
fn a_queued_area_reclaim_waits_its_turn() {
    let mut w = world();
    let units = engineers(&mut w, &[(300, 512)]);
    wrecks(&mut w, 500, 900, 1);
    w.tick(&[cmd(Command::Move {
        units: units.clone(),
        target: FxVec2::from_ints(700, 512),
        queue: false,
    })])
    .unwrap();
    w.tick(&[cmd(Command::ReclaimArea {
        units: units.clone(),
        pos: FxVec2::from_ints(500, 900),
        radius: Fx::from_int(60),
        queue: true,
    })])
    .unwrap();
    let row = w.state.units.row(units[0]).unwrap();
    let kinds: Vec<_> = w
        .state
        .orders
        .iter(&w.state.units, row)
        .map(|o| o.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            mc_sim::tables::OrderKind::Move,
            mc_sim::tables::OrderKind::ReclaimArea
        ]
    );
    run_until_idle(&mut w, &units, 4000);
    assert!(wrecks_left(&w).is_empty());
}

/// A Gleaner at `(x, y)`: the ARC mobile reclaimer, whose beam reaches 550 m.
fn reclaimer(w: &mut World, x: i32, y: i32) -> UnitId {
    spawn(w, RECLAIMER, &[(x, y)])[0]
}

/// The mass left in the wreck nearest `(x, y)`, zero once it is gone.
fn mass_near(w: &World, x: i32, y: i32) -> Fx {
    let at = FxVec2::from_ints(x, y);
    let wr = &w.state.wrecks;
    wr.slots
        .iter()
        .filter(|&r| wr.pos[r].distance(at) < Fx::from_int(30))
        .map(|r| wr.mass[r])
        .sum()
}

#[test]
fn a_reclaimer_given_a_circle_wider_than_its_reach_clears_all_of_it() {
    let mut w = world();
    let reclaimer = reclaimer(&mut w, 1000, 200);
    let centre = FxVec2::from_ints(1000, 1000);
    // Farther apart than its beam's 550 m, and none within reach of the middle.
    let fields = [(150, 1000), (1850, 1000), (1000, 1850)];
    for (x, y) in fields {
        wrecks(&mut w, x, y, 2);
    }
    w.tick(&[cmd(Command::ReclaimArea {
        units: vec![reclaimer],
        pos: centre,
        radius: Fx::from_int(900),
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &[reclaimer], 6000);
    assert!(wrecks_left(&w).is_empty(), "left: {:?}", wrecks_left(&w));
}

#[test]
fn a_reclaimer_sent_to_a_point_stops_for_the_wrecks_on_its_way() {
    let mut w = world();
    let reclaimer = reclaimer(&mut w, 300, 1000);
    // On the way, and a little off it.
    let fields = [(700, 1000), (1100, 1100)];
    for (x, y) in fields {
        wrecks(&mut w, x, y, 3);
    }
    let goal = FxVec2::from_ints(1700, 1000);
    w.tick(&[cmd(Command::ReclaimArea {
        units: vec![reclaimer],
        pos: goal,
        radius: Fx::ZERO,
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &[reclaimer], 4000);
    let row = w.state.units.row(reclaimer).unwrap();
    let at = w.state.units.pos[row];
    assert!(
        at.distance(goal) < Fx::from_int(30),
        "it did not go on to the point: at {at:?}"
    );
    for (x, y) in fields {
        assert_eq!(
            mass_near(&w, x, y),
            Fx::ZERO,
            "the field at ({x}, {y}) was passed by"
        );
    }
}

#[test]
fn a_reclaimer_told_to_reclaim_a_wreck_drops_its_move_and_stops_in_reach() {
    let mut w = world();
    let reclaimer = reclaimer(&mut w, 300, 1000);
    w.tick(&[cmd(Command::Move {
        units: vec![reclaimer],
        target: FxVec2::from_ints(1800, 1000),
        queue: false,
    })])
    .unwrap();
    for _ in 0..40 {
        w.tick(&[]).unwrap();
    }
    wrecks(&mut w, 1000, 1600, 1);
    let wreck = {
        let wr = &w.state.wrecks;
        let r = wr.slots.iter().next().unwrap();
        wr.slots.handle(r)
    };
    w.tick(&[cmd(Command::ReclaimWreck {
        units: vec![reclaimer],
        wreck,
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &[reclaimer], 3000);
    assert!(wrecks_left(&w).is_empty());
    let row = w.state.units.row(reclaimer).unwrap();
    let at = w.state.units.pos[row];
    let reach = w
        .blueprints
        .unit(w.state.units.blueprint[row])
        .reclaimer
        .unwrap()
        .range;
    assert!(
        at.distance(FxVec2::from_ints(1000, 1600)) <= reach,
        "it went on instead of stopping in reach of the wreck: at {at:?}"
    );
    assert!(
        at.x < Fx::from_int(1300),
        "it kept on to where it was first sent: at {at:?}"
    );
}

/// Replay 20261001-172856 mark 1: a reclaimer given a circle of wrecks its side had seen
/// but had no eyes on any more ended the order where it stood. The player is shown
/// wrecks anywhere explored, so the reclaimer goes for those too.
#[test]
fn a_reclaimer_given_a_circle_out_of_sight_goes_to_the_wrecks_it_knows_of() {
    let mut w = world();
    w.state.fog_enabled = true;
    let reclaimer = reclaimer(&mut w, 300, 300);
    let centre = FxVec2::from_ints(1700, 1700);
    wrecks(&mut w, 1700, 1700, 3);
    // Seen once, then left behind under the fog.
    let mask = w.team_mask(0);
    w.fog.reveal(centre, Fx::from_int(300), Fx::ZERO, mask);
    w.tick(&[]).unwrap();
    assert!(w.fog.is_explored(centre, mask) && !w.fog.is_detected(centre, mask));
    w.tick(&[cmd(Command::ReclaimArea {
        units: vec![reclaimer],
        pos: centre,
        radius: Fx::from_int(200),
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &[reclaimer], 6000);
    assert!(wrecks_left(&w).is_empty(), "left: {:?}", wrecks_left(&w));
}
