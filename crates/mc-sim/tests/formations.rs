//! Air transit, altitude bands, and formation integration.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

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

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, flags: u16) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, 512),
        heading: Angle::ZERO,
        count: 1,
        flags,
        build: 1000,
    })
}

fn group(w: &mut World, key: &str, count: i32) -> Vec<mc_sim::UnitId> {
    let commands: Vec<_> = (0..count)
        .map(|i| {
            cmd(Command::DebugSpawn {
                owner: 0,
                blueprint: w.blueprints.id_of(key).unwrap(),
                pos: FxVec2::from_ints(350 + (i % 5) * 24, 350 + (i / 5) * 24),
                heading: Angle::ZERO,
                count: 1,
                flags: flag::PASSIVE,
                build: 1000,
            })
        })
        .collect();
    w.tick(&commands).unwrap();
    let bp = w.blueprints.id_of(key).unwrap();
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.blueprint[r] == bp)
        .map(|r| w.state.units.id(r))
        .collect()
}

#[test]
fn repeating_vs_arrive_face_the_order_and_hold_their_slots() {
    let mut w = world();
    let ids = group(&mut w, "aster_t1_interceptor", 15);
    w.tick(&[cmd(Command::Move {
        units: ids.clone(),
        target: FxVec2::from_ints(950, 650),
        queue: false,
    })])
    .unwrap();
    let goals: Vec<_> = ids
        .iter()
        .map(|&id| {
            let r = w.state.units.row(id).unwrap();
            let o = *w.state.orders.front(&w.state.units, r).unwrap();
            (r, o.pos + o.offset, o.heading)
        })
        .collect();
    for _ in 0..500 {
        w.tick(&[]).unwrap();
    }
    for &(r, goal, heading) in &goals {
        assert!(
            w.state.units.pos[r].distance(goal) < Fx::ONE,
            "slot {r}: {:?} vs {goal:?}, order {:?} move {:?} speed {:?} flags {}",
            w.state.units.pos[r],
            w.state.orders.front(&w.state.units, r),
            w.state.units.move_goal[r],
            w.state.units.speed[r],
            w.state.units.flags[r]
        );
        assert!(
            w.state.orders.front(&w.state.units, r).is_none(),
            "order stuck for {r}"
        );
        assert_eq!(w.state.units.speed[r], Fx::ZERO);
        assert_eq!(w.state.units.heading[r], heading);
        assert_eq!(w.state.units.bank[r], 0);
    }
    let positions = w.state.units.pos.clone();
    for _ in 0..80 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(w.state.units.pos, positions, "idle formation drifted");
}

#[test]
fn mixed_land_and_air_selection_has_independent_layouts() {
    let mut w = world();
    let air = group(&mut w, "aster_t1_interceptor", 10);
    let land = group(&mut w, "aster_t1_tank", 9);
    let all = air.iter().chain(&land).copied().collect();
    w.tick(&[cmd(Command::Move {
        units: all,
        target: FxVec2::from_ints(900, 400),
        queue: false,
    })])
    .unwrap();
    let locals = |ids: &[mc_sim::UnitId]| -> Vec<FxVec2> {
        ids.iter()
            .map(|&id| {
                let r = w.state.units.row(id).unwrap();
                let o = w.state.orders.front(&w.state.units, r).unwrap();
                o.offset.rotate(-o.heading)
            })
            .collect()
    };
    let ground = locals(&land);
    let mut xs: Vec<_> = ground.iter().map(|p| p.x.floor_int()).collect();
    xs.sort();
    xs.dedup();
    assert_eq!(xs.len(), 3, "land must have three block ranks");
    let av = locals(&air);
    assert!(
        av.iter().map(|p| p.y).max().unwrap() - av.iter().map(|p| p.y).min().unwrap()
            > Fx::from_int(100),
        "two repeating Vs should be wider than the land block"
    );
    for _ in 0..500 {
        w.tick(&[]).unwrap();
    }
    for id in land {
        let r = w.state.units.row(id).unwrap();
        assert!(
            w.state.orders.front(&w.state.units, r).is_none(),
            "ground unit never arrived"
        );
    }
}

#[test]
fn vertically_separated_aircraft_do_not_shove_each_other() {
    let mut w = world();
    // Water prevents idle landing, keeping both altitude bands under test.
    w.terrain = Heightfield::from_samples(
        256,
        256,
        vec![0; 257 * 257],
        Fx::from_int(-20),
        Fx::ONE,
        Fx::ZERO,
    );
    // Emulate a future alternate-altitude blueprint without adding new units.
    let high = w.blueprints.id_of("aster_t1_bomber").unwrap();
    Arc::make_mut(&mut w.blueprints).units[high.index()]
        .motion
        .as_mut()
        .unwrap()
        .altitude = Fx::from_int(80);
    w.tick(&[
        spawn(&w, 0, "aster_t1_interceptor", 500, flag::PASSIVE),
        spawn(&w, 0, "aster_t1_bomber", 500, flag::PASSIVE),
    ])
    .unwrap();
    let positions = w.state.units.pos.clone();
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        positions, w.state.units.pos,
        "different altitude bands collided in XY"
    );
    let rows: Vec<_> = w.state.units.slots.iter().collect();
    assert!((w.state.units.z[rows[0]] - w.state.units.z[rows[1]]).abs() > Fx::from_int(50));
}

#[test]
fn a_turn_banks_then_levels_and_a_queued_leg_completes() {
    let mut w = world();
    let ids = group(&mut w, "aster_t1_interceptor", 5);
    let r = w.state.units.row(ids[0]).unwrap();
    w.tick(&[cmd(Command::Move {
        units: ids.clone(),
        target: FxVec2::from_ints(900, 350),
        queue: false,
    })])
    .unwrap();
    for _ in 0..150 {
        w.tick(&[]).unwrap();
        if w.state.units.speed[r] > Fx::from_int(35) && w.state.units.pos[r].x > Fx::from_int(500) {
            break;
        }
    }
    assert!(
        w.state.units.speed[r] > Fx::from_int(35),
        "formation never reached cruise before the turn"
    );
    w.tick(&[
        cmd(Command::Move {
            units: ids.clone(),
            target: FxVec2::from_ints(700, 900),
            queue: false,
        }),
        cmd(Command::Move {
            units: ids.clone(),
            target: FxVec2::from_ints(1100, 900),
            queue: true,
        }),
    ])
    .unwrap();
    let mut banked = false;
    for _ in 0..700 {
        w.tick(&[]).unwrap();
        banked |= w.state.units.bank[r].abs() > 1000;
    }
    assert!(banked, "turning jet remained flat");
    for id in ids {
        let r = w.state.units.row(id).unwrap();
        assert!(
            w.state.orders.front(&w.state.units, r).is_none(),
            "queued leg did not finish"
        );
        assert!(w.state.units.pos[r].distance(FxVec2::from_ints(1100, 900)) < Fx::from_int(90));
        assert_eq!(w.state.units.bank[r], 0);
    }
}

