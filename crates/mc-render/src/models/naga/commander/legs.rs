//! The commander's legs: reverse-kneed and built heavy. Each bone is a bronze frame inside
//! a thick armour shell, with plates lapped over the shell and swept back into points, and
//! rams working in the gaps; the hip, knee, hock and ankle are great bronze drums that each
//! bone's ends sit on, so the leg stays whole however the shader bends it. The foot is a
//! broad armoured sole under a toe cap, held in a yoke round the ankle drum.
//!
//! Every piece rides its bone (`rig::THIGH`, `SHIN`, `TARSUS`, `FOOT`) and turns about that
//! bone's upper joint (`entity.wgsl` `hock_leg`): nothing reaches across a joint to the next
//! bone, and a drum centred on a joint is the same drum whichever way the joint turns.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, rig};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::plating::{joint, piston, side_plate};
use super::{ANKLE, FOOT, HIP, HOCK, KNEE};

/// The drums' radii: the hip, the knee, the hock (the biggest) and the ankle.
pub(super) const HIP_R: f32 = 1.2;
pub(super) const KNEE_R: f32 = 1.05;
pub(super) const HOCK_R: f32 = 1.25;
pub(super) const ANKLE_R: f32 = 0.85;
/// Half the leg's width across (y) at its drums, about the joints' line.
const HALF: f32 = 1.0;

/// The left leg (the right is drawn by mirroring it).
pub(super) fn leg(b: &mut MeshBuilder) {
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, thigh);
        b.with_limb(rig::SHIN, shin);
        b.with_limb(rig::TARSUS, tarsus);
        b.with_limb(rig::FOOT, foot);
    });
}

/// A bone in the leg's (x, z) plane from `a` to `c`: its unit direction, and the unit
/// normal on the side `toward` points to.
fn bone(a: Vec3, c: Vec3, toward: Vec2) -> (Vec2, Vec2) {
    let u = Vec2::new(c.x - a.x, c.z - a.z).normalize();
    let n = Vec2::new(-u.y, u.x);
    (u, if n.dot(toward) < 0.0 { -n } else { n })
}

fn xz(p: Vec3) -> Vec2 {
    Vec2::new(p.x, p.z)
}

/// A point in the leg's plane at `y` across.
fn at(p: Vec2, y: f32) -> Vec3 {
    v3(p.x, y, p.y)
}

/// A profile for `side_plate` from points in the (x, z) plane.
fn profile(points: &[Vec2]) -> Vec<[f32; 2]> {
    points.iter().map(|p| [p.x, p.y]).collect()
}

/// One cross-section of a bone's armour shell at `p` (leg plane), centred `y` across:
/// `half` either side, reaching `front` out along `n` and `back` out against it (a
/// negative `back` stops the shell short of the bone's line), its corners cut.
fn section(
    b: &MeshBuilder,
    p: Vec2,
    n: Vec2,
    y: f32,
    half: f32,
    front: f32,
    back: f32,
) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() {
        &[
            [1.0, 0.5],
            [0.62, 1.0],
            [-0.62, 1.0],
            [-1.0, 0.5],
            [-1.0, -0.5],
            [-0.62, -1.0],
            [0.62, -1.0],
            [1.0, -0.5],
        ]
    } else {
        &[[1.0, 0.8], [-1.0, 0.8], [-1.0, -0.8], [1.0, -0.8]]
    };
    shape
        .iter()
        .map(|&[s, t]| {
            let out = if t > 0.0 { front } else { back } * t;
            at(p + n * out, y + s * half)
        })
        .collect()
}

/// A bone's armour shell from joint `a` to joint `c`, stopping short of both drums: at
/// each of `stations` (share of the way along, half width, front, back) a section, centred
/// across where the bone is.
fn shell(b: &mut MeshBuilder, (a, c): (Vec3, Vec3), n: Vec2, stations: &[(f32, f32, f32, f32)]) {
    let rings: Vec<Vec<Vec3>> = stations
        .iter()
        .map(|&(t, half, front, back)| {
            let p = a.lerp(c, t);
            section(b, xz(p), n, p.y, half, front, back)
        })
        .collect();
    b.loft(&rings, true, true);
}

