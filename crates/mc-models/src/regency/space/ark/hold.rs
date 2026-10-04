//! The hold and its ramp: the deck forward of the hinge, the front bulkhead, the roof's
//! underside on its ribs and the walls lit along their length; the ramp, ribbed and lit
//! at its edges, hung on a graphite hinge drum.
use glam::{Vec2, Vec3};

use super::super::super::kit::{metal, seam, v3};
use super::super::super::machine::red_slot;
use super::{drum, FLOOR, FRONT, HINGE, LANE, LIP, ROOF};
use crate::builder::MeshBuilder;
use crate::material::GLOW_LASER;
use crate::part;

pub(super) fn build(b: &mut MeshBuilder) {
    seam(b);
    b.block(v3(HINGE, -LANE, FLOOR - 8.0), v3(FRONT, LANE, FLOOR));
    b.block(
        v3(FRONT - 2.0, -LANE, FLOOR),
        v3(FRONT + 2.0, LANE, ROOF + 1.0),
    );
    if b.mid() {
        b.block(v3(LIP, -LANE, ROOF), v3(FRONT, LANE, ROOF + 1.5));
        drum(
            b,
            v3(HINGE, 0.0, FLOOR - 1.6),
            Vec3::Y,
            1.8,
            LANE * 2.0 - 1.0,
        );
    }
    if b.fine() {
        inside(b);
    }
    ramp(b);
}

/// The hold's ribs, wall lights and bulkhead: it reads as lit inside with the ramp down.
fn inside(b: &mut MeshBuilder) {
    let ribs = 9;
    for k in 0..ribs {
        let x = LIP + 4.0 + k as f32 * (FRONT - LIP - 8.0) / (ribs - 1) as f32;
        metal(b);
        b.block(v3(x - 0.9, -LANE, ROOF - 2.2), v3(x + 0.9, LANE, ROOF));
        b.mirror_y(|b| {
            b.block(
                v3(x - 0.9, LANE - 0.8, FLOOR - 2.0),
                v3(x + 0.9, LANE, ROOF),
            );
            // A knee where the rib meets the roof.
            b.beam(
                v3(x, LANE - 0.4, ROOF - 9.0),
                v3(x, LANE - 6.0, ROOF - 1.0),
                Vec2::new(1.2, 1.0),
                Vec2::new(1.2, 1.0),
            );
        });
    }
    b.mirror_y(|b| {
        for z in [FLOOR + 3.0, ROOF - 3.5] {
            red_slot(
                b,
                v3((LIP + FRONT) * 0.5, LANE - 0.8, z),
                -Vec3::Y,
                Vec3::X,
                FRONT - LIP - 10.0,
                0.5,
            );
        }
        // The belly mouth's lit edges, seen from under the ship.
        red_slot(
            b,
            v3((LIP + HINGE) * 0.5, LANE + 1.2, FLOOR - 7.5),
            -Vec3::Z,
            Vec3::X,
            HINGE - LIP - 4.0,
            0.6,
        );
    });
    // The bulkhead: a plated door frame lit round its edge, the machinery over it.
    seam(b);
    b.block(
        v3(FRONT - 2.6, -LANE * 0.6, FLOOR),
        v3(FRONT - 2.0, LANE * 0.6, ROOF - 6.0),
    );
    for (a, c) in [
        (
            v3(FRONT - 2.7, -LANE * 0.62, FLOOR + 0.5),
            v3(FRONT - 2.7, -LANE * 0.62, ROOF - 6.0),
        ),
        (
            v3(FRONT - 2.7, LANE * 0.62, FLOOR + 0.5),
            v3(FRONT - 2.7, LANE * 0.62, ROOF - 6.0),
        ),
        (
            v3(FRONT - 2.7, -LANE * 0.62, ROOF - 6.0),
            v3(FRONT - 2.7, LANE * 0.62, ROOF - 6.0),
        ),
    ] {
        red_slot(b, a.lerp(c, 0.5), -Vec3::X, c - a, (c - a).length(), 0.6);
    }
    metal(b);
    for y in [-14.0, -5.0, 5.0, 14.0] {
        b.cylinder_between(
            v3(FRONT - 2.0, y, ROOF - 5.0),
            v3(FRONT - 4.5, y, ROOF - 1.0),
            1.2,
            1.0,
            6,
        );
    }
}

/// The ramp (`part::RAMP`): authored lying open, its lip on the ground.
fn ramp(b: &mut MeshBuilder) {
    b.with_part(part::RAMP, |b| {
        metal(b);
        b.extrude_y(
            &[
                [LIP, 0.0],
                [HINGE, FLOOR],
                [HINGE + 0.9, FLOOR - 1.8],
                [LIP + 4.0, 0.0],
            ],
            -LANE + 0.4,
            LANE - 0.4,
        );
        if !b.mid() {
            return;
        }
        let up = |x: f32| (x - LIP) / (HINGE - LIP) * FLOOR;
        // Its side plates, standing proud of the deck.
        super::super::super::kit::dark_plate(b);
        b.mirror_y(|b| {
            b.extrude_y(
                &[
                    [LIP + 1.0, 0.4],
                    [HINGE - 0.5, FLOOR - 0.2],
                    [HINGE - 2.5, FLOOR + 1.6],
                    [LIP + 5.0, 2.4],
                ],
                LANE - 2.2,
                LANE - 0.4,
            )
        });
        if !b.fine() {
            return;
        }
        // Cross treads up its length, and its edges lit.
        seam(b);
        for k in 1..12 {
            let x = LIP + k as f32 * (HINGE - LIP) / 12.0;
            b.block(
                v3(x - 0.6, -LANE + 2.4, up(x) - 0.2),
                v3(x + 0.6, LANE - 2.4, up(x) + 0.35),
            );
        }
        b.mirror_y(|b| {
            b.paint(GLOW_LASER);
            b.extrude_y(
                &[
                    [LIP + 2.0, 2.45],
                    [HINGE - 2.4, FLOOR + 1.65],
                    [HINGE - 2.4, FLOOR + 1.95],
                    [LIP + 2.0, 2.75],
                ],
                LANE - 1.6,
                LANE - 1.0,
            );
        });
    });
}
