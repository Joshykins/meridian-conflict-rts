//! Naval rules: submarines dive by themselves and on order, only sonar finds a
//! dived hull, only torpedoes reach one, and dead ships sink to the seabed.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::{KIND_WRECK, STATE_RADAR, UNIT_DIVE_GOAL, UNIT_DIVE_MASK, WRECK_SINKING};
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{
    Command, MatchConfig, PlayerCommand, PlayerSetup, RenderFrame, SimEvent, UnitId, World,
};
use std::path::Path;
use std::sync::Arc;

/// The sea: 20 m of water over a flat bed at zero, with a strip of land along the west edge.
const WATER: i32 = 20;

fn blueprints() -> Arc<Blueprints> {
    Arc::new(Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap())
}

fn sea(fog: bool) -> World {
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..40 {
            samples[y * 257 + x] = 40;
        }
    }
    let terrain =
        Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(WATER));
    let map = MapData {
        name: "sea".into(),
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
        fog,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints(), Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32, flags: u16) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.flags[row] |= flags;
    w.state.units.id(row)
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

fn order(w: &mut World, player: u8, command: Command) {
    w.tick(&[PlayerCommand { player, command }]).unwrap();
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn z(w: &World, id: UnitId) -> Fx {
    w.state.units.z[row(w, id)]
}

fn under(w: &World, id: UnitId) -> bool {
    let r = row(w, id);
    w.state.units.z[r] + w.bp(r).height < Fx::from_int(WATER)
}

fn near(a: Fx, b: Fx) -> bool {
    (a - b).abs() < Fx::ratio(1, 100)
}

fn frame(w: &World, viewer: Option<u8>) -> RenderFrame {
    let mut frame = RenderFrame::default();
    w.write_render_frame(viewer, &mut frame);
    frame
}

#[test]
fn a_new_submarine_dives_by_itself_and_surfaces_on_order() {
    let mut w = sea(false);
    let sub = spawn(&mut w, "aster_t1_submarine", 0, 1000, 1000, flag::PASSIVE);
    assert_eq!(z(&w, sub), Fx::from_int(WATER), "it starts on the surface");
    w.tick(&[]).unwrap();
    assert!(
        z(&w, sub) < Fx::from_int(WATER) && !under(&w, sub),
        "it is on its way down"
    );
    run(&mut w, 40);
    // Height 3.6 and two metres of water over it.
    assert!(near(z(&w, sub), Fx::from_int(WATER) - Fx::ratio(56, 10)));
    assert!(under(&w, sub));
    let unit = frame(&w, Some(0))
        .units
        .into_iter()
        .find(|u| u.unit_id == sub.0)
        .unwrap();
    assert_eq!(unit._pad3[0] & UNIT_DIVE_MASK, 255);
    assert_ne!(unit._pad3[0] & UNIT_DIVE_GOAL, 0);

    order(
        &mut w,
        0,
        Command::SetDive {
            units: vec![sub],
            dive: false,
        },
    );
    run(&mut w, 40);
    assert_eq!(z(&w, sub), Fx::from_int(WATER));
    assert_eq!(w.state.units.dive[row(&w, sub)], 0);
    // Someone else's order does nothing.
    order(
        &mut w,
        1,
        Command::SetDive {
            units: vec![sub],
            dive: true,
        },
    );
    run(&mut w, 40);
    assert_eq!(z(&w, sub), Fx::from_int(WATER));
    order(
        &mut w,
        0,
        Command::SetDive {
            units: vec![sub],
            dive: true,
        },
    );
    run(&mut w, 40);
    assert!(under(&w, sub));
}

#[test]
fn a_moving_submarine_keeps_its_depth() {
    let mut w = sea(false);
    let sub = spawn(&mut w, "aster_t1_submarine", 0, 1000, 1000, flag::PASSIVE);
    run(&mut w, 40);
    order(
        &mut w,
        0,
        Command::Move {
            units: vec![sub],
            target: FxVec2::from_ints(1400, 1200),
            queue: false,
        },
    );
    for _ in 0..60 {
        w.tick(&[]).unwrap();
        assert!(under(&w, sub));
    }
    assert!(
        w.state.units.pos[row(&w, sub)].distance(FxVec2::from_ints(1000, 1000)) > Fx::from_int(100)
    );
}

#[test]
fn only_sonar_finds_a_dived_submarine() {
    let mut w = sea(true);
    let sub = spawn(
        &mut w,
        "aster_t1_submarine",
        0,
        1000,
        1000,
        flag::PASSIVE | flag::INVULNERABLE,
    );
    // Vision 380 and radar 1400, well within reach of it.
    spawn(
        &mut w,
        "aster_t1_frigate",
        1,
        1250,
        1000,
        flag::PASSIVE | flag::INVULNERABLE,
    );
    w.tick(&[]).unwrap();
    assert!(
        w.detects(1, row(&w, sub)),
        "a surfaced submarine is in plain sight"
    );
    run(&mut w, 40);
    assert!(
        !w.detects(1, row(&w, sub)),
        "radar and eyes do not see under water"
    );
    assert!(w.detects(0, row(&w, sub)), "its owner always does");
    assert!(!frame(&w, Some(1)).units.iter().any(|u| u.unit_id == sub.0));

    // A submarine of theirs listens 700 m round.
    let hunter = spawn(
        &mut w,
        "aster_t1_submarine",
        1,
        1600,
        1000,
        flag::PASSIVE | flag::INVULNERABLE,
    );
    w.tick(&[]).unwrap();
    assert!(w.detects(1, row(&w, sub)));
    let contact = frame(&w, Some(1))
        .units
        .into_iter()
        .find(|u| u.unit_id == sub.0)
        .expect("drawn as a sonar contact");
    assert_ne!(
        contact.owner_flags & STATE_RADAR,
        0,
        "sonar is a contact, not a sighting"
    );

    // And a sonar station on the water, 1600 m round, while it has power.
    order(
        &mut w,
        1,
        Command::DebugRemove {
            units: vec![hunter],
        },
    );
    w.tick(&[]).unwrap();
    assert!(!w.detects(1, row(&w, sub)));
    w.state.players[1].free_build = true;
    spawn(&mut w, "aster_t1_sonar", 1, 1800, 1400, 0);
    w.tick(&[]).unwrap();
    assert!(w.detects(1, row(&w, sub)));
}

#[test]
fn guns_never_reach_a_dived_submarine() {
    let mut w = sea(false);
    let sub = spawn(&mut w, "aster_t1_submarine", 0, 1000, 1000, flag::PASSIVE);
    run(&mut w, 40);
    assert!(under(&w, sub));
    let frigate = spawn(&mut w, "aster_t1_frigate", 1, 1250, 1000, 0);
    let boat = spawn(&mut w, "aster_t1_attack_boat", 1, 1120, 1000, 0);
    let health = w.state.units.health[row(&w, sub)];
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        for id in [frigate, boat] {
            assert!(!w.state.units.weapon_target[row(&w, id)].contains(&sub));
        }
    }
    // Shelling the water right over it: the splash stays on the surface.
    let mut shots = 0;
    let over = w.state.units.pos[row(&w, sub)];
    order(
        &mut w,
        1,
        Command::AttackGround {
            units: vec![frigate],
            pos: over,
            queue: false,
        },
    );
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        shots += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::Impact { .. }))
            .count();
    }
    assert!(shots > 0, "the frigate shelled the spot");
    assert_eq!(w.state.units.health[row(&w, sub)], health);
    // Told to attack it, the frigate gives up.
    order(
        &mut w,
        1,
        Command::Attack {
            units: vec![frigate],
            target: sub,
            queue: false,
        },
    );
    run(&mut w, 2);
    assert_eq!(
        w.state.units.order_head[row(&w, frigate)],
        mc_sim::tables::NO_ORDER
    );
}

