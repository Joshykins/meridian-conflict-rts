//! The Sarissa's turret, turning on a toothed graphite race over the deck: a long housing
//! in two tiers with a seam-dark band between them, its roof low under the gun so the
//! breech can sink into it as the gun rises, tall plated cheeks either side of the
//! trunnion with plates swept back off them into spikes, a capacitor bank across its back
//! whose lids stand up with the heat (`charge_gear::VENT`), feathered glacis plates down
//! its nose and swept tail plates off its back.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::{HEAT_STAGE, VENT};
use crate::material::TEAM;
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{armour, hoop, swept, teeth, Course, Frame};
use super::super::super::parts::{bay, inlay, plan_loft, ridged};
use super::super::fusion_kit::coil_light;
use super::super::team_patch;
use super::{CHEEK, DECK, LINE, RACE, ROOF};

/// The housing's tiers in plan, half their outlines from the front round to the back.
const LOWER: [[f32; 2]; 5] = [
    [13.0, 7.0],
    [7.0, 11.4],
    [-17.0, 11.4],
    [-24.0, 8.0],
    [-26.5, 0.0],
];
const UPPER: [[f32; 2]; 5] = [
    [10.0, 6.0],
    [5.0, 10.4],
    [-16.4, 10.4],
    [-23.0, 7.2],
    [-25.4, 0.0],
];
/// The lower tier's top.
const WAIST: f32 = DECK + 3.0;
/// How thick the cheeks are, and how far back and forward of the trunnion they run.
const CHEEK_THICK: f32 = 2.2;
const CHEEK_X: (f32, f32) = (-8.0, 8.0);
/// The capacitor bank across the back.
const BANK: (f32, f32) = (-23.0, -13.0);

pub(super) fn draw(b: &mut MeshBuilder) {
    race(b);
    dark_plate(b);
    plan_loft(
        b,
        &LOWER,
        &[
            (DECK + 0.2, 0.97),
            (DECK + 0.8, 1.0),
            (WAIST, 1.0),
            (WAIST + 0.3, 0.97),
        ],
        true,
    );
    seam(b);
    plan_loft(
        b,
        &UPPER,
        &[(WAIST + 0.2, 1.02), (WAIST + 0.6, 1.02)],
        false,
    );
    dark_plate(b);
    plan_loft(
        b,
        &UPPER,
        &[(WAIST + 0.5, 1.0), (ROOF - 0.5, 1.0), (ROOF, 0.94)],
        false,
    );
    team_patch(b, -12.0, -9.0, 4.0, ROOF + 0.01);
    cheeks(b);
    bank(b);
    if !b.mid() {
        return;
    }
    glacis(b);
    flanks(b);
    tail(b);
}

/// The race the turret turns on: a seam-dark skirt, a graphite ring with teeth round it.
fn race(b: &mut MeshBuilder) {
    seam(b);
    b.prism(
        v3(-5.0, 0.0, DECK - 0.1),
        b.sides(16),
        RACE - 1.0,
        RACE - 1.4,
        0.6,
    );
    metal(b);
    hoop(
        b,
        v3(0.0, 0.0, DECK + 0.25),
        RACE - 0.6,
        1.4,
        0.5,
        b.sides(32),
    );
    if b.fine() {
        teeth(
            b,
            v3(0.0, 0.0, DECK + 0.1),
            RACE + 0.1,
            48,
            Vec3::new(0.5, 0.7, 0.4),
        );
    }
}

