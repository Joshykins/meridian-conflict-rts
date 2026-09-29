//! The foundation the tower stands on and the works round it: mirrored front to back.
//! The right side as placed (+y) is the control tower's, the left (-y) the tanks'. Each
//! upgrade builds onto the last (the tower stays as tall; see the module docs).
//! - Tech 1: a two-step foundation with steps up its front and a lit kerb, the control
//!   tower on the right bridged to the shaft, and a material tank on each left corner
//!   fed by a conduit off the flow channel on the face nearest it.
//! - Tech 2: a refinery on each right corner either side of the control tower, piped to
//!   the foot, and a collar round the shaft.
//! - Tech 3: a capacitor stack on the left between the tanks, armour plates on the
//!   foot's flanks, conduits from the tanks up to the crown, obstruction lamps.

use std::f32::consts::{FRAC_PI_2, PI};

use glam::{Affine3A, Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::body::{spire_half, CROWN};
use super::{chute, BASE, CAP, HOUSE};
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// Half the foundation's lower step, and its top.
const APRON: f32 = 15.4;
const STEP: f32 = 1.4;
/// Half the plinth the tower stands on (its top is `BASE`).
const PLINTH: f32 = 12.0;
/// The corners' works stand this far out on each axis, on the lower step.
const CORNER: f32 = 12.2;
const TANK_R: f32 = 3.0;
/// Top of a tank above the step.
const TANK_TOP: f32 = STEP + 8.6;
/// The control tower's centre (on the -x axis), its cab's floor and roof.
const CONTROL_X: f32 = -13.0;
const CAB: f32 = 15.0;
const CAB_TOP: f32 = 18.6;

/// The foundation as one block, for the coarse model.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.frustum_open(
        Vec3::ZERO,
        Vec2::splat(APRON * 2.0),
        Vec2::splat(PLINTH * 2.0),
        BASE,
        Vec2::ZERO,
    );
}

/// The foundation, the control tower, and the tanks on their conduits.
pub(super) fn foundation(b: &mut MeshBuilder) {
    // The lower step: dark, a lit kerb round its foot.
    b.paint(PLATING_DARK);
    b.with_bevel(0.15, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(APRON), 2.2),
            &[
                Section::new(0.0, 1.0),
                Section::new(STEP - 0.3, 1.0),
                Section::new(STEP, 0.985),
            ],
        );
    });
    if b.mid() {
        b.paint(GLOW);
        b.loft_z(
            &chamfered_rect(Vec2::splat(APRON + 0.05), 2.22),
            &[Section::new(0.5, 1.0), Section::new(0.65, 1.0)],
        );
    }
    // The plinth: plated, a dark band round its foot.
    b.paint(PLATING);
    b.with_bevel(0.15, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(PLINTH), 5.0),
            &[
                Section::new(STEP - 0.1, 1.0),
                Section::new(BASE - 0.3, 1.0),
                Section::new(BASE, 0.985),
            ],
        );
    });
    b.paint(ACCENT);
    b.loft_z(
        &chamfered_rect(Vec2::splat(PLINTH + 0.1), 5.05),
        &[Section::new(STEP, 1.0), Section::new(STEP + 0.45, 1.0)],
    );
    // Steps up the front of the plinth.
    b.paint(PLATING_DARK);
    b.block(
        v3(PLINTH - 0.5, -3.0, STEP),
        v3(PLINTH + 1.3, 3.0, STEP + 0.7),
    );
    b.block(
        v3(PLINTH - 0.5, -3.0, STEP + 0.7),
        v3(PLINTH + 0.6, 3.0, BASE),
    );
    if b.fine() {
        // Deck plates on the plinth's corners, and a lamp on each of the step's.
        b.paint(PLATING_DARK);
        b.radial(4, |b| {
            b.plate(v3(8.4, 8.4, BASE), Vec2::new(3.0, 3.0), 0.1, 0.04);
        });
        team_panel(b, v3(PLINTH - 2.2, 0.0, BASE), Vec2::new(2.0, 4.4));
    }
    // The control tower on the right as placed (+y); a tank on each corner of the left,
    // each fed off the channel down the face nearest it (front or back).
    b.yawed(Vec3::ZERO, -FRAC_PI_2, control_tower);
    left_corners(b, |b| {
        tank(b, Vec2::new(-CORNER, CORNER));
        // The conduit off the flow channel on this side: out from the face, then back
        // along the flank over the plinth to the tank's top, on a stanchion.
        let from = v3(0.0, spire_half(10.0) + 0.3, 10.0);
        let bend = v3(-3.8, CORNER - 0.6, 11.2);
        let to = v3(-CORNER + 0.8, CORNER - 0.6, TANK_TOP + 0.5);
        chute(b, from, bend, 0.6);
        chute(b, bend, to, 0.6);
        b.paint(METAL);
        b.beam(
            v3(-3.8, CORNER - 0.6, STEP),
            bend - Vec3::Z * 0.6,
            Vec2::splat(0.5),
            Vec2::splat(0.45),
        );
    });
}

