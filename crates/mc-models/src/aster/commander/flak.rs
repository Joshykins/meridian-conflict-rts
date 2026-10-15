//! The ARC commander's shoulder flak cannon (the `aa` refit): an open mount at the back
//! of the right shoulder that aims at what it shoots, whatever the torso is doing. A low
//! yoke turns on a ring; between its arms the cradle pitches, a sloped gun shield across
//! its front and two slim barrels through it that fire together and kick back together.
//! Behind the shield each breech stands open with a clip of rounds in its guide.
//! Conventional guns: nothing is lit. The shoulder howitzer stands on the same trunnion
//! and ring (`refits.rs`): the model has one mount.
use glam::{Affine3A, Vec2, Vec3};

use crate::aster::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;

/// The shoulder mount's trunnion, toward the back of the right pauldron, low on it.
pub(super) const TRUNNION: Vec3 = Vec3::new(-0.75, -3.1, 13.95);
/// Where the flak barrels end (the unit file's muzzles, over 1.6), and their spacing
/// either side of the trunnion's line.
pub(super) const MUZZLE: f32 = TRUNNION.x + 3.05;
const GAP: f32 = 0.3;
/// The barrels' bore radius.
const BORE: f32 = 0.075;

/// The fixed part: the ring, the yoke and the shielded cradle, what goes on first.
pub(super) fn house(b: &mut MeshBuilder) {
    let t = TRUNNION;
    turntable(b, t);
    b.with_mount(t, 0.35, |b| {
        b.with_yoke(|b| shield_yoke(b, t));
        shield_cradle(b, t);
    });
}

/// The guns: what goes on last, kicking back when they fire.
pub(super) fn guns(b: &mut MeshBuilder) {
    let t = TRUNNION;
    b.with_mount(t, 0.35, |b| {
        b.with_recoil(|b| {
            for dy in [-GAP, GAP] {
                flak_barrel(b, v3(t.x + 0.75, t.y + dy, t.z), MUZZLE - t.x - 0.75, BORE);
            }
            breeches(b, t);
        })
    });
}

/// The fixed ring the yoke turns on: a dark race on the pauldron's cap, a gunmetal
/// bearing band round its top.
pub(super) fn turntable(b: &mut MeshBuilder, t: Vec3) {
    let c = v3(-0.2, t.y, 0.0);
    b.paint(ACCENT);
    b.cylinder_between(
        c + Vec3::Z * 13.15,
        c + Vec3::Z * 13.38,
        0.86,
        0.82,
        b.sides(12),
    );
    if b.coarse() {
        return;
    }
    b.paint(METAL);
    b.cylinder_between(
        c + Vec3::Z * 13.33,
        c + Vec3::Z * 13.4,
        0.88,
        0.88,
        b.sides(12),
    );
}

/// Plate on a face looking along -y (`out` -1) or +y (`out` 1): `center` on the face,
/// `size` along x and z.
fn side_plate(b: &mut MeshBuilder, center: Vec3, out: f32, size: Vec2, thick: f32) {
    let roll = Affine3A::from_rotation_x(-out * std::f32::consts::FRAC_PI_2);
    b.with(Affine3A::from_translation(center) * roll, |b| {
        b.plate(Vec3::ZERO, v2(size.x, size.y), thick, thick * 0.5)
    });
}

/// Plate on a face looking along +x: `size` along y and z.
fn front_plate(b: &mut MeshBuilder, center: Vec3, size: Vec2, thick: f32) {
    let turn = Affine3A::from_rotation_y(std::f32::consts::FRAC_PI_2);
    b.with(Affine3A::from_translation(center) * turn, |b| {
        b.plate(Vec3::ZERO, v2(size.y, size.x), thick, thick * 0.5)
    });
}

