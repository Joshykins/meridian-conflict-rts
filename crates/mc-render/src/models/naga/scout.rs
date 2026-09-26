//! The Naga land scout: a low, six-legged walking machine on long sturdy legs, a sensor
//! mast curled up over its back carrying a turning radar array. A pointed sensor head with
//! two pairs of red optics under a plate brow, a bronze neck ring, three lapped back plates
//! swept back over a bronze spine, a bronze power cell either side, and a light plasmeric
//! repeater in a small turret over the neck.
//!
//! Finish (docs/STYLE.md "The Naga look"): dark plates (`dark_plate`), their seams dark with
//! red lines (`seam`), dark bronze on every joint, ram, rib and rail (`metal`). No violet:
//! it does not build.
//!
//! Rig: the body and mast are `HULL`. Each leg is two bones posed by `entity.wgsl`
//! `crawl_leg` (`MeshBuilder::set_crawl_legs`), pair by pair, the six feet in two
//! alternating tripods. The radar array turns on the mast's head (`part::SPINNER` about
//! `RADAR`). The repeater's turret turns about the unit's middle. The numbers match
//! `naga_t1_scout` in `data/factions/naga/units/land.ron`.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, rig};

use super::kit::*;
use super::plating::{piston, plate};

/// The repeater's muzzle: the unit file's weapon `muzzle`.
const MUZZLE: Vec3 = Vec3::new(1.6, 0.0, 2.3);
/// Where the turret turns: over the gap between the first two back plates.
const TURRET: Vec3 = Vec3::new(0.0, 0.0, 1.9);

/// The mast's spine, from the socket on the back up and round to its head.
const MAST: [Vec3; 5] = [
    Vec3::new(-1.5, 0.0, 1.6),
    Vec3::new(-2.1, 0.0, 2.15),
    Vec3::new(-2.35, 0.0, 2.85),
    Vec3::new(-2.2, 0.0, 3.55),
    Vec3::new(-1.85, 0.0, 4.05),
];
/// Half widths of the mast's armour at each joint, root to head.
const MAST_WIDTH: [f32; 5] = [0.56, 0.5, 0.45, 0.4, 0.36];
/// The middle of the mast's curl, for which way is "outside".
const CURL: Vec3 = Vec3::new(-1.7, 0.0, 2.8);
/// The radar array's bearing on the mast head: it turns about the vertical through here.
const RADAR: Vec3 = Vec3::new(-1.85, 0.0, 4.15);

/// Left legs, front to back: hip, knee, the foot's tip, and where in the cycle it lifts.
/// Two alternating tripods (front and rear left with the middle right), a little out of
/// step so it picks its way rather than marches. Long legs, knees held high.
const LEGS: [(Vec3, Vec3, Vec3, f32); 3] = [
    (
        Vec3::new(1.05, 0.55, 1.2),
        Vec3::new(1.95, 1.6, 2.45),
        Vec3::new(3.0, 2.8, 0.0),
        0.0,
    ),
    (
        Vec3::new(0.05, 0.7, 1.2),
        Vec3::new(0.25, 2.0, 2.6),
        Vec3::new(0.4, 3.35, 0.0),
        0.5,
    ),
    (
        Vec3::new(-0.95, 0.65, 1.25),
        Vec3::new(-1.7, 1.75, 2.5),
        Vec3::new(-2.6, 2.95, 0.0),
        0.06,
    ),
];

/// The back plates, front to back: middle x, half length, half width, base height and how
/// high its arch stands.
const TERGITES: [(f32, f32, f32, f32, f32); 3] = [
    (0.3, 0.3, 0.95, 1.55, 0.36),
    (-0.42, 0.3, 1.02, 1.6, 0.38),
    (-1.1, 0.26, 0.82, 1.64, 0.32),
];

