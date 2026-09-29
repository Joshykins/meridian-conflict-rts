//! The tower from the foundation to the head house: an armoured octagonal foot, a
//! tapering octagonal shaft braced by four sculpted fins on its diagonals, dark inset
//! panels, a flow channel glazed into each face, and the armoured crown (the head house)
//! the head turns on. Symmetric about both axes.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::{BASE, CAP, FOOT, HOUSE};
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// Half the crown's width.
pub(super) const CROWN: f32 = 5.9;
/// Half the spire's foot at the ground.
const SPIRE_FOOT: f32 = 8.6;
/// Half the spire's shaft at the foot, and how much it narrows by the crown.
const SPIRE_SHAFT: f32 = 5.4;
const SPIRE_TAPER: f32 = 0.84;
/// How far out from the axis the shaft's face is at height `z` (above the foot): where
/// works hung on the tower meet it.
pub(super) fn spire_half(z: f32) -> f32 {
    let f = ((z - FOOT) / (CAP - FOOT)).clamp(0.0, 1.0);
    SPIRE_SHAFT * (1.0 - (1.0 - SPIRE_TAPER) * f)
}

/// The foot as one block, the shaft and crown as another.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    let foot = SPIRE_FOOT;
    b.frustum_open(
        v3(0.0, 0.0, BASE),
        Vec2::splat(foot * 2.0),
        Vec2::splat(foot * 1.6),
        FOOT - BASE,
        Vec2::ZERO,
    );
    b.frustum_open(
        v3(0.0, 0.0, FOOT),
        Vec2::splat(CROWN * 1.9),
        Vec2::splat(CROWN * 2.0),
        CAP + HOUSE - FOOT,
        Vec2::ZERO,
    );
}

pub(super) fn body(b: &mut MeshBuilder) {
    spire(b);
    crown(b);
}

/// Emits `f` on the faces of the tower at these quarter turns (0 = front, +x).
fn faces(b: &mut MeshBuilder, quarters: &[u8], f: impl Fn(&mut MeshBuilder)) {
    for &q in quarters {
        b.yawed(Vec3::ZERO, f32::from(q) * FRAC_PI_2, |b| f(b));
    }
}

/// The spire, a flow channel glazed into each face.
fn spire(b: &mut MeshBuilder) {
    // The armoured foot on the foundation: sloped faces stepping in to a shoulder.
    b.paint(PLATING);
    b.with_bevel(0.2, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(SPIRE_FOOT), 2.8),
            &[
                Section::new(BASE - 0.2, 1.0),
                Section::new(5.9, 0.95),
                Section::new(FOOT, 0.76),
            ],
        );
    });
    let face = |z: f32| SPIRE_FOOT * (1.0 - 0.05 * (z - BASE) / (5.9 - BASE));
    // Inset armour panels with slats and a light slit on the flanks; a door in a dark
    // frame front and back.
    faces(b, &[1, 3], |b| {
        b.paint(PLATING_DARK);
        b.beam(
            v3(face(BASE + 0.4) - 0.3, 0.0, BASE + 0.4),
            v3(face(5.4) - 0.3, 0.0, 5.4),
            Vec2::new(7.0, 0.7),
            Vec2::new(7.0, 0.7),
        );
        if b.fine() {
            b.paint(ACCENT);
            for z in [3.6, 4.3] {
                let x = face(z) + 0.05;
                b.block(v3(x - 0.15, -2.6, z), v3(x + 0.05, 2.6, z + 0.3));
            }
        }
        if b.mid() {
            b.paint(GLOW);
            let x = face(5.5);
            b.block(v3(x - 0.2, -2.4, 5.5), v3(x + 0.02, 2.4, 5.68));
        }
    });
    faces(b, &[0, 2], |b| {
        b.paint(PLATING_DARK);
        b.beam(
            v3(face(BASE) - 0.3, 0.0, BASE),
            v3(face(5.6) - 0.3, 0.0, 5.6),
            Vec2::new(4.2, 0.8),
            Vec2::new(4.2, 0.8),
        );
        b.paint(ACCENT);
        b.beam(
            v3(face(BASE) - 0.1, 0.0, BASE),
            v3(face(5.0) - 0.1, 0.0, 5.0),
            Vec2::new(3.0, 0.8),
            Vec2::new(3.0, 0.8),
        );
    });
    if b.fine() {
        // Lamps on the foot's cut corners.
        b.paint(GLOW_LAMP);
        b.radial(4, |b| {
            let d = (SPIRE_FOOT * 0.95 - 1.2) * 0.98;
            b.cuboid(v3(d, d, 5.7), Vec3::splat(0.5));
        });
    }

    // The shaft.
    b.paint(PLATING);
    b.with_bevel(0.15, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(SPIRE_SHAFT), 1.7),
            &[
                Section::new(FOOT - 0.4, 1.0),
                Section::new(CAP + 0.2, SPIRE_TAPER),
            ],
        );
    });
    let (z0, z1) = (FOOT + 2.0, CAP - 2.5);
    // The flow channels: glazed down the middle of a dark panel on each face, from the
    // crown to the foot's shoulder, framed in steel.
    faces(b, &[0, 1, 2, 3], |b| {
        panel(b, z0, z1);
        let (z0, z1) = (FOOT - 0.2, CAP + 0.2);
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.beam(
            v3(spire_half(z0) + 0.05, 0.0, z0),
            v3(spire_half(z1) + 0.05, 0.0, z1),
            Vec2::new(1.6, 0.5),
            Vec2::new(1.6, 0.5),
        );
        b.paint(METAL);
        for y in [-1.1, 1.1] {
            b.beam(
                v3(spire_half(z0) + 0.05, y, z0),
                v3(spire_half(z1) + 0.05, y, z1),
                Vec2::new(0.5, 0.7),
                Vec2::new(0.5, 0.7),
            );
        }
    });
    // Four fins on the diagonals, from the foot's shoulder high up the shaft.
    for i in 0..4 {
        b.yawed(Vec3::ZERO, FRAC_PI_4 + i as f32 * FRAC_PI_2, fin);
    }
    // A band round the shaft where the fins end.
    b.paint(ACCENT);
    let z = 22.8;
    b.loft_z(
        &chamfered_rect(Vec2::splat(spire_half(z) + 0.2), 1.9),
        &[Section::new(z, 1.0), Section::new(z + 0.7, 1.0)],
    );
}

