//! The reclaim tower's head: gun house 0 on the head house's roof. What only turns
//! (turntable, cheeks or walls, drives) is built round the tower's axis; what pitches is
//! built round the trunnion at [`super::PIVOT_Z`] and ends at the intake mouth,
//! [`super::EMIT_X`] ahead of it. Heavy plant, not a gun: square-shouldered housings,
//! vane grilles over dark intakes, the only light the Materials glow in a working throat.

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::{ring_top, tier, EMIT_X, HEAD_SCALE, PIVOT_Z, REACH};
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// Which head the tower carries: the design question open to the user.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Turret {
    /// The Thresher's Cradle at tower scale: a processor box slung between two yoke
    /// cheeks, a short intake snout, a charge pack aft.
    Cradle,
    /// An armoured house with side walls and a roof aft, open at the front, where a
    /// heavy mantlet block pitches with a square grilled intake.
    Casemate,
    /// A machinery deck (engine house, counterweight slabs, a cab) under an A-frame, and
    /// a short box-girder boom with a collector head on its end.
    Crane,
}

/// The head: its fixed race on the head house, then gun house 0.
pub(super) fn turret(b: &mut MeshBuilder, tech: u8, kind: Turret) {
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
    match kind {
        Turret::Cradle => cradle(b, pivot, top, s),
        Turret::Casemate => casemate(b, pivot, top, s),
        Turret::Crane => crane(b, pivot, top, s),
    }
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

/// An armoured house open at the front, a mantlet block pitching in the opening.
fn casemate(b: &mut MeshBuilder, pivot: Vec3, top: f32, s: f32) {
    let pz = pivot.z;
    let (inner, outer) = (2.35 * s, 3.2 * s);
    let roof = pz + 2.5 * s;
    b.with_house(0, pivot, 0.0, |b| {
        // Side walls: sloped front and back, full height beside the trunnion.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.with_bevel(0.08 * s, |b| {
                b.extrude_y(
                    &[
                        [-4.4 * s, top],
                        [2.6 * s, top],
                        [2.2 * s, pz + 1.2 * s],
                        [1.0 * s, roof],
                        [-3.6 * s, roof],
                        [-4.4 * s, pz + 1.0 * s],
                    ],
                    inner,
                    outer,
                );
            });
            // The trunnion boss through the wall.
            b.paint(METAL);
            b.cylinder_between(
                v3(0.0, outer - 0.1, pz),
                v3(0.0, outer + 0.35 * s, pz),
                1.0 * s,
                0.85 * s,
                b.sides(10),
            );
            if b.fine() {
                // A bolted armour plate on the wall's face.
                b.paint(PLATING_DARK);
                b.extrude_y(
                    &[
                        [-3.4 * s, top + 0.4 * s],
                        [1.6 * s, top + 0.4 * s],
                        [1.2 * s, pz - 1.3 * s],
                        [-3.0 * s, pz - 1.3 * s],
                    ],
                    outer - 0.02,
                    outer + 0.12 * s,
                );
            }
        });
        // Rear wall and the roof over the back half: the drives live under it.
        b.paint(PLATING);
        b.block(v3(-4.4 * s, -inner, top), v3(-3.7 * s, inner, pz + 1.0 * s));
        b.paint(PLATING_DARK);
        b.block(
            v3(-3.8 * s, -outer, roof - 0.35 * s),
            v3(-1.2 * s, outer, roof),
        );
        team_panel(b, v3(-2.5 * s, 0.0, roof), Vec2::new(1.6 * s, 2.6 * s));
        if b.fine() {
            vent(
                b,
                v3(-3.2 * s, 1.9 * s, roof),
                Vec2::new(0.9 * s, 0.9 * s),
                3,
                ACCENT,
            );
            whip(b, v3(-3.4 * s, -2.6 * s, roof), 2.6 * s, 0.06);
            feed_drop(b, pivot, top, s);
        }
        b.with_recoil(|b| {
            // The mantlet block on its trunnion, a counterweight behind it.
            b.paint(PLATING);
            b.with_bevel(0.12 * s, |b| {
                b.chamfered_box(v3(0.5 * s, 0.0, pz), v3(4.8 * s, 4.4 * s, 3.4 * s), 0.5 * s);
            });
            b.paint(PLATING_DARK);
            b.chamfered_box(
                v3(-2.4 * s, 0.0, pz - 0.2 * s),
                v3(1.2 * s, 3.4 * s, 2.4 * s),
                0.3 * s,
            );
            // A face plate over the mantlet's front.
            b.paint(PLATING_DARK);
            b.block(
                v3(2.9 * s, -2.0 * s, pz - 1.5 * s),
                v3(3.2 * s, 2.0 * s, pz + 1.5 * s),
            );
            // The intake: a square throat widening to a grilled mouth.
            let mouth = REACH * s;
            b.paint(PLATING);
            b.beam(
                v3(3.1 * s, 0.0, pz),
                v3(mouth - 0.5 * s, 0.0, pz),
                Vec2::new(2.6 * s, 2.6 * s),
                Vec2::new(3.0 * s, 3.0 * s),
            );
            square_mouth(b, v3(mouth, 0.0, pz), 1.55 * s);
            if b.fine() {
                // Hydraulic lines along the throat's flanks back into the block.
                b.paint(METAL);
                b.mirror_y(|b| {
                    b.cylinder_between(
                        v3(2.4 * s, 1.55 * s, pz + 1.0 * s),
                        v3(mouth - 0.8 * s, 1.55 * s, pz + 1.0 * s),
                        0.14 * s,
                        0.14 * s,
                        6,
                    );
                });
            }
        });
    });
}

