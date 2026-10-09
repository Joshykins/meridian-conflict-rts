//! The Seraph, the Regency's tech 5 strike titan: a winged war-machine in the shape of a
//! knight, hanging in the air on its lift with its legs trailing and its toes pointed, two
//! great wings of blade-feathers spread from its back. It does not walk: it floats, and it
//! goes where it fights in one leap (the sim's dash).
//!
//! The body is the Regency's: faceted solids under layered dark plates (`body`), a narrow
//! horned helm with a red visor, high pauldrons swept back. A Rapid Pinch-fusion Repeater
//! runs the length of each forearm (`arms`), its bore split between two lit rails. On the
//! back between the wing roots stand two launch decks, eight cells each, open to the sky,
//! from which the cased seekers climb and fall round it (`deck`). The wings (`wings`) fan
//! out of a housing on each shoulder blade: seven dark feathers rimmed in red
//! (`pattern::EMBER_RIM`), each cut differently, the light in the plate, never a lit
//! piece. Its legs hang staggered, one drawn up; with nothing to shoot its forearms hang
//! at its sides, the repeaters pointed down (the unit file's `rest_pitch`).
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, graphite machinery, red light.
//! No violet: it does not build.
//!
//! Rig: `part::HULL` everything below the waist ring (`WAIST`), the legs fixed in their
//! hanging pose (`LEFT`, `RIGHT`); `part::TURRET` the torso, head, arms, wings and decks, turning about the
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

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::flight;
use crate::material::*;
use crate::{part, rig, Flight};

use super::kit::{dark_plate, v3};

/// Authored size (the unit file's is `SCALE` times it).
pub(super) const RADIUS: f32 = 15.0;
pub(super) const HEIGHT: f32 = 31.0;
/// How much bigger the unit file builds it.
#[cfg(test)]
const SCALE: f32 = 4.0;

/// A leg's joints in its hanging pose, given on the left: hip, knee, ankle and the
/// pointed toe's tip.
#[derive(Clone, Copy)]
struct Pose {
    hip: Vec3,
    knee: Vec3,
    ankle: Vec3,
    toe: Vec3,
}

/// The legs hang staggered: the left nearly straight, its toe the lowest point of the
/// model; the right drawn up, its knee forward and its shin swept back under it.
const LEFT: Pose = Pose {
    hip: Vec3::new(0.0, 1.5, 12.8),
    knee: Vec3::new(0.55, 1.75, 8.45),
    ankle: Vec3::new(-0.2, 1.9, 4.45),
    toe: Vec3::new(0.85, 1.95, 1.6),
};
const RIGHT: Pose = Pose {
    hip: Vec3::new(0.0, 1.5, 12.8),
    knee: Vec3::new(1.85, 1.7, 8.9),
    ankle: Vec3::new(-0.45, 1.85, 5.55),
    toe: Vec3::new(-0.6, 1.9, 2.55),
};
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

/// Mirrors what `f` builds across the centreline.
fn mirrored(b: &mut MeshBuilder, f: impl FnOnce(&mut MeshBuilder)) {
    b.with(Affine3A::from_scale(Vec3::new(1.0, -1.0, 1.0)), f);
}

pub(super) fn seraph(b: &mut MeshBuilder, _tech: u8) {
    b.set_hover();
    b.set_flight(Flight {
        hub: wings::HUB.to_array(),
        hip: LEFT.hip.to_array(),
        knees: [LEFT.knee.to_array(), RIGHT.knee.to_array()],
    });
    b.set_turret_pivot(v3(0.0, 0.0, WAIST));
    b.set_arm_pivot(ELBOW);
    b.set_recoil(ELBOW, MUZZLE.with_y(0.0), 0.25);
    if b.coarse() {
        coarse(b);
        return;
    }
    body::leg(b, LEFT);
    mirrored(b, |b| body::leg(b, RIGHT));
    body::pelvis(b);
    b.with_part(part::TURRET, |b| {
        body::torso(b);
        body::head(b);
        b.mirror_y(arms::arm);
        deck::decks(b);
        b.mirror_y(wings::wing);
    });
}

/// From far off: the legs two bars, the torso a wedge with the team colour on top, the
/// forearms bars that still pitch, each wing a few broad blades.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    let flat = |b: &mut MeshBuilder, p: Pose| {
        b.face(&[
            p.hip + v3(0.6, 0.6, 0.0),
            p.toe,
            p.hip + v3(-0.6, -0.6, 0.0),
        ]);
        b.face(&[
            p.hip + v3(-0.6, -0.6, 0.0),
            p.toe,
            p.hip + v3(0.6, 0.6, 0.0),
        ]);
    };
    b.with_flight(flight::THIGH, |b| {
        flat(b, LEFT);
        mirrored(b, |b| flat(b, RIGHT));
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
        b.mirror_y(wings::coarse);
    });
}