/// A small plate's cross-section: an arch over a flat underside, fewer sides than kit's
/// `shell_ring` (a leg plate a hand wide needs no more).
fn plate_ring(b: &MeshBuilder, c: Vec3, side: Vec3, up: Vec3, w: f32, h: f32) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() {
        &[
            [1.0, -0.6],
            [-1.0, -0.6],
            [-0.7, 0.55],
            [0.0, 1.0],
            [0.7, 0.55],
        ]
    } else {
        &[[1.0, -0.6], [-1.0, -0.6], [0.0, 1.0]]
    };
    shape
        .iter()
        .map(|[s, u]| c + side * (s * w) + up * (u * h))
        .collect()
}

/// A small frame member's cross-section: a keeled five-sided tube (three-sided at mid).
fn core_ring(b: &MeshBuilder, c: Vec3, side: Vec3, up: Vec3, w: f32, h: f32) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() {
        &[
            [1.0, -0.3],
            [0.5, -1.0],
            [-0.5, -1.0],
            [-1.0, -0.3],
            [0.0, 1.0],
        ]
    } else {
        &[[1.0, -0.5], [-1.0, -0.5], [0.0, 1.0]]
    };
    shape
        .iter()
        .map(|[s, u]| c + side * (s * w) + up * (u * h))
        .collect()
}

/// A small plate along `points` (`plate_ring`).
fn plate_arch(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3) {
    let r = rings(b, points, hint, plate_ring);
    b.loft(&r, true, true);
}

/// A small frame member along `points` (`core_ring`).
fn core(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)], hint: Vec3) {
    let r = rings(b, points, hint, core_ring);
    b.loft(&r, true, true);
}

pub(super) fn scout(b: &mut MeshBuilder, _tech: u8) {
    b.set_crawl_legs(&LEGS, 8.0, 0.55, 0.75);
    b.set_turret_pivot(TURRET);
    b.set_spinner_pivot(RADAR);
    b.set_dust_line(1.0);

    if b.coarse() {
        coarse(b);
        return;
    }
    body(b);
    b.mirror_y(|b| {
        for (i, &(hip, knee, foot, _)) in LEGS.iter().enumerate() {
            b.with_pair(i, |b| leg(b, hip, knee, foot));
        }
    });
    mast(b);
    b.with_part(part::SPINNER, radar);
    b.with_part(part::TURRET, gun);
}

/// Far off: a wedge of a body, flat legs that do not walk, the mast in two bars, the radar
/// a bar across its head and the gun a spike.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.frustum(
        v3(0.3, 0.0, 0.85),
        Vec2::new(4.2, 1.7),
        Vec2::new(3.0, 1.1),
        0.95,
        Vec2::new(-0.1, 0.0),
    );
    b.paint(TEAM);
    b.face(&[
        v3(0.6, 0.0, 1.81),
        v3(-0.2, 0.35, 1.81),
        v3(-0.2, -0.35, 1.81),
    ]);
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            for &(hip, knee, foot, _) in &LEGS {
                b.face(&[hip, foot, knee]);
                b.face(&[hip, knee, foot]);
            }
        });
    });
    dark_plate(b);
    let bar = |b: &mut MeshBuilder, a: Vec3, c: Vec3, wa: f32, wc: f32| {
        let (side, up) = frame(c - a, (a + c) * 0.5 - CURL);
        let tri = |p: Vec3, w: f32| {
            vec![
                p + up * w,
                p + side * w - up * (w * 0.6),
                p - side * w - up * (w * 0.6),
            ]
        };
        b.loft(&[tri(a, wa), tri(c, wc)], true, true);
    };
    bar(b, MAST[0], MAST[2], 0.45, 0.36);
    bar(b, MAST[2], MAST[4], 0.36, 0.3);
    b.with_part(part::SPINNER, |b| {
        dark_plate(b);
        b.beam(
            RADAR + v3(0.0, -1.3, 0.3),
            RADAR + v3(0.0, 1.3, 0.3),
            Vec2::new(0.5, 0.3),
            Vec2::new(0.5, 0.3),
        );
    });
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        let root = [
            v3(-0.2, 0.25, TURRET.z),
            v3(-0.2, -0.25, TURRET.z),
            v3(-0.2, 0.0, MUZZLE.z + 0.2),
        ];
        b.loft(&[root.to_vec(), vec![MUZZLE; 3]], true, false);
    });
}