/// The thigh: the hip drum, a bronze beam to the knee inside a thick shell, a cuisse over
/// the outside lapped by a front plate, its top swept back past the hip into a point, and
/// a ram down its back.
fn thigh(b: &mut MeshBuilder) {
    let (a, c) = (xz(HIP), xz(KNEE));
    let (u, n) = bone(HIP, KNEE, Vec2::new(1.0, 1.0));
    let y = HIP.y;
    joint(b, HIP, Vec3::Y * (HALF + 0.05), HIP_R);
    metal(b);
    b.beam(HIP, KNEE, Vec2::new(1.4, 1.2), Vec2::new(1.2, 1.0));
    dark_plate(b);
    shell(
        b,
        (HIP, KNEE),
        n,
        &[
            (0.2, 0.9, 1.15, 1.0),
            (0.5, 0.95, 1.3, 1.05),
            (0.78, 0.82, 1.05, 0.85),
        ],
    );
    // The cuisse over the outside, its top swept back past the hip into a point.
    let (o0, o1) = (y + 0.95, y + 1.2);
    side_plate(
        b,
        &profile(&[
            a + n * 1.45 + u * 0.35,
            c + n * 1.2 - u * 0.75,
            c + n * 0.2 - u * 0.45,
            c - n * 0.95 - u * 0.95,
            a - n * 1.15 + u * 1.2,
            a - n * 1.9 - u * 0.9,
            a + n * 0.4 - u * 0.2,
        ]),
        o0,
        o1,
    );
    if b.fine() {
        // A front plate lapped over the cuisse's leading edge.
        side_plate(
            b,
            &profile(&[
                a + n * 1.35 + u * 0.9,
                c + n * 1.35 - u * 0.95,
                c + n * 1.5 - u * 1.45,
                a + n * 1.6 + u * 1.1,
            ]),
            y - 0.9,
            o1 + 0.05,
        );
        // Red in the seam between the cuisse and the front plate.
        b.paint(GLOW_LASER);
        b.beam(
            at(a + n * 1.28 + u * 1.5, o1 + 0.02),
            at(c + n * 1.1 - u * 1.4, o1 + 0.02),
            Vec2::new(0.06, 0.06),
            Vec2::new(0.06, 0.06),
        );
    }
    // The ram that swings the thigh, along its back under the cuisse's rim.
    if !b.fine() {
        return;
    }
    piston(
        b,
        at(a - n * 1.05 + u * 0.8, y - 0.6),
        at(c - n * 0.9 - u * 0.9, y - 0.6),
        0.24,
    );
}

/// The shin, the long bone that shows most: twin bronze struts from knee to hock with a
/// ram along their front, the knee drum under a knee cap, and a heavy greave over the back
/// lapped by a second down its middle.
fn shin(b: &mut MeshBuilder) {
    let (k, h) = (xz(KNEE), xz(HOCK));
    let (u, n) = bone(KNEE, HOCK, Vec2::new(-1.0, 1.0));
    let y = KNEE.y;
    joint(b, KNEE, Vec3::Y * (HALF + 0.1), KNEE_R);
    metal(b);
    let sides = b.sides(8);
    for dy in [-0.6f32, 0.6] {
        b.cylinder_between(KNEE + Vec3::Y * dy, HOCK + Vec3::Y * dy, 0.34, 0.3, sides);
    }
    // The ram in front of the struts, where the greave leaves them bare.
    if b.fine() {
        piston(
            b,
            at(k - n * 0.95 + u * 1.2, y),
            at(h - n * 0.85 - u * 1.2, y),
            0.3,
        );
    }
    dark_plate(b);
    // The greave: the back half of the shell, the struts and ram showing in front.
    shell(
        b,
        (KNEE, HOCK),
        n,
        &[
            (0.2, 0.95, 1.2, -0.25),
            (0.55, 1.0, 1.3, -0.2),
            (0.82, 0.88, 1.1, -0.2),
        ],
    );
    // The knee cap, standing up over the thigh's front plate and swept back at its top.
    side_plate(
        b,
        &profile(&[
            k + Vec2::new(0.85, 1.25),
            k + Vec2::new(1.4, 0.1),
            k + Vec2::new(0.95, -1.3),
            k + Vec2::new(0.2, -0.9),
            k + Vec2::new(0.1, 0.55),
            k + Vec2::new(-0.35, 2.0),
        ]),
        y - 0.85,
        y + 1.25,
    );
    if b.fine() {
        // A second greave down the middle of the back, lapped over the first, its lower
        // end swept back past the hock into a point.
        side_plate(
            b,
            &profile(&[
                k + n * 1.25 + u * 1.3,
                k + n * 1.75 + u * 1.7,
                h + n * 1.55 - u * 1.3,
                h + n * 1.45 + u * 0.6,
                h + n * 1.15 - u * 0.4,
            ]),
            y - 0.55,
            y + 0.55,
        );
        // Red lines either side of it.
        b.paint(GLOW_LASER);
        for dy in [-0.6f32, 0.6] {
            b.beam(
                at(k + n * 1.3 + u * 1.9, y + dy),
                at(h + n * 1.25 - u * 1.4, y + dy),
                Vec2::new(0.06, 0.06),
                Vec2::new(0.06, 0.06),
            );
        }
        // A cable down the outside strut.
        metal(b);
        b.cylinder_between(
            at(k - n * 0.4 + u * 0.9, y + 1.02),
            at(h - n * 0.35 - u * 0.9, y + 1.02),
            0.1,
            0.1,
            5,
        );
    }
}

