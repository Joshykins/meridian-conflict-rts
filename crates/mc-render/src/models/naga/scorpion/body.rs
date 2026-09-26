//! The scorpion's body: head, neck collar, the plated abdomen, its flanks, the tail socket
//! and the belly.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::pattern;

use super::super::kit::*;
use super::*;

/// The abdomen's plates, front to back: middle x, half length, half width, base height
/// and how high its arch stands. Widest behind the first legs, narrowing to the socket.
const TERGITES: [(f32, f32, f32, f32, f32); 6] = [
    (2.3, 0.8, 3.5, 6.3, 1.3),
    (0.5, 0.8, 4.3, 6.45, 1.5),
    (-1.3, 0.78, 4.6, 6.6, 1.6),
    (-3.0, 0.74, 4.3, 6.75, 1.5),
    (-4.6, 0.66, 3.6, 6.85, 1.3),
    (-6.0, 0.58, 2.8, 6.9, 1.05),
];

pub(super) fn body(b: &mut MeshBuilder) {
    // The dark underbody the plates ride on, its seams lit: jaws to the tail's socket.
    seam(b);
    segment(
        b,
        &[
            (v3(11.8, 0.0, 4.4), 0.8, 0.45),
            (v3(10.4, 0.0, 4.85), 1.9, 0.95),
            (v3(7.8, 0.0, 5.2), 2.6, 1.25),
            (v3(5.2, 0.0, 5.3), 2.3, 1.2),
            (v3(3.5, 0.0, 5.3), 1.9, 1.05),
            (v3(1.8, 0.0, 5.6), 3.3, 1.6),
            (v3(-1.0, 0.0, 5.9), 4.1, 1.85),
            (v3(-3.8, 0.0, 6.2), 3.5, 1.75),
            (v3(-6.2, 0.0, 6.6), 2.3, 1.4),
            (v3(-7.2, 0.0, 6.9), 1.8, 1.2),
        ],
        Vec3::Z,
    );
    head(b);
    // The neck: a ringed collar between head and abdomen, a bronze bearing in its middle.
    if b.fine() {
        for (k, x) in [4.2f32, 3.5, 2.8].into_iter().enumerate() {
            if k == 1 {
                metal(b);
            } else {
                seam(b);
            }
            b.cylinder_between(
                v3(x + 0.25, 0.0, 5.5),
                v3(x - 0.25, 0.0, 5.5),
                2.4 - k as f32 * 0.1,
                2.4 - k as f32 * 0.1,
                10,
            );
        }
    }
    abdomen(b);
    flanks(b);
    socket(b);
    belly(b);

    // The coxae: a heavy bronze drum at each hip the leg turns in, capped with a plate.
    b.mirror_y(|b| {
        for &(hip, _, foot, _) in &LEGS {
            let out = (foot - hip).truncate().extend(0.0).normalize();
            metal(b);
            let sides = b.sides(10).min(10);
            b.cylinder_between(hip - out * 0.9, hip + out * 0.3, 1.25, 1.1, sides);
            if b.fine() {
                dark_plate(b);
                shell(
                    b,
                    &[
                        (hip - out * 0.8 + Vec3::Z * 0.55, 1.25, 0.65),
                        (hip + out * 0.35 + Vec3::Z * 0.45, 1.15, 0.55),
                    ],
                    Vec3::Z,
                );
            }
        }
    });

    // A crest of swept blades down the spine: the trailing edges of its plates.
    if b.fine() {
        dark_plate(b);
        for (i, x) in [8.4f32, 6.2, 2.6, 0.8, -1.0, -2.7, -4.3]
            .into_iter()
            .enumerate()
        {
            let tall = if i < 2 { 1.0 } else { 1.4 - i as f32 * 0.07 };
            let z = if i < 2 {
                7.0
            } else {
                TERGITES[(i - 2).min(5)].3 + TERGITES[(i - 2).min(5)].4 - 0.1
            };
            spike(
                b,
                v3(x + 0.4, 0.0, z - 0.1),
                v3(x - 1.1, 0.0, z + tall),
                0.34,
            );
        }
    }
}

