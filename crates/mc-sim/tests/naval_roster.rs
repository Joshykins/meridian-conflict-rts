//! The naval roster's own weapons and rules (docs/NAVY.md): interceptor torpedoes,
//! sea skimmers, high arcs from a dived hull, surfaced-only deck guns, torpedoes
//! under a dome, and the deep-wreck salvage rule.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, FireState, Handle};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// The sea: 20 m of water over a flat bed at zero, with a strip of land along the west
/// edge (x under about 312 m), `land` metres high.
const WATER: i32 = 20;

fn blueprints() -> Arc<Blueprints> {
    Arc::new(Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap())
}

fn shore(fog: bool, land: u16) -> World {
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..40 {
            samples[y * 257 + x] = land;
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

fn sea(fog: bool) -> World {
    shore(fog, 40)
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

fn health(w: &World, id: UnitId) -> Fx {
    w.state.units.row(id).map_or(Fx::ZERO, |r| w.state.units.health[r])
}

fn under(w: &World, id: UnitId) -> bool {
    let r = row(w, id);
    w.state.units.z[r] + w.bp(r).height < Fx::from_int(WATER)
}

/// Shots fired this tick by `owner` from weapon slot `weapon` of blueprint `key`.
fn fired(w: &World, key: &str, owner: u8, weapon: u8) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.events
        .iter()
        .filter(|e| {
            matches!(e, SimEvent::ShotFired { blueprint, owner: o, weapon: s, .. }
                if *blueprint == bp && *o == owner && *s == weapon)
        })
        .count()
}

/// A Barracuda puts two salvoes of torpedoes into `key`, 300 m off. Returns the
/// damage it took, the interceptions seen, and the hash of every tick.
fn torpedoes_at(key: &str) -> (Fx, usize, Vec<u64>) {
    let mut w = sea(false);
    let target = spawn(&mut w, key, 0, 1300, 1000, flag::PASSIVE);
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 1000, 1000, 0);
    let full = health(&w, target);
    order(
        &mut w,
        1,
        Command::Attack {
            units: vec![sub],
            target,
            queue: false,
        },
    );
    let (mut met, mut launched, mut hashes) = (0, 0, Vec::new());
    for _ in 0..140 {
        hashes.push(w.tick(&[]).unwrap());
        launched += fired(&w, "aster_t1_submarine", 1, 0);
        met += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::TorpedoIntercepted { .. }))
            .count();
    }
    assert!(launched >= 4, "two salvoes of two, only {launched} away");
    (full - health(&w, target), met, hashes)
}

#[test]
fn a_destroyer_s_interceptors_burst_the_torpedoes_coming_in() {
    let (damage, met, _) = torpedoes_at("aster_t2_destroyer");
    assert!(met >= 4, "only {met} torpedoes intercepted");
    assert_eq!(damage, Fx::ZERO, "a torpedo got through to the Marlin");

    // A frigate has no interceptor tubes.
    let (damage, met, _) = torpedoes_at("aster_t1_frigate");
    assert_eq!(met, 0);
    assert!(damage > Fx::ZERO, "the torpedoes missed the Pike");
}

#[test]
fn interceptors_never_take_a_unit_and_are_the_same_on_every_run() {
    let mut w = sea(false);
    let marlin = spawn(&mut w, "aster_t2_destroyer", 0, 1300, 1000, 0);
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 1000, 1000, flag::INVULNERABLE);
    let tubes = 3;
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        let r = row(&w, marlin);
        assert_eq!(w.state.units.weapon_target[r][tubes], Handle::NONE);
        assert!(w.state.units.row(sub).is_some());
    }
    let (_, _, a) = torpedoes_at("aster_t2_destroyer");
    let (_, _, b) = torpedoes_at("aster_t2_destroyer");
    assert_eq!(a, b);
}

#[test]
fn a_missile_ship_s_skimmers_hug_the_sea_climb_the_coast_and_strike_inland() {
    let mut w = sea(false);
    let target = spawn(&mut w, "aster_t1_power", 1, 150, 1000, 0);
    let ship = spawn(&mut w, "aster_t2_missile_ship", 0, 1050, 1000, 0);
    let full = health(&w, target);
    let heading = w.state.units.heading[row(&w, ship)];
    order(
        &mut w,
        0,
        Command::Attack {
            units: vec![ship],
            target,
            queue: false,
        },
    );
    let missiles = w.blueprints.id_of("aster_t2_missile_ship").unwrap();
    let mark = FxVec2::from_ints(150, 1000);
    let water = Fx::from_int(WATER);
    let (mut seen, mut highest) = (0, Fx::ZERO);
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        let p = &w.state.projectiles;
        for i in (0..p.len()).filter(|&i| p.blueprint[i] == missiles) {
            let at = p.pos[i];
            if at.xy().distance(mark) <= Fx::from_int(120) || p.age[i] < 20 {
                continue;
            }
            // 25 m over the sea and the land; up to 34 m just off the 20 m cliff,
            // where it is already climbing to the land's height.
            let over = at.z - w.terrain.height_at(at.xy()).max(water);
            highest = highest.max(over);
            assert!(over < Fx::from_int(40), "{over:?} m up at {at:?}");
            assert!(over > Fx::from_int(5), "down on the surface at {at:?}");
            seen += 1;
        }
        if health(&w, target) < full {
            break;
        }
    }
    assert!(seen > 20, "the missiles were barely seen");
    assert!(highest > Fx::from_int(20), "they never climbed to their height");
    assert!(health(&w, target) < full, "the power plant was not hit");
    assert_eq!(
        w.state.units.heading[row(&w, ship)],
        heading,
        "the ship turned to launch from its deck cells"
    );
}

