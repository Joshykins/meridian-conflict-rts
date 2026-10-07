//! The two Rapid Pinch-fusion Guns slung under the Colossus's chin, weapons 1 and 2, the left
//! (+y) and the right. Each stands on its bracket's pylon in a gun house of its own
//! (`MeshBuilder::with_house`): a plated cradle that turns, and in it a compact barrel that
//! pitches and kicks back, its mouth ringed by two short pinch coils lit in the prism. Each
//! picks its own mark.
//!
//! Drawn in its own frame: the origin on the trunnion, +x down the barrel, the middle of
//! the mouth at `(LEN, 0, 0)`.

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::super::commander::form::{ring, sleeve, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{collar, hoop_on};
use super::GUN_PIVOT;
use crate::{material::GLOW_LASER, pattern};

/// From the trunnion to the middle of the barrel's mouth.
pub(super) const LEN: f32 = 4.6;
/// How far the barrel kicks back when it fires.
const KICK: f32 = 0.8;

/// Both guns: weapon 1 on the left, weapon 2 its mirror on the right.
pub(super) fn guns(b: &mut MeshBuilder) {
    for (weapon, right) in [(1usize, false), (2, true)] {
        let draw = |b: &mut MeshBuilder| {
            b.with_house(weapon, GUN_PIVOT, KICK, |b| {
                b.at(GUN_PIVOT, gun);
            })
        };
        if right {
            b.with(Affine3A::from_scale(Vec3::new(1.0, -1.0, 1.0)), draw);
        } else {
            draw(b);
        }
    }
}

/// The gun in its own frame: the cradle, and the barrel in it.
fn gun(b: &mut MeshBuilder) {
    let fine = b.fine();
    // The cradle: a round plated turntable drum let up into the pod above it, a seam band
    // round it, a red slot either flank.
    dark_plate(b);
    b.pattern(pattern::EMBER_SLOT);
    let sides = b.sides(14);
    b.prism(v3(0.0, 0.0, -1.3), sides, 2.0, 1.9, 2.9);
    if fine {
        seam(b);
        b.prism(v3(0.0, 0.0, 0.1), sides, 2.06, 2.06, 0.3);
    }
    b.with_recoil(|b| {
        // The breech between the cheeks, a drum over the trunnion.
        collar(b, Vec3::ZERO, Vec3::Y, 0.85, 2.4);
        dark_plate(b);
        b.with_facets(|b| {
            b.beam(
                v3(-1.4, 0.0, 0.0),
                v3(1.6, 0.0, 0.0),
                Vec2::new(1.5, 1.6),
                Vec2::new(1.3, 1.4),
            );
        });
        // The shroud over the barrel's root, an octagon, and the barrel out of it.
        sleeve(
            b,
            &[
                ring(v3(1.4, 0.0, 0.0), Vec3::Z, 0.85, 0.85),
                ring(v3(3.8, 0.0, 0.0), Vec3::Z, 0.8, 0.8),
            ],
            &OCT,
        );
        metal(b);
        let sides = b.sides(10);
        b.cylinder_between(v3(2.4, 0.0, 0.0), v3(LEN - 0.2, 0.0, 0.0), 0.5, 0.42, sides);
        // Two short pinch coils round the muzzle end, lit in the prism.
        for (x, stage) in [(2.9f32, 3u32), (3.8, 5)] {
            dark_plate(b);
            hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, 0.62, 0.35, 0.7, sides);
            b.paint(GLOW_LASER).pattern(pattern::COIL + stage);
            b.face(&[
                v3(x + 0.71, -0.25, 0.45),
                v3(x + 0.71, 0.25, 0.45),
                v3(x + 0.71, 0.25, 0.58),
                v3(x + 0.71, -0.25, 0.58),
            ]);
        }
        dark_plate(b);
        hoop_on(b, v3(LEN - 0.1, 0.0, 0.0), Vec3::X, 0.46, 0.22, 0.5, sides);
    });
}