/// The head: a heavy wedge under an overhanging brow, horned, a cluster of red eyes in the
/// dark beneath the brow, the jaws below.
fn head(b: &mut MeshBuilder) {
    dark_plate(b);
    // The carapace: broad and low, climbing from the brow to the neck.
    shell(
        b,
        &[
            (v3(12.2, 0.0, 5.1), 0.8, 0.3),
            (v3(11.0, 0.0, 5.75), 2.1, 0.7),
            (v3(8.6, 0.0, 6.2), 2.9, 1.0),
            (v3(5.8, 0.0, 6.3), 3.0, 1.0),
            (v3(3.8, 0.0, 6.05), 2.4, 0.8),
        ],
        Vec3::Z,
    );
    // The brow: a visor jutting over the eyes, and a raised crest behind it.
    shell(
        b,
        &[
            (v3(10.4, 0.0, 6.45), 1.7, 0.45),
            (v3(12.9, 0.0, 5.75), 0.9, 0.22),
        ],
        Vec3::Z,
    );
    if b.fine() {
        shell(
            b,
            &[
                (v3(10.2, 0.0, 7.0), 0.8, 0.5),
                (v3(7.8, 0.0, 7.35), 1.1, 0.7),
                (v3(5.0, 0.0, 7.2), 0.9, 0.55),
            ],
            Vec3::Z,
        );
    }
    // Cheek plates hanging over the jaws' roots, flared out.
    b.mirror_y(|b| {
        dark_plate(b);
        let hint = v3(0.0, 1.0, 0.7);
        shell(
            b,
            &[
                (v3(10.6, 1.8, 5.0), 0.85, 0.35),
                (v3(8.2, 2.6, 5.35), 1.2, 0.5),
                (v3(5.4, 2.85, 5.45), 1.05, 0.45),
            ],
            hint,
        );
        if b.fine() {
            // Horns off the brow, swept back.
            blade(
                b,
                v3(10.0, 1.4, 6.5),
                v3(7.0, 2.4, 8.9),
                0.42,
                v3(0.0, 1.0, 0.2),
            );
            blade(
                b,
                v3(7.2, 2.6, 6.5),
                v3(5.0, 3.5, 7.9),
                0.34,
                v3(0.0, 1.0, 0.2),
            );
            // Twin crest ridges down the carapace.
            dark_plate(b);
            b.beam(
                v3(9.8, 1.3, 6.55),
                v3(4.6, 1.7, 7.05),
                Vec2::new(0.3, 0.36),
                Vec2::new(0.36, 0.28),
            );
            // A lit seam along the cheek.
            b.paint(GLOW_LASER);
            b.beam(
                v3(9.6, 2.3, 4.85),
                v3(6.2, 3.0, 5.0),
                Vec2::new(0.1, 0.07),
                Vec2::new(0.1, 0.07),
            );
        }
    });
    // Eyes: in the dark under the brow's lip, the pair in the middle biggest.
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        b.beam(
            v3(11.75, 0.35, 5.2),
            v3(11.95, 0.9, 5.05),
            Vec2::new(0.3, 0.2),
            Vec2::new(0.24, 0.16),
        );
        if b.fine() {
            b.beam(
                v3(11.3, 1.25, 5.05),
                v3(11.4, 1.6, 4.95),
                Vec2::new(0.2, 0.14),
                Vec2::new(0.16, 0.12),
            );
            b.beam(
                v3(10.7, 1.85, 5.0),
                v3(10.75, 2.1, 4.9),
                Vec2::new(0.16, 0.12),
                Vec2::new(0.14, 0.1),
            );
            b.beam(
                v3(9.2, 2.45, 6.05),
                v3(9.6, 2.35, 5.95),
                Vec2::new(0.16, 0.12),
                Vec2::new(0.12, 0.1),
            );
        }
    });
    // The jaws: two hooked, segmented chelicerae under the brow.
    b.mirror_y(|b| {
        dark_plate(b);
        segment(
            b,
            &[
                (v3(10.8, 0.75, 4.25), 0.55, 0.5),
                (v3(12.3, 0.72, 4.05), 0.5, 0.45),
                (v3(13.2, 0.55, 3.8), 0.3, 0.3),
            ],
            Vec3::Z,
        );
        if b.fine() {
            b.paint(METAL).pattern(pattern::EMBER);
            b.cylinder_between(v3(13.0, 0.6, 3.85), v3(13.9, 0.2, 3.25), 0.26, 0.04, 5);
            knuckle(b, v3(12.3, 0.72, 4.05), Vec3::Y, 0.36, 0.95);
        }
    });
    // The owner's colour: a chevron on the brow.
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.beam(
            v3(7.0, 1.15, 7.2),
            v3(5.2, 2.5, 6.95),
            Vec2::new(0.4, 0.07),
            Vec2::new(0.4, 0.07),
        )
    });
}