/// The cheeks: a plated wall either side of the gun, up past the trunnion, its front edge
/// raked back, two plates lapped down its outside swept back past it into spikes, a slot
/// let into its face, a bearing drum on its inside for the trunnion and the owner's
/// colour along its top.
fn cheeks(b: &mut MeshBuilder) {
    let (x0, x1) = CHEEK_X;
    let (z0, z1) = (ROOF - 0.3, LINE.pivot.z + 5.5);
    let h = z1 - z0;
    let fine = b.fine();
    b.mirror_y(|b| {
        let (y0, y1) = (CHEEK, CHEEK + CHEEK_THICK);
        let outline = |y: f32| {
            vec![
                v3(x0 - 2.0, y, z0),
                v3(x1 + 3.0, y, z0),
                v3(x1 - h * 0.25, y, z1),
                v3(x0 + h * 0.1, y, z1),
            ]
        };
        dark_plate(b);
        b.with_facets(|b| b.loft(&[outline(y0), outline(y1)], true, true));
        b.paint(TEAM);
        b.face(&[
            v3(x0 + h * 0.1 + 0.4, y0 + 0.2, z1 + 0.01),
            v3(x1 - h * 0.25 - 0.4, y0 + 0.2, z1 + 0.01),
            v3(x1 - h * 0.25 - 0.4, y1 - 0.2, z1 + 0.01),
            v3(x0 + h * 0.1 + 0.4, y1 - 0.2, z1 + 0.01),
        ]);
        // The trunnion's bearing.
        metal(b);
        b.cylinder_between(
            v3(0.0, y0 - 0.6, LINE.pivot.z),
            v3(0.0, y1 + 0.5, LINE.pivot.z),
            3.0,
            3.0,
            b.sides(14),
        );
        if !b.mid() {
            return;
        }
        dark_plate(b);
        armour(
            b,
            &Frame::new(v3(x1 + 1.5, y1, z0 + h * 0.62), -Vec3::X, Vec3::Y),
            &[
                [0.0, -h * 0.26],
                [0.0, h * 0.3],
                [12.0, h * 0.26],
                [19.0, h * 0.04],
                [12.0, -h * 0.26],
            ],
            0.45,
        );
        armour(
            b,
            &Frame::new(v3(x1 + 2.6, y1, z0 + h * 0.2), -Vec3::X, Vec3::Y),
            &[
                [0.0, -h * 0.18],
                [0.0, h * 0.14],
                [11.0, h * 0.12],
                [16.0, -h * 0.05],
                [11.0, -h * 0.18],
            ],
            0.4,
        );
        if fine {
            inlay(
                b,
                &[
                    v3(x0 + 0.5, y1 + 0.46, z0 + h * 0.45),
                    v3(x1 - 1.5, y1 + 0.46, z0 + h * 0.45),
                    v3(x1 - 1.5, y1 + 0.46, z0 + h * 0.45 + 0.45),
                    v3(x0 + 0.5, y1 + 0.46, z0 + h * 0.45 + 0.45),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        }
    });
}

/// The capacitor bank across the turret's back: a plated block standing on the roof, a lit
/// grille in its top under a row of lids that stand up with the heat of a burn, ridged
/// plates down its back face, and slots in its flanks.
fn bank(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1) = BANK;
    let (half, z0, z1) = (8.6, ROOF - 0.4, ROOF + 4.6);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                vec![
                    v3(x0, -half, z0),
                    v3(x1, -half, z0),
                    v3(x1, half, z0),
                    v3(x0, half, z0),
                ],
                vec![
                    v3(x0 + 0.8, -half + 0.6, z1),
                    v3(x1 - 0.4, -half + 0.6, z1),
                    v3(x1 - 0.4, half - 0.6, z1),
                    v3(x0 + 0.8, half - 0.6, z1),
                ],
            ],
            false,
            true,
        )
    });
    coil_light(b, HEAT_STAGE);
    b.face(&[
        v3(x0 + 1.4, -half + 1.2, z1 + 0.02),
        v3(x1 - 1.0, -half + 1.2, z1 + 0.02),
        v3(x1 - 1.0, half - 1.2, z1 + 0.02),
        v3(x0 + 1.4, half - 1.2, z1 + 0.02),
    ]);
    b.with_charge_gear(VENT, |b| {
        let n = 5;
        let w = (half * 2.0 - 2.4) / n as f32;
        for k in 0..n {
            let y = -half + 1.2 + w * k as f32;
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3((x0 + x1) * 0.5, y + 0.1, z1 + 0.02), Vec3::Y, Vec3::Z),
                &[
                    [0.0, -(x1 - x0) * 0.5 + 0.8],
                    [0.0, (x1 - x0) * 0.5 - 0.6],
                    [w - 0.4, (x1 - x0) * 0.5 - 0.6],
                    [w - 0.2, 0.0],
                    [w - 0.4, -(x1 - x0) * 0.5 + 0.8],
                ],
                0.35,
            );
        }
    });
    if !b.mid() {
        return;
    }
    dark_plate(b);
    ridged(
        b,
        &Frame::new(v3(x0 + 0.42, 0.0, z0 + 0.6), Vec3::Z, -v3(1.0, 0.0, -0.16)),
        &[[0.0, -6.6], [3.4, -6.0], [3.4, 6.0], [0.0, 6.6]],
        0.3,
        [1.7, 0.0],
        0.35,
    );
    if fine {
        b.mirror_y(|b| {
            inlay(
                b,
                &[
                    v3(x0 + 1.4, half - 0.28, z0 + 2.0),
                    v3(x1 - 1.0, half - 0.28, z0 + 2.0),
                    v3(x1 - 1.0, half - 0.28, z0 + 2.4),
                    v3(x0 + 1.4, half - 0.28, z0 + 2.4),
                ],
                v3(0.0, 1.0, 0.12),
                pattern::EMBER_SLOT,
            );
        });
    }
}

