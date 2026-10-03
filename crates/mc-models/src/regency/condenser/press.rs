//! Design C (`regency_fabricator~press`): a low plated casemate with the chamber's mouth in
//! its roof, a toothed ring turning flat round the mouth (`part::SPINNER`), and great field
//! plates leaning in over it from the corners like a press, a graphite core rising out of
//! the mouth between them where the matter condenses. The cells on the grid side feed the
//! casemate; the bin stands on the far side.
//! - Tech 1: the casemate, the ring, four corner plates over a short core, a cell, the bin.
//! - Tech 2: a second, taller ring of plates on the sides, the core raised, collared at the
//!   plates' tips; cells on both flanks.
//! - Tech 3: a plated head held at the top of the plates, the core up into it, a second row
//!   of cells.

use glam::{Vec2, Vec3};

use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::*;

/// The casemate's half-width, its chamfer and its roof.
const CASE: (f32, f32, f32) = (9.0, 2.6, 4.6);
/// The ring round the mouth: its radius to its middle.
const RING_R: f32 = 5.0;
/// The core: its radius and its top at each tier.
const CORE_R: f32 = 1.6;
const CORE_TOP: [f32; 3] = [9.0, 16.6, 24.6];
/// The corner plates, standing on the roof: foot out on the diagonal, tip in, the tip's
/// height; then the side plates' (tech 2).
const CORNER: (f32, f32, f32) = (8.0, 3.2, 12.6);
const SIDE: (f32, f32, f32) = (7.6, 3.0, 19.4);

pub(in crate::regency) fn press_plant(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    let (_, _, roof) = CASE;
    b.set_spinner_pivot(v3(0.0, 0.0, roof));

    // ---- Tech 1.
    casemate(b);
    mouth_ring(b);
    core(b, roof, CORE_TOP[0]);
    b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_4, |b| {
        b.radial(4, |b| field_plate(b, CORNER, 2.4));
    });
    flank_cells(b, -8.0, 5.5, 2.2, |s| v3(-1.6, s * (CASE.0 + 0.3), 3.2));
    bin(b, v3(CASE.0, 0.0, 2.8), v3(15.0, 0.0, 0.0), 3.2);

    // ---- Tech 2.
    tier(b, tech, 2, 0.2, |b| {
        b.radial(4, |b| field_plate(b, SIDE, 2.6));
    });
    tier(b, tech, 2, 0.45, |b| {
        core(b, CORE_TOP[0], CORE_TOP[1]);
        metal(b);
        hoop(b, v3(0.0, 0.0, SIDE.2 - 0.8), SIDE.1 + 0.4, 1.2, 1.0, 8);
    });
    tier(b, tech, 2, 0.75, |b| {
        flank_cells(b, 5.0, 4.0, 2.0, |s| v3(1.6, s * (CASE.0 + 0.3), 3.2));
    });

    // ---- Tech 3.
    tier(b, tech, 3, 0.25, |b| {
        core(b, CORE_TOP[1], CORE_TOP[2]);
        head(b);
    });
    tier(b, tech, 3, 0.8, |b| {
        for y in [-5.0, 5.0] {
            cell(b, v3(-20.4, y, 2.2), 3.4, 1.7);
        }
    });
}

/// The casemate: a seam footing, a plated block drawn in to its roof, plates lapped down
/// its four sides with a red slot under each.
fn casemate(b: &mut MeshBuilder) {
    let (half, cut, roof) = CASE;
    let plan = chamfered_rect(Vec2::splat(half), cut);
    seam(b);
    b.loft_z(&plan, &[Section::new(0.0, 1.06), Section::new(0.7, 1.05)]);
    dark_plate(b);
    b.loft_z(&plan, &[Section::new(0.7, 1.0), Section::new(roof, 0.9)]);
    if !b.fine() {
        return;
    }
    b.radial(4, |b| {
        let top = v3(half * 0.9 + 0.2, 0.0, roof - 0.2);
        let foot = v3(half + 0.5, 0.0, 1.0);
        let f = Frame::new(top, foot - top, v3(1.0, 0.0, 0.3));
        dark_plate(b);
        Course {
            count: 2,
            step: 1.4,
            len: 2.4,
            half: 3.0,
            tip: 0.0,
            thick: THICK,
            tail: 0.3,
        }
        .lay(b, &f);
        red_slot(
            b,
            v3(half + 0.2, 0.0, 1.6),
            v3(1.0, 0.0, 0.1),
            Vec3::Y,
            3.6,
            0.25,
        );
    });
}

/// The mouth in the roof and the toothed ring turning flat round it on four lugs.
fn mouth_ring(b: &mut MeshBuilder) {
    let fine = b.fine();
    let roof = CASE.2;
    seam(b);
    hoop(b, v3(0.0, 0.0, roof + 0.1), RING_R - 1.4, 1.2, 0.3, 8);
    b.with_part(part::SPINNER, |b| {
        metal(b);
        hoop(
            b,
            v3(0.0, 0.0, roof + 0.4),
            RING_R,
            1.2,
            0.8,
            if fine { 24 } else { 8 },
        );
        if fine {
            seam(b);
            teeth(
                b,
                v3(0.0, 0.0, roof + 0.4),
                RING_R + 0.6,
                24,
                v3(0.5, 0.45, 0.6),
            );
        }
        dark_plate(b);
        for k in 0..4 {
            let a = std::f32::consts::FRAC_PI_2 * k as f32;
            let d = v3(a.cos(), a.sin(), 0.0);
            b.cuboid(d * RING_R + Vec3::Z * (roof + 1.0), v3(1.3, 1.3, 0.5));
        }
    });
}

