//! Tempest: tech 2 flak battery. A heavy twin gun in a faceted house on a turning
//! pedestal, sunk in an octagonal emplacement with ready-round lockers round its
//! wall. The two long barrels fire together and kick back together; a stereo
//! rangefinder spans the house's roof and a fire-control dish stands on a post
//! behind it. Conventional guns: the bores stay dark, nothing is lit.
use super::*;
use crate::models::builder::{ngon, Section};
use glam::Vec3;

/// Trunnion height: the blueprint's gun pivot.
const TRUNNION: f32 = 5.0;
/// How far forward the guns hinge, inside the mantlet (the blueprint's pivot x).
const HINGE: f32 = 1.6;
/// Where the barrels end (the blueprint's muzzles), and their spacing either side.
const MUZZLE: f32 = 9.0;
const GAP: f32 = 0.7;
/// The pedestal's top, the house's floor.
const DECK: f32 = 3.3;

pub(super) fn build(b: &mut MeshBuilder) {
    emplacement(b);
    let z = TRUNNION;
    b.set_turret_pivot(v3(0.0, 0.0, z));
    b.set_arm_pivot(v3(HINGE, 0.0, z));
    b.set_recoil(v3(HINGE + 0.4, 0.0, z), v3(MUZZLE, 0.0, z), 0.8);
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let roof = turret_shell(b, 7.0, 5.6, DECK, 6.6);
        b.with_limb(rig::ARM_GUN, guns);
        if b.coarse() {
            return;
        }
        team_panel(
            b,
            roof.at(0.82, -0.45),
            v2(roof.length() * 0.25, roof.half_width * 0.6),
        );
        rangefinder(b, roof);
        fire_control(b, roof);
        if !b.fine() {
            return;
        }
        // Loader's hatch and a whip at the rear corner.
        let hatch = roof.at(0.82, 0.45);
        b.paint(ACCENT);
        b.plate(hatch, v2(0.9, 0.8), 0.07, 0.03);
        b.paint(METAL);
        b.block(hatch + v3(-0.5, -0.12, 0.0), hatch + v3(-0.42, 0.12, 0.14));
        antenna_unlit(b, roof.at(0.02, -0.85), 1.8, 0.2);
        // The shell hoist at the back of the house.
        b.paint(PLATING_DARK);
        b.block(v3(-3.3, -1.0, DECK + 0.3), v3(-2.6, 1.0, DECK + 1.9));
        b.paint(METAL);
        for y in [-0.55, 0.55] {
            b.block(
                v3(-3.35, y - 0.07, DECK + 0.25),
                v3(-2.6, y + 0.07, DECK + 1.95),
            );
        }
    });
}

/// The dug-in ring: a plinth, an octagonal wall with a dark top, lockers inside it,
/// and the pedestal the gun house turns on.
fn emplacement(b: &mut MeshBuilder) {
    if b.fine() {
        platform(b, 9.5);
    } else {
        b.paint(PLATING_DARK);
        if b.coarse() {
            b.decal(v3(0.0, 0.0, 1.3), v2(16.15, 16.15));
        } else {
            b.cuboid_open(v3(0.0, 0.0, 0.7), v3(16.15, 16.15, 1.2));
        }
    }
    let wall = ngon(8, 7.4);
    b.paint(PLATING);
    b.loft_z(
        &wall,
        &[Section::new(1.0, 1.0), Section::scaled(2.9, 0.95, 0.95)],
    );
    if !b.coarse() {
        b.paint(ACCENT);
        b.loft_z(
            &wall,
            &[
                Section::scaled(2.9, 0.95, 0.95),
                Section::scaled(3.05, 0.93, 0.93),
            ],
        );
    }
    if b.coarse() {
        return;
    }
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 1.0), b.sides(10), 2.6, 2.3, DECK - 1.0);
    // Ready-round lockers on the wall's four diagonals, lids banded.
    for i in 0..4 {
        let a = std::f32::consts::FRAC_PI_4 + i as f32 * std::f32::consts::FRAC_PI_2;
        let at = v3(a.cos() * 5.6, a.sin() * 5.6, 3.05);
        b.paint(PLATING_DARK);
        if b.fine() {
            b.chamfered_box(at + Vec3::Z * 0.35, v3(1.3, 1.3, 0.7), 0.12);
            b.paint(METAL);
            b.block(at + v3(-0.66, -0.08, 0.3), at + v3(0.66, 0.08, 0.74));
        } else {
            b.cuboid(at + Vec3::Z * 0.35, v3(1.3, 1.3, 0.7));
        }
    }
    // The turning ring round the pedestal's top.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, DECK - 0.2), b.sides(12), 2.75, 2.75, 0.2);
}

