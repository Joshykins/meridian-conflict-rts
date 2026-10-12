//! What the Sarissa's turret turns on, up to the plinth's top (`PLINTH`): a plinth stepped up in
//! three tiers, each an eight-sided course of dark plate with a seam-dark band over it,
//! plated buttresses climbing its four faces, and on each corner of the lowest tier a
//! heat-sink pylon: a plated tower with fins down its faces, a lit grille in its crown
//! and lids over the grille that stand up with the heat after a burn
//! (`charge_gear::VENT`).

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::{HEAT_STAGE, VENT};
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{armour, Course, Frame};
use super::super::super::parts::{inlay, ridged};
use super::super::fusion_kit::coil_light;
use super::super::{buttress, step};
use super::PLINTH;

/// The tiers: (foot radius, top radius, foot height, top height).
const TIERS: [(f32, f32, f32, f32); 3] = [
    (38.0, 35.5, 0.0, 5.0),
    (32.0, 29.5, 5.6, 12.5),
    (25.0, 23.4, 13.1, PLINTH),
];
/// How far out the pylons stand on the corners, and their half width.
const PYLON_R: f32 = 30.5;
const PYLON: f32 = 3.6;
/// The pylons' crown.
const PYLON_TOP: f32 = 17.0;

pub(super) fn draw(b: &mut MeshBuilder) {
    let fine = b.fine();
    for (k, &(r0, r1, z0, z1)) in TIERS.iter().enumerate() {
        step(b, 8, r0, r1, z0, z1 - z0);
        if k + 1 < TIERS.len() {
            seam(b);
            b.prism(Vec3::Z * z1, 8, r1 - 0.6, r1 - 0.8, 0.6);
        }
    }
    // A graphite ring round the deck the race sits in.
    metal(b);
    b.prism(Vec3::Z * (PLINTH - 0.4), 16, 22.6, 22.4, 0.5);
    for k in 0..4 {
        b.yawed(Vec3::ZERO, (90.0 * k as f32).to_radians(), |b| {
            face(b, fine)
        });
        b.yawed(Vec3::ZERO, (45.0 + 90.0 * k as f32).to_radians(), pylon);
    }
}

/// One face of the plinth, along +x: a plated buttress climbing from the lowest tier's lip
/// to the deck, feathered plates lapped down it, a ridged plate on each tier's face either
/// side of it and a lit slot under each.
fn face(b: &mut MeshBuilder, fine: bool) {
    buttress(b, 21.0, 6.0, PLINTH - 1.0, 35.0, 3.6, 5.4);
    if !b.mid() {
        return;
    }
    let slope = (PLINTH - 1.0 - 5.4) / (35.0 - 21.0);
    dark_plate(b);
    Course {
        count: if fine { 4 } else { 2 },
        step: 3.4,
        len: 4.2,
        half: 1.6,
        tip: 0.0,
        thick: 0.35,
        tail: 1.6,
    }
    .lay(
        b,
        &Frame::new(
            v3(23.0, 0.0, PLINTH - 1.0 - slope * 2.0 + 1.0),
            v3(1.0, 0.0, -slope),
            v3(slope, 0.0, 1.0),
        ),
    );
    for &(r0, r1, z0, z1) in &TIERS[..2] {
        // The tier's face plane at this bearing: an eight-sided prism with a flat to +x.
        let apothem = |r: f32| r * (std::f32::consts::PI / 8.0).cos();
        let (a0, a1) = (apothem(r0), apothem(r1));
        let h = z1 - z0;
        let out = v3(h, 0.0, a0 - a1).normalize();
        b.mirror_y(|b| {
            let y = a1 * 0.24;
            let c = v3((a0 + a1) * 0.5 + 0.05, y + 4.4, z0 + h * 0.5);
            dark_plate(b);
            ridged(
                b,
                &Frame::new(c - v3(0.0, 0.0, h * 0.32), Vec3::Z, out),
                &[[0.0, -4.2], [h * 0.64, -3.8], [h * 0.64, 3.8], [0.0, 4.2]],
                0.3,
                [h * 0.32, 0.0],
                0.3,
            );
            if fine {
                let foot = z0 + 0.6;
                let x = a0 - (a0 - a1) * 0.6 / h + 0.04;
                inlay(
                    b,
                    &[
                        v3(x, y + 1.0, foot),
                        v3(x, y + 7.8, foot),
                        v3(x - 0.02, y + 7.8, foot + 0.32),
                        v3(x - 0.02, y + 1.0, foot + 0.32),
                    ],
                    out,
                    pattern::EMBER_SLOT,
                );
            }
        });
    }
}

