//! The Flamberge's lighter guns: the beam secondaries and the AA house (the main houses
//! are [`super::house`]; the gravity lenses are `turrets::rondel::lens_post`).

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::{armour, collar, hoop, swept, Course, Frame};
use super::super::super::parts::{inlay, ridged};
use super::hull::{body_x, pointed, CHAMFERED};
use super::{AA, AA_MUZZLE, SECONDARY, SECONDARY_REACH, SECONDARY_TWIN};

/// `n` facets round a drum close up, half as many at the reduced level: a capital ship
/// carries dozens of drums, and from the reduced level's distance they read as round.
pub(super) fn facets(b: &MeshBuilder, n: usize) -> usize {
    if b.fine() {
        n
    } else {
        (n / 2).max(4)
    }
}

// ---- The beam secondaries ----------------------------------------------------------

/// A round face across a tube's mouth at `at` (+x out of it), `r` round: Regency plate with
/// a red gasket inside its rim (`EMBER_RIM`), the light in the plate, not a lit piece.
fn mouth_face(b: &mut MeshBuilder, at: Vec3, r: f32, sides: usize) {
    let ring: Vec<Vec3> = (0..sides)
        .map(|k| {
            let a = std::f32::consts::TAU * k as f32 / sides as f32;
            at + v3(0.0, a.cos() * r, a.sin() * r)
        })
        .collect();
    inlay(b, &ring, Vec3::X, pattern::EMBER_RIM);
}

/// Secondary `k` (house `4 + k`) on its barbette from the deck at `deck`: a low pointed
/// house on a skirted drum, cheek plates lapped back into spikes, a mantlet over two heavy
/// plasmeric repeaters side by side, each a plated barrel on a bronze breech with a
/// gasketed mouth. Authored facing forward; it rests trained outboard.
pub(super) fn secondary(b: &mut MeshBuilder, k: usize, deck: f32) {
    let p = SECONDARY[k];
    let base = p.z - 0.9;
    if !b.coarse() {
        let sides = facets(b, 10);
        dark_plate(b);
        b.prism(
            v3(p.x, p.y, deck - 0.3),
            sides,
            2.4,
            2.15,
            base - deck + 0.3,
        );
        if b.fine() {
            metal(b);
            hoop(b, v3(p.x, p.y, base - 0.04), 1.85, 0.4, 0.12, sides);
        }
    }
    b.with_house(4 + k, p, 0.45, |b| {
        if b.coarse() {
            return;
        }
        let fine = b.fine();
        b.at(v3(p.x, p.y, 0.0), |b| {
            dark_plate(b);
            b.loft_z(
                &pointed(2.6, 2.6, 1.8),
                &[
                    Section::new(base, 1.0),
                    Section::new(p.z + 0.1, 1.0),
                    Section::scaled(p.z + 0.85, 0.72, 0.66).shifted(-0.45, 0.0),
                ],
            );
            b.mirror_y(|b| {
                dark_plate(b);
                Course {
                    count: if fine { 2 } else { 1 },
                    step: 1.6,
                    len: 2.4,
                    half: 0.45,
                    tip: 0.3,
                    thick: 0.18,
                    tail: 1.0,
                }
                .lay(
                    b,
                    &Frame::new(
                        v3(1.4, 1.62, p.z - 0.2),
                        v3(-1.0, 0.2, 0.0),
                        v3(0.0, 1.0, 0.4),
                    ),
                );
            });
            if fine {
                inlay(
                    b,
                    &[
                        v3(-1.6, -0.5, p.z + 0.86),
                        v3(-0.4, -0.5, p.z + 0.86),
                        v3(-0.4, 0.5, p.z + 0.86),
                        v3(-1.6, 0.5, p.z + 0.86),
                    ],
                    Vec3::Z,
                    pattern::EMBER_MACHINED,
                );
            }
            b.with_recoil(|b| {
                let m = SECONDARY_REACH;
                // The mantlet over both breeches.
                dark_plate(b);
                ridged(
                    b,
                    &Frame::new(v3(1.0, 0.0, p.z - 0.5), Vec3::Z, Vec3::X),
                    &[[0.0, -1.1], [0.0, 1.1], [1.1, 0.9], [1.1, -0.9]],
                    0.3,
                    [0.55, 0.0],
                    0.12,
                );
                for y in [-SECONDARY_TWIN, SECONDARY_TWIN] {
                    let sides = facets(b, 8);
                    metal(b);
                    b.cylinder_between(v3(0.5, y, p.z), v3(1.6, y, p.z), 0.34, 0.34, sides);
                    dark_plate(b);
                    b.beam(
                        v3(1.5, y, p.z + 0.05),
                        v3(m.x - 0.5, y, p.z + m.z),
                        Vec2::new(0.42, 0.52),
                        Vec2::new(0.38, 0.4),
                    );
                    b.cylinder_between(
                        v3(m.x - 0.6, y, p.z + m.z),
                        v3(m.x, y, p.z + m.z),
                        0.26,
                        0.24,
                        sides,
                    );
                    if fine {
                        mouth_face(b, v3(m.x + 0.01, y, p.z + m.z), 0.22, sides);
                        metal(b);
                        b.cylinder_between(
                            v3(m.x - 1.4, y, p.z + m.z + 0.24),
                            v3(m.x - 0.5, y, p.z + m.z + 0.24),
                            0.1,
                            0.1,
                            6,
                        );
                    }
                }
            });
        });
    });
}