/// Two long barrels in one cradle: a wide cast mantlet, recuperators under each
/// barrel, white jackets, a yoke ahead of the mantlet and heavy slotted brakes.
fn guns(b: &mut MeshBuilder) {
    let z = TRUNNION;
    let r = 0.2;
    if b.coarse() {
        // From far off the pair are one flat slab.
        b.with_recoil(|b| {
            b.paint(METAL);
            b.beam(
                v3(HINGE, 0.0, z),
                v3(MUZZLE, 0.0, z),
                v2(2.0 * GAP + 0.5, 0.5),
                v2(2.0 * GAP + 0.4, 0.4),
            )
        });
        return;
    }
    b.paint(ACCENT);
    if b.fine() {
        b.chamfered_box(v3(HINGE + 0.35, 0.0, z), v3(1.1, 2.5, 1.3), 0.3);
        b.paint(METAL);
        b.mirror_y(|b| {
            b.cylinder_between(
                v3(HINGE + 0.8, GAP, z - 0.42),
                v3(HINGE + 2.6, GAP, z - 0.42),
                0.13,
                0.13,
                6,
            )
        });
    } else {
        b.cuboid(v3(HINGE + 0.35, 0.0, z), v3(1.1, 2.5, 1.3));
    }
    b.with_recoil(|b| {
        let sides = b.sides(8);
        b.mirror_y(|b| {
            b.paint(METAL);
            b.cylinder_between(
                v3(HINGE + 0.5, GAP, z),
                v3(MUZZLE - 1.0, GAP, z),
                r * 1.1,
                r * 0.9,
                sides,
            );
            b.paint(PLATING);
            b.cylinder_between(
                v3(HINGE + 0.6, GAP, z),
                v3(HINGE + 3.2, GAP, z),
                r * 1.9,
                r * 1.6,
                sides,
            );
            // The brake: a squared block with slots, the bore left dark.
            b.paint(ACCENT);
            let brake = (v3(MUZZLE - 0.5, GAP, z), v3(1.0, r * 3.0, r * 2.4));
            if !b.fine() {
                b.cuboid(brake.0, brake.1);
            } else {
                b.chamfered_box(brake.0, brake.1, r * 0.5);
                b.cylinder_between(
                    v3(HINGE + 4.4, GAP, z),
                    v3(HINGE + 4.6, GAP, z),
                    r * 1.35,
                    r * 1.35,
                    8,
                );
                b.paint(PLATING_DARK);
                for k in 0..3 {
                    let x = MUZZLE - 0.85 + 0.28 * k as f32;
                    b.block(
                        v3(x, GAP + r * 1.45, z - 0.14),
                        v3(x + 0.14, GAP + r * 1.55, z + 0.14),
                    );
                }
            }
        });
        // The yoke that keeps the pair in line.
        b.paint(ACCENT);
        b.chamfered_box(
            v3(HINGE + 3.5, 0.0, z),
            v3(0.35, 2.0 * GAP + 0.4, 0.35),
            0.08,
        );
    });
}

/// A stereo rangefinder across the house's rear roof: a long tube on two posts,
/// a lens hood at each end.
fn rangefinder(b: &mut MeshBuilder, roof: Roof) {
    let mid = roof.at(0.5, 0.0) + Vec3::Z * 0.45;
    let half = roof.half_width + 0.5;
    b.paint(PLATING_DARK);
    b.mirror_y(|b| b.block(mid + v3(-0.15, 0.6, -0.45), mid + v3(0.15, 0.8, -0.1)));
    b.paint(PLATING);
    b.cylinder_between(
        mid - Vec3::Y * half,
        mid + Vec3::Y * half,
        0.2,
        0.2,
        b.sides(8),
    );
    if b.mid() {
        b.paint(ACCENT);
        b.mirror_y(|b| b.cuboid(mid + v3(0.05, half, 0.0), v3(0.55, 0.35, 0.5)));
    }
    if b.fine() {
        b.paint(GLASS);
        b.mirror_y(|b| {
            b.block(
                mid + v3(0.33, half - 0.12, -0.12),
                mid + v3(0.35, half + 0.12, 0.12),
            )
        });
    }
}

/// The fire-control radar at the back of the roof: a dark cabin, a post and a dish
/// leaned back to look up along the guns' line.
fn fire_control(b: &mut MeshBuilder, roof: Roof) {
    let post = roof.at(0.12, 0.0);
    let head = post + Vec3::Z * 1.3;
    b.paint(PLATING_DARK);
    b.chamfered_box(post + Vec3::Z * 0.3, v3(0.8, 0.9, 0.6), 0.1);
    b.paint(METAL);
    b.cylinder_between(post + Vec3::Z * 0.6, head, 0.12, 0.1, b.sides(6));
    b.pitched(head, -0.5, |b| {
        b.paint(PLATING);
        b.cylinder_between(
            v3(-0.05, 0.0, 0.0),
            v3(0.15, 0.0, 0.0),
            0.8,
            0.6,
            b.sides(12),
        );
        if b.mid() {
            b.paint(ACCENT);
            b.cylinder_between(v3(0.15, 0.0, 0.0), v3(0.6, 0.0, 0.0), 0.06, 0.06, 5);
        }
    });
}
