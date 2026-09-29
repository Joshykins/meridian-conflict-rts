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

const COURIER: &str = "aster_t1_lift_ship";
const FRIGATE: &str = "aster_t3_frigate";
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
fn a_mark_beyond_the_drive_is_brought_in_to_its_range() {
    let mut w = world();
    let ship = add(&mut w, COURIER, 0, 2000, 3000);
    run(&mut w, seconds(10));
    warp(&mut w, ship, 14000, 3000);
    until(&mut w, ship, WarpPhase::Transit, seconds(10)).expect("it never jumped");
    until(&mut w, ship, WarpPhase::Idle, seconds(10)).expect("it never came out");
    let x = w.state.units.pos[row(&w, ship)].x.to_f32();
    assert!(
        (7990.0..8010.0).contains(&x),
        "a 6 km drive took it to x {x}"
    );
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
    assert!(energy(COURIER) < energy("aster_t2_lift_ship"));
    assert!(energy("aster_t2_lift_ship") < energy(FRIGATE));
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
    let foe = add(&mut snagged, "aster_t2_lift_ship", 1, 9300, 3000);
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
