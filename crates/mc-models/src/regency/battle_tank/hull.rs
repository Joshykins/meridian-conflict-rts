//! The battle tank's hull: an armoured core between two sponsons, on lift.

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::{armour, collar, red_slot, swept, Course, Frame};
use super::DECK;

/// `half` an outline (x, y >= 0, from the front round to the back) and its mirror,
/// counter-clockwise from above.
pub(super) fn plan(half: &[[f32; 2]]) -> Vec<[f32; 2]> {
    half.iter()
        .copied()
        .chain(
            half.iter()
                .rev()
                .filter(|p| p[1] > 0.0)
                .map(|&[x, y]| [x, -y]),
        )
        .collect()
}

/// The core: a pointed armoured body lofted from a seam-dark belly at `belly` up to the
/// deck, its flanks drawn in toward the top; two swept glacis plates over its nose, a red
/// optic either side of the prow, a raised plated drive housing over its tail with the
/// team colour on it and a lit vent across its back.
fn core(b: &mut MeshBuilder, half: &[[f32; 2]], belly: f32) {
    let outline = plan(half);
    b.with_facets(|b| {
        seam(b);
        b.loft_z(
            &outline,
            &[Section::new(belly, 0.82), Section::new(belly + 0.35, 0.95)],
        );
        dark_plate(b);
        b.loft_z(
            &outline,
            &[
                Section::new(belly + 0.35, 1.0),
                Section::new(DECK - 0.5, 1.0),
                Section::new(DECK, 0.86).shifted(-0.2, 0.0),
            ],
        );
    });
    let nose = half[0][0];
    // The glacis: a plate either side laid back over the nose, its spike drawn outward.
    b.mirror_y(|b| {
        dark_plate(b);
        let f = Frame::new(
            v3(nose - 0.7, 0.1, DECK - 0.35),
            v3(-1.0, 0.42, 0.16),
            v3(0.25, 0.1, 1.0),
        );
        armour(b, &f, &swept(2.8, 0.6, -1.0, 0.4), 0.2);
        red_slot(
            b,
            v3(nose - 0.75, 0.62, DECK - 0.75),
            v3(0.8, 0.6, 0.0),
            Vec3::Y,
            0.5,
            0.1,
        );
        if b.fine() {
            red_slot(
                b,
                v3(nose - 1.3, 0.98, DECK - 0.72),
                v3(0.6, 0.8, 0.0),
                Vec3::Y,
                0.3,
                0.08,
            );
        }
    });
    // The drive housing over the tail: plate over a bronze bus, a vent lit across it.
    let tail = half[half.len() - 1][0];
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan(&[
                [tail + 1.6, 0.8],
                [tail + 1.1, 1.2],
                [tail + 0.2, 1.1],
                [tail + 0.2, 0.0],
            ]),
            &[
                Section::new(DECK - 0.1, 1.0),
                Section::new(DECK + 0.45, 0.85),
            ],
        )
    });
    if b.fine() {
        collar(b, v3(tail + 0.15, 0.0, DECK + 0.05), Vec3::Y, 0.32, 2.0);
        red_slot(
            b,
            v3(tail + 0.6, 0.0, DECK + 0.45),
            Vec3::Z,
            Vec3::Y,
            1.2,
            0.06,
        );
    }
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            v3(tail + 1.45, 0.0, DECK + 0.47),
            v3(tail + 0.7, 0.75, DECK + 0.47),
            v3(tail + 0.4, 0.6, DECK + 0.47),
            v3(tail + 1.15, 0.0, DECK + 0.47),
        ])
    });
}

/// The core's outline at its widest: a pointed prow, square shoulders, a cut tail.
const CORE: [[f32; 2]; 6] = [
    [5.9, 0.0],
    [4.9, 1.15],
    [3.4, 1.85],
    [-4.3, 1.85],
    [-5.1, 1.25],
    [-5.1, 0.0],
];

/// The tank: the core between two armoured sponsons, each a seam-dark deck with a course
/// of four swept plates lapped back along it, its glacis sloped down in front, a skirt
/// plate down its outside lit red along its lower edge, and the bronze lift bus showing
/// in the gap between it and the core; a lift bell under each end of each sponson and
/// one under the core.
pub(super) fn hull(b: &mut MeshBuilder) {
    core(b, &CORE, 0.75);
    b.mirror_y(sponson);
    bell(b, v3(0.4, 0.0, 0.25), 0.9, 0.65);
    b.mirror_y(|b| {
        for x in [3.1f32, -3.4] {
            bell(b, v3(x, 2.65, 0.25), 0.62, 0.55);
        }
    });
}

fn sponson(b: &mut MeshBuilder) {
    seam(b);
    b.with_facets(|b| {
        b.extrude_z(
            &[
                [5.0, 2.0],
                [4.3, 3.2],
                [-4.5, 3.3],
                [-5.5, 2.7],
                [-5.1, 2.0],
            ],
            0.8,
            1.75,
        )
    });
    dark_plate(b);
    let f = Frame::new(v3(4.6, 2.55, 1.76), v3(-1.0, 0.0, 0.0), v3(0.0, 0.2, 1.0));
    let (count, step) = if b.fine() { (4, 2.25) } else { (2, 4.5) };
    let plates = Course {
        count,
        step,
        len: 2.8,
        half: 0.78,
        tip: -1.0,
        thick: 0.2,
        tail: 0.9,
    }
    .lay(b, &f);
    if b.fine() {
        for (g, len) in plates.iter().take(3) {
            red_slot(b, g.at(*len - 0.2, 0.0, -0.03), g.n, g.v, 0.5, 0.03);
        }
    }
    // The glacis, sloped down from the first plate to the sponson's point.
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(4.75, 2.6, 1.78), v3(0.5, 0.0, -0.86), v3(0.86, 0.0, 0.5)),
        &swept(1.15, 0.6, 0.0, 0.7),
        0.18,
    );
    // The skirt down its outside.
    let (front, back) = (v3(4.0, 3.32, 0.0), v3(-4.6, 3.38, 0.0));
    let out = Vec3::Y;
    dark_plate(b);
    armour(
        b,
        &Frame::new(front + Vec3::Z * 1.25, back - front, out),
        &[
            [0.0, -0.45],
            [0.0, 0.45],
            [8.0, 0.45],
            [9.0, 0.0],
            [8.0, -0.45],
        ],
        0.14,
    );
    if b.fine() {
        red_slot(
            b,
            front.lerp(back, 0.35) + v3(0.0, 0.16, 0.82),
            out,
            Vec3::X,
            2.4,
            0.03,
        );
        // The lift bus in the gap between the sponson and the core.
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(v3(3.6, 1.95, 1.85), v3(-4.0, 1.95, 1.85), 0.14, 0.14, sides);
        for x in [2.4f32, 0.0, -2.4] {
            collar(b, v3(x, 1.95, 1.85), Vec3::X, 0.22, 0.18);
        }
    }
}
