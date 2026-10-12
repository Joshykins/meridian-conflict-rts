//! The Targe's hull: a broad plated lozenge on lift, a sponson either side lapped in
//! swept plates, the projector's plinth over its middle.

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::{armour, collar, Course, Frame};
use super::super::parts::inlay;
use super::{DECK, PLINTH};

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

/// The body's outline, half of it.
pub(super) const BODY: [[f32; 2]; 6] = [
    [4.4, 0.0],
    [3.0, 1.35],
    [0.4, 2.0],
    [-2.8, 1.85],
    [-3.9, 1.0],
    [-3.9, 0.0],
];

/// The body, lofted from a seam-dark belly up to the deck, its flanks drawn in, faceted.
fn body(b: &mut MeshBuilder) {
    let outline = plan(&BODY);
    let belly = 0.7;
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
                Section::new(DECK - 0.45, 1.0),
                Section::new(DECK, 0.82).shifted(-0.2, 0.0),
            ],
        );
    });
}

/// The sensor head at the nose: a brow plate with two red optics let into it.
fn head(b: &mut MeshBuilder) {
    let z = DECK - 0.3;
    let f = Frame::new(v3(4.6, 0.0, z), v3(-1.0, 0.0, 0.2), Vec3::Z);
    dark_plate(b);
    armour(
        b,
        &f,
        &[
            [0.0, -0.1],
            [0.0, 0.1],
            [0.9, 0.85],
            [1.8, 0.75],
            [1.8, -0.75],
            [0.9, -0.85],
        ],
        0.16,
    );
    if b.fine() {
        // The optics, recessed in the plate's face (the light is the surface's).
        b.mirror_y(|b| {
            let c = f.at(1.15, 0.38, 0.17);
            let (s, t) = (f.u * 0.3, f.v * 0.16);
            inlay(
                b,
                &[c - s - t, c + s - t, c + s + t, c - s + t],
                f.n,
                pattern::EMBER_SLOT,
            );
        });
    }
}

/// A sponson: a seam-dark keel, three swept plates lapped back over it and lit red in
/// the face of the last, a graphite feed run along it, a lift bell under each end.
fn sponson(b: &mut MeshBuilder) {
    seam(b);
    b.with_facets(|b| {
        b.extrude_z(
            &[
                [2.4, 1.5],
                [1.0, 2.9],
                [-2.2, 3.5],
                [-4.3, 3.4],
                [-3.6, 2.2],
                [-3.0, 1.5],
            ],
            0.75,
            1.5,
        )
    });
    dark_plate(b);
    let f = Frame::new(v3(2.0, 2.0, 1.55), v3(-1.0, 0.3, 0.0), v3(0.0, 0.35, 1.0));
    let plates = Course {
        count: if b.fine() { 3 } else { 2 },
        step: if b.fine() { 1.7 } else { 3.4 },
        len: 2.6,
        half: 0.8,
        tip: -1.0,
        thick: 0.18,
        tail: 0.9,
    }
    .lay(b, &f);
    if b.fine() {
        let (g, len) = plates[plates.len() - 1];
        let c = g.at(len * 0.45, 0.0, 0.18);
        let (s, t) = (g.u * 0.6, g.v * 0.18);
        inlay(
            b,
            &[c - s - t, c + s - t, c + s + t, c - s + t],
            g.n,
            pattern::EMBER_SLOT,
        );
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(v3(1.9, 1.7, 1.6), v3(-3.2, 1.7, 1.6), 0.13, 0.13, sides);
        for x in [0.4f32, -1.8] {
            collar(b, v3(x, 1.7, 1.6), Vec3::X, 0.2, 0.16);
        }
    }
    for (x, y) in [(1.2f32, 2.3f32), (-3.0, 2.85)] {
        bell(b, v3(x, y, 0.25), 0.6, 0.5);
    }
}

/// The plinth the projector stands on: a plated octagonal drum stepping in from the deck,
/// a seam round it, the owner's colour on the deck behind it.
fn plinth(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(v3(0.0, 0.0, DECK - 0.1), 8, 2.1, 1.85, PLINTH - DECK - 0.25);
    seam(b);
    b.prism(v3(0.0, 0.0, PLINTH - 0.35), 8, 1.75, 1.75, 0.15);
    dark_plate(b);
    b.prism(v3(0.0, 0.0, PLINTH - 0.2), 8, 1.7, 1.4, 0.2);
    team(b);
}

/// The owner's colour: a chevron on the rear deck.
pub(super) fn team(b: &mut MeshBuilder) {
    b.paint(TEAM);
    b.face(&[
        v3(-2.3, 0.0, DECK + 0.01),
        v3(-3.2, 0.7, DECK + 0.01),
        v3(-3.2, -0.7, DECK + 0.01),
    ]);
}

pub(super) fn hull(b: &mut MeshBuilder) {
    body(b);
    b.mirror_y(sponson);
    bell(b, v3(-0.4, 0.0, 0.25), 0.85, 0.45);
    head(b);
    plinth(b);
}
