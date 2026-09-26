//! The Cyst, the Naga storage, on its 3 x 3 lot (36 m square): a vault between two cells.
//!
//! One building keeps mass and energy both:
//!
//! - The mass vault in the middle: an armoured octagon, plates lapped down its faces into
//!   spikes, a bronze hatch drum on its roof between two hatch rams (`part::PUMP`), the
//!   owner's colour across the hatch.
//! - Either side a long energy cell lying along x: a plated drum in bronze bands on plated
//!   saddles, bronze end caps, a red slot along its crown showing the charge, a course of
//!   plates lapped back along its top.
//! - Bronze feed pipes from the vault out to the cells.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;

use super::kit::{cable, dark_plate, metal, seam, v3};
use super::machine::*;

/// The vault: its radius at the foot and at the roof, and its height.
const VAULT: (f32, f32, f32) = (6.0, 4.6, 6.2);
/// The energy cells: how far out either side, their radius and height, their half length.
const CELL_Y: f32 = 9.4;
const CELL_R: f32 = 2.5;
const CELL_Z: f32 = 3.4;
const CELL_HALF: f32 = 9.0;
const THICK: f32 = 0.5;

pub(super) fn cyst(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        coarse(b);
        return;
    }
    vault(b);
    b.mirror_y(cell);
}

/// Far off: the vault, the two cells, the owner's colour on the vault.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    let (r0, r1, h) = VAULT;
    b.prism(Vec3::ZERO, 4, r0 * 1.2, r1 * 0.9, h + 1.8);
    b.mirror_y(|b| {
        dark_plate(b);
        b.cylinder_between(
            v3(-CELL_HALF - 1.0, CELL_Y, CELL_Z),
            v3(CELL_HALF + 1.0, CELL_Y, CELL_Z),
            CELL_R,
            CELL_R,
            4,
        );
    });
    b.paint(TEAM);
    let (r, z) = (r1 * 0.66, h + 1.82);
    b.face(&[v3(r, 0.0, z), v3(0.0, r, z), v3(-r, 0.0, z), v3(0.0, -r, z)]);
}

/// The mass vault: its plated octagon, plates lapped down its faces, the hatch drum and
/// its rams on the roof, the owner's colour across the hatch.
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
    // The hatch drum lying across the roof, and the rams either side that work it.
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
    for x in [-3.0f32, 3.0] {
        dark_plate(b);
        b.block(v3(x - 0.8, -0.8, h), v3(x + 0.8, 0.8, h + 0.6));
        piston(b, v3(x, 0.0, h + 0.6), v3(x, 0.0, h + 3.2), 0.4, true);
    }
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

#[cfg(test)]
mod tests {
    #[test]
    fn cyst() {
        super::super::check("naga_cyst", 12.9, 8.0, Some(3), &[]);
    }
}