fn body(b: &mut MeshBuilder) {
    // The dark frame the plates ride on, its seams lit red: nose to the mast's socket.
    seam(b);
    segment(
        b,
        &[
            (v3(2.55, 0.0, 1.12), 0.22, 0.14),
            (v3(2.15, 0.0, 1.25), 0.52, 0.28),
            (v3(1.55, 0.0, 1.3), 0.64, 0.33),
            (v3(0.95, 0.0, 1.3), 0.5, 0.28),
            (v3(0.55, 0.0, 1.34), 0.8, 0.4),
            (v3(-0.4, 0.0, 1.4), 0.95, 0.45),
            (v3(-1.2, 0.0, 1.46), 0.72, 0.4),
            (v3(-1.7, 0.0, 1.55), 0.42, 0.3),
        ],
        Vec3::Z,
    );
    head(b);
    abdomen(b);
    // The hip drums each leg turns in, bronze.
    b.mirror_y(|b| {
        for &(hip, _, foot, _) in &LEGS {
            let out = (foot - hip).truncate().extend(0.0).normalize();
            metal(b);
            let sides = b.sides(8);
            b.cylinder_between(hip - out * 0.3, hip + out * 0.16, 0.34, 0.3, sides);
        }
    });
    // The socket the mast stands in: an armoured collar.
    let root = MAST[0];
    dark_plate(b);
    let sides = b.sides(8);
    b.cylinder_between(
        root + v3(0.5, 0.0, -0.28),
        root + v3(-0.08, 0.0, 0.05),
        0.55,
        0.47,
        sides,
    );
    // The belly: a flat plate under it all.
    seam(b);
    b.frustum(
        v3(-0.2, 0.0, 0.84),
        Vec2::new(2.4, 0.7),
        Vec2::new(2.8, 0.95),
        0.3,
        Vec2::ZERO,
    );
    if b.fine() {
        // A ram each side from the back to the socket, bracing the mast.
        b.mirror_y(|b| piston(b, v3(-1.0, 0.45, 1.85), root + v3(0.15, 0.36, 0.22), 0.06));
    }
}

/// The head: a pointed sensor wedge under a plate brow, two pairs of red optics in the
/// dark beneath it, a bronze sensor bar under the nose, and a plate either side swept back
/// from the brow.
fn head(b: &mut MeshBuilder) {
    dark_plate(b);
    shell(
        b,
        &[
            (v3(2.62, 0.0, 1.22), 0.2, 0.07),
            (v3(2.25, 0.0, 1.4), 0.5, 0.15),
            (v3(1.6, 0.0, 1.5), 0.66, 0.2),
            (v3(1.0, 0.0, 1.45), 0.52, 0.16),
        ],
        Vec3::Z,
    );
    // The brow: a plate jutting over the optics.
    shell(
        b,
        &[
            (v3(2.2, 0.0, 1.5), 0.38, 0.1),
            (v3(2.72, 0.0, 1.32), 0.2, 0.05),
        ],
        Vec3::Z,
    );
    // Optics under the brow's lip, the middle pair biggest.
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        b.beam(
            v3(2.5, 0.07, 1.24),
            v3(2.56, 0.2, 1.21),
            Vec2::new(0.07, 0.05),
            Vec2::new(0.06, 0.04),
        );
        if b.fine() {
            b.beam(
                v3(2.34, 0.28, 1.22),
                v3(2.36, 0.36, 1.2),
                Vec2::new(0.05, 0.04),
                Vec2::new(0.04, 0.03),
            );
        }
    });
    // The sensor bar under the nose, bronze, across the head.
    metal(b);
    b.beam(
        v3(2.45, -0.3, 1.0),
        v3(2.45, 0.3, 1.0),
        Vec2::new(0.16, 0.14),
        Vec2::new(0.16, 0.14),
    );
    if b.fine() {
        b.mirror_y(|b| {
            // A plate off the brow, swept back to a point.
            dark_plate(b);
            plate(
                b,
                &[
                    v3(2.2, 0.36, 1.46),
                    v3(1.95, 0.46, 1.6),
                    v3(1.3, 0.6, 1.86),
                    v3(1.7, 0.5, 1.5),
                ],
                Vec3::Y * 0.05,
            );
        });
        // The neck: a bronze ring between head and back.
        metal(b);
        b.cylinder_between(v3(0.98, 0.0, 1.32), v3(0.84, 0.0, 1.33), 0.46, 0.46, 8);
    }
}

