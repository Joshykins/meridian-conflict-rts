//! The station from the plinth to the head's race: an armoured foot with sloped
//! shoulders and dark insets, four braces on its diagonals, the octagonal mast with a
//! conduit run down its front and back, and the two feed conduits up its flanks, held
//! by clamps, that carry the works' power to the head.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::{
    inset, BASE, DECK, ELBOW, EMITTER, FOOT, FOOT_TOP, FOOT_WALL, HALO, HALO_Z, MAST, MAST_HEAD,
    ROOF,
};
use crate::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;
use crate::{part, rig};

/// How far out from the axis the mast's flats are at height `z`.
pub(super) fn mast_face(z: f32) -> f32 {
    let f = ((z - FOOT_TOP) / (DECK - FOOT_TOP)).clamp(0.0, 1.0);
    (MAST + (MAST_HEAD - MAST) * f) * (PI / 8.0).cos()
}

/// The coarse model: the foot and mast as one block, the halo as a slab, the head and
/// the forearm.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, BASE),
        Vec2::splat(FOOT * 1.6),
        Vec2::splat(MAST_HEAD * 1.8),
        DECK - BASE,
        Vec2::ZERO,
    );
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING_DARK);
        b.cuboid(v3(0.0, 0.0, HALO_Z), v3(2.0 * HALO, 2.0 * HALO, 0.8));
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, (DECK + ROOF) * 0.5), v3(5.0, 5.0, ROOF - DECK));
        b.with_limb(rig::ARM_TOOL, |b| {
            b.cuboid_open(
                v3((ELBOW.x + EMITTER.x) * 0.5, 0.0, ELBOW.z),
                v3(EMITTER.x - ELBOW.x, 1.8, 1.8),
            );
        });
    });
}

pub(super) fn body(b: &mut MeshBuilder, tech: u8) {
    foot(b, tech);
    mast(b, tech);
}

/// The foot: upright armoured walls, sloped shoulders, an inset with a lit slit on each
/// face, a dark band where the walls meet the shoulders, and the braces.
fn foot(b: &mut MeshBuilder, tech: u8) {
    b.paint(PLATING);
    b.with_bevel(0.2, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(FOOT), 2.0),
            &[
                Section::new(BASE - 0.2, 1.0),
                Section::new(FOOT_WALL, 1.0),
                Section::new(FOOT_TOP, 0.66),
            ],
        );
    });
    b.paint(PLATING_DARK);
    b.loft_z(
        &chamfered_rect(Vec2::splat(FOOT + 0.06), 2.02),
        &[
            Section::new(FOOT_WALL - 0.35, 1.0),
            Section::new(FOOT_WALL - 0.05, 1.0),
        ],
    );
    for q in 0..4 {
        b.yawed(Vec3::ZERO, q as f32 * FRAC_PI_2, |b| {
            inset(b, FOOT, 3.2, BASE + 0.3, 3.9, Some(3.3));
        });
    }
    for i in 0..4 {
        b.yawed(Vec3::ZERO, FRAC_PI_4 + i as f32 * FRAC_PI_2, brace);
    }
    if b.fine() {
        // Lamps on the foot's cut corners.
        b.paint(GLOW_LAMP);
        b.radial(4, |b| {
            b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
                b.cuboid(
                    v3(FOOT * 1.2 + 0.05, 0.0, FOOT_WALL - 0.7),
                    v3(0.3, 0.6, 0.3),
                );
            });
        });
    }
    // Tech 3: heavy plates on the foot's corners.
    kit(b, tech, 3, 0.4, |b| {
        b.radial(4, |b| {
            b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
                b.paint(PLATING);
                let x = FOOT * std::f32::consts::SQRT_2 - 1.6;
                b.extrude_y(
                    &[
                        [x - 1.4, BASE],
                        [x + 1.2, BASE],
                        [x - 0.2, 4.4],
                        [x - 1.4, 4.4],
                    ],
                    -1.6,
                    1.6,
                );
            });
        });
    });
}

