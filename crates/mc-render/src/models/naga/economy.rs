//! The Naga's tech 1 economy: the Taproot (mass), the Heart (power) and the Cyst
//! (storage), each grown into its lot rather than set on a slab.
//!
//! - Taproot (3x3 lot): six thick plated roots grip the ore field and plunge into it
//!   round a swollen bulb, where the mass is drawn up; red veins run up the roots' flanks
//!   and six rams (`part::PUMP`) draw on the bulb in turn. Nothing hammers: it needs no
//!   power. On open water it stands on its roots over the sea, like a mangrove: the whole
//!   knot rides `LIFT` higher, its ground grip (`part::ASHORE`) gives way to prop roots
//!   and a stalk down into the water (`part::AFLOAT`), recorded as a `Pit` with nothing
//!   dug, which is how the shader knows to do it (`models::Pit`).
//! - Heart (2x2 lot): a ribbed organ in a cage of plated ribs off a vertebral spine that
//!   runs out both ends into the ground, red light in slits between the ribs, four great
//!   vessels arching down to the corners, cables to the sides, and four valves beating in
//!   turn (`part::PUMP`). Nothing on it can breach: no vents, no pressure vessels.
//! - Cyst (3x3 lot): two sacs held down by plated straps, a vertebral girdle over the
//!   waist between them. Energy is kept in the red one, its light between armour petals;
//!   mass in the dark dense one, bare metal under tiers of heavy scales.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::{Vec2, Vec3};

use crate::models::builder::{hash_unit, MeshBuilder};
use crate::models::material::*;
use crate::models::{part, Pit};

use super::kit::*;

// ---- shared pieces --------------------------------------------------------------------

/// Every point of `rings` kept out of the ground: a limb that runs into it flattens
/// against it instead.
fn grounded(mut rings: Vec<Vec<Vec3>>) -> Vec<Vec<Vec3>> {
    for p in rings.iter_mut().flatten() {
        p.z = p.z.max(0.0);
    }
    rings
}

/// A run of soft hide whose belly never dips under the ground.
fn limb(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3) {
    let r = grounded(rings(b, points, hint, chitin_ring));
    b.loft(&r, true, true);
}

/// An arched armour plate that never dips under the ground.
fn plate(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3) {
    let r = grounded(rings(b, points, hint, shell_ring));
    b.loft(&r, true, true);
}

/// A plated limb: soft hide along `points`, an armour plate over each span that stops
/// short of the next, and a bare joint drum at each of the inner points `joints`.
fn plated(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3, joints: &[usize]) {
    under_hide(b);
    limb(b, points, hint);
    for w in points.windows(2) {
        let ((a, wa, ha), (c, wc, hc)) = (w[0], w[1]);
        let (_, up) = frame(c - a, hint);
        let at = |t: f32| {
            let h = ha + (hc - ha) * t;
            (
                a.lerp(c, t) + up * (h * 0.3),
                (wa + (wc - wa) * t) * 1.2,
                h * 0.95,
            )
        };
        hide(b);
        plate(b, &[at(0.08), at(0.92)], hint);
    }
    for &i in joints {
        let (p, w, h) = points[i];
        let (side, _) = frame(points[i + 1].0 - points[i - 1].0, hint);
        knuckle(b, p, side, h * 0.8, w * 2.3);
    }
}

/// A bare metal band clamped round a strap, a vessel or a spine at `at`, across `dir`.
fn band(b: &mut MeshBuilder, at: Vec3, dir: Vec3, radius: f32, width: f32) {
    let half = dir.normalize() * (width * 0.5);
    metal(b);
    b.cylinder_between(at - half, at + half, radius, radius, b.sides(6));
}

/// A round tube through `points` (centre, radius), `sides` round.
fn tube(b: &mut MeshBuilder, points: &[(Vec3, f32)], sides: usize) {
    let r: Vec<Vec<Vec3>> = points
        .iter()
        .enumerate()
        .map(|(i, &(c, radius))| {
            let dir = if i + 1 < points.len() {
                points[i + 1].0 - c
            } else {
                c - points[i - 1].0
            };
            let (side, up) = frame(dir, Vec3::Z);
            (0..sides)
                .map(|k| {
                    let a = (k as f32 + 0.5) * TAU / sides as f32;
                    c + (side * a.cos() + up * a.sin()) * radius
                })
                .collect()
        })
        .collect();
    b.loft(&grounded(r), true, true);
}

/// A flat kite of hide lying on the ground from `at` out along `dir`: a root's grip.
fn grip(b: &mut MeshBuilder, at: Vec3, dir: Vec3, length: f32, width: f32) {
    let dir = dir.with_z(0.0).normalize();
    let across = Vec3::new(-dir.y, dir.x, 0.0);
    let at = at.with_z(0.0);
    hide(b);
    slab(
        b,
        [
            at - across * width,
            at + dir * length - across * (width * 0.3),
            at + dir * length + across * (width * 0.3),
            at + across * width,
        ],
        Vec3::Z * 0.3,
    );
}

/// The unit vector at `angle` round the z axis.
fn heading(angle: f32) -> Vec3 {
    Vec3::new(angle.cos(), angle.sin(), 0.0)
}

// ---- Taproot ----------------------------------------------------------------------------

