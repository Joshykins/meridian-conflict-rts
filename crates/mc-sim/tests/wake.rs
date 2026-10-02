//! Cone weapons (`Weapon::cone`, `wake.rs`): the Regency Wake's wake strikes every enemy
//! in the fan ahead of its projector on the tick it fires, the nearer harder, and nothing
//! outside the fan or past its reach; ground between the projector and a hull shields it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const CELLS: u32 = 256;
/// The ground stands 20 m up; a wall cell stands 20 m over it.
const GROUND: u16 = 20;
const WALL: u16 = 40;

/// A flat field, with a short wall where `wall` says (cells, inclusive: x range, y range).
fn world(wall: Option<((usize, usize), (usize, usize))>) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let side = CELLS as usize + 1;
    let mut samples = vec![GROUND; side * side];
    if let Some(((x0, x1), (y0, y1))) = wall {
        for y in y0..=y1 {
            for x in x0..=x1 {
                samples[y * side + x] = WALL;
            }
        }
    }
    let terrain = Heightfield::from_samples(CELLS, CELLS, samples, Fx::ZERO, Fx::ONE, Fx::ZERO);
    let map = MapData {
        name: "wake".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
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
        seed: 5,
        players: vec![player("Regency", 0), player("Aster", 1)],
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

fn health(w: &World, unit: UnitId) -> Fx {
    w.state
        .units
        .row(unit)
        .map_or(Fx::ZERO, |r| w.state.units.health[r])
}

/// Ticks the world until the Wake's first wake, and returns what each of `marks` lost
/// on that tick.
fn first_wake(w: &mut World, marks: &[UnitId]) -> Vec<Fx> {
    let breaker = w.blueprints.id_of("regency_t3_wake_tank").unwrap();
    for _ in 0..(12 * mc_core::TICKS_PER_SECOND) {
        let before: Vec<Fx> = marks.iter().map(|&m| health(w, m)).collect();
        w.tick(&[]).unwrap();
        let fired = w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == breaker));
        if fired {
            // Nothing flies: the wake struck on the tick it fired.
            let p = &w.state.projectiles;
            assert!(
                (0..p.len()).all(|i| p.blueprint[i] != breaker),
                "a wake left a projectile"
            );
            return marks
                .iter()
                .zip(before)
                .map(|(&m, hp)| hp - health(w, m))
                .collect();
        }
    }
    panic!("the Wake never fired");
}

#[test]
fn the_wake_strikes_everything_in_its_fan_at_once() {
    let mut w = world(None);
    add(&mut w, "regency_t3_wake_tank", 0, 512, 512, 0);
    // Bulwarks: dead ahead at 88 m, 11 degrees off at 141 m, out of the fan (69 degrees
    // off, in reach) and past its reach (188 m dead ahead).
    let near = add(&mut w, "aster_t2_tank", 1, 600, 512, 180);
    let far = add(&mut w, "aster_t2_tank", 1, 650, 540, 180);
    let beside = add(&mut w, "aster_t2_tank", 1, 560, 640, 180);
    let beyond = add(&mut w, "aster_t2_tank", 1, 700, 512, 180);
    let lost = first_wake(&mut w, &[near, far, beside, beyond]);
    let weapon = w
        .blueprints
        .unit(w.blueprints.id_of("regency_t3_wake_tank").unwrap())
        .weapons[0]
        .clone();
    let cone = weapon.cone.expect("the Wake's projector is a cone");
    assert!(lost[0] > Fx::ZERO, "the near Bulwark was not struck");
    assert!(lost[1] > Fx::ZERO, "the far Bulwark was not struck");
    assert!(
        lost[0] > lost[1],
        "nearer should take more: {} near, {} far",
        lost[0],
        lost[1]
    );
    assert!(lost[0] <= weapon.damage && lost[1] >= weapon.damage * cone.edge);
    assert_eq!(lost[2], Fx::ZERO, "struck outside its fan");
    assert_eq!(lost[3], Fx::ZERO, "struck past its reach");
}

#[test]
fn ground_between_shields_a_unit_from_the_wake() {
    // A 20 m wall across the line to the hidden Bulwark (cells are 8 m): x 584-592, y 520-536.
    let mut w = world(Some(((73, 74), (65, 67))));
    add(&mut w, "regency_t3_wake_tank", 0, 512, 512, 0);
    // In the open, 148 m off a little south of east; behind the wall, 101 m off 13 degrees
    // north of east (17 degrees off the open one's line: inside the fan).
    let open = add(&mut w, "aster_t2_tank", 1, 660, 500, 180);
    let hidden = add(&mut w, "aster_t2_tank", 1, 610, 535, 180);
    let lost = first_wake(&mut w, &[open, hidden]);
    assert!(lost[0] > Fx::ZERO, "the Bulwark in the open was not struck");
    assert_eq!(
        lost[1],
        Fx::ZERO,
        "the wall did not shield the hidden Bulwark"
    );
}
