//! The chine blades and the stern's drives. Every root is sunk into the hull
//! (`Body::point`), so nothing stands off it.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::GLOW_VIOLET;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::hoop_on;
use super::body::Body;

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

/// The chine blades from the head back to `tail`, flaring `flare` out at their tips, and
/// the shorter course under them.
pub(super) fn chine_blades(b: &mut MeshBuilder, body: &Body, tail: f32, flare: f32) {
    let (stern, bow) = body.span();
    b.mirror_y(|b| {
        dark_plate(b);
        strake(
            b,
            body,
            0.0,
            (bow - 26.0, tail),
            |t| 5.0 + flare * t * t,
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

/// The stern's drives (authoring frame): each mouth's middle and radius, a great plasma
/// drive on the centreline over two lesser ones.
pub(super) const DRIVES: [([f32; 3], f32); 3] = [
    ([-134.0, 0.0, 28.2], 5.2),
    ([-133.0, -8.2, 24.4], 2.9),
    ([-133.0, 8.2, 24.4], 2.9),
];

/// The stern's drives: each a violet throat deep in a plated cowl, a bronze hood round its
/// lip; hot air shimmers off them (`add_exhaust`).
pub(super) fn drives(b: &mut MeshBuilder) {
    let sides = b.sides(20);
    for (c, r) in DRIVES {
        let c = Vec3::from(c);
        // The cowl: a plated sleeve from inside the hull out to the lip.
        dark_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(c + Vec3::X * 9.0, c, r * 1.3, r * 1.18, 10);
        });
        metal(b);
        hoop_on(
            b,
            c - Vec3::X * 0.2,
            Vec3::X,
            r * 1.12,
            r * 0.22,
            0.9,
            sides,
        );
        // The throat, sunk a little behind the lip.
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            c + Vec3::X * 1.6,
            c + Vec3::X * 0.6,
            r * 0.92,
            r * 0.92,
            sides,
        );
        if b.fine() {
            seam(b);
            hoop_on(
                b,
                c + Vec3::X * 0.4,
                Vec3::X,
                r * 0.97,
                r * 0.08,
                0.6,
                sides,
            );
        }
        b.add_exhaust(c, -Vec3::X, r);
    }
}