/// How much higher the whole knot stands on open water, on its prop roots.
const LIFT: f32 = 3.5;
/// The bulb's profile, ground to neck: (height, radius).
const BULB: [(f32, f32); 8] = [
    (0.0, 5.2),
    (1.2, 5.4),
    (3.0, 5.3),
    (4.8, 4.9),
    (6.4, 4.1),
    (7.6, 3.2),
    (8.5, 2.6),
    (9.2, 2.7),
];
const ROOTS: usize = 6;
/// Half width and half height of a root at each point along it: thick where it leaves
/// the bulb, flattening where it runs into the ground.
const ROOT_GIRTH: [(f32, f32); 6] = [
    (1.9, 1.5),
    (1.7, 1.35),
    (1.45, 1.15),
    (1.25, 0.95),
    (1.15, 0.8),
    (1.2, 0.5),
];

/// The bulb's radius at height `z`.
fn bulb(z: f32) -> f32 {
    let i = BULB
        .iter()
        .position(|&(h, _)| h >= z)
        .unwrap_or(BULB.len() - 1)
        .max(1);
    let ((z0, r0), (z1, r1)) = (BULB[i - 1], BULB[i]);
    r0 + (r1 - r0) * ((z - z0) / (z1 - z0)).clamp(0.0, 1.0)
}

/// Which way root `i` runs.
fn root_angle(i: usize) -> f32 {
    TAU * i as f32 / ROOTS as f32 + 0.26 + (hash_unit(7, i as u32) - 0.5) * 0.28
}

/// Root `i`'s spine: out of the bulb's foot, over its shoulder and down along the
/// ground to where it goes in, curling a little as it goes, so the knot swirls. Each
/// reaches about as far as the lot's side, so the knot fills the square.
fn root_path(i: usize) -> [Vec3; 6] {
    let a = root_angle(i);
    let square = a.cos().abs().max(a.sin().abs());
    let reach = (14.4 / square).min(18.0) * (0.94 + hash_unit(11, i as u32) * 0.08);
    let at = |t: f32, z: f32| {
        let s = 5.6 + (reach - 5.6) * t;
        heading(a + 0.018 * s) * s + Vec3::Z * z
    };
    [
        heading(a) * 3.4 + Vec3::Z * 3.6,
        heading(a) * 5.6 + Vec3::Z * 4.3,
        at(0.3, 3.3),
        at(0.62, 1.8),
        at(0.85, 1.0),
        at(1.0, 0.5),
    ]
}

pub(super) fn taproot(b: &mut MeshBuilder, _tech: u8) {
    // Nothing is dug: the pit only tells the shader to lift the knot on water.
    b.set_pit(Pit {
        open: 0.0,
        radius: 5.0,
        stroke: 0.0,
        section: 0.0,
        rack: [0.0, 0.0],
        afloat_lift: LIFT,
    });
    if b.coarse() {
        taproot_coarse(b);
        return;
    }
    bulb_body(b);
    for i in 0..ROOTS {
        root(b, i);
        let between =
            root_angle(i) + (root_angle((i + 1) % ROOTS) - root_angle(i)).rem_euclid(TAU) * 0.5;
        bracts(b, between);
        if b.fine() {
            pump(b, between);
        }
    }
    crown(b);
    b.with_part(part::AFLOAT, |b| {
        // The stalk the bulb stands on, down into the water.
        hide(b);
        b.prism(Vec3::ZERO, b.sides(10), 3.6, 4.4, LIFT + 0.8);
    });
}

/// Far off: the bulb, a tent of hide along each root, the team's mark on top.
fn taproot_coarse(b: &mut MeshBuilder) {
    hide(b);
    b.prism(Vec3::ZERO, 6, 5.0, 2.6, 9.2);
    for i in 0..ROOTS {
        let p = root_path(i);
        let side = Vec3::new(-p[5].y, p[5].x, 0.0).normalize();
        let (top, tip) = (p[0] + Vec3::Z * 1.6, p[5].with_z(0.0));
        let eave = top.lerp(tip, 0.5) - Vec3::Z * 1.2;
        b.face(&[top, eave - side * 1.6, tip]);
        b.face(&[top, tip, eave + side * 1.6]);
    }
    b.paint(TEAM);
    b.prism(Vec3::Z * 9.2, 3, 2.0, 1.6, 0.3);
    b.with_part(part::AFLOAT, |b| {
        hide(b);
        b.frustum_open(
            Vec3::ZERO,
            Vec2::splat(6.0),
            Vec2::splat(5.0),
            LIFT + 0.8,
            Vec2::ZERO,
        );
    });
}

/// The bulb: a swollen core of soft hide, bare vertebral rings up its neck.
fn bulb_body(b: &mut MeshBuilder) {
    under_hide(b);
    let sides = b.sides(12);
    let rings: Vec<Vec<Vec3>> = BULB
        .iter()
        .map(|&(z, r)| {
            (0..sides)
                .map(|k| heading((k as f32 + 0.5) * TAU / sides as f32) * r + Vec3::Z * z)
                .collect()
        })
        .collect();
    b.loft(&rings, false, true);
    metal(b);
    for (z, r) in [(7.7, 2.85), (8.35, 2.7)] {
        b.prism(Vec3::Z * z, b.sides(10), r, r, 0.4);
    }
}

