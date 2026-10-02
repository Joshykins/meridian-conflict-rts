//! The launcher: a turning ring on the back, a bronze hinge on brackets at its rear, and
//! the pod on the hinge: two tubes under one faceted shroud, a keeled plate down its
//! spine swept back past the hinge, the tube mouths ringed in bronze and lit red inside.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::super::commander::form::{blade, ring, sleeve, OCT};
use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::strut;
use super::{MUZZLES, PIVOT, RING};

/// The pod's axis at rest, from the hinge toward the mouths.
fn axis() -> Vec3 {
    (MUZZLES[0].with_y(0.0) - PIVOT).normalize()
}

pub(super) fn mount(b: &mut MeshBuilder) {
    b.with_part(part::TURRET, |b| {
        // The ring: a dark plated turntable on the bronze bed.
        dark_plate(b);
        let sides = b.sides(12);
        b.cylinder_between(RING, RING + Vec3::Z * 0.22, 1.08, 1.0, sides);
        // The hinge brackets: a plated strut each side up from the ring's rear.
        b.mirror_y(|b| {
            strut(
                b,
                v3(-0.55, 0.6, RING.z + 0.2),
                PIVOT + v3(0.0, 0.6, 0.0),
                0.14,
            );
        });
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(
            PIVOT - Vec3::Y * 0.78,
            PIVOT + Vec3::Y * 0.78,
            0.17,
            0.17,
            sides,
        );
        b.with_limb(rig::ARM_GUN, pod);
    });
}

/// The pod, at rest: it pitches about the hinge as one piece.
fn pod(b: &mut MeshBuilder) {
    let u = axis();
    let up = Vec3::new(-u.z, 0.0, u.x);
    let at = |s: f32, h: f32| PIVOT + u * s + up * h;
    let len = (MUZZLES[0].with_y(0.0) - PIVOT).length();
    // The shroud: a faceted body over both tubes, the mouths' collars standing out of
    // its front face.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(at(-0.35, 0.05), up, 0.75, 0.32),
            ring(at(0.2, 0.08), up, 1.0, 0.44),
            ring(at(len - 0.55, 0.08), up, 1.02, 0.46),
            ring(at(len - 0.3, 0.06), up, 0.96, 0.42),
        ],
        &OCT,
    );
    // The spine plate, its point swept back past the hinge.
    blade(b, at(len - 0.55, 0.52), -u, up, len + 0.2, 0.42, 0.0, 0.16);
    // The owner's colour along the shroud's top.
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.beam(
            at(len - 0.75, 0.53) + Vec3::Y * 0.5,
            at(0.5, 0.53) + Vec3::Y * 0.5,
            Vec2::new(0.1, 0.03),
            Vec2::new(0.1, 0.03),
        )
    });
    for m in MUZZLES {
        // The mouth: a bronze collar, its rim on the muzzle.
        metal(b);
        let sides = b.sides(10);
        b.cylinder_between(m - u * 0.35, m, 0.34, 0.3, sides);
        // The seeker waiting in the tube, lit red.
        b.paint(GLOW_LASER);
        b.cylinder_between(m - u * 0.3, m - u * 0.08, 0.22, 0.22, b.sides(8));
    }
}

/// Far off: the ring a block, the pod a box on it.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            let mid = MUZZLES[0].with_y(0.0);
            b.beam(PIVOT, mid, Vec2::new(1.9, 0.8), Vec2::new(1.9, 0.8));
            // A sliver at each mouth for the turret to reach.
            for m in MUZZLES {
                b.face(&[
                    m,
                    m - axis() * 0.3 + Vec3::Z * 0.2,
                    m - axis() * 0.3 - Vec3::Z * 0.2,
                ]);
            }
        });
    });
}
