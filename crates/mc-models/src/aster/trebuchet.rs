//! The Trebuchet: tech 3 mobile heavy artillery, a charged howitzer on a tracked carriage.
//!
//! The carriage is long and low with no hard corners in its outline: one run of track a
//! side under a sculpted fender that slopes down at both ends, a pointed spine between
//! them, and a ground stake on each corner that it fires into the ground to plant
//! (`stakes`, posed by `entity.wgsl` `stake_pose`). On the turntable a low sloped
//! armoured house holds the howitzer (`bolt_rifle::siege_howitzer`) in a well between
//! its trunnion shoulders, laid up at rest.

use glam::Vec3;

use super::bolt_rifle::siege_howitzer;
use super::parts::*;
use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern, rig};

const REAR: f32 = -5.9;
const FRONT: f32 = 5.1;
/// Track inner edge, outer edge (y) and height.
const INNER: f32 = 1.92;
const OUTER: f32 = 4.72;
const TRACK_H: f32 = 1.5;
/// Under the deploy hinges (`entity.wgsl`), and the turret's floor.
const DECK: f32 = 1.88;
/// Top of the turntable the house stands on.
const TABLE: f32 = DECK + 0.4;
/// The howitzer's trunnion, and the way it rests: laid up 25 degrees.
const PIVOT: Vec3 = Vec3::new(-0.45, 0.0, 3.45);
const REST: f32 = 0.4363;
/// Bore behind and ahead of the trunnion, and half the housing's height.
const BACK: f32 = 1.1;
const AHEAD: f32 = 5.4;
const BORE_R: f32 = 0.66;
/// Inner face of the shoulders on each side, clear of the howitzer's plasma cells.
const CHEEK: f32 = BORE_R * 1.8 + 0.1;

pub(super) fn artillery_heavy(b: &mut MeshBuilder, _tech: u8) {
    let (breech, muzzle) = (PIVOT - axis() * BACK, PIVOT + axis() * AHEAD);
    b.set_treads((INNER + OUTER) * 0.5, OUTER - INNER, REAR);
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_arm_pivot(PIVOT);
    b.set_recoil(breech, muzzle, 1.4);

    if b.coarse() {
        b.mirror_y(|b| {
            b.with_part(part::LOCOMOTION, |b| {
                b.paint(TREAD);
                b.cuboid_open(
                    v3((FRONT + REAR) * 0.5, (INNER + OUTER) * 0.5, TRACK_H * 0.5),
                    v3(FRONT - REAR, OUTER - INNER, TRACK_H),
                );
            });
        });
        b.with_part(part::TURRET, |b| {
            b.with_limb(rig::ARM_GUN, |b| {
                b.with_recoil(|b| siege_howitzer(b, breech, muzzle, BORE_R));
            });
            b.paint(ACCENT);
            b.prism(v3(0.0, 0.0, DECK), 4, 2.0, 1.8, 0.4);
            b.paint(PLATING);
            b.frustum_open(
                v3(-0.3, 0.0, TABLE),
                v2(4.4, 3.8),
                v2(3.4, 2.9),
                2.95 - TABLE,
                v2(-0.25, 0.0),
            );
            team_panel(b, v3(-1.95, 0.0, 2.95), v2(0.7, 1.2));
        });
        return;
    }

    carriage(b);
    ground_stakes(b);
    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_GUN, |b| {
            b.with_recoil(|b| siege_howitzer(b, breech, muzzle, BORE_R));
        });
        // The turntable: a faceted drum that yaws with the gun.
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, DECK + 0.2), b.sides(10), 1.98, 1.86, 0.08);
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, DECK + 0.28), b.sides(10), 1.86, 1.62, 0.12);
        casemate(b);
        trunnion(b);
        if b.fine() {
            antenna(b, v3(-1.25, -1.35, TABLE), 1.1, 0.14);
        }
    });
}

fn axis() -> Vec3 {
    Vec3::new(REST.cos(), 0.0, REST.sin())
}

