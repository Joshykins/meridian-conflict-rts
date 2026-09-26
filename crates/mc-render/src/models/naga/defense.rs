//! The Naga's defences, each on a one-cell lot (12 m square, cut to an octagon): the Barb
//! (point defence), the Thornspitter (flak) and the Thornwall (wall section).
//!
//! - Barb: a squat armoured drum skirted in plates lapped down into spikes, a bronze race,
//!   and a low wedge of a turret with its plates lapped back, levelling a heavy repeater:
//!   three bronze barrels in one plated shroud, cooling rings, pressure vents either side
//!   and a muzzle brake with its rim glowing red. A brawler, low and heavy.
//! - Thornspitter: a ribbed bronze column braced by four plated legs, and on it a flak
//!   mount: a drum magazine across the back between armoured cheeks, twin barrels raised
//!   at the sky, a red sensor slit over them. From above it is an X under a cross.
//! - Thornwall: an armoured octagonal block on a bronze waist, a glacis plate on each
//!   face and a plate at each corner lapped up into a spike. It keeps inside the lot's
//!   octagon, so sections side by side meet across the middle of their faces.
//!
//! The numbers match `data/factions/naga/units/structures.ron`: a turret's pivot is the
//! weapon's `pivot` and its gun's tip the `muzzle`. Barrels recoil when they fire.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, rig};

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::*;

const BARB_PIVOT: Vec3 = Vec3::new(0.0, 0.0, 6.0);
const BARB_MUZZLE: Vec3 = Vec3::new(5.2, 0.0, 6.8);
const SPIT_PIVOT: Vec3 = Vec3::new(0.0, 0.0, 6.2);
const SPIT_MUZZLE: Vec3 = Vec3::new(4.4, 0.0, 7.4);
const THICK: f32 = 0.3;

/// A point on the line from `pivot` to `muzzle`, `x` metres out along x.
fn bore(pivot: Vec3, muzzle: Vec3, x: f32) -> Vec3 {
    pivot + (muzzle - pivot) * (x / (muzzle.x - pivot.x))
}

/// A skirt of `count` plates round a drum, each from `r0` at height `z0` lapped down and
/// out to `r1` near the ground, its spike pointing down.
pub(super) fn skirt(b: &mut MeshBuilder, count: usize, r0: f32, z0: f32, r1: f32, half: f32) {
    for k in 0..count {
        let a = std::f32::consts::TAU * (k as f32 + 0.5) / count as f32;
        let d = v3(a.cos(), a.sin(), 0.0);
        let top = d * r0 + Vec3::Z * z0;
        let foot = d * r1 + Vec3::Z * 0.35;
        let f = Frame::new(top, foot - top, d + Vec3::Z * 0.4);
        dark_plate(b);
        Course {
            count: 1,
            step: 0.0,
            len: (foot - top).length() * 0.8,
            half,
            tip: 0.0,
            thick: THICK,
            tail: (foot - top).length() * 0.2,
        }
        .lay(b, &f);
    }
}

// ---- Barb: point defence ------------------------------------------------------------

pub(super) fn barb(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(BARB_PIVOT);
    b.set_arm_pivot(BARB_PIVOT);
    b.set_recoil(BARB_PIVOT, BARB_MUZZLE, 0.5);
    if b.coarse() {
        barb_coarse(b);
        return;
    }
    barb_base(b);
    b.with_part(part::TURRET, |b| {
        barb_head(b);
        b.with_limb(rig::ARM_GUN, barb_gun);
    });
}

/// Far off: the drum, the turret's wedge, the gun in one bar, the owner's colour.
fn barb_coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, 5.4, 3.4, 3.6);
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.beam(
            v3(-2.6, 0.0, 5.8),
            v3(1.6, 0.0, 5.8),
            Vec2::new(3.6, 3.0),
            Vec2::new(2.2, 2.2),
        );
        b.paint(TEAM);
        b.face(&[
            v3(-2.2, -0.9, 7.32),
            v3(-0.6, -0.9, 7.32),
            v3(-0.6, 0.9, 7.32),
            v3(-2.2, 0.9, 7.32),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            b.beam(
                bore(BARB_PIVOT, BARB_MUZZLE, 1.2),
                BARB_MUZZLE,
                Vec2::new(1.0, 0.9),
                Vec2::new(0.5, 0.5),
            );
        });
    });
}

