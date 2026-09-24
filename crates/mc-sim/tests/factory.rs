//! A land factory is refitted in place, like the commander: the next suite is
//! bolted onto the same unit. Finished products walk out of the bay.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, mc_core::Fx::from_int(20));
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

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn(w: &World, key: &str, x: i32) -> PlayerCommand {
    spawn_at(w, key, FxVec2::from_ints(x, 512), Angle::ZERO)
}

fn spawn_at(w: &World, key: &str, pos: FxVec2, heading: Angle) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos,
        heading,
        count: 1,
        flags: 0,
        build: 1000,
    })
}

fn with_factory() -> (World, UnitId) {
    let mut w = world();
    w.tick(&[
        spawn(&w, "aster_t1_land_factory", 600),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let id = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == factory)
        .map(|r| w.state.units.id(r))
        .expect("the factory spawned");
    (w, id)
}

#[test]
fn a_factory_is_refitted_in_place_and_stays_the_same_unit() {
    let (mut w, fid) = with_factory();
    let (t1, t2) = (
        w.blueprints.id_of("aster_t1_land_factory").unwrap(),
        w.blueprints.id_of("aster_t2_land_factory").unwrap(),
    );
    let row = w.state.units.row(fid).unwrap();

    w.tick(&[cmd(Command::Upgrade { units: vec![fid] })])
        .unwrap();
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(OrderKind::Upgrade),
        "the refit is an order in its queue"
    );

    let mut shown = 0.0f32;
    let mut frame = mc_sim::RenderFrame::default();
    let done = (0..2500).any(|_| {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        assert_eq!(
            frame.units.iter().filter(|u| u.unit_id == fid.0).count(),
            1,
            "the refit is never drawn as a second unit"
        );
        let u = frame
            .units
            .iter()
            .find(|u| u.unit_id == fid.0)
            .expect("the factory stays on the map");
        assert_eq!(
            u.owner_flags & (flag::UNDER_CONSTRUCTION as u32) << 8,
            0,
            "the foundry is not rebuilt from the weld"
        );
        shown = shown.max(u.upgrade);
        w.state.units.blueprint[row] == t2
    });
    assert!(done, "the refit never finished");
    assert!(
        shown > 0.9,
        "the mirror reported the refit's progress ({shown})"
    );
    assert_eq!(w.state.units.row(fid), Some(row), "same unit, same id");
    assert_eq!(w.state.units.blueprint[row], t2, "it is now the next tier");
    assert_eq!(
        w.state.units.slots.iter().count(),
        1,
        "nothing is left behind"
    );
    let _ = t1;
}

#[test]
fn a_factory_rolls_a_finished_unit_out_of_the_bay() {
    let (mut w, fid) = with_factory();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    w.tick(&[cmd(Command::Produce {
        factories: vec![fid],
        blueprint: tank,
        count: 1,
    })])
    .unwrap();

    let mut product = None;
    let mut saw_walk = false;
    let mut last_x = Fx::from_int(-10_000);
    for _ in 0..800 {
        w.tick(&[]).unwrap();
        let factory = w.state.units.row(fid).unwrap();
        let Some(t) = w
            .state
            .units
            .slots
            .iter()
            .find(|&r| w.state.units.blueprint[r] == tank)
        else {
            continue;
        };
        product = Some(t);
        let x = w.state.units.pos[t].x;
        if w.state.units.has_flag(t, flag::IN_FACTORY)
            && !w.state.units.has_flag(t, flag::UNDER_CONSTRUCTION)
            && x > last_x + Fx::from_int(1)
        {
            saw_walk = true;
            assert!(
                w.state.units.gait[t] > 0,
                "the hull walks: gait {}",
                w.state.units.gait[t]
            );
            assert!(
                w.state.units.pos[t].x > w.state.units.pos[factory].x - Fx::from_int(20),
                "it leaves toward the exit, not the rear"
            );
        }
        last_x = x;
        if w.state.units.is_active(t) && !w.state.units.has_flag(t, flag::IN_FACTORY) {
            break;
        }
    }
    let t = product.expect("the factory built a tank");
    assert!(saw_walk, "the tank walked out of the bay");
    assert!(
        w.state.units.is_active(t) && !w.state.units.has_flag(t, flag::IN_FACTORY),
        "it left the factory"
    );
    let factory = w.state.units.row(fid).unwrap();
    assert!(
        w.state.units.pos[t].x > w.state.units.pos[factory].x + Fx::from_int(40),
        "it cleared the lot ({:?} vs factory {:?})",
        w.state.units.pos[t],
        w.state.units.pos[factory]
    );
}

