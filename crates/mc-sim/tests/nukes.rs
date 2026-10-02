//! Strategic missiles (`nukes.rs`): a silo assembles warheads and launches one where it
//! is told; the blast runs out over seconds and hurts everything; an interceptor array
//! shoots down a warhead coming down near it; a commander goes up as a small nuke.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::SimEvent;
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
        name: "nukes".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(4000, 4000)],
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
        players: vec![player("you", 0), player("them", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        Heightfield::flat(1024, 1024, Fx::from_int(20)),
        map,
        blueprints,
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

fn spawn(w: &mut World, owner: u8, key: &str, x: i32, y: i32) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let at = FxVec2::from_ints(x, y);
    w.tick(&[PlayerCommand {
        player: owner,
        command: Command::DebugSpawn {
            owner,
            blueprint,
            pos: at,
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    }])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == blueprint && u.owner[r] == owner)
        .min_by_key(|&r| u.pos[r].distance_sq(at))
        .unwrap();
    u.id(row)
}

fn alive(w: &World, id: UnitId) -> bool {
    w.state.units.row(id).is_some()
}

fn stock(w: &mut World, silo: UnitId, n: u8) {
    w.state.strategic.launchers.entry(silo).or_default().stock = n;
}

fn free(w: &mut World, player: u8) {
    w.tick(&[PlayerCommand {
        player,
        command: Command::DebugFreeBuild { player, on: true },
    }])
    .unwrap();
}

fn launch(w: &mut World, silo: UnitId, x: i32, y: i32) {
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::LaunchNuke {
            units: vec![silo],
            pos: FxVec2::from_ints(x, y),
        },
    }])
    .unwrap();
}

/// Ticks until an event matching `f`, at most `limit`; the tick count if it came.
fn until(w: &mut World, limit: u32, f: impl Fn(&SimEvent) -> bool) -> Option<u32> {
    for t in 0..limit {
        w.tick(&[]).unwrap();
        if w.events.iter().any(&f) {
            return Some(t);
        }
    }
    None
}

#[test]
fn a_silo_assembles_its_warheads_and_keeps_no_more_than_its_stock() {
    let mut w = world();
    free(&mut w, 0);
    let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    let spec = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t4_nuke_silo").unwrap())
        .strategic
        .clone()
        .unwrap();
    let per_round = (spec.round_seconds() * 10.0).ceil() as u32;
    let ready = until(&mut w, per_round + 20, |e| {
        matches!(e, SimEvent::RoundReady { warhead: true, .. })
    });
    assert!(
        ready.is_some(),
        "a round is ready after about {per_round} ticks"
    );
    assert_eq!(w.state.strategic.launchers[&silo].stock, 1);
    for _ in 0..per_round * 3 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.strategic.launchers[&silo].stock, spec.stock,
        "it stops at its stock"
    );
}

#[test]
fn a_warhead_flies_to_its_mark_and_the_blast_runs_out_over_seconds() {
    let mut w = world();
    let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    stock(&mut w, silo, 1);
    // Under the burst, halfway out, and out past the edge; one of our own in the middle too.
    let middle = spawn(&mut w, 1, "aster_t3_power", 3600, 3600);
    let own = spawn(&mut w, 0, "aster_t1_tank", 3620, 3600);
    let edge = spawn(&mut w, 1, "aster_t1_tank", 3600 + 480, 3600);
    let clear = spawn(&mut w, 1, "aster_t1_tank", 3600 + 900, 3600);
    // A plant far off, so losing the reactor does not lose them the match.
    spawn(&mut w, 1, "aster_t1_power", 7000, 7000);
    launch(&mut w, silo, 3600, 3600);
    assert!(w
        .events
        .iter()
        .any(|e| matches!(e, SimEvent::SiloOpening { .. })));
    let lit = until(&mut w, 80, |e| matches!(e, SimEvent::NuclearLaunch { .. }))
        .expect("it lights once the doors are open");
    assert!(lit >= 30, "the doors take a few seconds first ({lit})");
    assert_eq!(w.state.strategic.launchers[&silo].stock, 0);
    let flight = until(&mut w, 1500, |e| {
        matches!(
            e,
            SimEvent::NuclearDetonation {
                commander: false,
                ..
            }
        )
    })
    .expect("the warhead comes down");
    // About 4.3 km of ground: tens of seconds, not an instant.
    assert!(flight > 150 && flight < 900, "flight {flight} ticks");
    let SimEvent::NuclearDetonation { pos, .. } = w
        .events
        .iter()
        .find(|e| matches!(e, SimEvent::NuclearDetonation { .. }))
        .unwrap()
        .clone()
    else {
        unreachable!()
    };
    assert!(
        pos.xy().distance(FxVec2::from_ints(3600, 3600)) < Fx::from_int(2),
        "on its mark"
    );
    w.tick(&[]).unwrap();
    assert!(!alive(&w, middle), "the reactor under it is gone at once");
    assert!(!alive(&w, own), "its own side is not spared");
    assert!(alive(&w, edge), "the front has not reached the edge yet");
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    assert!(
        !alive(&w, edge),
        "a light tank near the edge is caught when the front gets there"
    );
    assert!(alive(&w, clear), "out past the radius nothing is hurt");
}