/// The drum: an armoured octagon skirted in plates, the bronze race on top, the owner's
/// colour round it.
fn barb_base(b: &mut MeshBuilder) {
    let fine = b.fine();
    seam(b);
    b.prism(Vec3::ZERO, 8, 4.2, 3.6, 3.4);
    skirt(b, if fine { 8 } else { 4 }, 3.5, 3.2, 5.3, 1.35);
    metal(b);
    hoop(b, Vec3::Z * 3.7, 2.9, 0.9, 0.6, if fine { 16 } else { 6 });
    b.paint(TEAM);
    hoop(
        b,
        Vec3::Z * 3.42,
        3.55,
        0.35,
        0.05,
        if fine { 16 } else { 6 },
    );
    if fine {
        seam(b);
        teeth(b, Vec3::Z * 3.7, 3.3, 20, v3(0.4, 0.35, 0.5));
        for k in 0..4 {
            let a = (45.0 + 90.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            red_slot(b, d * 4.0 + Vec3::Z * 1.6, d, v3(-d.y, d.x, 0.0), 0.9, 0.18);
        }
    }
}

/// The turret: a low wedge on the race, plates lapped back over it into spikes, the
/// owner's colour on its roof, trunnion cheeks either side of the gun.
fn barb_head(b: &mut MeshBuilder) {
    dark_plate(b);
    let ring = |z: f32, grow: f32| -> Vec<Vec3> {
        vec![
            v3(2.0 + grow, -1.3 - grow, z),
            v3(2.0 + grow, 1.3 + grow, z),
            v3(-1.2, 2.2 + grow, z),
            v3(-2.9 - grow, 1.1 + grow, z),
            v3(-2.9 - grow, -1.1 - grow, z),
            v3(-1.2, -2.2 - grow, z),
        ]
    };
    b.loft(
        &[ring(4.0, 0.0), ring(6.2, 0.0), ring(7.1, -0.5)],
        true,
        true,
    );
    // Its plates: one on the roof, one down each flank, lapped back into spikes.
    let roof = Frame::new(v3(1.6, 0.0, 7.12), v3(-1.0, 0.0, -0.08), Vec3::Z);
    Course {
        count: 2,
        step: 1.6,
        len: 2.6,
        half: 1.3,
        tip: 0.0,
        thick: THICK,
        tail: 0.9,
    }
    .lay(b, &roof);
    b.mirror_y(|b| {
        let f = Frame::new(v3(1.2, 2.1, 6.3), v3(-1.0, 0.25, -0.1), v3(0.0, 1.0, 0.35));
        dark_plate(b);
        Course {
            count: 2,
            step: 1.7,
            len: 2.6,
            half: 1.0,
            tip: -1.0,
            thick: THICK,
            tail: 1.0,
        }
        .lay(b, &f);
        dark_plate(b);
        b.block(v3(-0.5, 0.95, 5.1), v3(1.4, 1.35, 6.9));
    });
    b.paint(TEAM);
    b.face(&[
        v3(-2.3, -0.7, 7.13),
        v3(-1.0, -0.7, 7.13),
        v3(-1.0, 0.7, 7.13),
        v3(-2.3, 0.7, 7.13),
    ]);
}

/// The repeater: three bronze barrels in a plated shroud with cooling rings, pressure
/// vents either side, a muzzle brake whose rim glows red. The barrels and brake recoil.
fn barb_gun(b: &mut MeshBuilder) {
    let fine = b.fine();
    let at = |x: f32| bore(BARB_PIVOT, BARB_MUZZLE, x);
    let dir = (BARB_MUZZLE - BARB_PIVOT).normalize();
    let side = Vec3::Y;
    let up = side.cross(dir).normalize();
    // The trunnion across the cheeks.
    collar(b, BARB_PIVOT, Vec3::Y, 0.55, 1.9);
    dark_plate(b);
    b.cylinder_between(at(0.4), at(3.2), 0.8, 0.7, b.sides(8));
    if fine {
        for x in [1.2f32, 2.0, 2.8] {
            collar(b, at(x), dir, 0.84, 0.25);
        }
        // The pressure vents either side of the shroud.
        b.mirror_y(|b| {
            dark_plate(b);
            b.beam(
                at(2.3) + side * 0.8,
                at(3.1) + side * 0.85,
                Vec2::new(0.5, 0.45),
                Vec2::new(0.5, 0.35),
            );
        });
    }
    b.with_recoil(|b| {
        metal(b);
        for k in 0..3 {
            let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k as f32 / 3.0;
            let off = (side * a.cos() + up * a.sin()) * 0.26;
            b.cylinder_between(at(2.9) + off, at(4.75) + off, 0.17, 0.17, 6);
        }
        dark_plate(b);
        b.cylinder_between(at(4.55), BARB_MUZZLE, 0.55, 0.48, b.sides(8));
        // The brake's rim, hot from the last burst, and the bores through it.
        b.paint(GLOW_LASER);
        hoop_on(
            b,
            at(4.95),
            dir,
            0.53,
            0.08,
            0.12,
            if fine { 12 } else { 6 },
        );
        metal(b);
        b.cylinder_between(at(4.9), BARB_MUZZLE + dir * 0.02, 0.2, 0.2, 6);
    });
}

// ---- Thornspitter: anti-air flak ----------------------------------------------------

pub(super) fn spitter(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(SPIT_PIVOT);
    b.set_arm_pivot(SPIT_PIVOT);
    b.set_recoil(SPIT_PIVOT, SPIT_MUZZLE, 0.45);
    if b.coarse() {
        spitter_coarse(b);
        return;
    }
    spitter_base(b);
    b.with_part(part::TURRET, |b| {
        spitter_head(b);
        b.with_limb(rig::ARM_GUN, spitter_guns);
    });
}

/// Far off: the column on its legs, the mount, the barrels in one bar, the owner's colour.
fn spitter_coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, 4.9, 1.2, 5.0);
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.beam(
            v3(-2.2, 0.0, 6.4),
            v3(1.0, 0.0, 6.4),
            Vec2::new(3.4, 2.4),
            Vec2::new(2.4, 2.0),
        );
        b.paint(TEAM);
        b.face(&[
            v3(-1.8, -0.8, 7.62),
            v3(-0.2, -0.8, 7.62),
            v3(-0.2, 0.8, 7.62),
            v3(-1.8, 0.8, 7.62),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            b.beam(
                bore(SPIT_PIVOT, SPIT_MUZZLE, 1.0),
                SPIT_MUZZLE,
                Vec2::new(1.2, 0.7),
                Vec2::new(0.5, 0.5),
            );
        });
    });
}

