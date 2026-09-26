//! The Resolute heavy frigate (`aster_t3_frigate`) and the Zenith anti-ship rail cannon
//! (`aster_t4_anti_ship`): the frigate's spinal rail is fixed along the keel, so the
//! whole hull turns and pitches onto its mark; the Zenith reaches warships far beyond any other gun;
//! and a wing of air superiority fighters is the frigate's answer.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const FRIGATE: &str = "aster_t3_frigate";
const ZENITH: &str = "aster_t4_anti_ship";
const RAPTOR: &str = "aster_t3_air_superiority";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(1024, 1024, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
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
        seed: 3,
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

fn health(w: &World, id: UnitId) -> f32 {
    w.state
        .units
        .row(id)
        .map_or(0.0, |r| w.state.units.health[r].to_f32())
}

fn seconds(s: u32) -> usize {
    (s * TICKS_PER_SECOND) as usize
}

/// Lets a freshly spawned ship climb to its cruise height first.
fn settle(w: &mut World) {
    for _ in 0..seconds(20) {
        w.tick(&[]).unwrap();
    }
}

#[test]
fn the_spinal_rail_turns_the_whole_ship_onto_a_structure() {
    let mut w = world();
    // Nose east; the mark is due north, well inside the spinal's reach but beyond the turrets'.
    let ship = add(&mut w, FRIGATE, 0, 3000, 3000, 0);
    settle(&mut w);
    let mark = add(&mut w, ZENITH, 1, 3000, 4100, 0);
    let full = health(&w, mark);
    let mut hit_at = None;
    for t in 0..seconds(90) {
        w.tick(&[]).unwrap();
        if health(&w, mark) <= full - 5000.0 {
            hit_at = Some(t);
            break;
        }
    }
    let row = w.state.units.row(ship).unwrap();
    let heading = w.state.units.heading[row].0 as f32 * 360.0 / 65536.0;
    assert!(
        hit_at.is_some(),
        "the spinal rail never landed (ship heading {heading})"
    );
    assert!(
        (heading - 90.0).abs() < 6.0,
        "the hull did not lay onto the mark: {heading}"
    );
}

#[test]
fn the_spinal_rail_hits_land_units_far_past_the_turrets() {
    let mut w = world();
    let _ship = add(&mut w, FRIGATE, 0, 3000, 3000, 90);
    settle(&mut w);
    // A super-heavy tank off the beam, 2.3 km out: well past the turrets' 800 m.
    let tank = add(&mut w, "aster_t4_assault_tank", 1, 5300, 3000, 180);
    let row = w.state.units.row(tank).unwrap();
    w.state.units.fire_state[row] = FireState::HoldFire;
    let full = health(&w, tank);
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
    }
    assert!(
        health(&w, tank) <= full - 8000.0,
        "the spinal rail did not land on a tank at 2.3 km: {} of {full} left",
        health(&w, tank)
    );
}

#[test]
fn the_zenith_reaches_a_warship_far_beyond_any_other_gun() {
    let mut w = world();
    let zenith = add(&mut w, ZENITH, 0, 2000, 3000, 0);
    let ship = add(&mut w, FRIGATE, 1, 4400, 3000, 180);
    let row = w.state.units.row(ship).unwrap();
    w.state.units.fire_state[row] = FireState::HoldFire;
    settle(&mut w);
    let full = health(&w, ship);
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
    }
    let lost = full - health(&w, ship);
    assert!(
        lost >= 14000.0,
        "the Zenith took only {lost} off a frigate at 2.4 km"
    );
    assert!(health(&w, zenith) > 0.0);
}

/// Raptors sent at a frigate: how long it lasts and how many fighters it kills.
fn raptor_raid(count: i32) -> (Option<usize>, usize) {
    let mut w = world();
    let ship = add(&mut w, FRIGATE, 1, 4000, 4000, 0);
    settle(&mut w);
    let raptors: Vec<UnitId> = (0..count)
        .map(|i| add(&mut w, RAPTOR, 0, 2400, 3800 + i * 40, 0))
        .collect();
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: raptors.clone(),
            target: ship,
            queue: false,
        },
    }])
    .unwrap();
    let mut died = None;
    for t in 0..seconds(90) {
        w.tick(&[]).unwrap();
        if health(&w, ship) <= 0.0 {
            died = Some(t);
            break;
        }
    }
    let lost = raptors.iter().filter(|&&r| health(&w, r) <= 0.0).count();
    (died, lost)
}

