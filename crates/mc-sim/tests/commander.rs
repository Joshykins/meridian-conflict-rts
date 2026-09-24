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

/// The kit of the commander's `key` module, and what a bare commander becomes with it.
fn module(w: &World, key: &str) -> (mc_data::BlueprintId, mc_data::BlueprintId) {
    let acu = w.blueprints.id_of("aster_commander").unwrap();
    let set = w.blueprints.refit_set(acu).unwrap();
    let kit = set
        .slots
        .iter()
        .flat_map(|s| &s.modules)
        .find(|m| m.key == key)
        .unwrap()
        .kit;
    (kit, w.blueprints.refit_result(acu, kit).unwrap_or(acu))
}

fn refit(w: &World, acu: UnitId, key: &str) -> Command {
    Command::Refit {
        units: vec![acu],
        kit: module(w, key).0,
    }
}

fn live_units(w: &World) -> usize {
    w.state.units.slots.iter().count()
}

#[test]
fn a_commander_is_refitted_in_place_and_stays_the_same_unit() {
    let (mut w, acu) = with_commander();
    let (t1, t2) = (
        w.blueprints.id_of("aster_commander").unwrap(),
        module(&w, "eng_2").1,
    );
    let row = w.state.units.row(acu).unwrap();
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![acu],
        permille: 500,
    })])
    .unwrap();
    let hurt = w.state.units.health[row];

    w.tick(&[cmd(refit(&w, acu, "eng_2"))]).unwrap();
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
        w.tick(&[cmd(refit(&w, acu, "eng_2"))]).unwrap();
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
fn a_refit_drops_the_walk_and_then_pins_it_until_done() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let go = Command::Move {
        units: vec![acu],
        target: FxVec2::from_ints(560, 512),
        queue: false,
    };
    w.tick(&[cmd(go)]).unwrap();
    for _ in 0..10 {
        w.tick(&[]).unwrap();
    }
    assert!(
        w.state.units.gait[row] > 0,
        "walking is counted for the stride"
    );
    // Ordering the refit drops the walk: it begins where the commander stands.
    w.tick(&[cmd(refit(&w, acu, "eng_2"))]).unwrap();
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(OrderKind::Upgrade)
    );
    let began = (0..300).any(|_| {
        w.tick(&[]).unwrap();
        live_units(&w) == 2
    });
    assert!(began, "the refit began");
    // It is still coasting to a halt from its walk for a moment.
    for _ in 0..15 {
        w.tick(&[]).unwrap();
    }
    let at = w.state.units.pos[row];
    assert!(
        at.x < Fx::from_int(550),
        "it stopped short of where it was walking to"
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
    let t2 = module(&w, "eng_2").1;
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
fn stopping_a_build_leaves_the_waves_to_run_out() {
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
    let mut built = None;
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        built = frame
            .units
            .iter()
            .find(|u| u.blueprint == power.0 as u32 && u.weld.iter().any(|c| c.abs() > 0.5))
            .map(|u| u.build);
        if built.is_some() {
            break;
        }
    }
    let progress = built.expect("the commander was printing");
    w.tick(&[cmd(Command::Stop { units: vec![acu] })]).unwrap();
    w.write_render_frame(None, &mut frame);
    let work = frame
        .units
        .iter()
        .find(|u| u.blueprint == power.0 as u32)
        .expect("the site is still on the map");
    assert!(
        work.weld_count >= 1 && work.weld.iter().any(|c| c.abs() > 0.5),
        "the last origin is still lighting the hull: {:?} count {}",
        work.weld,
        work.weld_count
    );
    assert!(
        work.weld[0].abs() > 1.0 || work.weld[1].abs() > 1.0,
        "stopping does not move the print origin to the pad: {:?}",
        work.weld
    );
    assert!(
        (work.build - progress).abs() < 0.05,
        "the hull stays as far along as it was ({progress} -> {})",
        work.build
    );
    assert!(frame.build_sources.is_empty());
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
    }
    let work = frame
        .units
        .iter()
        .find(|u| u.blueprint == power.0 as u32)
        .expect("the site is still on the map");
    assert_eq!(work.weld_count, 0, "the waves have run out");
    assert!(
        work.weld.iter().all(|c| c.abs() < 0.5),
        "and the origin is gone: {:?}",
        work.weld
    );
}

