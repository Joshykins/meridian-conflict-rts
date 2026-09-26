//! Naga factories: the Brood (land), the Hatchery (air) and the Tidebrood (sea).
//!
//! Each is grown on its 8 x 8 lot (96 m square) round the lot origin, where the sim forms
//! the new unit, and opens toward +x, the way the finished unit leaves. Nothing stands on
//! the middle of the lot or in the lane out of it. They are rooted, not poured: plated
//! roots run out from their walls across the lot to its edges and corners.
//!
//! - The Brood: a pit of molten hide floored with a lit iris, walled either side by long
//!   hide bodies under armour hoops, three pairs of jointed claws curling over it on rams,
//!   and at the back a hunched carapace, a brood mother's head, its two red eye slits
//!   watching the pit. Tusks either side of the way out.
//! - The Hatchery: a crown of five tall ribbed talons hooked in over a launch cradle,
//!   the cradle a pad marked with the Naga eye, its pupil lit, and petals round it. Four
//!   plated buttress roots run from the talons to the lot's corners: an X under a crown
//!   from above, where the Brood is a U.
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

    /// The keel's top at `t`, `scale` times the path's half height out from the middle.
    fn keel(&self, t: f32, scale: f32) -> Vec3 {
        self.at(t) + self.frame(t).1 * (self.size(t).1 * scale)
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

// ---- Brood: land factory ---------------------------------------------------------

/// The walls' middle line, and the claws' stations along them.
const BROOD_WALL_Y: f32 = 27.0;
const BROOD_CLAWS: [f32; 3] = [-10.0, 6.0, 22.0];
/// The brood mother's head: (x, half width, half height) down its length, belly on the
/// ground; its face is the last station and its back the first, both upright (the two at
/// either end stand the same height).
const BROOD_HEAD: [(f32, f32, f32); 8] = [
    (-46.5, 14.0, 6.5),
    (-45.0, 18.5, 6.5),
    (-40.0, 23.5, 9.0),
    (-34.0, 25.5, 10.4),
    (-26.0, 24.5, 10.3),
    (-20.5, 20.0, 8.6),
    (-18.0, 15.5, 7.2),
    (-16.5, 13.5, 7.2),
];

pub(super) fn brood(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        brood_coarse(b);
        return;
    }
    brood_floor(b);
    b.mirror_y(|b| {
        brood_wall(b);
        for x in BROOD_CLAWS {
            brood_claw(b, x);
        }
        brood_roots(b);
    });
    brood_head(b);
}

/// Far off: the head, the walls, the claws as fins, the eyes and the owner's strip.
fn brood_coarse(b: &mut MeshBuilder) {
    hide(b);
    b.frustum_open(
        v3(-32.0, 0.0, 0.0),
        Vec2::new(30.0, 50.0),
        Vec2::new(14.0, 26.0),
        21.0,
        Vec2::new(0.0, 0.0),
    );
    b.mirror_y(|b| {
        hide(b);
        b.frustum_open(
            v3(5.0, BROOD_WALL_Y, 0.0),
            Vec2::new(58.0, 11.0),
            Vec2::new(52.0, 4.0),
            8.4,
            Vec2::ZERO,
        );
        for x in BROOD_CLAWS {
            let fin = [v3(x, 24.0, 7.0), v3(x, 16.0, 19.0), v3(x, 7.0, 13.5)];
            b.face(&fin);
            b.face(&[fin[0], fin[2], fin[1]]);
        }
    });
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        b.face(&[
            v3(-16.9, 2.5, 8.0),
            v3(-16.9, 6.5, 8.9),
            v3(-16.9, 6.5, 9.6),
            v3(-16.9, 2.5, 8.8),
        ])
    });
    b.paint(TEAM);
    b.face(&[
        v3(-36.0, -1.2, 21.05),
        v3(-28.0, -1.2, 21.05),
        v3(-28.0, 1.2, 21.05),
        v3(-36.0, 1.2, 21.05),
    ]);
}

