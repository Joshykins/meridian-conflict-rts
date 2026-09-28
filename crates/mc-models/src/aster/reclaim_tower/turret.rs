//! The reclaim tower's head: gun house 0 on the head house's roof. What only turns
//! (turntable, cheeks, drives) is built round the tower's axis; what pitches is built
//! round the trunnion at [`super::PIVOT_Z`] and ends at the barrel's mouth,
//! [`super::EMIT_X`] ahead of it. The head is the Thresher's Cradle at tower scale
//! with a long barrel: heavy plant, not a gun. The barrel is the same length at every
//! tier and gains a ribbed cooling jacket at tech 2 and a collector shroud round its
//! mouth at tech 3.

use glam::{Affine3A, Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::{EMIT_X, K, MUZZLE, PIVOT_Z, RING_TOP};
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// The head's machinery (drive house, feed drop) at the scale the Thresher's Cradle
/// was first fitted to the tower with: `K` over its authored 4.5 m unit.
const S: f32 = K / 4.52;

/// The head: its fixed race on the head house, then gun house 0.
pub(super) fn turret(b: &mut MeshBuilder, tech: u8) {
    let pivot = v3(0.0, 0.0, PIVOT_Z);
    let emit = EMIT_X;
    if b.coarse() {
        b.with_house(0, pivot, 0.0, |b| {
            b.paint(PLATING);
            b.cuboid_open(
                v3(-0.8 * S, 0.0, (RING_TOP + PIVOT_Z) * 0.5 + 0.3 * S),
                v3(5.6 * S, 5.2 * S, PIVOT_Z - RING_TOP + 2.6 * S),
            );
            team_panel(
                b,
                v3(-0.8 * S, 0.0, PIVOT_Z + 1.6 * S),
                Vec2::new(3.0 * S, 3.0 * S),
            );
            b.with_recoil(|b| {
                b.paint(PLATING_DARK);
                b.beam(
                    pivot + Vec3::X * 0.7 * K,
                    pivot + Vec3::X * emit,
                    Vec2::splat(0.7 * K),
                    Vec2::splat(0.6 * K),
                );
            });
        });
        return;
    }
    // The slewing ring's race, fixed on the roof.
    b.paint(ACCENT);
    b.prism(
        v3(0.0, 0.0, RING_TOP - 0.3),
        b.sides(12),
        3.6 * S,
        3.6 * S,
        0.3,
    );
    b.with_house(0, pivot, 0.0, |b| {
        b.paint(PLATING_DARK);
        b.prism(
            v3(0.0, 0.0, RING_TOP),
            b.sides(12),
            3.9 * S,
            3.7 * S,
            0.55 * S,
        );
        if b.fine() {
            b.paint(METAL);
            b.prism(
                v3(0.0, 0.0, RING_TOP + 0.05),
                12,
                3.96 * S,
                3.96 * S,
                0.14 * S,
            );
        }
    });
    cradle(b, pivot, RING_TOP + 0.55 * S, tech);
}

/// The Thresher's Cradle head at tower scale (the user's favourite reclaim head): two
/// plated yoke cheeks rising past the trunnion, a bridge low between them, and slung in
/// them a boxy processor with a charge pack aft and a long barrel forward. Round it, the
/// drive house and feed drop a head this size needs.
fn cradle(b: &mut MeshBuilder, pivot: Vec3, top: f32, tech: u8) {
    let k = K;
    let pz = pivot.z;
    b.with_house(0, pivot, 0.0, |b| {
        // Bridge and cheeks.
        b.paint(PLATING);
        b.block(
            v3(-0.55 * k, -0.62 * k, top),
            v3(0.3 * k, 0.62 * k, top + 0.18 * k),
        );
        let crown = pz + 0.42 * k;
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.with_bevel(0.06 * k, |b| {
                b.extrude_y(
                    &[
                        [-0.5 * k, top],
                        [0.34 * k, top],
                        [0.3 * k, crown - 0.1 * k],
                        [0.08 * k, crown],
                        [-0.3 * k, crown],
                        [-0.5 * k, pz - 0.1 * k],
                    ],
                    0.62 * k,
                    0.8 * k,
                );
            });
            b.paint(METAL);
            b.cylinder_between(
                v3(0.0, 0.56 * k, pz),
                v3(0.0, 0.9 * k, pz),
                0.2 * k,
                0.16 * k,
                b.sides(10),
            );
            if b.fine() {
                // A dark armour plate let into the cheek, and the elevation ram inside it.
                b.paint(PLATING_DARK);
                b.extrude_y(
                    &[
                        [-0.36 * k, top + 0.08 * k],
                        [0.2 * k, top + 0.08 * k],
                        [0.16 * k, pz - 0.26 * k],
                        [-0.36 * k, pz - 0.26 * k],
                    ],
                    0.8 * k - 0.02,
                    0.83 * k,
                );
                b.paint(METAL);
                b.cylinder_between(
                    v3(-0.42 * k, 0.56 * k, top + 0.1 * k),
                    v3(-0.2 * k, 0.56 * k, pz - 0.25 * k),
                    0.06 * k,
                    0.05 * k,
                    6,
                );
            }
            // Tech 3: an outer armour cheek bolted over the yoke.
            kit(b, tech, 3, 0.85, |b| {
                b.paint(PLATING);
                b.extrude_y(
                    &[
                        [-0.44 * k, top + 0.04 * k],
                        [0.3 * k, top + 0.04 * k],
                        [0.24 * k, crown - 0.16 * k],
                        [-0.26 * k, crown - 0.06 * k],
                    ],
                    0.83 * k,
                    0.9 * k,
                );
            });
        });
        // A drive house on the turntable's back edge, under the charge pack's swing.
        b.paint(PLATING_DARK);
        b.chamfered_box(
            v3(-2.9 * S, 0.0, top + 0.5 * S),
            v3(1.4 * S, 3.8 * S, 1.0 * S),
            0.3 * S,
        );
        if b.fine() {
            vent(
                b,
                v3(-2.9 * S, 1.1 * S, top + 1.0 * S),
                Vec2::new(0.9 * S, 0.7 * S),
                3,
                ACCENT,
            );
            feed_drop(b, pivot, top);
        }
        b.with_recoil(|b| {
            cradle_box(b, pivot, k);
            barrel(b, pivot, k, tech);
        });
    });
}