/// A machinery deck under an A-frame; a box-girder boom with a collector head pitching.
fn crane(b: &mut MeshBuilder, pivot: Vec3, top: f32, s: f32) {
    let pz = pivot.z;
    b.with_house(0, pivot, 0.0, |b| {
        // The deck: short at the front so the boom clears it pitched down.
        b.paint(PLATING_DARK);
        b.block(
            v3(-5.4 * s, -3.0 * s, top),
            v3(1.8 * s, 3.0 * s, top + 0.5 * s),
        );
        // Engine house aft, counterweight slabs behind it.
        b.paint(PLATING);
        b.with_bevel(0.1 * s, |b| {
            b.chamfered_box(
                v3(-3.4 * s, 0.0, top + 1.8 * s),
                v3(2.4 * s, 4.8 * s, 2.6 * s),
                0.4 * s,
            );
        });
        team_panel(
            b,
            v3(-3.4 * s, 0.0, top + 3.1 * s),
            Vec2::new(1.4 * s, 2.6 * s),
        );
        b.paint(ACCENT);
        for (i, x) in [-5.0, -5.5].iter().enumerate() {
            let h = 2.6 - 0.4 * i as f32;
            b.block(
                v3(x * s, -2.4 * s, top + 0.5 * s),
                v3((x + 0.45) * s, 2.4 * s, top + (0.5 + h) * s),
            );
        }
        // The A-frame each side, from the deck up to the trunnion bearing.
        b.mirror_y(|b| {
            b.paint(PLATING);
            let y = 2.5 * s;
            for x in [-2.2 * s, 1.5 * s] {
                b.beam(
                    v3(x, y, top + 0.4 * s),
                    v3(x * 0.15, y, pz - 0.4 * s),
                    Vec2::new(0.6 * s, 0.8 * s),
                    Vec2::new(0.5 * s, 0.6 * s),
                );
            }
            b.paint(PLATING_DARK);
            b.chamfered_box(v3(0.0, y, pz), v3(1.6 * s, 0.7 * s, 1.8 * s), 0.4 * s);
            b.paint(METAL);
            b.cylinder_between(
                v3(0.0, y + 0.3 * s, pz),
                v3(0.0, y + 0.65 * s, pz),
                0.7 * s,
                0.6 * s,
                b.sides(10),
            );
        });
        if b.mid() {
            // The operator's cab on the deck's front corner.
            b.paint(PLATING);
            b.block(
                v3(-0.4 * s, -2.9 * s, top + 0.5 * s),
                v3(1.6 * s, -1.9 * s, top + 2.0 * s),
            );
            b.paint(GLASS);
            b.block(
                v3(1.6 * s, -2.8 * s, top + 1.2 * s),
                v3(1.66 * s, -2.0 * s, top + 1.85 * s),
            );
        }
        if b.fine() {
            vent(
                b,
                v3(-3.4 * s, 1.6 * s, top + 3.1 * s),
                Vec2::new(1.4 * s, 0.9 * s),
                4,
                ACCENT,
            );
            feed_drop(b, pivot, top, s);
        }
        b.with_recoil(|b| {
            // Trunnion hub across the frame, the boom out of it.
            b.paint(METAL);
            b.cylinder_between(
                v3(0.0, -2.2 * s, pz),
                v3(0.0, 2.2 * s, pz),
                1.0 * s,
                1.0 * s,
                b.sides(12),
            );
            b.paint(PLATING);
            b.beam(
                v3(-1.6 * s, 0.0, pz),
                v3(4.9 * s, 0.0, pz),
                Vec2::new(2.4 * s, 2.0 * s),
                Vec2::new(2.0 * s, 1.7 * s),
            );
            // Flanges along the girder's top and bottom.
            if b.mid() {
                b.paint(PLATING_DARK);
                for z in [1.05, -1.05] {
                    b.beam(
                        v3(-1.4 * s, 0.0, pz + z * s),
                        v3(4.8 * s, 0.0, pz + z * 0.85 * s),
                        Vec2::new(2.6 * s, 0.2 * s),
                        Vec2::new(2.2 * s, 0.2 * s),
                    );
                }
            }
            // A ballast block on the boom's heel.
            b.paint(ACCENT);
            b.block(
                v3(-2.6 * s, -1.3 * s, pz - 1.0 * s),
                v3(-1.5 * s, 1.3 * s, pz + 1.0 * s),
            );
            // The collector head: a housing flaring to the grilled mouth.
            let mouth = REACH * s;
            b.paint(PLATING_DARK);
            b.beam(
                v3(4.6 * s, 0.0, pz),
                v3(mouth - 0.5 * s, 0.0, pz),
                Vec2::new(2.4 * s, 2.2 * s),
                Vec2::new(3.2 * s, 3.0 * s),
            );
            square_mouth(b, v3(mouth, 0.0, pz), 1.6 * s);
            if b.fine() {
                // Rams along the girder's back to the head, hoses beside them.
                b.paint(METAL);
                b.cylinder_between(
                    v3(0.6 * s, 0.0, pz + 1.3 * s),
                    v3(4.4 * s, 0.0, pz + 1.3 * s),
                    0.25 * s,
                    0.2 * s,
                    8,
                );
                b.paint(ACCENT).pattern(pattern::MASS_FLOW);
                b.mirror_y(|b| {
                    b.cylinder_between(
                        v3(4.4 * s, 1.35 * s, pz + 0.3 * s),
                        v3(0.2 * s, 1.35 * s, pz + 0.3 * s),
                        0.22 * s,
                        0.22 * s,
                        6,
                    );
                });
            }
        });
    });
}

