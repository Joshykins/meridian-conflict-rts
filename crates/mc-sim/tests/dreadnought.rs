//! The Dominion dreadnought (`aster_t4_dreadnought`): a broadside ship, its six Arc Cannon
//! casemates (three a side) and two bolt rifles fighting on their own while the hull
//! holds; its SAM cells reach aircraft far off; a hull field takes fire before the plates.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const DREADNOUGHT: &str = "aster_t4_dreadnought";

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

fn hold_fire(w: &mut World, id: UnitId) {
    let row = w.state.units.row(id).unwrap();
    w.state.units.fire_state[row] = FireState::HoldFire;
}

fn heading_of(w: &World, id: UnitId) -> f32 {
    let row = w.state.units.row(id).unwrap();
    w.state.units.heading[row].0 as f32 * 360.0 / 65536.0
}

fn seconds(s: u32) -> usize {
    (s * TICKS_PER_SECOND) as usize
}

/// Lets a freshly spawned ship climb to its cruise height first.
fn settle(w: &mut World) {
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
    }
}

/// Which of the ship's weapons fired this tick.
fn fired(w: &World, ship: UnitId) -> Vec<u8> {
    let bp = w.state.units.row(ship).map(|r| w.state.units.blueprint[r]);
    w.events
        .iter()
        .filter_map(|e| match e {
            SimEvent::ShotFired {
                blueprint, weapon, ..
            } if Some(*blueprint) == bp => Some(*weapon),
            _ => None,
        })
        .collect()
}

#[test]
fn its_casemates_and_bolt_rifles_fight_on_their_own() {
    let mut w = world();
    // Nose east; tanks off the port beam, 1.2 km out: the port casemates reach them, and
    // the hull only squares its beam to them.
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    settle(&mut w);
    let tank = "aster_t4_assault_tank";
    let marks: Vec<_> = [-200, 0, 200]
        .iter()
        .map(|dx| add(&mut w, tank, 1, 3000 + dx, 4200, 0))
        .collect();
    for &m in &marks {
        hold_fire(&mut w, m);
    }
    let mut seen = [false; 9];
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
        for wi in fired(&w, ship) {
            seen[wi as usize] = true;
        }
    }
    // The marks are off the port beam: every port casemate bears on them, and no
    // starboard one does.
    for (wi, name) in [
        (0, "fore port casemate"),
        (2, "midships port casemate"),
        (4, "aft port casemate"),
    ] {
        assert!(seen[wi], "the {name} never fired: {seen:?}");
    }
    assert!(
        !seen[1] && !seen[3] && !seen[5],
        "a starboard casemate fired across the hull: {seen:?}"
    );
    // The rifles sit on the stacked hull: the tanks are under its edge from there.
    assert!(
        !seen[6] && !seen[7],
        "a rifle fired down through the hull: {seen:?}"
    );
    // It lays its beam on them (`broadside`), it does not wheel its nose round.
    let heading = heading_of(&w, ship);
    assert!(
        heading < 20.0 || heading > 340.0,
        "the hull swung off its beam: {heading}"
    );
}

#[test]
fn its_rifles_take_a_warship_off_its_beam() {
    let mut w = world();
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    settle(&mut w);
    // A frigate climbing 1.3 km off its beam: it flies lower, but that far out it is
    // above the rifles' depression.
    let frigate = add(&mut w, "aster_t4_frigate", 1, 3000, 4300, 0);
    hold_fire(&mut w, frigate);
    let mut seen = [false; 9];
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
        for wi in fired(&w, ship) {
            seen[wi as usize] = true;
        }
    }
    assert!(seen[6], "the fore rifles never fired: {seen:?}");
}

#[test]
fn shots_strike_the_hull_not_a_disc_round_it() {
    let mut w = world();
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    hold_fire(&mut w, ship);
    settle(&mut w);
    // A Zenith off the port beam: its rails cross the 285 m of the ship's radius, most
    // of it open air, before they reach the flank 76 m out.
    let _gun = add(&mut w, "aster_t4_anti_ship", 1, 3000, 4200, 180);
    let mut struck = Vec::new();
    for _ in 0..seconds(25) {
        w.tick(&[]).unwrap();
        struck.extend(w.events.iter().filter_map(|e| match e {
            SimEvent::Impact { pos, .. } if pos.z.to_f32() > 300.0 => Some(pos.y.to_f32() - 3000.0),
            _ => None,
        }));
    }
    assert!(!struck.is_empty(), "the Zenith never struck the ship");
    for off in struck {
        assert!(
            off.abs() < 100.0,
            "a shot burst {off} m off the keel line, clear of the hull"
        );
    }
}

