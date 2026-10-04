//! City structures (`mc_sim::city`): shots meet the blocks in their way, blasts
//! reach the blocks round them, a hurt block burns down to a gutted shell, one
//! that runs out of health comes down and opens its ground, and guns do not fire
//! into a wall with their target behind it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{Heightfield, Prop, PropKind};
use mc_sim::city::{BURNING, DOWN, GUTTED};
use mc_sim::tables::{flag, Controller, FireState};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const CELLS: u32 = 256;

fn prop(kind: PropKind, x: i32, y: i32, heading: Angle, wear_milli: u16) -> Prop {
    Prop {
        kind,
        pos: FxVec2::from_ints(x, y),
        heading,
        scale_milli: 1000,
        wear_milli,
    }
}

/// A house across the line from x 500 to x 660 at y 512: its walls run x 572..588,
/// y 506..518, 9 m up.
fn house(wear_milli: u16) -> Prop {
    prop(PropKind::CityHouse, 580, 512, Angle::ZERO, wear_milli)
}

fn terrain() -> Heightfield {
    let n = CELLS as usize + 1;
    Heightfield::from_samples(
        CELLS,
        CELLS,
        vec![20 * 64; n * n],
        Fx::ZERO,
        Fx::ratio(1, 64),
        Fx::ZERO,
    )
}

fn map(props: Vec<Prop>) -> MapData {
    MapData {
        name: "city".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
        props,
    }
}

/// Flat ground at 20 m with `props` on it.
fn world(props: Vec<Prop>) -> World {
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
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        terrain(),
        map(props),
        blueprints,
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, y: i32, flags: u16) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, y),
        heading: Angle::ZERO,
        count: 1,
        flags,
        build: 1000,
    })
}

fn ids_of(w: &World, owner: u8) -> Vec<UnitId> {
    let u = &w.state.units;
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == owner)
        .map(|r| u.id(r))
        .collect()
}

fn attack_ground(w: &World, x: i32, y: i32) -> PlayerCommand {
    cmd(Command::AttackGround {
        units: ids_of(w, 0),
        pos: FxVec2::from_ints(x, y),
        queue: false,
    })
}

fn struck(w: &World) -> Vec<u32> {
    w.events
        .iter()
        .filter_map(|e| match e {
            SimEvent::Impact { on_structure, .. } => *on_structure,
            _ => None,
        })
        .collect()
}

/// Ticks until `done` holds, at most `limit`.
fn run_until(w: &mut World, limit: u32, mut done: impl FnMut(&World) -> bool) -> bool {
    for _ in 0..limit {
        w.tick(&[]).unwrap();
        if done(w) {
            return true;
        }
    }
    false
}

fn cell_of(x: i32, y: i32) -> (u32, u32) {
    let c = mc_map::CELL_SIZE_M;
    ((x / c) as u32, (y / c) as u32)
}

fn walkable(w: &World, x: i32, y: i32) -> bool {
    let c = cell_of(x, y);
    w.nav.no_blockers(c, c)
}

#[test]
fn a_shot_at_the_ground_behind_a_house_hits_the_house() {
    let mut w = world(vec![house(0)]);
    w.tick(&[spawn(&w, 0, "aster_t1_tank", 500, 512, 0)])
        .unwrap();
    w.tick(&[attack_ground(&w, 660, 512)]).unwrap();
    let mut hits = Vec::new();
    assert!(run_until(&mut w, 200, |w| {
        hits.extend(struck(w));
        hits.len() >= 3
    }));
    assert!(hits.iter().all(|&p| p == 0), "{hits:?}");
    let s = &w.state.city;
    assert!(s.health[0] < s.max[0], "the house took the shots");
    assert_eq!(s.touched, vec![0]);
    // Ground beyond it was never struck: every impact so far was on the house.
    let ground = w
        .events
        .iter()
        .filter(|e| {
            matches!(e, SimEvent::Impact { on_structure: None, on_unit: false, pos, .. }
                if pos.x > Fx::from_int(590))
        })
        .count();
    assert_eq!(ground, 0);
}

#[test]
fn attack_ground_on_a_point_inside_a_house_hits_it() {
    let mut w = world(vec![house(0)]);
    w.tick(&[spawn(&w, 0, "aster_t1_tank", 500, 512, 0)])
        .unwrap();
    w.tick(&[attack_ground(&w, 580, 512)]).unwrap();
    assert!(run_until(&mut w, 200, |w| !struck(w).is_empty()));
}

