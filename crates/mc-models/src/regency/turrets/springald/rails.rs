//! The Springald's gun past its breech, in the gun's frame (origin at the trunnion, +x down
//! the bore): three tall plated rails a third of a turn apart round a bottle of pinch
//! coils. The breech runs forward into a plated fairing that goes from round to three
//! lobes, and the rails grow out of its lobes, so gun and body are one piece. Its charge
//! is held at [`LINE`]'s muzzle between three gravity lenses, one on each rail, so the
//! three knots of its triune shot are gathered where the strands leave from.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::EXTEND;

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::{hoop_on, Course, Frame};
use super::super::bar_through;
use super::super::sunspear::{coil_light, lens};
use super::LINE;

/// The three bearings round the bore, the first straight up.
fn bearings() -> [f32; 3] {
    [0.0, 1.0, 2.0].map(|k| TAU * 0.25 + TAU * k / 3.0)
}

/// Out from the bore at bearing `a`, across it.
fn radial(a: f32) -> Vec3 {
    v3(0.0, a.cos(), a.sin())
}

/// Round the bore at bearing `a`, the way the bearing turns.
fn round(a: f32) -> Vec3 {
    v3(0.0, -a.sin(), a.cos())
}

/// `x` down the bore and `r` out from it at bearing `a`.
fn at(x: f32, a: f32, r: f32) -> Vec3 {
    v3(x, 0.0, 0.0) + radial(a) * r
}

/// Rails' distance out from the bore.
const RAIL_R: f32 = 3.6;

pub(super) fn draw(b: &mut MeshBuilder) {
    let len = LINE.len();
    let fine = b.fine();
    fairing(b);
    metal(b);
    b.cylinder_between(
        v3(14.0, 0.0, 0.0),
        v3(48.5, 0.0, 0.0),
        1.35,
        1.1,
        b.sides(10),
    );
    let stack: &[(f32, f32, u32)] = if fine {
        &[
            (30.0, 2.3, 1),
            (33.5, 2.2, 2),
            (37.0, 2.15, 3),
            (40.5, 2.1, 4),
            (44.0, 2.0, 5),
            (47.5, 1.9, 6),
        ]
    } else {
        &[(33.5, 2.2, 2), (40.5, 2.1, 4), (47.5, 1.9, 6)]
    };
    coils(b, stack);
    for a in bearings() {
        dark_plate(b);
        bar_through(
            b,
            &[
                (at(12.0, a, RAIL_R + 0.6), Vec2::new(2.2, 3.0)),
                (at(30.0, a, RAIL_R), Vec2::new(1.5, 2.2)),
                (at(len - 4.5, a, RAIL_R - 0.2), Vec2::new(1.1, 1.6)),
                (at(len - 2.4, a, RAIL_R - 0.6), Vec2::new(0.8, 1.1)),
            ],
            round(a),
        );
        Course {
            count: if fine { 3 } else { 1 },
            step: 7.0,
            len: 9.0,
            half: 0.8,
            tip: 0.0,
            thick: 0.3,
            tail: 2.0,
        }
        .lay(
            b,
            &Frame::new(at(len - 6.0, a, RAIL_R + 1.0), -Vec3::X, radial(a)),
        );
        // The lenses stand behind the charge at rest and run out round it through
        // the charge (`EXTEND`, 2.2 m at this model's gear scale).
        b.with_charge_gear(EXTEND, |b| {
            let x = len - 2.6;
            lens(b, at(x, a, 2.8), v3(x, 0.0, 0.0), 0.7)
        });
    }
    if fine {
        // A band tying the rails across the coils.
        dark_plate(b);
        hoop_on(
            b,
            v3(35.0, 0.0, 0.0),
            Vec3::X,
            RAIL_R,
            0.9,
            1.2,
            b.sides(12),
        );
    }
}

/// The breech's front run on into the rails: a plated fairing lofted from the breech's
/// full width, narrowing into three lobes, one under each rail's root, the hollows between
/// them closing onto the bore; so the gun is thickest at its neck and tapers to the muzzle.
fn fairing(b: &mut MeshBuilder) {
    let n = if b.fine() { 36 } else { 12 };
    // (x, middle radius, how far the lobes stand out and the hollows sink).
    let stations: &[(f32, f32, f32)] = &[
        (8.0, 4.8, 0.1),
        (13.0, 4.4, 0.6),
        (19.0, 3.6, 1.0),
        (25.0, 2.9, 1.3),
        (30.0, 2.4, 1.5),
    ];
    let rings: Vec<Vec<Vec3>> = stations
        .iter()
        .map(|&(x, mid, lobe)| {
            (0..n)
                .map(|i| {
                    let t = TAU * i as f32 / n as f32;
                    // Three lobes, the first straight up (bearing a quarter turn).
                    let r = mid + lobe * (3.0 * (t - TAU * 0.25)).cos();
                    at(x, t, r.max(0.8))
                })
                .collect()
        })
        .collect();
    dark_plate(b);
    b.loft(&rings, true, true);
}

/// Pinch coils down the bore at `(x, r, stage)`, each a plated ring wound in bronze with
/// a fusion light inside, lit from the breech through the charge.
fn coils(b: &mut MeshBuilder, coils: &[(f32, f32, u32)]) {
    let segs = b.sides(16);
    for &(x, r, stage) in coils {
        let c = Vec3::X * x;
        dark_plate(b);
        hoop_on(b, c, Vec3::X, r, 1.0, 1.2, segs);
        metal(b);
        hoop_on(b, c, Vec3::X, r + 0.6, 0.3, 0.8, segs);
        coil_light(b, stage);
        hoop_on(b, c, Vec3::X, r - 0.6, 0.2, 0.7, segs);
    }
}