#[test]
fn edge_orders_preserve_unique_air_slots() {
    let mut w = world();
    let ids = group(&mut w, "aster_t1_interceptor", 15);
    w.tick(&[cmd(Command::Move {
        units: ids.clone(),
        target: FxVec2::from_ints(1, 1),
        queue: false,
    })])
    .unwrap();
    let goals: Vec<_> = ids
        .iter()
        .map(|&id| {
            let r = w.state.units.row(id).unwrap();
            let o = w.state.orders.front(&w.state.units, r).unwrap();
            o.pos + o.offset
        })
        .collect();
    for (i, a) in goals.iter().enumerate() {
        assert!(a.x >= Fx::from_int(3) && a.y >= Fx::from_int(3));
        for b in &goals[i + 1..] {
            assert!(a.distance(*b) > Fx::from_int(10));
        }
    }
    for _ in 0..500 {
        w.tick(&[]).unwrap();
    }
    for (id, goal) in ids.iter().zip(goals) {
        let r = w.state.units.row(*id).unwrap();
        assert!(w.state.units.pos[r].distance(goal) < Fx::ONE);
    }
}

#[test]
fn air_snapshot_restores_banks_and_the_same_future() {
    let mut a = world();
    let ids = group(&mut a, "aster_t1_interceptor", 140);
    a.tick(&[cmd(Command::Move {
        units: ids,
        target: FxVec2::from_ints(1150, 1250),
        queue: false,
    })])
    .unwrap();
    for _ in 0..7 {
        a.tick(&[]).unwrap();
    }
    let snapshot = a.snapshot();
    let mut b = world();
    b.pool = Arc::new(Pool::new(4));
    b.restore(Heightfield::flat(256, 256, Fx::from_int(20)), &snapshot)
        .unwrap();
    assert_eq!(a.state.units.bank, b.state.units.bank);
    for _ in 0..120 {
        a.tick(&[]).unwrap();
        b.tick(&[]).unwrap();
        assert_eq!(a.hash(), b.hash());
        assert_eq!(a.state.units.bank, b.state.units.bank);
    }
}

#[test]
fn attack_move_returns_to_formation_after_combat() {
    let mut w = world();
    let ids = group(&mut w, "aster_t1_interceptor", 5);
    w.tick(&[spawn(
        &w,
        1,
        "aster_t1_interceptor",
        600,
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let enemy = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == 1)
        .unwrap();
    let enemy_id = w.state.units.id(enemy);
    w.tick(&[cmd(Command::AttackMove {
        units: ids.clone(),
        target: FxVec2::from_ints(1000, 600),
        queue: false,
    })])
    .unwrap();
    let mut fought = false;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        fought |= ids.iter().any(|&id| {
            let r = w.state.units.row(id).unwrap();
            w.state.units.flags[r] & flag::AIR_RUN != 0
        });
    }
    assert!(fought);
    w.tick(&[cmd(Command::DebugRemove {
        units: vec![enemy_id],
    })])
    .unwrap();
    for _ in 0..500 {
        w.tick(&[]).unwrap();
    }
    for id in ids {
        let r = w.state.units.row(id).unwrap();
        assert!(w.state.orders.front(&w.state.units, r).is_none());
        assert_eq!(w.state.units.speed[r], Fx::ZERO);
        assert!(w.state.units.pos[r].distance(FxVec2::from_ints(1000, 600)) < Fx::from_int(80));
    }
}

#[test]
fn bomber_repeats_committed_passes() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_bomber", 400, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            620,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    let bomber = w.blueprints.id_of("aster_t1_bomber").unwrap();
    let row = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == bomber)
        .unwrap();
    let target = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == 1)
        .unwrap();
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(row)],
        target: w.state.units.id(target),
        queue: false,
    })])
    .unwrap();
    let mut shots = Vec::new();
    let mut crossings = 0;
    let mut before = w.state.units.pos[row].x < w.state.units.pos[target].x;
    // Aircraft need time for full passes and continuous cruise-speed turns.
    for _ in 0..1000 {
        w.tick(&[]).unwrap();
        let side = w.state.units.pos[row].x < w.state.units.pos[target].x;
        if side != before {
            crossings += 1;
            before = side;
        }
        for event in &w.events {
            if matches!(event, mc_sim::SimEvent::ShotFired { blueprint, .. } if *blueprint == bomber)
            {
                shots.push(w.tick_count());
            }
        }
    }
    assert!(crossings >= 3, "only {crossings} passes");
    assert!(
        shots.len() >= 16,
        "only {} bombs in repeated passes",
        shots.len()
    );
}

#[test]
fn mixed_speed_aircraft_travel_together_in_one_altitude_band() {
    let mut w = world();
    let mut ids = group(&mut w, "aster_t1_interceptor", 5);
    ids.extend(group(&mut w, "aster_t1_bomber", 5));
    w.tick(&[cmd(Command::Move {
        units: ids.clone(),
        target: FxVec2::from_ints(1800, 400),
        queue: false,
    })])
    .unwrap();
    for _ in 0..100 {
        w.tick(&[]).unwrap();
    }
    let anchors: Vec<_> = ids
        .iter()
        .map(|&id| {
            let r = w.state.units.row(id).unwrap();
            let o = w
                .state
                .orders
                .front(&w.state.units, r)
                .expect("still travelling");
            w.state.units.pos[r] - o.offset
        })
        .collect();
    let min_x = anchors.iter().map(|p| p.x).min().unwrap();
    let max_x = anchors.iter().map(|p| p.x).max().unwrap();
    assert!(
        max_x - min_x < Fx::from_int(45),
        "fast aircraft left the bombers behind: {:?}",
        max_x - min_x
    );
}

