//! Fulgur: a long, faceted super-heavy hull with a centered turret and raised
//! upper-left induction cannon. Two compact AEB houses cover the flanks and
//! an independent rear-deck shatter projector defends against aircraft.

use glam::Vec3;

use super::parts::*;
use crate::models::builder::{MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

/// The main deck, on the raised centre tier: the turret race stands on it.
const DECK: f32 = 7.4;
/// The outer deck round the centre tier, over the tracks.
const OUTER_DECK: f32 = 6.4;
/// The AEB-2's muzzle, and where the barrel leaves the breech shroud.
const MUZZLE: Vec3 = Vec3::new(22.0, 2.5, 12.3);
const BREECH: Vec3 = Vec3::new(5.5, 2.5, 12.3);
/// Sponson house pivot (left; the right is its mirror) and muzzle.
const SPONSON: Vec3 = Vec3::new(9.0, 10.5, 7.2);
const SPONSON_MUZZLE: Vec3 = Vec3::new(15.5, 10.5, 7.8);
/// Top of the sponson a house stands on.
const SPONSON_TOP: f32 = 7.05;
/// Shatter house pivot and muzzle, and the engine deck it stands on.
const SHATTER: Vec3 = Vec3::new(-11.0, 0.0, 9.4);
const SHATTER_MUZZLE: Vec3 = Vec3::new(-5.5, 0.0, 10.2);
const ENGINE_DECK: f32 = 8.8;

const REAR: f32 = -19.0;
const FRONT: f32 = 19.5;
const HALF_WIDTH: f32 = 12.3;
/// Track inner and outer edge (y) and height.
const TRACK: (f32, f32, f32) = (6.8, 11.8, 4.2);

pub fn assault_tank(b: &mut MeshBuilder, _tech: u8) {
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
    b.block(v3(REAR + 0.8, -inner, 0.9), v3(FRONT - 0.9, inner, belt + 0.1));
    b.loft_z(&plan, &[Section::new(belt, 0.96), Section::new(band, 1.0)]);
    b.paint(PLATING);
    b.loft_z(&plan, &[Section::new(band, 0.98), Section::new(band + 0.4, 0.98), top]);
    // The centre tier the turret stands on: a second layer of plate, sloped all round.
    b.loft_z(
        &hull_plan(-9.2, 13.8, 7.4, 2.6),
        &[Section::new(OUTER_DECK - 0.1, 1.0), Section::scaled(DECK, 0.94, 0.88).shifted(0.2, 0.0)],
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

/// An independent compact electric-bore house on a faceted flank pedestal.
/// `side` is +1 for the left (weapon 1), −1 for the right (weapon 2).
fn sponson(b: &mut MeshBuilder, weapon: usize, side: f32) {
    let y = SPONSON.y * side;
    if b.coarse() {
        return;
    }
    // The box, sloped at its face, standing out over the front track.
    b.paint(PLATING);
    let (near, far) = if side > 0.0 { (8.0, 12.9) } else { (-12.9, -8.0) };
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
    b.prism(v3(SPONSON.x, y, SPONSON_TOP - 0.05), b.sides(10), 2.35, 2.3, 0.2);

    let pivot = v3(SPONSON.x, y, SPONSON.z);
    b.with_house(weapon, pivot, 0.25, |b| {
        // A low faceted house with a compact induction emitter.
        b.paint(PLATING);
        b.at(v3(8.7, y, 0.0), |b| {
            b.loft_z(
                &turret_plan(4.6, 3.8),
                &[
                    Section::new(SPONSON_TOP + 0.1, 0.9),
                    Section::new(7.95, 1.0),
                    Section::scaled(9.1, 0.7, 0.72).shifted(-0.3, 0.0),
                ],
            );
        });
        team_panel(b, v3(8.2, y, 9.1), v2(1.4, 1.5));
        b.with_recoil(|b| super::bore_tank::armored_bore(b, v3(10.5, y, SPONSON_MUZZLE.z), v3(SPONSON_MUZZLE.x, y, SPONSON_MUZZLE.z), 0.48));
        if b.fine() {
            b.paint(ACCENT);
            // Power feed at the rear of the compact bore.
            b.block(v3(6.2, y - 0.5, 7.5), v3(6.8, y + 0.5, 8.5));
            b.paint(GLASS);
            b.block(v3(9.55, y + 0.9 * side, 8.7), v3(9.75, y + 1.3 * side, 8.95));
        }
    });
}

/// The raised engine deck aft, its louvres, and the shatter house on it.
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
        b.plate(v3(-12.6, 4.1, ENGINE_DECK), v2(5.6, 2.4), 0.12, 0.04);
    });
    if b.fine() {
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(-15.6, 0.0, ENGINE_DECK), v2(1.2, 6.0), 0.08, 0.03);
        // Exhaust stacks at the tail corners, dark and hot inside.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(-16.2, 5.2, 7.0), v3(-16.6, 5.6, 9.6), 0.55, 0.5, 8);
        });
    }

    b.with_house(3, SHATTER, 0.55, |b| {
        b.paint(ACCENT);
        b.prism(v3(SHATTER.x, 0.0, ENGINE_DECK - 0.02), b.sides(10), 2.0, 1.8, 0.35);
        b.paint(PLATING);
        b.at(v3(SHATTER.x - 0.5, 0.0, 0.0), |b| {
            b.loft_z(&turret_plan(4.0, 3.3), &[
                Section::new(9.0, 1.0),
                Section::scaled(10.3, 0.7, 0.72).shifted(-0.35, 0.0),
            ]);
        });
        b.with_recoil(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(-11.4, 0.0, 10.2), SHATTER_MUZZLE, 0.65, 0.42, b.sides(6));
            for side in [-1.0, 1.0] {
                b.paint(PLATING);
                b.beam(v3(-10.3, side * 0.55, 10.3), v3(-6.1, side * 0.38, 10.2),
                    v2(0.45, 0.9), v2(0.25, 0.5));
                b.paint(GLOW);
                b.beam(v3(-9.4, side * 0.59, 10.25), v3(-6.3, side * 0.42, 10.2),
                    v2(0.08, 0.1), v2(0.06, 0.1));
            }
            b.paint(GLOW);
            b.cylinder_between(SHATTER_MUZZLE, SHATTER_MUZZLE + Vec3::X * 0.025, 0.25, 0.25, b.sides(6));
        });
    });
}

