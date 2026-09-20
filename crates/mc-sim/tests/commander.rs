//! The commander: refits in place, one job for its torso at a time.

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
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        deposits: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
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

fn spawn(w: &World, owner: u8, key: &str, x: i32, flags: u16) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, 512),
        heading: Angle::ZERO,
        count: 1,
        flags,
        build: 1000,
    })
}

/// A world with one commander of player 0's, building for free.
fn with_commander() -> (World, UnitId) {
    let mut w = world();
    let setup = [
        spawn(&w, 0, "aster_commander", 500, 0),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ];
    w.tick(&setup).unwrap();
    let id = w.state.units.id(w.state.units.slots.iter().next().unwrap());
    (w, id)
}

fn live_units(w: &World) -> usize {
    w.state.units.slots.iter().count()
}

#[test]
fn a_commander_is_refitted_in_place_and_stays_the_same_unit() {
    let (mut w, acu) = with_commander();
    let (t1, t2) = (
        w.blueprints.id_of("aster_commander").unwrap(),
        w.blueprints.id_of("aster_commander_t2").unwrap(),
    );
    let row = w.state.units.row(acu).unwrap();
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![acu],
        permille: 500,
    })])
    .unwrap();
    let hurt = w.state.units.health[row];

    w.tick(&[cmd(Command::Upgrade { units: vec![acu] })])
        .unwrap();
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(OrderKind::Upgrade),
        "the refit is an order in its queue"
    );
    let mut shown = 0.0f32;
    let mut frame = mc_sim::RenderFrame::default();
    let done = (0..1500).any(|_| {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        assert_eq!(
            frame.units.len(),
            1,
            "the refit is never drawn as a second unit"
        );
        shown = shown.max(frame.units[0].upgrade);
        w.state.units.blueprint[row] == t2
    });
    assert!(done, "the refit never finished");
    assert!(
        shown > 0.9,
        "the mirror reported the refit's progress ({shown})"
    );
    assert_eq!(w.state.units.row(acu), Some(row), "same unit, same id");
    assert_eq!(live_units(&w), 1, "nothing is left behind");
    assert!(w.state.orders.front(&w.state.units, row).is_none());
    let gained = w.blueprints.unit(t2).health - w.blueprints.unit(t1).health;
    assert!(
        w.state.units.health[row] >= hurt + gained,
        "it keeps its wounds and gains the new tier's health"
    );
    assert!(w.state.units.health[row] < w.blueprints.unit(t2).health);
    w.write_render_frame(None, &mut frame);
    assert_eq!(frame.units[0].upgrade, 0.0);
}

#[test]
fn stop_and_cancel_both_scrap_a_refit() {
    for cancel in [
        Command::Stop { units: Vec::new() },
        Command::CancelUpgrade { units: Vec::new() },
    ] {
        let (mut w, acu) = with_commander();
        let row = w.state.units.row(acu).unwrap();
        w.tick(&[cmd(Command::Upgrade { units: vec![acu] })])
            .unwrap();
        for _ in 0..30 {
            w.tick(&[]).unwrap();
        }
        assert_eq!(live_units(&w), 2, "the refit is under way");
        assert!(w.state.units.has_flag(row, flag::BUILDING));
        let cancel = match cancel {
            Command::Stop { .. } => Command::Stop { units: vec![acu] },
            _ => Command::CancelUpgrade { units: vec![acu] },
        };
        w.tick(&[cmd(cancel)]).unwrap();
        w.tick(&[]).unwrap();
        assert_eq!(live_units(&w), 1, "the half-built refit is scrapped");
        assert!(w.state.orders.front(&w.state.units, row).is_none());
        assert_eq!(
            w.state.units.blueprint[row],
            w.blueprints.id_of("aster_commander").unwrap()
        );
    }
}

