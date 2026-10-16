//! Repeat on a unit raised on its own lot (`site_repeat.rs`): once it is done and has
//! driven off, the lot is laid again; a builder with only it queued takes up the next,
//! and a site that dies takes its Repeat with it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{BlueprintId, Blueprints};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind};
use mc_sim::world::{snap_to_build_grid, MapData};
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, Refusal, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const FULGUR: &str = "aster_t4_assault_tank";
const LOT: FxVec2 = FxVec2::from_ints(1100, 1000);

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        doodads: Default::default(),
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(3500, 3500)],
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
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn fulgur(w: &World) -> BlueprintId {
    w.blueprints.id_of(FULGUR).unwrap()
}

/// Ticks until `done` holds, at most `limit`; whether it did.
fn run_until(w: &mut World, limit: usize, done: impl Fn(&World) -> bool) -> bool {
    for _ in 0..limit {
        if done(w) {
            return true;
        }
        w.tick(&[]).unwrap();
    }
    done(w)
}

/// The Fulgur sites standing, and the Fulgurs finished.
fn fulgurs(w: &World) -> (Vec<usize>, Vec<usize>) {
    let bp = fulgur(w);
    let u = &w.state.units;
    u.slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp)
        .partition(|&r| u.has_flag(r, flag::UNDER_CONSTRUCTION))
}

/// A Mason III lays a Fulgur's lot, the site is put on Repeat, and it is finished on free
/// build. `then`: where the engineer is queued to move after the Fulgur, if anywhere.
fn raise_on_repeat(w: &mut World, then: Option<FxVec2>) -> (UnitId, UnitId) {
    let engineer = w
        .spawn_unit(
            w.blueprints.id_of("aster_t3_engineer").unwrap(),
            0,
            FxVec2::from_ints(1000, 1000),
            Angle::ZERO,
            true,
        )
        .unwrap();
    let engineer = w.state.units.id(engineer);
    let bp = fulgur(w);
    let mut first = vec![
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
        cmd(Command::Build {
            units: vec![engineer],
            blueprint: bp,
            pos: LOT,
            heading: w.blueprints.unit(bp).build_heading(),
            queue: false,
        }),
    ];
    first.extend(then.map(|pos| {
        cmd(Command::Move {
            units: vec![engineer],
            target: pos,
            queue: true,
        })
    }));
    w.tick(&first).unwrap();
    assert!(
        run_until(w, 600, |w| !fulgurs(w).0.is_empty()),
        "the engineer never began the site"
    );
    let site = w.state.units.id(fulgurs(w).0[0]);
    w.tick(&[
        cmd(Command::SetRepeat {
            factories: vec![site],
            repeat: true,
        }),
        cmd(Command::DebugSetBuild {
            units: vec![site],
            permille: 999,
        }),
    ])
    .unwrap();
    let row = w.state.units.row(site).unwrap();
    assert!(
        w.state.units.has_flag(row, flag::REPEAT),
        "Repeat did not take"
    );
    (engineer, site)
}

#[test]
fn the_lot_is_laid_again_and_its_only_builder_takes_it_up() {
    let mut w = world();
    let (engineer, first) = raise_on_repeat(&mut w, None);
    assert!(
        run_until(&mut w, 600, |w| fulgurs(w).1.len() == 1
            && fulgurs(w).0.len() == 1),
        "the finished Fulgur's lot was never laid again"
    );
    let (sites, done) = fulgurs(&w);
    let u = &w.state.units;
    assert_eq!(u.id(done[0]), first, "the finished one is the first site");
    assert!(
        !u.has_flag(done[0], flag::REPEAT),
        "the finished unit keeps no Repeat"
    );
    let next = sites[0];
    let lot = snap_to_build_grid(w.blueprints.unit(fulgur(&w)), LOT);
    assert_eq!(u.pos[next], lot, "laid on the same lot");
    assert!(u.has_flag(next, flag::REPEAT), "the next site repeats too");
    assert!(
        u.pos[done[0]].distance(LOT) > Fx::from_int(30),
        "the finished Fulgur was not sent off its lot"
    );
    // The engineer had only the Fulgur queued: it works on the next.
    let next = u.id(next);
    assert!(
        run_until(&mut w, 300, |w| {
            let u = &w.state.units;
            u.row(engineer).is_some_and(|e| u.build_target[e] == next)
        }),
        "the engineer did not take up the next site"
    );
}

#[test]
fn with_other_work_queued_the_builder_leaves_the_next_site_unworked() {
    let mut w = world();
    let (engineer, _) = raise_on_repeat(&mut w, Some(FxVec2::from_ints(700, 1000)));
    assert!(
        run_until(&mut w, 600, |w| fulgurs(w).0.len() == 1
            && fulgurs(w).1.len() == 1),
        "the lot was never laid again"
    );
    let next = w.state.units.id(fulgurs(&w).0[0]);
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        let e = u.row(engineer).unwrap();
        assert_ne!(u.build_target[e], next, "the engineer went back to the lot");
        assert!(
            w.state
                .orders
                .iter(u, e)
                .all(|o| o.kind != OrderKind::Build),
            "the engineer was given the next site"
        );
    }
    let row = w.state.units.row(next).unwrap();
    assert!(w.state.units.has_flag(row, flag::UNDER_CONSTRUCTION));
}

#[test]
fn a_repeating_site_that_dies_is_not_laid_again() {
    let mut w = world();
    let (_, site) = raise_on_repeat(&mut w, None);
    // Killed half built, the way an enemy's guns would.
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![site],
        permille: 1000,
    })])
    .unwrap();
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        assert!(fulgurs(&w).0.is_empty() && fulgurs(&w).1.is_empty());
    }
    assert!(w.state.site_repeats.waiting.is_empty());
}

#[test]
fn a_lot_held_by_an_enemy_gives_up_and_says_so() {
    let mut w = world();
    let (_, site) = raise_on_repeat(&mut w, None);
    assert!(
        run_until(&mut w, 300, |w| w.state.units.row(site).is_some_and(|r| {
            !w.state.units.has_flag(r, flag::UNDER_CONSTRUCTION)
        })),
        "the Fulgur was never finished"
    );
    // An enemy parks on the lot and stays: nothing sends it off.
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of("aster_t1_tank").unwrap(),
            pos: LOT,
            heading: Angle::ZERO,
            count: 1,
            flags: flag::PASSIVE | flag::INVULNERABLE,
            build: 1000,
        },
    }])
    .unwrap();
    let mut refused = false;
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        refused |= w.events.iter().any(|e| {
            matches!(
                e,
                SimEvent::CommandRefused {
                    player: 0,
                    reason: Refusal::RepeatBlocked
                }
            )
        });
    }
    assert!(refused, "the blocked repeat was never given up");
    assert!(fulgurs(&w).0.is_empty(), "a site was laid under the enemy");
    assert!(w.state.site_repeats.waiting.is_empty());
}
