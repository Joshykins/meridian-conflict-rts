//! Fulgur: a long, faceted super-heavy hull with a broad centred turret and its
//! AEB-2 in an armoured shroud. A bolt rifle on each flank sponson, and a low twin
//! flak mount on the engine deck that rests facing aft to cover the tank's back.

use glam::{Affine3A, Vec3};

use super::bolt_rifle::{bolt_rifle, bore_face, hexagon, ring, seam, vents};
use super::parts::*;
use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern, rig};

/// The main deck, on the raised centre tier: the turret race stands on it.
const DECK: f32 = 7.4;
/// The outer deck round the centre tier, over the tracks.
const OUTER_DECK: f32 = 6.4;
/// The AEB-2's trunnion, well back between the turret's cheeks, and its muzzle, on
/// the turret's centre line.
const BREECH: Vec3 = Vec3::new(0.5, 0.0, 11.2);
const MUZZLE: Vec3 = Vec3::new(23.0, 0.0, 11.2);
/// Sponson house pivot (left; the right is its mirror), the bolt rifle's breech and muzzle.
const SPONSON: Vec3 = Vec3::new(9.0, 10.5, 7.2);
const SPONSON_BREECH: Vec3 = Vec3::new(10.3, 10.5, 8.5);
const SPONSON_MUZZLE: Vec3 = Vec3::new(17.5, 10.5, 8.5);
/// Top of the sponson a house stands on.
const SPONSON_TOP: f32 = 7.05;
/// Flak house pivot on the engine deck; its barrels as authored facing the nose (the
/// house rests turned aft, `facing: 180`), and their spacing.
const FLAK: Vec3 = Vec3::new(-13.5, 0.0, 8.8);
const FLAK_MUZZLE: Vec3 = Vec3::new(-6.6, 0.0, 9.45);
const FLAK_GAP: f32 = 0.55;
const ENGINE_DECK: f32 = 8.8;

const REAR: f32 = -19.0;
const FRONT: f32 = 19.5;
const HALF_WIDTH: f32 = 12.3;
/// Track inner and outer edge (y) and height.
const TRACK: (f32, f32, f32) = (6.8, 11.8, 4.2);

pub(super) fn assault_tank(b: &mut MeshBuilder, _tech: u8) {
    build(b);
}

fn build(b: &mut MeshBuilder) {
    let (inner, outer, _) = TRACK;
    b.set_treads((inner + outer) * 0.5, outer - inner, -18.6);
    b.set_dust_line(5.0);

    running_gear(b);
    hull(b);
    sponson(b, 1, 1.0);
    sponson(b, 2, -1.0);
    engine_deck(b);
    turret(b);
}

/// Two track units each side, a gap between them for the drive.
fn running_gear(b: &mut MeshBuilder) {
    let (inner, outer, h) = TRACK;
    b.mirror_y(|b| {
        if b.coarse() {
            track(b, -18.6, 18.2, inner, outer, h);
            return;
        }
        track(b, -18.6, -1.1, inner, outer, h);
        track(b, 1.1, 18.2, inner, outer, h);
        // The drive between them: a dark final-drive housing.
        b.paint(ACCENT);
        b.block(v3(-1.3, inner + 0.3, 1.2), v3(1.3, outer - 0.6, 3.4));
    });
}

