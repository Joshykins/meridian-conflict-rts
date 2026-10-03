//! Warp (`warp.rs`): capital ships charge their drive off the grid, jump, and come out;
//! an enemy warp dampener drags a jump that ends in its field and throws the ship out
//! hurt and stunned.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::RenderFrame;
use mc_sim::tables::{Controller, UnitId, WarpPhase};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const COURIER: &str = "aster_t2_lift_ship";
const FRIGATE: &str = "aster_t4_frigate";
const DAMPER: &str = "aster_t2_warp_damper";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(2048, 2048, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(15000, 15000)],
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
    for p in &mut w.state.players {
        p.bonus_storage[1] = Fx::from_int(200_000);
        p.energy = Fx::from_int(100_000);
    }
    w
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn seconds(s: u32) -> usize {
    (s * TICKS_PER_SECOND) as usize
}

fn run(w: &mut World, ticks: usize) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

fn warp(w: &mut World, ship: UnitId, x: i32, y: i32) {
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Warp {
            units: vec![ship],
            pos: FxVec2::from_ints(x, y),
            queue: false,
        },
    }])
    .unwrap();
}

/// Ticks until `ship` is in `phase`, at most `limit`.
fn until(w: &mut World, ship: UnitId, phase: WarpPhase, limit: usize) -> Option<usize> {
    for t in 0..limit {
        if w.state.units.warp[row(w, ship)].phase == phase {
            return Some(t);
        }
        w.tick(&[]).unwrap();
    }
    None
}

#[test]
fn a_ship_still_charges_after_its_nose_comes_onto_the_mark() {
    let mut w = world();
    let ship = add(&mut w, FRIGATE, 0, 6000, 6000);
    run(&mut w, seconds(10));
    // Due south, a half turn behind it: the turn outlasts the charge it may take.
    let mark = FxVec2::from_ints(6000, 2000);
    warp(&mut w, ship, 6000, 2000);
    let drive = w.bp(row(&w, ship)).warp.unwrap();
    let mut lined_up = None;
    for t in 0..seconds(30) {
        let r = row(&w, ship);
        let state = w.state.units.warp[r];
        if state.phase == WarpPhase::Transit {
            let at = lined_up.expect("it jumped before its nose was on the mark");
            assert!(
                t - at >= drive.spool_ticks as usize / 3,
                "it jumped {} ticks after lining up; a third of its {}-tick charge is left then",
                t - at,
                drive.spool_ticks
            );
            return;
        }
        let bearing = (mark - w.state.units.pos[r]).angle();
        let on = w.state.units.heading[r].delta_to(bearing).unsigned_abs() <= 546;
        if state.phase == WarpPhase::Spool && !on {
            assert!(
                state.charge <= drive.energy * Fx::ratio(2, 3),
                "it charged past two thirds while still turning"
            );
        }
        if on && lined_up.is_none() && state.phase == WarpPhase::Spool {
            lined_up = Some(t);
        }
        w.tick(&[]).unwrap();
    }
    panic!("it never jumped");
}

#[test]
fn a_courier_charges_off_the_grid_jumps_and_comes_out_where_it_was_sent() {
    let mut w = world();
    let ship = add(&mut w, COURIER, 0, 3000, 3000);
    run(&mut w, seconds(10));
    let before = w.state.players[0].energy;
    // Due north: it turns a quarter turn onto the mark while it charges.
    warp(&mut w, ship, 3000, 7000);
    let charged = until(&mut w, ship, WarpPhase::Transit, seconds(20)).expect("it never jumped");
    assert!(
        (seconds(2)..seconds(6)).contains(&charged),
        "a 3 s charge took {charged} ticks"
    );
    let spent = (before - w.state.players[0].energy).to_f32();
    assert!(
        (1400.0..1600.0).contains(&spent),
        "a jump's charge took {spent} energy, not 1500"
    );
    until(&mut w, ship, WarpPhase::Idle, seconds(10)).expect("it never came out");
    let r = row(&w, ship);
    let at = w.state.units.pos[r];
    assert!(
        at.distance(FxVec2::from_ints(3000, 7000)) < Fx::from_int(5),
        "it came out at {at:?}"
    );
    assert!(
        w.state.units.warp[r].recharge > 0,
        "the drive did not start its recharge"
    );
    assert!(
        w.state.orders.front(&w.state.units, r).is_none(),
        "the order was left in hand"
    );
}

