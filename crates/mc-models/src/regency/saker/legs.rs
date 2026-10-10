//! The Saker's legs: four, splayed low and wide round the hull, their feet planted far
//! out at its corners. Posed by entity.wgsl `crawl_leg` (`set_crawl_legs`), and braced by
//! the deploy (`set_crawl_brace`): packed, the feet stand in and the hull rides higher;
//! planting, each leg steps out in turn and the hull settles low between them.
//!
//! Each leg turns in a joint drum at its hip under a plated cap. The thigh is a plated
//! core with ridged plates lapped down its back, the last swept past the knee into a
//! spike, a plate down each side and a plain graphite strut under it; the knee turns in
//! a second drum; the shin is plated the same way down its outer face; the ankle turns
//! in a third drum over the foot (`feet`), a spade and toes.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::{part, rig};

use super::super::commander::form::{ring, sleeve, OCT};
use super::super::kit::{dark_plate, metal, seam};
use super::super::machine::{armour, collar, Frame};
use super::feet;
use super::parts::drum;

/// One pair of legs as the rig takes it: the left leg's hip, knee and ankle, and where in
/// the cycle it lifts.
pub(super) type Joints = (Vec3, Vec3, Vec3, f32);

/// The legs: the front and back left legs, and their girths.
pub(super) struct Legs {
    pub(super) joints: [Joints; 2],
    /// Thigh and shin: (across, out) at the root, the middle and the end.
    thigh: [[f32; 2]; 3],
    shin: [[f32; 2]; 3],
    /// The hip and knee drums' radii.
    drums: [f32; 2],
}

/// Reaching out low to knees over the hull's corners.
pub(super) const LEGS: Legs = Legs {
    joints: [
        (
            Vec3::new(8.0, 7.2, 4.2),
            Vec3::new(15.0, 15.2, 9.8),
            Vec3::new(17.8, 19.2, 1.2),
            0.0,
        ),
        (
            Vec3::new(-8.5, 7.2, 4.2),
            Vec3::new(-15.5, 15.2, 9.8),
            Vec3::new(-18.2, 19.2, 1.2),
            0.25,
        ),
    ],
    thigh: [[1.35, 1.4], [1.45, 1.7], [1.15, 1.25]],
    shin: [[1.25, 1.35], [1.3, 1.5], [1.0, 1.05]],
    drums: [1.8, 1.45],
};

/// The walk, at the authored size: the ground one cycle covers, the share of it a foot
/// is down, and how high a foot lifts.
const STRIDE: f32 = 21.0;
const STANCE: f32 = 0.75;
const LIFT: f32 = 2.5;
/// Packed, how much higher the hull rides, and what share nearer its hips the feet stand.
pub(super) const SINK: f32 = 2.4;
pub(super) const WIDEN: f32 = 0.22;

/// The rig, the sockets and every leg, at the builder's level of detail.
pub(super) fn legs(b: &mut MeshBuilder, l: &Legs) {
    // The stride is the authored one at the size the model is built: the builder scales
    // the joints, not the stride.
    b.set_crawl_legs(&l.joints, STRIDE, STANCE, LIFT);
    let scale = b.legs().map_or(1.0, |g| g.hip[2] / l.joints[0].0.z);
    b.set_crawl_legs(
        &l.joints,
        MeshBuilder::fitted_stride(STRIDE * scale),
        STANCE,
        LIFT,
    );
    b.set_crawl_brace(SINK, WIDEN);
    b.mirror_y(|b| {
        for (i, &(hip, knee, ankle, _)) in l.joints.iter().enumerate() {
            if b.mid() {
                // The socket in the hull's flank the hip drum turns in.
                seam(b);
                b.chamfered_box(
                    Vec3::new(hip.x, hip.y - 0.7, hip.z),
                    Vec3::new(4.4, 1.8, 3.4),
                    0.7,
                );
            }
            b.with_pair(i, |b| {
                b.with_part(part::LOCOMOTION, |b| {
                    if b.coarse() {
                        coarse(b, hip, knee, ankle);
                    } else {
                        leg(b, l, hip, knee, ankle);
                    }
                })
            });
        }
    });
}

/// Far off: thigh and shin as open three-sided spikes.
fn coarse(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, ankle: Vec3) {
    dark_plate(b);
    let tri = |at: Vec3, to: Vec3, r: f32| -> Vec<Vec<Vec3>> {
        let d = (to - at).normalize();
        let side = d.any_orthonormal_vector();
        let up = d.cross(side);
        let ring = (0..3)
            .map(|k| {
                let a = std::f32::consts::TAU * k as f32 / 3.0;
                at + (side * a.cos() + up * a.sin()) * r
            })
            .collect();
        vec![ring, vec![to; 3]]
    };
    b.with_limb(rig::THIGH, |b| b.loft(&tri(hip, knee, 1.7), false, false));
    b.with_limb(rig::SHIN, |b| b.loft(&tri(knee, ankle, 1.5), false, false));
}

