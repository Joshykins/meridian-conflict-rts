//! Crowds of mixed sizes: big hulls push small ones aside and not the other
//! way round, blocks crossing each other slip through, and a formation packs
//! each size at its own spacing instead of the largest member's.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "crowd".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(200, 200), FxVec2::from_ints(1800, 1800)],
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

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// Spawn `key` at each point, all facing `heading`; returns their ids in order.
fn spawn_at(w: &mut World, key: &str, points: &[(i32, i32)], heading: Angle) -> Vec<UnitId> {
    let bp = w.blueprints.id_of(key).unwrap();
    let before: Vec<_> = w.state.units.slots.iter().collect();
    let commands: Vec<_> = points
        .iter()
        .map(|&(x, y)| {
            cmd(Command::DebugSpawn {
                owner: 0,
                blueprint: bp,
                pos: FxVec2::from_ints(x, y),
                heading,
                count: 1,
                flags: flag::PASSIVE,
                build: 1000,
            })
        })
        .collect();
    w.tick(&commands).unwrap();
    w.state
        .units
        .slots
        .iter()
        .filter(|r| !before.contains(r) && w.state.units.blueprint[*r] == bp)
        .map(|r| w.state.units.id(r))
        .collect()
}

fn grid(x0: i32, y0: i32, cols: i32, count: i32, step: i32) -> Vec<(i32, i32)> {
    (0..count)
        .map(|i| (x0 + (i % cols) * step, y0 + (i / cols) * step))
        .collect()
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).unwrap()
}

fn radius(w: &World, id: UnitId) -> Fx {
    w.blueprints.unit(w.state.units.blueprint[row(w, id)]).radius
}

fn busy(w: &World, ids: &[UnitId]) -> usize {
    ids.iter()
        .filter(|&&id| w.state.orders.front(&w.state.units, row(w, id)).is_some())
        .count()
}

/// Ticks until every one of `ids` has finished its order, or `limit`.
fn run_until_idle(w: &mut World, ids: &[UnitId], limit: u32) -> u32 {
    for t in 0..limit {
        if busy(w, ids) == 0 {
            return t;
        }
        w.tick(&[]).unwrap();
    }
    limit
}

fn move_to(w: &mut World, ids: &[UnitId], x: i32, y: i32) {
    w.tick(&[cmd(Command::Move {
        units: ids.to_vec(),
        target: FxVec2::from_ints(x, y),
        queue: false,
    })])
    .unwrap();
}

#[test]
fn a_fulgur_drives_through_parked_tanks_on_its_line() {
    let mut w = world();
    let tanks = spawn_at(&mut w, "aster_t1_tank", &grid(560, 440, 10, 60, 14), Angle::ZERO);
    let fulgur = spawn_at(&mut w, "aster_t4_assault_tank", &[(300, 512)], Angle::ZERO)[0];
    move_to(&mut w, &[fulgur], 1200, 512);
    let mut wander = Fx::ZERO;
    let mut ticks = 0;
    for t in 0..1200 {
        if busy(&w, &[fulgur]) == 0 {
            ticks = t;
            break;
        }
        w.tick(&[]).unwrap();
        let y = w.state.units.pos[row(&w, fulgur)].y;
        wander = wander.max((y - Fx::from_int(512)).abs());
    }
    let alone = {
        let mut w = world();
        let fulgur = spawn_at(&mut w, "aster_t4_assault_tank", &[(300, 512)], Angle::ZERO)[0];
        move_to(&mut w, &[fulgur], 1200, 512);
        run_until_idle(&mut w, &[fulgur], 1200)
    };
    println!("fulgur through tanks: {ticks} ticks (alone {alone}), strayed {wander:?} m off its line");
    assert!(ticks > 0, "the Fulgur never got through");
    assert!(wander < Fx::from_int(6), "the tanks pushed the Fulgur {wander:?} m off its line");
    assert!(ticks * 10 < alone * 13, "{ticks} ticks through the tanks against {alone} alone");
    // What it drove through got out of the way rather than sitting inside it.
    let f = row(&w, fulgur);
    for &t in &tanks {
        let r = row(&w, t);
        let gap = w.state.units.pos[r].distance(w.state.units.pos[f]);
        assert!(gap >= radius(&w, t) + radius(&w, fulgur) - Fx::ONE, "a tank sits inside the Fulgur");
    }
}