#[test]
fn land_block_keeps_its_ranks_mid_march() {
    let mut w = world();
    let ids = group(&mut w, "aster_t1_tank", 25);
    w.tick(&[cmd(Command::Move {
        units: ids.clone(),
        target: FxVec2::from_ints(1700, 850),
        queue: false,
    })])
    .unwrap();
    let row = w.state.units.row(ids[0]).unwrap();
    let identity = w.state.orders.front(&w.state.units, row).unwrap().formation;
    let mut checked = 0;
    for _ in 0..500 {
        w.tick(&[]).unwrap();
        let Some(g) = w.state.formations.get(&identity) else {
            break;
        };
        if g.phase != 2 || g.anchor.x < Fx::from_int(750) || g.anchor.x > Fx::from_int(1300) {
            continue;
        }
        let samples: Vec<_> = ids
            .iter()
            .map(|id| {
                let row = w.state.units.row(*id).unwrap();
                let o = w
                    .state
                    .orders
                    .front(&w.state.units, row)
                    .expect("member left before the group arrived");
                w.state.units.pos[row] - o.offset.rotate(g.heading - o.heading)
            })
            .collect();
        let sum = samples.iter().copied().fold(FxVec2::ZERO, |a, b| a + b);
        let center = FxVec2::new(sum.x / samples.len() as i32, sum.y / samples.len() as i32);
        let worst = samples.iter().map(|p| p.distance(center)).max().unwrap();
        assert!(
            worst < Fx::from_int(6),
            "moving ranks distorted by {worst:?} at tick {}",
            w.tick_count()
        );
        checked += 1;
    }
    assert!(
        checked > 10,
        "no sustained moving formation ({checked} samples)"
    );
}

#[test]
fn tanks_do_not_overlap_an_amphibious_commander() {
    let mut w = world();
    let tanks = group(&mut w, "aster_t1_tank", 16);
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: w.blueprints.id_of("aster_commander").unwrap(),
        pos: FxVec2::from_ints(385, 385),
        heading: Angle::ZERO,
        count: 1,
        flags: flag::PASSIVE,
        build: 1000,
    })])
    .unwrap();
    let commander = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| {
            w.blueprints
                .unit(w.state.units.blueprint[r])
                .has(mc_data::cat::COMMANDER)
        })
        .unwrap();
    for _ in 0..80 {
        w.tick(&[]).unwrap();
    }
    for id in tanks {
        let row = w.state.units.row(id).unwrap();
        let required = w.blueprints.unit(w.state.units.blueprint[row]).radius
            + w.blueprints.unit(w.state.units.blueprint[commander]).radius;
        assert!(
            w.state.units.pos[row].distance(w.state.units.pos[commander]) >= required - Fx::HALF,
            "tank {row} overlaps commander"
        );
    }
}

#[test]
fn idle_fighter_turns_and_engages_an_enemy_behind_it() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_interceptor", 550, 0),
        spawn(
            &w,
            1,
            "aster_t1_interceptor",
            390,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    let row = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == 0)
        .unwrap();
    let start = w.state.units.pos[row];
    let mut flew = false;
    let mut fired = false;
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        flew |= w.state.units.speed[row] > Fx::from_int(30);
        fired |= w
            .events
            .iter()
            .any(|e| matches!(e, mc_sim::SimEvent::ShotFired { .. }));
    }
    assert!(flew && fired, "idle fighter failed to pursue and fire");
    assert!(w.state.units.pos[row].distance(start) > Fx::from_int(20));
}

#[test]
fn fighters_cannot_target_air_factories_even_with_an_explicit_attack() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_interceptor", 500, 0),
        spawn(&w, 1, "aster_t1_air_factory", 640, flag::PASSIVE),
    ])
    .unwrap();
    let row = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == 0)
        .unwrap();
    let target = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == 1)
        .unwrap();
    let factory_health = w.state.units.health[target];
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(row)],
        target: w.state.units.id(target),
        queue: false,
    })])
    .unwrap();
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        assert!(w.state.units.weapon_target[row]
            .iter()
            .all(|t| *t == mc_sim::Handle::NONE));
        assert!(w.state.orders.front(&w.state.units, row).is_none());
    }
    assert_eq!(w.state.units.health[target], factory_health);
}

#[test]
fn formation_spacing_and_free_move_are_real_commands() {
    let mut w = world();
    let ids = group(&mut w, "aster_t1_tank", 9);
    let command = |spacing, together| {
        cmd(Command::FormationMove {
            units: ids.clone(),
            target: FxVec2::from_ints(1200, 700),
            queue: false,
            attack_move: false,
            together,
            spacing,
        })
    };
    w.tick(&[command(0, true)]).unwrap();
    let extent = |w: &World| {
        ids.iter()
            .map(|id| {
                let r = w.state.units.row(*id).unwrap();
                w.state
                    .orders
                    .front(&w.state.units, r)
                    .unwrap()
                    .offset
                    .length()
            })
            .max()
            .unwrap()
    };
    let compact = extent(&w);
    w.tick(&[command(2, true)]).unwrap();
    assert!(extent(&w) > compact * Fx::ratio(3, 2));
    w.tick(&[command(1, false)]).unwrap();
    for &id in &ids {
        let r = w.state.units.row(id).unwrap();
        assert_eq!(
            w.state.orders.front(&w.state.units, r).unwrap().formation,
            0
        );
    }
    let c = command(2, true).command;
    assert_eq!(Command::decode(&c.encode()), Some(c));
}

#[test]
fn a_block_passes_a_structure_and_reforms_beyond_it() {
    let mut w = world();
    let ids = group(&mut w, "aster_t1_tank", 16);
    w.tick(&[spawn(&w, 0, "aster_t1_air_factory", 820, flag::PASSIVE)])
        .unwrap();
    w.tick(&[cmd(Command::Move {
        units: ids.clone(),
        target: FxVec2::from_ints(1500, 512),
        queue: false,
    })])
    .unwrap();
    for _ in 0..950 {
        w.tick(&[]).unwrap();
    }
    for id in &ids {
        let r = w.state.units.row(*id).unwrap();
        assert!(
            w.state.units.pos[r].x > Fx::from_int(1300),
            "unit {r} stuck behind structure at {:?}",
            w.state.units.pos[r]
        );
        assert!(
            w.state.orders.front(&w.state.units, r).is_none(),
            "unit {r} did not reform at destination"
        );
    }
    for (i, id) in ids.iter().enumerate() {
        let a = w.state.units.row(*id).unwrap();
        for other in &ids[i + 1..] {
            let b = w.state.units.row(*other).unwrap();
            let radii = w.blueprints.unit(w.state.units.blueprint[a]).radius
                + w.blueprints.unit(w.state.units.blueprint[b]).radius;
            assert!(w.state.units.pos[a].distance(w.state.units.pos[b]) >= radii - Fx::HALF);
        }
    }
}