/// A flak barrel from `breech` forward `length`: a gunmetal tube, a white thermal
/// jacket over its back third with dark clamp bands, a finned cooling stretch, and a
/// round ported brake with a dark bore.
fn flak_barrel(b: &mut MeshBuilder, breech: Vec3, length: f32, r: f32) {
    let at = |x: f32| breech + Vec3::X * x;
    b.paint(METAL);
    if b.coarse() {
        b.beam(
            breech,
            at(length),
            Vec2::splat(r * 2.4),
            Vec2::splat(r * 2.0),
        );
        return;
    }
    let sides = b.sides(10);
    b.cylinder_between(breech, at(length), r * 1.1, r * 0.92, sides);
    let jacket = length * 0.36;
    b.paint(PLATING);
    b.cylinder_between(at(0.0), at(jacket), r * 1.75, r * 1.6, sides);
    b.paint(ACCENT);
    for x in [jacket * 0.25, jacket] {
        b.cylinder_between(at(x - 0.04), at(x + 0.04), r * 1.85, r * 1.85, sides);
    }
    let brake = (r * 3.6).min(length * 0.18);
    b.cylinder_between(at(length - brake), at(length), r * 1.5, r * 1.55, sides);
    b.cylinder_between(
        at(length - brake - 0.05),
        at(length - brake),
        r * 1.2,
        r * 1.5,
        sides,
    );
    b.paint(TREAD);
    b.cylinder_between(at(length - 0.01), at(length + 0.005), r * 0.7, r * 0.7, 8);
    if !b.fine() {
        return;
    }
    // Cooling fins ahead of the jacket.
    b.paint(METAL);
    for k in 0..5 {
        let x = jacket + 0.12 + 0.09 * k as f32;
        b.cylinder_between(at(x), at(x + 0.035), r * 1.4, r * 1.4, sides);
    }
    // The brake's gas ports, two rows each side.
    b.paint(TREAD);
    for k in 0..2 {
        let x = length - brake * (0.75 - 0.4 * k as f32);
        for s in [-1.0, 1.0] {
            b.block(
                at(x) + v3(-0.03, s * r * 1.5 - 0.012, -r * 0.55),
                at(x) + v3(0.03, s * r * 1.5 + 0.012, r * 0.55),
            );
        }
    }
}
/// The yoke: a low dark base and two narrow gunmetal arms to the trunnion.
fn shield_yoke(b: &mut MeshBuilder, t: Vec3) {
    b.paint(ACCENT);
    b.chamfered_box(t + v3(-0.2, 0.0, -0.48), v3(1.7, 1.6, 0.2), 0.3);
    b.paint(METAL);
    b.at(v3(0.0, t.y, 0.0), |b| {
        b.mirror_y(|b| {
            b.extrude_y(
                &[
                    [-1.1, t.z - 0.4],
                    [0.35, t.z - 0.4],
                    [0.24, t.z + 0.22],
                    [-0.3, t.z + 0.26],
                    [-1.1, t.z - 0.02],
                ],
                0.6,
                0.76,
            )
        })
    });
    // The trunnion hubs on the arms' outer faces.
    b.paint(ACCENT);
    b.cylinder_between(
        t - Vec3::Y * 0.84,
        t + Vec3::Y * 0.84,
        0.2,
        0.2,
        b.sides(10),
    );
    // The ready magazine between the arms behind the cradle, standing on the floor.
    b.paint(PLATING_DARK);
    b.chamfered_box(t + v3(-0.82, 0.0, -0.17), v3(0.5, 1.2, 0.42), 0.06);
    if !b.fine() {
        return;
    }
    b.paint(METAL);
    for s in [-1.0, 1.0] {
        b.cylinder_between(
            t + Vec3::Y * s * 0.84,
            t + Vec3::Y * s * 0.88,
            0.13,
            0.13,
            b.sides(10),
        );
    }
    b.paint(PLATING);
    for s in [-1.0, 1.0] {
        side_plate(b, t + v3(-0.55, s * 0.76, -0.18), s, v2(0.6, 0.3), 0.04);
    }
}

/// The cradle: a dark body round the trunnion and a sloped white shield across its
/// front with raised plates and a sight on its top, the guns through it.
fn shield_cradle(b: &mut MeshBuilder, t: Vec3) {
    // The cradle body round the trunnion between the yoke's arms, carrying the shield.
    b.paint(ACCENT);
    b.at(v3(0.0, t.y, 0.0), |b| {
        b.extrude_y(
            &[
                [-0.3, t.z - 0.26],
                [0.55, t.z - 0.34],
                [0.55, t.z + 0.3],
                [-0.18, t.z + 0.24],
            ],
            -0.58,
            0.58,
        )
    });
    b.paint(PLATING);
    b.at(v3(0.0, t.y, 0.0), |b| {
        b.extrude_y_chamfered(
            &[
                [0.45, t.z - 0.45],
                [0.72, t.z - 0.45],
                [0.82, t.z - 0.1],
                [0.6, t.z + 0.5],
                [0.36, t.z + 0.5],
                [0.48, t.z - 0.1],
            ],
            0.6,
            0.12,
        )
    });
    b.paint(ACCENT);
    for dy in [-0.3, 0.3] {
        b.cylinder_between(
            t + v3(0.7, dy, 0.0),
            t + v3(0.92, dy, 0.0),
            0.15,
            0.13,
            b.sides(10),
        );
    }
    if !b.fine() {
        return;
    }
    // Raised plates on the shield's face, either side of the guns and over them.
    b.paint(PLATING);
    for dy in [-0.5, 0.5] {
        front_plate(b, t + v3(0.79, dy, -0.2), v2(0.16, 0.4), 0.04);
    }
    front_plate(b, t + v3(0.74, 0.0, 0.25), v2(0.9, 0.22), 0.04);
    b.paint(PLATING_DARK);
    b.chamfered_box(t + v3(0.45, -0.42, 0.6), v3(0.36, 0.2, 0.2), 0.04);
    b.paint(GLASS);
    b.block(t + v3(0.63, -0.5, 0.55), t + v3(0.65, -0.34, 0.65));
}

/// Behind the shield: a dark breech casing on each gun with a clip of rounds stood
/// in its feed guide, kicking back with the gun.
fn breeches(b: &mut MeshBuilder, t: Vec3) {
    for dy in [-0.3, 0.3] {
        b.paint(ACCENT);
        b.chamfered_box(t + v3(0.1, dy, 0.0), v3(0.8, 0.22, 0.32), 0.05);
        if !b.fine() {
            continue;
        }
        b.paint(METAL);
        b.block(
            t + v3(-0.05, dy - 0.09, 0.16),
            t + v3(0.35, dy + 0.09, 0.46),
        );
        b.paint(PLATING_DARK);
        for k in 0..3 {
            let x = -0.02 + 0.12 * k as f32;
            b.block(
                t + v3(x, dy - 0.07, 0.46),
                t + v3(x + 0.08, dy + 0.07, 0.56),
            );
        }
    }
}
