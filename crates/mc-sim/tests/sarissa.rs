//! The Sarissa (`regency_t4_anti_ship`), the Regency's anti-ship lance: it charges, then
//! holds a lance of fusion on a warship for a four-second burn, and shoots at nothing else.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const SARISSA: &str = "regency_t4_anti_ship";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "regency".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(4000, 4000)],
        props: Vec::new(),
    };
    let player = |faction: &str, team| PlayerSetup {
        name: faction.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 7,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

/// Spawns `key`; the other side's units hold their fire, so only the Regency guns shoot.
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
    if owner != 0 {
        w.state.units.fire_state[row] = FireState::HoldFire;
    }
    w.state.units.id(row)
}

fn health(w: &World, id: UnitId) -> f32 {
    w.state
        .units
        .row(id)
        .map_or(0.0, |r| w.state.units.health[r].to_f32())
}

fn shots(w: &World, key: &str) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.events
        .iter()
        .filter(|e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == bp))
        .count()
}

#[test]
fn the_sarissa_burns_a_frigate_down_and_leaves_the_rest_alone() {
    let mut w = world();
    let _gun = add(&mut w, SARISSA, 0, 1000, 1000, 0);
    // A tank and a gunship close by, which it cannot touch, and a frigate 2.5 km off.
    let tank = add(&mut w, "aster_t1_tank", 1, 1200, 1000, 180);
    let gunship = add(&mut w, "aster_t1_rotor_gunship", 1, 1000, 1250, 180);
    let frigate = add(&mut w, "aster_t4_frigate", 1, 3500, 1000, 180);
    let (tank_hp, gunship_hp) = (health(&w, tank), health(&w, gunship));
    // Each tick of the burn is a shot; the first burn's ticks and when it starts.
    let (mut first, mut last, mut ticks) = (None, 0, 0);
    let mut dead = None;
    for t in 0..(30 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        let n = shots(&w, SARISSA);
        if n > 0 {
            first.get_or_insert(t);
            if first.is_some_and(|f| t - f < 6 * TICKS_PER_SECOND) {
                ticks += n;
                last = t;
            }
        }
        if dead.is_none() && health(&w, frigate) <= 0.0 {
            dead = Some(t);
        }
    }
    let first = first.expect("the Sarissa never fired");
    assert!(
        first >= 3 * TICKS_PER_SECOND,
        "it fired at tick {first}, before its charge was done"
    );
    assert!(ticks >= 30, "its first burn held for {ticks} ticks");
    assert!(
        last - first <= 5 * TICKS_PER_SECOND,
        "its first burn ran {} ticks",
        last - first
    );
    let dead = dead.expect("the frigate outlived 30 s under the lance");
    assert!(
        dead <= first + 5 * TICKS_PER_SECOND,
        "the frigate took {} ticks to fall, past one burn",
        dead - first
    );
    assert_eq!(health(&w, tank), tank_hp, "it fired on a tank");
    assert_eq!(health(&w, gunship), gunship_hp, "it fired on an aircraft");
}

/// One burn every 11 s: the reload runs from the first tick of a burn to the next.
#[test]
fn the_sarissa_burns_again_eleven_seconds_on() {
    let mut w = world();
    let _gun = add(&mut w, SARISSA, 0, 1000, 1000, 0);
    // Two frigates side by side: the second is there when the first falls.
    add(&mut w, "aster_t4_frigate", 1, 3500, 700, 180);
    add(&mut w, "aster_t4_frigate", 1, 3500, 1300, 180);
    let (mut starts, mut quiet) = (Vec::new(), u32::MAX);
    for t in 0..(40 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        if shots(&w, SARISSA) > 0 {
            if quiet > TICKS_PER_SECOND {
                starts.push(t);
            }
            quiet = 0;
        } else {
            quiet = quiet.saturating_add(1);
        }
    }
    assert!(starts.len() >= 2, "it burned {} times", starts.len());
    let gap = (starts[1] - starts[0]) as f32 / TICKS_PER_SECOND as f32;
    assert!(
        (10.0..=12.5).contains(&gap),
        "{gap} s from one burn to the next"
    );
}
