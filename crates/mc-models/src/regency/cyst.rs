//! The Reliquary, the Regency storage, on its 3 x 3 lot (36 m square): a vault between two cells.
//! It upgrades in place (tech 2 and 3), each tier adding working machinery round it.
//!
//! One building keeps mass and energy both:
//!
//! - The mass vault in the middle: an armoured octagon, plates lapped down its faces into
//!   spikes, a bronze hatch drum on its roof, the owner's colour across the hatch.
//! - Either side a long energy cell lying along x: a plated drum in bronze bands on plated
//!   saddles, bronze end caps, a red slot along its crown showing the charge, a course of
//!   plates lapped back along its top.
//! - Bronze feed pipes from the vault out to the cells.
//! - Tech 2 stands a capacitor column past each end of either cell: plated, banded, lit
//!   red up its outer face, cabled into the cell's end cap.
//! - Tech 3 spans the vault with two plated portal gantries, a hoist hanging from each
//!   over the hatch on a bronze shaft.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::kit::{cable, dark_plate, metal, seam, v3};
use super::machine::tier;
use super::machine::*;

/// The vault: its radius at the foot and at the roof, and its height.
const VAULT: (f32, f32, f32) = (6.0, 4.6, 6.2);
/// The energy cells: how far out either side, their radius and height, their half length.
const CELL_Y: f32 = 9.4;
const CELL_R: f32 = 2.5;
const CELL_Z: f32 = 3.4;
const CELL_HALF: f32 = 9.0;
/// The capacitor columns: where they stand along x and across y, their radius, height.
const COLUMN: (f32, f32, f32, f32) = (12.6, CELL_Y, 1.5, 11.0);
/// The gantries: where along x, where their posts stand across y, the underside and top
/// of the beam.
const GANTRY: (f32, f32, f32, f32) = (5.6, 13.2, 14.0, 15.4);
const THICK: f32 = 0.5;

pub(super) fn cyst(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    vault(b);
    b.mirror_y(|b| {
        cell(b);
        tier(b, tech, 2, 0.3, |b| {
            for x in [-COLUMN.0, COLUMN.0] {
                column(b, x);
            }
        });
    });
    tier(b, tech, 3, 0.2, |b| {
        for x in [-GANTRY.0, GANTRY.0] {
            gantry(b, x);
        }
    });
}

/// Far off: the vault, the two cells as three-sided bars, each end's columns as a plate,
/// the gantries as one deck, the owner's colour on the vault.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    let (r0, r1, h) = VAULT;
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, r0 * 1.2, r1 * 0.9, h + 1.8);
    b.mirror_y(|b| {
        dark_plate(b);
        b.cylinder_between(
            v3(-CELL_HALF - 1.0, CELL_Y, CELL_Z),
            v3(CELL_HALF + 1.0, CELL_Y, CELL_Z),
            CELL_R,
            CELL_R,
            3,
        );
    });
    let (cx, cy, cr, ch) = COLUMN;
    if tech >= 2 {
        // Each end's columns as one plate across it, faced both ways.
        for x in [-cx, cx] {
            let y = cy + cr;
            let quad = [v3(x, -y, 0.0), v3(x, y, 0.0), v3(x, y, ch), v3(x, -y, ch)];
            dark_plate(b);
            b.face(&quad);
            b.face(&[quad[3], quad[2], quad[1], quad[0]]);
        }
    }
    if tech >= 3 {
        // The two beams as one deck over the vault.
        let (gx, gy, z0, z1) = GANTRY;
        dark_plate(b);
        b.block(v3(-gx - 0.8, -gy - 0.8, z0), v3(gx + 0.8, gy + 0.8, z1));
    }
    b.paint(TEAM);
    let (r, z) = (r1 * 0.66, h + 1.82);
    b.face(&[v3(r, 0.0, z), v3(0.0, r, z), v3(-r, 0.0, z), v3(0.0, -r, z)]);
}

