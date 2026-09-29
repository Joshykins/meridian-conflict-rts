//! A battleship fights broadside on (`motion.broadside`), and each battery fires the
//! moment it bears, never waiting for another (docs/NAVY.md "The Leviathan").

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// The sea: 20 m of water over a flat bed at zero, with a strip of land along the west
/// edge (x under about 312 m), `land` metres high.
const WATER: i32 = 20;

fn blueprints() -> Arc<Blueprints> {
    Arc::new(Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap())
}

fn shore(fog: bool, land: u16) -> World {
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..40 {
            samples[y * 257 + x] = land;
        }
    }
    let terrain =
        Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(WATER));
    let map = MapData {
        name: "sea".into(),
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
        seed: 7,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints(), Arc::new(Pool::new(1)), &config).unwrap()
}

fn sea(fog: bool) -> World {
    shore(fog, 40)
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32, flags: u16) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.flags[row] |= flags;
    w.state.units.id(row)
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

fn order(w: &mut World, player: u8, command: Command) {
    w.tick(&[PlayerCommand { player, command }]).unwrap();
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

/// Shots fired this tick by `owner` from weapon slot `weapon` of blueprint `key`.
fn fired(w: &World, key: &str, owner: u8, weapon: u8) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.events
        .iter()
        .filter(|e| {
            matches!(e, SimEvent::ShotFired { blueprint, owner: o, weapon: s, .. }
                if *blueprint == bp && *o == owner && *s == weapon)
        })
        .count()
}

const SHIP: &str = "aster_t3_battleship";

/// Shots of each main battery over 400 ticks of an Attack on a mark at (`tx`, `ty`), and
/// the mark's last bearing off the Leviathan's bow while it lived (degrees, signed).
fn engage(tx: i32, ty: i32) -> ([usize; 3], f64) {
    let mut w = sea(false);
    let ship = spawn(&mut w, SHIP, 0, 600, 1000, 0);
    let target = spawn(&mut w, SHIP, 1, tx, ty, flag::PASSIVE);
    order(
        &mut w,
        0,
        Command::Attack {
            units: vec![ship],
            target,
            queue: false,
        },
    );
    let (mut shots, mut off) = ([0usize; 3], f64::NAN);
    for _ in 0..400 {
        run(&mut w, 1);
        for (b, n) in shots.iter_mut().enumerate() {
            *n += fired(&w, SHIP, 0, b as u8);
        }
        if let Some(t) = w.state.units.row(target) {
            let r = row(&w, ship);
            let to = (w.state.units.pos[t] - w.state.units.pos[r]).angle();
            off = w.state.units.heading[r].delta_to(to) as f64 * 360.0 / 65536.0;
        }
    }
    (shots, off)
}

#[test]
fn a_battleship_turns_its_beam_to_a_mark_dead_ahead_and_every_battery_fires() {
    // Dead ahead, 900 m: the aft battery cannot bear until the hull comes round.
    let (shots, off) = engage(1500, 1080);
    assert!(
        shots.iter().all(|&n| n >= 6),
        "every battery fires, all three barrels a time: {shots:?}"
    );
    assert!(
        (60.0..=90.0).contains(&off.abs()),
        "mark {off:.1} deg off the bow once laid"
    );
}

#[test]
fn a_broadside_is_laid_on_the_nearer_beam() {
    let (_, left) = engage(1100, 1500);
    let (_, right) = engage(1100, 500);
    assert!(
        left > 0.0 && right < 0.0,
        "left {left:.1}, right {right:.1}"
    );
}