#[test]
fn assisting_a_factory_puts_a_build_beam_on_it() {
    let (mut w, fid) = with_factory();
    w.tick(&[spawn(&w, "aster_t1_engineer", 520)]).unwrap();
    let mason = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == w.blueprints.id_of("aster_t1_engineer").unwrap())
        .map(|r| w.state.units.id(r))
        .expect("the engineer spawned");
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    w.tick(&[
        cmd(Command::Produce {
            factories: vec![fid],
            blueprint: tank,
            count: 1,
        }),
        cmd(Command::Assist {
            units: vec![mason],
            target: fid,
            queue: false,
        }),
    ])
    .unwrap();

    let mut frame = mc_sim::RenderFrame::default();
    let mut hit = None;
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        hit = frame
            .build_sources
            .iter()
            .find(|(id, _)| *id == mason.0)
            .map(|(_, to)| *to);
        if hit.is_some() {
            break;
        }
    }
    let hit = hit.expect("the engineer put a construction beam on the factory");
    let work = frame
        .units
        .iter()
        .find(|u| u.unit_id == fid.0)
        .expect("the factory is on the map");
    let (dx, dy) = (hit[0] - work.pos[0], hit[1] - work.pos[1]);
    let reach = (dx * dx + dy * dy).sqrt();
    assert!(
        reach > work.radius * 0.5 && reach < work.radius * 1.15,
        "the beam hits the factory ({reach} m out of radius {})",
        work.radius
    );
    let engineer = w.state.units.row(mason).unwrap();
    assert!(
        w.state.units.has_flag(engineer, flag::BUILDING),
        "the engineer is feeding the bay"
    );
}

