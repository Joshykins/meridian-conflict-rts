//! Naga factories: the Hatchery (air) and the Tidebrood (sea). The land factory is the
//! hall of muster (`hall`).
//!
//! Each is grown on its 8 x 8 lot (96 m square) round the lot origin, where the sim forms
//! the new unit, and opens toward +x, the way the finished unit leaves. Nothing stands on
//! the middle of the lot or in the lane out of it. They are rooted, not poured: plated
//! roots run out from their walls across the lot to its edges and corners.
//!
//! - The Hatchery: a crown of five tall ribbed talons hooked in over a launch cradle,
//!   the cradle a pad marked with the Naga eye, its pupil lit, and petals round it. Four
//!   plated buttress roots run from the talons to the lot's corners: an X under a crown
//!   from above.
//! - The Tidebrood: a ribcage grown out over the water. Two long hide arms either side of
//!   a flooded slip, low arched ribs across it on a spine of bare vertebrae, a low hump at
//!   the back with its eyes and lit throat at the waterline, rooted buttresses arching
//!   out from the arms into the sea. Its origin is on the water (a naval structure stands
//!   on the surface), and it keeps to the surface: nothing reaches under it.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;

use super::kit::*;

/// A cross-section of hide or plate (`kit::chitin_ring`, `kit::shell_ring`).
type Ring = fn(&MeshBuilder, Vec3, Vec3, Vec3, f32, f32) -> Vec<Vec3>;

/// Nothing goes under the ground (or, for the Tidebrood, under the water it stands on).
fn grounded(mut ring: Vec<Vec3>) -> Vec<Vec3> {
    for p in &mut ring {
        p.z = p.z.max(0.0);
    }
    ring
}

/// A course for hide to follow: the quadratic curve from `a` toward `c` to `d`, its half
/// width and half height running from the first value to the second. Keels turn away
/// from `inside`.
#[derive(Clone, Copy)]
struct Path {
    a: Vec3,
    c: Vec3,
    d: Vec3,
    w: [f32; 2],
    h: [f32; 2],
    inside: Vec3,
}

impl Path {
    fn at(&self, t: f32) -> Vec3 {
        let s = 1.0 - t;
        self.a * (s * s) + self.c * (2.0 * s * t) + self.d * (t * t)
    }

    fn tangent(&self, t: f32) -> Vec3 {
        ((self.c - self.a) * (1.0 - t) + (self.d - self.c) * t).normalize()
    }

    fn size(&self, t: f32) -> (f32, f32) {
        (
            self.w[0] + (self.w[1] - self.w[0]) * t,
            self.h[0] + (self.h[1] - self.h[0]) * t,
        )
    }

    /// Across and up (toward the keel) at `t`.
    fn frame(&self, t: f32) -> (Vec3, Vec3) {
        frame(self.tangent(t), self.at(t) - self.inside)
    }

    /// The stretch from `t0` to `t1` in `n` steps, its cross-section `scale` times the path's.
    fn loft(&self, b: &mut MeshBuilder, t0: f32, t1: f32, n: usize, ring: Ring, scale: f32) {
        let rings: Vec<Vec<Vec3>> = (0..=n)
            .map(|i| {
                let t = t0 + (t1 - t0) * i as f32 / n as f32;
                let (side, up) = self.frame(t);
                let (w, h) = self.size(t);
                grounded(ring(b, self.at(t), side, up, w * scale, h * scale))
            })
            .collect();
        b.loft(&rings, true, true);
    }
}

/// Soft hide along `path` (its section `core` times the path's) under `plates` armour
/// plates that stop short of each other, the hide showing between them. At full detail a
/// bare metal collar rings the hide in each gap when `collars`.
fn plated(b: &mut MeshBuilder, path: &Path, plates: usize, core: f32, collars: bool) {
    let fine = b.fine();
    under_hide(b);
    path.loft(
        b,
        0.0,
        1.0,
        if fine { plates * 2 } else { plates + 1 },
        chitin_ring,
        core,
    );
    hide(b);
    let p = plates as f32;
    for k in 0..plates {
        let (t0, t1) = ((k as f32 + 0.07) / p, (k as f32 + 0.93) / p);
        path.loft(b, t0, t1, if fine { 2 } else { 1 }, shell_ring, core * 1.18);
    }
    if fine && collars {
        metal(b);
        for k in 1..plates {
            let t = k as f32 / p;
            path.loft(b, t - 0.025 / p, t + 0.025 / p, 1, chitin_ring, core * 1.1);
        }
    }
}