#[test]
fn the_aft_battery_comes_round_the_long_way_past_the_bow() {
    // Laid over the port bow, then given a mark on the starboard beam: the short way
    // round crosses the bow, which it cannot bear through, so it swings round by the stern.
    let mut w = sea(false);
    let ship = spawn(&mut w, SHIP, 0, 600, 1000, 0);
    let r = row(&w, ship);
    w.state.units.weapon_yaw[r][2] = Angle::from_degrees(60);
    let target = spawn(&mut w, SHIP, 1, 1100, 500, flag::PASSIVE);
    order(
        &mut w,
        0,
        Command::Attack {
            units: vec![ship],
            target,
            queue: false,
        },
    );
    let mut aft = 0;
    for _ in 0..400 {
        run(&mut w, 1);
        aft += fired(&w, SHIP, 0, 2);
    }
    let yaw = w.state.units.weapon_yaw[row(&w, ship)][2];
    assert!(
        aft > 0,
        "the aft battery never fired; laid {:.1} deg off the bow",
        Angle::ZERO.delta_to(yaw) as f64 * 360.0 / 65536.0
    );
}

#[test]
fn the_secondaries_on_the_engaged_beam_fire_and_the_far_side_s_hold() {
    // A mark 500 m off, a little to port of dead ahead: the hull lays its port beam to
    // it, and the two port secondaries (weapons 4 and 5) join in; the starboard pair
    // (6 and 7) cannot bear through the hull.
    let mut w = sea(false);
    let ship = spawn(&mut w, SHIP, 0, 600, 1000, 0);
    let target = spawn(&mut w, SHIP, 1, 1100, 1040, flag::PASSIVE);
    order(
        &mut w,
        0,
        Command::Attack {
            units: vec![ship],
            target,
            queue: false,
        },
    );
    let (mut port, mut starboard) = ([0usize; 2], 0);
    for _ in 0..300 {
        run(&mut w, 1);
        port[0] += fired(&w, SHIP, 0, 4);
        port[1] += fired(&w, SHIP, 0, 5);
        starboard += fired(&w, SHIP, 0, 6) + fired(&w, SHIP, 0, 7);
    }
    assert!(
        port.iter().all(|&n| n >= 6),
        "port secondaries fired {port:?}"
    );
    assert_eq!(starboard, 0, "the starboard secondaries cannot bear");
}

#[test]
fn a_secondary_at_rest_is_trained_outboard() {
    let mut w = sea(false);
    let ship = spawn(&mut w, SHIP, 0, 600, 1000, 0);
    run(&mut w, 10);
    let r = row(&w, ship);
    let deg = |a: Angle| a.0 as i16 as f64 * 360.0 / 65536.0;
    for (slot, want) in [(4, 90.0), (5, 90.0), (6, -90.0), (7, -90.0)] {
        let yaw = deg(w.state.units.weapon_yaw[r][slot]);
        assert!(
            (yaw - want).abs() < 1.0,
            "secondary {slot} rests at {yaw:.1}"
        );
    }
}

#[test]
fn a_missile_launcher_ashore_fires_on_a_battleship_offshore_but_not_on_a_dived_boat() {
    // The shore strip runs along the west edge; the launcher sits on it, the ship 500 m out.
    let mut w = sea(false);
    let javelin = spawn(&mut w, "aster_t2_missile", 0, 150, 1000, 0);
    spawn(&mut w, SHIP, 1, 650, 1000, flag::PASSIVE);
    let mut shots = 0;
    for _ in 0..200 {
        run(&mut w, 1);
        shots += fired(&w, "aster_t2_missile", 0, 0);
    }
    assert!(shots > 0, "the Javelin never fired on the Leviathan");
    assert!(w.state.units.row(javelin).is_some());

    let mut w = sea(false);
    spawn(&mut w, "aster_t2_missile", 0, 150, 1000, 0);
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 500, 1000, flag::PASSIVE);
    run(&mut w, 60);
    let r = row(&w, sub);
    assert!(
        w.state.units.z[r] + w.bp(r).height < Fx::from_int(WATER),
        "the boat is dived"
    );
    let mut shots = 0;
    for _ in 0..200 {
        run(&mut w, 1);
        shots += fired(&w, "aster_t2_missile", 0, 0);
    }
    assert_eq!(shots, 0, "a missile rack cannot touch a dived boat");
}