#[test]
fn a_few_raptors_lose_to_it_and_a_wing_brings_it_down() {
    let (few_kill, few_lost) = raptor_raid(4);
    let (wing_kill, wing_lost) = raptor_raid(12);
    eprintln!("4 raptors: frigate died {few_kill:?}, raptors lost {few_lost}");
    eprintln!("12 raptors: frigate died {wing_kill:?}, raptors lost {wing_lost}");
    assert!(few_kill.is_none(), "four Raptors brought a frigate down");
    let wing = wing_kill.expect("twelve Raptors could not bring a frigate down");
    assert!(wing <= seconds(40), "twelve Raptors took {} s", wing / 10);
    // It fights back: its rocket cells cost the wing some fighters, not most of them.
    assert!(
        (1..=6).contains(&wing_lost),
        "the wing lost {wing_lost} of twelve"
    );
}

#[test]
fn its_turrets_reach_the_ground_below_it() {
    let mut w = world();
    let _ship = add(&mut w, FRIGATE, 0, 3000, 3000, 0);
    settle(&mut w);
    // A tank off the port beam, close under the ship: only the turrets can hit it.
    let tank = add(&mut w, "aster_t2_tank", 1, 3000, 3300, 0);
    let row = w.state.units.row(tank).unwrap();
    w.state.units.fire_state[row] = FireState::HoldFire;
    let full = health(&w, tank);
    for _ in 0..seconds(20) {
        w.tick(&[]).unwrap();
    }
    assert!(
        health(&w, tank) < full,
        "no turret reached a tank 300 m out below the ship"
    );
}

/// The hull's pitch in degrees, nose up positive.
fn hull_pitch(w: &World, id: UnitId) -> f32 {
    let row = w.state.units.row(id).unwrap();
    Angle::ZERO.delta_to(w.state.units.arm_pitch[row][0]) as f32 * 360.0 / 65536.0
}

#[test]
fn the_spinal_rail_reaches_well_beyond_its_turrets() {
    let mut w = world();
    // A structure 1.9 km off the bow: past the old 1.4 km reach, far past the turrets'.
    let _ship = add(&mut w, FRIGATE, 0, 3000, 3000, 0);
    settle(&mut w);
    let mark = add(&mut w, ZENITH, 1, 4900, 3000, 0);
    let full = health(&w, mark);
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
    }
    assert!(
        health(&w, mark) <= full - 5000.0,
        "the spinal rail did not reach 1.9 km"
    );
}

#[test]
fn ground_fire_pitches_the_hull_down_and_the_spinal_fires_along_it() {
    let mut w = world();
    let ship = add(&mut w, FRIGATE, 0, 3000, 3000, 0);
    settle(&mut w);
    let spot = FxVec2::from_ints(4400, 3000);
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::AttackGround {
            units: vec![ship],
            pos: spot,
            queue: false,
        },
    }])
    .unwrap();
    let mut shot = None;
    for _ in 0..seconds(60) {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::ShotFired {
                pos,
                vel,
                weapon: 0,
                ..
            } = e
            {
                shot = Some((*pos, *vel, hull_pitch(&w, ship)));
            }
        }
        if shot.is_some() {
            break;
        }
    }
    let (pos, vel, pitch) = shot.expect("the spinal rail never fired on the ground");
    let dive = vel
        .z
        .to_f32()
        .atan2(vel.xy().length().to_f32())
        .to_degrees();
    eprintln!(
        "hull pitch {pitch:.1}, shot dive {dive:.1}, muzzle {:?}",
        pos.to_f32()
    );
    assert!(
        pitch < -10.0,
        "the hull did not pitch down onto the ground: {pitch}"
    );
    assert!(
        (pitch - dive).abs() < 3.0,
        "the shot left off the bore's line: hull {pitch}, shot {dive}"
    );
    // It stood off and laid the gun, instead of circling the point like a gunship.
    let row = w.state.units.row(ship).unwrap();
    let range = w.state.units.pos[row].distance(spot).to_f32();
    assert!(range > 900.0 && range <= 2400.0, "fired from {range} m");
    // Told to stop, it comes level again.
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Stop { units: vec![ship] },
    }])
    .unwrap();
    for _ in 0..seconds(15) {
        w.tick(&[]).unwrap();
    }
    assert!(
        hull_pitch(&w, ship).abs() < 0.5,
        "still pitched {} after stopping",
        hull_pitch(&w, ship)
    );
}

