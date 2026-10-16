//! The Gravitic Shock Mortar (`gravitic`) on the right shoulder: a gun of its own
//! (`with_mount`) that throws knots of held gravity high over the field, to shatter the
//! ground they land on. A keeled pad lies on the pauldron's inboard crown, mirroring the
//! auxiliary suite's on the left; a pedestal stands on it and carries a yoke whose cheeks
//! turn with the gun but do not pitch, and the gun pitches between them on its trunnions
//! (`TRUNNION`, the unit file's `pivot`).
//!
//! The gun is a split bore: a breech block, and out of its face two long keeled tines
//! above and below the line, closing to points; the knot leaves between the points
//! (`MUZZLE`, its `muzzle`). A violet crystal hangs free between the tines. It turns slowly
//! at rest; each shot flings it round the line (`with_spin`, the unit file's `fling`), fast
//! at the shot and slowing to rest over the reload, while the gun kicks back.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::kit::{cable, metal, nanite_plate, v3};
use super::form::{ring, sleeve, Ring, KEEL, OCT};
use super::shoulder::{DIAMOND, HEX, SLAB};

/// The trunnions the gun pitches on, over the right pauldron's inboard crown.
pub(super) const TRUNNION: Vec3 = Vec3::new(-0.6, -3.2, 23.4);
/// Between the tines' points: `MOUTH` metres ahead of the trunnions.
pub(super) const MUZZLE: Vec3 = Vec3::new(TRUNNION.x + MOUTH, TRUNNION.y, TRUNNION.z);
const MOUTH: f32 = 6.3;
/// The pedestal's foot on the pad.
const FOOT: Vec3 = Vec3::new(-1.0, -3.2, 21.95);
/// Half the yoke's span: the cheeks stand this far either side of the line.
const YOKE: f32 = 1.1;
/// How far the gun kicks back on a shot.
const KICK: f32 = 0.6;
/// The breech's face, where the tines leave it.
const FACE: f32 = 1.1;
/// The tines' inner faces stand this far off the line.
const GAP: f32 = 0.3;
/// The crystal: its middle along the line, its half-length, and its radius, which keeps
/// it clear of the tines as it turns.
const CORE: f32 = 2.3;
const CORE_HALF: f32 = 0.95;
const CORE_R: f32 = 0.26;

/// The suite's dark armour: Regency plate in the machined finish.
fn machined(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::EMBER_MACHINED);
}

/// A point on the line `t` metres ahead of the trunnions.
fn at(t: f32) -> Vec3 {
    TRUNNION + Vec3::X * t
}

/// The gun's section `t` metres ahead of the trunnions, `r` either side, flat on top.
fn hex(t: f32, r: f32) -> Ring {
    ring(at(t), Vec3::Z, r, r)
}

/// Square to the line, `a` radians round from the top.
fn round(a: f32) -> Vec3 {
    Vec3::Z * a.cos() + Vec3::NEG_Y * a.sin()
}

pub(super) fn mortar(b: &mut MeshBuilder) {
    b.module("gravitic", 0.0, pad);
    b.module("gravitic", 0.3, |b| {
        b.with_mount(TRUNNION, KICK, |b| {
            b.with_yoke(yoke);
            b.with_recoil(|b| {
                gun(b);
                b.with_spin(at(CORE), crystal);
            });
        });
    });
}

/// The pad on the pauldron's crown, keeled along its length and slotted down its flanks.
fn pad(b: &mut MeshBuilder) {
    nanite_plate(b);
    let c = FOOT - Vec3::Z * 0.45;
    sleeve(
        b,
        &[
            ring(c - Vec3::X * 1.9, Vec3::Z, 0.1, 0.06),
            ring(c - Vec3::X * 0.6, Vec3::Z, 0.95, 0.4),
            ring(c + Vec3::X * 0.8, Vec3::Z, 0.85, 0.36),
            ring(c + Vec3::X * 1.8, Vec3::Z, 0.08, 0.05),
        ],
        &KEEL,
    );
}