/// What pitches in the Cradle behind the barrel: the processor box and charge pack.
fn cradle_box(b: &mut MeshBuilder, p: Vec3, k: f32) {
    b.paint(PLATING);
    b.with_bevel(0.03 * k, |b| {
        b.chamfered_box(
            p + Vec3::X * 0.1 * k,
            v3(1.5 * k, 1.1 * k, 0.86 * k),
            0.22 * k,
        );
    });
    b.paint(ACCENT);
    b.chamfered_box(
        p + v3(0.1 * k, 0.0, -0.1 * k),
        v3(1.56 * k, 1.16 * k, 0.22 * k),
        0.24 * k,
    );
    // Roof: a lid with the owner's colour.
    b.paint(PLATING);
    b.plate(
        p + v3(-0.05 * k, 0.0, 0.43 * k),
        Vec2::new(1.1 * k, 0.8 * k),
        0.05 * k,
        0.02 * k,
    );
    team_panel(
        b,
        p + v3(-0.2 * k, 0.0, 0.48 * k),
        Vec2::new(0.5 * k, 0.5 * k),
    );
    // Charge pack behind the trunnion balances the barrel.
    b.paint(ACCENT);
    b.block(
        p + v3(-1.1 * k, -0.4 * k, -0.3 * k),
        p + v3(-0.6 * k, 0.4 * k, 0.34 * k),
    );
    if b.fine() {
        b.paint(PLATING_DARK);
        b.block(
            p + v3(-1.13 * k, -0.3 * k, -0.2 * k),
            p + v3(-1.1 * k, 0.3 * k, 0.24 * k),
        );
    }
}