/// Runs until `target` dies. Returns the ticks it took and the torpedo hits seen.
fn hunt(w: &mut World, target: UnitId, dived: bool) -> (u32, usize) {
    let mut hits = 0;
    for t in 0..900 {
        w.tick(&[]).unwrap();
        hits += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::Impact { on_unit: true, .. }))
            .count();
        let p = &w.state.projectiles;
        for i in 0..p.len() {
            assert!(
                p.pos[i].z <= Fx::from_int(WATER - 1),
                "torpedoes stay under water"
            );
        }
        match w.state.units.row(target) {
            None => return (t, hits),
            Some(_) if dived => assert!(under(w, target)),
            Some(_) => {}
        }
    }
    panic!("the target lived");
}

#[test]
fn torpedoes_sink_a_boat_and_a_dived_submarine() {
    let mut w = sea(false);
    let boat = spawn(&mut w, "aster_t1_attack_boat", 1, 1250, 1000, flag::PASSIVE);
    spawn(&mut w, "aster_t1_submarine", 0, 1000, 1000, 0);
    let (_, hits) = hunt(&mut w, boat, false);
    assert!(hits >= 3, "380 hit points take three torpedoes");

    let mut w = sea(false);
    let prey = spawn(&mut w, "aster_t1_submarine", 1, 1250, 1100, flag::PASSIVE);
    run(&mut w, 40);
    assert!(under(&w, prey));
    spawn(&mut w, "aster_t1_submarine", 0, 1000, 1000, 0);
    let (_, hits) = hunt(&mut w, prey, true);
    assert!(hits >= 6, "900 hit points take six torpedoes");
    assert_eq!(
        w.state.sinking.len(),
        1,
        "a submarine goes down like any ship"
    );
}

