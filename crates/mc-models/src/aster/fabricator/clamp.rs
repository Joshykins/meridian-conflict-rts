//! Design A (`fabricator~a`), the clamp: a stepped foundation with heat sinks on its
//! corners, a squat body, and on it a glazed column where the matter forms, gripped by
//! four armoured clamp plates leaning in off the diagonals.
//! - Tech 2: the column to its cap, the four clamps.
//! - Tech 3: a second, narrower column stage on the cap under a higher cap, upper clamps
//!   carried on from the lower ones' knees, capacitor bastions on the grid sides (±x).

use std::f32::consts::FRAC_PI_4;

use glam::Vec3;

use super::super::parts::*;
use super::*;

/// The step and the body: half widths, corner cuts and tops.
const STEP: (f32, f32, f32) = (8.6, 4.6, 2.2);
const BODY: (f32, f32, f32) = (6.2, 1.8, 6.6);
/// How far the body's head is drawn in.
const HEAD: f32 = 0.8;
const CORNICE: f32 = 7.2;
/// The column's stages: foot, head, radius.
const COLUMN: [(f32, f32, f32); 2] = [(CORNICE, 13.2, 2.6), (14.4, 19.6, 2.1)];
/// A clamp plate's path, (out, up), on its diagonal; the tech 3 one's from its knee.
const CLAMP: [(f32, f32); 3] = [(8.6, 2.2), (7.3, 8.2), (3.7, 12.4)];
const UPPER: [(f32, f32); 3] = [(7.3, 8.2), (5.4, 14.6), (2.9, 18.2)];

pub(in crate::aster) fn build(b: &mut MeshBuilder, tech: u8) {
    plinth(b);
    body(b);
    corner_sinks(b, 9.2, 13.6, 2.4);
    let (z0, z1, r) = COLUMN[0];
    chamber(b, z0, z1, r);
    cap(b, z1, r, 16.0);
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| clamp(b, &CLAMP, v2(2.6, 1.0)))
    });

    tech_3(b, tech, 0.2, |b| {
        b.radial(2, |b| bastion(b, v3(9.9, 0.0, DECK), 6.0, 1.8))
    });
    tech_3(b, tech, 0.45, |b| {
        let (z0, z1, r) = COLUMN[1];
        chamber(b, z0, z1, r);
        cap(b, z1, r, 22.0);
    });
    tech_3(b, tech, 0.75, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| clamp(b, &UPPER, v2(2.0, 0.8)))
        });
    });
}

/// The step on the plinth and the body tapering up off it to a dark cornice, hot vents
/// on the step's top along the axes.
fn body(b: &mut MeshBuilder) {
    let (h, c, top) = STEP;
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(0.0, 0.0, DECK),
            v2(h * 2.0, h * 2.0),
            v2(BODY.0 * 1.6, BODY.0 * 1.6),
            CORNICE - DECK,
            Vec2::ZERO,
        );
        return;
    }
    let plan = chamfered_rect(v2(h, h), c);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(DECK, 1.0), Section::new(DECK + 0.4, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(DECK + 0.4, 0.99), Section::new(top, 0.96)],
    );
    let (h, c, head) = BODY;
    let plan = chamfered_rect(v2(h, h), c);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(top, 1.0),
            Section::new(top + 1.2, 0.98),
            Section::new(head, HEAD),
        ],
    );
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[
            Section::new(head, HEAD * 1.04),
            Section::new(CORNICE, HEAD * 0.98),
        ],
    );
    if b.fine() {
        b.radial(4, |b| {
            vent(b, v3(7.5, 0.0, top), v2(1.2, 4.2), 5, GLOW_ORANGE)
        });
    }
}

/// A cap on a column stage whose head is at `z` (radius `r`): a dark octagonal lid and
/// the injector on it up to `top`.
fn cap(b: &mut MeshBuilder, z: f32, r: f32, top: f32) {
    if b.coarse() {
        return;
    }
    let lid = z + 1.2;
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, z), b.sides(8), r * 1.2, r * 0.92, lid - z);
    if b.coarse() {
        return;
    }
    b.paint(METAL);
    b.prism(
        v3(0.0, 0.0, lid),
        b.sides(8),
        r * 0.36,
        r * 0.2,
        top - lid - 0.3,
    );
    b.paint(GLOW_ORANGE);
    b.prism(v3(0.0, 0.0, top - 0.3), b.sides(8), r * 0.2, r * 0.12, 0.3);
}

/// A clamp plate on +x along `path`, (out, up): a thick armoured plate leaning in to
/// the column, a dark knee band, a pad where it bears on the column.
fn clamp(b: &mut MeshBuilder, path: &[(f32, f32); 3], size: Vec2) {
    if b.coarse() {
        return;
    }
    let pts: Vec<Vec3> = path.iter().map(|&(x, z)| v3(x, 0.0, z)).collect();
    b.paint(PLATING);
    sweep(b, &pts, size);
    if !b.fine() {
        return;
    }
    b.paint(ACCENT);
    let knee = pts[1];
    b.beam(
        knee - Vec3::Z * size.y,
        knee + Vec3::Z * size.y,
        size * v2(1.15, 1.6),
        size * v2(1.15, 1.6),
    );
    let tip = pts[2];
    b.beam(
        tip + Vec3::X * 0.2,
        tip - Vec3::X * 0.7,
        v2(size.x * 0.8, size.y * 1.4),
        v2(size.x * 0.7, size.y * 1.2),
    );
    // A hot seam down the plate's outer face.
    let along = (knee - pts[0]).normalize();
    let out = Vec3::Y.cross(along) * (size.y * 0.5 + 0.03);
    b.paint(GLOW_ORANGE);
    b.beam(
        pts[0] + out + along * 1.2,
        knee + out - along * 1.4,
        v2(size.x * 0.22, 0.06),
        v2(size.x * 0.22, 0.06),
    );
}
