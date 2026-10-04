//! Two-gun escort, battery cruiser and excavation destroyer, each with its own plan.
use super::super::kit::{dark_plate, metal, seam, v3};
use super::hull::{coarse_lobes, crest, drive, gun, lobe, mark};
use crate::builder::MeshBuilder;
use crate::material::GLOW_VIOLET;
use crate::{part, rig};
use glam::{Vec2, Vec3};

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
        gun(b, slot, v3(10.0, y, 7.0), v3(20.0, y, 7.0), 1.25);
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
        gun(b, slot, v3(46.0, y, 22.0), v3(58.0, y, 22.0), 2.5);
    }
    for (i, x) in [-28.0, 0.0, 28.0].into_iter().enumerate() {
        for (j, y) in [-55.0, 55.0].into_iter().enumerate() {
            gun(b, 2 + i * 2 + j, v3(x, y, 16.0), v3(x + 8.0, y, 16.0), 1.8);
        }
    }
}

pub(super) fn destroyer(b: &mut MeshBuilder, _tech: u8) {
    dark_plate(b);
    b.block(
        v3(-94.0, -24.0, 8.0),
        v3(-34.0, 24.0, if b.coarse() { 48.0 } else { 36.0 }),
    );
    if b.coarse() {
        coarse_lobes(
            b,
            &[[-73.0, 22.0], [-12.0, 83.0], [114.0, 29.0], [35.0, 34.0]],
            4.0,
            38.0,
        );
    } else {
        b.mirror_y(|b| {
            lobe(
                b,
                &[
                    (v3(-80.0, 28.0, 23.0), 13.0, 13.0),
                    (v3(-36.0, 61.0, 24.0), 19.0, 16.0),
                    (v3(9.0, 64.0, 21.0), 19.0, 16.0),
                    (v3(51.0, 53.0, 17.0), 17.0, 12.0),
                    (v3(87.0, 39.0, 11.0), 11.0, 7.0),
                    (v3(113.0, 28.0, 6.0), 1.0, 2.0),
                ],
            );
            for x in [-62.0, -24.0, 15.0] {
                drive(b, v3(x, 63.0, 2.0), 6.4);
            }
            crest(b, -49.0, 21.0, 35.0, 48.0, 36.0);
            dark_plate(b);
            b.beam(
                v3(-36.0, 35.0, 18.0),
                v3(10.0, 12.0, 16.0),
                Vec2::splat(3.0),
                Vec2::splat(2.0),
            );
        });
    }
    mark(
        b,
        v3(-48.0, 0.0, if b.coarse() { 48.1 } else { 36.1 }),
        12.0,
    );
    projector(b);
}

fn projector(b: &mut MeshBuilder) {
    let pivot = v3(12.0, 0.0, 16.0);
    let muzzle = v3(32.0, 0.0, 9.0);
    b.set_turret_pivot(pivot);
    b.set_arm_pivot(pivot);
    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_GUN, |b| {
            if b.coarse() {
                b.paint(GLOW_VIOLET);
                b.cylinder_between(muzzle - Vec3::X * 3.0, muzzle, 5.0, 5.0, 4);
                return;
            }
            let center = v3(24.0, 0.0, 12.0);
            seam(b);
            b.beam(pivot, center, Vec2::new(5.0, 4.0), Vec2::new(4.0, 4.0));
            metal(b);
            let count = if b.fine() { 16 } else { 8 };
            for i in 0..count {
                let a = std::f32::consts::TAU * i as f32 / count as f32;
                let c = std::f32::consts::TAU * (i + 1) as f32 / count as f32;
                let p = center + v3(0.0, a.cos(), a.sin()) * 10.0;
                let q = center + v3(0.0, c.cos(), c.sin()) * 10.0;
                b.beam(p, q, Vec2::splat(0.8), Vec2::splat(0.8));
            }
            for y in [-8.0, 8.0] {
                dark_plate(b);
                b.beam(
                    center + v3(-4.0, y, 0.0),
                    muzzle + v3(-1.0, y * 0.5, 0.0),
                    Vec2::new(2.4, 2.0),
                    Vec2::new(1.4, 1.4),
                );
            }
            b.paint(GLOW_VIOLET);
            b.spheroid(
                center,
                v3(4.2, 4.8, 4.8),
                b.sides(12),
                if b.fine() { 6 } else { 3 },
            );
            b.cylinder_between(center, muzzle, 3.8, 4.0, b.sides(12));
        })
    });
}
