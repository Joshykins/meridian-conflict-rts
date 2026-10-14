//! Probe: what the HUD costs the CPU a frame with 64 000 fighters selected, among as many
//! of the enemy's, all in one command group. It prints the HUD's spans for frames between
//! the sim's ticks, with a tick (a pull) before every frame, and with one every sixth
//! frame, as at 60 frames a second. Run:
//!
//!     cargo test --profile gate -p mc-game zz_hud_64k -- --ignored --nocapture

use super::*;
use mc_core::FxVec2;
use std::time::Instant;

#[test]
#[ignore = "probe: prints timings, asserts nothing"]
fn zz_hud_64k_selected() {
    const SIDE: u32 = 64_000;
    let mut rig = Rig::new("aster_t1_interceptor");
    let proto = rig.view.frame.units[0];
    rig.view.frame.units.clear();
    rig.view.index_of.clear();
    let mut own = Vec::new();
    for i in 0..2 * SIDE {
        let id = mc_sim::Handle::new(i as usize, 1).0;
        let at = [
            1000.0 + (i % 400) as f32 * 8.0,
            1000.0 + (i / 400) as f32 * 8.0,
            300.0,
        ];
        let mut u = proto;
        u.unit_id = id;
        u.owner_flags = u32::from(i >= SIDE);
        u.pos = at;
        u.prev_pos = at;
        rig.view.index_of.insert(id, i as usize);
        rig.view.frame.units.push(u);
        if i < SIDE {
            own.push(id);
            rig.view.status.queues.push(UnitOrders {
                unit_id: id,
                orders: vec![QueuedOrder {
                    formation: 77,
                    offset: [0.0; 2],
                    moving_slot: None,
                    formation_phase: 0,
                    kind: OrderKind::Move,
                    pos: [6000.0, 6000.0],
                    at: FxVec2::from_ints(6000, 6000),
                    blueprint: mc_data::BlueprintId(0),
                    radius: 0.0,
                }],
                ..Default::default()
            });
            rig.view.frame.fuel.push(mc_sim::mirror::FuelView {
                unit_id: id,
                fill: 0.8,
                dry_in: None,
            });
        }
    }
    rig.view.selection = own.clone().into();
    rig.view.groups.set(1, &own);
    let pull = |rig: &mut Rig| {
        rig.view.frame.tick += 1;
        let v = &mut rig.view;
        v.rows.rebuild(
            v.frame.tick,
            &v.frame.units,
            &v.status.queues,
            &v.frame.fuel,
        );
    };
    pull(&mut rig);
    rig.settle();
    // Frames with a pull every `every` frames: never, every frame, and as at 60 frames a
    // second with the sim's ten ticks a second.
    let time = |rig: &mut Rig, every: Option<usize>, title: &str| {
        let mut report = mc_core::perf::Report::new(title);
        for frame in 0..36 {
            if every.is_some_and(|n| frame % n == 0) {
                pull(rig);
            }
            let scope = mc_core::perf::Scope::begin();
            let start = Instant::now();
            rig.frame(&Input::default());
            let mut frame = scope.end();
            frame.push("probe.frame", 1, Some(start.elapsed().as_nanos() as u64));
            report.add(&frame, "probe.frame");
        }
        println!("{}", report.text());
    };
    time(&mut rig, None, "HUD, frames between pulls");
    time(&mut rig, Some(1), "HUD, a pull every frame");
    time(&mut rig, Some(6), "HUD, a pull every sixth frame");
}
