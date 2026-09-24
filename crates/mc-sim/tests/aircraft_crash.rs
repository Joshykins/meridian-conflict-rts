//! Aircraft death: ballistic debris, impact, salvage, and replay continuation.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn world_with_terrain(terrain: Heightfield) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
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

fn world() -> World {
    world_with_terrain(Heightfield::flat(256, 256, Fx::from_int(20)))
}

fn plane(w: &mut World) -> mc_sim::UnitId {
    w.tick(&[spawn(w, 0, "aster_t1_bomber", 512, flag::PASSIVE)])
        .unwrap();
    w.state.units.id(w.state.units.slots.iter().next().unwrap())
}

fn kill(w: &mut World, id: mc_sim::UnitId) {
    w.tick(&[cmd(Command::SelfDestruct { units: vec![id] })])
        .unwrap();
}

fn finish(w: &mut World) -> usize {
    let mut impacts = 0;
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        impacts += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::AircraftCrashed { .. }))
            .count();
    }
    impacts
}

#[test]
fn aircraft_falls_with_momentum_before_leaving_salvage() {
    let mut w = world();
    let id = plane(&mut w);
    w.tick(&[cmd(Command::Move {
        units: vec![id],
        target: FxVec2::from_ints(1200, 512),
        queue: false,
    })])
    .unwrap();
    for _ in 0..30 {
        w.tick(&[]).unwrap();
    }
    kill(&mut w, id);
    assert!(w.state.units.row(id).is_none());
    assert_eq!(w.state.aircraft_crashes.len(), 1);
    assert_eq!(w.state.wrecks.slots.live(), 0);
    assert!(w
        .events
        .iter()
        .any(|e| matches!(e, SimEvent::UnitDied { airborne: true, .. })));
    let origin = w.state.aircraft_crashes[0].pos;
    let mass = w.state.aircraft_crashes[0].mass;
    assert!(w.state.aircraft_crashes[0].velocity.x > Fx::ZERO);
    for _ in 0..10 {
        w.tick(&[]).unwrap();
    }
    let falling = &w.state.aircraft_crashes[0];
    assert!(falling.pos.x > origin.x);
    assert!(falling.pos.z < origin.z);
    assert_eq!(w.state.wrecks.slots.live(), 0);
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    let hull = frame.units.iter().find(|u| u.unit_id == id.0).unwrap();
    assert_eq!(hull.owner_flags, mc_sim::mirror::KIND_WRECK);
    assert_eq!(hull._pad, mc_sim::mirror::WRECK_FALLING);
    assert_ne!(hull._pad2[0], hull._pad2[1]);
    assert_ne!(hull.arm_pitch[0], hull.arm_pitch[1]);
    assert!(hull.pos[2] < hull.prev_pos[2]);
    assert_eq!(finish(&mut w), 1);
    assert!(w.state.aircraft_crashes.is_empty());
    assert_eq!(w.state.wrecks.slots.live(), 1);
    let row = w.state.wrecks.slots.iter().next().unwrap();
    assert_eq!(w.state.wrecks.mass[row], mass);
    assert_eq!(w.state.wrecks.z[row], Fx::from_int(20));
    assert!(w.state.wrecks.pos[row].x > origin.x);
    w.write_render_frame(None, &mut frame);
    assert!(frame.units.iter().all(|u| u._pad == 0));
}

#[test]
fn falling_aircraft_survives_snapshot_and_hashes_motion() {
    let mut a = world();
    let id = plane(&mut a);
    kill(&mut a, id);
    for _ in 0..12 {
        a.tick(&[]).unwrap();
    }
    let snapshot = a.snapshot();
    let mut b = world();
    b.restore(Heightfield::flat(256, 256, Fx::from_int(20)), &snapshot)
        .unwrap();
    assert_eq!(a.hash(), b.hash());
    b.state.aircraft_crashes[0].velocity.x += Fx::ONE;
    assert_ne!(a.hash(), b.hash());
    b.restore(Heightfield::flat(256, 256, Fx::from_int(20)), &snapshot)
        .unwrap();
    for _ in 0..80 {
        assert_eq!(a.tick(&[]).unwrap(), b.tick(&[]).unwrap());
    }
    assert_eq!(a.state.wrecks.slots.live(), 1);
}

