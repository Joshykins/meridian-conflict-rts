//! Air is the Behemoth's counter (`aster_t5_titan`, `docs/BALANCE.md`): it outranges the
//! ground, but it has no anti-air at all, and a wing of assault aircraft brings it down.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const TITAN: &str = "aster_t5_titan";
const STRAFER: &str = "aster_t3_assault_aircraft";

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
        seed: 5,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // The titan's side can run its field.
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::DebugFreeBuild {
            player: 0,
            on: true,
        },
    }])
    .unwrap();
    w
}

/// A titan at the middle of the map, and `n` assault aircraft 2.5 km off sent at it.
fn raid(w: &mut World, n: i32) -> (UnitId, Vec<UnitId>) {
    let titan = w.blueprints.id_of(TITAN).unwrap();
    let r = w
        .spawn_unit(titan, 0, FxVec2::from_ints(4000, 4000), Angle::ZERO, true)
        .unwrap();
    let titan = w.state.units.id(r);
    let strafer = w.blueprints.id_of(STRAFER).unwrap();
    let altitude = w.blueprints.unit(strafer).motion.unwrap().altitude;
    let wing: Vec<_> = (0..n)
        .map(|i| {
            let at = FxVec2::from_ints(1500 + (i % 8) * 40, 3900 + (i / 8) * 40);
            let r = w.spawn_unit(strafer, 1, at, Angle::ZERO, true).unwrap();
            w.state.units.z[r] = Fx::from_int(20) + altitude;
            w.state.units.id(r)
        })
        .collect();
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::Attack {
            units: wing.clone(),
            target: titan,
            queue: false,
        },
    }])
    .unwrap();
    (titan, wing)
}

/// The Thunderhead's rotary cannon is fixed to its nose, laid down the glide of its
/// pass. Aimed at the titan's middle, 240 m up, it never once bore and never fired;
/// aircraft strike a giant low, where their guns point.
#[test]
fn a_strafing_cannon_fires_on_a_titan() {
    let mut w = world();
    let (titan, _) = raid(&mut w, 2);
    let r = w.state.units.row(titan).unwrap();
    w.state.units.flags[r] |= flag::PASSIVE;
    let strafer = w.blueprints.id_of(STRAFER).unwrap();
    let mut shots = 0;
    for _ in 0..40 * TICKS_PER_SECOND {
        w.tick(&[]).unwrap();
        shots += w
            .events
            .iter()
            .filter(|e| {
                matches!(e, SimEvent::ShotFired { blueprint, weapon: 0, owner: 1, .. }
                    if *blueprint == strafer)
            })
            .count();
    }
    assert!(
        shots > 100,
        "the cannon fired {shots} rounds on the titan in 40 s"
    );
}

/// Forty Thunderheads, under half the titan's materials, bring it down, and none is lost: it has no anti-air.
#[test]
fn an_air_wing_brings_a_titan_down() {
    let mut w = world();
    let (titan, wing) = raid(&mut w, 40);
    for t in 0..240 * TICKS_PER_SECOND {
        w.tick(&[]).unwrap();
        if w.state.units.row(titan).is_none() {
            let lost = wing
                .iter()
                .filter(|&&id| w.state.units.row(id).is_none())
                .count();
            assert!(
                lost == 0,
                "the titan fell at {} s but took {lost} of 40 with it",
                t / 10
            );
            return;
        }
    }
    let r = w.state.units.row(titan).unwrap();
    panic!(
        "the titan still stands after 240 s at {} health",
        w.state.units.health[r].to_f32()
    );
}
