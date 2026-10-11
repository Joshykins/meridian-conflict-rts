//! The head (the turret part): a plated drum on the race, a survey sensor and mast on
//! its roof, cheeks holding the forearm's axle, and the halo round it: a ring of dark
//! segments on three spokes (at the back and the rear quarters, so the front is open
//! for the forearm) carrying the projector nodes. Servo housings at the spokes' roots
//! (tech 2) and tool arms under the nodes (tech 3) come with the upgrades.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::{projector, ARM_END, DECK, ELBOW, EMITTER, HALO, HALO_Z, NODES, ROOF};
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;
use crate::{part, rig};

/// Half the head drum.
const DRUM: f32 = 3.0;

pub(super) fn head(b: &mut MeshBuilder, tech: u8) {
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        b.with_bevel(0.15, |b| {
            b.loft_z(
                &chamfered_rect(Vec2::splat(DRUM), 1.0),
                &[
                    Section::new(DECK, 0.9),
                    Section::new(DECK + 1.0, 1.0),
                    Section::new(ROOF - 0.6, 1.0),
                    Section::new(ROOF, 0.86),
                ],
            );
        });
        b.paint(PLATING_DARK);
        b.loft_z(
            &chamfered_rect(Vec2::splat(DRUM + 0.06), 1.02),
            &[
                Section::new(HALO_Z - 0.5, 1.0),
                Section::new(HALO_Z + 0.5, 1.0),
            ],
        );
        if b.mid() {
            // A lit band round the drum over the halo.
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.loft_z(
                &chamfered_rect(Vec2::splat(DRUM + 0.05), 1.02),
                &[Section::new(ROOF - 1.4, 1.0), Section::new(ROOF - 1.0, 1.0)],
            );
        }
        team_panel(b, v3(-1.0, 0.0, ROOF), v2(1.6, 2.6));
        halo(b, tech);
        cheeks(b);
        survey(b);
        if b.fine() {
            b.paint(GLOW_LAMP);
            b.mirror_y(|b| b.cuboid(v3(2.6, 2.6, ROOF - 0.5), v3(0.35, 0.35, 0.3)));
        }
        forearm(b, tech);
    });
}

/// Cheeks on the roof either side of the forearm, the axle's bosses in them.
fn cheeks(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.with_bevel(0.08, |b| {
            b.extrude_y(
                &[
                    [-0.6, ROOF - 0.2],
                    [3.0, ROOF - 0.2],
                    [3.2, ELBOW.z + 0.2],
                    [2.0, ELBOW.z + 1.2],
                    [-0.6, ELBOW.z + 1.2],
                ],
                1.0,
                1.8,
            );
        });
        b.paint(METAL);
        b.cylinder_between(
            v3(ELBOW.x, 1.7, ELBOW.z),
            v3(ELBOW.x, 2.0, ELBOW.z),
            0.55,
            0.45,
            b.sides(8),
        );
    });
}

/// The survey head on the roof aft: a sensor drum on a post, its glazed eye forward,
/// and a whip mast beside it.
fn survey(b: &mut MeshBuilder) {
    let at = v3(-1.9, -1.5, ROOF);
    b.paint(METAL);
    b.prism(at, b.sides(6), 0.32, 0.26, 0.8);
    b.paint(PLATING_DARK);
    b.cylinder_between(
        at + v3(-0.7, 0.0, 1.2),
        at + v3(0.6, 0.0, 1.2),
        0.55,
        0.5,
        b.sides(8),
    );
    b.paint(GLASS);
    b.cylinder_between(
        at + v3(0.6, 0.0, 1.2),
        at + v3(0.7, 0.0, 1.2),
        0.38,
        0.34,
        b.sides(8),
    );
    if b.fine() {
        antenna(b, v3(-2.3, 1.6, ROOF), 2.6, 0.0);
    }
}

/// The halo: its ring, the spokes and the nodes; servo housings at the spokes' roots
/// from tech 2, tool arms under the nodes at tech 3.
fn halo(b: &mut MeshBuilder, tech: u8) {
    let segs = if b.fine() { 16 } else { 12 };
    let at = |a: f32, r: f32| v3(a.cos() * r, a.sin() * r, HALO_Z);
    let ring = |b: &mut MeshBuilder, r: f32, z: f32, size: Vec2| {
        for i in 0..segs {
            let (a0, a1) = (
                TAU * i as f32 / segs as f32,
                TAU * (i + 1) as f32 / segs as f32,
            );
            let p = |a: f32| v3(a.cos() * r, a.sin() * r, z);
            b.beam(p(a0), p(a1), size, size);
        }
    };
    b.paint(PLATING_DARK);
    ring(b, HALO, HALO_Z, Vec2::new(0.9, 0.7));
    b.paint(PLATING);
    for k in [1, 3, 5] {
        let a = TAU * k as f32 / 6.0;
        b.beam(
            at(a, DRUM - 0.1),
            at(a, HALO - 0.3),
            Vec2::new(0.8, 1.0),
            Vec2::new(0.6, 0.6),
        );
    }
    for k in 0..NODES {
        node(b, TAU * (k as f32 + 0.5) / NODES as f32, 1.0);
    }
    kit(b, tech, 2, 0.3, |b| {
        for k in [1, 3, 5] {
            b.yawed(Vec3::ZERO, TAU * k as f32 / 6.0, servo);
        }
    });
    kit(b, tech, 3, 0.45, |b| {
        for k in 0..NODES {
            b.yawed(Vec3::ZERO, TAU * (k as f32 + 0.5) / NODES as f32, tool);
        }
    });
    kit(b, tech, 2, 0.5, |b| {
        for k in 0..NODES {
            node(b, TAU * k as f32 / NODES as f32, 0.75);
        }
    });
    kit(b, tech, 3, 0.6, |b| {
        if b.fine() {
            b.paint(PLATING);
            ring(b, HALO + 1.3, HALO_Z + 0.5, Vec2::new(0.4, 0.3));
            b.paint(GLOW_RED);
            b.cuboid(v3(-2.3, 1.6, ROOF + 2.75), Vec3::splat(0.35));
        }
    });
}