// ---- AA --------------------------------------------------------------------------

/// The AA house on the crown at `deck`: a drum, a hex step on a bronze race with a
/// plated cheek either side swept back into spikes, and between them two AA tubes in a
/// clamped breech held up at the sky, gasketed mouths.
pub(super) fn aa_house(b: &mut MeshBuilder, deck: f32) {
    let base = AA.z - 0.9;
    if !b.coarse() {
        let sides = facets(b, 10);
        dark_plate(b);
        b.prism(
            v3(AA.x, 0.0, deck - 0.3),
            sides,
            2.3,
            2.1,
            base - deck + 0.3,
        );
        if b.fine() {
            metal(b);
            hoop(b, v3(AA.x, 0.0, base - 0.04), 1.8, 0.45, 0.14, sides);
        }
    }
    let d = AA_MUZZLE - AA;
    let len = d.length();
    b.with_house(3, AA, 0.3, |b| {
        if b.coarse() {
            return;
        }
        dark_plate(b);
        b.prism(v3(AA.x, 0.0, base), 6, 1.95, 1.7, 0.55);
        b.mirror_y(|b| {
            dark_plate(b);
            b.block(
                v3(AA.x - 1.2, 1.25, base + 0.5),
                v3(AA.x + 0.9, 1.7, AA.z + 0.65),
            );
            armour(
                b,
                &Frame::new(
                    v3(AA.x + 0.9, 1.7, AA.z + 0.25),
                    v3(-1.0, 0.0, 0.35),
                    Vec3::Y,
                ),
                &swept(3.6, 0.7, 0.3, 0.45),
                0.24,
            );
        });
        b.with_recoil(|b| {
            b.at(AA, |b| organ(b, len));
        });
    });
}

/// The AA organ in its own frame (the trunnion at the origin, +x up the bore).
fn organ(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    collar(b, Vec3::ZERO, Vec3::Y, 0.5, 2.4);
    dark_plate(b);
    body_x(
        b,
        0.0,
        &[[-0.9, 2.2, 1.15, 0.0], [0.5, 2.3, 1.2, 0.0]],
        &CHAMFERED,
    );
    let sides = facets(b, 8);
    for y in [-0.55f32, 0.55] {
        dark_plate(b);
        b.cylinder_between(v3(0.4, y, 0.0), v3(len - 0.5, y, 0.0), 0.38, 0.35, sides);
        if fine {
            metal(b);
            b.cylinder_between(
                v3(len - 0.7, y, 0.0),
                v3(len - 0.42, y, 0.0),
                0.42,
                0.42,
                sides,
            );
        }
        dark_plate(b);
        b.cylinder_between(v3(len - 0.45, y, 0.0), v3(len, y, 0.0), 0.36, 0.4, sides);
        if fine {
            mouth_face(b, v3(len + 0.01, y, 0.0), 0.36, sides);
        }
    }
    if fine {
        inlay(
            b,
            &[
                v3(-0.9, -0.4, 0.61),
                v3(0.3, -0.4, 0.61),
                v3(0.3, 0.4, 0.61),
                v3(-0.9, 0.4, 0.61),
            ],
            Vec3::Z,
            pattern::EMBER_SLOT,
        );
    }
}
