//! The Regent (`regency_t5_super_destroyer`): its guns are weapon groups, a house each
//! turning, charging and firing on its own (`mc_sim::mounts`); its balance against the
//! capital ships and the Zenith.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const REGENT: &str = "regency_t5_super_destroyer";
const SCOURGE: &str = "regency_t4_space_destroyer";
const DOMINION: &str = "aster_t4_dreadnought";
const ZENITH: &str = "aster_t4_anti_ship";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(2048, 2048, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(15000, 15000)],
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
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // Every hull field is paid for.
    for p in &mut w.state.players {
        p.free_build = true;
    }
    w
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

/// Health and hull field together.
fn left(w: &World, id: UnitId) -> f32 {
    w.state.units.row(id).map_or(0.0, |r| {
        (w.state.units.health[r] + w.state.units.shield_hp[r]).to_f32()
    })
}

fn set_fire(w: &mut World, id: UnitId, state: FireState) {
    let row = w.state.units.row(id).unwrap();
    w.state.units.fire_state[row] = state;
}

fn seconds(s: u32) -> usize {
    (s * TICKS_PER_SECOND) as usize
}

/// Lets freshly spawned ships climb to their cruise heights first.
fn settle(w: &mut World) {
    for _ in 0..seconds(60) {
        w.tick(&[]).unwrap();
    }
}

fn attack(w: &mut World, units: Vec<UnitId>, target: UnitId) {
    let cmd = PlayerCommand {
        player: w
            .state
            .units
            .row(units[0])
            .map_or(0, |r| w.state.units.owner[r]),
        command: Command::Attack {
            units,
            target,
            queue: false,
        },
    };
    w.tick(&[cmd]).unwrap();
}

/// The Regent at (4000, 8000) facing east, settled; `n` of `foe` (owner 1) `range` metres
/// off its beam to the north, spread along it, and settled too. Neither side fires while
/// they climb.
fn line_up(foe: &str, n: i32, range: i32) -> (World, UnitId, Vec<UnitId>) {
    let mut w = world();
    let regent = add(&mut w, REGENT, 0, 4000, 8000, 0);
    let foes: Vec<UnitId> = (0..n)
        .map(|k| add(&mut w, foe, 1, 4000 + (k - n / 2) * 450, 8000 + range, 180))
        .collect();
    for &u in foes.iter().chain([&regent]) {
        set_fire(&mut w, u, FireState::HoldFire);
    }
    settle(&mut w);
    for &u in foes.iter().chain([&regent]) {
        set_fire(&mut w, u, FireState::FireAtWill);
    }
    (w, regent, foes)
}

/// Fights it out for up to `cap` seconds: the foes ordered onto the Regent when `close`.
/// Returns the seconds until the Regent or every foe was dead, the Regent's share of its
/// health and field left, and how many foes still stand.
fn fight(
    w: &mut World,
    regent: UnitId,
    foes: &[UnitId],
    close: bool,
    cap: u32,
) -> (u32, f32, usize) {
    let full = left(w, regent);
    if close {
        attack(w, foes.to_vec(), regent);
    }
    let standing = |w: &World| {
        foes.iter()
            .filter(|&&f| w.state.units.row(f).is_some())
            .count()
    };
    for t in 0..seconds(cap) {
        w.tick(&[]).unwrap();
        if w.state.units.row(regent).is_none() || standing(w) == 0 {
            return (
                t as u32 / TICKS_PER_SECOND,
                left(w, regent) / full,
                standing(w),
            );
        }
    }
    (cap, left(w, regent) / full, standing(w))
}

/// Which guns of the Regent fired this tick, by gun slot.
fn guns_fired(w: &World, regent: UnitId) -> Vec<u16> {
    let bp = w
        .state
        .units
        .row(regent)
        .map(|r| w.state.units.blueprint[r]);
    w.events
        .iter()
        .filter_map(|e| match e {
            SimEvent::ShotFired { blueprint, gun, .. } if Some(*blueprint) == bp => Some(*gun),
            _ => None,
        })
        .collect()
}

/// The gun slots of weapon `w`'s guns.
fn slots(w: &World, regent: UnitId, weapon: usize) -> std::ops::Range<u16> {
    let bp = w.blueprints.unit(w.blueprints.id_of(REGENT).unwrap());
    let _ = regent;
    let first = bp.gun_slot(weapon) as u16;
    first..first + bp.weapons[weapon].guns() as u16
}

