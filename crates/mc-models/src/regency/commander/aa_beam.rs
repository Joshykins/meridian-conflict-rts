//! The AA Beam Projector (`aa_beam`): an anti-air beam on the right shoulder, in a mount of
//! its own that turns and pitches at what it shoots whatever the torso is doing
//! (`MeshBuilder::with_mount`). A bronze race on the pauldron's inboard crown carries a
//! low yoke (`with_yoke`: it turns, it does not pitch); between the yoke's cheeks the
//! projector pitches on its trunnions (`TRUNNION`, the unit file's `pivot`): a breech, a
//! low housing and two long flat prongs side by side out of it, the beam leaving the knot
//! held between their tips (`MUZZLE`, its `muzzle`).
//!
//! The red is the plates' own (`pattern::EMBER_SLOT`, `EMBER_RIM`), never a lit piece. No
//! violet: it does not build.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::kit::{dark_plate, metal, v3};
use super::form::{ring, sleeve, KEEL, OCT};

/// The trunnions the projector pitches on, over the right pauldron's inboard crown.
pub(super) const TRUNNION: Vec3 = Vec3::new(-1.1, -3.2, 23.2);
/// The knot the beam leaves from, ahead of the trunnions with the projector level.
pub(super) const MUZZLE: Vec3 = Vec3::new(2.5, -3.2, 23.2);
/// The top of the race the yoke turns on.
const RACE: f32 = 22.05;
/// Half the yoke's span: the cheeks stand this far either side of the projector.
const YOKE: f32 = 0.82;

pub(super) fn aa_beam(b: &mut MeshBuilder) {
    b.module("aa_beam", 0.0, race);
    b.module("aa_beam", 0.3, |b| {
        b.with_mount(TRUNNION, 0.0, |b| {
            b.with_yoke(yoke);
            prongs(b);
        })
    });
}

/// The suite's dark armour in the machined finish.
fn machined(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::EMBER_MACHINED);
}

/// Armour with a red optic let into each broad face.
fn ember_slot(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
}

/// A point `x` metres ahead of the trunnions, `y` across (+ inboard) and `z` up, with
/// the projector level.
fn at(x: f32, y: f32, z: f32) -> Vec3 {
    TRUNNION + v3(x, y, z)
}

/// The bronze race on the pauldron's crown the yoke turns on, and the dark collar under it.
fn race(b: &mut MeshBuilder) {
    let c = v3(TRUNNION.x, TRUNNION.y, 0.0);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(c + Vec3::Z * 21.3, Vec3::Y, 1.15, 1.15),
            ring(c + Vec3::Z * 21.8, Vec3::Y, 1.0, 1.0),
        ],
        &OCT,
    );
    metal(b);
    b.cylinder_between(
        c + Vec3::Z * 21.75,
        c + Vec3::Z * RACE,
        0.92,
        0.88,
        b.sides(12),
    );
}

/// The yoke: a keeled floor on the race and a broad cheek either side, rising past the
/// trunnion, each with a boss over its pin.
fn yoke(b: &mut MeshBuilder) {
    machined(b);
    sleeve(
        b,
        &[
            ring(
                at(-0.8, 0.0, RACE - TRUNNION.z + 0.15),
                Vec3::Z,
                YOKE + 0.2,
                0.18,
            ),
            ring(
                at(0.8, 0.0, RACE - TRUNNION.z + 0.15),
                Vec3::Z,
                YOKE + 0.2,
                0.18,
            ),
        ],
        &OCT,
    );
    for side in [1.0f32, -1.0] {
        let out = Vec3::Y * side;
        let y = YOKE * side;
        ember_slot(b);
        sleeve(
            b,
            &[
                ring(at(-0.1, y, RACE - TRUNNION.z + 0.25), out, 0.62, 0.13),
                ring(at(-0.15, y, 0.35), out, 0.55, 0.13),
            ],
            &OCT,
        );
        machined(b);
        sleeve(
            b,
            &[
                ring(at(-0.15, y, 0.35), out, 0.45, 0.11),
                ring(at(-0.55, y, 0.95), out, 0.03, 0.03),
            ],
            &KEEL,
        );
        b.cylinder_between(
            TRUNNION + out * (YOKE + 0.1),
            TRUNNION + out * (YOKE + 0.28),
            0.3,
            0.24,
            b.sides(8),
        );
    }
    metal(b);
    b.cylinder_between(
        TRUNNION - Vec3::Y * (YOKE + 0.3),
        TRUNNION + Vec3::Y * (YOKE + 0.3),
        0.16,
        0.16,
        b.sides(8),
    );
}

/// The breech block the trunnions run through, between the cheeks, and the heat spine
/// raked back off its top.
fn breech(b: &mut MeshBuilder, back: f32, front: f32, r: f32) {
    machined(b);
    sleeve(
        b,
        &[
            ring(at(back, 0.0, 0.0), Vec3::Z, r * 0.75, r * 0.75),
            ring(at(back + 0.25, 0.0, 0.0), Vec3::Z, r, r),
            ring(at(front - 0.2, 0.0, 0.0), Vec3::Z, r, r),
            ring(at(front, 0.0, 0.0), Vec3::Z, r * 0.82, r * 0.82),
        ],
        &OCT,
    );
    if b.fine() {
        sleeve(
            b,
            &[
                ring(at(back + 0.5, 0.0, r * 0.9), Vec3::Y, 0.1, 0.2),
                ring(at(back - 0.1, 0.0, r * 0.9 + 0.5), Vec3::Y, 0.02, 0.02),
            ],
            &KEEL,
        );
    }
}

/// The projector: the breech, a low housing and two long flat prongs side by side out of it, lit red on their
/// inner faces, the knot held between their tips.
fn prongs(b: &mut MeshBuilder) {
    breech(b, -1.0, 0.6, 0.6);
    let len = MUZZLE.x - TRUNNION.x;
    machined(b);
    sleeve(
        b,
        &[
            ring(at(0.6, 0.0, 0.0), Vec3::Z, 0.62, 0.5),
            ring(at(1.3, 0.0, 0.0), Vec3::Z, 0.55, 0.42),
        ],
        &OCT,
    );
    for side in [1.0f32, -1.0] {
        let out = Vec3::Y * side;
        b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
        sleeve(
            b,
            &[
                ring(at(0.9, 0.32 * side, 0.0), out, 0.42, 0.11),
                ring(at(len - 0.5, 0.3 * side, 0.0), out, 0.36, 0.1),
                ring(at(len + 0.35, 0.22 * side, 0.0), out, 0.08, 0.06),
            ],
            &OCT,
        );
        machined(b);
        sleeve(
            b,
            &[
                ring(at(1.0, 0.45 * side, 0.0), out, 0.32, 0.1),
                ring(at(len - 0.9, 0.44 * side, 0.0), out, 0.26, 0.08),
                ring(at(len - 0.2, 0.36 * side, 0.0), out, 0.02, 0.02),
            ],
            &KEEL,
        );
    }
}

/// From far off: the yoke a block and the projector a bar, still turning and pitching.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.module("aa_beam", 0.3, |b| {
        b.with_mount(TRUNNION, 0.0, |b| {
            dark_plate(b);
            b.with_yoke(|b| {
                b.beam(
                    at(-0.4, 0.0, RACE - TRUNNION.z),
                    at(-0.4, 0.0, 0.3),
                    Vec2::splat(1.4),
                    Vec2::splat(1.2),
                )
            });
            b.beam(
                at(-1.0, 0.0, 0.0),
                MUZZLE,
                Vec2::splat(1.2),
                Vec2::splat(0.7),
            );
        })
    });
}