/// Half of one back plate (the left), a cross-section at `x`: an arch from the spine's
/// edge out over the flank, its rim hanging past the frame under it, swept back at the spine.
fn tergite_ring(b: &MeshBuilder, x: f32, w: f32, z: f32, h: f32, sweep: f32) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() {
        &[
            [1.0, -0.72],
            [0.78, -0.3],
            [0.2, 0.72],
            [0.17, 0.95],
            [0.4, 0.9],
            [0.72, 0.6],
            [0.97, 0.0],
        ]
    } else {
        &[[1.0, -0.72], [0.2, 0.72], [0.2, 0.95], [0.75, 0.6]]
    };
    shape
        .iter()
        .map(|&[s, u]| v3(x - sweep * (1.0 - s), s * w, z + u * h))
        .collect()
}

/// The back: three arched plates either side of a bronze spine, apart so the working
/// frame shows between them; under the last two a bronze cell each side.
fn abdomen(b: &mut MeshBuilder) {
    let sweep = 0.18;
    for (i, &(x, half, w, z, h)) in TERGITES.iter().enumerate() {
        b.mirror_y(|b| {
            dark_plate(b);
            b.loft(
                &[
                    tergite_ring(b, x + half, w * 0.92, z - 0.03, h * 0.9, sweep),
                    tergite_ring(b, x, w, z, h, sweep),
                    tergite_ring(b, x - half, w * 0.96, z + 0.03, h * 1.02, sweep),
                ],
                true,
                true,
            );
        });
        // The vertebra on the spine under the plate, bronze.
        let top = z + h;
        metal(b);
        b.beam(
            v3(x + half * 0.9 - sweep, 0.0, top - 0.14),
            v3(x - half * 0.9 - sweep, 0.0, top - 0.1),
            Vec2::new(0.16, 0.14),
            Vec2::new(0.16, 0.16),
        );
        if b.fine() && i > 0 {
            // A red line each side of the spine.
            b.paint(GLOW_LASER);
            b.mirror_y(|b| {
                b.beam(
                    v3(x + half * 0.6 - sweep, 0.14, top - 0.13),
                    v3(x - half * 0.6 - sweep, 0.14, top - 0.11),
                    Vec2::new(0.03, 0.03),
                    Vec2::new(0.03, 0.03),
                );
            });
        }
    }
    // The owner's colour: a chevron across the front plate.
    b.paint(TEAM);
    let (x, _, w, z, h) = TERGITES[0];
    b.mirror_y(|b| {
        b.beam(
            v3(x - 0.26, 0.14, z + h * 0.99),
            v3(x - 0.08, w * 0.45, z + h * 0.88),
            Vec2::new(0.12, 0.025),
            Vec2::new(0.12, 0.025),
        )
    });
    // The power cells between the rear plates' rims, banded, a line to the mast.
    b.mirror_y(|b| {
        let (x, _, w, z, _) = TERGITES[1];
        let (front, back) = (
            v3(x + 0.3, w * 0.8, z - 0.12),
            v3(x - 0.75, w * 0.76, z - 0.08),
        );
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(front, back, 0.15, 0.15, sides);
        if b.fine() {
            seam(b);
            for t in [0.3f32, 0.7] {
                let at = front.lerp(back, t);
                b.cylinder_between(at - Vec3::X * 0.04, at + Vec3::X * 0.04, 0.17, 0.17, 8);
            }
            metal(b);
            cable(
                b,
                &[
                    back,
                    back + v3(-0.3, -0.3, 0.1),
                    MAST[0] + v3(0.1, 0.3, 0.2),
                ],
                0.035,
            );
        }
    });
}

