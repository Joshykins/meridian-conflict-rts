//! The Breacher's legs: short, heavy and reverse-kneed, set wide. Each bone is a dark
//! frame under light plates with rams down its back, every piece riding its bone so it
//! moves with the IK. A broad flat foot with no toes.

use glam::Vec3;

use super::super::limbs::*;
use super::super::parts::*;
use super::FOOT;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, pattern, rig};

const Y: Vec3 = Vec3::Y;

/// A left leg's joints at rest (the right is its mirror): the thigh runs forward from the
/// hip to the knee, the shin back down to the hock, the tarsus forward again to the ankle.
#[derive(Clone, Copy)]
pub(super) struct Leg {
    pub(super) hip: Vec3,
    pub(super) knee: Vec3,
    pub(super) hock: Vec3,
    pub(super) ankle: Vec3,
}

/// The left leg.
pub(super) fn leg(b: &mut MeshBuilder, l: Leg) {
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| thigh(b, l));
        b.with_limb(rig::SHIN, |b| shin(b, l));
        b.with_limb(rig::TARSUS, |b| tarsus(b, l));
        b.with_limb(rig::FOOT, |b| foot(b, l.ankle));
    });
}

/// From far off: each bone a bar.
pub(super) fn coarse_leg(b: &mut MeshBuilder, l: Leg) {
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING);
        b.with_limb(rig::THIGH, |b| tri_bar(b, l.hip, l.knee, 4.8, 4.0));
        b.with_limb(rig::SHIN, |b| tri_bar(b, l.knee, l.hock, 3.8, 3.2));
        b.with_limb(rig::TARSUS, |b| tri_bar(b, l.hock, l.ankle, 3.2, 2.8));
    });
}

/// The thigh: the hip actuator drum, a deep armoured thigh forward to the knee, a plate over
/// its front and outside, a ram along its back, the knee drum.
fn thigh(b: &mut MeshBuilder, l: Leg) {
    let fine = b.fine();
    let (hip, knee) = (l.hip, l.knee);
    let (_, _, face) = bone_frame(hip, knee);
    joint(b, hip, 3.6, 4.2);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    limb(
        b,
        hip,
        knee,
        &[
            (0.0, 3.4, 3.4, 3.4, 1.1),
            (0.45, 3.6, 3.6, 3.2, 1.1),
            (1.0, 3.0, 2.8, 2.8, 0.9),
        ],
    );
    b.paint(PLATING);
    armour(
        b,
        hip.lerp(knee, 0.08) + face * 3.2,
        hip.lerp(knee, 0.88) + face * 2.6,
        face,
        (3.8, 1.8),
        (3.0, 1.4),
    );
    if fine {
        armour(
            b,
            hip.lerp(knee, 0.12) + Y * 3.4,
            hip.lerp(knee, 0.8) + Y * 3.0,
            Y,
            (3.0, 1.0),
            (2.4, 0.8),
        );
        ram(
            b,
            hip.lerp(knee, 0.05) - face * 4.0,
            hip.lerp(knee, 0.95) - face * 3.0,
            0.9,
            0.55,
        );
    }
    joint(b, knee, 3.2, 3.0);
}

/// The shin: from the knee back and down to the hock, a broad greave over its face, twin
/// rams behind.
fn shin(b: &mut MeshBuilder, l: Leg) {
    let fine = b.fine();
    let (knee, hock) = (l.knee, l.hock);
    let (_, _, face) = bone_frame(knee, hock);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    limb(
        b,
        knee,
        hock,
        &[
            (0.0, 3.0, 2.4, 2.4, 0.9),
            (0.5, 2.6, 2.2, 2.2, 0.8),
            (1.0, 2.4, 2.0, 2.0, 0.7),
        ],
    );
    b.paint(PLATING);
    armour(
        b,
        knee.lerp(hock, 0.1) + face * 2.2,
        knee.lerp(hock, 0.9) + face * 2.0,
        face,
        (3.4, 1.6),
        (2.8, 1.3),
    );
    if fine {
        for s in [-1.0f32, 1.0] {
            ram(
                b,
                knee.lerp(hock, 0.1) - face * 2.6 + Y * (s * 1.4),
                knee.lerp(hock, 0.9) - face * 2.2 + Y * (s * 1.4),
                0.6,
                0.35,
            );
        }
        b.paint(TEAM).pattern(pattern::PLAIN);
        let at = knee.lerp(hock, 0.5) + face * 3.7;
        b.beam(at - Y * 2.0, at + Y * 2.0, v2(0.8, 0.2), v2(0.8, 0.2));
    }
    joint(b, hock, 2.8, 2.6);
}

