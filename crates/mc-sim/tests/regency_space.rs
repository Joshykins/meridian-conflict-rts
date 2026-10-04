//! Fleet integration: builder access, real cargo boarding and combat at cruise altitude.
use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, Handle, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let bp = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let player = |team| PlayerSetup {
        name: format!("side {team}"),
        faction: if team == 0 { "Regency" } else { "Aster" }.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 11,
        players: vec![player(0), player(1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let map = MapData {
        name: "fleet range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
        props: Vec::new(),
    };
    World::with_terrain(
        Heightfield::flat(256, 256, Fx::from_int(20)),
        map,
        bp,
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> usize {
    w.spawn_unit(
        w.blueprints.id_of(key).unwrap(),
        owner,
        FxVec2::from_ints(x, y),
        Angle::ZERO,
        true,
    )
    .unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}
fn ticks(w: &mut World, count: usize) {
    for _ in 0..count {
        w.tick(&[]).unwrap();
    }
}
fn until(w: &mut World, count: usize, done: impl Fn(&World) -> bool) -> bool {
    for _ in 0..count {
        if done(w) {
            return true;
        }
        w.tick(&[]).unwrap();
    }
    done(w)
}

#[test]
fn engineers_can_build_the_fleet_on_site_and_cargo_sizes_fit_their_roles() {
    let w = world();
    let bp = &w.blueprints;
    for (key, builder) in [
        ("regency_t2_transport", "regency_t2_engineer"),
        ("regency_t2_space_frigate", "regency_t2_engineer"),
        ("regency_t3_assault_transport", "regency_t3_engineer"),
        ("regency_t3_space_cruiser", "regency_t3_engineer"),
        ("regency_t3_space_destroyer", "regency_t3_engineer"),
    ] {
        let id = bp.id_of(key).unwrap();
        let u = bp.unit(id);
        assert!((u.has(cat::SPACE) && u.has(cat::AIR)) && u.is_site_built_unit());
        assert!(bp
            .unit(bp.id_of(builder).unwrap())
            .builder
            .as_ref()
            .unwrap()
            .builds
            .contains(&id));
        assert!(!u.lore.is_empty(), "{key}: codex text");
    }
    let commander = bp.unit(bp.id_of("regency_commander").unwrap());
    for key in ["regency_t2_transport", "regency_t3_assault_transport"] {
        let t = bp.unit(bp.id_of(key).unwrap()).transport.unwrap();
        assert!(commander.radius * 2 <= t.width && commander.height <= t.clearance);
        assert!(t.capacity >= commander.cargo_room().unwrap());
    }
    let ark = bp
        .unit(bp.id_of("regency_t3_assault_transport").unwrap())
        .transport
        .unwrap();
    for u in bp
        .units
        .iter()
        .filter(|u| u.faction == commander.faction && u.tech <= 3 && u.cargo_room().is_some())
    {
        assert!(
            u.radius * 2 <= ark.width && u.height <= ark.clearance,
            "Ark cannot fit {}",
            u.key
        );
    }
    assert_eq!(
        bp.unit(bp.id_of("regency_t2_space_frigate").unwrap())
            .weapons
            .len(),
        2
    );
    assert_eq!(
        bp.unit(bp.id_of("regency_t3_space_cruiser").unwrap())
            .weapons
            .len(),
        8
    );
}

#[test]
fn coffer_boards_flies_and_unloads_a_commander_as_its_full_load() {
    let mut w = world();
    let ship = add(&mut w, "regency_t2_transport", 0, 1000, 1000);
    let sid = w.state.units.id(ship);
    let commander = add(&mut w, "regency_commander", 0, 850, 1000);
    let cid = w.state.units.id(commander);
    w.tick(&[cmd(Command::Board {
        units: vec![cid],
        carrier: sid,
        queue: false,
    })])
    .unwrap();
    assert!(until(&mut w, 2000, |w| w.state.units.hangar[commander] == sid));
    assert_eq!(w.cargo_used(ship), 8);
    let tank = add(&mut w, "regency_t1_tank", 0, 850, 1020);
    let tid = w.state.units.id(tank);
    w.tick(&[cmd(Command::Board {
        units: vec![tid],
        carrier: sid,
        queue: false,
    })])
    .unwrap();
    ticks(&mut w, 100);
    assert_eq!(w.state.units.hangar[tank], Handle::NONE);
    w.tick(&[cmd(Command::Land {
        units: vec![sid],
        pos: FxVec2::from_ints(1550, 1000),
        unload: true,
        queue: false,
    })])
    .unwrap();
    assert!(until(&mut w, 2500, |w| w.state.units.hangar[commander]
        == Handle::NONE));
    ticks(&mut w, 180);
    assert!(!w.state.units.has_flag(commander, flag::IN_FACTORY));
    assert!(w.state.units.pos[commander].x > Fx::from_int(1300));
}

#[test]
fn ark_carries_a_commander_and_a_heavy_ground_unit_together() {
    let mut w = world();
    let ship = add(&mut w, "regency_t3_assault_transport", 0, 1000, 1000);
    let sid = w.state.units.id(ship);
    let commander = add(&mut w, "regency_commander", 0, 780, 1000);
    let tank = add(&mut w, "regency_t3_strider", 0, 700, 1000);
    let ids = vec![w.state.units.id(commander), w.state.units.id(tank)];
    w.tick(&[cmd(Command::Board {
        units: ids,
        carrier: sid,
        queue: false,
    })])
    .unwrap();
    assert!(until(&mut w, 2500, |w| w.state.units.hangar[commander]
        == sid
        && w.state.units.hangar[tank] == sid));
    assert!(w.cargo_used(ship) > 8 && w.cargo_used(ship) <= 96);
    w.tick(&[cmd(Command::Land {
        units: vec![sid],
        pos: FxVec2::from_ints(1550, 1000),
        unload: true,
        queue: false,
    })])
    .unwrap();
    assert!(until(&mut w, 2500, |w| w.cargo_used(ship) == 0));
}

#[test]
fn frigate_and_cruiser_fire_their_independent_casemates_at_cruise_height() {
    for (key, enemy, expected) in [
        ("regency_t2_space_frigate", "regency_t2_space_frigate", 2),
        ("regency_t3_space_cruiser", "regency_t2_space_frigate", 8),
    ] {
        let mut w = world();
        let ship = add(&mut w, key, 0, 500, 800);
        let target = add(&mut w, enemy, 1, 1000, 800);
        w.state.units.flags[target] |= flag::PASSIVE;
        w.state.units.health[target] = Fx::from_int(500000);
        let health = w.state.units.health[target];
        let id = w.state.units.id(ship);
        let target_id = w.state.units.id(target);
        let blueprint = w.state.units.blueprint[ship];
        w.tick(&[cmd(Command::Attack {
            units: vec![id],
            target: target_id,
            queue: false,
        })])
        .unwrap();
        let mut fired = std::collections::BTreeSet::new();
        for _ in 0..200 {
            w.tick(&[]).unwrap();
            for event in &w.events {
                if let SimEvent::ShotFired {
                    blueprint: b,
                    weapon,
                    ..
                } = event
                {
                    if *b == blueprint {
                        fired.insert(*weapon);
                    }
                }
            }
        }
        assert_eq!(fired.len(), expected, "{key}: fired slots {fired:?}");
        assert!(
            w.state.units.health[target] < health,
            "{key}: no anti-ship damage"
        );
    }
}

#[test]
fn destroyer_holds_station_and_sweeps_a_wide_sustained_fusion_beam_over_the_ground() {
    let mut w = world();
    let ship = add(&mut w, "regency_t3_space_destroyer", 0, 500, 700);
    let target = add(&mut w, "aster_t4_assault_tank", 1, 1200, 700);
    w.state.units.flags[target] |= flag::PASSIVE;
    w.state.units.health[target] = Fx::from_int(500000);
    let id = w.state.units.id(ship);
    let tid = w.state.units.id(target);
    let blueprint = w.state.units.blueprint[ship];
    let beam = &w.blueprints.unit(blueprint).weapons[0];
    assert!(beam.beam && beam.hitscan && beam.walk >= Fx::from_int(40));
    w.tick(&[cmd(Command::Attack {
        units: vec![id],
        target: tid,
        queue: false,
    })])
    .unwrap();
    ticks(&mut w, 120);
    let station = w.state.units.pos[ship];
    let mut across = (Fx::from_int(10000), Fx::from_int(-10000));
    let mut shots = 0;
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        for event in &w.events {
            match event {
                SimEvent::ShotFired { blueprint: b, .. } if *b == blueprint => shots += 1,
                SimEvent::Impact { pos, .. } => {
                    across.0 = across.0.min(pos.y);
                    across.1 = across.1.max(pos.y);
                }
                _ => {}
            }
        }
    }
    assert!(shots > 40, "sustained stream: {shots} shots");
    assert!(
        across.1 - across.0 > Fx::from_int(40),
        "sweep width: {across:?}"
    );
    assert!(
        w.state.units.pos[ship].distance(station) < Fx::from_int(8),
        "ship did not hold station"
    );
}