/// The column: ribbed bronze on a plated foot, braced by four legs on the diagonals,
/// each plated and lapped down into a spike, the owner's colour on the foot.
fn spitter_base(b: &mut MeshBuilder) {
    let fine = b.fine();
    seam(b);
    b.prism(Vec3::ZERO, 8, 2.6, 2.2, 1.4);
    ribbed(
        b,
        Vec3::Z * 1.4,
        Vec3::Z * 4.8,
        1.0,
        if fine { 3 } else { 0 },
    );
    metal(b);
    hoop(b, Vec3::Z * 4.95, 1.6, 1.2, 0.5, if fine { 16 } else { 6 });
    if fine {
        seam(b);
        teeth(b, Vec3::Z * 4.95, 2.2, 16, v3(0.35, 0.3, 0.45));
    }
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f32).to_radians();
        b.yawed(Vec3::ZERO, a, |b| {
            let (root, foot) = (v3(1.0, 0.0, 4.2), v3(4.6, 0.0, 0.4));
            let down = (foot - root).normalize();
            ribbed(b, root, foot, 0.32, if fine { 2 } else { 0 });
            let f = Frame::new(root + Vec3::Z * 0.55, down, v3(-down.z, 0.0, down.x));
            dark_plate(b);
            Course {
                count: if fine { 2 } else { 1 },
                step: 1.6,
                len: 2.6,
                half: 0.8,
                tip: 0.0,
                thick: THICK,
                tail: 0.9,
            }
            .lay(b, &f);
            dark_plate(b);
            b.block(v3(4.0, -0.7, 0.0), v3(5.2, 0.7, 0.8));
            if fine {
                piston(b, v3(4.6, 0.0, 0.8), v3(3.0, 0.0, 2.2), 0.22, false);
            }
        });
    }
    b.paint(TEAM);
    hoop(b, Vec3::Z * 1.42, 2.3, 0.4, 0.05, if fine { 16 } else { 6 });
}

