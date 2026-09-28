//! The tower from the ground to the head house, in the two bodies open to the user
//! (CLAUDE.md section 9):
//! - Spire: an armoured octagonal foot, a tapering octagonal shaft braced by four
//!   sculpted fins on its diagonals, dark inset panels with light slits, and the flow
//!   channel glazed into its back.
//! - Cage: a round armoured foot, a dark core ringed by a glazed flow band, held in a
//!   cage of eight bowed ribs.
//!
//! Both carry an armoured crown (the head house) the head turns on.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::{CAP, FOOT, HOUSE};
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
/// The cage: its foot's radius at the ground, the core's, and the ribs' widest.
const CAGE_FOOT: f32 = 8.4;
const CAGE_CORE: f32 = 4.4;
const CAGE_RIB: f32 = 6.5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Body {
    Spire,
    Cage,
}

impl Body {
    /// How far out from the axis the shaft's face is at height `z` (above the foot):
    /// where works hung on the tower meet it.
    pub(super) fn half(self, z: f32) -> f32 {
        match self {
            Body::Spire => spire_half(z),
            Body::Cage => CAGE_RIB,
        }
    }
}

/// The spire's shaft half width at height `z`.
fn spire_half(z: f32) -> f32 {
    let f = ((z - FOOT) / (CAP - FOOT)).clamp(0.0, 1.0);
    SPIRE_SHAFT * (1.0 - (1.0 - SPIRE_TAPER) * f)
}

