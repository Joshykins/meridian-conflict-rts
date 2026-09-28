//! What stands round the tower: the foundation and plant every tier has, and what each
//! upgrade builds onto it (the tower stays as tall; see the module docs).
//! - Tech 1: an apron slab, a podium the shaft stands on with steps up its front, the
//!   plant house behind taking the drop chute, and one silo fed from it.
//! - Tech 2: a second silo, a separator house in front, two processing pods hung on the
//!   shaft's flanks piped up to the head house, a gallery round the shaft.
//! - Tech 3: buttresses at the shaft's foot, a press house, a capacitor bank, an
//!   outside chute from the head house to the second silo, obstruction lamps.

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::body::{rail_square, Body, HOUSE_HALF};
use super::{chute, CAP, HOUSE, PODIUM};
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// Half the apron slab's width: the 3x3 lot.
const APRON: f32 = 17.6;
/// Top of the apron slab.
const SLAB: f32 = 0.4;
/// Half the podium's width.
const PODIUM_HALF: f32 = 10.2;
/// The plant house's centre (x) and its roof.
const PLANT_X: f32 = -12.6;
const PLANT_TOP: f32 = 8.0;
/// Where the silos stand, behind the flanks.
const SILO: [Vec2; 2] = [Vec2::new(-11.6, -11.6), Vec2::new(-11.6, 11.6)];
const SILO_R: f32 = 3.6;
const SILO_TOP: f32 = 12.0;

/// The podium and the plant behind it as one block: the rest is too small to see.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.block(
        v3(PLANT_X - 3.2, -PODIUM_HALF, 0.0),
        v3(PODIUM_HALF, PODIUM_HALF, PODIUM),
    );
}

/// The apron, the podium with steps up its front, the plant house and the first silo.
pub(super) fn foundation(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.loft_z(
        &chamfered_rect(Vec2::splat(APRON), 3.2),
        &[Section::new(0.0, 1.0), Section::new(SLAB, 0.99)],
    );
    b.paint(PLATING);
    b.with_bevel(0.2, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(PODIUM_HALF), 2.4),
            &[
                Section::new(SLAB, 1.0),
                Section::new(PODIUM - 0.5, 0.97),
                Section::new(PODIUM, 0.95),
            ],
        );
    });
    b.paint(ACCENT);
    b.loft_z(
        &chamfered_rect(Vec2::splat(PODIUM_HALF + 0.12), 2.5),
        &[Section::new(SLAB, 1.0), Section::new(SLAB + 0.7, 1.0)],
    );
    if b.mid() {
        // Steps up the front of the podium.
        b.paint(PLATING_DARK);
        for i in 0..4 {
            let z = SLAB + (PODIUM - SLAB) * (i as f32 + 1.0) / 4.0;
            let x = PODIUM_HALF + 2.4 - i as f32 * 0.8;
            b.block(v3(PODIUM_HALF - 1.0, -2.2, SLAB), v3(x, 2.2, z));
        }
    }
    if b.fine() {
        // Deck plates on the podium round the shaft.
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            b.plate(
                v3(6.0, PODIUM_HALF - 2.4, PODIUM - 0.05),
                Vec2::new(5.0, 2.6),
                0.12,
                0.04,
            );
        });
        team_panel(
            b,
            v3(PODIUM_HALF - 2.0, 0.0, PODIUM - 0.05),
            Vec2::new(2.2, 4.0),
        );
    }
    plant(b);
    silo(b, SILO[0]);
    conveyor(
        b,
        v3(PLANT_X + 1.0, -4.4, PLANT_TOP - 1.2),
        (SILO[0] + Vec2::new(0.4, 2.2)).extend(SILO_TOP + 0.6),
    );
}

/// The upgrades' works, each on its tier's kit.
pub(super) fn tiers(b: &mut MeshBuilder, tech: u8, body: Body) {
    kit(b, tech, 2, 0.3, |b| {
        silo(b, SILO[1]);
        conveyor(
            b,
            v3(PLANT_X + 1.0, 4.4, PLANT_TOP - 1.2),
            (SILO[1] + Vec2::new(0.4, -2.2)).extend(SILO_TOP + 0.6),
        );
        separator(b);
    });
    kit(b, tech, 2, 0.6, |b| {
        pods(b, body);
        gallery(b, body);
    });
    kit(b, tech, 3, 0.35, |b| {
        buttresses(b, body);
        press_house(b);
        capacitors(b);
    });
    kit(b, tech, 3, 0.7, |b| {
        // The outside chute: from the head house's back corner down to the second silo.
        let from = v3(-HOUSE_HALF + 0.8, HOUSE_HALF - 0.4, CAP + 0.8);
        let bend = v3(-9.2, 9.2, SILO_TOP + 6.0);
        chute(b, from, bend, 0.7);
        chute(b, bend, SILO[1].extend(SILO_TOP + 1.2), 0.7);
        if b.fine() {
            b.paint(GLOW_RED);
            b.radial(4, |b| {
                b.cuboid(
                    v3(HOUSE_HALF - 0.6, HOUSE_HALF - 0.6, CAP + HOUSE + 1.3),
                    Vec3::splat(0.45),
                )
            });
        }
    });
}

