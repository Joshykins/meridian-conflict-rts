//! The commander's body: the pelvis on the hips, then (on the turret) a waist of bare
//! bronze ribs and rams, a broad chest of lapped plates, the back's plates and two blades
//! swept back behind the head, stacked pauldrons over great shoulder drums, the upper arms
//! and the head.

use glam::{Vec2, Vec3};

use crate::models::builder::{MeshBuilder, Section};
use crate::models::material::*;

use super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::plating::{joint, piston, plate, side_plate};
use super::{ELBOW, NECK, WAIST};

/// The pelvis (hull): a dark block between the hip drums, a pointed plate down the front
/// and plates swept back and down behind, and the lower half of the waist ring.
pub(super) fn pelvis(b: &mut MeshBuilder) {
    seam(b);
    b.chamfered_box(v3(-0.4, 0.0, 11.3), v3(2.4, 3.2, 1.6), 0.5);
    metal(b);
    let sides = b.sides(12);
    b.prism(v3(0.0, 0.0, 11.9), sides, 1.35, 1.3, WAIST - 11.9);
    dark_plate(b);
    plate(
        b,
        &[
            v3(0.75, -1.0, 12.0),
            v3(1.15, -0.6, 10.3),
            v3(1.45, 0.0, 9.6),
            v3(1.15, 0.6, 10.3),
            v3(0.75, 1.0, 12.0),
        ],
        Vec3::X * 0.3,
    );
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        side_plate(
            b,
            &[
                [-1.1, 12.1],
                [-1.5, 10.5],
                [-2.7, 9.5],
                [-2.3, 10.9],
                [-2.0, 12.1],
            ],
            0.25,
            1.55,
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

/// The waist: the upper waist ring, a bronze spine and rib hoops round it, and a ram
/// either side from the ring to the chest.
fn waist(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(12);
    b.prism(v3(0.0, 0.0, WAIST), sides, 1.3, 1.2, 0.35);
    b.cylinder_between(
        v3(-0.7, 0.0, WAIST),
        v3(-0.9, 0.0, 14.6),
        0.55,
        0.5,
        b.sides(8),
    );
    if !b.fine() {
        b.chamfered_box(v3(0.0, 0.0, 13.4), v3(1.9, 2.4, 1.9), 0.4);
    } else {
        seam(b);
        b.chamfered_box(v3(-0.2, 0.0, 13.4), v3(1.3, 1.6, 1.9), 0.3);
        metal(b);
        for z in [12.95f32, 13.45, 13.95] {
            b.mirror_y(|b| {
                cable(
                    b,
                    &[
                        v3(-0.7, 0.3, z),
                        v3(-0.3, 1.2, z),
                        v3(0.5, 1.05, z - 0.05),
                        v3(0.95, 0.0, z - 0.1),
                    ],
                    0.13,
                );
            });
        }
    }
    b.mirror_y(|b| piston(b, v3(-0.4, 1.35, WAIST + 0.1), v3(-0.3, 1.75, 14.5), 0.18));
}

/// The chest: a dark core with lapped plates on it, the gaps between them showing it.
fn chest(b: &mut MeshBuilder) {
    seam(b);
    b.extrude_y_chamfered(
        &[
            [-2.0, 14.2],
            [1.2, 14.2],
            [2.3, 15.2],
            [2.5, 16.9],
            [1.7, 18.4],
            [-1.2, 18.8],
            [-2.4, 17.8],
            [-2.5, 15.3],
        ],
        2.5,
        0.6,
    );
    dark_plate(b);
    b.mirror_y(|b| {
        // The lower breastplate, its outer edge swept back round the flank.
        plate(
            b,
            &[
                v3(2.35, 0.12, 15.1),
                v3(2.5, 0.12, 16.75),
                v3(2.45, 2.2, 16.55),
                v3(1.9, 2.75, 15.4),
                v3(1.35, 2.45, 14.4),
            ],
            Vec3::X * 0.18,
        );
        // The upper breastplate, lapped over the lower and swept up and back toward the
        // shoulder into a point.
        let up = Vec3::new(0.88, 0.0, 0.47) * 0.2;
        plate(
            b,
            &[
                v3(2.62, 0.12, 16.6),
                v3(2.6, 2.3, 16.4),
                v3(2.0, 3.1, 17.4),
                v3(0.9, 3.3, 18.9),
                v3(1.85, 0.12, 18.5),
            ],
            up,
        );
        // The flank: bronze ribs along the side between two plates, each plate's tail
        // swept back past the chest.
        metal(b);
        for z in [15.55f32, 16.15] {
            b.cylinder_between(v3(-1.9, 2.55, z), v3(1.9, 2.55, z), 0.2, 0.2, b.sides(6));
        }
        dark_plate(b);
        side_plate(
            b,
            &[
                [-1.8, 14.4],
                [1.4, 14.4],
                [2.0, 15.2],
                [-0.6, 15.3],
                [-2.9, 14.6],
            ],
            2.45,
            2.75,
        );
        side_plate(
            b,
            &[
                [-1.2, 16.5],
                [1.9, 16.5],
                [1.7, 17.3],
                [-1.0, 17.7],
                [-3.2, 17.0],
            ],
            2.45,
            2.75,
        );
        // The collar over the top of the chest, swept back.
        plate(
            b,
            &[
                v3(1.35, 0.55, 18.62),
                v3(1.5, 2.7, 18.45),
                v3(-0.6, 3.0, 18.85),
                v3(-2.5, 2.3, 18.95),
                v3(-1.3, 0.55, 19.0),
            ],
            Vec3::Z * 0.2,
        );
    });
    if b.fine() {
        // Intake slats under the breastplates, over bronze.
        b.mirror_y(|b| {
            metal(b);
            b.block(v3(1.05, 0.5, 14.25), v3(1.55, 2.2, 14.75));
            dark_plate(b);
            for k in 0..4 {
                let y = 0.7 + 0.42 * k as f32;
                b.block(v3(1.3, y - 0.07, 14.2), v3(1.75, y + 0.07, 14.85));
            }
        });
        // A red line down the seam between the breastplates.
        b.paint(GLOW_LASER);
        b.beam(
            v3(2.6, 0.0, 15.2),
            v3(2.55, 0.0, 16.6),
            Vec2::new(0.07, 0.06),
            Vec2::new(0.07, 0.06),
        );
    }
}

/// The back: three plates lapped down it over bronze ribs, and a blade either side swept
/// back and up behind the head, the outline's points seen from above.
fn back(b: &mut MeshBuilder) {
    if b.fine() {
        metal(b);
        for z in [15.7f32, 17.0] {
            b.cylinder_between(v3(-2.6, -1.9, z), v3(-2.6, 1.9, z), 0.16, 0.16, 6);
        }
    }
    dark_plate(b);
    b.mirror_y(|b| {
        let plates = if b.fine() { 3 } else { 1 };
        for (i, top) in [18.7f32, 17.4, 16.1].into_iter().enumerate().take(plates) {
            let out = 0.08 * i as f32;
            plate(
                b,
                &[
                    v3(-2.2 - out, 0.2, top),
                    v3(-2.2 - out, 2.3, top - 0.1),
                    v3(-3.1 - out, 2.7, top - 1.3),
                    v3(-2.75 - out, 0.2, top - 1.45),
                ],
                Vec3::X * -0.2,
            );
        }
        // The blade: its root on the collar, its tip the point furthest back.
        plate(
            b,
            &[
                v3(-0.6, 1.2, 18.9),
                v3(-0.2, 1.4, 19.5),
                v3(-3.9, 2.4, 21.2),
                v3(-2.4, 1.7, 18.95),
            ],
            Vec3::Y * 0.22,
        );
        // A bronze tube up the back inside the blade, banded, capped dark: the feed the
        // cannon arm draws on, seen from above between the blades.
        let (root, top) = (v3(-2.2, 0.75, 16.6), v3(-3.1, 0.95, 20.1));
        metal(b);
        b.cylinder_between(root, top, 0.42, 0.38, b.sides(10));
        seam(b);
        let up = (top - root).normalize();
        b.cylinder_between(top, top + up * 0.3, 0.46, 0.3, b.sides(10));
        if b.fine() {
            for t in [0.3f32, 0.6] {
                let at = root.lerp(top, t);
                b.cylinder_between(at - up * 0.08, at + up * 0.08, 0.47, 0.47, 10);
            }
        }
        if b.fine() {
            // The ram that holds it up.
            piston(b, v3(-2.3, 1.3, 17.6), v3(-2.6, 1.8, 19.9), 0.12);
        }
    });
}

/// The pauldron's centre of curvature (y, z) and its three plates, top first: radius, the
/// arc they span (degrees, from the inside over the top to the outside), and how far
/// forward each starts and how far back its trailing point reaches.
const PAULDRON: Vec2 = Vec2::new(4.5, 17.6);
const PAULDRON_PLATES: [(f32, f32, f32, f32, f32); 3] = [
    (2.15, 112.0, 0.0, 1.5, -3.0),
    (2.0, 22.0, -42.0, 1.25, -2.4),
    (1.85, -18.0, -78.0, 1.0, -1.9),
];

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

/// The left shoulder: a great bronze drum, three plates stacked over it like shingles,
/// each drawn out behind into a swept point, the team colour along the top, and the
/// upper arm hung down to the elbow.
fn shoulder(b: &mut MeshBuilder) {
    joint(b, v3(-0.2, 4.2, 17.4), Vec3::Y * 0.75, 0.95);
    dark_plate(b);
    let n = if b.fine() { 5 } else { 3 };
    for (i, &(r, from, to, front, tip)) in PAULDRON_PLATES.iter().enumerate() {
        // The trailing point: the plate narrows and rises as it sweeps back.
        let mid = (from + to) * 0.5;
        let tail_centre = PAULDRON + Vec2::new(0.25, 0.35 + 0.1 * i as f32);
        b.loft(
            &[
                arc_ring(front, PAULDRON, r, from, to, 0.24, n),
                arc_ring(-0.9, PAULDRON, r, from, to, 0.24, n),
                arc_ring(tip, tail_centre, r * 1.02, mid + 12.0, mid - 12.0, 0.1, n),
            ],
            true,
            true,
        );
    }
    if b.fine() {
        // Struts under the plates, from the drum out to each plate's inside.
        metal(b);
        for &(r, from, to, front, _) in &PAULDRON_PLATES {
            let a = ((from + to) * 0.5).to_radians();
            let out = v3(
                front - 1.0,
                PAULDRON.x + a.cos() * (r - 0.3),
                PAULDRON.y + a.sin() * (r - 0.3),
            );
            b.cylinder_between(v3(-0.2, 4.4, 17.4), out, 0.12, 0.1, 6);
        }
    }
    // The team colour on the top plate's crown.
    b.paint(TEAM);
    let top = PAULDRON.y + PAULDRON_PLATES[0].0 + 0.03;
    plate(
        b,
        &[
            v3(1.0, 4.0, top - 0.1),
            v3(1.0, 5.0, top - 0.1),
            v3(-0.9, 5.0, top - 0.08),
            v3(-0.9, 4.0, top - 0.08),
        ],
        Vec3::Z * 0.08,
    );
    // The upper arm: a bronze beam from the drum to the elbow, a plate down its outside
    // swept back at the elbow, a ram down its front.
    let shoulder = v3(-0.25, 4.5, 17.2);
    metal(b);
    b.beam(
        shoulder,
        ELBOW + Vec3::Z * 0.3,
        Vec2::new(0.9, 0.9),
        Vec2::new(0.75, 0.75),
    );
    dark_plate(b);
    side_plate(
        b,
        &[
            [0.45, 16.8],
            [0.35, 14.6],
            [-0.9, 13.5],
            [-1.9, 13.2],
            [-1.0, 14.9],
            [-0.9, 16.9],
        ],
        5.15,
        5.4,
    );
    piston(b, v3(0.35, 4.6, 16.6), v3(0.25, 4.7, 14.4), 0.15);
    joint(b, ELBOW, Vec3::Y * 0.6, 0.6);
}

/// The head: narrow and tall, a flat face with the one red optic in it, cheek plates and
/// two horn plates swept back and up from the temples. It turns about the neck idle.
fn head(b: &mut MeshBuilder) {
    metal(b);
    b.cylinder_between(
        NECK - v3(0.3, 0.0, 0.4),
        NECK + Vec3::Z * 0.3,
        0.45,
        0.4,
        b.sides(8),
    );
    b.with_head(NECK, |b| {
        dark_plate(b);
        let plan = [
            [1.95, 0.32],
            [1.7, 0.6],
            [0.4, 0.66],
            [-0.2, 0.38],
            [-0.2, -0.38],
            [0.4, -0.66],
            [1.7, -0.6],
            [1.95, -0.32],
        ];
        b.loft_z(
            &plan,
            &[
                Section::new(18.9, 0.85),
                Section::new(20.4, 1.0),
                Section::new(21.7, 0.9).shifted(-0.1, 0.0),
                Section::scaled(22.3, 0.6, 0.55).shifted(-0.35, 0.0),
            ],
        );
        // The optic in a dark socket.
        seam(b);
        let eye = v3(1.92, 0.0, 20.7);
        b.cylinder_between(
            eye - Vec3::X * 0.2,
            eye + Vec3::X * 0.08,
            0.36,
            0.36,
            b.sides(10),
        );
        b.paint(GLOW_LASER);
        b.cylinder_between(eye, eye + Vec3::X * 0.14, 0.26, 0.22, b.sides(10));
        dark_plate(b);
        b.mirror_y(|b| {
            // Horn plate: from the temple, swept back and up past the crown.
            plate(
                b,
                &[
                    v3(1.5, 0.62, 20.95),
                    v3(0.65, 0.68, 22.35),
                    v3(-1.5, 1.0, 23.3),
                    v3(-0.45, 0.72, 21.35),
                ],
                Vec3::Y * 0.14,
            );
            // Cheek plate, swept back under the horn.
            if !b.fine() {
                return;
            }
            plate(
                b,
                &[
                    v3(1.75, 0.64, 19.3),
                    v3(1.85, 0.66, 20.25),
                    v3(0.3, 0.72, 20.65),
                    v3(-0.75, 0.82, 19.8),
                ],
                Vec3::Y * 0.12,
            );
        });
    });
}
