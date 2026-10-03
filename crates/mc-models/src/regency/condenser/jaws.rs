//! Design B (`regency_fabricator~b`), the jaws: two great curved plates rising off the
//! flanks (±y) and leaning in over a column of matter condensing out of a graphite
//! basin, red slots on their inner faces where the field is pressed; field drivers on the
//! diagonals cabled into the basin.
//! - Tech 2: the basin, the column, the two jaws, the drivers.
//! - Tech 3: a second, taller pair of jaws on the grid sides (±x), the column carried up
//!   between them.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, shell, v3};
use super::super::machine::*;
use super::*;

/// A jaw's path on +y, (out, up, half width, arch).
type Jaw = [(f32, f32, f32, f32); 4];
const JAW: Jaw = [
    (10.4, 0.3, 5.0, 0.9),
    (9.6, 5.5, 4.6, 0.9),
    (7.0, 11.0, 3.6, 0.8),
    (3.6, 14.9, 2.4, 0.6),
];
const HIGH_JAW: Jaw = [
    (10.4, 0.3, 4.4, 0.9),
    (9.8, 7.0, 4.0, 0.85),
    (7.0, 15.0, 3.0, 0.75),
    (3.0, 20.9, 1.9, 0.6),
];
/// The basin's top, and the column of matter on it at each tier: (top, radius).
const BASIN: f32 = 2.8;
const COLUMN: [(f32, f32); 2] = [(12.8, 1.3), (19.4, 1.05)];

pub(in crate::regency) fn build(b: &mut MeshBuilder, tech: u8) {
    footing(b, 8.4, 1.2);
    if !b.coarse() {
        basin(b);
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| b.radial(4, driver));
    }
    column(b, BASIN, COLUMN[0]);
    b.mirror_y(|b| jaw(b, &JAW));

    tech_3(b, tech, 0.3, |b| {
        b.yawed(Vec3::ZERO, -FRAC_PI_2, |b| {
            b.mirror_y(|b| jaw(b, &HIGH_JAW))
        })
    });
    tech_3(b, tech, 0.65, |b| column(b, COLUMN[0].0 + 0.6, COLUMN[1]));
}

/// The graphite basin the column stands in, a seam rim round it.
fn basin(b: &mut MeshBuilder) {
    metal(b);
    b.prism(v3(0.0, 0.0, 1.2), b.sides(8), 2.8, 3.8, BASIN - 1.2);
    seam(b);
    hoop(b, Vec3::Z * BASIN, 3.5, 0.7, 0.3, 8);
}

/// The column of matter from `z` up to `top`, `r` across: hot product between graphite
/// bands, a graphite cap over it.
fn column(b: &mut MeshBuilder, z: f32, (top, r): (f32, f32)) {
    let sides = if b.coarse() { 4 } else { 8 };
    b.paint(GLOW_ORANGE);
    b.prism(v3(0.0, 0.0, z), sides, r, r * 0.9, top - z);
    if b.coarse() {
        return;
    }
    metal(b);
    b.prism(v3(0.0, 0.0, top), b.sides(8), r * 1.25, r * 0.9, 0.6);
    if b.fine() {
        let n = ((top - z) / 3.0) as usize;
        for k in 1..=n {
            let at = z + (top - z) * k as f32 / (n + 1) as f32;
            hoop(b, Vec3::Z * at, r * 1.02, 0.3, 0.35, 8);
        }
    }
}

/// A jaw on +y along `path`: an arched plate leaning in over the column, plates lapped
/// up its back, red slots on its inner face high up.
fn jaw(b: &mut MeshBuilder, path: &Jaw) {
    let pts: Vec<(Vec3, f32, f32)> = path
        .iter()
        .map(|&(y, z, w, h)| (v3(0.0, y, z), w, h))
        .collect();
    if b.coarse() {
        // A bent plate: a triangle of section swept foot to tip.
        dark_plate(b);
        let ring = |p: (Vec3, f32, f32)| {
            vec![
                p.0 + v3(-p.1, 0.0, 0.0),
                p.0 + v3(0.0, p.2, 0.0),
                p.0 + v3(p.1, 0.0, 0.0),
            ]
        };
        b.loft(&[ring(pts[0]), ring(pts[3])], false, false);
        return;
    }
    // A seam footing from the machine's footing out under the jaw's foot.
    let (y0, w0) = (path[0].0, path[0].2);
    seam(b);
    b.cuboid_open(
        v3(0.0, (7.6 + y0 + 0.6) * 0.5, 0.6),
        v3(w0 * 1.7, y0 + 0.6 - 7.6, 1.2),
    );
    dark_plate(b);
    shell(b, &pts, Vec3::Y);
    if !b.fine() {
        return;
    }
    // Plates lapped up its outer face, low down.
    let (p0, p1) = (pts[0].0, pts[1].0);
    let up = p1 - p0;
    let out = v3(0.0, up.z, -up.y).normalize();
    let f = Frame::new(p0 + out * (pts[0].2 + 0.05) + up * 0.12, up, out);
    dark_plate(b);
    Course {
        count: 2,
        step: up.length() * 0.42,
        len: up.length() * 0.55,
        half: pts[0].1 * 0.5,
        tip: 0.0,
        thick: THICK,
        tail: 0.8,
    }
    .lay(b, &f);
    // Red slots across the inner face, where it bears on the column.
    let (p2, p3) = (pts[2].0, pts[3].0);
    let along = p3 - p2;
    let inward = -v3(0.0, along.z, -along.y).normalize();
    for t in [0.35f32, 0.65] {
        let at = p2 + along * t + inward * (pts[2].2 * 0.62 + 0.02);
        red_slot(b, at, inward, Vec3::X, pts[2].1 * 1.1, 0.24);
    }
}

/// A field driver on +x (turned onto a diagonal): a graphite drum lying out along it on
/// a seam saddle, cabled into the basin, a red slot along its top.
fn driver(b: &mut MeshBuilder) {
    let (a, c) = (v3(5.6, 0.0, 2.2), v3(8.6, 0.0, 2.2));
    seam(b);
    b.cuboid_open(v3(7.1, 0.0, 1.6), v3(2.4, 2.0, 0.8));
    collar(b, (a + c) * 0.5, Vec3::X, 1.0, 3.2);
    shaft(b, a, v3(3.4, 0.0, 2.2), 0.35);
    if b.fine() {
        red_slot(b, v3(7.1, 0.0, 3.21), Vec3::Z, Vec3::X, 2.2, 0.26);
    }
}