fn hull(b: &mut MeshBuilder) {
    let (inner, outer, _) = TRACK;
    let plan = hull_plan(REAR, FRONT, HALF_WIDTH, 7.0);
    let (belt, band) = (3.7, 5.0);
    let top = Section::scaled(OUTER_DECK, 0.86, 0.84).shifted(-0.4, 0.0);
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(-0.3, 0.0, 1.0),
            v2(FRONT - REAR, HALF_WIDTH * 2.0),
            v2((FRONT - REAR) * 0.84, HALF_WIDTH * 1.6),
            DECK - 1.0,
            v2(-0.4, 0.0),
        );
        return;
    }
    // Dark tub between the tracks, the dark frame band over them, and the white
    // upper hull sloping in from it to the deck.
    b.paint(ACCENT);
    b.block(
        v3(REAR + 0.8, -inner, 0.9),
        v3(FRONT - 0.9, inner, belt + 0.1),
    );
    b.loft_z(&plan, &[Section::new(belt, 0.96), Section::new(band, 1.0)]);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(band, 0.98),
            Section::new(band + 0.4, 0.98),
            top,
        ],
    );
    // The centre tier the turret stands on: a second layer of plate, sloped all round.
    b.loft_z(
        &hull_plan(-9.2, 13.8, 7.4, 2.6),
        &[
            Section::new(OUTER_DECK - 0.1, 1.0),
            Section::scaled(DECK, 0.94, 0.88).shifted(0.2, 0.0),
        ],
    );

    // The glacis, front and rear slopes as the loft makes them (profile x, z).
    let glacis = ([FRONT * 0.98, band + 0.4], [FRONT * 0.86 - 0.4, OUTER_DECK]);
    // Add-on armour: a thick plate over the glacis, a dark frame round it.
    on_slope(b, glacis.0, glacis.1, 0.45, |b| {
        b.paint(ACCENT);
        b.plate(Vec3::ZERO, v2(1.9, 12.4), 0.14, 0.05);
        b.paint(PLATING);
        b.plate(v3(0.0, 0.0, 0.14), v2(1.6, 11.6), 0.3, 0.12);
    });
    // Team colour flash on the deck behind each sponson, clear of the turret's sweep.
    b.mirror_y(|b| team_panel(b, v3(1.2, 8.9, OUTER_DECK), v2(4.4, 1.6)));
    if !b.fine() {
        return;
    }
    on_slope(b, glacis.0, glacis.1, 0.88, |b| {
        glow_strip(b, Vec3::ZERO, v2(0.22, 6.0), GLOW)
    });
    b.mirror_y(|b| {
        // Skirts over each track's upper run, hung off the frame band.
        b.paint(PLATING);
        for (x0, x1) in [(-15.8, -1.6), (1.6, 14.9)] {
            b.block(v3(x0, outer + 0.05, 1.9), v3(x1, outer + 0.4, belt + 0.05));
        }
        // Blue deck-edge strips: the earned emitters, more than any tier below.
        glow_strip(b, v3(-4.0, 9.6, OUTER_DECK), v2(8.0, 0.22), GLOW);
        // Tow shackles at the nose.
        b.paint(ACCENT);
        b.block(v3(FRONT - 0.3, 6.0, 2.2), v3(FRONT + 0.25, 6.9, 3.0));
    });
}

/// A flank sponson and its gun house: a Paladin-pattern bolt rifle on a low faceted
/// house. `side` is +1 for the left (weapon 1), −1 for the right (weapon 2); the rifle
/// is mirrored with it, so its plasma cell is outboard on both.
fn sponson(b: &mut MeshBuilder, weapon: usize, side: f32) {
    let y = SPONSON.y * side;
    if b.coarse() {
        return;
    }
    // The box, sloped at its face, standing out over the front track.
    b.paint(PLATING);
    let (near, far) = if side > 0.0 {
        (8.0, 12.9)
    } else {
        (-12.9, -8.0)
    };
    b.extrude_y(
        &[
            [5.2, 4.7],
            [12.4, 4.7],
            [13.6, 5.9],
            [13.0, SPONSON_TOP],
            [5.8, SPONSON_TOP],
            [5.2, 6.3],
        ],
        near,
        far,
    );
    b.paint(ACCENT);
    b.prism(
        v3(SPONSON.x, y, SPONSON_TOP - 0.05),
        b.sides(10),
        2.35,
        2.3,
        0.2,
    );

    let pivot = v3(SPONSON.x, y, SPONSON.z);
    b.with_house(weapon, pivot, 0.25, |b| {
        b.paint(PLATING);
        b.at(v3(8.5, y, 0.0), |b| {
            b.loft_z(
                &turret_plan(4.4, 3.8),
                &[
                    Section::new(SPONSON_TOP + 0.1, 0.9),
                    Section::new(7.9, 1.0),
                    Section::scaled(8.9, 0.72, 0.74).shifted(-0.3, 0.0),
                ],
            );
        });
        team_panel(b, v3(7.8, y, 8.9), v2(1.3, 1.4));
        b.with_recoil(|b| {
            b.with(Affine3A::from_scale(v3(1.0, side, 1.0)), |b| {
                bolt_rifle(b, SPONSON_BREECH, SPONSON_MUZZLE, 0.6)
            })
        });
    });
}

