//! Decks (`decks.rs`): a bridge's road over a strait. Land units cross on top and
//! stand at the deck's height; ships sail under it and the two pass each other;
//! nothing is built on it or its projectors; shells coming down onto it stop there, so a ship under a
//! bridge is out of a land battery's reach while one beside it is not, and a ship
//! under it cannot shoot up through it while one beside it can.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{Deck, Heightfield};
use mc_sim::placement::{SiteMap, Unfit};
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// The water's height over the flat bed at zero, and the banks' and the deck's.
const WATER: i32 = 20;
const BANK: u16 = 40;
/// The strait runs north to south between the banks (x 480 m to 1568 m); the
/// deck crosses it east to west along this line.
const ROAD: i32 = 1020;

fn blueprints() -> Arc<Blueprints> {
    Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
    )
}

/// The strait's ground and its deck.
fn ground() -> Heightfield {
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in (0..60).chain(197..257) {
            samples[y * 257 + x] = BANK;
        }
    }
    let deck = Deck {
        a: FxVec2::from_ints(440, ROAD),
        b: FxVec2::from_ints(1_608, ROAD),
        half: Fx::from_int(60),
        top: Fx::from_int(BANK as i32),
    };
    Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(WATER))
        .with_decks(vec![deck])
}

fn strait() -> World {
    let terrain = ground();
    let map = MapData {
        doodads: Default::default(),
        name: "strait".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(200, 300), FxVec2::from_ints(1_850, 1_700)],
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
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints(), Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32, flags: u16) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.flags[row] |= flags;
    w.state.units.id(row)
}

fn order(w: &mut World, command: Command) {
    w.tick(&[PlayerCommand { player: 0, command }]).unwrap();
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn go(w: &mut World, unit: UnitId, x: i32, y: i32) {
    order(
        w,
        Command::Move {
            units: vec![unit],
            target: FxVec2::from_ints(x, y),
            queue: false,
        },
    );
}

#[test]
fn a_tank_crosses_on_the_deck_while_a_frigate_sails_under_it() {
    let mut w = strait();
    let tank = spawn(&mut w, "aster_t1_tank", 0, 300, ROAD, flag::PASSIVE);
    let frigate = spawn(&mut w, "aster_t1_frigate", 0, 1_020, 400, flag::PASSIVE);
    go(&mut w, tank, 1_800, ROAD);
    go(&mut w, frigate, 1_020, 1_700);
    let (mut tank_low, mut ship_high) = (Fx::from_int(1_000), Fx::from_int(-1_000));
    let mut closest = Fx::from_int(1_000);
    for _ in 0..1_200 {
        w.tick(&[]).unwrap();
        let (t, f) = (row(&w, tank), row(&w, frigate));
        let units = &w.state.units;
        let over_water = (units.pos[t].x - Fx::from_int(1_020)).abs() < Fx::from_int(400);
        if over_water {
            tank_low = tank_low.min(units.z[t]);
        }
        if (units.pos[f].y - Fx::from_int(ROAD)).abs() < Fx::from_int(50) {
            ship_high = ship_high.max(units.z[f]);
        }
        closest = closest.min(units.pos[t].distance(units.pos[f]));
    }
    let units = &w.state.units;
    let (t, f) = (row(&w, tank), row(&w, frigate));
    assert!(
        units.pos[t].x > Fx::from_int(1_700),
        "the tank stopped at {:?}",
        units.pos[t]
    );
    assert!(
        units.pos[f].y > Fx::from_int(1_600),
        "the frigate stopped at {:?}",
        units.pos[f]
    );
    assert_eq!(
        tank_low,
        Fx::from_int(BANK as i32),
        "the tank left the deck"
    );
    assert!(
        ship_high <= Fx::from_int(WATER) + Fx::ONE,
        "the frigate rode up to {ship_high:?}"
    );
    println!("tank and frigate came within {closest:?} m");
}

/// A tank drives straight over a frigate moored under the deck: neither turns
/// aside nor shoves the other.
#[test]
fn a_tank_on_the_deck_passes_over_a_frigate_under_it() {
    let mut w = strait();
    let frigate = spawn(&mut w, "aster_t1_frigate", 0, 1_020, ROAD, flag::PASSIVE);
    let tank = spawn(&mut w, "aster_t1_tank", 0, 700, ROAD, flag::PASSIVE);
    let moored = w.state.units.pos[row(&w, frigate)];
    go(&mut w, tank, 1_400, ROAD);
    let mut closest = Fx::from_int(1_000);
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        let units = &w.state.units;
        closest = closest.min(units.pos[row(&w, tank)].distance(units.pos[row(&w, frigate)]));
    }
    let shoved = w.state.units.pos[row(&w, frigate)].distance(moored);
    assert!(
        closest < Fx::from_int(6),
        "the tank kept {closest:?} m from the frigate"
    );
    assert_eq!(shoved, Fx::ZERO, "the frigate was shoved");
}