#[test]
fn aircraft_hits_water_surface_and_stays_within_map() {
    let terrain = Heightfield::from_samples(
        256,
        256,
        vec![0; 257 * 257],
        Fx::from_int(-40),
        Fx::ONE,
        Fx::from_int(5),
    );
    let mut w = world_with_terrain(terrain);
    let id = plane(&mut w);
    kill(&mut w, id);
    w.state.aircraft_crashes[0].velocity.x = Fx::from_int(-300);
    // It strikes the surface once, then sinks: 45 m of water takes it a good while.
    let mut splash = None;
    let mut ticks = 0;
    while splash.is_none() {
        w.tick(&[]).unwrap();
        ticks += 1;
        assert!(ticks < 100);
        splash = w.events.iter().find_map(|e| match e {
            SimEvent::AircraftCrashed { pos, .. } => Some(*pos),
            _ => None,
        });
    }
    assert_eq!(splash.unwrap().z, Fx::from_int(5));
    assert_eq!(w.state.wrecks.slots.live(), 0);
    let mut depth = Fx::from_int(5);
    let mut settled = None;
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        assert!(!w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::AircraftCrashed { .. })));
        if let Some(crash) = w.state.aircraft_crashes.first() {
            // Down all the way, never back up, and slowly: no more than 1 m a tick after the plunge.
            assert!(crash.pos.z <= depth);
            assert!(crash.pos.z >= Fx::from_int(-40));
            assert!(crash.pos.x >= Fx::ZERO);
            depth = crash.pos.z;
        }
        if let Some(pos) = w.events.iter().find_map(|e| match e {
            SimEvent::ShipSettled { pos, .. } => Some(*pos),
            _ => None,
        }) {
            settled = Some(pos);
            break;
        }
    }
    assert_eq!(settled.unwrap().z, Fx::from_int(-40));
    assert!(w.state.aircraft_crashes.is_empty());
    let row = w.state.wrecks.slots.iter().next().unwrap();
    assert_eq!(w.state.wrecks.z[row], Fx::from_int(-40));
    assert!(w.state.wrecks.pos[row].x >= Fx::ZERO);
}

#[test]
fn ditched_aircraft_sinks_slowly_and_hangs_nose_down() {
    let terrain = Heightfield::from_samples(
        256,
        256,
        vec![0; 257 * 257],
        Fx::from_int(-40),
        Fx::ONE,
        Fx::from_int(5),
    );
    let mut w = world_with_terrain(terrain);
    let id = plane(&mut w);
    kill(&mut w, id);
    let mut under = 0;
    let mut frame = mc_sim::RenderFrame::default();
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        let Some(crash) = w.state.aircraft_crashes.first() else {
            break;
        };
        if crash.splashed == 0 {
            continue;
        }
        under += 1;
        if under == 30 {
            // Three seconds in: it has stopped, turned nose down, and sinks at a walking pace.
            assert!(crash.velocity.x.abs() < Fx::ratio(1, 10));
            let sink = -crash.velocity.z;
            assert!(sink > Fx::ratio(3, 10) && sink < Fx::ratio(4, 10));
            w.write_render_frame(None, &mut frame);
            let hull = frame.units.iter().find(|u| u.unit_id == id.0).unwrap();
            assert_eq!(hull._pad, mc_sim::mirror::WRECK_FALLING);
            let tau = std::f32::consts::TAU;
            let pitch = hull.arm_pitch[1] - (hull.arm_pitch[1] / tau).round() * tau;
            assert!((pitch + 0.55).abs() < 0.05, "pitch {pitch}");
            let roll = hull._pad2[1] - (hull._pad2[1] / tau).round() * tau;
            assert!(roll.abs() < 0.01, "roll {roll}");
            assert_eq!(hull.heading, hull.prev_heading);
        }
    }
    // 45 m at 3.5 m/s, after a short plunge.
    assert!((100..160).contains(&under), "{under} ticks under water");
    assert_eq!(w.state.wrecks.slots.live(), 1);
}

#[test]
fn shallow_water_takes_a_crash_like_ground() {
    let terrain = Heightfield::from_samples(
        256,
        256,
        vec![0; 257 * 257],
        Fx::ratio(9, 2),
        Fx::ONE,
        Fx::from_int(5),
    );
    let mut w = world_with_terrain(terrain);
    let id = plane(&mut w);
    kill(&mut w, id);
    assert_eq!(finish(&mut w), 1);
    assert!(w.state.aircraft_crashes.is_empty());
    let row = w.state.wrecks.slots.iter().next().unwrap();
    assert_eq!(w.state.wrecks.z[row], Fx::from_int(5));
}

#[test]
fn reclaimed_and_factory_aircraft_do_not_crash() {
    for flags in [flag::RECLAIMED, flag::IN_FACTORY] {
        let mut w = world();
        let id = plane(&mut w);
        let row = w.state.units.row(id).unwrap();
        w.state.units.flags[row] |= flags;
        kill(&mut w, id);
        assert!(w.state.aircraft_crashes.is_empty());
        assert_eq!(w.state.wrecks.slots.live(), 0);
        assert!(!w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::UnitDied { .. })));
    }
}

#[test]
fn aircraft_without_salvage_still_completes_its_crash() {
    let mut w = world();
    let id = plane(&mut w);
    kill(&mut w, id);
    w.state.aircraft_crashes[0].mass = Fx::ZERO;
    assert_eq!(finish(&mut w), 1);
    assert_eq!(w.state.wrecks.slots.live(), 0);
}

#[test]
fn fog_hides_the_falling_hull() {
    let mut w = world();
    let id = plane(&mut w);
    kill(&mut w, id);
    w.state.fog_enabled = true;
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(Some(1), &mut frame);
    assert!(frame.units.is_empty());
}
