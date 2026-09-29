//! Plates and joints for the Regency's walkers (docs/STYLE.md "The Regency look"), used by the
//! commander, the Artificer and the Outrider: armour plates cut from a flat outline, whose swept-back
//! trailing edges are the spikes, and the bronze joints and rams that show in the gaps
//! between them.

use glam::Vec3;

use crate::builder::MeshBuilder;

use super::kit::{metal, seam};

/// An armour plate: the flat `outline` (any number of points, convex or not) given a
/// thickness along `thick`. A spike is a point of the outline swept back past the rest.
pub(super) fn plate(b: &mut MeshBuilder, outline: &[Vec3], thick: Vec3) {
    b.loft(
        &[
            outline.to_vec(),
            outline.iter().map(|&p| p + thick).collect(),
        ],
        true,
        true,
    );
}

/// A plate lying in the x-z plane: `profile` (x, z) from `y0` out to `y1`.
pub(super) fn side_plate(b: &mut MeshBuilder, profile: &[[f32; 2]], y0: f32, y1: f32) {
    b.extrude_y(profile, y0, y1);
}

/// A joint: a bronze drum `radius` round `at`, `half` either side along its axis, with a
/// dark hub proud of each face up close.
pub(super) fn joint(b: &mut MeshBuilder, at: Vec3, half: Vec3, radius: f32) {
    metal(b);
    let sides = if b.fine() { 10 } else { 6 };
    b.cylinder_between(at - half, at + half, radius, radius, sides);
    if b.fine() {
        seam(b);
        let cap = half.normalize() * (radius * 0.18);
        b.cylinder_between(at + half, at + half + cap, radius * 0.55, radius * 0.4, 8);
        b.cylinder_between(at - half, at - half - cap, radius * 0.55, radius * 0.4, 8);
    }
}

/// A hydraulic ram from `a` (the barrel) to `c` (the rod's eye), all bronze; a big one
/// (`radius` 0.1 or more) has a dark seal where the rod leaves the barrel, up close.
pub(super) fn piston(b: &mut MeshBuilder, a: Vec3, c: Vec3, radius: f32) {
    if b.coarse() {
        return;
    }
    let mid = a.lerp(c, 0.55);
    let big = radius >= 0.1;
    let sides = b.sides(if big { 8 } else { 5 });
    metal(b);
    b.cylinder_between(a, mid, radius, radius, sides);
    b.cylinder_between(mid, c, radius * 0.5, radius * 0.5, sides.min(6));
    if big && b.fine() {
        seam(b);
        let d = (c - a).normalize() * (radius * 0.5);
        b.cylinder_between(mid - d, mid + d, radius * 1.2, radius * 1.2, 8);
    }
}
