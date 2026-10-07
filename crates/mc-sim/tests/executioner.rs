//! Executioner charge timing, simultaneous rocket salvos and Paladin matchup.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "regency".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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
        seed: 3,
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

#[test]
fn executioner_charges_and_releases_six_unguided_rockets_together() {
    let mut w = world();
    let tank = add(&mut w, "regency_t3_assault_tank", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 790, 512, 180);
    let row = w.state.units.row(target).unwrap();
    w.state.units.flags[row] |= mc_sim::tables::flag::PASSIVE | mc_sim::tables::flag::INVULNERABLE;
    let bp = w.blueprints.id_of("regency_t3_assault_tank").unwrap();
    let gun = &w.blueprints.unit(bp).weapons[0];
    let charge_ticks = gun.charge_ticks;
    assert!(charge_ticks >= TICKS_PER_SECOND as u16);
    let rockets = &w.blueprints.unit(bp).weapons[2];
    assert!(!rockets.guided);
    assert!(rockets.spread >= Angle::from_degrees(6).0);
    let mut charged = None;
    let mut fired = false;
    let mut volleys = 0;
    let mut repeater = false;
    for tick in 0..(12 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        let mut rockets_this_tick = 0;
        for event in &w.events {
            match event {
                SimEvent::WeaponCharging {
                    unit, weapon: 0, ..
                } if *unit == tank => charged = Some(tick),
                SimEvent::ShotFired {
                    blueprint,
                    weapon: 0,
                    ..
                } if *blueprint == bp => {
                    assert!(
                        tick - charged.expect("fusion gun must charge before firing")
                            >= u32::from(charge_ticks)
                    );
                    fired = true;
                }
                SimEvent::ShotFired {
                    blueprint,
                    weapon: 1,
                    ..
                } if *blueprint == bp => repeater = true,
                SimEvent::ShotFired {
                    blueprint,
                    weapon: 2,
                    ..
                } if *blueprint == bp => rockets_this_tick += 1,
                _ => {}
            }
        }
        if rockets_this_tick > 0 {
            assert_eq!(
                rockets_this_tick, 6,
                "the entire rocket volley leaves on one tick"
            );
            volleys += 1;
        }
    }
    assert!(fired && repeater && volleys >= 1);
}

#[test]
fn executioner_has_a_small_advantage_against_a_paladin() {
    for range in [200, 320, 390] {
        let mut w = world();
        let tank = add(&mut w, "regency_t3_assault_tank", 0, 512, 512, 0);
        let paladin = add(&mut w, "aster_t3_assault_bot", 1, 512 + range, 512, 180);
        for _ in 0..(90 * TICKS_PER_SECOND) {
            w.tick(&[]).unwrap();
            if w.state.units.row(tank).is_none() || w.state.units.row(paladin).is_none() {
                break;
            }
        }
        let hp = |id| {
            w.state
                .units
                .row(id)
                .map_or(0.0, |r| w.state.units.health[r].to_f32())
        };
        eprintln!(
            "range {range}: Executioner {}, Paladin {}",
            hp(tank),
            hp(paladin)
        );
        assert!(
            hp(tank) > 0.0 && hp(paladin) == 0.0,
            "Executioner should narrowly win this stationary duel"
        );
        assert!(hp(tank) < 4200.0, "advantage should not be overwhelming");
    }
}