/// The turning pedestal, the yoke's floor and its two slotted cheeks rising past the
/// trunnions, each crowned with a blade raked back, and a boss over each pin.
fn yoke(b: &mut MeshBuilder) {
    let top = v3(-0.7, TRUNNION.y, 22.85);
    machined(b);
    sleeve(
        b,
        &[
            ring(FOOT, Vec3::NEG_X, 0.55, 0.5),
            ring(top, Vec3::NEG_X, 0.6, 0.5),
        ],
        &OCT,
    );
    sleeve(
        b,
        &[
            ring(top - Vec3::Y * (YOKE + 0.16), Vec3::Z, 0.5, 0.2),
            ring(top + Vec3::Y * (YOKE + 0.16), Vec3::Z, 0.5, 0.2),
        ],
        &OCT,
    );
    for side in [1.0f32, -1.0] {
        let out = Vec3::Y * side;
        let y = out * YOKE;
        let crown = TRUNNION + y + v3(-0.25, 0.0, 0.5);
        nanite_plate(b);
        sleeve(
            b,
            &[
                ring(top + y + v3(0.05, 0.0, -0.1), out, 0.7, 0.15),
                ring(crown, out, 0.7, 0.15),
            ],
            &SLAB,
        );
        machined(b);
        sleeve(
            b,
            &[
                ring(crown, out, 0.55, 0.13),
                ring(crown + v3(-0.5, 0.0, 0.8), out, 0.03, 0.03),
            ],
            &KEEL,
        );
        b.cylinder_between(
            TRUNNION + out * (YOKE + 0.14),
            TRUNNION + out * (YOKE + 0.34),
            0.38,
            0.28,
            b.sides(8),
        );
    }
}

/// The gun that pitches and kicks: the trunnion pin, the breech block and the
/// counterweights at its back, the tines out of its face and the bands between them.
fn gun(b: &mut MeshBuilder) {
    metal(b);
    b.cylinder_between(
        TRUNNION - Vec3::Y * (YOKE + 0.3),
        TRUNNION + Vec3::Y * (YOKE + 0.3),
        0.2,
        0.2,
        b.sides(8),
    );
    machined(b);
    sleeve(
        b,
        &[
            hex(-1.6, 0.6),
            hex(-1.3, 0.8),
            hex(FACE - 0.3, 0.8),
            hex(FACE, 0.68),
        ],
        &HEX,
    );
    socket(b);
    // Counterweight slabs either side of the breech's back.
    machined(b);
    for side in [1.0f32, -1.0] {
        let out = Vec3::Y * side;
        sleeve(
            b,
            &[
                ring(at(-1.5) + out * 0.73, out, 0.55, 0.12),
                ring(at(-0.3) + out * 0.8, out, 0.55, 0.12),
            ],
            &SLAB,
        );
    }
    for side in [1.0f32, -1.0] {
        tine(b, Vec3::Z * side);
    }
    // Bands down either side holding the tines apart, clear of the crystal.
    machined(b);
    for t in [1.5, 4.0] {
        for side in [1.0f32, -1.0] {
            let out = Vec3::NEG_Y * side;
            sleeve(
                b,
                &[
                    ring(at(t) + out * 0.52 + Vec3::Z * 0.65, out, 0.2, 0.1),
                    ring(at(t) + out * 0.52 - Vec3::Z * 0.65, out, 0.2, 0.1),
                ],
                &OCT,
            );
        }
    }
    if b.fine() {
        // Heat spines along the breech's top and a feed line down each side of it.
        for (t, h) in [(-1.0, 0.55), (-0.3, 0.45)] {
            let root = at(t) + Vec3::Z * 0.78;
            sleeve(
                b,
                &[
                    ring(root, Vec3::X, 0.1, 0.16),
                    ring(root + v3(-0.45, 0.0, h), Vec3::X, 0.02, 0.02),
                ],
                &KEEL,
            );
        }
        metal(b);
        for side in [1.0f32, -1.0] {
            let s = Vec3::Y * side;
            cable(
                b,
                &[
                    at(-1.1) + s * 0.83 - Vec3::Z * 0.35,
                    at(0.4) + s * 0.75 - Vec3::Z * 0.4,
                    at(FACE - 0.05) + s * 0.62 - Vec3::Z * 0.3,
                ],
                0.08,
            );
        }
    }
}