/// Two armour plates, one over the other, on the bulb between two roots.
fn bracts(b: &mut MeshBuilder, angle: f32) {
    let d = heading(angle);
    let on = |z: f32| (d * (bulb(z) + 0.3) + Vec3::Z * z, 1.75, 0.5);
    hide(b);
    plate(b, &[on(0.3), on(2.6), on(4.9)], d);
    plate(b, &[on(5.3), on(7.5)], d);
}

/// One root: plated where it runs over the ground, its grip where it goes in (on land),
/// or a prop root down into the sea (afloat).
fn root(b: &mut MeshBuilder, i: usize) {
    let p = root_path(i);
    let pts: Vec<(Vec3, f32, f32)> = p
        .iter()
        .zip(ROOT_GIRTH)
        .map(|(&c, (w, h))| (c, w, h))
        .collect();
    let d = (p[5] - p[4]).with_z(0.0).normalize();
    plated(b, &pts[..5], Vec3::Z, &[2, 3]);
    // A buttress of hide from the root's back up the bulb: what grips the stalk.
    let out = heading(root_angle(i));
    let across = Vec3::new(-out.y, out.x, 0.0) * 0.25;
    let fin = [
        out * 3.9 + Vec3::Z * 4.2,
        out * 3.4 + Vec3::Z * 7.9,
        out * 5.4 + Vec3::Z * 6.0,
        p[2] + Vec3::Z * 1.1,
    ];
    hide(b);
    slab(b, fin.map(|q| q - across), across * 2.0);
    if i.is_multiple_of(2) {
        // The team's mark on the root's back.
        let (side, up) = frame(p[2] - p[1], Vec3::Z);
        let c = p[1].lerp(p[2], 0.5) + up * (1.28 * 1.6);
        let dir = (p[2] - p[1]).normalize() * 0.9;
        b.paint(TEAM);
        slab(
            b,
            [
                c - dir - side * 0.6,
                c + dir - side * 0.6,
                c + dir + side * 0.6,
                c - dir + side * 0.6,
            ],
            up * 0.14,
        );
    }
    if b.fine() {
        // Red veins up both flanks, under the plates' rims.
        b.paint(GLOW_LASER);
        for sign in [-1.0, 1.0] {
            let vein: Vec<Vec3> = (2..5)
                .map(|k| {
                    let (c, w, h) = pts[k];
                    let (side, up) = frame(pts[k + 1].0 - pts[k - 1].0, Vec3::Z);
                    c + side * (w * 0.86 * sign) - up * (h * 0.55)
                })
                .collect();
            for w in vein.windows(2) {
                b.beam(w[0], w[1], Vec2::splat(0.16), Vec2::splat(0.12));
            }
        }
    }
    b.with_part(part::ASHORE, |b| {
        under_hide(b);
        limb(b, &pts[4..], Vec3::Z);
        grip(b, p[5], d, 2.4, 1.1);
        let across = Vec3::new(-d.y, d.x, 0.0);
        grip(b, p[4], d + across * 0.8, 2.6, 0.8);
        grip(b, p[4], d - across * 0.8, 2.6, 0.8);
    });
    b.with_part(part::AFLOAT, |b| {
        let lifted = p[4] + Vec3::Z * LIFT;
        let prop = [
            (lifted, 1.15, 0.8),
            (p[4] + d * 1.6 + Vec3::Z * (LIFT * 0.7 + 0.4), 1.1, 0.85),
            (p[5].with_z(0.0) + d * 1.7 + Vec3::Z * 0.4, 1.25, 0.7),
        ];
        plated(b, &prop, Vec3::Z, &[]);
    });
}

/// A ram between two roots, drawing on the bulb: the barrel hung off the bulb's
/// plates, the rod (`part::PUMP`) stroking into its socket in the ground.
fn pump(b: &mut MeshBuilder, angle: f32) {
    let d = heading(angle);
    let at = |s: f32, z: f32| d * s + Vec3::Z * z;
    let (top, mid, foot) = (at(bulb(4.6) + 0.5, 4.6), at(6.3, 2.2), at(7.1, 0.8));
    metal(b);
    b.cylinder_between(top, mid, 0.42, 0.42, 6);
    // Its socket in the ground (afloat, the rod just reaches down toward the water).
    b.with_part(part::ASHORE, |b| {
        under_hide(b);
        b.prism(foot.with_z(0.0), 6, 0.62, 0.5, 1.2);
        grip(b, foot, d, 1.8, 0.9);
    });
    b.with_part(part::PUMP, |b| {
        metal(b);
        b.cylinder_between(top.lerp(mid, 0.6), foot, 0.22, 0.22, 6);
    });
}

/// The crown: raked blades round the neck, the team's collar under them, and the mass
/// glowing where it comes up.
fn crown(b: &mut MeshBuilder) {
    let n = 7;
    hide(b);
    for k in 0..n {
        let a = TAU * k as f32 / n as f32 + 0.2;
        let d = heading(a);
        let across = Vec3::new(-d.y, d.x, 0.0);
        blade(
            b,
            d * 2.45 + Vec3::Z * 9.1,
            d * 3.9 + across * 0.5 + Vec3::Z * 11.0,
            0.6,
            across,
        );
    }
    hide(b);
    b.prism(Vec3::Z * 8.9, b.sides(10), 2.8, 2.45, 0.4);
    b.paint(GLOW_LASER);
    b.prism(Vec3::Z * 9.3, b.sides(8), 2.05, 1.1, 0.75);
}

