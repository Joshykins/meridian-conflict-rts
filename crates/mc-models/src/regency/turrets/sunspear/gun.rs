//! The Sunspear's gun, in its own frame (origin at the trunnion, +x down the bore): a
//! faceted core stepping down from the breech to a block at the mouth, banded, swept
//! plates down its top, bottom and sides; four plated rails crossed round it like an X,
//! each lapped in swept plates with slots down its flanks, all ending flush with the
//! mouth; clamp bands round the rails; and the mantlet the gun turns in, ridged over.
//!
//! The core and rails recoil. Through the charge the rails and their bands flare off
//! the core (`FLARE`), opening the mouth round the knot, and snap back as it fires.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::FLARE;

use super::super::super::commander::form::{ring, sleeve, OCT};
use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{Course, Frame};
use super::super::super::parts::{face_out, ridged};
use super::super::bar_through;
use super::{slot, MOUTH};

/// The core's half width (its octagon's flat) down the bore: a step at the end of the
/// breech's sleeve, a block at the mouth.
const CORE: [[f32; 2]; 9] = [
    [-4.0, 2.4],
    [9.0, 2.2],
    [14.0, 2.2],
    [14.6, 1.95],
    [24.0, 1.6],
    [29.6, 1.45],
    [30.2, 1.7],
    [MOUTH - 0.6, 1.7],
    [MOUTH, 1.45],
];
/// How far off the bore's line each rail stands, breech to mouth.
const RAIL: [[f32; 2]; 5] = [
    [6.5, 3.0],
    [14.0, 2.9],
    [24.0, 2.6],
    [30.0, 2.6],
    [MOUTH, 2.6],
];
/// A rail's section at its root: across, and deep off the bore.
const RAIL_SIZE: Vec2 = Vec2::new(1.2, 1.4);

pub(super) fn gun(b: &mut MeshBuilder) {
    mantlet(b);
    b.with_recoil(|b| {
        core(b);
        b.with_charge_gear(FLARE, |b| {
            for k in 0..4 {
                let a = (45.0 + 90.0 * k as f32).to_radians();
                rail(b, v3(0.0, a.cos(), a.sin()));
            }
            let bands: &[f32] = if b.fine() {
                &[12.0, 19.5, 27.0]
            } else {
                &[19.5]
            };
            for &x in bands {
                let r = lerp(&RAIL, x) + RAIL_SIZE.y * 0.5 * taper(x) + 0.2;
                metal(b);
                sleeve(
                    b,
                    &[
                        ring(v3(x, 0.0, 0.0), Vec3::Z, r, r),
                        ring(v3(x + 0.7, 0.0, 0.0), Vec3::Z, r, r),
                    ],
                    &OCT,
                );
            }
        });
    });
}

/// The value at `x` between the stations either side of it.
fn lerp(stations: &[[f32; 2]], x: f32) -> f32 {
    let i = stations
        .iter()
        .rposition(|s| s[0] <= x)
        .unwrap_or(0)
        .min(stations.len() - 2);
    let (a, c) = (stations[i], stations[i + 1]);
    a[1] + (c[1] - a[1]) * ((x - a[0]) / (c[0] - a[0])).clamp(0.0, 1.0)
}

/// How much a rail has narrowed by `x`: to 0.8 at the mouth.
fn taper(x: f32) -> f32 {
    1.0 - 0.2 * ((x - RAIL[0][0]) / (MOUTH - RAIL[0][0])).clamp(0.0, 1.0)
}

/// The mantlet the gun turns in, in the casemate's face: a faceted collar drawn in at
/// its front, a ridged plate over its top and under it.
fn mantlet(b: &mut MeshBuilder) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(7.5, 0.0, 0.0), Vec3::Z, 3.6, 3.6),
            ring(v3(11.6, 0.0, 0.0), Vec3::Z, 3.6, 3.6),
            ring(v3(12.6, 0.0, 0.0), Vec3::Z, 3.0, 3.0),
        ],
        &OCT,
    );
    if b.fine() {
        for n in [Vec3::Z, -Vec3::Z] {
            ridged(
                b,
                &Frame::new(n * 3.62 + Vec3::X * 7.9, Vec3::X, n),
                &[[0.0, -1.9], [3.4, -1.5], [3.4, 1.5], [0.0, 1.9]],
                0.3,
                [1.2, 0.0],
                0.25,
            );
        }
    }
}

