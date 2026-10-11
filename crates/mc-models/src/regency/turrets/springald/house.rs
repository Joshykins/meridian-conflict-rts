//! The Springald's turret, turning on a toothed graphite race: a long housing in two
//! tiers, a seam-dark band between them, feathered glacis plates lapped down its front
//! under the gun, plated cheeks either side of the trunnion (a short breech: it lobs at up
//! to 75 degrees, so nothing long hangs behind the trunnion), radiator banks on its back
//! flanks whose lids stand up with the heat after the shot (`charge_gear::VENT`),
//! machinery bays and lockers down its sides, an optic head on its front corner and swept
//! tail plates off its back. The plant that feeds the gun stands on its back (`back`).

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::{HEAT_STAGE, VENT};
use crate::material::TEAM;
use crate::pattern;

use super::super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::super::machine::{armour, hoop, teeth, Course, Frame};
use super::super::super::parts::{bay, inlay, locker, plan_loft, ridged};
use super::super::sunspear::coil_light;
use super::super::team_patch;
use super::{CHEEK, DECK, LINE, RACE, ROOF};

/// The housing's tiers in plan, half their outlines from the front round to the back.
const LOWER: [[f32; 2]; 5] = [
    [11.5, 6.4],
    [5.5, 9.8],
    [-15.0, 9.8],
    [-21.5, 7.0],
    [-23.5, 0.0],
];
const UPPER: [[f32; 2]; 5] = [
    [10.5, 5.6],
    [5.0, 9.0],
    [-14.6, 9.0],
    [-20.6, 6.4],
    [-22.6, 0.0],
];
/// The lower tier's top, and the band between the tiers.
const WAIST: f32 = DECK + 3.2;
/// How thick the cheeks are.
const CHEEK_THICK: f32 = 1.8;

