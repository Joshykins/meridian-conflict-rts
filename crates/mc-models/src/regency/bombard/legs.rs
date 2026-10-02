//! One left leg: a long keeled thigh from the hip drum down to a forward bronze knee, a
//! faceted shin tapering back to the ankle with a greave swept up off its outside, and an
//! armoured boot, its toe a keel. Thigh, shin and boot ride their bones (`rig::THIGH`,
//! `rig::SHIN`, `rig::FOOT`, the boot turning about the ankle); nothing reaches across a
//! joint.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::{part, rig};

use super::super::commander::form::{ball, blade, ring, sleeve, KEEL, OCT};
use super::super::kit::{dark_plate, metal};
use super::{ANKLE, HIP, KNEE};

pub(super) fn leg(b: &mut MeshBuilder) {
    let (hip, knee, ankle) = (HIP, KNEE, ANKLE);
    // The sole, under the ankle.
    let foot = Vec3::new(ankle.x, ankle.y, 0.0);
    let out = (foot - hip).truncate().extend(0.0).normalize_or(Vec3::Y);
    // Toward the outside of the leg's bend, for the keels.
    let outside = (Vec3::Z + out * 0.5).normalize();
    let thigh = knee - hip;
    let shin = ankle - knee;
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(hip + thigh * 0.1, outside, 0.4, 0.42),
                    ring(hip + thigh * 0.5, outside, 0.46, 0.5),
                    ring(knee - thigh * 0.08, outside, 0.34, 0.36),
                ],
                &KEEL,
            );
            // A plate off the top of the thigh, its point swept back past the hip.
            blade(
                b,
                knee - thigh * 0.15 + outside * 0.3,
                -thigh,
                outside,
                thigh.length() * 0.85,
                0.26,
                0.6,
                0.12,
            );
        });
        b.with_limb(rig::SHIN, |b| {
            // The knee: a bronze ball, the joint showing in the gap.
            metal(b);
            ball(b, knee, 0.3);
            dark_plate(b);
            let end = ankle - Vec3::Z * 0.05;
            sleeve(
                b,
                &[
                    ring(knee + shin * 0.08, outside, 0.38, 0.38),
                    ring(knee + shin * 0.45, outside, 0.34, 0.35),
                    ring(knee + shin * 0.82, outside, 0.24, 0.25),
                    ring(end, outside, 0.14, 0.14),
                ],
                &OCT,
            );
            // The greave: a blade swept up and back off the shin's outside.
            blade(
                b,
                knee + shin * 0.7 + outside * 0.12,
                -shin,
                outside,
                shin.length() * 0.6,
                0.24,
                0.7,
                0.12,
            );
        });
        // The boot: a faceted block, its toe a keel, turning on the ankle.
        b.with_limb(rig::FOOT, |b| {
            metal(b);
            ball(b, ankle, 0.24);
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(foot + Vec3::new(-0.75, 0.0, 0.3), Vec3::Z, 0.34, 0.28),
                    ring(foot + Vec3::new(0.0, 0.0, 0.34), Vec3::Z, 0.42, 0.34),
                    ring(foot + Vec3::new(0.95, 0.0, 0.24), Vec3::Z, 0.32, 0.22),
                ],
                &KEEL,
            );
        });
    });
}
