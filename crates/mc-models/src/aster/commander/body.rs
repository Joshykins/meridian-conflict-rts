//! The commander's pelvis, torso and pauldrons: the black frame and the white plates on
//! it, every plate seated on a face of the piece it is fixed to.

use glam::Vec3;

use super::{ARM_Z, CHEST_HALF, GIRTH, PROW, SHOULDER_Y};
use crate::aster::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// The chest in side profile (x, z), before `PROW` pushes its front out: a
/// lower slope, the breast, an upper slope to a flat top, the back.
const CHEST: [[f32; 2]; 8] = [
    [-1.7, 10.3],
    [1.2, 10.3],
    [2.2, 11.2],
    [2.2, 12.35],
    [1.2, 13.35],
    [-1.8, 13.35],
    [-2.2, 12.5],
    [-2.2, 10.9],
];

/// The chest's profile with its front `prow` metres further forward.
fn chest(prow: f32) -> [[f32; 2]; 8] {
    let mut p = CHEST;
    for q in &mut p[1..5] {
        q[0] += prow;
    }
    p
}

/// A pauldron's black block in side profile (x, z), standing `SHOULDER_Y` out: chamfered
/// fore and aft, its top level at `POD_TOP` for the shoulder mount's ring.
const POD: [[f32; 2]; 8] = [
    [-1.35, 11.75],
    [-1.05, 11.45],
    [0.85, 11.45],
    [1.25, 11.95],
    [1.25, 12.9],
    [0.9, POD_TOP],
    [-1.1, POD_TOP],
    [-1.35, 13.0],
];
/// The pauldron's top: the right one carries the shoulder mount's ring on it
/// (`flak::turntable`, 13.15 to 13.4).
const POD_TOP: f32 = 13.25;
/// Half the pauldron's width across.
const POD_HALF: f32 = 0.95;

/// The pelvis: black skirts under a white plate on the front. It sits on the hips.
pub(super) fn pelvis(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.chamfered_box(v3(-0.1, 0.0, 8.65), v3(2.2, 2.2, 1.6), 0.5);
    b.frustum(
        v3(0.95, 0.0, 7.7),
        v2(0.45, 1.2),
        v2(0.65, 1.7),
        1.7,
        v2(0.18, 0.0),
    );
    b.frustum(
        v3(-1.15, 0.0, 7.9),
        v2(0.4, 1.3),
        v2(0.5, 1.8),
        1.4,
        v2(-0.12, 0.0),
    );
    b.paint(PLATING);
    b.plate(v3(1.05, 0.0, 9.4), v2(0.7, 1.35), 0.1, 0.04);
    b.plate(v3(-0.1, 0.0, 9.45), v2(1.2, 1.4), 0.08, 0.03);
}

/// Waist, abdomen and chest, with the breastplates, the vent between them and the plates
/// over the back where a pack bolts on.
pub(super) fn torso(b: &mut MeshBuilder) {
    let p = chest(PROW);
    let (lower, breast, upper) = ([p[1], p[2]], [p[2], p[3]], [p[3], p[4]]);
    // Waist ring, and the abdomen under the chest: grey, it is the joint the torso turns on.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 9.05), b.sides(8), 1.2, 1.2, 0.5);
    b.paint(METAL);
    b.chamfered_box(v3(-0.1, 0.0, 9.95), v3(1.9, 2.4, 0.9), 0.45);
    b.paint(ACCENT);
    b.extrude_y_chamfered(&p, CHEST_HALF, 0.5);

    b.paint(PLATING);
    // The breastplates either side of the vent, and the plates on the slopes above
    // and below them. Black shows between them all.
    on_slope(b, breast[0], breast[1], 0.5, |b| {
        b.mirror_y(|b| b.plate(v3(0.0, 1.0, 0.0), v2(1.0, 1.05), 0.2, 0.07))
    });
    on_slope(b, upper[0], upper[1], 0.5, |b| {
        b.mirror_y(|b| b.plate(v3(0.0, 1.15, 0.0), v2(1.25, 0.95), 0.18, 0.06))
    });
    on_slope(b, lower[0], lower[1], 0.5, |b| {
        b.mirror_y(|b| b.plate(v3(0.0, 0.75, 0.0), v2(0.95, 0.9), 0.14, 0.05))
    });
    // The top behind the head, and the back: a spine and two shoulder-blade plates
    // where a pack would bolt on.
    on_slope(b, p[4], p[5], 0.3, |b| {
        b.mirror_y(|b| b.plate(v3(0.0, 0.95, 0.0), v2(1.3, 0.75), 0.1, 0.04))
    });
    on_slope(b, p[5], p[6], 0.5, |b| {
        b.mirror_y(|b| b.plate(v3(0.0, 1.1, 0.0), v2(0.8, 0.95), 0.12, 0.05))
    });
    b.paint(ACCENT);
    b.block(v3(-2.45, -0.5, 10.6), v3(-2.15, 0.5, 13.0));

    // The vent in the breast between the plates, and the intakes under them: drawn
    // louvres, not slats of mesh.
    b.paint(ACCENT).pattern(pattern::LOUVRE);
    on_slope(b, breast[0], breast[1], 0.5, |b| {
        b.plate(Vec3::ZERO, v2(0.85, 0.7), 0.1, 0.03)
    });
    if b.fine() {
        on_slope(b, lower[0], lower[1], 0.5, |b| {
            b.plate(Vec3::ZERO, v2(0.6, 0.45), 0.06, 0.02)
        });
    }
    collar(b);
}