/// The barrel from the processor box's face to the mouth at `MUZZLE`: a breech collar,
/// a long tapering tube banded along its length and carried by stays from the box's
/// roof, feed lines down its flanks, a slotted collector sleeve, then the vaned mouth.
fn barrel(b: &mut MeshBuilder, p: Vec3, k: f32, tech: u8) {
    let x = |u: f32| p + Vec3::X * u * k;
    let tip = x(MUZZLE);
    let sleeve = MUZZLE - 0.42;
    let sides = b.sides(12);
    // Breech collar where the barrel leaves the box.
    b.paint(PLATING_DARK);
    b.cylinder_between(x(0.8), x(1.0), 0.44 * k, 0.4 * k, sides);
    // The tube.
    b.paint(METAL);
    b.cylinder_between(x(0.95), x(sleeve + 0.05), 0.3 * k, 0.25 * k, sides);
    // Bands along it.
    if b.mid() {
        b.paint(ACCENT);
        for u in [1.12, 1.55] {
            b.cylinder_between(x(u), x(u + 0.07), 0.34 * k, 0.33 * k, sides);
        }
    }
    // The collector sleeve: wider, dark, with the mouth ring in front of it.
    b.paint(PLATING_DARK);
    b.cylinder_between(x(sleeve), x(MUZZLE - 0.1), 0.36 * k, 0.38 * k, sides);
    if b.fine() {
        // Slots down the sleeve show the Materials glow drawn into it.
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        round_bore(b, p, |b| {
            b.block(
                x(sleeve + 0.06) + v3(0.0, -0.05 * k, 0.33 * k),
                x(MUZZLE - 0.16) + v3(0.0, 0.05 * k, 0.39 * k),
            );
        });
    }
    if b.fine() {
        // Stays from the box's roof out to the first band, and feed lines along the
        // flanks from the charge pack to the sleeve.
        b.paint(PLATING_DARK);
        b.beam(
            p + v3(0.55 * k, 0.0, 0.43 * k),
            x(1.5) + Vec3::Z * 0.28 * k,
            Vec2::new(0.16 * k, 0.1 * k),
            Vec2::new(0.1 * k, 0.08 * k),
        );
        b.paint(METAL);
        b.mirror_y(|b| {
            b.cylinder_between(
                p + v3(-0.8 * k, 0.5 * k, 0.1 * k),
                p + v3(0.9 * k, 0.5 * k, 0.1 * k),
                0.06 * k,
                0.06 * k,
                6,
            );
            b.cylinder_between(
                p + v3(0.9 * k, 0.5 * k, 0.1 * k),
                x(1.2) + v3(0.0, 0.3 * k, 0.0),
                0.06 * k,
                0.05 * k,
                6,
            );
            b.cylinder_between(
                x(1.2) + v3(0.0, 0.3 * k, 0.0),
                x(sleeve) + v3(0.0, 0.3 * k, 0.0),
                0.05 * k,
                0.05 * k,
                6,
            );
        });
    }
    // Tech 2: a ribbed cooling jacket over the tube's middle.
    kit(b, tech, 2, 0.8, |b| {
        b.paint(PLATING_DARK);
        b.cylinder_between(x(1.22), x(1.52), 0.33 * k, 0.32 * k, sides);
        if b.mid() {
            b.paint(METAL);
            for i in 0..5 {
                let u = 1.25 + i as f32 * 0.06;
                b.cylinder_between(x(u), x(u + 0.025), 0.37 * k, 0.37 * k, sides);
            }
        }
    });
    // Tech 3: a square collector shroud round the mouth on four stand-off vanes.
    kit(b, tech, 3, 0.9, |b| {
        b.paint(PLATING);
        round_bore(b, p, |b| {
            b.block(
                x(MUZZLE - 0.34) + v3(0.0, -0.5 * k, 0.5 * k),
                x(MUZZLE - 0.04) + v3(0.0, 0.5 * k, 0.58 * k),
            );
            b.block(
                x(MUZZLE - 0.3) + v3(0.0, -0.04 * k, 0.36 * k),
                x(MUZZLE - 0.08) + v3(0.0, 0.04 * k, 0.5 * k),
            );
        });
    });
    round_mouth(b, tip, 0.44 * k);
}

/// Emits `f` four times, each turned a further quarter about the bore (the x axis
/// through the trunnion `p`).
fn round_bore(b: &mut MeshBuilder, p: Vec3, f: impl Fn(&mut MeshBuilder)) {
    for i in 0..4 {
        let turn = Affine3A::from_translation(p)
            * Affine3A::from_rotation_x(i as f32 * std::f32::consts::FRAC_PI_2)
            * Affine3A::from_translation(-p);
        b.with(turn, |b| f(b));
    }
}

/// A round intake mouth facing +x, its face at `at`: a dark ring, collector vanes
/// across it, the Materials glow in the throat.
fn round_mouth(b: &mut MeshBuilder, at: Vec3, radius: f32) {
    let sides = b.sides(12);
    b.paint(ACCENT);
    b.cylinder_between(at - Vec3::X * 0.5 * radius, at, radius, radius * 0.9, sides);
    b.paint(GLOW_MATERIALS);
    b.cylinder_between(
        at - Vec3::X * 0.05,
        at + Vec3::X * 0.02,
        radius * 0.6,
        radius * 0.6,
        b.sides(10),
    );
    if b.mid() {
        b.paint(ACCENT);
        for y in [1.0, -1.0] {
            b.cuboid(
                at + v3(0.0, y * 0.4 * radius, 0.0),
                v3(0.3 * radius, 0.1 * radius, 1.2 * radius),
            );
        }
        b.cuboid(at, v3(0.3 * radius, 1.2 * radius, 0.1 * radius));
    }
}

/// The feed from under the head down through the turntable into the tower.
fn feed_drop(b: &mut MeshBuilder, pivot: Vec3, top: f32) {
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.cylinder_between(
        v3(-1.2 * S, 0.0, pivot.z - 1.2 * S),
        v3(-0.6 * S, 0.0, top - 0.4 * S),
        0.45 * S,
        0.45 * S,
        8,
    );
}
