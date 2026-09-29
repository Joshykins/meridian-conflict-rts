//! The scorpion's legs and claws.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::super::kit::*;
use super::*;

/// One left leg of pair `pair`: a heavy armoured thigh on a ram up to a spiked knee, a
/// plated shin down to a clawed foot, red light in the seams.
pub(super) fn leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, foot: Vec3, pair: usize) {
    let out = (foot - hip).truncate().extend(0.0).normalize();
    // Toward the outside of the leg's bend, for the keels.
    let outside = Vec3::Z + out * 0.4;
    let heft = if pair == 0 || pair == 3 { 1.0 } else { 1.08 };
    let thigh = knee - hip;
    let shin = foot - knee;
    let ankle = knee + shin * 0.66;
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            seam(b);
            segment(
                b,
                &[
                    (hip, 0.95 * heft, 1.0 * heft),
                    (knee - thigh * 0.05, 0.75 * heft, 0.8 * heft),
                ],
                outside,
            );
            // The armour over it: an arched plate along the top, keeled.
            dark_plate(b);
            shell(
                b,
                &[
                    (
                        hip + thigh * 0.12 + outside.normalize() * 0.35,
                        1.15 * heft,
                        0.85 * heft,
                    ),
                    (
                        hip + thigh * 0.55 + outside.normalize() * 0.4,
                        1.12 * heft,
                        0.9 * heft,
                    ),
                    (
                        knee - thigh * 0.08 + outside.normalize() * 0.3,
                        0.92 * heft,
                        0.7 * heft,
                    ),
                ],
                outside,
            );
            if b.fine() {
                // The ram under the thigh that lifts it.
                let (_, up) = frame(thigh, outside);
                ram(
                    b,
                    hip + thigh * 0.1 - up * 0.95,
                    knee - thigh * 0.18 - up * 0.75,
                    0.3,
                );
                // A red seam along the plate's flank.
                b.paint(GLOW_LASER);
                let (side, _) = frame(thigh, outside);
                b.beam(
                    hip + thigh * 0.25 + side * 1.05 * heft,
                    hip + thigh * 0.8 + side * 0.9 * heft,
                    Vec2::new(0.1, 0.1),
                    Vec2::new(0.08, 0.08),
                );
                // A blade over the knee, raked back.
                dark_plate(b);
                blade(
                    b,
                    knee - thigh.normalize() * 0.7 + Vec3::Z * 0.6,
                    knee + Vec3::Z * 2.4 - shin.normalize() * 0.6 - Vec3::X * 0.7,
                    0.4,
                    out,
                );
            }
        });
        b.with_limb(rig::SHIN, |b| {
            knuckle(b, knee, out.cross(Vec3::Z), 0.92 * heft, 1.7 * heft);
            seam(b);
            segment(
                b,
                &[
                    (knee + shin * 0.03, 0.72 * heft, 0.8 * heft),
                    (ankle, 0.5, 0.55),
                ],
                out + Vec3::Z,
            );
            dark_plate(b);
            // The shin's armour, on its outer face.
            shell(
                b,
                &[
                    (knee + shin * 0.08, 0.95 * heft, 0.75 * heft),
                    (knee + shin * 0.4, 0.85 * heft, 0.68),
                    (ankle - shin * 0.04, 0.6, 0.45),
                ],
                out + Vec3::Z,
            );
            // The foot: a heavy hooked claw driven into the ground, a spur behind it.
            seam(b);
            knuckle(b, ankle, out.cross(Vec3::Z), 0.5, 0.95);
            dark_plate(b);
            segment(
                b,
                &[
                    (ankle, 0.5, 0.52),
                    (ankle.lerp(foot, 0.55), 0.36, 0.4),
                    (foot + Vec3::Z * 0.06, 0.05, 0.05),
                ],
                out + Vec3::Z,
            );
            if b.fine() {
                let back = -out * 0.9 + Vec3::Z * 0.2;
                spike(
                    b,
                    ankle + Vec3::Z * -0.2,
                    ankle.lerp(foot, 0.7) + back + Vec3::Z * 0.1,
                    0.24,
                );
                ram(
                    b,
                    knee + shin * 0.12 - out * 0.7,
                    ankle - out * 0.45 + Vec3::Z * 0.2,
                    0.22,
                );
                b.paint(GLOW_LASER);
                let (_, up) = frame(shin, out + Vec3::Z);
                b.beam(
                    knee + shin * 0.14 + up * 0.72,
                    knee + shin * 0.55 + up * 0.55,
                    Vec2::new(0.14, 0.09),
                    Vec2::new(0.1, 0.07),
                );
            }
        });
    });
}