#[test]
fn its_sam_cells_reach_aircraft_far_off() {
    let mut w = world();
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    settle(&mut w);
    // A gunship loitering 2.6 km off: past every gun on the ship but inside the cells'.
    let gunship = add(&mut w, "aster_t2_gunship", 1, 5600, 3000, 180);
    hold_fire(&mut w, gunship);
    let full = health(&w, gunship);
    let mut cells = false;
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
        cells |= fired(&w, ship).contains(&8);
    }
    assert!(cells, "the SAM cells never fired");
    assert!(
        health(&w, gunship) < full,
        "the SAMs never reached the gunship"
    );
}

#[test]
fn its_hull_field_takes_fire_before_the_plates() {
    let mut w = world();
    // Its upkeep is paid: an unpowered field is down.
    w.state.players[0].free_build = true;
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    hold_fire(&mut w, ship);
    settle(&mut w);
    let row = w.state.units.row(ship).unwrap();
    let shield = w.state.units.shield_hp[row];
    assert!(shield > Fx::ZERO, "the ship has no field up");
    // A Zenith under it: its rail is built for warships.
    let _gun = add(&mut w, "aster_t4_anti_ship", 1, 4200, 3000, 180);
    let full = health(&w, ship);
    let mut lowest = shield;
    // Up to the first round only: one Zenith shot is less than the field holds.
    for _ in 0..seconds(25) {
        w.tick(&[]).unwrap();
        let row = w.state.units.row(ship).unwrap();
        lowest = lowest.min(w.state.units.shield_hp[row]);
        if lowest < shield {
            break;
        }
    }
    assert!(lowest < shield, "the field took nothing");
    assert!(
        (health(&w, ship) - full).abs() < 1.0,
        "the plates were hit while the field held"
    );
}

#[test]
fn it_lays_its_beam_on_a_mark_dead_ahead() {
    let mut w = world();
    w.state.players[0].free_build = true;
    // Nose east, a tank dead ahead inside the casemates' reach: none bears until the
    // hull comes round to put it on a beam.
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    settle(&mut w);
    let tank = add(&mut w, "aster_t4_assault_tank", 1, 5200, 3000, 180);
    hold_fire(&mut w, tank);
    let mut casemates = false;
    for _ in 0..seconds(60) {
        w.tick(&[]).unwrap();
        casemates |= fired(&w, ship).iter().any(|&wi| wi <= 5);
    }
    let heading = heading_of(&w, ship);
    assert!(casemates, "no casemate fired (heading {heading})");
    assert!(
        (heading - 90.0).abs() < 15.0 || (heading - 270.0).abs() < 15.0,
        "the beam is not on the mark: {heading}"
    );
}

#[test]
fn the_zenith_outranges_every_gun_on_it() {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(DREADNOUGHT).unwrap());
    let zenith = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t4_anti_ship").unwrap());
    for weapon in bp.weapons.iter().filter(|w| !w.missile) {
        assert!(
            weapon.range_max < zenith.weapons[0].range_max,
            "{}",
            weapon.name
        );
    }
}

/// One `gun` (owner 1) against one `ship` (owner 0, field powered) `range` metres off,
/// the ship either parked there or ordered to attack the gun from there. Returns the
/// seconds until one of them died (or the cap) and both health fractions at that time.
fn duel(ship: &str, gun: &str, range: i32, attack: bool) -> (u32, f32, f32) {
    let mut w = world();
    w.state.players[0].free_build = true;
    let s = add(&mut w, ship, 0, 3000, 3000, 0);
    hold_fire(&mut w, s);
    settle(&mut w);
    let row = w.state.units.row(s).unwrap();
    w.state.units.fire_state[row] = FireState::default();
    let g = add(&mut w, gun, 1, 3000 + range, 3000, 180);
    if attack {
        let cmd = PlayerCommand {
            player: 0,
            command: Command::Attack {
                units: vec![s],
                target: g,
                queue: false,
            },
        };
        w.tick(&[cmd]).unwrap();
    }
    let (ship_full, gun_full) = (health(&w, s), health(&w, g));
    for t in 0..seconds(240) {
        w.tick(&[]).unwrap();
        if w.state.units.row(s).is_none() || w.state.units.row(g).is_none() {
            return (
                t as u32 / TICKS_PER_SECOND,
                health(&w, s) / ship_full,
                health(&w, g) / gun_full,
            );
        }
    }
    (240, health(&w, s) / ship_full, health(&w, g) / gun_full)
}