/// A gorget round the back and sides of the head: two raised black cheeks on the chest's
/// top that the head looks out between, clear of it as it turns.
fn collar(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.extrude_y(
            &[[-1.4, 13.3], [0.9, 13.3], [0.55, 13.7], [-1.3, 13.9]],
            1.15,
            1.45,
        )
    });
    b.extrude_y(
        &[[-1.75, 13.3], [-1.15, 13.3], [-1.25, 13.95], [-1.6, 13.85]],
        -1.15,
        1.15,
    );
    b.paint(PLATING);
    b.mirror_y(|b| {
        on_slope(b, [0.55, 13.7], [-1.3, 13.9], 0.5, |b| {
            b.plate(v3(0.0, 1.3, 0.0), v2(1.4, 0.26), 0.06, 0.02)
        })
    });
}

/// The pauldrons and the upper arms hung from them to the elbows at the waist.
pub(super) fn shoulders(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        // The shoulder joint out of the chest's flank.
        b.paint(METAL);
        b.cylinder_between(
            v3(-0.1, CHEST_HALF - 0.1, 12.35),
            v3(-0.1, SHOULDER_Y - POD_HALF + 0.05, 12.35),
            0.62,
            0.62,
            b.sides(10),
        );
        pod(b);
        upper_arm(b);
    });
}

/// A long upper arm from under the pauldron to the elbow the forearm pitches about.
fn upper_arm(b: &mut MeshBuilder) {
    let g = GIRTH;
    let (top, elbow) = (v3(0.0, SHOULDER_Y, 11.7), v3(-0.3, SHOULDER_Y, 9.95));
    b.paint(ACCENT);
    b.beam(top, elbow, v2(0.85, 0.85) * g, v2(0.75, 0.75) * g);
    // A white plate down the outside of it.
    b.paint(PLATING);
    b.beam(
        top + v3(0.0, 0.43 * g, -0.25),
        elbow + v3(0.05, 0.38 * g, 0.55),
        v2(0.08, 0.72),
        v2(0.08, 0.6),
    );
    b.paint(METAL);
    b.cylinder_between(
        v3(-0.3, SHOULDER_Y - 0.55, ARM_Z + 0.1),
        v3(-0.3, SHOULDER_Y + 0.55, ARM_Z + 0.1),
        0.5,
        0.5,
        b.sides(8),
    );
}

/// The heavy pauldron: a black block, a white plate on its top carrying the team colour,
/// a white plate across its front and a vent let into its outer face.
fn pod(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.at(v3(0.0, SHOULDER_Y, 0.0), |b| {
        b.extrude_y_chamfered(&POD, POD_HALF, 0.22)
    });
    b.paint(PLATING);
    b.plate(v3(-0.1, SHOULDER_Y, POD_TOP), v2(1.75, 1.3), 0.1, 0.04);
    team_panel(b, v3(-0.1, SHOULDER_Y, POD_TOP + 0.1), v2(1.2, 0.8));
    let front = POD[3][0];
    b.block(
        v3(front - 0.04, SHOULDER_Y - 0.65, 11.98),
        v3(front + 0.06, SHOULDER_Y + 0.65, 12.88),
    );
    b.paint(ACCENT).pattern(pattern::LOUVRE);
    let out = SHOULDER_Y + POD_HALF;
    b.block(v3(-0.95, out - 0.04, 11.75), v3(0.65, out + 0.06, 12.95));
}