/// The pit's floor: a lit iris of molten hide plates round the lot origin, flush with the
/// ground so what forms on it stands where it will walk off.
fn brood_floor(b: &mut MeshBuilder) {
    under_hide(b);
    b.prism(Vec3::ZERO, b.sides(12), 8.5, 8.2, 0.12);
    let petals = 10;
    for k in 0..petals {
        let a0 = (k as f32 + 0.06) / petals as f32 * std::f32::consts::TAU;
        let a1 = (k as f32 + 0.94) / petals as f32 * std::f32::consts::TAU;
        let dir = |a: f32, r: f32| v3(a.cos() * r, a.sin() * r, 0.0);
        if k % 2 == 0 {
            hide(b);
        } else {
            under_hide(b);
        }
        slab(
            b,
            [dir(a0, 9.2), dir(a1, 9.2), dir(a1, 18.5), dir(a0, 18.5)],
            Vec3::Z * 0.14,
        );
    }
    if b.fine() {
        // The veins between the petals, lit.
        b.paint(GLOW_LASER);
        for k in 0..petals {
            let a = k as f32 / petals as f32 * std::f32::consts::TAU;
            let d = v3(a.cos(), a.sin(), 0.0);
            b.beam(
                d * 9.4 + Vec3::Z * 0.06,
                d * 18.2 + Vec3::Z * 0.06,
                Vec2::new(0.35, 0.1),
                Vec2::new(0.15, 0.1),
            );
        }
        b.mirror_y(|b| {
            // The lit seam along the wall's foot.
            b.paint(GLOW_LASER);
            b.beam(
                v3(-16.0, 22.8, 0.3),
                v3(30.0, 22.8, 0.3),
                Vec2::new(0.3, 0.3),
                Vec2::new(0.3, 0.3),
            );
        });
    }
}

/// One side wall: a long hide body under armour hoops, tusks at its front end.
fn brood_wall(b: &mut MeshBuilder) {
    let hoops: Vec<f32> = (0..8).map(|k| -19.0 + k as f32 * 6.8).collect();
    let (hoops, half) = if b.fine() {
        (hoops, 2.4)
    } else {
        (hoops.iter().step_by(2).copied().collect(), 3.8)
    };
    hooped_body(
        b,
        BROOD_WALL_Y,
        &[
            (-25.0, 5.2, 4.3),
            (-10.0, 5.6, 4.4),
            (8.0, 5.6, 4.3),
            (24.0, 5.0, 3.9),
            (32.0, 3.8, 3.1),
            (35.0, 2.2, 2.0),
        ],
        &hoops,
        half,
        0.75,
    );
    // Tusks either side of the way out, curling in and down to dig into the ground.
    let tusk = Path {
        a: v3(31.0, 24.5, 3.6),
        c: v3(40.0, 24.0, 4.2),
        d: v3(44.5, 18.5, 2.0),
        w: [2.2, 0.9],
        h: [2.0, 0.8],
        inside: v3(36.0, 18.0, 0.0),
    };
    plated(b, &tusk, 2, 0.72, true);
    tip(b, &tusk, 3.2, 0.7);
    if b.fine() {
        // A cable along the wall's top from claw to claw.
        metal(b);
        for pair in BROOD_CLAWS.windows(2) {
            let (x0, x1) = (pair[0], pair[1]);
            cable(
                b,
                &[
                    v3(x0, 25.4, 8.2),
                    v3((x0 + x1) * 0.5, 25.8, 8.0),
                    v3(x1, 25.4, 8.2),
                ],
                0.35,
            );
        }
    }
}

/// A claw at `x`: three plated joints rising off the wall's inner shoulder, curling over
/// the pit and hooking down, a drum at its root, a pair of rams from the wall's back.
fn brood_claw(b: &mut MeshBuilder, x: f32) {
    let claw = Path {
        a: v3(x, 24.0, 7.6),
        c: v3(x, 20.0, 27.0),
        d: v3(x + 2.5, 6.5, 15.0),
        w: [3.2, 1.2],
        h: [2.8, 1.0],
        inside: v3(x, 13.0, 9.0),
    };
    plated(b, &claw, 3, 0.72, true);
    tip(b, &claw, 3.8, 0.85);
    knuckle(b, claw.a, Vec3::X, 2.7, 4.8);
    if b.fine() {
        // A second, smaller talon inside the hook.
        hide(b);
        let hook = claw.at(0.86);
        spike(b, hook, hook + v3(0.0, -2.2, -2.8), 0.5);
        for dx in [-2.3f32, 2.3] {
            let on = claw.at(0.3) + v3(dx * 0.6, 0.8, -0.4);
            ram(b, v3(x + dx, 30.0, 7.6), on, 0.6);
        }
    }
}

