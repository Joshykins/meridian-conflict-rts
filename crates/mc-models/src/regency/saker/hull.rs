//! The Saker's hull, slung low between the legs and built up in tiers: a seam-dark
//! belly band, a broad lower tier, an upper tier drawn in under the turret's race. A ram
//! prow juts ahead, lapped glacis plates over it and optic pods by its nose; a stern block
//! stands behind, swept plates lapped back off it into spikes and a bay in its face where
//! the plates part over the machinery. A sponson rides each flank between the legs, plated
//! over its top, a machinery bay let into its side.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::gpu_consts::surface_mark as mark;
use crate::pattern;

use super::super::kit::{dark_plate, seam, v3};
use super::super::machine::{Course, Frame};
use super::super::parts::{bay, inlay, plan_loft, ridged};
use super::DECK;

/// The hull's shape.
pub(super) struct Hull {
    /// Half each tier's outline in plan (y >= 0), nose round to tail.
    lower: &'static [[f32; 2]],
    upper: &'static [[f32; 2]],
    /// Its belly and the lower tier's top, planted.
    pub(super) belly: f32,
    waist: f32,
    /// The prow: where it leaves the hull, its nose, the nose's height, and half its
    /// width where it leaves.
    prow: [f32; 4],
    /// The stern block: from its front back to its face, half its width, its top.
    stern: [f32; 4],
    /// Each sponson: from its front back to its tail, out to, and from its foot up to its
    /// top.
    sponson: [f32; 5],
}

/// An arrowhead: the long prow narrow, the sponsons long.
pub(super) const HULL: Hull = Hull {
    lower: &[
        [12.0, 0.0],
        [9.5, 5.0],
        [6.0, 6.8],
        [-9.0, 6.8],
        [-12.0, 4.8],
        [-12.5, 0.0],
    ],
    upper: &[
        [8.5, 0.0],
        [6.5, 4.4],
        [3.0, 5.8],
        [-8.0, 5.8],
        [-10.5, 3.6],
        [-11.0, 0.0],
    ],
    belly: 2.2,
    waist: 4.8,
    prow: [8.5, 15.4, 2.8, 3.8],
    stern: [-9.5, -13.6, 3.4, 7.2],
    sponson: [5.0, -6.0, 8.8, 2.8, 5.8],
};

fn widest(half: &[[f32; 2]]) -> f32 {
    half.iter().map(|p| p[1]).fold(0.0, f32::max)
}

pub(super) fn hull(b: &mut MeshBuilder, h: &Hull) {
    if b.coarse() {
        dark_plate(b);
        plan_loft(
            b,
            &[h.lower[2], h.lower[3]],
            &[(h.belly, 1.0), (DECK, 0.9)],
            false,
        );
        return;
    }
    seam(b);
    plan_loft(b, h.lower, &[(h.belly, 0.84), (h.belly + 1.0, 0.9)], true);
    dark_plate(b);
    plan_loft(
        b,
        h.lower,
        &[
            (h.belly + 0.95, 0.95),
            (h.belly + 1.6, 1.0),
            (h.waist - 0.35, 1.0),
            (h.waist, 0.95),
        ],
        false,
    );
    seam(b);
    plan_loft(
        b,
        h.upper,
        &[(h.waist - 0.1, 1.02), (h.waist + 0.35, 1.02)],
        false,
    );
    dark_plate(b);
    plan_loft(
        b,
        h.upper,
        &[(h.waist + 0.3, 1.0), (DECK - 0.5, 1.0), (DECK, 0.93)],
        false,
    );
    prow(b, h);
    stern(b, h);
    b.mirror_y(|b| sponson(b, h));
    deck(b, h);
    marks(b, h);
}

/// A section across the hull at `x`, `half` wide, from `z0` up to `z1`: flat belly,
/// chamfered shoulders.
fn section(x: f32, half: f32, z0: f32, z1: f32) -> Vec<Vec3> {
    let h = z1 - z0;
    [
        [-1.0, 0.0],
        [1.0, 0.0],
        [1.08, 0.55],
        [0.72, 1.0],
        [-0.72, 1.0],
        [-1.08, 0.55],
    ]
    .iter()
    .map(|&[t, u]| v3(x, t * half, z0 + u * h))
    .collect()
}