#[test]
fn a_queued_refit_waits_its_turn_and_then_pins_it_until_done() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let go = Command::Move {
        units: vec![acu],
        target: FxVec2::from_ints(560, 512),
        queue: false,
    };
    w.tick(&[cmd(go), cmd(Command::Upgrade { units: vec![acu] })])
        .unwrap();
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(OrderKind::Move)
    );
    let began = (0..300).any(|_| {
        w.tick(&[]).unwrap();
        live_units(&w) == 2
    });
    assert!(began, "the refit began once it had walked there");
    // It is still coasting to a halt from its walk for a moment.
    for _ in 0..15 {
        w.tick(&[]).unwrap();
    }
    let at = w.state.units.pos[row];
    assert!(at.distance(FxVec2::from_ints(560, 512)) < Fx::from_int(8));
    assert!(
        w.state.units.gait[row] > 0,
        "walking is counted for the stride"
    );

    // Orders to move and to build do not interrupt a refit: they wait behind it, the latest replacing the last.
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    w.tick(&[cmd(Command::Move {
        units: vec![acu],
        target: FxVec2::from_ints(400, 512),
        queue: false,
    })])
    .unwrap();
    w.tick(&[cmd(Command::Build {
        units: vec![acu],
        blueprint: power,
        pos: FxVec2::from_ints(600, 560),
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();
    let kinds: Vec<OrderKind> = w
        .state
        .orders
        .iter(&w.state.units, row)
        .map(|o| o.kind)
        .collect();
    assert_eq!(kinds, [OrderKind::Upgrade, OrderKind::Build]);
    // An enemy walks up meanwhile: it cannot move, but it can shoot.
    w.tick(&[spawn(
        &w,
        1,
        "aster_t1_tank",
        660,
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let mut fired = false;
    for _ in 0..60 {
        w.tick(&[]).unwrap();
        fired |= w
            .events
            .iter()
            .any(|e| matches!(e, mc_sim::SimEvent::ShotFired { owner: 0, .. }));
        assert_eq!(
            w.state.units.pos[row], at,
            "it stands still while it is refitted"
        );
    }
    assert!(fired, "it defends itself while it is refitted");
    assert_eq!(
        live_units(&w),
        3,
        "the refit is still under way (commander, its refit, the tank)"
    );

    // Done, it gets on with what was asked of it meanwhile.
    let t2 = w.blueprints.id_of("aster_commander_t2").unwrap();
    let done = (0..1500).any(|_| {
        w.tick(&[]).unwrap();
        w.state.units.blueprint[row] == t2
    });
    assert!(done);
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(OrderKind::Build)
    );
}

#[test]
fn it_turns_to_its_work_and_holds_fire_while_it_builds() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    // A target dummy well inside rifle range, straight ahead; the site is behind the commander.
    w.tick(&[spawn(
        &w,
        1,
        "aster_t1_tank",
        640,
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let fired = (0..60).any(|_| {
        w.tick(&[]).unwrap();
        w.state.projectiles.len() > 0
    });
    assert!(fired, "left alone it shoots at what it sees");

    let site = FxVec2::from_ints(452, 512);
    let build = Command::Build {
        units: vec![acu],
        blueprint: w.blueprints.id_of("aster_t1_power").unwrap(),
        pos: site,
        heading: Angle::ZERO,
        queue: false,
    };
    w.tick(&[cmd(build)]).unwrap();
    let mut building = 0;
    let mut working = 0;
    let mut first_aimed = None;
    for t in 0..400 {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        if u.has_flag(row, flag::BUILDING) {
            building += 1;
            let yaw = u.heading[row] + u.weapon_yaw[row][0];
            let to_site = (site - u.pos[row]).angle();
            assert!(
                yaw.delta_to(to_site).unsigned_abs() < 1200,
                "it builds only with its torso on the site"
            );
            first_aimed.get_or_insert(t);
        }
        // Shots already in the air land within a few ticks; after that, none of its own while it works.
        working = if u.has_flag(row, flag::WORKING) {
            working + 1
        } else {
            0
        };
        assert!(
            working < 12 || !w.state.projectiles.owner.contains(&0),
            "it fired while it was working"
        );
        if u.slots.iter().count() == 3 && u.slots.iter().all(|r| u.is_active(r)) {
            break;
        }
    }
    assert!(building > 10, "it built ({building} ticks)");
    assert!(
        first_aimed.unwrap() >= 3,
        "turning the torso round took time"
    );

    // The job done, the gun comes back up.
    for _ in 0..30 {
        w.tick(&[]).unwrap();
    }
    let fired_again = (0..80).any(|_| {
        w.tick(&[]).unwrap();
        w.state.projectiles.owner.contains(&0)
    });
    assert!(fired_again, "with the building finished it fights again");
}

#[test]
fn its_arms_point_down_at_what_is_close_and_come_level_again() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    // A tank almost at its feet: the rifle has to point well down to bear on it.
    w.tick(&[spawn(
        &w,
        1,
        "aster_t1_tank",
        524,
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let mut lowest = 0i16;
    let mut steps = Vec::new();
    let mut muzzle_z = None;
    for _ in 0..40 {
        let before = Angle::ZERO.delta_to(w.state.units.arm_pitch[row][0]);
        w.tick(&[]).unwrap();
        let now = Angle::ZERO.delta_to(w.state.units.arm_pitch[row][0]);
        steps.push((now - before).unsigned_abs());
        lowest = lowest.min(now);
        // The shot is spent the tick it is fired at this range: the event says where it left from.
        for e in &w.events {
            if let mc_sim::SimEvent::ShotFired { pos, .. } = e {
                muzzle_z = Some(pos.z - w.state.units.z[row]);
            }
        }
    }
    assert!(lowest < -2000, "the gun arm pitched down ({lowest} steps)");
    assert!(
        steps.iter().all(|s| *s <= 1700),
        "it swings down at the arm's rate, it does not snap: {steps:?}"
    );
    let level = w.bp(row).weapons[0].muzzle.z;
    assert!(
        muzzle_z.expect("it fired") < level - Fx::ONE,
        "its last shot left the lowered muzzle, not where the muzzle is when level"
    );
    assert_eq!(
        w.state.units.prev_weapon_yaw[row][0], w.state.units.weapon_yaw[row][0],
        "settled: last tick's yaw is this tick's"
    );

    w.tick(&[cmd(Command::DebugRemove {
        units: ids_of(&w, 1),
    })])
    .unwrap();
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.units.arm_pitch[row][0],
        Angle::ZERO,
        "nothing to shoot: the arm comes level"
    );
}

fn ids_of(w: &World, owner: u8) -> Vec<UnitId> {
    let u = &w.state.units;
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == owner)
        .map(|r| u.id(r))
        .collect()
}

#[test]
fn a_build_beam_lands_where_the_laser_meets_the_work() {
    let (mut w, acu) = with_commander();
    let site = FxVec2::from_ints(452, 512);
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    w.tick(&[cmd(Command::Build {
        units: vec![acu],
        blueprint: power,
        pos: site,
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();
    let mut frame = mc_sim::RenderFrame::default();
    let mut beam = None;
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        beam = frame
            .projectiles
            .iter()
            .find(|p| p.color & mc_sim::mirror::PROJECTILE_BEAM != 0)
            .copied();
        if beam.is_some() {
            break;
        }
    }
    let beam = beam.expect("the commander put a construction beam on the site");
    let work = frame
        .units
        .iter()
        .find(|u| u.blueprint == power.0 as u32)
        .expect("the site is on the map");
    let (dx, dy, dz) = (
        beam.pos[0] - work.pos[0],
        beam.pos[1] - work.pos[1],
        beam.pos[2] - work.pos[2],
    );
    let reach = (dx * dx + dy * dy).sqrt();
    assert!(
        reach > work.radius * 0.5 && reach < work.radius * 1.15,
        "the beam hits the near side ({reach} m out of radius {})",
        work.radius
    );
    assert!(
        dz > 1.0 && dz < work.radius,
        "and part-way up the hull ({dz})"
    );
    assert!(
        work.weld.iter().any(|c| c.abs() > 0.5),
        "the site remembers the weld in model space: {:?}",
        work.weld
    );
    assert_eq!(frame.build_sources.len(), 1);
}

#[test]
fn an_upgrading_structure_is_drawn_as_a_construction_site() {
    let (mut w, _) = with_commander();
    w.tick(&[spawn(&w, 0, "aster_t1_land_factory", 700, 0)])
        .unwrap();
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let fid = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == factory)
        .map(|r| w.state.units.id(r))
        .expect("the factory spawned");
    w.tick(&[cmd(Command::Upgrade { units: vec![fid] })])
        .unwrap();
    let mut frame = mc_sim::RenderFrame::default();
    let mut seen = None;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        seen = frame.units.iter().find(|u| u.unit_id == fid.0).copied();
        if seen.is_some_and(|u| u.build > 0.0 && u.build < 1.0) {
            break;
        }
    }
    let shown = seen.expect("the factory is on the map");
    assert!(
        shown.owner_flags & (flag::UNDER_CONSTRUCTION as u32) << 8 != 0,
        "it is drawn as a site"
    );
    assert!(
        shown.build > 0.0 && shown.build < 1.0,
        "its progress is the refit's ({})",
        shown.build
    );
    assert!(
        shown.weld[2] > 0.0,
        "it has a print origin: {:?}",
        shown.weld
    );
    assert_eq!(
        frame
            .units
            .iter()
            .filter(|u| u.blueprint == factory.0 as u32)
            .count(),
        1,
        "the successor stays hidden"
    );
}

#[test]
fn assisting_an_upgrade_puts_a_build_beam_on_it() {
    let (mut w, acu) = with_commander();
    w.tick(&[spawn(&w, 0, "aster_t1_land_factory", 600, 0)])
        .unwrap();
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let fid = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == factory)
        .map(|r| w.state.units.id(r))
        .expect("the factory spawned");
    w.tick(&[
        cmd(Command::Upgrade { units: vec![fid] }),
        cmd(Command::Assist {
            units: vec![acu],
            target: fid,
            queue: false,
        }),
    ])
    .unwrap();
    let mut frame = mc_sim::RenderFrame::default();
    let mut beam = None;
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        beam = frame
            .projectiles
            .iter()
            .find(|p| p.color & mc_sim::mirror::PROJECTILE_BEAM != 0)
            .copied();
        if beam.is_some() {
            break;
        }
    }
    let beam = beam.expect("the commander put a construction beam on the upgrade");
    let work = frame
        .units
        .iter()
        .find(|u| u.unit_id == fid.0)
        .expect("the factory is on the map");
    let (dx, dy) = (beam.pos[0] - work.pos[0], beam.pos[1] - work.pos[1]);
    let reach = (dx * dx + dy * dy).sqrt();
    assert!(
        reach > work.radius * 0.5 && reach < work.radius * 1.15,
        "the beam hits the factory ({reach} m out of radius {})",
        work.radius
    );
    assert!(
        work.weld.iter().any(|c| c.abs() > 0.5),
        "the factory remembers the weld: {:?}",
        work.weld
    );
    assert!(
        frame.build_sources.iter().any(|(id, _)| *id == acu.0),
        "the commander is listed as printing"
    );
}

#[test]
fn assisting_a_refit_puts_a_build_beam_on_the_commander() {
    let (mut w, acu) = with_commander();
    w.tick(&[spawn(&w, 0, "aster_t1_engineer", 530, 0)])
        .unwrap();
    let mason = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == w.blueprints.id_of("aster_t1_engineer").unwrap())
        .map(|r| w.state.units.id(r))
        .expect("the engineer spawned");
    w.tick(&[
        cmd(Command::Upgrade { units: vec![acu] }),
        cmd(Command::Assist {
            units: vec![mason],
            target: acu,
            queue: false,
        }),
    ])
    .unwrap();
    let mut frame = mc_sim::RenderFrame::default();
    let mut beam = None;
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        beam = frame
            .projectiles
            .iter()
            .find(|p| p.color & mc_sim::mirror::PROJECTILE_BEAM != 0)
            .copied();
        if beam.is_some() {
            break;
        }
    }
    assert!(
        beam.is_some(),
        "the engineer put a construction beam on the refit"
    );
    assert!(
        frame.build_sources.iter().any(|(id, _)| *id == mason.0),
        "the engineer is listed as printing"
    );
}
