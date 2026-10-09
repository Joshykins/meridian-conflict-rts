//! The Auxiliary Engineering Suite (`aux_eng`): a nanite projector on the left shoulder.
//! A keeled pad lies on the pauldron's inboard crown; a pedestal stands on it from a
//! knuckle (`AUX_HINGE`) and carries a yoke, and the projector pitches in the yoke on its
//! trunnions (`AUX_WRIST`, the unit file's `hinge`) to point at the work. At its back a
//! nanite crystal sits between two slotted cage slabs, three vanes standing off them;
//! then the breech block the trunnions run through; a six-sided barrel jacketed in slotted
//! plates out to a raised band; and at the muzzle a flared collar, three fins round it and
//! the bore sunk into its face, the beam leaving the middle of its mouth (`AUX_EMITTER`,
//! its `emitters`).
//!
//! The violet is the plates' own (`kit::nanite_plate`: the slot regency.wgsl lets into a
//! face), never a lit piece, so every slotted face is broad and long enough on a model
//! this size to carry one (about 0.6 m by 1.1 m). The dark plates are the machined finish
//! (`pattern::EMBER_MACHINED`).
//!
//! Stowed (`MeshBuilder::with_fold`, entity.wgsl's folding gear), the pedestal rocks back
//! and the projector tips its muzzle up, in line with the pauldron's spikes.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::kit::{cable, metal, nanite_plate, v3};
use super::form::{ring, sleeve, KEEL, OCT};

/// The pedestal's knuckle, on the pad.
pub(super) const AUX_HINGE: Vec3 = Vec3::new(-1.0, 3.2, 21.95);
/// The beam line: the crystal's back, the trunnions on it, and the muzzle.
const BACK: Vec3 = Vec3::new(-2.5, 3.2, 24.05);
pub(super) const AUX_WRIST: Vec3 = Vec3::new(-0.6, 3.2, 23.889);
pub(super) const AUX_EMITTER: Vec3 = Vec3::new(3.4, 3.2, 23.55);
/// Half the yoke's span: the cheeks stand this far either side of the line.
const YOKE: f32 = 0.98;
/// Stowed, the pedestal rocks back and the projector tips its muzzle up.
const AUX_STOWED: f32 = 0.2;
const AUX_HEAD_STOWED: f32 = 0.45;

/// A hexagon flat on top and bottom: broad faces, each wide enough for a nanite slot.
const HEX: [[f32; 2]; 6] = [
    [1.0, 0.0],
    [0.5, 0.87],
    [-0.5, 0.87],
    [-1.0, 0.0],
    [-0.5, -0.87],
    [0.5, -0.87],
];
/// A flat plate with bevelled edges: one broad face each side for a nanite slot.
const SLAB: [[f32; 2]; 8] = [
    [1.0, 0.35],
    [0.9, 1.0],
    [-0.9, 1.0],
    [-1.0, 0.35],
    [-1.0, -0.35],
    [-0.9, -1.0],
    [0.9, -1.0],
    [1.0, -0.35],
];
/// A square stood on its corner: the crystal's four long faces.
const DIAMOND: [[f32; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];

pub(super) fn aux_eng(b: &mut MeshBuilder) {
    b.module("aux_eng", 0.0, pad);
    b.module("aux_eng", 0.3, |b| {
        b.with_fold(AUX_HINGE, AUX_STOWED, pedestal);
        b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, projector);
    });
}

/// The suite's dark armour: Regency plate in the machined finish, fine grain and
/// recessed fasteners.
fn machined(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::EMBER_MACHINED);
}

/// The beam line's direction, square to it in the upright plane, and across it.
fn line() -> (Vec3, Vec3, Vec3) {
    let d = (AUX_EMITTER - BACK).normalize();
    let up = Vec3::new(-d.z, 0.0, d.x);
    (d, up, d.cross(up))
}

/// A point on the beam line `t` metres ahead of the trunnions.
fn on_line(t: f32) -> Vec3 {
    AUX_WRIST + line().0 * t
}

/// The pad on the pauldron's crown, keeled along its length and slotted down its flanks,
/// and the knuckle on it.
fn pad(b: &mut MeshBuilder) {
    nanite_plate(b);
    let at = AUX_HINGE - Vec3::Z * 0.45;
    sleeve(
        b,
        &[
            ring(at - Vec3::X * 1.8, Vec3::Z, 0.1, 0.06),
            ring(at - Vec3::X * 0.6, Vec3::Z, 0.85, 0.36),
            ring(at + Vec3::X * 0.7, Vec3::Z, 0.75, 0.32),
            ring(at + Vec3::X * 1.7, Vec3::Z, 0.08, 0.05),
        ],
        &KEEL,
    );
    metal(b);
    b.cylinder_between(
        AUX_HINGE - Vec3::Y * 0.5,
        AUX_HINGE + Vec3::Y * 0.5,
        0.32,
        0.32,
        b.sides(10),
    );
}