#[test]
fn a_jump_reaches_across_the_whole_map() {
    let mut w = world();
    let ship = add(&mut w, COURIER, 0, 1000, 1000);
    run(&mut w, seconds(10));
    warp(&mut w, ship, 15000, 15000);
    until(&mut w, ship, WarpPhase::Transit, seconds(10)).expect("it never jumped");
    until(&mut w, ship, WarpPhase::Idle, seconds(20)).expect("it never came out");
    let at = w.state.units.pos[row(&w, ship)];
    assert!(
        at.distance(FxVec2::from_ints(15000, 15000)) < Fx::from_int(5),
        "a 20 km jump came out at {at:?}"
    );
}

#[test]
fn ships_sent_together_come_out_in_the_formation_they_left_in() {
    let mut w = world();
    let spots = [(3000, 3000), (3400, 3000), (3200, 3500)];
    let ships: Vec<UnitId> = spots
        .iter()
        .map(|&(x, y)| add(&mut w, COURIER, 0, x, y))
        .collect();
    run(&mut w, seconds(10));
    let before: Vec<FxVec2> = ships
        .iter()
        .map(|&s| w.state.units.pos[row(&w, s)])
        .collect();
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Warp {
            units: ships.clone(),
            pos: FxVec2::from_ints(10000, 9000),
            queue: false,
        },
    }])
    .unwrap();
    for &s in &ships {
        until(&mut w, s, WarpPhase::Idle, seconds(30)).expect("it never came out");
    }
    let mut middle = FxVec2::ZERO;
    for &s in &ships {
        middle += w.state.units.pos[row(&w, s)] * Fx::ratio(1, 3);
    }
    assert!(
        middle.distance(FxVec2::from_ints(10000, 9000)) < Fx::from_int(10),
        "the group came out about {middle:?}"
    );
    for (i, &s) in ships.iter().enumerate() {
        for (j, &t) in ships.iter().enumerate().skip(i + 1) {
            let now = w.state.units.pos[row(&w, t)] - w.state.units.pos[row(&w, s)];
            let was = before[j] - before[i];
            assert!(
                (now - was).length() < Fx::from_int(10),
                "ships {i} and {j} stood {was:?} apart and came out {now:?} apart"
            );
        }
    }
}

#[test]
fn a_starved_grid_charges_the_drive_only_as_far_as_it_can_pay() {
    let mut w = world();
    let ship = add(&mut w, COURIER, 0, 3000, 3000);
    run(&mut w, seconds(10));
    // Half a charge in the store and nothing coming in.
    w.state.players[0].energy = Fx::from_int(750);
    warp(&mut w, ship, 7000, 3000);
    assert!(
        until(&mut w, ship, WarpPhase::Transit, seconds(15)).is_none(),
        "it jumped on half a charge"
    );
    let r = row(&w, ship);
    let charge = w.state.units.warp[r].charge.to_f32();
    assert_eq!(w.state.units.warp[r].phase, WarpPhase::Spool);
    assert!(
        (700.0..=760.0).contains(&charge),
        "charged {charge} of 750 to be had"
    );
    w.state.players[0].energy = Fx::from_int(5000);
    until(&mut w, ship, WarpPhase::Transit, seconds(5)).expect("it never finished its charge");
}

#[test]
fn a_larger_ship_draws_more_for_its_jump() {
    let blueprints =
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let energy = |key: &str| {
        let bp = blueprints.unit(blueprints.id_of(key).unwrap());
        bp.warp.expect("a drive").energy
    };
    assert!(energy(COURIER) < energy("aster_t3_lift_ship"));
    assert!(energy("aster_t3_lift_ship") < energy(FRIGATE));
}

