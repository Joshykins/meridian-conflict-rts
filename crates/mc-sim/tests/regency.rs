//! Regency combat: Harrow turns its body within the tail arc, immediately sweeps an
//! excavation lance over the surface, and charges two six-shot corkscrew area salvos.
//! A lot remembers the faction it was levelled for.

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
fn the_scorpion_turns_its_body_onto_a_mark_behind_it() {
    let mut w = world();
    // Facing east, with an ARC tank 150 m to its west: dead astern.
    let scorpion = add(&mut w, "regency_t4_scorpion", 0, 512, 512, 0);
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
    let scorpion = add(&mut w, "regency_t4_scorpion", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 712, 512, 180);
    let t = w.state.units.row(target).unwrap();
    w.state.units.flags[t] |= mc_sim::tables::flag::PASSIVE | mc_sim::tables::flag::INVULNERABLE;
    let beam = w.blueprints.id_of("regency_t4_scorpion").unwrap();
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
fn the_claws_charge_then_throw_two_sixes_onto_a_ground_area() {
    let mut w = world();
    let scorpion = add(&mut w, "regency_t4_scorpion", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 800, 512, 180);
    let t = w.state.units.row(target).unwrap();
    w.state.units.flags[t] |= mc_sim::tables::flag::PASSIVE | mc_sim::tables::flag::INVULNERABLE;
    let id = w.blueprints.id_of("regency_t4_scorpion").unwrap();
    let mut charge_at = [None; 2];
    let mut thrown = [0; 2];
    let mut impacts = Vec::new();
    let mut marks = Vec::new();
    let mut seen = Vec::new();
    for tick in 0..40 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::WeaponCharging {
                    unit,
                    blueprint,
                    weapon: which @ (1 | 2),
                    ..
                } if *blueprint == id => {
                    assert_eq!(*unit, scorpion);
                    charge_at[*which as usize - 1] = Some(tick);
                }
                SimEvent::ShotFired {
                    pos,
                    blueprint,
                    weapon: which @ (1 | 2),
                    ..
                } if *blueprint == id => {
                    let slot = *which as usize - 1;
                    let charged = charge_at[slot].expect("each claw charges before firing");
                    assert!(
                        tick - charged
                            >= w.blueprints.unit(id).weapons[*which as usize].charge_ticks as i32
                    );
                    let r = w.state.units.row(scorpion).unwrap();
                    let mount = w.blueprints.unit(id).weapons[*which as usize].muzzle;
                    let claw = w.state.units.pos[r] + mount.xy().rotate(w.state.units.heading[r]);
                    assert!(pos.xy().distance(claw) < Fx::from_int(2));
                    thrown[slot] += 1;
                }
                SimEvent::Impact {
                    pos,
                    blueprint,
                    weapon: 1 | 2,
                    ..
                } if *blueprint == id => impacts.push(*pos),
                _ => {}
            }
        }
        let p = &w.state.projectiles;
        for i in (0..p.len()).filter(|&i| p.blueprint[i] == id && p.weapon[i] != 0) {
            if !seen.contains(&p.serial[i]) {
                seen.push(p.serial[i]);
                marks.push(p.mark[i]);
                // Straight out for the first part of flight.
                let direction = (p.mark[i] - p.origin[i]).normalize();
                assert!(p.vel[i].normalize().dot(direction) > Fx::ratio(999, 1000));
            }
        }
    }
    assert_eq!(thrown, [6, 6], "two charged six-shot salvos");
    assert_eq!(marks.len(), 12);
    assert_eq!(impacts.len(), 12, "all charges arrive");
    let centre = FxVec2::from_ints(800, 512);
    assert!(marks
        .iter()
        .all(|m| m.xy().distance(centre) > Fx::from_int(8)
            && m.xy().distance(centre) < Fx::from_int(11)
            && m.z == Fx::from_int(20)));
    let mut unique: Vec<_> = marks
        .iter()
        .map(|p| (p.x.raw(), p.y.raw(), p.z.raw()))
        .collect();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        12,
        "the two rings interleave into twelve area points"
    );
    assert!(impacts
        .iter()
        .all(|p| p.xy().distance(centre) < Fx::from_int(24)));
}

#[test]
fn the_tail_stays_within_its_sweep_while_the_body_turns() {
    let mut w = world();
    // A tank off the scorpion's left quarter, 100 degrees off the nose: past the tail's
    // reach, so the body comes round while the tail lays on it.
    let scorpion = add(&mut w, "regency_t4_scorpion", 0, 512, 512, 0);
    let tank = add(&mut w, "aster_t1_tank", 1, 512 - 35, 512 + 197, 0);
    let mut worst = 0i32;
    for _ in 0..(3 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        let (Some(r), Some(t)) = (w.state.units.row(scorpion), w.state.units.row(tank)) else {
            break;
        };
        let u = &w.state.units;
        let root = u.pos[r]
            + w.blueprints
                .unit(w.state.units.blueprint[r])
                .turret_at
                .unwrap()
                .rotate(u.heading[r]);
        let bearing = (u.pos[t] - root).angle();
        let laid = u.heading[r] + u.weapon_yaw[r][0];
        // The lance intentionally crosses its mark; it must stay inside its 32 m swath.
        worst = worst.max(bearing.delta_to(laid) as i32);
    }
    assert!(
        worst <= Angle::from_degrees(12).0 as i32,
        "laid {} degrees past its mark",
        worst * 360 / 65536
    );
}

#[test]
fn the_lance_fires_without_spin_up_and_sweeps_both_sides_of_the_mark() {
    let mut w = world();
    add(&mut w, "regency_t4_scorpion", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 712, 512, 180);
    let t = w.state.units.row(target).unwrap();
    w.state.units.flags[t] |= mc_sim::tables::flag::PASSIVE | mc_sim::tables::flag::INVULNERABLE;
    let id = w.blueprints.id_of("regency_t4_scorpion").unwrap();
    assert_eq!(w.blueprints.unit(id).weapons[0].spin_ticks, 0);
    let mut first = None;
    let mut directions = Vec::new();
    for tick in 0..100 {
        w.tick(&[]).unwrap();
        for event in &w.events {
            if let SimEvent::ShotFired {
                blueprint,
                weapon: 0,
                vel,
                ..
            } = event
            {
                if *blueprint == id {
                    first.get_or_insert(tick);
                    directions.push(vel.y);
                }
            }
        }
    }
    assert!(
        first.is_some_and(|t| t < 5),
        "ready lance lights immediately: {first:?}"
    );
    assert!(directions.iter().any(|y| *y > Fx::ZERO));
    assert!(directions.iter().any(|y| *y < Fx::ZERO));
    assert!(directions.len() > 80, "holds through its terrain sweep");
}

/// A lot remembers which faction it was levelled for, so the renderer clads the slopes
/// round a Regency structure in Regency armour (`TerrainEdit::faction`).
#[test]
fn a_lot_records_the_faction_it_was_levelled_for() {
    let mut w = world();
    add(&mut w, "regency_t1_power", 0, 600, 600, 0);
    add(&mut w, "aster_t1_power", 1, 900, 900, 0);
    let faction = |key: &str| {
        w.blueprints
            .unit(w.blueprints.id_of(key).unwrap())
            .faction
            .0
    };
    let edits: Vec<u8> = w.state.terrain_edits.iter().map(|e| e.faction).collect();
    assert_eq!(
        edits,
        [faction("regency_t1_power"), faction("aster_t1_power")]
    );
    assert_ne!(edits[0], edits[1]);
}
