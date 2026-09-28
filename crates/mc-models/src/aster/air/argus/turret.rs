//! The Argus's reclaim turret, hung under the belly: a gun house of its own (weapon
//! slot 0, `reclaimer.heads[0]` in air.ron). A slewing ring under the keel turns
//! with the head's yaw; what is inside `with_recoil` pitches about the trunnion, from
//! level down to straight below, since it works from cruise height at wrecks up to
//! 1200 m out. Everything that swings up when it looks down stays clear of the ring.
//!
//! Reclaim is plant, not a weapon: a processor, an intake mouth, the salvage showing
//! through glazed panels (`pattern::MASS_FLOW`), the lit throat (`GLOW_MATERIALS`).
//! Coordinates are the airframe's authored ones (before `SCALE` and `LIFT`).

use super::*;

/// Where the keel is over the turret: the slewing ring sits under it.
const KEEL: f32 = 0.46;
/// The trunnion (authored) and the intake mouth the beam leaves from, at rest.
const CRADLE: Vec3 = Vec3::new(0.3, 0.0, -0.5);
const CRADLE_MOUTH: Vec3 = Vec3::new(1.3, 0.0, -0.5);

/// The Cradle turret under the keel: its fixed fairing, then gun house 0.
pub(super) fn build(b: &mut MeshBuilder) {
    let pivot = CRADLE;
    // The fixed part: a dark fairing let into the keel, the ring's race under it.
    b.paint(PLATING_DARK);
    b.prism(v3(pivot.x, 0.0, KEEL - 0.16), b.sides(10), 0.52, 0.5, 0.2);
    b.with_house(0, pivot, 0.0, |b| {
        b.paint(METAL);
        b.prism(v3(pivot.x, 0.0, KEEL - 0.26), b.sides(12), 0.58, 0.58, 0.1);
        cradle(b, pivot);
    });
}

/// The round intake mouth facing +x at `at`: a dark lip, two vanes, the lit throat.
fn mouth(b: &mut MeshBuilder, at: Vec3, r: f32) {
    b.paint(ACCENT);
    b.cylinder_between(at - Vec3::X * 0.12, at, r, r * 0.92, b.sides(10));
    b.paint(GLOW_MATERIALS);
    b.cylinder_between(
        at - Vec3::X * 0.03,
        at + Vec3::X * 0.01,
        r * 0.6,
        r * 0.6,
        8,
    );
    if b.fine() {
        b.paint(ACCENT);
        b.mirror_y(|b| b.cuboid(at + v3(0.04, r * 0.66, 0.0), v3(0.18, r * 0.2, r * 1.3)));
    }
}

/// The Cradle, hanging: two long cheeks from the ring down past the trunnion, the
/// processor box slung between them, a charge pack aft, a short snout forward.
fn cradle(b: &mut MeshBuilder, p: Vec3) {
    let top = KEEL - 0.26;
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.block(v3(p.x - 0.36, -0.52, top - 0.12), v3(p.x + 0.26, 0.52, top));
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_y(
            &[
                [p.x - 0.36, top - 0.12],
                [p.x + 0.26, top - 0.12],
                [p.x + 0.26, p.z + 0.1],
                [p.x + 0.08, p.z - 0.26],
                [p.x - 0.18, p.z - 0.26],
                [p.x - 0.32, p.z - 0.05],
            ],
            0.38,
            0.52,
        );
        b.paint(METAL);
        b.cylinder_between(
            v3(p.x, 0.32, p.z),
            v3(p.x, 0.58, p.z),
            0.13,
            0.11,
            b.sides(8),
        );
        if b.fine() {
            // A dark stiffening rib down each cheek.
            b.paint(ACCENT);
            b.beam(
                v3(p.x - 0.05, 0.535, top - 0.14),
                v3(p.x - 0.05, 0.535, p.z + 0.14),
                v2(0.03, 0.12),
                v2(0.03, 0.12),
            );
        }
    });
    b.with_recoil(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        if b.fine() {
            b.chamfered_box(p + Vec3::X * 0.06, v3(0.93, 0.68, 0.53), 0.13);
        } else {
            b.cuboid(p + Vec3::X * 0.06, v3(0.93, 0.68, 0.53));
        }
        b.paint(ACCENT);
        b.chamfered_box(p + v3(0.06, 0.0, -0.06), v3(0.97, 0.72, 0.14), 0.14);
        // The charge pack behind the trunnion, short enough to clear the ring.
        b.block(p + v3(-0.56, -0.24, -0.19), p + v3(-0.38, 0.24, 0.2));
        // Glazed flanks: the salvage streaming through the processor.
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.block(p + v3(-0.2, -0.35, -0.02), p + v3(0.34, 0.35, 0.14));
        }
        // The snout: a tapering tube, a collar, the mouth.
        b.paint(METAL);
        b.cylinder_between(p + Vec3::X * 0.5, p + Vec3::X * 0.9, 0.21, 0.18, b.sides(8));
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(p + Vec3::X * 0.62, p + Vec3::X * 0.68, 0.23, 0.23, 8);
        }
        mouth(b, CRADLE_MOUTH, 0.25);
    });
}