fn turret(b: &mut MeshBuilder) {
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_recoil(BREECH, MUZZLE, 1.2);
    b.set_arm_pivot(BREECH);
    b.with_part(part::TURRET, |b| {
        if b.coarse() {
            b.paint(PLATING);
            b.frustum_open(v3(-0.2, 0.0, DECK), v2(16.2, 11.6), v2(8.8, 3.2),
                12.9 - DECK, v2(0.2, 2.0));
            team_panel(b, v3(0.0, 2.0, 12.9), v2(6.0, 2.6));
            b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| super::bore_tank::armored_bore(b, BREECH, MUZZLE, 1.15));
            return;
        }
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, DECK - 0.05), b.sides(12), 6.4, 6.3, 0.35);
        b.paint(PLATING);
        // Restore the low, broad turret. Only a narrow left breech fairing
        // rises to the bore; the rest of the roof stays below the weapon.
        b.loft_z(&turret_plan(16.2, 11.6), &[
            Section::new(DECK + 0.2, 1.0),
            Section::scaled(10.4, 0.79, 0.72).shifted(-0.8, 0.0),
        ]);
        b.at(v3(0.0, 2.5, 0.0), |b| {
            b.paint(PLATING);
            b.extrude_y_chamfered(&[
                [-5.3, 9.7], [5.6, 9.7], [6.2, 11.6],
                [5.8, 12.9], [3.6, 13.25], [-2.8, 12.85], [-5.3, 10.8],
            ], 1.65, 0.42);
        });
        // An elevating receiver overlaps the sliding barrel throughout recoil.
        super::bore_tank::bore_socket(b, BREECH, 1.15, 1.2);
        team_panel(b, v3(-1.2, -1.6, 10.44), v2(6.0, 2.1));
        if b.fine() {
            b.paint(PLATING_DARK);
            b.at(v3(-2.5, -3.0, 0.0), |b| {
                b.loft_z(&turret_plan(6.0, 2.8), &[
                    Section::new(10.1, 1.0), Section::scaled(11.25, 0.8, 0.68),
                ]);
            });
            b.paint(GLOW);
            b.beam(v3(-3.6, -3.0, 11.28), v3(-0.9, -3.0, 11.28), v2(0.12, 0.08), v2(0.12, 0.08));
            b.paint(GLASS);
            b.chamfered_box(v3(3.5, -2.4, 10.65), v3(1.3, 1.0, 0.6), 0.2);
            antenna(b, v3(-5.6, -2.4, 10.0), 2.2, 0.2);
        }
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| super::bore_tank::armored_bore(b, BREECH, MUZZLE, 1.15));
    });
}