#[test]
fn an_interceptor_array_shoots_down_a_warhead_coming_down_near_it() {
    let mut w = world();
    let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    stock(&mut w, silo, 1);
    let array = spawn(&mut w, 1, "aster_t3_nuke_defense", 3400, 3400);
    stock(&mut w, array, 2);
    let target = spawn(&mut w, 1, "aster_t3_power", 3600, 3600);
    launch(&mut w, silo, 3600, 3600);
    let mut killed = false;
    for _ in 0..1500 {
        w.tick(&[]).unwrap();
        assert!(
            !w.events
                .iter()
                .any(|e| matches!(e, SimEvent::NuclearDetonation { .. })),
            "the warhead must not get through"
        );
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::WarheadIntercepted { killed: true, .. }))
        {
            killed = true;
            break;
        }
    }
    assert!(killed, "the array's interceptor got it");
    assert!(alive(&w, target));
    assert_eq!(
        w.state.strategic.launchers[&array].stock, 1,
        "one interceptor for one warhead"
    );
    assert!(w
        .state
        .strategic
        .missiles
        .iter()
        .all(|m| m.kind != mc_sim::nukes::MissileKind::Warhead));
}

#[test]
fn an_array_ignores_a_warhead_bound_elsewhere() {
    let mut w = world();
    let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    stock(&mut w, silo, 1);
    let array = spawn(&mut w, 1, "aster_t3_nuke_defense", 600, 4000);
    stock(&mut w, array, 1);
    launch(&mut w, silo, 4400, 4400);
    let hit = until(&mut w, 1500, |e| {
        matches!(e, SimEvent::NuclearDetonation { .. })
    });
    assert!(hit.is_some());
    assert_eq!(w.state.strategic.launchers[&array].stock, 1);
}

#[test]
fn a_commander_goes_up_as_a_small_nuke() {
    let mut w = world();
    let commander = spawn(&mut w, 0, "aster_commander", 1500, 1500);
    let near = spawn(&mut w, 1, "aster_t1_tank", 1560, 1500);
    let row = w.state.units.row(commander).unwrap();
    w.state.units.health[row] = Fx::ZERO;
    w.tick(&[]).unwrap();
    assert!(w.events.iter().any(|e| matches!(
        e,
        SimEvent::NuclearDetonation {
            commander: true,
            ..
        }
    )));
    for _ in 0..5 {
        w.tick(&[]).unwrap();
    }
    assert!(!alive(&w, near));
}

