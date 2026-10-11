//! The Saker's turret, turning on a graphite race over the hull's middle: a housing in
//! two tiers at its back, swept plates lapped back over its roof, the fusion housed in a
//! plated hump with slots let into it, an optic head at its front corner, conduit bays down
//! its flanks, lockers on its quarters and swept tail plates. At its front an open cradle
//! where the gun's trunnion rides between layered cheeks.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::TEAM;
use crate::pattern;

use super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::machine::{armour, hoop, Course, Frame};
use super::super::parts::{bay, inlay, locker, plan_loft, ridged};
use super::DECK;

/// The turret roof, and the hump of plate on it that houses the fusion.
const ROOF: f32 = 10.8;
const HUMP: f32 = 1.1;
/// Where the cradle's cheeks stand off the bore, and how thick they are.
pub(super) const CHEEK: f32 = 2.05;
const CHEEK_THICK: f32 = 0.7;

/// The housing's tiers in plan, half their outlines from the front round to the back.
const LOWER: [[f32; 2]; 5] = [
    [-1.6, 3.8],
    [-3.2, 5.4],
    [-9.4, 5.4],
    [-11.8, 3.6],
    [-12.4, 0.0],
];
const UPPER: [[f32; 2]; 5] = [
    [-2.4, 3.2],
    [-3.8, 4.6],
    [-9.0, 4.6],
    [-11.0, 3.0],
    [-11.6, 0.0],
];
const HUMP_PLAN: [[f32; 2]; 5] = [
    [-4.6, 1.8],
    [-5.4, 2.3],
    [-9.2, 2.3],
    [-10.2, 1.2],
    [-10.2, 0.0],
];

/// The owner's colour as a flat patch on an upward face at height `z`.
fn team_patch(b: &mut MeshBuilder, x: [f32; 2], half: f32, z: f32) {
    b.paint(TEAM);
    b.face(&[
        v3(x[0], -half, z),
        v3(x[1], -half, z),
        v3(x[1], half, z),
        v3(x[0], half, z),
    ]);
}

pub(super) fn turret(b: &mut MeshBuilder, pivot: Vec3) {
    if b.coarse() {
        dark_plate(b);
        b.prism(v3(-5.5, 0.0, DECK), 4, 6.5, 5.5, ROOF - DECK);
        team_patch(b, [-7.5, -4.5], 1.2, ROOF + 0.01);
        return;
    }
    metal(b);
    let segs = b.sides(20);
    hoop(b, v3(0.0, 0.0, DECK + 0.1), 5.6, 0.6, 0.4, segs);
    seam(b);
    b.prism(v3(-6.4, 0.0, DECK), b.sides(12), 5.0, 5.0, 0.45);
    dark_plate(b);
    plan_loft(
        b,
        &LOWER,
        &[
            (DECK + 0.35, 0.96),
            (DECK + 0.9, 1.0),
            (DECK + 2.4, 1.0),
            (DECK + 2.7, 0.96),
        ],
        true,
    );
    seam(b);
    plan_loft(b, &UPPER, &[(DECK + 2.6, 1.02), (DECK + 2.95, 1.02)], false);
    dark_plate(b);
    plan_loft(
        b,
        &UPPER,
        &[(DECK + 2.9, 1.0), (ROOF - 0.5, 1.0), (ROOF, 0.9)],
        false,
    );
    hump(b);
    roof(b);
    flanks(b);
    if b.fine() {
        optics(b);
    }
    cheeks(b, pivot);
}

/// The fusion's hump on the roof: its housing, plates lapped over each side of it with
/// slots let into them, and the owner's colour on its crown.
fn hump(b: &mut MeshBuilder) {
    dark_plate(b);
    plan_loft(
        b,
        &HUMP_PLAN,
        &[
            (ROOF - 0.1, 1.0),
            (ROOF + HUMP * 0.6, 1.0),
            (ROOF + HUMP, 0.84),
        ],
        false,
    );
    team_patch(b, [-8.4, -5.6], 1.1, ROOF + HUMP + 0.01);
    if !b.mid() {
        return;
    }
    b.mirror_y(|b| {
        // A slot let into the hump's flank, and a plate lapped over its back.
        if b.fine() {
            inlay(
                b,
                &[
                    v3(-5.8, 2.3, ROOF + 0.15),
                    v3(-8.8, 2.3, ROOF + 0.15),
                    v3(-8.8, 2.3, ROOF + 0.45),
                    v3(-5.8, 2.3, ROOF + 0.45),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        }
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(-8.9, 1.1, ROOF + HUMP - 0.02),
                v3(-1.0, 0.0, -0.35),
                Vec3::Z,
            ),
            &[[0.0, -1.0], [0.0, 1.0], [1.4, 0.9], [2.4, 0.2], [1.4, -0.6]],
            0.22,
        );
    });
}

/// Swept plates lapped back over the roof either side of the hump, and swept tail plates
/// off the housing's back.
fn roof(b: &mut MeshBuilder) {
    if !b.mid() {
        return;
    }
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count: if fine { 3 } else { 2 },
            step: 2.1,
            len: 2.6,
            half: 0.75,
            tip: 0.6,
            thick: 0.2,
            tail: 0.9,
        }
        .lay(
            b,
            &Frame::new(v3(-3.1, 3.4, ROOF - 0.05), v3(-1.0, 0.0, -0.02), Vec3::Z),
        );
        // Tail plates swept back off the housing's back into spikes.
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(-10.4, 1.6, ROOF - 0.35),
                v3(-1.0, 0.25, -0.3),
                v3(0.0, 0.2, 1.0),
            ),
            &[[0.0, -0.8], [0.0, 0.9], [1.8, 0.8], [3.2, 0.3], [1.8, -0.6]],
            0.24,
        );
        if fine {
            armour(
                b,
                &Frame::new(
                    v3(-11.0, 3.4, DECK + 2.0),
                    v3(-0.6, 0.6, -0.15),
                    v3(0.2, 0.3, 1.0),
                ),
                &[
                    [0.0, -0.6],
                    [0.0, 0.7],
                    [1.4, 0.6],
                    [2.5, 0.15],
                    [1.4, -0.5],
                ],
                0.22,
            );
        }
    });
}