#[test]
fn a_dived_strategic_submarine_lobs_a_high_arc_and_gives_itself_away() {
    let mut w = sea(true);
    let target = spawn(&mut w, "aster_t1_power", 1, 150, 1000, 0);
    let kraken = spawn(&mut w, "aster_t3_submarine", 0, 1650, 1000, 0);
    // Radar reaches the submarine; nothing of theirs has sonar.
    spawn(
        &mut w,
        "aster_t1_frigate",
        1,
        1650,
        1400,
        flag::PASSIVE | flag::INVULNERABLE,
    );
    // It fires only where it is told: its own sonar hears the frigate.
    let r = row(&w, kraken);
    w.state.units.fire_state[r] = FireState::HoldFire;
    run(&mut w, 60);
    assert!(under(&w, kraken));
    assert!(!w.detects(1, row(&w, kraken)), "radar found a dived hull");
    let full = health(&w, target);
    let fire = PlayerCommand {
        player: 0,
        command: Command::AttackGround {
            units: vec![kraken],
            pos: FxVec2::from_ints(150, 1000),
            queue: false,
        },
    };
    let arcs = w.blueprints.id_of("aster_t3_submarine").unwrap();
    let (mut launches, mut last, mut highest) = (0, None, Fx::ZERO);
    let mut hit = None;
    for t in 0..260u32 {
        w.tick(if t == 0 { std::slice::from_ref(&fire) } else { &[] })
            .unwrap();
        assert!(under(&w, kraken), "it surfaced to fire");
        for e in &w.events {
            if let SimEvent::DivedLaunch { pos, blueprint, .. } = e {
                assert_eq!(*blueprint, arcs);
                assert!(pos.z < Fx::from_int(WATER), "launched from above the water");
                launches += 1;
                last = Some(t);
            }
        }
        let p = &w.state.projectiles;
        for i in (0..p.len()).filter(|&i| p.blueprint[i] == arcs && p.weapon[i] == 1) {
            highest = highest.max(p.pos[i].z);
        }
        if let Some(l) = last {
            if t - l < 75 {
                assert!(w.detects(1, row(&w, kraken)), "hidden {} ticks after a launch", t - l);
            } else if t - l > 85 {
                assert!(!w.detects(1, row(&w, kraken)), "still seen {} ticks after", t - l);
            }
        }
        if hit.is_none() && health(&w, target) < full {
            hit = Some(t);
        }
    }
    assert_eq!(launches, 4, "one salvo of four");
    assert!(highest > Fx::from_int(1000), "the arc topped out at {highest:?} m");
    assert!(hit.is_some(), "the power plant was not hit");
    assert!(last.unwrap() + 85 < 260, "the test never saw the boat go quiet");
}

#[test]
fn a_hunter_killer_s_deck_gun_only_works_surfaced() {
    // A low shore, so the deck gun has a clear line up the beach.
    let mut w = shore(false, 24);
    let tank = spawn(
        &mut w,
        "aster_t1_tank",
        1,
        250,
        1000,
        flag::PASSIVE | flag::INVULNERABLE,
    );
    let moray = spawn(&mut w, "aster_t2_submarine", 0, 420, 1000, 0);
    run(&mut w, 40);
    assert!(under(&w, moray));
    let home = w.state.units.pos[row(&w, moray)];
    for _ in 0..150 {
        w.tick(&[]).unwrap();
        assert_eq!(fired(&w, "aster_t2_submarine", 0, 1), 0, "it fired dived");
        let r = row(&w, moray);
        assert_eq!(w.state.units.weapon_target[r][1], Handle::NONE);
    }
    assert!(
        w.state.units.pos[row(&w, moray)].distance(home) < Fx::from_int(2),
        "it went after the tank dived"
    );
    let _ = tank;
    order(
        &mut w,
        0,
        Command::SetDive {
            units: vec![moray],
            dive: false,
        },
    );
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        if fired(&w, "aster_t2_submarine", 0, 1) > 0 {
            assert!(!under(&w, moray));
            return;
        }
    }
    panic!("the surfaced Moray never fired its deck gun");
}