#[test]
fn an_air_factory_prints_a_fighter_that_takes_off() {
    let mut w = world();
    w.tick(&[
        spawn(&w, "aster_t1_air_factory", 600),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let factory_bp = w.blueprints.id_of("aster_t1_air_factory").unwrap();
    let fid = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == factory_bp)
        .map(|r| w.state.units.id(r))
        .expect("the aerie spawned");
    let interceptor = w.blueprints.id_of("aster_t1_interceptor").unwrap();
    let cruise = w.blueprints.unit(interceptor).motion.unwrap().altitude;
    w.tick(&[cmd(Command::Produce {
        factories: vec![fid],
        blueprint: interceptor,
        count: 1,
    })])
    .unwrap();

    let mut product = None;
    for _ in 0..800 {
        w.tick(&[]).unwrap();
        let Some(t) = w
            .state
            .units
            .slots
            .iter()
            .find(|&r| w.state.units.blueprint[r] == interceptor)
        else {
            continue;
        };
        product = Some(t);
        if w.state.units.is_active(t) && !w.state.units.has_flag(t, flag::IN_FACTORY) {
            break;
        }
    }
    let t = product.expect("the factory built an interceptor");
    assert!(
        w.state.units.is_active(t) && !w.state.units.has_flag(t, flag::IN_FACTORY),
        "it left the factory"
    );
    for _ in 0..40 {
        w.tick(&[]).unwrap();
    }
    let ground = w.state.units.z[w.state.units.row(fid).unwrap()];
    assert!(
        w.state.units.z[t] > ground + cruise / 2,
        "it climbed off the pad ({:?} vs factory {:?})",
        w.state.units.z[t],
        ground
    );
}

#[test]
fn an_air_factory_rolls_a_mason_off_the_apron() {
    let mut w = world();
    w.tick(&[
        spawn(&w, "aster_t1_air_factory", 600),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let factory_bp = w.blueprints.id_of("aster_t1_air_factory").unwrap();
    let fid = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == factory_bp)
        .map(|r| w.state.units.id(r))
        .expect("the aerie spawned");
    let t = produce_mason_from_air_factory(&mut w, fid).expect("the factory built a mason");
    assert!(
        w.state.units.is_active(t) && !w.state.units.has_flag(t, flag::IN_FACTORY),
        "it left the factory"
    );
}

fn produce_mason_from_air_factory(w: &mut World, fid: UnitId) -> Option<usize> {
    let mason = w.blueprints.id_of("aster_t1_engineer").unwrap();
    w.tick(&[cmd(Command::Produce {
        factories: vec![fid],
        blueprint: mason,
        count: 1,
    })])
    .unwrap();
    let mut product = None;
    for _ in 0..800 {
        w.tick(&[]).unwrap();
        let Some(t) = w
            .state
            .units
            .slots
            .iter()
            .find(|&r| w.state.units.blueprint[r] == mason)
        else {
            continue;
        };
        product = Some(t);
        if w.state.units.is_active(t) && !w.state.units.has_flag(t, flag::IN_FACTORY) {
            break;
        }
    }
    product
}

#[test]
fn an_air_factory_facing_south_still_releases_a_mason() {
    let mut w = world();
    w.tick(&[
        spawn_at(
            &w,
            "aster_t1_air_factory",
            FxVec2::from_ints(600, 512),
            Angle::from_degrees(270),
        ),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let factory_bp = w.blueprints.id_of("aster_t1_air_factory").unwrap();
    let fid = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == factory_bp)
        .map(|r| w.state.units.id(r))
        .expect("the aerie spawned");
    let t = produce_mason_from_air_factory(&mut w, fid).expect("the factory built a mason");
    assert!(
        w.state.units.is_active(t) && !w.state.units.has_flag(t, flag::IN_FACTORY),
        "mason finished but never left the south-facing aerie (flags={:?}, pos={:?}, factory={:?})",
        w.state.units.flags[t],
        w.state.units.pos[t],
        w.state.units.pos[w.state.units.row(fid).unwrap()]
    );
}

#[test]
fn an_air_factory_releases_a_mason_when_the_apron_is_blocked() {
    let mut w = world();
    w.tick(&[
        spawn_at(
            &w,
            "aster_t1_air_factory",
            FxVec2::from_ints(600, 600),
            Angle::from_degrees(270),
        ),
        spawn_at(
            &w,
            "aster_t1_land_factory",
            FxVec2::from_ints(600, 600 - 96),
            Angle::from_degrees(270),
        ),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let factory_bp = w.blueprints.id_of("aster_t1_air_factory").unwrap();
    let fid = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == factory_bp)
        .map(|r| w.state.units.id(r))
        .expect("the aerie spawned");
    let t = produce_mason_from_air_factory(&mut w, fid).expect("the factory built a mason");
    assert!(
        w.state.units.is_active(t) && !w.state.units.has_flag(t, flag::IN_FACTORY),
        "mason finished but never left (flags={:?}, pos={:?}, factory={:?})",
        w.state.units.flags[t],
        w.state.units.pos[t],
        w.state.units.pos[w.state.units.row(fid).unwrap()]
    );
}

fn queue(w: &World, id: UnitId) -> Vec<(OrderKind, mc_data::BlueprintId)> {
    let row = w.state.units.row(id).unwrap();
    w.state
        .orders
        .iter(&w.state.units, row)
        .map(|o| (o.kind, o.blueprint))
        .collect()
}

#[test]
fn tiers_queue_one_after_another_and_their_units_wait_for_them() {
    let (mut w, fid) = with_factory();
    let id = |w: &World, k: &str| w.blueprints.id_of(k).unwrap();
    let (t2, t3, tank2) = (
        id(&w, "aster_t2_land_factory"),
        id(&w, "aster_t3_land_factory"),
        id(&w, "aster_t2_tank"),
    );

    // A T2 unit is refused before an upgrade is queued, and taken once one is.
    let produce = |count| {
        cmd(Command::Produce {
            factories: vec![fid],
            blueprint: tank2,
            count,
        })
    };
    w.tick(&[produce(1)]).unwrap();
    assert!(
        queue(&w, fid).is_empty(),
        "a T1 factory cannot make a T2 tank yet"
    );
    w.tick(&[cmd(Command::Upgrade { units: vec![fid] }), produce(2)])
        .unwrap();
    w.tick(&[cmd(Command::Upgrade { units: vec![fid] })])
        .unwrap();
    assert_eq!(
        queue(&w, fid),
        vec![
            (OrderKind::Upgrade, t2),
            (OrderKind::Produce, tank2),
            (OrderKind::Produce, tank2),
            (OrderKind::Upgrade, t3),
        ],
    );
    // The line ends at T3: a third press adds nothing.
    w.tick(&[cmd(Command::Upgrade { units: vec![fid] })])
        .unwrap();
    assert_eq!(queue(&w, fid).len(), 4);

    // Cancelling T2 would take the tanks and T3 with it.
    let (mut w2, f2) = with_factory();
    w2.tick(&[
        cmd(Command::Upgrade { units: vec![f2] }),
        cmd(Command::Produce {
            factories: vec![f2],
            blueprint: tank2,
            count: 2,
        }),
        cmd(Command::Upgrade { units: vec![f2] }),
    ])
    .unwrap();
    assert_eq!(queue(&w2, f2).len(), 4);
    w2.tick(&[cmd(Command::CancelRefit {
        units: vec![f2],
        kit: t2,
    })])
    .unwrap();
    assert!(queue(&w2, f2).is_empty(), "{:?}", queue(&w2, f2));

    // Run it: T2 goes on, both tanks roll out, then T3 goes on.
    let row = w.state.units.row(fid).unwrap();
    let tanks = |w: &World| {
        w.state
            .units
            .slots
            .iter()
            .filter(|&r| {
                w.state.units.blueprint[r] == tank2 && !w.state.units.has_flag(r, flag::IN_FACTORY)
            })
            .count()
    };
    let mut saw_t2 = false;
    for _ in 0..20_000 {
        w.tick(&[]).unwrap();
        if w.state.units.blueprint[row] == t2 {
            saw_t2 = true;
            assert!(tanks(&w) <= 2);
        }
        if w.state.units.blueprint[row] == t3 {
            break;
        }
    }
    assert!(saw_t2, "the factory went through T2");
    assert_eq!(w.state.units.blueprint[row], t3, "then on to T3");
    assert_eq!(
        tanks(&w),
        2,
        "both T2 tanks were built between the upgrades"
    );
    assert!(queue(&w, fid).is_empty());
}

#[test]
fn an_assist_gives_way_to_a_queued_build_once_the_factory_runs_dry() {
    let (mut w, fid) = with_factory();
    w.tick(&[spawn(&w, "aster_t1_engineer", 520)]).unwrap();
    let mason = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == w.blueprints.id_of("aster_t1_engineer").unwrap())
        .map(|r| w.state.units.id(r))
        .expect("the engineer spawned");
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    w.tick(&[
        cmd(Command::Produce {
            factories: vec![fid],
            blueprint: tank,
            count: 2,
        }),
        cmd(Command::Assist {
            units: vec![mason],
            target: fid,
            queue: false,
        }),
        cmd(Command::Build {
            units: vec![mason],
            blueprint: power,
            pos: FxVec2::from_ints(560, 400),
            heading: Angle::ZERO,
            queue: true,
        }),
    ])
    .unwrap();
    let front = |w: &World| {
        let row = w.state.units.row(mason).unwrap();
        w.state.orders.front(&w.state.units, row).map(|o| o.kind)
    };
    let factory_busy = |w: &World| {
        let row = w.state.units.row(fid).unwrap();
        w.state.orders.front(&w.state.units, row).is_some()
    };
    let mut ticks = 0;
    while factory_busy(&w) {
        assert_eq!(
            front(&w),
            Some(OrderKind::Assist),
            "the engineer keeps helping while the factory has tanks to print (tick {ticks})"
        );
        w.tick(&[]).unwrap();
        ticks += 1;
        assert!(ticks < 5000, "the factory finished its run");
    }
    let moved_on = (0..3).any(|_| {
        w.tick(&[]).unwrap();
        front(&w) == Some(OrderKind::Build)
    });
    assert!(moved_on, "with the factory idle, the queued build takes over");
}

#[test]
fn an_assist_with_nothing_queued_waits_by_an_idle_factory() {
    let (mut w, fid) = with_factory();
    w.tick(&[spawn(&w, "aster_t1_engineer", 520)]).unwrap();
    let mason = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == w.blueprints.id_of("aster_t1_engineer").unwrap())
        .map(|r| w.state.units.id(r))
        .expect("the engineer spawned");
    w.tick(&[cmd(Command::Assist {
        units: vec![mason],
        target: fid,
        queue: false,
    })])
    .unwrap();
    for _ in 0..100 {
        w.tick(&[]).unwrap();
    }
    let row = w.state.units.row(mason).unwrap();
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(OrderKind::Assist),
        "the assist stays on"
    );
}
