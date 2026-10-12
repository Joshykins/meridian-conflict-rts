//! The Sarissa's gun, in the gun's frame (origin at the trunnion, +x down the bore): a
//! breech jacket round the trunnion, and out of it a split bore: a core rod wound with
//! pinch coils between two plated flank rails, and over and under it a jaw, a broad plated
//! half-shell closing on the bore toward the mouth. Through the charge the jaws part up and
//! down off the bore (`charge_gear::PART`), the slots in their edges rippling toward the
//! lens; they stay open through the burn and slam shut when it ends. The lance leaves
//! from [`LINE`]'s muzzle, held between gravity lenses on the jaws' tips, and the core
//! runs back into the jacket while it burns (`with_recoil`).

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::PART;
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{armour, hoop_on, swept, Course, Frame};
use super::super::super::parts::{drum, inlay, ridged};
use super::super::fusion_kit::{coil_light, lens};
use super::super::{bar_through, hull_x, CHAMFERED};
use super::{CHEEK, LINE};

/// Out from the bore at bearing `a` (0 along +y, a quarter turn straight up).
fn radial(a: f32) -> Vec3 {
    v3(0.0, a.cos(), a.sin())
}

/// Round the bore at bearing `a`, the way the bearing turns.
fn round(a: f32) -> Vec3 {
    v3(0.0, -a.sin(), a.cos())
}

/// `x` down the bore and `r` out from it at bearing `a`.
fn at(x: f32, a: f32, r: f32) -> Vec3 {
    v3(x, 0.0, 0.0) + radial(a) * r
}

/// The jacket's front, where the core slides out of it.
const MOUTH: f32 = 12.0;
/// How far the lens heads stand off the bore at the muzzle.
const HEAD_R: f32 = 2.1;

pub(super) fn draw(b: &mut MeshBuilder) {
    jacket(b);
    let len = LINE.len();
    b.with_recoil(|b| core(b, len - 2.4));
    for side in [1.0f32, -1.0] {
        rail(b, side);
        b.with_charge_gear(PART, |b| jaw(b, side, len));
    }
}

