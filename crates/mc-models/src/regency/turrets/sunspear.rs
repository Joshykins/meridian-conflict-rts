//! The Sunspear (tech 3 point defence, a 4 x 4 lot, the Citadel's size): a Pinch-fusion
//! Cannon. Like the Halberd it gathers its charge in front of the bore, but squeezes it
//! until it fuses, white before it goes, one great shot. The `muzzle` is the middle of
//! the charge.
//!
//! Two tall plated rails either side of a short bore reach past its mouth, gravity lenses
//! in heads at their tips aimed into the charge, and a stack of wound pinch coils narrows
//! down the bore between them. On the turret's back the fusion core stands caged: a red
//! star on a bronze post, held in three gimbal rings and a plated band, conduits running
//! to the trunnions; a radiator pack on either flank. It stands on a round platform
//! ringed by eight pylons.
//!
//! It works with its charge (`rig::CHARGE_GEAR_MASK`, entity.wgsl `charge_gear_pose`):
//! through the charge the rails part, the lens heads slide out along them into the
//! charge, the pinch coils light one after another from the breech, red going white, and
//! the gimbal cage spins up round the core, which lights first. The shot blinds every
//! light and kicks the bore back; the radiator lids stand up off glowing grilles, and as
//! the gun cools the coils fade through orange to an ember, the rails close and the lids
//! settle.
//!
//! Drawn at [`SCALE`] times the authored metres below. Its pivot and muzzle ([`LINE`]) are
//! the unit file's (`data/factions/regency/units/structures.ron`).

use std::f32::consts::FRAC_PI_4;

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::{HEAT_STAGE, REACH, SPIN, SPREAD, VENT};
use crate::material::*;
use crate::{part, pattern, rig};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::*;

/// The model's size over the metres it is authored in.
const SCALE: f32 = 1.4;
/// The gun's line as authored.
const AUTHORED: Line = Line::new(Vec3::new(0.0, 0.0, 13.6), Vec3::new(25.0, 0.0, 13.6));
/// The gun's line as drawn: the unit file's pivot and muzzle (tests hold them equal).
#[cfg(test)]
pub(super) const LINE: Line = Line::new(
    Vec3::new(0.0, 0.0, 13.6 * SCALE),
    Vec3::new(25.0 * SCALE, 0.0, 13.6 * SCALE),
);
/// How far round the charge its projectors stand (drawn): none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 4.0 * SCALE;
/// The foot of the core's cradle, authored (the cable runs from it to the trunnions).
pub(in crate::regency) const CORE: Vec3 = Vec3::new(-6.2, 0.0, 11.4);
/// The core above it.
const CORE_MID: Vec3 = Vec3::new(-6.2, 0.0, 15.8);

fn coarse_fusion(b: &mut MeshBuilder, line: &Line, base_r: f32, base_h: f32, mouth: f32) {
    coarse(
        b,
        line,
        &Coarse {
            base_r,
            base_h,
            x0: -10.0,
            x1: 6.0,
            half: 5.0,
            top: line.pivot.z + 3.0,
            gun: Vec2::splat(3.6),
            tip: Vec2::splat(2.0),
            mouth: Some((mouth, 2.6)),
        },
    );
}

/// The owner's colour round the top of a base at `z`, and the race the turret rides.
fn race(b: &mut MeshBuilder, z: f32, r: f32) {
    let segs = if b.fine() { 32 } else { 10 };
    b.paint(TEAM);
    hoop(b, Vec3::Z * (z + 0.02), r + 1.0, 1.4, 0.05, segs);
    metal(b);
    hoop(b, Vec3::Z * (z + 0.3), r - 0.4, 2.2, 0.6, segs);
}

pub(crate) fn sunspear(b: &mut MeshBuilder, _tech: u8) {
    b.with(Affine3A::from_scale(Vec3::splat(SCALE)), |b| {
        AUTHORED.rig(b, 1.8);
        if b.coarse() {
            coarse_fusion(b, &AUTHORED, 16.0, 7.0, 18.5);
            return;
        }
        b.set_charge_gear(CORE_MID, 1.0);
        ringed(b);
        b.with_part(part::TURRET, |b| {
            plan_hull(
                b,
                &[[4.0, 6.0], [-3.0, 7.0], [-9.0, 5.4], [-11.0, 2.0]],
                &[(7.6, 1.0), (9.2, 1.0), (11.0, 0.8), (11.6, 0.6)],
            );
            cheeks(b, [-3.0, 3.0], 4.6, [11.0, 15.6], 0.8);
            caged_core(b, CORE);
            radiators(b);
            b.with_limb(rig::ARM_GUN, |b| {
                gun_frame(b, &AUTHORED, |b| {
                    rails(b, AUTHORED.len());
                    pinch_coils(b);
                })
            });
        });
    });
}

