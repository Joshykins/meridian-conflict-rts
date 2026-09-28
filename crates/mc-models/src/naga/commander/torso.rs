//! The commander's body: a heavy pelvis over the hip drums, then (on the turret) a thick
//! waist of bronze ribs and rams under lapped belly plates, a deep broad chest of lapped
//! plates, the back's plates, feed stacks and two blades swept back behind the head,
//! pauldrons stacked over great shoulder blocks, the upper arms and the head.
//!
//! Everything touches what holds it: the hip drums sink into the pelvis, the shoulder
//! blocks into the chest's flanks, the pauldrons sit close round the blocks and the upper
//! arms hang from them.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;

use super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::machine::hoop_on;
use super::super::plating::{joint, piston, plate, side_plate};
use super::{ELBOW, NECK, SHOULDER, WAIST};

/// The pelvis (hull): a broad dark block the hip drums sink into, a pointed plate down
/// the front, a plate swept back over each hip, plates swept back and down behind, and
/// the bronze turntable the waist turns on.
pub(super) fn pelvis(b: &mut MeshBuilder) {
    seam(b);
    b.chamfered_box(v3(-0.3, 0.0, 10.5), v3(3.2, 5.1, 2.2), 0.6);
    metal(b);
    let sides = b.sides(12);
    b.prism(v3(0.0, 0.0, 11.5), sides, 2.0, 1.95, WAIST - 11.5);
    dark_plate(b);
    // The front plate, pointed down between the thighs.
    plate(
        b,
        &[
            v3(1.25, -1.5, 11.7),
            v3(1.55, -1.0, 9.7),
            v3(1.8, 0.0, 8.8),
            v3(1.55, 1.0, 9.7),
            v3(1.25, 1.5, 11.7),
        ],
        Vec3::X * 0.35,
    );
    b.mirror_y(|b| {
        // Over the hip: a plate from the front of the drum swept back past it.
        plate(
            b,
            &[
                v3(1.5, 2.3, 11.75),
                v3(1.2, 4.3, 11.55),
                v3(-1.3, 4.4, 11.7),
                v3(-3.3, 3.2, 12.3),
                v3(-1.6, 2.3, 11.85),
            ],
            Vec3::Z * 0.25,
        );
        if !b.fine() {
            return;
        }
        // Behind: a plate swept back and down from the seat.
        side_plate(
            b,
            &[
                [-1.5, 11.6],
                [-1.9, 10.0],
                [-3.4, 9.0],
                [-2.9, 10.6],
                [-2.4, 11.7],
            ],
            0.3,
            2.1,
        );
    });
}

/// Everything on the turret but the forearms.
pub(super) fn torso(b: &mut MeshBuilder) {
    waist(b);
    chest(b);
    back(b);
    b.mirror_y(shoulder);
    head(b);
}

/// The waist: the upper turntable ring, a thick dark core with bronze ribs round it and
/// two belly plates lapped over its front, and a heavy ram either side from the ring up
/// into the chest.
fn waist(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(12);
    b.prism(v3(0.0, 0.0, WAIST), sides, 1.95, 1.85, 0.4);
    seam(b);
    b.chamfered_box(v3(-0.3, 0.0, 12.95), v3(2.8, 3.4, 1.9), 0.5);
    if b.fine() {
        metal(b);
        for z in [12.5f32, 13.05, 13.6] {
            b.mirror_y(|b| {
                cable(
                    b,
                    &[
                        v3(-1.6, 0.3, z),
                        v3(-1.2, 1.75, z),
                        v3(0.4, 1.8, z - 0.05),
                        v3(1.15, 0.4, z - 0.1),
                    ],
                    0.16,
                );
            });
        }
    }
    dark_plate(b);
    // The belly plates, the upper lapped over the lower, both pointed at the bottom.
    for (lo, hi, out) in [(12.0f32, 13.1f32, 1.1f32), (12.7, 13.8, 1.25)] {
        plate(
            b,
            &[
                v3(out, -1.3, hi),
                v3(out, -1.05, lo + 0.35),
                v3(out + 0.1, 0.0, lo),
                v3(out, 1.05, lo + 0.35),
                v3(out, 1.3, hi),
            ],
            Vec3::X * 0.3,
        );
    }
    if b.fine() {
        b.mirror_y(|b| piston(b, v3(-0.5, 1.7, WAIST + 0.15), v3(-0.3, 2.2, 14.2), 0.26));
    }
}

