//! The Colossus, the Regency's tech 5 walker: its lance burns for a few seconds and closes,
//! the burn takes a base-wide patch of the field, its twin guns fire on their own turrets
//! and its rockets fall scattered over the field.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const COLOSSUS: &str = "regency_t5_colossus";
const LANCE: u8 = 0;
const ROCKETS: u8 = 3;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "colossus".into(),
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

fn spawn(w: &World, owner: u8, key: &str, at: (i32, i32), flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(at.0, at.1),
            heading: Angle::ZERO,
            count: 1,
            flags,
            build: 1000,
        },
    }
}

/// The ticks one weapon of the Colossus fired on over the next `ticks`.
fn fired_on(world: &mut World, weapon: u8, ticks: u32) -> Vec<u32> {
    let colossus = world.blueprints.id_of(COLOSSUS).unwrap();
    let mut fired = Vec::new();
    for _ in 0..ticks {
        world.tick(&[]).unwrap();
        let tick = world.tick_count();
        for e in &world.events {
            if let SimEvent::ShotFired {
                blueprint,
                weapon: w,
                ..
            } = e
            {
                if *blueprint == colossus && *w == weapon {
                    fired.push(tick);
                }
            }
        }
    }
    fired
}

#[test]
fn it_is_a_shielded_striding_tech_5() {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(COLOSSUS).unwrap());
    assert_eq!(bp.tech, 5);
    assert!(bp.shield.is_some_and(|s| s.is_hull()), "a hull field");
    let motion = bp.motion.unwrap();
    assert!(
        motion.stride,
        "it steps over the ground and what stands on it"
    );
    assert_eq!(bp.weapons.len(), 4);
    assert!(bp.weapons[LANCE as usize].beam && bp.weapons[LANCE as usize].star_beam);
    assert!(
        bp.weapons[1].mount && bp.weapons[2].mount,
        "the guns turn on their own"
    );
    assert_eq!(bp.weapons[ROCKETS as usize].muzzles.len(), 8);
    assert!(
        bp.weapons.iter().all(|w| w.target_mask & cat::AIR == 0),
        "no anti-air: it wants an escort"
    );
}

