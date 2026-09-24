//! Player orders: fire states, attack-ground, bombard, patrol, cancelling one
//! queued order, and orbit for every aircraft.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::RenderFrame;
use mc_sim::tables::{flag, Controller, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{
    Command, FireState, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World,
};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "orders".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(256, 256), FxVec2::from_ints(1800, 1800)],
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
        seed: 11,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn order(command: Command) -> Vec<PlayerCommand> {
    vec![PlayerCommand { player: 0, command }]
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("unit is alive")
}

fn queue(w: &World, id: UnitId) -> Vec<(OrderKind, FxVec2)> {
    let r = row(w, id);
    w.state
        .orders
        .iter(&w.state.units, r)
        .map(|o| (o.kind, o.pos))
        .collect()
}

/// A target that never shoots back and never dies.
fn dummy(w: &mut World, key: &str, x: i32, y: i32) -> UnitId {
    let id = add(w, key, 1, x, y);
    let r = row(w, id);
    w.state.units.flags[r] |= flag::PASSIVE | flag::INVULNERABLE;
    id
}

/// Shots fired by `shooter`'s blueprint over `ticks` ticks.
fn shots(w: &mut World, shooter: UnitId, ticks: u32) -> usize {
    let bp = w.state.units.blueprint[row(w, shooter)];
    let mut n = 0;
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
        n += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::ShotFired { blueprint, owner: 0, .. } if *blueprint == bp))
            .count();
    }
    n
}

/// Where `shooter`'s blueprint's shots came down over `ticks` ticks.
fn impacts(w: &mut World, shooter: UnitId, ticks: u32) -> Vec<FxVec2> {
    let bp = w.state.units.blueprint[row(w, shooter)];
    let mut out = Vec::new();
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::Impact { pos, blueprint, .. } = e {
                if *blueprint == bp {
                    out.push(pos.xy());
                }
            }
        }
    }
    out
}

#[test]
fn hold_fire_waits_for_an_attack_order() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 500, 500);
    let enemy = dummy(&mut w, "aster_t1_tank", 620, 500);
    w.tick(&order(Command::SetFireState {
        units: vec![tank],
        state: FireState::HoldFire,
    }))
    .unwrap();
    assert_eq!(w.state.units.fire_state[row(&w, tank)], FireState::HoldFire);
    assert_eq!(
        shots(&mut w, tank, 60),
        0,
        "a unit holding fire opened up by itself"
    );

    w.tick(&order(Command::Attack {
        units: vec![tank],
        target: enemy,
        queue: false,
    }))
    .unwrap();
    assert!(
        shots(&mut w, tank, 60) > 0,
        "an Attack order is obeyed while holding fire"
    );
}

/// How far `id` has strayed from `(x, y)`, metres.
fn strayed(w: &World, id: UnitId, x: i32, y: i32) -> f32 {
    w.state.units.pos[row(w, id)]
        .distance(FxVec2::from_ints(x, y))
        .to_f32()
}

/// A tank's reach against another tank: gun range plus the target's radius.
fn tank_reach(w: &World) -> i32 {
    let bp = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t1_tank").unwrap());
    (bp.max_weapon_range() + bp.radius).to_f32() as i32
}

#[test]
fn an_idle_unit_chases_an_enemy_just_out_of_range_then_walks_back() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 500, 500);
    let x = 500 + tank_reach(&w) + 12;
    let enemy = add(&mut w, "aster_t1_tank", 1, x, 500);
    let r = row(&w, enemy);
    w.state.units.flags[r] |= flag::PASSIVE;
    assert!(
        shots(&mut w, tank, 200) > 0,
        "it never went after the enemy"
    );
    assert!(strayed(&w, tank, 500, 500) > 4.0, "it shot without moving");
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        if w.state.units.row(enemy).is_none() && queue(&w, tank).is_empty() {
            break;
        }
    }
    assert!(
        w.state.units.row(enemy).is_none(),
        "the chase never finished the enemy"
    );
    assert!(
        queue(&w, tank).is_empty(),
        "still busy: {:?}",
        queue(&w, tank)
    );
    assert!(
        strayed(&w, tank, 500, 500) < 6.0,
        "it did not come back: {} m off",
        strayed(&w, tank, 500, 500)
    );
}

