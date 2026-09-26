//! The Naga's shape vocabulary, shared by every Naga model: the paints of their hide, and
//! the pieces grown war-machines are put together from (hide segments, arched armour
//! plates, raked blades and spikes, joint drums, hydraulic rams, cables, flat slabs).
//!
//! The look (see `mod.rs`): black hide plates, soft hide between them lit red at its seams,
//! bare metal at every joint, and visible construction: separate parts with gaps between
//! them, not smooth sleeves.

use glam::Vec3;

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::pattern;

pub(super) fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// A cross-section of hide: a flat belly, flanks, and a keel along the top (`up`).
pub(super) fn chitin_ring(b: &MeshBuilder, c: Vec3, side: Vec3, up: Vec3, w: f32, h: f32) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() {
        &[[1.0, -0.25], [0.62, -1.0], [-0.62, -1.0], [-1.0, -0.25], [-0.78, 0.5], [-0.3, 0.86], [0.0, 1.0], [0.3, 0.86], [0.78, 0.5]]
    } else {
        &[[1.0, -0.4], [0.0, -1.0], [-1.0, -0.4], [-0.55, 0.7], [0.55, 0.7]]
    };
    shape.iter().map(|[s, u]| c + side * (s * w) + up * (u * h)).collect()
}

/// A cross-section of an armour plate: an arch over the top whose rim hangs past the
/// hide under it, hollowed underneath.
pub(super) fn shell_ring(b: &MeshBuilder, c: Vec3, side: Vec3, up: Vec3, w: f32, h: f32) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() {
        &[[1.0, -0.62], [0.74, -0.22], [-0.74, -0.22], [-1.0, -0.62], [-0.95, 0.05], [-0.62, 0.68], [0.0, 1.0], [0.62, 0.68], [0.95, 0.05]]
    } else {
        &[[1.0, -0.62], [-1.0, -0.62], [-0.7, 0.62], [0.0, 1.0], [0.7, 0.62]]
    };
    shape.iter().map(|[s, u]| c + side * (s * w) + up * (u * h)).collect()
}

/// Across and up for a limb running `dir`, its keel turned toward `hint`.
pub(super) fn frame(dir: Vec3, hint: Vec3) -> (Vec3, Vec3) {
    let dir = dir.normalize();
    let up = (hint - dir * hint.dot(dir)).try_normalize().unwrap_or(Vec3::Z);
    (up.cross(dir).normalize(), up)
}

pub(super) fn rings(
    b: &MeshBuilder,
    points: &[(Vec3, f32, f32)],
    hint: Vec3,
    ring: fn(&MeshBuilder, Vec3, Vec3, Vec3, f32, f32) -> Vec<Vec3>,
) -> Vec<Vec<Vec3>> {
    points
        .iter()
        .enumerate()
        .map(|(i, &(c, w, h))| {
            let dir = if i + 1 < points.len() { points[i + 1].0 - c } else { c - points[i - 1].0 };
            let (side, up) = frame(dir, hint);
            ring(b, c, side, up, w, h)
        })
        .collect()
}

/// A run of hide from joint to joint, each point with its half width and half height.
pub(super) fn segment(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3) {
    let r = rings(b, points, hint, chitin_ring);
    b.loft(&r, true, true);
}

/// An arched armour plate along `points` (half width, height of the arch).
pub(super) fn shell(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3) {
    let r = rings(b, points, hint, shell_ring);
    b.loft(&r, true, true);
}

/// A raked blade standing off the hide at `root`, pointing along `to`, flat across `flat`.
pub(super) fn blade(b: &mut MeshBuilder, root: Vec3, to: Vec3, width: f32, flat: Vec3) {
    let dir = to - root;
    let (side, up) = frame(dir, flat);
    let base: Vec<Vec3> = vec![
        root + side * width,
        root + up * (width * 0.35) - dir * 0.12,
        root - side * width,
        root - up * (width * 0.35) + dir * 0.08,
    ];
    b.loft(&[base, vec![to; 4]], true, false);
}

/// A raked spike standing off the hide at `root`, pointing along `to`.
pub(super) fn spike(b: &mut MeshBuilder, root: Vec3, to: Vec3, width: f32) {
    let dir = to - root;
    let (side, up) = frame(dir, Vec3::Z);
    let base: Vec<Vec3> = vec![
        root + side * width,
        root + up * (width * 0.9) - dir * 0.18,
        root - side * width,
        root - up * (width * 0.5) + dir * 0.1,
    ];
    b.loft(&[base, vec![to; 4]], true, false);
}

/// A drum across a joint: the bare knuckle a limb turns on.
pub(super) fn knuckle(b: &mut MeshBuilder, at: Vec3, axis: Vec3, radius: f32, width: f32) {
    if b.coarse() {
        return;
    }
    let half = axis.normalize() * (width * 0.5);
    b.paint(ACCENT).pattern(pattern::EMBER);
    let sides = if b.fine() { 8 } else { 5 };
    b.cylinder_between(at - half, at + half, radius, radius, sides);
    if b.fine() {
        // Its hub: bare metal caps.
        metal(b);
        b.cylinder_between(at + half, at + half * 1.15, radius * 0.5, radius * 0.38, 6);
        b.cylinder_between(at - half, at - half * 1.15, radius * 0.5, radius * 0.38, 6);
    }
}

/// A hydraulic ram from `a` (the cylinder) to `c` (the rod's eye). Close up only.
pub(super) fn ram(b: &mut MeshBuilder, a: Vec3, c: Vec3, radius: f32) {
    if !b.fine() {
        return;
    }
    let mid = a.lerp(c, 0.55);
    b.paint(ACCENT);
    b.cylinder_between(a, mid, radius, radius, 6);
    b.paint(METAL).pattern(pattern::EMBER);
    b.cylinder_between(mid, c, radius * 0.55, radius * 0.55, 6);
}

/// Carapace plate: `PLATING_DARK` marked as Naga hide (`pattern::EMBER`), which keeps the
/// field grime off it (grown hide does not gather dirt the way paint does).
pub(super) fn hide(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::EMBER);
}

/// The soft hide between the plates, its seams lit red.
pub(super) fn under_hide(b: &mut MeshBuilder) {
    b.paint(ACCENT).pattern(pattern::EMBER);
}

/// Bare working metal: rams, cables, vertebrae. It reads a shade lighter than the plates.
pub(super) fn metal(b: &mut MeshBuilder) {
    b.paint(METAL).pattern(pattern::EMBER);
}

/// A flat plate: the quad `outline` given a thickness along `thick`.
pub(super) fn slab(b: &mut MeshBuilder, outline: [Vec3; 4], thick: Vec3) {
    b.loft(&[outline.to_vec(), outline.iter().map(|&p| p + thick).collect()], true, true);
}

/// A cable run through `points`, a tube of `radius`.
pub(super) fn cable(b: &mut MeshBuilder, points: &[Vec3], radius: f32) {
    for w in points.windows(2) {
        b.cylinder_between(w[0], w[1], radius, radius, 5);
    }
}
