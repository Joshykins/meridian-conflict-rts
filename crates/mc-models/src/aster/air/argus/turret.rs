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
use glam::Vec2;

/// The reclaim turret designs on offer until the user picks one.
#[derive(Clone, Copy)]
pub(in crate::aster) enum Turret {
    /// The Thresher's Cradle hung upside down from the belly: a processor box on side
    /// trunnions between two long yoke cheeks, a short snout, a charge pack aft.
    Cradle,
    /// A side-arm mount: one heavy arm down the left carries a trunnion drum, the
    /// processor and a flared, vaned intake hood cantilevered off it.
    Hood,
    /// A trunnion drum across the head between two rounded cheeks, a stout square
    /// intake with a mantlet out of its front.
    Drum,
}

/// Where the keel is over the turret: the slewing ring sits under it.
const KEEL: f32 = 0.46;
/// Trunnions (authored) and the intake mouths the beam leaves from, at rest.
const CRADLE: Vec3 = Vec3::new(0.3, 0.0, -0.5);
const CRADLE_MOUTH: Vec3 = Vec3::new(1.3, 0.0, -0.5);
const HOOD: Vec3 = Vec3::new(0.3, 0.0, -0.45);
const HOOD_MOUTH: Vec3 = Vec3::new(1.2, 0.0, -0.45);
const DRUM: Vec3 = Vec3::new(0.3, 0.0, -0.42);
const DRUM_MOUTH: Vec3 = Vec3::new(1.22, 0.0, -0.42);

