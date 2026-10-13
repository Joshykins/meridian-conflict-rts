//! A Pinch-plasma Battery: a faceted ball mount turning and pitching in its socket,
//! its face a pale plate, two forks side by side out of the face. Each fork is a pair
//! of blades, one over the other, with a red-lit slot between them at the root: plasma
//! is gathered and squeezed in the gap ahead of the blades' tips, then pinched out as
//! one jet (the Halberd's way, at a warship's size). The weapon's muzzles are those
//! gaps.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::GLOW_LASER;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::hoop_on;
use super::super::dress::pale_plate;

/// The house's size against the authored one.
pub(super) const SIZE: f32 = 1.35;
/// Each fork's charge point ahead of the pivot (authored, before `SIZE`), port fork.
const GAP: Vec3 = Vec3::new(24.0, 3.6, 0.0);
/// The ball's radius (authored, before `SIZE`); its middle is the pivot.
const BALL: f32 = 8.0;
/// The ball's radius as built: the socket it sits in is cut to it.
pub(super) const BALL_R: f32 = BALL * SIZE;
/// How far forward the forks stand of where they were authored round a flat house.
const FORK_X: f32 = 3.0;

/// Where a battery at `pivot` gathers its plasma: the port fork's gap, then starboard's.
#[cfg(test)]
pub(super) fn muzzles(pivot: Vec3) -> [Vec3; 2] {
    gaps(SIZE).map(|g| pivot + g)
}

/// Where the ball (`house`) built at `scale` gathers its plasma off its pivot: the port
/// fork's gap, then starboard's.
#[cfg(test)]
pub(in crate::regency::space) fn gaps(scale: f32) -> [Vec3; 2] {
    let g = GAP * scale;
    [g, v3(g.x, -g.y, g.z)]
}

/// A battery bound to `weapon`, its ball turning and pitching about `pivot`.
pub(super) fn battery(b: &mut MeshBuilder, weapon: usize, pivot: Vec3) {
    b.with_house(weapon, pivot, 0.8 * SIZE, |b| {
        b.with(
            glam::Affine3A::from_translation(pivot) * glam::Affine3A::from_scale(Vec3::splat(SIZE)),
            house,
        );
    });
}

/// The ball, authored about its pivot at the origin, facing +x: all of it pitches. The
/// Regent's grand batteries are the same ball, scaled.
pub(in crate::regency::space) fn house(b: &mut MeshBuilder) {
    b.with_recoil(|b| {
        let sides = b.sides(10);
        let rings = if b.fine() { 6 } else { 4 };
        dark_plate(b);
        b.with_facets(|b| b.spheroid(Vec3::ZERO, Vec3::splat(BALL), sides, rings));
        // Its face: a pale plate the forks stand out of.
        pale_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(
                v3(BALL * 0.72, 0.0, 0.0),
                v3(BALL * 0.9, 0.0, 0.0),
                BALL * 0.66,
                BALL * 0.42,
                6,
            )
        });
        if b.fine() {
            // A graphite girdle round the ball's waist, where it turns in the socket.
            metal(b);
            hoop_on(b, Vec3::ZERO, Vec3::Y, BALL * 1.01, 0.5, 1.6, sides);
        }
        for side in [1.0f32, -1.0] {
            fork(b, GAP.y * side);
        }
    });
}

/// One fork at `y`: a graphite breech, two blades over the red slot, the gap ahead.
fn fork(b: &mut MeshBuilder, y: f32) {
    b.at(Vec3::X * FORK_X, |b| fork_at(b, y));
}

/// One fork at `y`, authored with its breech at x 4.
fn fork_at(b: &mut MeshBuilder, y: f32) {
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
