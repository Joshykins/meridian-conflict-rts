//! A queue too long for the strip: the strip's last slot opens the whole of it over the
//! strip, where every entry takes its clicks, and a press away puts it back.

use super::*;

/// The queue strip's top edge, and a point just inside the tray's foot over it.
const STRIP_Y: f32 = DECK_Y - GAP - 62.0;
const TRAY_FOOT: f32 = STRIP_Y - GAP - 8.0;

/// A right-click at `at` on a still HUD.
fn right_tap(rig: &mut Rig, at: Vec2) -> Vec<HudAction> {
    rig.frame(&Input {
        cursor: at,
        ..Default::default()
    });
    rig.frame(&Input {
        cursor: at,
        right_pressed: true,
        ..Default::default()
    })
}

#[test]
fn a_long_queue_opens_whole_over_the_strip() {
    let mut rig = Rig::new("aster_t1_land_factory");
    let builds = rig
        .blueprints
        .unit(rig.blueprints.id_of("aster_t1_land_factory").unwrap())
        .builder
        .as_ref()
        .unwrap()
        .builds
        .clone();
    let order = |b| QueuedOrder {
        formation: 0,
        offset: [0.0; 2],
        moving_slot: None,
        formation_phase: 0,
        kind: OrderKind::Produce,
        pos: [0.0, 0.0],
        at: mc_core::FxVec2::ZERO,
        blueprint: b,
        radius: 0.0,
    };
    // Thirty alternating entries, each its own stack, and a third product only at the end:
    // far more than the strip has room for.
    let mut orders: Vec<QueuedOrder> = (0..30).map(|i| order(builds[i % 2])).collect();
    orders.push(order(builds[2]));
    rig.view.status.queues = vec![UnitOrders {
        unit_id: 7,
        orders,
        progress: 0.4,
        ..Default::default()
    }];
    rig.settle();
    let left = build_x(FACTORY_FAMILIES);
    let above = Vec2::new(left + 100.0, TRAY_FOOT);
    assert!(!rig.hud.covers(above), "the tray starts shut");

    // The last product is on no strip tile.
    let row = STRIP_Y + 31.0;
    let strip_hits: Vec<HudAction> = (0..200)
        .flat_map(|i| right_tap(&mut rig, Vec2::new(left + 14.0 + i as f32 * 5.0, row)))
        .collect();
    assert!(!strip_hits.contains(&HudAction::Cancel(builds[2])));

    // The more tile, at the end of the strip's tiles, opens the tray.
    let opened = (0..200).any(|i| {
        rig.tap(Vec2::new(left + 14.0 + i as f32 * 5.0, row));
        rig.hud.covers(above)
    });
    assert!(opened, "a tile on the strip opens the whole queue");

    // Every entry is there, and takes its right-click.
    let hits: Vec<HudAction> = (0..12)
        .flat_map(|y| (0..120).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            right_tap(
                &mut rig,
                Vec2::new(left + 14.0 + x as f32 * 6.0, TRAY_FOOT - y as f32 * 17.0),
            )
        })
        .collect();
    assert!(hits.contains(&HudAction::Cancel(builds[2])), "{hits:?}");

    // A press out on the battlefield puts it away.
    rig.tap(Vec2::new(900.0, 400.0));
    rig.settle();
    assert!(!rig.hud.covers(above), "a press away shuts the tray");
}
