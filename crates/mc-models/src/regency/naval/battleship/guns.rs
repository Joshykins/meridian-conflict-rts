//! The Flamberge's guns: the three twin Pinch-fusion Cannon houses, the beam
//! secondaries, the AA house and the counter-seeker heads.
//!
//! A Pinch-fusion gun gathers its charge in front of its bore, held between projectors:
//! a twin house holds two, side by side, so its projectors stand either side of each and
//! one between them. A main house is the Sunspear's gun made twin: a stepped platform
//! with a gimballed core let into its back, three tall plated rails out past the bores,
//! gravity lenses in heads at their tips, pinch coils down each bore.

use glam::{Vec2, Vec3};

use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{
    armour, collar, hoop, hoop_on, mouth_rim, red_slot, shaft, swept, Course, Frame,
};
use super::hull::{body_x, mirrored, pointed, team_patch, tier, CHAMFERED};
use super::{charge, AA, AA_MUZZLE, MAIN, SECONDARY, SECONDARY_REACH, SECONDARY_TWIN, TWIN};

/// `n` facets round a drum close up, half as many at the reduced level: a capital ship
/// carries dozens of drums, and from the reduced level's distance they read as round.
fn facets(b: &MeshBuilder, n: usize) -> usize {
    if b.fine() {
        n
    } else {
        (n / 2).max(4)
    }
}

/// Main gun `i` (house `i`) on its barbette up from the deck at `deck`.
pub(super) fn main_gun(b: &mut MeshBuilder, i: usize, deck: f32) {
    let p = MAIN[i];
    let base = p.z - 1.3;
    if !b.coarse() {
        let sides = facets(b, 14);
        // A tall barbette (the raised house's) stands on a plated tower, pointed forward
        // and swept back aft, a short drum over it.
        let drum = if base - deck > 2.5 {
            let top = base - 1.0;
            tier(
                b,
                p.x,
                &pointed(6.8, 6.4, 5.0),
                deck - 0.4,
                top,
                Vec2::new(0.9, 0.88),
            );
            b.mirror_y(|b| {
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(
                        v3(p.x - 4.4, 3.9, top - 0.2),
                        v3(-1.0, -0.12, -0.3),
                        v3(0.0, 0.3, 1.0),
                    ),
                    &swept(5.5, 0.9, -0.4, 0.45),
                    0.26,
                );
            });
            top - 0.1
        } else {
            deck - 0.4
        };
        dark_plate(b);
        b.prism(v3(p.x, 0.0, drum), sides, 4.7, 4.4, base - drum);
        seam(b);
        b.prism(v3(p.x, 0.0, base - 0.35), sides, 4.5, 4.5, 0.3);
        if b.fine() {
            metal(b);
            hoop(b, v3(p.x, 0.0, base - 0.05), 3.9, 0.6, 0.16, sides);
        }
    }
    b.with_house(i, p, 0.6, |b| {
        if b.coarse() {
            coarse_house(b, i);
            return;
        }
        platform(b, i);
        b.with_recoil(|b| rails(b, i));
    });
}

/// Far off, a house is a plated wedge from its back to its charges.
fn coarse_house(b: &mut MeshBuilder, i: usize) {
    let (p, c) = (MAIN[i], charge(i));
    let base = p.z - 1.3;
    dark_plate(b);
    b.loft(
        &[
            vec![
                v3(p.x - 5.5, -3.8, base),
                v3(p.x - 5.5, 3.8, base),
                v3(p.x - 5.5, 0.0, p.z + 1.5),
            ],
            vec![
                v3(c.x, -2.6, c.z - 0.6),
                v3(c.x, 2.6, c.z - 0.6),
                v3(c.x, 0.0, c.z + 0.7),
            ],
        ],
        true,
        true,
    );
}

/// The owner's colour on a house's roof at `z`.
fn roof_colour(b: &mut MeshBuilder, i: usize, z: f32) {
    let x = MAIN[i].x;
    team_patch(b, x - 4.4, x - 2.6, 1.2, z + 0.01);
}

// ---- The main houses ---------------------------------------------------------------