/// The left claw on its arm: a swollen, spiked palm, a fixed outer finger, and the inner
/// finger that bites (`rig::CLAW_JAW`), both toothed, a red bite between them.
/// A claw: its arm, palm and fixed finger, then the moving finger. A Gravitic Bomb is
/// charged in the throat between the fingers, held off a bronze containment collar round
/// a lit emitter (`BOMB_AT`), and thrown from there.
pub(super) fn claw(b: &mut MeshBuilder) {
    let tip_fixed = v3(18.7, 3.5, 5.2);
    let tip_jaw = v3(18.3, 2.75, 5.05);
    b.with_claw(false, |b| {
        seam(b);
        segment(b, &[(SHOULDER, 0.85, 0.85), (ELBOW, 0.7, 0.72)], Vec3::Z);
        segment(b, &[(ELBOW, 0.72, 0.72), (WRIST, 0.85, 0.85)], Vec3::Z);
        dark_plate(b);
        shell(
            b,
            &[
                (SHOULDER.lerp(ELBOW, 0.1) + Vec3::Z * 0.35, 0.95, 0.75),
                (SHOULDER.lerp(ELBOW, 0.9) + Vec3::Z * 0.3, 0.85, 0.65),
            ],
            Vec3::Z,
        );
        shell(
            b,
            &[
                (ELBOW.lerp(WRIST, 0.12) + Vec3::Z * 0.3, 0.88, 0.68),
                (ELBOW.lerp(WRIST, 0.92) + Vec3::Z * 0.35, 1.0, 0.75),
            ],
            Vec3::Z,
        );
        knuckle(b, ELBOW, Vec3::Z, 0.75, 1.4);
        knuckle(b, WRIST, v3(0.3, 1.0, 0.0), 0.8, 1.2);
        // The palm: swollen, its outer face plated, spikes along its top.
        dark_plate(b);
        segment(
            b,
            &[
                (WRIST, 0.95, 0.9),
                (WRIST.lerp(PALM, 0.3), 1.7, 1.3),
                (WRIST.lerp(PALM, 0.7), 1.6, 1.2),
                (PALM, 1.1, 0.95),
            ],
            Vec3::Z,
        );
        // The fixed finger.
        segment(
            b,
            &[
                (PALM + v3(-0.3, 0.35, 0.15), 0.62, 0.62),
                (v3(17.6, 4.15, 5.35), 0.46, 0.5),
                (tip_fixed, 0.06, 0.1),
            ],
            Vec3::Z,
        );
        if b.fine() {
            ram(
                b,
                SHOULDER + v3(0.4, -0.3, -0.7),
                ELBOW + v3(-0.5, -0.4, -0.6),
                0.26,
            );
            ram(
                b,
                ELBOW + v3(0.3, -0.5, -0.5),
                WRIST + v3(-0.4, -0.6, -0.4),
                0.24,
            );
            // Spikes along the palm's top, in the darker seam paint (plate on a blade this thin
            // catches the sun as a white sliver).
            seam(b);
            for k in 0..3 {
                let at =
                    WRIST.lerp(PALM, 0.2 + k as f32 * 0.25) + v3(0.0, 0.3, 1.1 - k as f32 * 0.05);
                spike(b, at, at + v3(-0.9, 0.3, 0.95 - k as f32 * 0.15), 0.3);
            }
            // Teeth inside the fixed finger.
            seam(b);
            for k in 0..3 {
                let at = PALM.lerp(tip_fixed, 0.25 + k as f32 * 0.22) + v3(0.0, -0.35, 0.0);
                spike(b, at, at + v3(0.25, -0.45, -0.05), 0.14);
            }
            b.paint(TEAM);
            b.beam(
                WRIST.lerp(PALM, 0.2) + v3(0.0, 1.55, 0.2),
                WRIST.lerp(PALM, 0.75) + v3(0.0, 1.45, 0.2),
                Vec2::new(0.08, 0.5),
                Vec2::new(0.08, 0.45),
            );
            b.paint(GLOW_LASER);
            b.beam(
                WRIST.lerp(PALM, 0.35) + v3(0.0, -1.62, -0.1),
                WRIST.lerp(PALM, 0.85) + v3(0.0, -1.2, -0.1),
                Vec2::new(0.1, 0.12),
                Vec2::new(0.08, 0.1),
            );
        }
        // The bomb cradle: a bronze containment collar in the throat, its emitter lit.
        let throat = (BOMB_AT - PALM).normalize();
        let collar = PALM + throat * 0.55;
        metal(b);
        let sides = b.sides(10).min(10);
        b.cylinder_between(
            collar - throat * 0.35,
            collar + throat * 0.1,
            0.62,
            0.72,
            sides,
        );
        b.paint(GLOW_LASER);
        b.cylinder_between(collar, collar + throat * 0.45, 0.3, 0.16, 6);
        // Heavier plates over the palm's back, their edges swept into blades.
        if b.fine() {
            dark_plate(b);
            shell(
                b,
                &[
                    (WRIST.lerp(PALM, 0.2) + v3(0.0, 0.2, 1.0), 1.4, 0.45),
                    (WRIST.lerp(PALM, 0.85) + v3(0.0, 0.1, 0.9), 1.1, 0.4),
                ],
                Vec3::Z,
            );
            spike(
                b,
                ELBOW + v3(-0.5, -0.1, 0.6),
                ELBOW + v3(-1.8, 0.4, 2.3),
                0.34,
            );
            spike(
                b,
                WRIST + v3(0.6, 0.0, 0.8),
                WRIST + v3(-0.2, 0.3, 2.2),
                0.3,
            );
        }
    });
    // The moving finger, hinged inside the palm.
    b.with_claw(true, |b| {
        dark_plate(b);
        segment(
            b,
            &[
                (JAW_HINGE + v3(-0.4, 0.1, 0.0), 0.55, 0.55),
                (v3(17.3, 2.7, 5.2), 0.45, 0.46),
                (tip_jaw, 0.06, 0.1),
            ],
            Vec3::Z,
        );
        if b.fine() {
            knuckle(b, JAW_HINGE, Vec3::Z, 0.45, 1.0);
            seam(b);
            for k in 0..3 {
                let at = JAW_HINGE.lerp(tip_jaw, 0.3 + k as f32 * 0.2) + v3(0.0, 0.3, 0.0);
                spike(b, at, at + v3(0.25, 0.42, -0.05), 0.13);
            }
            b.paint(GLOW_LASER);
            b.beam(
                JAW_HINGE + v3(0.3, 0.42, 0.0),
                tip_jaw + v3(-0.5, 0.2, 0.0),
                Vec2::new(0.08, 0.1),
                Vec2::new(0.06, 0.08),
            );
        }
    });
}
