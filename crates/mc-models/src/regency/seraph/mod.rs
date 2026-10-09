//! The Seraph, the Regency's tech 5 strike titan: a winged war-machine in the shape of a
//! knight, hanging in the air on its lift with its legs trailing and its toes pointed, two
//! great wings of blade-feathers spread from its back. It does not walk: it floats, and it
//! goes where it fights in one leap (the sim's dash).
//!
//! The body is the Regency's: faceted solids under layered dark plates (`body`), a narrow
//! horned helm with a red visor, high pauldrons swept back. A Rapid Pinch-fusion Repeater
//! runs the length of each forearm (`arms`), its bore split between two lit rails. On the
//! back between the wing roots stand two launch decks, eight cells each, open to the sky,
//! from which the cased seekers climb and fall round it (`deck`). The wings (`wings`) are
//! a binder from each shoulder blade carrying the feathers: long dark blades rimmed in red
//! (`pattern::EMBER_RIM`), the light in the plate, never a lit piece.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, graphite machinery, red light.
//! No violet: it does not build.
//!
//! Rig: `part::HULL` everything below the waist ring (`WAIST`), the legs fixed in their
//! hanging pose; `part::TURRET` the torso, head, arms, wings and decks, turning about the
//! waist to face what it shoots. The forearms pitch about the elbows (`rig::ARM_GUN`, `ELBOW`
//! and its mirror). The helm idles about `NECK`. The hull is a hovercraft's (`set_hover`):
//! the shader bobs it.
//!
//! Authored [`SCALE`] times smaller than `data/factions/regency/units/experimental.ron`
//! (`regency_t5_seraph`) builds it: every number there is this file's times `SCALE`
//! (tests).

mod arms;
mod body;
mod deck;
#[cfg(test)]
mod tests;
mod wings;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::kit::{dark_plate, v3};

/// Authored size (the unit file's is `SCALE` times it).
pub(super) const RADIUS: f32 = 15.0;
pub(super) const HEIGHT: f32 = 31.0;
/// How much bigger the unit file builds it.
#[cfg(test)]
const SCALE: f32 = 4.0;

/// The left leg's joints in its hanging pose: hip, knee a little forward, ankle, and the
/// pointed toe's tip, the lowest point of the model.
const HIP: Vec3 = Vec3::new(0.0, 1.5, 12.8);
const KNEE: Vec3 = Vec3::new(0.75, 1.75, 8.5);
const ANKLE: Vec3 = Vec3::new(-0.15, 1.9, 4.5);
const TOE: Vec3 = Vec3::new(1.0, 1.95, 1.6);
/// The ring the torso turns on.
const WAIST: f32 = 14.6;
/// The left shoulder's ball, and the left elbow the forearm pitches about.
const SHOULDER: Vec3 = Vec3::new(-0.3, 3.7, 19.3);
const ELBOW: Vec3 = Vec3::new(-0.2, 4.35, 15.4);
/// The left repeater's muzzle (the right's is its mirror): the unit file's weapon
/// `muzzle`, its `pivot` the elbow.
const MUZZLE: Vec3 = Vec3::new(7.4, 4.75, 15.25);
/// Where the helm turns while it idles.
const NECK: Vec3 = Vec3::new(0.25, 0.0, 20.5);

/// The wings, one per design variant (`regency_seraph~name`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Wings {
    /// A: a binder rising from each shoulder blade, five long feathers fanned off its
    /// elbow and a fan of short inner blades under it: the reference's planform.
    Freedom,
    /// B: three wings a side, upper, middle and lower, each a course of swept plates
    /// laid like feathers.
    Seraph,
    /// C: a sunburst of seven long blades a side, raying from a hub on the back.
    Halo,
}

pub(super) fn seraph(b: &mut MeshBuilder, _tech: u8) {
    build(b, Wings::Freedom);
}

pub(super) fn seraph_seraph(b: &mut MeshBuilder, _tech: u8) {
    build(b, Wings::Seraph);
}

pub(super) fn seraph_halo(b: &mut MeshBuilder, _tech: u8) {
    build(b, Wings::Halo);
}

fn build(b: &mut MeshBuilder, wings: Wings) {
    b.set_hover();
    b.set_turret_pivot(v3(0.0, 0.0, WAIST));
    b.set_arm_pivot(ELBOW);
    b.set_recoil(ELBOW, MUZZLE.with_y(0.0), 0.25);
    if b.coarse() {
        coarse(b, wings);
        return;
    }
    b.mirror_y(body::leg);
    body::pelvis(b);
    b.with_part(part::TURRET, |b| {
        body::torso(b);
        body::head(b);
        b.mirror_y(arms::arm);
        deck::decks(b);
        b.mirror_y(|b| wings::wing(b, wings));
    });
}

/// From far off: the legs two bars, the torso a wedge with the team colour on top, the
/// forearms bars that still pitch, each wing a few broad blades.
fn coarse(b: &mut MeshBuilder, wings: Wings) {
    dark_plate(b);
    b.mirror_y(|b| {
        b.face(&[HIP + v3(0.6, 0.6, 0.0), TOE, HIP + v3(-0.6, -0.6, 0.0)]);
        b.face(&[HIP + v3(-0.6, -0.6, 0.0), TOE, HIP + v3(0.6, 0.6, 0.0)]);
    });
    b.frustum_open(
        v3(0.0, 0.0, 12.2),
        Vec2::new(1.8, 2.4),
        Vec2::new(2.4, 4.6),
        WAIST - 12.2,
        Vec2::ZERO,
    );
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.frustum_open(
            v3(0.0, 0.0, WAIST),
            Vec2::new(2.2, 3.0),
            Vec2::new(3.0, 7.0),
            20.4 - WAIST,
            Vec2::new(-0.2, 0.0),
        );
        b.paint(TEAM);
        b.face(&[
            v3(1.2, -1.4, 20.45),
            v3(1.2, 1.4, 20.45),
            v3(-1.6, 1.4, 20.45),
            v3(-1.6, -1.4, 20.45),
        ]);
        dark_plate(b);
        b.face(&[v3(0.9, -0.6, 20.4), v3(0.0, 0.0, 23.4), v3(0.9, 0.6, 20.4)]);
        b.with_limb(rig::ARM_GUN, |b| {
            b.mirror_y(|b| {
                b.beam(
                    ELBOW.with_y(MUZZLE.y),
                    MUZZLE,
                    Vec2::splat(0.9),
                    Vec2::splat(0.5),
                )
            });
        });
        b.mirror_y(|b| wings::coarse(b, wings));
    });
}