/// The raised engine deck aft, its louvres, and the flak house on it.
fn engine_deck(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    b.paint(PLATING);
    b.frustum(
        v3(-12.5, 0.0, 6.0),
        v2(8.2, 13.4),
        v2(7.4, 11.6),
        ENGINE_DECK - 6.0,
        v2(0.2, 0.0),
    );
    b.mirror_y(|b| {
        // Engine louvres: the glow between the slats is drawn, and breathes.
        b.paint(ACCENT).pattern(pattern::FURNACE);
        b.plate(v3(-11.0, 4.1, ENGINE_DECK), v2(4.4, 2.4), 0.12, 0.04);
    });
    if b.fine() {
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(-16.2, 0.0, ENGINE_DECK), v2(0.8, 6.0), 0.08, 0.03);
        // Exhaust stacks at the tail corners, dark and hot inside.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(-16.2, 5.2, 7.0), v3(-16.6, 5.6, 9.6), 0.55, 0.5, 8);
        });
    }
    flak(b);
}

/// The twin heavy flak mount, built facing the nose; the house rests turned aft. Low,
/// so the main gun passes over it: a faceted shield house, ammunition boxes on its
/// flanks, and two long barrels in recoil sleeves out to slotted brakes.
fn flak(b: &mut MeshBuilder) {
    b.with_house(3, FLAK, 0.35, |b| {
        b.paint(ACCENT);
        b.prism(
            v3(FLAK.x, 0.0, ENGINE_DECK - 0.02),
            b.sides(10),
            2.1,
            2.0,
            0.3,
        );
        b.paint(PLATING);
        b.at(v3(FLAK.x + 0.2, 0.0, 0.0), |b| {
            b.loft_z(
                &turret_plan(4.8, 4.0),
                &[
                    Section::new(ENGINE_DECK + 0.2, 1.0),
                    Section::scaled(9.85, 0.76, 0.72).shifted(-0.3, 0.0),
                ],
            );
        });
        if b.fine() {
            team_panel(b, v3(FLAK.x - 0.9, 0.0, 9.85), v2(1.2, 1.4));
            b.mirror_y(|b| {
                b.paint(PLATING_DARK);
                b.block(
                    v3(FLAK.x - 1.6, 1.85, ENGINE_DECK + 0.25),
                    v3(FLAK.x + 0.8, 2.35, 9.6),
                );
                b.paint(ACCENT);
                b.block(v3(FLAK.x - 1.3, 2.35, 9.1), v3(FLAK.x + 0.5, 2.4, 9.35));
            });
        }
        b.with_recoil(|b| {
            for side in [-1.0, 1.0] {
                let y = side * FLAK_GAP;
                let z = FLAK_MUZZLE.z;
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.cuboid(v3(FLAK.x + 1.9, y, z), v3(1.0, 0.5, 0.6));
                b.paint(PLATING).pattern(pattern::PLAIN);
                b.cylinder_between(
                    v3(FLAK.x + 2.3, y, z),
                    v3(FLAK.x + 3.9, y, z),
                    0.27,
                    0.25,
                    b.sides(8),
                );
                b.paint(METAL).pattern(pattern::PLAIN);
                b.cylinder_between(
                    v3(FLAK.x + 3.9, y, z),
                    v3(FLAK_MUZZLE.x - 0.6, y, z),
                    0.17,
                    0.15,
                    b.sides(8),
                );
                b.paint(PLATING_DARK).pattern(pattern::PLAIN);
                b.cuboid(v3(FLAK_MUZZLE.x - 0.3, y, z), v3(0.6, 0.34, 0.34));
                if b.fine() {
                    b.paint(ACCENT);
                    for k in 0..3 {
                        let x = FLAK_MUZZLE.x - 0.52 + k as f32 * 0.17;
                        b.block(v3(x, y - 0.18, z - 0.08), v3(x + 0.08, y + 0.18, z + 0.08));
                    }
                }
            }
        });
    });
}