#[test]
fn tanks_sent_through_a_parked_fulgur_flow_round_it() {
    let mut w = world();
    let fulgur = spawn_at(&mut w, "aster_t4_assault_tank", &[(700, 512)], Angle::ZERO)[0];
    let start = w.state.units.pos[row(&w, fulgur)];
    let tanks = spawn_at(&mut w, "aster_t1_tank", &grid(420, 440, 6, 36, 14), Angle::ZERO);
    move_to(&mut w, &tanks, 700, 512);
    run_until_idle(&mut w, &tanks, 900);
    move_to(&mut w, &tanks, 1000, 512);
    let ticks = run_until_idle(&mut w, &tanks, 1500);
    let shoved = w.state.units.pos[row(&w, fulgur)].distance(start);
    println!("tanks round a parked fulgur: shoved {shoved:?} m, {ticks} ticks, {} still busy", busy(&w, &tanks));
    assert!(shoved < Fx::from_int(3), "tanks shoved the parked Fulgur {shoved:?} m");
}

#[test]
fn two_blocks_pass_through_each_other_head_on() {
    let crossing = |both: bool| {
        let mut w = world();
        let east = spawn_at(&mut w, "aster_t1_tank", &grid(300, 440, 5, 25, 14), Angle::ZERO);
        let west = if both {
            spawn_at(&mut w, "aster_t1_tank", &grid(1000, 446, 5, 25, 14), Angle(0x8000))
        } else {
            Vec::new()
        };
        move_to(&mut w, &east, 1060, 480);
        if both {
            move_to(&mut w, &west, 330, 480);
        }
        let all: Vec<_> = east.iter().chain(&west).copied().collect();
        let t = run_until_idle(&mut w, &all, 3000);
        (t, busy(&w, &all))
    };
    let (alone, _) = crossing(false);
    let (both, left) = crossing(true);
    println!("head-on crossing: {both} ticks (one block alone {alone}), {left} still busy");
    assert_eq!(left, 0, "{left} tanks never got past the other block");
    assert!(both * 10 < alone * 14, "crossing took {both} ticks against {alone} alone");
}

#[test]
fn a_block_turned_across_another_keeps_moving() {
    // Two blocks pass through each other; mid-crossing one is sent off at a
    // right angle and must shoulder out through the other, not grind to a halt.
    let mut w = world();
    let east = spawn_at(&mut w, "aster_t1_tank", &grid(300, 440, 5, 25, 14), Angle::ZERO);
    let west = spawn_at(&mut w, "aster_t1_tank", &grid(900, 446, 5, 25, 14), Angle(0x8000));
    move_to(&mut w, &east, 1000, 480);
    move_to(&mut w, &west, 250, 480);
    for _ in 0..150 {
        w.tick(&[]).unwrap();
    }
    move_to(&mut w, &east, 640, 900);
    let all: Vec<_> = east.iter().chain(&west).copied().collect();
    let t = run_until_idle(&mut w, &all, 3000);
    println!("turned mid-crossing: {t} ticks, {} still busy", busy(&w, &all));
    assert_eq!(busy(&w, &all), 0);
}