/// A `GLOW_LASER` brush lit with the charge at `stage` (0 first, 6 last; `HEAT_STAGE` lit
/// by the shot instead): the Regency's plasma coil light.
pub(in crate::regency) fn coil_light(b: &mut MeshBuilder, stage: u32) {
    b.paint(GLOW_LASER).pattern(pattern::COIL + stage);
}

/// A round platform stepped up to a hub, ringed by eight plated pylons, each tied to the
/// hub by a bronze shaft and capped by a plate swept out into a spike.
fn ringed(b: &mut MeshBuilder) {
    let fine = b.fine();
    step(b, 8, 12.6, 12.0, 0.0, 2.0);
    seam(b);
    b.prism(Vec3::Z * 2.0, 8, 10.4, 10.4, 0.4);
    step(b, 8, 9.4, 8.4, 2.4, 4.6);
    for k in 0..8 {
        b.yawed(Vec3::ZERO, (22.5 + 45.0 * k as f32).to_radians(), |b| {
            b.at(v3(14.4, 0.0, 0.0), |b| step(b, 4, 2.0, 1.3, 0.0, 9.0));
            shaft(b, v3(13.2, 0.0, 6.8), v3(8.4, 0.0, 6.6), 0.4);
            if fine {
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(v3(13.4, 0.0, 8.6), v3(1.0, 0.0, 0.6), v3(-0.6, 0.0, 1.0)),
                    &swept(3.2, 1.4, 0.0, 0.5),
                    0.3,
                );
                slit(
                    b,
                    v3(15.55, 0.0, 6.0),
                    v3(1.0, 0.0, 0.08),
                    Vec3::Z,
                    1.2,
                    0.26,
                );
            }
        });
    }
    race(b, 7.0, 6.2);
}

/// The rails gun in its own frame: a wide breech, a short bronze bore through plated
/// sleeves, and either side a tall plated rail out past its mouth, plates lapped back
/// along its outer face, tied across the bore in bronze. At each rail's tip a plated head
/// sleeved over it carries two gravity lenses on its inner face. The bore recoils; the
/// rails part through the charge (`SPREAD`, the ties telescoping), and the heads slide
/// out along them into the charge (`REACH`).
pub(in crate::regency) fn rails(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    collar(b, Vec3::ZERO, Vec3::Y, 1.6, 10.6);
    dark_plate(b);
    hull_x(
        b,
        &[[-3.0, 7.0, 4.4, 0.0], [2.5, 7.0, 4.0, 0.0]],
        &CHAMFERED,
    );
    let ties: &[f32] = if fine { &[8.0, 15.0] } else { &[] };
    b.mirror_y(|b| {
        b.with_charge_gear(SPREAD, |b| {
            dark_plate(b);
            bar_through(
                b,
                &[
                    (v3(1.0, 3.2, 0.0), Vec2::new(1.4, 3.4)),
                    (v3(14.0, 3.6, 0.0), Vec2::new(1.2, 3.0)),
                    (v3(22.0, 4.0, 0.0), Vec2::new(1.0, 2.6)),
                    (v3(len + 0.2, 3.9, 0.0), Vec2::new(0.8, 2.0)),
                ],
                Vec3::Y,
            );
            Course {
                count: if fine { 3 } else { 1 },
                step: 6.0,
                len: 7.0,
                half: 1.2,
                tip: 0.0,
                thick: 0.3,
                tail: 1.5,
            }
            .lay(
                b,
                &Frame::new(v3(len - 4.0, 4.35, 0.0), v3(-1.0, 0.05, 0.0), Vec3::Y),
            );
            if fine {
                slit(b, v3(17.0, 3.04, 0.0), -Vec3::Y, Vec3::X, 4.0, 0.3);
            }
            // The ties' outer sleeves, riding out with the rail over their fixed middles.
            metal(b);
            for &x in ties {
                for z in [-1.3f32, 1.3] {
                    bar_through(
                        b,
                        &[
                            (v3(x, 1.6, z), Vec2::new(0.62, 0.62)),
                            (v3(x, 3.3, z), Vec2::new(0.62, 0.62)),
                        ],
                        Vec3::X,
                    );
                }
            }
        });
        b.with_charge_gear(REACH, |b| lens_head(b, len));
    });
    metal(b);
    for &x in ties {
        for z in [-1.3f32, 1.3] {
            bar_through(
                b,
                &[
                    (v3(x, -2.5, z), Vec2::new(0.5, 0.5)),
                    (v3(x, 2.5, z), Vec2::new(0.5, 0.5)),
                ],
                Vec3::X,
            );
        }
    }
    b.with_recoil(|b| {
        metal(b);
        b.cylinder_between(v3(2.5, 0.0, 0.0), v3(18.5, 0.0, 0.0), 1.1, 1.0, b.sides(10));
        for x in [5.5f32, 10.0, 14.5] {
            dark_plate(b);
            b.cylinder_between(
                v3(x, 0.0, 0.0),
                v3(x + 1.0, 0.0, 0.0),
                1.6,
                1.6,
                b.sides(10),
            );
        }
        dark_plate(b);
        b.cylinder_between(
            v3(17.0, 0.0, 0.0),
            v3(18.5, 0.0, 0.0),
            1.6,
            1.5,
            b.sides(10),
        );
        coil_light(b, 6);
        hoop_on(b, v3(18.45, 0.0, 0.0), Vec3::X, 1.1, 0.3, 0.12, b.sides(10));
    });
}