#[test]
fn a_warhead_goes_through_a_dome() {
    let mut w = world();
    let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    stock(&mut w, silo, 1);
    free(&mut w, 1);
    let dome = spawn(&mut w, 1, "aster_t3_shield", 3600, 3700);
    let row = w.state.units.row(dome).unwrap();
    let full = w
        .blueprints
        .unit(w.state.units.blueprint[row])
        .shield
        .unwrap()
        .health;
    w.state.units.shield_open[row] = 255;
    w.state.units.prev_shield_open[row] = 255;
    w.state.units.shield_hp[row] = full;
    let under = spawn(&mut w, 1, "aster_t2_tank", 3620, 3720);
    spawn(&mut w, 1, "aster_t1_power", 7000, 7000);
    launch(&mut w, silo, 3600, 3600);
    until(&mut w, 1500, |e| {
        matches!(e, SimEvent::NuclearDetonation { .. })
    })
    .expect("it comes down");
    for _ in 0..50 {
        w.tick(&[]).unwrap();
    }
    assert!(
        !alive(&w, under),
        "a full dome does not save what is under it from a warhead"
    );
}

fn cmd(w: &mut World, command: Command) {
    w.tick(&[PlayerCommand { player: 0, command }]).unwrap();
}

#[test]
fn with_auto_build_off_a_launcher_builds_only_what_is_queued() {
    let mut w = world();
    free(&mut w, 0);
    let array = spawn(&mut w, 0, "aster_t3_nuke_defense", 600, 600);
    cmd(
        &mut w,
        Command::SetAutoBuild {
            units: vec![array],
            on: false,
        },
    );
    let spec = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t3_nuke_defense").unwrap())
        .strategic
        .clone()
        .unwrap();
    let per_round = (spec.round_seconds() * 10.0).ceil() as u32;
    for _ in 0..per_round + 20 {
        w.tick(&[]).unwrap();
    }
    // It had begun its first round when auto-build went off: that one is kept as queued.
    assert_eq!(
        w.state.strategic.launchers[&array].stock, 1,
        "only the round in hand"
    );
    for _ in 0..per_round * 2 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.strategic.launchers[&array].stock, 1,
        "nothing queued, nothing more built"
    );
    cmd(
        &mut w,
        Command::QueueRounds {
            units: vec![array],
            count: 2,
        },
    );
    for _ in 0..per_round * 4 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.strategic.launchers[&array].stock, 3,
        "exactly the two queued"
    );
    cmd(
        &mut w,
        Command::SetAutoBuild {
            units: vec![array],
            on: true,
        },
    );
    for _ in 0..per_round * 3 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.strategic.launchers[&array].stock, spec.stock,
        "auto-build fills it"
    );
}

#[test]
fn engineers_assisting_a_silo_speed_up_its_warhead() {
    let alone = {
        let mut w = world();
        free(&mut w, 0);
        spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
        until(&mut w, 5000, |e| matches!(e, SimEvent::RoundReady { .. })).unwrap()
    };
    let helped = {
        let mut w = world();
        free(&mut w, 0);
        let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
        let masons: Vec<_> = (0..3)
            .map(|i| spawn(&mut w, 0, "aster_t3_engineer", 560 + i * 12, 660))
            .collect();
        cmd(
            &mut w,
            Command::Assist {
                units: masons,
                target: silo,
                queue: false,
            },
        );
        until(&mut w, 5000, |e| matches!(e, SimEvent::RoundReady { .. })).unwrap()
    };
    assert!(
        helped * 2 < alone,
        "three Mason IIIs (power 180) beside a silo (60) at least halve it: {helped} vs {alone}"
    );
}

fn warhead(w: &World) -> Option<&mc_sim::nukes::StrategicMissile> {
    w.state
        .strategic
        .missiles
        .iter()
        .find(|m| m.kind == mc_sim::nukes::MissileKind::Warhead)
}