/// A tall dark panel up the face (+x) from `z0` to `z1`.
fn panel(b: &mut MeshBuilder, z0: f32, z1: f32) {
    b.paint(PLATING_DARK);
    b.beam(
        v3(spire_half(z0) - 0.2, 0.0, z0),
        v3(spire_half(z1) - 0.2, 0.0, z1),
        Vec2::new(3.4, 0.6),
        Vec2::new(2.8, 0.6),
    );
}

/// One fin, built along +x: a sculpted brace from the foot's shoulder up the shaft's cut
/// corner, a dark spine down its outer edge.
fn fin(b: &mut MeshBuilder) {
    // The shaft's cut corner lies this far out on the diagonal, falling with the taper.
    let corner = |z: f32| (spire_half(z) - 0.85) * std::f32::consts::SQRT_2;
    let top = 23.0;
    let profile = [
        [corner(FOOT) - 0.4, FOOT - 1.0],
        [corner(FOOT) + 2.6, FOOT - 1.0],
        [corner(FOOT) + 2.6, FOOT + 1.2],
        [corner(15.0) + 1.2, 15.0],
        [corner(top) + 0.2, top],
        [corner(top) - 0.4, top],
    ];
    b.paint(PLATING);
    b.with_bevel(0.12, |b| b.extrude_y(&profile, -0.75, 0.75));
    if b.mid() {
        b.paint(PLATING_DARK);
        b.beam(
            v3(corner(FOOT) + 2.7, 0.0, FOOT + 1.1),
            v3(corner(15.0) + 1.3, 0.0, 15.0),
            Vec2::new(0.9, 0.3),
            Vec2::new(0.9, 0.3),
        );
        b.beam(
            v3(corner(15.0) + 1.3, 0.0, 15.0),
            v3(corner(top) + 0.3, 0.0, top),
            Vec2::new(0.9, 0.3),
            Vec2::new(0.7, 0.3),
        );
    }
}

// ---- Crown -----------------------------------------------------------------------

/// The armoured crown the head turns on: flared out from the shaft, straight sides, a
/// dark band round it with lamps, a slope in to its roof.
fn crown(b: &mut MeshBuilder) {
    let under = spire_half(CAP - 1.4) / CROWN;
    b.paint(PLATING);
    b.with_bevel(0.15, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(CROWN), 1.9),
            &[
                Section::new(CAP - 1.6, under),
                Section::new(CAP + 0.6, 1.0),
                Section::new(CAP + HOUSE - 0.5, 1.0),
                Section::new(CAP + HOUSE, 0.93),
            ],
        );
    });
    b.paint(PLATING_DARK);
    b.loft_z(
        &chamfered_rect(Vec2::splat(CROWN + 0.08), 1.95),
        &[Section::new(CAP + 1.0, 1.0), Section::new(CAP + 2.1, 1.0)],
    );
    if b.fine() {
        b.paint(GLOW_LAMP);
        b.radial(4, |b| {
            let d = (CROWN - 0.95) * 1.0 + 0.2;
            b.cuboid(v3(d, d, CAP + 1.55), v3(0.45, 0.45, 0.3));
        });
        faces(b, &[0, 1, 2, 3], |b| {
            b.paint(ACCENT);
            for y in [-2.2, -1.2, 1.2, 2.2] {
                b.block(
                    v3(CROWN + 0.05, y - 0.2, CAP + 1.2),
                    v3(CROWN + 0.2, y + 0.2, CAP + 1.9),
                );
            }
        });
    }
}
