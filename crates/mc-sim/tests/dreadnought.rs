//! The Dominion dreadnought (`aster_t4_dreadnought`): its spinal AEB is fixed along the
//! keel like the frigate's rail, so the whole hull lays it, and its discharge leaves a
//! lightning storm on the mark; its Arc Cannon batteries and bolt rifles fight on their
//! own; its SAM cells reach aircraft far off; a hull field takes fire before the plates.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const DREADNOUGHT: &str = "aster_t4_dreadnought";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(1024, 1024, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(7000, 7000)],
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

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32, heading: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(
            id,
            owner,
            FxVec2::from_ints(x, y),
            Angle::from_degrees(heading),
            true,
        )
        .unwrap();
    w.state.units.id(row)
}

fn health(w: &World, id: UnitId) -> f32 {
    w.state
        .units
        .row(id)
        .map_or(0.0, |r| w.state.units.health[r].to_f32())
}

fn hold_fire(w: &mut World, id: UnitId) {
    let row = w.state.units.row(id).unwrap();
    w.state.units.fire_state[row] = FireState::HoldFire;
}

fn heading_of(w: &World, id: UnitId) -> f32 {
    let row = w.state.units.row(id).unwrap();
    w.state.units.heading[row].0 as f32 * 360.0 / 65536.0
}

fn seconds(s: u32) -> usize {
    (s * TICKS_PER_SECOND) as usize
}

/// Lets a freshly spawned ship climb to its cruise height first.
fn settle(w: &mut World) {
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
    }
}

/// Which of the ship's weapons fired this tick.
fn fired(w: &World, ship: UnitId) -> Vec<u8> {
    let bp = w.state.units.row(ship).map(|r| w.state.units.blueprint[r]);
    w.events
        .iter()
        .filter_map(|e| match e {
            SimEvent::ShotFired {
                blueprint, weapon, ..
            } if Some(*blueprint) == bp => Some(*weapon),
            _ => None,
        })
        .collect()
}

#[test]
fn the_spinal_aeb_turns_the_whole_ship_and_leaves_a_storm() {
    let mut w = world();
    w.state.players[0].free_build = true;
    // Nose east; a block of factories due north, inside the spinal's reach.
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    settle(&mut w);
    let factory = "aster_t3_land_factory";
    let mut base = Vec::new();
    for dx in [-120, 0, 120] {
        base.push(add(&mut w, factory, 1, 3000 + dx, 4900, 0));
    }
    let mut discharged = false;
    let mut storm = false;
    for _ in 0..seconds(90) {
        w.tick(&[]).unwrap();
        discharged |= w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::BoreDischarge { weapon, .. } if *weapon == 0));
        storm |= !w.state.storms.is_empty();
        if discharged && storm {
            break;
        }
    }
    let heading = heading_of(&w, ship);
    assert!(discharged, "the spinal AEB never fired (heading {heading})");
    assert!(storm, "the spinal AEB raised no storm");
    assert!(
        (heading - 90.0).abs() < 6.0,
        "the hull did not lay onto the mark: {heading}"
    );
}

#[test]
fn its_casemates_and_bolt_rifles_fight_on_their_own() {
    let mut w = world();
    // Nose east; tanks off the port beam, 1.2 km out: the batteries and the port rifles
    // reach them, and the spinal is laid across the hull.
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    settle(&mut w);
    let tank = "aster_t4_assault_tank";
    let marks: Vec<_> = [-200, 0, 200]
        .iter()
        .map(|dx| add(&mut w, tank, 1, 3000 + dx, 4200, 0))
        .collect();
    for &m in &marks {
        hold_fire(&mut w, m);
    }
    let mut seen = [false; 8];
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
        for wi in fired(&w, ship) {
            seen[wi as usize] = true;
        }
    }
    // The marks are off the port beam: the port casemates bear on them while the hull
    // comes round for the spinal (after that they are dead ahead, the rifles' and the
    // spinal's), and the starboard ones never do.
    for (wi, name) in [(1, "fore port casemate"), (3, "aft port casemate")] {
        assert!(seen[wi], "the {name} never fired: {seen:?}");
    }
    assert!(
        !seen[2] && !seen[4],
        "a starboard casemate fired across the hull: {seen:?}"
    );
    assert!(seen[5], "the fore rifles never fired: {seen:?}");
}

#[test]
fn its_sam_cells_reach_aircraft_far_off() {
    let mut w = world();
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    settle(&mut w);
    // A gunship loitering 2.6 km off: past every gun on the ship but inside the cells'.
    let gunship = add(&mut w, "aster_t2_gunship", 1, 5600, 3000, 180);
    hold_fire(&mut w, gunship);
    let full = health(&w, gunship);
    let mut cells = false;
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
        cells |= fired(&w, ship).iter().any(|&wi| wi == 7);
    }
    assert!(cells, "the SAM cells never fired");
    assert!(
        health(&w, gunship) < full,
        "the SAMs never reached the gunship"
    );
}

#[test]
fn its_hull_field_takes_fire_before_the_plates() {
    let mut w = world();
    // Its upkeep is paid: an unpowered field is down.
    w.state.players[0].free_build = true;
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    hold_fire(&mut w, ship);
    settle(&mut w);
    let row = w.state.units.row(ship).unwrap();
    let shield = w.state.units.shield_hp[row];
    assert!(shield > Fx::ZERO, "the ship has no field up");
    // A Zenith under it: its rail is built for warships.
    let _gun = add(&mut w, "aster_t4_anti_ship", 1, 4200, 3000, 180);
    let full = health(&w, ship);
    let mut lowest = shield;
    for _ in 0..seconds(25) {
        w.tick(&[]).unwrap();
        let row = w.state.units.row(ship).unwrap();
        lowest = lowest.min(w.state.units.shield_hp[row]);
    }
    assert!(lowest < shield, "the field took nothing");
    assert!(
        (health(&w, ship) - full).abs() < 1.0,
        "the plates were hit while the field held"
    );
}

#[test]
fn the_zenith_still_outranges_its_spinal() {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(DREADNOUGHT).unwrap());
    let zenith = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t4_anti_ship").unwrap());
    assert!(bp.weapons[0].range_max < zenith.weapons[0].range_max);
}

#[test]
fn attack_ground_lays_the_spinal_on_the_point() {
    let mut w = world();
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    settle(&mut w);
    let cmd = PlayerCommand {
        player: 0,
        command: Command::AttackGround {
            units: vec![ship],
            pos: FxVec2::from_ints(3000, 1000),
            queue: false,
        },
    };
    w.tick(&[cmd]).unwrap();
    let mut discharged = false;
    for _ in 0..seconds(90) {
        w.tick(&[]).unwrap();
        discharged |= w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::BoreDischarge { weapon, .. } if *weapon == 0));
        if discharged {
            break;
        }
    }
    assert!(discharged, "the spinal never fired on the ground");
    let heading = heading_of(&w, ship);
    assert!(
        (heading - 270.0).abs() < 6.0,
        "the hull did not lay onto the point: {heading}"
    );
}
