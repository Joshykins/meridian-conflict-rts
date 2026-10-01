//! Aircraft on guard: they circle halfway out from the centre, a group in
//! formation, break off to fight what comes into the area, then circle again.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
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
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(player: u8, command: Command) -> PlayerCommand {
    PlayerCommand { player, command }
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

/// Guards an area of `area` metres round `centre`: aircraft circle at half that.
fn guard(w: &mut World, rows: &[usize], centre: FxVec2, area: i32) {
    let ids = rows.iter().map(|&r| w.state.units.id(r)).collect();
    w.tick(&[cmd(
        0,
        Command::Guard {
            units: ids,
            pos: centre,
            target: mc_sim::Handle::NONE,
            radius: Fx::from_int(area),
            queue: false,
        },
    )])
    .unwrap();
}

/// Nearest and farthest any of `rows` came from `centre` over `ticks`.
fn band(w: &mut World, rows: &[usize], centre: FxVec2, ticks: u32) -> (f64, f64) {
    let (mut near, mut far) = (f64::MAX, 0.0f64);
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
        for &r in rows {
            let d = w.state.units.pos[r].distance(centre).to_f64();
            near = near.min(d);
            far = far.max(d);
        }
    }
    (near, far)
}

#[test]
fn aircraft_circle_halfway_out() {
    let mut w = world();
    let centre = FxVec2::from_ints(2000, 2000);
    let scout = add(&mut w, "aster_t1_air_scout", 0, 2000, 2400);
    guard(&mut w, &[scout], centre, 800);
    band(&mut w, &[scout], centre, 400);
    let (near, far) = band(&mut w, &[scout], centre, 400);
    assert!(
        near > 360.0 && far < 440.0,
        "orbit band {near:.0}..{far:.0}"
    );
}

#[test]
fn the_argus_orbits_on_the_whole_ring_and_circles_where_it_is_left_idle() {
    let mut w = world();
    let centre = FxVec2::from_ints(2000, 2000);
    let argus = add(&mut w, "aster_t3_support", 0, 2000, 2400);
    guard(&mut w, &[argus], centre, 500);
    band(&mut w, &[argus], centre, 600);
    let (near, far) = band(&mut w, &[argus], centre, 400);
    assert!(
        near > 440.0 && far < 560.0,
        "orbit band {near:.0}..{far:.0}"
    );

    // Stopped, it takes up an orbit of its own where it is, 400 m round.
    let id = w.state.units.id(argus);
    w.tick(&[cmd(0, Command::Stop { units: vec![id] })])
        .unwrap();
    w.tick(&[]).unwrap();
    let o = *w
        .state
        .orders
        .front(&w.state.units, argus)
        .expect("orbiting");
    assert_eq!(o.kind, mc_sim::tables::OrderKind::Guard);
    assert_eq!(o.radius, Fx::from_int(400));
    let here = o.pos;
    band(&mut w, &[argus], here, 900);
    let (near, far) = band(&mut w, &[argus], here, 400);
    assert!(
        near > 340.0 && far < 460.0,
        "idle orbit band {near:.0}..{far:.0}"
    );
}

#[test]
fn a_group_circles_in_formation() {
    let mut w = world();
    let centre = FxVec2::from_ints(2000, 2000);
    let rows: Vec<_> = (0..5)
        .map(|i| add(&mut w, "aster_t1_air_scout", 0, 1400 + i * 30, 1400))
        .collect();
    guard(&mut w, &rows, centre, 700);
    for &r in &rows {
        let o = w.state.orders.front(&w.state.units, r).unwrap();
        assert_eq!(o.kind, OrderKind::Guard);
        assert_ne!(o.formation, 0, "the group shares a formation");
    }
    band(&mut w, &rows, centre, 600);
    let mut spread = 0.0f64;
    let mut closest = f64::MAX;
    let (near, far) = {
        let (mut near, mut far) = (f64::MAX, 0.0f64);
        for _ in 0..400 {
            w.tick(&[]).unwrap();
            for (i, &a) in rows.iter().enumerate() {
                let d = w.state.units.pos[a].distance(centre).to_f64();
                near = near.min(d);
                far = far.max(d);
                for &b in &rows[i + 1..] {
                    let gap = w.state.units.pos[a].distance(w.state.units.pos[b]).to_f64();
                    spread = spread.max(gap);
                    closest = closest.min(gap);
                }
            }
        }
        (near, far)
    };
    assert!(
        near > 250.0 && far < 450.0,
        "orbit band {near:.0}..{far:.0}"
    );
    // Held together in a V rather than strung round the circle or stacked up.
    assert!(spread < 150.0, "the flight spread over {spread:.0} m");
    assert!(closest > 8.0, "two aircraft flew {closest:.1} m apart");
}