/// The chest: a deep dark core with lapped plates on it, the gaps between them showing it.
fn chest(b: &mut MeshBuilder) {
    seam(b);
    b.extrude_y_chamfered(
        &[
            [-2.6, 13.4],
            [1.5, 13.4],
            [2.8, 14.6],
            [3.1, 16.4],
            [2.4, 18.0],
            [-1.0, 18.6],
            [-2.9, 17.8],
            [-3.1, 14.6],
        ],
        3.1,
        0.7,
    );
    dark_plate(b);
    b.mirror_y(|b| {
        // The lower breastplate, its outer edge swept back round the flank.
        plate(
            b,
            &[
                v3(2.95, 0.12, 14.5),
                v3(3.2, 0.12, 16.2),
                v3(3.1, 2.6, 16.0),
                v3(2.4, 3.2, 14.7),
                v3(1.7, 2.9, 13.6),
            ],
            Vec3::X * 0.22,
        );
        // The upper breastplate, lapped over the lower and swept up and back toward the
        // shoulder into a point.
        plate(
            b,
            &[
                v3(3.3, 0.12, 16.0),
                v3(3.25, 2.7, 15.8),
                v3(2.6, 3.5, 17.0),
                v3(1.2, 3.7, 18.7),
                v3(2.4, 0.12, 18.1),
            ],
            Vec3::new(0.88, 0.0, 0.47) * 0.24,
        );
        // The flank: bronze ribs along the side between two plates, each plate's tail
        // swept back past the chest.
        metal(b);
        for z in [14.95f32, 15.6] {
            b.cylinder_between(v3(-2.4, 3.0, z), v3(2.4, 3.0, z), 0.24, 0.24, b.sides(6));
        }
        dark_plate(b);
        side_plate(
            b,
            &[
                [-2.4, 13.7],
                [1.8, 13.7],
                [2.5, 14.6],
                [-0.8, 14.7],
                [-3.6, 14.0],
            ],
            2.9,
            3.25,
        );
        side_plate(
            b,
            &[
                [-1.8, 15.9],
                [2.5, 15.9],
                [2.3, 16.7],
                [-1.4, 17.1],
                [-3.9, 16.4],
            ],
            2.9,
            3.25,
        );
        // The collar over the top of the chest, swept back.
        plate(
            b,
            &[
                v3(1.9, 0.6, 18.25),
                v3(2.0, 3.1, 18.05),
                v3(-0.6, 3.4, 18.5),
                v3(-3.1, 2.6, 18.6),
                v3(-1.5, 0.6, 18.75),
            ],
            Vec3::Z * 0.24,
        );
    });
    if b.fine() {
        // Intake slats under the breastplates, over bronze.
        b.mirror_y(|b| {
            metal(b);
            b.block(v3(1.5, 0.5, 13.45), v3(2.1, 2.5, 14.0));
            dark_plate(b);
            for k in 0..4 {
                let y = 0.75 + 0.48 * k as f32;
                b.block(v3(1.8, y - 0.08, 13.4), v3(2.35, y + 0.08, 14.15));
            }
        });
        // A red line down the seam between the breastplates.
        b.paint(GLOW_LASER);
        b.beam(
            v3(3.2, 0.0, 14.6),
            v3(3.15, 0.0, 16.0),
            Vec2::new(0.08, 0.07),
            Vec2::new(0.08, 0.07),
        );
    }
}