/// A rail's head (+y side): a plated sleeve over the rail's tip, overlapping it far enough
/// to stay on it when it slides out, its nose swept in toward the charge, two gravity
/// lenses on its inner face.
fn lens_head(b: &mut MeshBuilder, len: f32) {
    dark_plate(b);
    bar_through(
        b,
        &[
            (v3(len - 3.4, 3.95, 0.0), Vec2::new(1.3, 2.5)),
            (v3(len - 0.4, 3.9, 0.0), Vec2::new(1.3, 2.6)),
            (v3(len + 0.1, 3.75, 0.0), Vec2::new(0.9, 1.8)),
        ],
        Vec3::Y,
    );
    for z in [-0.8f32, 0.8] {
        lens(b, v3(len - 0.6, 3.2, z), v3(len, 0.0, 0.0), 0.62);
    }
}

/// A gravity lens at `at`, turned to `toward`: a bronze housing ringed in plate, a red
/// lens in its face, four bronze vanes round it.
pub(in crate::regency) fn lens(b: &mut MeshBuilder, at: Vec3, toward: Vec3, r: f32) {
    let d = (toward - at).normalize();
    let sides = b.sides(10);
    metal(b);
    b.cylinder_between(at - d * r * 0.9, at, r * 1.15, r, sides);
    dark_plate(b);
    hoop_on(b, at - d * r * 0.35, d, r * 1.2, r * 0.35, r * 0.4, sides);
    coil_light(b, 6);
    b.cylinder_between(at, at + d * 0.1, r * 0.72, r * 0.6, sides);
    if b.fine() {
        let side = d.cross(Vec3::Z).normalize();
        metal(b);
        for e in [Vec3::Z, -Vec3::Z, side, -side] {
            b.beam(
                at + e * r * 1.05 - d * r * 0.6,
                at + e * r * 0.9 + d * r * 0.35,
                Vec2::new(0.12, 0.3),
                Vec2::new(0.1, 0.18),
            );
        }
    }
}

/// The pinch coils in the gun's frame: a magnetic bottle down the bore, coils narrowing
/// toward the mouth, each wound in bronze, a red light inside each that comes on with
/// the charge, the coil nearest the breech first.
pub(in crate::regency) fn pinch_coils(b: &mut MeshBuilder) {
    let fine = b.fine();
    let coils: &[(f32, f32, u32)] = if fine {
        &[
            (7.4, 2.3, 1),
            (9.2, 2.15, 2),
            (11.9, 2.0, 3),
            (13.7, 1.85, 4),
            (16.0, 1.7, 5),
        ]
    } else {
        &[(9.2, 2.15, 2), (13.7, 1.85, 4)]
    };
    for &(x, r, stage) in coils {
        dark_plate(b);
        hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, r, 0.7, 0.8, b.sides(16));
        metal(b);
        hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, r + 0.4, 0.2, 0.55, b.sides(16));
        coil_light(b, stage);
        hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, r - 0.4, 0.12, 0.5, b.sides(16));
    }
}