#[test]
fn guarding_bombers_break_off_to_attack_then_circle_again() {
    let mut w = world();
    let centre = FxVec2::from_ints(2000, 2000);
    let rows: Vec<_> = (0..2)
        .map(|i| add(&mut w, "aster_t1_bomber", 0, 2000 + i * 40, 2300))
        .collect();
    let tank = add(&mut w, "aster_t1_tank", 1, 2250, 2000);
    let tank_id = w.state.units.id(tank);
    guard(&mut w, &rows, centre, 600);
    let mut ticks = 0;
    while w.state.units.row(tank_id).is_some() {
        w.tick(&[]).unwrap();
        ticks += 1;
        assert!(
            ticks < 3000,
            "the bombers never killed the tank near their circle"
        );
    }
    // The chase ends on the orders pass after the kill.
    w.tick(&[]).unwrap();
    for &r in &rows {
        let o = w.state.orders.front(&w.state.units, r).unwrap();
        assert_eq!(o.kind, OrderKind::Guard, "still on guard");
    }
    band(&mut w, &rows, centre, 600);
    let (near, far) = band(&mut w, &rows, centre, 400);
    assert!(
        near > 200.0 && far < 420.0,
        "back on the circle: {near:.0}..{far:.0}"
    );
}

#[test]
fn aircraft_on_guard_ignore_enemies_far_from_the_area() {
    let mut w = world();
    let centre = FxVec2::from_ints(1000, 1000);
    let bomber = add(&mut w, "aster_t1_bomber", 0, 1000, 1300);
    let tank = add(&mut w, "aster_t1_tank", 1, 3500, 3500);
    let tank_id = w.state.units.id(tank);
    guard(&mut w, &[bomber], centre, 600);
    let (_, far) = band(&mut w, &[bomber], centre, 900);
    assert!(far < 450.0, "wandered {far:.0} m off the circle");
    assert!(w.state.units.row(tank_id).is_some());
}

#[test]
fn a_queued_order_ends_a_guard_at_once() {
    let mut w = world();
    let centre = FxVec2::from_ints(2000, 2000);
    let lone = add(&mut w, "aster_t1_air_scout", 0, 2000, 2300);
    let rows: Vec<_> = (0..4)
        .map(|i| add(&mut w, "aster_t1_air_scout", 0, 1400 + i * 30, 1400))
        .collect();
    guard(&mut w, &[lone], centre, 600);
    guard(&mut w, &rows, centre, 700);
    band(&mut w, &rows, centre, 200);
    let away = FxVec2::from_ints(3000, 1000);
    let ids = |rows: &[usize]| {
        rows.iter()
            .map(|&r| w.state.units.id(r))
            .collect::<Vec<_>>()
    };
    let (one, group) = (ids(&[lone]), ids(&rows));
    w.tick(&[
        cmd(
            0,
            Command::Move {
                units: one,
                target: away,
                queue: true,
            },
        ),
        cmd(
            0,
            Command::FormationMove {
                units: group,
                target: away,
                queue: true,
                attack_move: false,
                facing: None,
                together: true,
                spacing: 0,
                shape: 0,
            },
        ),
    ])
    .unwrap();
    w.tick(&[]).unwrap();
    for &r in rows.iter().chain([&lone]) {
        let kinds: Vec<_> = w
            .state
            .orders
            .iter(&w.state.units, r)
            .map(|o| o.kind)
            .collect();
        assert_eq!(kinds, vec![OrderKind::Move], "the guard gave way");
    }
    let before = w.state.units.pos[lone].distance(away);
    band(&mut w, &[lone], centre, 100);
    assert!(
        w.state.units.pos[lone].distance(away) < before,
        "heading for the queued move"
    );
}

#[test]
fn a_dragged_centre_moves_the_circle() {
    let mut w = world();
    let (from, to) = (FxVec2::from_ints(2000, 2000), FxVec2::from_ints(2800, 2400));
    let rows: Vec<_> = (0..3)
        .map(|i| add(&mut w, "aster_t1_air_scout", 0, 1900 + i * 30, 2300))
        .collect();
    guard(&mut w, &rows, from, 700);
    band(&mut w, &rows, from, 300);
    let ids = rows.iter().map(|&r| w.state.units.id(r)).collect();
    w.tick(&[cmd(
        0,
        Command::RelocateOrder {
            units: ids,
            kind: OrderKind::Guard,
            from,
            to,
        },
    )])
    .unwrap();
    for &r in &rows {
        let o = w.state.orders.front(&w.state.units, r).unwrap();
        assert_eq!(
            (o.kind, o.pos, o.radius),
            (OrderKind::Guard, to, Fx::from_int(700))
        );
    }
    band(&mut w, &rows, to, 900);
    let (near, far) = band(&mut w, &rows, to, 400);
    assert!(
        near > 250.0 && far < 450.0,
        "orbit band {near:.0}..{far:.0}"
    );
}

