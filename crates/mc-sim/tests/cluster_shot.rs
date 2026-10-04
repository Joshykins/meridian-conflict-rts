//! Cluster shots (`Weapon::cluster`, `cluster.rs`): the Sower's Heavy Gravitic Seekers
//! climb on a high arc, break into sub-seekers on the way down and rain them over the
//! mark, each with its share of the damage; intercept lasers can take the pieces.
//! `cargo test -p mc-sim --test sim -- cluster_shot::`

use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const SOWER: &str = "regency_t2_bombard";
const CORONA: &str = "aster_t2_missile_defense";
const TARGET: &str = "aster_t2_land_factory";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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

fn spawn(w: &World, owner: u8, key: &str, x: i32, y: i32, flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, y),
            heading: Angle::from_degrees(180),
            count: 1,
            flags,
            build: 1000,
        },
    }
}

#[derive(Default, Debug)]
struct Volley {
    fired: usize,
    splits: Vec<(FxVec3, u8)>,
    hits: Vec<FxVec3>,
    lased: usize,
}

/// One Sower at `x` firing on a passive factory at x = 600 for `ticks`, with `coronas`
/// guarding the factory. What it fired, where its seekers split and where the pieces
/// landed, and the factory's health lost.
fn bombard(x: i32, coronas: usize, ticks: u32) -> (Volley, f64) {
    let mut w = world();
    let sower = w.blueprints.id_of(SOWER).unwrap();
    let target = w.blueprints.id_of(TARGET).unwrap();
    let mut spawns = vec![
        spawn(&w, 1, SOWER, x, 1000, flag::INVULNERABLE),
        spawn(&w, 0, TARGET, 600, 1000, flag::PASSIVE),
    ];
    for i in 0..coronas as i32 {
        spawns.push(spawn(&w, 0, CORONA, 660, 960 + 80 * i, flag::INVULNERABLE));
    }
    w.tick(&spawns).unwrap();
    let row = (0..w.state.units.owner.len())
        .find(|&r| w.state.units.blueprint[r] == target)
        .unwrap();
    let id = w.state.units.id(row);
    let full = w.state.units.health[row].to_f64();
    let mut v = Volley::default();
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::ShotFired { blueprint, .. } if *blueprint == sower => v.fired += 1,
                SimEvent::ClusterSplit {
                    pos,
                    count,
                    blueprint,
                    ..
                } if *blueprint == sower => v.splits.push((*pos, *count)),
                SimEvent::Impact { pos, blueprint, .. } if *blueprint == sower => v.hits.push(*pos),
                SimEvent::MissileLased { killed: true, .. } => v.lased += 1,
                _ => {}
            }
        }
    }
    let left = w
        .state
        .units
        .row(id)
        .map_or(0.0, |r| w.state.units.health[r].to_f64());
    (v, full - left)
}

#[test]
fn seekers_split_over_the_mark_and_rain_their_pieces_round_it() {
    // Two salvos of two (reload 9 s): the first is down, the second still flying.
    let (v, dealt) = bombard(1500, 0, 200);
    let w = world();
    let weapon = &w
        .blueprints
        .unit(w.blueprints.id_of(SOWER).unwrap())
        .weapons[0];
    let cluster = weapon.cluster.expect("the Sower's seekers split");
    println!("{v:?} dealt {dealt:.0}");
    assert_eq!(v.fired, 4, "two salvos");
    assert!(v.splits.len() >= 2, "each seeker splits");
    for &(pos, count) in &v.splits {
        assert_eq!(count, cluster.count);
        // High over the mark, coming down; the ground is at 20 m.
        let over = pos.z - Fx::from_int(20);
        assert!(
            over > Fx::from_int(100) && over <= cluster.height + Fx::from_int(30),
            "split at {over:?} m"
        );
        assert!(pos.xy().distance(FxVec2::from_ints(600, 1000)) < Fx::from_int(300));
    }
    let landed = v.hits.len() / cluster.count as usize;
    assert_eq!(
        v.hits.len() % cluster.count as usize,
        0,
        "every piece lands"
    );
    assert!(landed >= 2, "{landed} seekers came down");
    // The pieces land apart, over the target and round it.
    let centre = FxVec2::from_ints(600, 1000);
    let mut apart = 0;
    for (a, p) in v.hits.iter().enumerate() {
        assert!(
            p.xy().distance(centre) < Fx::from_int(120),
            "a piece landed far off at {p:?}"
        );
        apart += v.hits[a + 1..]
            .iter()
            .filter(|q| q.xy().distance(p.xy()) > Fx::from_int(6))
            .count();
    }
    let pairs = v.hits.len() * (v.hits.len() - 1) / 2;
    assert!(
        apart * 10 >= pairs * 8,
        "pieces bunched: {apart} of {pairs} pairs apart"
    );
    // Each piece carries its share: a volley never deals more than the weapon's damage
    // per seeker, and a factory-sized target takes most of it.
    let most = (weapon.damage * landed as i32).to_f64();
    assert!(dealt <= most + 1.0, "dealt {dealt} of at most {most}");
    assert!(dealt >= most * 0.5, "dealt only {dealt} of {most}");
}

#[test]
fn missile_defence_takes_seekers_and_their_pieces() {
    let (bare, _) = bombard(1500, 0, 200);
    let (guarded, _) = bombard(1500, 2, 200);
    println!("bare {bare:?}\nguarded {guarded:?}");
    assert!(guarded.lased > 0, "the lasers took nothing");
    assert!(
        guarded.hits.len() < bare.hits.len(),
        "{} pieces landed through two Coronas, {} with none",
        guarded.hits.len(),
        bare.hits.len()
    );
}

#[test]
fn sower_draws_cased_missiles_and_smaller_cluster_pieces() {
    use mc_sim::mirror::PROJECTILE_MISSILE;
    use mc_sim::RenderFrame;

    let mut w = world();
    let sower = w.blueprints.id_of(SOWER).unwrap();
    let caliber = w.blueprints.unit(sower).weapons[0].caliber;
    assert!(
        caliber > 0.0 && caliber * (0.165 / 0.14) < 0.60,
        "the body and folded fins must fit the launch cell"
    );
    w.tick(&[
        spawn(&w, 1, SOWER, 1500, 1000, flag::INVULNERABLE),
        spawn(&w, 0, TARGET, 600, 1000, flag::PASSIVE),
    ])
    .unwrap();
    let mut frame = RenderFrame::default();
    let (mut carrier, mut piece, mut landed_piece) = (false, false, false);
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        for p in &frame.projectiles {
            assert_ne!(p.color & PROJECTILE_MISSILE, 0, "solid missile geometry");
            assert_eq!(p._pad[0], 0.0, "no violet plasma orb");
            assert_eq!(p.aim[3], p.prev_aim[3]);
            if (p.aim[3] - caliber).abs() < 0.0001 {
                carrier = true;
            } else {
                assert!((p.aim[3] - caliber * 0.55).abs() < 0.0001);
                piece = true;
                landed_piece |= p.color >> 16 != 0;
            }
        }
    }
    assert!(
        carrier && piece && landed_piece,
        "carrier, split pieces and their last stretch"
    );
}
