//! Combat: twin barrels take turns.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
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

#[test]
fn paladin_projectors_take_turns() {
    let mut w = world();
    let paladin = w.blueprints.id_of("aster_t3_assault_bot").unwrap();
    let reload = w.blueprints.unit(paladin).weapons[0].reload_ticks as u32;

    let mut shots = Vec::new();
    let mut record = |w: &World| {
        let tick = w.tick_count();
        for e in &w.events {
            if let SimEvent::ShotFired {
                blueprint, weapon, ..
            } = e
            {
                if *blueprint == paladin {
                    shots.push((tick, *weapon));
                }
            }
        }
    };
    w.tick(&[
        spawn(&w, 0, "aster_t3_assault_bot", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            650,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    record(&w);
    for _ in 0..120 {
        w.tick(&[]).unwrap();
        record(&w);
    }

    assert!(shots.len() >= 4, "the Paladin never opened up: {shots:?}");
    for pair in shots.windows(2) {
        assert_ne!(
            pair[0].0, pair[1].0,
            "both projectors fired on the same tick: {shots:?}"
        );
        assert_ne!(
            pair[0].1, pair[1].1,
            "the same projector fired twice in a row: {shots:?}"
        );
    }
    for barrel in 0..=1 {
        let times: Vec<u32> = shots
            .iter()
            .filter(|(_, w)| *w == barrel)
            .map(|(t, _)| *t)
            .collect();
        for pair in times.windows(2) {
            assert_eq!(
                pair[1] - pair[0],
                reload,
                "projector {barrel} reload is not {reload}: {shots:?}"
            );
        }
    }
}

#[test]
fn javelin_ripples_a_volley_out_the_tubes() {
    let mut w = world();
    let javelin = w.blueprints.id_of("aster_t2_missile").unwrap();
    let weapon = &w.blueprints.unit(javelin).weapons[0];
    assert_eq!(weapon.salvo, 6);
    assert!(weapon.missile);
    assert_eq!(weapon.muzzles.len(), 6);
    assert!(weapon.salvo_delay_ticks > 0);

    // In front of the rack, so the hull is already on the tubes' bearing.
    w.tick(&[
        spawn(&w, 0, "aster_t2_missile", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            800,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();

    let mut fire_ticks = Vec::new();
    let mut first_vel = None;
    let mut record = |w: &World| {
        for e in &w.events {
            if let SimEvent::ShotFired { blueprint, vel, .. } = e {
                if *blueprint == javelin {
                    fire_ticks.push(w.tick_count());
                    first_vel.get_or_insert(*vel);
                }
            }
        }
    };
    record(&w);
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        record(&w);
    }
    assert_eq!(fire_ticks.len(), 6, "the volley was {fire_ticks:?}");
    assert!(
        fire_ticks.windows(2).all(|w| w[1] > w[0]),
        "the tubes dumped together: {fire_ticks:?}"
    );
    let vel = first_vel.expect("a missile left a tube");
    assert!(
        vel.x > vel.y.abs() * 2,
        "the missile did not leave along the hull ({vel:?})"
    );
    let slope = vel.z.to_f32() / vel.x.to_f32().hypot(vel.y.to_f32());
    assert!(
        (slope - 1.19).abs() < 0.2,
        "the missile did not leave at the elevated rake ({vel:?}, slope {slope})"
    );
}

#[test]
fn javelin_fires_before_the_rack_faces_the_target() {
    let mut w = world();
    let javelin = w.blueprints.id_of("aster_t2_missile").unwrap();
    w.tick(&[
        spawn(&w, 0, "aster_t2_missile", 500, 0),
        cmd(Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of("aster_t1_tank").unwrap(),
            pos: FxVec2::from_ints(500, 812),
            heading: Angle::ZERO,
            count: 1,
            flags: flag::PASSIVE | flag::INVULNERABLE,
            build: 1000,
        }),
    ])
    .unwrap();

    let mut first_vel = None;
    let mut first_yaw = None;
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::ShotFired { blueprint, vel, .. } = e {
                if *blueprint == javelin {
                    first_vel.get_or_insert(*vel);
                    if first_yaw.is_none() {
                        let row = w
                            .state
                            .units
                            .slots
                            .iter()
                            .find(|&r| w.state.units.blueprint[r] == javelin)
                            .unwrap();
                        first_yaw = Some(w.state.units.weapon_yaw[row][0]);
                    }
                }
            }
        }
        if first_vel.is_some() {
            break;
        }
    }
    let _ = first_vel.expect("the rack waited to face the target");
    let yaw = first_yaw.unwrap();
    let slewed = Angle::ZERO.delta_to(yaw).unsigned_abs();
    assert!(
        slewed > 0 && slewed < Angle::from_degrees(80).0 as u16,
        "the rack should have been mid-slew, not on target ({})",
        yaw.0
    );
}

#[test]
fn trebuchet_charges_before_the_first_shot() {
    let mut w = world();
    let trebuchet = w.blueprints.id_of("aster_t3_artillery").unwrap();
    let charge = w.blueprints.unit(trebuchet).weapons[0].charge_ticks;
    let deploy = w.blueprints.unit(trebuchet).motion.unwrap().deploy_ticks;
    assert!(charge > 10, "the siege gun should wind up for a long beat");
    assert!(deploy > 10, "the siege gun should plant before it fires");
    assert!(
        w.blueprints.unit(trebuchet).weapons[0].plasma > 1.0,
        "the siege slug should carry a plasma sheath"
    );

    // In front, past minimum range, dummy so the tube can settle on it.
    w.tick(&[
        spawn(&w, 0, "aster_t3_artillery", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            900,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();

    let mut charged_at = None;
    let mut fired_at = None;
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        let tick = w.tick_count();
        for e in &w.events {
            match e {
                SimEvent::WeaponCharging { blueprint, .. } if *blueprint == trebuchet => {
                    charged_at.get_or_insert(tick);
                }
                SimEvent::ShotFired { blueprint, .. } if *blueprint == trebuchet => {
                    fired_at.get_or_insert(tick);
                }
                _ => {}
            }
        }
        if fired_at.is_some() {
            break;
        }
    }

    let charged = charged_at.expect("the Trebuchet never charged");
    let fired = fired_at.expect("the Trebuchet never fired");
    let row = (0..w.state.units.slots.rows())
        .find(|&r| w.state.units.slots.is_alive(r) && w.state.units.blueprint[r] == trebuchet)
        .expect("the Trebuchet is gone");
    assert_eq!(
        w.state.units.deploy[row], deploy,
        "it should be fully planted when it fires"
    );
    assert!(
        charged < fired,
        "it fired on tick {fired} before charging on {charged}"
    );
    assert_eq!(
        fired - charged,
        charge as u32,
        "the first shot should wait out the charge ({charge} ticks), charged {charged} fired {fired}"
    );
}

#[test]
fn trebuchet_packs_before_it_moves() {
    let mut w = world();
    let trebuchet = w.blueprints.id_of("aster_t3_artillery").unwrap();
    let deploy = w.blueprints.unit(trebuchet).motion.unwrap().deploy_ticks;

    w.tick(&[
        spawn(&w, 0, "aster_t3_artillery", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            900,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();

    let row = (0..w.state.units.slots.rows())
        .find(|&r| w.state.units.slots.is_alive(r) && w.state.units.blueprint[r] == trebuchet)
        .unwrap();
    let id = w.state.units.id(row);
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        if w.state.units.deploy[row] >= deploy {
            break;
        }
    }
    assert_eq!(w.state.units.deploy[row], deploy, "it never planted");
    let planted = w.state.units.pos[row];

    w.tick(&[cmd(Command::Move {
        units: vec![id],
        target: FxVec2::from_ints(200, 512),
        queue: false,
    })])
    .unwrap();
    assert_eq!(
        w.state.units.pos[row], planted,
        "it rolled off while the spade was still down"
    );
    assert!(
        w.state.units.deploy[row] < deploy,
        "a move order should start packing"
    );

    let mut packed_at = None;
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        if w.state.units.deploy[row] == 0 {
            packed_at = Some(w.tick_count());
            break;
        }
        assert_eq!(
            w.state.units.pos[row], planted,
            "it moved while still planted"
        );
    }
    packed_at.expect("it never packed");
    w.tick(&[]).unwrap();
    assert_ne!(
        w.state.units.pos[row], planted,
        "once packed it should roll"
    );
}

#[test]
fn trebuchet_does_not_aim_until_planted() {
    let mut w = world();
    let trebuchet = w.blueprints.id_of("aster_t3_artillery").unwrap();
    let deploy = w.blueprints.unit(trebuchet).motion.unwrap().deploy_ticks;

    // Off to the side so a slew is obvious: ahead would already be yaw 0.
    w.tick(&[
        spawn(&w, 0, "aster_t3_artillery", 500, 0),
        cmd(Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of("aster_t1_tank").unwrap(),
            pos: FxVec2::from_ints(500, 900),
            heading: Angle::ZERO,
            count: 1,
            flags: flag::PASSIVE | flag::INVULNERABLE,
            build: 1000,
        }),
    ])
    .unwrap();

    let row = (0..w.state.units.slots.rows())
        .find(|&r| w.state.units.slots.is_alive(r) && w.state.units.blueprint[r] == trebuchet)
        .unwrap();
    let id = w.state.units.id(row);

    for _ in 0..80 {
        w.tick(&[]).unwrap();
        if w.state.units.deploy[row] >= deploy {
            break;
        }
        assert_eq!(
            w.state.units.weapon_yaw[row][0],
            Angle::ZERO,
            "the turret slewed while still planting"
        );
    }
    assert_eq!(w.state.units.deploy[row], deploy, "it never planted");

    let mut slewed = None;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        let yaw = Angle::ZERO
            .delta_to(w.state.units.weapon_yaw[row][0])
            .unsigned_abs();
        if yaw > Angle::from_degrees(8).0 as u16 {
            slewed = Some(yaw);
            break;
        }
    }
    slewed.expect("the turret never turned once planted");

    w.tick(&[cmd(Command::Move {
        units: vec![id],
        target: FxVec2::from_ints(200, 512),
        queue: false,
    })])
    .unwrap();

    let mut packed = false;
    let mut last_yaw = Angle::ZERO
        .delta_to(w.state.units.weapon_yaw[row][0])
        .unsigned_abs();
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        if w.state.units.deploy[row] == 0 {
            packed = true;
        }
        if packed {
            let yaw = Angle::ZERO
                .delta_to(w.state.units.weapon_yaw[row][0])
                .unsigned_abs();
            assert!(
                yaw <= last_yaw + 64,
                "the turret slewed toward the target while rolling"
            );
            last_yaw = yaw;
        }
    }
    assert!(packed, "it never packed");
    assert!(
        last_yaw < Angle::from_degrees(8).0 as u16,
        "the turret should have come home once it was rolling"
    );
}

#[test]
fn bastion_walks_a_salvo_across_the_barrels() {
    let mut w = world();
    let bastion = w.blueprints.id_of("aster_t2_point_defense").unwrap();
    let weapon = &w.blueprints.unit(bastion).weapons[0];
    assert_eq!(weapon.salvo, 3);
    assert_eq!(weapon.muzzles.len(), 3);
    assert_eq!(weapon.charge_ticks, 0);

    w.tick(&[
        spawn(&w, 0, "aster_t2_point_defense", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            800,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();

    let mut shots = Vec::new();
    let mut charged = false;
    let mut record = |w: &World| {
        for e in &w.events {
            match e {
                SimEvent::WeaponCharging { blueprint, .. } if *blueprint == bastion => {
                    charged = true;
                }
                SimEvent::ShotFired { blueprint, pos, .. } if *blueprint == bastion => {
                    shots.push((w.tick_count(), *pos));
                }
                _ => {}
            }
        }
    };
    record(&w);
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        record(&w);
    }

    assert!(!charged, "the Bastion should fire without a charge");
    assert!(shots.len() >= 3, "the salvo was {shots:?}");
    let salvo = &shots[..3];
    assert!(
        salvo.windows(2).all(|w| w[1].0 > w[0].0),
        "the barrels dumped together: {salvo:?}"
    );
    let ys: Vec<f32> = salvo.iter().map(|(_, p)| p.y.to_f32()).collect();
    let (lo, hi) = (
        ys.iter().copied().fold(f32::MAX, f32::min),
        ys.iter().copied().fold(f32::MIN, f32::max),
    );
    assert!(hi - lo > 2.0, "shots left from the same mouth: y {ys:?}");
}

#[test]
fn bulwark_dumps_both_rails() {
    let mut w = world();
    let bulwark = w.blueprints.id_of("aster_t2_tank").unwrap();
    let weapon = &w.blueprints.unit(bulwark).weapons[0];
    assert_eq!(weapon.salvo, 2);
    assert_eq!(weapon.muzzles.len(), 2);
    assert_eq!(weapon.salvo_delay_ticks, 0);

    w.tick(&[
        spawn(&w, 0, "aster_t2_tank", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            650,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();

    let mut shots = Vec::new();
    let mut record = |w: &World| {
        for e in &w.events {
            if let SimEvent::ShotFired { blueprint, pos, .. } = e {
                if *blueprint == bulwark {
                    shots.push((w.tick_count(), *pos));
                }
            }
        }
    };
    record(&w);
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        record(&w);
    }

    assert!(shots.len() >= 2, "the Bulwark never opened up: {shots:?}");
    let first = shots[0].0;
    let salvo: Vec<_> = shots.iter().filter(|(t, _)| *t == first).collect();
    assert_eq!(salvo.len(), 2, "both rails should fire together: {shots:?}");
    let ys: Vec<f32> = salvo.iter().map(|(_, p)| p.y.to_f32()).collect();
    let (lo, hi) = (
        ys.iter().copied().fold(f32::MAX, f32::min),
        ys.iter().copied().fold(f32::MIN, f32::max),
    );
    assert!(hi - lo > 0.8, "shots left from the same mouth: y {ys:?}");
}

fn unit_of(w: &World, owner: u8, key: &str) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == owner && w.state.units.blueprint[r] == id)
        .expect(key)
}

/// The range fights with the economy off; a generator still asks for power.
fn powered(w: &mut World) {
    w.state.players[0].free_build = true;
}

#[test]
fn weapons_keep_their_target_when_a_closer_enemy_appears() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_tank", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            650,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();

    let shooter = unit_of(&w, 0, "aster_t1_tank");
    let first = unit_of(&w, 1, "aster_t1_tank");
    let first_id = w.state.units.id(first);
    for _ in 0..8 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.units.weapon_target[shooter][0], first_id,
        "the tank should have locked the first enemy"
    );

    w.tick(&[spawn(
        &w,
        1,
        "aster_t1_tank",
        560,
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let closer = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == 1 && r != first)
        .expect("closer enemy");
    let closer_id = w.state.units.id(closer);
    assert_ne!(closer_id, first_id);

    for _ in 0..30 {
        w.tick(&[]).unwrap();
        assert_eq!(
            w.state.units.weapon_target[shooter][0], first_id,
            "a closer enemy should not steal the lock"
        );
    }

    let shooter_id = w.state.units.id(shooter);
    w.tick(&[cmd(Command::Attack {
        units: vec![shooter_id],
        target: closer_id,
        queue: false,
    })])
    .unwrap();
    assert_eq!(
        w.state.units.weapon_target[shooter][0], closer_id,
        "an Attack order should take the gun"
    );

    w.tick(&[cmd(Command::Stop {
        units: vec![shooter_id],
    })])
    .unwrap();
    for _ in 0..8 {
        w.tick(&[]).unwrap();
        assert_eq!(
            w.state.units.weapon_target[shooter][0], closer_id,
            "stop should not send the gun back to the old lock"
        );
    }

    w.tick(&[cmd(Command::DebugDamage {
        units: vec![closer_id],
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    assert_eq!(
        w.state.units.weapon_target[shooter][0], first_id,
        "once the lock dies it should pick the remaining enemy"
    );
}

#[test]
fn an_attack_order_fires_at_full_range() {
    let mut w = world();
    // Tank gun is 180 m. 90 % of that is 162 m. Park the dummy just inside
    // max range so a close-in walk would move, but a fire order must not.
    w.tick(&[
        spawn(&w, 0, "aster_t1_tank", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            670,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    let shooter = unit_of(&w, 0, "aster_t1_tank");
    let target = unit_of(&w, 1, "aster_t1_tank");
    let start = w.state.units.pos[shooter];
    let range = w.bp(shooter).weapons[0].range_max;
    let gap = start.distance(w.state.units.pos[target]) - w.bp(target).radius;
    assert!(
        gap <= range,
        "the dummy should be in range ({gap:?} / {range:?})"
    );
    assert!(
        gap > range * Fx::ratio(9, 10),
        "the dummy should sit past the old 90% close-in"
    );

    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(shooter)],
        target: w.state.units.id(target),
        queue: false,
    })])
    .unwrap();
    assert_eq!(
        w.state.units.flags[shooter] & flag::HOLD,
        flag::HOLD,
        "in range it should stand and shoot"
    );
    assert_eq!(
        w.state.units.weapon_target[shooter][0],
        w.state.units.id(target)
    );

    let mut fired = false;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        assert_eq!(
            w.state.units.flags[shooter] & flag::HOLD,
            flag::HOLD,
            "it should keep holding while the target stays in range"
        );
        assert!(
            start.distance(w.state.units.pos[shooter]) < Fx::from_int(2),
            "it walked in instead of firing ({:?})",
            w.state.units.pos[shooter]
        );
        fired |= w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::ShotFired { .. }));
    }
    assert!(fired, "an Attack order in range should fire");
}

#[test]
fn a_turret_takes_an_attack_order() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[
        spawn(&w, 0, "aster_t1_point_defense", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            580,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            700,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    let turret = unit_of(&w, 0, "aster_t1_point_defense");
    let closer = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == 1 && w.state.units.pos[r].x < Fx::from_int(640))
        .expect("closer tank");
    let farther = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == 1 && r != closer)
        .expect("farther tank");
    let closer_id = w.state.units.id(closer);
    let farther_id = w.state.units.id(farther);
    for _ in 0..8 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.units.weapon_target[turret][0], closer_id,
        "the turret should have locked the closer tank"
    );

    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(turret)],
        target: farther_id,
        queue: false,
    })])
    .unwrap();
    assert_eq!(
        w.state.units.weapon_target[turret][0], farther_id,
        "an Attack order on a turret should take the gun"
    );
    assert_eq!(
        w.state.orders.front(&w.state.units, turret).map(|o| o.kind),
        Some(OrderKind::Attack)
    );

    let mut fired = false;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        assert_eq!(w.state.units.weapon_target[turret][0], farther_id);
        fired |= w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::ShotFired { .. }));
    }
    assert!(fired, "the turret should fire at the named target");
}