/// The mass vault: its plated octagon, plates lapped down its faces, the hatch drum on
/// the roof, the owner's colour across the hatch.
fn vault(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (r0, r1, h) = VAULT;
    seam(b);
    b.prism(Vec3::ZERO, 8, r0 + 0.8, r0 + 0.5, 0.9);
    dark_plate(b);
    b.prism(Vec3::Z * 0.9, 8, r0, r1, h - 0.9);
    for k in 0..4 {
        let a = (90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let top = d * (r1 + 0.2) + Vec3::Z * (h - 0.3);
        let foot = d * (r0 + 0.6) + Vec3::Z * 1.0;
        let f = Frame::new(top, foot - top, d + Vec3::Z * 0.35);
        dark_plate(b);
        Course {
            count: 2,
            step: 2.0,
            len: 3.2,
            half: 2.0,
            tip: 0.0,
            thick: THICK,
            tail: 0.3,
        }
        .lay(b, &f);
    }
    // The hatch drum lying across the roof.
    collar(b, v3(0.0, 0.0, h + 1.0), Vec3::Y, 1.5, 5.2);
    if fine {
        for y in [-2.9f32, 2.9] {
            collar(b, v3(0.0, y, h + 1.0), Vec3::Y, 1.8, 0.5);
        }
    }
    b.paint(TEAM);
    b.face(&[
        v3(-0.9, -2.4, h + 2.52),
        v3(0.9, -2.4, h + 2.52),
        v3(0.9, 2.4, h + 2.52),
        v3(-0.9, 2.4, h + 2.52),
    ]);
}

/// One energy cell (+y side): its saddles, the plated drum in bronze bands, its end caps,
/// the charge slot along its crown, plates lapped back over it, the feed pipe to the vault.
fn cell(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(12);
    let axis = |x: f32| v3(x, CELL_Y, CELL_Z);
    let saddles: &[f32] = if fine {
        &[-6.0, 0.0, 6.0]
    } else {
        &[-5.0, 5.0]
    };
    for &x in saddles {
        seam(b);
        b.frustum_open(
            v3(x, CELL_Y, 0.0),
            Vec2::new(2.2, CELL_R * 2.4),
            Vec2::new(1.8, CELL_R * 1.6),
            CELL_Z - CELL_R * 0.4,
            Vec2::ZERO,
        );
    }
    dark_plate(b);
    b.cylinder_between(axis(-CELL_HALF), axis(CELL_HALF), CELL_R, CELL_R, sides);
    let bands: &[f32] = if fine {
        &[-7.5, -4.5, -1.5, 1.5, 4.5, 7.5]
    } else {
        &[-4.5, 4.5]
    };
    for &x in bands {
        collar(b, axis(x), Vec3::X, CELL_R + 0.15, 0.5);
    }
    metal(b);
    b.cylinder_between(
        axis(CELL_HALF),
        axis(CELL_HALF + 1.4),
        CELL_R * 0.8,
        CELL_R * 0.35,
        sides,
    );
    b.cylinder_between(
        axis(-CELL_HALF),
        axis(-CELL_HALF - 1.4),
        CELL_R * 0.8,
        CELL_R * 0.35,
        sides,
    );
    // Plates lapped back along its top, the charge showing in a slot between them.
    let f = Frame::new(
        axis(CELL_HALF - 0.5) + Vec3::Z * (CELL_R - 0.1),
        -Vec3::X,
        Vec3::Z,
    );
    dark_plate(b);
    Course {
        count: 3,
        step: 5.4,
        len: 7.0,
        half: 1.3,
        tip: 0.0,
        thick: THICK,
        tail: 1.4,
    }
    .lay(b, &f);
    // The charge: a row of short red slots along the cell's outer shoulder.
    for k in 0..4 {
        let x = -5.4 + 3.6 * k as f32;
        red_slot(
            b,
            v3(x, CELL_Y + CELL_R * 0.72, CELL_Z + CELL_R * 0.69),
            v3(0.0, 0.7, 0.7),
            Vec3::X,
            2.2,
            0.2,
        );
    }
    if fine {
        metal(b);
        for z in [1.4f32, 2.4] {
            cable(
                b,
                &[v3(-2.0, VAULT.0 - 0.2, z), v3(-2.0, CELL_Y - CELL_R, z)],
                0.28,
            );
            cable(
                b,
                &[v3(2.0, VAULT.0 - 0.2, z), v3(2.0, CELL_Y - CELL_R, z)],
                0.28,
            );
        }
    }
}

/// A tech 2 capacitor column at `x` past one end of the +y cell: a seam footing, a plated
/// octagon shaft in bronze bands, plates lapped down it, a red strip up its outer face, a
/// bronze cap, and a cable into the cell's end cap.
fn column(b: &mut MeshBuilder, x: f32) {
    let fine = b.fine();
    let (_, y, r, h) = COLUMN;
    let at = v3(x, y, 0.0);
    let s = x.signum();
    seam(b);
    b.prism(at, 8, r + 0.7, r + 0.5, 0.8);
    dark_plate(b);
    b.prism(at + Vec3::Z * 0.8, 8, r, r * 0.85, h - 1.8);
    let bands: &[f32] = if fine { &[3.0, 5.6, 8.2] } else { &[5.6] };
    for &z in bands {
        collar(b, at + Vec3::Z * z, Vec3::Z, r * 0.95, 0.4);
    }
    metal(b);
    let sides = b.sides(8);
    b.prism(at + Vec3::Z * (h - 1.0), sides, r * 0.85, r * 0.5, 1.0);
    for k in 0..2 {
        let a = if k == 0 { 90.0f32 } else { 270.0 }.to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let top = at + d * (r + 0.1) + Vec3::Z * (h - 1.6);
        let f = Frame::new(top, -Vec3::Z + d * 0.1, d + Vec3::Z * 0.2);
        dark_plate(b);
        Course {
            count: if fine { 2 } else { 1 },
            step: 2.6,
            len: 3.4,
            half: 0.9,
            tip: 0.0,
            thick: THICK,
            tail: 0.8,
        }
        .lay(b, &f);
    }
    let out = v3(s, 0.0, 0.0);
    red_slot(
        b,
        at + out * (r * 0.95) + Vec3::Z * h * 0.5,
        out,
        Vec3::Z,
        h - 4.0,
        0.22,
    );
    if fine {
        let (z, cr, half) = (CELL_Z, CELL_R, CELL_HALF);
        metal(b);
        cable(
            b,
            &[
                at - out * (r * 0.9) + Vec3::Z * 6.8,
                v3(s * (half + 1.6), y, z + cr * 0.4),
            ],
            0.3,
        );
    }
}

/// A tech 3 gantry across the vault at `x`: a plated post past each cell with a bronze
/// shaft up it, the plated beam over the top with plates lapped back off it, a bronze
/// trolley and the hoist hanging from it over the hatch, red slots under the beam.
fn gantry(b: &mut MeshBuilder, x: f32) {
    let fine = b.fine();
    let (_, gy, z0, z1) = GANTRY;
    for y in [-gy, gy] {
        let at = v3(x, y, 0.0);
        seam(b);
        b.block(at + v3(-1.3, -1.3, 0.0), at + v3(1.3, 1.3, 1.0));
        dark_plate(b);
        b.beam(
            at + Vec3::Z * 1.0,
            at + Vec3::Z * z0,
            Vec2::new(1.6, 1.4),
            Vec2::new(1.2, 1.1),
        );
        if fine {
            shaft(
                b,
                at + v3(0.0, -y.signum() * 0.9, 1.0),
                at + v3(0.0, -y.signum() * 0.9, z0 - 0.6),
                0.25,
            );
        }
    }
    dark_plate(b);
    b.block(v3(x - 0.9, -gy - 0.9, z0), v3(x + 0.9, gy + 0.9, z1));
    let s = x.signum();
    for side in [-1.0f32, 1.0] {
        let f = Frame::new(
            v3(x + s * 0.2, side * 2.0, z1 + 0.05),
            v3(0.0, side, 0.0),
            Vec3::Z + v3(s, 0.0, 0.0) * 0.1,
        );
        dark_plate(b);
        Course {
            count: if fine { 2 } else { 1 },
            step: 4.0,
            len: 5.6,
            half: 0.85,
            tip: 0.0,
            thick: THICK,
            tail: 1.2,
        }
        .lay(b, &f);
    }
    metal(b);
    b.block(v3(x - 1.1, -1.2, z0 - 1.0), v3(x + 1.1, 1.2, z0));
    // The hoist: a bronze shaft down to a plated grab head over the hatch, its red eye
    // looking down.
    let head = VAULT.2 + 4.2;
    shaft(b, v3(x, 0.0, z0 - 1.0), v3(x, 0.0, head + 1.4), 0.3);
    if fine {
        collar(b, v3(x, 0.0, head + 1.6), Vec3::Z, 0.6, 0.4);
    }
    dark_plate(b);
    b.frustum(
        v3(x, 0.0, head),
        Vec2::new(1.6, 2.2),
        Vec2::new(1.1, 1.4),
        1.4,
        Vec2::ZERO,
    );
    red_slot(b, v3(x, 0.0, head - 0.02), -Vec3::Z, Vec3::Y, 1.2, 0.2);
    red_slot(b, v3(x, 6.0, z0 - 0.02), -Vec3::Z, Vec3::Y, 3.0, 0.2);
    red_slot(b, v3(x, -6.0, z0 - 0.02), -Vec3::Z, Vec3::Y, 3.0, 0.2);
}

#[cfg(test)]
mod tests {
    #[test]
    fn cyst() {
        for (tech, height) in [(1, 8.0), (2, 12.0), (3, 16.0)] {
            super::super::check_at("regency_cyst", tech, 12.9, height, Some(3), &[]);
        }
    }
}