/// Feathered plates lapped down the housing's nose under the gun, either side of the
/// middle, and a ridged plate over the nose itself.
fn glacis(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count: if fine { 3 } else { 2 },
            step: 2.2,
            len: 3.0,
            half: 1.8,
            tip: 0.4,
            thick: 0.32,
            tail: 1.0,
        }
        .lay(
            b,
            &Frame::new(
                v3(9.0, 4.4, ROOF - 0.2),
                v3(1.0, 0.25, -0.6),
                v3(0.6, 0.2, 1.0),
            ),
        );
    });
    dark_plate(b);
    ridged(
        b,
        &Frame::new(v3(12.6, 0.0, DECK + 0.8), Vec3::Z, v3(1.0, 0.0, 0.1)),
        &[[0.0, -4.4], [3.6, -3.6], [3.6, 3.6], [0.0, 4.4]],
        0.35,
        [1.8, 0.0],
        0.4,
    );
}

/// Down each flank: a machinery bay in the lower tier between plates, a slot along the
/// waist, and a course of plates swept back along the upper tier.
fn flanks(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        bay(
            b,
            &Frame::new(v3(-14.5, 11.45, DECK + 0.9), Vec3::X, Vec3::Y),
            16.0,
            0.9,
        );
        dark_plate(b);
        Course {
            count: if fine { 4 } else { 2 },
            step: 4.4,
            len: 5.2,
            half: 0.9,
            tip: -0.4,
            thick: 0.3,
            tail: 1.6,
        }
        .lay(
            b,
            &Frame::new(v3(4.0, 10.45, WAIST + 1.6), -Vec3::X, Vec3::Y),
        );
        if fine {
            inlay(
                b,
                &[
                    v3(-16.0, 11.42, WAIST - 0.6),
                    v3(6.0, 11.42, WAIST - 0.6),
                    v3(6.0, 11.42, WAIST - 0.25),
                    v3(-16.0, 11.42, WAIST - 0.25),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        }
    });
}

/// Swept plates off the housing's back, fanned either side of the middle.
fn tail(b: &mut MeshBuilder) {
    for (y, yaw) in [(0.0f32, 0.0f32), (4.6, 0.3), (-4.6, -0.3)] {
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(-24.0, y, WAIST - 0.4),
                v3(-yaw.cos(), yaw.sin(), -0.22),
                Vec3::Z,
            ),
            &swept(6.5, 1.8, 0.0, 0.4),
            0.35,
        );
    }
}