// ---- Heart ------------------------------------------------------------------------------

/// The organ's middle and radii: long along x, under the spine.
const CORE: Vec3 = Vec3::new(0.0, 0.0, 3.4);
const CORE_R: Vec3 = Vec3::new(5.2, 4.3, 3.3);
/// Where the ribs come off the spine along x.
const RIBS: [f32; 5] = [-3.9, -1.95, 0.0, 1.95, 3.9];
/// The gaps between them, where the heart's light shows.
const GAPS: [f32; 4] = [-2.93, -0.98, 0.98, 2.93];

/// How much of the core's section is left at `x` (1 in the middle, 0 at its ends).
fn core_fraction(x: f32) -> f32 {
    (1.0 - (x / CORE_R.x).powi(2)).max(0.0).sqrt()
}

pub(super) fn heart(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        heart_coarse(b);
        return;
    }
    under_hide(b);
    b.lumpy_spheroid(
        CORE,
        CORE_R,
        b.sides(14),
        if b.fine() { 8 } else { 5 },
        0.05,
        3,
    );
    // The organ's light, showing between the ribs: slits down its flanks, and across
    // its top where the gaps run over it.
    b.paint(GLOW_LASER);
    for &x in &GAPS {
        let f = core_fraction(x);
        if b.fine() {
            b.mirror_y(|b| {
                b.spheroid(
                    Vec3::new(x, CORE_R.y * f + 0.02, CORE.z + 0.2),
                    Vec3::new(0.22, 0.16, 1.6 * f + 0.3),
                    5,
                    3,
                )
            });
        }
        b.spheroid(
            Vec3::new(x, 0.0, CORE.z + CORE_R.z * f - 0.1),
            Vec3::new(0.22, 1.9 * f, 0.26),
            if b.fine() { 5 } else { 4 },
            3,
        );
    }
    b.mirror_y(|b| {
        for &x in &RIBS {
            rib(b, x);
        }
        for &x in &[GAPS[1], GAPS[2]] {
            vein_cable(b, x);
        }
    });
    spine(b);
    for (path, radius) in VESSELS {
        vessel(b, path, radius);
    }
    if b.fine() {
        for [x, y] in VALVES {
            valve(b, x, y);
        }
    }
    // Tendons splayed from its foot into the ground.
    for k in 0..8 {
        let d = heading(TAU * (k as f32 + 0.5) / 8.0);
        let r = 4.2 * (1.0 - 0.1 * (d.x.abs() - d.y.abs()).abs());
        grip(b, d * r, d, 2.6, 0.75);
    }
}

/// Far off: the organ, the spine running out both ends, the vessels, the team's mark.
fn heart_coarse(b: &mut MeshBuilder) {
    hide(b);
    b.frustum(
        Vec3::ZERO,
        Vec2::new(10.4, 8.6),
        Vec2::new(6.0, 4.4),
        6.7,
        Vec2::ZERO,
    );
    for sx in [-1.0f32, 1.0] {
        b.beam(
            Vec3::new(1.5 * sx, 0.0, 6.6),
            Vec3::new(10.3 * sx, 0.0, 0.45),
            Vec2::new(1.4, 0.9),
            Vec2::new(1.4, 0.9),
        );
    }
    for (path, radius) in VESSELS {
        let (top, foot) = (
            Vec3::from(path[1]) + Vec3::Z * radius,
            Vec3::from(path[path.len() - 1]).with_z(0.0),
        );
        let across = Vec3::new(-foot.y, foot.x, 0.0).normalize() * (radius * 0.9);
        b.face(&[top, foot - across, foot + across]);
    }
    b.paint(TEAM);
    b.prism(Vec3::Z * 6.7, 3, 1.6, 1.2, 0.3);
}

/// A rib off the spine at `x`, down the +y flank of the organ and into the ground.
fn rib(b: &mut MeshBuilder, x: f32) {
    let f = core_fraction(x);
    let (ry, rz) = (CORE_R.y * f, CORE_R.z * f);
    let pts = [
        (Vec3::new(x, 0.55, CORE.z + rz + 0.45), 0.42, 0.42),
        (
            Vec3::new(x, ry * 0.72 + 0.32, CORE.z + rz * 0.72 + 0.38),
            0.44,
            0.42,
        ),
        (
            Vec3::new(x * 1.03, ry + 0.42, CORE.z + rz * 0.1),
            0.46,
            0.42,
        ),
        (Vec3::new(x * 1.06, ry * 0.96 + 0.6, 1.4), 0.48, 0.42),
        (Vec3::new(x * 1.1, ry + 1.6, 0.3), 0.55, 0.36),
    ];
    hide(b);
    if b.fine() {
        plate(
            b,
            &[pts[0], pts[1], pts[2], pts[4]],
            Vec3::new(0.0, 1.0, 0.8),
        );
    } else {
        plate(b, &[pts[0], pts[2], pts[4]], Vec3::new(0.0, 1.0, 0.8));
    }
}