#[test]
fn a_shield_stops_incoming_fire() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[
        spawn(&w, 0, "aster_t2_shield", 600, 0),
        spawn(&w, 0, "aster_t1_tank", 650, 0),
        spawn(&w, 1, "aster_t1_tank", 800, 0),
    ])
    .unwrap();

    let shield = unit_of(&w, 0, "aster_t2_shield");
    let friend = unit_of(&w, 0, "aster_t1_tank");
    let spec = w
        .blueprints
        .unit(w.state.units.blueprint[shield])
        .shield
        .unwrap();
    assert_eq!(w.state.units.shield_open[shield], 255);
    assert_eq!(w.state.units.shield_hp[shield], spec.health);
    let friend_hp = w.state.units.health[friend];
    let gap = w.state.units.pos[shield].distance(w.state.units.pos[friend]);
    assert!(gap < spec.radius, "the tank should sit under the dome");

    let mut hits = 0;
    for _ in 0..50 {
        w.tick(&[]).unwrap();
        hits += w
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    SimEvent::Impact {
                        on_shield: true,
                        ..
                    }
                )
            })
            .count();
    }
    assert!(hits >= 2, "the dome was never struck: {hits} hits");
    assert_eq!(
        w.state.units.health[friend], friend_hp,
        "the tank under the dome was hurt"
    );
}