/// The pedestal from the knuckle up to the yoke, the yoke's floor and its two slotted
/// cheeks either side of the line, and the trunnion pins through them.
fn pedestal(b: &mut MeshBuilder) {
    let top = v3(-0.75, 3.2, 22.85);
    machined(b);
    sleeve(
        b,
        &[
            ring(AUX_HINGE, Vec3::NEG_X, 0.46, 0.42),
            ring(AUX_HINGE.lerp(top, 0.55), Vec3::NEG_X, 0.38, 0.5),
            ring(top, Vec3::NEG_X, 0.55, 0.46),
        ],
        &OCT,
    );
    sleeve(
        b,
        &[
            ring(top - Vec3::Y * (YOKE + 0.14), Vec3::Z, 0.42, 0.18),
            ring(top + Vec3::Y * (YOKE + 0.14), Vec3::Z, 0.42, 0.18),
        ],
        &OCT,
    );
    for side in [1.0f32, -1.0] {
        let out = Vec3::Y * side;
        let y = out * YOKE;
        // Each cheek a broad keeled plate, ridge outward, rising past the trunnion to a
        // point raked back: its flanks carry the slot.
        let crown = AUX_WRIST + y + v3(-0.2, 0.0, 0.45);
        nanite_plate(b);
        sleeve(
            b,
            &[
                ring(top + y + v3(0.05, 0.0, -0.1), out, 0.6, 0.14),
                ring(crown, out, 0.6, 0.14),
            ],
            &SLAB,
        );
        machined(b);
        sleeve(
            b,
            &[
                ring(crown, out, 0.5, 0.12),
                ring(crown + v3(-0.4, 0.0, 0.7), out, 0.03, 0.03),
            ],
            &KEEL,
        );
        // A boss over each trunnion pin.
        machined(b);
        b.cylinder_between(
            AUX_WRIST + out * (YOKE + 0.12),
            AUX_WRIST + out * (YOKE + 0.3),
            0.34,
            0.26,
            b.sides(8),
        );
    }
    metal(b);
    b.cylinder_between(
        AUX_WRIST - Vec3::Y * (YOKE + 0.32),
        AUX_WRIST + Vec3::Y * (YOKE + 0.32),
        0.18,
        0.18,
        b.sides(8),
    );
}

/// The projector, pitching on its trunnions: crystal cage, breech, barrel and muzzle.
fn projector(b: &mut MeshBuilder) {
    let (d, up, across) = line();
    let back = BACK.distance(AUX_WRIST);
    let len = AUX_WRIST.distance(AUX_EMITTER);
    // The breech block the trunnions run through, between the cheeks.
    machined(b);
    sleeve(
        b,
        &[
            ring(on_line(-0.85), up, 0.62, 0.62),
            ring(on_line(-0.55), up, 0.8, 0.8),
            ring(on_line(0.6), up, 0.8, 0.8),
            ring(on_line(0.9), up, 0.66, 0.66),
        ],
        &HEX,
    );
    cage(b, back);
    // The barrel: a slotted six-sided jacket out to the band, then a plain run to the
    // muzzle collar.
    let band = len * 0.62;
    nanite_plate(b);
    sleeve(
        b,
        &[
            ring(on_line(0.85), up, 0.72, 0.72),
            ring(on_line(band - 0.1), up, 0.7, 0.7),
        ],
        &HEX,
    );
    machined(b);
    sleeve(
        b,
        &[
            ring(on_line(band - 0.15), up, 0.76, 0.76),
            ring(on_line(band), up, 0.82, 0.82),
            ring(on_line(band + 0.25), up, 0.8, 0.8),
            ring(on_line(band + 0.35), up, 0.5, 0.5),
            ring(on_line(len - 0.6), up, 0.46, 0.46),
            ring(on_line(len - 0.3), up, 0.6, 0.6),
        ],
        &HEX,
    );
    muzzle(b, len);
    // The fins round the muzzle, swept forward, each rooted on the collar.
    machined(b);
    for k in 0..3 {
        let a = std::f32::consts::TAU * (k as f32 / 3.0);
        let r = up * a.cos() + across * a.sin();
        sleeve(
            b,
            &[
                ring(on_line(len - 1.2) + r * 0.42, r, 0.14, 0.12),
                ring(on_line(len - 0.35) + r * 0.82, r, 0.17, 0.14),
                ring(on_line(len + 0.3) + r * 0.55, r, 0.02, 0.02),
            ],
            &KEEL,
        );
    }
    if b.fine() {
        // Heat spines along the breech's top, and a feed line down each side of the
        // jacket below its slots.
        machined(b);
        for (t, h) in [(-0.35, 0.6), (0.3, 0.46)] {
            let root = on_line(t) + up * 0.7;
            sleeve(
                b,
                &[
                    ring(root, d, 0.12, 0.18),
                    ring(root + up * h - d * 0.5, d, 0.02, 0.02),
                ],
                &KEEL,
            );
        }
        metal(b);
        for side in [1.0f32, -1.0] {
            let s = across * side;
            cable(
                b,
                &[
                    on_line(-0.3) + s * 0.72 - up * 0.42,
                    on_line(1.0) + s * 0.66 - up * 0.42,
                    on_line(band - 0.2) + s * 0.62 - up * 0.4,
                ],
                0.08,
            );
        }
    }
}