/// The side's roots: out from the wall's back to the lot edge, from the head to the rear
/// corner, from the wall's front end to the front corner.
fn brood_roots(b: &mut MeshBuilder) {
    let plates = if b.fine() { 2 } else { 1 };
    for x in [-6.0f32, 16.0] {
        root(
            b,
            v3(x, 31.0, 1.6),
            Vec2::new(x - 9.0, 46.5),
            3.6,
            -3.0,
            plates,
        );
    }
    root(
        b,
        v3(-36.0, 21.0, 2.4),
        Vec2::new(-45.0, 45.0),
        4.6,
        4.0,
        plates + 1,
    );
    root(
        b,
        v3(30.0, 29.5, 1.5),
        Vec2::new(45.5, 45.5),
        3.4,
        -3.0,
        plates,
    );
}

/// The brood mother's head at the back: a hunched hide mass under tergite plates, a brow
/// over two red eye slits looking down the pit, a crest down its back, the owner's strip
/// on top.
fn brood_head(b: &mut MeshBuilder) {
    let fine = b.fine();
    let tergites: &[f32] = if fine {
        &[-43.0, -37.5, -32.0, -26.5, -21.0]
    } else {
        &[-40.0, -31.0, -22.0]
    };
    hooped_body(
        b,
        0.0,
        &BROOD_HEAD,
        tergites,
        if fine { 2.4 } else { 3.8 },
        0.9,
    );
    // The brow, jutting over the face.
    let (x, w, h) = BROOD_HEAD[BROOD_HEAD.len() - 1];
    hide(b);
    hoop(b, v3(x - 0.2, 0.0, h + 0.5), w * 1.05, h * 1.05, 1.1, 0.8);
    eyes(b, x + 0.12, h + 1.2, 4.6, 5.4);
    if fine {
        // The face under the brow: mandible plates either side of a lit maw.
        b.mirror_y(|b| {
            hide(b);
            blade(b, v3(x, 6.0, 3.0), v3(x + 5.0, 4.0, 0.6), 1.1, Vec3::Y);
            metal(b);
            b.cylinder_between(v3(x - 1.0, 9.0, 11.0), v3(x - 1.0, 11.5, 12.0), 0.9, 0.9, 8);
        });
        b.paint(GLOW_LASER);
        b.beam(
            v3(x + 0.1, -3.0, 3.2),
            v3(x + 0.1, 3.0, 3.2),
            Vec2::new(0.3, 0.4),
            Vec2::new(0.3, 0.4),
        );
        // A crest of blades raked back down the spine.
        for &t in &[-43.0f32, -37.5, -26.5, -21.0] {
            let (_, _, h) = BROOD_HEAD
                .iter()
                .copied()
                .min_by(|p, q| (p.0 - t).abs().total_cmp(&(q.0 - t).abs()))
                .unwrap_or(BROOD_HEAD[0]);
            let top = h * 2.0 + 1.0;
            blade(
                b,
                v3(t + 1.0, 0.0, top),
                v3(t - 3.0, 0.0, top + 2.2),
                0.8,
                Vec3::Y,
            );
        }
        // Rams from the head's flanks to the walls' rear ends.
        b.mirror_y(|b| {
            ram(b, v3(-28.0, 20.0, 12.0), v3(-19.0, 25.0, 7.6), 0.7);
            ram(b, v3(-40.0, 18.0, 12.5), v3(-44.0, 28.0, 2.0), 0.7);
        });
    }
    b.paint(TEAM);
    b.beam(
        v3(-36.0, 0.0, 22.45),
        v3(-28.0, 0.0, 22.45),
        Vec2::new(2.4, 0.3),
        Vec2::new(2.4, 0.3),
    );
}

// ---- Hatchery: air factory -------------------------------------------------------

/// The talons' bearings round the cradle (degrees): none in front, where the way out is.
const TALONS: [f32; 7] = [45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0];

fn outward(degrees: f32) -> Vec3 {
    let a = degrees.to_radians();
    v3(a.cos(), a.sin(), 0.0)
}