#[test]
fn a_paladin_hull_shield_runs_on_grid_power() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[spawn(&w, 0, "aster_t3_assault_bot", 600, 0)])
        .unwrap();
    let paladin = unit_of(&w, 0, "aster_t3_assault_bot");
    assert!(w.bp(paladin).shield.unwrap().is_hull());
    assert!(
        w.bp(paladin).economy.energy_upkeep > Fx::ZERO,
        "a shield draws energy"
    );
    for _ in 0..20 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(w.state.units.shield_open[paladin], 255);
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    let shown = frame
        .shields
        .iter()
        .find(|s| s.unit_id == w.state.units.id(paladin).0)
        .expect("the wrap should be in the frame");
    assert!(shown.open > 0.9);
    assert!(shown.radius > 13.0, "wrap radius {}", shown.radius);
    assert!(shown.height > 20.0, "wrap height {}", shown.height);
    assert_eq!((shown.packed >> 25) & 1, 1, "hull bit");

    // The grid runs dry: the wrap drops like a dome does.
    w.state.players[0].free_build = false;
    w.state.players[0].energy = Fx::ZERO;
    for _ in 0..20 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.units.shield_open[paladin], 0,
        "a hull wrap should fold when its upkeep cannot be paid"
    );
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    if let Some(shown) = frame
        .shields
        .iter()
        .find(|s| s.unit_id == w.state.units.id(paladin).0)
    {
        assert_eq!((shown.packed >> 26) & 1, 1, "stalled bit");
    }
}