#[test]
fn a_chase_gives_up_at_the_end_of_its_leash() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 500, 500);
    let x = 500 + tank_reach(&w) + 12;
    let enemy = dummy(&mut w, "aster_t1_tank", x, 500);
    let mut far = 0.0f32;
    let mut gave_up = false;
    for _ in 0..1500 {
        // The enemy backs off as fast as the tank follows, and slips away
        // once the tank turns for home.
        let r = row(&w, enemy);
        let t = w.state.units.pos[row(&w, tank)];
        gave_up |= queue(&w, tank)
            .first()
            .is_some_and(|o| o.0 == OrderKind::AttackMove);
        let ex = if gave_up {
            1500
        } else {
            x.max(t.x.to_f32() as i32 + tank_reach(&w) + 12)
        };
        w.state.units.pos[r] = FxVec2::from_ints(ex, 500);
        w.tick(&[]).unwrap();
        far = far.max(strayed(&w, tank, 500, 500));
        if gave_up && queue(&w, tank).is_empty() {
            break;
        }
    }
    assert!(gave_up, "it never turned back");
    let leash = tank_reach(&w) as f32;
    assert!(far > 4.0, "it never set off");
    // Braking from full speed carries it a little past the end.
    assert!(far < leash * 1.2, "it ran {far} m on a {leash} m leash");
    assert!(strayed(&w, tank, 500, 500) < 6.0, "it did not come back");
}

#[test]
fn holding_position_shoots_what_is_in_range_but_never_chases() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 500, 500);
    w.tick(&order(Command::SetFireState {
        units: vec![tank],
        state: FireState::HoldPosition,
    }))
    .unwrap();
    let reach = tank_reach(&w);
    let enemy = dummy(&mut w, "aster_t1_tank", 500 + reach + 12, 500);
    assert_eq!(
        shots(&mut w, tank, 200),
        0,
        "it went after an enemy out of range"
    );
    assert!(strayed(&w, tank, 500, 500) < 1.0, "it left its spot");

    let r = row(&w, enemy);
    w.state.units.pos[r] = FxVec2::from_ints(500 + reach - 8, 500);
    assert!(
        shots(&mut w, tank, 120) > 0,
        "it did not fire on an enemy in range"
    );
    assert!(strayed(&w, tank, 500, 500) < 1.0, "it left its spot");
}

#[test]
fn holding_position_stops_a_unit_where_it_stands_and_later_moves_keep_it() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 500, 500);
    w.tick(&order(Command::Move {
        units: vec![tank],
        target: FxVec2::from_ints(1200, 500),
        queue: false,
    }))
    .unwrap();
    for _ in 0..40 {
        w.tick(&[]).unwrap();
    }
    w.tick(&order(Command::SetFireState {
        units: vec![tank],
        state: FireState::HoldPosition,
    }))
    .unwrap();
    assert!(
        queue(&w, tank).is_empty(),
        "the move was kept: {:?}",
        queue(&w, tank)
    );
    let stop = w.state.units.pos[row(&w, tank)];
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    let rolled = w.state.units.pos[row(&w, tank)].distance(stop).to_f32();
    assert!(
        rolled < 20.0,
        "it kept rolling for {rolled} m, speed {}",
        w.state.units.speed[row(&w, tank)].to_f32()
    );

    // Sent on, it goes, and holds there.
    w.tick(&order(Command::Move {
        units: vec![tank],
        target: FxVec2::from_ints(700, 500),
        queue: false,
    }))
    .unwrap();
    for _ in 0..400 {
        w.tick(&[]).unwrap();
    }
    assert!(
        strayed(&w, tank, 700, 500) < 6.0,
        "the move after holding was not obeyed"
    );
    assert_eq!(
        w.state.units.fire_state[row(&w, tank)],
        FireState::HoldPosition
    );
}