/// One left leg, long and sturdy: a hip drum, a plated thigh over a bronze bone and a ram
/// up to a raised bronze knee drum, a plated shin down to an armoured foot that ends in a
/// pointed tip.
fn leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, foot: Vec3) {
    let out = (foot - hip).truncate().extend(0.0).normalize();
    // Toward the outside of the leg's bend, for the keels.
    let outside = (Vec3::Z + out * 0.4).normalize();
    let thigh = knee - hip;
    let shin = foot - knee;
    let ankle = knee + shin * 0.72;
    let axis = out.cross(Vec3::Z);
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            metal(b);
            let sides = b.sides(6);
            b.cylinder_between(hip, knee - thigh * 0.05, 0.12, 0.1, sides);
            dark_plate(b);
            let (root, end) = (
                (hip + thigh * 0.12 + outside * 0.08, 0.3, 0.2),
                (knee - thigh * 0.1 + outside * 0.08, 0.26, 0.18),
            );
            if b.fine() {
                let middle = (hip + thigh * 0.55 + outside * 0.11, 0.33, 0.22);
                plate_arch(b, &[root, middle, end], outside);
            } else {
                plate_arch(b, &[root, end], outside);
            }
            if b.fine() {
                // The ram under the thigh that lifts it.
                let (_, up) = frame(thigh, outside);
                piston(
                    b,
                    hip + thigh * 0.12 - up * 0.2,
                    knee - thigh * 0.2 - up * 0.16,
                    0.07,
                );
            }
        });
        b.with_limb(rig::SHIN, |b| {
            // The knee: a bronze drum, and the bronze bone of the shin under its plates.
            metal(b);
            let sides = if b.fine() { 8 } else { 4 };
            b.cylinder_between(knee - axis * 0.2, knee + axis * 0.2, 0.2, 0.2, sides);
            b.cylinder_between(knee, ankle, 0.1, 0.08, b.sides(6));
            dark_plate(b);
            plate_arch(
                b,
                &[
                    (knee + shin * 0.06, 0.25, 0.19),
                    (ankle - shin * 0.02, 0.17, 0.13),
                ],
                out + Vec3::Z,
            );
            // The foot: an armoured shoe ending in a point in the ground.
            seam(b);
            let sides = b.sides(6);
            b.cylinder_between(ankle, foot + Vec3::Z * 0.01, 0.15, 0.008, sides);
            dark_plate(b);
            b.cylinder_between(ankle - shin * 0.06, ankle + shin * 0.1, 0.19, 0.16, sides);
        });
    });
}

