//! The Vane's hull: an armoured arrowhead on lift, the battery's plinth over its middle.

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::{armour, collar, red_slot, swept, Course, Frame};
use super::RACE;

/// `half` an outline (x, y >= 0, from the front round to the back) and its mirror,
/// counter-clockwise from above.
fn plan(half: &[[f32; 2]]) -> Vec<[f32; 2]> {
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

/// A body lofted from a seam-dark belly at `belly` up to `deck`, its flanks drawn in
/// toward the top, faceted.
fn body(b: &mut MeshBuilder, half: &[[f32; 2]], belly: f32, deck: f32) {
    let outline = plan(half);
    b.with_facets(|b| {
        seam(b);
        b.loft_z(
            &outline,
            &[Section::new(belly, 0.82), Section::new(belly + 0.3, 0.95)],
        );
        dark_plate(b);
        b.loft_z(
            &outline,
            &[
                Section::new(belly + 0.3, 1.0),
                Section::new(deck - 0.45, 1.0),
                Section::new(deck, 0.82).shifted(-0.2, 0.0),
            ],
        );
    });
}

/// The sensor head at the nose (`nose` its point, at `z`): a brow plate over two pairs of
/// red optics.
fn head(b: &mut MeshBuilder, nose: f32, z: f32) {
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(nose + 0.2, 0.0, z), v3(-1.0, 0.0, 0.2), Vec3::Z),
        &[
            [0.0, -0.1],
            [0.0, 0.1],
            [0.9, 0.8],
            [1.7, 0.7],
            [1.7, -0.7],
            [0.9, -0.8],
        ],
        0.16,
    );
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(nose - 0.3, 0.4, z - 0.3),
            v3(0.8, 0.6, 0.0),
            Vec3::Y,
            0.34,
            0.12,
        );
        if b.fine() {
            red_slot(
                b,
                v3(nose - 0.75, 0.72, z - 0.25),
                v3(0.6, 0.8, 0.0),
                Vec3::Y,
                0.22,
                0.09,
            );
        }
    });
}

/// The plinth the battery turns on: a faceted block from the deck up to the race, a swept
/// plate laid back down each side of it, the team colour on its back.
fn plinth(b: &mut MeshBuilder, deck: f32) {
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan(&[
                [1.5, 0.0],
                [1.1, 1.1],
                [-1.4, 1.2],
                [-2.3, 0.6],
                [-2.4, 0.0],
            ]),
            &[
                Section::new(deck - 0.1, 1.15),
                Section::new(RACE - 0.4, 1.0),
                Section::new(RACE, 0.9),
            ],
        )
    });
    b.mirror_y(|b| {
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(1.2, 1.15, RACE - 0.15),
                v3(-1.0, 0.25, -0.35),
                v3(0.0, 1.0, 0.4),
            ),
            &swept(3.0, 0.4, 1.0, 0.55),
            0.15,
        );
    });
    b.paint(TEAM);
    b.face(&[
        v3(-1.5, 0.0, RACE + 0.01),
        v3(-2.15, 0.45, RACE + 0.01),
        v3(-2.15, -0.45, RACE + 0.01),
    ]);
}

/// The unit's hull: an arrowhead of a body with a swept sponson either side, each lapped
/// in three plates drawn back into points and lit red under their trailing edges, a lift
/// bell under each end of each sponson and one under the body, the sensor head at the
/// nose and the plinth over its middle.
pub(super) fn hull(b: &mut MeshBuilder) {
    let deck = 2.2;
    body(
        b,
        &[
            [4.6, 0.0],
            [3.4, 0.95],
            [1.0, 1.5],
            [-2.6, 1.45],
            [-3.4, 0.8],
            [-3.4, 0.0],
        ],
        0.7,
        deck,
    );
    b.mirror_y(|b| {
        seam(b);
        b.with_facets(|b| {
            b.extrude_z(
                &[
                    [2.6, 1.0],
                    [1.6, 2.3],
                    [-2.0, 3.2],
                    [-4.0, 3.5],
                    [-3.3, 2.5],
                    [-3.1, 1.0],
                ],
                0.75,
                1.5,
            )
        });
        dark_plate(b);
        let f = Frame::new(v3(2.1, 1.7, 1.55), v3(-1.0, 0.38, 0.0), v3(0.0, 0.3, 1.0));
        let plates = Course {
            count: if b.fine() { 3 } else { 2 },
            step: if b.fine() { 1.6 } else { 3.2 },
            len: 2.4,
            half: 0.75,
            tip: -1.0,
            thick: 0.18,
            tail: 0.8,
        }
        .lay(b, &f);
        if b.fine() {
            for (g, len) in plates.iter().take(2) {
                red_slot(b, g.at(*len - 0.2, 0.0, -0.02), g.n, g.v, 0.5, 0.03);
            }
            metal(b);
            let sides = b.sides(8);
            b.cylinder_between(v3(1.8, 1.55, 1.6), v3(-2.8, 1.55, 1.6), 0.13, 0.13, sides);
            for x in [0.6f32, -1.4] {
                collar(b, v3(x, 1.55, 1.6), Vec3::X, 0.2, 0.16);
            }
        }
        for (x, y) in [(1.3f32, 2.05f32), (-2.6, 2.75)] {
            bell(b, v3(x, y, 0.25), 0.55, 0.5);
        }
    });
    bell(b, v3(-0.2, 0.0, 0.25), 0.8, 0.45);
    head(b, 4.6, deck - 0.3);
    plinth(b, deck);
}
