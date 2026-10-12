//! The pinch-fusion pieces the Regency's fusion guns share (the Skyspear and the fusion
//! howitzer, the Springald, the Rondel, the Flamberge's gun houses, the strategic
//! launchers): the prism coil light, the parting rails with their gravity-lens heads, the
//! pinch coils and the caged core. They were the first Sunspear's; its redesign
//! (`sunspear`) uses none of them.

use std::f32::consts::FRAC_PI_4;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::{REACH, SPIN, SPREAD};
use crate::material::*;
use crate::pattern;

use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::*;
use super::*;

/// The foot of the core's cradle on the tech 3 mobile fusion guns' turrets, at the size
/// those guns are authored (`fusion_guns`).
pub(in crate::regency) const CORE: Vec3 = Vec3::new(-6.2, 0.0, 11.4);

/// A `GLOW_PRISM` brush lit with the charge at `stage` (0 first, 6 last; `HEAT_STAGE` lit
/// by the shot instead): pinch fusion's coil light in a power generator's star's colours
/// (entity.wgsl), not the red of the Regency's plasma guns.
pub(in crate::regency) fn coil_light(b: &mut MeshBuilder, stage: u32) {
    b.paint(GLOW_PRISM).pattern(pattern::COIL + stage);
}

/// The rails gun in its own frame: a wide breech, a short bronze bore through plated
/// sleeves, and either side a tall plated rail out past its mouth, plates lapped back
/// along its outer face, tied across the bore in bronze. At each rail's tip a plated head
/// sleeved over it carries two gravity lenses on its inner face. The bore recoils; the
/// rails part through the charge (`SPREAD`, the ties telescoping), and the heads slide
/// out along them into the charge (`REACH`). With `slide`, the rails and their heads, and
/// the bore, are a siege gun's sliding pieces (`MeshBuilder::with_siege`), and nothing ties
/// the rails across the bore.
pub(in crate::regency) fn rails(b: &mut MeshBuilder, len: f32, slide: bool) {
    let fine = b.fine();
    collar(b, Vec3::ZERO, Vec3::Y, 1.6, 10.6);
    dark_plate(b);
    hull_x(
        b,
        &[[-3.0, 7.0, 4.4, 0.0], [2.5, 7.0, 4.0, 0.0]],
        &CHAMFERED,
    );
    let ties: &[f32] = if fine && !slide { &[8.0, 15.0] } else { &[] };
    b.mirror_y(|b| {
        if slide {
            b.with_siege(0, |b| rail(b, len, ties));
        } else {
            rail(b, len, ties);
        }
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
    if slide {
        b.with_siege(0, bore);
    } else {
        bore(b);
    }
}

/// The bore, recoiling: a bronze tube through plated sleeves, lit at its mouth.
fn bore(b: &mut MeshBuilder) {
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

/// One rail (+y side) with its head: the rail parts through the charge (`SPREAD`), its
/// ties' outer sleeves (at `ties`) riding out with it, and the head slides out (`REACH`).
fn rail(b: &mut MeshBuilder, len: f32, ties: &[f32]) {
    let fine = b.fine();
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

/// A gravity lens at `at`, turned to `toward`: a bronze housing ringed in plate, a lens
/// of fusion light in its face, four bronze vanes round it.
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
/// toward the mouth, each wound in bronze, a fusion light inside each that comes on with
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

/// The caged core over the turret's back at `c` (the cradle's foot): a star core held in
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
