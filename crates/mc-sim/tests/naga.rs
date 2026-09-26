//! The Naga's own rules. The battle scorpion's tail reaches only across its nose
//! (`aim_arc`), so it turns its whole body onto a mark behind it instead of swinging the
//! tail round; its beam holds on its mark every tick; its claws throw their bombs from
//! between their fingers to land around the mark.

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
        name: "naga".into(),
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
        players: vec![player("Naga", 0), player("Aster", 1)],
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
fn the_scorpion_turns_its_body_onto_a_mark_behind_it() {
    let mut w = world();
    // Facing east, with an ARC tank 150 m to its west: dead astern.
    let scorpion = add(&mut w, "naga_t3_scorpion", 0, 512, 512, 0);
    let tank = add(&mut w, "aster_t1_tank", 1, 362, 512, 0);
    let hp = |w: &World| {
        w.state
            .units
            .row(tank)
            .map_or(0.0, |r| w.state.units.health[r].to_f32())
    };
    let reach = Angle::from_degrees(60).0 as i32 + Angle::from_degrees(1).0 as i32;
    for _ in 0..(8 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        let row = w.state.units.row(scorpion).unwrap();
        let yaw = Angle::ZERO.delta_to(w.state.units.weapon_yaw[row][0]) as i32;
        assert!(
            yaw.abs() <= reach,
            "the tail swung {} degrees off the nose",
            yaw * 360 / 65536
        );
    }
    // It squared up to the tank (due west) and the beam killed it.
    let row = w.state.units.row(scorpion).unwrap();
    let off = w.state.units.heading[row]
        .delta_to(Angle::from_degrees(180))
        .unsigned_abs();
    assert!(
        off <= Angle::from_degrees(3).0,
        "still {} degrees off the tank",
        off as u32 * 360 / 65536
    );
    assert_eq!(hp(&w), 0.0, "the beam killed it");
}

#[test]
fn the_beam_holds_on_its_mark_every_tick() {
    let mut w = world();
    let scorpion = add(&mut w, "naga_t3_scorpion", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 712, 512, 180);
    let beam = w.blueprints.id_of("naga_t3_scorpion").unwrap();
    let mut first = None;
    let mut fed = 0;
    for t in 0..(12 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        let fired = w.events.iter().any(
            |e| matches!(e, SimEvent::ShotFired { blueprint, weapon: 0, .. } if *blueprint == beam),
        );
        if fired && first.is_none() {
            first = Some(t);
        }
        // Three seconds of it, from its first shot, while the Paladin still stands.
        if let Some(f) = first {
            if t < f + 3 * TICKS_PER_SECOND && w.state.units.row(target).is_some() {
                fed += usize::from(fired);
            }
        }
    }
    assert!(first.is_some(), "the beam never fired");
    assert!(
        fed as u64 >= 3 * TICKS_PER_SECOND as u64 * 9 / 10,
        "fired on {fed} ticks of three seconds' worth"
    );
    assert!(w.state.units.row(scorpion).is_some());
}

#[test]
fn the_claws_throw_bombs_that_land_around_the_mark() {
    let mut w = world();
    let scorpion = add(&mut w, "naga_t3_scorpion", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 772, 512, 180);
    let id = w.blueprints.id_of("naga_t3_scorpion").unwrap();
    let bomb = w.blueprints.unit(id).weapons[1].clone();
    let mut charged = 0;
    let mut throws = Vec::new();
    let mut landed = Vec::new();
    for _ in 0..(12 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        let row = w.state.units.row(scorpion).unwrap();
        let (pos, heading) = (w.state.units.pos[row], w.state.units.heading[row]);
        let Some(mark) = w.state.units.row(target).map(|r| w.state.units.pos[r]) else {
            break;
        };
        for e in &w.events {
            match e {
                SimEvent::WeaponCharging {
                    unit,
                    blueprint,
                    weapon: 1 | 2,
                    ..
                } if *blueprint == id => {
                    assert_eq!(*unit, scorpion, "the charge names its unit");
                    charged += 1;
                }
                SimEvent::ShotFired {
                    pos: at,
                    blueprint,
                    weapon: which @ (1 | 2),
                    ..
                } if *blueprint == id => {
                    // Out of the claw that threw it: between its fingers.
                    let side = if *which == 1 { 1 } else { -1 };
                    let claw =
                        pos + FxVec2::new(bomb.muzzle.x, bomb.muzzle.y * side).rotate(heading);
                    throws.push(at.xy().distance(claw).to_f32());
                }
                SimEvent::Impact {
                    pos: at,
                    blueprint,
                    weapon: 1 | 2,
                    ..
                } if *blueprint == id => landed.push(at.xy().distance(mark).to_f32()),
                _ => {}
            }
        }
    }
    assert!(charged >= 2, "each claw charges first ({charged})");
    assert!(throws.len() >= 6, "{} bombs thrown", throws.len());
    assert!(
        throws.iter().all(|d| *d < 1.5),
        "thrown from off the claws: {throws:?}"
    );
    assert!(landed.len() >= 6, "{} bombs landed", landed.len());
    // Around the mark, not all on one spot.
    assert!(landed.iter().all(|d| *d < 50.0), "{landed:?}");
    assert!(landed.iter().any(|d| *d > 3.0), "{landed:?}");
}