#[test]
fn battleships_on_ground_fire_fire_as_their_charge_ends() {
    // Three Leviathans shelling one area: each battery lays on a point of its own and slews
    // to a new one after every shot, so they come ready at different times. Every shot
    // comes the charge's length after that battery began charging, and no battery fires
    // without one.
    let mut w = sea(false);
    let ships: Vec<UnitId> = (0..3)
        .map(|i| spawn(&mut w, SHIP, 0, 600 + i * 60, 900 + i * 90, 0))
        .collect();
    order(
        &mut w,
        0,
        Command::AttackGround {
            units: ships.clone(),
            pos: FxVec2::from_ints(1600, 1500),
            queue: false,
        },
    );
    let charge = w.bp(row(&w, ships[0])).weapons[0].charge_ticks as u32;
    // Per ship and battery: the tick its charge began, until it fires.
    let mut charging = [[None::<u32>; 3]; 3];
    let mut shots = 0;
    for tick in 0..1500u32 {
        run(&mut w, 1);
        let at: Vec<FxVec2> = ships
            .iter()
            .map(|&s| w.state.units.pos[row(&w, s)])
            .collect();
        let mut shot = [[false; 3]; 3];
        for e in &w.events {
            match e {
                SimEvent::WeaponCharging { unit, weapon, .. } if *weapon < 3 => {
                    let s = ships.iter().position(|&s| s == *unit).unwrap();
                    charging[s][*weapon as usize] = Some(tick);
                }
                SimEvent::ShotFired {
                    weapon,
                    pos,
                    owner: 0,
                    ..
                } if *weapon < 3 => {
                    let s = (0..3).min_by_key(|&s| at[s].distance(pos.xy())).unwrap();
                    shot[s][*weapon as usize] = true;
                }
                _ => {}
            }
        }
        for s in 0..3 {
            for b in 0..3 {
                if shot[s][b] {
                    shots += 1;
                    let began = charging[s][b].take();
                    assert_eq!(
                        began.map(|t| tick - t),
                        Some(charge),
                        "ship {s} battery {b} fired at tick {tick}, charge began {began:?}"
                    );
                }
            }
        }
    }
    assert!(shots >= 27, "{shots} battery shots");
}

/// The tick each main battery of a Leviathan first began its charge, over 400 ticks of an
/// Attack on a mark off its starboard bow, with the aft battery (weapon 2) first slewed
/// `aft` degrees off the bow (`None`: left at rest, astern).
fn first_charges(aft: Option<i32>) -> [Option<u32>; 3] {
    let mut w = sea(false);
    let ship = spawn(&mut w, SHIP, 0, 600, 1000, 0);
    let r = row(&w, ship);
    if let Some(aft) = aft {
        w.state.units.weapon_yaw[r][2] = Angle::from_degrees(aft);
    }
    let target = spawn(&mut w, SHIP, 1, 1100, 500, flag::PASSIVE);
    order(
        &mut w,
        0,
        Command::Attack {
            units: vec![ship],
            target,
            queue: false,
        },
    );
    let mut first = [None; 3];
    for tick in 0..400u32 {
        run(&mut w, 1);
        for e in &w.events {
            if let SimEvent::WeaponCharging { unit, weapon, .. } = e {
                if *unit == ship && *weapon < 3 {
                    first[*weapon as usize].get_or_insert(tick);
                }
            }
        }
    }
    first
}

#[test]
fn a_battery_that_bears_fires_while_another_is_still_turning() {
    // The aft battery starts laid over the port bow and has the long way round by the
    // stern to come onto a starboard mark. The forward batteries bear at once: they
    // charge long before it does, and on the very tick they do with it at rest astern.
    let (turning, laid) = (first_charges(Some(60)), first_charges(None));
    let [Some(fore), Some(second), Some(aft)] = turning else {
        panic!("a battery never charged: {turning:?}");
    };
    assert!(
        fore.max(second) + 20 < aft,
        "the forward batteries waited for the aft one: {turning:?}"
    );
    assert_eq!(
        turning[..2],
        laid[..2],
        "the forward batteries' timing hangs on the aft battery's"
    );
}