#[test]
fn a_direct_fire_tank_shells_the_ground() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 400, 500);
    let spot = FxVec2::from_ints(800, 500);
    w.tick(&order(Command::AttackGround {
        units: vec![tank],
        pos: spot,
        queue: false,
    }))
    .unwrap();
    let hits = impacts(&mut w, tank, 300);
    assert!(hits.len() >= 3, "only {} shells landed", hits.len());
    for h in &hits {
        assert!(
            h.distance(spot) < Fx::from_int(40),
            "a shell landed at {h:?}, far from {spot:?}"
        );
    }
    let range = w.bp(row(&w, tank)).weapons[0].range_max;
    assert!(
        w.state.units.pos[row(&w, tank)].distance(spot) <= range,
        "it never closed to range"
    );
    assert_eq!(
        queue(&w, tank),
        vec![(OrderKind::AttackGround, spot)],
        "it stays on the order"
    );
}

#[test]
fn artillery_shells_the_ground_and_anti_air_is_not_given_the_order() {
    let mut w = world();
    let gun = add(&mut w, "aster_t1_artillery", 0, 400, 500);
    let aa = add(&mut w, "aster_t1_mobile_aa", 0, 400, 560);
    let spot = FxVec2::from_ints(650, 500);
    w.tick(&order(Command::AttackGround {
        units: vec![gun, aa],
        pos: spot,
        queue: false,
    }))
    .unwrap();
    assert!(
        queue(&w, aa).is_empty(),
        "an anti-air gun cannot shoot the ground"
    );
    let hits = impacts(&mut w, gun, 250);
    assert!(!hits.is_empty(), "the howitzer never fired");
    for h in &hits {
        assert!(
            h.distance(spot) < Fx::from_int(60),
            "a shell landed at {h:?}"
        );
    }
}

#[test]
fn bombard_spreads_shots_over_the_circle() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 450, 500);
    let spot = FxVec2::from_ints(600, 500);
    let radius = Fx::from_int(40);
    w.tick(&order(Command::Bombard {
        units: vec![tank],
        pos: spot,
        radius,
        queue: false,
    }))
    .unwrap();
    let hits = impacts(&mut w, tank, 300);
    assert!(hits.len() >= 5, "only {} shells landed", hits.len());
    for h in &hits {
        assert!(
            h.distance(spot) <= radius + Fx::from_int(12),
            "a shell landed at {h:?}"
        );
    }
    let spread = hits.iter().map(|h| h.distance(hits[0])).max().unwrap();
    assert!(
        spread > Fx::from_int(15),
        "every shell fell on the same spot"
    );
    let r = row(&w, tank);
    let front = *w.state.orders.front(&w.state.units, r).unwrap();
    assert_eq!((front.kind, front.radius), (OrderKind::Bombard, radius));
}

#[test]
fn ground_fire_is_deterministic() {
    let run = || {
        let mut w = world();
        let tank = add(&mut w, "aster_t1_tank", 0, 450, 500);
        w.tick(&order(Command::Bombard {
            units: vec![tank],
            pos: FxVec2::from_ints(600, 500),
            radius: Fx::from_int(40),
            queue: false,
        }))
        .unwrap();
        (0..200).map(|_| w.tick(&[]).unwrap()).last().unwrap()
    };
    assert_eq!(run(), run());
}

#[test]
fn patrol_loops_through_its_points() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 400, 400);
    let a = FxVec2::from_ints(500, 400);
    let b = FxVec2::from_ints(500, 520);
    w.tick(&order(Command::Patrol {
        units: vec![tank],
        points: vec![a, b],
        queue: false,
    }))
    .unwrap();
    let mut visits = Vec::new();
    for _ in 0..900 {
        w.tick(&[]).unwrap();
        let pos = w.state.units.pos[row(&w, tank)];
        for (name, p) in [('a', a), ('b', b)] {
            if pos.distance(p) < Fx::from_int(8) && visits.last() != Some(&name) {
                visits.push(name);
            }
        }
    }
    assert!(visits.len() >= 4, "patrol stopped looping: {visits:?}");
    assert!(visits.windows(2).all(|p| p[0] != p[1]));
    let kinds: Vec<_> = queue(&w, tank).iter().map(|q| q.0).collect();
    assert_eq!(
        kinds,
        vec![OrderKind::Patrol; 2],
        "the loop keeps both waypoints"
    );
}

