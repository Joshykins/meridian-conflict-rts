//! The Colossus's pelvis and four legs. The pelvis is a faceted drum under the hull that
//! does not turn; each leg is born in it as a heavy shoulder and runs out under the hull's
//! sweep to a knee beyond it, then drops in a long shin to a heavy foot, the feet standing
//! far out to the sides, wider than the hull is long. Every leg is one continuous form: the
//! thigh a single loft whose sections swell into a great swept shell, bands of plate lapped
//! over it; the shin a keeled loft narrowing to the ankle; a plated knee joint between.
//! They step in turn (rear left, front right, rear right, front left), so three always stand.
//!
//! A [`Style`] sets where the joints stand and how thick the limbs are; the main model and
//! its variant differ only in theirs.

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::super::commander::form::{ring, sleeve, KEEL, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{collar, shaft};
use super::RACE;

/// A leg's rest joints (hip, knee, foot's tip) and the cycle's share at which it lifts.
pub(super) type Joints = (Vec3, Vec3, Vec3, f32);

/// Where a leg's joints stand and how thick its limbs are.
pub(super) struct Style {
    /// The hips' height and how far out from the middle they sit.
    pub(super) hip_z: f32,
    pub(super) hip_r: f32,
    /// The knee, out from the hip along the leg's plane and up from it.
    pub(super) knee_out: f32,
    pub(super) knee_up: f32,
    /// Where the left feet's tips stand (x, y): the front one ahead of the middle and the
    /// rear one behind it.
    pub(super) front_foot: (f32, f32),
    pub(super) rear_foot: (f32, f32),
    /// The thigh's half width and half depth at the shoulder and at the knee, the shin's at
    /// the knee and at the ankle.
    pub(super) thigh: [f32; 4],
    pub(super) shin: [f32; 4],
    /// The knee joint's radius, and the foot block's.
    pub(super) knee_r: f32,
    pub(super) foot_r: f32,
}

/// The main legs: high and arched, the knee well above and outside the hull.
pub(super) const ARCHED: Style = Style {
    hip_z: 29.0,
    hip_r: 7.2,
    knee_out: 20.5,
    knee_up: 8.0,
    front_foot: (15.0, 29.0),
    rear_foot: (-17.0, 29.0),
    thigh: [5.0, 4.2, 3.4, 3.0],
    shin: [3.6, 3.6, 2.1, 2.2],
    knee_r: 4.6,
    foot_r: 4.2,
};

/// The ground one walking cycle covers at the authored size, the share of it each foot
/// stands and how high a foot lifts (`crawl_leg`). The builder scales the joints and the
/// lift but not the stride, so `rig` scales it to the build.
pub(super) const STRIDE: f32 = 28.0;
pub(super) const STANCE: f32 = 0.75;
pub(super) const LIFT: f32 = 6.0;
/// Where in the cycle each pair lifts: the rear's left leg a quarter cycle before the
/// front's, so the feet swing in the order rear left, front right, rear right, front left,
/// each a quarter of the cycle.
const FRONT_PHASE: f32 = 0.0;
const REAR_PHASE: f32 = 0.25;

/// A leg at rest from its hip (at `hip_az` round the pelvis) to its foot's tip at `foot`, the
/// knee out and up from the hip, in the plane through hip and foot.
fn leg_joints(s: &Style, hip_az: f32, foot: (f32, f32), phase: f32) -> Joints {
    let a = hip_az.to_radians();
    let hip = v3(a.cos() * s.hip_r, a.sin() * s.hip_r, s.hip_z);
    let foot = v3(foot.0, foot.1, 0.0);
    let out = (foot - hip).with_z(0.0).normalize();
    let knee = hip + out * s.knee_out + Vec3::Z * s.knee_up;
    (hip, knee, foot, phase)
}

/// The front left leg and the rear left leg (the right ones are their mirrors).
pub(super) fn legs(s: &Style) -> [Joints; 2] {
    [
        leg_joints(s, 40.0, s.front_foot, FRONT_PHASE),
        leg_joints(s, 140.0, s.rear_foot, REAR_PHASE),
    ]
}

/// The walking rig, its stride grown with the build (168 m at the unit file's size, to the
/// nearest that loops with the gait, `fitted_stride`): a stride left at the authored size
/// has the long legs patter through tiny steps.
pub(super) fn rig(b: &mut MeshBuilder, s: &Style, legs: &[Joints; 2]) {
    b.set_crawl_legs(legs, STRIDE, STANCE, LIFT);
    let scale = b.legs().map_or(1.0, |l| l.hip[2] / s.hip_z);
    b.set_crawl_legs(
        legs,
        MeshBuilder::fitted_stride(STRIDE * scale),
        STANCE,
        LIFT,
    );
}

/// Far off: flat legs that do not walk, under the pelvis's spindle.
pub(super) fn coarse(b: &mut MeshBuilder, legs: &[Joints; 2]) {
    dark_plate(b);
    b.loft_z(
        &[[2.0, 0.0], [-1.0, 1.7], [-1.0, -1.7]],
        &[Section::new(24.0, 0.3), Section::new(RACE, 3.4)],
    );
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING_DARK);
        let flat = |b: &mut MeshBuilder, (hip, knee, foot, _): Joints| {
            let side = (foot - hip).with_z(0.0).normalize().cross(Vec3::Z) * 2.4;
            b.face(&[hip - side, foot, knee]);
            b.face(&[hip + side, knee, foot]);
            b.face(&[hip + side, foot, knee]);
            b.face(&[hip - side, knee, foot]);
        };
        for pair in legs {
            let pair = *pair;
            b.mirror_y(|b| flat(b, pair));
        }
    });
}