/// A heat-sink pylon on the corner along +x: a plated tower standing on the lowest tier,
/// stepped in at its waist, fins down its outer faces, a lit grille in its crown under
/// four lids that stand up with the heat, a swept plate over its outer shoulder and a
/// plated strut back to the second tier.
fn pylon(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (z0, waist) = (TIERS[0].3, 11.0);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                square(PYLON_R, PYLON + 0.6, z0 - 0.4),
                square(PYLON_R, PYLON + 0.6, waist),
                square(PYLON_R, PYLON, waist + 0.6),
                square(PYLON_R, PYLON, PYLON_TOP - 0.5),
            ],
            false,
            true,
        )
    });
    seam(b);
    b.loft(
        &[
            square(PYLON_R, PYLON + 0.75, waist - 0.2),
            square(PYLON_R, PYLON + 0.75, waist + 0.2),
        ],
        false,
        true,
    );
    // The grille in its crown, lit by the heat of a burn.
    coil_light(b, HEAT_STAGE);
    b.face(&square(PYLON_R, PYLON - 0.5, PYLON_TOP - 0.45));
    b.with_charge_gear(VENT, |b| {
        let w = (PYLON * 2.0 - 0.6) / 4.0;
        for k in 0..4 {
            let x = PYLON_R - PYLON + 0.3 + w * k as f32;
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3(x + 0.1, 0.0, PYLON_TOP - 0.45), Vec3::X, Vec3::Z),
                &[
                    [0.0, -PYLON + 0.2],
                    [0.0, PYLON - 0.2],
                    [w - 0.5, PYLON - 0.2],
                    [w - 0.2, 0.0],
                    [w - 0.5, -PYLON + 0.2],
                ],
                0.4,
            );
        }
    });
    // A strut back to the second tier.
    dark_plate(b);
    b.beam(
        v3(PYLON_R - PYLON - 0.2, 0.0, waist - 1.0),
        v3(TIERS[1].1 * 0.92 - 1.0, 0.0, TIERS[1].3 - 1.0),
        glam::Vec2::new(2.4, 2.0),
        glam::Vec2::new(2.0, 1.6),
    );
    if !b.mid() {
        return;
    }
    // A swept plate over its outer shoulder.
    dark_plate(b);
    armour(
        b,
        &Frame::new(
            v3(PYLON_R + PYLON + 0.2, 0.0, PYLON_TOP - 1.4),
            v3(0.25, 0.0, -1.0),
            Vec3::X,
        ),
        &super::super::super::machine::swept(7.0, 2.6, 0.0, 0.45),
        0.4,
    );
    if !fine {
        return;
    }
    // Fins down its outer face and its flanks, and a slot in each over the waist.
    for (n, side) in [(Vec3::X, Vec3::Y), (Vec3::Y, Vec3::X), (-Vec3::Y, Vec3::X)] {
        let face = v3(PYLON_R, 0.0, 0.0) + n * (PYLON + 0.6);
        let slot = v3(PYLON_R, 0.0, 0.0) + n * PYLON;
        dark_plate(b);
        for k in 0..5 {
            let t = -PYLON + 0.9 + (PYLON * 2.0 - 1.8) * k as f32 / 4.0;
            let c = face + side * t;
            let (lo, hi) = (
                c + v3(0.0, 0.0, z0 + 1.0) - side * 0.16,
                c + n * 0.5 + v3(0.0, 0.0, waist - 1.2) + side * 0.16,
            );
            b.block(lo.min(hi), lo.max(hi));
        }
        inlay(
            b,
            &[
                slot + side * (-PYLON + 0.8) + v3(0.0, 0.0, waist + 1.4),
                slot + side * (PYLON - 0.8) + v3(0.0, 0.0, waist + 1.4),
                slot + side * (PYLON - 0.8) + v3(0.0, 0.0, waist + 1.75),
                slot + side * (-PYLON + 0.8) + v3(0.0, 0.0, waist + 1.75),
            ],
            n,
            pattern::EMBER_SLOT,
        );
    }
}

/// A square ring about (`x`, 0) of half width `half` at height `z`.
fn square(x: f32, half: f32, z: f32) -> Vec<Vec3> {
    vec![
        v3(x - half, -half, z),
        v3(x + half, -half, z),
        v3(x + half, half, z),
        v3(x - half, half, z),
    ]
}