/// A spike off the end of `path`, carrying on the way it runs, `length` metres.
fn tip(b: &mut MeshBuilder, path: &Path, length: f32, width: f32) {
    let end = path.at(1.0);
    hide(b);
    spike(
        b,
        end - path.tangent(1.0) * 0.4,
        end + path.tangent(1.0) * length,
        width,
    );
}

/// A plated root from `from` (high on the building) down and out along the ground to `to`,
/// `w` metres across where it leaves the building, flat and tapering, bowed `bend` metres
/// to its left on the way.
fn root(b: &mut MeshBuilder, from: Vec3, to: Vec2, w: f32, bend: f32, plates: usize) {
    let h = [w * 0.34, w * 0.12];
    let end = to.extend(h[1]);
    let mid = (from + end) * 0.5;
    let left = Vec3::Z.cross(end - from).normalize_or_zero() * bend;
    let path = Path {
        a: from,
        c: v3(mid.x, mid.y, h[0] * 1.2) + left,
        d: end,
        w: [w, w * 0.3],
        h,
        inside: mid - Vec3::Z * 1.0e4,
    };
    plated(b, &path, plates, 1.0, false);
}

/// An armour hoop over a hide body that runs along x: the arch over the top of its
/// cross-section at `at` (half width `w`, half height `h`), from foot to foot, `half`
/// metres either side of `at.x` and `thick` deep.
fn hoop(b: &mut MeshBuilder, at: Vec3, w: f32, h: f32, half: f32, thick: f32) {
    let shape: &[[f32; 2]] = if b.fine() {
        &[
            [-0.72, -0.85],
            [-1.0, -0.25],
            [-0.78, 0.5],
            [0.0, 1.0],
            [0.78, 0.5],
            [1.0, -0.25],
            [0.72, -0.85],
        ]
    } else {
        &[
            [-0.8, -0.75],
            [-1.0, -0.4],
            [-0.55, 0.7],
            [0.55, 0.7],
            [1.0, -0.4],
            [0.8, -0.75],
        ]
    };
    let points: Vec<Vec3> = shape
        .iter()
        .map(|[s, u]| at + v3(0.0, s * w * 1.05, u * h * 1.05))
        .collect();
    let n = points.len();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|i| {
            let dir = points[(i + 1).min(n - 1)] - points[i.saturating_sub(1)];
            let (side, up) = frame(dir, points[i] - at);
            grounded(shell_ring(b, points[i], side, up, half, thick))
        })
        .collect();
    b.loft(&rings, true, true);
}

/// A hide body running along x through `points` (x, half width, half height), its belly
/// on the ground, under armour hoops at `hoops` (x), each `half` metres either side.
fn hooped_body(
    b: &mut MeshBuilder,
    y: f32,
    points: &[(f32, f32, f32)],
    hoops: &[f32],
    half: f32,
    thick: f32,
) {
    under_hide(b);
    let spine: Vec<(Vec3, f32, f32)> = points
        .iter()
        .map(|&(x, w, h)| (v3(x, y, h), w, h))
        .collect();
    segment(b, &spine, Vec3::Z);
    hide(b);
    for &x in hoops {
        let k = points
            .windows(2)
            .position(|p| x <= p[1].0)
            .unwrap_or(points.len() - 2);
        let (p, q) = (points[k], points[k + 1]);
        let f = ((x - p.0) / (q.0 - p.0)).clamp(0.0, 1.0);
        let (w, h) = (p.1 + (q.1 - p.1) * f, p.2 + (q.2 - p.2) * f);
        hoop(b, v3(x, y, h), w, h, half, thick);
    }
}

/// A pair of red eye slits on a face looking along +x at `x`, `gap` metres apart about
/// the middle, their outer ends raised.
fn eyes(b: &mut MeshBuilder, x: f32, z: f32, gap: f32, length: f32) {
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        let inner = v3(x, gap * 0.5, z);
        b.beam(
            inner,
            inner + v3(0.0, length, length * 0.32),
            Vec2::new(0.4, 1.1),
            Vec2::new(0.4, 0.4),
        );
    });
}

// ---- Hatchery: air factory -------------------------------------------------------

/// The arms' middle line either side of the slip, and the ribs across it: (x, the top of
/// the arch), highest at the back.
const TIDE_ARM_Y: f32 = 25.5;
const TIDE_RIBS: [(f32, f32); 6] = [
    (-24.0, 18.2),
    (-13.0, 18.0),
    (-2.0, 17.4),
    (9.0, 16.5),
    (20.0, 15.2),
    (31.0, 13.4),
];
/// The hump at the back: (x, half width, half height) down its length, on the water; its
/// face is the last station and its back the first, both upright.
const TIDE_HUMP: [(f32, f32, f32); 7] = [
    (-46.5, 12.0, 3.6),
    (-45.0, 17.0, 3.6),
    (-41.0, 22.0, 6.0),
    (-36.0, 23.5, 6.6),
    (-30.5, 19.0, 5.6),
    (-28.5, 16.0, 5.0),
    (-27.0, 15.0, 5.0),
];