/// Distance from each small member's slot to the nearest other small slot.
fn slot_spacing(w: &World, ids: &[UnitId], key: &str) -> (Fx, Fx) {
    let bp = w.blueprints.id_of(key).unwrap();
    let slots: Vec<_> = ids
        .iter()
        .map(|&id| row(w, id))
        .filter(|&r| w.state.units.blueprint[r] == bp)
        .map(|r| w.state.orders.front(&w.state.units, r).unwrap().offset)
        .collect();
    let mut near: Vec<Fx> = slots
        .iter()
        .enumerate()
        .map(|(i, a)| {
            slots
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, b)| a.distance(*b))
                .min()
                .unwrap()
        })
        .collect();
    near.sort();
    (near[0], near[near.len() / 2])
}

fn mixed_block(big: &str) {
    let mut w = world();
    let tanks = spawn_at(&mut w, "aster_t1_tank", &grid(300, 400, 6, 24, 16), Angle::ZERO);
    let alone_spacing = {
        let mut w = world();
        let tanks = spawn_at(&mut w, "aster_t1_tank", &grid(300, 400, 6, 24, 16), Angle::ZERO);
        move_to(&mut w, &tanks, 900, 600);
        slot_spacing(&w, &tanks, "aster_t1_tank").1
    };
    let heavy = spawn_at(&mut w, big, &[(250, 450)], Angle::ZERO)[0];
    let all: Vec<_> = tanks.iter().copied().chain([heavy]).collect();
    move_to(&mut w, &all, 900, 600);
    let (least, median) = slot_spacing(&w, &all, "aster_t1_tank");
    println!("{big} + 24 tanks: tank slots {least:?}..{median:?} m apart (tanks alone {alone_spacing:?})");
    assert!(median <= alone_spacing + Fx::ONE, "{big} spreads the tanks to {median:?} m");
    // Nobody's slot sits inside another's hull.
    let orders: Vec<_> = all
        .iter()
        .map(|&id| (radius(&w, id), w.state.orders.front(&w.state.units, row(&w, id)).unwrap().offset))
        .collect();
    for (i, &(ra, a)) in orders.iter().enumerate() {
        for &(rb, b) in &orders[i + 1..] {
            assert!(a.distance(b) >= ra + rb, "slots {a:?} and {b:?} overlap");
        }
    }
    let t = run_until_idle(&mut w, &all, 2000);
    println!("  arrived in {t} ticks, {} busy", busy(&w, &all));
    assert_eq!(busy(&w, &all), 0);
}

#[test]
fn a_paladin_does_not_spread_a_block_of_tanks() {
    mixed_block("aster_t3_assault_bot");
}

#[test]
fn a_fulgur_does_not_spread_a_block_of_tanks() {
    mixed_block("aster_t4_assault_tank");
}

/// A mixed block sent back the way it came: every tank's rank is now on the
/// far side of a heavy. Each has to go round it, not press into it for good.
fn about_turn(big: &str, heavies: i32) -> (u32, usize) {
    let mut w = world();
    let tanks = spawn_at(&mut w, "aster_t1_tank", &grid(300, 380, 6, 30, 16), Angle::ZERO);
    let points: Vec<_> = (0..heavies).map(|i| (230, 400 + i * 90)).collect();
    let heavy = spawn_at(&mut w, big, &points, Angle::ZERO);
    let all: Vec<_> = tanks.iter().chain(&heavy).copied().collect();
    move_to(&mut w, &all, 900, 500);
    run_until_idle(&mut w, &all, 2500);
    move_to(&mut w, &all, 300, 520);
    let t = run_until_idle(&mut w, &all, 2500);
    let left = busy(&w, &all);
    println!("{big} x{heavies} about turn: {t} ticks, {left} busy");
    (t, left)
}

#[test]
fn tanks_find_their_way_round_heavies_in_their_own_block() {
    for (big, n) in [("aster_t4_assault_tank", 2), ("aster_t3_assault_bot", 3)] {
        let (t, left) = about_turn(big, n);
        assert_eq!(left, 0, "{left} units never reached their ranks behind the {big}s");
        assert!(t < 1500, "{t} ticks to turn the block round");
    }
}

