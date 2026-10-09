//! The Vassal, the two-gun escort frigate. The Suzerain and the Scourge have modules of
//! their own (`suzerain/`, `destroyer/`).
use super::super::kit::{dark_plate, v3};
use super::hull::{coarse_lobes, crest, drive, gun, lobe, mark};
use crate::builder::MeshBuilder;

pub(super) fn frigate(b: &mut MeshBuilder, _tech: u8) {
    dark_plate(b);
    b.block(
        v3(-36.0, -9.0, 3.0),
        v3(-4.0, 9.0, if b.coarse() { 16.0 } else { 11.0 }),
    );
    if b.coarse() {
        coarse_lobes(
            b,
            &[[-32.0, 8.0], [-7.0, 23.0], [44.0, 10.0], [4.0, 11.0]],
            2.0,
            12.0,
        );
    } else {
        b.mirror_y(|b| {
            lobe(
                b,
                &[
                    (v3(-32.0, 9.0, 7.0), 5.0, 4.0),
                    (v3(-10.0, 18.0, 7.0), 7.0, 5.0),
                    (v3(16.0, 19.0, 5.0), 6.0, 3.0),
                    (v3(43.0, 10.0, 3.0), 0.5, 1.0),
                ],
            );
            drive(b, v3(-26.0, 12.0, 1.0), 2.8);
            crest(b, -16.0, 8.0, 10.0, 16.0, 16.0);
        });
    }
    mark(
        b,
        v3(-8.0, 0.0, if b.coarse() { 16.08 } else { 11.08 }),
        4.0,
    );
    for (slot, y) in [-16.0, 16.0].into_iter().enumerate() {
        gun(b, slot, v3(10.0, y, 12.0), v3(20.0, y, 12.0), 1.25);
    }
}
