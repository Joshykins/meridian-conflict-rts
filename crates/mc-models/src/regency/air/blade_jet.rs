//! The pieces the Regency's tech 3 jets are put together from (docs/STYLE.md "The Regency
//! look", "Aircraft"): Megatron's Cybertronian jet bent to the Regency's rules, symmetrical.
//!
//! - A faceted hull lofted through hard-chined stations (`hull`), never a round tube.
//! - A nose blade (`blade`): a long flat knife ahead of the nose, keeled top and bottom.
//! - Wings and fins as thick flat slabs (`slab`); the fins turned down, not up.
//! - Swept-back armour plates laid over the hull and the wings, each lapping the next, their
//!   trailing edges the spikes (`lap`, `feathers`); dark bronze machinery in the gaps
//!   between them (`workings`).
//! - Red optics (`optics`) and red heat in the nozzles (`nozzle`), never ARC's orange.
//! - The owner's colour as a chevron on the back (`chevron`).

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::machine::collar;

/// One cross-section of a hull at `x`, mirrored about y = 0: the keel's z, then (z, half
/// width) at the chine and at the shoulder, and the spine's z. The chine is the widest.
#[derive(Clone, Copy)]
pub(super) struct Station {
    pub(super) x: f32,
    pub(super) keel: f32,
    pub(super) chine: [f32; 2],
    pub(super) shoulder: [f32; 2],
    pub(super) spine: f32,
}

pub(super) const fn st(
    x: f32,
    keel: f32,
    chine: [f32; 2],
    shoulder: [f32; 2],
    spine: f32,
) -> Station {
    Station {
        x,
        keel,
        chine,
        shoulder,
        spine,
    }
}

/// The hull lofted tail first through `stations` to the `nose`'s point, in dark plate. The
/// coarse level keeps the `coarse` stations as a diamond: keel, chines, spine.
pub(super) fn hull(b: &mut MeshBuilder, stations: &[Station], nose: Vec3, coarse: &[usize]) {
    let ring = |s: &Station, simple: bool| -> Vec<Vec3> {
        let p = |y: f32, z: f32| v3(s.x, y, z);
        if simple {
            return vec![
                p(0.0, s.keel),
                p(-s.chine[1], s.chine[0]),
                p(0.0, s.spine),
                p(s.chine[1], s.chine[0]),
            ];
        }
        vec![
            p(0.0, s.keel),
            p(-s.chine[1], s.chine[0]),
            p(-s.shoulder[1], s.shoulder[0]),
            p(0.0, s.spine),
            p(s.shoulder[1], s.shoulder[0]),
            p(s.chine[1], s.chine[0]),
        ]
    };
    let mut rings: Vec<Vec<Vec3>> = if b.coarse() {
        coarse.iter().map(|&i| ring(&stations[i], true)).collect()
    } else {
        stations.iter().map(|s| ring(s, false)).collect()
    };
    let n = rings[0].len();
    rings.push(vec![nose; n]);
    dark_plate(b);
    b.with_profile_bevel(0.05, |b| b.with_facets(|b| b.loft(&rings, true, false)));
}

/// The two stations either side of `x` and how far between them.
fn between(stations: &[Station], x: f32) -> (Station, Station, f32) {
    let i = stations
        .windows(2)
        .position(|w| x <= w[1].x)
        .unwrap_or(stations.len() - 2);
    let (a, c) = (stations[i], stations[i + 1]);
    (a, c, ((x - a.x) / (c.x - a.x)).clamp(0.0, 1.0))
}

/// The hull's top at (`x`, `y`): down from the spine to the shoulder, then to the chine,
/// flat beyond it.
pub(super) fn surface(stations: &[Station], x: f32, y: f32) -> f32 {
    let (a, c, t) = between(stations, x);
    let l = |p: f32, q: f32| p + (q - p) * t;
    let spine = l(a.spine, c.spine);
    let (sz, sy) = (
        l(a.shoulder[0], c.shoulder[0]),
        l(a.shoulder[1], c.shoulder[1]),
    );
    let (cz, cy) = (l(a.chine[0], c.chine[0]), l(a.chine[1], c.chine[1]));
    let y = y.abs();
    if y <= sy {
        spine + (sz - spine) * (y / sy.max(1e-3))
    } else if y <= cy {
        sz + (cz - sz) * ((y - sy) / (cy - sy).max(1e-3))
    } else {
        cz
    }
}

/// The hull's half width at `x`, out to the chine.
pub(super) fn half_width(stations: &[Station], x: f32) -> f32 {
    let (a, c, t) = between(stations, x);
    a.chine[1] + (c.chine[1] - a.chine[1]) * t
}

/// Pulls each point of `ring` in toward its middle by up to `inset`.
fn inset(ring: &[Vec3], inset: f32) -> Vec<Vec3> {
    let mid = ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
    ring.iter()
        .map(|&p| {
            let to = mid - p;
            p + to.normalize_or_zero() * to.length().min(inset)
        })
        .collect()
}