/// A square intake mouth facing +x, `half` its half size, with its face at `at`: a
/// heavy frame, vane slats across a dark throat, the Materials glow deep in it.
fn square_mouth(b: &mut MeshBuilder, at: Vec3, half: f32) {
    let depth = 0.5 * half;
    let back = at.x - depth;
    b.paint(ACCENT);
    // The frame: four bars round the opening.
    for (y0, y1, z0, z1) in [
        (-half, half, half * 0.72, half),
        (-half, half, -half, -half * 0.72),
        (half * 0.72, half, -half * 0.72, half * 0.72),
        (-half, -half * 0.72, -half * 0.72, half * 0.72),
    ] {
        b.block(v3(back, y0, at.z + z0), v3(at.x, y1, at.z + z1));
    }
    // The throat's back, glowing.
    b.paint(GLOW_MATERIALS);
    b.block(
        v3(back - 0.1, -half * 0.72, at.z - half * 0.72),
        v3(back + 0.02, half * 0.72, at.z + half * 0.72),
    );
    if b.mid() {
        // Vane slats across the opening: it swallows, it does not fire.
        b.paint(PLATING_DARK);
        for k in [-0.36, 0.0, 0.36] {
            b.block(
                v3(back + 0.1, -half * 0.72, at.z + (k - 0.06) * half),
                v3(at.x - 0.1, half * 0.72, at.z + (k + 0.06) * half),
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