#[test]
fn a_paladin_hull_shield_stops_incoming_fire() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[
        spawn(&w, 0, "aster_t3_assault_bot", 600, flag::PASSIVE),
        spawn(&w, 1, "aster_t1_tank", 700, 0),
    ])
    .unwrap();

    let paladin = unit_of(&w, 0, "aster_t3_assault_bot");
    let spec = w.bp(paladin).shield.expect("paladin carries a hull wrap");
    assert!(spec.is_hull());
    assert_eq!(w.state.units.shield_open[paladin], 255);
    let hull = w.state.units.health[paladin];
    let shield = w.state.units.shield_hp[paladin];

    let mut hits = 0;
    for _ in 0..50 {
        w.tick(&[]).unwrap();
        hits += w
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    SimEvent::Impact {
                        on_shield: true,
                        ..
                    }
                )
            })
            .count();
    }
    assert!(hits >= 2, "the wrap was never struck: {hits} hits");
    assert_eq!(
        w.state.units.health[paladin], hull,
        "shots reached the Paladin through its wrap"
    );
    assert!(
        w.state.units.shield_hp[paladin] < shield,
        "the wrap did not take the hits"
    );
}

#[test]
fn a_paladin_hull_shield_does_not_cover_a_neighbour() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[
        spawn(&w, 0, "aster_t3_assault_bot", 600, flag::PASSIVE),
        spawn(&w, 0, "aster_t1_tank", 640, flag::PASSIVE),
        spawn(&w, 1, "aster_t1_tank", 720, 0),
    ])
    .unwrap();

    let paladin = unit_of(&w, 0, "aster_t3_assault_bot");
    let friend = unit_of(&w, 0, "aster_t1_tank");
    let friend_hp = w.state.units.health[friend];
    let wrap = w.state.units.shield_hp[paladin];
    let gap = w.state.units.pos[paladin].distance(w.state.units.pos[friend]);
    assert!(
        gap > w.bp(paladin).shield.unwrap().radius,
        "the tank should stand outside the wrap"
    );

    let mut hull_hits = 0;
    for _ in 0..50 {
        w.tick(&[]).unwrap();
        hull_hits += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::Impact { on_unit: true, .. }))
            .count();
    }
    assert!(
        hull_hits >= 2,
        "the neighbour was never struck: {hull_hits}"
    );
    assert!(
        w.state.units.health[friend] < friend_hp,
        "the tank beside the Paladin was covered"
    );
    assert_eq!(
        w.state.units.shield_hp[paladin], wrap,
        "the Paladin's wrap ate shots meant for the tank"
    );
}

#[test]
fn a_broken_shield_shatters_and_lets_shots_through() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[
        spawn(&w, 0, "aster_t2_shield", 600, 0),
        spawn(&w, 0, "aster_t1_tank", 650, 0),
        spawn(&w, 1, "aster_t1_tank", 800, 0),
    ])
    .unwrap();

    let shield = unit_of(&w, 0, "aster_t2_shield");
    let friend = unit_of(&w, 0, "aster_t1_tank");
    let friend_hp = w.state.units.health[friend];

    // Regen outpaces a Warden's cannon, so the test keeps the bubble one scratch
    // from breaking until a shot lands.
    let mut broke = false;
    for _ in 0..60 {
        w.state.units.shield_hp[shield] = Fx::from_int(5);
        w.tick(&[]).unwrap();
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::ShieldBroken { .. }))
        {
            broke = true;
            break;
        }
    }
    assert!(broke, "the bubble never shattered");
    let open = w.state.units.shield_open[shield];
    assert!(open < 255, "the glass should have started peeling");
    assert!(
        open > 0,
        "a break peels over a few ticks, it does not pop off"
    );
    assert_eq!(w.state.units.shield_hp[shield], Fx::ZERO);
    assert!(
        w.state.units.shield_recharge[shield] > 0,
        "shots pass while the peel is still on screen"
    );

    for _ in 0..40 {
        w.tick(&[]).unwrap();
    }
    assert!(
        w.state.units.health[friend] < friend_hp,
        "shots should reach the tank once the dome is down"
    );
    assert_eq!(
        w.state.units.shield_open[shield], 0,
        "the peel should have finished"
    );
}

#[test]
fn a_t2_shield_names_its_successor() {
    let w = world();
    let t2 = w.blueprints.id_of("aster_t2_shield").unwrap();
    let t3 = w.blueprints.id_of("aster_t3_shield").unwrap();
    assert_eq!(w.blueprints.unit(t2).upgrades_to, Some(t3));
    assert!(
        w.blueprints.unit(t3).shield.unwrap().radius > w.blueprints.unit(t2).shield.unwrap().radius
    );
}

#[test]
fn a_stalled_shield_closes() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[spawn(&w, 0, "aster_t2_shield", 600, 0)]).unwrap();
    let shield = unit_of(&w, 0, "aster_t2_shield");
    assert_eq!(w.state.units.shield_open[shield], 255);
    w.state.players[0].free_build = false;
    w.tick(&[]).unwrap();
    assert!(
        w.state.units.shield_open[shield] < 255,
        "a stall should start folding the bubble"
    );
}