#[test]
fn a_warhead_climbs_straight_then_turns_over_smoothly_onto_its_mark() {
    let mut w = world();
    let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    stock(&mut w, silo, 1);
    spawn(&mut w, 1, "aster_t1_power", 7000, 7000);
    launch(&mut w, silo, 6000, 3000);
    until(&mut w, 80, |e| matches!(e, SimEvent::NuclearLaunch { .. })).expect("it lights");
    let m = warhead(&w).unwrap();
    let spec = w.blueprints.unit(m.blueprint).strategic.clone().unwrap();
    let path = mc_sim::nukes::WarheadPath::new(m.origin, m.mark, spec.apogee);
    let expected = path.ticks_left(spec.speed, m.age, m.travelled);
    let start = m.pos;
    let (mut vel, mut top, mut ticks) = (m.vel.to_f32(), m.pos.z, 0);
    loop {
        w.tick(&[]).unwrap();
        ticks += 1;
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::NuclearDetonation { .. }))
        {
            break;
        }
        let m = warhead(&w).expect("still flying");
        top = top.max(m.pos.z);
        let v = m.vel.to_f32();
        if ticks <= 45 {
            assert!(
                m.pos.xy().distance(start.xy()) < Fx::ONE,
                "tick {ticks}: still straight up out of the tube"
            );
        }
        let (a, b) = (glam_len(vel), glam_len(v));
        if a > 1.0 && b > 1.0 {
            let cos = (vel[0] * v[0] + vel[1] * v[1] + vel[2] * v[2]) / (a * b);
            // Never a kink: a few degrees a tick at most.
            assert!(
                cos > 0.995,
                "tick {ticks}: heading turned {:.1} degrees in a tick",
                cos.clamp(-1.0, 1.0).acos().to_degrees()
            );
            assert!(
                (b - a).abs() < a * 0.2 + 1.0,
                "tick {ticks}: speed jumped from {a} to {b}"
            );
        }
        vel = v;
    }
    assert_eq!(
        ticks, expected,
        "the flight time shown is the flight time flown"
    );
    assert!(
        vel[2] < -0.8 * glam_len(vel),
        "it comes down steep: {vel:?}"
    );
    assert!(top.to_f32() > 20.0 + 2000.0, "a high arc ({top})");
}

fn glam_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[test]
fn an_array_holds_fire_until_it_can_meet_the_warhead_inside_its_cover() {
    let mut w = world();
    let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    stock(&mut w, silo, 1);
    let array = spawn(&mut w, 1, "aster_t3_nuke_defense", 6900, 6900);
    stock(&mut w, array, 2);
    // Power for the array: a dark grid fires nothing.
    spawn(&mut w, 1, "aster_t3_power", 6500, 7200);
    let cover = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t3_nuke_defense").unwrap())
        .strategic
        .clone()
        .unwrap()
        .coverage;
    let at = FxVec2::from_ints(6900, 6900);
    launch(&mut w, silo, 7000, 7000);
    let mut fired = None;
    for t in 0..2000 {
        w.tick(&[]).unwrap();
        if std::env::var("NUKE_DEBUG").is_ok() {
            for m in &w.state.strategic.missiles {
                eprintln!(
                    "{t} {:?} {:?} v{:?} {:?}",
                    m.kind,
                    m.pos.to_f32(),
                    m.vel.to_f32(),
                    w.events
                        .iter()
                        .filter(|e| matches!(e, SimEvent::WarheadIntercepted { .. }))
                        .count()
                );
            }
        }
        assert!(
            !w.events
                .iter()
                .any(|e| matches!(e, SimEvent::NuclearDetonation { .. })),
            "a long shot is still stopped"
        );
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::InterceptorLaunch { .. }))
            && fired.is_none()
        {
            let m = warhead(&w).expect("the warhead is still up");
            // It fires ahead of the warhead reaching the cover, so as to meet it inside.
            assert!(
                m.pos.xy().distance(at) <= cover * 2,
                "fired while the warhead was {} m out",
                m.pos.xy().distance(at)
            );
            assert!(m.vel.z < Fx::ZERO, "fired at a warhead still climbing");
            fired = Some(t);
        }
        if let Some(SimEvent::WarheadIntercepted { pos, .. }) = w
            .events
            .iter()
            .find(|e| matches!(e, SimEvent::WarheadIntercepted { killed: true, .. }))
        {
            assert!(
                pos.xy().distance(at) <= cover,
                "met {} m out, outside the cover",
                pos.xy().distance(at)
            );
        }
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::WarheadIntercepted { killed: true, .. }))
        {
            assert_eq!(
                w.state.strategic.launchers[&array].stock, 1,
                "one interceptor for one warhead"
            );
            return;
        }
    }
    panic!("never intercepted (fired at {fired:?})");
}