#[test]
fn a_single_point_patrol_comes_back_to_the_start() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 400, 400);
    let far = FxVec2::from_ints(560, 400);
    w.tick(&order(Command::Patrol {
        units: vec![tank],
        points: vec![far],
        queue: false,
    }))
    .unwrap();
    let q = queue(&w, tank);
    assert_eq!(
        q,
        vec![
            (OrderKind::Patrol, far),
            (OrderKind::Patrol, FxVec2::from_ints(400, 400))
        ]
    );
    let mut went = false;
    let mut back = false;
    for _ in 0..900 {
        w.tick(&[]).unwrap();
        let pos = w.state.units.pos[row(&w, tank)];
        went |= pos.distance(far) < Fx::from_int(8);
        back |= went && pos.distance(FxVec2::from_ints(400, 400)) < Fx::from_int(8);
    }
    assert!(went && back, "went {went}, came back {back}");
}

#[test]
fn aircraft_patrol_and_waypoints_can_be_dragged() {
    let mut w = world();
    let plane = add(&mut w, "aster_t1_interceptor", 0, 400, 400);
    let a = FxVec2::from_ints(700, 400);
    let b = FxVec2::from_ints(700, 900);
    w.tick(&order(Command::Patrol {
        units: vec![plane],
        points: vec![a, b],
        queue: false,
    }))
    .unwrap();
    let moved = FxVec2::from_ints(1100, 900);
    w.tick(&order(Command::RelocateOrder {
        units: vec![plane],
        kind: OrderKind::Patrol,
        from: b,
        to: moved,
    }))
    .unwrap();
    assert!(queue(&w, plane).contains(&(OrderKind::Patrol, moved)));
    let mut legs = 0;
    let mut last = FxVec2::ZERO;
    for _ in 0..1500 {
        w.tick(&[]).unwrap();
        let front = queue(&w, plane)[0].1;
        if front != last {
            legs += 1;
            last = front;
        }
    }
    assert!(legs >= 4, "the aircraft flew only {legs} legs");
}

#[test]
fn cancelling_a_begun_build_leaves_the_site_and_moves_on() {
    let mut w = world();
    let eng = add(&mut w, "aster_t1_engineer", 0, 400, 400);
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    let mut sites = Vec::new();
    for (i, x) in [(0, 440), (1, 520)] {
        let site = mc_sim::snap_to_build_grid(w.blueprints.unit(power), FxVec2::from_ints(x, 400));
        sites.push(site);
        w.tick(&order(Command::Build {
            units: vec![eng],
            blueprint: power,
            pos: site,
            heading: Angle::ZERO,
            queue: i > 0,
        }))
        .unwrap();
    }
    let begun = |w: &World| {
        w.state
            .units
            .slots
            .iter()
            .find(|&r| w.state.units.blueprint[r] == power && w.state.units.pos[r] == sites[0])
    };
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        if begun(&w).is_some() {
            break;
        }
    }
    let site = begun(&w).expect("the first structure was begun");
    w.tick(&order(Command::CancelOrder {
        units: vec![eng],
        kind: OrderKind::Build,
        pos: sites[0],
    }))
    .unwrap();
    assert!(
        w.state.units.slots.is_alive(site),
        "the half-built site was torn down"
    );
    assert!(w.state.units.has_flag(site, flag::UNDER_CONSTRUCTION));
    assert_eq!(queue(&w, eng), vec![(OrderKind::Build, sites[1])]);
    let mut second = false;
    for _ in 0..300 {
        w.tick(&[]).unwrap();
        second |= w
            .state
            .units
            .slots
            .iter()
            .any(|r| w.state.units.blueprint[r] == power && w.state.units.pos[r] == sites[1]);
    }
    assert!(second, "the engineer did not go on to the next structure");
}

#[test]
fn a_patrol_waypoint_can_be_cancelled() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 400, 400);
    let points = [
        FxVec2::from_ints(500, 400),
        FxVec2::from_ints(500, 500),
        FxVec2::from_ints(400, 500),
    ];
    w.tick(&order(Command::Patrol {
        units: vec![tank],
        points: points.to_vec(),
        queue: false,
    }))
    .unwrap();
    w.tick(&order(Command::CancelOrder {
        units: vec![tank],
        kind: OrderKind::Patrol,
        pos: points[0],
    }))
    .unwrap();
    assert_eq!(
        queue(&w, tank),
        vec![
            (OrderKind::Patrol, points[1]),
            (OrderKind::Patrol, points[2])
        ]
    );
    // Another kind at the same place is left alone.
    w.tick(&order(Command::CancelOrder {
        units: vec![tank],
        kind: OrderKind::Move,
        pos: points[1],
    }))
    .unwrap();
    assert_eq!(queue(&w, tank).len(), 2);
}