/// The spine along the top: bare vertebrae over the ribs, running out as a plated neck
/// ahead and a tail behind, both plunging into the ground; the team's mark on the middle.
fn spine(b: &mut MeshBuilder) {
    let fine = b.fine();
    for k in 0..9 {
        let x = -5.2 + k as f32 * 1.3;
        let z = CORE.z + CORE_R.z * core_fraction(x) + 0.75;
        band(b, Vec3::new(x, 0.0, z), Vec3::X, 0.55, 0.7);
        if fine && k % 2 == 1 {
            hide(b);
            spike(
                b,
                Vec3::new(x, 0.0, z + 0.4),
                Vec3::new(x - 0.6, 0.0, z + 1.2),
                0.3,
            );
        }
    }
    b.paint(TEAM);
    b.plate(
        Vec3::new(0.0, 0.0, CORE.z + CORE_R.z + 1.22),
        Vec2::new(1.5, 1.2),
        0.18,
        0.08,
    );
    for sx in [-1.0f32, 1.0] {
        let at = |x: f32, z: f32| Vec3::new(x * sx, 0.0, z);
        plated(
            b,
            &[
                (at(5.3, 4.2), 0.85, 0.75),
                (at(7.3, 3.8), 0.8, 0.7),
                (at(9.2, 2.2), 0.72, 0.64),
                (at(10.7, 0.5), 0.8, 0.5),
            ],
            Vec3::Z,
            &[1],
        );
        grip(b, at(10.4, 0.0), Vec3::X * sx, 1.1, 0.9);
    }
}

/// The great vessels, each from a gap in the ribs over the top of the heart, arching
/// out and down into the ground near a corner: one great arch, a second behind it and two
/// lesser, so the organ is not a four-legged thing. (Path, radius.)
const VESSELS: [(&[[f32; 3]], f32); 4] = [
    (
        &[
            [0.98, 1.0, 6.3],
            [1.9, 2.5, 7.4],
            [3.9, 4.4, 7.1],
            [5.9, 6.4, 4.6],
            [7.0, 7.6, 1.6],
            [7.4, 8.0, 0.5],
        ],
        1.05,
    ),
    (
        &[
            [-0.98, -1.0, 6.3],
            [-2.2, -2.3, 7.2],
            [-4.3, -4.0, 6.6],
            [-6.3, -5.8, 3.8],
            [-7.2, -6.6, 1.2],
            [-7.5, -6.9, 0.45],
        ],
        0.9,
    ),
    (
        &[
            [2.93, -0.8, 5.9],
            [3.9, -2.0, 6.8],
            [5.6, -3.8, 5.6],
            [7.0, -5.4, 2.4],
            [7.5, -5.9, 0.4],
        ],
        0.75,
    ),
    (
        &[
            [-2.93, 0.9, 5.9],
            [-3.9, 2.2, 6.6],
            [-5.3, 3.9, 5.2],
            [-6.5, 5.5, 2.2],
            [-6.9, 6.0, 0.4],
        ],
        0.7,
    ),
];
/// The valves beating on the top of the organ, in the gaps opposite the vessels.
const VALVES: [[f32; 2]; 4] = [[0.98, -2.0], [-0.98, 2.0], [2.93, 1.5], [-2.93, -1.5]];

/// A great vessel: soft hide under plates that stop short at each bare metal clamp,
/// swelling where it goes into the ground.
fn vessel(b: &mut MeshBuilder, path: &[[f32; 3]], radius: f32) {
    let n = path.len();
    let path: Vec<(Vec3, f32)> = path
        .iter()
        .enumerate()
        .map(|(i, &p)| (Vec3::from(p), radius * if i + 1 == n { 1.15 } else { 1.0 }))
        .collect();
    under_hide(b);
    let sides = b.sides(8);
    if b.fine() {
        tube(b, &path, sides);
    } else {
        let short: Vec<(Vec3, f32)> = path
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != n - 2)
            .map(|(_, &p)| p)
            .collect();
        tube(b, &short, sides);
    }
    // Armour along its top, stopping short at each clamp.
    hide(b);
    for w in path.windows(2).take(n - 2) {
        let (a, c) = (w[0].0, w[1].0);
        let up = |p: Vec3| p + Vec3::Z * (radius * 0.35);
        plate(
            b,
            &[
                (up(a.lerp(c, 0.12)), radius * 1.05, radius * 0.8),
                (up(a.lerp(c, 0.88)), radius * 1.05, radius * 0.8),
            ],
            Vec3::Z,
        );
    }
    if b.fine() {
        for w in path.windows(2).take(n - 2) {
            band(b, w[1].0, w[1].0 - w[0].0, radius * 1.18, 0.4);
        }
    }
    // Where it goes in: a boss of hide, and its grip spread on the ground.
    let foot = path[n - 1].0;
    hide(b);
    b.prism(
        foot.with_z(0.0),
        b.sides(8),
        radius * 1.7,
        radius * 1.25,
        0.6,
    );
    grip(b, foot, foot.with_z(0.0), 1.6, radius);
}