/// A section of the core from `z0` to `z1`: a graphite column, banded, with a red slot
/// up each side where the matter is condensing, the owner's colour on its cap.
fn core(b: &mut MeshBuilder, z0: f32, z1: f32) {
    let sides = b.sides(8);
    metal(b);
    b.prism(v3(0.0, 0.0, z0), sides, CORE_R, CORE_R * 0.9, z1 - z0);
    seam(b);
    b.prism(
        v3(0.0, 0.0, z1 - 0.5),
        sides,
        CORE_R * 1.15,
        CORE_R * 1.1,
        0.5,
    );
    b.paint(TEAM);
    let t = CORE_R * 0.75;
    b.face(&[
        v3(t, 0.0, z1 + 0.02),
        v3(0.0, t, z1 + 0.02),
        v3(-t, 0.0, z1 + 0.02),
        v3(0.0, -t, z1 + 0.02),
    ]);
    for k in 0..4 {
        let a = std::f32::consts::FRAC_PI_2 * k as f32 + std::f32::consts::PI / 8.0;
        let d = v3(a.cos(), a.sin(), 0.0);
        let r = CORE_R * 0.95 * (std::f32::consts::PI / 8.0).cos();
        red_slot(
            b,
            d * r + Vec3::Z * ((z0 + z1) * 0.5),
            d,
            Vec3::Z,
            (z1 - z0) * 0.6,
            0.3,
        );
    }
}

/// A field plate on +x (turned into place by the caller): a long swept plate leaning in
/// from its foot `(out, _, _)` on the ground to its tip `(_, tip_out, tip_z)` over the
/// mouth, `half` its half-width, on a seam footing; a red line down its inner face where
/// the field runs.
fn field_plate(b: &mut MeshBuilder, (out, tip_out, tip_z): (f32, f32, f32), half: f32) {
    let foot = v3(out, 0.0, CASE.2 - 0.3);
    let tip = v3(tip_out, 0.0, tip_z);
    seam(b);
    b.prism(foot, 6, 1.6, 1.3, 0.9);
    let up = tip - foot;
    let len = up.length();
    let f = Frame::new(foot + Vec3::Z * 0.6, up, v3(1.0, 0.0, 0.35));
    dark_plate(b);
    armour(b, &f, &swept(len - 0.6, half, 0.0, 0.8), THICK * 1.6);
    if b.fine() {
        // A graphite rib up its back.
        metal(b);
        b.beam(
            f.at(0.4, 0.0, THICK * 1.6),
            f.at(len * 0.85, 0.0, THICK * 1.6),
            Vec2::new(0.6, 0.5),
            Vec2::new(0.4, 0.4),
        );
    }
    red_slot(b, f.at(len * 0.55, 0.0, -0.02), -f.n, f.u, len * 0.5, 0.3);
}

/// Tech 3's head: a plated octagon held on the side plates' tips, the core going up into
/// it, red slots round it, the owner's colour on its crown.
fn head(b: &mut MeshBuilder) {
    let z0 = SIDE.2 + 0.4;
    seam(b);
    b.prism(v3(0.0, 0.0, z0), 8, 4.8, 4.8, 0.6);
    dark_plate(b);
    b.prism(v3(0.0, 0.0, z0 + 0.6), 8, 4.6, 3.4, 5.4);
    metal(b);
    let sides = b.sides(8);
    b.prism(v3(0.0, 0.0, z0 + 6.0), sides, 2.0, 1.6, 0.8);
    b.paint(TEAM);
    let t = 1.2;
    let top = z0 + 6.82;
    b.face(&[
        v3(t, 0.0, top),
        v3(0.0, t, top),
        v3(-t, 0.0, top),
        v3(0.0, -t, top),
    ]);
    if b.fine() {
        for k in 0..4 {
            let a = std::f32::consts::FRAC_PI_2 * k as f32 + std::f32::consts::PI / 8.0;
            let d = v3(a.cos(), a.sin(), 0.0);
            red_slot(
                b,
                d * 4.2 + Vec3::Z * (z0 + 2.4),
                d + Vec3::Z * 0.2,
                v3(-d.y, d.x, 0.0),
                1.6,
                0.3,
            );
        }
    }
}

/// Far off: the casemate, the plates as one tent of four faces to the tier's height, the
/// cell and the bin, the owner's colour on top.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    let (half, _, roof) = CASE;
    dark_plate(b);
    b.cuboid_open(v3(0.0, 0.0, roof * 0.5), v3(half * 2.0, half * 2.0, roof));
    let top = if tech >= 3 {
        SIDE.2 + 6.8
    } else {
        [CORNER.2, SIDE.2][tech as usize - 1]
    };
    b.frustum_open(
        v3(0.0, 0.0, roof),
        Vec2::splat(half * 1.6),
        Vec2::splat(3.0),
        top - roof,
        Vec2::ZERO,
    );
    flank_cells(b, -8.0, 5.5, 2.2, |_| Vec3::ZERO);
    bin(b, Vec3::ZERO, v3(15.0, 0.0, 0.0), 3.2);
    b.paint(TEAM);
    b.decal(v3(0.0, 0.0, top + 0.02), Vec2::splat(2.4));
}