pub(super) fn draw(b: &mut MeshBuilder) {
    race(b);
    dark_plate(b);
    plan_loft(
        b,
        &LOWER,
        &[
            (DECK + 0.3, 0.97),
            (DECK + 0.9, 1.0),
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
        &[(WAIST + 0.5, 1.0), (ROOF - 0.6, 1.0), (ROOF, 0.93)],
        false,
    );
    team_patch(b, -11.6, -7.0, 3.0, ROOF + 0.01);
    cheeks(b);
    radiators(b);
    if !b.mid() {
        return;
    }
    glacis(b);
    flanks(b);
    tail(b);
    if b.fine() {
        optics(b);
    }
}

/// The race the turret turns on: a seam-dark skirt, a graphite ring with teeth round it.
fn race(b: &mut MeshBuilder) {
    let segs = b.sides(32);
    seam(b);
    b.prism(
        v3(-4.0, 0.0, DECK - 0.1),
        b.sides(16),
        RACE - 1.0,
        RACE - 1.4,
        0.6,
    );
    metal(b);
    hoop(b, v3(0.0, 0.0, DECK + 0.25), RACE - 0.8, 1.2, 0.5, segs);
    if b.fine() {
        teeth(
            b,
            v3(0.0, 0.0, DECK + 0.1),
            RACE - 0.2,
            40,
            Vec3::new(0.45, 0.6, 0.35),
        );
    }
}

/// The cheeks: a plated wall either side of the gun from the housing's middle out past
/// the trunnion, two plates lapped down its outside swept back past it into spikes, a slot
/// let into its face and the owner's colour along its top.
fn cheeks(b: &mut MeshBuilder) {
    let (x0, x1) = (-6.5, 6.5);
    let (z0, z1) = (ROOF - 0.3, LINE.pivot.z + 4.5);
    let h = z1 - z0;
    let fine = b.fine();
    b.mirror_y(|b| {
        let (y0, y1) = (CHEEK, CHEEK + CHEEK_THICK);
        dark_plate(b);
        b.with_facets(|b| {
            b.loft(
                &[
                    vec![
                        v3(x0, y0, z0),
                        v3(x1, y0, z0),
                        v3(x1 - h * 0.35, y0, z1),
                        v3(x0 + h * 0.15, y0, z1),
                    ],
                    vec![
                        v3(x0, y1, z0),
                        v3(x1, y1, z0),
                        v3(x1 - h * 0.35, y1, z1),
                        v3(x0 + h * 0.15, y1, z1),
                    ],
                ],
                true,
                true,
            )
        });
        b.paint(TEAM);
        b.face(&[
            v3(x0 + h * 0.15 + 0.3, y0 + 0.2, z1 + 0.01),
            v3(x1 - h * 0.35 - 0.3, y0 + 0.2, z1 + 0.01),
            v3(x1 - h * 0.35 - 0.3, y1 - 0.2, z1 + 0.01),
            v3(x0 + h * 0.15 + 0.3, y1 - 0.2, z1 + 0.01),
        ]);
        if !b.mid() {
            return;
        }
        dark_plate(b);
        armour(
            b,
            &Frame::new(v3(x1 + 0.4, y1, z0 + h * 0.35), -Vec3::X, Vec3::Y),
            &[
                [0.0, -h * 0.32],
                [0.0, h * 0.28],
                [(x1 - x0) * 0.75, h * 0.3],
                [(x1 - x0) + h * 0.5, h * 0.02],
                [(x1 - x0) * 0.75, -h * 0.32],
            ],
            0.45,
        );
        if fine {
            ridged(
                b,
                &Frame::new(v3(x1 - 1.0, y1 + 0.45, z0 + h * 0.74), -Vec3::X, Vec3::Y),
                &[
                    [0.0, -h * 0.18],
                    [0.0, h * 0.2],
                    [(x1 - x0) * 0.62, h * 0.18],
                    [(x1 - x0) * 0.9 + h * 0.3, h * 0.02],
                    [(x1 - x0) * 0.62, -h * 0.18],
                ],
                0.3,
                [(x1 - x0) * 0.35, 0.0],
                0.22,
            );
            inlay(
                b,
                &[
                    v3(x0 + 1.4, y1 + 0.47, z0 + h * 0.46),
                    v3(x1 - 3.4, y1 + 0.47, z0 + h * 0.46),
                    v3(x1 - 3.4, y1 + 0.47, z0 + h * 0.54),
                    v3(x0 + 1.4, y1 + 0.47, z0 + h * 0.54),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        }
    });
}

/// A radiator bank hung on either flank of the upper tier, over the lower tier's ledge: a
/// plated box with fins down its outside, a fusion-lit floor under bars (`HEAT_STAGE`:
/// dark until the shot) and lids that stand up off it with the heat after the shot
/// (`VENT`).
fn radiators(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1, y0, y1, z0, z1) = (-19.5, -9.0, 8.6, 10.6, WAIST + 0.4, ROOF + 1.6);
    b.mirror_y(|b| {
        dark_plate(b);
        b.with_facets(|b| b.block(v3(x0, y0, z0), v3(x1, y1, z1 - 0.6)));
        coil_light(b, HEAT_STAGE);
        b.face(&[
            v3(x0 + 0.4, y0 + 0.4, z1 - 0.45),
            v3(x1 - 0.4, y0 + 0.4, z1 - 0.45),
            v3(x1 - 0.4, y1 - 0.4, z1 - 0.45),
            v3(x0 + 0.4, y1 - 0.4, z1 - 0.45),
        ]);
        if fine {
            metal(b);
            for k in 0..11 {
                let x = x0 + 0.8 + (x1 - x0 - 1.6) * k as f32 / 10.0;
                b.block(
                    v3(x - 0.12, y0 + 0.4, z1 - 0.42),
                    v3(x + 0.12, y1 - 0.4, z1 - 0.15),
                );
            }
            // Fins down its outside.
            dark_plate(b);
            for k in 0..7 {
                let x = x0 + 1.2 + (x1 - x0 - 2.4) * k as f32 / 6.0;
                b.block(
                    v3(x - 0.18, y1, z0 + 0.6),
                    v3(x + 0.18, y1 + 0.55, z1 - 1.0),
                );
            }
        }
        b.with_charge_gear(VENT, |b| {
            let w = (x1 - x0) / 4.0;
            for k in 0..4 {
                let a = x0 + w * k as f32;
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(v3(a + 0.15, (y0 + y1) * 0.5, z1), Vec3::X, Vec3::Z),
                    &[
                        [0.0, -(y1 - y0) * 0.5],
                        [0.0, (y1 - y0) * 0.5],
                        [w - 0.6, (y1 - y0) * 0.5],
                        [w - 0.3, 0.0],
                        [w - 0.6, -(y1 - y0) * 0.5],
                    ],
                    0.35,
                );
            }
        });
    });
}

/// Feathered plates lapped down the housing's front under the gun, either side of the
/// middle, and a ridged plate over the nose.
fn glacis(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count: if fine { 3 } else { 2 },
            step: 1.9,
            len: 2.6,
            half: 1.5,
            tip: 0.4,
            thick: 0.3,
            tail: 0.8,
        }
        .lay(
            b,
            &Frame::new(
                v3(6.4, 3.4, ROOF + 0.02),
                v3(1.0, 0.25, -0.45),
                v3(0.4, 0.0, 1.0),
            ),
        );
    });
    dark_plate(b);
    ridged(
        b,
        &Frame::new(
            v3(7.0, 0.0, ROOF - 0.1),
            v3(1.0, 0.0, -0.55),
            v3(0.55, 0.0, 1.0),
        ),
        &[[0.0, -1.6], [4.0, -1.4], [5.4, 0.0], [4.0, 1.4], [0.0, 1.6]],
        0.3,
        [2.0, 0.0],
        0.35,
    );
}

