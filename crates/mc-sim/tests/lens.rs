//! Gravitic lenses (`Weapon::lens`, `lens.rs`): the Regency Lodestone plants, charges and
//! sends a front of gravity out to its target; where it strikes, a well opens on the target
//! that crushes and slows every enemy round it, never a friend, and lets go when the target
//! leaves its reach.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};

const TPS: u32 = TICKS_PER_SECOND;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const CELLS: u32 = 256;
const LENS: &str = "regency_t3_gravitic_lens";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let side = CELLS as usize + 1;
    let terrain = Heightfield::from_samples(
        CELLS,
        CELLS,
        vec![20; side * side],
        Fx::ZERO,
        Fx::ONE,
        Fx::ZERO,
    );
    let map = MapData {
        name: "lens".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
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
        seed: 5,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn health(w: &World, id: UnitId) -> Fx {
    w.state
        .units
        .row(id)
        .map_or(Fx::ZERO, |r| w.state.units.health[r])
}

fn order(player: u8, command: Command) -> PlayerCommand {
    PlayerCommand { player, command }
}

/// Ticks `w` until a lens's front has struck, at most `limit` ticks; the ticks from its
/// launch to its strike, if it struck.
fn until_struck(w: &mut World, limit: u32) -> Option<u32> {
    let mut launched = None;
    for tick in 0..limit {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::LensLaunched { .. } => launched = Some(tick),
                SimEvent::LensStruck { .. } => return launched.map(|l| tick - l),
                SimEvent::WeaponCharging { .. } if launched.is_some() => {
                    panic!("the lens charged again with its front out")
                }
                _ => {}
            }
        }
    }
    None
}

#[test]
fn the_well_crushes_all_round_its_target_and_spares_friends() {
    let mut w = world();
    let lens = add(&mut w, LENS, 0, 400, 1000);
    let target = add(&mut w, "aster_t2_tank", 1, 1400, 1000);
    let beside = add(&mut w, "aster_t1_tank", 1, 1400, 1020);
    let outside = add(&mut w, "aster_t1_tank", 1, 1400, 1080);
    let friend = add(&mut w, "regency_t1_tank", 0, 1380, 1000);
    // Nobody but the lens fires: hold the rest.
    for id in [target, beside, outside, friend] {
        w.tick(&[order(
            w.state.units.owner[w.state.units.row(id).unwrap()],
            Command::Stop { units: vec![id] },
        )])
        .unwrap();
    }
    w.tick(&[order(
        0,
        Command::Attack {
            units: vec![lens],
            target,
            queue: false,
        },
    )])
    .unwrap();
    // 1000 m at 400 m/s: the front runs two and a half seconds before it strikes.
    let run = until_struck(&mut w, TPS * 20).expect("the front never struck");
    assert!(
        (TPS * 5 / 2 - 1..=TPS * 5 / 2 + 1).contains(&run),
        "the front ran {run} ticks"
    );
    assert_eq!(w.state.lenses.len(), 1);
    assert_eq!(w.state.lenses[0].target, target);

    let before = [target, beside, outside, friend].map(|id| health(&w, id));
    for _ in 0..TPS {
        w.tick(&[]).unwrap();
    }
    let after = [target, beside, outside, friend].map(|id| health(&w, id));
    // Holding, it is not heard charging again.
    for _ in 0..TPS * 3 {
        w.tick(&[]).unwrap();
        assert!(
            !w.events
                .iter()
                .any(|e| matches!(e, SimEvent::WeaponCharging { .. })),
            "the lens charged again while its well held"
        );
    }
    assert!(after[0] < before[0], "the target took nothing");
    assert!(
        after[1] < before[1],
        "a hull beside the target took nothing"
    );
    assert_eq!(after[2], before[2], "a hull outside the well was struck");
    assert_eq!(after[3], before[3], "a friend in the well was struck");
}

#[test]
fn what_it_holds_keeps_a_fixed_share_of_its_speed_and_it_lets_go_out_of_reach() {
    let mut w = world();
    let lens = add(&mut w, LENS, 0, 400, 400);
    let target = add(&mut w, "aster_t3_assault_bot", 1, 1300, 400);
    w.tick(&[order(
        0,
        Command::Attack {
            units: vec![lens],
            target,
            queue: false,
        },
    )])
    .unwrap();
    assert!(
        until_struck(&mut w, TPS * 20).is_some(),
        "the front never struck"
    );
    let row = w.state.units.row(target).unwrap();
    let full = w
        .blueprints
        .unit(w.state.units.blueprint[row])
        .motion
        .unwrap()
        .speed;

    // It walks across the lens's front, staying in its reach.
    w.tick(&[order(
        1,
        Command::Move {
            units: vec![target],
            target: FxVec2::from_ints(1300, 1300),
            queue: false,
        },
    )])
    .unwrap();
    let mut seen = Vec::new();
    for t in 0..(TPS * 8) {
        w.tick(&[]).unwrap();
        let r = w.state.units.row(target).unwrap();
        if t % TPS == TPS - 1 {
            seen.push(w.state.units.speed[r]);
        }
    }
    // Up to its pace inside a couple of seconds, then held there: 40% of its full speed.
    let cap = full * Fx::ratio(2, 5);
    for &speed in &seen[2..] {
        assert!(speed <= cap, "it makes {speed:?} in the well, over {cap:?}");
        assert!(
            speed >= cap * Fx::ratio(9, 10),
            "it makes {speed:?}, short of {cap:?}"
        );
    }
    let r = w.state.units.row(target).unwrap();

    // Out of its reach, the link lets go.
    w.state.units.pos[r] = FxVec2::from_ints(1900, 1900);
    w.tick(&[]).unwrap();
    assert!(
        w.events
            .iter()
            .any(|e| matches!(e, SimEvent::LensReleased { .. })),
        "the lens held a target out of its reach"
    );
    assert!(w.state.lenses.is_empty());
}