#[test]
fn two_builders_print_on_their_own_side() {
    let (mut w, acu) = with_commander();
    w.tick(&[spawn(&w, 0, "aster_t1_engineer", 400, 0)])
        .unwrap();
    let mason = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == w.blueprints.id_of("aster_t1_engineer").unwrap())
        .map(|r| w.state.units.id(r))
        .expect("the engineer spawned");
    let site = FxVec2::from_ints(452, 512);
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    w.tick(&[cmd(Command::Build {
        units: vec![acu, mason],
        blueprint: power,
        pos: site,
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();
    let mut frame = mc_sim::RenderFrame::default();
    let mut beams = Vec::new();
    for _ in 0..120 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        beams = frame
            .projectiles
            .iter()
            .filter(|p| p.color & mc_sim::mirror::PROJECTILE_BEAM != 0)
            .map(|p| p.pos[0])
            .collect();
        if beams.len() == 2 {
            break;
        }
    }
    assert_eq!(beams.len(), 2, "both builders put a beam on the site");
    let (lo, hi) = (beams[0].min(beams[1]), beams[0].max(beams[1]));
    assert!(hi - lo > 8.0, "each beam hits its own side ({lo} and {hi})");
    let work = frame
        .units
        .iter()
        .find(|u| u.blueprint == power.0 as u32)
        .expect("the site is on the map");
    assert!(
        work.weld_count >= 2,
        "each builder raises a wave origin ({})",
        work.weld_count
    );
    let first = work.weld_first as usize;
    let welds = &frame.welds[first..first + work.weld_count as usize];
    let span = welds.iter().map(|w| w.local[0]).fold(0.0f32, f32::max)
        - welds.iter().map(|w| w.local[0]).fold(0.0f32, f32::min);
    assert!(
        span > 6.0,
        "the origins sit on different sides ({span} m apart): {:?}",
        welds.iter().map(|w| w.local).collect::<Vec<_>>()
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
    assert_eq!(
        work.owner_flags & (flag::UNDER_CONSTRUCTION as u32) << 8,
        0,
        "the foundry is refitted, not rebuilt"
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
        cmd(refit(&w, acu, "eng_2")),
        cmd(Command::Assist {
            units: vec![mason],
            target: acu,
            queue: false,
        }),
    ])
    .unwrap();
    let mut frame = mc_sim::RenderFrame::default();
    let mut beam = None;
    for _ in 0..120 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        if frame.build_sources.iter().any(|(id, _)| *id == mason.0) {
            beam = frame
                .projectiles
                .iter()
                .find(|p| p.color & mc_sim::mirror::PROJECTILE_BEAM != 0)
                .copied();
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

#[test]
fn an_engineer_arm_folds_away_and_aims_at_its_work() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_engineer", 500, 0),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let mason = w.state.units.id(w.state.units.slots.iter().next().unwrap());
    let row = w.state.units.row(mason).unwrap();
    let arm = w
        .bp(row)
        .builder
        .as_ref()
        .and_then(|b| b.arm)
        .expect("the engineer has a build arm");
    assert_eq!(
        w.bp(row).motion.unwrap().layer,
        mc_data::MoveLayer::Hover,
        "it rides the water, not the bed"
    );
    assert!(
        arm.shoulder.is_some(),
        "the arm is two-bone: boom then forearm"
    );
    assert_eq!(
        w.state.units.arm_pitch[row][0], arm.rest,
        "it spawns with the boom folded up"
    );
    assert_eq!(
        w.state.units.arm_pitch[row][1],
        Angle::ZERO,
        "the forearm sits level until it aims"
    );
    assert_ne!(arm.rest, Angle::ZERO);

    let site = FxVec2::from_ints(530, 512);
    w.tick(&[cmd(Command::Build {
        units: vec![mason],
        blueprint: w.blueprints.id_of("aster_t1_power").unwrap(),
        pos: site,
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();
    let mut aimed = None;
    for i in 0..80 {
        w.tick(&[]).unwrap();
        if w.state.units.has_flag(row, flag::BUILDING) {
            aimed = Some(i);
            break;
        }
    }
    assert!(
        aimed.expect("it started printing") >= 3,
        "unfolding took time"
    );
    assert!(
        w.state.units.arm_pitch[row][0]
            .delta_to(Angle::ZERO)
            .unsigned_abs()
            < 728,
        "the boom unfolded to level before it printed"
    );
    let work_pitch = Angle::ZERO.delta_to(w.state.units.arm_pitch[row][1]);
    let rest_pitch = Angle::ZERO.delta_to(arm.rest);
    assert!(
        work_pitch.abs() < rest_pitch.abs() / 2,
        "the forearm aimed at the work ({work_pitch} vs boom rest {rest_pitch})"
    );

    w.tick(&[cmd(Command::Stop { units: vec![mason] })])
        .unwrap();
    for _ in 0..40 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.units.arm_pitch[row][0], arm.rest,
        "stop folds the boom up again"
    );
    assert_eq!(
        w.state.units.arm_pitch[row][1],
        Angle::ZERO,
        "stop levels the forearm first"
    );
}

/// Ticks until `row` has the blueprint `want`, or gives up.
fn run_until(w: &mut World, row: usize, want: mc_data::BlueprintId, ticks: usize) -> bool {
    (0..ticks).any(|_| {
        w.tick(&[]).unwrap();
        w.state.units.blueprint[row] == want
    })
}

#[test]
fn refits_queue_one_behind_another_and_each_goes_on_in_turn() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    // The rail cannon only goes over the cannon; queued after it, it fits.
    let rail_alone = refit(&w, acu, "railgun");
    w.tick(&[cmd(rail_alone)]).unwrap();
    assert!(
        w.state.orders.front(&w.state.units, row).is_none(),
        "the rail cannon needs the cannon under it"
    );
    let queue = [
        cmd(refit(&w, acu, "cannon")),
        cmd(refit(&w, acu, "railgun")),
        cmd(refit(&w, acu, "mfe")),
    ];
    w.tick(&queue).unwrap();
    assert_eq!(w.state.orders.iter(&w.state.units, row).count(), 3);

    // While the cannon goes on, the mirror says which pieces are going up.
    let mut frame = mc_sim::RenderFrame::default();
    let (_, with_cannon) = module(&w, "cannon");
    let mut raised = 0;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        raised = raised.max(frame.units[0].refit_modules);
    }
    assert_eq!(raised, w.blueprints.look(with_cannon));

    let (_, rail) = module(&w, "railgun");
    let rail = w
        .blueprints
        .refit_result(with_cannon, module(&w, "railgun").0)
        .unwrap_or(rail);
    let (mfe_kit, _) = module(&w, "mfe");
    let done = w.blueprints.refit_result(rail, mfe_kit).unwrap();
    assert!(run_until(&mut w, row, done, 6000), "every refit went on");
    let names: Vec<&str> = w.bp(row).weapons.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["Vulcan Machine Gun", "Meridian Rail Cannon"]);
    assert_eq!(live_units(&w), 1);
    assert!(w.state.orders.front(&w.state.units, row).is_none());
}

#[test]
fn cancelling_a_refit_takes_out_the_tiers_queued_over_it() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let queue = [
        cmd(refit(&w, acu, "cannon")),
        cmd(refit(&w, acu, "railgun")),
        cmd(refit(&w, acu, "eng_2")),
    ];
    w.tick(&queue).unwrap();
    let cancel = Command::CancelRefit {
        units: vec![acu],
        kit: module(&w, "cannon").0,
    };
    w.tick(&[cmd(cancel)]).unwrap();
    let left: Vec<_> = w
        .state
        .orders
        .iter(&w.state.units, row)
        .map(|o| o.blueprint)
        .collect();
    assert_eq!(
        left,
        [module(&w, "eng_2").0],
        "the rail cannon went with the cannon"
    );
}

#[test]
fn a_new_back_pack_replaces_the_old_and_a_shield_comes_up() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let (_, with_mfe) = module(&w, "mfe");
    w.tick(&[cmd(refit(&w, acu, "mfe"))]).unwrap();
    assert!(run_until(&mut w, row, with_mfe, 6000));
    w.tick(&[cmd(refit(&w, acu, "shield"))]).unwrap();
    let with_shield = module(&w, "shield").1;
    assert!(
        run_until(&mut w, row, with_shield, 6000),
        "the shield replaced the engine"
    );
    assert_eq!(w.bp(row).economy.mass_income, Fx::from_int(1));
    let full = w.bp(row).shield.unwrap().health;
    assert_eq!(
        w.state.units.shield_hp[row], full,
        "the new field starts charged"
    );
    let rose = (0..40).any(|_| {
        w.tick(&[]).unwrap();
        w.state.units.shield_open[row] == 255
    });
    assert!(rose, "and rises");
}

#[test]
fn the_auxiliary_suite_folds_out_to_build_with_a_second_beam() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let (_, aux) = module(&w, "aux_eng");
    w.tick(&[cmd(refit(&w, acu, "aux_eng"))]).unwrap();
    assert!(run_until(&mut w, row, aux, 6000));
    assert_eq!(w.state.units.deploy[row], 0, "folded away at rest");
    let build = Command::Build {
        units: vec![acu],
        blueprint: w.blueprints.id_of("aster_t1_power").unwrap(),
        pos: FxVec2::from_ints(560, 512),
        heading: Angle::ZERO,
        queue: false,
    };
    w.tick(&[cmd(build)]).unwrap();
    let mut frame = mc_sim::RenderFrame::default();
    let two_beams = (0..300).any(|_| {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        frame
            .projectiles
            .iter()
            .filter(|p| p.color & mc_sim::mirror::PROJECTILE_BEAM != 0)
            .count()
            == 2
    });
    assert!(
        two_beams,
        "the folded-out suite builds with a beam of its own"
    );
    assert!(frame.units[0].deploy > 0.99);
}