/// A bone from `from` to `to`: its outer face in the leg's plane (the side toward
/// `toward`).
fn outer(from: Vec3, to: Vec3, across: Vec3, toward: Vec3) -> Vec3 {
    let along = (to - from).normalize();
    let n = across.cross(along).normalize();
    if n.dot(toward) < 0.0 {
        -n
    } else {
        n
    }
}

/// One plated limb from `a` to `c`, its outer face `n`, `girth` (across, out) at its root,
/// middle and end: the plated core, `plates` ridged plates lapped down its outer face (the
/// last swept `past` beyond `c`), at full detail a plate down each side and a plain strut
/// along its inner face.
fn limb(
    b: &mut MeshBuilder,
    a: Vec3,
    c: Vec3,
    n: Vec3,
    girth: &[[f32; 2]; 3],
    plates: usize,
    past: f32,
) {
    let along = (c - a).normalize();
    let len = a.distance(c);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(a + along * 0.5, n, girth[0][0] * 0.85, girth[0][1] * 0.85),
            ring(
                a + along * len * 0.5,
                n,
                girth[1][0] * 0.85,
                girth[1][1] * 0.85,
            ),
            ring(c - along * 0.6, n, girth[2][0] * 0.85, girth[2][1] * 0.85),
        ],
        &OCT,
    );
    let fine = b.fine();
    let count = if fine { plates } else { 2 };
    let step = (len + past - 1.2) / count as f32;
    for k in 0..count {
        let s0 = 0.8 + step * k as f32;
        let t = ((s0 + step * 0.5) / len).min(1.0);
        let g = if t < 0.5 {
            girth[0][1] + (girth[1][1] - girth[0][1]) * t * 2.0
        } else {
            girth[1][1] + (girth[2][1] - girth[1][1]) * (t - 0.5) * 2.0
        };
        let w = girth[1][0] * 0.95;
        let l = step * 1.3;
        let last = k + 1 == count;
        let end = if last { 0.0 } else { w * 0.6 };
        dark_plate(b);
        armour(
            b,
            &Frame::new(a + along * s0 + n * (g * 0.8), along + n * 0.06, n),
            &[
                [0.0, -w],
                [0.0, w],
                [l * 0.7, w * 0.92],
                [l, end],
                [l, -end],
                [l * 0.7, -w * 0.92],
            ],
            0.32,
        );
    }
    if !fine {
        return;
    }
    let side = along.cross(n).normalize();
    for sign in [-1.0f32, 1.0] {
        let sd = side * sign;
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                a + along * (len * 0.22) + sd * (girth[1][0] * 0.82),
                along,
                sd,
            ),
            &[
                [0.0, -0.7],
                [0.0, 0.7],
                [len * 0.45, 0.6],
                [len * 0.58, 0.0],
                [len * 0.45, -0.6],
            ],
            0.24,
        );
    }
    // A plain graphite strut along the inner face, a collar at each end.
    let (s0, s1) = (
        a + along * (len * 0.15) - n * (girth[1][1] * 0.95),
        c - along * (len * 0.18) - n * (girth[2][1] * 0.95),
    );
    metal(b);
    b.cylinder_between(s0, s1, 0.24, 0.24, 6);
    for p in [s0, s1] {
        collar(b, p, s1 - s0, 0.38, 0.5);
    }
}

/// One leg: hip drum and its cap, the thigh, knee drum, shin, ankle drum and foot.
fn leg(b: &mut MeshBuilder, l: &Legs, hip: Vec3, knee: Vec3, ankle: Vec3) {
    let out = (ankle - hip).with_z(0.0).normalize();
    let across = out.cross(Vec3::Z);
    let tn = outer(hip, knee, across, Vec3::Z);
    let sn = outer(knee, ankle, across, out);
    let [hip_r, knee_r] = l.drums;
    b.with_limb(rig::THIGH, |b| {
        drum(b, hip, across, hip_r, 3.0);
        if b.mid() {
            // A plated cap arched over the hip drum.
            dark_plate(b);
            armour(
                b,
                &Frame::new(hip - out * 1.6 + Vec3::Z * (hip_r + 0.1), out, Vec3::Z),
                &[[0.0, -1.7], [0.0, 1.7], [2.4, 1.5], [3.4, 0.0], [2.4, -1.5]],
                0.36,
            );
        }
        limb(b, hip, knee, tn, &l.thigh, 3, 1.4);
    });
    b.with_limb(rig::SHIN, |b| {
        drum(b, knee, across, knee_r, 2.6);
        limb(b, knee, ankle, sn, &l.shin, 3, -0.6);
        drum(b, ankle, across, 0.85, 2.0);
    });
    b.with_limb(rig::FOOT, |b| feet::foot(b, ankle, out, across));
}