#[test]
fn a_t2_shield_stays_up_while_it_is_refitted() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[spawn(&w, 0, "aster_t2_shield", 600, 0)]).unwrap();
    let shield = unit_of(&w, 0, "aster_t2_shield");
    let id = w.state.units.id(shield);
    let t2_radius = w.bp(shield).shield.unwrap().radius.to_f32();
    let t3_radius = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t3_shield").unwrap())
        .shield
        .unwrap()
        .radius
        .to_f32();
    w.tick(&[cmd(Command::Upgrade { units: vec![id] })])
        .unwrap();
    assert_eq!(
        w.state.units.shield_open[shield], 255,
        "the dome should stay up while the next kit is built onto it"
    );

    let child = w
        .state
        .units
        .row(w.state.units.build_target[shield])
        .expect("the successor is assembling");
    w.state.units.build_progress[child] = w.bp(child).build_time * Fx::ratio(1, 2);
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    let shown = frame
        .shields
        .iter()
        .find(|s| s.unit_id == id.0)
        .expect("the bubble is still drawn");
    assert!(
        shown.open > 0.9,
        "the projected dome is still open ({})",
        shown.open
    );
    assert!(
        shown.radius > t2_radius + 1.0 && shown.radius < t3_radius,
        "it should be growing toward the next tier ({} -> {} vs {}..{})",
        t2_radius,
        shown.radius,
        t2_radius,
        t3_radius
    );

    w.state.units.build_progress[child] = w.bp(child).build_time - Fx::ONE;
    w.tick(&[]).unwrap();
    w.tick(&[]).unwrap();
    let t3 = unit_of(&w, 0, "aster_t3_shield");
    assert_eq!(t3, shield, "the generator is the same unit");
    assert_eq!(w.state.units.id(t3), id);
    assert_eq!(
        w.state.units.shield_open[t3], 255,
        "the new dome should take over already open"
    );
    w.write_render_frame(None, &mut frame);
    assert!(
        frame
            .shields
            .iter()
            .any(|s| s.open > 0.9 && s.radius > t2_radius + 10.0),
        "the successor's bubble is visible at the larger radius"
    );
}

#[test]
fn a_broken_shield_fills_immediately_and_rises_full() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[spawn(&w, 0, "aster_t2_shield", 600, 0)]).unwrap();
    let shield = unit_of(&w, 0, "aster_t2_shield");
    let spec = w.bp(shield).shield.unwrap();
    w.state.units.shield_hp[shield] = Fx::ZERO;
    w.state.units.shield_open[shield] = 0;
    w.state.units.prev_shield_open[shield] = 0;
    w.state.units.shield_recharge[shield] = 1;

    w.tick(&[]).unwrap();
    let filling = w.state.units.shield_hp[shield];
    assert!(filling > Fx::ZERO, "recovery should start on the next tick");
    assert!(filling < spec.health, "one tick must not refill the dome");
    assert_eq!(w.state.units.shield_open[shield], 0);
    assert_eq!(
        w.state.units.shield_recharge[shield], 1,
        "a filling dome is still down"
    );

    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    let shown = frame
        .shields
        .iter()
        .find(|s| s.unit_id == w.state.units.id(shield).0)
        .expect("charge should be visible while the dome is down");
    assert!(
        shown.health > 0.0,
        "the selection bar should already be filling"
    );
    assert_eq!(shown.open, 0.0, "the bubble itself stays closed");

    w.state.units.shield_hp[shield] = spec.health - Fx::ONE;
    w.tick(&[]).unwrap();
    assert_eq!(w.state.units.shield_hp[shield], spec.health);
    assert_eq!(w.state.units.shield_recharge[shield], 0);
    assert!(
        w.state.units.shield_open[shield] > 0,
        "the dome should start rising once it is full"
    );
}

#[test]
fn engineers_boost_live_shield_regen_for_energy() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[spawn(&w, 0, "aster_t2_shield", 600, 0)]).unwrap();
    let shield = unit_of(&w, 0, "aster_t2_shield");
    let spec = w.bp(shield).shield.unwrap();
    let half = spec.health / 2;

    w.state.units.shield_hp[shield] = half;
    w.tick(&[]).unwrap();
    let solo = w.state.units.shield_hp[shield] - half;
    assert!(solo > Fx::ZERO, "a live dome should regen on its own");

    w.tick(&[spawn(&w, 0, "aster_t1_engineer", 580, 0)])
        .unwrap();
    let engineer = unit_of(&w, 0, "aster_t1_engineer");
    let eid = w.state.units.id(engineer);
    let sid = w.state.units.id(shield);
    w.tick(&[cmd(Command::Assist {
        units: vec![eid],
        target: sid,
        queue: false,
    })])
    .unwrap();
    let working = (0..40).any(|_| {
        w.tick(&[]).unwrap();
        w.state.units.has_flag(engineer, flag::BUILDING)
    });
    assert!(working, "the engineer never started feeding the dome");

    let demand = w.state.players[0].energy_demand;
    assert!(
        demand > w.bp(shield).economy.energy_upkeep,
        "feeding a dome should ask for extra energy ({demand} vs upkeep {})",
        w.bp(shield).economy.energy_upkeep
    );

    w.state.units.shield_hp[shield] = half;
    w.tick(&[]).unwrap();
    let helped = w.state.units.shield_hp[shield] - half;
    assert!(
        helped > solo,
        "assist should add regen on top of the generator ({helped} vs {solo})"
    );
}

#[test]
fn engineers_cannot_boost_a_recovering_shield() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[
        spawn(&w, 0, "aster_t2_shield", 600, 0),
        spawn(&w, 0, "aster_t1_engineer", 580, 0),
    ])
    .unwrap();
    let shield = unit_of(&w, 0, "aster_t2_shield");
    let engineer = unit_of(&w, 0, "aster_t1_engineer");
    let spec = w.bp(shield).shield.unwrap();
    w.state.units.shield_hp[shield] = Fx::ZERO;
    w.state.units.shield_open[shield] = 0;
    w.state.units.shield_recharge[shield] = 1;
    w.state.units.shield_hp[shield] = spec.health / 10;

    let eid = w.state.units.id(engineer);
    let sid = w.state.units.id(shield);
    w.tick(&[cmd(Command::Assist {
        units: vec![eid],
        target: sid,
        queue: false,
    })])
    .unwrap();
    let start = w.state.units.shield_hp[shield];
    w.tick(&[]).unwrap();
    let gained = w.state.units.shield_hp[shield] - start;
    assert_eq!(
        gained,
        spec.regen * 2 / 10,
        "recovery should be the generator's doubled refill only"
    );
    assert!(
        !w.state.units.has_flag(engineer, flag::BUILDING),
        "an engineer must not work a downed recovery"
    );
    assert_eq!(
        w.state
            .orders
            .front(&w.state.units, engineer)
            .map(|o| o.kind),
        Some(OrderKind::Assist),
        "assist waits through recovery until another order is given"
    );
    assert_eq!(w.state.units.shield_open[shield], 0);
}