/// A stepped platform on a bronze race, and let into its back a gimballed core: a
/// star flattened in a plated well, two bronze rings tilted across it.
fn platform(b: &mut MeshBuilder, i: usize) {
    let p = MAIN[i];
    let base = p.z - 1.3;
    let fine = b.fine();
    let sides = facets(b, 10);
    dark_plate(b);
    b.prism(v3(p.x - 1.0, 0.0, base), sides, 5.0, 4.8, 0.6);
    let plan = mirrored(&[
        [4.6, 2.4],
        [1.4, 4.6],
        [-4.4, 4.6],
        [-6.6, 2.6],
        [-7.0, 0.0],
    ]);
    b.at(v3(p.x, 0.0, 0.0), |b| {
        seam(b);
        b.loft_z(
            &plan,
            &[
                Section::new(base + 0.6, 0.97),
                Section::new(base + 0.9, 0.97),
            ],
        );
        dark_plate(b);
        b.loft_z(
            &plan,
            &[
                Section::new(base + 0.9, 1.0),
                Section::scaled(p.z + 1.0, 0.88, 0.82).shifted(-0.4, 0.0),
            ],
        );
    });
    roof_colour(b, i, p.z + 1.0);
    // The core's well, the star and its rings.
    let core = v3(p.x - 4.4, 0.0, p.z + 1.15);
    dark_plate(b);
    hoop(b, core, 1.55, 0.45, 0.5, facets(b, 14));
    b.paint(GLOW_PRISM);
    b.spheroid(
        core,
        v3(0.7, 0.7, 0.35),
        facets(b, 10),
        if fine { 5 } else { 3 },
    );
    metal(b);
    for axis in [v3(1.0, 0.0, 2.0), v3(-1.0, 0.0, 2.0)] {
        hoop_on(b, core, axis, 1.2, 0.22, 0.28, facets(b, 14));
    }
    b.mirror_y(|b| {
        // Cheek plates.
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(p.x + 1.8, 4.4, p.z - 0.6),
                v3(-1.0, 0.1, 0.0),
                v3(0.0, 1.0, 0.25),
            ),
            &swept(8.6, 0.75, 0.3, 0.5),
            0.26,
        );
    });
}

/// The rails gun in the house's frame: a breech, two bronze bores wound with pinch coils,
/// and three tall plated rails out past their mouths, heads at their tips carrying gravity
/// lenses aimed into the charges, tied across in bronze.
fn rails(b: &mut MeshBuilder, i: usize) {
    let fine = b.fine();
    let (p, c) = (MAIN[i], charge(i));
    let (x, z) = (p.x, c.z);
    let mouth = c.x - 3.0;
    collar(b, p, Vec3::Y, 1.1, 6.6);
    dark_plate(b);
    body_x(
        b,
        0.0,
        &[[x + 1.0, 8.0, 2.4, z], [x + 4.2, 7.6, 2.1, z]],
        &CHAMFERED,
    );
    let sides = facets(b, 10);
    let coils: &[(f32, f32)] = if fine {
        &[(6.5, 0.9), (8.5, 0.82), (10.4, 0.74)]
    } else {
        &[(8.5, 0.82)]
    };
    for y in [-TWIN, TWIN] {
        metal(b);
        b.cylinder_between(v3(x + 4.0, y, z), v3(mouth, y, z), 0.6, 0.55, sides);
        dark_plate(b);
        b.cylinder_between(v3(mouth - 0.6, y, z), v3(mouth, y, z), 0.7, 0.66, sides);
        b.paint(GLOW_PRISM);
        hoop_on(b, v3(mouth + 0.02, y, z), Vec3::X, 0.45, 0.14, 0.06, sides);
        // Each coil two plated discs round the bore, a groove lit with fusion between them.
        for &(dx, r) in coils {
            let at = v3(x + dx, y, z);
            dark_plate(b);
            for s in [-1.0f32, 1.0] {
                b.cylinder_between(
                    at + Vec3::X * (0.08 * s),
                    at + Vec3::X * (0.34 * s),
                    r,
                    r * 0.94,
                    sides,
                );
            }
            b.paint(GLOW_PRISM);
            b.cylinder_between(
                at - Vec3::X * 0.09,
                at + Vec3::X * 0.09,
                r * 0.86,
                r * 0.86,
                sides,
            );
        }
    }
    b.mirror_y(|b| {
        dark_plate(b);
        body_x(
            b,
            3.45,
            &[
                [x + 3.0, 0.9, 2.8, z],
                [c.x - 2.0, 0.8, 2.4, z],
                [c.x + 0.3, 0.7, 1.8, z],
            ],
            &CHAMFERED,
        );
        lens_head(b, c, 3.5, 1.3, 2.8);
        dark_plate(b);
        Course {
            count: if fine { 2 } else { 1 },
            step: 4.4,
            len: 5.0,
            half: 1.0,
            tip: 0.0,
            thick: 0.25,
            tail: 1.4,
        }
        .lay(
            b,
            &Frame::new(v3(c.x - 1.6, 4.15, z), v3(-1.0, 0.04, 0.0), Vec3::Y),
        );
        // The middle rail's lenses, each side.
        for dz in [-0.65f32, 0.65] {
            if !fine {
                break;
            }
            lens(b, v3(c.x - 0.5, 0.5, z + dz), v3(c.x, TWIN, z), 0.34);
        }
    });
    dark_plate(b);
    body_x(
        b,
        0.0,
        &[[x + 4.2, 0.6, 2.6, z + 0.1], [c.x - 2.8, 0.55, 2.2, z]],
        &CHAMFERED,
    );
    lens_head(b, c, 0.0, 0.9, 2.8);
    if fine {
        metal(b);
        for tx in [x + 5.2, c.x - 4.2] {
            for tz in [-1.15f32, 1.15] {
                b.beam(
                    v3(tx, -3.45, z + tz),
                    v3(tx, 3.45, z + tz),
                    Vec2::new(0.34, 0.34),
                    Vec2::new(0.34, 0.34),
                );
            }
        }
    }
}

