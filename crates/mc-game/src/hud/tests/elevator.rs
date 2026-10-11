//! A space elevator on the HUD: its build menu queues what it brings down, its panel
//! stands between the order card and the menu, and the queue strip counts down to the
//! next crates instead of a build's progress, and on a narrow screen the build menu keeps
//! its room first.

use super::*;
use mc_sim::mirror::{ElevatorPhase, ElevatorView};

const KEY: &str = "aster_t5_space_elevator";

fn view(phase: ElevatorPhase, riding: u16, riding_materials: f32) -> ElevatorView {
    ElevatorView {
        phase,
        left: 5.2,
        through: 0.4,
        to_touchdown: 5.2,
        approach: 0.83,
        cycle: 0.13,
        period: 30.0,
        payload: 20_000.0,
        trip: [7.0, 18.0, 4.0],
        riding,
        riding_materials,
    }
}

fn produce(b: BlueprintId) -> QueuedOrder {
    QueuedOrder {
        formation: 0,
        offset: [0.0; 2],
        moving_slot: None,
        formation_phase: 0,
        kind: OrderKind::Produce,
        pos: [0.0, 0.0],
        at: mc_core::FxVec2::ZERO,
        blueprint: b,
        radius: 0.0,
    }
}

/// The order card's columns for the rig's selection, and so where the panels after it go.
fn families(rig: &Rig) -> usize {
    let stats = FrameStats::default();
    let s = Scene {
        view: &rig.view,
        blueprints: &rig.blueprints,
        map: &rig.map,
        camera: &rig.camera,
        gpu: &stats,
        hover: None,
        show_reclaim: false,
        placing: None,
        placing_open: None,
        net: None,
        net_notices: &[],
        prefabs: crate::prefabs::Library::none(),
    };
    let u = s.view.frame.units[0];
    selection::order_families(&s, &[&u])
}

#[test]
fn an_elevator_shows_its_crates_and_queues_from_its_build_menu() {
    let mut rig = Rig::new(KEY);
    let bp = rig.blueprints.unit(rig.blueprints.id_of(KEY).unwrap());
    let builds = bp.builder.as_ref().unwrap().builds.clone();
    let heavy = builds
        .iter()
        .copied()
        .max_by_key(|&b| rig.blueprints.unit(b).cost_materials)
        .expect("it brings something down");
    let materials = rig.blueprints.unit(heavy).cost_materials.to_f32();
    rig.view.status.queues = vec![UnitOrders {
        unit_id: 7,
        orders: vec![produce(heavy), produce(heavy), produce(builds[0])],
        elevator: Some(view(ElevatorPhase::Inbound, 2, materials * 2.0)),
        ..Default::default()
    }];
    rig.settle();
    let panel_x = ORDERS_X + selection::orders_width(families(&rig)) + GAP;
    assert!(
        rig.hud.covers(Vec2::new(panel_x + 150.0, DECK_Y + 120.0)),
        "the elevator's panel stands right of the order card"
    );
    // The build menu right of the panel: its first card queues what it brings down.
    let menu_x = panel_x + crate::hud::elevator::WIDTH + GAP;
    let tile = Vec2::new(menu_x + 14.0 + 30.0 + 40.0, BUILD_Y + 44.0 + 32.0 + 40.0);
    let first = first_on_shelves(&rig, KEY, 1);
    assert_eq!(rig.click(tile), vec![HudAction::Build(first)]);
    // Its queue strip is up over the menu, and every phase draws.
    assert!(rig
        .hud
        .covers(Vec2::new(menu_x + 200.0, BUILD_Y - GAP - 30.0)));
    for phase in [
        ElevatorPhase::Tethering,
        ElevatorPhase::Inbound,
        ElevatorPhase::Docked,
        ElevatorPhase::Sinking,
        ElevatorPhase::Away,
    ] {
        rig.view.status.queues[0].elevator = Some(view(phase, 0, 0.0));
        rig.frame(&Input::default());
    }
}

#[test]
fn an_elevator_not_yet_running_still_takes_its_queue() {
    // Under construction, or before the mirror has a view of it: no panel, and the menu
    // sits right of the order card like any factory's.
    let mut rig = Rig::new(KEY);
    rig.settle();
    let menu_x = build_x(families(&rig));
    let tile = Vec2::new(menu_x + 14.0 + 30.0 + 40.0, BUILD_Y + 44.0 + 32.0 + 40.0);
    let first = first_on_shelves(&rig, KEY, 1);
    assert_eq!(rig.click(tile), vec![HudAction::Build(first)]);
}

#[test]
fn the_strip_counts_down_to_the_next_crates() {
    let v = view(ElevatorPhase::Away, 3, 9000.0);
    let (doing, progress, eta) = crate::hud::elevator::strip(&v);
    assert_eq!(doing, "Next Crates");
    assert!((progress - v.approach).abs() < 1e-6 && progress.is_finite());
    assert_eq!(eta, v.to_touchdown);
    let line = |phase| crate::hud::elevator::phase_line(&view(phase, 0, 0.0)).0;
    assert_eq!(line(ElevatorPhase::Tethering), "Lowering cable  6 s");
    assert_eq!(line(ElevatorPhase::Inbound), "Crates inbound  6 s");
    assert_eq!(line(ElevatorPhase::Docked), "Landed \u{2014} unloading");
    assert_eq!(line(ElevatorPhase::Sinking), "Crates sinking");
    assert_eq!(line(ElevatorPhase::Away), "Next crates in 6 s");
}

/// On a narrow screen the panel shrinks and then gives way, so the build menu is always
/// at least `MENU_MIN` wide and its first card still queues what the elevator brings down.
#[test]
fn on_a_narrow_screen_the_build_menu_keeps_its_room() {
    for width in [1920.0f32, 1440.0, 1350.0, 1280.0, 1080.0] {
        let mut rig = Rig::sized(KEY, Vec2::new(width, 1080.0));
        let bp = rig.blueprints.unit(rig.blueprints.id_of(KEY).unwrap());
        let builds = bp.builder.as_ref().unwrap().builds.clone();
        rig.view.status.queues = vec![UnitOrders {
            unit_id: 7,
            orders: vec![produce(builds[0])],
            elevator: Some(view(ElevatorPhase::Away, 1, 100.0)),
            ..Default::default()
        }];
        rig.settle();
        let after_orders = ORDERS_X + selection::orders_width(families(&rig)) + GAP;
        let panel = crate::hud::elevator::width_for(width - EDGE - after_orders);
        let menu_x = after_orders + if panel > 0.0 { panel + GAP } else { 0.0 };
        // Never squeezed under what it needs by the panel: that gives way first.
        assert!(
            width - EDGE - menu_x >= 380.0 || panel == 0.0,
            "{width}: the menu has {} points beside a panel",
            width - EDGE - menu_x
        );
        let want = if width > 1800.0 {
            crate::hud::elevator::WIDTH
        } else if width > 1400.0 {
            190.0
        } else {
            0.0
        };
        assert_eq!(panel, want, "{width}: the panel shrinks, then gives way");
        let tile = Vec2::new(menu_x + 14.0 + 30.0 + 40.0, BUILD_Y + 44.0 + 32.0 + 40.0);
        let first = first_on_shelves(&rig, KEY, 1);
        assert_eq!(rig.click(tile), vec![HudAction::Build(first)], "{width}");
        if panel > 0.0 {
            assert!(rig
                .hud
                .covers(Vec2::new(after_orders + panel * 0.5, DECK_Y + 120.0)));
        }
    }
}
