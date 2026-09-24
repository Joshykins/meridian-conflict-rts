//! Airbases and the guard order: aircraft dock, mend below, are fired out of
//! the tunnels, come home by themselves, and a guarded area is defended.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, Handle, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

/// 256 cells of 8 m of flat dry ground at 20 m.
fn field() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::from_samples(
        256,
        256,
        vec![60; 257 * 257],
        Fx::from_int(-40),
        Fx::ONE,
        Fx::from_int(5),
    );
    let map = MapData {
        name: "field".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
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
        players: vec![player("you", 0), player("other", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn add(w: &mut World, owner: u8, key: &str, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

fn id(w: &World, row: usize) -> UnitId {
    w.state.units.id(row)
}

fn give(w: &mut World, player: u8, command: Command) {
    w.tick(&[PlayerCommand { player, command }]).unwrap();
}

fn run(w: &mut World, ticks: usize) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

fn stored(w: &World, row: usize) -> bool {
    w.state.units.hangar[row] != Handle::NONE && w.state.units.has_flag(row, flag::IN_FACTORY)
}

/// Runs until `done`, at most `limit` ticks; the ticks it took.
fn until(w: &mut World, limit: usize, done: impl Fn(&World) -> bool) -> Option<usize> {
    for t in 0..limit {
        if done(w) {
            return Some(t);
        }
        w.tick(&[]).unwrap();
    }
    done(w).then_some(limit)
}

const BASE: &str = "aster_t2_airbase";

#[test]
fn docked_aircraft_go_below_mend_and_leave_through_the_tunnels() {
    let mut w = field();
    let base = add(&mut w, 0, BASE, 800, 800);
    let keys = [
        "aster_t1_bomber",
        "aster_t1_rotor_gunship",
        "aster_t2_gunship",
    ];
    let planes: Vec<usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| add(&mut w, 0, k, 400 + 60 * i as i32, 400))
        .collect();
    for &p in &planes {
        w.state.units.health[p] = w.state.units.health[p] / 3;
    }
    let ids: Vec<UnitId> = planes.iter().map(|&p| id(&w, p)).collect();
    let c = Command::Dock {
        units: ids.clone(),
        base: id(&w, base),
        queue: false,
    };
    give(&mut w, 0, c);
    let took = until(&mut w, 1800, |w| planes.iter().all(|&p| stored(w, p)));
    assert!(took.is_some(), "every aircraft went down the hatch");
    for &p in &planes {
        assert_eq!(w.state.units.pos[p], w.state.units.pos[base]);
        assert!(
            w.state.units.order_head[p] == mc_sim::tables::NO_ORDER,
            "stored aircraft hold no orders"
        );
    }
    let view = w.hangar_view(base).unwrap();
    assert_eq!(view.stored.len(), 3);
    // Mended below: full within 25 s at 4% a second.
    run(&mut w, 260);
    for &p in &planes {
        assert_eq!(
            w.state.units.health[p],
            w.unit_max_health(p),
            "mended below"
        );
    }
    // Their own side lists them, marked stored so nothing draws them; the enemy never
    // hears of them.
    let mut frame = Default::default();
    w.write_render_frame(Some(0), &mut frame);
    let frame: &mc_sim::mirror::RenderFrame = &frame;
    let listed: Vec<_> = frame
        .units
        .iter()
        .filter(|u| ids.iter().any(|i| i.0 == u.unit_id))
        .collect();
    assert_eq!(listed.len(), 3);
    assert!(listed.iter().all(|u| u.stored()));
    let mut enemy = Default::default();
    w.write_render_frame(Some(1), &mut enemy);
    let enemy: &mc_sim::mirror::RenderFrame = &enemy;
    assert!(enemy
        .units
        .iter()
        .all(|u| !ids.iter().any(|i| i.0 == u.unit_id)));

    // Called out: fired from the corner tunnels, fast, then free.
    let c = Command::Launch {
        units: vec![id(&w, base)],
        blueprint: None,
        count: 0,
    };
    give(&mut w, 0, c);
    let out = until(&mut w, 40, |w| planes.iter().all(|&p| !stored(w, p)));
    assert!(out.is_some(), "all three out within one launch round");
    let centre = w.state.units.pos[base];
    for &p in &planes {
        let d = w.state.units.pos[p].distance(centre);
        assert!(
            d > Fx::from_int(28),
            "left through a tunnel mouth, not the hatch: {d:?}"
        );
    }
    run(&mut w, 15);
    for &p in &planes {
        assert!(
            w.state.units.pos[p].distance(centre) > Fx::from_int(55),
            "shot out and kept going"
        );
    }
    // Called-out aircraft wait outside, they do not dive straight back in.
    run(&mut w, 150);
    assert!(planes.iter().all(|&p| !stored(&w, p)));
}

#[test]
fn a_full_base_turns_the_rest_away_and_a_lost_base_takes_its_aircraft_with_it() {
    let mut w = field();
    let base = add(&mut w, 0, BASE, 800, 800);
    let planes: Vec<usize> = (0..12)
        .map(|i| add(&mut w, 0, "aster_t1_rotor_gunship", 400 + 30 * i, 500))
        .collect();
    let ids: Vec<UnitId> = planes.iter().map(|&p| id(&w, p)).collect();
    let c = Command::Dock {
        units: ids,
        base: id(&w, base),
        queue: false,
    };
    give(&mut w, 0, c);
    run(&mut w, 2400);
    let inside = planes.iter().filter(|&&p| stored(&w, p)).count();
    assert_eq!(inside, 10, "capacity is ten");
    // Those left over have given up on the dock order.
    for &p in &planes {
        if !stored(&w, p) {
            let front = w.state.orders.front(&w.state.units, p).map(|o| o.kind);
            assert_ne!(front, Some(OrderKind::Dock));
        }
    }
    let ids: Vec<UnitId> = planes.iter().map(|&p| id(&w, p)).collect();
    w.state.units.health[base] = Fx::ZERO;
    run(&mut w, 3);
    let alive = ids
        .iter()
        .filter(|&&i| w.state.units.row(i).is_some())
        .count();
    assert_eq!(alive, 2, "the ten below died with the base");
}

#[test]
fn idle_aircraft_inside_the_reach_come_home_to_land() {
    let mut w = field();
    let base = add(&mut w, 0, BASE, 800, 800);
    let near = add(&mut w, 0, "aster_t1_bomber", 1400, 800);
    let far = add(&mut w, 0, "aster_t1_rotor_gunship", 1900, 1900);
    // Both stop in the air after a hop and would land where they are.
    for (row, to) in [(near, (1450, 900)), (far, (1950, 1950))] {
        let c = Command::Move {
            units: vec![id(&w, row)],
            target: FxVec2::from_ints(to.0, to.1),
            queue: false,
        };
        give(&mut w, 0, c);
    }
    let took = until(&mut w, 2400, |w| stored(w, near));
    assert!(
        took.is_some(),
        "an idle aircraft within reach lands in the base"
    );
    assert!(!stored(&w, far), "one outside the reach lands in the open");
    assert_eq!(w.state.units.hangar[near], id(&w, base));
}

#[test]
fn a_guarding_base_sends_its_aircraft_at_intruders_and_takes_them_back() {
    let mut w = field();
    let base = add(&mut w, 0, BASE, 800, 800);
    let planes: Vec<usize> = (0..3)
        .map(|i| add(&mut w, 0, "aster_t1_rotor_gunship", 780 + 20 * i, 760))
        .collect();
    let ids: Vec<UnitId> = planes.iter().map(|&p| id(&w, p)).collect();
    let c = Command::Dock {
        units: ids,
        base: id(&w, base),
        queue: false,
    };
    give(&mut w, 0, c);
    assert!(until(&mut w, 1500, |w| planes.iter().all(|&p| stored(w, p))).is_some());
    let c = Command::Guard {
        units: vec![id(&w, base)],
        pos: FxVec2::from_ints(1100, 800),
        radius: Fx::from_int(500),
        queue: false,
    };
    give(&mut w, 0, c);
    // An enemy outside the area is left alone.
    let tank = add(&mut w, 1, "aster_t1_tank", 1900, 800);
    run(&mut w, 60);
    assert!(
        planes.iter().all(|&p| stored(&w, p)),
        "nothing to guard against yet"
    );
    // It drives in: the base sends everyone out, and they go after it.
    w.state.units.pos[tank] = FxVec2::from_ints(1350, 800);
    w.state.units.prev_pos[tank] = w.state.units.pos[tank];
    let tank_id = id(&w, tank);
    assert!(
        until(&mut w, 60, |w| planes.iter().all(|&p| !stored(w, p))).is_some(),
        "all three launched"
    );
    for &p in &planes {
        let front = w.state.orders.front(&w.state.units, p).copied().unwrap();
        assert_eq!(front.kind, OrderKind::Guard);
        assert_eq!(front.target, id(&w, base), "they know where home is");
    }
    assert!(
        until(&mut w, 1800, |w| w.state.units.row(tank_id).is_none()).is_some(),
        "the intruder was destroyed"
    );
    // Area clear: home and below again.
    assert!(
        until(&mut w, 1800, |w| planes.iter().all(|&p| stored(w, p))).is_some(),
        "back down the hatch once the area was clear"
    );
}

#[test]
fn a_new_base_guards_its_whole_reach_and_keeps_a_set_area_inside_it() {
    let mut w = field();
    let base = add(&mut w, 0, BASE, 300, 300);
    let reach = w.bp(base).airbase.as_ref().unwrap().reach;
    assert_eq!(w.state.units.guard[base], (w.state.units.pos[base], reach));
    let c = Command::Guard {
        units: vec![id(&w, base)],
        pos: FxVec2::from_ints(1900, 1900),
        radius: Fx::from_int(9000),
        queue: false,
    };
    give(&mut w, 0, c);
    let (at, radius) = w.state.units.guard[base];
    assert!(at.distance(w.state.units.pos[base]) <= reach + Fx::ONE);
    assert_eq!(radius, reach);
    // It is a setting, not an order: the queue stays free for an upgrade.
    assert!(w.state.orders.front(&w.state.units, base).is_none());
    let c = Command::Stop {
        units: vec![id(&w, base)],
    };
    give(&mut w, 0, c);
    assert_eq!(w.state.units.guard[base].1, Fx::ZERO, "Stop ends the guard");
}

#[test]
fn aircraft_come_down_the_shaft_together_and_all_the_way() {
    let mut w = field();
    let base = add(&mut w, 0, BASE, 800, 800);
    let planes: Vec<usize> = (0..6)
        .map(|i| add(&mut w, 0, "aster_t1_rotor_gunship", 700 + 12 * i, 700))
        .collect();
    let ids: Vec<UnitId> = planes.iter().map(|&p| id(&w, p)).collect();
    let c = Command::Dock {
        units: ids,
        base: id(&w, base),
        queue: false,
    };
    give(&mut w, 0, c);
    let ground = w.terrain.height_at(w.state.units.pos[base]);
    let mut most_down = 0;
    let mut deepest = Fx::from_int(1000);
    for _ in 0..1500 {
        w.tick(&[]).unwrap();
        let down = planes
            .iter()
            .filter(|&&p| {
                w.state.units.slots.is_alive(p) && !stored(&w, p) && w.state.units.z[p] < ground
            })
            .count();
        most_down = most_down.max(down);
        for &p in &planes {
            if !stored(&w, p) {
                deepest = deepest.min(w.state.units.z[p] - ground);
            }
        }
        if planes.iter().all(|&p| stored(&w, p)) {
            break;
        }
    }
    assert!(planes.iter().all(|&p| stored(&w, p)), "all went below");
    assert!(
        most_down >= 3,
        "a cluster goes down together, not one at a time: {most_down}"
    );
    assert!(
        deepest < Fx::from_int(-15),
        "they sink right down the shaft: {deepest:?}"
    );
}

#[test]
fn launched_aircraft_run_out_of_the_tunnel_mouth() {
    let mut w = field();
    let base = add(&mut w, 0, BASE, 800, 800);
    let plane = add(&mut w, 0, "aster_t1_rotor_gunship", 760, 760);
    let c = Command::Dock {
        units: vec![id(&w, plane)],
        base: id(&w, base),
        queue: false,
    };
    give(&mut w, 0, c);
    assert!(until(&mut w, 900, |w| stored(w, plane)).is_some());
    let c = Command::Launch {
        units: vec![id(&w, base)],
        blueprint: None,
        count: 0,
    };
    give(&mut w, 0, c);
    assert!(until(&mut w, 20, |w| !stored(w, plane)).is_some());
    let a = w.bp(base).airbase.clone().unwrap();
    let centre = w.state.units.pos[base];
    let mouth = a
        .tunnels
        .iter()
        .map(|t| t.mouth.xy().length())
        .fold(Fx::ZERO, Fx::max);
    // In the tunnel: out of reach, moving outward faster each tick.
    let mut last = w.state.units.pos[plane].distance(centre);
    assert!(
        last < mouth - Fx::from_int(8),
        "starts at the back of the tunnel: {last:?}"
    );
    let mut steps = Vec::new();
    while w.state.units.has_flag(plane, flag::IN_FACTORY) {
        w.tick(&[]).unwrap();
        let d = w.state.units.pos[plane].distance(centre);
        steps.push(d - last);
        last = d;
        assert!(steps.len() < 20, "leaves the tunnel");
    }
    assert!(steps.len() >= 3, "{steps:?}");
    assert!(
        steps.windows(2).all(|p| p[1] > p[0]),
        "speeding up: {steps:?}"
    );
    // Freed at the mouth, and flying on its own the same tick: at most a tick's flight past it.
    assert!(
        last >= mouth - Fx::ONE && last <= mouth + Fx::from_int(8),
        "free at the mouth: {last:?} vs {mouth:?}"
    );
    run(&mut w, 5);
    assert!(
        w.state.units.pos[plane].distance(centre) > mouth + Fx::from_int(20),
        "and on out"
    );
}

#[test]
fn an_upgrade_keeps_the_base_and_what_it_holds() {
    let mut w = field();
    let base = add(&mut w, 0, "aster_t1_airbase", 800, 800);
    w.state.players[0].free_build = true;
    let plane = add(&mut w, 0, "aster_t1_bomber", 760, 760);
    let c = Command::Dock {
        units: vec![id(&w, plane)],
        base: id(&w, base),
        queue: false,
    };
    give(&mut w, 0, c);
    assert!(until(&mut w, 1500, |w| stored(w, plane)).is_some());
    let base_id = id(&w, base);
    let t2 = w.blueprints.id_of(BASE).unwrap();
    let c = Command::Upgrade {
        units: vec![base_id],
    };
    give(&mut w, 0, c);
    assert!(
        until(&mut w, 6000, |w| w.state.units.blueprint[base] == t2).is_some(),
        "upgraded"
    );
    assert_eq!(id(&w, base), base_id, "the same building");
    assert!(stored(&w, plane), "the aircraft below is still there");
    let reach = w.bp(base).airbase.as_ref().unwrap().reach;
    assert_eq!(
        w.state.units.guard[base].1, reach,
        "the guard grew with the reach"
    );
}

#[test]
fn a_base_set_not_to_take_idle_aircraft_is_passed_by() {
    let mut w = field();
    let base = add(&mut w, 0, BASE, 800, 800);
    let c = Command::SetAutoLand {
        units: vec![id(&w, base)],
        on: false,
    };
    give(&mut w, 0, c);
    let near = add(&mut w, 0, "aster_t1_bomber", 1100, 800);
    let c = Command::Move {
        units: vec![id(&w, near)],
        target: FxVec2::from_ints(1150, 900),
        queue: false,
    };
    give(&mut w, 0, c);
    run(&mut w, 1200);
    assert!(!stored(&w, near), "it landed in the open instead");
    // Sent there, it still goes in.
    let c = Command::Dock {
        units: vec![id(&w, near)],
        base: id(&w, base),
        queue: false,
    };
    give(&mut w, 0, c);
    assert!(until(&mut w, 1500, |w| stored(w, near)).is_some());
}

#[test]
fn guards_hold_their_spot_chase_what_comes_in_and_walk_back() {
    let mut w = field();
    let tanks: Vec<usize> = (0..2)
        .map(|i| add(&mut w, 0, "aster_t1_tank", 600 + 20 * i, 600))
        .collect();
    let ids: Vec<UnitId> = tanks.iter().map(|&t| id(&w, t)).collect();
    let spot = FxVec2::from_ints(900, 600);
    let c = Command::Guard {
        units: ids,
        pos: spot,
        radius: Fx::from_int(300),
        queue: false,
    };
    give(&mut w, 0, c);
    run(&mut w, 400);
    for &t in &tanks {
        assert!(
            w.state.units.pos[t].distance(spot) < Fx::from_int(40),
            "holding the spot"
        );
    }
    let before: Vec<FxVec2> = tanks.iter().map(|&t| w.state.units.pos[t]).collect();
    // Outside the area and out of gun range: left alone.
    let far = add(&mut w, 1, "aster_t1_scout", 1500, 600);
    w.state.units.flags[far] |= flag::PASSIVE;
    run(&mut w, 100);
    for (&t, b) in tanks.iter().zip(&before) {
        assert!(
            w.state.units.pos[t].distance(*b) < Fx::from_int(4),
            "no chase outside the area"
        );
    }
    // Inside the area, beyond gun range: chased.
    w.state.units.pos[far] = FxVec2::from_ints(1150, 600);
    w.state.units.prev_pos[far] = w.state.units.pos[far];
    let far_id = id(&w, far);
    run(&mut w, 8);
    assert!(tanks.iter().any(|&t| {
        w.state
            .orders
            .front(&w.state.units, t)
            .is_some_and(|o| o.kind == OrderKind::Attack && o.target == far_id)
    }));
    assert!(
        until(&mut w, 1200, |w| w.state.units.row(far_id).is_none()).is_some(),
        "intruder killed"
    );
    run(&mut w, 600);
    for &t in &tanks {
        let front = w.state.orders.front(&w.state.units, t).copied().unwrap();
        assert_eq!(front.kind, OrderKind::Guard, "still guarding");
        assert!(
            w.state.units.pos[t].distance(spot) < Fx::from_int(40),
            "walked back"
        );
    }
}

#[test]
fn stored_aircraft_given_orders_are_fired_out_to_carry_them_out() {
    let mut w = field();
    let base = add(&mut w, 0, BASE, 800, 800);
    let planes: Vec<usize> = (0..3).map(|i| add(&mut w, 0, "aster_t1_rotor_gunship", 760 + 15 * i, 760)).collect();
    let ids: Vec<UnitId> = planes.iter().map(|&p| id(&w, p)).collect();
    let c = Command::Dock { units: ids.clone(), base: id(&w, base), queue: false };
    give(&mut w, 0, c);
    assert!(until(&mut w, 1500, |w| planes.iter().all(|&p| stored(w, p))).is_some());
    // Two picked from the hangar and sent somewhere; the third stays below.
    let goal = FxVec2::from_ints(1500, 1200);
    let c = Command::Move { units: ids[..2].to_vec(), target: goal, queue: false };
    give(&mut w, 0, c);
    assert!(until(&mut w, 900, |w| planes[..2]
        .iter()
        .all(|&p| w.state.units.pos[p].distance(goal) < Fx::from_int(40)))
    .is_some(), "they went out and got there");
    assert!(stored(&w, planes[2]), "the one not ordered stayed below");
    // Launched by name: only that one goes.
    let c = Command::Launch { units: vec![ids[2]], blueprint: None, count: 0 };
    give(&mut w, 0, c);
    assert!(until(&mut w, 60, |w| !stored(w, planes[2])).is_some());
}

/// A coast: a strip of land (20 m up) along the west edge, then 20 m of sea over a
/// flat bed.
fn coast() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..40 {
            samples[y * 257 + x] = 40;
        }
    }
    let terrain = Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(20));
    let map = MapData {
        name: "coast".into(),
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
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

#[test]
fn a_roost_of_torpedo_bombers_goes_after_dived_submarines() {
    let mut w = coast();
    let base = add(&mut w, 0, BASE, 150, 1000);
    let planes: Vec<usize> = (0..3).map(|i| add(&mut w, 0, "aster_t2_torpedo_bomber", 120 + 20 * i, 900)).collect();
    let ids: Vec<UnitId> = planes.iter().map(|&p| id(&w, p)).collect();
    // Docked first, before anything hostile is about.
    let c = Command::Dock { units: ids.clone(), base: id(&w, base), queue: false };
    give(&mut w, 0, c);
    assert!(until(&mut w, 2400, |w| planes.iter().all(|&p| stored(w, p))).is_some(), "went below");
    let subs: Vec<UnitId> = (0..2)
        .map(|i| {
            let r = add(&mut w, 1, "aster_t1_submarine", 900 + 60 * i, 1000);
            w.state.units.flags[r] |= flag::PASSIVE;
            id(&w, r)
        })
        .collect();
    let health = |w: &World| -> Fx {
        subs.iter().filter_map(|&s| w.state.units.row(s)).map(|r| w.state.units.health[r]).fold(Fx::ZERO, |a, h| a + h)
    };
    let start = health(&w);
    let dived_at_start = subs.iter().all(|&s| w.state.units.row(s).is_some_and(|r| w.state.units.dive_goal[r]));
    assert!(dived_at_start, "the submarines go down as soon as they are out");
    // Launched at them, and they go after them instead of flying straight home.
    let mut stores = 0;
    let mut was_stored = true;
    for _ in 0..1800 {
        w.tick(&[]).unwrap();
        let now = planes.iter().any(|&p| w.state.units.slots.is_alive(p) && stored(&w, p));
        if now && !was_stored {
            stores += 1;
        }
        was_stored = now;
        if subs.iter().all(|&s| w.state.units.row(s).is_none()) {
            break;
        }
    }
    let sunk = subs.iter().filter(|&&s| w.state.units.row(s).is_none()).count();
    assert!(health(&w) < start, "the submarines were hit");
    assert!(sunk >= 1, "at least one went to the bottom");
    assert!(stores <= 4, "no launch-and-land loop: went below {stores} times");
}