/// A node on the halo at angle `a`, `s` its scale: a plated pod straddling the ring, a
/// conduit band round its outer face, an emitter cup under it.
fn node(b: &mut MeshBuilder, a: f32, s: f32) {
    b.yawed(Vec3::ZERO, a, |b| {
        b.paint(PLATING);
        b.chamfered_box(
            v3(HALO, 0.0, HALO_Z),
            v3(1.8 * s, 1.6 * s, 1.6 * s),
            0.4 * s,
        );
        if b.fine() || (b.mid() && s >= 1.0) {
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(
                v3(HALO + 0.9 * s - 0.05, -0.55 * s, HALO_Z - 0.4 * s),
                v3(HALO + 0.9 * s + 0.08, 0.55 * s, HALO_Z + 0.4 * s),
            );
        }
        if b.fine() {
            b.paint(METAL);
            b.prism(
                v3(HALO - 0.2 * s, 0.0, HALO_Z - 1.3 * s),
                6,
                0.35 * s,
                0.5 * s,
                0.5 * s,
            );
        }
    });
}

/// A servo housing at a spoke's root (built along +x): a dark armoured box, its drive
/// drum across the spoke, a conduit slit on its outer face.
fn servo(b: &mut MeshBuilder) {
    let x = DRUM + 0.9;
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(x, 0.0, HALO_Z), v3(1.6, 1.9, 1.7), 0.4);
    if b.fine() {
        b.paint(METAL);
        b.cylinder_between(
            v3(x, -1.15, HALO_Z + 0.2),
            v3(x, 1.15, HALO_Z + 0.2),
            0.55,
            0.55,
            8,
        );
    }
    if b.mid() {
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.block(
            v3(x + 0.78, -0.6, HALO_Z - 0.5),
            v3(x + 0.86, 0.6, HALO_Z - 0.1),
        );
    }
}

/// A tool arm under a node (built along +x): a shoulder drum on the node's outer side,
/// an upper link out and down, an elbow, a forearm back in, and a print head with its
/// nozzle pointing down at the ground round the mast.
fn tool(b: &mut MeshBuilder) {
    let shoulder = v3(HALO + 0.55, 0.0, HALO_Z - 0.95);
    let elbow = v3(HALO + 1.3, 0.0, HALO_Z - 2.0);
    let wrist = v3(HALO + 0.7, 0.0, HALO_Z - 3.0);
    let fine = b.fine();
    if fine {
        b.paint(METAL);
        b.cylinder_between(
            shoulder - Vec3::Y * 0.4,
            shoulder + Vec3::Y * 0.4,
            0.28,
            0.28,
            6,
        );
    }
    b.paint(PLATING_DARK);
    b.beam(
        shoulder,
        elbow,
        Vec2::new(0.42, 0.36),
        Vec2::new(0.38, 0.32),
    );
    if fine {
        b.paint(METAL);
        b.cylinder_between(elbow - Vec3::Y * 0.3, elbow + Vec3::Y * 0.3, 0.24, 0.24, 6);
    }
    b.paint(PLATING);
    b.beam(elbow, wrist, Vec2::new(0.36, 0.32), Vec2::new(0.32, 0.3));
    b.paint(PLATING_DARK);
    b.chamfered_box(wrist - Vec3::Z * 0.2, Vec3::splat(0.62), 0.14);
    if fine {
        b.paint(METAL);
        b.prism(wrist - Vec3::Z * 0.95, 6, 0.07, 0.24, 0.45);
    }
}

/// The forearm (`rig::ARM_TOOL`): a slim armoured bar from the axle to the projector, a
/// dark belly, a lit conduit on its back at tech 2.
fn forearm(b: &mut MeshBuilder, tech: u8) {
    let z = ELBOW.z;
    b.with_limb(rig::ARM_TOOL, |b| {
        b.paint(METAL);
        b.cylinder_between(
            v3(ELBOW.x, -1.75, z),
            v3(ELBOW.x, 1.75, z),
            0.5,
            0.5,
            b.sides(8),
        );
        let section = [
            [-0.9, z - 0.6],
            [-0.5, z - 1.0],
            [0.5, z - 1.0],
            [0.9, z - 0.6],
            [0.9, z + 0.6],
            [0.5, z + 0.9],
            [-0.5, z + 0.9],
            [-0.9, z + 0.6],
        ];
        b.paint(PLATING);
        b.with_bevel(0.06, |b| b.extrude_x(&section, ELBOW.x - 1.2, ARM_END));
        b.paint(PLATING_DARK);
        b.block(
            v3(ELBOW.x, -0.92, z - 0.4),
            v3(ARM_END - 0.4, 0.92, z + 0.2),
        );
        if b.fine() {
            b.paint(METAL);
            b.mirror_y(|b| {
                b.cylinder_between(
                    v3(ELBOW.x + 0.8, 0.95, z - 0.65),
                    v3(ARM_END - 0.6, 0.95, z - 0.65),
                    0.12,
                    0.12,
                    6,
                );
            });
        }
        projector(b, v3(ARM_END, 0.0, z), EMITTER, 1.15, tech);
        kit(b, tech, 2, 0.6, |b| {
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(
                v3(ELBOW.x + 1.2, -0.5, z + 0.9),
                v3(ARM_END - 0.6, 0.5, z + 1.15),
            );
        });
    });
}