/// The breech jacket round the trunnion: a plated block, the gun's thickest part, short
/// behind the trunnion so it can sink into the turret's roof as the gun rises, a ridged
/// plate lapped back over its top and plates down its flanks swept back into spikes, a
/// lit gasket in its back.
fn jacket(b: &mut MeshBuilder) {
    drum(b, Vec3::ZERO, Vec3::Y, 3.0, (CHEEK - 0.1) * 2.0);
    dark_plate(b);
    b.with_facets(|b| {
        hull_x(
            b,
            &[
                [-6.0, 8.0, 6.4, 0.0],
                [-3.6, 10.6, 9.4, 0.0],
                [6.0, 11.4, 10.4, 0.0],
                [MOUTH, 10.6, 10.0, 0.0],
            ],
            &CHAMFERED,
        )
    });
    seam(b);
    hoop_on(b, v3(MOUTH, 0.0, 0.0), Vec3::X, 4.4, 1.8, 0.7, b.sides(16));
    if !b.mid() {
        return;
    }
    dark_plate(b);
    ridged(
        b,
        &Frame::new(v3(MOUTH + 0.2, 0.0, 5.0), -Vec3::X, Vec3::Z),
        &[
            [0.0, -4.0],
            [0.0, 4.0],
            [9.0, 3.8],
            [15.6, 1.2],
            [15.6, -1.2],
            [9.0, -3.8],
        ],
        0.4,
        [6.0, 0.0],
        0.55,
    );
    b.mirror_y(|b| {
        dark_plate(b);
        armour(
            b,
            &Frame::new(v3(MOUTH + 0.2, 5.72, 1.0), -Vec3::X, Vec3::Y),
            &[
                [0.0, -2.0],
                [0.0, 2.8],
                [9.0, 2.6],
                [15.0, 0.8],
                [9.0, -1.8],
            ],
            0.4,
        );
    });
    if b.fine() {
        inlay(
            b,
            &[
                v3(-6.02, -2.6, -1.8),
                v3(-6.02, 2.6, -1.8),
                v3(-6.02, 2.6, 1.8),
                v3(-6.02, -2.6, 1.8),
            ],
            -Vec3::X,
            pattern::EMBER_RIM,
        );
        b.mirror_y(|b| {
            inlay(
                b,
                &[
                    v3(MOUTH - 1.6, 5.98, -2.6),
                    v3(0.0, 5.98, -2.6),
                    v3(0.0, 5.98, -2.1),
                    v3(MOUTH - 1.6, 5.98, -2.1),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        });
    }
}

/// The pinch coils down the core: (x, radius, charge stage).
const CORE_COILS: [(f32, f32, u32); 6] = [
    (20.0, 2.4, 1),
    (26.0, 2.3, 2),
    (32.0, 2.2, 3),
    (38.0, 2.1, 4),
    (44.0, 2.0, 5),
    (50.0, 1.9, 6),
];

/// The core out of the jacket's mouth to `end`: a graphite rod wound with pinch coils that
/// light stage by stage through the charge, a lens on its end looking down the bore.
fn core(b: &mut MeshBuilder, end: f32) {
    let segs = b.sides(16);
    let fine = b.fine();
    metal(b);
    b.cylinder_between(
        v3(MOUTH - 1.0, 0.0, 0.0),
        v3(end - 0.8, 0.0, 0.0),
        1.6,
        1.1,
        b.sides(10),
    );
    for (i, &(x, r, stage)) in CORE_COILS.iter().enumerate() {
        if !fine && i % 2 == 1 {
            continue;
        }
        let c = Vec3::X * x;
        dark_plate(b);
        hoop_on(b, c, Vec3::X, r, 1.0, 1.4, segs);
        metal(b);
        hoop_on(b, c, Vec3::X, r + 0.6, 0.3, 1.0, segs);
        coil_light(b, stage);
        hoop_on(b, c, Vec3::X, r - 0.6, 0.2, 0.9, segs);
    }
    lens(b, v3(end, 0.0, 0.0), v3(end + 5.0, 0.0, 0.0), 1.1);
}

/// The lens head on a tip at bearing `a`: a plated collar, a swept plate over it and a
/// gravity lens on its inner face looking at the muzzle.
fn head(b: &mut MeshBuilder, a: f32, len: f32) {
    let x = len - 2.6;
    let r = HEAD_R + 0.9;
    dark_plate(b);
    bar_through(
        b,
        &[
            (at(x - 1.2, a, r), Vec2::new(1.8, 1.8)),
            (at(x + 1.0, a, r), Vec2::new(1.6, 1.6)),
        ],
        round(a),
    );
    if b.mid() {
        armour(
            b,
            &Frame::new(at(x + 1.2, a, r + 0.9), -Vec3::X, radial(a)),
            &swept(4.2, 0.9, 0.0, 0.5),
            0.24,
        );
    }
    lens(b, at(len - 1.2, a, HEAD_R), v3(len, 0.0, 0.0), 0.55);
}

/// A plated rail along one flank of the split bore (`side` +y or -y), from the jacket to
/// the mouth, tying the coils' lugs; it stays put while the jaws part.
fn rail(b: &mut MeshBuilder, side: f32) {
    let y = side * 3.6;
    dark_plate(b);
    b.with_facets(|b| {
        bar_through(
            b,
            &[
                (v3(MOUTH, y, 0.0), Vec2::new(2.6, 3.6)),
                (v3(40.0, y * 0.95, 0.0), Vec2::new(2.0, 2.8)),
                (v3(52.0, y * 0.85, 0.0), Vec2::new(1.4, 2.0)),
            ],
            Vec3::Z,
        )
    });
    if b.fine() {
        inlay(
            b,
            &[
                v3(MOUTH + 2.0, y + side * 1.32, -0.3),
                v3(38.0, y * 0.95 + side * 1.02, -0.3),
                v3(38.0, y * 0.95 + side * 1.02, 0.3),
                v3(MOUTH + 2.0, y + side * 1.32, 0.3),
            ],
            Vec3::Y * side,
            pattern::EMBER_SLOT,
        );
    }
}

/// One jaw of the split bore, above the coils (`side` 1) or below (-1), all of it charge
/// gear: a broad plated half-shell from the jacket to the mouth, closing on the bore as it
/// goes, feathered plates down its back, lit slots in its edges and a lens head at its
/// tip.
fn jaw(b: &mut MeshBuilder, side: f32, len: f32) {
    let fine = b.fine();
    let up = Vec3::Z * side;
    let a = if side > 0.0 { TAU / 4.0 } else { -TAU / 4.0 };
    // (x, half width, inner face off the bore, depth)
    let stations: [(f32, f32, f32, f32); 4] = [
        (MOUTH + 0.5, 4.6, 2.9, 2.6),
        (36.0, 4.0, 2.7, 2.2),
        (48.0, 3.0, 2.6, 1.8),
        (len - 1.6, 1.4, HEAD_R + 0.4, 1.2),
    ];
    let rings: Vec<Vec<Vec3>> = stations
        .iter()
        .map(|&(x, half, inner, depth)| {
            [
                (-half, inner),
                (half, inner),
                (half * 0.8, inner + depth),
                (0.0, inner + depth * 1.25),
                (-half * 0.8, inner + depth),
            ]
            .iter()
            .map(|&(y, z)| v3(x, y, 0.0) + up * z)
            .collect()
        })
        .collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
    head(b, a, len);
    if !b.mid() {
        return;
    }
    dark_plate(b);
    b.mirror_y(|b| {
        Course {
            count: if fine { 5 } else { 3 },
            step: 5.4,
            len: 6.2,
            half: 1.1,
            tip: 0.4,
            thick: 0.3,
            tail: 1.6,
        }
        .lay(
            b,
            &Frame::new(
                v3(46.0, 1.4, 0.0) + up * 4.6,
                -Vec3::X + up * 0.01,
                up + Vec3::Y * 0.3,
            ),
        );
    });
    if fine {
        b.mirror_y(|b| {
            inlay(
                b,
                &[
                    v3(MOUTH + 3.0, 4.52, 0.0) + up * 3.2,
                    v3(46.0, 3.25, 0.0) + up * 2.8,
                    v3(46.0, 3.25, 0.0) + up * 3.3,
                    v3(MOUTH + 3.0, 4.52, 0.0) + up * 3.8,
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        });
        for x in [24.0f32, 40.0] {
            metal(b);
            b.block(
                (v3(x - 0.4, -4.5, 0.0) + up * 2.7).min(v3(x + 0.4, 4.5, 0.0) + up * 5.4),
                (v3(x - 0.4, -4.5, 0.0) + up * 2.7).max(v3(x + 0.4, 4.5, 0.0) + up * 5.4),
            );
        }
    }
}