/// Half of one plate of the abdomen (the left), a cross-section at `x`: an arch from the
/// spine's edge out over the flank, its rim hanging past the body, hollowed under, swept
/// back `sweep` metres at the spine so the plates read as chevrons.
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
        .map(|&[s, u]| v3(x - sweep * (1.0 - s.abs()), s * w, z + u * h))
        .collect()
}

/// The abdomen: a bare, ribbed spine down the middle, and on either side of it domed
/// chevron plates, each apart from the next so the machinery shows between them, rims
/// hanging over the flanks. Vertebrae ride the spine between the plates.
fn abdomen(b: &mut MeshBuilder) {
    let sweep = 0.7;
    for (i, &(x, half, w, z, h)) in TERGITES.iter().enumerate() {
        b.mirror_y(|b| {
            dark_plate(b);
            b.loft(
                &[
                    tergite_ring(b, x + half, w * 0.92, z - 0.1, h * 0.9, sweep),
                    tergite_ring(b, x, w, z, h, sweep),
                    tergite_ring(b, x - half, w * 0.97, z + 0.12, h * 1.02, sweep),
                ],
                true,
                true,
            );
        });
        let top = z + h;
        // The vertebra on the spine under the plate's middle, keeled, a blade raked back.
        metal(b);
        b.beam(
            v3(x + half * 0.9 - sweep, 0.0, top - 0.45),
            v3(x - half * 0.9 - sweep, 0.0, top - 0.3),
            Vec2::new(0.62, 0.55),
            Vec2::new(0.62, 0.6),
        );
        if !b.fine() {
            continue;
        }
        dark_plate(b);
        blade(
            b,
            v3(x - sweep, 0.0, top),
            v3(
                x - half - 1.6 - sweep * 0.5,
                0.0,
                top + 1.0 - i as f32 * 0.06,
            ),
            0.4,
            Vec3::Y,
        );
        b.mirror_y(|b| {
            // A lit seam down each side of the spine.
            b.paint(GLOW_LASER);
            b.beam(
                v3(x + half * 0.7 - sweep, 0.5, top - 0.55),
                v3(x - half * 0.7 - sweep, 0.5, top - 0.45),
                Vec2::new(0.08, 0.08),
                Vec2::new(0.08, 0.08),
            );
            // The flank plate hanging off the rim over the legs, a barb at its back corner.
            dark_plate(b);
            let rim = v3(x, w * 0.97, z - 0.72 * h + 0.25);
            b.loft(
                &[
                    vec![
                        rim + v3(half, 0.0, 0.0),
                        rim + v3(half * 0.95, 0.45, -1.5),
                        rim + v3(half * 0.95, 0.15, -1.6),
                        rim + v3(half, -0.3, 0.0),
                    ],
                    vec![
                        rim + v3(-half, 0.0, 0.1),
                        rim + v3(-half * 1.05, 0.5, -1.6),
                        rim + v3(-half * 1.05, 0.2, -1.7),
                        rim + v3(-half, -0.3, 0.1),
                    ],
                ],
                true,
                true,
            );
            spike(
                b,
                rim + v3(-half * 0.6, 0.4, -1.25),
                rim + v3(-half - 1.1, 1.05, -1.35),
                0.22,
            );
            // Two lit gill slits on the rear plates' flanks.
            if i >= 2 {
                b.paint(GLOW_LASER);
                for k in 0..2 {
                    let at = rim + v3(half * 0.6 - k as f32 * half * 0.9, 0.52, -0.8);
                    b.beam(
                        at,
                        at - v3(half * 0.5, 0.0, -0.12),
                        Vec2::new(0.08, 0.08),
                        Vec2::new(0.08, 0.05),
                    );
                }
            }
            // A blade on the plate's shoulder, raked back and out.
            dark_plate(b);
            let shoulder = v3(x - sweep * 0.3, w * 0.7, z + 0.6 * h);
            blade(
                b,
                shoulder,
                shoulder + v3(-half - 1.1, w * 0.25, 1.0),
                0.3,
                v3(0.0, 1.0, -0.4),
            );
        });
        // A rib of bare metal across the gap behind the plate.
        if i + 1 < TERGITES.len() {
            let gx = (x - half + TERGITES[i + 1].0 + TERGITES[i + 1].1) * 0.5 - sweep * 0.5;
            metal(b);
            b.mirror_y(|b| {
                cable(
                    b,
                    &[
                        v3(gx, 0.3, top - 0.5),
                        v3(gx, w * 0.5, z + h * 0.55),
                        v3(gx, w * 0.85, z + h * 0.05),
                    ],
                    0.16,
                )
            });
        }
    }
    // The owner's colour: a chevron across the widest plate.
    if b.fine() {
        b.paint(TEAM);
        let (x, _, w, z, h) = TERGITES[2];
        b.mirror_y(|b| {
            b.beam(
                v3(x - 0.62, 0.55, z + h * 0.98),
                v3(x - 0.2, w * 0.4, z + h * 0.88),
                Vec2::new(0.4, 0.07),
                Vec2::new(0.4, 0.07),
            )
        });
    }
}