#[test]
fn engineers_keep_assisting_a_shield_until_ordered_otherwise() {
    let mut w = world();
    powered(&mut w);
    w.tick(&[
        spawn(&w, 0, "aster_t2_shield", 600, 0),
        spawn(&w, 0, "aster_t1_engineer", 580, 0),
    ])
    .unwrap();
    let shield = unit_of(&w, 0, "aster_t2_shield");
    let engineer = unit_of(&w, 0, "aster_t1_engineer");
    let spec = w.bp(shield).shield.unwrap();
    let eid = w.state.units.id(engineer);
    let sid = w.state.units.id(shield);

    w.tick(&[cmd(Command::Assist {
        units: vec![eid],
        target: sid,
        queue: false,
    })])
    .unwrap();

    for _ in 0..20 {
        w.tick(&[]).unwrap();
        assert_eq!(
            w.state
                .orders
                .front(&w.state.units, engineer)
                .map(|o| o.kind),
            Some(OrderKind::Assist),
            "a full dome must not drop the assist"
        );
        assert!(
            !w.state.units.has_flag(engineer, flag::BUILDING),
            "a full dome should not be worked"
        );
    }

    w.state.units.shield_hp[shield] = spec.health / 2;
    let feeding = (0..40).any(|_| {
        w.tick(&[]).unwrap();
        w.state.units.has_flag(engineer, flag::BUILDING)
    });
    assert!(feeding, "the engineer should resume when the dome is hit");
    assert_eq!(
        w.state
            .orders
            .front(&w.state.units, engineer)
            .map(|o| o.kind),
        Some(OrderKind::Assist),
    );

    w.state.units.shield_hp[shield] = spec.health;
    w.tick(&[]).unwrap();
    assert!(
        !w.state.units.has_flag(engineer, flag::BUILDING),
        "a topped-up dome should idle the beam"
    );
    assert_eq!(
        w.state
            .orders
            .front(&w.state.units, engineer)
            .map(|o| o.kind),
        Some(OrderKind::Assist),
        "assist must survive a full recharge"
    );

    w.tick(&[cmd(Command::Move {
        units: vec![eid],
        target: FxVec2::from_ints(400, 512),
        queue: false,
    })])
    .unwrap();
    assert_eq!(
        w.state
            .orders
            .front(&w.state.units, engineer)
            .map(|o| o.kind),
        Some(OrderKind::Move),
    );
}

#[test]
fn petrel_carpets_a_salvo_of_bombs() {
    let mut w = world();
    let bomber = w.blueprints.id_of("aster_t1_bomber").unwrap();
    let weapon = &w.blueprints.unit(bomber).weapons[0];
    assert_eq!(weapon.salvo, 8);
    assert_eq!(weapon.muzzles.len(), 8);
    assert_eq!(weapon.salvo_batch, 2);
    let spacing = weapon.salvo_delay_ticks as u32;
    assert_eq!(spacing, 1);
    let fighter = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t1_interceptor").unwrap());
    let speed = w.blueprints.unit(bomber).motion.unwrap().speed;
    assert!(speed < fighter.motion.unwrap().speed);
    assert!(speed >= fighter.motion.unwrap().speed * Fx::ratio(9, 10));

    w.tick(&[
        spawn(&w, 0, "aster_t1_bomber", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            545,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();

    let mut fire_ticks = Vec::new();
    let mut drop_positions = Vec::new();
    let mut saw_live_trail = false;
    let mut saw_terminal_trail = false;
    let mut frame = mc_sim::RenderFrame::default();
    let mut record = |w: &World| {
        w.write_render_frame(None, &mut frame);
        for p in &frame.projectiles {
            use mc_sim::mirror::{
                PROJECTILE_BOMB, PROJECTILE_ENDS_SHIFT, PROJECTILE_MISSILE, PROJECTILE_SMOKE,
                PROJECTILE_TRAIL,
            };
            assert_ne!(p.color & PROJECTILE_BOMB, 0, "bomb lost its opaque casing");
            assert_ne!(p.color & PROJECTILE_SMOKE, 0, "bomb lost its white wake");
            assert_eq!(p.color & (PROJECTILE_MISSILE | PROJECTILE_TRAIL), 0);
            assert!((p.wake - 1.2).abs() < 0.001);
            if p.color >> PROJECTILE_ENDS_SHIFT != 0 {
                saw_terminal_trail = true;
            } else {
                saw_live_trail = true;
            }
        }
        for e in &w.events {
            if let SimEvent::ShotFired { blueprint, .. } = e {
                if *blueprint == bomber {
                    let row = unit_row(w, 0, "aster_t1_bomber");
                    let motion = w.blueprints.unit(bomber).motion.unwrap();
                    assert!(w.state.units.speed[row] >= motion.speed * Fx::ratio(3, 4));
                    assert!(w.state.units.pos[row].distance(w.state.units.prev_pos[row]) > Fx::ONE);
                    if let SimEvent::ShotFired { pos, vel, .. } = e {
                        drop_positions.push(FxVec2::new(pos.x, pos.y));
                        assert_eq!(vel.z, Fx::ZERO, "a dropped bomb launched upwards");
                        let inherited = FxVec2::from_angle(w.state.units.heading[row])
                            * (w.state.units.speed[row] / mc_core::TICKS_PER_SECOND as i32);
                        assert!(
                            (FxVec2::new(vel.x, vel.y) - inherited).length() < Fx::ratio(1, 100)
                        );
                    }
                    fire_ticks.push(w.tick_count());
                }
            }
        }
    };
    record(&w);
    // Starting inside release distance requires an outbound leg and a new approach.
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        record(&w);
    }
    assert!(fire_ticks.len() >= 8, "the carpet was {fire_ticks:?}");
    let salvo = &fire_ticks[..8];
    assert!(
        salvo.chunks_exact(2).all(|pair| pair[0] == pair[1]),
        "bombs must drop in simultaneous pairs: {fire_ticks:?}"
    );
    assert!(
        [salvo[0], salvo[2], salvo[4], salvo[6]]
            .windows(2)
            .all(|pair| pair[1] - pair[0] == spacing),
        "the bay lost its paired release cadence: {fire_ticks:?}"
    );
    for pair in drop_positions[..8].chunks_exact(2) {
        let separation = pair[0].distance(pair[1]);
        assert!(
            separation > Fx::ratio(1, 2) && separation < Fx::from_int(3),
            "paired bombs must leave separate left/right racks: {pair:?}"
        );
    }
    assert!(
        saw_live_trail && saw_terminal_trail,
        "white wake must survive through impact"
    );
}

fn unit_row(w: &World, owner: u8, key: &str) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == bp && w.state.units.owner[r] == owner)
        .expect(key)
}

