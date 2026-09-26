//! The Naga's architecture: an old empire's, built for ceremony. The shapes every Naga
//! building is put together from.
//!
//! - Mass sits on a stepped dais, never straight on the ground.
//! - Walls are long, low and battered; above them rise tall swept blades, curving back
//!   and in, tallest toward the building's seat of honour. Edges are knife edges.
//! - Symmetry about the building's axis, and an axis you walk down: a lit processional
//!   way, a gate of two obelisks at its end.
//! - Crew scale shows: steps up the dais, doors, slit windows lit from inside.
//! - Black plate (`hide`), a shade lighter trim along every step and crown (`metal`), and
//!   red light only in lines: inlaid along edges and ways, or burning in a slot between
//!   two walls. Never veins, never glow washed over a surface.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;

use super::kit::{hide, metal, v3};

/// A swept blade along `spine`: a knife-edged lens in section, `widths` wide across
/// `wide` (made square to the spine at each point) and `thicks` thick square to that.
/// Each list has one value per spine point; the blade ends where its values reach zero.
pub(super) fn blade_sweep(
    b: &mut MeshBuilder,
    spine: &[Vec3],
    widths: &[f32],
    thicks: &[f32],
    wide: Vec3,
) {
    let lens: &[[f32; 2]] = if b.fine() {
        &[
            [1.0, 0.0],
            [0.45, 0.8],
            [-0.45, 0.8],
            [-1.0, 0.0],
            [-0.45, -0.8],
            [0.45, -0.8],
        ]
    } else {
        &[[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]]
    };
    let n = spine.len();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|i| {
            let dir = (spine[(i + 1).min(n - 1)] - spine[i.saturating_sub(1)]).normalize();
            let across = (wide - dir * wide.dot(dir)).normalize();
            let thick = dir.cross(across);
            lens.iter()
                .map(|[s, t]| {
                    let p = spine[i] + across * (s * widths[i]) + thick * (t * thicks[i]);
                    // Nothing goes under the ground.
                    p.max(Vec3::new(p.x, p.y, 0.0))
                })
                .collect()
        })
        .collect();
    b.loft(&rings, true, true);
}

/// The points of the quadratic curve from `a` toward `c` to `d`, `n` steps.
pub(super) fn curve(a: Vec3, c: Vec3, d: Vec3, n: usize) -> Vec<Vec3> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let s = 1.0 - t;
            a * (s * s) + c * (2.0 * s * t) + d * (t * t)
        })
        .collect()
}

/// `n + 1` values running from `from` to `to` along a curve's points.
pub(super) fn taper(from: f32, to: f32, n: usize) -> Vec<f32> {
    (0..=n)
        .map(|i| from + (to - from) * i as f32 / n as f32)
        .collect()
}

/// One step of a dais: a slab from `min` to `max` (x, y) and `z0` to `z1`, its sides
/// battered in by `batter`, a cornice of lighter metal standing proud round its top at
/// full detail.
pub(super) fn step(b: &mut MeshBuilder, min: Vec2, max: Vec2, z0: f32, z1: f32, batter: f32) {
    let size = max - min;
    let centre = ((min + max) * 0.5).extend(z0);
    let top = size - Vec2::splat(batter * 2.0);
    hide(b);
    b.frustum_open(centre, size, top, z1 - z0, Vec2::ZERO);
    if b.fine() {
        let band = 0.3f32.min((z1 - z0) * 0.3);
        metal(b);
        b.frustum_open(
            centre + Vec3::Z * (z1 - z0 - band),
            top + Vec2::splat(0.3),
            top + Vec2::splat(0.2),
            band + 0.02,
            Vec2::ZERO,
        );
    }
}

/// One leg of a pointed arch's centre line, as (y, z): from its springing at `side` times
/// `half` and height `spring`, up to the apex on y = 0 at `apex`, `n` steps. It rises
/// straight off the springing and leans in to meet the other leg at a point.
pub(super) fn ogive_leg(side: f32, half: f32, spring: f32, apex: f32, n: usize) -> Vec<Vec2> {
    curve(
        Vec3::new(side * half, spring, 0.0),
        Vec3::new(side * half, spring + (apex - spring) * 0.72, 0.0),
        Vec3::new(0.0, apex, 0.0),
        n,
    )
    .iter()
    .map(|p| p.truncate())
    .collect()
}