#[test]
fn nothing_is_built_on_a_deck_or_its_projectors() {
    let w = strait();
    let wall = w.blueprints.id_of("aster_t1_point_defense").unwrap();
    let bp = w.blueprints.unit(wall);
    // The deck; a projector on the west bank (150 m past the deck's end on a
    // 1168 m deck); the bank beside it and far off.
    let (deck, projector) = (FxVec2::from_ints(1_020, ROAD), FxVec2::from_ints(360, ROAD));
    let (beside, far) = (
        FxVec2::from_ints(360, ROAD + 200),
        FxVec2::from_ints(250, 600),
    );
    assert!(!w.can_place(bp, deck));
    assert!(!w.can_place(bp, projector));
    assert!(w.can_place(bp, beside));
    assert!(w.can_place(bp, far));
    // The placement preview says so before an engineer walks there.
    let sites = SiteMap::new(&ground(), &[]);
    let heading = bp.build_heading();
    assert_eq!(sites.check(bp, deck, heading), Err(Unfit::Bridge));
    assert_eq!(sites.check(bp, projector, heading), Err(Unfit::Bridge));
    assert_eq!(sites.check(bp, beside, heading), Ok(()));
}

/// Shells from a battery on the bank: a boat under the deck comes through
/// unharmed, one just past its side does not. (A frigate, 66 m long, sticks out
/// past the deck's sides.)
#[test]
fn shells_stop_on_the_deck_over_a_ship() {
    let hurt = |y: i32| {
        let mut w = strait();
        let gun = spawn(&mut w, "aster_t1_artillery", 0, 1_750, y, 0);
        let ship = spawn(&mut w, "aster_t1_attack_boat", 1, 1_400, y, flag::PASSIVE);
        let full = w.state.units.health[row(&w, ship)];
        order(
            &mut w,
            Command::Attack {
                units: vec![gun],
                target: ship,
                queue: false,
            },
        );
        for _ in 0..300 {
            w.tick(&[]).unwrap();
        }
        let left = w
            .state
            .units
            .row(ship)
            .map_or(Fx::ZERO, |r| w.state.units.health[r]);
        full - left
    };
    assert_eq!(hurt(ROAD), Fx::ZERO, "shells went through the deck");
    assert!(
        hurt(ROAD + 200) > Fx::ZERO,
        "the battery never hit the open water"
    );
}

/// A frigate left to fire at will on a tank on the deck: from under the deck it
/// holds its fire (every shot would meet the underside); from the open water
/// beside the bridge it shoots up over the side.
#[test]
fn a_ship_under_the_deck_cannot_shoot_up_through_it() {
    let hurt = |ship_at: (i32, i32), tank_at: (i32, i32)| {
        let mut w = strait();
        spawn(&mut w, "aster_t1_frigate", 0, ship_at.0, ship_at.1, 0);
        let tank = spawn(
            &mut w,
            "aster_t1_tank",
            1,
            tank_at.0,
            tank_at.1,
            flag::PASSIVE,
        );
        let t = row(&w, tank);
        assert_eq!(
            w.state.units.z[t],
            Fx::from_int(BANK as i32),
            "the tank is not on the deck"
        );
        let full = w.state.units.health[t];
        let mut fired = 0;
        for _ in 0..300 {
            w.tick(&[]).unwrap();
            fired += w.state.projectiles.len();
        }
        let left = w
            .state
            .units
            .row(tank)
            .map_or(Fx::ZERO, |r| w.state.units.health[r]);
        (full - left, fired)
    };
    let (under, shots) = hurt((1_020, ROAD), (1_300, ROAD));
    assert_eq!(under, Fx::ZERO, "the frigate shot up through the deck");
    assert_eq!(shots, 0, "the frigate wasted shots on the deck's underside");
    let (beside, _) = hurt((1_020, ROAD + 300), (1_100, ROAD + 40));
    assert!(
        beside > Fx::ZERO,
        "the frigate beside the bridge never hit the tank on it"
    );
}
