//! The Behemoth in the sea: a walker 480 m tall wades straight across any strait on the
//! maps (the deepest sea on them is about 80 m) and keeps its guns in the fight while it
//! does, since its arms and shoulders are far above the water.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const TITAN: &str = "aster_t5_titan";
const TANK: &str = "aster_t1_tank";
const FRIGATE: &str = "aster_t1_frigate";
/// The surface, and the ground either side of the strait 20 m above it.
const WATER: i32 = 100;
/// How far under the surface the strait's bed lies: as deep as the deepest sea on the maps.
const DEPTH: i32 = 82;

/// Land either side of a strait 2 km across, from x = 1200 m to x = 3200 m.
fn strait() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let cells = 512u32;
    let stride = cells as usize + 1;
    let mut samples = vec![(WATER + 20) as u16; stride * stride];
    for row in samples.chunks_exact_mut(stride) {
        for s in &mut row[150..=400] {
            *s = (WATER - DEPTH) as u16;
        }
    }
    let terrain = Heightfield::from_samples(
        cells,
        cells,
        samples,
        Fx::ZERO,
        Fx::ONE,
        Fx::from_int(WATER),
    );
    let map = MapData {
        name: "strait".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(3800, 3800)],
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
        seed: 11,
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

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).unwrap()
}

/// A target that holds its fire and does not die, so the titan always has one.
fn dummy(w: &mut World, key: &str, x: i32, y: i32) -> UnitId {
    let id = add(w, key, 1, x, y, 180);
    let r = row(w, id);
    w.state.units.fire_state[r] = FireState::HoldFire;
    w.state.units.health[r] = Fx::from_int(1_000_000_000);
    id
}

fn wading(w: &World, id: UnitId) -> bool {
    let r = row(w, id);
    w.terrain.height_at(w.state.units.pos[r]) < w.terrain.water_level()
}

fn move_to(w: &mut World, id: UnitId, goal: FxVec2) {
    let cmd = PlayerCommand {
        player: 0,
        command: Command::Move {
            units: vec![id],
            target: goal,
            queue: false,
        },
    };
    w.tick(&[cmd]).unwrap();
}

#[test]
fn the_titan_wades_across_the_deepest_sea_and_fights_the_whole_way() {
    let mut w = strait();
    let titan = add(&mut w, TITAN, 0, 600, 2048, 0);
    // Ships in the strait and tanks on the far shore, always in reach of something.
    for y in [1700, 2048, 2400] {
        dummy(&mut w, FRIGATE, 2600, y);
        dummy(&mut w, TANK, 3700, y);
    }
    let goal = FxVec2::from_ints(3700, 2048);
    move_to(&mut w, titan, goal);
    let bp = w.blueprints.id_of(TITAN).unwrap();
    let (mut in_water, mut deepest, mut shots) = (0usize, 0.0f32, [0usize; 8]);
    let mut arrived = false;
    for _ in 0..TICKS_PER_SECOND as usize * 300 {
        w.tick(&[]).unwrap();
        let p = w.state.units.pos[row(&w, titan)];
        if wading(&w, titan) {
            in_water += 1;
            deepest = deepest.max((w.terrain.water_level() - w.terrain.height_at(p)).to_f32());
            for e in &w.events {
                if let SimEvent::ShotFired {
                    blueprint, weapon, ..
                } = e
                {
                    if *blueprint == bp {
                        shots[*weapon as usize] += 1;
                    }
                }
            }
        }
        if p.distance(goal).to_f32() < 150.0 {
            arrived = true;
            break;
        }
    }
    let at = w.state.units.pos[row(&w, titan)];
    assert!(
        arrived,
        "stopped at {:?}, never crossed the strait",
        at.to_f32()
    );
    assert!(deepest >= DEPTH as f32 - 1.0, "only waded {deepest} m deep");
    // Two kilometres of sea at 26 m/s: over a minute in the water, with the pods
    // (weapon 0, 24 rockets every 14 s) and the Tempest (weapon 1, two shells a second
    // at full spin) firing close to flat out the whole way.
    let seconds = in_water as f32 / TICKS_PER_SECOND as f32;
    assert!(seconds > 60.0, "only {seconds} s in the water");
    let rate = |w: usize| shots[w] as f32 / seconds;
    assert!(
        rate(0) > 1.2,
        "the pods fired {:.2}/s in the water",
        rate(0)
    );
    assert!(
        rate(1) > 1.2,
        "the Tempest fired {:.2}/s in the water",
        rate(1)
    );
}

/// A move order out to sea puts the titan out at sea, not back on the beach: the
/// destination is not held to ground the nav grid calls passable for land units.
#[test]
fn a_move_out_to_sea_ends_in_the_sea() {
    let mut w = strait();
    let titan = add(&mut w, TITAN, 0, 600, 2048, 0);
    let goal = FxVec2::from_ints(2200, 2048);
    move_to(&mut w, titan, goal);
    for _ in 0..TICKS_PER_SECOND as usize * 120 {
        w.tick(&[]).unwrap();
    }
    let at = w.state.units.pos[row(&w, titan)];
    assert!(wading(&w, titan), "stood on the shore at {:?}", at.to_f32());
    let radius = w.blueprints.unit(w.blueprints.id_of(TITAN).unwrap()).radius;
    assert!(
        at.distance(goal) <= radius,
        "stopped at {:?}, {} m short of the goal",
        at.to_f32(),
        at.distance(goal).to_f32()
    );
}
