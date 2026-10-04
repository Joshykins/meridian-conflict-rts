//! The +y wing: a faceted delta from the hull's beam, lit along its leading edge, laid in
//! three rows of feathered plates whose last row runs out past the trailing edge into
//! the saw teeth, a graphite spar showing in the gaps between the teeth, armoured cuffs
//! over the leading edge and a blade pod at the tip.
use glam::{Vec2, Vec3};

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{red_slot, Frame};
use super::{drum, shard};
use crate::builder::MeshBuilder;
use crate::material::GLOW_LASER;

/// The chords, root to tip: span (y), leading and trailing edge (x), the chord line's
/// height, and the section's depth over and under it.
const WING: [(f32, f32, f32, f32, f32, f32); 3] = [
    (34.0, 100.0, -132.0, 55.0, 11.0, 9.0),
    (58.0, 30.0, -112.0, 55.0, 6.0, 4.5),
    (80.0, -70.0, -102.0, 55.0, 1.6, 1.2),
];

pub(super) fn build(b: &mut MeshBuilder) {
    let rings: Vec<Vec<Vec3>> = (0..WING.len()).map(chord).collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
    if !b.mid() {
        return;
    }
    // The leading edge, lit from root to tip.
    b.paint(GLOW_LASER);
    for w in rings.windows(2) {
        b.cylinder_between(
            w[0][0] + Vec3::Z * 0.3,
            w[1][0] + Vec3::Z * 0.3,
            0.35,
            0.35,
            4,
        );
    }
    feathers(b);
    cuffs(b);
    spar(b);
    pod(b);
}

/// The section at chord `i`, as a ring in model space.
fn chord(i: usize) -> Vec<Vec3> {
    let (y, le, te, z, over, under) = WING[i];
    let q = le * 0.4 + te * 0.6;
    vec![
        v3(le, y, z),
        v3(q, y, z + over),
        v3(te, y, z + 0.6),
        v3(q, y, z - under),
    ]
}

/// The wing's chord at span fraction `s` (0 root, 1 tip): span, leading and trailing
/// edge.
fn span(s: f32) -> (usize, f32, f32, f32, f32) {
    let (i, k) = if s < 0.5 {
        (0, s * 2.0)
    } else {
        (1, s * 2.0 - 1.0)
    };
    let l = |a: f32, c: f32| a + (c - a) * k;
    let (y0, le0, te0, ..) = WING[i];
    let (y1, le1, te1, ..) = WING[i + 1];
    (i, k, l(y0, y1), l(le0, le1), l(te0, te1))
}

/// The upper surface's height at `x` on chord `i`.
fn over(i: usize, x: f32) -> f32 {
    let (_, le, te, z, over, _) = WING[i];
    let q = le * 0.4 + te * 0.6;
    if x > q {
        z + over * (le - x) / (le - q)
    } else {
        z + over + (0.6 - over) * (q - x) / (q - te)
    }
}

/// The upper surface at `x` at span fraction `s`, and the slope's normal there.
fn top(s: f32, x: f32) -> (Vec3, Vec3) {
    let (i, k, y, ..) = span(s);
    let z = |x: f32| over(i, x) * (1.0 - k) + over(i + 1, x) * k;
    let rise = (z(x - 1.0) - z(x + 1.0)) * 0.5;
    (v3(x, y, z(x)), v3(-rise, 0.0, 1.0).normalize())
}

