//! The Citadel, tech 3 rail point defence: it breaks heavies (Paladins) that walk into
//! it, where a Bastion cannot, and a swarm of light tanks outpaces its reload.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const CITADEL: &str = "aster_t3_point_defense";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 1000), FxVec2::from_ints(1700, 1000)],
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
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32, heading: i32) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(
            bp,
            owner,
            FxVec2::from_ints(x, y),
            Angle::from_degrees(heading),
            true,
        )
        .unwrap();
    w.state.units.id(row)
}

/// How a push of `count` `attacker`s, set off 900 m out on an attack-move through the
/// defence, ends against one `defense`: whether it stands, how many attackers are left
/// and the seconds it took.
struct Push {
    stands: bool,
    left: usize,
    seconds: u32,
    health: i32,
}

fn push(defense: &str, attacker: &str, count: usize) -> Push {
    let mut w = world();
    let gun = spawn(&mut w, defense, 0, 700, 1000, 0);
    let mut units = Vec::new();
    for i in 0..count as i32 {
        let (col, row) = (i / 6, i % 6);
        units.push(spawn(
            &mut w,
            attacker,
            1,
            1600 + 30 * col,
            1000 + 34 * (row - 3) + 17,
            180,
        ));
    }
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::AttackMove {
            units: units.clone(),
            target: FxVec2::from_ints(500, 1000),
            queue: false,
        },
    }])
    .unwrap();
    let alive = |w: &World| {
        units
            .iter()
            .filter(|&&u| w.state.units.row(u).is_some())
            .count()
    };
    let hz = mc_core::TICKS_PER_SECOND;
    let mut ticks = 0;
    while ticks < 240 * hz && w.state.units.row(gun).is_some() && alive(&w) > 0 {
        w.tick(&[]).unwrap();
        ticks += 1;
    }
    let health = w
        .state
        .units
        .row(gun)
        .map_or(0, |r| w.state.units.health[r].to_f64() as i32);
    Push {
        stands: w.state.units.row(gun).is_some(),
        left: alive(&w),
        seconds: ticks / hz,
        health,
    }
}

#[test]
fn a_citadel_breaks_a_paladin_push_that_a_bastion_cannot() {
    let citadel = push(CITADEL, "aster_t3_assault_bot", 2);
    let bastion = push("aster_t2_point_defense", "aster_t3_assault_bot", 1);
    assert!(
        citadel.stands && citadel.left == 0,
        "the Citadel fell to two Paladins ({} left, {} s)",
        citadel.left,
        citadel.seconds
    );
    assert!(!bastion.stands, "a Bastion held a Paladin");
    // Not a wall: three Paladins, about its own mass, take it.
    assert!(!push(CITADEL, "aster_t3_assault_bot", 3).stands);
}

/// Every slug kills a light tank outright and most of it is wasted, so a swarm does
/// better per mass than heavies do: a third more than its own mass overruns it.
#[test]
fn a_swarm_of_light_tanks_overruns_a_citadel() {
    let w = world();
    let mass = |key| {
        w.blueprints
            .unit(w.blueprints.id_of(key).unwrap())
            .cost_mass
            .to_f64()
    };
    let tanks = (mass(CITADEL) * 1.35 / mass("aster_t2_tank")) as usize;
    let swarm = push(CITADEL, "aster_t2_tank", tanks);
    assert!(
        !swarm.stands,
        "the Citadel held {tanks} Bulwarks ({} left)",
        swarm.left
    );
}

/// Each shot throws its spent cartridge straight out of the breech behind the gun, hard
/// enough to clear the keep: it comes down past the lot's corners, lies there, and is
/// worth nothing.
#[test]
fn a_citadel_throws_its_cartridges_out_behind_clear_of_the_keep() {
    let mut w = world();
    let gun = spawn(&mut w, CITADEL, 0, 700, 1000, 0);
    let mark = spawn(&mut w, "aster_t3_assault_bot", 1, 1150, 1000, 180);
    let r = w.state.units.row(mark).unwrap();
    w.state.units.flags[r] |= flag::INVULNERABLE | flag::PASSIVE;
    let casing = w.blueprints.id_of("aster_t3_citadel_casing").unwrap();
    for _ in 0..25 * mc_core::TICKS_PER_SECOND {
        w.tick(&[]).unwrap();
    }
    let at = w.state.units.pos[w.state.units.row(gun).unwrap()];
    let lying: Vec<_> = w.state.sabots.iter().filter(|c| c.lying > 0).collect();
    assert!(
        lying.len() >= 2,
        "{} cartridges lie by the gun",
        lying.len()
    );
    // The lot is 48 m square: its corners are 34 m out.
    for c in lying {
        let off = c.rest.xy() - at;
        assert!(
            off.x.to_f32() < -34.0 && off.y.to_f32().abs() < 8.0,
            "a cartridge lies at {:?} from the gun, not out behind it",
            (off.x.to_f32(), off.y.to_f32())
        );
    }
    assert!(
        w.state
            .wrecks
            .slots
            .iter()
            .all(|r| w.state.wrecks.blueprint[r] != casing),
        "a cartridge became a wreck"
    );
}

/// Once its mark is dead the gun stays laid where it last fired: it does not swing
/// back to face the way the keep does.
#[test]
fn a_citadel_leaves_its_gun_laid_where_it_last_fired() {
    let mut w = world();
    let gun = spawn(&mut w, CITADEL, 0, 700, 1000, 0);
    // A Bulwark off to the north-east dies to one slug.
    let mark = spawn(&mut w, "aster_t2_tank", 1, 1000, 1300, 180);
    let r = w.state.units.row(mark).unwrap();
    w.state.units.flags[r] |= flag::PASSIVE;
    let hz = mc_core::TICKS_PER_SECOND;
    let mut ticks = 0;
    while w.state.units.row(mark).is_some() {
        assert!(ticks < 30 * hz, "the Citadel never killed its mark");
        w.tick(&[]).unwrap();
        ticks += 1;
    }
    let g = w.state.units.row(gun).unwrap();
    let laid = (
        w.state.units.weapon_yaw[g][0],
        w.state.units.arm_pitch[g][0],
    );
    for _ in 0..20 * hz {
        w.tick(&[]).unwrap();
    }
    let yaw = w.state.units.weapon_yaw[g][0];
    assert!(
        Angle::ZERO.delta_to(yaw) > Angle::ZERO.delta_to(Angle::from_degrees(30)),
        "the gun was never turned to its mark"
    );
    assert_eq!(
        (yaw, w.state.units.arm_pitch[g][0]),
        laid,
        "the gun moved with nothing to shoot"
    );
}

#[test]
#[ignore = "prints push outcomes: cargo test -p mc-sim --test citadel -- --ignored --nocapture"]
fn zz_probe_citadel_pushes() {
    for (attacker, counts) in [
        ("aster_t3_assault_bot", &[1usize, 2, 3, 4, 5][..]),
        ("aster_t2_tank", &[4, 8, 12, 16][..]),
        ("aster_t3_sniper", &[2, 4, 6][..]),
    ] {
        for defense in [CITADEL, "aster_t2_point_defense"] {
            for &n in counts {
                let p = push(defense, attacker, n);
                println!(
                    "{defense:>24} vs {n:>2} {attacker:<22} stands {:<5} hp {:>6} left {:>2} in {:>3} s",
                    p.stands, p.health, p.left, p.seconds
                );
            }
        }
    }
}