#[test]
fn moving_onto_a_parked_commander_finishes_in_a_clear_block() {
    let mut w = world();
    let ids = group(&mut w, "aster_t1_tank", 9);
    w.tick(&[spawn(&w, 0, "aster_commander", 950, flag::PASSIVE)])
        .unwrap();
    let commander = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| {
            w.blueprints
                .unit(w.state.units.blueprint[r])
                .has(mc_data::cat::COMMANDER)
        })
        .unwrap();
    let at = w.state.units.pos[commander];
    w.tick(&[cmd(Command::Move {
        units: ids.clone(),
        target: at,
        queue: false,
    })])
    .unwrap();
    for _ in 0..650 {
        w.tick(&[]).unwrap();
    }
    for id in ids {
        let r = w.state.units.row(id).unwrap();
        assert!(
            w.state.orders.front(&w.state.units, r).is_none(),
            "unit {r} keeps trying to occupy the commander: pos {:?}, goal {:?}, commander {:?}, groups {:?}",w.state.units.pos[r],w.state.orders.front(&w.state.units,r),w.state.units.pos[commander],w.state.formations
        );
        let gap = w.blueprints.unit(w.state.units.blueprint[r]).radius
            + w.blueprints.unit(w.state.units.blueprint[commander]).radius;
        assert!(w.state.units.pos[r].distance(w.state.units.pos[commander]) >= gap);
    }
}

#[test]
fn disordered_units_start_forward_without_waiting_to_form() {
    for key in ["aster_t1_tank", "aster_t1_interceptor"] {
        let mut w = world();
        let ids = group(&mut w, key, 25);
        let start: Vec<_> = ids
            .iter()
            .map(|&id| w.state.units.pos[w.state.units.row(id).unwrap()])
            .collect();
        w.tick(&[cmd(Command::Move {
            units: ids.clone(),
            target: FxVec2::from_ints(1800, 398),
            queue: false,
        })])
        .unwrap();
        let first = w.state.units.row(ids[0]).unwrap();
        let identity = w
            .state
            .orders
            .front(&w.state.units, first)
            .unwrap()
            .formation;
        let g = &w.state.formations[&identity];
        assert_eq!(g.phase, 1, "fixture must still be out of formation");
        assert!(
            g.anchor.x > Fx::from_int(398),
            "{key} anchor waited to gather"
        );
        for (i, &id) in ids.iter().enumerate() {
            let row = w.state.units.row(id).unwrap();
            assert!(
                w.state.units.pos[row].x > start[i].x,
                "{key} member {i} did not move forward on the command tick"
            );
        }
        for _ in 0..20 {
            w.tick(&[]).unwrap();
        }
        for (i, &id) in ids.iter().enumerate() {
            let row = w.state.units.row(id).unwrap();
            assert!(
                w.state.units.pos[row].x > start[i].x + Fx::from_int(5),
                "{key} member {i} stopped to assemble"
            );
        }
    }
}

#[test]
fn cruise_is_above_the_largest_dome_and_bombs_hit_its_roof() {
    let mut w = world();
    w.state.players[1].free_build = true;
    w.tick(&[
        spawn(&w, 0, "aster_t1_bomber", 350, 0),
        spawn(&w, 1, "aster_t3_shield", 650, flag::PASSIVE),
    ])
    .unwrap();
    let bomber = 0;
    let shield = 1;
    let spec = w
        .blueprints
        .unit(w.state.units.blueprint[shield])
        .shield
        .unwrap();
    let roof = w.state.units.z[shield] + spec.radius;
    assert!(w.state.units.z[bomber] > roof + Fx::from_int(15));
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(bomber)],
        target: w.state.units.id(shield),
        queue: false,
    })])
    .unwrap();
    let health = w.state.units.health[shield];
    let mut impacts = 0;
    for _ in 0..250 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let mc_sim::SimEvent::Impact {
                on_shield: true,
                pos,
                ..
            } = e
            {
                assert!(pos.z > w.state.units.z[shield] + spec.radius / 2);
                impacts += 1;
            }
        }
    }
    assert!(
        impacts >= 4,
        "bombs failed to strike the dome from above ({impacts})"
    );
    assert_eq!(
        w.state.units.health[shield], health,
        "bombs bypassed the shield"
    );
}

#[test]
fn aircraft_roll_into_a_reversal_and_cruise_along_their_heading() {
    let mut w = world();
    let ids = group(&mut w, "aster_t1_interceptor", 1);
    let row = w.state.units.row(ids[0]).unwrap();
    w.tick(&[cmd(Command::Move {
        units: ids.clone(),
        target: FxVec2::from_ints(1500, 350),
        queue: false,
    })])
    .unwrap();
    for _ in 0..70 {
        w.tick(&[]).unwrap();
    }
    let start = w.state.units.pos[row];
    let heading = w.state.units.heading[row];
    w.tick(&[cmd(Command::Move {
        units: ids,
        target: FxVec2::from_ints(200, 900),
        queue: false,
    })])
    .unwrap();
    let motion = w
        .blueprints
        .unit(w.state.units.blueprint[row])
        .motion
        .unwrap();
    assert!(
        heading.delta_to(w.state.units.heading[row]).unsigned_abs() < motion.turn_rate / 2,
        "full yaw applied instantly"
    );
    let mut distance = Fx::ZERO;
    for _ in 0..50 {
        let before = w.state.units.heading[row];
        w.tick(&[]).unwrap();
        let step = w.state.units.pos[row] - w.state.units.prev_pos[row];
        let nose = FxVec2::from_angle(w.state.units.heading[row]);
        assert!(
            step.cross(nose).abs() < Fx::ratio(1, 100),
            "cruise aircraft slid sideways"
        );
        assert!(before.delta_to(w.state.units.heading[row]).unsigned_abs() <= motion.turn_rate);
        distance += step.length();
    }
    assert!(distance > Fx::from_int(100));
    assert!(w.state.units.pos[row].distance(start) > Fx::from_int(30));
}

