//! Bombing runs: lead on moving targets, release timing, map edges, and
//! fixed-wing formation flight without weaving.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
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

fn cmd(player: u8, command: Command) -> PlayerCommand {
    PlayerCommand { player, command }
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

fn f(x: Fx) -> f64 {
    x.to_f64()
}

#[derive(Default)]
struct Track {
    passes: u32,
    salvos: u32,
    prev_dist: f64,
    closing: bool,
    in_salvo: u32,
}

struct Report {
    passes: u32,
    salvos: u32,
    hits: u32,
    impacts: u32,
    miss: f64,
    along: f64,
}

/// `mover`: where the target drives, if anywhere. Returns per-bomber sums.
fn run(
    key: &str,
    starts: &[(i32, i32)],
    target: (i32, i32),
    mover: Option<(i32, i32)>,
    target_key: &str,
    ticks: u32,
    attack_move: bool,
) -> Report {
    let mut w = world();
    let rows: Vec<_> = starts
        .iter()
        .map(|&(x, y)| add(&mut w, key, 0, x, y))
        .collect();
    let t = add(&mut w, target_key, 1, target.0, target.1);
    w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
    let ids: Vec<_> = rows.iter().map(|&r| w.state.units.id(r)).collect();
    let mut cmds = vec![if attack_move {
        cmd(
            0,
            Command::AttackMove {
                units: ids,
                target: FxVec2::from_ints(target.0, target.1),
                queue: false,
            },
        )
    } else {
        cmd(
            0,
            Command::Attack {
                units: ids,
                target: w.state.units.id(t),
                queue: false,
            },
        )
    }];
    if let Some((x, y)) = mover {
        // Out and back, so the target is under way for the whole test.
        for leg in 0..6 {
            let to = if leg % 2 == 0 { (x, y) } else { target };
            cmds.push(cmd(
                1,
                Command::Move {
                    units: vec![w.state.units.id(t)],
                    target: FxVec2::from_ints(to.0, to.1),
                    queue: leg > 0,
                },
            ));
        }
    }
    w.tick(&cmds).unwrap();
    let bp = w.state.units.blueprint[rows[0]];
    let splash = f(w.blueprints.unit(bp).weapons[0].splash);
    let tr = f(w.blueprints.unit(w.state.units.blueprint[t]).radius);
    let mut tracks: Vec<Track> = rows
        .iter()
        .map(|_| Track {
            prev_dist: 1e9,
            ..Default::default()
        })
        .collect();
    let (mut hits, mut impacts, mut miss) = (0, 0, 0.0);
    let mut along = 0.0;
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
        let tp = w.state.units.pos[t];
        for e in &w.events {
            match e {
                SimEvent::Impact {
                    pos,
                    blueprint,
                    weapon: 0,
                    ..
                } if *blueprint == bp => {
                    impacts += 1;
                    let d = f(pos.xy().distance(tp));
                    miss += d;
                    along +=
                        f((pos.xy() - tp).dot(FxVec2::from_angle(w.state.units.heading[rows[0]])));
                    if d <= splash + tr {
                        hits += 1;
                    }
                }
                _ => {}
            }
        }
        for (i, &r) in rows.iter().enumerate() {
            let k = &mut tracks[i];
            let fired = w
                .events
                .iter()
                .filter(|e| {
                    matches!(e, SimEvent::ShotFired { blueprint, weapon: 0, pos, .. }
                if *blueprint == bp && pos.xy().distance(w.state.units.pos[r]) < Fx::from_int(30))
                })
                .count() as u32;
            if fired > 0 {
                if k.in_salvo == 0 {
                    k.salvos += 1;
                }
                k.in_salvo = 30;
            } else {
                k.in_salvo = k.in_salvo.saturating_sub(1);
            }
            let d = f(w.state.units.pos[r].distance(tp));
            if d < k.prev_dist {
                k.closing = true;
            } else if k.closing && d > k.prev_dist && d < 80.0 {
                k.passes += 1;
                k.closing = false;
            } else if d > k.prev_dist {
                k.closing = false;
            }
            k.prev_dist = d;
        }
    }
    Report {
        passes: tracks.iter().map(|k| k.passes).sum(),
        salvos: tracks.iter().map(|k| k.salvos).sum(),
        hits,
        impacts,
        miss: if impacts > 0 {
            miss / impacts as f64
        } else {
            -1.0
        },
        along: if impacts > 0 {
            along / impacts as f64
        } else {
            0.0
        },
    }
}

const BOMBERS: [&str; 3] = [
    "aster_t1_bomber",
    "aster_t2_fire_bomber",
    "aster_t3_strategic_bomber",
];