/// One Zenith costs about 70% of a Dominion and brings it down reliably: parked close,
/// parked at the ship's own reach, or with the ship ordered in from out of range.
#[test]
fn one_zenith_brings_down_a_dominion() {
    let ship = w_cost(DREADNOUGHT);
    let gun = w_cost("aster_t4_anti_ship");
    assert!(
        (0.6..=0.8).contains(&(gun / ship)),
        "Zenith {gun} vs Dominion {ship}"
    );
    for (range, attack) in [(1500, false), (2300, false), (4000, true)] {
        let (t, ship, gun) = duel(DREADNOUGHT, "aster_t4_anti_ship", range, attack);
        assert!(
            ship == 0.0 && gun > 0.5,
            "at {range} m (attack {attack}) after {t} s: ship {ship:.2}, Zenith {gun:.2}"
        );
    }
}

fn w_cost(key: &str) -> f32 {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(key).unwrap());
    bp.cost_mass.to_f32()
}

/// Where the entity shader draws the tips of weapon `wi`'s barrels on `u`: each muzzle
/// pitched and turned in its house about the pivot (`HousePose`), then carried by the
/// hull's bank, its pitch and its heading.
fn drawn_barrel_tips(
    w: &World,
    frame: &mc_sim::RenderFrame,
    u: &mc_sim::mirror::UnitInstance,
    wi: usize,
) -> Vec<[f32; 3]> {
    let weapon = &w
        .blueprints
        .unit(mc_data::BlueprintId(u.blueprint as u16))
        .weapons[wi];
    let house = frame.houses[(u.status[1] >> mc_sim::mirror::UNIT_HOUSE_SHIFT) as usize - 1];
    let [_, yaw, _, pitch] = house.pose[wi];
    let rot_z = |v: [f32; 3], a: f32| {
        let (s, c) = a.sin_cos();
        [v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]]
    };
    let rot_xz = |v: [f32; 3], a: f32| {
        let (s, c) = a.sin_cos();
        [v[0] * c - v[2] * s, v[1], v[0] * s + v[2] * c]
    };
    let roll = |v: [f32; 3], a: f32| {
        let (s, c) = a.sin_cos();
        [v[0], v[1] * c - v[2] * s, v[1] * s + v[2] * c]
    };
    let pivot = weapon.pivot.unwrap().to_f32();
    weapon
        .muzzles
        .iter()
        .map(|m| {
            let m = m.to_f32();
            let arm = rot_z(
                rot_xz([m[0] - pivot[0], m[1] - pivot[1], m[2] - pivot[2]], pitch),
                yaw,
            );
            let local = [pivot[0] + arm[0], pivot[1] + arm[1], pivot[2] + arm[2]];
            let world = rot_z(rot_xz(roll(local, u._pad2[1]), u.arm_pitch[1]), u.heading);
            [
                u.pos[0] + world[0],
                u.pos[1] + world[1],
                u.pos[2] + world[2],
            ]
        })
        .collect()
}

#[test]
fn casemate_shots_leave_the_barrels_as_drawn() {
    let mut w = world();
    // Nose east; tanks off the port beam: the casemates fire at them.
    let ship = add(&mut w, DREADNOUGHT, 0, 3000, 3000, 0);
    settle(&mut w);
    let marks: Vec<_> = [-200, 0, 200]
        .iter()
        .map(|dx| add(&mut w, "aster_t4_assault_tank", 1, 3000 + dx, 4200, 0))
        .collect();
    for &m in &marks {
        hold_fire(&mut w, m);
    }
    let mut frame = mc_sim::RenderFrame::default();
    let mut shots = 0;
    for _ in 0..seconds(40) {
        w.tick(&[]).unwrap();
        let casemate = |wi: u8| (0..=5).contains(&wi);
        if !fired(&w, ship).into_iter().any(casemate) {
            continue;
        }
        w.write_render_frame(None, &mut frame);
        let u = *frame.units.iter().find(|u| u.unit_id == ship.0).unwrap();
        for e in &frame.events {
            let SimEvent::ShotFired { pos, weapon, .. } = e else {
                continue;
            };
            if !casemate(*weapon) {
                continue;
            }
            let at = pos.to_f32();
            let off = drawn_barrel_tips(&w, &frame, &u, *weapon as usize)
                .iter()
                .map(|t| {
                    ((t[0] - at[0]).powi(2) + (t[1] - at[1]).powi(2) + (t[2] - at[2]).powi(2))
                        .sqrt()
                })
                .fold(f32::MAX, f32::min);
            assert!(
                off < 0.5,
                "casemate {weapon}'s shot left {off:.1} m from its drawn barrels"
            );
            shots += 1;
        }
    }
    assert!(shots >= 6, "only {shots} casemate shots");
}