#[test]
fn a_jump_into_an_enemy_dampener_drags_and_throws_the_ship_out_hurt_and_stunned() {
    let mut clean = world();
    let mut snagged = world();
    let mut ships = Vec::new();
    for (w, damper) in [(&mut clean, false), (&mut snagged, true)] {
        let ship = add(w, FRIGATE, 0, 3000, 3000);
        if damper {
            add(w, DAMPER, 1, 9000, 3600);
        }
        run(w, seconds(20));
        warp(w, ship, 9000, 3000);
        until(w, ship, WarpPhase::Transit, seconds(20)).expect("it never jumped");
        ships.push(ship);
    }
    let (a, b) = (ships[0], ships[1]);
    let clean_transit = until(&mut clean, a, WarpPhase::Emerge, seconds(60)).unwrap();
    let snag_transit = until(&mut snagged, b, WarpPhase::Emerge, seconds(60)).unwrap();
    assert!(
        snag_transit >= clean_transit * 5 / 2,
        "a dampened transit took {snag_transit} ticks, a clean one {clean_transit}"
    );
    let full = snagged
        .blueprints
        .unit(snagged.blueprints.id_of(FRIGATE).unwrap())
        .health;
    let r = row(&snagged, b);
    let lost = (full - snagged.state.units.health[r]).to_f32() / full.to_f32();
    assert!((0.24..0.26).contains(&lost), "it lost {lost} of its health");
    assert_eq!(
        clean.state.units.health[row(&clean, a)],
        full,
        "the clean jump hurt"
    );
    assert!(snagged.state.units.stun[r][0] > 0, "it was not stunned");
    // Stunned: it lists, holds fire and ignores orders.
    let foe = add(&mut snagged, "aster_t3_lift_ship", 1, 9300, 3000);
    let foe_health = snagged.state.units.health[row(&snagged, foe)];
    let z = snagged.state.units.z[r];
    warp(&mut snagged, b, 3000, 3000);
    run(&mut snagged, seconds(15));
    let r = row(&snagged, b);
    assert!(
        snagged.state.units.bank[r].unsigned_abs() > 2000,
        "it did not list: bank {}",
        snagged.state.units.bank[r]
    );
    assert!(
        snagged.state.units.z[r] < z - Fx::from_int(20),
        "it did not sink"
    );
    assert_eq!(
        snagged.state.units.warp[r].phase,
        WarpPhase::Idle,
        "it spooled while stunned"
    );
    assert_eq!(
        snagged.state.units.health[row(&snagged, foe)],
        foe_health,
        "it fired while stunned"
    );
    // The stun wears off (25 s) and it carries out the order it was given meanwhile.
    run(&mut snagged, seconds(12));
    assert_eq!(snagged.state.units.stun[r][0], 0);
    let jumped = until(&mut snagged, b, WarpPhase::Spool, seconds(90));
    assert!(jumped.is_some(), "it did not take up its orders again");
}

#[test]
fn the_ships_own_side_never_sees_its_jump_dampened() {
    let mut w = world();
    let ship = add(&mut w, FRIGATE, 0, 3000, 3000);
    let damper = add(&mut w, DAMPER, 1, 9000, 3600);
    run(&mut w, seconds(20));
    let mut frame = RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    assert!(frame.dampers.is_empty(), "the enemy's field is shown");
    w.write_render_frame(Some(1), &mut frame);
    assert!(frame.dampers.iter().any(|d| d.unit_id == damper.0));
    warp(&mut w, ship, 9000, 3000);
    until(&mut w, ship, WarpPhase::Transit, seconds(20)).expect("it never jumped");
    w.tick(&[]).unwrap();
    let clean = |w: &World| w.state.units.warp[row(w, ship)].length;
    w.write_render_frame(Some(0), &mut frame);
    let ours = frame.warps.iter().find(|j| j.unit_id == ship.0).unwrap();
    assert!(!ours.dampened, "its own side sees the snag");
    assert!(ours.length < clean(&w), "its own side sees the drag");
    w.write_render_frame(Some(1), &mut frame);
    let theirs = frame.warps.iter().find(|j| j.unit_id == ship.0).unwrap();
    assert!(theirs.dampened && theirs.length == clean(&w));
    // It comes out, hurt, as a clean jump to its own side.
    for _ in 0..seconds(60) {
        w.tick(&[]).unwrap();
        let arrived = |f: &RenderFrame| {
            f.events.iter().find_map(|e| match e {
                SimEvent::WarpArrived { dampened, .. } => Some(*dampened),
                _ => None,
            })
        };
        w.write_render_frame(Some(0), &mut frame);
        if let Some(dampened) = arrived(&frame) {
            assert!(!dampened, "its own side hears a dampened exit");
            let u = frame.units.iter().find(|u| u.unit_id == ship.0).unwrap();
            assert_eq!(u.fx[..2], [1.0, 0.0], "its own side sees it torn out");
            w.write_render_frame(None, &mut frame);
            assert_eq!(arrived(&frame), Some(true));
            return;
        }
    }
    panic!("it never came out");
}

#[test]
fn destroying_the_dampener_before_the_ship_comes_out_spares_it() {
    let mut w = world();
    let ship = add(&mut w, FRIGATE, 0, 3000, 3000);
    let damper = add(&mut w, DAMPER, 1, 9000, 3600);
    run(&mut w, seconds(20));
    warp(&mut w, ship, 9000, 3000);
    until(&mut w, ship, WarpPhase::Transit, seconds(20)).expect("it never jumped");
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::DebugRemove {
            units: vec![damper],
        },
    }])
    .unwrap();
    until(&mut w, ship, WarpPhase::Emerge, seconds(60)).expect("it never came out");
    let r = row(&w, ship);
    assert_eq!(
        w.state.units.stun[r][0], 0,
        "stunned by a dampener that is gone"
    );
    assert_eq!(
        w.state.units.health[r],
        w.blueprints.unit(w.state.units.blueprint[r]).health
    );
}

