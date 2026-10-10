//! The Lens (`regency_gravitic_lens`): the Regency's tech 3 gravitic sniper, a small mobile
//! cousin of the Fallen, their tech 5 gravitic engine, and their answer to ARC's Arbalest.
//! Its legs are its crown: four great black shards stand on their boots round it and lean
//! in to meet over its middle ([`legs`]), and between their points, under the knees, it
//! holds a knot of gravity at [`CORE`]. The body hangs low between them, the well sunk in
//! its back with three prongs leaning up toward the core.
//!
//! It plants to fire (`deploy`): each leg steps out in turn to brace wider, then the body
//! settles to the ground between them; packing runs it back. Once down, it throws a
//! gravitic link from the core to its mark and holds a small well there.
//!
//! Authored planted, at the size the unit file gives it, the gravity held at [`CORE`] (the
//! unit file's `muzzle` and `pivot`). The gravity itself is not part of the model (renderer
//! `lens_fx`): nothing comes within [`CORE_CLEAR`] of it, and the light is in the plates
//! (`pattern::NANITE_SLOT` on the shards' inner faces and the well), never a lit piece.

mod legs;
#[cfg(test)]
mod tests;

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::gpu_consts::surface_mark as mark;

use super::commander::form::OCT;
use super::kit::{dark_plate, nanite_plate, seam, spike, v3};
use super::machine::hoop;

/// The unit file's size, planted.
pub(super) const RADIUS: f32 = 8.5;
pub(super) const HEIGHT: f32 = 7.5;

/// Where the Lens holds its gravity, planted: high over its middle, under the knees.
pub(super) const CORE: Vec3 = Vec3::new(0.0, 0.0, 5.9);
/// How far round the core the model leaves open for the gravity.
#[cfg(test)]
pub(super) const CORE_CLEAR: f32 = 0.85;

/// The body, planted: its belly, the top of its seam band, its back, and its reach.
const BELLY: f32 = 0.45;
const BAND: f32 = 0.9;
const BACK: f32 = 2.5;
const BODY: f32 = 3.0;

pub(super) fn lens(b: &mut MeshBuilder, _tech: u8) {
    b.set_dust_line(0.4);
    // The owner's colour on the body's back, behind the well.
    b.mark(
        v3(-2.8, -0.3, BACK - 0.15),
        v3(-1.8, 0.3, BACK + 0.15),
        mark::TEAM,
        0.0,
    );
    legs::legs(b);
    let plan: Vec<[f32; 2]> = OCT.iter().map(|&[x, y]| [x * BODY, y * BODY]).collect();
    if b.coarse() {
        dark_plate(b);
        b.loft_z(&plan, &[Section::new(BELLY, 0.8), Section::new(BACK, 0.85)]);
        return;
    }
    body(b, &plan);
    well(b, BACK + 0.02, 1.3, 0.35);
}

/// The hung body: a seam-dark band under plated flanks drawn in to its back, red optics at
/// its front.
fn body(b: &mut MeshBuilder, plan: &[[f32; 2]]) {
    seam(b);
    b.loft_z(
        plan,
        &[Section::new(BELLY, 0.8), Section::new(BAND + 0.02, 0.96)],
    );
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            plan,
            &[
                Section::new(BAND, 1.0),
                Section::new(BAND + (BACK - BAND) * 0.55, 1.0),
                Section::new(BACK, 0.82),
            ],
        )
    });
    b.mirror_y(|b| {
        b.mark(v3(2.9, 0.25, 1.5), v3(3.1, 0.75, 1.75), mark::LAMP, 0.0);
        b.mark(v3(-2.9, 0.4, 1.0), v3(-1.0, 2.6, 2.2), mark::OUTLINE, 0.0);
    });
}

/// The well under the core, its mouth at `z`, `r` across: a plated drum ringed in a dark
/// seam, its inner lip glazed so the gravity shows in it, and three prongs leaning up off
/// its rim toward the core to anchor it from below.
fn well(b: &mut MeshBuilder, z: f32, r: f32, depth: f32) {
    let sides = b.sides(12);
    dark_plate(b);
    b.prism(v3(0.0, 0.0, z - depth), sides, r * 1.12, r, depth);
    seam(b);
    hoop(b, v3(0.0, 0.0, z + 0.04), r * 0.92, r * 0.16, 0.14, sides);
    nanite_plate(b);
    b.prism(v3(0.0, 0.0, z - 0.05), sides, r * 0.78, r * 0.62, 0.12);
    dark_plate(b);
    for k in 0..3 {
        let a = (120.0 * k as f32 + 60.0f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let root = d * (r * 0.9) + Vec3::Z * z;
        // Stops short of the core, leaving it room.
        let to = CORE + d - Vec3::Z;
        spike(b, root, root.lerp(to, 0.8), r * 0.16);
    }
}