/// The pelvis: a faceted drum from a point under the hips up to the hull's race, in stepped
/// plates, the race on top. It is the root of every leg.
pub(super) fn pelvis(b: &mut MeshBuilder, s: &Style) {
    dark_plate(b);
    let z = s.hip_z;
    sleeve(
        b,
        &[
            ring(v3(0.0, 0.0, z - 5.0), Vec3::X, 0.6, 0.6),
            ring(v3(0.0, 0.0, z - 2.8), Vec3::X, 4.6, 4.6),
            ring(v3(0.0, 0.0, z - 0.6), Vec3::X, 8.0, 8.0),
            ring(v3(0.0, 0.0, z + 3.2), Vec3::X, 8.6, 8.6),
            ring(v3(0.0, 0.0, RACE - 0.7), Vec3::X, 7.4, 7.4),
        ],
        &OCT,
    );
    if b.fine() {
        // Plates lapped round it, each a ring a little proud of the drum.
        for (z0, r) in [(z - 1.6, 8.5f32), (z + 1.2, 8.9)] {
            sleeve(
                b,
                &[
                    ring(v3(0.0, 0.0, z0), Vec3::X, r * 0.94, r * 0.94),
                    ring(v3(0.0, 0.0, z0 + 0.6), Vec3::X, r, r),
                    ring(v3(0.0, 0.0, z0 + 2.4), Vec3::X, r * 0.97, r * 0.97),
                ],
                &OCT,
            );
        }
    }
    // The race the hull turns on.
    metal(b);
    let sides = b.sides(20);
    b.prism(v3(0.0, 0.0, RACE - 0.8), sides, 7.5, 7.45, 0.8);
}

/// Interpolates a limb's two (half width, half depth) pairs at `t`.
fn dims(d: &[f32; 4], t: f32) -> (f32, f32) {
    (d[0] + (d[2] - d[0]) * t, d[1] + (d[3] - d[1]) * t)
}