#[test]
fn the_vulcan_spins_up_before_it_fires() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let spin = w.bp(row).weapons[0].spin_ticks;
    assert!(spin > 2, "the machine gun is a rotary");
    w.tick(&[spawn(
        &w,
        1,
        "aster_t1_tank",
        640,
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let first = (0..60).find(|_| {
        w.tick(&[]).unwrap();
        w.state.projectiles.len() > 0
    });
    let first = first.expect("it fires once spun up");
    // It starts spinning on the tick the target turns up, before this count begins.
    assert!(
        first as u16 + 2 >= spin,
        "fired after {first} ticks, before spinning up ({spin})"
    );
    assert_eq!(w.state.units.spin[row][0], spin);
}

#[test]
fn shoulder_guns_keep_firing_while_it_builds() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let (_, art) = module(&w, "artillery");
    w.tick(&[cmd(refit(&w, acu, "artillery"))]).unwrap();
    assert!(run_until(&mut w, row, art, 6000));
    // A target out past the machine gun, in howitzer range; work behind the commander.
    w.tick(&[spawn(
        &w,
        1,
        "aster_t1_power",
        1100,
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let build = Command::Build {
        units: vec![acu],
        blueprint: w.blueprints.id_of("aster_t1_power").unwrap(),
        pos: FxVec2::from_ints(440, 512),
        heading: Angle::ZERO,
        queue: false,
    };
    w.tick(&[cmd(build)]).unwrap();
    let howitzer = w
        .bp(row)
        .weapons
        .iter()
        .position(|x| x.mount)
        .expect("the howitzer is a mount");
    let mut fired_working = false;
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        let working = w.state.units.has_flag(row, flag::WORKING);
        if working
            && w.state.units.weapon_cooldown[row][howitzer]
                == w.bp(row).weapons[howitzer].reload_ticks
        {
            fired_working = true;
        }
        if working {
            assert_eq!(
                w.state.units.weapon_salvo_left[row][0], 0,
                "the arm's gun holds while it builds"
            );
        }
    }
    assert!(
        fired_working,
        "the howitzer fired while the commander was building"
    );
}

#[test]
fn ordering_a_refit_drops_everything_but_the_build_queue() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    w.tick(&[
        cmd(Command::Move {
            units: vec![acu],
            target: FxVec2::from_ints(900, 512),
            queue: false,
        }),
        cmd(Command::Build {
            units: vec![acu],
            blueprint: power,
            pos: FxVec2::from_ints(600, 620),
            heading: Angle::ZERO,
            queue: true,
        }),
        cmd(Command::Move {
            units: vec![acu],
            target: FxVec2::from_ints(300, 512),
            queue: true,
        }),
    ])
    .unwrap();
    w.tick(&[cmd(refit(&w, acu, "eng_2"))]).unwrap();
    let kinds: Vec<OrderKind> = w
        .state
        .orders
        .iter(&w.state.units, row)
        .map(|o| o.kind)
        .collect();
    assert_eq!(kinds, [OrderKind::Build, OrderKind::Upgrade]);
}

/// The bare commander rebuilt with the rail cannon: the Vulcan and the rail cannon share its torso.
fn with_rail_cannon() -> (World, UnitId) {
    let (mut w, acu) = with_commander();
    let cannon = module(&w, "cannon").1;
    let rail = w
        .blueprints
        .refit_result(cannon, module_kit(&w, "railgun"))
        .unwrap();
    w.tick(&[
        cmd(Command::DebugRemove { units: vec![acu] }),
        cmd(Command::DebugSpawn {
            owner: 0,
            blueprint: rail,
            pos: FxVec2::from_ints(500, 512),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        }),
    ])
    .unwrap();
    let id = w.state.units.id(w.state.units.slots.iter().next().unwrap());
    (w, id)
}

fn module_kit(w: &World, key: &str) -> mc_data::BlueprintId {
    let acu = w.blueprints.id_of("aster_commander").unwrap();
    let set = w.blueprints.refit_set(acu).unwrap();
    set.slots
        .iter()
        .flat_map(|s| &s.modules)
        .find(|m| m.key == key)
        .unwrap()
        .kit
}

fn dummy(w: &World, x: i32, y: i32) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner: 1,
        blueprint: w.blueprints.id_of("aster_t1_tank").unwrap(),
        pos: FxVec2::from_ints(x, y),
        heading: Angle::ZERO,
        count: 1,
        flags: flag::PASSIVE | flag::INVULNERABLE,
        build: 1000,
    })
}