#[test]
fn every_aircraft_takes_an_orbit_order() {
    let mut w = world();
    for key in [
        "aster_t1_interceptor",
        "aster_t1_bomber",
        "aster_t2_gunship",
        "aster_t3_strategic_bomber",
    ] {
        let plane = add(&mut w, key, 0, 400, 400);
        let centre = FxVec2::from_ints(900, 900);
        w.tick(&order(Command::Orbit {
            units: vec![plane],
            pos: centre,
            target: UnitId::NONE,
            radius: Fx::ZERO,
            queue: false,
        }))
        .unwrap();
        assert_eq!(queue(&w, plane), vec![(OrderKind::Orbit, centre)], "{key}");
        let r = row(&w, plane);
        assert!(w.bp(r).orbit_radius > Fx::ZERO, "{key}");
    }
    let mut far = Fx::ZERO;
    for _ in 0..600 {
        w.tick(&[]).unwrap();
    }
    for r in w.state.units.slots.iter() {
        far = far.max(w.state.units.pos[r].distance(FxVec2::from_ints(900, 900)));
    }
    assert!(
        far < Fx::from_int(800),
        "an aircraft wandered off its orbit: {far:?}"
    );
}

#[test]
fn the_mirror_carries_the_fire_state_and_bombard_radius() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 400, 400);
    let spot = FxVec2::from_ints(700, 400);
    w.tick(&[
        PlayerCommand {
            player: 0,
            command: Command::SetFireState {
                units: vec![tank],
                state: FireState::HoldFire,
            },
        },
        PlayerCommand {
            player: 0,
            command: Command::Bombard {
                units: vec![tank],
                pos: spot,
                radius: Fx::from_int(30),
                queue: false,
            },
        },
    ])
    .unwrap();
    let mut frame = RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    let unit = frame.units.iter().find(|u| u.unit_id == tank.0).unwrap();
    assert_eq!(unit.fire_state(), FireState::HoldFire);
    assert_eq!(unit.kill_count(), 0);
    let mut orders = Vec::new();
    w.write_orders(Some(0), &[tank.0], None, &mut orders);
    let q = &orders[0].orders[0];
    assert_eq!((q.kind, q.at, q.radius), (OrderKind::Bombard, spot, 30.0));
}

#[test]
fn produced_units_fire_at_will() {
    let mut w = world();
    let factory = add(&mut w, "aster_t1_land_factory", 0, 600, 600);
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    w.tick(&[
        PlayerCommand {
            player: 0,
            command: Command::DebugFreeBuild {
                player: 0,
                on: true,
            },
        },
        PlayerCommand {
            player: 0,
            command: Command::Produce {
                factories: vec![factory],
                blueprint: tank,
                count: 1,
            },
        },
    ])
    .unwrap();
    for _ in 0..600 {
        w.tick(&[]).unwrap();
    }
    let made = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == tank && w.state.units.is_active(r))
        .expect("the factory finished a tank");
    assert_eq!(w.state.units.fire_state[made], FireState::FireAtWill);
}

#[test]
fn bombers_and_gunships_attack_the_ground() {
    for key in ["aster_t1_bomber", "aster_t2_gunship"] {
        let mut w = world();
        let plane = add(&mut w, key, 0, 300, 300);
        let spot = FxVec2::from_ints(1000, 1000);
        w.tick(&order(Command::AttackGround {
            units: vec![plane],
            pos: spot,
            queue: false,
        }))
        .unwrap();
        let hits = impacts(&mut w, plane, 900);
        let near = hits
            .iter()
            .filter(|h| h.distance(spot) < Fx::from_int(60))
            .count();
        assert!(
            near >= 2,
            "{key}: {near} of {} shots near the mark",
            hits.len()
        );
    }
}