/// A valve on the top of the organ at (`x`, `y`): a sleeve, and its plunger beating in
/// it (`part::PUMP`).
fn valve(b: &mut MeshBuilder, x: f32, y: f32) {
    let at = Vec3::new(x, y, 0.0);
    let q = 1.0 - (x / CORE_R.x).powi(2) - (y / CORE_R.y).powi(2);
    let root = CORE.z + CORE_R.z * q.max(0.0).sqrt();
    metal(b);
    b.cylinder_between(
        at + Vec3::Z * (root - 0.3),
        at + Vec3::Z * (root + 0.7),
        0.45,
        0.45,
        8,
    );
    b.with_part(part::PUMP, |b| {
        metal(b);
        b.cylinder_between(
            at + Vec3::Z * (root + 0.2),
            at + Vec3::Z * (root + 1.3),
            0.24,
            0.24,
            6,
        );
        hide(b);
        b.cylinder_between(
            at + Vec3::Z * (root + 1.3),
            at + Vec3::Z * (root + 1.55),
            0.5,
            0.42,
            8,
        );
    });
}

/// A cable from the organ's +y flank at `x`, arching out and down into the ground.
fn vein_cable(b: &mut MeshBuilder, x: f32) {
    let pts = [
        Vec3::new(x, CORE_R.y - 0.4, 2.2),
        Vec3::new(x * 1.3, 6.0, 3.2),
        Vec3::new(x * 1.6, 8.9, 2.2),
        Vec3::new(x * 1.8, 10.5, 0.35),
    ];
    metal(b);
    let sides = if b.fine() { 5 } else { 4 };
    if b.fine() {
        tube(b, &pts.map(|p| (p, 0.32)), sides);
    } else {
        tube(b, &[(pts[0], 0.32), (pts[1], 0.32), (pts[3], 0.32)], sides);
    }
    under_hide(b);
    b.prism(pts[3].with_z(0.0), sides, 0.75, 0.55, 0.6);
}

// ---- Cyst -------------------------------------------------------------------------------

/// A sac: its middle (lifted, so it bulges over where it meets the ground) and radii.
#[derive(Clone, Copy)]
struct Sac {
    c: Vec3,
    r: Vec3,
}

/// The red chamber, energy, toward +x; the dark dense one, mass, toward -x.
const ENERGY: Sac = Sac {
    c: Vec3::new(6.9, 0.0, 1.4),
    r: Vec3::new(7.4, 8.4, 6.6),
};
const MASS: Sac = Sac {
    c: Vec3::new(-7.2, 0.0, 1.0),
    r: Vec3::new(7.8, 9.1, 5.4),
};

impl Sac {
    /// The point on its skin at `lat`, `lon`, at `scale` of its radii.
    fn at(self, lat: f32, lon: f32, scale: f32) -> Vec3 {
        self.c + Vec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()) * self.r * scale
    }

    /// The latitude where it meets the ground.
    fn base(self) -> f32 {
        -(self.c.z / self.r.z).asin()
    }

    /// Its skin between two latitudes, a full turn round.
    fn dome(
        self,
        b: &mut MeshBuilder,
        lat: [f32; 2],
        scale: f32,
        bands: usize,
        rough: f32,
        seed: u32,
    ) {
        let sides = b.sides(16);
        let rings: Vec<Vec<Vec3>> = (0..=bands)
            .map(|j| {
                let l = lat[0] + (lat[1] - lat[0]) * j as f32 / bands as f32;
                let pole = l >= FRAC_PI_2 - 1e-3;
                (0..sides)
                    .map(|k| {
                        let bump = if pole {
                            1.0
                        } else {
                            1.0 + rough * (hash_unit(seed, (j * sides + k) as u32) * 2.0 - 1.0)
                        };
                        self.at(
                            l,
                            TAU * (k as f32 + 0.5 * (j % 2) as f32) / sides as f32,
                            scale * bump,
                        )
                    })
                    .collect()
            })
            .collect();
        b.loft(&grounded(rings), false, true);
    }

    /// A curved armour scale on its skin between two longitudes and latitudes, standing
    /// `lift` off it and `thick` deep.
    fn scale(
        self,
        b: &mut MeshBuilder,
        lon: [f32; 2],
        lat: [f32; 2],
        bands: usize,
        lift: f32,
        thick: f32,
    ) {
        let across = if b.fine() { 3 } else { 2 };
        let rings: Vec<Vec<Vec3>> = (0..=bands)
            .map(|j| {
                let l = lat[0] + (lat[1] - lat[0]) * j as f32 / bands as f32;
                // The upper edge a little narrower, so a scale tapers as it climbs.
                let narrow = 1.0 - 0.12 * j as f32 / bands as f32;
                let mid = (lon[0] + lon[1]) * 0.5;
                let half = (lon[1] - lon[0]) * 0.5 * narrow;
                let mut ring: Vec<Vec3> = (0..across)
                    .map(|k| {
                        self.at(
                            l,
                            mid - half + 2.0 * half * k as f32 / (across - 1) as f32,
                            lift + thick,
                        )
                    })
                    .collect();
                ring.push(self.at(l, mid + half, lift));
                ring.push(self.at(l, mid - half, lift));
                ring
            })
            .collect();
        b.loft(&grounded(rings), true, true);
    }

    /// The skin's height at (`x`, `y`), or 0 off it.
    fn height(self, x: f32, y: f32, scale: f32) -> f32 {
        let r = self.r * scale;
        let q = 1.0 - ((x - self.c.x) / r.x).powi(2) - ((y - self.c.y) / r.y).powi(2);
        if q <= 0.0 {
            0.0
        } else {
            (self.c.z + r.z * q.sqrt()).max(0.0)
        }
    }

    /// A plated strap over it along y at `x`: three plates buckled together, pegged into
    /// the ground at both ends.
    fn strap(self, b: &mut MeshBuilder, x: f32, width: f32) {
        let k = (1.0 - ((x - self.c.x) / self.r.x).powi(2)).max(0.05).sqrt();
        let (ry, rz) = (self.r.y * k, self.r.z * k);
        let t0 = (-(self.c.z) / rz).asin();
        let arc = |t: f32| {
            let out = Vec3::new(0.0, t.cos() / ry, t.sin() / rz).normalize();
            (
                Vec3::new(x, self.c.y + t.cos() * ry, self.c.z + t.sin() * rz) + out * 0.55,
                out,
            )
        };
        let spans = 3;
        let span = (PI - 2.0 * t0) / spans as f32;
        for s in 0..spans {
            let steps = if b.fine() { 2 } else { 1 };
            let pts: Vec<(Vec3, Vec3)> = (0..=steps)
                .map(|j| arc(t0 + span * (s as f32 + 0.05 + 0.9 * j as f32 / steps as f32)))
                .collect();
            let rings: Vec<Vec<Vec3>> = pts
                .iter()
                .enumerate()
                .map(|(j, &(c, out))| {
                    let dir = if j + 1 < pts.len() {
                        pts[j + 1].0 - c
                    } else {
                        c - pts[j - 1].0
                    };
                    let (side, up) = frame(dir, out);
                    shell_ring(b, c, side, up, width, 0.4)
                })
                .collect();
            hide(b);
            b.loft(&grounded(rings), true, true);
            if s > 0 {
                let (c, _) = arc(t0 + span * s as f32);
                band(b, c, Vec3::X, 0.42, width * 2.3);
            }
        }
        // Pegged at both ends.
        for t in [t0, PI - t0] {
            let (c, _) = arc(t);
            let out = Vec3::new(0.0, t.cos().signum(), 0.0);
            grip(b, c.with_z(0.0) - out * 0.3, out, 2.0, width * 1.1);
            if b.fine() {
                metal(b);
                spike(
                    b,
                    c + Vec3::Z * 0.9 + out * 0.2,
                    (c + out * 1.4).with_z(0.0),
                    0.25,
                );
            }
        }
    }
}

