//! Design B (`fabricator~b`), the crucible: an armoured drum with heat sinks on its
//! corners, a neck braced off it, and on the neck a broad crucible brimming with hot
//! condensate; a condenser head hangs over the melt on four arms off the rim.
//! - Tech 2: the drum, the crucible, the head on its arms.
//! - Tech 3: a glazed injector column stacked on the head under a cap, four tall struts
//!   from the rim holding it, capacitor bastions on the grid sides (±x).

use std::f32::consts::{FRAC_PI_4, TAU};

use glam::Vec3;

use super::super::parts::*;
use super::*;

/// The drum: radius at foot and top, its top.
const DRUM: (f32, f32, f32) = (8.6, 7.8, 3.0);
/// The neck's top, and the crucible's sections (up, radius) to its rim.
const NECK: f32 = 6.8;
const BOWL: [(f32, f32); 3] = [(NECK, 3.2), (8.2, 5.2), (11.4, 7.2)];
const RIM: (f32, f32, f32) = (6.3, 7.5, 12.2);
/// An arm's path, (out, up), on its diagonal, and the head it holds.
const ARM: [(f32, f32); 3] = [(6.6, 12.1), (5.4, 14.7), (2.1, 15.3)];
const HEAD: (f32, f32) = (14.2, 15.9);
/// Tech 3's column on the head, and a strut's path.
const COLUMN: (f32, f32, f32) = (16.0, 20.4, 1.3);
const STRUT: [(f32, f32); 3] = [(7.2, 12.2), (6.0, 16.6), (1.9, 19.8)];

pub(in crate::aster) fn build(b: &mut MeshBuilder, tech: u8) {
    plinth(b);
    corner_sinks(b, 9.4, 13.6, 2.4);
    drum(b);
    crucible(b);
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| arm(b, &ARM, v2(1.4, 0.9)))
    });
    head(b);

    tech_3(b, tech, 0.2, |b| {
        b.radial(2, |b| bastion(b, v3(9.9, 0.0, DECK), 6.0, 1.8))
    });
    tech_3(b, tech, 0.5, |b| {
        let (z0, z1, r) = COLUMN;
        chamber(b, z0, z1, r);
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, z1), b.sides(8), r * 1.25, r * 0.8, 0.9);
        if !b.coarse() {
            b.paint(METAL);
            b.prism(v3(0.0, 0.0, z1 + 0.9), b.sides(8), r * 0.4, r * 0.2, 0.7);
        }
    });
    tech_3(b, tech, 0.75, |b| {
        b.radial(4, |b| arm(b, &STRUT, v2(1.2, 0.8)));
    });
}

/// The armoured drum, ribs from it up to the neck on the axes, the neck.
fn drum(b: &mut MeshBuilder) {
    let (r0, r1, top) = DRUM;
    let sides = b.sides(8);
    b.paint(PLATING);
    if b.coarse() {
        b.prism(v3(0.0, 0.0, DECK), 4, r0 * 1.1, r1, top - DECK);
        return;
    }
    let plan = ngon(sides, 1.0);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(DECK, r0), Section::new(DECK + 0.5, r0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(DECK + 0.5, r0 * 0.99), Section::new(top, r1)],
    );
    b.paint(ACCENT);
    b.loft_z(&plan, &[Section::new(top, 3.8), Section::new(NECK, 3.3)]);
    b.radial(4, |b| {
        b.paint(PLATING);
        b.beam(
            v3(r1 * 0.85, 0.0, top),
            v3(3.4, 0.0, NECK - 0.2),
            v2(1.4, 1.0),
            v2(1.2, 0.8),
        );
        if b.fine() {
            vent(
                b,
                v3(r1 * 0.68 + 0.6, 0.0, top),
                v2(1.0, 3.0),
                4,
                GLOW_ORANGE,
            );
        }
    });
}

/// The crucible on the neck: a flared octagonal bowl, a dark rim round the melt.
fn crucible(b: &mut MeshBuilder) {
    let plan = ngon(b.sides(8), 1.0);
    let (r0, r1, rim) = RIM;
    let top = BOWL[2].0;
    if b.coarse() {
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, NECK), 4, 4.0, 9.0, top - NECK);
        b.paint(GLOW_ORANGE);
        b.decal(v3(0.0, 0.0, top + 0.05), v2(8.0, 8.0));
        return;
    }
    let sections: Vec<Section> = BOWL.iter().map(|&(z, r)| Section::new(z, r)).collect();
    b.paint(PLATING);
    b.loft_z(&plan, &sections);
    if b.fine() {
        b.paint(ACCENT);
        b.loft_z(&plan, &[Section::new(9.6, 6.18), Section::new(10.2, 6.6)]);
    }
    // The melt, standing a hand proud of the bowl's top inside the rim.
    b.paint(GLOW_ORANGE).pattern(pattern::HEAT);
    b.loft_z(
        &plan,
        &[
            Section::new(top, r0 * 1.02),
            Section::new(top + 0.25, r0 * 1.02),
        ],
    );
    b.paint(ACCENT);
    annulus(b, plan.len(), r0, r1, top - 0.1, rim);
}

/// The condenser head over the melt: a dark octagonal block, its emitter aimed down.
fn head(b: &mut MeshBuilder) {
    let (z0, z1) = HEAD;
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, z0), b.sides(8), 2.4, 1.7, z1 - z0);
    if b.coarse() {
        return;
    }
    b.paint(GLOW_ORANGE);
    b.prism(v3(0.0, 0.0, z0 - 0.9), b.sides(8), 0.5, 1.5, 0.9);
    team_panel(b, v3(0.0, 0.0, z1), v2(1.6, 1.6));
}

/// An arm on +x along `path`, (out, up): a plated bar from the rim in to what it holds.
fn arm(b: &mut MeshBuilder, path: &[(f32, f32); 3], size: Vec2) {
    if b.coarse() {
        return;
    }
    let pts: Vec<Vec3> = path.iter().map(|&(x, z)| v3(x, 0.0, z)).collect();
    b.paint(PLATING);
    sweep(b, &pts, size);
    if b.fine() {
        b.paint(ACCENT);
        b.beam(
            pts[0] - Vec3::Z * 0.3,
            pts[0] + Vec3::Z * 0.6,
            size * 1.5,
            size * 1.3,
        );
    }
}

/// A flat ring round the z axis, `r0` to `r1` out and `z0` to `z1` up, its flats square
/// to the axes like [`ngon`]'s.
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