/// The tarsus: from the hock forward and down to the ankle, a heel cap over the hock, a
/// shock ram down its back.
fn tarsus(b: &mut MeshBuilder, l: Leg) {
    let (hock, ankle) = (l.hock, l.ankle);
    let (_, _, face) = bone_frame(hock, ankle);
    b.paint(PLATING);
    limb(
        b,
        hock,
        ankle,
        &[(0.0, 2.6, 2.2, 2.0, 0.7), (1.0, 2.4, 2.0, 2.0, 0.7)],
    );
    // The heel cap: a heavy plate round the back of the hock.
    armour(
        b,
        hock - Vec3::X * 2.0 + Vec3::Z * 2.0,
        hock - Vec3::X * 2.8 - Vec3::Z * 1.6,
        -Vec3::X,
        (3.0, 1.4),
        (2.6, 1.2),
    );
    if b.fine() {
        ram(
            b,
            hock - face * 2.4,
            ankle - face * 2.2 + Vec3::Z * 1.0,
            0.6,
            0.35,
        );
    }
    joint(b, ankle + Vec3::Z * 0.4, 2.4, 2.0);
}

/// A great flat foot: a dark sole pad, a light shell over it rising to the ankle, a prow
/// plate over the front, ankle guards, a heel block. No toes.
fn foot(b: &mut MeshBuilder, ankle: Vec3) {
    let fine = b.fine();
    let (rear, front, width) = FOOT;
    let (x0, x1) = (ankle.x + rear, ankle.x + front);
    let half = width * 0.5;
    b.paint(TREAD);
    b.extrude_y(
        &[[x0 + 0.6, 0.0], [x1 - 0.6, 0.0], [x1, 0.8], [x0, 0.8]],
        ankle.y - half,
        ankle.y + half,
    );
    b.paint(PLATING);
    b.extrude_y(
        &[
            [x0, 0.8],
            [x1, 0.8],
            [x1 - 0.6, 2.0],
            [ankle.x + 3.0, 3.8],
            [ankle.x - 2.6, 4.4],
            [x0 + 0.4, 2.6],
        ],
        ankle.y - half + 0.3,
        ankle.y + half - 0.3,
    );
    b.extrude_y(
        &[
            [ankle.x + 3.6, 2.8],
            [x1 + 0.3, 0.9],
            [x1 + 0.1, 1.9],
            [ankle.x + 3.4, 4.2],
        ],
        ankle.y - half + 0.6,
        ankle.y + half - 0.6,
    );
    for s in [-1.0f32, 1.0] {
        b.beam(
            v3(ankle.x - 1.6, ankle.y + s * (half - 0.8), 3.4),
            v3(ankle.x + 2.2, ankle.y + s * (half - 0.8), 3.4),
            v2(0.8, 3.2),
            v2(0.8, 2.4),
        );
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        v3(x0 - 0.4, ankle.y - 2.0, 0.6),
        v3(x0 + 2.2, ankle.y + 2.0, 3.0),
    );
    if fine {
        b.paint(METAL);
        for k in 0..4 {
            let x = x0 + 1.6 + k as f32 * (x1 - x0 - 3.2) / 3.0;
            b.cuboid(v3(x, ankle.y + half - 0.1, 1.4), v3(0.5, 0.3, 0.5));
            b.cuboid(v3(x, ankle.y - half + 0.1, 1.4), v3(0.5, 0.3, 0.5));
        }
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(
            v3(x1 - 0.6, ankle.y - half + 0.8, 1.0),
            v3(x1 + 0.05, ankle.y + half - 0.8, 1.5),
        );
    }
}