/// Three rows of plates a chord, lapped aft; the last row's tails are the saw teeth.
fn feathers(b: &mut MeshBuilder) {
    let count = if b.fine() { 8 } else { 5 };
    let rows: &[(f32, f32, f32)] = if b.fine() {
        &[(0.78, 0.3, 0.0), (0.46, 0.3, 0.0), (0.16, 0.2, 1.0)]
    } else {
        &[(0.46, 0.4, 0.0), (0.16, 0.2, 1.0)]
    };
    for j in 0..count {
        let s = (j as f32 + 0.5) / count as f32;
        let (_, _, _, le, te) = span(s);
        let half = (10.5 - 7.0 * s) * 5.0 / count as f32 * 1.6;
        for &(at, len, tail) in rows {
            let x = te + (le - te) * at;
            let (p, n) = top(s, x);
            let f = Frame::new(p + n * 0.2, v3(-1.0, 0.1 + s * 0.25, 0.0), n);
            let tail = tail * (17.0 - 7.0 * s);
            shard(b, &f, (le - te) * len + tail, half, 0.4, 1.1);
        }
        if b.fine() {
            // A red slot where each feather of the last row lifts off the one before.
            let x = te + (le - te) * 0.2;
            let (p, n) = top(s, x);
            red_slot(b, p + n * 0.9, n, Vec3::Y, half * 0.8, 0.35);
        }
    }
}

/// Armoured cuffs over the leading edge, swept back.
fn cuffs(b: &mut MeshBuilder) {
    let count = if b.fine() { 6 } else { 3 };
    for j in 0..count {
        let s = (j as f32 + 0.3) / count as f32;
        let (_, _, _, le, te) = span(s);
        let x = le - (le - te) * 0.04;
        let (p, n) = top(s, x);
        let f = Frame::new(p + n * 0.3, v3(-1.0, 0.2, 0.0), n);
        shard(b, &f, (le - te) * 0.28, 4.5 - 2.5 * s, -0.5, 0.9);
    }
}

/// The spar along the trailing edge, in the gaps between the teeth, and its drums.
fn spar(b: &mut MeshBuilder) {
    let a = v3(WING[0].2 + 6.0, WING[0].0 + 2.0, 55.4);
    let c = v3(WING[2].2 + 4.0, WING[2].0 - 4.0, 55.4);
    metal(b);
    b.cylinder_between(a, c, 1.3, 0.8, b.sides(8));
    for k in [0.25, 0.6] {
        drum(b, a.lerp(c, k), c - a, 2.2, 3.0);
    }
    if b.fine() {
        // The wing's root fairing, where its spar runs into the hull.
        metal(b);
        b.cylinder_between(v3(96.0, 36.0, 55.0), v3(-60.0, 46.0, 62.0), 1.6, 1.6, 6);
    }
}

/// The tip's blade pod: a long plated spindle with a red light at its nose, swept back
/// into a spike.
fn pod(b: &mut MeshBuilder) {
    let (y, le, te) = (WING[2].0, WING[2].1, WING[2].2);
    let nose = v3(le + 14.0, y, 55.0);
    let tail = v3(te - 18.0, y + 1.5, 56.0);
    dark_plate(b);
    b.with_facets(|b| {
        b.beam(
            nose,
            v3(le - 6.0, y, 55.0),
            Vec2::new(0.6, 0.6),
            Vec2::new(4.0, 4.4),
        );
        b.beam(
            v3(le - 6.0, y, 55.0),
            v3(te - 2.0, y + 0.5, 55.4),
            Vec2::new(4.0, 4.4),
            Vec2::new(3.2, 3.6),
        );
        b.beam(
            v3(te - 2.0, y + 0.5, 55.4),
            tail,
            Vec2::new(3.2, 3.6),
            Vec2::new(0.3, 0.3),
        );
    });
    if b.fine() {
        seam(b);
        b.cylinder_between(v3(le - 7.0, y, 55.0), v3(le - 9.0, y, 55.0), 2.4, 2.4, 6);
        b.paint(GLOW_LASER);
        b.cylinder_between(nose - Vec3::X * 3.0, nose - Vec3::X * 4.0, 0.7, 0.7, 6);
        red_slot(
            b,
            v3((le + te) * 0.5, y + 2.05, 55.4),
            Vec3::Y,
            Vec3::X,
            14.0,
            0.5,
        );
    }
}