/// One brace, built along +x: a sculpted fin from the foot's sloped face up the mast,
/// a dark spine down its edge.
fn brace(b: &mut MeshBuilder) {
    let corner = FOOT * std::f32::consts::SQRT_2 - 2.0 * 0.7;
    b.paint(PLATING);
    b.with_bevel(0.1, |b| {
        b.extrude_y(
            &[
                [MAST * 0.8, 6.0],
                [corner - 0.6, 4.8],
                [corner - 1.2, 6.2],
                [MAST + 0.6, 13.0],
                [MAST * 0.85, 13.0],
            ],
            -0.5,
            0.5,
        );
    });
    if b.mid() {
        b.paint(PLATING_DARK);
        b.beam(
            v3(corner - 1.1, 0.0, 6.2),
            v3(MAST + 0.7, 0.0, 12.9),
            Vec2::new(0.6, 0.25),
            Vec2::new(0.6, 0.25),
        );
    }
}

/// The mast: plated, a conduit run down its front and back, a dark band at its head,
/// the feed conduits up its flanks, the race the head turns on. A collar at tech 2.
fn mast(b: &mut MeshBuilder, tech: u8) {
    b.paint(PLATING);
    b.with_bevel(0.12, |b| {
        b.loft_z(
            &ngon(8, MAST),
            &[
                Section::new(FOOT_TOP - 0.6, 1.0),
                Section::new(DECK - 0.8, MAST_HEAD / MAST),
            ],
        );
    });
    let (z0, z1) = (FOOT_TOP + 1.0, DECK - 2.0);
    for q in [0.0, PI] {
        b.yawed(Vec3::ZERO, q, |b| {
            b.paint(PLATING_DARK);
            b.beam(
                v3(mast_face(z0) - 0.1, 0.0, z0 - 0.4),
                v3(mast_face(z1) - 0.1, 0.0, z1 + 0.4),
                Vec2::new(1.3, 0.3),
                Vec2::new(1.1, 0.3),
            );
            if b.mid() {
                b.paint(ACCENT).pattern(pattern::CONDUIT);
                b.beam(
                    v3(mast_face(z0) + 0.06, 0.0, z0),
                    v3(mast_face(z1) + 0.06, 0.0, z1),
                    Vec2::new(0.7, 0.2),
                    Vec2::new(0.6, 0.2),
                );
            }
        });
    }
    // The feed conduits up the flanks, from the foot's shoulder to the race.
    b.mirror_y(|b| {
        let (a, c) = (FOOT_TOP - 0.5, DECK - 1.0);
        let y = |z: f32| mast_face(z) + 0.4;
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.cylinder_between(v3(0.0, y(a), a), v3(0.0, y(c), c), 0.32, 0.32, b.sides(6));
        b.paint(PLATING_DARK);
        b.block(v3(-0.6, y(a) - 0.5, a - 0.6), v3(0.6, y(a) + 0.5, a + 0.4));
        if b.fine() {
            b.paint(METAL);
            for z in [11.0, 14.5, 18.0] {
                b.block(
                    v3(-0.45, y(z) - 0.45, z - 0.15),
                    v3(0.45, y(z) + 0.42, z + 0.15),
                );
            }
        }
    });
    // A dark band under the race, and the race.
    b.paint(PLATING_DARK);
    b.loft_z(
        &ngon(8, MAST_HEAD + 0.12),
        &[Section::new(DECK - 2.4, 1.0), Section::new(DECK - 1.6, 1.0)],
    );
    b.prism(
        v3(0.0, 0.0, DECK - 0.8),
        b.sides(10),
        MAST_HEAD + 0.3,
        3.2,
        0.8,
    );
    kit(b, tech, 2, 0.6, |b| {
        let z = 15.0;
        b.paint(PLATING_DARK);
        b.loft_z(
            &ngon(8, MAST * 0.9 + 0.55),
            &[
                Section::new(z - 0.4, 0.9),
                Section::new(z, 1.0),
                Section::new(z + 1.0, 1.0),
                Section::new(z + 1.4, 0.9),
            ],
        );
        if b.mid() {
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.loft_z(
                &ngon(8, MAST * 0.9 + 0.6),
                &[Section::new(z + 0.3, 1.0), Section::new(z + 0.7, 1.0)],
            );
        }
    });
}
