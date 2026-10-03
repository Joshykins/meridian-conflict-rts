//! The Material Fabricator's machine, the carousel: a low octagonal base, a carousel
//! of eight cells turning round a finned hub, four guide columns on the hub's head
//! holding a capped crown, and a hammer dropping between them onto the matter in the hub's head
//! (`part::FAB_INDEX`, `part::FAB_PRESS`).
//! - Tech 2: all of that, lamps on the crown.
//! - Tech 3: a finned condenser on the crown under a higher cap.

use std::f32::consts::{FRAC_PI_4, TAU};

use glam::Vec3;

use super::super::parts::*;
use super::*;
use crate::builder::ngon;
use crate::part;

/// The base: radius to its corners, top; the carousel's track on it.
const BASE: (f32, f32) = (10.8, 2.0);
const TRACK: f32 = 2.4;
/// A cell: its middle out from the axis, size (out, across, up).
const CELL: (f32, Vec3) = (7.3, Vec3::new(2.6, 3.0, 3.4));
/// The hub: radius at foot and head, its head; the head band's top.
const HUB: (f32, f32, f32) = (4.6, 3.8, 9.0);
const HEAD: f32 = 9.6;
/// The hammer: foot, top, radius. The crown: foot, top.
const HAMMER: (f32, f32, f32) = (10.8, 12.6, 3.4);
const CROWN: (f32, f32) = (14.0, 15.0);

pub(in crate::aster) fn build(b: &mut MeshBuilder, tech: u8) {
    plinth(b);
    let sides = if b.coarse() { 4 } else { 8 };
    b.paint(PLATING);
    b.prism(
        v3(0.0, 0.0, DECK),
        sides,
        BASE.0,
        BASE.0 * 0.95,
        BASE.1 - DECK,
    );
    if b.coarse() {
        b.prism(
            v3(0.0, 0.0, BASE.1),
            4,
            HUB.0 * 1.3,
            HUB.1 * 1.2,
            CROWN.1 + 0.95 - BASE.1,
        );
        tech_3(b, tech, 0.5, |b| {
            b.prism(v3(0.0, 0.0, CROWN.1), 4, 3.6, 3.0, 20.2 - CROWN.1)
        });
        return;
    }
    b.paint(ACCENT);
    annulus(b, 8, 5.4, 9.2, BASE.1, TRACK);
    b.with_part(part::FAB_INDEX, carousel);
    hub(b);
    b.with_part(part::FAB_PRESS, hammer);
    crown(b);

    tech_3(b, tech, 0.5, |b| {
        let (z, top) = (CROWN.1, 20.2);
        b.paint(PLATING);
        b.loft_z(
            &ngon(8, 1.0),
            &[Section::new(z, 3.0), Section::new(top, 2.7)],
        );
        b.radial(8, |b| {
            b.paint(ACCENT);
            b.beam(
                v3(2.8, 0.0, z + 0.5),
                v3(2.6, 0.0, top - 0.5),
                v2(0.3, 1.2),
                v2(0.3, 1.0),
            );
        });
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, top), 8, 3.2, 2.6, 0.8);
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, top + 0.8), 8, 1.1, 0.7, 21.95 - top - 0.8);
    });
}

/// The carousel: eight cells on a dark ring, every other one with a window onto the
/// matter in it. It repeats every quarter turn.
fn carousel(b: &mut MeshBuilder) {
    let (out, size) = CELL;
    b.paint(ACCENT);
    let ring = if b.fine() { 16 } else { 8 };
    annulus(b, ring, 5.6, 9.0, TRACK, TRACK + 0.4);
    for k in 0..8 {
        b.yawed(Vec3::ZERO, k as f32 * FRAC_PI_4, |b| {
            let mid = TRACK + 0.4 + size.z * 0.5;
            b.paint(PLATING);
            if b.fine() {
                b.chamfered_box(v3(out, 0.0, mid), size, 0.35);
            } else {
                b.cuboid(v3(out, 0.0, mid), size);
            }
            if b.fine() {
                // A lid on each cell, and a rib down its outer face.
                b.paint(ACCENT);
                b.chamfered_box(
                    v3(out, 0.0, mid + size.z * 0.5 + 0.1),
                    v3(size.x * 0.75, size.y * 0.75, 0.2),
                    0.15,
                );
                b.cuboid(
                    v3(out + size.x * 0.5 + 0.05, 0.0, mid - 0.6),
                    v3(0.1, size.y * 0.5, size.z * 0.5),
                );
            }
            if k % 2 == 0 {
                matter(
                    b,
                    v3(out + size.x * 0.5 + 0.03, 0.0, mid + 0.3),
                    v3(0.08, 1.2, 0.5),
                );
            }
        });
    }
}