pub(super) fn cyst(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        cyst_coarse(b);
        return;
    }
    energy_chamber(b);
    mass_chamber(b);
    girdle(b);
    // Roots from the sacs' feet to the lot's corners and ends.
    let roots: [(Vec3, Vec3); 6] = [
        (Vec3::new(11.0, 5.2, 2.2), Vec3::new(15.4, 11.6, 0.4)),
        (Vec3::new(11.0, -5.2, 2.2), Vec3::new(15.4, -11.6, 0.4)),
        (Vec3::new(-11.8, 5.6, 1.9), Vec3::new(-15.6, 11.8, 0.4)),
        (Vec3::new(-11.8, -5.6, 1.9), Vec3::new(-15.6, -11.8, 0.4)),
        (Vec3::new(13.6, 0.0, 2.0), Vec3::new(15.9, 0.6, 0.4)),
        (Vec3::new(-14.4, 0.0, 1.6), Vec3::new(-15.9, -0.6, 0.4)),
    ];
    for (a, c) in roots {
        let bend = a.lerp(c, 0.5) + Vec3::Z * 0.9;
        plated(
            b,
            &[(a, 0.8, 0.7), (bend, 0.72, 0.62), (c, 0.8, 0.45)],
            Vec3::Z,
            &[1],
        );
        grip(b, c, c - a, 1.1, 0.8);
    }
}

/// Far off: two sacs, the red one's light on top, the team's mark on the waist.
fn cyst_coarse(b: &mut MeshBuilder) {
    hide(b);
    b.prism(ENERGY.c.with_z(0.0), 6, 10.0, 4.0, ENERGY.c.z + ENERGY.r.z);
    b.prism(MASS.c.with_z(0.0), 6, 10.2, 4.2, MASS.c.z + MASS.r.z);
    b.paint(GLOW_LASER);
    let top = ENERGY.c.with_z(ENERGY.c.z + ENERGY.r.z + 0.05);
    b.face(&[
        top + Vec3::X * 2.2,
        top + Vec3::new(-1.1, 1.9, 0.0),
        top + Vec3::new(-1.1, -1.9, 0.0),
    ]);
    b.paint(TEAM);
    b.prism(Vec3::new(-0.2, 0.0, 3.8), 3, 1.8, 1.4, 0.3);
}