/// The working flanks: cable runs from the head back to the socket under the plates'
/// rims, a ram from the body down to each hip, rams across the neck.
fn flanks(b: &mut MeshBuilder) {
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        metal(b);
        for (dy, dz) in [(0.0f32, 0.0f32), (0.25, 0.32)] {
            cable(
                b,
                &[
                    v3(9.0, 2.0 + dy, 5.0 + dz),
                    v3(5.6, 2.2 + dy, 5.2 + dz),
                    v3(3.4, 1.8 + dy, 5.3 + dz),
                    v3(1.2, 3.0 + dy, 5.7 + dz),
                    v3(-2.0, 3.4 + dy, 6.0 + dz),
                    v3(-5.0, 2.8 + dy, 6.3 + dz),
                    v3(-6.9, 1.8 + dy, 6.7 + dz),
                ],
                0.13,
            );
        }
        for &(hip, _, _, _) in &LEGS {
            ram(
                b,
                v3(hip.x - 0.7, 1.9, 6.6),
                hip + v3(0.3, -0.1, 1.05),
                0.26,
            );
        }
        // Across the neck, head to the first plate.
        ram(b, v3(5.2, 1.5, 6.1), v3(2.9, 2.0, 6.5), 0.24);
        ram(b, v3(5.0, 2.1, 5.2), v3(2.7, 2.6, 5.5), 0.22);
    });
}

/// The raised socket at the back the tail plugs into: an armoured collar on two rams.
fn socket(b: &mut MeshBuilder) {
    let root = TAIL[0];
    dark_plate(b);
    let sides = b.sides(12).min(12);
    b.cylinder_between(
        root + v3(1.7, 0.0, -0.9),
        root + v3(-0.2, 0.0, 0.1),
        2.35,
        2.05,
        sides,
    );
    if b.fine() {
        seam(b);
        b.cylinder_between(
            root + v3(-0.2, 0.0, 0.1),
            root + v3(-0.55, 0.0, 0.3),
            1.85,
            1.85,
            12,
        );
        b.mirror_y(|b| {
            ram(b, v3(-3.4, 2.1, 7.6), root + v3(0.2, 1.9, 0.7), 0.34);
            dark_plate(b);
            blade(
                b,
                root + v3(0.8, 1.9, 0.4),
                root + v3(-1.4, 2.9, 1.6),
                0.36,
                v3(0.0, 1.0, 0.3),
            );
        });
    }
}

/// Under it all: ribbed belly plates.
fn belly(b: &mut MeshBuilder) {
    seam(b);
    b.frustum(
        v3(1.0, 0.0, 3.3),
        Vec2::new(14.0, 4.4),
        Vec2::new(16.0, 6.0),
        1.2,
        Vec2::ZERO,
    );
    if b.fine() {
        dark_plate(b);
        for x in [6.4f32, 3.6, 0.8, -2.0, -4.6] {
            b.beam(
                v3(x + 0.9, 0.0, 3.35),
                v3(x - 0.9, 0.0, 3.4),
                Vec2::new(5.2, 0.4),
                Vec2::new(5.4, 0.4),
            );
        }
    }
}
