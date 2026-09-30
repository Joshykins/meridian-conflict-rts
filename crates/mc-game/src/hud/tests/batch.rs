//! A factory's batch on the queue strip: the Batch switch, how many are ready, and Send
//! while some wait.

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
            made: 2,
            size: 3,
            places: vec![
                ([100.0, 0.0], true),
                ([100.0, 10.0], true),
                ([100.0, 20.0], false),
            ],
            facing: [1.0, 0.0],
            spacing: 10.0,
        }),
        ..Default::default()
    }];
    rig.settle();
    assert_eq!(rig.click(batch), vec![HudAction::Batch(false)]);
    let send = Vec2::new(batch.x - 48.0 - 10.0 - 31.0, batch.y);
    assert_eq!(rig.click(send), vec![HudAction::SendBatch]);

    // Nothing waiting: Send stays where it is, dark, and Pause with it.
    rig.view.status.queues[0].batch.as_mut().unwrap().made = 0;
    rig.settle();
    assert_eq!(rig.click(send), vec![]);
    let pause = Vec2::new(send.x - 31.0 - 10.0 - 48.0, batch.y);
    assert_eq!(rig.click(pause), vec![HudAction::PauseWork(true)]);
}
