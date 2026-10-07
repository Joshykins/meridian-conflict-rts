//! Four load-bearing limbs: recessed rotary bearings, broad shield-shaped greaves,
//! short plated ankles and bevelled soles. Three feet remain planted through the gait.
use super::super::commander::form::{ring, sleeve, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::hoop_on;
use super::RACE;
use crate::builder::{MeshBuilder, Section};
use crate::{material::*, part, pattern, rig};
use glam::{Vec2, Vec3};

pub(super) type Joints = (Vec3, Vec3, Vec3, f32);
pub(super) struct Style {
    pub(super) hip_z: f32,
    knee_out: f32,
    foot_y: f32,
    armor: f32,
}
pub(super) const SIEGE: Style = Style {
    hip_z: 13.0,
    knee_out: 19.0,
    foot_y: 32.0,
    armor: 1.0,
};
pub(super) const STRIDE: f32 = 28.0;
pub(super) const STANCE: f32 = 0.75;
pub(super) const LIFT: f32 = 4.0;

pub(super) fn legs(s: &Style) -> [Joints; 2] {
    let joints = |x: f32, phase: f32| {
        let hip = v3(x, 10.0, s.hip_z);
        let foot = v3(x * 1.7, s.foot_y, 0.0);
        let out = (foot - hip).with_z(0.0).normalize();
        (hip, hip + out * s.knee_out + Vec3::Z * 9.5, foot, phase)
    };
    [joints(12.5, 0.0), joints(-12.5, 0.25)]
}
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
pub(super) fn pelvis(b: &mut MeshBuilder, s: &Style) {
    dark_plate(b);
    b.chamfered_box(v3(0.0, 0.0, s.hip_z + 1.8), v3(30.0, 22.0, 8.0), 3.0);
    metal(b);
    b.prism(v3(0.0, 0.0, RACE - 1.5), b.sides(20), 9.0, 8.6, 1.5);
    b.mirror_y(|b| {
        seam(b);
        b.chamfered_box(v3(-1.0, 11.0, 16.5), v3(17.0, 2.0, 4.0), 0.6);
        if b.fine() {
            for i in 0..8 {
                metal(b);
                b.chamfered_box(
                    v3(-8.0 + i as f32 * 2.0, 11.5, 16.5),
                    v3(0.45, 1.0, 3.0),
                    0.15,
                );
            }
        }
    });
}
fn bearing(b: &mut MeshBuilder, at: Vec3, across: Vec3, radius: f32, half: f32) {
    seam(b);
    b.cylinder_between(
        at - across * half,
        at + across * half,
        radius,
        radius,
        b.sides(10),
    );
    for sign in [-1.0, 1.0] {
        let cap = at + across * (half * sign);
        metal(b);
        hoop_on(
            b,
            cap,
            across * sign,
            radius * 0.88,
            radius * 0.17,
            0.35,
            b.sides(10),
        );
        b.cylinder_between(
            cap,
            cap + across * (sign * 0.42),
            radius * 0.60,
            radius * 0.53,
            b.sides(8),
        );
        dark_plate(b);
        b.cylinder_between(
            cap + across * (sign * 0.42),
            cap + across * (sign * 0.52),
            radius * 0.27,
            radius * 0.27,
            b.sides(8),
        );
    }
}
pub(super) fn leg(b: &mut MeshBuilder, s: &Style, (hip, knee, foot, _): Joints) {
    let out = (foot - hip).with_z(0.0).normalize();
    let across = out.cross(Vec3::Z);
    let k = s.armor;
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            bearing(b, hip, across, 3.7, 4.4);
            let along = (knee - hip).normalize();
            let up = (Vec3::Z - along * along.z).normalize();
            metal(b);
            sleeve(
                b,
                &[
                    ring(hip + along * 2.0, up, 3.0, 2.6),
                    ring(knee - along * 2.0, up, 3.0, 2.6),
                ],
                &OCT,
            );
            // Two fitted armor segments leave the rotary joints visibly exposed.
            for (a, c) in [(0.17, 0.50), (0.54, 0.81)] {
                dark_plate(b);

                sleeve(
                    b,
                    &[
                        ring(hip.lerp(knee, a), up, 4.0 * k, 3.3),
                        ring(hip.lerp(knee, a + 0.06), up, 4.7 * k, 3.8),
                        ring(hip.lerp(knee, c), up, 4.1 * k, 3.5),
                    ],
                    &OCT,
                );
            }
        });
        b.with_limb(rig::SHIN, |b| {
            bearing(b, knee, across, 3.8, 4.9 * k);
            let ankle = foot + Vec3::Z * 4.3;
            let down = (ankle - knee).normalize();
            let face = (out - down * out.dot(down)).normalize();
            // Dark central chassis, with a broad flat shield over its outer face.
            metal(b);
            sleeve(
                b,
                &[
                    ring(knee, face, 3.8 * k, 2.7),
                    ring(ankle, face, 2.5 * k, 2.0),
                ],
                &OCT,
            );
            let a = knee + down * 1.0 + face * 0.5;
            let c = knee.lerp(ankle, 0.72) + face * 1.0;
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(a, face, 4.7 * k, 3.3),
                    ring(a + down * 3.0, face, 5.1 * k, 3.8),
                    ring(c, face, 3.8 * k, 3.0),
                    ring(c + down * 1.1, face, 3.0 * k, 2.4),
                ],
                &OCT,
            );
            // The inset armored spine owns the red slit, with a chamfer at each end.
            b.paint(PLATING).pattern(pattern::EMBER);
            sleeve(
                b,
                &[
                    ring(a + down * 1.0 + face * 3.0, face, 1.25, 0.6),
                    ring(a + down * 3.0 + face * 3.55, face, 1.6, 0.6),
                    ring(c + face * 2.8, face, 1.05, 0.5),
                ],
                &OCT,
            );
            dark_plate(b);
            b.pattern(pattern::EMBER_SLOT);
            b.face(&[
                a + down * 3.0 + face * 4.16 - across * 0.7,
                a + down * 3.0 + face * 4.16 + across * 0.7,
                c + face * 3.31 + across * 0.5,
                c + face * 3.31 - across * 0.5,
            ]);
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(knee.lerp(ankle, 0.77), face, 3.3 * k, 2.7),
                    ring(ankle, face, 3.0 * k, 2.4),
                ],
                &OCT,
            );
            // Flush steel edging and a real ankle gap distinguish shin from foot.
            if b.fine() {
                for side in [-1.0, 1.0] {
                    metal(b);
                    b.beam(
                        a + across * (side * 4.3 * k) + face,
                        c + across * (side * 3.5 * k) + face,
                        Vec2::new(0.25, 0.4),
                        Vec2::new(0.22, 0.35),
                    );
                }
            }
            bearing(b, ankle, across, 1.9, 2.8 * k);
            b.at(foot, |b| {
                let sole = [
                    [-1.0, -0.7],
                    [-0.7, -1.0],
                    [0.7, -1.0],
                    [1.0, -0.7],
                    [1.0, 0.65],
                    [0.7, 1.0],
                    [-0.7, 1.0],
                    [-1.0, 0.65],
                ];
                seam(b);
                b.loft_z(
                    &sole,
                    &[Section::new(0.0, 5.2 * k), Section::new(0.6, 5.2 * k)],
                );
                dark_plate(b);
                b.loft_z(
                    &sole,
                    &[
                        Section::new(0.6, 5.2 * k),
                        Section::new(1.4, 4.9 * k),
                        Section::new(3.5, 3.2 * k),
                    ],
                );
            });
        });
    });
}
pub(super) fn coarse(b: &mut MeshBuilder, legs: &[Joints; 2]) {
    dark_plate(b);
    b.loft_z(
        &[[2.0, 0.0], [-1.0, 1.7], [-1.0, -1.7]],
        &[Section::new(19.0, 4.0), Section::new(RACE, 4.0)],
    );
    b.with_part(part::LOCOMOTION, |b| {
        for &(hip, knee, foot, _) in legs {
            b.mirror_y(|b| {
                let side = (foot - hip).with_z(0.0).normalize().cross(Vec3::Z) * 3.4;
                b.face(&[hip - side, foot, knee]);
                b.face(&[hip + side, knee, foot]);
                b.face(&[hip + side, foot, knee]);
                b.face(&[hip - side, knee, foot]);
            });
        }
    });
}
