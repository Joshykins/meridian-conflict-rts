//! Cone weapons (`Weapon::cone`, `wake.rs`): the Regency Wake's wake rolls out over the fan
//! ahead of its projector and strikes every enemy in it as the front arrives, the nearer
//! sooner and harder, and nothing outside the fan or past its reach; ground between the
//! projector and a hull shields it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{Heightfield, Prop, PropKind};
use mc_sim::tables::{Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const CELLS: u32 = 256;
/// The ground stands 20 m up; a wall cell stands 20 m over it.
const GROUND: u16 = 20;
const WALL: u16 = 40;

/// A flat field, with a short wall where `wall` says (cells, inclusive: x range, y range),
/// and trees standing at `trees`.
fn world(wall: Option<((usize, usize), (usize, usize))>, trees: &[(i32, i32)]) -> World {
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
        props: trees
            .iter()
            .map(|&(x, y)| Prop {
                kind: PropKind::TreeConifer,
                pos: FxVec2::from_ints(x, y),
                heading: Angle::ZERO,
                scale_milli: 1000,
                wear_milli: 0,
            })
            .collect(),
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
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // One wake a test: the second would come before the first has rolled out, after the
    // marks have moved.
    let breaker = w.blueprints.id_of("regency_t3_wake_tank").unwrap();
    Arc::make_mut(&mut w.blueprints).units[breaker.index()].weapons[0].reload_ticks = 600;
    w
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

/// What each of `marks` lost to the Wake's first wake, and the tick (counted from the one
/// it fired on) its front reached each; `None` for one it never reached. Each unit is
/// struck once a wake, so the first loss each takes is the first wake's.
fn first_wake(w: &mut World, marks: &[UnitId]) -> Vec<Option<(u32, Fx)>> {
    let breaker = w.blueprints.id_of("regency_t3_wake_tank").unwrap();
    let mut fired = None;
    let mut struck = vec![None; marks.len()];
    for tick in 0..(20 * mc_core::TICKS_PER_SECOND) {
        let before: Vec<Fx> = marks.iter().map(|&m| health(w, m)).collect();
        w.tick(&[]).unwrap();
        if fired.is_none()
            && w.events.iter().any(
                |e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == breaker),
            )
        {
            // Nothing flies but the front.
            let p = &w.state.projectiles;
            assert!(
                (0..p.len()).all(|i| p.blueprint[i] != breaker),
                "a wake left a projectile"
            );
            fired = Some(tick);
        }
        let Some(start) = fired else {
            continue;
        };
        for (i, (&m, hp)) in marks.iter().zip(before).enumerate() {
            let lost = hp - health(w, m);
            if struck[i].is_none() && lost > Fx::ZERO {
                struck[i] = Some((tick - start, lost));
            }
        }
        // The first wake has rolled out to its full reach and gone.
        let weapon = &w.blueprints.unit(breaker).weapons[0];
        let cone = weapon.cone.unwrap();
        let out = weapon.range_max / cone.speed * mc_core::TICKS_PER_SECOND as i32;
        if tick - start > out.ceil_int() as u32 + 1 {
            return struck;
        }
    }
    panic!("the Wake never fired");
}

#[test]
fn the_wake_rolls_out_over_its_fan() {
    let mut w = world(None, &[]);
    add(&mut w, "regency_t3_wake_tank", 0, 512, 512, 0);
    // Bulwarks: dead ahead at 88 m, 5 degrees off at 300 m, out of the fan (30 degrees
    // off, in reach) and past its reach (460 m dead ahead).
    let near = add(&mut w, "aster_t2_tank", 1, 600, 512, 180);
    let far = add(&mut w, "aster_t2_tank", 1, 811, 538, 180);
    let beside = add(&mut w, "aster_t2_tank", 1, 642, 587, 180);
    let beyond = add(&mut w, "aster_t2_tank", 1, 972, 512, 180);
    let struck = first_wake(&mut w, &[near, far, beside, beyond]);
    let weapon = w
        .blueprints
        .unit(w.blueprints.id_of("regency_t3_wake_tank").unwrap())
        .weapons[0]
        .clone();
    let cone = weapon.cone.expect("the Wake's projector is a cone");
    let (near_at, near_lost) = struck[0].expect("the near Bulwark was not struck");
    let (far_at, far_lost) = struck[1].expect("the far Bulwark was not struck");
    assert!(
        near_lost > far_lost,
        "nearer should take more: {near_lost} near, {far_lost} far"
    );
    assert!(near_lost <= weapon.damage && far_lost >= weapon.damage * cone.edge);
    // The front rolls out at its own pace: 212 m further on is nearly two seconds later.
    let gap = (Fx::from_int(212) / cone.speed * mc_core::TICKS_PER_SECOND as i32).floor_int();
    assert!(
        far_at >= near_at + gap as u32 - 2,
        "the far Bulwark was struck at tick {far_at}, the near at {near_at}: the front \
         should take about {gap} ticks between them"
    );
    assert_eq!(struck[2], None, "struck outside its fan");
    assert_eq!(struck[3], None, "struck past its reach");
}

#[test]
fn ground_between_shields_a_unit_from_the_wake() {
    // A 20 m wall beside the line to the open Bulwark (cells are 8 m): x 584-592, y 520-528.
    let mut w = world(Some(((73, 74), (65, 66))), &[]);
    add(&mut w, "regency_t3_wake_tank", 0, 512, 512, 0);
    // In the open, 200 m dead east; behind the wall, 109 m off 6 degrees north of east
    // (inside the fan).
    let open = add(&mut w, "aster_t2_tank", 1, 712, 512, 180);
    let hidden = add(&mut w, "aster_t2_tank", 1, 620, 524, 180);
    let struck = first_wake(&mut w, &[open, hidden]);
    assert!(
        struck[0].is_some(),
        "the Bulwark in the open was not struck"
    );
    assert_eq!(
        struck[1], None,
        "the wall did not shield the hidden Bulwark"
    );
}

#[test]
fn a_half_turn_either_side_rolls_out_all_round() {
    let mut w = world(None, &[]);
    add(&mut w, "regency_t3_wake_tank", 0, 512, 512, 0);
    // The projector opened to a full circle (`angle: 180`).
    let bp = w.blueprints.id_of("regency_t3_wake_tank").unwrap();
    let cone = Arc::make_mut(&mut w.blueprints).units[bp.index()].weapons[0]
        .cone
        .as_mut()
        .unwrap();
    cone.half = Angle(32768);
    // Bulwarks ahead, off to one side and behind it.
    let ahead = add(&mut w, "aster_t2_tank", 1, 662, 512, 180);
    let beside = add(&mut w, "aster_t2_tank", 1, 512, 632, 180);
    let behind = add(&mut w, "aster_t2_tank", 1, 402, 512, 180);
    let struck = first_wake(&mut w, &[ahead, beside, behind]);
    assert!(
        struck.iter().all(Option::is_some),
        "a full circle missed some: {struck:?}"
    );
}

#[test]
fn the_trees_it_rolls_over_burn_down() {
    // Trees 150 m ahead in the fan, and 150 m off to one side, out of it.
    let mut w = world(None, &[(662, 520), (512, 662)]);
    add(&mut w, "regency_t3_wake_tank", 0, 512, 512, 0);
    let mark = add(&mut w, "aster_t2_tank", 1, 712, 512, 180);
    first_wake(&mut w, &[mark]);
    assert!(!w.is_prop_alive(0), "the tree in the fan still stands");
    assert!(w.is_prop_alive(1), "the tree out of the fan burned");
}