#[test]
fn bomber_combat_snapshot_preserves_the_return_leg() {
    let mut a = world();
    a.tick(&[
        spawn(&a, 0, "aster_t1_bomber", 350, 0),
        spawn(
            &a,
            1,
            "aster_t1_tank",
            650,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    a.tick(&[cmd(Command::Attack {
        units: vec![a.state.units.id(0)],
        target: a.state.units.id(1),
        queue: false,
    })])
    .unwrap();
    for _ in 0..130 {
        a.tick(&[]).unwrap();
    }
    let snapshot = a.snapshot();
    let mut b = world();
    b.pool = Arc::new(Pool::new(4));
    b.restore(Heightfield::flat(256, 256, Fx::from_int(20)), &snapshot)
        .unwrap();
    for _ in 0..500 {
        a.tick(&[]).unwrap();
        b.tick(&[]).unwrap();
        assert_eq!(a.hash(), b.hash());
    }
}

#[test]
fn direct_anti_air_rounds_survive_the_climb_to_cruise_height() {
    let mut w = world();
    let turret = w.blueprints.id_of("aster_t1_point_defense").unwrap();
    // Exercise the generic ground AA projectile path with an existing gun.
    Arc::make_mut(&mut w.blueprints).units[turret.index()].weapons[0].target_mask =
        mc_data::cat::AIR;
    w.tick(&[
        spawn(&w, 0, "aster_t1_point_defense", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_bomber",
            620,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    let mut hits = 0;
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        hits += w.events.iter().filter(|e|matches!(e,mc_sim::SimEvent::Impact {on_unit:true,blueprint,..} if *blueprint==turret)).count();
    }
    assert!(hits > 0, "AA shells expired below the flight band");
}

#[test]
fn a_bomber_attacks_near_the_map_edge_without_getting_pinned() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_bomber", 350, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            18,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    w.state.units.heading[0] = Angle::HALF_TURN;
    w.state.units.prev_heading[0] = Angle::HALF_TURN;
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(0)],
        target: w.state.units.id(1),
        queue: false,
    })])
    .unwrap();
    let mut slow = 0;
    let mut bombs = 0;
    for i in 0..1000 {
        w.tick(&[]).unwrap();
        let moved = w.state.units.pos[0].distance(w.state.units.prev_pos[0]);
        if i > 50 && moved < Fx::ONE {
            slow += 1;
        } else {
            slow = 0;
        }
        assert!(
            slow < 5,
            "bomber pinned to map edge at {:?}, tick {i}",
            w.state.units.pos[0]
        );
        bombs += w
            .events
            .iter()
            .filter(|e| matches!(e, mc_sim::SimEvent::ShotFired { .. }))
            .count();
    }
    assert!(bombs >= 16, "edge target received only {bombs} bombs");
}

#[test]
fn bomber_returns_to_last_seen_target_after_egress_into_fog() {
    let mut w = world();
    w.state.fog_enabled = true;
    w.tick(&[
        spawn(&w, 0, "aster_t1_bomber", 500, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            650,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(0)],
        target: w.state.units.id(1),
        queue: false,
    })])
    .unwrap();
    let mut hidden = 0;
    let mut bombs = 0;
    for _ in 0..1100 {
        let aim = w.state.units.air_aim[0];
        let detected = w.detects(0, 1);
        w.tick(&[]).unwrap();
        if !detected {
            hidden += 1;
            assert_eq!(w.state.units.air_aim[0], aim, "read unseen target position");
        }
        bombs += w
            .events
            .iter()
            .filter(|e| matches!(e, mc_sim::SimEvent::ShotFired { .. }))
            .count();
    }
    assert!(hidden > 100, "fixture did not leave vision during return");
    assert!(
        bombs >= 16,
        "forgot target during wide return: {bombs} bombs"
    );
}

#[test]
fn bank_and_last_seen_flight_state_participate_in_the_sync_hash() {
    let mut w = world();
    w.tick(&[spawn(&w, 0, "aster_t1_bomber", 500, flag::PASSIVE)])
        .unwrap();
    let original = w.hash();
    w.state.units.bank[0] += 10;
    assert_ne!(w.hash(), original);
    w.state.units.bank[0] -= 10;
    w.state.units.air_aim[0].x += Fx::ONE;
    assert_ne!(w.hash(), original);
    w.state.units.air_aim[0].x -= Fx::ONE;
    w.state.units.air_turn_ticks[0] += 1;
    assert_ne!(w.hash(), original);
    w.state.units.air_turn_ticks[0] -= 1;
    w.state.units.air_break_ticks[0] += 1;
    assert_ne!(w.hash(), original);
}

#[test]
fn crossing_aircraft_do_not_change_each_others_flight_paths() {
    // Compare each flight with the identical flight in clear air. Both hulls
    // occupy the same altitude and cross head-on at cruise speed.
    let mut paired = world();
    let mut solos = [world(), world()];
    for w in std::iter::once(&mut paired).chain(solos.iter_mut()) {
        w.tick(&[
            spawn(w, 0, "aster_t1_bomber", 500, flag::PASSIVE),
            spawn(w, 0, "aster_t1_bomber", 800, flag::PASSIVE),
        ])
        .unwrap();
        w.state.units.heading[1] = Angle::HALF_TURN;
        w.state.units.prev_heading[1] = Angle::HALF_TURN;
        w.tick(&[
            cmd(Command::Move {
                units: vec![w.state.units.id(0)],
                target: FxVec2::from_ints(1000, 512),
                queue: false,
            }),
            cmd(Command::Move {
                units: vec![w.state.units.id(1)],
                target: FxVec2::from_ints(300, 512),
                queue: false,
            }),
        ])
        .unwrap();
    }
    // Put the irrelevant aircraft in each control world on a distant lane.
    solos[0].state.units.pos[1].y += Fx::from_int(500);
    solos[1].state.units.pos[0].y += Fx::from_int(500);
    let mut nearest = Fx::MAX;
    for _ in 0..100 {
        paired.tick(&[]).unwrap();
        nearest = nearest.min(paired.state.units.pos[0].distance(paired.state.units.pos[1]));
        for (row, solo) in solos.iter_mut().enumerate() {
            solo.tick(&[]).unwrap();
            assert_eq!(
                paired.state.units.pos[row], solo.state.units.pos[row],
                "air contact displaced flight {row}"
            );
            assert_eq!(
                paired.state.units.heading[row], solo.state.units.heading[row],
                "air contact steered flight {row}"
            );
            assert_eq!(paired.state.units.speed[row], solo.state.units.speed[row]);
        }
    }
    assert!(
        nearest
            < paired
                .blueprints
                .unit(paired.state.units.blueprint[0])
                .radius
                * 2,
        "fixture did not cross the hulls: {nearest:?}"
    );
}