#[test]
fn a_silo_fires_every_mark_it_is_given_in_turn() {
    let mut w = world();
    let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    stock(&mut w, silo, 2);
    spawn(&mut w, 1, "aster_t1_power", 7000, 7000);
    launch(&mut w, silo, 3000, 3000);
    launch(&mut w, silo, 3000, 4500);
    // A third has no warhead left for it.
    launch(&mut w, silo, 4500, 3000);
    assert_eq!(w.state.strategic.launchers[&silo].targets.len(), 2);
    let first =
        until(&mut w, 80, |e| matches!(e, SimEvent::NuclearLaunch { .. })).expect("the first goes");
    let second = until(&mut w, 80, |e| matches!(e, SimEvent::NuclearLaunch { .. }))
        .expect("the second follows");
    assert!(
        second > 10 && second < 40,
        "a short gap between them ({first}, then {second} more)"
    );
    assert_eq!(w.state.strategic.launchers[&silo].stock, 0);
    let marks: Vec<_> = w
        .state
        .strategic
        .missiles
        .iter()
        .map(|m| m.mark.xy())
        .collect();
    assert_eq!(
        marks,
        [FxVec2::from_ints(3000, 3000), FxVec2::from_ints(3000, 4500)],
        "in the order given"
    );
    let row = w.state.units.row(silo).unwrap();
    assert!(
        w.state.units.deploy[row] > 0,
        "the doors stayed open between them"
    );
}

#[test]
fn a_launch_order_takes_the_silo_with_the_most_warheads_free() {
    let mut w = world();
    let a = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    let b = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 1600);
    stock(&mut w, a, 1);
    stock(&mut w, b, 2);
    let order = |w: &mut World, x, y| {
        w.tick(&[PlayerCommand {
            player: 0,
            command: Command::LaunchNuke {
                units: vec![a, b],
                pos: FxVec2::from_ints(x, y),
            },
        }])
        .unwrap();
    };
    let queued = |w: &World| {
        (
            w.state.strategic.launchers[&a].targets.len(),
            w.state.strategic.launchers[&b].targets.len(),
        )
    };
    order(&mut w, 600, 400);
    assert_eq!(
        queued(&w),
        (0, 1),
        "the fuller silo first, though the other is nearer"
    );
    order(&mut w, 600, 400);
    assert_eq!(queued(&w), (1, 1), "then, one free each, the nearer");
    order(&mut w, 600, 400);
    assert_eq!(queued(&w), (1, 2));
    order(&mut w, 600, 400);
    assert_eq!(queued(&w), (1, 2), "nothing free, nothing done");
}

/// Degrees between two directions.
fn turn_deg(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    (dot / (glam_len(a) * glam_len(b)).max(1e-6))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

/// Flies `marks` in from a silo at (600, 600) against an array at `array`, and returns,
/// for every interceptor, whether it killed its warhead, the ticks it flew and the
/// degrees it turned after its boost.
fn intercept_run(array: (i32, i32), marks: &[(i32, i32)]) -> Vec<(bool, u32, f32)> {
    use mc_sim::nukes::MissileKind;
    let mut w = world();
    let silo = spawn(&mut w, 0, "aster_t4_nuke_silo", 600, 600);
    stock(&mut w, silo, marks.len() as u8);
    let defence = spawn(&mut w, 1, "aster_t3_nuke_defense", array.0, array.1);
    stock(&mut w, defence, marks.len() as u8);
    spawn(&mut w, 1, "aster_t3_power", array.0 + 300, array.1 - 300);
    for &(x, y) in marks {
        launch(&mut w, silo, x, y);
    }
    // serial -> (last velocity, degrees turned, ticks flown)
    let mut flying: std::collections::BTreeMap<u32, ([f32; 3], f32, u32)> = Default::default();
    let mut out = Vec::new();
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        let now: Vec<_> = w
            .state
            .strategic
            .missiles
            .iter()
            .filter(|m| m.kind == MissileKind::Interceptor)
            .map(|m| (m.serial, m.vel.to_f32(), m.age))
            .collect();
        for &(serial, vel, age) in &now {
            let e = flying.entry(serial).or_insert((vel, 0.0, 0));
            if age > 13 {
                e.1 += turn_deg(e.0, vel);
            }
            e.0 = vel;
            e.2 = age;
        }
        let gone: Vec<u32> = flying
            .keys()
            .copied()
            .filter(|s| now.iter().all(|n| n.0 != *s))
            .collect();
        // Interceptors spent this tick take this tick's kills, one each.
        let mut kills = w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::WarheadIntercepted { killed: true, .. }))
            .count();
        for s in gone {
            let (_, turned, ticks) = flying.remove(&s).unwrap();
            out.push((kills > 0, ticks, turned));
            kills = kills.saturating_sub(1);
        }
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::NuclearDetonation { .. }))
        {
            out.push((false, 0, 0.0));
        }
        if w.state.strategic.missiles.is_empty() && !out.is_empty() {
            break;
        }
    }
    out
}

