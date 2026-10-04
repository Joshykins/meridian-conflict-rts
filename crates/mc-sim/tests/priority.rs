//! A unit's own priority in a stall (`Command::SetPriority`): First or Last overrides
//! the side's focus for all its work, helpers take their leader's, builders on a site
//! the site's, and only its own side sees it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::focus::{Focus, Priority};
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "priority".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1800, 1800)],
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
        players: vec![player("you", 0), player("them", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        Heightfield::flat(128, 128, Fx::from_int(20)),
        map,
        blueprints,
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// One `key` of player 0's at (x, y), `build` permille done; its id.
fn spawn(w: &mut World, key: &str, x: i32, y: i32, build: u16) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let at = FxVec2::from_ints(x, y);
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint,
        pos: at,
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build,
    })])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == blueprint && u.owner[r] == 0)
        .min_by_key(|&r| u.pos[r].distance_sq(at))
        .unwrap();
    u.id(row)
}

fn set_priority(units: Vec<UnitId>, priority: Priority) -> PlayerCommand {
    cmd(Command::SetPriority { units, priority })
}

/// A tick with no energy in store and materials to spare.
fn starve(w: &mut World) {
    w.state.players[0].energy = Fx::ZERO;
    w.state.players[0].mass = Fx::from_int(1000);
    w.tick(&[]).unwrap();
}

/// What `race` sets up: two power plant sites, one engineer on each.
struct Race {
    site_a: UnitId,
    site_b: UnitId,
    a: UnitId,
    b: UnitId,
}

/// Two power plant sites with one engineer on each, on 20 energy a second with none in
/// store: each site asks for 18. `orders` gives the first tick's priorities and focus.
/// How fast each site goes up, in build time a second.
fn race(orders: impl FnOnce(&Race) -> Vec<PlayerCommand>) -> (f64, f64) {
    let mut w = world();
    spawn(&mut w, "aster_t1_power", 300, 300, 1000);
    w.state.players[0].income_permille[1] = 800;
    spawn(&mut w, "aster_mass_storage", 330, 300, 1000);
    let r = Race {
        site_a: spawn(&mut w, "aster_t1_power", 500, 500, 100),
        site_b: spawn(&mut w, "aster_t1_power", 600, 500, 100),
        a: spawn(&mut w, "aster_t1_engineer", 520, 470, 1000),
        b: spawn(&mut w, "aster_t1_engineer", 580, 470, 1000),
    };
    let mut first = vec![
        cmd(Command::Assist {
            units: vec![r.a],
            target: r.site_a,
            queue: false,
        }),
        cmd(Command::Assist {
            units: vec![r.b],
            target: r.site_b,
            queue: false,
        }),
    ];
    first.extend(orders(&r));
    w.tick(&first).unwrap();
    for _ in 0..20 {
        starve(&mut w);
    }
    let progress =
        |w: &World, id: UnitId| w.state.units.build_progress[w.state.units.row(id).unwrap()];
    let (a0, b0) = (progress(&w, r.site_a), progress(&w, r.site_b));
    for _ in 0..50 {
        starve(&mut w);
    }
    (
        (progress(&w, r.site_a) - a0).to_f64() / 5.0,
        (progress(&w, r.site_b) - b0).to_f64() / 5.0,
    )
}

#[test]
fn a_first_engineer_builds_at_full_speed_while_the_rest_shares_what_is_left() {
    let (a, b) = race(|r| vec![set_priority(vec![r.a], Priority::First)]);
    assert!((a - 5.0).abs() < 0.1, "first built at {a} a second");
    assert!(b > 0.0 && b < 1.0, "even built at {b} a second");
}

#[test]
fn a_last_engineer_builds_only_out_of_what_the_rest_leaves() {
    let (a, b) = race(|r| vec![set_priority(vec![r.a], Priority::Last)]);
    let (even, _) = race(|_| Vec::new());
    assert!(b > even * 1.5, "the other built at {b}, {even} when even");
    assert!(a < b * 0.25, "last built at {a}, the other at {b}");
}

#[test]
fn a_units_own_priority_overrides_the_focus_and_even_follows_it() {
    // Power Last holds both power sites back, but engineer a's own First wins.
    let (a, b) = race(|r| {
        vec![
            cmd(Command::SetFocus {
                focus: Focus {
                    mines: Priority::Even,
                    power: Priority::Last,
                },
            }),
            set_priority(vec![r.a], Priority::First),
        ]
    });
    assert!((a - 5.0).abs() < 0.1, "first built at {a} a second");
    assert!(b < 1.0, "the focus still puts the other last: {b}");
    // Power First and nothing of its own: both go first with the focus, and share alike.
    let (a, b) = race(|_| {
        vec![cmd(Command::SetFocus {
            focus: Focus {
                mines: Priority::Even,
                power: Priority::First,
            },
        })]
    });
    assert!((a - b).abs() < 0.1, "{a} vs {b}");
}