fn talon(degrees: f32) -> Path {
    let u = outward(degrees);
    Path {
        a: u * 19.0 + Vec3::Z * 2.5,
        c: u * 25.5 + Vec3::Z * 20.0,
        d: u * 11.0 + Vec3::Z * 28.0,
        w: [5.2, 1.3],
        h: [4.6, 1.1],
        inside: u * 12.0 + Vec3::Z * 13.0,
    }
}

pub(super) fn hatchery(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        hatchery_coarse(b);
        return;
    }
    hatchery_cradle(b);
    for (i, &deg) in TALONS.iter().enumerate() {
        hatchery_talon(b, deg);
        if let Some(&next) = TALONS.get(i + 1) {
            hatchery_girdle(b, deg, next);
        }
    }
    let plates = if b.fine() { 4 } else { 2 };
    for deg in [45.0f32, 135.0, 225.0, 315.0] {
        let u = outward(deg);
        // The buttress: from high on the talon's back down to the lot's corner.
        root(
            b,
            u * 23.0 + Vec3::Z * 8.0,
            (u * 62.0).truncate(),
            3.8,
            0.0,
            plates,
        );
        if b.fine() {
            let brace = talon(deg);
            ram(b, u * 36.0 + Vec3::Z * 1.6, brace.at(0.3) + u * 2.6, 0.7);
        }
        b.paint(TEAM);
        let (at, along) = (u * 50.0 + Vec3::Z * 1.1, u * 2.6);
        b.beam(
            at - along,
            at + along,
            Vec2::new(1.5, 0.3),
            Vec2::new(1.1, 0.3),
        );
    }
    root(
        b,
        v3(-23.0, 0.0, 7.0),
        Vec2::new(-46.5, 0.0),
        3.4,
        0.0,
        plates,
    );
}

/// Far off: the talons as leaning pyramids, the buttresses flat on the ground, the pad
/// with its lit pupil and the owner's colour at each corner.
fn hatchery_coarse(b: &mut MeshBuilder) {
    hide(b);
    for deg in TALONS {
        let (u, s) = (outward(deg), outward(deg + 90.0));
        let base = [u * 17.5 + s * 3.0, u * 24.0, u * 17.5 - s * 3.0];
        let top = u * 8.0 + Vec3::Z * 31.0;
        b.loft(&[base.to_vec(), vec![top; 3]], false, false);
    }
    for deg in [45.0f32, 135.0, 225.0, 315.0] {
        let (u, s) = (outward(deg), outward(deg + 90.0));
        let z = Vec3::Z * 0.8;
        b.face(&[
            u * 22.0 - s * 4.0 + z,
            u * 62.0 - s * 1.2 + z,
            u * 62.0 + s * 1.2 + z,
            u * 22.0 + s * 4.0 + z,
        ]);
        b.paint(TEAM);
        let z = Vec3::Z * 0.9;
        b.face(&[
            u * 48.0 - s * 1.4 + z,
            u * 53.0 - s * 1.2 + z,
            u * 53.0 + s * 1.2 + z,
            u * 48.0 + s * 1.4 + z,
        ]);
        hide(b);
    }
    under_hide(b);
    let pad: Vec<Vec3> = (0..8)
        .map(|k| outward(k as f32 * 45.0 + 22.5) * 9.0 + Vec3::Z * 0.6)
        .collect();
    b.face(&pad);
    b.paint(GLOW_LASER);
    b.face(&[
        v3(-6.0, 0.0, 0.65),
        v3(0.0, -1.3, 0.65),
        v3(6.0, 0.0, 0.65),
        v3(0.0, 1.3, 0.65),
    ]);
}