#[test]
fn dragging_a_followed_guard_leaves_the_unit() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 0, 2000, 2000);
    let scout = add(&mut w, "aster_t1_air_scout", 0, 2000, 2300);
    let (tank_id, scout_id) = (w.state.units.id(tank), w.state.units.id(scout));
    w.tick(&[cmd(
        0,
        Command::Guard {
            units: vec![scout_id],
            pos: w.state.units.pos[tank],
            target: tank_id,
            radius: Fx::from_int(600),
            queue: false,
        },
    )])
    .unwrap();
    // The player picks the centre up where it was a moment ago; the tank has since moved on.
    let seen = w.state.units.pos[tank];
    w.tick(&[cmd(
        0,
        Command::Move {
            units: vec![tank_id],
            target: FxVec2::from_ints(2600, 2000),
            queue: false,
        },
    )])
    .unwrap();
    for _ in 0..20 {
        w.tick(&[]).unwrap();
    }
    assert_ne!(
        w.state.orders.front(&w.state.units, scout).unwrap().pos,
        seen
    );
    let to = FxVec2::from_ints(1400, 1400);
    w.tick(&[cmd(
        0,
        Command::RelocateOrder {
            units: vec![scout_id],
            kind: OrderKind::Guard,
            from: seen,
            to,
        },
    )])
    .unwrap();
    for _ in 0..200 {
        w.tick(&[]).unwrap();
    }
    let o = w.state.orders.front(&w.state.units, scout).unwrap();
    assert_eq!(
        (o.kind, o.pos),
        (OrderKind::Guard, to),
        "circles the new spot, not the tank"
    );
}

#[test]
fn a_guarding_bomber_holds_one_mark_through_its_run_over_a_crowd() {
    // A loose army across the area, and a bomber flying in to it from its
    // factory well outside: the enemy nearest the bomber changes every few
    // ticks, and a pass needs one mark from the run in to the release line, the
    // same one its bay aims at (mark 2 of 20260930-194544: a Hellkite flew over
    // a whole army on guard at one enemy with its bay on another, and never
    // opened it).
    let mut w = world();
    let centre = FxVec2::from_ints(2000, 3000);
    let bomber = add(&mut w, "aster_t2_fire_bomber", 0, 1600, 150);
    let drive: Vec<_> = (0..12)
        .map(|i| {
            let (x, y) = (
                1700 + (i * 7 % 5) * 250 + (i / 5) * 40,
                2000 + (i / 5) * 200,
            );
            let row = add(&mut w, "aster_t1_tank", 1, x, y);
            cmd(
                1,
                Command::Move {
                    units: vec![w.state.units.id(row)],
                    target: FxVec2::from_ints(x, y + 1500),
                    queue: false,
                },
            )
        })
        .collect();
    w.tick(&drive).unwrap();
    guard(&mut w, &[bomber], centre, 1500);
    let hellkite = w.state.units.blueprint[bomber];
    let (mut bombs, mut passes, mut quiet) = (0, 0, u32::MAX);
    for _ in 0..1200 {
        w.tick(&[]).unwrap();
        let dropped = w
            .events
            .iter()
            .filter(|e| {
                matches!(e, SimEvent::ShotFired { blueprint, weapon: 0, .. } if *blueprint == hellkite)
            })
            .count();
        if dropped > 0 && quiet > 30 {
            // The bay opened: on the mark the pilot is flying at.
            passes += 1;
            let units = &w.state.units;
            let o = w.state.orders.front(units, bomber).unwrap();
            assert_eq!(o.kind, OrderKind::Attack, "bombing from the guard itself");
            assert_eq!(units.weapon_target[bomber][0], o.target, "bay off the mark");
        }
        quiet = if dropped > 0 {
            0
        } else {
            quiet.saturating_add(1)
        };
        bombs += dropped;
    }
    // A carpet is 24 bombs, one every 9 s at most.
    assert!(
        passes >= 3 && bombs >= 72,
        "{bombs} bombs in {passes} passes in 120 s over the army"
    );
}
