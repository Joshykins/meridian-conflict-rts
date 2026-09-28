//! What stands round the tower: the works every tier has, and what each upgrade builds
//! onto it (the tower stays as tall; see the module docs).
//! - Tech 1: a processing module behind the tower fed by a glazed conduit from the
//!   flow channel, and one material tank piped to it.
//! - Tech 2: a second tank, a refinery on the front-right corner, two processing pods
//!   hung on the shaft's flanks piped up to the crown, a collar round the shaft.
//! - Tech 3: a capacitor stack on the front-left corner, armour plates on the foot's flanks,
//!   twin conduits from the tanks up to the crown, obstruction lamps.

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::body::{spire_half, CROWN};
use super::{chute, CAP, FOOT, HOUSE};
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// The processing module's centre (x) and its roof.
const MODULE_X: f32 = -12.4;
const MODULE_TOP: f32 = 7.0;
/// Where the tanks stand, behind the flanks.
const TANK: [Vec2; 2] = [Vec2::new(-11.8, -11.8), Vec2::new(-11.8, 11.8)];
const TANK_R: f32 = 3.0;
const TANK_TOP: f32 = 10.0;

/// The first tank as a block, for the coarse model.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.cuboid_open(
        TANK[0].extend(TANK_TOP * 0.5),
        v3(TANK_R * 1.8, TANK_R * 1.8, TANK_TOP),
    );
}

/// The processing module, the first tank and its pipe.
pub(super) fn foundation(b: &mut MeshBuilder) {
    module(b);
    tank(b, TANK[0]);
    chute(
        b,
        v3(MODULE_X, -4.2, 4.2),
        (TANK[0] + Vec2::new(0.0, TANK_R - 0.2)).extend(4.2),
        0.55,
    );
    // The conduit from the foot of the flow channel down into the module's roof.
    chute(
        b,
        v3(-6.3, 0.0, FOOT + 0.6),
        v3(MODULE_X + 2.0, 0.0, MODULE_TOP + 0.6),
        0.75,
    );
}

/// The upgrades' works, each on its tier's kit.
pub(super) fn tiers(b: &mut MeshBuilder, tech: u8) {
    kit(b, tech, 2, 0.3, |b| {
        tank(b, TANK[1]);
        chute(
            b,
            v3(MODULE_X, 4.2, 4.2),
            (TANK[1] - Vec2::new(0.0, TANK_R - 0.2)).extend(4.2),
            0.55,
        );
        refinery(b);
    });
    kit(b, tech, 2, 0.6, |b| {
        pods(b);
        collar(b);
    });
    kit(b, tech, 3, 0.35, |b| {
        capacitors(b);
        foot_armour(b);
    });
    kit(b, tech, 3, 0.7, |b| {
        // Twin conduits from the tanks up to the crown's back corners.
        for (tank, y) in [(TANK[0], -1.0), (TANK[1], 1.0)] {
            let top = tank.extend(TANK_TOP + 0.6);
            let bend = v3(-9.0, y * 7.4, 17.0);
            chute(b, top, bend, 0.6);
            chute(b, bend, v3(-CROWN + 0.9, y * (CROWN - 0.9), CAP - 0.6), 0.6);
        }
        if b.fine() {
            b.paint(GLOW_RED);
            b.radial(4, |b| {
                b.cuboid(
                    v3(CROWN - 0.9, CROWN - 0.9, CAP + HOUSE + 0.2),
                    Vec3::splat(0.45),
                )
            });
        }
    });
}

/// The processing module behind the tower: armoured sides, a roof sloping in, a dark
/// band with a light slit round it, the conduit's intake on the roof.
fn module(b: &mut MeshBuilder) {
    let half = Vec2::new(3.3, 4.8);
    b.at(v3(MODULE_X, 0.0, 0.0), |b| {
        b.paint(PLATING);
        b.with_bevel(0.15, |b| {
            b.loft_z(
                &chamfered_rect(half, 1.1),
                &[
                    Section::new(0.0, 1.0),
                    Section::new(MODULE_TOP - 1.6, 1.0),
                    Section::new(MODULE_TOP, 0.8),
                ],
            );
        });
        b.paint(PLATING_DARK);
        b.loft_z(
            &chamfered_rect(half + Vec2::splat(0.08), 1.15),
            &[Section::new(1.4, 1.0), Section::new(4.0, 1.0)],
        );
        if b.mid() {
            b.paint(GLOW);
            b.loft_z(
                &chamfered_rect(half + Vec2::splat(0.1), 1.16),
                &[Section::new(4.5, 1.0), Section::new(4.68, 1.0)],
            );
        }
        // The intake on the roof.
        b.paint(PLATING_DARK);
        b.prism(v3(2.0, 0.0, MODULE_TOP - 0.1), b.sides(8), 1.4, 1.1, 0.8);
        if b.fine() {
            // Roof plates and a vent bank, a hatch in the back.
            b.paint(PLATING_DARK);
            b.plate(v3(-0.8, 0.0, MODULE_TOP), Vec2::new(2.4, 5.6), 0.14, 0.05);
            vent(
                b,
                v3(-0.8, 1.6, MODULE_TOP + 0.14),
                Vec2::new(1.8, 1.8),
                4,
                ACCENT,
            );
            b.paint(ACCENT);
            b.block(v3(-half.x - 0.12, -1.6, 0.0), v3(-half.x + 0.05, 1.6, 3.4));
        }
    });
}