/// Energy: a red-lit sac under seven armour petals and a lid, light between them, two
/// straps over it.
fn energy_chamber(b: &mut MeshBuilder) {
    let sac = ENERGY;
    let base = sac.base();
    b.paint(GLOW_LASER);
    sac.dome(
        b,
        [base, FRAC_PI_2],
        0.96,
        if b.fine() { 6 } else { 4 },
        0.0,
        0,
    );
    let petals = 7;
    let bands = if b.fine() { 5 } else { 3 };
    hide(b);
    for k in 0..petals {
        let a = TAU * k as f32 / petals as f32 + 0.3;
        let gap = 0.075;
        sac.scale(
            b,
            [a + gap, a + TAU / petals as f32 - gap],
            [base, 1.02],
            bands,
            1.0,
            0.07,
        );
    }
    // The lid, clear of the petals' tops by a ring of light.
    hide(b);
    sac.dome(b, [1.14, FRAC_PI_2], 1.05, 2, 0.0, 0);
    metal(b);
    b.prism(
        sac.c.with_z(sac.c.z + sac.r.z * 1.04),
        b.sides(8),
        1.3,
        0.9,
        0.35,
    );
    for dx in [-2.6f32, 2.6] {
        sac.strap(b, sac.c.x + dx, 0.85);
    }
}

/// Mass: a dark dense sac, bare metal under three tiers of heavy scales, two straps.
fn mass_chamber(b: &mut MeshBuilder) {
    let sac = MASS;
    let base = sac.base();
    metal(b);
    sac.dome(
        b,
        [base, FRAC_PI_2],
        1.0,
        if b.fine() { 7 } else { 4 },
        0.02,
        5,
    );
    let tiers: [([f32; 2], usize, f32); 3] = [
        ([base, 0.42], 9, 0.0),
        ([0.34, 0.8], 8, 0.5),
        ([0.72, 1.12], 6, 0.2),
    ];
    hide(b);
    for (lat, count, turn) in tiers {
        for k in 0..count {
            let a = TAU * (k as f32 + turn) / count as f32;
            let gap = 0.05;
            sac.scale(
                b,
                [a + gap, a + TAU / count as f32 - gap],
                lat,
                if b.fine() { 2 } else { 1 },
                1.01,
                0.13,
            );
        }
    }
    hide(b);
    sac.dome(b, [1.2, FRAC_PI_2], 1.1, 2, 0.0, 0);
    for dx in [-3.0f32, 3.0] {
        sac.strap(b, sac.c.x + dx, 1.0);
    }
}

/// The girdle over the waist between the two sacs: bare vertebrae along y with plates
/// between them, gripping the ground at both ends; the team's mark on its top.
fn girdle(b: &mut MeshBuilder) {
    let x = -0.2;
    let n = if b.fine() { 9 } else { 5 };
    let pts: Vec<Vec3> = (0..n)
        .map(|k| {
            let y = -10.0 + 20.0 * k as f32 / (n - 1) as f32;
            // Over the petals and scales, not the skin under them.
            let skin = ENERGY.height(x, y, 1.1).max(MASS.height(x, y, 1.16));
            Vec3::new(x, y, (skin + 0.55).max(0.5))
        })
        .collect();
    under_hide(b);
    let spine: Vec<(Vec3, f32, f32)> = pts.iter().map(|&p| (p, 0.75, 0.55)).collect();
    limb(b, &spine, Vec3::Z);
    for (k, w) in pts.windows(2).enumerate() {
        hide(b);
        let up = Vec3::Z * 0.25;
        plate(
            b,
            &[
                (w[0].lerp(w[1], 0.15) + up, 1.0, 0.6),
                (w[0].lerp(w[1], 0.85) + up, 1.0, 0.6),
            ],
            Vec3::Z,
        );
        // Joints only where they stand clear of the ground.
        if k > 0 && w[0].z > 1.0 {
            if k % 2 == 0 {
                knuckle(b, w[0], Vec3::Y, 0.75, 1.2);
            } else {
                band(b, w[0], Vec3::Y, 0.7, 0.6);
            }
        }
    }
    for end in [pts[0], pts[n - 1]] {
        grip(b, end, Vec3::Y * end.y.signum(), 1.6, 1.1);
    }
    let middle = pts[n / 2];
    b.paint(TEAM);
    b.plate(middle + Vec3::Z * 0.8, Vec2::new(1.4, 1.6), 0.2, 0.08);
}

#[cfg(test)]
mod tests {
    use crate::models::{build_model_scaled, part};

    #[test]
    fn taproot() {
        super::super::check("naga_taproot", 12.8, 11.0, Some(3), &[]);
        // Built on water it stands on prop roots, its grip on the ground left out, and
        // its rams draw on the bulb.
        let model = build_model_scaled("naga_taproot", 12.8, 11.0, 1).unwrap();
        assert!(
            model.pit.is_some_and(|p| p.afloat_lift > 0.0),
            "no afloat lift"
        );
        for lod in 0..2 {
            for kind in [part::AFLOAT, part::ASHORE] {
                assert!(
                    model.lods[lod].vertices.iter().any(|v| v.part == kind),
                    "lod {lod}: no part {kind}"
                );
            }
        }
        assert!(
            model.lods[0].vertices.iter().any(|v| v.part == part::PUMP),
            "no rams"
        );
    }

    #[test]
    fn heart() {
        super::super::check("naga_heart", 6.9, 7.5, Some(2), &[]);
    }

    #[test]
    fn cyst() {
        super::super::check("naga_cyst", 12.9, 8.0, Some(3), &[]);
    }
}
