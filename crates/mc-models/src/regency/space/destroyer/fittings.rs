//! The chine blades and the stern's drive lenses. Every root is sunk into the hull
//! (`Body::point`), so nothing stands off it.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::GLOW_VIOLET;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::hoop_on;
use super::body::Body;

/// Far off, the chine blades as flat triangles (plan x, y; height), port side.
pub(super) const BLADES_FAR: [([[f32; 2]; 3], f32); 1] =
    [([[110.0, 12.0], [-138.0, 44.0], [-100.0, 22.0]], 25.0)];

/// A strake out of the hull at `phi`, from `head` back to `tail`, standing `span(t)` out
/// beyond the surface and drooping `droop`; the last of it swept into a spike.
fn strake(
    b: &mut MeshBuilder,
    body: &Body,
    phi: f32,
    (head, tail): (f32, f32),
    span: impl Fn(f32) -> f32,
    thick: (f32, f32),
    droop: f32,
) {
    let along = if b.fine() { 18 } else { 5 };
    let rings: Vec<Vec<Vec3>> = (0..=along)
        .map(|i| {
            let t = i as f32 / along as f32;
            let x = head + (tail - head) * t;
            let p = body.point(x.max(body.span().0 + 1.0), phi);
            let inner = v3(x, p.y - 2.5, p.z);
            let outer = v3(x, p.y + span(t), p.z - droop * t);
            let th = thick.0 + (thick.1 - thick.0) * t;
            let mid = inner.lerp(outer, 0.55);
            vec![
                inner + Vec3::Z * th,
                mid + Vec3::Z * (th * 0.7),
                outer,
                mid - Vec3::Z * (th * 0.7),
                inner - Vec3::Z * th,
            ]
        })
        .collect();
    b.loft(&rings, true, true);
}

/// C: the chine blades the length of the hull, and a shorter course under them.
pub(super) fn blades(b: &mut MeshBuilder, body: &Body) {
    let (stern, bow) = body.span();
    b.mirror_y(|b| {
        dark_plate(b);
        strake(
            b,
            body,
            0.0,
            (bow - 26.0, stern - 4.0),
            |t| 5.0 + 16.0 * t * t,
            (1.8, 0.6),
            2.6,
        );
        strake(
            b,
            body,
            -34.0,
            (bow - 64.0, stern + 46.0),
            |t| 2.0 + 6.0 * t,
            (0.9, 0.4),
            1.0,
        );
    });
}

/// C: the blunt stern's three drive lenses, each sunk in a bronze hood.
pub(super) fn stern_lenses(b: &mut MeshBuilder, body: &Body) {
    let (stern, _) = body.span();
    let s = body.at(stern);
    let sides = b.sides(14);
    for (y, z) in [
        (0.0, s.chine + 3.4),
        (-4.6, s.chine - 2.2),
        (4.6, s.chine - 2.2),
    ] {
        let c = v3(stern, y, z);
        b.paint(GLOW_VIOLET);
        b.cylinder_between(c + Vec3::X * 0.4, c - Vec3::X * 0.3, 2.3, 2.3, sides);
        metal(b);
        hoop_on(b, c - Vec3::X * 0.6, Vec3::X, 2.9, 0.9, 2.0, sides);
    }
    if b.fine() {
        // A seam-dark plate frame round the cluster.
        seam(b);
        hoop_on(
            b,
            v3(stern - 0.2, 0.0, s.chine),
            Vec3::X,
            8.2,
            1.0,
            0.6,
            sides,
        );
    }
}
