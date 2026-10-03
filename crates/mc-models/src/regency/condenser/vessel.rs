//! Design A (`regency_fabricator`): a tall armoured vessel stacked a stage higher at each
//! tier, girdled by a toothed press ring turning round its waist on plated carriages
//! (`part::SPINNER`), the cells on the grid side cabled into it, the bin on the far side.
//! - Tech 1: the vessel and its field collars, the press ring, one cell, the bin.
//! - Tech 2: an upper stage held by four plated buttresses, cells on both flanks.
//! - Tech 3: a third stage, the buttresses carried up over it into a plated crown, a second
//!   row of cells.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::*;

/// The vessel's stages: (radius, foot, top of the straight body). Each stands on the
/// hatch of the one below.
const STAGES: [(f32, f32, f32); 3] = [(5.5, 1.2, 8.6), (4.0, 11.62, 16.8), (2.9, 19.22, 24.0)];
/// The press ring: its height, radius to its middle.
const RING_Z: f32 = 5.2;
const RING_R: f32 = 7.6;
/// The buttresses' feet out on the diagonals, and where they meet the upper stage.
const BUTTRESS: (f32, f32, f32) = (12.5, 5.0, 16.4);

/// The top of a stage's hatch.
fn hatch(stage: usize) -> f32 {
    let (r, _, z1) = STAGES[stage];
    z1 + r * 0.4 + 0.82
}

pub(in crate::regency) fn vessel_plant(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    b.set_spinner_pivot(v3(0.0, 0.0, RING_Z));

    // ---- Tech 1: the plinth, the vessel and its collars, the press ring, a cell, the bin.
    seam(b);
    b.prism(Vec3::ZERO, 8, 10.0, 9.6, 0.6);
    dark_plate(b);
    b.prism(Vec3::Z * 0.6, 8, 9.6, 8.8, 0.6);
    let (r, z0, z1) = STAGES[0];
    vessel(b, Vec3::ZERO, r, z0, z1);
    if b.fine() {
        for z in [3.2, 7.4] {
            field_collar(b, Vec3::ZERO, r, z);
        }
    }
    press_ring(b);
    flank_cells(b, -8.0, 5.5, 2.2, |s| v3(-1.6, s * (r * 0.92 + 0.1), 8.3));
    bin(b, v3(5.4, 0.0, 2.4), v3(14.5, 0.0, 0.0), 3.2);

    // ---- Tech 2: the upper stage on four buttresses, cells on the flanks.
    tier(b, tech, 2, 0.2, |b| {
        b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_4, |b| {
            b.radial(4, buttress);
        });
    });
    tier(b, tech, 2, 0.45, |b| {
        let (r, z0, z1) = STAGES[1];
        vessel(b, Vec3::ZERO, r, z0, z1);
        if b.fine() {
            for z in [13.6, 15.6] {
                field_collar(b, Vec3::ZERO, r, z);
            }
        }
    });
    tier(b, tech, 2, 0.7, |b| {
        flank_cells(b, 5.0, 4.0, 2.0, |s| v3(1.6, s * (r * 0.92 + 0.1), 8.3));
    });

    // ---- Tech 3: a third stage, a plated crown over it, a second row of cells.
    tier(b, tech, 3, 0.2, |b| {
        let (r, z0, z1) = STAGES[2];
        vessel(b, Vec3::ZERO, r, z0, z1);
        if b.fine() {
            field_collar(b, Vec3::ZERO, r, 21.4);
        }
    });
    tier(b, tech, 3, 0.5, |b| {
        b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_4, |b| {
            b.radial(4, |b| {
                let (_, top_out, top_z) = BUTTRESS;
                strut(b, v3(top_out, 0.0, top_z + 0.4), v3(3.6, 0.0, 27.0), 0.7);
            });
        });
        dark_plate(b);
        hoop(b, v3(0.0, 0.0, 27.2), 3.9, 1.4, 1.0, 8);
        if b.fine() {
            for k in 0..4 {
                let a = std::f32::consts::FRAC_PI_2 * k as f32 + std::f32::consts::PI / 8.0;
                let d = v3(a.cos(), a.sin(), 0.0);
                red_slot(b, d * 4.5 + Vec3::Z * 27.2, d, v3(-d.y, d.x, 0.0), 1.2, 0.3);
            }
        }
    });
    tier(b, tech, 3, 0.8, |b| {
        for y in [-5.0, 5.0] {
            cell(b, v3(-20.0, y, 2.2), 3.4, 1.7);
        }
    });
}

