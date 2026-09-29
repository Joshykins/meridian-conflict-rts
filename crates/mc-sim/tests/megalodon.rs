//! The Megalodon, the tier 4 experimental submarine (docs/NAVY.md): built on the water,
//! AEB strike missiles whose volley spreads over as many marks unless it is ordered onto
//! one, deck rails that only work surfaced, and warheads of its own.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::{snap_to_build_grid, MapData};
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

const MEGALODON: &str = "aster_t4_submarine";
/// Weapon slots in naval.ron.
const STRIKE: u8 = 1;
const RAILS: [u8; 2] = [3, 4];

/// 45 m of sea over a flat bed, land west of x 320 m.
fn sea() -> World {
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..40 {
            samples[y * 257 + x] = 60;
        }
    }
    let terrain = Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(45));
    let map = MapData {
        name: "megalodon".into(),
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
        seed: 5,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32, flags: u16) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let at = snap_to_build_grid(w.blueprints.unit(bp), FxVec2::from_ints(x, y));
    let row = w.spawn_unit(bp, owner, at, Angle::ZERO, true).unwrap();
    w.state.units.flags[row] |= flags;
    w.state.units.id(row)
}

fn order(w: &mut World, command: Command) {
    w.tick(&[PlayerCommand { player: 0, command }]).unwrap();
}

/// Four buildings on the shore, 1.35 km off a Megalodon out at sea.
fn marks(w: &mut World) -> Vec<UnitId> {
    [700, 900, 1100, 1300]
        .map(|y| spawn(w, "aster_t1_power", 1, 150, y, flag::INVULNERABLE))
        .to_vec()
}

/// The marks the strike missiles in the air are flying at, over `ticks`.
fn struck(w: &mut World, ticks: u32) -> BTreeSet<UnitId> {
    let bp = w.blueprints.id_of(MEGALODON).unwrap();
    let mut seen = BTreeSet::new();
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
        let p = &w.state.projectiles;
        seen.extend(
            (0..p.len())
                .filter(|&i| p.blueprint[i] == bp && p.weapon[i] == STRIKE)
                .map(|i| p.target[i]),
        );
    }
    seen
}

#[test]
fn it_is_built_on_the_water_and_not_ashore() {
    let w = sea();
    let bp = w
        .blueprints
        .unit(w.blueprints.id_of(MEGALODON).unwrap())
        .clone();
    assert!(bp.is_site_built_unit(), "not built on site");
    assert!(w.can_place(&bp, snap_to_build_grid(&bp, FxVec2::from_ints(1300, 1000))));
    assert!(!w.can_place(&bp, snap_to_build_grid(&bp, FxVec2::from_ints(120, 1000))));
}

#[test]
fn a_strike_volley_spreads_over_four_marks() {
    let mut w = sea();
    let marks = marks(&mut w);
    spawn(&mut w, MEGALODON, 0, 1500, 1000, 0);
    let seen = struck(&mut w, 120);
    assert_eq!(seen.len(), 4, "the volley went at {seen:?}");
    assert!(seen.iter().all(|t| marks.contains(t)));
}

#[test]
fn an_ordered_strike_volley_stays_on_its_mark() {
    let mut w = sea();
    let marks = marks(&mut w);
    let boat = spawn(&mut w, MEGALODON, 0, 1500, 1000, 0);
    order(
        &mut w,
        Command::Attack {
            units: vec![boat],
            target: marks[2],
            queue: false,
        },
    );
    let seen = struck(&mut w, 120);
    assert_eq!(seen, BTreeSet::from([marks[2]]));
}

#[test]
fn its_deck_rails_fire_only_with_the_boat_surfaced() {
    let mut w = sea();
    let boat = spawn(&mut w, MEGALODON, 0, 1500, 1000, 0);
    spawn(
        &mut w,
        "aster_t1_frigate",
        1,
        1500,
        1900,
        flag::PASSIVE | flag::INVULNERABLE,
    );
    let bp = w.blueprints.id_of(MEGALODON).unwrap();
    let rails = |w: &World| {
        w.events
            .iter()
            .filter(|e| {
                matches!(e, SimEvent::ShotFired { blueprint, weapon, .. }
                    if *blueprint == bp && RAILS.contains(weapon))
            })
            .count()
    };
    let mut dived = 0;
    for _ in 0..150 {
        w.tick(&[]).unwrap();
        dived += rails(&w);
    }
    assert_eq!(dived, 0, "a rail fired with the boat under");
    order(
        &mut w,
        Command::SetDive {
            units: vec![boat],
            dive: false,
        },
    );
    let mut up = 0;
    for _ in 0..250 {
        w.tick(&[]).unwrap();
        up += rails(&w);
    }
    assert!(up > 0, "the rails never fired surfaced");
}

#[test]
fn it_holds_four_warheads_and_launches_one_from_under_the_water() {
    let mut w = sea();
    let bp = w.blueprints.id_of(MEGALODON).unwrap();
    assert_eq!(
        w.blueprints.unit(bp).strategic.as_ref().map(|s| s.stock),
        Some(4)
    );
    let boat = spawn(&mut w, MEGALODON, 0, 1500, 1000, 0);
    for _ in 0..80 {
        w.tick(&[]).unwrap();
    }
    w.state.strategic.launchers.entry(boat).or_default().stock = 4;
    order(
        &mut w,
        Command::LaunchNuke {
            units: vec![boat],
            pos: FxVec2::from_ints(300, 1500),
        },
    );
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::NuclearLaunch { .. }))
        {
            return;
        }
    }
    panic!("no warhead left the boat");
}
