//! Fulgur: a long, faceted super-heavy hull with a broad centred turret and its
//! AEB-2 set back between armoured cheeks. A bolt rifle on each flank sponson, and on
//! the engine deck a rotary AA gun that rests facing aft, raised to the sky.

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
/// The AA gun's house on the engine deck: its pivot is the gun's trunnion. Its muzzle
/// as authored facing the nose and level (the house rests turned aft, `facing: 180`).
const AA: Vec3 = Vec3::new(-13.5, 0.0, 9.55);
/// The Sparrow's rotary gun at this scale, its barrels this long in its own units.
const AA_SCALE: f32 = 0.9;
const AA_LENGTH: f32 = 5.5;
const AA_MUZZLE: Vec3 = Vec3::new(AA.x + AA_LENGTH * AA_SCALE, 0.0, AA.z);
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
        track_unit(b, -18.6, -1.1);
        track_unit(b, 1.1, 18.2);
        // The drive between them: a dark final-drive housing.
        b.paint(ACCENT);
        b.block(v3(-1.3, inner + 0.3, 1.2), v3(1.3, outer - 0.6, 3.4));
    });
}

/// One open track unit on the +y side, from `x0` to `x1`: the belt round a sprocket at
/// each end, road wheels along its lower run and return rollers under its upper run, all
/// seen from the flank through the open side.
fn track_unit(b: &mut MeshBuilder, x0: f32, x1: f32) {
    let (inner, outer, h) = TRACK;
    // Where the wheels begin: the belt's far half is solid behind them.
    let face = outer - 1.0;
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD);
        let lozenge = [
            [x0 + 0.5 * h, 0.0],
            [x1 - 0.5 * h, 0.0],
            [x1, 0.5 * h],
            [x1 - 0.22 * h, h],
            [x0 + 0.22 * h, h],
            [x0, 0.5 * h],
        ];
        b.with_profile_bevel(0.09 * h, |b| b.extrude_y(&lozenge, inner, face));
        // The near half: the lower and upper runs, and the belt round each end.
        b.extrude_y(
            &[
                [x0 + 0.45 * h, 0.0],
                [x1 - 0.45 * h, 0.0],
                [x1 - 0.45 * h, 0.17 * h],
                [x0 + 0.45 * h, 0.17 * h],
            ],
            face,
            outer,
        );
        b.extrude_y(
            &[
                [x0 + 0.3 * h, 0.84 * h],
                [x1 - 0.3 * h, 0.84 * h],
                [x1 - 0.3 * h, h],
                [x0 + 0.3 * h, h],
            ],
            face,
            outer,
        );
        for (end, dir) in [(x1, 1.0), (x0, -1.0)] {
            let back = end - dir * 0.55 * h;
            let mut cap = vec![
                [back, 0.0],
                [end - dir * 0.5 * h, 0.0],
                [end, 0.5 * h],
                [end - dir * 0.22 * h, h],
                [back, h],
            ];
            if dir < 0.0 {
                cap.reverse();
            }
            b.with_profile_bevel(0.09 * h, |b| b.extrude_y(&cap, face, outer));
            sprocket(b, v3(end - dir * 0.46 * h, outer, 0.5 * h), 0.36 * h);
        }
        // Road wheels on the lower run.
        let (first, last) = (x0 + 0.95 * h, x1 - 0.95 * h);
        let count = (((last - first) / (0.62 * h)).round() as usize + 1).max(2);
        for i in 0..count {
            let x = first + (last - first) * i as f32 / (count - 1) as f32;
            road_wheel(b, v3(x, outer, 0.4 * h), 0.27 * h, outer - face);
        }
        if b.fine() {
            // Return rollers under the upper run.
            b.paint(METAL);
            for f in [0.25, 0.5, 0.75] {
                let x = x0 + (x1 - x0) * f;
                b.cylinder_between(
                    v3(x, face, 0.77 * h),
                    v3(x, outer - 0.25, 0.77 * h),
                    0.07 * h,
                    0.07 * h,
                    8,
                );
            }
        }
    });
}

/// A road wheel on the +y side, its outer face at `center.y`: a dark rim over a light
/// disc, and a hub cap.
fn road_wheel(b: &mut MeshBuilder, center: Vec3, r: f32, width: f32) {
    let at = |y: f32| v3(center.x, center.y + y, center.z);
    b.paint(ACCENT);
    b.cylinder_between(at(-width + 0.05), at(-0.1), r, r, b.sides(12));
    b.paint(METAL);
    b.cylinder_between(at(-0.12), at(-0.02), r * 0.78, r * 0.72, b.sides(12));
    if b.fine() {
        b.paint(PLATING_DARK);
        b.cylinder_between(at(-0.03), at(0.12), r * 0.32, r * 0.24, 8);
    }
}

