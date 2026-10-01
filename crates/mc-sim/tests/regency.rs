//! The Regency's own rules. The battle scorpion's tail reaches only across its nose
//! (`aim_arc`), so it turns its whole body onto a mark behind it instead of swinging the
//! tail round, and does not lay past the mark while the body comes round; its beam runs
//! up, then holds on its mark every tick; its claws throw their bombs from between their
//! fingers as fast streaks that curve in on the mark, each from its own angle.
//! A lot remembers the faction it was levelled for.

use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
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
    let scorpion = add(&mut w, "regency_t3_scorpion", 0, 512, 512, 0);
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
    let scorpion = add(&mut w, "regency_t3_scorpion", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 712, 512, 180);
    let beam = w.blueprints.id_of("regency_t3_scorpion").unwrap();
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
    let scorpion = add(&mut w, "regency_t3_scorpion", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 772, 512, 180);
    let id = w.blueprints.id_of("regency_t3_scorpion").unwrap();
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
    // On the mark, each on the side it came in from: they home on the target.
    assert!(landed.iter().all(|d| *d < 20.0), "{landed:?}");
    assert!(landed.iter().any(|d| *d > 1.0), "{landed:?}");
}

/// Degrees between two directions across the ground.
fn bearing_gap(a: FxVec3, b: FxVec3) -> f32 {
    let d = a.xy().angle().delta_to(b.xy().angle()) as f32;
    (d * 360.0 / 65536.0).abs()
}

#[test]
fn the_bombs_streak_out_and_curve_in_from_around_the_mark() {
    let mut w = world();
    add(&mut w, "regency_t3_scorpion", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 800, 512, 180);
    let id = w.blueprints.id_of("regency_t3_scorpion").unwrap();
    // Per shot (by serial): how it left, the line to the mark then, and how it arrived.
    let mut shots: Vec<(u32, FxVec3, FxVec3, Option<FxVec3>)> = Vec::new();
    let mut hits = 0;
    for _ in 0..(10 * TICKS_PER_SECOND) {
        let before: Vec<(u32, FxVec3)> = {
            let p = &w.state.projectiles;
            (0..p.len())
                .filter(|&i| p.blueprint[i] == id && p.weapon[i] != 0)
                .map(|i| (p.serial[i], p.vel[i]))
                .collect()
        };
        w.tick(&[]).unwrap();
        let Some(mark) = w.state.units.row(target).map(|r| w.state.units.pos[r]) else {
            break;
        };
        let p = &w.state.projectiles;
        for i in (0..p.len()).filter(|&i| p.blueprint[i] == id && p.weapon[i] != 0) {
            if p.age[i] == 1 && !shots.iter().any(|s| s.0 == p.serial[i]) {
                let line = (mark - p.pos[i].xy()).extend(Fx::ZERO);
                shots.push((p.serial[i], p.vel[i], line, None));
            }
        }
        // A shot gone since last tick arrived on what it last flew.
        for (serial, vel) in before {
            if !(0..p.len()).any(|i| p.serial[i] == serial) {
                if let Some(s) = shots.iter_mut().find(|s| s.0 == serial) {
                    s.3 = Some(vel);
                }
            }
        }
        hits += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::Impact { blueprint, weapon: 1 | 2, on_unit: true, .. } if *blueprint == id))
            .count();
    }
    assert!(shots.len() >= 6, "{} bombs thrown", shots.len());
    let speed = |v: FxVec3| v.length().to_f32() * TICKS_PER_SECOND as f32;
    // Fast streaks, not a lob.
    assert!(
        shots.iter().all(|s| speed(s.1) > 200.0),
        "left at {:?} m/s",
        shots.iter().map(|s| speed(s.1)).collect::<Vec<_>>()
    );
    // Fanned off the line to the mark: the outer ones well out to either side.
    let off: Vec<f32> = shots.iter().map(|s| bearing_gap(s.2, s.1)).collect();
    assert!(
        off.iter().filter(|d| **d > 25.0).count() >= 4,
        "left off the line by {off:?}"
    );
    // Every one arrived, and not down one line: their last legs come in from around it.
    let arrived: Vec<FxVec3> = shots.iter().filter_map(|s| s.3).collect();
    assert!(
        arrived.len() >= 6 && hits >= 6,
        "{} arrived, {hits} hit",
        arrived.len()
    );
    let widest = arrived
        .iter()
        .flat_map(|a| arrived.iter().map(move |b| bearing_gap(*a, *b)))
        .fold(0.0f32, f32::max);
    assert!(
        widest > 30.0,
        "all came in within {widest} degrees of each other"
    );
}

#[test]
fn the_tail_does_not_lay_past_its_mark_while_the_body_turns() {
    let mut w = world();
    // A tank off the scorpion's left quarter, 100 degrees off the nose: past the tail's
    // reach, so the body comes round while the tail lays on it.
    let scorpion = add(&mut w, "regency_t3_scorpion", 0, 512, 512, 0);
    let tank = add(&mut w, "aster_t1_tank", 1, 512 - 35, 512 + 197, 0);
    let mut worst = 0i32;
    for _ in 0..(3 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        let (Some(r), Some(t)) = (w.state.units.row(scorpion), w.state.units.row(tank)) else {
            break;
        };
        let u = &w.state.units;
        let root = u.pos[r] + FxVec2::new(Fx::from_int(-884) / 100, Fx::ZERO).rotate(u.heading[r]);
        let bearing = (u.pos[t] - root).angle();
        let laid = u.heading[r] + u.weapon_yaw[r][0];
        // How far past the mark (on the far side from where it came round: the left) it lies.
        worst = worst.max(bearing.delta_to(laid) as i32);
    }
    assert!(
        worst <= Angle::from_degrees(2).0 as i32,
        "laid {} degrees past its mark",
        worst * 360 / 65536
    );
}

#[test]
fn the_beam_runs_up_before_it_lights_and_down_after() {
    let mut w = world();
    let scorpion = add(&mut w, "regency_t3_scorpion", 0, 512, 512, 0);
    let target = add(&mut w, "aster_t1_tank", 1, 712, 512, 180);
    let id = w.blueprints.id_of("regency_t3_scorpion").unwrap();
    let run = w.blueprints.unit(id).weapons[0].spin_ticks;
    assert!(run > 0);
    let mut levels = Vec::new();
    let mut first = None;
    for t in 0..(10 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        let r = w.state.units.row(scorpion).unwrap();
        let spin = w.state.units.spin[r];
        // Last tick's level is kept, a step behind.
        assert!(spin[0].abs_diff(spin[3]) <= 1, "{spin:?}");
        levels.push(spin[0]);
        let fired = w.events.iter().any(
            |e| matches!(e, SimEvent::ShotFired { blueprint, weapon: 0, .. } if *blueprint == id),
        );
        if fired && first.is_none() {
            first = Some(t);
            assert_eq!(spin[0], run, "lit before it had run up");
        }
    }
    assert!(first.is_some(), "the beam never fired");
    assert!(w.state.units.row(target).is_none(), "the tank still stands");
    // With nothing left to shoot, it ran down again.
    assert_eq!(*levels.last().unwrap(), 0);
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