/// A material tank at `at`: a dark ring footing, a ribbed drum with a glazed sight
/// strip facing the tower, a domed top with a filler cap.
fn tank(b: &mut MeshBuilder, at: Vec2) {
    let sides = b.sides(14);
    b.at(at.extend(0.0), |b| {
        b.paint(PLATING_DARK);
        b.prism(Vec3::ZERO, sides, TANK_R + 0.5, TANK_R + 0.35, 1.0);
        b.paint(PLATING);
        b.with_bevel(0.1, |b| {
            b.prism(v3(0.0, 0.0, 1.0), sides, TANK_R, TANK_R, TANK_TOP - 2.2);
        });
        b.prism(
            v3(0.0, 0.0, TANK_TOP - 1.2),
            sides,
            TANK_R,
            TANK_R * 0.72,
            0.9,
        );
        b.prism(
            v3(0.0, 0.0, TANK_TOP - 0.3),
            sides,
            TANK_R * 0.72,
            TANK_R * 0.3,
            0.5,
        );
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, TANK_TOP + 0.2), 8, 0.7, 0.7, 0.5);
        // Ribs.
        b.paint(PLATING_DARK);
        b.radial(if b.fine() { 6 } else { 3 }, |b| {
            b.beam(
                v3(TANK_R - 0.05, 0.0, 1.0),
                v3(TANK_R - 0.05, 0.0, TANK_TOP - 1.2),
                Vec2::new(0.5, 0.35),
                Vec2::new(0.5, 0.35),
            );
        });
        // The sight strip, facing the tower.
        let to_tower = (-at).normalize();
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        let p = (to_tower * (TANK_R + 0.02)).extend(0.0);
        b.beam(
            p + Vec3::Z * 2.0,
            p + Vec3::Z * (TANK_TOP - 2.0),
            Vec2::new(0.8, 0.2),
            Vec2::new(0.8, 0.2),
        );
        if b.fine() {
            b.paint(GLOW_LAMP);
            b.cuboid(p + Vec3::Z * (TANK_TOP - 1.6), v3(0.3, 0.3, 0.25));
        }
    });
}

/// Tech 2: the refinery on the front-right corner: a low armoured block with three
/// domed cells on its roof, piped to the tower's foot.
fn refinery(b: &mut MeshBuilder) {
    let at = v3(11.8, -11.8, 0.0);
    b.at(at, |b| {
        b.paint(PLATING);
        b.with_bevel(0.15, |b| {
            b.loft_z(
                &chamfered_rect(Vec2::new(3.4, 3.4), 1.0),
                &[
                    Section::new(0.0, 1.0),
                    Section::new(3.6, 1.0),
                    Section::new(4.4, 0.86),
                ],
            );
        });
        b.paint(PLATING_DARK);
        b.loft_z(
            &chamfered_rect(Vec2::new(3.48, 3.48), 1.05),
            &[Section::new(1.0, 1.0), Section::new(2.8, 1.0)],
        );
        for (x, y) in [(-1.3, 1.3), (1.3, 1.3), (0.0, -1.3)] {
            b.paint(METAL);
            b.prism(v3(x, y, 4.3), b.sides(10), 1.05, 1.05, 1.3);
            b.prism(v3(x, y, 5.6), b.sides(10), 1.05, 0.4, 0.55);
            if b.mid() {
                b.paint(GLOW);
                b.prism(v3(x, y, 5.1), b.sides(10), 1.08, 1.08, 0.14);
            }
        }
    });
    b.paint(METAL);
    b.cylinder_between(
        at + v3(-3.2, 3.2, 2.2),
        v3(6.2, -6.2, 2.4),
        0.45,
        0.45,
        b.sides(6),
    );
}

