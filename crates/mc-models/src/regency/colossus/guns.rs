//! The two Rapid Pinch-fusion Guns on the Colossus's front shoulders, weapons 1 and 2, the
//! left (+y) and the right. Each stands in a gun house of its own
//! (`MeshBuilder::with_house`): a turret drum that turns, and on it a long heavy barrel that
//! pitches and kicks back, ringed by three pinch coils lit in the prism. They stand on the
//! deck so they read from above. Each picks its own mark.
//!
//! Drawn in its own frame: the origin on the trunnion, +x down the barrel, the middle of
//! the mouth at `(LEN, 0, 0)`.

use glam::{Affine3A, Vec2, Vec3};

use super::detail::armor as dark_plate;
use crate::builder::MeshBuilder;

use super::super::commander::form::{ring, sleeve, OCT};
use super::super::kit::{metal, seam, v3};
use super::super::machine::{collar, hoop_on};
use super::GUN_PIVOT;
use crate::{material::GLOW_LASER, pattern};

/// From the trunnion to the middle of the barrel's mouth.
pub(super) const LEN: f32 = 12.0;
/// The barrel's radius at its root.
const BORE: f32 = 0.8;
/// How far the barrel kicks back when it fires.
const KICK: f32 = 1.0;

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

/// The gun in its own frame: a turret drum sunk into the shoulder plate, a faceted
/// mantlet on it, and one long barrel out of a shroud, pinch coils lit in the prism down it
/// and a squared muzzle brake.
fn gun(b: &mut MeshBuilder) {
    let fine = b.fine();
    dark_plate(b);
    b.pattern(pattern::EMBER_SLOT);
    let sides = b.sides(14);
    b.prism(v3(0.0, 0.0, -2.6), sides, 3.4, 3.1, 2.2);
    if fine {
        seam(b);
        b.prism(v3(0.0, 0.0, -0.5), sides, 3.2, 3.2, 0.3);
    }
    b.with_recoil(|b| {
        // The mantlet: a faceted block over the trunnion.
        dark_plate(b);
        b.with_facets(|b| {
            b.beam(
                v3(-2.6, 0.0, 0.0),
                v3(2.2, 0.0, 0.0),
                Vec2::new(3.2, 2.4),
                Vec2::new(2.7, 2.0),
            );
        });
        collar(b, Vec3::ZERO, Vec3::Y, 1.1, 3.6);
        sleeve(
            b,
            &[
                ring(v3(2.0, 0.0, 0.0), Vec3::Z, BORE * 1.9, BORE * 1.9),
                ring(v3(LEN * 0.42, 0.0, 0.0), Vec3::Z, BORE * 1.7, BORE * 1.7),
            ],
            &OCT,
        );
        metal(b);
        let bs = b.sides(10);
        b.cylinder_between(
            v3(LEN * 0.4, 0.0, 0.0),
            v3(LEN - 0.6, 0.0, 0.0),
            BORE,
            BORE * 0.85,
            bs,
        );
        for (i, stage) in [(0.55f32, 2u32), (0.68, 4), (0.81, 6)] {
            let x = LEN * i;
            dark_plate(b);
            hoop_on(
                b,
                v3(x, 0.0, 0.0),
                Vec3::X,
                BORE * 1.35,
                BORE * 0.6,
                0.8,
                bs,
            );
            b.paint(GLOW_LASER).pattern(pattern::COIL + stage);
            b.face(&[
                v3(x + 0.81, -0.3, BORE * 0.9),
                v3(x + 0.81, 0.3, BORE * 0.9),
                v3(x + 0.81, 0.3, BORE * 1.15),
                v3(x + 0.81, -0.3, BORE * 1.15),
            ]);
        }
        // The muzzle brake: a squared block with a port either side.
        dark_plate(b);
        b.with_facets(|b| {
            b.chamfered_box(
                v3(LEN - 0.5, 0.0, 0.0),
                v3(1.4, BORE * 2.6, BORE * 2.4),
                0.15,
            );
        });
        if fine {
            seam(b);
            for side in [-1.0f32, 1.0] {
                b.chamfered_box(
                    v3(LEN - 0.5, side * BORE * 1.31, 0.0),
                    v3(0.7, 0.05, BORE * 1.2),
                    0.02,
                );
            }
        }
    });
}