/// A pointed arch's outline as (y, z), springing to springing over the apex, pushed out
/// `grow` metres square to the line (in, where negative).
pub(super) fn ogive_outline(half: f32, spring: f32, apex: f32, n: usize, grow: f32) -> Vec<Vec2> {
    let leg = |side: f32| -> Vec<Vec2> {
        let pts = ogive_leg(side, half, spring, apex, n);
        (0..pts.len())
            .map(|i| {
                let t = (pts[(i + 1).min(n)] - pts[i.saturating_sub(1)]).normalize();
                pts[i] + Vec2::new(-t.y, t.x) * (-side * grow)
            })
            .collect()
    };
    let mut out = leg(-1.0);
    let right = leg(1.0);
    // Both legs end at the apex; the outline keeps one point there, where they meet.
    let (l, r) = (out.pop().unwrap_or_default(), right[n]);
    out.push(Vec2::new(0.0, l.y.max(r.y)));
    out.extend(right.iter().rev().skip(1));
    out
}

/// A band of square section along a curve in the plane `x`: `points` are (y, z), the band
/// `w` wide in that plane and `depth` deep along x. Arches, buttresses.
pub(super) fn band_yz(b: &mut MeshBuilder, x: f32, points: &[Vec2], w: f32, depth: f32) {
    let n = points.len();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|i| {
            let t = (points[(i + 1).min(n - 1)] - points[i.saturating_sub(1)]).normalize();
            let across = Vec3::new(0.0, -t.y, t.x) * (w * 0.5);
            let p = Vec3::new(x, points[i].x, points[i].y);
            let d = Vec3::X * (depth * 0.5);
            vec![
                p + across - d,
                p + across + d,
                p - across + d,
                p - across - d,
            ]
        })
        .collect();
    b.loft(&rings, true, true);
}

/// A flight of `count` steps up to height `top`, rising toward `-dir` from `at` (its foot,
/// in the middle), `width` across. Crew scale: every building has somewhere to walk up.
pub(super) fn stairs(b: &mut MeshBuilder, at: Vec3, dir: Vec2, width: f32, top: f32, count: usize) {
    if !b.fine() {
        return;
    }
    let rise = top / count as f32;
    let tread = rise * 1.6;
    let across = Vec2::new(-dir.y, dir.x) * (width * 0.5);
    let back = -dir;
    hide(b);
    for k in 0..count {
        let near = at.truncate() + back * (tread * k as f32);
        let far = near + back * (tread * (count - k) as f32);
        let z = rise * (k + 1) as f32;
        let corners = [near - across, near + across, far + across, far - across];
        let ring = |z: f32| -> Vec<Vec3> { corners.iter().map(|c| c.extend(z)).collect() };
        b.loft(&[ring(at.z + z - rise), ring(at.z + z)], false, true);
    }
}

/// A line of red light along `a` to `b`, `w` wide and proud of the surface by a little:
/// an inlay. Close up only.
pub(super) fn inlay(b: &mut MeshBuilder, from: Vec3, to: Vec3, w: f32) {
    if !b.fine() {
        return;
    }
    b.paint(GLOW_LASER);
    b.beam(from, to, Vec2::splat(w), Vec2::splat(w));
}

/// A slit window lit from inside, upright, on a face looking along `out` at `at`.
pub(super) fn slit(b: &mut MeshBuilder, at: Vec3, out: Vec3, width: f32, height: f32) {
    if !b.fine() {
        return;
    }
    b.paint(GLOW_LASER);
    let across = Vec3::Z.cross(out).normalize() * (width * 0.5);
    let up = v3(0.0, 0.0, height * 0.5);
    let p = at + out * 0.05;
    b.face(&[
        p - across - up,
        p + across - up,
        p + across + up,
        p - across + up,
    ]);
}