/// The hub: an octagonal column with fins down it, a dark band at its head, the matter
/// in the head where the hammer strikes, four guide columns up to the crown.
fn hub(b: &mut MeshBuilder) {
    let (r0, r1, top) = HUB;
    let plan = ngon(8, 1.0);
    b.paint(PLATING);
    b.loft_z(&plan, &[Section::new(BASE.1, r0), Section::new(top, r1)]);
    if b.fine() {
        b.radial(8, |b| {
            b.yawed(Vec3::ZERO, TAU / 16.0, |b| {
                b.paint(ACCENT);
                b.beam(
                    v3(r0 * 0.95, 0.0, TRACK + 0.6),
                    v3(r1 * 0.95 + 0.6, 0.0, top - 0.5),
                    v2(0.3, 1.2),
                    v2(0.3, 0.9),
                );
            })
        });
    }
    b.paint(ACCENT);
    b.loft_z(&plan, &[Section::new(top, 4.9), Section::new(HEAD, 4.8)]);
    matter(b, v3(0.0, 0.0, HEAD + 0.15), v3(4.0, 4.0, 0.3));
    let post = 4.4;
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(PLATING);
            b.cuboid(
                v3(post, 0.0, (HEAD + CROWN.0) * 0.5),
                v3(0.9, 0.9, CROWN.0 - HEAD),
            );
        })
    });
}

/// The hammer: a heavy octagonal block with dark bands, its rod up into the crown.
fn hammer(b: &mut MeshBuilder) {
    let (z0, z1, r) = HAMMER;
    let plan = ngon(8, 1.0);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(z0, r * 0.9),
            Section::new(z0 + 0.3, r),
            Section::new(z1, r),
        ],
    );
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, z1), 8, 0.8, 0.8, CROWN.0 + 0.5 - z1);
    if b.fine() {
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[
                Section::new(z1 - 0.8, r * 1.03),
                Section::new(z1 - 0.4, r * 1.03),
            ],
        );
    }
}

/// The crown on the guide columns: a dark octagonal cap, a vent on it, lamps round it.
fn crown(b: &mut MeshBuilder) {
    let (z0, z1) = CROWN;
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, z0), 8, 5.0, 4.3, z1 - z0);
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, z1), 8, 1.0, 0.7, 15.95 - z1);
    let face = 4.65 * (std::f32::consts::PI / 8.0).cos();
    b.radial(4, |b| {
        lamp(b, v3(face + 0.04, 0.0, (z0 + z1) * 0.5), v3(0.1, 1.4, 0.4))
    });
}

/// A flat ring round the z axis, `r0` to `r1` out and `z0` to `z1` up.
fn annulus(b: &mut MeshBuilder, sides: usize, r0: f32, r1: f32, z0: f32, z1: f32) {
    let section = [(r0, z0), (r1, z0), (r1, z1), (r0, z1)];
    let rings: Vec<Vec<Vec3>> = (0..=sides)
        .map(|i| {
            let a = (i as f32 + 0.5) * TAU / sides as f32;
            let (c, s) = (a.cos(), a.sin());
            section.iter().map(|&(r, z)| v3(c * r, s * r, z)).collect()
        })
        .collect();
    b.loft(&rings, false, false);
}