fn turret(b: &mut MeshBuilder) {
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_recoil(BREECH, MUZZLE, 1.2);
    b.set_arm_pivot(BREECH);
    let roof = 10.7;
    b.with_part(part::TURRET, |b| {
        if b.coarse() {
            b.paint(PLATING);
            b.frustum_open(
                v3(-0.4, 0.0, DECK),
                v2(18.0, 13.2),
                v2(14.0, 10.0),
                roof - DECK,
                v2(-0.8, 0.0),
            );
            team_panel(b, v3(-3.0, 0.0, roof), v2(6.0, 3.0));
            b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
                b.paint(PLATING_DARK);
                b.beam(BREECH, MUZZLE, v2(2.2, 2.2), v2(1.0, 1.0));
            });
            return;
        }
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, DECK - 0.05), b.sides(12), 6.6, 6.5, 0.35);
        let plan = turret_plan(18.0, 13.2);
        b.at(v3(-0.4, 0.0, 0.0), |b| {
            b.paint(ACCENT);
            b.loft_z(
                &plan,
                &[Section::new(DECK + 0.2, 0.97), Section::new(8.4, 0.97)],
            );
            b.paint(PLATING);
            b.loft_z(
                &plan,
                &[
                    Section::new(8.4, 1.0),
                    Section::new(9.9, 1.0),
                    Section::scaled(roof, 0.84, 0.8).shifted(-0.8, 0.0),
                ],
            );
        });
        // Armoured cheeks either side of the gun, higher than the roof: the gun rides
        // deep between them.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.at(v3(0.0, 2.3, 0.0), |b| {
                b.extrude_y_chamfered(
                    &[
                        [-4.5, roof - 0.2],
                        [6.2, roof - 0.2],
                        [8.0, 11.3],
                        [7.4, 12.5],
                        [1.6, 12.8],
                        [-4.5, 11.7],
                    ],
                    0.75,
                    0.3,
                );
            });
            if b.fine() {
                seam(b, -2.5, 5.0, 3.05, 11.9, 1.0);
            }
        });
        // Stowage bustle on the turret's back.
        b.paint(PLATING_DARK);
        b.block(v3(-10.4, -4.2, 8.7), v3(-8.9, 4.2, 10.3));
        team_panel(b, v3(-5.0, 3.6, roof), v2(4.0, 1.7));
        if b.fine() {
            // Commander's cupola with vision blocks, a sensor box, an antenna.
            b.paint(PLATING_DARK);
            b.prism(v3(-4.6, -3.9, roof - 0.02), b.sides(8), 1.2, 1.0, 0.6);
            b.paint(GLASS);
            b.chamfered_box(v3(-3.7, -3.9, roof + 0.35), v3(0.3, 1.0, 0.25), 0.08);
            b.paint(PLATING_DARK);
            b.chamfered_box(v3(-7.4, 0.0, roof + 0.3), v3(1.6, 2.2, 0.7), 0.2);
            antenna(b, v3(-7.6, 3.6, roof), 2.4, 0.2);
            // Smoke dischargers at the front corners.
            b.mirror_y(|b| {
                b.paint(ACCENT);
                for k in 0..3 {
                    let at = v3(3.2 - k as f32 * 0.6, 4.4, roof);
                    b.cylinder_between(at, at + v3(0.5, 0.35, 0.3), 0.18, 0.18, 6);
                }
            });
        }
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
            main_gun(b, BREECH, MUZZLE, 1.0)
        });
    });
}

/// The AEB-2, level from `breech` to `muzzle`: the ARC's heaviest gun. A faceted housing
/// sunk between the cheeks with capacitor pods on its back, a core carried in swept
/// strakes, a stepped crown at the muzzle. Its light is thin seams and lines, never lit
/// rings.
fn main_gun(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, r: f32) {
    let length = muzzle.x - breech.x;
    b.at(breech, |b| {
        housing(b, length, r);
        capacitor(b, length, r);
        crown(b, length, r);
    });
}

/// An octagonal core, `r0` across, along x.
fn core(b: &mut MeshBuilder, x0: f32, x1: f32, r0: f32) {
    let core: Vec<[f32; 2]> = (0..8)
        .map(|k| {
            let a = (k as f32 + 0.5) * std::f32::consts::TAU / 8.0;
            [a.cos() * r0, a.sin() * r0]
        })
        .collect();
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.extrude_x(&core, x0, x1);
}

