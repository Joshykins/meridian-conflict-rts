//! The pieces the Regency's heavy machines (the Saker, the Springald) are put together
//! from, beyond the shared kit: faceted plates gathered to a ridge, joint drums with their
//! hubs, bays where the plates part over the machinery, and faces laid in a plate with its
//! light or finish drawn by the surface.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::{hoop_on, Frame};

/// A faceted armour plate: `outline` (points `[s, t]` in `f`) `thick` deep out of its
/// face, its top drawn in and gathered to a ridge at `ridge` (`[s, t]`) `rise` above it,
/// so each facet catches the light its own way. With the current brush.
pub(super) fn ridged(
    b: &mut MeshBuilder,
    f: &Frame,
    outline: &[[f32; 2]],
    thick: f32,
    ridge: [f32; 2],
    rise: f32,
) {
    let n = outline.len() as f32;
    let c = outline
        .iter()
        .fold(Vec2::ZERO, |a, &[s, t]| a + Vec2::new(s, t))
        / n;
    let lower: Vec<Vec3> = outline.iter().map(|&[s, t]| f.at(s, t, 0.0)).collect();
    let upper: Vec<Vec3> = outline
        .iter()
        .map(|&[s, t]| {
            let q = c + (Vec2::new(s, t) - c) * 0.9;
            f.at(q.x, q.y, thick)
        })
        .collect();
    let top = f.at(ridge[0], ridge[1], thick + rise);
    b.with_facets(|b| {
        b.loft(&[lower, upper.clone()], b.fine(), false);
        for k in 0..upper.len() {
            face_out(b, &[upper[k], upper[(k + 1) % upper.len()], top], f.n);
        }
    });
}

/// A polygon wound so its front faces `out`.
pub(super) fn face_out(b: &mut MeshBuilder, points: &[Vec3], out: Vec3) {
    let n = (points[1] - points[0]).cross(points[2] - points[0]);
    if n.dot(out) < 0.0 {
        let rev: Vec<Vec3> = points.iter().rev().copied().collect();
        b.face(&rev);
    } else {
        b.face(points);
    }
}

/// A face laid just proud of a surface whose outward normal is `out`, drawn as `look`
/// (a Regency plate pattern: its red slot, its gasket or machined finish).
pub(super) fn inlay(b: &mut MeshBuilder, corners: &[Vec3], out: Vec3, look: u32) {
    let out = out.normalize();
    let points: Vec<Vec3> = corners.iter().map(|&p| p + out * 0.03).collect();
    dark_plate(b);
    b.pattern(look);
    face_out(b, &points, out);
}

/// A joint drum `width` long about `axis` at `at`, `r` round: a seam-dark drum, a
/// graphite race and hub at each end, a plated cap on the hub.
pub(super) fn drum(b: &mut MeshBuilder, at: Vec3, axis: Vec3, r: f32, width: f32) {
    let axis = axis.normalize();
    let half = axis * (width * 0.5);
    let sides = b.sides(10);
    seam(b);
    b.cylinder_between(at - half, at + half, r, r, sides);
    if !b.mid() {
        return;
    }
    for sign in [-1.0f32, 1.0] {
        let cap = at + half * sign;
        let d = axis * sign;
        metal(b);
        hoop_on(b, cap + d * 0.05, d, r * 0.84, r * 0.2, 0.3, sides);
        b.cylinder_between(cap, cap + d * 0.4, r * 0.6, r * 0.52, sides);
        if b.fine() {
            dark_plate(b);
            b.cylinder_between(cap + d * 0.4, cap + d * 0.55, r * 0.32, r * 0.3, 6);
        }
    }
}

/// A bay where the plates part over the machinery, in `f` (`u` along it, `n` out of the
/// surface): a seam-dark floor `len` long and `half` either side, two plain graphite
/// shafts down it on cross struts, and a faceted lip plate either side standing over it.
pub(super) fn bay(b: &mut MeshBuilder, f: &Frame, len: f32, half: f32) {
    seam(b);
    b.loft(
        &[
            vec![
                f.at(0.0, -half, 0.0),
                f.at(len, -half, 0.0),
                f.at(len, half, 0.0),
                f.at(0.0, half, 0.0),
            ],
            vec![
                f.at(0.0, -half, 0.06),
                f.at(len, -half, 0.06),
                f.at(len, half, 0.06),
                f.at(0.0, half, 0.06),
            ],
        ],
        false,
        true,
    );
    if !b.fine() {
        return;
    }
    metal(b);
    let r = (half * 0.2).min(0.22);
    for t in [-0.45f32, 0.45] {
        b.cylinder_between(
            f.at(0.15, t * half, r + 0.08),
            f.at(len - 0.15, t * half, r + 0.08),
            r,
            r,
            6,
        );
    }
    for s in [0.5f32, len - 0.5] {
        b.beam(
            f.at(s, -half * 0.8, r + 0.08),
            f.at(s, half * 0.8, r + 0.08),
            Vec2::splat(r * 1.6),
            Vec2::splat(r * 1.6),
        );
    }
    dark_plate(b);
    for side in [-1.0f32, 1.0] {
        let t0 = side * half;
        let t1 = side * (half + 0.55);
        ridged(
            b,
            f,
            &[[-0.2, t0], [len + 0.2, t0], [len + 0.5, t1], [-0.5, t1]],
            0.3,
            [len * 0.5, side * (half + 0.2)],
            0.18,
        );
    }
}

/// A plated locker on a face: a chamfered box `size` (along `u`, across `v`, out `n`)
/// its lid machined.
pub(super) fn locker(b: &mut MeshBuilder, f: &Frame, size: Vec3) {
    let (l, w, h) = (size.x, size.y * 0.5, size.z);
    let ring = |z: f32, k: f32| -> Vec<Vec3> {
        [[0.0, -w], [l, -w], [l, w], [0.0, w]]
            .iter()
            .map(|&[s, t]| {
                let q = Vec2::new(l * 0.5, 0.0) + (Vec2::new(s, t) - Vec2::new(l * 0.5, 0.0)) * k;
                f.at(q.x, q.y, z)
            })
            .collect()
    };
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[ring(0.0, 1.0), ring(h * 0.75, 1.0), ring(h, 0.86)],
            false,
            true,
        )
    });
    if b.fine() {
        let c = ring(h, 0.72);
        inlay(b, &c, f.n, crate::pattern::EMBER_MACHINED);
    }
}

/// The full outline from `half` (y >= 0, front round to back), mirrored across.
fn outline(half: &[[f32; 2]]) -> Vec<[f32; 2]> {
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

/// A solid in plan: `half` the outline lofted through `levels` (height, and how far the
/// outline is drawn in about its middle there), with the current brush.
pub(super) fn plan_loft(
    b: &mut MeshBuilder,
    half: &[[f32; 2]],
    levels: &[(f32, f32)],
    bottom: bool,
) {
    let out = outline(half);
    let (lo, hi) = out.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
        (lo.min(p[0]), hi.max(p[0]))
    });
    let mid = (lo + hi) * 0.5;
    let rings: Vec<Vec<Vec3>> = levels
        .iter()
        .map(|&(z, k)| {
            out.iter()
                .map(|&[x, y]| v3(mid + (x - mid) * k, y * k, z))
                .collect()
        })
        .collect();
    b.with_facets(|b| b.loft(&rings, bottom, true));
}
