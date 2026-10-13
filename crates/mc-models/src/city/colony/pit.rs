//! A gun pit (`PropKind::ColonyGunPit`): rebel-built from what the colony had to
//! hand, a revetted wall on three sides of a gun, open at the back (-y). Its walls
//! are the plan's three parts, 2 m high; a unit stands in it, so its floor is left
//! clear.

use glam::{Vec2, Vec3};

use super::*;
use crate::builder::{hash_unit, MeshBuilder};
use crate::gpu_consts::city as pat;

/// The walls' middle lines: `SIDE` out either side of x, the front `FRONT` out along
/// y, open at `BACK`.
const SIDE: f32 = 7.0;
const FRONT: f32 = 7.0;
const BACK: f32 = -7.0;

/// A ring of sandbags in three stepped courses round prefab panels lining its inside,
/// a course of bags along its top, ammunition crates behind it.
pub(super) fn sandbags(b: &mut MeshBuilder, _tech: u8) {
    if !b.mid() {
        sweep(b, &line(0.0), -0.4, 1.95, 2.8, 1.5, pat::SANDBAG);
        return;
    }
    let ring = line(1.6);
    for (z0, z1, w0, w1) in [
        (-0.4, 0.75, 2.8, 2.5),
        (0.75, 1.45, 2.3, 2.0),
        (1.45, 1.95, 1.8, 1.5),
    ] {
        sweep(b, &ring, z0, z1, w0, w1, pat::SANDBAG);
    }
    // Salvaged prefab panels lining its inside, a few in an accent colour.
    let inner = 1.3;
    let mut k = 0;
    for (a, c) in [
        (
            v3(-SIDE + inner, BACK + 0.6, 0.0),
            v3(-SIDE + inner, FRONT - 1.8, 0.0),
        ),
        (
            v3(-SIDE + 1.8, FRONT - inner, 0.0),
            v3(SIDE - 1.8, FRONT - inner, 0.0),
        ),
        (
            v3(SIDE - inner, FRONT - 1.8, 0.0),
            v3(SIDE - inner, BACK + 0.6, 0.0),
        ),
    ] {
        let len = (c - a).length();
        let along = (c - a) / len;
        // Into the pit: right of the way the walls run.
        let into = v3(along.y, -along.x, 0.0);
        for t in cells(0.0, len, 2.3) {
            let at = a + along * t;
            let lean = 0.12 + 0.1 * hash_unit(17, k);
            let look = if hash_unit(29, k) > 0.75 {
                pat::ACCENT
            } else {
                pat::PREFAB
            };
            // Its foot out on the floor, its head into the bags.
            slab(
                b,
                at + into * lean,
                at - into * 0.15 + Vec3::Z * 1.55,
                along,
                1.08,
                0.12,
                look,
            );
            k += 1;
        }
    }
    if !b.fine() {
        return;
    }
    bag_course(b, &ring, 1.95, 0.95);
    crates(b);
}

/// Walls of cast concrete blocks in two staggered courses, faced outside with
/// salvaged prefab panels leant against them, sandbags along their top, ammunition
/// crates behind.
pub(super) fn blocks(b: &mut MeshBuilder, _tech: u8) {
    if !b.mid() {
        sweep(b, &line(0.0), -0.3, 2.05, 1.4, 1.1, pat::CONCRETE);
        return;
    }
    let thick = 1.1;
    let h = thick * 0.5;
    // The runs of block: up the west side, across the front over both corners, down
    // the east side.
    let runs = [
        (v3(-SIDE, BACK, 0.0), v3(-SIDE, FRONT - h, 0.0)),
        (v3(-SIDE - h, FRONT, 0.0), v3(SIDE + h, FRONT, 0.0)),
        (v3(SIDE, FRONT - h, 0.0), v3(SIDE, BACK, 0.0)),
    ];
    for (course, (z0, z1)) in [(-0.3, 0.6), (0.6, 1.45)].into_iter().enumerate() {
        for (a, c) in runs {
            let len = (c - a).length();
            let along = (c - a) / len;
            let across = v3(-along.y, along.x, 0.0);
            if !b.fine() {
                slab(
                    b,
                    (a + c) * 0.5 + Vec3::Z * z0,
                    (a + c) * 0.5 + Vec3::Z * z1,
                    along,
                    len * 0.5,
                    thick,
                    pat::CONCRETE,
                );
                continue;
            }
            // Blocks 1.9 m long, the upper course half a block along.
            let shift: f32 = if course == 1 { 0.95 } else { 0.0 };
            let mut t = 0.0f32;
            while t < len - 0.05 {
                let first = if t == 0.0 { 1.9 - shift } else { 1.9 };
                let l = first.min(len - t);
                let mid = a + along * (t + l * 0.5);
                let half = (along * (l - 0.1) + across * thick).abs() * 0.5;
                let c = mid + Vec3::Z * z0;
                boxed(
                    b,
                    c - v3(half.x, half.y, 0.0),
                    c + v3(half.x, half.y, z1 - z0),
                    pat::CONCRETE,
                );
                t += l;
            }
        }
    }
    let top = line(1.0);
    sweep(b, &top, 1.45, 1.85, 1.25, 1.05, pat::SANDBAG);
    if b.fine() {
        bag_course(b, &top, 1.85, 0.8);
    }
    // Salvaged panels leant against the outside, some in an accent colour, at odd
    // leans.
    let mut k = 0;
    for (a, c) in runs {
        let len = (c - a).length();
        let along = (c - a) / len;
        let out = v3(-along.y, along.x, 0.0);
        let face = h + 0.08;
        for t in cells(0.4, len - 0.8, 2.2) {
            if hash_unit(41, k) < 0.15 {
                k += 1;
                continue;
            }
            let at = a + along * t;
            let lean = 0.35 + 0.35 * hash_unit(43, k);
            let tall = 1.7 + 0.35 * hash_unit(47, k);
            let look = if hash_unit(53, k) > 0.7 {
                pat::ACCENT
            } else {
                pat::PREFAB
            };
            slab(
                b,
                at + out * (face + lean) + Vec3::Z * -0.2,
                at + out * face + Vec3::Z * tall,
                along,
                0.98,
                0.14,
                look,
            );
            k += 1;
        }
    }
    if b.fine() {
        crates(b);
    }
}

