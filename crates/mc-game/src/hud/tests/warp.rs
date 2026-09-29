//! Warp on the HUD: the order card's Warp button, and what the unit card says of a jump,
//! a stun, the drive's recharge and a dampener's field.

use super::*;
use crate::game::Targeting;
use mc_sim::mirror::{DamperView, WarpView};
use mc_sim::tables::WarpPhase;

/// What the rig's HUD reads, for asking the card's lines directly.
fn scene<'a>(rig: &'a Rig, stats: &'a FrameStats) -> Scene<'a> {
    Scene {
        view: &rig.view,
        blueprints: &rig.blueprints,
        map: &rig.map,
        camera: &rig.camera,
        gpu: stats,
        hover: None,
        show_reclaim: false,
        placing: None,
        net: None,
        net_notices: &[],
    }
}

fn spooling(charge: f32) -> WarpView {
    WarpView {
        unit_id: 7,
        owner: 0,
        blueprint: BlueprintId(0),
        phase: WarpPhase::Spool,
        ticks: 20,
        length: 30,
        from: [4000.0, 4000.0, 300.0],
        to: [8000.0, 4000.0, 300.0],
        bearing: 0.0,
        radius: 20.0,
        dampened: false,
        charge,
        aligned: true,
        energy: 1500.0,
        draw: 500.0,
    }
}

#[test]
fn a_ship_with_a_drive_is_offered_warp_on_its_card() {
    let mut rig = Rig::new("aster_t1_lift_ship");
    let warp = HudAction::Target(Targeting::Warp);
    let found = (0..6).any(|col| {
        (0..4).any(|row| {
            let at = order_slot(col, row) + Vec2::new(20.0, 0.0);
            rig.click(at).contains(&warp)
        })
    });
    assert!(found, "no Warp on a Courier's card");
    // A tank has no drive and no button.
    let mut rig = Rig::new("aster_t1_tank");
    let found = (0..6).any(|col| {
        (0..4).any(|row| {
            let at = order_slot(col, row) + Vec2::new(20.0, 0.0);
            rig.click(at).contains(&warp)
        })
    });
    assert!(!found, "a tank was offered Warp");
}

#[test]
fn the_card_tells_a_jump_a_stun_and_the_drives_recharge() {
    let stats = FrameStats::default();
    let mut rig = Rig::new("aster_t1_lift_ship");
    let bp = rig
        .blueprints
        .unit(rig.blueprints.id_of("aster_t1_lift_ship").unwrap());
    // Ready: its charge and the key.
    let s = scene(&rig, &stats);
    let u = s.view.frame.units[0];
    let (label, value, ..) = super::super::warp::drive_line(&s, &u, bp).expect("a drive line");
    assert_eq!(label, "Warp drive ready  \u{b7}  O");
    assert_eq!(value, "1,500 E");
    // Charging: the activity line and its share.
    rig.view.frame.warps = vec![spooling(0.64)];
    let s = scene(&rig, &stats);
    let a = super::super::warp::activity(&s, &u).expect("charging");
    assert_eq!(
        (a.label.as_str(), a.value.as_str(), a.progress),
        ("Charging warp", "64%", Some(0.64))
    );
    assert!(super::super::warp::drive_line(&s, &u, bp).is_none());
    // In warp: the seconds left.
    let mut transit = spooling(1.0);
    transit.phase = WarpPhase::Transit;
    transit.ticks = 10;
    transit.length = 40;
    rig.view.frame.warps = vec![transit];
    let s = scene(&rig, &stats);
    let a = super::super::warp::activity(&s, &u).expect("in warp");
    assert_eq!((a.label.as_str(), a.value.as_str()), ("In warp", "3 s"));
    // Out and recharging: the drive's line counts down.
    rig.view.frame.warps.clear();
    rig.view.status.queues = vec![UnitOrders {
        unit_id: 7,
        warp_recharge: 31.2,
        ..Default::default()
    }];
    let s = scene(&rig, &stats);
    assert!(super::super::warp::activity(&s, &u).is_none());
    let (label, value, share, ..) = super::super::warp::drive_line(&s, &u, bp).unwrap();
    assert_eq!(
        (label.as_str(), value.as_str()),
        ("Warp drive recharging", "32 s")
    );
    assert!(share.is_some_and(|p| (p - (1.0 - 31.2 / 40.0)).abs() < 1e-4));
    // Stunned: electric, with the seconds, over anything else.
    rig.view.status.queues[0].stunned = 17.4;
    let s = scene(&rig, &stats);
    let a = super::super::warp::activity(&s, &u).expect("stunned");
    assert_eq!(a.tone, super::super::warp::STUN);
    assert_eq!(a.value, "18 s");
    // The card draws all of it without trouble.
    rig.view.frame.warps = vec![spooling(0.3)];
    rig.settle();
}

#[test]
fn a_dampeners_card_says_whether_its_field_is_up() {
    let stats = FrameStats::default();
    let mut rig = Rig::new("aster_t2_warp_damper");
    let bp = rig
        .blueprints
        .unit(rig.blueprints.id_of("aster_t2_warp_damper").unwrap());
    let field = |live| DamperView {
        unit_id: 7,
        owner: 0,
        pos: [4000.0, 4000.0, 0.0],
        radius: 1600.0,
        live,
    };
    rig.view.frame.dampers = vec![field(true)];
    let s = scene(&rig, &stats);
    let u = s.view.frame.units[0];
    let (label, value, ..) = super::super::warp::damper_line(&s, &u, bp).unwrap();
    assert_eq!(
        (label.as_str(), value.as_str()),
        ("Warp field up", "1,600 m")
    );
    rig.view.frame.dampers = vec![field(false)];
    let s = scene(&rig, &stats);
    let (label, ..) = super::super::warp::damper_line(&s, &u, bp).unwrap();
    assert_eq!(label, "Warp field down  \u{b7}  no power");
    rig.settle();
}