/// The mount: a yoke on the column, the drum magazine across its back between armoured
/// cheeks lapped back into spikes, a red sensor slit, the owner's colour on the drum.
fn spitter_head(b: &mut MeshBuilder) {
    let fine = b.fine();
    dark_plate(b);
    b.prism(Vec3::Z * 5.2, 8, 1.7, 1.5, 0.8);
    // The drum magazine across the back.
    let drum = v3(-1.4, 0.0, 6.6);
    dark_plate(b);
    b.cylinder_between(
        drum - Vec3::Y * 1.4,
        drum + Vec3::Y * 1.4,
        1.0,
        1.0,
        b.sides(10),
    );
    collar(b, drum - Vec3::Y * 1.45, Vec3::Y, 0.8, 0.2);
    collar(b, drum + Vec3::Y * 1.45, Vec3::Y, 0.8, 0.2);
    b.paint(TEAM);
    b.face(&[
        v3(-1.8, -0.8, 7.62),
        v3(-1.0, -0.8, 7.62),
        v3(-1.0, 0.8, 7.62),
        v3(-1.8, 0.8, 7.62),
    ]);
    // Armoured cheeks either side of the barrels.
    b.mirror_y(|b| {
        let f = Frame::new(v3(1.2, 1.25, 7.0), v3(-1.0, 0.3, -0.25), v3(0.0, 1.0, 0.25));
        dark_plate(b);
        Course {
            count: 2,
            step: 1.3,
            len: 2.4,
            half: 1.0,
            tip: -1.0,
            thick: THICK,
            tail: 1.0,
        }
        .lay(b, &f);
        dark_plate(b);
        b.block(v3(-0.6, 0.7, 5.6), v3(1.2, 1.1, 7.2));
    });
    if fine {
        red_slot(b, v3(1.22, 0.0, 7.5), Vec3::X, Vec3::Y, 1.0, 0.16);
    }
}

/// Twin barrels raised at the sky, each a plated jacket over a bronze tube, joined by a
/// trunnion; the tubes recoil.
fn spitter_guns(b: &mut MeshBuilder) {
    let fine = b.fine();
    let at = |x: f32| bore(SPIT_PIVOT, SPIT_MUZZLE, x);
    let dir = (SPIT_MUZZLE - SPIT_PIVOT).normalize();
    collar(b, SPIT_PIVOT, Vec3::Y, 0.5, 1.4);
    for y in [-0.36f32, 0.36] {
        let off = Vec3::Y * y;
        dark_plate(b);
        b.cylinder_between(at(0.3) + off, at(2.6) + off, 0.34, 0.3, b.sides(8));
        if fine {
            collar(b, at(1.6) + off, dir, 0.38, 0.2);
        }
        b.with_recoil(|b| {
            metal(b);
            b.cylinder_between(at(2.5) + off, at(4.2) + off, 0.17, 0.17, 6);
            dark_plate(b);
            b.cylinder_between(at(4.1) + off, SPIT_MUZZLE + off * 0.2, 0.26, 0.24, 6);
        });
    }
}