/// The walls' middle line, from the back of the west side round the front to the back
/// of the east side, its two front corners cut back `cut` metres.
fn line(cut: f32) -> Vec<Vec2> {
    let mut out = vec![Vec2::new(-SIDE, BACK)];
    if cut > 0.0 {
        out.extend([
            Vec2::new(-SIDE, FRONT - cut),
            Vec2::new(-SIDE + cut, FRONT),
            Vec2::new(SIDE - cut, FRONT),
            Vec2::new(SIDE, FRONT - cut),
        ]);
    } else {
        out.extend([Vec2::new(-SIDE, FRONT), Vec2::new(SIDE, FRONT)]);
    }
    out.push(Vec2::new(SIDE, BACK));
    out
}

/// A battered wall along `line`, from `z0` to `z1`, `w0` thick at its foot and `w1` at
/// its top, its corners mitred, its ends closed.
fn sweep(b: &mut MeshBuilder, line: &[Vec2], z0: f32, z1: f32, w0: f32, w1: f32, pattern: u32) {
    let n = line.len();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|i| {
            let into = (line[i.max(1)] - line[i.max(1) - 1]).normalize();
            let out = (line[(i + 1).min(n - 1)] - line[(i + 1).min(n - 1) - 1]).normalize();
            let left = (into + out).normalize().perp();
            let left = left / left.dot(into.perp()).max(0.3);
            let p = line[i];
            vec![
                (p - left * w0 * 0.5).extend(z0),
                (p + left * w0 * 0.5).extend(z0),
                (p + left * w1 * 0.5).extend(z1),
                (p - left * w1 * 0.5).extend(z1),
            ]
        })
        .collect();
    paint(b, pattern);
    b.loft(&rings, true, true);
}

/// A slab standing from the middle of its foot `foot` to the middle of its head
/// `head`, `half` either way `along`, `thick` through.
fn slab(
    b: &mut MeshBuilder,
    foot: Vec3,
    head: Vec3,
    along: Vec3,
    half: f32,
    thick: f32,
    pattern: u32,
) {
    let through = along.cross(head - foot).normalize() * thick * 0.5;
    let ring = |c: Vec3| -> Vec<Vec3> {
        let a = along * half;
        vec![
            c - a - through,
            c + a - through,
            c + a + through,
            c - a + through,
        ]
    };
    paint(b, pattern);
    b.loft(&[ring(foot), ring(head)], true, true);
}

/// A course of sandbags along `line` on a wall's top at `z`, end to end, each `wide`
/// across. Full detail.
fn bag_course(b: &mut MeshBuilder, line: &[Vec2], z: f32, wide: f32) {
    for pair in line.windows(2) {
        let (p, q) = (pair[0], pair[1]);
        let len = p.distance(q);
        let along = (q - p) / len;
        let (n, step) = grid(len, 0.85);
        for i in 0..n {
            let at = p + along * ((i as f32 + 0.5) * step);
            bag(b, at.extend(z), along.extend(0.0), step, wide, 0.24);
        }
    }
}

/// A sandbag lying at `at` (the middle of its underside) along `along`: `len` long,
/// `wide` across and `h` high, its ends drawn in.
fn bag(b: &mut MeshBuilder, at: Vec3, along: Vec3, len: f32, wide: f32, h: f32) {
    let across = Vec3::Z.cross(along);
    let ring = |t: f32, k: f32| -> Vec<Vec3> {
        let c = at + along * t + Vec3::Z * h * 0.5;
        let (s, u) = (across * wide * 0.5 * k, Vec3::Z * h * 0.5 * k.max(0.7));
        vec![c - s - u, c + s - u, c + s + u, c - s + u]
    };
    let e = len * 0.5;
    paint(b, pat::SANDBAG);
    b.loft(
        &[
            ring(-e, 0.75),
            ring(-e + 0.1, 1.0),
            ring(e - 0.1, 1.0),
            ring(e, 0.75),
        ],
        true,
        true,
    );
}

/// Ammunition crates stacked behind the wall's open ends. Full detail.
fn crates(b: &mut MeshBuilder) {
    for (x, y, z, look) in [
        (8.4, -8.3, 0.0, pat::ACCENT),
        (8.4, -8.3, 0.7, pat::STEEL),
        (9.7, -8.1, 0.0, pat::STEEL),
        (-8.6, -8.4, 0.0, pat::STEEL),
    ] {
        boxed(
            b,
            v3(x - 0.6, y - 0.4, z),
            v3(x + 0.6, y + 0.4, z + 0.7),
            look,
        );
        boxed(
            b,
            v3(x - 0.64, y - 0.44, z + 0.6),
            v3(x + 0.64, y + 0.44, z + 0.7),
            pat::TRIM,
        );
    }
}