/// The ram prow: a wedge from the hull out to its nose, glacis plates lapped back over
/// it, and a pod of optics either side of its nose.
fn prow(b: &mut MeshBuilder, h: &Hull) {
    let [x0, nose, nz, half] = h.prow;
    let mid = (x0 + nose) * 0.5;
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                section(x0, half, h.belly + 0.9, DECK - 0.3),
                section(
                    mid,
                    half * 0.66,
                    (h.belly + nz) * 0.5,
                    (DECK + nz) * 0.5 + 0.2,
                ),
                section(nose, half * 0.26, nz - 0.7, nz + 0.6),
            ],
            true,
            true,
        )
    });
    // The glacis plates over its back, lapped toward the hull.
    let top0 = v3(x0 - 0.4, 0.0, DECK - 0.3 + 0.02);
    let top1 = v3(nose - 0.4, 0.0, nz + 0.62);
    let along = (top1 - top0).normalize();
    let up = Vec3::Y.cross(along).normalize();
    let up = if up.z < 0.0 { -up } else { up };
    let len = top0.distance(top1);
    let plates: &[(f32, f32)] = if b.fine() {
        &[(0.0, 0.62), (0.42, 0.6)]
    } else {
        &[(0.0, 1.0)]
    };
    for &(from, share) in plates {
        let l = len * share;
        let w = half * 0.62 * (1.0 - from * 0.5);
        dark_plate(b);
        ridged(
            b,
            &Frame::new(top0 + along * (len * from), along, up),
            &[
                [0.0, -w],
                [l * 0.6, -w * 0.82],
                [l, -w * 0.2],
                [l, w * 0.2],
                [l * 0.6, w * 0.82],
                [0.0, w],
            ],
            0.28,
            [l * 0.4, 0.0],
            0.32,
        );
    }
    if b.fine() {
        b.mirror_y(|b| {
            // A pod of optics on the prow's shoulder, red let into its face.
            let at = v3(nose - 3.4, half * 0.36, nz + 0.75);
            dark_plate(b);
            b.chamfered_box(at, v3(1.4, 0.8, 0.7), 0.25);
            inlay(
                b,
                &[
                    at + v3(0.7, -0.28, -0.18),
                    at + v3(0.7, 0.28, -0.18),
                    at + v3(0.7, 0.28, 0.18),
                    at + v3(0.7, -0.28, 0.18),
                ],
                Vec3::X,
                pattern::EMBER_SLOT,
            );
        });
    }
}

/// The stern block, swept plates lapped back off it into spikes, a bay in its face.
fn stern(b: &mut MeshBuilder, h: &Hull) {
    let [x0, x1, half, top] = h.stern;
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                section(x0, half, h.waist - 1.4, top),
                section(x1 + 1.0, half, h.belly + 1.0, top - 0.3),
                section(x1, half * 0.86, h.belly + 1.4, top - 0.9),
            ],
            true,
            true,
        )
    });
    if !b.mid() {
        return;
    }
    b.mirror_y(|b| {
        dark_plate(b);
        let course = Course {
            count: if b.fine() { 2 } else { 1 },
            step: 1.6,
            len: (x0 - x1) * 0.75,
            half: half * 0.42,
            tip: 0.5,
            thick: 0.26,
            tail: 1.6,
        };
        course.lay(
            b,
            &Frame::new(
                v3(x0 + 0.4, half * 0.5, top + 0.02),
                v3(-1.0, 0.0, -0.12),
                Vec3::Z,
            ),
        );
    });
    let face_z = (h.belly + 1.4 + top - 0.9) * 0.5;
    bay(
        b,
        &Frame::new(v3(x1 - 0.03, -half * 0.55, face_z), Vec3::Y, -Vec3::X),
        half * 1.1,
        0.55,
    );
}

