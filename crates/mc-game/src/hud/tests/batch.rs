//! A factory's batch on the queue strip: the Batch switch, how many are ready, Send while
//! some wait, and the size stepper.

use super::*;
use mc_sim::mirror::{BatchView, UNIT_BATCH};

/// The strip's Repeat switch, and each switch or tile a step left of it.
fn strip(step: f32) -> Vec2 {
    let repeat = Vec2::new(1920.0 - EDGE - 12.0 - 48.0, DECK_Y - GAP - 31.0);
    Vec2::new(repeat.x - step * (48.0 + 10.0 + 48.0), repeat.y)
}

#[test]
fn a_batching_factory_shows_what_is_ready_and_sends_it_early() {
    let mut rig = Rig::new("aster_t1_land_factory");
    let batch = strip(1.0);
    assert_eq!(rig.click(batch), vec![HudAction::Batch(true)]);

    // On, with two of three formed up: the switch turns it off, and Send appears beside it.
    rig.view.frame.units[0].status[0] |= UNIT_BATCH;
    rig.view.status.queues = vec![UnitOrders {
        unit_id: 7,
        batch: Some(BatchView {
            group: 0,
            count: 2,
            size: 3,
            fixed: false,
            units: vec![20, 21],
            next: Some([100.0, 20.0]),
            linked: vec![[100.0, 0.0]],
            index: 0,
        }),
        ..Default::default()
    }];
    rig.settle();
    assert_eq!(rig.click(batch), vec![HudAction::Batch(false)]);
    let send = Vec2::new(batch.x - 48.0 - 10.0 - 31.0, batch.y);
    assert_eq!(rig.click(send), vec![HudAction::SendBatch]);

    // Nothing waiting: Send stays where it is, dark, and Pause with it.
    rig.view.status.queues[0].batch.as_mut().unwrap().count = 0;
    rig.settle();
    assert_eq!(rig.click(send), vec![]);
    // The size stepper beside it: its laps (3) until a step sets a size.
    let size = Vec2::new(send.x - 31.0 - 10.0 - 42.0, batch.y);
    assert_eq!(
        rig.click(size - Vec2::X * 25.0),
        vec![HudAction::BatchSize(Some(2))]
    );
    assert_eq!(
        rig.click(size + Vec2::X * 25.0),
        vec![HudAction::BatchSize(Some(4))]
    );
    assert_eq!(rig.right_click(size), vec![], "no size set to clear");
    let view = rig.view.status.queues[0].batch.as_mut().unwrap();
    (view.fixed, view.size) = (true, 20);
    assert_eq!(rig.right_click(size), vec![HudAction::BatchSize(None)]);
    let pause = Vec2::new(size.x - 42.0 - 10.0 - 48.0, batch.y);
    assert_eq!(rig.click(pause), vec![HudAction::PauseWork(true)]);
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
        |ids: &[u32]| super::super::build::queue::batch_linked(&queues, ids.iter().copied());
    assert_eq!(linked(&[1, 2]), (2, 2), "one batch: all on");
    assert_eq!(
        linked(&[1, 2, 3]),
        (2, 3),
        "two batches: a click links them"
    );
    assert_eq!(linked(&[3, 5]), (1, 2));
    assert_eq!(linked(&[5]), (0, 1));
}