#[test]
fn a_mark_in_under_the_hull_is_left_to_the_turrets_and_the_hull_stays_level() {
    let mut w = world();
    let ship = add(&mut w, FRIGATE, 0, 3000, 3000, 0);
    settle(&mut w);
    // A structure almost under the bow: far too steep to dive onto.
    let mark = add(&mut w, ZENITH, 1, 3150, 3000, 0);
    let full = health(&w, mark);
    let frigate = w.blueprints.id_of(FRIGATE).unwrap();
    let mut steepest = 0.0f32;
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
        steepest = steepest.min(hull_pitch(&w, ship));
        for e in &w.events {
            if let SimEvent::ShotFired {
                weapon: 0,
                blueprint,
                ..
            } = e
            {
                if *blueprint != frigate {
                    continue;
                }
                panic!("the spinal rail fired into its dead zone");
            }
        }
    }
    assert!(
        steepest > -1.0,
        "the hull pitched {steepest} at a mark it cannot lay on"
    );
    assert!(
        health(&w, mark) < full,
        "the turrets left the mark under the ship alone"
    );
    // It did not move off to get a shot: it only moves when ordered.
    let row = w.state.units.row(ship).unwrap();
    assert!(
        w.state.units.pos[row]
            .distance(FxVec2::from_ints(3000, 3000))
            .to_f32()
            < 5.0
    );
}

#[test]
fn the_dead_zone_grows_with_height() {
    let low = mc_sim::combat::spinal_dead_zone(200.0, 47.0);
    let high = mc_sim::combat::spinal_dead_zone(560.0, 47.0);
    assert!(low > 200.0 && high > low * 2.0, "{low} {high}");
    assert_eq!(mc_sim::combat::spinal_dead_zone(0.0, 47.0), 0.0);
}

/// Where the ship is, in metres.
fn at(w: &World, id: UnitId) -> FxVec2 {
    w.state.units.pos[w.state.units.row(id).unwrap()]
}

#[test]
fn it_holds_where_it_is_on_marks_already_in_reach() {
    let mut w = world();
    let ship = add(&mut w, FRIGATE, 0, 3000, 3000, 0);
    settle(&mut w);
    let start = at(&w, ship);
    // A tank 2.2 km off the bow: inside the spinal's 2.4 km, past the old 4/5 standoff.
    let tank = add(&mut w, "aster_t4_assault_tank", 1, 5200, 3000, 180);
    let row = w.state.units.row(tank).unwrap();
    w.state.units.fire_state[row] = FireState::HoldFire;
    // Idle, it shoots from where it stands.
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
    }
    let idle = at(&w, ship).distance(start).to_f32();
    assert!(idle < 5.0, "idle, it moved {idle} m toward a mark in reach");
    // Ordered onto it, it still shoots from where it stands.
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: vec![ship],
            target: tank,
            queue: false,
        },
    }])
    .unwrap();
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
    }
    let ordered = at(&w, ship).distance(start).to_f32();
    assert!(
        ordered < 5.0,
        "ordered to attack, it moved {ordered} m toward a mark in reach"
    );
}

#[test]
fn ordered_onto_a_far_mark_it_closes_only_until_the_spinal_reaches() {
    let mut w = world();
    let ship = add(&mut w, FRIGATE, 0, 2000, 3000, 0);
    settle(&mut w);
    let tank = add(&mut w, "aster_t4_assault_tank", 1, 5500, 3000, 180);
    let row = w.state.units.row(tank).unwrap();
    w.state.units.fire_state[row] = FireState::HoldFire;
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: vec![ship],
            target: tank,
            queue: false,
        },
    }])
    .unwrap();
    let mut fired = false;
    for _ in 0..seconds(80) {
        w.tick(&[]).unwrap();
        fired |= w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::ShotFired { weapon: 0, .. }));
    }
    assert!(fired, "the spinal never fired");
    // It brakes to a stand just inside the spinal's reach, then does not edge in.
    let stood = at(&w, ship);
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
    }
    let range = stood.distance(at(&w, tank)).to_f32();
    let crept = at(&w, ship).distance(stood).to_f32();
    assert!(range > 2000.0, "it closed to {range} m");
    assert!(crept < 1.0, "it crept {crept} m in while firing");
}