/// The upgrades' works, each on its tier's kit.
pub(super) fn tiers(b: &mut MeshBuilder, tech: u8) {
    kit(b, tech, 2, 0.3, |b| {
        mirror_x(b, |b| refinery(b, Vec2::new(CORNER, CORNER)));
    });
    kit(b, tech, 2, 0.6, collar);
    kit(b, tech, 3, 0.35, |b| {
        b.yawed(Vec3::ZERO, PI, capacitors);
        foot_armour(b);
    });
    kit(b, tech, 3, 0.7, |b| {
        // Conduits from the tanks' tops up to the crown's corners over them.
        left_corners(b, |b| {
            let top = v3(-CORNER, CORNER, TANK_TOP + 0.6);
            let bend = v3(-9.0, 7.4, 18.0);
            chute(b, top, bend, 0.6);
            chute(b, bend, v3(-CROWN + 0.9, CROWN - 0.9, CAP - 0.6), 0.6);
        });
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

/// Emits `f` twice: as written, and mirrored front to back (x negated).
fn mirror_x(b: &mut MeshBuilder, f: impl Fn(&mut MeshBuilder)) {
    f(b);
    b.with(Affine3A::from_scale(Vec3::new(-1.0, 1.0, 1.0)), |b| f(b));
}

/// Emits `f`, written for the back corner on +y fed from the +y face, on both corners of
/// the left (-y) side instead, each fed from the face nearest it.
fn left_corners(b: &mut MeshBuilder, f: impl Fn(&mut MeshBuilder)) {
    mirror_x(b, |b| b.yawed(Vec3::ZERO, FRAC_PI_2, |b| f(b)));
}

/// The control tower, built on the centre line behind the spire (-x) and turned to its
/// right side: a plated column, a glazed cab on it with a sensor dome and a mast, a
/// bridge from the cab to the shaft.
fn control_tower(b: &mut MeshBuilder) {
    b.at(v3(CONTROL_X, 0.0, 0.0), |b| {
        b.paint(PLATING);
        b.with_bevel(0.12, |b| {
            b.loft_z(
                &chamfered_rect(Vec2::new(2.0, 2.4), 0.6),
                &[Section::new(STEP - 0.1, 1.0), Section::new(CAB + 0.2, 0.9)],
            );
        });
        b.paint(PLATING_DARK);
        b.block(v3(-2.05, -0.9, STEP + 1.0), v3(-1.7, 0.9, CAB - 1.0));
        b.paint(PLATING);
        b.with_bevel(0.12, |b| {
            b.loft_z(
                &chamfered_rect(Vec2::new(3.0, 3.3), 0.9),
                &[
                    Section::new(CAB - 0.6, 0.7),
                    Section::new(CAB + 0.3, 1.0),
                    Section::new(CAB_TOP - 0.5, 1.0),
                    Section::new(CAB_TOP, 0.86),
                ],
            );
        });
        // The cab's glazing all round, lit from inside.
        b.paint(GLASS);
        b.loft_z(
            &chamfered_rect(Vec2::new(3.06, 3.36), 0.92),
            &[Section::new(CAB + 0.8, 1.0), Section::new(CAB + 2.2, 1.0)],
        );
        if b.mid() {
            b.paint(GLOW_LAMP);
            b.block(v3(-3.08, -1.8, CAB + 1.25), v3(-3.0, 1.8, CAB + 1.75));
        }
        b.paint(METAL);
        b.prism(v3(0.6, 0.0, CAB_TOP), b.sides(8), 1.1, 0.4, 0.9);
        if b.fine() {
            antenna(b, v3(-1.6, 1.4, CAB_TOP), 4.0, 0.0);
            b.paint(GLOW_RED);
            b.cuboid(v3(0.6, 0.0, CAB_TOP + 1.0), Vec3::splat(0.3));
        }
    });
    // The bridge to the shaft.
    let z = CAB + 0.6;
    b.paint(PLATING_DARK);
    b.beam(
        v3(CONTROL_X + 2.6, 0.0, z),
        v3(-spire_half(z) + 0.1, 0.0, z),
        Vec2::new(1.8, 1.4),
        Vec2::new(1.8, 1.4),
    );
    if b.fine() {
        b.paint(GLOW);
        b.beam(
            v3(CONTROL_X + 2.6, 0.0, z - 0.72),
            v3(-spire_half(z) + 0.1, 0.0, z - 0.72),
            Vec2::new(0.6, 0.06),
            Vec2::new(0.6, 0.06),
        );
    }
}

/// A material tank at `at` on the lower step: a dark ring footing, a ribbed drum with a
/// glazed sight strip facing the tower, a domed top with a filler cap.
fn tank(b: &mut MeshBuilder, at: Vec2) {
    let sides = b.sides(12);
    let to_tower = (-at).normalize();
    b.at(at.extend(STEP), |b| {
        let top = TANK_TOP - STEP;
        b.paint(PLATING_DARK);
        b.prism(Vec3::ZERO, sides, TANK_R + 0.4, TANK_R + 0.3, 0.9);
        b.paint(PLATING);
        b.with_bevel(0.1, |b| {
            b.prism(v3(0.0, 0.0, 0.9), sides, TANK_R, TANK_R, top - 2.1);
        });
        b.prism(v3(0.0, 0.0, top - 1.2), sides, TANK_R, TANK_R * 0.72, 0.9);
        b.prism(
            v3(0.0, 0.0, top - 0.3),
            sides,
            TANK_R * 0.72,
            TANK_R * 0.3,
            0.5,
        );
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, top + 0.2), 8, 0.7, 0.7, 0.5);
        b.paint(PLATING_DARK);
        b.radial(if b.fine() { 4 } else { 3 }, |b| {
            b.beam(
                v3(TANK_R - 0.05, 0.0, 0.9),
                v3(TANK_R - 0.05, 0.0, top - 1.2),
                Vec2::new(0.5, 0.35),
                Vec2::new(0.5, 0.35),
            );
        });
        let p = (to_tower * (TANK_R + 0.02)).extend(0.0);
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.beam(
            p + Vec3::Z * 1.8,
            p + Vec3::Z * (top - 1.8),
            Vec2::new(0.8, 0.2),
            Vec2::new(0.8, 0.2),
        );
        if b.fine() {
            b.paint(GLOW_LAMP);
            b.cuboid(p + Vec3::Z * (top - 1.4), v3(0.3, 0.3, 0.25));
        }
    });
}

/// Tech 2: a refinery on a front corner (`at`): a low armoured block with three domed
/// cells on its roof, piped to the tower's foot.
fn refinery(b: &mut MeshBuilder, at: Vec2) {
    b.at(at.extend(STEP), |b| {
        b.paint(PLATING);
        b.with_bevel(0.15, |b| {
            b.loft_z(
                &chamfered_rect(Vec2::new(3.0, 3.0), 0.9),
                &[
                    Section::new(0.0, 1.0),
                    Section::new(3.2, 1.0),
                    Section::new(3.9, 0.86),
                ],
            );
        });
        b.paint(PLATING_DARK);
        b.loft_z(
            &chamfered_rect(Vec2::new(3.08, 3.08), 0.95),
            &[Section::new(0.8, 1.0), Section::new(2.5, 1.0)],
        );
        for (x, y) in [(-1.1, -1.1), (1.1, -1.1), (0.0, 1.1)] {
            b.paint(METAL);
            b.prism(v3(x, y, 3.8), b.sides(8), 0.95, 0.95, 1.2);
            b.prism(v3(x, y, 5.0), b.sides(8), 0.95, 0.35, 0.5);
            if b.fine() {
                b.paint(GLOW);
                b.prism(v3(x, y, 4.55), 8, 0.98, 0.98, 0.14);
            }
        }
    });
    b.paint(METAL);
    b.cylinder_between(
        (at - Vec2::splat(2.6)).extend(STEP + 2.0),
        v3(6.4, 6.4, BASE + 1.4),
        0.45,
        0.45,
        b.sides(6),
    );
}

/// Tech 2: a dark collar round the shaft, lamps at its corners.
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

/// Tech 3: a capacitor stack, built on the +y flank of the lower step and turned to the
/// left: an armoured block, three cells with lit caps, cabled into the plinth.
fn capacitors(b: &mut MeshBuilder) {
    let at = v3(0.0, 13.7, STEP);
    b.at(at, |b| {
        b.paint(PLATING_DARK);
        b.with_bevel(0.12, |b| {
            b.chamfered_box(v3(0.0, 0.0, 1.5), v3(6.6, 2.8, 3.0), 0.7);
        });
        for x in [-2.1, 0.0, 2.1] {
            b.paint(PLATING);
            b.prism(v3(x, 0.0, 3.0), b.sides(8), 0.9, 0.9, 2.2);
            if b.fine() {
                b.paint(ACCENT);
                b.prism(v3(x, 0.0, 5.2), 8, 0.95, 0.72, 0.3);
            }
            b.paint(GLOW);
            b.prism(v3(x, 0.0, 5.5), b.sides(8), 0.55, 0.4, 0.3);
        }
    });
    if b.fine() {
        b.paint(PLATING_DARK);
        b.beam(
            at + v3(0.0, -1.5, 0.6),
            v3(0.0, PLINTH + 0.1, STEP + 0.6),
            Vec2::new(1.0, 0.6),
            Vec2::new(1.0, 0.6),
        );
    }
}

/// Tech 3: heavy plates over the foot's flanks, under the conduits.
fn foot_armour(b: &mut MeshBuilder) {
    let r = 8.7;
    for q in [1, 3] {
        b.yawed(Vec3::ZERO, q as f32 * std::f32::consts::FRAC_PI_2, |b| {
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [r - 0.9, BASE],
                    [r + 0.4, BASE],
                    [r - 0.3, 5.6],
                    [r - 0.9, 5.6],
                ],
                -2.8,
                2.8,
            );
            b.paint(ACCENT);
            b.extrude_y(
                &[
                    [r - 0.85, 5.6],
                    [r - 0.3, 5.6],
                    [r - 0.4, 6.1],
                    [r - 0.85, 6.1],
                ],
                -2.9,
                2.9,
            );
        });
    }
}