/// The toothed press ring round the vessel's waist: a plated race on four posts down to the
/// plinth, and the graphite ring turning on it, toothed round its rim, with four plated
/// lugs on it so its turning shows.
fn press_ring(b: &mut MeshBuilder) {
    let fine = b.fine();
    let at = v3(0.0, 0.0, RING_Z);
    let race = at - Vec3::Z * 0.8;
    for k in 0..4 {
        let a = std::f32::consts::FRAC_PI_2 * k as f32 + std::f32::consts::FRAC_PI_4;
        let d = v3(a.cos(), a.sin(), 0.0);
        strut(
            b,
            d * RING_R + Vec3::Z * 1.2,
            d * RING_R + race - Vec3::Z * 0.3,
            0.5,
        );
    }
    dark_plate(b);
    hoop(b, race, RING_R, 1.8, 0.6, if fine { 24 } else { 8 });
    b.with_part(part::SPINNER, |b| {
        metal(b);
        hoop(b, at, RING_R, 1.2, 0.8, if fine { 24 } else { 8 });
        if fine {
            seam(b);
            teeth(b, at, RING_R + 0.6, 24, v3(0.5, 0.45, 0.7));
        }
        dark_plate(b);
        for k in 0..4 {
            let a = std::f32::consts::FRAC_PI_2 * k as f32;
            let d = v3(a.cos(), a.sin(), 0.0);
            b.cuboid(at + d * RING_R + Vec3::Z * 0.6, v3(1.4, 1.4, 0.5));
        }
    });
}

/// A buttress on +x (turned onto a diagonal by the caller): a seam footing, a plated strut
/// up to the upper stage, plates lapped down its back.
fn buttress(b: &mut MeshBuilder) {
    let (foot, top_out, top_z) = BUTTRESS;
    seam(b);
    b.prism(v3(foot, 0.0, 0.0), 6, 1.8, 1.5, 1.0);
    strut(b, v3(foot, 0.0, 0.9), v3(top_out, 0.0, top_z), 0.8);
    if b.fine() {
        let top = v3(top_out + 0.9, 0.0, top_z - 0.2);
        let down = v3(foot, 0.0, 1.0) - top;
        let f = Frame::new(top, down, v3(0.8, 0.0, 0.6));
        dark_plate(b);
        Course {
            count: 2,
            step: down.length() * 0.38,
            len: down.length() * 0.42,
            half: 1.0,
            tip: 0.0,
            thick: THICK,
            tail: 0.6,
        }
        .lay(b, &f);
    }
}

/// Far off: the vessel as one block to the tier's height, the ring as a plate, the cell,
/// the bin, the owner's colour on top.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    let top = hatch(tech as usize - 1);
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, 9.0, STAGES[tech as usize - 1].0, top);
    flank_cells(b, -8.0, 5.5, 2.2, |_| Vec3::ZERO);
    bin(b, Vec3::ZERO, v3(14.5, 0.0, 0.0), 3.2);
    b.paint(TEAM);
    let t = 2.0;
    b.face(&[
        v3(t, 0.0, top + 0.02),
        v3(0.0, t, top + 0.02),
        v3(-t, 0.0, top + 0.02),
        v3(0.0, -t, top + 0.02),
    ]);
}
