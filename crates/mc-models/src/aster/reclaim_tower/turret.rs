//! The reclaim tower's head: gun house 0 on the head house's roof. What only turns
//! (turntable, cheeks, drives) is built round the tower's axis; what pitches is built
//! round the trunnion at [`super::PIVOT_Z`] and ends at the barrel's tip,
//! [`super::EMIT_X`] ahead of it. The head is the Thresher's Cradle at tower scale
//! with a long, slim barrel: heavy plant, not a gun. The barrel is the same length at
//! every tier, ends in a plain emitter head (no flared mouth), and gains a ribbed
//! cooling jacket at tech 2 and guide rails and induction rings at tech 3.

use glam::{Vec2, Vec3};

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

/// The barrel from the processor box's face to its tip at `MUZZLE`: a breech collar, a
/// slim tube banded along its length and held by a stay from the box's roof, feed lines
/// down its flanks, and a grooved emitter head with the lit bore set into its face.
fn barrel(b: &mut MeshBuilder, p: Vec3, k: f32, tech: u8) {
    let x = |u: f32| p + Vec3::X * u * k;
    let head = MUZZLE - 0.36;
    let sides = b.sides(12);
    // Breech collar where the barrel leaves the box.
    b.paint(PLATING_DARK);
    b.cylinder_between(x(0.8), x(1.0), 0.34 * k, 0.3 * k, sides);
    // The tube.
    b.paint(METAL);
    b.cylinder_between(x(0.95), x(head + 0.02), 0.2 * k, 0.18 * k, sides);
    if b.mid() {
        b.paint(ACCENT);
        for u in [1.12, 1.62] {
            b.cylinder_between(x(u), x(u + 0.05), 0.225 * k, 0.225 * k, sides);
        }
    }
    // The emitter head: grooved, a lip, the bore lit in its face.
    b.paint(PLATING_DARK);
    b.cylinder_between(x(head), x(MUZZLE - 0.04), 0.25 * k, 0.23 * k, sides);
    if b.mid() {
        b.paint(ACCENT);
        for u in [head + 0.08, head + 0.18] {
            b.cylinder_between(x(u), x(u + 0.03), 0.258 * k, 0.255 * k, sides);
        }
    }
    b.paint(ACCENT);
    b.cylinder_between(x(MUZZLE - 0.04), x(MUZZLE), 0.23 * k, 0.2 * k, sides);
    b.paint(GLOW_MATERIALS);
    b.cylinder_between(
        x(MUZZLE) - Vec3::X * 0.05,
        x(MUZZLE) + Vec3::X * 0.02,
        0.12 * k,
        0.12 * k,
        b.sides(10),
    );
    if b.fine() {
        // A stay from the box's roof out to the second band, and feed lines along the
        // flanks from the charge pack to the emitter head.
        b.paint(PLATING_DARK);
        b.beam(
            p + v3(0.55 * k, 0.0, 0.43 * k),
            x(1.62) + Vec3::Z * 0.2 * k,
            Vec2::new(0.14 * k, 0.1 * k),
            Vec2::new(0.08 * k, 0.07 * k),
        );
        b.paint(METAL);
        b.mirror_y(|b| {
            b.cylinder_between(
                p + v3(-0.8 * k, 0.5 * k, 0.1 * k),
                p + v3(0.9 * k, 0.5 * k, 0.1 * k),
                0.05 * k,
                0.05 * k,
                6,
            );
            b.cylinder_between(
                p + v3(0.9 * k, 0.5 * k, 0.1 * k),
                x(1.15) + v3(0.0, 0.24 * k, 0.0),
                0.05 * k,
                0.04 * k,
                6,
            );
            b.cylinder_between(
                x(1.15) + v3(0.0, 0.24 * k, 0.0),
                x(head) + v3(0.0, 0.24 * k, 0.0),
                0.04 * k,
                0.04 * k,
                6,
            );
        });
    }
    // Tech 2: a ribbed cooling jacket over the tube's middle.
    kit(b, tech, 2, 0.8, |b| {
        b.paint(PLATING_DARK);
        b.cylinder_between(x(1.22), x(1.56), 0.235 * k, 0.225 * k, sides);
        if b.mid() {
            b.paint(METAL);
            for i in 0..6 {
                let u = 1.25 + i as f32 * 0.055;
                b.cylinder_between(x(u), x(u + 0.02), 0.27 * k, 0.27 * k, sides);
            }
        }
    });
    // Tech 3: guide rails along the flanks and induction rings ahead of the jacket.
    kit(b, tech, 3, 0.9, |b| {
        b.paint(PLATING);
        b.mirror_y(|b| {
            b.beam(
                x(1.0) + v3(0.0, 0.27 * k, 0.0),
                x(head - 0.02) + v3(0.0, 0.25 * k, 0.0),
                Vec2::new(0.07 * k, 0.12 * k),
                Vec2::new(0.07 * k, 0.1 * k),
            );
        });
        b.paint(ACCENT);
        for u in [1.72, 1.8] {
            b.cylinder_between(x(u), x(u + 0.04), 0.3 * k, 0.3 * k, sides);
        }
    });
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