#[test]
fn a_frigate_sinks_slowly_and_leaves_a_wreck_on_the_seabed() {
    let mut w = sea(false);
    let frigate = spawn(&mut w, "aster_t1_frigate", 0, 1000, 1000, flag::PASSIVE);
    w.tick(&[]).unwrap();
    order(
        &mut w,
        0,
        Command::SelfDestruct {
            units: vec![frigate],
        },
    );
    assert!(w
        .events
        .iter()
        .any(|e| matches!(e, SimEvent::UnitDied { .. })));
    assert_eq!(w.state.sinking.len(), 1);
    assert_eq!(w.state.wrecks.slots.live(), 0);
    let mut last = w.state.sinking[0].z;
    let mut ticks = 0;
    let settled = loop {
        w.tick(&[]).unwrap();
        ticks += 1;
        assert!(ticks < 400, "it never reached the bottom");
        if let Some(pos) = w.events.iter().find_map(|e| match e {
            SimEvent::ShipSettled { pos, .. } => Some(*pos),
            _ => None,
        }) {
            break pos;
        }
        let hull = &w.state.sinking[0];
        assert!(hull.z <= last);
        last = hull.z;
        if ticks == 10 {
            assert!(
                hull.z > Fx::from_int(WATER - 1),
                "it barely settles at first"
            );
        }
        let drawn = frame(&w, Some(0))
            .units
            .into_iter()
            .find(|u| u.owner_flags & KIND_WRECK != 0 && u._pad == WRECK_SINKING)
            .expect("the sinking hull is drawn");
        assert_eq!(drawn.unit_id, frigate.0);
        assert!(drawn.pos[2] <= drawn.prev_pos[2]);
    };
    assert!(
        (80..=200).contains(&ticks),
        "20 m of water should take 8 to 20 seconds, took {ticks} ticks"
    );
    assert!(w.state.sinking.is_empty());
    // It rests three tenths of its 10 m height over the bed.
    assert!(near(settled.z, Fx::from_int(3)));
    let wreck = w.state.wrecks.slots.iter().next().expect("a wreck");
    assert!(near(w.state.wrecks.z[wreck], Fx::from_int(3)));
    let bank = w.state.wrecks.bank[wreck];
    assert!(
        bank.unsigned_abs() >= Angle::from_degrees(15).0,
        "it lies with a list"
    );
    let drawn = frame(&w, Some(0))
        .units
        .into_iter()
        .find(|u| u.owner_flags & KIND_WRECK != 0)
        .unwrap();
    assert_eq!(drawn._pad, 0);
    assert!(drawn._pad2[0].abs() > 0.2 && drawn._pad2[0] == drawn._pad2[1]);
}

#[test]
fn sinking_is_the_same_on_every_run() {
    let go = || {
        let mut w = sea(false);
        let boat = spawn(&mut w, "aster_t1_attack_boat", 1, 1250, 1000, flag::PASSIVE);
        spawn(&mut w, "aster_t1_submarine", 0, 1000, 1000, 0);
        hunt(&mut w, boat, false);
        let mut hashes = Vec::new();
        for _ in 0..150 {
            hashes.push(w.tick(&[]).unwrap());
        }
        hashes
    };
    assert_eq!(go(), go());
}