#[test]
fn the_rail_cannon_turns_the_whole_torso_onto_its_target() {
    let (mut w, acu) = with_rail_cannon();
    let row = w.state.units.row(acu).unwrap();
    // Off to the side and past the Vulcan's reach: only the rail cannon can take it.
    w.tick(&[dummy(&w, 500, 512 + 380)]).unwrap();
    let rail = w
        .bp(row)
        .weapons
        .iter()
        .position(|g| g.name == "Meridian Rail Cannon")
        .unwrap() as u8;
    let mut fired = None;
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let mc_sim::SimEvent::ShotFired { pos, weapon, .. } = e {
                if *weapon == rail {
                    fired.get_or_insert(*pos);
                }
            }
        }
    }
    let torso = w.state.units.heading[row] + w.state.units.weapon_yaw[row][0];
    let bearing = Angle::from_degrees(90);
    assert!(
        torso.delta_to(bearing).unsigned_abs() < 400,
        "the torso faces the target ({} steps off)",
        torso.delta_to(bearing)
    );
    let from = fired.expect("the rail cannon fired");
    assert!(
        from.y > w.state.units.pos[row].y + Fx::from_int(8),
        "the shot left the turned arm's muzzle, out towards the target: {from:?}"
    );
}

#[test]
fn the_vulcan_fires_while_anything_is_in_its_cone() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    // Two tanks some way apart, both inside its cone.
    w.tick(&[dummy(&w, 700, 512 - 50), dummy(&w, 700, 512 + 50)])
        .unwrap();
    let (mut shots, mut low, mut high) = (0, i16::MAX, i16::MIN);
    for t in 0..120 {
        w.tick(&[]).unwrap();
        if t < 20 {
            continue;
        }
        shots += w
            .events
            .iter()
            .filter(|e| matches!(e, mc_sim::SimEvent::ShotFired { weapon: 0, .. }))
            .count();
        let yaw =
            Angle::ZERO.delta_to(w.state.units.heading[row] + w.state.units.weapon_yaw[row][0]);
        low = low.min(yaw);
        high = high.max(yaw);
    }
    // 100 ticks at a shot every 0.2 s is 50: it does not wait to be dead on.
    assert!(shots >= 40, "it kept firing ({shots} shots)");
    assert!(
        high - low < Angle::from_degrees(10).0 as i16,
        "it holds on its mark, it does not swing between them ({low}..{high} steps)"
    );
}