/// A thick flat plate: the polygon `outline` given a thickness along `thick`, its far face
/// bevelled in up close when `bevel`.
pub(super) fn slab(b: &mut MeshBuilder, outline: &[Vec3], thick: Vec3, bevel: bool) {
    let base = outline.to_vec();
    let far: Vec<Vec3> = base.iter().map(|&p| p + thick).collect();
    if bevel && b.fine() {
        let mid: Vec<Vec3> = base.iter().map(|&p| p + thick * 0.6).collect();
        let top = inset(&far, thick.length() * 0.6);
        b.with_facets(|b| b.loft(&[base, mid, top], true, true));
    } else {
        b.with_facets(|b| b.loft(&[base, far], true, true));
    }
}

/// A flat wing or plane in plan: `outline` (x, y) at height `z`, `thick` deep, left side
/// (the caller mirrors it). Dark plate.
pub(super) fn wing(b: &mut MeshBuilder, outline: &[[f32; 2]], z: f32, thick: f32) {
    let points: Vec<Vec3> = outline.iter().map(|&[x, y]| v3(x, y, z)).collect();
    dark_plate(b);
    b.with_bevel(0.04, |b| slab(b, &points, Vec3::Z * thick, false));
}

/// A red pin line let into a plate's face from `a` to `c`. Close up only.
pub(super) fn pin_line(b: &mut MeshBuilder, a: Vec3, c: Vec3) {
    if !b.fine() {
        return;
    }
    b.paint(GLOW_LASER);
    b.beam(a, c, Vec2::new(0.05, 0.03), Vec2::new(0.05, 0.03));
}

/// An intake under a chine: a seam-dark mouth from `a` back to `c`, `size` across and
/// deep, a red line along its lip. Close up only.
pub(super) fn intake(b: &mut MeshBuilder, a: Vec3, c: Vec3, size: Vec2) {
    if !b.fine() {
        return;
    }
    seam(b);
    b.beam(a, c, size, size * 1.05);
    pin_line(
        b,
        a + Vec3::Z * (size.y * 0.5),
        a.lerp(c, 0.6) + Vec3::Z * (size.y * 0.5),
    );
}

/// Swept plates laid flat over a wing at `z`, each outline (x, y) lifted `step` over the
/// last so its tail laps the head of the next, bevelled up close. Dark plate.
pub(super) fn feathers(b: &mut MeshBuilder, plates: &[&[[f32; 2]]], z: f32, thick: f32, step: f32) {
    dark_plate(b);
    for (k, outline) in plates.iter().enumerate() {
        let lift = z + step * k as f32;
        let points: Vec<Vec3> = outline.iter().map(|&[x, y]| v3(x, y, lift)).collect();
        slab(b, &points, Vec3::Z * thick, true);
    }
}

/// An armour plate laid on the hull through the plan `outline` (x, y, lift), `lift` over
/// the hull's top at each point, `thick` deep. Dark plate.
pub(super) fn lap(b: &mut MeshBuilder, stations: &[Station], outline: &[[f32; 3]], thick: f32) {
    let base: Vec<Vec3> = outline
        .iter()
        .map(|&[x, y, lift]| v3(x, y, surface(stations, x, y) + lift))
        .collect();
    dark_plate(b);
    slab(b, &base, Vec3::Z * thick, true);
}

/// [`lap`] either side of the middle, `outline` the left one.
pub(super) fn lap_pair(
    b: &mut MeshBuilder,
    stations: &[Station],
    outline: &[[f32; 3]],
    thick: f32,
) {
    lap(b, stations, outline, thick);
    let right: Vec<[f32; 3]> = outline.iter().rev().map(|&[x, y, l]| [x, -y, l]).collect();
    lap(b, stations, &right, thick);
}

/// The nose blade: a flat knife from `root` forward to `tip`, `half` either side at its
/// root and `thick` deep, keeled top and bottom, in dark plate with a bronze collar where
/// it leaves the nose.
pub(super) fn blade(b: &mut MeshBuilder, root: Vec3, tip: Vec3, half: f32, thick: f32) {
    let along = (tip - root).normalize();
    let side = Vec3::Z.cross(along).normalize();
    let up = along.cross(side).normalize();
    let ring = |c: Vec3, w: f32, h: f32| -> Vec<Vec3> {
        vec![c - up * h, c - side * w, c + up * h, c + side * w]
    };
    let shoulder = root.lerp(tip, 0.55);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                ring(root, half, thick * 0.5),
                ring(shoulder, half * 0.8, thick * 0.42),
                vec![tip; 4],
            ],
            true,
            false,
        )
    });
    if b.fine() {
        collar(b, root + along * 0.1, along, thick * 0.75, 0.3);
    }
}