/// Ranks nobody stands on once the block has finished its order: members
/// that gave up pressed against another hull short of the block. Ranks are
/// traded on the march, so any member of the right size may hold one.
fn short_of_ranks(w: &mut World, ids: &[UnitId], x: i32, y: i32) -> (u32, Vec<(FxVec2, Fx)>) {
    move_to(w, ids, x, y);
    let slots: Vec<_> = ids
        .iter()
        .map(|&id| {
            let r = row(w, id);
            let o = *w.state.orders.front(&w.state.units, r).unwrap();
            (o.pos + o.offset, radius(w, id))
        })
        .collect();
    let t = run_until_idle(w, ids, 3000);
    let empty = slots
        .into_iter()
        .filter(|&(slot, r)| {
            !ids.iter().any(|&id| {
                radius(w, id) == r && w.state.units.pos[row(w, id)].distance(slot) <= r * 2
            })
        })
        .collect();
    (t, empty)
}

#[test]
fn a_mixed_block_marching_about_reaches_every_rank() {
    let mut w = world();
    let tanks = spawn_at(&mut w, "aster_t1_tank", &grid(400, 400, 8, 40, 16), Angle::ZERO);
    let heavy = spawn_at(
        &mut w,
        "aster_t3_assault_bot",
        &[(340, 420), (340, 470), (340, 520), (300, 445)],
        Angle::ZERO,
    );
    let bulwarks = spawn_at(&mut w, "aster_t2_tank", &grid(300, 560, 4, 8, 20), Angle::ZERO);
    let all: Vec<_> = tanks.iter().chain(&heavy).chain(&bulwarks).copied().collect();
    let mut total = 0;
    for &(x, y) in &[(1000, 500), (1000, 1100), (400, 700), (900, 300), (500, 1200), (1300, 900)] {
        let (t, short) = short_of_ranks(&mut w, &all, x, y);
        println!("to ({x},{y}): {t} ticks, {} ranks left empty {short:?}", short.len());
        total += short.len();
    }
    assert_eq!(total, 0, "{total} ranks left empty: members gave up short of them");
}

/// Tick the world once and count members under orders that made no headway,
/// pressed on a hull more than twice their size: a tank whose rank lies past
/// a Paladin must go round it, not lean on it.
fn tick_pinned(w: &mut World, ids: &[UnitId]) -> usize {
    let before: Vec<_> = ids.iter().map(|&id| w.state.units.pos[row(w, id)]).collect();
    w.tick(&[]).unwrap();
    ids.iter()
        .zip(before)
        .map(|(&id, was)| (row(w, id), was))
        .filter(|&(r, was)| {
            w.state.orders.front(&w.state.units, r).is_some()
                && w.state.units.pos[r].distance(was) < Fx::ratio(1, 10)
                && ids.iter().map(|&o| row(w, o)).any(|o| {
                    let (ra, rb) = (
                        w.blueprints.unit(w.state.units.blueprint[r]).radius,
                        w.blueprints.unit(w.state.units.blueprint[o]).radius,
                    );
                    rb > ra * 2
                        && w.state.units.pos[r].distance(w.state.units.pos[o]) < ra + rb + Fx::from_int(3)
                })
        })
        .count()
}

#[test]
fn tanks_turned_mid_march_go_round_the_heavies() {
    for big in ["aster_t3_assault_bot", "aster_t4_assault_tank"] {
        let mut w = world();
        let tanks = spawn_at(&mut w, "aster_t1_tank", &grid(400, 400, 8, 40, 16), Angle::ZERO);
        let heavy = spawn_at(&mut w, big, &[(330, 430), (330, 520)], Angle::ZERO);
        let all: Vec<_> = tanks.iter().chain(&heavy).copied().collect();
        let mut pinned = 0;
        for &(x, y) in &[(1200, 500), (700, 1300), (300, 300), (1300, 1200), (400, 1300)] {
            move_to(&mut w, &all, x, y);
            for _ in 0..120 {
                pinned += tick_pinned(&mut w, &all);
            }
        }
        let t = run_until_idle(&mut w, &all, 3000);
        println!("{big}: {pinned} member-ticks pinned on a heavy, finished in {t}");
        assert!(pinned < 200, "{big}: {pinned} member-ticks pinned on a heavy");
    }
}

