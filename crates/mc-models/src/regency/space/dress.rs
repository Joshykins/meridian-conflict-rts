//! The dressing every faceted Regency warship hull shares: plate courses, the pale
//! second plate, blades out of the chine, stern drives, belly lifts and the owner's mark.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::{GLOW_VIOLET, PLATING, TEAM};
use crate::pattern;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::hoop_on;
use super::body::{feather, Body, CHINE};

/// The second plate: a paler steel under the same film (regency.wgsl `REG_FILM_PALE`),
/// laid in bands against the charcoal for a two-tone hull.
pub(super) fn pale_plate(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::EMBER);
}

/// A course of `count` plates staggered from `from` (forward) back to `to`, each a
/// feather over `band` (`u` low, high and its spike's tip), `thick` deep.
pub(super) fn course(
    b: &mut MeshBuilder,
    body: &Body,
    (from, to): (f32, f32),
    count: usize,
    band: (f32, f32, f32),
    thick: f32,
) {
    let step = (from - to) / count as f32;
    let over = step * 0.22;
    for k in 0..count {
        let head = from - step * k as f32;
        feather(
            b,
            body,
            (head, head - step - over),
            band,
            thick,
            thick * 0.5,
            0.42,
        );
    }
}

/// A blade out of the hull's chine from `head` back to `tail`, reaching `span(t)` out
/// beyond the chine and drooping `droop` by its tail: a hard, flat-faceted lozenge.
pub(super) fn strake(
    b: &mut MeshBuilder,
    body: &Body,
    (head, tail): (f32, f32),
    span: impl Fn(f32) -> f32,
    thick: (f32, f32),
    droop: f32,
) {
    let along = if b.fine() { 12 } else { 4 };
    let rings: Vec<Vec<Vec3>> = (0..=along)
        .map(|i| {
            let t = i as f32 / along as f32;
            let x = head + (tail - head) * t;
            let p = body.point(x.max(body.span().0 + 1.0), CHINE);
            let inner = p - Vec3::Y * 4.0;
            let outer = v3(x, p.y + span(t), p.z - droop * t);
            let th = thick.0 + (thick.1 - thick.0) * t;
            let mid = inner.lerp(outer, 0.4);
            vec![
                inner + Vec3::Z * th,
                mid + Vec3::Z * (th * 0.8),
                outer,
                mid - Vec3::Z * (th * 0.5),
                inner - Vec3::Z * th,
            ]
        })
        .collect();
    b.with_facets(|b| b.loft(&rings, true, true));
}

/// The stern's drives: each `(mouth's middle, radius)` a violet throat deep in a
/// faceted cowl `cowl` long, a graphite hood round its lip; hot air shimmers off them.
pub(super) fn drives(b: &mut MeshBuilder, drives: &[([f32; 3], f32)], cowl: f32) {
    let sides = b.sides(20);
    for &(c, r) in drives {
        let c = Vec3::from(c);
        dark_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(c + Vec3::X * cowl, c, r * 1.32, r * 1.2, 8);
        });
        metal(b);
        hoop_on(
            b,
            c - Vec3::X * 0.5,
            Vec3::X,
            r * 1.12,
            r * 0.22,
            2.2,
            sides,
        );
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            c + Vec3::X * 4.0,
            c + Vec3::X * 1.5,
            r * 0.92,
            r * 0.92,
            sides,
        );
        if b.fine() {
            seam(b);
            hoop_on(
                b,
                c + Vec3::X * 1.0,
                Vec3::X,
                r * 0.97,
                r * 0.08,
                1.5,
                sides,
            );
        }
        b.add_exhaust(c, -Vec3::X, r);
    }
}

/// The lift plates under the belly: gravity lenses `r` across set flush in faceted
/// rings.
pub(super) fn lifts(b: &mut MeshBuilder, body: &Body, at: &[(f32, f32)], r: f32) {
    for &(x, y) in at {
        let u = body.u_down(x, y);
        let n = body.normal(x, u);
        let p = body.point(x, u);
        let sides = b.sides(16);
        dark_plate(b);
        b.with_facets(|b| b.cylinder_between(p - n * 1.5, p + n * 1.8, r * 1.47, r * 1.37, 8));
        b.paint(GLOW_VIOLET);
        let lens = p + n * 2.2;
        b.cylinder_between(p + n * 1.8, lens, r, r, sides);
        b.add_lift(lens, r);
    }
}

/// The owner's colour: a chevron on the deck at `at`, `size` long.
pub(super) fn mark(b: &mut MeshBuilder, at: Vec3, size: f32) {
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            at,
            at + v3(-size, size * 0.42, 0.0),
            at + v3(-size * 1.3, size * 0.2, 0.0),
        ])
    });
}
