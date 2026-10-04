//! The Gravitic Seeker cells: two blocks of eight open cells on the hump either side of
//! the spine, port block first (`space.ron` weapons 1 and 2, their `muzzles` in `FIRE`
//! order). Each is an armoured box stood on the hull, its deck let into eight open cells,
//! each a lit rim round the head of the seeker waiting in it.

use glam::Vec2;
#[cfg(test)]
use glam::Vec3;

use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::GLOW_LASER;

use super::super::super::kit::{dark_plate, metal, seam};
use super::body::Body;

/// The blocks' middle along the hull, out from the spine, and the cells' spacing.
const X: f32 = -60.0;
const Y: f32 = 15.5;
const PITCH: f32 = 2.4;
/// Cells along and across.
const NX: usize = 4;
const NY: usize = 2;
/// The order the cells fire in, (along, across): spread over the block.
const FIRE: [(usize, usize); 8] = [
    (0, 0),
    (2, 1),
    (1, 0),
    (3, 1),
    (0, 1),
    (2, 0),
    (1, 1),
    (3, 0),
];
/// How far under its cell's deck a seeker is launched from.
#[cfg(test)]
const MUZZLE_DROP: f32 = 1.6;

fn half() -> Vec2 {
    Vec2::new(
        PITCH * NX as f32 * 0.5 + 0.45,
        PITCH * NY as f32 * 0.5 + 0.45,
    )
}

/// The block's middle and its deck's height, port side `1.0`, starboard `-1.0`. The deck
/// clears the hull at the block's inner edge, where it stands highest.
fn block_at(body: &Body, side: f32) -> (Vec2, f32) {
    let h = half();
    let deck = [X - h.x, X, X + h.x]
        .iter()
        .map(|&x| body.point(x, body.phi_up(x, Y - h.y)).z)
        .fold(f32::MIN, f32::max)
        + 1.6;
    (Vec2::new(X, Y * side), deck)
}

/// The cells' middles in firing order.
fn cells(centre: Vec2) -> impl Iterator<Item = Vec2> {
    FIRE.iter().map(move |&(i, j)| {
        let at = |k: usize, n: usize| (k as f32 - (n as f32 - 1.0) * 0.5) * PITCH;
        centre + Vec2::new(at(i, NX), at(j, NY))
    })
}

/// Where each block's seekers leave from, port block first, in firing order (authoring
/// frame).
#[cfg(test)]
pub(super) fn muzzles(body: &Body) -> Vec<Vec<Vec3>> {
    [1.0, -1.0]
        .iter()
        .map(|&side| {
            let (centre, deck) = block_at(body, side);
            cells(centre)
                .map(|m| m.extend(deck - MUZZLE_DROP))
                .collect()
        })
        .collect()
}

pub(super) fn blocks(b: &mut MeshBuilder, body: &Body) {
    if b.coarse() {
        return;
    }
    for side in [1.0f32, -1.0] {
        block(b, body, side);
    }
}

fn block(b: &mut MeshBuilder, body: &Body, side: f32) {
    let h = half();
    let (centre, deck) = block_at(body, side);
    // The box runs down into the hull at its outer edge.
    let foot = [X - h.x, X, X + h.x]
        .iter()
        .map(|&x| body.point(x, body.phi_up(x, Y + h.y)).z)
        .fold(f32::MIN, f32::max)
        - 1.5;
    let plan = chamfered_rect(h, 0.6);
    b.at(centre.extend(0.0), |b| {
        dark_plate(b);
        b.loft_z(
            &plan,
            &[Section::new(foot, 1.06), Section::new(deck - 0.3, 1.0)],
        );
        seam(b);
        b.loft_z(
            &plan,
            &[Section::new(deck - 0.3, 1.0), Section::new(deck, 0.97)],
        );
    });
    let fine = b.fine();
    let sides = if fine { 6 } else { 4 };
    for m in cells(centre) {
        // The cell's mouth: a dark well inside a lit rim.
        if fine {
            b.paint(GLOW_LASER);
            b.plate(m.extend(deck), Vec2::splat(PITCH * 0.9), 0.05, 0.01);
        }
        seam(b);
        b.plate(m.extend(deck + 0.02), Vec2::splat(PITCH * 0.72), 0.06, 0.02);
        if fine {
            // The seeker waiting in it: a bronze body, its lit head over the well.
            let top = deck + 0.3;
            metal(b);
            b.cylinder_between(m.extend(top - 1.2), m.extend(top - 0.5), 0.42, 0.42, sides);
            b.paint(GLOW_LASER);
            b.cylinder_between(m.extend(top - 0.5), m.extend(top), 0.42, 0.12, sides);
        }
    }
}