/// Down each flank: two bays where the plates part over the conduits, cables along them,
/// slots let into the waist and lockers on the housing's quarters.
fn flanks(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        for x in [2.6f32, -8.4] {
            let f = Frame::new(v3(x, 9.82, DECK + 1.7), -Vec3::X, Vec3::Y);
            bay(b, &f, 5.2, 0.5);
        }
        if fine {
            metal(b);
            cable(
                b,
                &[
                    v3(3.0, 10.1, DECK + 2.0),
                    v3(-2.8, 10.2, DECK + 2.0),
                    v3(-8.0, 10.1, DECK + 1.8),
                    v3(-14.4, 10.05, DECK + 1.4),
                ],
                0.16,
            );
            inlay(
                b,
                &[
                    v3(-15.2, 9.82, WAIST - 1.1),
                    v3(-19.8, 9.82, WAIST - 1.1),
                    v3(-19.8, 9.82, WAIST - 0.75),
                    v3(-15.2, 9.82, WAIST - 0.75),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        }
        locker(
            b,
            &Frame::new(v3(-1.0, 9.0, WAIST + 1.4), -Vec3::X, Vec3::Y),
            Vec3::new(3.2, 1.8, 0.6),
        );
    });
}

/// Swept plates off the housing's back into spikes, either side of the middle.
fn tail(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(-21.6, 2.4, ROOF - 0.4),
                v3(-1.0, 0.3, -0.35),
                v3(0.0, 0.25, 1.0),
            ),
            &[[0.0, -1.2], [0.0, 1.3], [2.6, 1.2], [4.6, 0.3], [2.6, -1.0]],
            0.32,
        );
        if fine {
            armour(
                b,
                &Frame::new(
                    v3(-21.0, 6.0, WAIST - 0.4),
                    v3(-0.65, 0.6, -0.2),
                    v3(0.25, 0.3, 1.0),
                ),
                &[[0.0, -1.0], [0.0, 1.1], [2.2, 1.0], [3.8, 0.2], [2.2, -0.8]],
                0.3,
            );
        }
    });
}

/// The optic head on the housing's front corner: a faceted block, red optics let into its
/// face, a smaller pod beside it.
fn optics(b: &mut MeshBuilder) {
    let at = v3(8.4, -6.0, ROOF - 0.4);
    dark_plate(b);
    ridged(
        b,
        &Frame::new(at, Vec3::X, Vec3::Z),
        &[
            [-2.4, -0.9],
            [0.9, -1.0],
            [1.5, 0.0],
            [0.9, 1.0],
            [-2.4, 0.9],
        ],
        1.0,
        [-0.6, 0.0],
        0.35,
    );
    for (dy, dz) in [(-0.45f32, 0.32f32), (0.45, 0.32), (0.0, 0.72)] {
        inlay(
            b,
            &[
                at + v3(1.38, dy - 0.2, dz - 0.14),
                at + v3(1.38, dy + 0.2, dz - 0.14),
                at + v3(1.38, dy + 0.2, dz + 0.14),
                at + v3(1.38, dy - 0.2, dz + 0.14),
            ],
            Vec3::X,
            pattern::EMBER_SLOT,
        );
    }
    dark_plate(b);
    let pod = at + v3(-1.0, -1.7, 0.0);
    b.chamfered_box(pod + v3(0.0, 0.0, 0.45), v3(1.5, 0.9, 0.9), 0.28);
    inlay(
        b,
        &[
            pod + v3(0.77, -0.28, 0.25),
            pod + v3(0.77, 0.28, 0.25),
            pod + v3(0.77, 0.28, 0.65),
            pod + v3(0.77, -0.28, 0.65),
        ],
        Vec3::X,
        pattern::EMBER_SLOT,
    );
}