/// A rail's head at `y`: a plated sleeve over the rail's tip, its nose swept in; on the
/// outer rails two gravity lenses on its inner face aimed into the charge.
fn lens_head(b: &mut MeshBuilder, c: Vec3, y: f32, w: f32, h: f32) {
    dark_plate(b);
    body_x(
        b,
        y,
        &[
            [c.x - 3.2, w, h, c.z],
            [c.x + 0.1, w, h, c.z],
            [c.x + 0.7, w * 0.6, h * 0.66, c.z],
        ],
        &CHAMFERED,
    );
    if y > 0.0 {
        for dz in [-0.65f32, 0.65] {
            lens(
                b,
                v3(c.x - 0.5, y - w * 0.5 - 0.05, c.z + dz),
                v3(c.x, TWIN, c.z),
                0.4,
            );
        }
    }
}

/// A gravity lens at `at`, turned to `toward`: a bronze housing ringed in plate, a lens
/// of fusion light in its face.
fn lens(b: &mut MeshBuilder, at: Vec3, toward: Vec3, r: f32) {
    let d = (toward - at).normalize();
    let sides = facets(b, 8);
    metal(b);
    b.cylinder_between(at - d * r * 0.9, at, r * 1.15, r, sides);
    b.paint(GLOW_PRISM);
    b.cylinder_between(at, at + d * 0.1, r * 0.72, r * 0.6, sides);
}

// ---- The beam secondaries ----------------------------------------------------------

