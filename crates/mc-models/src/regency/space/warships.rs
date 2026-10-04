//! Two-gun escort and battery cruiser, each with its own plan. The destroyer has a
//! module of its own (`destroyer/`).
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

pub(super) fn cruiser(b: &mut MeshBuilder, _tech: u8) {
    dark_plate(b);
    b.with_facets(|b| {
        b.extrude_z(
            &[
                [-74.0, -9.0],
                [-74.0, 9.0],
                [46.0, 16.0],
                [86.0, 0.0],
                [46.0, -16.0],
            ],
            5.0,
            if b.coarse() { 30.0 } else { 23.0 },
        )
    });
    if b.coarse() {
        coarse_lobes(
            b,
            &[[-48.0, 16.0], [-22.0, 77.0], [74.0, 59.0], [34.0, 40.0]],
            3.0,
            16.0,
        );
    } else {
        b.mirror_y(|b| {
            lobe(
                b,
                &[
                    (v3(-48.0, 18.0, 13.0), 8.0, 8.0),
                    (v3(-30.0, 52.0, 11.0), 15.0, 7.0),
                    (v3(0.0, 66.0, 9.0), 16.0, 6.0),
                    (v3(37.0, 67.0, 9.0), 14.0, 6.0),
                    (v3(74.0, 56.0, 6.0), 2.0, 3.0),
                ],
            );
            for x in [-32.0, 1.0, 32.0] {
                drive(b, v3(x, 65.0, 1.0), 4.4);
            }
            crest(b, -23.0, 10.0, 23.0, 30.0, 28.0);
        });
    }
    mark(
        b,
        v3(22.0, 0.0, if b.coarse() { 30.08 } else { 23.08 }),
        8.0,
    );
    for (slot, y) in [-12.0, 12.0].into_iter().enumerate() {
        gun(b, slot, v3(46.0, y, 27.0), v3(58.0, y, 27.0), 2.5);
    }
    for (i, x) in [-28.0, 0.0, 28.0].into_iter().enumerate() {
        for (j, y) in [-55.0, 55.0].into_iter().enumerate() {
            gun(b, 2 + i * 2 + j, v3(x, y, 16.0), v3(x + 8.0, y, 16.0), 1.8);
        }
    }
}