#[test]
fn everyone_building_a_prioritised_site_is_paid_first() {
    let (a, b) = race(|r| vec![set_priority(vec![r.site_a], Priority::First)]);
    assert!(
        (a - 5.0).abs() < 0.1,
        "the first site built at {a} a second"
    );
    assert!(b < 1.0, "the other at {b}");
    // The engineer's own setting beats the site's.
    let (a, _) = race(|r| {
        vec![
            set_priority(vec![r.site_a], Priority::First),
            set_priority(vec![r.a], Priority::Last),
        ]
    });
    assert!(a < 1.0, "the engineer's Last held the first site back: {a}");
}

#[test]
fn helpers_take_the_priority_of_the_factory_they_assist() {
    let mut w = world();
    spawn(&mut w, "aster_t1_power", 300, 300, 1000);
    spawn(&mut w, "aster_mass_storage", 330, 300, 1000);
    let first = spawn(&mut w, "aster_t1_land_factory", 500, 600, 1000);
    let even = spawn(&mut w, "aster_t1_land_factory", 700, 600, 1000);
    let h_first = spawn(&mut w, "aster_t1_engineer", 500, 520, 1000);
    let h_even = spawn(&mut w, "aster_t1_engineer", 700, 520, 1000);
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let mut orders = vec![set_priority(vec![first], Priority::First)];
    for (factory, helper) in [(first, h_first), (even, h_even)] {
        orders.push(cmd(Command::Produce {
            factories: vec![factory],
            blueprint: tank,
            count: 5,
        }));
        orders.push(cmd(Command::Assist {
            units: vec![helper],
            target: factory,
            queue: false,
        }));
    }
    w.tick(&orders).unwrap();
    for _ in 0..200 {
        starve(&mut w);
    }
    let paid = |w: &World, id: UnitId| {
        let f = &w.flows[w.state.units.row(id).unwrap()];
        assert!(f.wanted[1] > Fx::ZERO, "{id:?} is at work");
        (f.used[1] / f.wanted[1]).to_f64()
    };
    let (f, h, e) = (paid(&w, first), paid(&w, h_first), paid(&w, h_even));
    assert!((h - f).abs() < 0.01, "helper paid {h}, its factory {f}");
    assert!(
        h > e + 0.1,
        "helper of the first factory {h}, of the other {e}"
    );
}

#[test]
fn only_units_with_work_or_sites_take_a_priority_and_a_finished_plant_drops_it() {
    let mut w = world();
    w.tick(&[cmd(Command::DebugFreeBuild {
        player: 0,
        on: true,
    })])
    .unwrap();
    let tank = spawn(&mut w, "aster_t1_tank", 400, 500, 1000);
    let site = spawn(&mut w, "aster_t1_power", 500, 500, 100);
    let mason = spawn(&mut w, "aster_t1_engineer", 520, 470, 1000);
    w.tick(&[set_priority(vec![tank, site, mason], Priority::First)])
        .unwrap();
    let priority = |w: &World, id: UnitId| w.state.units.priority[w.state.units.row(id).unwrap()];
    assert_eq!(
        priority(&w, tank),
        Priority::Even,
        "a tank has no work to order"
    );
    assert_eq!(priority(&w, site), Priority::First);
    assert_eq!(priority(&w, mason), Priority::First);
    // Another player cannot change them.
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::SetPriority {
            units: vec![mason],
            priority: Priority::Last,
        },
    }])
    .unwrap();
    assert_eq!(priority(&w, mason), Priority::First);

    w.tick(&[cmd(Command::Assist {
        units: vec![mason],
        target: site,
        queue: false,
    })])
    .unwrap();
    let built = (0..2000).any(|_| {
        w.tick(&[]).unwrap();
        let row = w.state.units.row(site).unwrap();
        !w.state
            .units
            .has_flag(row, mc_sim::tables::flag::UNDER_CONSTRUCTION)
    });
    assert!(built, "the plant was finished");
    assert_eq!(
        priority(&w, site),
        Priority::Even,
        "a power plant has no work of its own to order"
    );
}

#[test]
fn only_its_own_side_sees_a_units_priority() {
    let mut w = world();
    let mason = spawn(&mut w, "aster_t1_engineer", 500, 500, 1000);
    w.tick(&[set_priority(vec![mason], Priority::Last)])
        .unwrap();
    let mut frame = mc_sim::mirror::RenderFrame::default();
    let mark = |w: &World, frame: &mut mc_sim::mirror::RenderFrame, viewer| {
        w.write_render_frame(viewer, frame);
        frame
            .units
            .iter()
            .find(|u| u.unit_id == mason.0)
            .expect("drawn")
            .priority()
    };
    assert_eq!(mark(&w, &mut frame, Some(0)), Priority::Last);
    assert_eq!(
        mark(&w, &mut frame, None),
        Priority::Last,
        "observers see it"
    );
    assert_eq!(
        mark(&w, &mut frame, Some(1)),
        Priority::Even,
        "the enemy does not"
    );
}
