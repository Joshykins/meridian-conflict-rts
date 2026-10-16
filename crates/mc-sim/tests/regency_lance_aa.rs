//! The Bardiche (`regency_t3_lance_aa`), the Regency's tech 3 lance anti-air: one fast
//! Pinch lance bolt a shot at one aircraft, for the heavy planes. A strategic bomber
//! flying in at it is hit from about 1000 m out, every bolt fired lands, and each bolt
//! strikes only the plane it was fired at.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const LANCE: &str = "regency_t3_lance_aa";
const BOMBER: &str = "aster_t3_strategic_bomber";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        doodads: Default::default(),
        name: "lance".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(7000, 7000)],
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
        seed: 11,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

/// Spawns a bomber of the other side's at `(x, y)`, at its flying height, holding its
/// bombs and flying for `to`.
fn bomber(w: &mut World, x: i32, y: i32, to: FxVec2) -> UnitId {
    let bp = w.blueprints.id_of(BOMBER).unwrap();
    let motion = w.blueprints.unit(bp).motion.unwrap();
    let row = w
        .spawn_unit(bp, 1, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.z[row] = Fx::from_int(20) + motion.altitude;
    w.state.units.speed[row] = motion.speed;
    w.state.units.fire_state[row] = FireState::HoldFire;
    let id = w.state.units.id(row);
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::Move {
            units: vec![id],
            target: to,
            queue: false,
        },
    }])
    .unwrap();
    id
}

fn health(w: &World, id: UnitId) -> Option<Fx> {
    w.state.units.row(id).map(|r| w.state.units.health[r])
}

/// A strategic bomber flying in at a pair of Bardiches facing it, from 1600 m out, is
/// struck from about 1000 m (it comes into reach about 1260 m out over the ground at its
/// height, then the lance charges and the bolt flies), every bolt fired lands on it for
/// the bolt's full damage, and the pair bring it down in the one pass, two bolts each
/// (it goes down about over them: the second pair is a reload behind the first).
#[test]
fn bolts_land_on_a_strategic_bomber_from_long_range() {
    let mut w = world();
    let lance_bp = w.blueprints.id_of(LANCE).unwrap();
    let bolt = w.blueprints.unit(lance_bp).weapons[0].clone();
    assert!(
        !bolt.airburst && !bolt.flak && !bolt.missile && bolt.salvo <= 1 && bolt.cluster.is_none(),
        "one plain bolt a shot"
    );
    for y in [3960, 4040] {
        w.spawn_unit(
            lance_bp,
            0,
            FxVec2::from_ints(4000, y),
            Angle::from_degrees(180),
            true,
        )
        .unwrap();
    }
    let centre = FxVec2::from_ints(4000, 4000);
    let plane = bomber(&mut w, 2400, 4000, FxVec2::from_ints(6400, 4000));
    let start = health(&w, plane).unwrap();
    let (mut fired, mut landed, mut first_hit_at) = (0, 0, None);
    let mut down_at = None;
    for _ in 0..(14 * TICKS_PER_SECOND) {
        let before = health(&w, plane);
        let at = w
            .state
            .units
            .row(plane)
            .map(|r| w.state.units.pos[r].distance(centre).to_f32());
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::ShotFired { blueprint, .. } if *blueprint == lance_bp => fired += 1,
                SimEvent::Impact { on_unit: true, .. } => landed += 1,
                _ => {}
            }
        }
        match (before, health(&w, plane)) {
            (Some(b), Some(a)) if a < b => {
                first_hit_at = first_hit_at.or(at);
                let bolts = ((b - a) / bolt.damage).round_int();
                assert_eq!(b - a, bolt.damage * bolts, "every hit is whole bolts");
            }
            (Some(_), None) => {
                down_at = at;
                break;
            }
            _ => {}
        }
    }
    let first = first_hit_at.expect("no bolt struck the bomber");
    assert!(
        first > 900.0,
        "first struck {first:.0} m out: it should be hit from about 1000 m"
    );
    assert!(
        fired > 0 && landed == fired,
        "{landed} of {fired} bolts landed"
    );
    let down = down_at.unwrap_or_else(|| {
        panic!(
            "the bomber survived {} of {start:?}",
            health(&w, plane).unwrap().to_f32()
        )
    });
    assert!(
        down < 500.0,
        "it came down {down:.0} m out, not on its pass"
    );
}

/// Two bombers flying side by side, a few lengths apart: each bolt strikes one of them,
/// never both.
#[test]
fn each_bolt_strikes_one_plane() {
    let mut w = world();
    let lance_bp = w.blueprints.id_of(LANCE).unwrap();
    let damage = w.blueprints.unit(lance_bp).weapons[0].damage;
    w.spawn_unit(
        lance_bp,
        0,
        FxVec2::from_ints(4000, 4000),
        Angle::ZERO,
        true,
    )
    .unwrap();
    let planes = [
        bomber(&mut w, 2600, 3970, FxVec2::from_ints(6400, 3970)),
        bomber(&mut w, 2600, 4030, FxVec2::from_ints(6400, 4030)),
    ];
    let mut fired = 0;
    let mut hits = 0;
    for _ in 0..(12 * TICKS_PER_SECOND) {
        let before = planes.map(|p| health(&w, p));
        w.tick(&[]).unwrap();
        let shots = w
            .events
            .iter()
            .filter(
                |e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == lance_bp),
            )
            .count();
        assert!(shots <= 1, "{shots} bolts in one tick");
        fired += shots;
        let struck: Vec<Fx> = planes
            .iter()
            .zip(before)
            .filter_map(|(&p, b)| Some(b? - health(&w, p).unwrap_or(Fx::ZERO)))
            .filter(|&d| d > Fx::ZERO)
            .collect();
        assert!(struck.len() <= 1, "one bolt struck {} planes", struck.len());
        if let Some(&d) = struck.first() {
            assert_eq!(d, damage, "a whole bolt on one plane");
            hits += 1;
        }
    }
    assert!(fired >= 2, "only {fired} bolts fired");
    assert_eq!(hits, fired, "{hits} of {fired} bolts struck a plane");
}