#[test]
fn a_bombarding_bomber_spreads_its_runs_over_the_circle() {
    let mut w = world();
    let plane = add(&mut w, "aster_t1_bomber", 0, 300, 300);
    let spot = FxVec2::from_ints(1000, 1000);
    let radius = Fx::from_int(150);
    w.tick(&order(Command::Bombard {
        units: vec![plane],
        pos: spot,
        radius,
        queue: false,
    }))
    .unwrap();
    let bp = w.state.units.blueprint[row(&w, plane)];
    // Each run's carpet, split where the bombs stop falling for a while.
    let mut runs: Vec<Vec<FxVec2>> = Vec::new();
    let mut quiet = u32::MAX;
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        quiet = quiet.saturating_add(1);
        for e in &w.events {
            if let SimEvent::Impact { pos, blueprint, .. } = e {
                if *blueprint == bp {
                    if quiet > 40 {
                        runs.push(Vec::new());
                    }
                    runs.last_mut().unwrap().push(pos.xy());
                    quiet = 0;
                }
            }
        }
    }
    let middles: Vec<FxVec2> = runs
        .iter()
        .map(|r| {
            let sum = r.iter().fold(FxVec2::ZERO, |a, &p| a + p);
            FxVec2::new(sum.x / r.len() as i32, sum.y / r.len() as i32)
        })
        .collect();
    assert!(middles.len() >= 3, "only {} runs", middles.len());
    for m in &middles {
        assert!(
            m.distance(spot) <= radius + Fx::from_int(40),
            "a run landed at {m:?}"
        );
    }
    let spread = middles
        .iter()
        .map(|m| m.distance(middles[0]))
        .max()
        .unwrap();
    assert!(
        spread > Fx::from_int(40),
        "every run hit the same spot: {middles:?}"
    );
}

#[test]
fn a_fighter_cannot_attack_the_ground() {
    let mut w = world();
    let plane = add(&mut w, "aster_t1_interceptor", 0, 300, 300);
    w.tick(&order(Command::AttackGround {
        units: vec![plane],
        pos: FxVec2::from_ints(1000, 1000),
        queue: false,
    }))
    .unwrap();
    assert!(queue(&w, plane).is_empty());
}

#[test]
fn a_bombarding_turret_is_on_each_point_before_it_fires() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 450, 500);
    let spot = FxVec2::from_ints(600, 500);
    w.tick(&order(Command::Bombard {
        units: vec![tank],
        pos: spot,
        radius: Fx::from_int(60),
        queue: false,
    }))
    .unwrap();
    let bp = w.state.units.blueprint[row(&w, tank)];
    let mut points = Vec::new();
    for _ in 0..400 {
        let r = row(&w, tank);
        let aim = w.state.units.ground_aim[r][0];
        w.tick(&[]).unwrap();
        let fired = w.events.iter().any(
            |e| matches!(e, SimEvent::ShotFired { blueprint, weapon: 0, .. } if *blueprint == bp),
        );
        if !fired {
            continue;
        }
        let units = &w.state.units;
        assert!(
            aim.distance(spot) <= Fx::from_int(60),
            "aimed outside the circle: {aim:?}"
        );
        let bearing = (aim - units.pos[r]).angle();
        let facing = units.heading[r] + units.weapon_yaw[r][0];
        let off = facing.delta_to(bearing).unsigned_abs();
        assert!(off <= 400, "fired {off} angle steps off its point {aim:?}");
        points.push(aim);
        // The next point is chosen once the shot is away, so the gun has to slew to it.
        assert_ne!(units.ground_aim[r][0], aim, "the same point twice");
    }
    assert!(points.len() >= 5, "only {} shots", points.len());
}

#[test]
fn a_bombarding_paladin_fires_both_arms_where_its_torso_points() {
    let mut w = world();
    let paladin = add(&mut w, "aster_t3_assault_bot", 0, 450, 500);
    let spot = FxVec2::from_ints(640, 500);
    w.tick(&order(Command::Bombard {
        units: vec![paladin],
        pos: spot,
        radius: Fx::from_int(80),
        queue: false,
    }))
    .unwrap();
    let bp = w.state.units.blueprint[row(&w, paladin)];
    let mut shots = [0; 2];
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        let r = row(&w, paladin);
        for e in &w.events {
            let SimEvent::ShotFired {
                blueprint, weapon, ..
            } = e
            else {
                continue;
            };
            if *blueprint != bp {
                continue;
            }
            // The model turns one torso, by weapon 0's yaw: an arm aimed anywhere
            // else would fire from where no arm is drawn.
            let units = &w.state.units;
            let off = units.weapon_yaw[r][0]
                .delta_to(units.weapon_yaw[r][*weapon as usize])
                .unsigned_abs();
            assert!(
                off <= 400,
                "arm {weapon} fired {off} angle steps off the torso"
            );
            shots[*weapon as usize] += 1;
        }
    }
    assert!(shots.iter().all(|&n| n >= 3), "shots per arm: {shots:?}");
}