#[test]
fn a_bomber_flies_a_run_instead_of_hovering() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_bomber", 400, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            620,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    let bomber = unit_row(&w, 0, "aster_t1_bomber");
    let tank = unit_row(&w, 1, "aster_t1_tank");
    let tank_x = w.state.units.pos[tank].x;
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(bomber)],
        target: w.state.units.id(tank),
        queue: false,
    })])
    .unwrap();

    let cruise = w
        .blueprints
        .unit(w.state.units.blueprint[bomber])
        .motion
        .unwrap()
        .speed;
    let start = w.state.units.pos[bomber];
    let mut past = false;
    let mut flying = 0;
    for _ in 0..120 {
        w.tick(&[]).unwrap();
        let pos = w.state.units.pos[bomber];
        if w.state.units.speed[bomber] > cruise / 2 {
            flying += 1;
        }
        if pos.x > tank_x {
            past = true;
        }
    }
    assert!(
        past,
        "it never crossed the target ({start:?} -> {:?})",
        w.state.units.pos[bomber]
    );
    assert!(
        flying >= 80,
        "it spent {flying} ticks flying; a bomber should not hover on the target"
    );
    assert!(
        start.distance(w.state.units.pos[bomber]) > Fx::from_int(80),
        "it stayed put over the target"
    );
}

#[test]
fn interceptors_dogfight_instead_of_facing_each_other() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_interceptor", 480, 0),
        spawn(&w, 1, "aster_t1_interceptor", 720, 0),
    ])
    .unwrap();
    let a = unit_row(&w, 0, "aster_t1_interceptor");
    let b = unit_row(&w, 1, "aster_t1_interceptor");
    w.tick(&[
        cmd(Command::Attack {
            units: vec![w.state.units.id(a)],
            target: w.state.units.id(b),
            queue: false,
        }),
        PlayerCommand {
            player: 1,
            command: Command::Attack {
                units: vec![w.state.units.id(b)],
                target: w.state.units.id(a),
                queue: false,
            },
        },
    ])
    .unwrap();

    let cruise = w
        .blueprints
        .unit(w.state.units.blueprint[a])
        .motion
        .unwrap()
        .speed;
    let start_a = w.state.units.pos[a];
    let start_b = w.state.units.pos[b];
    let mid = start_a.lerp(start_b, Fx::HALF);
    let mut flying = 0;
    let mut travelled = [Fx::ZERO; 2];
    let mut farthest = Fx::ZERO;
    let headed = w.state.units.heading[a];
    let mut turned = false;
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        if !w.state.units.slots.is_alive(a) || !w.state.units.slots.is_alive(b) {
            break;
        }
        for (i, row) in [a, b].into_iter().enumerate() {
            travelled[i] += w.state.units.pos[row].distance(w.state.units.prev_pos[row]);
        }
        if w.state.units.speed[a] > cruise / 2 && w.state.units.speed[b] > cruise / 2 {
            flying += 1;
        }
        farthest = farthest
            .max(w.state.units.pos[a].distance(mid))
            .max(w.state.units.pos[b].distance(mid));
        if w.state.units.heading[a].delta_to(headed).unsigned_abs() > 0x2000 {
            turned = true;
        }
        assert_eq!(
            w.state.units.flags[a] & flag::HOLD,
            0,
            "an interceptor hovered to shoot"
        );
        assert_eq!(
            w.state.units.flags[b] & flag::HOLD,
            0,
            "an interceptor hovered to shoot"
        );
    }
    assert!(
        flying >= 40,
        "they only flew {flying} ticks; fighters should stay in the air"
    );
    assert!(
        farthest < Fx::from_int(280),
        "they flew the fight across the map ({farthest:?} from the merge)"
    );
    assert!(turned, "fighter A never banked; they flew a straight slash");
    if w.state.units.slots.is_alive(a) {
        assert!(
            travelled[0] > Fx::from_int(180),
            "fighter A sat still and traded shots"
        );
    }
    if w.state.units.slots.is_alive(b) {
        assert!(
            travelled[1] > Fx::from_int(180),
            "fighter B sat still and traded shots"
        );
    }
}

#[test]
fn idle_aircraft_land_in_place() {
    let mut w = world();
    w.tick(&[spawn(&w, 0, "aster_t1_interceptor", 500, 0)])
        .unwrap();
    let row = unit_row(&w, 0, "aster_t1_interceptor");
    let start = w.state.units.pos[row];
    for _ in 0..300 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.units.z[row],
        w.terrain.height_at(start),
        "idle aircraft never landed"
    );
    assert!(
        w.state.units.speed[row] < Fx::from_int(4),
        "an idle fighter kept flying ({:?})",
        w.state.units.speed[row]
    );
    assert!(
        start.distance(w.state.units.pos[row]) < Fx::from_int(8),
        "an idle fighter wandered ({start:?} -> {:?})",
        w.state.units.pos[row]
    );
}

#[test]
fn a_fighter_cannot_shoot_off_its_nose() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_interceptor", 500, 0),
        cmd(Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of("aster_t1_interceptor").unwrap(),
            pos: FxVec2::from_ints(500, 680),
            heading: Angle::ZERO,
            count: 1,
            flags: flag::PASSIVE | flag::INVULNERABLE,
            build: 1000,
        }),
    ])
    .unwrap();
    let fighter = w.blueprints.id_of("aster_t1_interceptor").unwrap();
    let row = unit_row(&w, 0, "aster_t1_interceptor");
    let target = unit_row(&w, 1, "aster_t1_interceptor");
    let arc = w.blueprints.unit(fighter).weapons[0].half_arc;
    let mut shots = 0;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        let fired = w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == fighter))
            .count();
        if fired != 0 {
            let bearing = (w.state.units.pos[target] - w.state.units.pos[row]).angle();
            assert!(
                w.state.units.heading[row].delta_to(bearing).unsigned_abs() <= arc + 364,
                "fighter fired before turning its nose into the firing arc"
            );
        }
        shots += fired;
    }
    assert!(shots > 0, "idle fighter never turned to engage");
}