/// The beam line's hexagon `t` metres ahead of the trunnions, `r` across, as `sleeve`
/// lays `HEX` out.
fn hex_ring(t: f32, r: f32) -> Vec<Vec3> {
    let (_, up, across) = line();
    HEX.iter()
        .map(|&[s, u]| on_line(t) + (across * s + up * u) * r)
        .collect()
}

/// The muzzle collar's open front, an armoured lip with a red gasket round the bore, and
/// the bore: a cavity sunk into the collar, the beam leaving the middle of its mouth.
fn muzzle(b: &mut MeshBuilder, len: f32) {
    let outer = hex_ring(len, 0.52);
    let mouth = hex_ring(len, 0.32);
    machined(b);
    b.with_facets(|b| b.loft(&[hex_ring(len - 0.3, 0.6), outer.clone()], false, false));
    b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
    for k in 0..HEX.len() {
        let n = (k + 1) % HEX.len();
        b.face(&[outer[k], mouth[k], mouth[n], outer[n]]);
    }
    metal(b);
    b.with_facets(|b| b.loft(&[mouth, hex_ring(len - 0.28, 0.22)], false, true));
}

/// The crystal behind the breech, between two broad slotted cage plates either side of
/// it, a narrow rail over and under it, and three vanes raked back off the cage.
fn cage(b: &mut MeshBuilder, back: f32) {
    let (d, up, across) = line();
    let waist = on_line(-(back + 0.85) * 0.5 - 0.1);
    nanite_plate(b);
    sleeve(
        b,
        &[
            ring(on_line(-back), up, 0.02, 0.02),
            ring(waist, up, 0.62, 0.62),
            ring(on_line(-0.8), up, 0.44, 0.44),
        ],
        &DIAMOND,
    );
    for side in [1.0f32, -1.0] {
        let out = across * side;
        sleeve(
            b,
            &[
                ring(on_line(-0.7) + out * 0.78, out, 0.62, 0.12),
                ring(on_line(-back + 0.35) + out * 0.78, out, 0.56, 0.12),
            ],
            &SLAB,
        );
    }
    machined(b);
    for side in [1.0f32, -1.0] {
        let out = up * side;
        sleeve(
            b,
            &[
                ring(on_line(-0.7) + out * 0.66, out, 0.2, 0.1),
                ring(waist + out * 0.78, out, 0.18, 0.12),
                ring(on_line(-back + 0.15) + out * 0.3, out, 0.1, 0.06),
            ],
            &KEEL,
        );
    }
    for k in 0..3 {
        let a = 0.5 + std::f32::consts::TAU * (k as f32 / 3.0);
        let r = up * a.cos() + across * a.sin();
        let root = waist + r * 0.8 + d * 0.15;
        sleeve(
            b,
            &[
                ring(root, r, 0.26, 0.08),
                ring(root + r * 0.45 - d * 0.4, r, 0.22, 0.08),
                ring(root + r * 0.65 - d * 1.0, r, 0.02, 0.02),
            ],
            &KEEL,
        );
    }
}

/// From far off: the pedestal as a bar and the projector as another, still folding.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.module("aux_eng", 0.3, |b| {
        machined(b);
        b.with_fold(AUX_HINGE, AUX_STOWED, |b| {
            b.beam(AUX_HINGE, AUX_WRIST, Vec2::splat(0.9), Vec2::splat(1.4))
        });
        b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, |b| {
            b.beam(BACK, AUX_EMITTER, Vec2::splat(1.4), Vec2::splat(0.8))
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trunnions_are_on_the_beam_line() {
        let (d, _, _) = line();
        let off = (AUX_WRIST - BACK) - d * (AUX_WRIST - BACK).dot(d);
        assert!(off.length() < 1e-3, "{off}");
    }
}