/// The launch cradle at the lot origin: a raised pad marked with the Naga eye, its slit
/// pupil lit along the way out, and a ring of hide petals round it, open in front.
fn hatchery_cradle(b: &mut MeshBuilder) {
    let fine = b.fine();
    hide(b);
    b.prism(Vec3::ZERO, b.sides(16), 10.0, 9.2, 0.35);
    under_hide(b);
    b.prism(Vec3::Z * 0.35, b.sides(16), 8.6, 8.2, 0.3);
    b.paint(GLOW_LASER);
    slab(
        b,
        [
            v3(-6.5, 0.0, 0.66),
            v3(0.0, -1.4, 0.66),
            v3(6.5, 0.0, 0.66),
            v3(0.0, 1.4, 0.66),
        ],
        Vec3::Z * 0.08,
    );
    if fine {
        // The iris round the pupil: a lit ring.
        let n = 16;
        for k in 0..n {
            let (a, c) = (
                outward(k as f32 * 360.0 / n as f32),
                outward((k as f32 + 0.8) * 360.0 / n as f32),
            );
            b.beam(
                a * 7.4 + Vec3::Z * 0.68,
                c * 7.4 + Vec3::Z * 0.68,
                Vec2::new(0.25, 0.06),
                Vec2::new(0.25, 0.06),
            );
        }
    }
    let petals: &[f32] = &[62.0, 100.0, 140.0, 180.0, 220.0, 260.0, 298.0];
    for &deg in petals {
        let u = outward(deg);
        let petal = Path {
            a: u * 8.6 + Vec3::Z * 0.9,
            c: u * 11.8 + Vec3::Z * 1.6,
            d: u * 12.6 + Vec3::Z * 6.8,
            w: [1.4, 0.5],
            h: [1.2, 0.45],
            inside: u * 7.0 + Vec3::Z * 6.0,
        };
        plated(b, &petal, 1, 0.75, false);
        tip(b, &petal, 1.8, 0.35);
    }
}

/// A talon: a tall hide spire rising off the ground, bowed out and hooked in over the
/// cradle, under five plates, bare collars between them, a lit seam up its inside and a
/// hooked spike at its tip.
fn hatchery_talon(b: &mut MeshBuilder, deg: f32) {
    let path = talon(deg);
    let fine = b.fine();
    plated(b, &path, if fine { 4 } else { 2 }, 0.72, true);
    tip(b, &path, 4.0, 0.9);
    if fine {
        // The seam up its inner face: the hollow lit.
        b.paint(GLOW_LASER);
        let seam = |t: f32| path.at(t) - path.frame(t).1 * (path.size(t).1 * 0.74);
        for k in 0..5 {
            let (t0, t1) = (0.1 + k as f32 * 0.15, 0.22 + k as f32 * 0.15);
            b.beam(
                seam(t0),
                seam(t1),
                Vec2::new(0.45, 0.12),
                Vec2::new(0.4, 0.12),
            );
        }
        // Barbs off its back, raked up.
        for t in [0.35f32, 0.6] {
            let back = path.keel(t, 0.85);
            let (_, up) = path.frame(t);
            spike(b, back, back + up * 2.2 + path.tangent(t) * 1.6, 0.45);
        }
    }
}

/// The low girdle of hide round the talons' feet from bearing `a` to bearing `c`, and a
/// tendon strung between the two talons high up.
fn hatchery_girdle(b: &mut MeshBuilder, a: f32, c: f32) {
    let r = 18.0;
    let half = (c - a).to_radians() * 0.5;
    let ring = Path {
        a: outward(a) * r + Vec3::Z * 1.6,
        c: outward((a + c) * 0.5) * (r / half.cos()) + Vec3::Z * 1.6,
        d: outward(c) * r + Vec3::Z * 1.6,
        w: [2.0, 2.0],
        h: [1.6, 1.6],
        inside: Vec3::Z * -1.0e4,
    };
    plated(b, &ring, if b.fine() { 2 } else { 1 }, 1.0, false);
    if b.fine() {
        metal(b);
        let (p, q) = (talon(a).at(0.62), talon(c).at(0.62));
        let sag = (p + q) * 0.5 - Vec3::Z * 3.0;
        cable(b, &[p, p.lerp(sag, 0.6), sag, q.lerp(sag, 0.6), q], 0.4);
    }
}

// ---- Tidebrood: naval factory ----------------------------------------------------

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
    fn brood_fits_its_lot() {
        super::super::check("naga_brood", 46.0, 22.0, Some(8), &[]);
    }

    #[test]
    fn hatchery_fits_its_lot() {
        super::super::check("naga_hatchery", 46.0, 30.0, Some(8), &[]);
    }

    #[test]
    fn tidebrood_fits_its_lot() {
        super::super::check("naga_tidebrood", 46.0, 20.0, Some(8), &[]);
    }
}