/// The tarsus, hock to ankle: the hock, the biggest drum on the leg, a short massive block
/// down to the ankle under a front plate, and the heel spur, a plate swept back behind the
/// hock.
fn tarsus(b: &mut MeshBuilder) {
    let (h, a) = (xz(HOCK), xz(ANKLE));
    let (u, n) = bone(HOCK, ANKLE, Vec2::new(1.0, 1.0));
    let y = HOCK.y;
    joint(b, HOCK, Vec3::Y * (HALF + 0.15), HOCK_R);
    metal(b);
    b.beam(HOCK, ANKLE, Vec2::new(1.3, 1.3), Vec2::new(1.1, 1.1));
    dark_plate(b);
    shell(
        b,
        (HOCK, ANKLE),
        n,
        &[(0.3, 0.78, 1.05, 0.95), (0.7, 0.7, 0.95, 0.85)],
    );
    // The heel spur, swept back and down behind the hock drum.
    side_plate(
        b,
        &profile(&[
            h + Vec2::new(0.35, 0.95),
            h + Vec2::new(-0.9, 0.85),
            h + Vec2::new(-2.4, 0.15),
            h + Vec2::new(-0.95, -0.7),
            h + Vec2::new(0.3, -0.95),
        ]),
        y - 0.55,
        y + 0.55,
    );
    if b.fine() {
        // The front plate, lapped over the block's front.
        side_plate(
            b,
            &profile(&[
                h + n * 1.0 + u * 0.2,
                h + n * 1.25 + u * 0.5,
                a + n * 1.1 - u * 0.55,
                a + n * 0.85 - u * 0.4,
            ]),
            y - 0.85,
            y + 0.85,
        );
        piston(
            b,
            at(h - n * 1.0 + u * 0.6, y + 0.35),
            at(a - n * 0.95 - u * 0.5, y + 0.35),
            0.18,
        );
    }
}

/// The foot: a broad armoured sole with no toes, a toe cap lapped over its front and
/// pointed ahead, a heel plate swept back, and a yoke of two cheek plates rising round
/// the ankle drum.
fn foot(b: &mut MeshBuilder) {
    let (x, y, z) = (ANKLE.x, ANKLE.y, ANKLE.z);
    let (rear, front, width) = FOOT;
    let w = width * 0.5;
    joint(b, ANKLE, Vec3::Y * 0.95, ANKLE_R);
    // The sole: a dark slab, its outline pointed ahead and cut in behind.
    let plan = |k: f32| -> Vec<[f32; 2]> {
        vec![
            [x + front, y],
            [x + front - 1.1, y + w * k],
            [x + rear + 0.6, y + w * k],
            [x + rear, y + w * 0.6 * k],
            [x + rear, y - w * 0.6 * k],
            [x + rear + 0.6, y - w * k],
            [x + front - 1.1, y - w * k],
        ]
    };
    seam(b);
    b.extrude_z(&plan(1.0), 0.0, 0.35);
    // The foot's armour on the sole, drawn in as it rises.
    dark_plate(b);
    b.loft(
        &[
            plan(0.98).iter().map(|p| v3(p[0], p[1], 0.35)).collect(),
            plan(0.8)
                .iter()
                .map(|p| v3(x + (p[0] - x) * 0.8 - 0.1, p[1], 0.85))
                .collect(),
        ],
        true,
        true,
    );
    // The yoke: a cheek plate either side of the ankle drum, down onto the foot.
    for side in [-1.0f32, 1.0] {
        let y0 = y + side * 0.95;
        side_plate(
            b,
            &[
                [x - 1.3, 0.8],
                [x + 1.5, 0.8],
                [x + 0.9, z + 0.3],
                [x + 0.1, z + 0.95],
                [x - 0.8, z + 0.55],
            ],
            y0.min(y0 + side * 0.3),
            y0.max(y0 + side * 0.3),
        );
    }
    // The toe cap: a plate lapped over the front, pointed ahead and sloped down to it.
    let cap = |k: f32, zc: f32, dx: f32| -> Vec<Vec3> {
        [
            [front + 0.1, 0.0],
            [front - 1.2, w * 1.02],
            [0.9, w * 0.92],
            [0.9, -w * 0.92],
            [front - 1.2, -w * 1.02],
        ]
        .iter()
        .map(|p| v3(x + p[0] * k + dx, y + p[1] * k, zc))
        .collect()
    };
    b.loft(&[cap(1.0, 0.3, 0.0), cap(0.82, 0.95, -0.2)], true, true);
    if b.fine() {
        // A heel plate behind the yoke, swept back past the sole.
        side_plate(
            b,
            &[
                [x - 0.8, 1.2],
                [x + rear - 0.3, 0.45],
                [x + rear + 0.2, 0.25],
                [x - 0.2, 0.7],
            ],
            y - w * 0.55,
            y + w * 0.55,
        );
        // Red along the seam where the toe cap laps the foot.
        b.paint(GLOW_LASER);
        b.beam(
            v3(x + 1.0, y - w * 0.7, 0.9),
            v3(x + 1.0, y + w * 0.7, 0.9),
            Vec2::new(0.07, 0.07),
            Vec2::new(0.07, 0.07),
        );
    }
}
