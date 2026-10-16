//! Gravitic lenses (`Weapon::lens`, `lens.rs`): the Regency Lodestone plants, charges and
//! sends a front of gravity out to its target; where it strikes, a well opens on the target
//! and collapses at once, one crush of the weapon's damage on every enemy round it, never a
//! friend or an aircraft. Then the front is gone and the gun fires again after its reload.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};

const TPS: u32 = TICKS_PER_SECOND;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const CELLS: u32 = 256;
const LENS: &str = "regency_t3_gravitic_lens";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
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
        doodads: Default::default(),
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

/// `add`, never picking a target or firing.
fn passive(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let id = add(w, key, owner, x, y);
    let row = w.state.units.row(id).unwrap();
    w.state.units.flags[row] |= flag::PASSIVE;
    id
}

fn health(w: &World, id: UnitId) -> Fx {
    w.state
        .units
        .row(id)
        .map_or(Fx::ZERO, |r| w.state.units.health[r])
}

fn attack(w: &mut World, lens: UnitId, target: UnitId) {
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: vec![lens],
            target,
            queue: false,
        },
    }])
    .unwrap();
}

/// The lens's damage a strike and the reach of its crush round the target.
fn lens_weapon(w: &World, lens: UnitId) -> (Fx, Fx) {
    let row = w.state.units.row(lens).unwrap();
    let weapon = &w.blueprints.unit(w.state.units.blueprint[row]).weapons[0];
    (weapon.damage, weapon.lens.unwrap().radius)
}

/// Ticks `w` until a lens's front has struck, at most `limit` ticks, holding `pin` at its
/// point (an idle gunship drifts off); the ticks from its launch to its strike, if it
/// struck.
fn until_struck(w: &mut World, limit: u32, (pin, at): (UnitId, FxVec2)) -> Option<u32> {
    let mut launched = None;
    for tick in 0..limit {
        let row = w.state.units.row(pin).unwrap();
        w.state.units.pos[row] = at;
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
fn one_strike_crushes_all_round_its_target_and_spares_friends_and_aircraft() {
    let mut w = world();
    let lens = add(&mut w, LENS, 0, 400, 1000);
    // Walls, which stand where they are put (tanks this close push apart): the target, one
    // whose side is 8 m from it, one 24 m off, a friend's 8 m off; and a gunship over it.
    let all = [
        ("aster_wall", 1, 1400, 1000),
        ("aster_wall", 1, 1400, 1014),
        ("aster_wall", 1, 1400, 1030),
        ("regency_wall", 0, 1400, 986),
        ("aster_t2_gunship", 1, 1404, 1000),
    ]
    .map(|(key, owner, x, y)| passive(&mut w, key, owner, x, y));
    let (damage, radius) = lens_weapon(&w, lens);
    assert_eq!(radius, Fx::from_int(10));
    let before = all.map(|id| health(&w, id));
    attack(&mut w, lens, all[0]);
    // 1000 m at 700 m/s: the front runs a little under a second and a half.
    let pin = (all[4], FxVec2::from_ints(1404, 1000));
    let run = until_struck(&mut w, TPS * 20, pin).expect("the front never struck");
    assert!(
        (TPS * 10 / 7 - 1..=TPS * 10 / 7 + 2).contains(&run),
        "the front ran {run} ticks"
    );
    // The well collapsed with the strike: nothing is left of the front.
    assert!(w.state.lenses.is_empty(), "the front outlived its strike");
    let after = all.map(|id| health(&w, id));
    assert_eq!(before[0] - after[0], damage, "the target");
    assert_eq!(before[1] - after[1], damage, "a hull beside the target");
    assert_eq!(after[2], before[2], "a hull outside the well was struck");
    assert_eq!(after[3], before[3], "a friend in the well was struck");
    assert_eq!(after[4], before[4], "an aircraft over the well was struck");

    // Nothing lingers: no more crush and no slow once it has struck.
    for _ in 0..TPS * 2 {
        w.tick(&[]).unwrap();
    }
    let later = all.map(|id| health(&w, id));
    assert_eq!(later, after, "the well kept crushing after its strike");
}

#[test]
fn it_fires_again_once_its_reload_is_done() {
    let mut w = world();
    let lens = add(&mut w, LENS, 0, 400, 1000);
    let target = passive(&mut w, "aster_t2_tank", 1, 1400, 1000);
    // Struck and never harmed, so it stands to be shot again.
    let row = w.state.units.row(target).unwrap();
    w.state.units.flags[row] |= flag::INVULNERABLE;
    attack(&mut w, lens, target);
    let mut launches = Vec::new();
    let mut strikes = 0;
    for tick in 0..TPS * 30 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::LensLaunched { .. } => launches.push(tick),
                SimEvent::LensStruck { .. } => strikes += 1,
                _ => {}
            }
        }
        assert!(w.state.lenses.len() <= 1, "a second front with one out");
        if launches.len() == 3 {
            break;
        }
    }
    assert_eq!(launches.len(), 3, "it fired {launches:?}");
    assert!(strikes >= 2, "{strikes} strikes");
    // A shot every reload (6 s), the front's flight inside it.
    for pair in launches.windows(2) {
        let gap = pair[1] - pair[0];
        assert!(
            (TPS * 6..=TPS * 6 + 1).contains(&gap),
            "{gap} ticks between shots"
        );
    }
}

#[test]
fn a_front_whose_target_dies_on_the_way_strikes_nothing() {
    let mut w = world();
    let lens = add(&mut w, LENS, 0, 400, 1000);
    let target = passive(&mut w, "aster_t2_tank", 1, 1400, 1000);
    let beside = passive(&mut w, "aster_t2_tank", 1, 1400, 1014);
    attack(&mut w, lens, target);
    let mut out = false;
    for _ in 0..TPS * 20 {
        w.tick(&[]).unwrap();
        if w.events
            .iter()
            .any(|e| matches!(e, SimEvent::LensLaunched { .. }))
        {
            out = true;
            break;
        }
    }
    assert!(out, "the lens never fired");
    let full = health(&w, beside);
    let row = w.state.units.row(target).unwrap();
    w.state.units.health[row] = Fx::ZERO;
    for _ in 0..TPS * 2 {
        w.tick(&[]).unwrap();
        assert!(
            !w.events
                .iter()
                .any(|e| matches!(e, SimEvent::LensStruck { .. })),
            "a front struck with its target gone"
        );
    }
    assert!(w.state.lenses.is_empty(), "the front was kept");
    assert_eq!(health(&w, beside), full, "the hull beside it was struck");
}