#[test]
fn bomber_returns_in_one_continuous_turn_between_salvos() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_bomber", 700, 0),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            1000,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    // Center the fixture so a boundary escape cannot mask the ordinary pattern.
    w.state.units.pos[0].y = Fx::from_int(1000);
    w.state.units.pos[1].y = Fx::from_int(1000);
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(0)],
        target: w.state.units.id(1),
        queue: false,
    })])
    .unwrap();
    let mut salvos = 0;
    let mut last_shot = 0;
    let mut turn_started = false;
    let mut straight_ticks = 0;
    let mut turn_finished = false;
    let mut trace = String::from("tick,x,y,turn,shot\n");
    for tick in 1..1200 {
        let heading = w.state.units.heading[0];
        w.tick(&[]).unwrap();
        let turn = heading.delta_to(w.state.units.heading[0]).unsigned_abs();
        let shot = w.events.iter().any(|e| matches!(e, mc_sim::SimEvent::ShotFired { blueprint, .. } if *blueprint == w.state.units.blueprint[0]));
        let p = w.state.units.pos[0];
        trace.push_str(&format!(
            "{tick},{},{},{turn},{}\n",
            p.x.to_f32(),
            p.y.to_f32(),
            u8::from(shot)
        ));
        if shot {
            if tick - last_shot > 10 {
                salvos += 1;
                turn_started = false;
                turn_finished = false;
                straight_ticks = 0;
            }
            last_shot = tick;
        }
        if salvos > 0 && tick - last_shot > 10 {
            if turn > 90 {
                assert!(!turn_finished, "second turn before next salvo, tick {tick}");
                turn_started = true;
                straight_ticks = 0;
            } else if turn_started {
                straight_ticks += 1;
                if straight_ticks >= 10 {
                    turn_finished = true;
                }
            }
        }
    }
    if let Ok(path) = std::env::var("AIRCRAFT_TRACE") {
        std::fs::write(path, trace).unwrap();
    }
    assert!(salvos >= 4, "only {salvos} salvos during repeated turns");
}

#[test]
fn idle_aircraft_land_then_lift_clear_before_departing() {
    for key in ["aster_t1_interceptor", "aster_t1_bomber"] {
        let mut w = world();
        w.tick(&[spawn(&w, 0, key, 500, flag::PASSIVE)]).unwrap();
        let start = w.state.units.pos[0];
        let ground = w.terrain.height_at(start);
        let altitude = w
            .blueprints
            .unit(w.state.units.blueprint[0])
            .motion
            .unwrap()
            .altitude;
        let mut previous = w.state.units.z[0];
        for _ in 0..300 {
            w.tick(&[]).unwrap();
            assert!(w.state.units.z[0] <= previous, "idle aircraft climbed");
            previous = w.state.units.z[0];
        }
        assert_eq!(w.state.units.z[0], ground, "{key} never landed");
        assert_eq!(w.state.units.pos[0], start);
        w.tick(&[cmd(Command::Move {
            units: vec![w.state.units.id(0)],
            target: start + FxVec2::from_ints(800, 0),
            queue: false,
        })])
        .unwrap();
        assert_eq!(
            w.state.units.pos[0], start,
            "aircraft taxied through the ground during takeoff"
        );
        assert!(w.state.units.z[0] > ground);
        for _ in 0..120 {
            w.tick(&[]).unwrap();
        }
        assert!(
            (w.state.units.z[0] - ground - altitude).abs() < Fx::from_int(10),
            "takeoff should be easing into the cruise band"
        );
        assert!(w.state.units.pos[0].x > start.x + Fx::from_int(300));
        w.tick(&[cmd(Command::Stop {
            units: vec![w.state.units.id(0)],
        })])
        .unwrap();
        for _ in 0..300 {
            w.tick(&[]).unwrap();
        }
        assert_eq!(
            w.state.units.z[0], ground,
            "stopped aircraft failed to land"
        );
    }
}

#[test]
fn idle_aircraft_stay_aloft_only_when_no_ground_is_in_reach() {
    for water in [false, true] {
        let mut w = world();
        if water {
            w.terrain = Heightfield::from_samples(
                256,
                256,
                vec![0; 257 * 257],
                Fx::from_int(-20),
                Fx::ONE,
                Fx::ZERO,
            );
        }
        w.tick(&[spawn(&w, 0, "aster_t1_bomber", 500, flag::PASSIVE)])
            .unwrap();
        if !water {
            w.tick(&[spawn(&w, 0, "aster_t1_point_defense", 500, flag::PASSIVE)])
                .unwrap();
        }
        for _ in 0..600 {
            w.tick(&[]).unwrap();
        }
        let surface = w
            .terrain
            .height_at(w.state.units.pos[0])
            .max(w.terrain.water_level());
        if water {
            assert_eq!(w.state.units.z[0], surface + Fx::from_int(200));
        } else {
            // Occupied ground: it sets down beside the structure instead.
            assert_eq!(w.state.units.z[0], surface);
            assert!(w.state.units.pos[0].distance(w.state.units.pos[1]) > Fx::from_int(8));
        }
    }
}

#[test]
fn t1_fighters_and_bombers_share_the_lower_cruise_band() {
    let w = world();
    for key in ["aster_t1_interceptor", "aster_t1_bomber"] {
        let m = w
            .blueprints
            .unit(w.blueprints.id_of(key).unwrap())
            .motion
            .unwrap();
        assert_eq!(m.altitude, Fx::from_int(200));
        let shield = w
            .blueprints
            .unit(w.blueprints.id_of("aster_t3_shield").unwrap())
            .shield
            .unwrap();
        assert!(m.altitude > shield.radius);
    }
}

#[test]
fn fighter_sheds_speed_to_tighten_a_turn_then_accelerates_on_pursuit() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_interceptor", 900, flag::INVULNERABLE),
        spawn(
            &w,
            1,
            "aster_t1_bomber",
            650,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    let motion = w
        .blueprints
        .unit(w.state.units.blueprint[0])
        .motion
        .unwrap();
    w.state.units.speed[0] = motion.speed;
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(0)],
        target: w.state.units.id(1),
        queue: false,
    })])
    .unwrap();
    let mut slowest = motion.speed;
    let mut tightened = false;
    let mut recovered = false;
    for _ in 0..160 {
        let before = w.state.units.heading[0];
        w.tick(&[]).unwrap();
        let speed = w.state.units.speed[0];
        slowest = slowest.min(speed);
        if before.delta_to(w.state.units.heading[0]).unsigned_abs() > motion.turn_rate {
            tightened = true;
        }
        if slowest < motion.speed * Fx::ratio(7, 10) && speed > motion.speed * Fx::ratio(9, 10) {
            recovered = true;
        }
        assert!(
            speed >= motion.speed * Fx::HALF,
            "fighter stopped in combat"
        );
    }
    assert!(
        slowest < motion.speed * Fx::ratio(7, 10),
        "fighter held constant cruise speed"
    );
    assert!(tightened, "slowing down did not tighten the turn");
    assert!(recovered, "fighter never accelerated back into pursuit");
}