#[test]
fn the_ai_never_puts_a_sonar_on_land() {
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let ore = |x: i32, y: i32| mc_map::OreRegion {
        points: [(-1, -1), (1, -1), (1, 1), (-1, 1)]
            .into_iter()
            .map(|(dx, dy)| FxVec2::from_ints(x + dx * 50, y + dy * 50))
            .collect(),
    };
    let map = MapData {
        name: "flat".into(),
        content_id: 1,
        ore: vec![
            ore(800, 512),
            ore(512, 800),
            ore(3296, 3584),
            ore(3584, 3296),
        ],
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(3584, 3584)],
        props: Vec::new(),
    };
    let player = |team| PlayerSetup {
        name: format!("ai{team}"),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller: Controller::Ai,
        start: team,
    };
    let config = MatchConfig {
        seed: 42,
        players: vec![player(0), player(1)],
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints(), Arc::new(Pool::new(1)), &config).unwrap();
    let sonar = w.blueprints.id_of("aster_t1_sonar").unwrap();
    let mut radar = false;
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        let units = &w.state.units;
        for r in units.slots.iter() {
            assert_ne!(units.blueprint[r], sonar, "a sonar went up on dry land");
            assert!(w.state.orders.iter(units, r).all(|o| o.blueprint != sonar));
            radar |= w.bp(r).has(cat::INTEL);
        }
    }
    assert!(radar, "the AI still builds its radar");
}

/// Drives a submarine from west of a parked frigate to east of it, on a line through
/// it. Returns the widest the submarine strayed off the line while alongside, the
/// closest it came to the frigate, and how far the frigate was shoved.
fn drive_past_a_frigate(dived: bool) -> (Fx, Fx, Fx) {
    let mut w = sea(false);
    let frigate = spawn(&mut w, "aster_t1_frigate", 0, 1200, 1000, flag::PASSIVE);
    let sub = spawn(&mut w, "aster_t1_submarine", 0, 900, 1000, flag::PASSIVE);
    if !dived {
        order(
            &mut w,
            0,
            Command::SetDive {
                units: vec![sub],
                dive: false,
            },
        );
    }
    run(&mut w, 40);
    assert_eq!(under(&w, sub), dived);
    order(
        &mut w,
        0,
        Command::Move {
            units: vec![sub],
            target: FxVec2::from_ints(1500, 1000),
            queue: false,
        },
    );
    let mut stray = Fx::ZERO;
    let mut closest = Fx::from_int(1000);
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        let at = w.state.units.pos[row(&w, sub)];
        let ship = w.state.units.pos[row(&w, frigate)];
        closest = closest.min(at.distance(ship));
        // Alongside the frigate: the flow field's own wander further on is not ours.
        if (at.x - Fx::from_int(1200)).abs() < Fx::from_int(60) {
            stray = stray.max((at.y - Fx::from_int(1000)).abs());
        }
    }
    let shoved = w.state.units.pos[row(&w, frigate)].distance(FxVec2::from_ints(1200, 1000));
    (stray, closest, shoved)
}

#[test]
fn a_dived_submarine_passes_straight_under_a_ship() {
    let (stray, closest, shoved) = drive_past_a_frigate(true);
    assert!(stray < Fx::ONE, "it strayed {stray:?} m off its line");
    assert!(
        closest < Fx::from_int(2),
        "it kept {closest:?} m from the frigate"
    );
    assert_eq!(shoved, Fx::ZERO, "the frigate was pushed");

    // Surfaced, the hulls bump as before: 15 + 10 m of radius keep them apart.
    let (_, closest, _) = drive_past_a_frigate(false);
    assert!(
        closest > Fx::from_int(15),
        "a surfaced submarine went through the frigate"
    );
}

/// A boat at full speed firing its rotary gun: every round, the shot's own and the
/// stream's, leaves from the gun where the boat is drawn at that moment, not from
/// where the gun was when the shot was fired, nor from a tick ahead of the hull.
#[test]
fn a_fast_boat_s_rounds_leave_from_the_gun_as_drawn() {
    use mc_sim::mirror::{PROJECTILE_FRESH, PROJECTILE_STARTS_SHIFT};
    let mut w = sea(false);
    let boat = spawn(&mut w, "aster_t1_attack_boat", 0, 500, 1000, 0);
    spawn(
        &mut w,
        "aster_t1_frigate",
        1,
        900,
        1090,
        flag::PASSIVE | flag::INVULNERABLE,
    );
    order(
        &mut w,
        0,
        Command::Move {
            units: vec![boat],
            target: FxVec2::from_ints(1800, 1000),
            queue: false,
        },
    );
    let mut checked = 0;
    for _ in 0..240 {
        w.tick(&[]).unwrap();
        let Some(r) = w.state.units.row(boat) else {
            break;
        };
        let travel = (w.state.units.pos[r] - w.state.units.prev_pos[r])
            .length()
            .to_f32();
        let f = frame(&w, None);
        let hull = f
            .units
            .iter()
            .find(|u| u.unit_id == boat.0)
            .expect("the boat is drawn");
        for p in &f.projectiles {
            if p.color & PROJECTILE_FRESH == 0 || travel < 2.0 {
                continue;
            }
            // Drawn leaving this far through the tick, from here.
            let t = (p.color >> PROJECTILE_STARTS_SHIFT) as f32 / 255.0;
            let out: [f32; 3] =
                std::array::from_fn(|a| p.prev_pos[a] + (p.pos[a] - p.prev_pos[a]) * t);
            let at: [f32; 3] =
                std::array::from_fn(|a| hull.prev_pos[a] + (hull.pos[a] - hull.prev_pos[a]) * t);
            let off = ((out[0] - at[0]).powi(2) + (out[1] - at[1]).powi(2)).sqrt();
            // The muzzle is 4.6 m forward of the hull's origin, on a turret 3.3 m forward.
            assert!(
                off < 5.2,
                "a round left {off:.1} m from the hull at {travel:.1} m a tick"
            );
            checked += 1;
        }
    }
    assert!(checked > 20, "only {checked} rounds seen leaving at speed");
}