/// The plant house behind the podium: the drop chute comes into a hopper on its roof.
fn plant(b: &mut MeshBuilder) {
    let at = v3(PLANT_X, 0.0, SLAB);
    let size = v3(6.4, 10.0, PLANT_TOP - SLAB);
    b.paint(PLATING);
    b.with_bevel(0.2, |b| {
        b.chamfered_box(at + Vec3::Z * size.z * 0.5, size, 1.0);
    });
    b.paint(PLATING_DARK);
    b.block(
        at + v3(-size.x * 0.5 - 0.1, -size.y * 0.5 - 0.1, size.z - 1.0),
        at + v3(size.x * 0.5 + 0.1, size.y * 0.5 + 0.1, size.z - 0.4),
    );
    // The hopper the chute drops into.
    b.paint(ACCENT);
    b.prism(v3(PLANT_X + 1.6, 0.0, PLANT_TOP), b.sides(8), 1.8, 1.3, 1.0);
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.prism(v3(PLANT_X + 1.6, 0.0, PLANT_TOP + 0.95), 8, 1.2, 1.2, 0.12);
        // A roller door, and louvres on the back.
        b.paint(ACCENT);
        b.block(
            at + v3(-size.x * 0.5 - 0.15, -2.0, 0.0),
            at + v3(-size.x * 0.5, 2.0, 4.2),
        );
        vent(
            b,
            v3(PLANT_X - 1.2, 2.6, PLANT_TOP),
            Vec2::new(2.2, 2.4),
            3,
            ACCENT,
        );
    }
}

/// A material silo at `at`: a banded drum on a ring footing, a cone roof with the
/// glazed throat a conveyor or chute drops into, a ladder up its side.
fn silo(b: &mut MeshBuilder, at: Vec2) {
    let sides = b.sides(12);
    let base = at.extend(SLAB);
    b.paint(PLATING_DARK);
    b.prism(base, sides, SILO_R + 0.5, SILO_R + 0.3, 1.2);
    b.paint(PLATING);
    b.prism(
        base + Vec3::Z * 1.2,
        sides,
        SILO_R,
        SILO_R,
        SILO_TOP - 1.0 - SLAB - 1.2,
    );
    b.prism(at.extend(SILO_TOP - 1.0), sides, SILO_R, SILO_R * 0.35, 1.4);
    b.paint(ACCENT);
    for z in [4.5, 8.5] {
        b.prism(at.extend(z), sides, SILO_R + 0.12, SILO_R + 0.12, 0.45);
    }
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.prism(at.extend(SILO_TOP + 0.35), b.sides(8), 1.2, 1.0, 0.6);
    if b.fine() {
        b.paint(METAL);
        b.block(
            (at + Vec2::new(SILO_R - 0.1, -0.4)).extend(1.6),
            (at + Vec2::new(SILO_R + 0.35, 0.4)).extend(SILO_TOP - 0.8),
        );
    }
}

/// A covered conveyor gallery from `from` up to `to`, on a trestle under its middle.
fn conveyor(b: &mut MeshBuilder, from: Vec3, to: Vec3) {
    b.paint(PLATING_DARK);
    b.beam(from, to, Vec2::new(1.5, 1.2), Vec2::new(1.5, 1.2));
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.beam(
            from + Vec3::Z * 0.65,
            to + Vec3::Z * 0.65,
            Vec2::new(0.9, 0.2),
            Vec2::new(0.9, 0.2),
        );
    }
    if b.mid() {
        let mid = from.lerp(to, 0.5);
        b.paint(METAL);
        b.beam(
            mid.with_z(SLAB),
            mid - Vec3::Z * 0.6,
            Vec2::splat(0.6),
            Vec2::splat(0.5),
        );
    }
}

/// Tech 2: the separator house on the front-right corner, piped into the podium.
fn separator(b: &mut MeshBuilder) {
    let at = v3(12.0, -12.0, SLAB);
    b.paint(PLATING);
    b.with_bevel(0.15, |b| {
        b.chamfered_box(at + Vec3::Z * 3.0, v3(6.0, 6.0, 6.0), 1.2);
    });
    // The separator drum on its roof.
    b.paint(PLATING_DARK);
    b.prism(at + Vec3::Z * 6.0, b.sides(10), 2.2, 2.0, 2.6);
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.prism(at + Vec3::Z * 7.2, b.sides(10), 2.26, 2.26, 0.7);
    b.paint(METAL);
    b.cylinder_between(
        at + v3(-2.8, 2.8, 4.2),
        v3(7.0, -7.0, PODIUM + 1.2),
        0.5,
        0.5,
        b.sides(6),
    );
    if b.fine() {
        vent(b, at + v3(-1.6, -1.6, 6.0), Vec2::new(1.6, 1.4), 3, ACCENT);
    }
}