#[test]
fn sustained_fighter_combat_varies_speed_and_turn_radius() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_interceptor", 850, flag::INVULNERABLE),
        spawn(&w, 1, "aster_t1_interceptor", 1100, flag::INVULNERABLE),
    ])
    .unwrap();
    w.state.units.pos[0].y = Fx::from_int(1000);
    w.state.units.pos[1].y = Fx::from_int(1000);
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(0)],
        target: w.state.units.id(1),
        queue: false,
    })])
    .unwrap();
    let mut slow = [Fx::MAX; 2];
    let mut fast = [Fx::ZERO; 2];
    let mut shots = [0; 2];
    let mut restored: Option<World> = None;
    let mut trace = String::from("tick,row,x,y,speed,turn\n");
    for tick in 0..800 {
        let before = [w.state.units.heading[0], w.state.units.heading[1]];
        w.tick(&[]).unwrap();
        if let Some(other) = &mut restored {
            other.tick(&[]).unwrap();
            assert_eq!(
                w.hash(),
                other.hash(),
                "fighter maneuver diverged after restore"
            );
        } else if w.state.units.air_break_ticks[0] > 0 {
            let mut other = world();
            other.pool = Arc::new(Pool::new(4));
            other
                .restore(Heightfield::flat(256, 256, Fx::from_int(20)), &w.snapshot())
                .unwrap();
            restored = Some(other);
        }
        for row in 0..2 {
            let speed = w.state.units.speed[row];
            if tick > 500 {
                slow[row] = slow[row].min(speed);
                fast[row] = fast[row].max(speed);
            }
            let p = w.state.units.pos[row];
            trace.push_str(&format!(
                "{tick},{row},{},{},{},{}\n",
                p.x.to_f32(),
                p.y.to_f32(),
                speed.to_f32(),
                before[row].delta_to(w.state.units.heading[row])
            ));
        }
        for e in &w.events {
            if let mc_sim::SimEvent::ShotFired { owner, .. } = e {
                shots[*owner as usize] += 1;
            }
        }
    }
    if let Ok(path) = std::env::var("FIGHTER_TRACE") {
        std::fs::write(path, trace).unwrap();
    }
    assert!(
        restored.is_some(),
        "fixture never exercised a pursuit breakaway"
    );
    assert!(
        shots.iter().sum::<i32>() > 10,
        "mutual circling prevented firing opportunities"
    );
    for row in 0..2 {
        assert!(
            fast[row] - slow[row] > Fx::from_int(10),
            "fighter {row} settled at fixed speed: {:?}..{:?}",
            slow[row],
            fast[row]
        );
    }
}

#[test]
fn aircraft_descent_is_slow_and_flares_before_touchdown() {
    for key in ["aster_t1_interceptor", "aster_t1_bomber"] {
        let mut w = world();
        w.tick(&[spawn(&w, 0, key, 500, flag::PASSIVE)]).unwrap();
        let ground = w.terrain.height_at(w.state.units.pos[0]);
        w.state.units.z[0] = ground + Fx::from_int(200);
        let mut previous_drop = Fx::MAX;
        let mut flared = false;
        for tick in 0..300 {
            let before = w.state.units.z[0];
            w.tick(&[]).unwrap();
            let drop = before - w.state.units.z[0];
            assert!(
                drop >= Fx::ZERO && drop <= Fx::ratio(12, 10),
                "{key} dropped {drop:?} in one tick"
            );
            if before > ground && before < ground + Fx::from_int(20) {
                assert!(drop <= previous_drop, "{key} accelerated into touchdown");
                flared |= drop < Fx::HALF;
            }
            if tick < 100 {
                assert!(
                    w.state.units.z[0] > ground + Fx::from_int(70),
                    "{key} landed too abruptly"
                );
            }
            previous_drop = drop;
        }
        assert_eq!(w.state.units.z[0], ground, "{key} never touched down");
        assert!(flared, "{key} did not ease into touchdown");
    }
}

#[test]
fn aircraft_capture_close_slots_from_crosswise_and_away_headings() {
    // A bomber's slow turn can otherwise orbit outside the 24 m hover radius.
    for key in ["aster_t1_interceptor", "aster_t1_bomber"] {
        for distance in [20, 32, 48, 80] {
            for heading in [Angle(0x4000), Angle(0x8000), Angle(0xc000)] {
                let mut w = world();
                let ids = group(&mut w, key, 2);
                let row = w.state.units.row(ids[0]).unwrap();
                let target = w.state.units.pos[row] + FxVec2::from_ints(distance, 0);
                w.tick(&[cmd(Command::Move {
                    units: ids,
                    target,
                    queue: false,
                })])
                .unwrap();
                let order = *w.state.orders.front(&w.state.units, row).unwrap();
                let target = order.pos + order.offset;
                w.state.units.pos[row] = target - FxVec2::from_ints(distance, 0);
                let motion = w
                    .blueprints
                    .unit(w.state.units.blueprint[row])
                    .motion
                    .unwrap();
                // Reproduce a straggler chasing its slot after the anchor settles.
                let formation = w.state.formations.get_mut(&order.formation).unwrap();
                formation.anchor = order.pos;
                formation.speed = Fx::ZERO;
                formation.heading = order.heading;
                w.state.units.heading[row] = heading;
                w.state.units.speed[row] = motion.speed / 3;
                w.state.units.z[row] = Fx::from_int(220);
                for _ in 0..400 {
                    w.tick(&[]).unwrap();
                }
                assert!(w.state.orders.front(&w.state.units, row).is_none(),
                    "{key} kept circling: distance {distance}, heading {heading:?}, remaining {:?}, speed {:?}",
                    w.state.units.pos[row].distance(target), w.state.units.speed[row]);
                assert!(w.state.units.pos[row].distance(target) <= Fx::HALF);
                assert_eq!(w.state.units.speed[row], Fx::ZERO);
                assert_eq!(w.state.units.heading[row], order.heading);
            }
        }
    }
}

