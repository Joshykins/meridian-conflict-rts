//! Design B (`regency_fabricator~orbit`): an armoured casemate under four plated field
//! pylons, and between them, held up in the air, the matter it is condensing: a dark mote
//! with gravity rings tumbling round it (`part::ORBIT`), one more ring a tier. The cells on
//! the grid side feed the casemate; the bin stands on the far side.
//! - Tech 1: the casemate, the pylons, the mote and one ring, one cell, the bin.
//! - Tech 2: the pylons raised, a second, wider ring, cells on both flanks.
//! - Tech 3: the pylons raised again into a plated crown, a third ring, a second row of
//!   cells.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::*;

/// The casemate: radius at its foot and its top, and its height.
const CASE: (f32, f32, f32) = (9.6, 7.8, 5.0);
/// The mote: where it hangs, and its radius.
const MOTE: Vec3 = Vec3::new(0.0, 0.0, 12.0);
const MOTE_R: f32 = 1.8;
/// The pylons: out on the diagonals, and their tops at each tier.
const PYLON_OUT: f32 = 8.4;
const PYLON_TOP: [f32; 3] = [14.6, 20.4, 26.0];
/// The rings round the mote, one a tier: radius, the line it lies square to, the line it
/// tumbles about, and its rate (rad/s).
const RINGS: [(f32, Vec3, Vec3, f32); 3] = [
    (
        3.4,
        Vec3::new(0.3, 0.0, 1.0),
        Vec3::new(1.0, 0.0, -0.3),
        0.9,
    ),
    (
        5.0,
        Vec3::new(0.0, 1.0, 0.25),
        Vec3::new(0.0, -0.25, 1.0),
        -0.6,
    ),
    (
        6.4,
        Vec3::new(1.0, 0.4, 0.0),
        Vec3::new(-0.4, 1.0, 0.3),
        0.4,
    ),
];

pub(in crate::regency) fn orbit_plant(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    b.set_spinner_pivot(MOTE);

    // ---- Tech 1.
    casemate(b);
    b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_4, |b| {
        b.radial(4, |b| pylon(b, CASE.2, PYLON_TOP[0], true));
    });
    mote(b);
    ring(b, 0);
    flank_cells(b, -8.0, 5.5, 2.2, |s| {
        v3(-1.6, s * (CASE.1 * 0.92 + 0.6), 3.6)
    });
    bin(b, v3(8.0, 0.0, 2.6), v3(15.0, 0.0, 0.0), 3.2);

    // ---- Tech 2.
    tier(b, tech, 2, 0.2, |b| {
        b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_4, |b| {
            b.radial(4, |b| pylon(b, PYLON_TOP[0], PYLON_TOP[1], false));
        });
    });
    tier(b, tech, 2, 0.5, |b| ring(b, 1));
    tier(b, tech, 2, 0.75, |b| {
        flank_cells(b, 5.0, 4.0, 2.0, |s| {
            v3(1.6, s * (CASE.1 * 0.92 + 0.6), 3.6)
        });
    });

    // ---- Tech 3.
    tier(b, tech, 3, 0.2, |b| {
        b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_4, |b| {
            b.radial(4, |b| pylon(b, PYLON_TOP[1], PYLON_TOP[2], false));
        });
        dark_plate(b);
        hoop(
            b,
            v3(0.0, 0.0, PYLON_TOP[2] + 0.4),
            PYLON_OUT - 0.2,
            1.6,
            0.9,
            8,
        );
    });
    tier(b, tech, 3, 0.5, |b| ring(b, 2));
    tier(b, tech, 3, 0.8, |b| {
        for y in [-5.0, 5.0] {
            cell(b, v3(-20.4, y, 2.2), 3.4, 1.7);
        }
    });
}

/// The casemate: a seam footing, a plated octagon drawn in to its top, plates lapped down
/// four of its faces, a red slot on each, a graphite field plate on top under the mote.
fn casemate(b: &mut MeshBuilder) {
    let (r0, r1, h) = CASE;
    seam(b);
    b.prism(Vec3::ZERO, 8, r0 + 0.5, r0 + 0.3, 0.8);
    dark_plate(b);
    b.prism(Vec3::Z * 0.8, 8, r0, r1, h - 0.8);
    metal(b);
    let sides = b.sides(8);
    b.prism(Vec3::Z * h, sides, r1 * 0.55, r1 * 0.45, 0.6);
    b.paint(TEAM);
    let t = 2.0;
    b.face(&[
        v3(t, 0.0, h + 0.62),
        v3(0.0, t, h + 0.62),
        v3(-t, 0.0, h + 0.62),
        v3(0.0, -t, h + 0.62),
    ]);
    if !b.fine() {
        return;
    }
    for k in 0..4 {
        let a = std::f32::consts::FRAC_PI_2 * k as f32 + std::f32::consts::PI / 8.0;
        let d = v3(a.cos(), a.sin(), 0.0);
        let top = d * (r1 * 0.92 + 0.2) + Vec3::Z * (h - 0.2);
        let foot = d * (r0 * 0.92 + 0.5) + Vec3::Z * 1.0;
        let f = Frame::new(top, foot - top, d + Vec3::Z * 0.3);
        dark_plate(b);
        Course {
            count: 2,
            step: 1.6,
            len: 2.6,
            half: 2.2,
            tip: 0.0,
            thick: THICK,
            tail: 0.3,
        }
        .lay(b, &f);
        let mid = (top + foot) * 0.5 + d * 0.35;
        red_slot(b, mid, d + Vec3::Z * 0.2, v3(-d.y, d.x, 0.0), 2.4, 0.25);
    }
}