#[test]
fn a_dampener_on_our_own_side_leaves_our_jumps_alone() {
    let mut w = world();
    let ship = add(&mut w, COURIER, 0, 3000, 3000);
    add(&mut w, DAMPER, 0, 6000, 3300);
    run(&mut w, seconds(10));
    warp(&mut w, ship, 6000, 3000);
    until(&mut w, ship, WarpPhase::Emerge, seconds(20)).expect("it never came out");
    assert_eq!(w.state.units.stun[row(&w, ship)][0], 0);
}

#[test]
fn in_warp_it_is_drawn_leaving_then_listed_for_its_side_and_unseen_by_the_enemy() {
    let mut w = world();
    let ship = add(&mut w, COURIER, 0, 3000, 3000);
    run(&mut w, seconds(10));
    warp(&mut w, ship, 7000, 3000);
    let mut events = w.events.clone();
    for _ in 0..seconds(10) {
        w.tick(&[]).unwrap();
        events.extend(w.events.iter().cloned());
        if w.state.units.warp[row(&w, ship)].phase == WarpPhase::Transit {
            break;
        }
    }
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::WarpSpooling { .. })));
    assert!(w.events.iter().any(|e| matches!(
        e,
        SimEvent::WarpJumped {
            dampened: false,
            ..
        }
    )));
    let mut frame = RenderFrame::default();
    let find = |frame: &RenderFrame| frame.units.iter().find(|u| u.unit_id == ship.0).copied();
    // The tick it jumps: drawn where it left, streaking out.
    w.write_render_frame(Some(0), &mut frame);
    let u = find(&frame).expect("not drawn as it left");
    assert!(u.pos[0] < 3100.0, "drawn at {:?}, not where it left", u.pos);
    assert_eq!(u.fx[..2], [0.0, 1.0]);
    assert_eq!(frame.warps.len(), 1);
    w.tick(&[]).unwrap();
    w.write_render_frame(Some(0), &mut frame);
    assert!(find(&frame)
        .expect("dropped from its side's list")
        .in_warp());
    w.write_render_frame(Some(1), &mut frame);
    assert!(find(&frame).is_none(), "the enemy sees a ship in warp");
    // Coming out: a streak to nothing.
    until(&mut w, ship, WarpPhase::Emerge, seconds(10)).unwrap();
    w.write_render_frame(Some(0), &mut frame);
    let u = find(&frame).unwrap();
    assert!(!u.in_warp());
    assert_eq!(u.fx[..2], [1.0, 0.0]);
}

#[test]
fn the_interface_hears_how_long_the_drive_recharges_and_the_stun_lasts() {
    let mut w = world();
    let ship = add(&mut w, COURIER, 0, 3000, 3000);
    run(&mut w, seconds(10));
    let listed = |w: &World| {
        let mut out = Vec::new();
        w.write_orders(Some(0), &[ship.0], None, &mut out);
        let q = out.pop().expect("the ship is listed");
        (q.warp_recharge, q.stunned)
    };
    assert_eq!(listed(&w), (0.0, 0.0), "a fresh drive is ready");
    warp(&mut w, ship, 3000, 7000);
    until(&mut w, ship, WarpPhase::Idle, seconds(30)).expect("it never came out");
    // The Courier's drive recharges for 40 s after it comes out.
    let (recharge, _) = listed(&w);
    assert!(
        (39.0..=40.0).contains(&recharge),
        "the drive recharges for {recharge} s"
    );
    run(&mut w, seconds(10));
    let (later, _) = listed(&w);
    assert!((later - (recharge - 10.0)).abs() < 0.2, "{later} s left");
    // A stun is listed in seconds.
    let r = row(&w, ship);
    w.state.units.stun[r] = [seconds(18) as u16, seconds(25) as u16];
    let (_, stunned) = listed(&w);
    assert!((stunned - 18.0).abs() < 0.01, "stunned for {stunned} s");
}

#[test]
fn a_ship_in_a_fight_still_turns_onto_its_jump_and_goes() {
    // Its guns lay the hull on an enemy abeam; the jump is due north. The hull is the
    // drive's while it charges, so the fight must not hold the nose off the mark.
    let mut w = world();
    let ship = add(&mut w, FRIGATE, 0, 3000, 3000);
    add(&mut w, FRIGATE, 1, 4200, 3000);
    run(&mut w, seconds(10));
    warp(&mut w, ship, 3000, 9000);
    until(&mut w, ship, WarpPhase::Transit, seconds(30)).expect("it never jumped");
}