#[test]
fn the_middle_of_the_carpet_lands_on_a_standing_target() {
    for key in BOMBERS {
        let r = run(
            key,
            &[(1500, 2000)],
            (2000, 2000),
            None,
            "aster_t1_tank",
            1500,
            false,
        );
        assert!(
            r.salvos >= 5 && r.salvos + 1 >= r.passes,
            "{key}: {} salvos in {} passes",
            r.salvos,
            r.passes
        );
        // Less than a tick of flight either way; it used to be two ticks short.
        assert!(
            r.along.abs() < 9.0,
            "{key}: carpet centred {:.1} m along the run",
            r.along
        );
    }
    let r = run(
        "aster_t1_bomber",
        &[(1500, 2000)],
        (2000, 2000),
        None,
        "aster_t1_tank",
        1500,
        false,
    );
    assert!(
        r.hits * 4 >= r.impacts * 3,
        "light bomber hit with {} of {}",
        r.hits,
        r.impacts
    );
    let r = run(
        "aster_t3_strategic_bomber",
        &[(1500, 2000)],
        (2000, 2000),
        None,
        "aster_t1_tank",
        1500,
        false,
    );
    assert!(r.miss < 10.0, "strategic bomb missed by {:.1} m", r.miss);
}

#[test]
fn bombers_lead_a_target_that_is_driving() {
    for key in BOMBERS {
        for to in [(2000, 3400), (3000, 3000)] {
            let r = run(
                key,
                &[(1200, 2000)],
                (2000, 2000),
                Some(to),
                "aster_t1_tank",
                1500,
                false,
            );
            // Flying at where the tank is, a crossing target got one or two
            // salvos in five passes: the release line ran beside it.
            assert!(
                r.salvos >= 3,
                "{key} to {to:?}: {} salvos in {} passes",
                r.salvos,
                r.passes
            );
            assert!(
                r.salvos + 1 >= r.passes,
                "{key} to {to:?}: {} salvos in {} passes",
                r.salvos,
                r.passes
            );
        }
    }
    let r = run(
        "aster_t1_bomber",
        &[(1200, 2000)],
        (2000, 2000),
        Some((2000, 3400)),
        "aster_t1_tank",
        1500,
        false,
    );
    assert!(
        r.hits * 2 >= r.impacts,
        "light bomber hit a crossing tank with {} of {}",
        r.hits,
        r.impacts
    );
}

#[test]
fn every_pass_beside_a_map_edge_drops() {
    for key in BOMBERS {
        for target in [(400, 400), (2000, 150), (2000, 450)] {
            let r = run(
                key,
                &[(1200, 1200)],
                target,
                None,
                "aster_t1_tank",
                1500,
                false,
            );
            assert!(
                r.salvos >= 5,
                "{key} at {target:?}: {} salvos in {} passes",
                r.salvos,
                r.passes
            );
        }
    }
}

#[test]
fn a_flight_of_bombers_does_not_weave_on_a_move() {
    for key in BOMBERS {
        let mut w = world();
        let rows: Vec<_> = (0..8)
            .map(|i| add(&mut w, key, 0, 1200 + (i % 4) * 30, 2000 + (i / 4) * 30))
            .collect();
        let ids: Vec<_> = rows.iter().map(|&r| w.state.units.id(r)).collect();
        w.tick(&[cmd(
            0,
            Command::Move {
                units: ids,
                target: FxVec2::from_ints(3000, 3200),
                queue: false,
            },
        )])
        .unwrap();
        let mut prev: Vec<_> = rows.iter().map(|&r| w.state.units.heading[r]).collect();
        let mut last = vec![0i32; rows.len()];
        let mut reversals = 0;
        for _ in 0..1000 {
            w.tick(&[]).unwrap();
            for (i, &r) in rows.iter().enumerate() {
                let turn = prev[i].delta_to(w.state.units.heading[r]) as i32;
                prev[i] = w.state.units.heading[r];
                let sign = turn.signum() * (turn.abs() > 40) as i32;
                if sign != 0 {
                    reversals += (last[i] != 0 && sign != last[i]) as u32;
                    last[i] = sign;
                }
            }
        }
        // One turn onto the route and one to settle facing; chasing a slot a
        // few lengths ahead, eight light bombers reversed thirty-five times.
        assert!(
            reversals <= 8,
            "{key}: {reversals} reversals of turn among eight aircraft"
        );
        for &r in &rows {
            assert_eq!(
                w.state.units.flags[r] & flag::MOVING,
                0,
                "{key}: still flying"
            );
        }
    }
}