/// Each gun of a group turns and fires on its own: a Scourge off the port beam draws the
/// ventral lances and every port battery, and no battery across the hull; each port
/// battery's house turns from where it rests onto its mark.
#[test]
fn its_groups_guns_turn_and_fire_on_their_own() {
    let (mut w, regent, _foes) = line_up(SCOURGE, 1, 1500);
    let row = w.state.units.row(regent).unwrap();
    let batteries = slots(&w, regent, 2);
    let (port, starboard) = (
        batteries.start..batteries.start + 6,
        batteries.start + 6..batteries.end,
    );
    let mut seen = std::collections::BTreeSet::new();
    // The yaws the port batteries are laid at while their mark lives: each its own.
    let mut laid: Vec<Vec<u16>> = Vec::new();
    for _ in 0..seconds(30) {
        w.tick(&[]).unwrap();
        seen.extend(guns_fired(&w, regent));
        let m = &w.state.units.mounts[row];
        if port
            .clone()
            .all(|g| m[g as usize].target != mc_sim::Handle::NONE)
        {
            laid.push(port.clone().map(|g| m[g as usize].yaw.0).collect());
        }
    }
    assert!(
        slots(&w, regent, 1).all(|g| seen.contains(&g)),
        "a ventral lance never fired: {seen:?}"
    );
    assert!(
        port.clone().all(|g| seen.contains(&g)),
        "a port battery never fired: {seen:?}"
    );
    assert!(
        !starboard.clone().any(|g| seen.contains(&g)),
        "a starboard battery fired across the hull: {seen:?}"
    );
    // Off the beam, each port battery turns from its rest out to port onto the Scourge, and
    // from each pivot down the hull it lies a different way.
    let last = laid
        .last()
        .expect("the port batteries were never all on the Scourge");
    let rest = Angle::from_degrees(90).0;
    assert!(
        last.iter().all(|&y| y != rest),
        "a battery stayed at rest: {last:?}"
    );
    assert!(
        last.iter().any(|&y| y != last[0]),
        "the batteries all lie one way: {last:?}"
    );
}

/// Its guns choose only past the gate every gun of the unit goes through: holding fire,
/// or stunned, each gun lets go of its mark.
#[test]
fn holding_fire_or_stunned_its_guns_let_go() {
    let (mut w, regent, _foes) = line_up(SCOURGE, 1, 1500);
    for _ in 0..seconds(5) {
        w.tick(&[]).unwrap();
    }
    let row = w.state.units.row(regent).unwrap();
    let marked = |w: &World| {
        w.state.units.mounts[row]
            .iter()
            .filter(|m| m.target != mc_sim::Handle::NONE)
            .count()
    };
    assert!(
        marked(&w) > 4,
        "too few guns on the Scourge: {}",
        marked(&w)
    );
    set_fire(&mut w, regent, FireState::HoldFire);
    w.tick(&[]).unwrap();
    assert_eq!(marked(&w), 0, "a gun kept its mark holding fire");
    set_fire(&mut w, regent, FireState::FireAtWill);
    for _ in 0..seconds(2) {
        w.tick(&[]).unwrap();
    }
    assert!(marked(&w) > 4);
    w.state.units.stun[row] = [100, 100];
    w.tick(&[]).unwrap();
    assert_eq!(marked(&w), 0, "a gun kept its mark stunned");
    let fired = guns_fired(&w, regent);
    assert!(fired.is_empty(), "a gun fired stunned: {fired:?}");
}

/// The keel guns reach the ground right under the hull, where nothing on its deck can.
#[test]
fn its_keel_guns_reach_the_ground_under_it() {
    let mut w = world();
    let regent = add(&mut w, REGENT, 0, 4000, 8000, 0);
    set_fire(&mut w, regent, FireState::HoldFire);
    settle(&mut w);
    let tank = add(&mut w, "aster_t4_assault_tank", 1, 4800, 8050, 0);
    set_fire(&mut w, tank, FireState::HoldFire);
    set_fire(&mut w, regent, FireState::FireAtWill);
    let keel = slots(&w, regent, 4);
    let mut fired = false;
    for _ in 0..seconds(20) {
        w.tick(&[]).unwrap();
        fired |= guns_fired(&w, regent).iter().any(|g| keel.contains(g));
    }
    assert!(fired, "no keel gun fired on the tank under the hull");
}

/// A snapshot whose units keep other guns than their blueprints have is refused: the
/// weapons phase reads each gun by its slot.
#[test]
fn a_snapshot_with_the_wrong_guns_is_refused() {
    let (mut w, regent, _foes) = line_up(SCOURGE, 1, 1500);
    assert!(w.state.validate_mounts(&w.blueprints).is_ok());
    let row = w.state.units.row(regent).unwrap();
    w.state.units.mounts[row].pop();
    assert!(w.state.validate_mounts(&w.blueprints).is_err());
}