#[test]
fn a_shell_s_blast_reaches_the_houses_round_it_and_no_further() {
    // Two houses 30 m apart along y, a third 200 m off.
    let mut w = world(vec![
        prop(PropKind::CityHouse, 900, 500, Angle::ZERO, 0),
        prop(PropKind::CityHouse, 900, 530, Angle::ZERO, 0),
        prop(PropKind::CityHouse, 900, 730, Angle::ZERO, 0),
    ]);
    // A T3 howitzer (48 m blast) lobbing onto the street between the first two.
    w.tick(&[spawn(&w, 0, "aster_t3_artillery", 300, 515, 0)])
        .unwrap();
    w.tick(&[attack_ground(&w, 900, 515)]).unwrap();
    assert!(run_until(&mut w, 600, |w| {
        let s = &w.state.city;
        s.health[0] < s.max[0] && s.health[1] < s.max[1]
    }));
    assert_eq!(w.state.city.health[2], w.state.city.max[2]);
}

#[test]
fn a_fire_burns_a_house_down_to_its_shell_and_goes_out() {
    // Worn to half before the match: under the burning line, and not alight.
    let mut w = world(vec![house(500)]);
    assert_eq!(
        w.state.city.flags[0] & BURNING,
        0,
        "old damage does not burn"
    );
    assert_eq!(w.state.city.touched, vec![0]);
    w.tick(&[spawn(&w, 0, "aster_t1_tank", 500, 512, 0)])
        .unwrap();
    w.tick(&[attack_ground(&w, 660, 512)]).unwrap();
    assert!(run_until(&mut w, 200, |w| w.state.city.flags[0] & BURNING != 0));
    assert!(w
        .events
        .iter()
        .any(|e| matches!(e, SimEvent::StructureAlight { prop: 0, .. })));
    w.tick(&[cmd(Command::Stop {
        units: ids_of(&w, 0),
    })])
    .unwrap();
    let max = w.state.city.max[0];
    let floor = max * 200 / 1000;
    let mut last = w.state.city.health[0];
    assert!(run_until(&mut w, 2000, |w| {
        let h = w.state.city.health[0];
        assert!(h <= last && h >= floor, "{h} after {last}, floor {floor}");
        last = h;
        w.state.city.flags[0] & GUTTED != 0
    }));
    assert_eq!(w.state.city.health[0], floor);
    assert_eq!(w.state.city.flags[0] & (BURNING | DOWN), 0);
    // It stays out and standing.
    for _ in 0..200 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(w.state.city.health[0], floor);
    assert_eq!(w.state.city.flags[0] & (BURNING | DOWN), 0);
    assert!(w.is_prop_alive(0));
}

#[test]
fn a_block_that_comes_down_opens_its_ground_and_a_tank_drives_through() {
    // A wall right across the map at x 1000, 64 m segments along y; the middle one
    // has one point left.
    let props: Vec<Prop> = (0..32)
        .map(|i| {
            prop(
                PropKind::CityWall,
                1000,
                32 + 64 * i,
                Angle::QUARTER_TURN,
                if i == 8 { 999 } else { 0 },
            )
        })
        .collect();
    let mut w = world(props);
    assert!(!walkable(&w, 1000, 544));
    // A tank on the near side shoots the weak segment down.
    w.tick(&[spawn(&w, 0, "aster_t1_tank", 900, 544, 0)])
        .unwrap();
    w.tick(&[attack_ground(&w, 1000, 544)]).unwrap();
    assert!(run_until(&mut w, 300, |w| w.state.city.is_down(8)));
    let collapsed = w.events.iter().any(|e| {
        matches!(e, SimEvent::StructureCollapsed { prop: 8, height, .. } if *height == Fx::from_int(26))
    });
    assert!(collapsed);
    assert!(!w.is_prop_alive(8));
    assert!(walkable(&w, 1000, 544), "the breach is open");
    assert!(!walkable(&w, 1000, 480 - 16), "its neighbours still stand");
    assert!(w.is_prop_alive(7) && w.is_prop_alive(9));
    // Through the breach.
    w.tick(&[cmd(Command::Move {
        units: ids_of(&w, 0),
        target: FxVec2::from_ints(1100, 544),
        queue: false,
    })])
    .unwrap();
    let tank = w.state.units.slots.iter().next().unwrap();
    assert!(
        run_until(&mut w, 600, |w| w.state.units.pos[tank].x
            > Fx::from_int(1080)),
        "the tank should drive through the breach"
    );
}