pub(super) fn tidebrood(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        tidebrood_coarse(b);
        return;
    }
    let fine = b.fine();
    b.mirror_y(|b| {
        tide_arm(b);
        for (x, top) in TIDE_RIBS {
            tide_rib(b, x, top);
        }
    });
    tide_spine(b);
    let hoops: &[f32] = if fine {
        &[-44.0, -38.5, -33.0]
    } else {
        &[-40.0, -32.0]
    };
    hooped_body(b, 0.0, &TIDE_HUMP, hoops, if fine { 2.4 } else { 3.4 }, 0.8);
    let (x, _, h) = TIDE_HUMP[TIDE_HUMP.len() - 1];
    eyes(b, x + 0.1, h + 1.6, 4.4, 3.4);
    // The throat at the waterline the young slip out of, lit.
    b.paint(GLOW_LASER);
    b.beam(
        v3(x + 0.1, -5.5, 1.3),
        v3(x + 0.1, 5.5, 1.3),
        Vec2::new(0.3, 0.9),
        Vec2::new(0.3, 0.9),
    );
    b.paint(TEAM);
    b.beam(
        v3(-40.5, 0.0, 14.95),
        v3(-35.5, 0.0, 14.95),
        Vec2::new(2.0, 0.3),
        Vec2::new(2.0, 0.3),
    );
}

/// Far off: the arms, the hump, the ribs as strips across the slip, the owner's strip.
fn tidebrood_coarse(b: &mut MeshBuilder) {
    hide(b);
    b.frustum_open(
        v3(-37.0, 0.0, 0.0),
        Vec2::new(21.0, 44.0),
        Vec2::new(12.0, 30.0),
        13.4,
        Vec2::ZERO,
    );
    b.mirror_y(|b| {
        b.frustum_open(
            v3(3.0, TIDE_ARM_Y, 0.0),
            Vec2::new(82.0, 15.0),
            Vec2::new(76.0, 7.0),
            6.4,
            Vec2::ZERO,
        );
    });
    for (x, top) in TIDE_RIBS {
        b.face(&[
            v3(x - 1.5, -20.0, top - 1.0),
            v3(x + 1.5, -20.0, top - 1.0),
            v3(x + 1.5, 20.0, top - 1.0),
            v3(x - 1.5, 20.0, top - 1.0),
        ]);
    }
    under_hide(b);
    b.face(&[
        v3(-27.0, -1.2, 17.2),
        v3(31.0, -1.2, 12.4),
        v3(31.0, 1.2, 12.4),
        v3(-27.0, 1.2, 17.2),
    ]);
    b.paint(TEAM);
    b.face(&[
        v3(-41.0, -1.2, 13.45),
        v3(-35.0, -1.2, 13.45),
        v3(-35.0, 1.2, 13.45),
        v3(-41.0, 1.2, 13.45),
    ]);
}

/// One arm: a long hide body lying on the water under armour hoops, a prow at its front,
/// buttresses arching out from its back into the sea, rooted where they meet it.
fn tide_arm(b: &mut MeshBuilder) {
    let fine = b.fine();
    let hoops: Vec<f32> = (0..8).map(|k| -30.0 + k as f32 * 9.0).collect();
    let (hoops, half) = if fine {
        (hoops, 2.6)
    } else {
        (hoops.iter().step_by(2).copied().collect(), 4.0)
    };
    hooped_body(
        b,
        TIDE_ARM_Y,
        &[
            (-40.0, 6.0, 3.0),
            (-28.0, 7.2, 3.3),
            (4.0, 7.2, 3.2),
            (30.0, 6.2, 2.9),
            (38.0, 4.6, 2.4),
            (41.0, 2.6, 1.6),
        ],
        &hoops,
        half,
        0.7,
    );
    // The prow: a plated tusk curling in toward the mouth of the slip.
    let prow = Path {
        a: v3(38.0, 23.0, 2.6),
        c: v3(44.5, 22.5, 3.0),
        d: v3(46.0, 17.5, 1.4),
        w: [2.0, 0.8],
        h: [1.8, 0.7],
        inside: v3(40.0, 17.0, 0.0),
    };
    plated(b, &prow, 2, 0.72, true);
    tip(b, &prow, 2.4, 0.6);
    let plates = if fine { 2 } else { 1 };
    for x in [-15.0f32, 17.0] {
        let buttress = Path {
            a: v3(x, 29.5, 5.6),
            c: v3(x, 40.0, 9.5),
            d: v3(x, 45.0, 1.4),
            w: [3.6, 2.6],
            h: [3.0, 2.0],
            inside: v3(x, 37.0, -8.0),
        };
        plated(b, &buttress, plates, 0.75, true);
        // Where it meets the sea it spreads in a plated foot.
        hide(b);
        b.prism(v3(x, 44.0, 0.0), b.sides(8), 3.4, 2.5, 1.4);
        if fine {
            ram(
                b,
                v3(x + 3.5, 31.5, 5.2),
                buttress.at(0.45) + v3(1.6, 0.0, 0.0),
                0.5,
            );
            ram(
                b,
                v3(x - 3.5, 31.5, 5.2),
                buttress.at(0.45) - v3(1.6, 0.0, 0.0),
                0.5,
            );
        }
    }
    root(
        b,
        v3(-40.0, 20.0, 4.0),
        Vec2::new(-46.5, 45.0),
        3.6,
        3.0,
        plates,
    );
}