/// From 3 km it brings down a lone Scourge or Dominion in about twenty seconds, untouched.
#[test]
fn a_lone_capital_ship_falls_in_about_twenty_seconds() {
    for foe in [SCOURGE, DOMINION] {
        let (mut w, regent, foes) = line_up(foe, 1, 3000);
        let (t, hp, standing) = fight(&mut w, regent, &foes, false, 120);
        assert!(
            standing == 0 && (15..=30).contains(&t) && hp > 0.95,
            "{foe}: {t} s, Regent {hp:.2}"
        );
    }
}

/// It beats five Scourges or five Dominions in its reach, and eight are about even: it
/// barely outlasts eight Scourges, and eight Dominions bring it down at a heavy price.
#[test]
fn it_beats_five_and_is_about_even_with_eight() {
    for foe in [SCOURGE, DOMINION] {
        let (mut w, regent, foes) = line_up(foe, 5, 2600);
        let (t, hp, standing) = fight(&mut w, regent, &foes, false, 300);
        assert!(
            standing == 0 && hp > 0.3,
            "{foe} x5: {t} s, Regent {hp:.2}, {standing} standing"
        );
        let (mut w, regent, foes) = line_up(foe, 8, 2600);
        let (t, hp, standing) = fight(&mut w, regent, &foes, false, 300);
        let even = (standing == 0 && hp < 0.6) || (hp == 0.0 && standing <= 5);
        assert!(even, "{foe} x8: {t} s, Regent {hp:.2}, {standing} standing");
    }
}

/// A dozen Zeniths behind missile defence bring it down, its guns in reach of theirs: its
/// direct fire never outreaches the Zenith's rail.
#[test]
fn a_dozen_zeniths_behind_missile_defence_bring_it_down() {
    let bp = |w: &World, key: &str| w.blueprints.unit(w.blueprints.id_of(key).unwrap()).clone();
    let mut w = world();
    let zenith = bp(&w, ZENITH).weapons[0].range_max;
    for weapon in bp(&w, REGENT).weapons.iter().filter(|g| !g.missile) {
        assert!(
            weapon.range_max < zenith,
            "{} outreaches the Zenith",
            weapon.name
        );
    }
    let regent = add(&mut w, REGENT, 0, 4000, 8000, 0);
    set_fire(&mut w, regent, FireState::HoldFire);
    settle(&mut w);
    let guns: Vec<UnitId> = (0..12)
        .map(|k| {
            add(
                &mut w,
                ZENITH,
                1,
                4000 + (k % 6 - 3) * 360,
                10600 + (k / 6) * 240,
                270,
            )
        })
        .collect();
    for k in 0..6 {
        add(
            &mut w,
            "aster_t3_missile_defense",
            1,
            3100 + k * 360,
            10720,
            0,
        );
    }
    set_fire(&mut w, regent, FireState::FireAtWill);
    let (t, hp, standing) = fight(&mut w, regent, &guns, false, 300);
    assert!(
        hp == 0.0 && standing >= 6,
        "{t} s: Regent {hp:.2}, {standing} Zeniths standing"
    );
}

/// Where `regent`'s move order went when it is sent to `x, y`.
fn sent_to(w: &mut World, regent: UnitId, x: i32, y: i32) -> FxVec2 {
    let cmd = PlayerCommand {
        player: 0,
        command: Command::FormationMove {
            units: vec![regent],
            target: FxVec2::from_ints(x, y),
            queue: false,
            attack_move: false,
            facing: None,
            together: true,
            spacing: 1,
            shape: 0,
        },
    };
    w.tick(&[cmd]).unwrap();
    let row = w.state.units.row(regent).unwrap();
    w.state.orders.front(&w.state.units, row).unwrap().pos
}

/// A Regent ordered alongside another parked one goes there: the room it needs is its
/// hull, a beam wide, not the disc of its 1.9 km radius (mark 2 of 20261014-005113: every
/// click within 3.8 km of a parked Regent sent the ship 4 km off somewhere else).
#[test]
fn a_regent_parks_abreast_another_a_beam_apart() {
    let mut w = world();
    add(&mut w, REGENT, 0, 6000, 6000, 0);
    let regent = add(&mut w, REGENT, 0, 12000, 7100, 180);
    settle(&mut w);
    // Flying west to lie 1.1 km off the parked one's beam: 200 m between their sides.
    assert_eq!(
        sent_to(&mut w, regent, 6500, 7100),
        FxVec2::from_ints(6500, 7100)
    );
    // Onto its hull, it is found the nearest room clear of it: a beam over, not a radius.
    let click = FxVec2::from_ints(6500, 6500);
    let shifted = sent_to(&mut w, regent, 6500, 6500);
    assert_ne!(shifted, click);
    assert!(shifted.distance(click) <= Fx::from_int(1000), "{shifted:?}");
    assert!(
        (shifted.y - Fx::from_int(6000)).abs() > Fx::from_int(906),
        "{shifted:?}"
    );
}
