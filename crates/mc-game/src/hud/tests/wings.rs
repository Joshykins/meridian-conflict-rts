//! A carrier's air wings in the construction panel's place, as a factory's: a tile a wing
//! that queues it, a queue strip whose stacks take a right-click, and Repeat on the strip.

use super::*;
use mc_sim::mirror::WingView;

/// Everything a left click (or a right one) anywhere along the line `y` produced.
fn sweep(rig: &mut Rig, y: f32, right: bool) -> Vec<HudAction> {
    rig.settle();
    (0..192)
        .flat_map(|i| {
            let at = Vec2::new(i as f32 * 10.0 + 5.0, y);
            if right {
                rig.right_click(at)
            } else {
                rig.tap(at)
            }
        })
        .collect()
}

#[test]
fn a_carriers_panel_queues_wings_cancels_them_and_repeats() {
    let mut rig = Rig::new("aster_t3_carrier");
    rig.view.frame.wings = vec![WingView {
        unit_id: 7,
        queue: vec![1, 1],
        progress: 0.4,
        launching: 0,
    }];
    // The tiles: one a wing, each queueing its own.
    let tiles = BUILD_Y + 44.0 + build::STRIP_DROP + 40.0;
    let hits = sweep(&mut rig, tiles, false);
    let wings: Vec<&HudAction> = hits
        .iter()
        .filter(|a| matches!(a, HudAction::QueueWing(_)))
        .collect();
    for i in 0..4 {
        assert!(wings.contains(&&HudAction::QueueWing(i)), "{hits:?}");
    }
    // The strip: the queued stack takes one out, and Repeat is there.
    let strip = BUILD_Y - GAP - 31.0;
    let hits = sweep(&mut rig, strip, true);
    assert!(hits.contains(&HudAction::CancelWing(1)), "{hits:?}");
    let hits = sweep(&mut rig, strip, false);
    assert!(hits.contains(&HudAction::Repeat(true)), "{hits:?}");
    assert!(hits.contains(&HudAction::PauseWork(true)), "{hits:?}");
    assert!(!hits.iter().any(|a| matches!(a, HudAction::Batch(_))));
}
