//! Changing the formation settings re-forms orders already given: a circling
//! flight opens out, breaks up to circle free, and closes back into a V.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, Order, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
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

fn tick(w: &mut World, command: Command) {
    w.tick(&[PlayerCommand { player: 0, command }]).unwrap();
}

fn fronts(w: &World, rows: &[usize]) -> Vec<Order> {
    rows.iter()
        .map(|&r| *w.state.orders.front(&w.state.units, r).unwrap())
        .collect()
}

fn widest(orders: &[Order]) -> f64 {
    orders
        .iter()
        .map(|o| o.offset.length().to_f64())
        .fold(0.0, f64::max)
}

#[test]
fn a_circling_flight_takes_new_settings_at_once() {
    let mut w = world();
    let scout = w.blueprints.id_of("aster_t1_air_scout").unwrap();
    let rows: Vec<usize> = (0..5)
        .map(|i| {
            w.spawn_unit(
                scout,
                0,
                FxVec2::from_ints(1400 + i * 30, 1400),
                Angle::ZERO,
                true,
            )
            .unwrap()
        })
        .collect();
    let ids: Vec<_> = rows.iter().map(|&r| w.state.units.id(r)).collect();
    let centre = FxVec2::from_ints(2000, 2000);
    tick(
        &mut w,
        Command::Orbit {
            units: ids.clone(),
            pos: centre,
            target: mc_sim::Handle::NONE,
            radius: Fx::from_int(350),
            queue: false,
        },
    );
    for _ in 0..100 {
        w.tick(&[]).unwrap();
    }
    let standard = fronts(&w, &rows);

    tick(
        &mut w,
        Command::Reform {
            units: ids.clone(),
            together: true,
            spacing: 2,
        },
    );
    let wide = fronts(&w, &rows);
    assert!(wide
        .iter()
        .all(|o| o.kind == OrderKind::Orbit && o.pos == centre));
    assert!(wide
        .iter()
        .all(|o| o.formation != 0 && o.formation == wide[0].formation));
    assert_ne!(
        wide[0].formation, standard[0].formation,
        "a new group forms up"
    );
    assert!(
        widest(&wide) > widest(&standard) * 1.3,
        "wide {:.0} vs standard {:.0}",
        widest(&wide),
        widest(&standard)
    );

    tick(
        &mut w,
        Command::Reform {
            units: ids.clone(),
            together: false,
            spacing: 2,
        },
    );
    let free = fronts(&w, &rows);
    assert!(free
        .iter()
        .all(|o| o.kind == OrderKind::Orbit && o.formation == 0 && o.offset == FxVec2::ZERO));

    tick(
        &mut w,
        Command::Reform {
            units: ids,
            together: true,
            spacing: 0,
        },
    );
    let compact = fronts(&w, &rows);
    assert!(compact.iter().all(|o| o.formation != 0));
    assert!(
        widest(&compact) < widest(&standard),
        "compact {:.0} vs standard {:.0}",
        widest(&compact),
        widest(&standard)
    );
}

#[test]
fn a_move_keeps_its_destination_and_queue() {
    let mut w = world();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let rows: Vec<usize> = (0..6)
        .map(|i| {
            w.spawn_unit(
                tank,
                0,
                FxVec2::from_ints(800 + i * 12, 800),
                Angle::ZERO,
                true,
            )
            .unwrap()
        })
        .collect();
    let ids: Vec<_> = rows.iter().map(|&r| w.state.units.id(r)).collect();
    tick(
        &mut w,
        Command::Move {
            units: ids.clone(),
            target: FxVec2::from_ints(1400, 800),
            queue: false,
        },
    );
    tick(
        &mut w,
        Command::Move {
            units: ids.clone(),
            target: FxVec2::from_ints(1400, 1400),
            queue: true,
        },
    );
    let before = fronts(&w, &rows);
    tick(
        &mut w,
        Command::Reform {
            units: ids,
            together: false,
            spacing: 2,
        },
    );
    for (&r, b) in rows.iter().zip(&before) {
        let q: Vec<_> = w.state.orders.iter(&w.state.units, r).copied().collect();
        assert_eq!(q.len(), 2, "the queue is kept");
        assert_eq!(q[0].pos, b.pos, "same destination");
        assert_eq!(q[0].formation, 0, "moving free");
    }
    assert!(widest(&fronts(&w, &rows)) > widest(&before));
}

#[test]
fn a_patrol_keeps_each_unit_in_one_slot_round_the_loop() {
    let mut w = world();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let rows: Vec<usize> = (0..6)
        .map(|i| {
            w.spawn_unit(
                tank,
                0,
                FxVec2::from_ints(800 + i * 12, 800),
                Angle::ZERO,
                true,
            )
            .unwrap()
        })
        .collect();
    let ids: Vec<_> = rows.iter().map(|&r| w.state.units.id(r)).collect();
    let points = vec![
        FxVec2::from_ints(1400, 800),
        FxVec2::from_ints(1400, 1400),
        FxVec2::from_ints(800, 1400),
    ];
    tick(
        &mut w,
        Command::Patrol {
            units: ids.clone(),
            points,
            queue: false,
        },
    );
    tick(
        &mut w,
        Command::Reform {
            units: ids,
            together: true,
            spacing: 2,
        },
    );
    let legs =
        |r: usize| -> Vec<Order> { w.state.orders.iter(&w.state.units, r).copied().collect() };
    let leg_count = legs(rows[0]).len();
    assert_eq!(leg_count, 3);
    for leg in 0..leg_count {
        let formation = legs(rows[0])[leg].formation;
        assert_ne!(formation, 0);
        assert!(
            rows.iter().all(|&r| legs(r)[leg].formation == formation),
            "one group per leg"
        );
    }
    for &r in &rows {
        let local: Vec<FxVec2> = legs(r)
            .iter()
            .map(|o| o.offset.rotate(-o.heading))
            .collect();
        assert!(
            local.iter().all(|p| p.distance(local[0]) < Fx::ONE),
            "slot turned with each leg"
        );
    }
}