#[test]
fn a_tank_block_slips_past_a_column_of_heavies_crossing_it() {
    for big in ["aster_t3_assault_bot", "aster_t4_assault_tank"] {
        let mut w = world();
        let tanks = spawn_at(&mut w, "aster_t1_tank", &grid(300, 600, 6, 30, 16), Angle::ZERO);
        let heavy = spawn_at(&mut w, big, &grid(620, 380, 2, 6, 70), Angle(0x4000));
        move_to(&mut w, &tanks, 1100, 640);
        move_to(&mut w, &heavy, 655, 1300);
        let mut pinned = 0;
        let everyone: Vec<_> = tanks.iter().chain(&heavy).copied().collect();
        for _ in 0..1500 {
            if busy(&w, &tanks) == 0 {
                break;
            }
            pinned += tick_pinned(&mut w, &everyone);
        }
        let t = run_until_idle(&mut w, &tanks, 1);
        println!("{big} column: {pinned} member-ticks pinned, {} tanks busy", busy(&w, &tanks));
        let _ = t;
        assert!(pinned < 150, "{big}: {pinned} member-ticks pinned on a heavy");
    }
}

/// Member-seconds spent glued to a heavy: under orders, touching a hull more
/// than twice its size for three seconds on end while its rank is elsewhere.
/// A tank chasing its rank past a Fulgur rides on its flank, still moving,
/// never getting there.
fn glued_probe(seed: u32, leg_ticks: u32) -> u32 {
    let mut w = world();
    let mut all = Vec::new();
    for (i, (key, n)) in [
        ("aster_t1_tank", 30),
        ("aster_t1_scout", 10),
        ("aster_t1_artillery", 10),
        ("aster_t2_tank", 12),
        ("aster_t2_missile", 6),
        ("aster_t3_assault_bot", 4),
        ("aster_t3_artillery", 2),
        ("aster_t4_assault_tank", 2),
    ]
    .into_iter()
    .enumerate()
    {
        let x = 300 + (i as i32 % 4) * 130;
        let y = 300 + (i as i32 / 4) * 200;
        all.extend(spawn_at(&mut w, key, &grid(x, y, 5, n, 22), Angle::ZERO));
    }
    let mut glued = vec![0u32; all.len()];
    // Distance from its rank when each member's current streak began.
    let mut began = vec![Fx::ZERO; all.len()];
    let mut total = 0;
    let mut rng = seed;
    for leg in 0..(3600 / leg_ticks) {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let x = 300 + (rng >> 8) as i32 % 1400;
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let y = 300 + (rng >> 8) as i32 % 1400;
        // Every third leg to half the army, crossing the other half.
        let who: Vec<_> = if leg % 3 == 2 {
            all.iter().copied().step_by(2).collect()
        } else {
            all.clone()
        };
        move_to(&mut w, &who, x, y);
        for _ in 0..leg_ticks {
            w.tick(&[]).unwrap();
            for (i, &id) in all.iter().enumerate() {
                let r = row(&w, id);
                let ra = radius(&w, id);
                let mut away = Fx::ZERO;
                let on = w.state.orders.front(&w.state.units, r).copied().is_some_and(|o| {
                    let off_rank = w.state.formations.get(&o.formation).is_some_and(|g| {
                        away = (g.anchor + o.offset.rotate(g.heading - o.heading)).distance(w.state.units.pos[r]);
                        away > ra * 2
                    });
                    off_rank
                        && all.iter().any(|&b| {
                            let rb = radius(&w, b);
                            rb > ra * 2
                                && w.state.units.pos[row(&w, b)].distance(w.state.units.pos[r])
                                    < ra + rb + Fx::from_int(3)
                        })
                });
                glued[i] = if on { glued[i] + 1 } else { 0 };
                if glued[i] == 1 {
                    began[i] = away;
                }
                // Getting there after all, round the heavy: not stuck.
                if glued[i] > 0 && away < began[i] - Fx::from_int(3) {
                    glued[i] = 1;
                    began[i] = away;
                }
                if glued[i] >= 30 {
                    total += 1;
                }
            }
        }
    }
    println!("seed {seed}: {} member-seconds glued to a heavy past the first three", total / 10);
    total / 10
}