/// A tine on the `out` side of the line: slotted along its length from the breech's
/// face, closing on the line to a machined point.
fn tine(b: &mut MeshBuilder, out: Vec3) {
    let off = |t: f32, h: f32| at(t) + out * (GAP + h);
    nanite_plate(b);
    sleeve(
        b,
        &[
            ring(off(FACE - 0.4, 0.3), out, 0.55, 0.3),
            ring(off(3.8, 0.27), out, 0.48, 0.27),
            ring(off(5.6, 0.2) - out * 0.05, out, 0.36, 0.2),
        ],
        &KEEL,
    );
    machined(b);
    sleeve(
        b,
        &[
            ring(off(5.55, 0.2) - out * 0.05, out, 0.38, 0.22),
            ring(MUZZLE + out * 0.2, out, 0.05, 0.05),
        ],
        &KEEL,
    );
}

/// The socket in the breech's face the crystal hangs off: a dark lip round a violet
/// gasket, and the cavity behind it.
fn socket(b: &mut MeshBuilder) {
    let loop_at = |t: f32, r: f32| -> Vec<Vec3> {
        HEX.iter()
            .map(|&[s, u]| at(t) + (Vec3::NEG_Y * s + Vec3::Z * u) * r)
            .collect()
    };
    let rim = loop_at(FACE, 0.68);
    let lip = loop_at(FACE, 0.42);
    b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
    for k in 0..HEX.len() {
        let n = (k + 1) % HEX.len();
        b.face(&[rim[k], rim[n], lip[n], lip[k]]);
    }
    metal(b);
    b.with_facets(|b| b.loft(&[lip, loop_at(FACE - 0.4, 0.3)], false, true));
}

/// The crystal, held free between the tines: a long violet diamond on the line, with
/// three vanes round its waist so its turning reads from afar.
fn crystal(b: &mut MeshBuilder) {
    nanite_plate(b);
    sleeve(
        b,
        &[
            ring(at(CORE - CORE_HALF), Vec3::Z, 0.02, 0.02),
            ring(at(CORE - 0.2), Vec3::Z, CORE_R, CORE_R),
            ring(at(CORE + 0.2), Vec3::Z, CORE_R, CORE_R),
            ring(at(CORE + CORE_HALF), Vec3::Z, 0.02, 0.02),
        ],
        &DIAMOND,
    );
    machined(b);
    for k in 0..3 {
        let r = round(std::f32::consts::TAU * (k as f32 / 3.0) + 0.4);
        sleeve(
            b,
            &[
                ring(at(CORE - 0.5) + r * 0.14, r, 0.02, 0.02),
                ring(at(CORE - 0.1) + r * 0.2, r, 0.06, 0.06),
                ring(at(CORE + 0.35) + r * 0.16, r, 0.02, 0.02),
            ],
            &KEEL,
        );
    }
}

/// From far off: the pedestal as a bar, the breech and each tine as others, still
/// pitching, and the crystal as a bar turning between the tines.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.module("gravitic", 0.3, |b| {
        machined(b);
        b.beam(FOOT, TRUNNION, Vec2::splat(0.9), Vec2::splat(0.9));
        b.with_mount(TRUNNION, KICK, |b| {
            b.with_recoil(|b| {
                b.beam(at(-1.6), at(FACE), Vec2::splat(1.6), Vec2::splat(1.4));
                for side in [1.0f32, -1.0] {
                    let out = Vec3::Z * side;
                    b.beam(
                        at(FACE - 0.4) + out * (GAP + 0.3),
                        MUZZLE + out * 0.2,
                        Vec2::new(1.0, 0.6),
                        Vec2::splat(0.1),
                    );
                }
                nanite_plate(b);
                b.with_spin(at(CORE), |b| {
                    b.beam(
                        at(CORE - CORE_HALF),
                        at(CORE + CORE_HALF),
                        Vec2::splat(CORE_R),
                        Vec2::splat(CORE_R),
                    );
                });
            });
        });
    });
}