/// A sponson along the left flank between the legs: a plated wedge, plates lapped back
/// over its top, a machinery bay let into its side.
fn sponson(b: &mut MeshBuilder, h: &Hull) {
    let [x0, x1, y_out, z0, z1] = h.sponson;
    let y_in = widest(h.lower) - 0.6;
    let ring = |x: f32, k: f32| -> Vec<Vec3> {
        [
            [y_in, z0],
            [y_out - 0.5, z0 + 0.3],
            [y_out, z0 + (z1 - z0) * 0.55],
            [y_out - 0.6, z1],
            [y_in, z1 + 0.2],
        ]
        .iter()
        .map(|&[y, z]| v3(x, y_in + (y - y_in) * k, z0 + (z - z0) * (0.5 + 0.5 * k)))
        .collect()
    };
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                ring(x0, 0.35),
                ring(x0 - 1.6, 1.0),
                ring(x1 + 2.2, 1.0),
                ring(x1, 0.5),
            ],
            true,
            true,
        )
    });
    // Plates lapped back over its top, the last swept out past its tail.
    let len = x0 - x1;
    let w = (y_out - y_in) * 0.5;
    let mid_y = (y_in + y_out) * 0.5;
    let plates: &[(f32, f32)] = if b.fine() {
        &[(0.0, 0.55), (0.42, 0.62)]
    } else {
        &[(0.0, 0.95)]
    };
    for &(from, share) in plates {
        let l = len * share;
        dark_plate(b);
        ridged(
            b,
            &Frame::new(
                v3(x0 - 0.6 - len * from, mid_y, z1 + 0.04),
                -Vec3::X,
                Vec3::Z,
            ),
            &[
                [0.0, -w],
                [0.0, w],
                [l * 0.7, w * 1.05],
                [l, w * 0.5],
                [l * 0.7, -w],
            ],
            0.26,
            [l * 0.45, w * 0.15],
            0.28,
        );
    }
    if b.mid() {
        bay(
            b,
            &Frame::new(
                v3(x0 - 2.4, y_out + 0.02, (z0 + z1) * 0.5 + 0.1),
                -Vec3::X,
                Vec3::Y,
            ),
            len - 4.8,
            0.6,
        );
    }
}

/// Plates on the deck ahead of the turret's race, laid thin so the turret clears them.
fn deck(b: &mut MeshBuilder, h: &Hull) {
    if !b.mid() {
        return;
    }
    let front = h.upper[0][0];
    let half = h.upper[1][1] * 0.7;
    dark_plate(b);
    Course {
        count: if b.fine() { 2 } else { 1 },
        step: 0.9,
        len: 1.6,
        half,
        tip: 0.0,
        thick: 0.18,
        tail: 0.3,
    }
    .lay(
        b,
        &Frame::new(
            v3(front - 0.4, 0.0, DECK - 0.4),
            v3(-1.0, 0.0, 0.25),
            Vec3::Z,
        ),
    );
    if b.fine() {
        // Machined service panels on the deck either side of the race.
        b.mirror_y(|b| {
            let y = widest(h.upper) * 0.86;
            inlay(
                b,
                &[
                    v3(-2.5, y - 1.0, DECK + 0.01),
                    v3(2.5, y - 1.0, DECK + 0.01),
                    v3(2.5, y, DECK + 0.01),
                    v3(-2.5, y, DECK + 0.01),
                ],
                Vec3::Z,
                pattern::EMBER_MACHINED,
            );
        });
    }
}

/// What the hull's surface carries: panel lines on the upper tier's flanks, a lapped edge
/// along the waist, a groove along the lower tier, lamps in the prow and the stern's
/// corners, the owner's colour on the stern.
fn marks(b: &mut MeshBuilder, h: &Hull) {
    let lo = widest(h.lower);
    let up = widest(h.upper);
    let [_, x1, half, top] = h.stern;
    let [x0s, x1s, ..] = h.sponson;
    let [px, _, nz, ph] = h.prow;
    b.mirror_y(|b| {
        b.mark(
            v3(x1s + 1.0, up - 0.5, h.waist + 0.5),
            v3(x0s - 1.0, up + 0.3, DECK - 0.6),
            mark::OUTLINE,
            0.0,
        );
        b.mark(
            v3(x1s, lo - 0.4, h.waist - 0.5),
            v3(x0s, lo + 0.3, h.waist + 0.05),
            mark::LAP,
            0.0,
        );
        b.mark(
            v3(x1s, lo - 0.4, h.belly + 1.2),
            v3(x0s, lo + 0.3, h.belly + 1.6),
            mark::GROOVE,
            0.0,
        );
        b.mark(
            v3(x1 - 0.3, half * 0.6, top - 1.6),
            v3(x1 + 0.6, half * 0.95, top - 1.2),
            mark::LAMP,
            0.0,
        );
        b.mark(
            v3(px + 1.0, ph * 0.5, nz + 0.4),
            v3(px + 3.0, ph * 0.9, nz + 1.6),
            mark::LAMP,
            0.0,
        );
    });
    b.mark(
        v3(x1 + 1.2, -half * 0.3, top - 0.6),
        v3(x1 + 2.6, half * 0.3, top + 0.2),
        mark::TEAM,
        0.0,
    );
}