/// The foot as one block, the shaft and crown as another, and the first tank.
pub(super) fn coarse(b: &mut MeshBuilder, body: Body) {
    b.paint(PLATING);
    let foot = match body {
        Body::Spire => SPIRE_FOOT,
        Body::Cage => CAGE_FOOT * 0.9,
    };
    b.frustum_open(
        Vec3::ZERO,
        Vec2::splat(foot * 2.0),
        Vec2::splat(foot * 1.6),
        FOOT,
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

pub(super) fn body(b: &mut MeshBuilder, body: Body) {
    match body {
        Body::Spire => spire(b),
        Body::Cage => cage(b),
    }
    crown(b, body);
}

/// Emits `f` on the faces of the tower at these quarter turns (0 = front, +x).
fn faces(b: &mut MeshBuilder, quarters: &[u8], f: impl Fn(&mut MeshBuilder)) {
    for &q in quarters {
        b.yawed(Vec3::ZERO, f32::from(q) * FRAC_PI_2, |b| f(b));
    }
}

// ---- Spire -----------------------------------------------------------------------

fn spire(b: &mut MeshBuilder) {
    // The armoured foot: sloped faces stepping in to a shoulder.
    b.paint(PLATING);
    b.with_bevel(0.2, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(SPIRE_FOOT), 2.8),
            &[
                Section::new(0.0, 1.0),
                Section::new(5.4, 0.93),
                Section::new(FOOT, 0.74),
            ],
        );
    });
    // Its faces: an inset armour panel with slats and a light slit on the flanks and
    // back, a door in a dark frame at the front.
    let face = |z: f32| SPIRE_FOOT * (1.0 - 0.07 * z / 5.4);
    faces(b, &[1, 2, 3], |b| {
        b.paint(PLATING_DARK);
        b.beam(
            v3(face(0.8) - 0.3, 0.0, 0.8),
            v3(face(4.8) - 0.3, 0.0, 4.8),
            Vec2::new(7.0, 0.75),
            Vec2::new(7.0, 0.75),
        );
        if b.fine() {
            b.paint(ACCENT);
            for z in [1.6, 2.6, 3.6] {
                let x = face(z) + 0.1;
                b.block(v3(x - 0.15, -2.6, z), v3(x + 0.05, 2.6, z + 0.35));
            }
        }
        if b.mid() {
            b.paint(GLOW);
            let x = face(5.0);
            b.block(v3(x - 0.2, -2.4, 4.95), v3(x + 0.02, 2.4, 5.15));
        }
    });
    b.paint(PLATING_DARK);
    b.beam(
        v3(face(0.1) - 0.3, 0.0, 0.1),
        v3(face(4.6) - 0.3, 0.0, 4.6),
        Vec2::new(4.2, 0.8),
        Vec2::new(4.2, 0.8),
    );
    b.paint(ACCENT);
    b.beam(
        v3(face(0.1) - 0.1, 0.0, 0.1),
        v3(face(3.8) - 0.1, 0.0, 3.8),
        Vec2::new(3.0, 0.8),
        Vec2::new(3.0, 0.8),
    );
    if b.fine() {
        // Lamps on the foot's cut corners.
        b.paint(GLOW_LAMP);
        b.radial(4, |b| {
            let d = (SPIRE_FOOT * 0.93 - 1.2) * 0.98;
            b.cuboid(v3(d, d, 5.0), Vec3::splat(0.5));
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
    // Tall dark panels up the front and flanks, a light slit at each head.
    faces(b, &[0, 1, 3], |b| {
        let (z0, z1) = (FOOT + 2.0, CAP - 2.5);
        b.paint(PLATING_DARK);
        b.beam(
            v3(spire_half(z0) - 0.2, 0.0, z0),
            v3(spire_half(z1) - 0.2, 0.0, z1),
            Vec2::new(3.4, 0.6),
            Vec2::new(2.8, 0.6),
        );
        if b.mid() {
            b.paint(GLOW);
            b.beam(
                v3(spire_half(z1 + 0.4) + 0.05, 0.0, z1 + 0.4),
                v3(spire_half(z1 + 0.7) + 0.05, 0.0, z1 + 0.7),
                Vec2::new(2.4, 0.12),
                Vec2::new(2.4, 0.12),
            );
        }
        if b.fine() {
            b.paint(ACCENT);
            for z in [12.0, 16.0, 20.0] {
                b.beam(
                    v3(spire_half(z) + 0.1, 0.0, z),
                    v3(spire_half(z + 0.4) + 0.1, 0.0, z + 0.4),
                    Vec2::new(3.0, 0.12),
                    Vec2::new(3.0, 0.12),
                );
            }
        }
    });
    // The flow channel glazed into the back, framed in steel.
    faces(b, &[2], |b| {
        let (z0, z1) = (FOOT - 0.2, CAP + 0.2);
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.beam(
            v3(spire_half(z0) - 0.1, 0.0, z0),
            v3(spire_half(z1) - 0.1, 0.0, z1),
            Vec2::new(1.6, 0.5),
            Vec2::new(1.6, 0.5),
        );
        b.paint(METAL);
        for y in [-1.1, 1.1] {
            b.beam(
                v3(spire_half(z0) - 0.1, y, z0),
                v3(spire_half(z1) - 0.1, y, z1),
                Vec2::new(0.5, 0.7),
                Vec2::new(0.5, 0.7),
            );
        }
    });
    // Four fins on the diagonals, from the foot's shoulder high up the shaft.
    for i in 0..4 {
        b.yawed(Vec3::ZERO, FRAC_PI_4 + i as f32 * FRAC_PI_2, |b| fin(b));
    }
    // A band round the shaft where the fins end.
    b.paint(ACCENT);
    let z = 22.8;
    b.loft_z(
        &chamfered_rect(Vec2::splat(spire_half(z) + 0.2), 1.9),
        &[Section::new(z, 1.0), Section::new(z + 0.7, 1.0)],
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

// ---- Cage ------------------------------------------------------------------------

fn cage(b: &mut MeshBuilder) {
    let sides = b.sides(16);
    // The round armoured foot, stepping in to a shoulder, a dark band round it.
    b.paint(PLATING);
    b.prism(Vec3::ZERO, sides, CAGE_FOOT, CAGE_FOOT * 0.93, 5.4);
    b.prism(
        v3(0.0, 0.0, 5.4),
        sides,
        CAGE_FOOT * 0.93,
        CAGE_RIB,
        FOOT - 5.4,
    );
    b.paint(PLATING_DARK);
    b.prism(
        v3(0.0, 0.0, 1.2),
        sides,
        CAGE_FOOT - 0.02,
        CAGE_FOOT * 0.955,
        3.2,
    );
    if b.mid() {
        b.paint(GLOW);
        b.prism(
            v3(0.0, 0.0, 4.9),
            sides,
            CAGE_FOOT * 0.936,
            CAGE_FOOT * 0.934,
            0.18,
        );
    }
    if b.fine() {
        // Doors round the band, a lamp over each.
        faces(b, &[0, 1, 2, 3], |b| {
            b.paint(ACCENT);
            b.block(
                v3(CAGE_FOOT - 0.4, -1.2, 0.0),
                v3(CAGE_FOOT + 0.05, 1.2, 3.4),
            );
            b.paint(GLOW_LAMP);
            b.cuboid(v3(CAGE_FOOT - 0.05, 0.0, 3.8), v3(0.3, 0.8, 0.25));
        });
    }
    // The dark core, and the glazed band the flow is seen falling through.
    b.paint(PLATING_DARK);
    b.prism(
        v3(0.0, 0.0, FOOT),
        sides,
        CAGE_CORE,
        CAGE_CORE * 0.9,
        CAP - FOOT + 0.2,
    );
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.prism(
        v3(0.0, 0.0, 11.5),
        sides,
        CAGE_CORE * 0.99 + 0.1,
        CAGE_CORE * 0.93 + 0.1,
        11.0,
    );
    // Rings the ribs are tied to.
    b.paint(ACCENT);
    for z in [11.0, 22.6] {
        b.prism(v3(0.0, 0.0, z), sides, CAGE_RIB + 0.1, CAGE_RIB + 0.1, 0.6);
    }
    // Eight ribs bowing out from the shoulder and in to the crown.
    let ribs = if b.fine() { 8 } else { 4 };
    b.radial(ribs, |b| {
        let (a, m, c) = (
            v3(CAGE_RIB - 0.3, 0.0, FOOT - 0.4),
            v3(CAGE_RIB + 0.4, 0.0, 16.8),
            v3(CROWN - 1.0, 0.0, CAP),
        );
        b.paint(PLATING);
        b.beam(a, m, Vec2::new(1.1, 1.0), Vec2::new(0.95, 1.0));
        b.beam(m, c, Vec2::new(0.95, 1.0), Vec2::new(1.1, 1.0));
        if b.fine() {
            b.paint(PLATING_DARK);
            b.cuboid(m + Vec3::X * 0.1, v3(1.1, 1.3, 1.6));
        }
    });
    if b.fine() {
        // A lamp on the upper ring over each quarter.
        faces(b, &[0, 1, 2, 3], |b| {
            b.paint(GLOW_LAMP);
            b.cuboid(v3(CAGE_RIB + 0.2, 0.0, 23.35), v3(0.3, 0.6, 0.25));
        });
    }
}

// ---- Crown -----------------------------------------------------------------------

/// The armoured crown the head turns on: flared out from the shaft, straight sides, a
/// dark band round it with lamps, a slope in to its roof.
fn crown(b: &mut MeshBuilder, body: Body) {
    let under = match body {
        Body::Spire => spire_half(CAP - 1.4),
        Body::Cage => CROWN - 1.0,
    } / CROWN;
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
