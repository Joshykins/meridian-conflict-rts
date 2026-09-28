//! The reclaim tower's head: gun house 0 on the head house's roof. What only turns
//! (turntable, cheeks, drives) is built round the tower's axis; what pitches is
//! built round the trunnion at [`super::PIVOT_Z`] and ends at the intake mouth,
//! [`super::EMIT_X`] ahead of it. The head is the Thresher's Cradle at tower scale. Heavy plant, not a gun: square-shouldered housings,
//! vane grilles over dark intakes, the only light the Materials glow in a working throat.

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::{ring_top, tier, EMIT_X, HEAD_SCALE, PIVOT_Z, REACH};
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// The head: its fixed race on the head house, then gun house 0.
pub(super) fn turret(b: &mut MeshBuilder, tech: u8) {
    let t = tier(tech);
    let (pz, s, emit) = (PIVOT_Z[t], HEAD_SCALE[t], EMIT_X[t]);
    let ring = ring_top(tech);
    let pivot = v3(0.0, 0.0, pz);
    if b.coarse() {
        b.with_house(0, pivot, 0.0, |b| {
            b.paint(PLATING);
            b.cuboid(
                v3(-0.8 * s, 0.0, (ring + pz) * 0.5 + 0.3 * s),
                v3(5.6 * s, 5.2 * s, pz - ring + 2.6 * s),
            );
            team_panel(
                b,
                v3(-0.8 * s, 0.0, pz + 1.6 * s),
                Vec2::new(3.0 * s, 3.0 * s),
            );
            b.with_recoil(|b| {
                b.paint(PLATING_DARK);
                b.cuboid(v3(emit * 0.6, 0.0, pz), v3(emit * 0.8, 2.6 * s, 2.6 * s));
            });
        });
        return;
    }
    // The slewing ring's race, fixed on the roof.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, ring - 0.3), b.sides(12), 3.4 * s, 3.4 * s, 0.3);
    b.with_house(0, pivot, 0.0, |b| {
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, ring), b.sides(12), 3.7 * s, 3.5 * s, 0.55 * s);
        if b.fine() {
            b.paint(METAL);
            b.prism(v3(0.0, 0.0, ring + 0.05), 12, 3.76 * s, 3.76 * s, 0.14 * s);
        }
    });
    let top = ring + 0.55 * s;
    cradle(b, pivot, top, s);
}

/// The Thresher's Cradle head at tower scale (the user's favourite reclaim head): two
/// plated yoke cheeks rising past the trunnion, a bridge low between them, and slung in
/// them a boxy processor with a charge pack aft and a short snout forward ending in a
/// vaned round mouth. Round it, the drive house and feed drop a head this size needs.
fn cradle(b: &mut MeshBuilder, pivot: Vec3, top: f32, s: f32) {
    // The Thresher's head is authored with its mouth 1.55 of its scale out.
    let k = REACH * s / 1.55;
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
        });
        // A drive house on the turntable's back edge, under the charge pack's swing.
        b.paint(PLATING_DARK);
        b.chamfered_box(
            v3(-2.9 * s, 0.0, top + 0.5 * s),
            v3(1.4 * s, 3.8 * s, 1.0 * s),
            0.3 * s,
        );
        if b.fine() {
            vent(
                b,
                v3(-2.9 * s, 1.1 * s, top + 1.0 * s),
                Vec2::new(0.9 * s, 0.7 * s),
                3,
                ACCENT,
            );
            feed_drop(b, pivot, top, s);
        }
        b.with_recoil(|b| cradle_box(b, pivot, k));
    });
}

/// What pitches in the Cradle: the processor box, charge pack, snout and mouth.
fn cradle_box(b: &mut MeshBuilder, p: Vec3, k: f32) {
    let tip = p + Vec3::X * 1.55 * k;
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
    // Charge pack behind the trunnion balances the snout.
    b.paint(ACCENT);
    b.block(
        p + v3(-1.05 * k, -0.38 * k, -0.3 * k),
        p + v3(-0.6 * k, 0.38 * k, 0.32 * k),
    );
    // Snout: a tube out of the box, a collar, then the mouth.
    b.paint(METAL);
    b.cylinder_between(
        p + Vec3::X * 0.8 * k,
        tip - Vec3::X * 0.1 * k,
        0.34 * k,
        0.28 * k,
        b.sides(10),
    );
    if b.mid() {
        b.paint(ACCENT);
        b.cylinder_between(
            p + Vec3::X * 1.0 * k,
            p + Vec3::X * 1.1 * k,
            0.37 * k,
            0.37 * k,
            b.sides(10),
        );
    }
    if b.fine() {
        // Feed pipes from the pack along the flanks into the snout.
        b.paint(METAL);
        b.mirror_y(|b| {
            b.cylinder_between(
                p + v3(-0.8 * k, 0.5 * k, 0.1 * k),
                p + v3(0.85 * k, 0.5 * k, 0.1 * k),
                0.06 * k,
                0.06 * k,
                6,
            );
        });
    }
    round_mouth(b, tip, 0.4 * k);
}

/// A round intake mouth facing +x, its face at `at`: a dark ring, collector vanes
/// across it, the Materials glow in the throat.
fn round_mouth(b: &mut MeshBuilder, at: Vec3, radius: f32) {
    let sides = b.sides(10);
    b.paint(ACCENT);
    b.cylinder_between(at - Vec3::X * 0.3 * radius, at, radius, radius * 0.9, sides);
    b.paint(GLOW_MATERIALS);
    b.cylinder_between(
        at - Vec3::X * 0.05,
        at + Vec3::X * 0.02,
        radius * 0.55,
        radius * 0.55,
        b.sides(8),
    );
    if b.mid() {
        b.paint(ACCENT);
        for y in [1.0, -1.0] {
            b.cuboid(
                at + v3(0.12 * radius, y * 0.72 * radius, 0.0),
                v3(0.5 * radius, 0.14 * radius, 0.44 * radius),
            );
        }
    }
}

/// The feed from under the head down through the turntable into the tower.
fn feed_drop(b: &mut MeshBuilder, pivot: Vec3, top: f32, s: f32) {
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.cylinder_between(
        v3(-1.2 * s, 0.0, pivot.z - 1.2 * s),
        v3(-0.6 * s, 0.0, top - 0.4 * s),
        0.45 * s,
        0.45 * s,
        8,
    );
}