/// `SEED=n LEG=ticks cargo test --release --test crowd -- --ignored --nocapture glued_probe_run`
#[test]
#[ignore]
fn glued_probe_run() {
    let var = |k: &str, or: u32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(or);
    glued_probe(var("SEED", 12345), var("LEG", 250));
}

#[test]
fn tanks_in_a_mixed_block_do_not_ride_on_its_heavies() {
    // About 470 now. With heavies always in the middle of the block, one
    // seed alone came to over a thousand; the old layout, spacing everyone
    // at the widest hull's width, a handful, as nobody came near a heavy.
    let mut total = 0;
    for seed in 1..=3 {
        total += glued_probe(seed, 250);
    }
    assert!(total < 600, "{total} member-seconds riding on a heavy");
}

/// Members that parked short of their rank when their order ended, over a
/// run of whole-army moves each left to finish: the ones a player sees
/// stuck on another hull. `SEED` picks the destinations.
fn parked_short(seed: u32) -> (usize, usize) {
    let mut w = world();
    let mut all = Vec::new();
    for (i, (key, n)) in [
        ("aster_t1_tank", 30),
        ("aster_t1_scout", 10),
        ("aster_t1_artillery", 10),
        ("aster_t2_tank", 12),
        ("aster_t2_missile", 6),
        ("aster_t3_assault_bot", 4),
        ("aster_t3_artillery", 2),
        ("aster_t4_assault_tank", 2),
    ]
    .into_iter()
    .enumerate()
    {
        let x = 300 + (i as i32 % 4) * 130;
        let y = 300 + (i as i32 / 4) * 200;
        all.extend(spawn_at(&mut w, key, &grid(x, y, 5, n, 22), Angle::ZERO));
    }
    let mut rng = seed;
    let (mut short, mut orders) = (0, 0);
    for _ in 0..6 {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let x = 300 + (rng >> 8) as i32 % 1400;
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let y = 300 + (rng >> 8) as i32 % 1400;
        move_to(&mut w, &all, x, y);
        let mut last: Vec<Option<FxVec2>> = vec![None; all.len()];
        for _ in 0..2000 {
            if busy(&w, &all) == 0 {
                break;
            }
            for (i, &id) in all.iter().enumerate() {
                let r = row(&w, id);
                last[i] = w.state.orders.front(&w.state.units, r).and_then(|o| {
                    let g = w.state.formations.get(&o.formation)?;
                    Some(g.anchor + o.offset.rotate(g.heading - o.heading))
                });
            }
            w.tick(&[]).unwrap();
            for (i, &id) in all.iter().enumerate() {
                let r = row(&w, id);
                if let (Some(slot), None) = (last[i], w.state.orders.front(&w.state.units, r)) {
                    orders += 1;
                    if w.state.units.pos[r].distance(slot) > radius(&w, id) * 2 {
                        short += 1;
                    }
                    last[i] = None;
                }
            }
        }
    }
    (short, orders)
}

#[test]
fn a_mixed_army_parks_on_its_ranks() {
    let (mut short, mut orders) = (0, 0);
    for seed in 1..=3 {
        let (s, o) = parked_short(seed);
        println!("seed {seed}: {s} of {o} orders ended short of the rank");
        short += s;
        orders += o;
    }
    assert!(short * 50 <= orders, "{short} of {orders} orders ended short of the rank");
}
