//! The commander's legs: heavy armoured columns on broad hip and knee drums, the
//! greaves marked with a hazard flash, standing on big boxed feet.

use glam::{Affine3A, Vec3};

use super::{FOOT, GIRTH};
use crate::aster::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, pattern, rig};

/// Lean and long in the limb, stood up rather than crouched at rest. It walks: a foot
/// is down for half the cycle, the other landing as it lifts, so the body never leaves
/// the ground. Its long stride needs more reach than the straight legs have, so it
/// settles a little onto bent knees as it gets going (0.9 m: with the foot planted over
/// half the stride the knees just reach). Striding, the feet come in under the hips
/// (`entity.wgsl` `stride_upright`). The hips sit in close under the pelvis and the legs
/// run straight out to the feet, as a person stands with their feet apart. Standing, it
/// carries its weight on one leg and eases the other now and then (`idle_stance`).
pub(super) fn legs(b: &mut MeshBuilder) {
    // The knee on the line from hip to ankle, so the leg runs straight out.
    let (hip, ankle) = (v3(-0.1, 1.15, 8.7), v3(-0.3, 2.1, 1.3));
    let knee = v3(
        0.7,
        hip.y + (ankle.y - hip.y) * (8.7 - 4.75) / (8.7 - 1.3),
        4.75,
    );
    b.set_legs(hip, knee, ankle, 16.0, 0.5, 1.3);
    b.set_walk_crouch(0.9);
    let (rear, front, width) = FOOT;
    b.set_foot(-rear, front, width);
    b.mirror_y(|b| leg(b, hip, knee, ankle));
}

/// A leg, splayed: `knee` and `ankle` lie on one line out from the hip. The leg is built
/// upright under the hip and rolled out about it whole, so every plate, hub and cap runs
/// along the leg; only the foot stays square on the ground.
fn leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, ankle: Vec3) {
    let roll = (ankle.y - hip.y).atan2(hip.z - ankle.z);
    let upright = |p: Vec3| v3(p.x, hip.y, hip.z - (hip.z - p.z) / roll.cos());
    let splay = Affine3A::from_translation(hip)
        * Affine3A::from_rotation_x(roll)
        * Affine3A::from_translation(-hip);
    let (foot_at, knee, ankle) = (ankle, upright(knee), upright(ankle));
    b.with_part(part::LOCOMOTION, |b| {
        if b.coarse() {
            // From this far the legs are their white greaves and cuisses on the feet.
            b.paint(PLATING);
            b.frustum_open(
                v3(foot_at.x + 0.5, foot_at.y, 0.0),
                v2(3.4, 1.9) * GIRTH,
                v2(1.6, 1.7) * GIRTH,
                hip.z,
                v2(hip.x - foot_at.x - 0.5, hip.y - foot_at.y),
            );
            return;
        }
        b.with(splay, |b| upright_leg(b, hip, knee, ankle));
        b.with_limb(rig::FOOT, |b| {
            // Square under the shin: the leg bends in its own plane, so a foot turned
            // out would twist against it at the ankle.
            b.at(v3(foot_at.x, foot_at.y, 0.0), foot);
        });
    });
}

