//! A factory's batch on the queue strip: the Batch switch, and while it is on, the size
//! stepper joined to its left and Send beside that while some are ready. The strip folds
//! to fit a narrow screen.

use super::*;
use mc_sim::mirror::{BatchView, UNIT_BATCH};

/// The strip's Repeat switch, and each switch a step left of it.
fn strip(step: f32) -> Vec2 {
    let repeat = Vec2::new(1920.0 - EDGE - 12.0 - 48.0, BUILD_Y - GAP - 31.0);
    Vec2::new(repeat.x - step * (48.0 + 10.0 + 48.0), repeat.y)
}

/// The Batch switch: left of Pause, past the Priority control (120 wide).
fn batch_switch() -> Vec2 {
    strip(1.0) - Vec2::X * (48.0 + 10.0 + 120.0 + 10.0 + 48.0)
}

/// A factory batching with `count` of `size` ready.
fn batching(rig: &mut Rig, count: u16, size: u16) {
    rig.view.frame.units[0].status[0] |= UNIT_BATCH;
    rig.view.status.queues = vec![UnitOrders {
        unit_id: 7,
        batch: Some(BatchView {
            group: 0,
            count,
            size,
            units: (0..u32::from(count)).map(|i| 20 + i).collect(),
            next: Some([100.0, 20.0]),
            linked: vec![7],
        }),
        ..Default::default()
    }];
    rig.settle();
}

#[test]
fn a_batching_factory_sets_its_size_and_sends_what_is_ready_early() {
    let mut rig = Rig::new("aster_t1_land_factory");
    let batch = batch_switch();
    assert_eq!(rig.click(batch), vec![HudAction::Batch(true)]);

    // On, with two of ten ready: the switch turns it off, the stepper beside it sets the
    // size, and Send beside that lets the two go.
    batching(&mut rig, 2, 10);
    assert_eq!(rig.click(batch), vec![HudAction::Batch(false)]);
    let size = Vec2::new(batch.x - 48.0 - 4.0 - 38.0, batch.y);
    assert_eq!(
        rig.click(size - Vec2::X * 25.0),
        vec![HudAction::BatchSize(9)]
    );
    assert_eq!(
        rig.click(size + Vec2::X * 25.0),
        vec![HudAction::BatchSize(11)]
    );
    let send = Vec2::new(size.x - 38.0 - 4.0 - 32.0, batch.y);
    assert_eq!(rig.click(send), vec![HudAction::SendBatch]);

    // Nothing ready: no Send. The size goes no lower than one.
    batching(&mut rig, 0, 1);
    assert_eq!(rig.click(send), vec![]);
    assert_eq!(rig.click(size - Vec2::X * 25.0), vec![]);
    // Pause and Repeat stay where they were.
    assert_eq!(rig.click(strip(1.0)), vec![HudAction::PauseWork(true)]);
    assert_eq!(rig.click(strip(0.0)), vec![HudAction::Repeat(true)]);
}

#[test]
fn on_a_narrow_screen_the_switches_fold_to_their_glyphs() {
    // A 4:3 screen: the canvas is 1440 points across, not 1920.
    let mut rig = Rig::sized("aster_t1_land_factory", Vec2::new(1440.0, 1080.0));
    batching(&mut rig, 3, 10);
    // Folded switches are 40 wide, 6 apart, from the strip's right end.
    let repeat = Vec2::new(1440.0 - EDGE - 12.0 - 20.0, BUILD_Y - GAP - 31.0);
    assert_eq!(rig.click(repeat), vec![HudAction::Repeat(true)]);
    let pause = repeat - Vec2::X * 46.0;
    assert_eq!(rig.click(pause), vec![HudAction::PauseWork(true)]);
    // Priority folds to its chevrons, 60 wide, between Pause and Batch.
    let batch = pause - Vec2::X * (20.0 + 6.0 + 60.0 + 6.0 + 20.0);
    assert_eq!(rig.click(batch), vec![HudAction::Batch(false)]);
    let size = batch - Vec2::X * (20.0 + 3.0 + 31.0);
    assert_eq!(
        rig.click(size + Vec2::X * 20.0),
        vec![HudAction::BatchSize(11)]
    );
}

#[test]
fn factories_count_as_batched_only_when_linked_in_one() {
    use mc_sim::mirror::UnitOrders;
    let q = |id: u32, group: Option<u32>| UnitOrders {
        unit_id: id,
        batch: group.map(|group| BatchView {
            group,
            ..Default::default()
        }),
        ..Default::default()
    };
    let queues = [q(1, Some(4)), q(2, Some(4)), q(3, Some(9)), q(5, None)];
    let linked =
        |ids: &[u32]| super::super::build::batch::batch_linked(&queues, ids.iter().copied());
    assert_eq!(linked(&[1, 2]), (2, 2), "one batch: all on");
    assert_eq!(
        linked(&[1, 2, 3]),
        (2, 3),
        "two batches: a click links them"
    );
    assert_eq!(linked(&[3, 5]), (1, 2));
    assert_eq!(linked(&[5]), (0, 1));
}