/// A down-turned fin: a plate hung from `root` (its leading point) on the airframe, swept
/// back `chord` and turned down and out by `droop` (0 straight down, 1 flat out), reaching
/// `span`; its tip chord is `taper` of the root's.
#[expect(clippy::too_many_arguments, reason = "a fin is placed by all of these")]
pub(super) fn fin(
    b: &mut MeshBuilder,
    root: Vec3,
    chord: f32,
    span: f32,
    droop: f32,
    sweep: f32,
    taper: f32,
    thick: f32,
) {
    let out = Vec3::new(0.0, droop, -(1.0 - droop * droop).max(0.0).sqrt()).normalize();
    let tip = root + out * span - Vec3::X * sweep;
    let outline = [
        root,
        root - Vec3::X * chord,
        tip - Vec3::X * (chord * taper),
        tip,
    ];
    let n = out.cross(Vec3::X).normalize() * thick;
    let base: Vec<Vec3> = outline.iter().map(|&p| p - n * 0.5).collect();
    dark_plate(b);
    b.with_bevel(0.03, |b| slab(b, &base, n, false));
}

/// A pair of red optics either side of the nose at `x`, `half` out, `z` up: a long slit
/// under a brow and a small one behind it.
pub(super) fn optics(b: &mut MeshBuilder, x: f32, half: f32, z: f32, len: f32) {
    if b.coarse() {
        return;
    }
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        b.beam(
            v3(x, half, z),
            v3(x - len, half + len * 0.22, z + 0.03),
            Vec2::new(0.1, 0.1),
            Vec2::new(0.07, 0.08),
        );
        if b.fine() {
            let back = x - len * 1.35;
            b.beam(
                v3(back, half + len * 0.3, z + 0.04),
                v3(back - len * 0.35, half + len * 0.38, z + 0.04),
                Vec2::new(0.06, 0.07),
                Vec2::new(0.05, 0.05),
            );
        }
    });
}

/// A plasma jet's nozzle: a bronze can from `at` back `len`, a seam-dark lip and red heat
/// in its mouth, marked as an exhaust. Gives the mouth.
pub(super) fn nozzle(b: &mut MeshBuilder, at: Vec3, radius: f32, len: f32) -> Vec3 {
    let mouth = at - Vec3::X * len;
    if b.coarse() {
        return mouth;
    }
    let sides = b.sides(10);
    metal(b);
    b.cylinder_between(at, mouth + Vec3::X * 0.12, radius, radius * 0.88, sides);
    seam(b);
    b.cylinder_between(
        mouth + Vec3::X * 0.12,
        mouth,
        radius * 0.9,
        radius * 0.92,
        sides,
    );
    b.paint(GLOW_LASER);
    b.cylinder_between(
        mouth + Vec3::X * 0.06,
        mouth - Vec3::X * 0.02,
        radius * 0.68,
        radius * 0.6,
        sides,
    );
    b.add_exhaust(mouth, -Vec3::X, radius * 0.7);
    mouth
}

/// The bronze workings down a gap: a shaft from `a` to `c`, a collar at each `gaps`
/// fraction, and a cable either side. Close up only.
pub(super) fn workings(b: &mut MeshBuilder, a: Vec3, c: Vec3, r: f32, gaps: &[f32]) {
    if b.coarse() {
        return;
    }
    metal(b);
    let sides = b.sides(8);
    b.cylinder_between(a, c, r, r, sides);
    if !b.fine() {
        return;
    }
    let along = (c - a).normalize();
    for &t in gaps {
        collar(b, a.lerp(c, t), along, r * 1.45, r * 1.6);
    }
    let side = Vec3::Z.cross(along).normalize_or(Vec3::Y) * (r * 1.6);
    for s in [side, -side] {
        let points = [
            a + s,
            a.lerp(c, 0.5) + s * 1.15 - Vec3::Z * (r * 0.3),
            c + s,
        ];
        metal(b);
        cable(b, &points, r * 0.28);
    }
}

/// The owner's colour: a chevron let flat into the back at `at`, pointing forward, `size`
/// long.
pub(super) fn chevron(b: &mut MeshBuilder, at: Vec3, size: f32) {
    let (x, z) = (at.x, at.z);
    let outline = [
        v3(x + size * 0.5, 0.0, z),
        v3(x - size * 0.4, size * 0.7, z),
        v3(x - size * 0.62, size * 0.5, z),
        v3(x - 0.05 * size, 0.1 * size, z),
        v3(x - size * 0.62, -size * 0.5, z),
        v3(x - size * 0.4, -size * 0.7, z),
    ];
    b.paint(TEAM);
    if b.coarse() {
        b.face(&[outline[0], outline[1], outline[5]]);
        return;
    }
    slab(b, &outline, Vec3::Z * 0.05, false);
}
