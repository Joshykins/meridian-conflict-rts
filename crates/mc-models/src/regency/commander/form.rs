//! The Exarch's forms, shared by the Regency's walkers: faceted solids lofted through
//! cross-sections (`sleeve`), thick bevelled blades (`blade`), and the profiles they are cut to.

use glam::Vec3;

use crate::builder::MeshBuilder;

use super::super::machine::{armour, swept, Frame};
use super::super::plating::piston;

/// One cross-section of a sleeve: centred on `c`, `w` either side across and `h` either
/// side along `out` (the way its profile's `t` points; across is `along x out`).
#[derive(Clone, Copy)]
pub(in crate::regency) struct Ring {
    c: Vec3,
    out: Vec3,
    w: f32,
    h: f32,
}

pub(in crate::regency) fn ring(c: Vec3, out: Vec3, w: f32, h: f32) -> Ring {
    Ring { c, out, w, h }
}

/// An octagon with long flat faces: the plain armoured section.
pub(in crate::regency) const OCT: [[f32; 2]; 8] = [
    [1.0, 0.42],
    [0.58, 1.0],
    [-0.58, 1.0],
    [-1.0, 0.42],
    [-1.0, -0.42],
    [-0.58, -1.0],
    [0.58, -1.0],
    [1.0, -0.42],
];

/// A section with a ridge down its `out` face and a flat back: a keeled plate or talon.
pub(in crate::regency) const KEEL: [[f32; 2]; 7] = [
    [1.0, 0.1],
    [0.55, 0.72],
    [0.0, 1.0],
    [-0.55, 0.72],
    [-1.0, 0.1],
    [-0.7, -0.8],
    [0.7, -0.8],
];

/// A thick arch over the top of `out`, open underneath: a pauldron or a helmet.
pub(in crate::regency) const ARCH: [[f32; 2]; 10] = [
    [1.0, -0.35],
    [0.95, 0.35],
    [0.55, 0.9],
    [-0.1, 1.0],
    [-0.75, 0.6],
    [-0.62, 0.42],
    [-0.05, 0.72],
    [0.4, 0.62],
    [0.66, 0.24],
    [0.7, -0.35],
];

/// A solid lofted through `rings` in order, each cut to `profile`, capped both ends.
/// Below full detail an octagon or an arch is cut with every other point, a keel with
/// five.
pub(in crate::regency) fn sleeve(b: &mut MeshBuilder, rings: &[Ring], profile: &[[f32; 2]]) {
    let coarse: Vec<[f32; 2]>;
    let profile = if !b.fine() && profile.len() >= 8 {
        coarse = profile.iter().copied().step_by(2).collect();
        &coarse[..]
    } else if !b.fine() && profile.len() == KEEL.len() {
        coarse = [0, 2, 4, 5, 6].iter().map(|&i| profile[i]).collect();
        &coarse[..]
    } else {
        profile
    };
    let n = rings.len();
    let loops: Vec<Vec<Vec3>> = (0..n)
        .map(|i| {
            let r = rings[i];
            let along = (rings[(i + 1).min(n - 1)].c - rings[i.saturating_sub(1)].c).normalize();
            let side = along.cross(r.out).normalize();
            let out = side.cross(along).normalize();
            profile
                .iter()
                .map(|&[s, t]| r.c + side * (s * r.w) + out * (t * r.h))
                .collect()
        })
        .collect();
    // Armour: flat planes meeting at clear edges, never smoothed into a curve.
    b.with_facets(|b| b.loft(&loops, true, true));
}

/// A thick blade: from its leading edge at `o` (`half` either side across) along `u` for
/// `len` to a point at `tip` across (-1 to 1), its face turned toward `n`, `thick` deep,
/// its edges bevelled up close. A small one is drawn at full detail only.
#[expect(
    clippy::too_many_arguments,
    reason = "a blade is placed by all of these"
)]
pub(in crate::regency) fn blade(
    b: &mut MeshBuilder,
    o: Vec3,
    u: Vec3,
    n: Vec3,
    len: f32,
    half: f32,
    tip: f32,
    thick: f32,
) {
    // Below full detail only the blades that shape the outline are drawn.
    if !b.fine() && len * half < 1.6 {
        return;
    }
    armour(b, &Frame::new(o, u, n), &swept(len, half, tip, 0.45), thick);
}

/// A bronze ball joint of radius `r` at `at`, coarser below full detail.
pub(in crate::regency) fn ball(b: &mut MeshBuilder, at: Vec3, r: f32) {
    let (sides, rings) = if b.fine() { (10, 5) } else { (6, 3) };
    b.spheroid(at, Vec3::splat(r), sides, rings);
}

/// A ram from `a` to `c` (`plating::piston`), at full detail only: it lives in the gaps.
pub(in crate::regency) fn ram(b: &mut MeshBuilder, a: Vec3, c: Vec3, r: f32) {
    if b.fine() {
        piston(b, a, c, r);
    }
}