/// A toothed drive sprocket or idler on the +y side, its outer face at `center.y`.
fn sprocket(b: &mut MeshBuilder, center: Vec3, r: f32) {
    let at = |y: f32| v3(center.x, center.y + y, center.z);
    b.paint(METAL);
    b.cylinder_between(at(-0.1), at(0.1), r, r, b.sides(12));
    b.paint(ACCENT);
    b.cylinder_between(at(0.05), at(0.3), r * 0.42, r * 0.3, b.sides(8));
    if b.fine() {
        b.paint(METAL);
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::TAU / 8.0;
            let (c, s) = (a.cos(), a.sin());
            b.with(
                Affine3A::from_translation(at(0.0) + v3(c, 0.0, s) * r * 1.05)
                    * Affine3A::from_rotation_y(-a),
                |b| b.cuboid(Vec3::ZERO, v3(r * 0.2, 0.18, r * 0.16)),
            );
        }
    }
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
            // Three plates a run, hung over the upper run so the wheels show under them.
            let step = (x1 - x0) / 3.0;
            for k in 0..3 {
                let a = x0 + step * k as f32 + 0.08;
                b.paint(PLATING);
                b.block(
                    v3(a, outer + 0.05, 2.9),
                    v3(a + step - 0.16, outer + 0.4, belt + 0.05),
                );
                b.paint(ACCENT);
                b.block(
                    v3(a + 0.1, outer + 0.38, 2.9),
                    v3(a + step - 0.26, outer + 0.44, 3.05),
                );
            }
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

/// The raised engine deck aft, its two exhaust vents, and the AA gun on it.
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
        // The engine's two exhaust vents, either side of the turret: louvres with the fire
        // breathing between the slats, and the hot air shimmering over them.
        b.paint(ACCENT).pattern(pattern::FURNACE);
        b.plate(v3(-11.0, 4.1, ENGINE_DECK), v2(4.4, 2.4), 0.12, 0.04);
        b.pattern(pattern::PLAIN);
        if b.fine() {
            b.add_exhaust(v3(-11.0, 4.1, ENGINE_DECK + 0.15), v3(-0.3, 0.0, 2.6), 1.2);
        }
    });
    if b.fine() {
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(-16.2, 0.0, ENGINE_DECK), v2(0.8, 6.0), 0.08, 0.03);
    }
    aa_gun(b);
}

/// The rotary AA gun, the Sparrow's gatling, built facing the nose: a collar on the deck,
/// a low faceted house, and a yoke on it that the gun's receiver sits in, pitching on
/// its trunnion. The house rests turned aft and the gun raised to the sky (the sim's idle
/// pitch for a land AA gun), so only the house and yoke have to be low enough for the
/// main gun to pass over.
fn aa_gun(b: &mut MeshBuilder) {
    b.with_house(3, AA, 0.2, |b| {
        b.paint(ACCENT);
        b.prism(
            v3(AA.x, 0.0, ENGINE_DECK - 0.02),
            b.sides(10),
            1.9,
            1.8,
            0.3,
        );
        b.paint(PLATING);
        b.at(v3(AA.x - 0.3, 0.0, 0.0), |b| {
            b.loft_z(
                &turret_plan(3.8, 3.3),
                &[
                    Section::new(ENGINE_DECK + 0.2, 1.0),
                    Section::scaled(9.25, 0.84, 0.8).shifted(-0.2, 0.0),
                ],
            );
        });
        // The yoke: a block on the house round the trunnion, its arms either side of
        // the receiver, the trunnion's pin through them.
        b.paint(PLATING_DARK);
        b.chamfered_box(v3(AA.x - 0.55, 0.0, 9.35), v3(1.9, 1.9, 0.5), 0.12);
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(
                &[
                    [AA.x - 1.1, 9.2],
                    [AA.x + 0.55, 9.2],
                    [AA.x + 0.4, 9.85],
                    [AA.x - 0.25, 9.98],
                    [AA.x - 1.0, 9.7],
                ],
                0.55,
                0.8,
            );
            b.paint(METAL);
            b.cylinder_between(v3(AA.x, 0.8, AA.z), v3(AA.x, 0.9, AA.z), 0.22, 0.22, 8);
        });
        if b.fine() {
            team_panel(b, v3(AA.x - 1.55, 0.0, 9.25), v2(0.7, 1.2));
        }
        b.with_recoil(|b| {
            b.with(
                Affine3A::from_translation(v3(AA.x, 0.0, AA.z))
                    * Affine3A::from_scale(Vec3::splat(AA_SCALE)),
                |b| super::aa::rotary(b, 0.0, AA_LENGTH),
            );
            // The front clamp's hub, the bore line's end.
            b.paint(PLATING_DARK);
            b.cylinder_between(v3(AA_MUZZLE.x - 0.1, 0.0, AA.z), AA_MUZZLE, 0.12, 0.12, 8);
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