/// The breech block, and the faceted housing over the rear of the barrel with its raised
/// top plate, flank vents and a seam down each flank.
fn housing(b: &mut MeshBuilder, length: f32, r: f32) {
    let l = |f: f32| f * length;
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(&hexagon(r * 1.9, r * 1.8), l(-0.12)),
            ring(&hexagon(r * 2.1, r * 2.0), l(-0.04)),
        ],
        true,
        true,
    );
    b.paint(PLATING);
    b.loft(
        &[
            ring(&hexagon(r * 2.2, r * 2.1), l(-0.05)),
            ring(&hexagon(r * 2.4, r * 2.3), l(0.06)),
            ring(&hexagon(r * 2.4, r * 2.3), l(0.3)),
            ring(&hexagon(r * 1.5, r * 1.4), l(0.42)),
        ],
        true,
        true,
    );
    b.paint(PLATING_DARK);
    b.loft(
        &[
            ring(
                &[
                    [-r * 0.7, r * 1.1],
                    [r * 0.7, r * 1.1],
                    [r * 0.55, r * 1.35],
                    [-r * 0.55, r * 1.35],
                ],
                l(0.0),
            ),
            ring(
                &[
                    [-r * 0.6, r * 1.1],
                    [r * 0.6, r * 1.1],
                    [r * 0.45, r * 1.28],
                    [-r * 0.45, r * 1.28],
                ],
                l(0.32),
            ),
        ],
        true,
        true,
    );
    if b.fine() {
        b.mirror_y(|b| {
            seam(b, l(0.08), l(0.28), r * 1.2, r * 0.25, r);
            vents(b, l(0.08), l(0.025), 7, r * 1.2, (-r * 0.6, -r * 0.2), r);
        });
    }
}

/// The muzzle crown: a dark stage, a light faceted stage with ports, the bore in it.
fn crown(b: &mut MeshBuilder, length: f32, r: f32) {
    let l = |f: f32| f * length;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(&hexagon(r * 1.3, r * 1.25), l(0.89)),
            ring(&hexagon(r * 1.55, r * 1.5), l(0.915)),
        ],
        true,
        true,
    );
    b.paint(PLATING);
    b.loft(
        &[
            ring(&hexagon(r * 1.65, r * 1.6), l(0.915)),
            ring(&hexagon(r * 1.7, r * 1.65), l(0.97)),
            ring(&hexagon(r * 1.3, r * 1.25), length),
        ],
        true,
        true,
    );
    bore_face(b, length, v2(r * 0.8, r * 0.75));
    if b.fine() {
        b.mirror_y(|b| vents(b, l(0.925), l(0.014), 3, r * 0.85, (-r * 0.25, r * 0.25), r));
    }
}

/// Twin capacitor pods ride the housing's back and feed the core forward of it, which is
/// carried in three swept strakes with a blue line at each root.
fn capacitor(b: &mut MeshBuilder, length: f32, r: f32) {
    let l = |f: f32| f * length;
    core(b, l(0.4), l(0.9), r * 0.55);
    for angle in [0.0, 2.1, -2.1] {
        b.with(Affine3A::from_rotation_x(angle), |b| {
            b.paint(PLATING).pattern(pattern::PLAIN);
            b.extrude_y(
                &[
                    [l(0.4), r * 0.5],
                    [l(0.88), r * 0.5],
                    [l(0.86), r * 0.9],
                    [l(0.5), r * 1.1],
                ],
                -r * 0.12,
                r * 0.12,
            );
            if b.fine() {
                b.paint(GLOW);
                b.block(
                    v3(l(0.45), -r * 0.13, r * 0.5),
                    v3(l(0.84), r * 0.13, r * 0.56),
                );
                b.paint(ACCENT);
                b.block(
                    v3(l(0.52), -r * 0.14, r * 0.98),
                    v3(l(0.7), r * 0.14, r * 1.06),
                );
            }
        });
    }
    b.mirror_y(|b| {
        let (y, z, pr) = (r * 0.72, r * 1.62, r * 0.36);
        b.paint(PLATING_DARK);
        b.cylinder_between(v3(l(-0.13), y, z), v3(l(0.22), y, z), pr, pr, b.sides(8));
        b.paint(METAL);
        for x in [l(-0.14), l(0.0), l(0.1), l(0.21)] {
            b.cylinder_between(
                v3(x, y, z),
                v3(x + l(0.012), y, z),
                pr * 1.08,
                pr * 1.08,
                b.sides(8),
            );
        }
        b.paint(GLOW);
        b.block(
            v3(l(-0.1), y - r * 0.04, z + pr * 0.94),
            v3(l(0.19), y + r * 0.04, z + pr * 1.02),
        );
        if b.fine() {
            b.paint(METAL);
            b.cylinder_between(
                v3(l(0.22), y, z),
                v3(l(0.34), y * 0.8, r * 1.3),
                r * 0.09,
                r * 0.09,
                6,
            );
            b.cylinder_between(
                v3(l(0.34), y * 0.8, r * 1.3),
                v3(l(0.44), r * 0.35, r * 0.75),
                r * 0.09,
                r * 0.09,
                6,
            );
        }
    });
}