#[test]
fn interceptors_fly_straight_at_their_warheads_from_every_side() {
    let mark = (4000, 4000);
    let mut worst = (0u32, 0.0f32);
    let mut bad = Vec::new();
    for reach in [0, 700, 1400, 2100] {
        for dir in 0..8 {
            let a = std::f32::consts::FRAC_PI_4 * dir as f32;
            let at = (
                mark.0 + (reach as f32 * a.cos()) as i32,
                mark.1 + (reach as f32 * a.sin()) as i32,
            );
            for (killed, ticks, turned) in intercept_run(at, &[mark]) {
                worst = (worst.0.max(ticks), worst.1.max(turned));
                if !killed || turned > 120.0 {
                    bad.push((at, killed, ticks, turned));
                }
            }
            if reach == 0 {
                break;
            }
        }
    }
    println!("worst: {} ticks, {:.0} degrees turned", worst.0, worst.1);
    assert!(bad.is_empty(), "misses or doubling back: {bad:?}");
}

#[test]
fn a_salvo_is_met_one_interceptor_a_warhead_without_doubling_back() {
    let marks = [
        (4000, 4000),
        (4300, 3800),
        (3700, 4200),
        (4100, 4400),
        (3900, 3600),
    ];
    let runs = intercept_run((4200, 4200), &marks);
    println!("{runs:?}");
    assert_eq!(runs.iter().filter(|r| r.0).count(), marks.len(), "{runs:?}");
    assert!(runs.iter().all(|r| r.2 <= 120.0), "doubling back: {runs:?}");
}

#[test]
fn a_round_finishes_through_a_mass_stall() {
    // A stall used to scale the round's last bit as well, which then shrank every tick
    // until it rounded to nothing: the array sat at 100% and never got its interceptor.
    let mut w = world();
    let array = spawn(&mut w, 0, "aster_t3_nuke_defense", 600, 600);
    let spec = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t3_nuke_defense").unwrap())
        .strategic
        .clone()
        .unwrap();
    w.state
        .strategic
        .launchers
        .entry(array)
        .or_default()
        .progress = spec.round_time * Fx::ratio(99, 100);
    // A silo beside it keeps the side stalled however little the array still asks for.
    spawn(&mut w, 0, "aster_t4_nuke_silo", 700, 600);
    let silo = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t4_nuke_silo").unwrap())
        .strategic
        .clone()
        .unwrap();
    // About a third of the mass both ask for, every tick; energy to spare.
    let per_tick = |s: &mc_data::strategic::Strategic| s.round_mass * s.power / s.round_time;
    let trickle = (per_tick(&spec) + per_tick(&silo)) / 30;
    let mut ready = None;
    for t in 0..600 {
        w.state.players[0].mass = trickle;
        w.state.players[0].energy = Fx::from_int(1_000_000);
        w.tick(&[]).unwrap();
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::RoundReady { .. }))
        {
            ready = Some(t);
            break;
        }
    }
    let l = &w.state.strategic.launchers[&array];
    assert!(
        ready.is_some(),
        "the round is finished while mass stalls (stuck at {} of {})",
        l.progress,
        spec.round_time
    );
    assert_eq!(l.stock, 1);
}