/// The core: the faceted octagon down the bore, its dark mouth, graphite bands, and
/// courses of swept plates along its top, bottom and sides between the rails.
fn core(b: &mut MeshBuilder) {
    dark_plate(b);
    let rings: Vec<_> = CORE
        .iter()
        .map(|&[x, w]| ring(v3(x, 0.0, 0.0), Vec3::Z, w, w))
        .collect();
    sleeve(b, &rings, &OCT);
    // The mouth: a dark face let into the end of the block.
    let w = CORE[CORE.len() - 1][1] * 0.72;
    seam(b);
    let mouth: Vec<Vec3> = OCT
        .iter()
        .map(|&[s, t]| v3(MOUTH + 0.02, -s * w, t * w))
        .collect();
    face_out(b, &mouth, Vec3::X);
    if !b.fine() {
        return;
    }
    for x in [17.0f32, 22.0] {
        let w = lerp(&CORE, x) + 0.14;
        metal(b);
        sleeve(
            b,
            &[
                ring(v3(x, 0.0, 0.0), Vec3::Z, w, w),
                ring(v3(x + 0.5, 0.0, 0.0), Vec3::Z, w, w),
            ],
            &OCT,
        );
    }
    let slope = (lerp(&CORE, 15.0) - lerp(&CORE, 29.0)) / 14.0;
    dark_plate(b);
    for n in [Vec3::Z, -Vec3::Z, Vec3::Y, -Vec3::Y] {
        let x0 = 29.0;
        Course {
            count: 3,
            step: 4.6,
            len: 5.2,
            half: 0.6,
            tip: 0.0,
            thick: 0.2,
            tail: 1.0,
        }
        .lay(
            b,
            &Frame::new(
                n * (lerp(&CORE, x0) + 0.01) + Vec3::X * x0,
                -Vec3::X + n * slope,
                n,
            ),
        );
    }
}

/// One rail standing off the bore along `out` (in the y-z plane): a plated bar narrowing
/// to the mouth and ending flush with it, slots down both flanks, a course of swept
/// plates down its outer face.
fn rail(b: &mut MeshBuilder, out: Vec3) {
    let across = Vec3::X.cross(out);
    let points: Vec<(Vec3, Vec2)> = RAIL
        .iter()
        .map(|&[x, r]| (Vec3::X * x + out * r, RAIL_SIZE * taper(x)))
        .collect();
    dark_plate(b);
    b.with_facets(|b| bar_through(b, &points, across));
    let spans: &[(f32, f32)] = if b.fine() {
        &[(9.0, 15.0), (17.0, 23.0), (25.0, 31.6)]
    } else {
        &[(17.0, 23.0)]
    };
    for &(x0, x1) in spans {
        let x = (x0 + x1) * 0.5;
        let half = RAIL_SIZE.x * 0.5 * taper(x);
        for side in [1.0f32, -1.0] {
            slot(
                b,
                Vec3::X * x + out * lerp(&RAIL, x) + across * (side * half),
                across * side,
                v3(x1 - x0, 0.0, 0.0) + out * (lerp(&RAIL, x1) - lerp(&RAIL, x0)),
                x1 - x0,
                0.34,
            );
        }
    }
    if b.fine() {
        let x0 = 31.0;
        let lift = |x: f32| lerp(&RAIL, x) + RAIL_SIZE.y * 0.5 * taper(x);
        let slope = (lift(10.0) - lift(x0)) / (x0 - 10.0);
        dark_plate(b);
        Course {
            count: 3,
            step: 7.0,
            len: 7.4,
            half: 0.5,
            tip: 0.0,
            thick: 0.2,
            tail: 1.6,
        }
        .lay(
            b,
            &Frame::new(
                Vec3::X * x0 + out * (lift(x0) + 0.01),
                -Vec3::X + out * slope,
                out,
            ),
        );
    }
}
