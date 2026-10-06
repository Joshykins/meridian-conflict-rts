//! A Pinch-plasma Battery: a low faceted gun house on a graphite ring, its roof a pale
//! plate, carrying two forks side by side. Each fork is a pair of blades, one over the
//! other, with a red-lit slot between them at the root: plasma is gathered and
//! squeezed in the gap ahead of the blades' tips, then pinched out as one jet (the
//! Halberd's way, at a warship's size). The weapon's muzzles are those gaps.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::GLOW_LASER;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::hoop_on;
use super::pale_plate;

/// The house's size against the authored one.
pub(super) const SIZE: f32 = 1.9;
/// Each fork's charge point ahead of the pivot (authored, before `SIZE`), port fork.
const GAP: Vec3 = Vec3::new(21.0, 3.6, 0.0);
/// How far below the pivot the house's ring sits on the deck.
pub(super) const RISE: f32 = 6.0 * SIZE;

/// Where a battery at `pivot` gathers its plasma: the port fork's gap, then starboard's.
#[cfg(test)]
pub(super) fn muzzles(pivot: Vec3) -> [Vec3; 2] {
    let g = GAP * SIZE;
    [pivot + g, pivot + v3(g.x, -g.y, g.z)]
}

/// A battery bound to `weapon`, turning about `pivot`; its ring stands on the deck
/// `RISE` below.
pub(super) fn battery(b: &mut MeshBuilder, weapon: usize, pivot: Vec3) {
    b.with_house(weapon, pivot, 1.6 * SIZE, |b| {
        b.with(
            glam::Affine3A::from_translation(pivot) * glam::Affine3A::from_scale(Vec3::splat(SIZE)),
            house,
        );
    });
}

/// The house, authored about its pivot at the origin, facing +x.
fn house(b: &mut MeshBuilder) {
    let base = -6.0;
    let sides = b.sides(12);
    // The ring it turns on: graphite, a dark seam under its lip.
    metal(b);
    b.prism(v3(-1.0, 0.0, base - 1.5), sides, 10.5, 10.0, 3.0);
    // The house: a long hexagon, its prow pointed, its sides raked in.
    let plan: Vec<[f32; 2]> = vec![
        [10.0, 0.0],
        [6.0, 8.5],
        [-9.0, 9.0],
        [-12.5, 5.0],
        [-12.5, -5.0],
        [-9.0, -9.0],
        [6.0, -8.5],
    ];
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(base + 1.5, 1.0),
                Section::new(base + 5.5, 1.0),
                Section::new(base + 8.5, 0.8),
            ],
        )
    });
    // The roof: a pale plate, inset.
    pale_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(base + 8.3, 0.74),
                Section::new(base + 9.3, 0.62),
            ],
        )
    });
    if !b.coarse() {
        // Its optic: a red slit across the prow's upper face.
        b.paint(GLOW_LASER);
        b.with_facets(|b| {
            b.loft_z(
                &[[8.3, 1.6], [7.6, 3.4], [7.1, 3.4], [7.8, 1.6]],
                &[
                    Section::new(base + 6.0, 1.0).shifted(-0.2, 0.0),
                    Section::new(base + 6.6, 1.0).shifted(-0.55, 0.0),
                ],
            )
        });
        if b.fine() {
            // A graphite vent block on the roof's tail.
            metal(b);
            b.chamfered_box(v3(-8.0, 0.0, base + 9.7), v3(4.0, 7.0, 1.4), 0.5);
        }
    }
    b.with_recoil(|b| {
        for side in [1.0f32, -1.0] {
            fork(b, GAP.y * side);
        }
    });
}

/// One fork at `y`: a graphite breech, two blades over the red slot, the gap ahead.
fn fork(b: &mut MeshBuilder, y: f32) {
    metal(b);
    b.chamfered_box(v3(4.0, y, 0.0), v3(8.0, 4.4, 4.6), 0.8);
    let blade = |b: &mut MeshBuilder, z: f32, flip: f32| {
        // A blade: thick at its root, thinning and leaning in toward the gap at its tip.
        let rings: Vec<Vec<Vec3>> = [
            (6.0, 2.0, 1.5, 0.0),
            (13.0, 1.8, 1.2, -0.25),
            (18.5, 1.3, 0.8, -0.55),
        ]
        .iter()
        .map(|&(x, half, tall, lean)| {
            let c = v3(x, y, z + lean * flip);
            vec![
                c + v3(0.0, -half, -tall * flip * 0.5),
                c + v3(0.0, half, -tall * flip * 0.5),
                c + v3(0.0, half * 0.7, tall * flip * 0.5),
                c + v3(0.0, -half * 0.7, tall * flip * 0.5),
            ]
        })
        .collect();
        let rings: Vec<Vec<Vec3>> = if flip < 0.0 {
            rings
                .into_iter()
                .map(|r| r.into_iter().rev().collect())
                .collect()
        } else {
            rings
        };
        b.with_facets(|b| b.loft(&rings, true, true));
    };
    dark_plate(b);
    blade(b, 1.9, 1.0);
    blade(b, -1.9, -1.0);
    if !b.coarse() {
        // The slot between the blades at their root, lit.
        b.paint(GLOW_LASER);
        b.beam(
            v3(7.5, y, 0.0),
            v3(13.0, y, 0.0),
            Vec2::new(1.2, 0.7),
            Vec2::new(0.5, 0.3),
        );
        if b.fine() {
            // The containment rings the charge is squeezed through.
            seam(b);
            for x in [8.5, 11.5] {
                hoop_on(b, v3(x, y, 0.0), Vec3::X, 1.5, 0.3, 0.5, 8);
            }
        }
    }
}
