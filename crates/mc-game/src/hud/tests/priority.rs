//! A unit's own priority in a stall: Last / Even / First on the queue strip beside Pause,
//! and small in the head of the order card's last column.

use super::*;
use mc_sim::focus::Priority;
use mc_sim::mirror::UNIT_PRIORITY_SHIFT;

/// The strip's Pause switch, on a factory's strip (Repeat is right of it).
fn pause() -> Vec2 {
    let repeat = Vec2::new(1920.0 - EDGE - 12.0 - 48.0, BUILD_Y - GAP - 31.0);
    repeat - Vec2::X * (48.0 + 10.0 + 48.0)
}

/// The strip's Priority segment `n` (0 Last, 1 Even, 2 First): 120 wide, 10 left of Pause.
fn strip_segment(n: f32) -> Vec2 {
    let right = pause().x - 48.0 - 10.0;
    Vec2::new(right - 120.0 + 20.0 + n * 40.0, pause().y)
}

/// The order card's small Priority segment `n` in column `col`'s head.
fn card_segment(col: usize, n: f32) -> Vec2 {
    let cx = ORDERS_X + 14.0 + col as f32 * (selection::ORDER_W + selection::ORDER_GAP);
    Vec2::new(
        cx + selection::ORDER_W - 54.0 + 9.0 + n * 18.0,
        DECK_Y + 24.5,
    )
}

fn set(rig: &mut Rig, p: Priority) {
    rig.view.frame.units[0].status[0] |= (p as u32) << UNIT_PRIORITY_SHIFT;
    rig.settle();
}

#[test]
fn the_strip_sets_a_factorys_priority_and_the_lit_one_goes_back_to_even() {
    let mut rig = Rig::new("aster_t1_land_factory");
    assert_eq!(
        rig.click(strip_segment(2.0)),
        vec![HudAction::Priority(Priority::First)]
    );
    assert_eq!(
        rig.click(strip_segment(0.0)),
        vec![HudAction::Priority(Priority::Last)]
    );
    set(&mut rig, Priority::First);
    assert_eq!(
        rig.click(strip_segment(2.0)),
        vec![HudAction::Priority(Priority::Even)]
    );
    assert_eq!(
        rig.click(strip_segment(1.0)),
        vec![HudAction::Priority(Priority::Even)]
    );
}

#[test]
fn the_card_head_sets_an_idle_engineers_priority_and_a_tank_has_none() {
    // An idle engineer has no queue, so no strip: its card's last column (Work) has it.
    let mut rig = Rig::new("aster_t1_engineer");
    let col = selection::order_families(
        &Scene {
            view: &rig.view,
            blueprints: &rig.blueprints,
            map: &rig.map,
            camera: &rig.camera,
            gpu: &FrameStats::default(),
            hover: None,
            show_reclaim: false,
            placing: None,
            placing_open: None,
            net: None,
            net_notices: &[],
        },
        &[&rig.view.frame.units[0]],
    ) - 1;
    assert_eq!(
        rig.click(card_segment(col, 2.0)),
        vec![HudAction::Priority(Priority::First)]
    );
    set(&mut rig, Priority::Last);
    assert_eq!(
        rig.click(card_segment(col, 0.0)),
        vec![HudAction::Priority(Priority::Even)]
    );
    // A tank has no work to order.
    let mut rig = Rig::new("aster_t1_tank");
    assert_eq!(rig.click(card_segment(3, 2.0)), vec![]);
}