pub(super) fn build(b: &mut MeshBuilder, turret: Turret) {
    let pivot = match turret {
        Turret::Cradle => CRADLE,
        Turret::Hood => HOOD,
        Turret::Drum => DRUM,
    };
    // The fixed part: a dark fairing let into the keel, the ring's race under it.
    b.paint(PLATING_DARK);
    b.prism(v3(pivot.x, 0.0, KEEL - 0.16), b.sides(10), 0.52, 0.5, 0.2);
    b.with_house(0, pivot, 0.0, |b| {
        b.paint(METAL);
        b.prism(v3(pivot.x, 0.0, KEEL - 0.26), b.sides(12), 0.58, 0.58, 0.1);
        match turret {
            Turret::Cradle => cradle(b, pivot),
            Turret::Hood => hood(b, pivot),
            Turret::Drum => drum(b, pivot),
        }
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

/// A square intake mouth facing +x at `at`, `half` its half width and height: a dark
/// frame, horizontal vanes across the lit throat.
fn square_mouth(b: &mut MeshBuilder, at: Vec3, half: Vec2) {
    b.paint(ACCENT);
    b.beam(
        at - Vec3::X * 0.1,
        at,
        half * 2.0 + Vec2::splat(0.04),
        half * 2.0,
    );
    b.paint(GLOW_MATERIALS);
    b.cuboid(at + Vec3::X * 0.005, v3(0.02, half.x * 1.5, half.y * 1.5));
    if b.fine() {
        b.paint(ACCENT);
        for k in [-1.0, 0.0, 1.0] {
            b.cuboid(
                at + v3(0.03, 0.0, k * half.y * 0.5),
                v3(0.08, half.x * 1.7, 0.035),
            );
        }
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

/// The side-arm mount: a heavy arm down the left side to a trunnion drum, the
/// processor and its flared intake hood cantilevered off it, a radiator aft.
fn hood(b: &mut MeshBuilder, p: Vec3) {
    let top = KEEL - 0.26;
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.extrude_y(
        &[
            [p.x - 0.42, top],
            [p.x + 0.3, top],
            [p.x + 0.24, p.z + 0.1],
            [p.x + 0.06, p.z - 0.24],
            [p.x - 0.2, p.z - 0.24],
            [p.x - 0.36, p.z + 0.02],
        ],
        0.4,
        0.62,
    );
    // A brace from the ring across to the arm, and the trunnion drum.
    b.paint(ACCENT);
    b.beam(
        v3(p.x - 0.36, 0.0, top - 0.02),
        v3(p.x - 0.36, 0.42, top - 0.3),
        v2(0.16, 0.1),
        v2(0.16, 0.1),
    );
    b.paint(METAL);
    b.cylinder_between(
        v3(p.x, 0.3, p.z),
        v3(p.x, 0.7, p.z),
        0.24,
        0.22,
        b.sides(10),
    );
    b.with_recoil(|b| {
        // The processor on the trunnion.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        if b.fine() {
            b.chamfered_box(p + Vec3::X * -0.08, v3(0.72, 0.62, 0.52), 0.1);
        } else {
            b.cuboid(p + Vec3::X * -0.08, v3(0.72, 0.62, 0.52));
        }
        // The hood flaring forward to its square mouth, two bands round it.
        let ring = |x: f32, w: f32, h: f32| {
            vec![
                v3(p.x + x, -w, p.z - h),
                v3(p.x + x, w, p.z - h),
                v3(p.x + x, w, p.z + h),
                v3(p.x + x, -w, p.z + h),
            ]
        };
        b.paint(ACCENT);
        b.loft(
            &[ring(0.26, 0.26, 0.2), ring(0.82, 0.36, 0.28)],
            false,
            false,
        );
        if b.fine() {
            b.paint(METAL);
            for (x, w, h) in [(0.42, 0.3, 0.24), (0.66, 0.35, 0.27)] {
                b.cuboid(p + Vec3::X * x, v3(0.05, 2.0 * w, 2.0 * h));
            }
            // A glazed window on the open (right) flank.
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.block(p + v3(-0.3, -0.32, -0.1), p + v3(0.16, -0.3, 0.12));
            // Radiator fins aft of the processor.
            b.paint(ACCENT);
            for y in [-0.18, 0.0, 0.18] {
                b.block(
                    p + v3(-0.58, y - 0.03, -0.16),
                    p + v3(-0.44, y + 0.03, 0.16),
                );
            }
        }
        square_mouth(b, HOOD_MOUTH, v2(0.38, 0.3));
    });
}

/// A trunnion drum across the head between two rounded cheeks, a stout square intake
/// with a mantlet out of the drum's front.
fn drum(b: &mut MeshBuilder, p: Vec3) {
    let top = KEEL - 0.26;
    let r = 0.36;
    b.mirror_y(|b| {
        // The cheek: straight down from the ring, rounded round the drum's end.
        let mut plan = vec![[p.x - 0.44, top], [p.x + 0.44, top]];
        let n = if b.fine() { 8 } else { 4 };
        for k in 0..=n {
            let a = std::f32::consts::PI * k as f32 / n as f32;
            plan.push([p.x + 0.44 * a.cos(), p.z - 0.44 * a.sin()]);
        }
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_y(&plan, 0.36, 0.5);
        if b.fine() {
            b.paint(METAL);
            b.cylinder_between(v3(p.x, 0.5, p.z), v3(p.x, 0.56, p.z), 0.2, 0.18, 8);
        }
    });
    b.with_recoil(|b| {
        b.paint(ACCENT);
        b.cylinder_between(p - Vec3::Y * 0.34, p + Vec3::Y * 0.34, r, r, b.sides(12));
        if b.fine() {
            // The salvage runs round the drum under a glazed band.
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.cylinder_between(
                p - Vec3::Y * 0.12,
                p + Vec3::Y * 0.12,
                r + 0.01,
                r + 0.01,
                12,
            );
            b.paint(METAL);
            b.mirror_y(|b| {
                b.cylinder_between(
                    p + Vec3::Y * 0.3,
                    p + Vec3::Y * 0.35,
                    r + 0.02,
                    r + 0.02,
                    12,
                )
            });
        }
        // The mantlet, then the square intake body with two ribs.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.block(p + v3(0.28, -0.3, -0.26), p + v3(0.4, 0.3, 0.26));
        b.beam(
            p + Vec3::X * 0.4,
            p + Vec3::X * 1.12,
            v2(0.46, 0.42),
            v2(0.42, 0.38),
        );
        if b.fine() {
            b.paint(ACCENT);
            for x in [0.62, 0.9] {
                b.cuboid(p + Vec3::X * x, v3(0.06, 0.5, 0.46));
            }
        }
        square_mouth(b, DRUM_MOUTH, v2(0.22, 0.2));
    });
}