/// Tech 2: a processing pod hung on each flank of the shaft: an armoured capsule with a
/// sloped belly, a dark inset, a glazed window on the flow, a feed pipe up to the crown.
fn pods(b: &mut MeshBuilder) {
    let z = 14.0;
    let face = spire_half(z);
    b.mirror_y(|b| {
        let c = v3(0.6, face + 1.6, z);
        b.paint(PLATING);
        b.with_bevel(0.12, |b| {
            b.loft_z(
                &chamfered_rect(Vec2::new(2.3, 1.6), 0.7)
                    .iter()
                    .map(|p| [p[0] + c.x, p[1] + c.y])
                    .collect::<Vec<_>>(),
                &[
                    Section::scaled(z - 3.6, 0.5, 0.6),
                    Section::new(z - 2.2, 1.0),
                    Section::new(z + 2.6, 1.0),
                    Section::new(z + 3.2, 0.85),
                ],
            );
        });
        b.paint(PLATING_DARK);
        b.block(c + v3(-2.0, 1.58, -1.4), c + v3(2.0, 1.72, 1.8));
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.block(c + v3(-1.1, 1.7, -0.8), c + v3(1.1, 1.8, 1.2));
        if b.mid() {
            b.paint(GLOW);
            b.block(c + v3(-1.8, 1.68, 2.1), c + v3(1.8, 1.76, 2.26));
        }
        // The feed pipe up to the crown.
        b.paint(METAL);
        b.cylinder_between(
            c + v3(1.4, 0.2, 3.1),
            v3(1.4, CROWN - 1.2, CAP - 1.2),
            0.35,
            0.35,
            b.sides(6),
        );
    });
}

/// Tech 2: a dark collar round the shaft above the pods, lamps at its corners.
fn collar(b: &mut MeshBuilder) {
    let z = 19.2;
    let half = spire_half(z) + 0.8;
    b.paint(PLATING_DARK);
    b.loft_z(
        &chamfered_rect(Vec2::splat(half), 2.0),
        &[
            Section::new(z - 0.6, 0.9),
            Section::new(z, 1.0),
            Section::new(z + 0.8, 1.0),
            Section::new(z + 1.2, 0.93),
        ],
    );
    if b.fine() {
        b.paint(GLOW_LAMP);
        b.radial(4, |b| {
            let d = half - 0.7;
            b.cuboid(v3(d, d, z + 0.4), v3(0.4, 0.4, 0.3));
        });
    }
}

/// Tech 3: a capacitor stack on the front-left corner: an armoured block, four cells
/// with lit caps, cabled into the tower's foot.
fn capacitors(b: &mut MeshBuilder) {
    let at = v3(11.8, 11.8, 0.0);
    b.at(at, |b| {
        b.paint(PLATING_DARK);
        b.with_bevel(0.12, |b| {
            b.chamfered_box(v3(0.0, 0.0, 1.8), v3(6.4, 6.4, 3.6), 1.0);
        });
        for (x, y) in [(-1.4, -1.4), (1.4, -1.4), (-1.4, 1.4), (1.4, 1.4)] {
            b.paint(PLATING);
            b.prism(v3(x, y, 3.6), b.sides(10), 1.0, 1.0, 2.6);
            b.paint(ACCENT);
            b.prism(v3(x, y, 6.2), b.sides(10), 1.05, 0.8, 0.3);
            b.paint(GLOW);
            b.prism(v3(x, y, 6.5), b.sides(10), 0.6, 0.45, 0.3);
        }
    });
    if b.fine() {
        b.paint(PLATING_DARK);
        b.beam(
            at + v3(-3.2, -2.2, 1.0),
            v3(6.4, 5.0, 1.0),
            Vec2::new(0.8, 0.8),
            Vec2::new(0.8, 0.8),
        );
    }
}

/// Tech 3: heavy plates over the foot's flanks.
fn foot_armour(b: &mut MeshBuilder) {
    let r = 8.7;
    for q in [1, 3] {
        b.yawed(Vec3::ZERO, q as f32 * std::f32::consts::FRAC_PI_2, |b| {
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [r - 0.9, 0.0],
                    [r + 0.5, 0.0],
                    [r - 0.2, 4.4],
                    [r - 0.9, 4.4],
                ],
                -2.8,
                2.8,
            );
            b.paint(ACCENT);
            b.extrude_y(
                &[
                    [r - 0.85, 4.4],
                    [r - 0.2, 4.4],
                    [r - 0.3, 5.0],
                    [r - 0.85, 5.0],
                ],
                -2.9,
                2.9,
            );
        });
    }
}