#[test]
fn bomber_attack_move_drops_on_each_pass_through_a_cluster() {
    for fog in [false, true] {
        for heading in [0, 90, 180, 270] {
            let mut w = world();
            w.state.fog_enabled = fog;
            let spawn_at = |owner, key: &str, x, y, heading, flags| {
                cmd(Command::DebugSpawn {
                    owner,
                    blueprint: w.blueprints.id_of(key).unwrap(),
                    pos: FxVec2::from_ints(x, y),
                    heading: Angle::from_degrees(heading),
                    count: 1,
                    flags,
                    build: 1000,
                })
            };
            w.tick(&[
                spawn_at(0, "aster_t1_bomber", 700, 1000, heading, 0),
                spawn_at(
                    1,
                    "aster_t1_tank",
                    1000,
                    1000,
                    0,
                    flag::PASSIVE | flag::INVULNERABLE,
                ),
                // This enemy is acquired first on some return arcs, but the
                // bomber is still flying a pass through the original target.
                spawn_at(
                    1,
                    "aster_t1_tank",
                    1000,
                    1100,
                    0,
                    flag::PASSIVE | flag::INVULNERABLE,
                ),
            ])
            .unwrap();
            w.tick(&[cmd(Command::AttackMove {
                units: vec![w.state.units.id(0)],
                target: FxVec2::from_ints(1000, 1000),
                queue: false,
            })])
            .unwrap();
            let mut shots = 0;
            let mut passes = Vec::new();
            let mut near = false;
            for tick in 0..1800 {
                w.tick(&[]).unwrap();
                shots += w
                    .events
                    .iter()
                    .filter(|e| matches!(e, mc_sim::SimEvent::ShotFired { .. }))
                    .count();
                let distance = w.state.units.pos[0].distance(w.state.units.pos[1]);
                if distance < Fx::from_int(60) {
                    near = true;
                } else if near && distance > Fx::from_int(100) {
                    passes.push((tick, shots));
                    shots = 0;
                    near = false;
                }
            }
            assert!(
                passes.len() >= 4,
                "not enough passes, fog {fog}, heading {heading}: {passes:?}"
            );
            // A launch facing away may need one pass to reach release speed
            // and altitude. Every subsequent pass must finish a full rack.
            assert!(
                passes.iter().skip(1).all(|&(_, bombs)| bombs == 8),
                "skipped return pass, fog {fog}, heading {heading}: {passes:?}"
            );
        }
    }
}

#[test]
fn bombs_burst_on_the_water_and_ground_fire_reaches_a_commander_under_it() {
    for ground_fire in [false, true] {
        let mut w = world();
        w.terrain = Heightfield::from_samples(
            256,
            256,
            vec![0; 257 * 257],
            Fx::from_int(-40),
            Fx::ONE,
            Fx::ZERO,
        );
        w.tick(&[
            spawn(&w, 0, "aster_t1_bomber", 350, 0),
            spawn(&w, 1, "aster_commander", 650, flag::PASSIVE),
        ])
        .unwrap();
        let commander = 1;
        let health = w.state.units.health[commander];
        let units = vec![w.state.units.id(0)];
        w.tick(&[cmd(if ground_fire {
            Command::AttackGround {
                units,
                pos: w.state.units.pos[commander],
                queue: false,
            }
        } else {
            Command::Attack {
                units,
                target: w.state.units.id(commander),
                queue: false,
            }
        })])
        .unwrap();
        let mut impacts = 0;
        for _ in 0..400 {
            w.tick(&[]).unwrap();
            for e in &w.events {
                if let mc_sim::SimEvent::Impact { pos, on_unit, .. } = e {
                    assert!(!on_unit, "a bomb struck the hull under water");
                    assert!(
                        pos.z.abs() <= Fx::ONE,
                        "bomb burst at {:?}, not the surface",
                        pos.z
                    );
                    impacts += 1;
                }
            }
        }
        if ground_fire {
            assert!(impacts > 0, "no bombs dropped on the point");
            assert!(
                w.state.units.health[commander] < health,
                "the blast did not carry down"
            );
        } else {
            assert_eq!(impacts, 0, "bombed a target it cannot see");
            assert_eq!(
                w.state.units.health[commander], health,
                "bombed under water"
            );
        }
    }
}

#[test]
fn bombs_hit_a_commander_on_dry_land() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_bomber", 350, 0),
        spawn(&w, 1, "aster_commander", 650, flag::PASSIVE),
    ])
    .unwrap();
    let health = w.state.units.health[1];
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(0)],
        target: w.state.units.id(1),
        queue: false,
    })])
    .unwrap();
    for _ in 0..400 {
        w.tick(&[]).unwrap();
    }
    assert!(w.state.units.health[1] < health);
}

#[test]
fn a_second_block_sent_to_the_same_spot_does_not_jostle_forever() {
    // The AI sends each new batch to its staging point while the last batch
    // is still settling there: the two blocks' ranks overlap. Neither may
    // hold its order for ever, shoving the other round in a circle.
    let mut w = world();
    let ids = group(&mut w, "aster_t1_tank", 30);
    let target = FxVec2::from_ints(900, 650);
    let (first, second) = ids.split_at(15);
    w.tick(&[cmd(Command::AttackMove {
        units: first.to_vec(),
        target,
        queue: false,
    })])
    .unwrap();
    for _ in 0..30 {
        w.tick(&[]).unwrap();
    }
    w.tick(&[cmd(Command::AttackMove {
        units: second.to_vec(),
        target,
        queue: false,
    })])
    .unwrap();
    for _ in 0..1200 {
        w.tick(&[]).unwrap();
    }
    let busy: Vec<_> = ids
        .iter()
        .map(|&id| w.state.units.row(id).unwrap())
        .filter(|&r| w.state.orders.front(&w.state.units, r).is_some())
        .map(|r| {
            let o = *w.state.orders.front(&w.state.units, r).unwrap();
            (
                w.state.units.pos[r],
                o.pos + o.offset,
                w.state.units.flags[r],
                w.state.units.stuck_ticks[r],
                w.state
                    .formations
                    .get(&o.formation)
                    .map(|g| (g.anchor, g.phase)),
                o.pos,
            )
        })
        .collect();
    assert!(
        busy.is_empty(),
        "{} tanks still jostling: {busy:?}",
        busy.len()
    );
}
