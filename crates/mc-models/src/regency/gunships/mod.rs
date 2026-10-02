//! The Regency's hovering combat craft (`data/factions/regency/units/air_gunships.ron`,
//! docs/STYLE.md "The Regency look"): no rotors, no jets, each held up on gravity lift bells
//! (`regency::lift`) with red plasma crackling under their mouths.
//!
//! - The Quiver (`quiver`), the tech 2 drone carrier: a hull with six bays down its back,
//!   a Wick in each.
//! - The Wick (`wick`): the Quiver's drone, a pinched-plasmeric charge on one small bell.
//! - The Reaper (`reaper`), the tech 3 beam assault craft: a heavy craft on six bells, a
//!   Pinch-fusion Beam projector under its nose.
//!
//! Each file has its own catalogue entries (`MODELS`). Finish: dark plate lapped over dark
//! bronze, red optics and charges; no violet (none of them builds). An aircraft's origin is
//! the bottom of its lift bells; the shader heaves the hull in flight (`hover_sway`).

pub(crate) mod quiver;
pub(crate) mod reaper;
pub(crate) mod wick;

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::*;

use super::kit::{dark_plate, seam, v3};
use super::machine::{armour, red_slot, Frame};

/// `half` an outline (x, y >= 0, from the nose round to the tail) and its mirror,
/// counter-clockwise from above.
fn plan(half: &[[f32; 2]]) -> Vec<[f32; 2]> {
    half.iter()
        .copied()
        .chain(
            half.iter()
                .rev()
                .filter(|p| p[1] > 0.0)
                .map(|&[x, y]| [x, -y]),
        )
        .collect()
}

/// A faceted body of the outline `half` (mirrored), a seam-dark belly from `belly` and dark
/// plate up to `deck`, its flanks drawn in toward the top by `tuck`.
fn body(b: &mut MeshBuilder, half: &[[f32; 2]], belly: f32, deck: f32, tuck: f32) {
    let outline = plan(half);
    b.with_facets(|b| {
        seam(b);
        b.loft_z(
            &outline,
            &[Section::new(belly, 0.8), Section::new(belly + 0.25, 0.95)],
        );
        dark_plate(b);
        b.loft_z(
            &outline,
            &[
                Section::new(belly + 0.25, 1.0),
                Section::new(deck - (deck - belly) * 0.3, 1.0),
                Section::new(deck, tuck),
            ],
        );
    });
}

/// The sensor head at `nose` (its point, at `z`): a brow plate `w` wide over pairs of red
/// optics either side.
fn head(b: &mut MeshBuilder, nose: f32, z: f32, w: f32) {
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(nose, 0.0, z), v3(-1.0, 0.0, 0.25), Vec3::Z),
        &[
            [0.0, -0.08],
            [0.0, 0.08],
            [w * 0.6, w * 0.5],
            [w * 1.2, w * 0.45],
            [w * 1.2, -w * 0.45],
            [w * 0.6, -w * 0.5],
        ],
        0.14 * w.max(0.5),
    );
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(nose - w * 0.35, w * 0.32, z - 0.12 * w),
            v3(0.7, 0.7, 0.0),
            Vec3::Y,
            0.3 * w,
            0.1 * w,
        );
        if b.fine() {
            red_slot(
                b,
                v3(nose - w * 0.75, w * 0.48, z - 0.1 * w),
                v3(0.5, 0.85, 0.0),
                Vec3::Y,
                0.2 * w,
                0.08 * w,
            );
        }
    });
}

/// The owner's mark: a team-coloured chevron laid on a deck at `z`, pointing forward.
fn team_mark(b: &mut MeshBuilder, x: f32, z: f32, size: f32) {
    b.paint(TEAM);
    b.face(&[
        v3(x + size * 0.6, 0.0, z + 0.01),
        v3(x - size * 0.5, size * 0.55, z + 0.01),
        v3(x - size * 0.2, 0.0, z + 0.01),
    ]);
    b.face(&[
        v3(x + size * 0.6, 0.0, z + 0.01),
        v3(x - size * 0.2, 0.0, z + 0.01),
        v3(x - size * 0.5, -size * 0.55, z + 0.01),
    ]);
}

/// Budgets for the library's checks (`regency::triangles`).
#[cfg(test)]
pub(super) fn triangles(key: &str) -> Option<usize> {
    match key.split('~').next().unwrap_or(key) {
        "regency_wick" => Some(1200),
        "regency_drone_carrier" => Some(4500),
        "regency_assault_aircraft" => Some(9000),
        _ => None,
    }
}

/// Faceted plates and bells that keep their sides when reduced.
#[cfg(test)]
pub(super) fn reduced_share(key: &str) -> Option<f32> {
    match key.split('~').next().unwrap_or(key) {
        "regency_wick" | "regency_drone_carrier" | "regency_assault_aircraft" => Some(0.5),
        _ => None,
    }
}