fn economy_of(w: &World, id: UnitId) -> mc_sim::mirror::UnitOrders {
    let mut out = Vec::new();
    w.write_orders(Some(0), &[id.0], None, &mut out);
    out.into_iter()
        .next()
        .expect("a watched unit is listed, orders or not")
}

#[test]
fn a_stalled_engineer_gets_its_share_of_what_it_wants() {
    let mut w = world();
    let eng = add(&mut w, "aster_t1_engineer", 0, 400, 400);
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    let site = mc_sim::snap_to_build_grid(w.blueprints.unit(power), FxVec2::from_ints(440, 400));
    w.state.players[0].mass = Fx::from_int(5000);
    w.state.players[0].energy = Fx::from_int(50_000);
    w.tick(&order(Command::Build {
        units: vec![eng],
        blueprint: power,
        pos: site,
        heading: Angle::ZERO,
        queue: false,
    }))
    .unwrap();
    let mut full = None;
    for _ in 0..200 {
        // Nothing here stores resources, so the stock is topped up before every tick.
        w.state.players[0].mass = Fx::from_int(5000);
        w.state.players[0].energy = Fx::from_int(50_000);
        w.tick(&[]).unwrap();
        let e = economy_of(&w, eng);
        if e.mass_wanted > 0.0 {
            full = Some(e);
            break;
        }
    }
    let full = full.expect("the engineer began building");
    assert!(full.energy_wanted > 0.0);
    assert!((full.mass_used - full.mass_wanted).abs() < 1e-3, "{full:?}");
    assert_eq!(full.efficiency, 1.0);

    // A third of a tick's want in store: the side stalls and the engineer slows.
    let per_tick = Fx::from_f32(full.mass_wanted / 10.0);
    w.state.players[0].mass = per_tick / 3;
    w.state.players[0].energy = Fx::from_int(50_000);
    w.tick(&[]).unwrap();
    let stalled = economy_of(&w, eng);
    assert!(
        stalled.efficiency < 0.5 && stalled.efficiency > 0.2,
        "{stalled:?}"
    );
    assert!((stalled.mass_wanted - full.mass_wanted).abs() < 1e-3);
    let expect = stalled.mass_wanted * stalled.efficiency;
    assert!((stalled.mass_used - expect).abs() < 1e-2, "{stalled:?}");
    assert!((stalled.energy_used - stalled.energy_wanted * stalled.efficiency).abs() < 1e-2);
}

#[test]
fn mines_and_radar_report_their_flows() {
    let mut w = world();
    let mex = add(&mut w, "aster_core_mine", 0, 606, 606);
    let radar = add(&mut w, "aster_t1_radar", 0, 700, 600);
    w.state.players[0].energy = Fx::from_int(50_000);
    w.tick(&[]).unwrap();
    let m = w.bp(row(&w, mex)).mine.unwrap();
    let income = w.state.mines.by_unit[&mex].rate(&m).to_f32();
    let m = economy_of(&w, mex);
    assert!(income > 0.0);
    assert!((m.mass_made - income).abs() < 1e-3, "{m:?}");
    assert_eq!((m.mass_wanted, m.mass_used), (0.0, 0.0));
    let upkeep = w.bp(row(&w, radar)).economy.energy_upkeep.to_f32();
    let r = economy_of(&w, radar);
    assert!(upkeep > 0.0);
    assert!((r.energy_wanted - upkeep).abs() < 1e-3, "{r:?}");
    assert!((r.energy_used - upkeep).abs() < 1e-3, "{r:?}");
    assert!(r.orders.is_empty());
}