#[test]
fn shatter_impacts_report_actual_aircraft_motion_for_fragment_lead() {
    for gun in ["aster_t3_shatter", "aster_t3_mobile_aa"] {
        let mut w = world();
        let gun_bp = w.blueprints.id_of(gun).unwrap();
        w.spawn_unit(gun_bp, 0, FxVec2::from_ints(500, 512), Angle::ZERO, true)
            .unwrap();
        let target_bp = w.blueprints.id_of("aster_t1_rotor_gunship").unwrap();
        let target = w
            .spawn_unit(target_bp, 1, FxVec2::from_ints(650, 512), Angle::ZERO, true)
            .unwrap();
        w.state.units.flags[target] |= flag::PASSIVE | flag::INVULNERABLE;
        let id = w.state.units.id(target);
        w.tick(&[PlayerCommand {
            player: 1,
            command: Command::Move {
                units: vec![id],
                target: FxVec2::from_ints(650, 1000),
                queue: false,
            },
        }])
        .unwrap();
        let mut moving_hits = 0;
        for _ in 0..80 {
            w.tick(&[]).unwrap();
            let u = &w.state.units;
            let measured =
                (u.pos[target] - u.prev_pos[target]).extend(u.z[target] - u.prev_z[target]);
            for event in &w.events {
                if let SimEvent::Impact {
                    blueprint,
                    target_motion,
                    on_unit: true,
                    ..
                } = event
                {
                    if *blueprint == gun_bp {
                        assert_eq!(*target_motion, measured);
                        if target_motion.xy().length() > Fx::ratio(1, 10) {
                            moving_hits += 1;
                        }
                    }
                }
            }
        }
        assert!(
            moving_hits >= 2,
            "{gun} needs impacts against a moving aircraft: {moving_hits}"
        );
    }
}

#[test]
fn shatter_barrel_tracks_close_overhead_and_crossing_aircraft_before_firing() {
    for gun in ["aster_t3_shatter", "aster_t3_mobile_aa"] {
        for offset in [0, 20, 60, 250] {
            let mut w = world();
            let gun_bp = w.blueprints.id_of(gun).unwrap();
            let shooter = w
                .spawn_unit(gun_bp, 0, FxVec2::from_ints(500, 512), Angle::ZERO, true)
                .unwrap();
            let target_bp = w.blueprints.id_of("aster_t1_rotor_gunship").unwrap();
            let target = w
                .spawn_unit(
                    target_bp,
                    1,
                    FxVec2::from_ints(500 + offset, 512),
                    Angle::ZERO,
                    true,
                )
                .unwrap();
            w.state.units.flags[target] |= flag::PASSIVE | flag::INVULNERABLE;
            let mut shots = 0;
            for tick in 0..65 {
                // Switch sides and heights while the weapon is ready to expose
                // shots fired during an unfinished elevation slew.
                if tick == 25 {
                    w.state.units.pos[target] = FxVec2::from_ints(500 + offset, 560);
                    w.state.units.prev_pos[target] = w.state.units.pos[target];
                    w.state.units.z[target] += Fx::from_int(100);
                    w.state.units.prev_z[target] = w.state.units.z[target];
                    w.state.units.weapon_cooldown[shooter][0] = 0;
                }
                w.tick(&[]).unwrap();
                for e in &w.events {
                    if let SimEvent::ShotFired { blueprint, vel, .. } = e {
                        if *blueprint != gun_bp {
                            continue;
                        }
                        let pitch = w.state.units.arm_pitch[shooter][0];
                        let shot_pitch = FxVec2::new(vel.xy().length(), vel.z).angle();
                        assert!(pitch.delta_to(shot_pitch).unsigned_abs() <= Angle::from_degrees(4).0,
                            "{gun} offset {offset}: barrel pitch {pitch:?}, shot pitch {shot_pitch:?}");
                        shots += 1;
                    }
                }
            }
            assert!(shots >= 2, "{gun} failed to engage at offset {offset}");
        }
    }
}

#[test]
fn shatter_blast_damages_wider_air_groups_with_increased_damage() {
    for (gun, damage, offset) in [
        ("aster_t3_shatter", 1500, 54),
        ("aster_t3_mobile_aa", 875, 37),
    ] {
        let mut w = world();
        let gun_bp = w.blueprints.id_of(gun).unwrap();
        let target_bp = w.blueprints.id_of("aster_t1_rotor_gunship").unwrap();
        let center = w
            .spawn_unit(target_bp, 1, FxVec2::from_ints(900, 900), Angle::ZERO, true)
            .unwrap();
        let edge = w
            .spawn_unit(
                target_bp,
                1,
                FxVec2::from_ints(900, 900 + offset),
                Angle::ZERO,
                true,
            )
            .unwrap();
        let outside = w
            .spawn_unit(
                target_bp,
                1,
                FxVec2::from_ints(900, 1000),
                Angle::ZERO,
                true,
            )
            .unwrap();
        let ground_bp = w.blueprints.id_of("aster_t1_tank").unwrap();
        let ground = w
            .spawn_unit(ground_bp, 1, FxVec2::from_ints(900, 900), Angle::ZERO, true)
            .unwrap();
        for row in [center, edge, outside, ground] {
            w.state.units.flags[row] |= flag::PASSIVE;
            w.state.units.health[row] = Fx::from_int(2000);
        }
        let pos = w.state.units.pos[center]
            .extend(w.state.units.z[center] + w.blueprints.unit(target_bp).height / 2);
        w.state
            .projectiles
            .spawn(
                pos,
                mc_core::FxVec3::new(Fx::ONE, Fx::ZERO, Fx::ZERO),
                0,
                mc_sim::Handle::NONE,
                gun_bp,
                0,
                30,
            )
            .unwrap();
        w.tick(&[]).unwrap();
        for row in [center, edge] {
            assert_eq!(
                w.state.units.health[row],
                Fx::from_int(2000 - damage),
                "{gun} target {row}"
            );
        }
        assert_eq!(w.state.units.health[outside], Fx::from_int(2000));
        assert_eq!(w.state.units.health[ground], Fx::from_int(2000));
    }
}

#[test]
fn a_splash_shell_hurts_a_half_built_site() {
    let mut w = world();
    let setup = [
        spawn(&w, 0, "aster_t1_artillery", 500, 0),
        cmd(Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of("aster_t1_power").unwrap(),
            pos: FxVec2::from_ints(700, 512),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 300,
        }),
    ];
    w.tick(&setup).unwrap();
    let site = unit_of(&w, 1, "aster_t1_power");
    let site_id = w.state.units.id(site);
    assert!(w.state.units.has_flag(site, flag::UNDER_CONSTRUCTION));
    let start = w.state.units.health[site];
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        if w.state.units.row(site_id).is_none() {
            return;
        }
    }
    let row = w.state.units.row(site_id).unwrap();
    assert!(
        w.state.units.health[row] < start,
        "artillery never hurt the site"
    );
}