/// Tech 2: a processing pod hung on each flank of the shaft, piped up to the head
/// house, a glazed window showing the stream through it.
fn pods(b: &mut MeshBuilder, body: Body) {
    let z = 13.0;
    let face = body.half(z);
    b.mirror_y(|b| {
        let c = v3(0.8, face + 1.7, z);
        b.paint(PLATING);
        b.with_bevel(0.12, |b| b.chamfered_box(c, v3(4.6, 3.4, 6.4), 0.9));
        b.paint(PLATING_DARK);
        b.frustum(
            c - Vec3::Z * 4.4,
            Vec2::new(1.4, 1.2),
            Vec2::new(3.6, 2.8),
            1.2,
            Vec2::ZERO,
        );
        b.paint(ACCENT);
        b.block(c + v3(-2.4, -1.8, 2.2), c + v3(2.4, 1.8, 2.7));
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.block(c + v3(-1.2, 1.68, -1.6), c + v3(1.2, 1.78, 1.4));
        // The feed pipe up to the head house.
        b.paint(METAL);
        b.cylinder_between(
            c + v3(1.4, 0.4, 3.2),
            v3(1.4, HOUSE_HALF - 0.8, CAP - 0.2),
            0.4,
            0.4,
            b.sides(6),
        );
    });
}

/// Tech 2: a gallery round the shaft above the pods, railed.
fn gallery(b: &mut MeshBuilder, body: Body) {
    let z = 20.5;
    let half = body.half(z) + 1.9;
    b.paint(PLATING_DARK);
    b.loft_z(
        &chamfered_rect(Vec2::splat(half), 1.6),
        &[Section::new(z - 0.4, 0.92), Section::new(z, 1.0)],
    );
    if b.fine() {
        b.paint(METAL);
        rail_square(b, z, half - 0.1, 1.0);
        // Brackets under it.
        b.paint(PLATING_DARK);
        b.radial(4, |b| {
            b.beam(
                v3(body.half(z - 2.5), 2.0, z - 2.5),
                v3(half - 0.3, 2.0, z - 0.4),
                Vec2::splat(0.35),
                Vec2::splat(0.35),
            );
        });
    }
}

/// Tech 3: buttresses on the shaft's diagonals, from the podium's corners.
fn buttresses(b: &mut MeshBuilder, body: Body) {
    let height = 11.0;
    let foot = body.half(PODIUM) * std::f32::consts::SQRT_2 - 0.6;
    for i in 0..4 {
        let angle = std::f32::consts::FRAC_PI_4 + i as f32 * std::f32::consts::FRAC_PI_2;
        b.yawed(Vec3::ZERO, angle, |b| {
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [foot - 0.4, PODIUM],
                    [foot + 4.2, PODIUM],
                    [foot + 0.6, PODIUM + height],
                    [foot - 0.4, PODIUM + height],
                ],
                -1.0,
                1.0,
            );
            b.paint(ACCENT);
            b.extrude_y(
                &[
                    [foot + 0.5, PODIUM + height],
                    [foot + 0.9, PODIUM + height - 0.8],
                    [foot + 0.9, PODIUM + height + 0.4],
                    [foot - 0.4, PODIUM + height + 0.4],
                ],
                -1.1,
                1.1,
            );
        });
    }
}

/// Tech 3: a press house on the rear-left, between the plant and the second silo.
fn press_house(b: &mut MeshBuilder) {
    let at = v3(-4.4, 13.4, SLAB);
    b.paint(PLATING);
    b.with_bevel(0.15, |b| {
        b.chamfered_box(at + Vec3::Z * 3.4, v3(6.4, 5.4, 6.8), 0.8);
    });
    b.paint(PLATING_DARK);
    b.plate(at + Vec3::Z * 6.8, Vec2::new(5.2, 4.2), 0.2, 0.06);
    if b.fine() {
        b.paint(METAL);
        for x in [-1.4, 1.4] {
            b.prism(at + v3(x, 0.0, 7.0), 8, 0.8, 0.8, 1.6);
        }
    }
}

/// Tech 3: a capacitor bank on the front-left corner, cabled into the podium.
fn capacitors(b: &mut MeshBuilder) {
    let at = v3(12.4, 11.6, SLAB);
    b.paint(PLATING_DARK);
    b.chamfered_box(at + Vec3::Z * 2.4, v3(5.6, 6.4, 4.8), 0.8);
    b.paint(METAL);
    for (x, y) in [(-1.3, -1.6), (1.3, -1.6), (-1.3, 1.6), (1.3, 1.6)] {
        b.prism(at + v3(x, y, 4.8), b.sides(8), 1.0, 1.0, 2.2);
        if b.fine() {
            b.paint(ACCENT);
            b.prism(at + v3(x, y, 7.0), 8, 0.7, 0.5, 0.4);
            b.paint(METAL);
        }
    }
    if b.fine() {
        b.paint(PLATING_DARK);
        b.beam(
            at + v3(-2.8, -2.0, 1.0),
            v3(PODIUM_HALF - 1.0, 7.0, 1.0),
            Vec2::new(0.8, 0.8),
            Vec2::new(0.8, 0.8),
        );
    }
}