/// Shots the Vulcan fires this tick while its mark is more than its sweep cone off the
/// barrel, and whether it fired at all.
fn vulcan_off_cone(w: &World, row: usize, mark: FxVec2) -> (usize, usize) {
    let shots = w
        .events
        .iter()
        .filter(|e| matches!(e, mc_sim::SimEvent::ShotFired { weapon: 0, .. }))
        .count();
    let barrel = w.state.units.heading[row] + w.state.units.weapon_yaw[row][0];
    let off = barrel
        .delta_to((mark - w.state.units.pos[row]).angle())
        .unsigned_abs();
    let outside = off > Angle::from_degrees(35).0;
    (if outside { shots } else { 0 }, shots)
}

#[test]
fn the_vulcan_keeps_firing_as_it_swings_to_its_next_target() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let pos = w.state.units.pos[row];
    // One tank dead ahead, another a quarter turn round and a little further off.
    w.tick(&[dummy(&w, 700, 512)]).unwrap();
    let first = w.state.units.id(w.state.units.slots.iter().last().unwrap());
    w.tick(&[dummy(&w, 500, 730)]).unwrap();
    let next = pos + FxVec2::from_ints(0, 730 - 512);
    for _ in 0..30 {
        w.tick(&[]).unwrap();
    }
    // The first goes; the gun is mid-stream as it comes round to the second.
    w.tick(&[cmd(Command::DebugRemove { units: vec![first] })])
        .unwrap();
    let mut swinging = 0;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        swinging += vulcan_off_cone(&w, row, next).0;
    }
    assert!(
        swinging >= 3,
        "it kept firing as it swung round ({swinging} shots outside its cone)"
    );
}