// ---- Thornwall -----------------------------------------------------------------------

/// An octagon about the middle at height `z`: the square of half side `half` with its
/// corners cut where `|x| + |y|` passes `cut`.
fn octagon(half: f32, cut: f32, z: f32) -> Vec<Vec3> {
    let c = cut - half;
    vec![
        v3(half, -c, z),
        v3(half, c, z),
        v3(c, half, z),
        v3(-c, half, z),
        v3(-half, c, z),
        v3(-half, -c, z),
        v3(-c, -half, z),
        v3(c, -half, z),
    ]
}

pub(super) fn thornwall(b: &mut MeshBuilder, _tech: u8) {
    let fine = b.fine();
    // The block: an octagon cut like the lot, a footing, a bronze waist, the armour.
    seam(b);
    b.loft(
        &[octagon(6.0, 8.6, 0.0), octagon(5.7, 8.2, 1.0)],
        false,
        true,
    );
    if !b.coarse() {
        metal(b);
        b.loft(
            &[octagon(5.2, 7.5, 1.0), octagon(5.2, 7.5, 1.8)],
            false,
            false,
        );
    }
    dark_plate(b);
    if b.coarse() {
        b.loft(
            &[octagon(5.5, 7.9, 1.0), octagon(3.4, 4.8, 4.4)],
            false,
            true,
        );
    } else {
        b.loft(
            &[
                octagon(5.5, 7.9, 1.8),
                octagon(4.9, 7.0, 3.6),
                octagon(3.4, 4.8, 4.4),
            ],
            false,
            true,
        );
    }
    if fine {
        // Bronze ribs standing in the waist.
        metal(b);
        for k in 0..8 {
            let a = (22.5 + 45.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            b.beam(
                d * 5.35 + Vec3::Z * 0.9,
                d * 5.35 + Vec3::Z * 1.9,
                Vec2::new(0.5, 0.4),
                Vec2::new(0.5, 0.4),
            );
        }
    }
    if !b.coarse() {
        // A glacis plate on each face, and a plate at each corner lapped up into a spike.
        for k in 0..4 {
            let a = (90.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            let f = Frame::new(
                d * 5.55 + Vec3::Z * 1.9,
                d * -1.2 + Vec3::Z * 2.3,
                d + Vec3::Z * 0.45,
            );
            dark_plate(b);
            armour(
                b,
                &f,
                &[[0.0, -2.6], [0.0, 2.6], [2.3, 2.0], [2.3, -2.0]],
                THICK,
            );
        }
        for k in 0..if fine { 4 } else { 0 } {
            let a = (45.0 + 90.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            let f = Frame::new(
                d * 5.3 + Vec3::Z * 1.9,
                d * -0.55 + Vec3::Z * 2.6,
                d + Vec3::Z * 0.2,
            );
            dark_plate(b);
            Course {
                count: 1,
                step: 0.0,
                len: 2.0,
                half: 1.1,
                tip: 0.0,
                thick: THICK,
                tail: 1.6,
            }
            .lay(b, &f);
        }
    }
    b.paint(TEAM);
    b.face(&[
        v3(-1.0, -1.0, 4.42),
        v3(1.0, -1.0, 4.42),
        v3(1.0, 1.0, 4.42),
        v3(-1.0, 1.0, 4.42),
    ]);
}

#[cfg(test)]
mod tests {
    #[test]
    fn barb_fits() {
        super::super::check("naga_barb", 5.5, 8.0, Some(1), &[[5.2, 0.0, 6.8]]);
    }

    #[test]
    fn spitter_fits() {
        super::super::check("naga_spitter", 5.5, 8.5, Some(1), &[[4.4, 0.0, 7.4]]);
    }

    #[test]
    fn thornwall_fits() {
        super::super::check("naga_thornwall", 6.0, 5.0, Some(1), &[]);
    }
}