/// Secondary `k` (house `4 + k`) on its barbette from the deck at `deck`: a low pointed
/// house, cheek plates swept back, two heavy plasmeric repeaters side by side with red
/// emitter faces. Authored facing forward; it rests trained outboard.
pub(super) fn secondary(b: &mut MeshBuilder, k: usize, deck: f32) {
    let p = SECONDARY[k];
    let base = p.z - 0.9;
    if !b.coarse() {
        let sides = facets(b, 10);
        dark_plate(b);
        b.prism(v3(p.x, p.y, deck - 0.3), sides, 2.3, 2.1, base - deck + 0.3);
        if b.fine() {
            metal(b);
            hoop(b, v3(p.x, p.y, base - 0.04), 1.8, 0.4, 0.12, sides);
        }
    }
    b.with_house(4 + k, p, 0.3, |b| {
        if b.coarse() {
            return;
        }
        let fine = b.fine();
        b.at(v3(p.x, p.y, 0.0), |b| {
            dark_plate(b);
            b.loft_z(
                &pointed(2.4, 2.4, 1.7),
                &[
                    Section::new(base, 1.0),
                    Section::new(p.z + 0.1, 1.0),
                    Section::scaled(p.z + 0.8, 0.72, 0.66).shifted(-0.4, 0.0),
                ],
            );
            b.mirror_y(|b| {
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(
                        v3(1.0, 1.55, p.z - 0.2),
                        v3(-1.0, 0.2, 0.0),
                        v3(0.0, 1.0, 0.4),
                    ),
                    &swept(3.8, 0.5, 0.3, 0.45),
                    0.2,
                );
            });
            if fine {
                red_slot(b, v3(-0.6, 0.0, p.z + 0.82), Vec3::Z, Vec3::Y, 1.0, 0.16);
            }
            b.with_recoil(|b| {
                let m = SECONDARY_REACH;
                for y in [-SECONDARY_TWIN, SECONDARY_TWIN] {
                    metal(b);
                    b.cylinder_between(v3(0.5, y, p.z), v3(1.4, y, p.z), 0.34, 0.34, facets(b, 8));
                    dark_plate(b);
                    b.beam(
                        v3(1.3, y, p.z + 0.05),
                        v3(m.x - 0.12, y, p.z + m.z),
                        Vec2::new(0.42, 0.55),
                        Vec2::new(0.4, 0.4),
                    );
                    b.paint(GLOW_LASER);
                    b.beam(
                        v3(m.x - 0.14, y, p.z + m.z),
                        v3(m.x, y, p.z + m.z),
                        Vec2::new(0.32, 0.3),
                        Vec2::new(0.32, 0.3),
                    );
                    if fine {
                        metal(b);
                        b.cylinder_between(
                            v3(m.x - 1.0, y, p.z + m.z + 0.24),
                            v3(m.x - 0.1, y, p.z + m.z + 0.24),
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
/// clamped breech held up at the sky, red rims at their mouths.
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
            mouth_rim(b, v3(len, y, 0.0), 0.4, sides);
        }
    }
    if fine {
        red_slot(b, v3(-0.3, 0.0, 0.6), Vec3::Z, Vec3::Y, 1.4, 0.08);
    }
}

// ---- Missile defence ---------------------------------------------------------------

/// A counter-seeker head centred on `at` on a bronze post up from `foot`: a faceted dark
/// head, the missile defence's red round its waist and on its lens, a launch rail swept
/// back either side.
pub(super) fn counter_seeker(b: &mut MeshBuilder, at: Vec3, foot: f32) {
    if b.coarse() {
        return;
    }
    let fine = b.fine();
    if at.z - 0.45 - foot > 0.05 {
        shaft(b, v3(at.x, at.y, foot), at - Vec3::Z * 0.45, 0.3);
    }
    let head = ngon(6, 0.8);
    b.at(v3(at.x, at.y, 0.0), |b| {
        dark_plate(b);
        b.loft_z(
            &head,
            &[
                Section::new(at.z - 0.55, 0.6),
                Section::new(at.z - 0.12, 1.0),
                Section::new(at.z + 0.18, 1.0),
                Section::new(at.z + 0.48, 0.5),
            ],
        );
        b.paint(GLOW_LASER);
        b.loft_z(
            &head,
            &[
                Section::new(at.z - 0.08, 1.06),
                Section::new(at.z + 0.1, 1.06),
            ],
        );
    });
    b.paint(GLOW_LASER);
    b.cuboid(at + Vec3::Z * 0.52, Vec3::new(0.32, 0.32, 0.1));
    if fine {
        for side in [-1.0f32, 1.0] {
            dark_plate(b);
            b.beam(
                at + v3(0.45, 0.7 * side, -0.1),
                at + v3(-0.55, 0.95 * side, 0.55),
                Vec2::new(0.28, 0.2),
                Vec2::new(0.12, 0.2),
            );
        }
    }
}
