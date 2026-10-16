//! The Gravitic Shock Mortar (`gravitic`) on the right shoulder: a gun of its own
//! (`with_mount`) that throws knots of held gravity high over the field, to shatter the
//! ground they land on. A keeled pad lies on the pauldron's inboard crown, mirroring the
//! auxiliary suite's on the left; a pedestal stands on it and carries a yoke whose cheeks
//! turn with the gun but do not pitch, and the gun pitches between them on its trunnions
//! (`TRUNNION`, the unit file's `pivot`): a breech block, a long slotted barrel and a
//! flared muzzle collar, the knot leaving the middle of its bore (`MUZZLE`, its `muzzle`).
//!
//! Round the barrel floats the halo: a ring of plates held off it by gravity alone, three
//! shards on its inside pointing in at the bore. It drifts round at rest; each shot flings
//! it round the barrel (`with_spin`, the unit file's `fling`), fast at the shot and
//! slowing to rest over the reload, while the barrel kicks back through it.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::kit::{cable, metal, nanite_plate, v3};
use super::form::{ring, sleeve, Ring, KEEL, OCT};
use super::shoulder::{HEX, SLAB};

/// The trunnions the gun pitches on, over the right pauldron's inboard crown.
pub(super) const TRUNNION: Vec3 = Vec3::new(-0.6, -3.2, 23.4);
/// The bore's mouth: `MOUTH` metres ahead of the trunnions.
pub(super) const MUZZLE: Vec3 = Vec3::new(TRUNNION.x + MOUTH, TRUNNION.y, TRUNNION.z);
const MOUTH: f32 = 4.4;
/// The pedestal's foot on the pad.
const FOOT: Vec3 = Vec3::new(-1.0, -3.2, 21.95);
/// Half the yoke's span: the cheeks stand this far either side of the line.
const YOKE: f32 = 1.1;
/// How far the barrel kicks back through the halo on a shot.
const KICK: f32 = 0.6;
/// The halo: its middle along the barrel, its radius, its plates.
const HALO_AT: f32 = 2.45;
const HALO: f32 = 1.4;
const HALO_PLATES: usize = 12;

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
                b.with_spin(at(HALO_AT), halo);
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
/// counterweights at its back, the slotted barrel, its bands and the muzzle collar.
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
            hex(-1.4, 0.62),
            hex(-1.1, 0.85),
            hex(0.45, 0.85),
            hex(0.75, 0.7),
        ],
        &HEX,
    );
    // Counterweight slabs either side of the breech's back.
    for side in [1.0f32, -1.0] {
        let out = Vec3::Y * side;
        sleeve(
            b,
            &[
                ring(at(-1.3) + out * 0.78, out, 0.55, 0.12),
                ring(at(-0.15) + out * 0.85, out, 0.55, 0.12),
            ],
            &SLAB,
        );
    }
    // The barrel: a slotted jacket tapering to the collar, bound by two bands either side
    // of the halo.
    nanite_plate(b);
    sleeve(b, &[hex(0.7, 0.62), hex(3.85, 0.5)], &HEX);
    machined(b);
    for t in [1.2, 3.65] {
        sleeve(
            b,
            &[hex(t - 0.15, 0.6), hex(t, 0.68), hex(t + 0.15, 0.6)],
            &HEX,
        );
    }
    sleeve(
        b,
        &[
            hex(3.8, 0.56),
            hex(4.0, 0.72),
            hex(MOUTH - 0.12, 0.76),
            hex(MOUTH, 0.68),
        ],
        &HEX,
    );
    bore(b);
    if b.fine() {
        // Heat spines along the breech's top and a feed line down each side of it.
        machined(b);
        for (t, h) in [(-0.8, 0.55), (-0.1, 0.45)] {
            let root = at(t) + Vec3::Z * 0.82;
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
                    at(-0.9) + s * 0.88 - Vec3::Z * 0.35,
                    at(0.6) + s * 0.72 - Vec3::Z * 0.4,
                    at(1.05) + s * 0.6 - Vec3::Z * 0.35,
                ],
                0.08,
            );
        }
    }
}

/// The bore sunk into the collar's face: a dark lip round a violet gasket, and the cavity
/// behind it, the knot leaving the middle of the mouth.
fn bore(b: &mut MeshBuilder) {
    let loop_at = |t: f32, r: f32| -> Vec<Vec3> {
        HEX.iter()
            .map(|&[s, u]| at(t) + (Vec3::NEG_Y * s + Vec3::Z * u) * r)
            .collect()
    };
    let rim = loop_at(MOUTH, 0.68);
    let lip = loop_at(MOUTH, 0.42);
    b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
    for k in 0..HEX.len() {
        let n = (k + 1) % HEX.len();
        b.face(&[rim[k], rim[n], lip[n], lip[k]]);
    }
    metal(b);
    b.with_facets(|b| b.loft(&[lip, loop_at(MOUTH - 0.4, 0.3)], false, true));
}

/// The halo, held free of the barrel: a ring of plates, every other one slotted, and
/// three shards on its inside pointing in at the bore.
fn halo(b: &mut MeshBuilder) {
    let step = std::f32::consts::TAU / HALO_PLATES as f32;
    for k in 0..HALO_PLATES {
        // Each plate a little short of the next, so the ring reads as plates, not a hoop.
        let (p, q) = (
            round(k as f32 * step + 0.03),
            round((k + 1) as f32 * step - 0.03),
        );
        let mid = (p + q).normalize();
        if k % 2 == 0 {
            nanite_plate(b);
        } else {
            machined(b);
        }
        sleeve(
            b,
            &[
                ring(at(HALO_AT) + p * HALO, mid, 0.34, 0.2),
                ring(at(HALO_AT) + q * HALO, mid, 0.34, 0.2),
            ],
            &OCT,
        );
    }
    nanite_plate(b);
    for k in 0..3 {
        let r = round(std::f32::consts::TAU * (k as f32 / 3.0) + step * 0.5);
        sleeve(
            b,
            &[
                ring(at(HALO_AT) + r * (HALO - 0.1), r, 0.3, 0.16),
                ring(at(HALO_AT + 0.15) + r * 0.95, r, 0.02, 0.02),
            ],
            &KEEL,
        );
    }
}

/// From far off: the pedestal as a bar, the gun as another, still pitching, and the halo
/// as a square about the barrel.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.module("gravitic", 0.3, |b| {
        machined(b);
        b.beam(FOOT, TRUNNION, Vec2::splat(0.9), Vec2::splat(0.9));
        b.with_mount(TRUNNION, KICK, |b| {
            b.with_recoil(|b| {
                b.beam(at(-1.4), MUZZLE, Vec2::splat(1.5), Vec2::splat(1.2));
                b.with_spin(at(HALO_AT), |b| {
                    for k in 0..4 {
                        let a = std::f32::consts::FRAC_PI_2 * k as f32;
                        b.beam(
                            at(HALO_AT) + round(a) * HALO,
                            at(HALO_AT) + round(a + std::f32::consts::FRAC_PI_2) * HALO,
                            Vec2::splat(0.4),
                            Vec2::splat(0.4),
                        );
                    }
                });
            });
        });
    });
}