/// [`leg`] before it is rolled out: straight down from the hip.
fn upright_leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, ankle: Vec3) {
    let (y, g) = (Vec3::Y, GIRTH);
    b.with_limb(rig::THIGH, |b| {
        b.paint(ACCENT);
        b.beam(hip, knee, v2(1.35, 1.5) * g, v2(1.1, 1.2) * g);
        // The hip drum, its outer end a broad black disc with a white cap.
        b.paint(METAL);
        b.cylinder_between(hip - y * 0.82, hip + y * 0.82, 0.88, 0.88, b.sides(10));
        b.paint(ACCENT);
        b.cylinder_between(hip + y * 0.8, hip + y * 0.92, 0.9, 0.84, b.sides(12));
        b.paint(PLATING);
        if b.fine() {
            b.cylinder_between(hip + y * 0.92, hip + y * 0.98, 0.56, 0.5, 12);
        }
        // Cuisse: a white plate over the front and outside of the thigh.
        b.beam(
            hip + v3(0.55 * g, 0.0, -0.5),
            knee + v3(0.5 * g, 0.0, 0.8),
            v2(1.6, 0.72) * g,
            v2(1.5, 0.6) * g,
        );
        if b.fine() {
            // A hydraulic ram down the back of the thigh.
            b.paint(METAL);
            b.cylinder_between(
                hip + v3(-0.72 * g, 0.0, -0.4),
                knee + v3(-0.66 * g, 0.0, 0.6),
                0.18,
                0.15,
                6,
            );
        }
    });
    b.with_limb(rig::SHIN, |b| {
        // The knee drum, a broad black disc on its outside with a white cap.
        b.paint(METAL);
        b.cylinder_between(knee - y * 0.78, knee + y * 0.78, 0.76, 0.76, b.sides(10));
        b.paint(ACCENT);
        b.cylinder_between(
            knee + y * 0.76,
            knee + y * 0.92,
            0.82 * g,
            0.74 * g,
            b.sides(12),
        );
        if b.fine() {
            b.paint(PLATING);
            b.cylinder_between(knee + y * 0.92, knee + y * 0.98, 0.48 * g, 0.42 * g, 12);
        }
        // The shin, and a heavy black calf behind it carrying the ram.
        b.paint(ACCENT);
        b.beam(knee, ankle, v2(1.05, 1.2) * g, v2(0.9, 1.0) * g);
        b.beam(
            knee + v3(-0.5 * g, 0.0, -0.9),
            ankle + v3(-0.45 * g, 0.0, 1.1),
            v2(1.25, 0.8) * g,
            v2(1.0, 0.6) * g,
        );
        // Greave, with a black knee cap standing proud between it and the cuisse.
        b.paint(PLATING);
        b.beam(
            knee + v3(0.36 * g, 0.0, -0.7),
            ankle + v3(0.52 * g, 0.0, 0.55),
            v2(1.55, 1.1) * g,
            v2(1.6, 0.82) * g,
        );
        b.paint(ACCENT);
        b.frustum(
            knee + v3(0.72 * g, 0.0, -0.6),
            v2(0.7, 1.35) * g,
            v2(0.42, 0.9) * g,
            1.35,
            v2(0.13, 0.0),
        );
        if b.fine() {
            // A hazard flash on the greave's outer face, under the knee.
            let side = 0.79 * g;
            b.paint(PLATING).pattern(pattern::HAZARD);
            b.beam(
                knee + v3(0.42 * g, side, -1.3),
                knee + v3(0.5 * g, side, -2.5),
                v2(0.05, 0.75 * g),
                v2(0.05, 0.75 * g),
            );
            // The calf ram.
            b.paint(METAL);
            b.cylinder_between(
                knee + v3(-1.05 * g, 0.0, -0.6),
                ankle + v3(-0.9 * g, 0.0, 0.6),
                0.2,
                0.15,
                6,
            );
            b.module("eng_3", 0.72, |b| {
                // Suite III: a feed conduit down the front of each greave.
                b.paint(ACCENT).pattern(pattern::CONDUIT);
                b.beam(
                    knee + v3(0.91 * g + 0.02, 0.0, -1.1),
                    ankle + v3(0.93 * g + 0.02, 0.0, 0.9),
                    v2(0.3, 0.06),
                    v2(0.26, 0.06),
                );
            });
        }
    });
    b.with_limb(rig::FOOT, |b| {
        b.paint(METAL);
        b.cylinder_between(ankle - y * 0.65, ankle + y * 0.65, 0.58, 0.58, b.sides(8));
    });
}

/// A foot about its ankle (x forward, the ankle at x = 0, y = 0, the sole at z = 0): a
/// broad armoured boot, not a track. A dark tread under a black upper with a white toe
/// cap and instep, a heel block behind and black guards either side of the ankle.
fn foot(b: &mut MeshBuilder) {
    let (rear, front, width) = FOOT;
    let (x0, x1, h, w) = (-rear, front, 1.1, width * 0.5);
    let sole = if b.fine() { 0.28 } else { 0.0 };
    if b.fine() {
        b.paint(TREAD);
        b.extrude_y(&[[x0, 0.0], [x1, 0.0], [x1, sole], [x0, sole]], -w, w);
    }
    b.paint(ACCENT);
    let upper = [
        [x0 + 0.05, sole],
        [x1 - 0.05, sole],
        [x1 - 0.05, 0.42 * h],
        [x1 - 1.1, h],
        [x0 + 0.7, h],
        [x0 + 0.05, 0.6 * h],
    ];
    b.extrude_y_chamfered(&upper, w - 0.06, 0.12);
    if !b.fine() {
        return;
    }
    b.paint(PLATING);
    on_slope(b, upper[2], upper[3], 0.5, |b| {
        b.plate(Vec3::ZERO, v2(1.05, width - 0.5), 0.14, 0.05)
    });
    b.plate(v3(0.15, 0.0, h), v2(1.3, width - 0.55), 0.1, 0.04);
    // Heel block and the guards either side of the ankle, black.
    b.paint(ACCENT);
    b.chamfered_box(v3(x0 + 0.4, 0.0, 0.8 * h), v3(0.6, 1.2, 0.8 * h), 0.12);
    b.mirror_y(|b| {
        b.extrude_y(
            &[[-0.65, h], [0.55, h], [0.35, 1.85], [-0.5, 1.95]],
            0.68,
            0.86,
        )
    });
}