/// The mast: four armoured segments from the socket up and round to its head, each a
/// bronze core on a bronze drum under an arched plate over the outside of the curl, its
/// trailing edge swept back into a point, each stopping short of both joints so every
/// joint shows as a gap.
fn mast(b: &mut MeshBuilder) {
    for i in 0..MAST.len() - 1 {
        let (a, c) = (MAST[i], MAST[i + 1]);
        let (wa, wc) = (MAST_WIDTH[i], MAST_WIDTH[i + 1]);
        let dir = c - a;
        let along = dir.normalize();
        let outward = ((a + c) * 0.5 - CURL).normalize();
        let (side, up) = frame(dir, outward);
        let (p0, p1) = (a + dir * 0.18, c - dir * 0.1);
        metal(b);
        core(
            b,
            &[(a, wa * 0.5, wa * 0.5), (c, wc * 0.5, wc * 0.5)],
            outward,
        );
        if b.fine() {
            // The joint's drum.
            b.cylinder_between(
                a - Vec3::Y * (wa * 0.85),
                a + Vec3::Y * (wa * 0.85),
                wa * 0.6,
                wa * 0.6,
                6,
            );
        }
        dark_plate(b);
        let arch = |p: Vec3, w: f32, k: f32| (p + up * (w * 0.4), w * k, w * 0.72 * k);
        shell(b, &[arch(p0, wa, 0.9), arch(p1, wc, 1.05)], outward);
        if !b.fine() || i == 0 {
            continue;
        }
        // The arch's trailing edge drawn back over the joint behind it into a point.
        let rim = p0 + up * (wa * 1.05);
        plate(
            b,
            &[
                rim + side * (wa * 0.35) + along * 0.25,
                rim - side * (wa * 0.35) + along * 0.25,
                rim - along * 0.32 + up * (0.12 + wa * 0.2),
            ],
            -up * 0.05,
        );
    }
    // The head: a bronze drum under the radar's bearing.
    let head = MAST[MAST.len() - 1];
    metal(b);
    let sides = b.sides(8);
    b.cylinder_between(
        head - Vec3::Y * 0.26,
        head + Vec3::Y * 0.26,
        0.2,
        0.2,
        sides,
    );
    b.prism(v3(RADAR.x, 0.0, head.z), sides, 0.24, 0.2, RADAR.z - head.z);
}

/// The radar array on its bearing, turning: a bronze hub, and across it a long dark
/// plate whose ends sweep back into points, a narrower plate lapped over its back, the
/// face's elements a red line along its front edge, bronze ribs behind.
fn radar(b: &mut MeshBuilder) {
    let (x, z) = (RADAR.x, RADAR.z);
    metal(b);
    b.prism(v3(x, 0.0, z), b.sides(8), 0.28, 0.26, 0.22);
    dark_plate(b);
    let array = [
        [x + 0.34, -1.0],
        [x + 0.34, 1.0],
        [x - 0.25, 1.35],
        [x - 0.12, 0.9],
        [x - 0.3, 0.0],
        [x - 0.12, -0.9],
        [x - 0.25, -1.35],
    ];
    b.loft_z(
        &array,
        &[
            crate::models::builder::Section::new(z + 0.2, 1.0),
            crate::models::builder::Section::new(z + 0.46, 1.0),
        ],
    );
    plate(
        b,
        &[
            v3(x - 0.05, -0.75, z + 0.46),
            v3(x - 0.05, 0.75, z + 0.46),
            v3(x - 0.36, 1.0, z + 0.5),
            v3(x - 0.4, -1.0, z + 0.5),
        ],
        Vec3::Z * 0.1,
    );
    // The elements: a row of small red lenses along the face.
    b.paint(GLOW_LASER);
    let lenses: &[f32] = if b.fine() {
        &[-0.75, -0.25, 0.25, 0.75]
    } else {
        &[-0.5, 0.5]
    };
    for &y in lenses {
        b.block(
            v3(x + 0.33, y - 0.07, z + 0.3),
            v3(x + 0.37, y + 0.07, z + 0.36),
        );
    }
    if b.fine() {
        metal(b);
        for y in [-0.6f32, 0.0, 0.6] {
            b.beam(
                v3(x - 0.2, y, z + 0.33),
                v3(x - 0.45, y, z + 0.28),
                Vec2::new(0.08, 0.1),
                Vec2::new(0.08, 0.06),
            );
        }
    }
}

