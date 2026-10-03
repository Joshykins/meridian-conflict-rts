//! The Narwhal (`aster_t3_rail_trimaran`): a Zenith rail on a gun house along the keel of
//! a trimaran. The house trains only a few degrees, so the hull turns to aim and the
//! barrel elevates in its cradle; it shoots spaceships and nothing else.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const NARWHAL: &str = "aster_t3_rail_trimaran";
const FRIGATE: &str = "aster_t4_frigate";

/// Open sea, 30 m deep, about 4 km square.
fn sea() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::from_samples(
        512,
        512,
        vec![0u16; 513 * 513],
        Fx::ZERO,
        Fx::ONE,
        Fx::from_int(30),
    );
    let map = MapData {
        name: "sea".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(3700, 3700)],
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
        seed: 5,
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
    let id = w.state.units.id(row);
    // Targets hold their fire, so only the Narwhal's gun is in play.
    if key != NARWHAL {
        w.state.units.fire_state[row] = FireState::HoldFire;
    }
    id
}

fn health(w: &World, id: UnitId) -> f32 {
    w.state
        .units
        .row(id)
        .map_or(0.0, |r| w.state.units.health[r].to_f32())
}

fn seconds(s: u32) -> usize {
    (s * TICKS_PER_SECOND) as usize
}

fn heading(w: &World, id: UnitId) -> f32 {
    let row = w.state.units.row(id).unwrap();
    w.state.units.heading[row].0 as f32 * 360.0 / 65536.0
}

/// The rail house's elevation (weapon 0's house, `arm_pitch` slot 2), degrees.
fn elevation(w: &World, id: UnitId) -> f32 {
    let row = w.state.units.row(id).unwrap();
    Angle::ZERO.delta_to(w.state.units.arm_pitch[row][2]) as f32 * 360.0 / 65536.0
}

fn shots(w: &World, key: &str) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.events
        .iter()
        .filter(|e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == bp))
        .count()
}

#[test]
fn it_turns_its_hull_onto_a_warship_and_brings_it_down() {
    let mut w = sea();
    // Nose east; the frigate is due north, 2.2 km out, far past any other ship's gun.
    let ship = add(&mut w, NARWHAL, 0, 1000, 800, 0);
    let frigate = add(&mut w, FRIGATE, 1, 1000, 3000, 180);
    let full = health(&w, frigate);
    let mut hit_at = None;
    for t in 0..seconds(60) {
        w.tick(&[]).unwrap();
        if health(&w, frigate) <= full - 5000.0 {
            hit_at = Some(t);
            break;
        }
    }
    let heading = heading(&w, ship);
    assert!(
        hit_at.is_some(),
        "the rail never landed (heading {heading}, elevation {})",
        elevation(&w, ship)
    );
    assert!(
        (heading - 90.0).abs() < 6.0,
        "the hull did not come round onto the mark: {heading}"
    );
}

#[test]
fn the_barrel_elevates_steeply_for_a_warship_close_overhead() {
    let mut w = sea();
    let ship = add(&mut w, NARWHAL, 0, 1000, 1000, 0);
    // Off the bow, only a few hundred metres out: the frigate cruises high over it.
    let _frigate = add(&mut w, FRIGATE, 1, 1450, 1000, 180);
    let mut fired = 0;
    let mut steepest: f32 = 0.0;
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
        fired += shots(&w, NARWHAL);
        steepest = steepest.max(elevation(&w, ship));
    }
    assert!(
        steepest > 20.0,
        "the barrel only rose to {steepest} degrees"
    );
    assert!(fired > 0, "it never fired on the frigate overhead");
}

#[test]
fn it_leaves_ships_and_land_units_alone() {
    let mut w = sea();
    let _ship = add(&mut w, NARWHAL, 0, 1000, 1000, 0);
    let _destroyer = add(&mut w, "aster_t2_destroyer", 1, 1600, 1000, 180);
    let _battleship = add(&mut w, "aster_t3_battleship", 1, 1000, 2400, 180);
    let mut fired = 0;
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
        fired += shots(&w, NARWHAL);
    }
    assert_eq!(
        fired, 0,
        "the rail fired on something that is not a spaceship"
    );
}

/// One Narwhal (owner 1) against one warship `ship` (owner 0, fighting back, field
/// powered) `range` metres off, parked there or ordered to attack the Narwhal. Returns
/// the seconds until one of them died (or the cap) and both health fractions then.
fn duel(ship: &str, range: i32, attack: bool) -> (u32, f32, f32) {
    let mut w = sea();
    w.state.players[0].free_build = true;
    let s = add(&mut w, ship, 0, 600, 2000, 0);
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
    }
    let row = w.state.units.row(s).unwrap();
    w.state.units.fire_state[row] = FireState::FireAtWill;
    let g = add(&mut w, NARWHAL, 1, 600 + range, 2000, 180);
    if attack {
        let cmd = mc_sim::PlayerCommand {
            player: 0,
            command: mc_sim::Command::Attack {
                units: vec![s],
                target: g,
                queue: false,
            },
        };
        w.tick(&[cmd]).unwrap();
    }
    let (ship_full, gun_full) = (health(&w, s), health(&w, g));
    for t in 0..seconds(240) {
        w.tick(&[]).unwrap();
        if w.state.units.row(s).is_none() || w.state.units.row(g).is_none() {
            return (
                t as u32 / TICKS_PER_SECOND,
                health(&w, s) / ship_full,
                health(&w, g) / gun_full,
            );
        }
    }
    (240, health(&w, s) / ship_full, health(&w, g) / gun_full)
}

/// One Narwhal costs under half a Resolute and brings it down however the
/// frigate comes at it; it is no match for a Dominion on its own.
#[test]
fn one_narwhal_brings_down_a_resolute_but_not_a_dominion() {
    for (range, attack) in [(1500, false), (2300, false), (3200, true)] {
        let (t, ship, gun) = duel(FRIGATE, range, attack);
        assert!(
            ship == 0.0 && gun > 0.4,
            "Resolute at {range} m (attack {attack}) after {t} s: ship {ship:.2}, Narwhal {gun:.2}"
        );
    }
    let (t, ship, gun) = duel("aster_t4_dreadnought", 2300, false);
    assert!(
        gun == 0.0 && ship > 0.5,
        "Dominion after {t} s: ship {ship:.2}, Narwhal {gun:.2}"
    );
}
