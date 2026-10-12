//! The Mistral (`aster_t3_mobile_aa`): ARC's tech 3 rail AA. One slug a shot at one
//! aircraft, fast enough that the lead (combat.rs `direct_air_aim`) lands it on a
//! strategic bomber crossing at the edge of its reach.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const MISTRAL: &str = "aster_t3_mobile_aa";
const ECLIPSE: &str = "aster_t3_strategic_bomber";
/// Where the Mistral stands.
const GUN: (i32, i32) = (2000, 2000);

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(1024, 1024, Fx::from_int(20));
    let map = MapData {
        name: "mistral".into(),
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
        seed: 11,
        players: vec![player("you", 0), player("raider", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

/// An Eclipse at (`x`, `y`), at its cruise height and speed, holding its bombs.
fn eclipse(w: &mut World, x: i32, y: i32) -> UnitId {
    let id = w.blueprints.id_of(ECLIPSE).unwrap();
    let motion = w.blueprints.unit(id).motion.unwrap();
    let row = w
        .spawn_unit(id, 1, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.z[row] = Fx::from_int(20) + motion.altitude;
    w.state.units.speed[row] = motion.speed;
    w.state.units.flags[row] |= flag::PASSIVE;
    w.state.units.id(row)
}

fn fly(w: &mut World, units: Vec<UnitId>, x: i32, y: i32) {
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::Move {
            units,
            target: FxVec2::from_ints(x, y),
            queue: false,
        },
    }])
    .unwrap();
}

/// Slugs fired and slugs that struck an aircraft, this tick.
fn shots(w: &World, gun: mc_data::BlueprintId) -> (usize, usize) {
    let (mut fired, mut hit) = (0, 0);
    for event in &w.events {
        match event {
            SimEvent::ShotFired { blueprint, .. } if *blueprint == gun => fired += 1,
            SimEvent::Impact {
                blueprint,
                on_unit: true,
                ..
            } if *blueprint == gun => hit += 1,
            _ => {}
        }
    }
    (fired, hit)
}

/// The gun is what the brief asked for: one slug at aircraft only, no flak, no split,
/// no cluster, outranging the heavy air it is made to meet.
#[test]
fn the_skyrail_is_one_slug_at_one_aircraft() {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(MISTRAL).unwrap());
    assert!(bp.has(cat::ANTI_AIR) && bp.has(cat::LAND));
    let [gun] = &bp.weapons[..] else {
        panic!("the Mistral carries one weapon");
    };
    assert_eq!(gun.target_mask, cat::AIR);
    assert!(gun.rail && !gun.flak && !gun.airburst && !gun.split && !gun.missile);
    assert!(gun.cluster.is_none() && gun.salvo <= 1 && gun.splash <= Fx::from_int(5));
    let reach = |key: &str| {
        let b = w.blueprints.unit(w.blueprints.id_of(key).unwrap());
        b.weapons.iter().map(|w| w.range_max).max().unwrap()
    };
    assert!(gun.range_max > reach(ECLIPSE));
    assert!(gun.range_max > reach("aster_t3_assault_aircraft"));
}

/// Eclipses fly in from 1250 m and on past the guns, 300 m to one side. The slugs land,
/// nearly every one. One Mistral gets three into the bomber on its pass (4500 of its
/// 6000); a pair kills it before it is past.
#[test]
fn mistrals_kill_an_eclipse_crossing_their_reach() {
    for guns in [1, 2] {
        let mut w = world();
        let gun = w.blueprints.id_of(MISTRAL).unwrap();
        for i in 0..guns {
            w.spawn_unit(
                gun,
                0,
                FxVec2::from_ints(GUN.0, GUN.1 - 40 * i),
                Angle::ZERO,
                true,
            )
            .unwrap();
        }
        let bomber = eclipse(&mut w, GUN.0 - 1214, GUN.1 + 300);
        fly(&mut w, vec![bomber], GUN.0 + 1500, GUN.1 + 300);
        let (mut fired, mut hit) = (0, 0);
        let mut alive = true;
        for _ in 0..20 * TICKS_PER_SECOND {
            w.tick(&[]).unwrap();
            let (f, h) = shots(&w, gun);
            fired += f;
            hit += h;
            if w.state.units.row(bomber).is_none() {
                alive = false;
                break;
            }
        }
        assert!(
            fired >= 3 && hit * 5 >= fired * 4,
            "{guns} guns: {hit} of {fired} slugs hit"
        );
        if guns == 1 {
            assert!(hit >= 3, "one gun: {hit} of {fired} slugs hit");
        } else {
            assert!(!alive, "the Eclipse flew past a pair: {hit} of {fired} hit");
        }
    }
}

/// A pair of Eclipses side by side: every slug that lands takes its full damage off one
/// of them, never both.
#[test]
fn each_slug_strikes_one_aircraft() {
    let mut w = world();
    let gun = w.blueprints.id_of(MISTRAL).unwrap();
    let damage = w.blueprints.unit(gun).weapons[0].damage;
    w.spawn_unit(gun, 0, FxVec2::from_ints(GUN.0, GUN.1), Angle::ZERO, true)
        .unwrap();
    let pair = [
        eclipse(&mut w, GUN.0 - 1200, GUN.1 + 300),
        eclipse(&mut w, GUN.0 - 1200, GUN.1 + 330),
    ];
    fly(&mut w, pair.to_vec(), GUN.0 + 1500, GUN.1 + 315);
    const TOUGH: i32 = 100_000;
    let mut hits = 0;
    for _ in 0..12 * TICKS_PER_SECOND {
        for id in pair {
            // Tough enough that neither dies and every hit shows.
            if let Some(r) = w.state.units.row(id) {
                w.state.units.health[r] = Fx::from_int(TOUGH);
            }
        }
        w.tick(&[]).unwrap();
        let (_, h) = shots(&w, gun);
        let lost = pair.map(|id| {
            let r = w.state.units.row(id).unwrap();
            Fx::from_int(TOUGH) - w.state.units.health[r]
        });
        let struck = lost.iter().filter(|&&l| l > Fx::ZERO).count();
        assert!(struck <= h, "{struck} aircraft hurt by {h} slugs");
        assert_eq!(lost[0] + lost[1], damage * h as i32);
        hits += h;
    }
    assert!(hits >= 2, "only {hits} slugs landed");
}