#[test]
fn torpedoes_run_under_a_shield_boat_s_dome_that_stops_shells() {
    let mut w = sea(false);
    // The dome's upkeep is paid.
    w.state.players[0].free_build = true;
    let boat = spawn(&mut w, "aster_t2_shield_boat", 0, 1000, 1000, flag::PASSIVE);
    let pike = spawn(&mut w, "aster_t1_frigate", 0, 1040, 1000, flag::PASSIVE);
    run(&mut w, 80);
    let dome = |w: &World| w.state.units.shield_hp[row(w, boat)];
    assert!(dome(&w) > Fx::ZERO, "the dome is not up");
    let full = health(&w, pike);

    let gun = spawn(&mut w, "aster_t1_frigate", 1, 1400, 1000, 0);
    order(
        &mut w,
        1,
        Command::Attack {
            units: vec![gun],
            target: pike,
            queue: false,
        },
    );
    let mut on_dome = 0;
    for _ in 0..120 {
        w.tick(&[]).unwrap();
        on_dome += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::Impact { on_shield: true, .. }))
            .count();
    }
    assert!(on_dome >= 3, "only {on_dome} shells broke on the dome");
    assert_eq!(health(&w, pike), full, "a shell got under the dome");
    let charged = dome(&w);
    order(&mut w, 1, Command::DebugRemove { units: vec![gun] });
    run(&mut w, 30);

    let sub = spawn(&mut w, "aster_t1_submarine", 1, 1300, 1000, 0);
    order(
        &mut w,
        1,
        Command::Attack {
            units: vec![sub],
            target: pike,
            queue: false,
        },
    );
    let mut on_hull = 0;
    for _ in 0..150 {
        w.tick(&[]).unwrap();
        assert!(dome(&w) > Fx::ZERO, "the dome fell");
        on_hull += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::Impact { on_unit: true, on_shield: false, .. }))
            .count();
    }
    assert!(on_hull >= 2, "only {on_hull} torpedoes struck the Pike");
    assert!(health(&w, pike) < full, "the torpedoes did not hurt the Pike");
    assert!(dome(&w) >= charged, "the dome took the torpedoes");
}

#[test]
fn only_a_salvage_boat_reaches_a_wreck_in_deep_water() {
    let mut w = sea(false);
    w.state.players[0].free_build = true;
    let frigate = spawn(&mut w, "aster_t1_frigate", 0, 1000, 1000, flag::PASSIVE);
    order(
        &mut w,
        0,
        Command::SelfDestruct {
            units: vec![frigate],
        },
    );
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        if w.state.wrecks.slots.live() > 0 {
            break;
        }
    }
    let at = w.state.wrecks.slots.iter().next().expect("a wreck on the bed");
    let wreck = w.state.wrecks.slots.handle(at);
    assert!(Fx::from_int(WATER) - w.state.wrecks.z[at] > Fx::from_int(10));
    let mass = w.state.wrecks.mass[at];

    // An engineer hovering right over it cannot reach down 17 m.
    let engineer = spawn(&mut w, "aster_t1_engineer", 0, 1000, 1050, 0);
    order(
        &mut w,
        0,
        Command::ReclaimWreck {
            units: vec![engineer],
            wreck,
            queue: false,
        },
    );
    run(&mut w, 100);
    assert_eq!(w.state.wrecks.mass[at], mass, "the engineer reclaimed a deep wreck");
    assert_eq!(w.state.units.order_head[row(&w, engineer)], mc_sim::tables::NO_ORDER);
    order(&mut w, 0, Command::DebugRemove { units: vec![engineer] });

    // The Trawler sails into reach, raises its mast, and only then works.
    let trawler = spawn(&mut w, "aster_t1_salvage_boat", 0, 1250, 1000, 0);
    let need = w.bp(row(&w, trawler)).motion.unwrap().deploy_ticks;
    assert!(need > 0);
    order(
        &mut w,
        0,
        Command::ReclaimWreck {
            units: vec![trawler],
            wreck,
            queue: false,
        },
    );
    let mut started = None;
    for t in 0..600 {
        w.tick(&[]).unwrap();
        let r = row(&w, trawler);
        let left = if w.state.wrecks.slots.is_alive(at) {
            w.state.wrecks.mass[at]
        } else {
            Fx::ZERO
        };
        if left < mass && started.is_none() {
            started = Some(t);
            assert!(w.state.units.deploy[r] >= need, "it worked with the mast down");
            assert!(
                w.state.units.pos[r].distance(FxVec2::from_ints(1000, 1000)) < Fx::from_int(150),
                "it reached from where it started"
            );
        }
        if left == Fx::ZERO {
            break;
        }
    }
    assert!(started.is_some(), "the Trawler never reclaimed the wreck");
    assert!(!w.state.wrecks.slots.is_alive(at), "the wreck was not cleared");

    // Sent off, it lowers the mast before it gets under way.
    let r = row(&w, trawler);
    assert_eq!(w.state.units.deploy[r], need);
    let from = w.state.units.pos[r];
    order(
        &mut w,
        0,
        Command::Move {
            units: vec![trawler],
            target: FxVec2::from_ints(1500, 1000),
            queue: false,
        },
    );
    for _ in 0..need as usize / 2 {
        w.tick(&[]).unwrap();
        assert_eq!(w.state.units.pos[row(&w, trawler)], from, "it sailed with the mast up");
    }
    run(&mut w, 100);
    let r = row(&w, trawler);
    assert_eq!(w.state.units.deploy[r], 0);
    assert!(w.state.units.pos[r].distance(from) > Fx::from_int(20));
}