/// A field pylon section on +x (the caller turns it onto a diagonal) from `z0` to `z1`: a
/// plated column, graphite collar at the joint, plates lapped down its outer face, and at
/// its head a red emitter slot facing the mote. The first section has a seam footing.
fn pylon(b: &mut MeshBuilder, z0: f32, z1: f32, first: bool) {
    let at = v3(PYLON_OUT, 0.0, 0.0);
    if first {
        seam(b);
        b.prism(at + Vec3::Z * (z0 - 0.6), 6, 1.6, 1.4, 0.6);
    }
    dark_plate(b);
    b.prism(at + Vec3::Z * z0, 6, 1.1, 0.95, z1 - z0 - 0.6);
    metal(b);
    b.prism(at + Vec3::Z * (z1 - 0.6), 6, 1.2, 1.0, 0.6);
    red_slot(
        b,
        at + v3(-1.0, 0.0, z1 - 1.6),
        -Vec3::X,
        Vec3::Z,
        1.6,
        0.35,
    );
    if b.fine() {
        let top = at + v3(0.9, 0.0, z1 - 1.0);
        let f = Frame::new(top, -Vec3::Z, v3(1.0, 0.0, 0.15));
        dark_plate(b);
        Course {
            count: 2,
            step: (z1 - z0) * 0.38,
            len: (z1 - z0) * 0.45,
            half: 0.8,
            tip: 0.0,
            thick: THICK * 0.8,
            tail: 0.4,
        }
        .lay(b, &f);
    }
}

/// The mote: the matter forming, a dark faceted lump with a hot seam round its middle
/// where it is still taking shape.
fn mote(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.paint(ROCK);
    b.with_facets(|b| {
        b.spheroid(
            MOTE,
            v3(MOTE_R, MOTE_R, MOTE_R * 1.2),
            if fine { 6 } else { 4 },
            if fine { 3 } else { 2 },
        )
    });
    if fine {
        b.paint(GLOW_ORANGE);
        hoop(b, MOTE, MOTE_R * 1.02, 0.25, 0.3, 6);
    }
}

/// Ring `k` round the mote, tumbling: a graphite band with plated weights on it.
fn ring(b: &mut MeshBuilder, k: usize) {
    let (r, normal, spin, rate) = RINGS[k];
    let normal = normal.normalize();
    let segs = if b.fine() { 24 } else { 10 };
    b.with_orbit(spin, rate, |b| {
        metal(b);
        hoop_on(b, MOTE, normal, r, 0.5, 0.35, segs);
        let e1 = normal.any_orthonormal_vector();
        let e2 = normal.cross(e1);
        dark_plate(b);
        for j in 0..3 {
            let a = std::f32::consts::TAU * j as f32 / 3.0;
            let d = e1 * a.cos() + e2 * a.sin();
            b.cylinder_between(
                MOTE + d * r - normal * 0.4,
                MOTE + d * r + normal * 0.4,
                0.55,
                0.55,
                4,
            );
        }
    });
}

/// Far off: the casemate, the pylons as one plate each way at the tier's height, the mote,
/// the cell and the bin, the owner's colour on top.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    let (r0, r1, h) = CASE;
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, r0, r1, h);
    let top = PYLON_TOP[tech as usize - 1];
    let s = PYLON_OUT * std::f32::consts::FRAC_1_SQRT_2;
    for (x, y) in [(s, s), (-s, s)] {
        let quad = [v3(x, y, h), v3(-x, -y, h), v3(-x, -y, top), v3(x, y, top)];
        b.face(&quad);
        b.face(&[quad[3], quad[2], quad[1], quad[0]]);
    }
    b.paint(ROCK);
    b.prism(
        MOTE - Vec3::Z * MOTE_R,
        3,
        MOTE_R,
        MOTE_R * 0.4,
        MOTE_R * 2.0,
    );
    flank_cells(b, -8.0, 5.5, 2.2, |_| Vec3::ZERO);
    bin(b, Vec3::ZERO, v3(15.0, 0.0, 0.0), 3.2);
    b.paint(TEAM);
    let t = 2.0;
    b.face(&[
        v3(t, 0.0, h + 0.02),
        v3(0.0, t, h + 0.02),
        v3(-t, 0.0, h + 0.02),
        v3(0.0, -t, h + 0.02),
    ]);
}