/// One limb bone from `a` to `c`, its keeled section swelling from `d[0..2]` at `a` to
/// `d[2..4]` at `c`, the ridge turned toward `out`, then bands of plate lapped over it: each
/// a ring `swell` times the bone's section stepping up at its leading edge and easing back
/// into it, so the plates read as lames of one shell.
fn bone(b: &mut MeshBuilder, a: Vec3, c: Vec3, out: Vec3, d: &[f32; 4], swell: f32) {
    dark_plate(b);
    let at = |t: f32, k: f32| {
        let (w, h) = dims(d, t);
        ring(a.lerp(c, t), out, w * k, h * k)
    };
    sleeve(b, &[at(0.0, 1.0), at(0.5, 1.0), at(1.0, 1.0)], &KEEL);
    let bands: &[f32] = if b.fine() { &[0.04, 0.3, 0.56] } else { &[0.3] };
    for &t0 in bands {
        let t1 = (t0 + 0.4).min(1.0);
        sleeve(
            b,
            &[
                at(t0, 1.0),
                at(t0 + 0.03, swell),
                at((t0 + t1) * 0.5, swell * 0.985),
                at(t1, 1.0),
            ],
            &KEEL,
        );
    }
}

/// One leg from `hip` through `knee` to the foot's tip at `foot`. Its bones ride
/// `rig::THIGH` and `rig::SHIN`.
pub(super) fn leg(b: &mut MeshBuilder, s: &Style, (hip, knee, foot, _): Joints) {
    let out = (foot - hip).with_z(0.0).normalize();
    let across = out.cross(Vec3::Z);
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            let along = (knee - hip).normalize();
            let up = (Vec3::Z - along * along.z).normalize();
            // The shoulder: a drum across the leg where it leaves the pelvis, then the shell.
            metal(b);
            collar(
                b,
                hip + out * 0.8,
                across,
                s.thigh[1] * 1.05,
                s.thigh[0] * 2.0 + 0.8,
            );
            bone(b, hip, knee, up, &s.thigh, 1.1);
        });
        b.with_limb(rig::SHIN, |b| {
            // The knee: a plated drum across the leg, banded, capped in bronze.
            dark_plate(b);
            let sides = b.sides(12);
            let half = s.thigh[2] + 0.5;
            b.cylinder_between(
                knee - across * half,
                knee + across * half,
                s.knee_r,
                s.knee_r,
                sides,
            );
            if b.fine() {
                seam(b);
                for k in [-0.35f32, 0.35] {
                    b.cylinder_between(
                        knee + across * (half * k - 0.15),
                        knee + across * (half * k + 0.15),
                        s.knee_r * 1.04,
                        s.knee_r * 1.04,
                        sides,
                    );
                }
            }
            metal(b);
            for sgn in [-1.0f32, 1.0] {
                b.cylinder_between(
                    knee + across * (sgn * half),
                    knee + across * (sgn * (half + 0.5)),
                    s.knee_r * 0.55,
                    s.knee_r * 0.45,
                    sides,
                );
            }
            let ankle = foot + Vec3::Z * (s.foot_r + 1.6);
            let drop = (ankle - knee).normalize();
            bone(b, knee, ankle, out, &s.shin, 1.12);
            if b.fine() {
                // Hydraulic rods down either side of the shin, bronze, collared along it.
                seam(b);
                for sgn in [-1.0f32, 1.0] {
                    let off = across * (sgn * (s.shin[0] + 0.9)) + out * 0.2;
                    let (p, q) = (knee.lerp(ankle, 0.1) + off, knee.lerp(ankle, 0.72) + off);
                    shaft(b, p, q, 0.34);
                    for t in [0.0f32, 0.5, 1.0] {
                        collar(b, p.lerp(q, t), drop, 0.6, 0.8);
                    }
                    dark_plate(b);
                    b.cylinder_between(p, p.lerp(q, 0.45), 0.6, 0.55, b.sides(8));
                }
            }
            // The foot: a collar at the ankle, then a heavy armoured block down to the ground.
            collar(b, ankle, Vec3::Z, s.shin[2] * 0.8, 0.8);
            dark_plate(b);
            let r = s.foot_r;
            sleeve(
                b,
                &[
                    ring(ankle - Vec3::Z * 0.4, across, s.shin[2], s.shin[3]),
                    ring(foot + Vec3::Z * (r * 0.6), across, r, r * 1.15),
                    ring(foot + Vec3::Z * 0.3, across, r * 0.92, r * 1.08),
                    ring(foot, across, r * 0.7, r * 0.85),
                ],
                &OCT,
            );
        });
    });
}