/// The back: three plates lapped down it over bronze ribs, a housing between them with
/// two banded feed stacks standing out of it, and a blade either side swept back and up
/// behind the head, the outline's points seen from above.
fn back(b: &mut MeshBuilder) {
    if b.fine() {
        metal(b);
        for z in [14.9f32, 16.3] {
            b.cylinder_between(v3(-3.1, -2.3, z), v3(-3.1, 2.3, z), 0.2, 0.2, 6);
        }
    }
    // The housing the stacks stand in, sunk into the back.
    seam(b);
    b.chamfered_box(v3(-3.1, 0.0, 16.6), v3(1.4, 2.6, 2.6), 0.35);
    dark_plate(b);
    b.mirror_y(|b| {
        let plates = if b.fine() { 3 } else { 1 };
        for (i, top) in [18.4f32, 17.0, 15.6].into_iter().enumerate().take(plates) {
            let out = 0.1 * i as f32;
            plate(
                b,
                &[
                    v3(-2.7 - out, 1.35, top),
                    v3(-2.7 - out, 2.8, top - 0.1),
                    v3(-3.7 - out, 3.2, top - 1.4),
                    v3(-3.35 - out, 1.35, top - 1.55),
                ],
                Vec3::X * -0.24,
            );
        }
        // The blade: its root on the collar, its tip the point furthest back.
        plate(
            b,
            &[
                v3(-0.9, 1.5, 18.6),
                v3(-0.5, 1.7, 19.2),
                v3(-4.4, 2.8, 21.0),
                v3(-2.9, 2.1, 18.7),
            ],
            Vec3::Y * 0.26,
        );
        // A feed stack out of the housing: a banded bronze tube, capped dark.
        let (root, top) = (v3(-3.0, 0.7, 16.8), v3(-3.8, 0.85, 20.0));
        metal(b);
        b.cylinder_between(root, top, 0.5, 0.45, b.sides(10));
        let up = (top - root).normalize();
        if b.fine() {
            seam(b);
            b.cylinder_between(top, top + up * 0.35, 0.55, 0.36, 10);
            for t in [0.45f32, 0.75] {
                let at = root.lerp(top, t);
                b.cylinder_between(at - up * 0.1, at + up * 0.1, 0.56, 0.56, 10);
            }
        }
    });
}

/// The pauldron's centre of curvature (y, z) is the shoulder's; its three plates, top
/// first: radius, the arc they span (degrees, from the inside over the top to the
/// outside), and how far forward each starts and how far back its trailing point reaches.
const PAULDRON_PLATES: [(f32, f32, f32, f32, f32); 3] = [
    (2.55, 115.0, 0.0, 1.7, -3.4),
    (2.4, 22.0, -42.0, 1.4, -2.8),
    (2.25, -16.0, -68.0, 1.15, -2.2),
];
/// The shoulder block's radius, just inside the pauldron's lowest plate.
const BLOCK_R: f32 = 1.95;

/// One pauldron plate's cross-section at `x`: the arc of `radius` from `from` to `to`
/// degrees round `centre`, `thick` deep.
fn arc_ring(
    x: f32,
    centre: Vec2,
    radius: f32,
    from: f32,
    to: f32,
    thick: f32,
    n: usize,
) -> Vec<Vec3> {
    let at = |r: f32, i: usize| {
        let a = (from + (to - from) * i as f32 / (n - 1) as f32).to_radians();
        v3(x, centre.x + a.cos() * r, centre.y + a.sin() * r)
    };
    let mut ring: Vec<Vec3> = (0..n).map(|i| at(radius, i)).collect();
    ring.extend((0..n).rev().map(|i| at(radius - thick, i)));
    ring
}