/// The head opens through the charge, the beam burns its salvo of ticks without a break
/// and closes; the next burn waits out the reload.
#[test]
fn the_lance_burns_a_few_seconds_then_closes() {
    let mut w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(COLOSSUS).unwrap());
    let lance = &bp.weapons[LANCE as usize];
    let (salvo, delay) = (lance.salvo as u32, lance.salvo_delay_ticks as u32);
    assert!(lance.charge_ticks >= 20 && salvo * delay >= 30);
    w.tick(&[spawn(&w, 0, COLOSSUS, (500, 512), 0)]).unwrap();
    w.tick(&[spawn(
        &w,
        1,
        "aster_t3_assault_bot",
        (1000, 512),
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let burn = fired_on(&mut w, LANCE, 400);
    assert_eq!(
        burn.len(),
        salvo as usize,
        "one burn in the first 40 s: {burn:?}"
    );
    for pair in burn.windows(2) {
        assert!(
            pair[1] - pair[0] <= delay,
            "the beam never breaks: {burn:?}"
        );
    }
    let span = burn[burn.len() - 1] - burn[0];
    assert!(
        (30..=100).contains(&span),
        "burns a few seconds, not {span} ticks"
    );
}

/// A base's worth of tanks round the mark: the burn takes the lot, though only one is
/// aimed at.
#[test]
fn the_burn_takes_a_base_wide_patch() {
    let mut w = world();
    w.tick(&[spawn(&w, 0, COLOSSUS, (500, 512), 0)]).unwrap();
    let mark = (1000, 512);
    let ring = [
        (0, 0),
        (60, 0),
        (-60, 0),
        (0, 60),
        (0, -60),
        (50, 50),
        (-50, -50),
    ];
    for (dx, dy) in ring {
        w.tick(&[spawn(
            &w,
            1,
            "aster_t2_tank",
            (mark.0 + dx, mark.1 + dy),
            flag::PASSIVE,
        )])
        .unwrap();
    }
    for _ in 0..300 {
        w.tick(&[]).unwrap();
    }
    let u = &w.state.units;
    let tank = w.blueprints.id_of("aster_t2_tank").unwrap();
    let left = (0..u.blueprint.len())
        .filter(|&r| u.blueprint[r] == tank && u.health[r] > Fx::ZERO)
        .count();
    assert_eq!(left, 0, "{left} of the 7 tanks survived the burn");
}

/// The rockets leave in one ripple of eight, and come down scattered: a volley on one mark
/// does not land on one point.
#[test]
fn the_rockets_scatter() {
    let mut w = world();
    // Both on one tick, so the first rocket of the ripple is not missed by the log.
    w.tick(&[
        spawn(&w, 0, COLOSSUS, (500, 512), 0),
        spawn(
            &w,
            1,
            "aster_t3_assault_bot",
            (2100, 512),
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    let rocket_splash = Fx::from_int(55);
    let mut landed = Vec::new();
    let mut fired = Vec::new();
    let colossus = w.blueprints.id_of(COLOSSUS).unwrap();
    // The spawn tick's own events count too: the first rocket may leave on it.
    for k in 0..=200 {
        if k > 0 {
            w.tick(&[]).unwrap();
        }
        let tick = w.tick_count();
        for e in &w.events {
            match e {
                SimEvent::ShotFired {
                    blueprint, weapon, ..
                } if *blueprint == colossus && *weapon == ROCKETS => fired.push(tick),
                SimEvent::Impact { pos, splash, .. } if *splash == rocket_splash => {
                    landed.push(pos.xy());
                }
                _ => {}
            }
        }
    }
    assert_eq!(fired.len(), 8, "one volley of eight: {fired:?}");
    assert!(landed.len() >= 8, "{} rockets landed", landed.len());
    let far = landed
        .iter()
        .flat_map(|a| landed.iter().map(|b| a.distance(*b)))
        .fold(Fx::ZERO, Fx::max);
    assert!(
        far > Fx::from_int(40),
        "they fall apart, not on one point: {far:?}"
    );
}

/// Both guns fire under the head, taking turns.
#[test]
fn the_twin_guns_take_turns() {
    let mut w = world();
    w.tick(&[spawn(&w, 0, COLOSSUS, (500, 512), 0)]).unwrap();
    w.tick(&[spawn(
        &w,
        1,
        "aster_t3_assault_bot",
        (1100, 512),
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let left = fired_on(&mut w, 1, 200);
    let mut w2 = world();
    w2.tick(&[spawn(&w2, 0, COLOSSUS, (500, 512), 0)]).unwrap();
    w2.tick(&[spawn(
        &w2,
        1,
        "aster_t3_assault_bot",
        (1100, 512),
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let right = fired_on(&mut w2, 2, 200);
    assert!(left.len() >= 5 && right.len() >= 5, "{left:?} {right:?}");
    assert!(left.iter().all(|t| !right.contains(t)), "never on one tick");
}

/// Durable structures across the footprint must be destroyed during the first burn,
/// while a control structure outside the footprint survives that burn.
#[test]
fn first_burn_demolishes_a_spread_out_base() {
    let mut w = world();
    w.tick(&[spawn(&w, 0, COLOSSUS, (500, 512), 0)]).unwrap();
    w.tick(&[spawn(
        &w,
        1,
        "aster_t3_assault_bot",
        (1100, 512),
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let key = "aster_t3_land_factory";
    for at in [(1100, 352), (1100, 672), (1250, 512), (1450, 850)] {
        w.tick(&[spawn(&w, 1, key, at, flag::PASSIVE)]).unwrap();
    }
    for _ in 0..160 {
        w.tick(&[]).unwrap();
    }
    let u = &w.state.units;
    let factory = w.blueprints.id_of(key).unwrap();
    let survivors = (0..u.blueprint.len())
        .filter(|&r| u.blueprint[r] == factory && u.health[r] > Fx::ZERO)
        .count();
    assert_eq!(
        survivors, 1,
        "the three buildings in the burn must fall; the distant control survives"
    );
}