#[test]
fn a_gun_holds_fire_while_a_house_stands_between_it_and_its_target() {
    let mut w = world(vec![house(0)]);
    w.tick(&[
        spawn(&w, 0, "aster_t1_tank", 500, 512, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            660,
            512,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    w.tick(&[cmd(Command::SetFireState {
        units: ids_of(&w, 0),
        state: FireState::HoldPosition,
    })])
    .unwrap();
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        assert!(
            !w.events
                .iter()
                .any(|e| matches!(e, SimEvent::ShotFired { owner: 0, .. })),
            "it fired into the house"
        );
    }
    let shooter = w.state.units.slots.iter().next().unwrap();
    assert_ne!(w.state.units.shot_blocked[shooter] & 1, 0);
    assert_eq!(w.state.city.health[0], w.state.city.max[0]);
}

#[test]
fn a_restored_snapshot_keeps_the_city_and_steps_alike() {
    let props = vec![
        house(500),
        prop(PropKind::CityOffice, 700, 700, Angle::ZERO, 0),
    ];
    let mut w = world(props.clone());
    w.tick(&[spawn(&w, 0, "aster_t1_tank", 500, 512, 0)])
        .unwrap();
    w.tick(&[attack_ground(&w, 660, 512)]).unwrap();
    assert!(run_until(&mut w, 200, |w| w.state.city.flags[0] & BURNING != 0));
    let bytes = w.snapshot();
    let mut r = world(props);
    r.restore(terrain(), &bytes).unwrap();
    assert_eq!(r.state.city.health, w.state.city.health);
    assert_eq!(r.state.city.flags, w.state.city.flags);
    assert_eq!(r.state.city.touched, w.state.city.touched);
    for _ in 0..150 {
        assert_eq!(w.tick(&[]).unwrap(), r.tick(&[]).unwrap());
    }
}

#[test]
fn a_structure_built_on_the_rubble_and_removed_leaves_the_rubble_open() {
    let mut w = world(vec![house(999)]);
    w.tick(&[spawn(&w, 0, "aster_t1_tank", 500, 512, 0)])
        .unwrap();
    w.tick(&[attack_ground(&w, 580, 512)]).unwrap();
    assert!(run_until(&mut w, 200, |w| w.state.city.is_down(0)));
    assert!(walkable(&w, 580, 512));
    // A wall on the rubble, then taken away: its lot is given back, and the house
    // that stood there is not put back in the way.
    w.tick(&[spawn(&w, 0, "aster_wall", 582, 510, 0)]).unwrap();
    let wall_bp = w.blueprints.id_of("aster_wall").unwrap();
    let wall = {
        let u = &w.state.units;
        let row = u.slots.iter().find(|&r| u.blueprint[r] == wall_bp).unwrap();
        u.id(row)
    };
    assert!(!walkable(&w, 582, 510), "the wall blocks its lot");
    w.tick(&[cmd(Command::DebugRemove { units: vec![wall] })])
        .unwrap();
    for (x, y) in [(574, 508), (580, 512), (586, 516)] {
        assert!(walkable(&w, x, y), "({x}, {y}) is blocked again");
    }
}

#[test]
fn a_snapshot_of_a_bad_city_table_is_refused() {
    let props = vec![house(0)];
    let mut w = world(props.clone());
    w.tick(&[]).unwrap();
    // Health over its whole.
    w.state.city.health[0] = w.state.city.max[0] + 1;
    let bytes = w.snapshot();
    let mut r = world(props.clone());
    assert!(r.restore(terrain(), &bytes).is_err());
    // A touched row out of range.
    let mut w = world(props.clone());
    w.state.city.touched.push(7);
    let bytes = w.snapshot();
    assert!(r.restore(terrain(), &bytes).is_err());
    // Another map's city.
    let mut other = world(vec![house(0), house(0)]);
    let bytes = other.snapshot();
    assert!(r.restore(terrain(), &bytes).is_err());
}