#[test]
fn the_vulcan_does_not_open_up_before_its_first_target_is_in_its_cone() {
    let (mut w, acu) = with_commander();
    let row = w.state.units.row(acu).unwrap();
    let pos = w.state.units.pos[row];
    w.tick(&[dummy(&w, 500, 730)]).unwrap();
    let mark = pos + FxVec2::from_ints(0, 730 - 512);
    let (mut outside, mut fired) = (0, 0);
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        let (o, f) = vulcan_off_cone(&w, row, mark);
        outside += o;
        fired += f;
    }
    assert!(fired > 0, "it engaged");
    assert_eq!(outside, 0, "it held fire until the target was in its cone");
}

#[test]
fn a_beaten_side_goes_in_the_same_tick_half_built_sites_and_all() {
    let mut w = world();
    let site = |w: &World, key: &str, x: i32| {
        cmd(Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, 900),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 300,
        })
    };
    let setup = [
        spawn(&w, 1, "aster_t1_engineer", 960, 0),
        site(&w, "aster_t1_power", 1040),
        site(&w, "aster_t1_air_factory", 1200),
    ];
    w.tick(&setup).unwrap();
    assert_eq!(ids_of(&w, 1).len(), 3);
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::Resign,
    }])
    .unwrap();
    assert!(w.state.players[1].defeated);
    assert!(ids_of(&w, 1).is_empty(), "a defeated side leaves no husks");

    // Nothing lifts a beaten side's unit back off zero: whatever is left of it goes.
    w.tick(&[spawn(&w, 1, "aster_t1_engineer", 960, 0)])
        .unwrap();
    assert!(ids_of(&w, 1).is_empty());
}