#[test]
fn a_frigate_turns_to_bring_its_gun_onto_a_boat_astern() {
    let mut w = sea(false);
    let frigate = spawn(&mut w, "aster_t1_frigate", 0, 1000, 1000, 0);
    spawn(
        &mut w,
        "aster_t1_attack_boat",
        1,
        700,
        1000,
        flag::PASSIVE | flag::INVULNERABLE,
    );
    let gun = w.bp(row(&w, frigate)).weapons[0].half_arc;
    for t in 0..300 {
        w.tick(&[]).unwrap();
        let r = row(&w, frigate);
        // Whatever the hull does, the gun stays inside its arc.
        let yaw = mc_core::Angle::ZERO.delta_to(w.state.units.weapon_yaw[r][0]);
        assert!(yaw.unsigned_abs() <= gun);
        let fired = w.events.iter().any(|e| {
            matches!(
                e,
                SimEvent::ShotFired {
                    weapon: 0,
                    owner: 0,
                    ..
                }
            )
        });
        if fired {
            let heading = mc_core::Angle::ZERO.delta_to(w.state.units.heading[r]);
            assert!(
                heading.unsigned_abs() > Angle::from_degrees(40).0,
                "it fired astern without turning (tick {t})"
            );
            return;
        }
    }
    panic!("the frigate never fired at the boat astern");
}

/// A submarine fires at a boat; returns the world just after the torpedoes are away.
fn torpedoes_away() -> (World, UnitId) {
    let mut w = sea(false);
    let boat = spawn(&mut w, "aster_t1_attack_boat", 1, 1300, 1000, flag::PASSIVE);
    spawn(&mut w, "aster_t1_submarine", 0, 1000, 1000, 0);
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        if !w.state.projectiles.pos.is_empty() {
            return (w, boat);
        }
    }
    panic!("no torpedoes");
}

#[test]
fn a_torpedo_whose_mark_is_gone_bursts_where_it_is() {
    let (mut w, boat) = torpedoes_away();
    let remove = PlayerCommand {
        player: 1,
        command: Command::DebugRemove { units: vec![boat] },
    };
    let mut bursts = 0;
    for t in 0..3 {
        w.tick(if t == 0 {
            std::slice::from_ref(&remove)
        } else {
            &[]
        })
        .unwrap();
        for e in &w.events {
            if let SimEvent::Impact { pos, on_unit, .. } = e {
                assert!(!on_unit);
                assert!(
                    pos.x < Fx::from_int(1200),
                    "it burst out by the launch, not at the mark"
                );
                bursts += 1;
            }
        }
    }
    assert!(bursts >= 1);
    assert!(w.state.projectiles.pos.is_empty());
}

#[test]
fn a_torpedo_fired_at_a_point_runs_there_and_bursts() {
    let mut w = sea(false);
    let sub = spawn(&mut w, "aster_t1_submarine", 0, 1000, 1000, 0);
    run(&mut w, 40);
    let point = FxVec2::from_ints(1250, 1000);
    order(
        &mut w,
        0,
        Command::AttackGround {
            units: vec![sub],
            pos: point,
            queue: false,
        },
    );
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        if let Some(pos) = w.events.iter().find_map(|e| match e {
            SimEvent::Impact { pos, .. } => Some(*pos),
            _ => None,
        }) {
            assert!(
                pos.xy().distance(point) < Fx::from_int(8),
                "it burst at {pos:?}, not at the point"
            );
            return;
        }
    }
    panic!("the torpedo never burst");
}