/// Half a rib at `x`: out of the arm's inner shoulder, up and over the slip to the
/// spine at `top`, under two plates.
fn tide_rib(b: &mut MeshBuilder, x: f32, top: f32) {
    let rib = Path {
        a: v3(x, 19.2, 4.8),
        c: v3(x, 18.8, top),
        d: v3(x, 2.2, top),
        w: [2.5, 1.6],
        h: [2.2, 1.4],
        inside: v3(x, 6.0, 4.0),
    };
    plated(b, &rib, 2, 0.75, false);
    if b.fine() {
        // A ram from the arm's back up under the rib's shoulder.
        ram(
            b,
            v3(x + 1.6, 27.5, 6.2),
            rib.at(0.28) + v3(1.1, 0.5, 0.0),
            0.45,
        );
        b.paint(GLOW_LASER);
        let seam = |t: f32| rib.at(t) - rib.frame(t).1 * (rib.size(t).1 * 0.76);
        b.beam(
            seam(0.45),
            seam(0.9),
            Vec2::new(0.35, 0.1),
            Vec2::new(0.3, 0.1),
        );
    }
}

/// The spine along the top of the ribs: a bare vertebra at each rib's crown, plated keels
/// between them, a cord of metal the length of it, and its back end let down onto the hump.
fn tide_spine(b: &mut MeshBuilder) {
    let fine = b.fine();
    for (i, &(x, top)) in TIDE_RIBS.iter().enumerate() {
        metal(b);
        b.chamfered_box(v3(x, 0.0, top - 0.1), v3(4.0, 5.2, 2.6), 0.9);
        let next = TIDE_RIBS.get(i + 1).copied();
        if let Some((x1, top1)) = next {
            hide(b);
            let keel = |f: f32, w: f32, h: f32| {
                (
                    v3(x + (x1 - x) * f, 0.0, top + (top1 - top) * f + 0.9),
                    w,
                    h,
                )
            };
            shell(
                b,
                &[
                    keel(0.22, 1.6, 0.9),
                    keel(0.5, 2.0, 1.1),
                    keel(0.78, 1.6, 0.9),
                ],
                Vec3::Z,
            );
            if fine {
                metal(b);
                b.cylinder_between(
                    v3(x, 0.0, top - 0.5),
                    v3(x1, 0.0, top1 - 0.5),
                    0.55,
                    0.55,
                    6,
                );
                blade(
                    b,
                    v3(x - 1.2, 0.0, top + 1.1),
                    v3(x - 4.0, 0.0, top + 2.4),
                    0.6,
                    Vec3::Y,
                );
            }
        }
    }
    // Down from the first rib's crown to the hump's back.
    let (x, top) = TIDE_RIBS[0];
    let tail = Path {
        a: v3(x - 1.5, 0.0, top - 0.3),
        c: v3(x - 5.0, 0.0, top - 0.6),
        d: v3(-33.0, 0.0, 12.4),
        w: [1.8, 1.5],
        h: [1.3, 1.1],
        inside: v3(-30.0, 0.0, 0.0),
    };
    plated(b, &tail, 2, 0.8, true);
}

#[cfg(test)]
mod tests {
    #[test]
    fn tidebrood_fits_its_lot() {
        super::super::check("naga_tidebrood", 46.0, 20.0, Some(8), &[]);
    }
}