/// The turret over the neck: a bronze ring, a small plated housing swept back to a point,
/// and the light repeater: a bronze barrel through a seam-dark collar, its bore red.
fn gun(b: &mut MeshBuilder) {
    let (z, m) = (TURRET.z, MUZZLE);
    metal(b);
    let sides = b.sides(10);
    b.prism(TURRET, sides, 0.34, 0.32, 0.12);
    dark_plate(b);
    b.loft_z(
        &[
            [0.45, 0.24],
            [-0.2, 0.3],
            [-0.7, 0.0],
            [-0.2, -0.3],
            [0.45, -0.24],
        ],
        &[
            crate::models::builder::Section::new(z + 0.12, 1.0),
            crate::models::builder::Section::new(m.z + 0.2, 0.8).shifted(-0.05, 0.0),
        ],
    );
    seam(b);
    b.cylinder_between(v3(0.4, 0.0, m.z), v3(0.62, 0.0, m.z), 0.13, 0.13, sides);
    metal(b);
    b.cylinder_between(
        v3(0.6, 0.0, m.z),
        v3(m.x - 0.12, 0.0, m.z),
        0.07,
        0.06,
        b.sides(6),
    );
    b.paint(GLOW_LASER);
    b.cylinder_between(v3(m.x - 0.12, 0.0, m.z), m, 0.08, 0.07, b.sides(6));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model, Model};

    fn model() -> Model {
        build_model("naga_scout").unwrap()
    }

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check("naga_scout", 3.8, 4.0, None, &[MUZZLE.to_array()]);
    }

    #[test]
    fn the_unit_files_gun_and_radar_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("naga_t1_scout").unwrap());
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        assert_eq!(bp.visual.mesh, "naga_scout");
        assert!(v(bp.weapons[0].muzzle).distance(MUZZLE) < 1e-3);
        assert!(bp.turret_at.is_none(), "the turret turns about the middle");
        assert!(bp.radar.to_f32() > 0.0, "it carries radar");

        let model = model();
        assert_eq!(Vec3::from(model.turret_pivot), TURRET);
        assert_eq!(Vec3::from(model.spinner_pivot), RADAR);
        for lod in &model.lods {
            // The array turns about its bearing: it reaches out either side of it.
            let array: Vec<Vec3> = lod
                .vertices
                .iter()
                .filter(|v| v.part == part::SPINNER)
                .map(|v| Vec3::from(v.pos))
                .collect();
            let span = array.iter().map(|p| p.y).fold(f32::MIN, f32::max);
            assert!(span > 1.0, "array spans {span}");
            assert!(array.iter().all(|p| p.z >= RADAR.z - 0.01));
            // No violet: it does not build.
            assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
        }
    }

    #[test]
    fn stands_on_six_legs_each_rigged_to_its_pair() {
        let model = model();
        let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
        assert_eq!(crawl.pairs, 3);
        assert!(crawl.pairs <= crate::models::MAX_CRAWL_PAIRS);
        assert_eq!(crawl.tail_count, 0, "the mast stands still");
        assert_eq!(crawl.claw, None);
        assert_eq!(crawl.gpu().len(), crate::models::CRAWL_SLOTS);
        for lod in &model.lods[..2] {
            for pair in 0..3u32 {
                let bones = |limb: u32| {
                    lod.vertices.iter().filter(move |v| {
                        v.part == part::LOCOMOTION
                            && v.rig & rig::LIMB_MASK == limb
                            && (v.rig & rig::PAIR_MASK) >> rig::PAIR_SHIFT == pair
                    })
                };
                for limb in [rig::THIGH, rig::SHIN] {
                    assert!(
                        bones(limb).any(|v| v.pos[1] > 0.0),
                        "pair {pair} left bone {limb}"
                    );
                    assert!(
                        bones(limb).any(|v| v.pos[1] < 0.0),
                        "pair {pair} right bone {limb}"
                    );
                }
                // The foot's tip reaches the ground its pair's rest pose names.
                let [_, _, foot] = crawl.joints[pair as usize];
                let low = bones(rig::SHIN).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
                assert!(low < 0.08, "pair {pair} foot at {low}");
                let reach = bones(rig::SHIN).map(|v| v.pos[1]).fold(0.0f32, f32::max);
                assert!(
                    (reach - foot[1]).abs() < 0.25,
                    "pair {pair} reaches y {reach} for {}",
                    foot[1]
                );
            }
        }
    }
}