/// Tracks, fenders and the spine between them.
fn carriage(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        track(b, REAR - 0.08, FRONT - 0.04, INNER, OUTER, TRACK_H);
        // The fender: a sculpted guard over the top run, sloping down at both ends and
        // falling away outboard, the wheels showing under it.
        let guard = |x: f32, drop: f32, pinch: f32| -> Vec<Vec3> {
            [
                [INNER - 0.05, 1.3],
                [OUTER + 0.12 - pinch, 1.2],
                [OUTER + 0.12 - pinch, 1.52],
                [OUTER - 0.45 - pinch, 1.94],
                [INNER + 0.35, 2.04],
                [INNER - 0.05, 1.92],
            ]
            .iter()
            .map(|p| v3(x, p[0], p[1] - drop))
            .collect()
        };
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.loft(
            &[
                guard(REAR + 0.05, 0.5, 1.25),
                guard(REAR + 1.2, 0.12, 0.45),
                guard(REAR + 3.0, 0.0, 0.08),
                guard(FRONT - 3.2, 0.0, 0.0),
                guard(FRONT - 1.5, 0.1, 0.35),
                guard(FRONT + 0.05, 0.62, 1.3),
            ],
            true,
            true,
        );
        // Team flash on the fender's crown, forward, the same read as the other hulls.
        b.paint(TEAM);
        let flash = v3(FRONT - 2.6, INNER + 0.95, 2.02);
        if b.fine() {
            b.plate(flash, v2(1.5, 0.9), 0.06, 0.03);
        } else {
            b.decal(flash + Vec3::Z * 0.05, v2(1.5, 0.9));
        }
        if b.fine() {
            // A dark strake along the fender's slope.
            b.paint(ACCENT);
            b.beam(
                v3(REAR + 1.3, OUTER - 0.1, 1.76),
                v3(FRONT - 1.9, OUTER - 0.1, 1.76),
                v2(0.34, 0.08),
                v2(0.34, 0.08),
            );
        }
    });
    // The spine: a pointed lozenge rising between the fenders to a sloped glacis.
    let plan = hull_plan(REAR + 0.1, FRONT - 0.1, 1.95, 1.35);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[
            Section::new(0.7, 0.9),
            Section::new(1.3, 1.0),
            Section::scaled(DECK, 0.8, 0.8).shifted(-0.3, 0.0),
        ],
    );
    b.paint(PLATING);
    // Glacis plate forward of the turntable, engine deck behind it.
    b.extrude_y(
        &[
            [1.9, DECK],
            [3.6, DECK - 0.4],
            [3.8, DECK - 0.48],
            [3.85, DECK - 0.4],
            [2.1, DECK + 0.05],
        ],
        -1.25,
        1.25,
    );
    b.extrude_y(
        &[
            [-5.1, DECK - 0.34],
            [-2.0, DECK - 0.02],
            [-1.9, DECK + 0.1],
            [-5.0, DECK - 0.2],
        ],
        -1.3,
        1.3,
    );
    // Hull race the turntable turns on.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, DECK - 0.02), b.sides(10), 2.1, 2.04, 0.22);
    if b.fine() {
        vent(b, v3(-3.5, 0.0, DECK - 0.14), v2(1.9, 0.9), 5, GLOW);
        b.paint(GLOW);
        b.plate(v3(3.5, 0.0, DECK - 0.4), v2(0.08, 1.2), 0.04, 0.02);
    }
}

/// The trunnion hub on each side of the howitzer, where the shoulders hold it.
fn trunnion(b: &mut MeshBuilder) {
    let out = CHEEK + 0.34;
    b.paint(METAL);
    b.cylinder_between(
        v3(PIVOT.x, -out - 0.1, PIVOT.z),
        v3(PIVOT.x, out + 0.1, PIVOT.z),
        0.3,
        0.3,
        b.sides(10),
    );
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.cylinder_between(
            v3(PIVOT.x, out + 0.1, PIVOT.z),
            v3(PIVOT.x, out + 0.18, PIVOT.z),
            0.4,
            0.34,
            b.sides(10),
        );
    });
}

/// A low armoured house with sloped faces, trunnion shoulders rising out of its roof and
/// the howitzer's breech sunk in a well between them.
fn casemate(b: &mut MeshBuilder) {
    let plan = [
        [1.95, -0.8],
        [1.95, 0.8],
        [1.0, 1.9],
        [-1.9, 1.9],
        [-2.5, 1.35],
        [-2.5, -1.35],
        [-1.9, -1.9],
        [1.0, -1.9],
    ];
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.loft_z(
        &plan,
        &[
            Section::new(TABLE, 1.0),
            Section::new(TABLE + 0.3, 1.0),
            Section::scaled(2.95, 0.82, 0.78).shifted(-0.25, 0.0),
        ],
    );
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.extrude_y(
            &[
                [-1.45, 2.8],
                [0.55, 2.8],
                [0.1, 3.5],
                [-0.35, 3.82],
                [-0.8, 3.72],
                [-1.2, 3.3],
            ],
            CHEEK,
            CHEEK + 0.34,
        );
        if b.fine() {
            b.paint(ACCENT);
            b.block(v3(-1.6, 1.62, 2.52), v3(0.9, 1.68, 2.62));
            b.paint(GLOW);
            b.block(v3(-1.2, 1.65, 2.55), v3(0.3, 1.7, 2.59));
        }
    });
    b.paint(ACCENT);
    b.block(v3(-1.6, -0.95, 2.9), v3(0.4, 0.95, 2.96));
    team_panel(b, v3(-1.95, 0.0, 2.95), v2(0.7, 1.2));
    if b.fine() {
        vent(b, v3(1.25, 0.0, 2.72), v2(0.7, 1.0), 3, GLOW);
    }
}