/// The left shoulder: a joint drum out of the chest's flank into a great bronze block,
/// three plates stacked close over the block like shingles, each drawn out behind into a
/// swept point, the team colour along the top, and the upper arm hung from the block
/// down to the elbow.
fn shoulder(b: &mut MeshBuilder) {
    let (x, y, z) = (SHOULDER.x, SHOULDER.y, SHOULDER.z);
    joint(b, v3(x, 3.5, z), Vec3::Y * 0.75, 1.35);
    // The block: dark, a bronze ring round its front where the pauldron leaves it bare.
    seam(b);
    let sides = b.sides(12);
    b.cylinder_between(
        v3(x - 1.2, y, z),
        v3(x + 1.2, y, z),
        BLOCK_R,
        BLOCK_R,
        sides,
    );
    metal(b);
    hoop_on(
        b,
        v3(x + 0.9, y, z),
        Vec3::X,
        BLOCK_R - 0.1,
        0.35,
        0.5,
        sides,
    );
    dark_plate(b);
    let centre = Vec2::new(y, z);
    let n = if b.fine() { 5 } else { 3 };
    for (i, &(r, from, to, front, tip)) in PAULDRON_PLATES.iter().enumerate() {
        // The trailing point: the plate narrows and rises as it sweeps back.
        let mid = (from + to) * 0.5;
        let tail_centre = centre + Vec2::new(0.3, 0.4 + 0.1 * i as f32);
        b.loft(
            &[
                arc_ring(front, centre, r, from, to, 0.28, n),
                arc_ring(-1.0, centre, r, from, to, 0.28, n),
                arc_ring(tip, tail_centre, r * 1.02, mid + 12.0, mid - 12.0, 0.12, n),
            ],
            true,
            true,
        );
    }
    // The team colour on the top plate's crown.
    b.paint(TEAM);
    let top = z + PAULDRON_PLATES[0].0 + 0.03;
    plate(
        b,
        &[
            v3(1.2, y - 0.6, top - 0.12),
            v3(1.2, y + 0.6, top - 0.12),
            v3(-1.0, y + 0.6, top - 0.1),
            v3(-1.0, y - 0.6, top - 0.1),
        ],
        Vec3::Z * 0.1,
    );
    // The upper arm: a bronze beam from the block to the elbow, a plate down its outside
    // swept back at the elbow, a ram down its front.
    let from = v3(ELBOW.x + 0.1, ELBOW.y, z - 1.0);
    metal(b);
    b.beam(
        from,
        ELBOW + Vec3::Z * 0.3,
        Vec2::new(1.3, 1.3),
        Vec2::new(1.05, 1.05),
    );
    dark_plate(b);
    let side = ELBOW.y + 0.7;
    side_plate(
        b,
        &[
            [0.75, z - 0.6],
            [0.6, ELBOW.z + 0.9],
            [-0.9, ELBOW.z - 0.4],
            [-2.1, ELBOW.z - 0.6],
            [-1.2, ELBOW.z + 1.2],
            [-1.2, z - 0.6],
        ],
        side,
        side + 0.3,
    );
    piston(
        b,
        v3(0.55, ELBOW.y - 0.1, z - 1.2),
        v3(0.45, ELBOW.y - 0.1, ELBOW.z + 0.5),
        0.2,
    );
    joint(b, ELBOW, Vec3::Y * 0.72, 0.85);
}

/// The head: narrow and tall, sunk between the pauldrons, a flat face with the one red
/// optic in it, cheek plates and two horn plates swept back and up from the temples. It
/// turns about the neck idle.
fn head(b: &mut MeshBuilder) {
    let z = NECK.z;
    metal(b);
    b.cylinder_between(
        NECK - v3(0.3, 0.0, 0.5),
        NECK + Vec3::Z * 0.3,
        0.6,
        0.5,
        b.sides(8),
    );
    b.with_head(NECK, |b| {
        dark_plate(b);
        let plan = [
            [2.0, 0.36],
            [1.75, 0.66],
            [0.4, 0.74],
            [-0.3, 0.42],
            [-0.3, -0.42],
            [0.4, -0.74],
            [1.75, -0.66],
            [2.0, -0.36],
        ];
        b.loft_z(
            &plan,
            &[
                Section::new(z + 0.05, 0.9),
                Section::new(z + 1.5, 1.05),
                Section::new(z + 2.7, 0.92).shifted(-0.1, 0.0),
                Section::scaled(z + 3.3, 0.6, 0.55).shifted(-0.35, 0.0),
            ],
        );
        // The optic in a dark socket.
        seam(b);
        let eye = v3(1.98, 0.0, z + 1.8);
        b.cylinder_between(
            eye - Vec3::X * 0.2,
            eye + Vec3::X * 0.1,
            0.4,
            0.4,
            b.sides(10),
        );
        b.paint(GLOW_LASER);
        b.cylinder_between(eye, eye + Vec3::X * 0.16, 0.3, 0.25, b.sides(10));
        dark_plate(b);
        b.mirror_y(|b| {
            // Horn plate: from the temple, swept back and up past the crown.
            plate(
                b,
                &[
                    v3(1.55, 0.68, z + 2.05),
                    v3(0.65, 0.75, z + 3.4),
                    v3(-1.6, 1.1, z + 4.3),
                    v3(-0.45, 0.8, z + 2.4),
                ],
                Vec3::Y * 0.16,
            );
            // Cheek plate, swept back under the horn.
            if !b.fine() {
                return;
            }
            plate(
                b,
                &[
                    v3(1.8, 0.72, z + 0.35),
                    v3(1.9, 0.74, z + 1.35),
                    v3(0.3, 0.8, z + 1.75),
                    v3(-0.8, 0.9, z + 0.9),
                ],
                Vec3::Y * 0.14,
            );
        });
    });
}