/// Down each flank: a bay where the plates part over the conduits, the cables run along
/// it, and lockers on the housing's quarters.
fn flanks(b: &mut MeshBuilder) {
    if !b.mid() {
        return;
    }
    let fine = b.fine();
    b.mirror_y(|b| {
        let f = Frame::new(v3(-3.8, 5.42, DECK + 1.55), -Vec3::X, Vec3::Y);
        bay(b, &f, 5.0, 0.42);
        if fine {
            metal(b);
            cable(
                b,
                &[
                    v3(-3.4, 5.6, DECK + 1.75),
                    v3(-6.0, 5.65, DECK + 1.75),
                    v3(-8.4, 5.6, DECK + 1.35),
                ],
                0.12,
            );
        }
        locker(
            b,
            &Frame::new(v3(-9.6, 4.6, DECK + 3.2), -Vec3::X, v3(0.0, 1.0, 0.0)),
            Vec3::new(1.8, 1.3, 0.45),
        );
    });
}

/// The optic head on the housing's front corner: a faceted block, red optics let into its
/// face, a smaller pod beside it.
fn optics(b: &mut MeshBuilder) {
    let at = v3(-3.6, -3.2, ROOF + 0.05);
    dark_plate(b);
    ridged(
        b,
        &Frame::new(at, Vec3::X, Vec3::Z),
        &[
            [-1.6, -0.6],
            [0.6, -0.7],
            [1.0, 0.0],
            [0.6, 0.7],
            [-1.6, 0.6],
        ],
        0.7,
        [-0.4, 0.0],
        0.25,
    );
    for (dy, dz) in [(-0.3f32, 0.22f32), (0.3, 0.22), (0.0, 0.5)] {
        inlay(
            b,
            &[
                at + v3(0.92, dy - 0.14, dz - 0.1),
                at + v3(0.92, dy + 0.14, dz - 0.1),
                at + v3(0.92, dy + 0.14, dz + 0.1),
                at + v3(0.92, dy - 0.14, dz + 0.1),
            ],
            Vec3::X,
            pattern::EMBER_SLOT,
        );
    }
    dark_plate(b);
    let pod = at + v3(-0.6, -1.15, 0.0);
    b.chamfered_box(pod + v3(0.0, 0.0, 0.3), v3(1.0, 0.6, 0.6), 0.2);
    inlay(
        b,
        &[
            pod + v3(0.52, -0.2, 0.15),
            pod + v3(0.52, 0.2, 0.15),
            pod + v3(0.52, 0.2, 0.45),
            pod + v3(0.52, -0.2, 0.45),
        ],
        Vec3::X,
        pattern::EMBER_SLOT,
    );
}

/// The cradle's cheeks: a plated wall either side of the gun from the housing's front out
/// past the trunnion, two plates lapped down its outside and swept back past it into
/// spikes, a slot let into its face.
fn cheeks(b: &mut MeshBuilder, pivot: Vec3) {
    let (x0, x1) = (-2.4, 3.6);
    let (z0, z1) = (DECK + 0.3, pivot.z + 1.9);
    let h = z1 - z0;
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        b.with_facets(|b| {
            b.loft(
                &[
                    vec![
                        v3(x0, CHEEK, z0),
                        v3(x1, CHEEK, z0),
                        v3(x1 - h * 0.3, CHEEK, z1),
                        v3(x0, CHEEK, z1),
                    ],
                    vec![
                        v3(x0, CHEEK + CHEEK_THICK, z0),
                        v3(x1, CHEEK + CHEEK_THICK, z0),
                        v3(x1 - h * 0.3, CHEEK + CHEEK_THICK, z1),
                        v3(x0, CHEEK + CHEEK_THICK, z1),
                    ],
                ],
                true,
                true,
            )
        });
        if !b.mid() {
            return;
        }
        let out = CHEEK + CHEEK_THICK;
        dark_plate(b);
        armour(
            b,
            &Frame::new(v3(x1 + 0.3, out, z0 + h * 0.4), -Vec3::X, Vec3::Y),
            &[
                [0.0, -h * 0.35],
                [0.0, h * 0.3],
                [(x1 - x0) * 0.8, h * 0.3],
                [(x1 - x0) + h * 0.6, h * 0.05],
                [(x1 - x0) * 0.8, -h * 0.35],
            ],
            0.32,
        );
        if fine {
            ridged(
                b,
                &Frame::new(v3(x1 - 0.4, out + 0.3, z0 + h * 0.78), -Vec3::X, Vec3::Y),
                &[
                    [0.0, -h * 0.2],
                    [0.0, h * 0.22],
                    [(x1 - x0) * 0.7, h * 0.2],
                    [(x1 - x0) + h * 0.4, h * 0.02],
                    [(x1 - x0) * 0.7, -h * 0.2],
                ],
                0.24,
                [(x1 - x0) * 0.4, 0.0],
                0.16,
            );
            inlay(
                b,
                &[
                    v3(x0 + 0.6, out + 0.33, z0 + h * 0.48),
                    v3(x1 - 1.6, out + 0.33, z0 + h * 0.48),
                    v3(x1 - 1.6, out + 0.33, z0 + h * 0.6),
                    v3(x0 + 0.6, out + 0.33, z0 + h * 0.6),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        }
    });
}