/// The caged core over the turret's back at `c` (the cradle's foot): a red core held in
/// three bronze gimbal rings and a plated band, on a plated cradle in four bronze fingers,
/// conduits running forward to the trunnions either side.
pub(in crate::regency) fn caged_core(b: &mut MeshBuilder, c: Vec3) {
    let fine = b.fine();
    let mid = c + Vec3::Z * 4.4;
    dark_plate(b);
    b.cylinder_between(c - Vec3::Z * 0.4, c + Vec3::Z * 0.5, 3.0, 2.7, b.sides(12));
    shaft(b, c, mid - Vec3::Z * 1.2, 0.9);
    coil_light(b, 0);
    b.spheroid(
        mid,
        Vec3::splat(1.35),
        if fine { 12 } else { 8 },
        if fine { 8 } else { 5 },
    );
    // The gimbal cage spins up round the core through the charge.
    b.with_charge_gear(SPIN, |b| {
        metal(b);
        let segs = b.sides(18);
        for axis in [Vec3::Z, v3(1.0, 0.0, 0.9), v3(-1.0, 0.0, 0.9)] {
            hoop_on(b, mid, axis, 2.4, 0.35, 0.45, segs);
        }
        dark_plate(b);
        hoop_on(b, mid, Vec3::Y, 2.95, 0.45, 0.8, segs);
    });
    for k in 0..4 {
        let a = FRAC_PI_4 + std::f32::consts::FRAC_PI_2 * k as f32;
        let d = v3(a.cos(), a.sin(), 0.0);
        metal(b);
        bar_through(
            b,
            &[
                (c + d * 2.4 + Vec3::Z * 0.3, Vec2::new(0.8, 0.7)),
                (c + d * 3.2 + Vec3::Z * 3.4, Vec2::new(0.7, 0.6)),
                (mid + d * 2.0 + Vec3::Z * 1.8, Vec2::new(0.5, 0.4)),
            ],
            v3(-d.y, d.x, 0.0),
        );
    }
    if fine {
        b.mirror_y(|b| {
            metal(b);
            super::super::kit::cable(
                b,
                &[
                    c + v3(2.0, 1.6, 0.6),
                    c + v3(4.4, 3.0, 1.4),
                    v3(-1.6, 4.4, 12.6),
                ],
                0.38,
            );
        });
    }
}

/// A radiator pack on either flank of the turret's back: a plated box, a grille of bars
/// over a red-lit floor (`HEAT_STAGE`: dark until the shot) and three lids over it that
/// stand up off it with the heat after the shot (`VENT`) and settle as it cools.
fn radiators(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1, y0, y1, z0, z1) = (-9.6, -3.8, 3.4, 5.8, 8.6, 11.9);
    b.mirror_y(|b| {
        dark_plate(b);
        b.block(v3(x0, y0, z0), v3(x1, y1, z1 - 0.5));
        // The walls round the grille.
        for (a, c) in [
            (v3(x0, y0, z1 - 0.5), v3(x1, y0 + 0.3, z1)),
            (v3(x0, y1 - 0.3, z1 - 0.5), v3(x1, y1, z1)),
            (v3(x0, y0, z1 - 0.5), v3(x0 + 0.3, y1, z1)),
            (v3(x1 - 0.3, y0, z1 - 0.5), v3(x1, y1, z1)),
        ] {
            b.block(a, c);
        }
        coil_light(b, HEAT_STAGE);
        b.face(&[
            v3(x0 + 0.3, y0 + 0.3, z1 - 0.35),
            v3(x1 - 0.3, y0 + 0.3, z1 - 0.35),
            v3(x1 - 0.3, y1 - 0.3, z1 - 0.35),
            v3(x0 + 0.3, y1 - 0.3, z1 - 0.35),
        ]);
        if fine {
            metal(b);
            let bars = 7;
            for k in 0..bars {
                let x = x0 + 0.6 + (x1 - x0 - 1.2) * k as f32 / (bars - 1) as f32;
                b.block(
                    v3(x - 0.08, y0 + 0.3, z1 - 0.3),
                    v3(x + 0.08, y1 - 0.3, z1 - 0.12),
                );
            }
        }
        b.with_charge_gear(VENT, |b| {
            let lids = 3;
            let w = (x1 - x0) / lids as f32;
            for k in 0..lids {
                let a = x0 + w * k as f32;
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(v3(a + 0.1, (y0 + y1) * 0.5, z1), Vec3::X, Vec3::Z),
                    &[
                        [0.0, -(y1 - y0) * 0.5],
                        [0.0, (y1 - y0) * 0.5],
                        [w - 0.2, (y1 - y0) * 0.5],
                        [w - 0.2, -(y1 - y0) * 0.5],
                    ],
                    0.25,
                );
            }
        });
        if fine {
            slit(
                b,
                v3((x0 + x1) * 0.5, y1 + 0.02, z0 + 1.0),
                Vec3::Y,
                Vec3::X,
                4.0,
                0.3,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sunspear_holds_its_charge() {
        super::super::super::check_charge(
            "regency_fusion_cannon",
            24.0,
            24.0,
            Some(4),
            &[LINE.muzzle.to_array()],
            HOLD,
        );
    }

    /// The line as drawn is the authored one at the model's scale.
    #[test]
    fn sunspear_line_is_drawn_at_scale() {
        assert!(LINE.pivot.distance(AUTHORED.pivot * SCALE) < 1e-4);
        assert!(LINE.muzzle.distance(AUTHORED.muzzle * SCALE) < 1e-4);
    }
}
