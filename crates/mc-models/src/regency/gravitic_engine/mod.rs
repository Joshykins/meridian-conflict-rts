//! The Fallen (`regency_t5_gravitic_engine`, mesh "regency_gravitic_engine"): the
//! Regency's tech 5 gravitic engine, authored at its unit file's size (metres; a 22 x 22
//! lot of 12 m cells, 264 m square, 240 m high).
//!
//! A crown of great black blade-shards standing round a well, leaning in over it. Each
//! shard (`shard`) is a faceted graphite core under courses of dark plate, its points
//! the plates' swept edges, its inner faces glazed so the held energy shows through
//! them. Between the shards' feet stand the works (`works`): gantries spanning blade to
//! blade, coil drums, conduits running in to the well; round them tall spike masts, their
//! stays strung down to the ground and across to the shards. A low ring wall closes the
//! site (`site`).
//!
//! The energy itself is not part of the model: the renderer draws it, violet, held in the
//! open room between the shards at [`CORE`] (model metres, at the unit file's size), with
//! prongs standing up out of the well floor to anchor it from below. Nothing of the model
//! comes within [`CORE_CLEAR`] of that point.
//!
//! Three designs are drawn while the user picks one (CLAUDE.md section 9):
//! - `regency_gravitic_engine` (A): the crown. Six tall shards close to a point over the
//!   well, twisting as they rise; six shorter ones lean in before them.
//! - `regency_gravitic_engine~b` (B): the open ring. Nine shards, tall and short in turn,
//!   stand nearly upright round a deep stepped well open to the sky, six masts strung to
//!   their points.
//! - `regency_gravitic_engine~c` (C): the split pyramid. Eight broad shards make a low
//!   pyramid's four faces, split down the middle of each by a cleft; four tall spires
//!   stand at its corners, the energy held over the crossing between them.
//!
//! Inside each, machinery turns about the z axis (`rotors`): a stack of rotor drums rising
//! out of the well, neighbours turning opposite ways, and a great band round the inside
//! of the shards. They are orbit pieces (`MeshBuilder::with_orbit`): today they turn at
//! their own rates while the engine stands powered.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plate over dark graphite works, red
//! optics in the shards' boots, light only in the plates (`pattern::NANITE_SLOT`,
//! `EMBER_RIM`, `EMBER_SLOT`), never a lit piece of mesh.

mod rotors;
mod shard;
mod site;
#[cfg(test)]
mod tests;
mod works;

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::{hash_unit, MeshBuilder};
use crate::material::TEAM;

use super::kit::v3;
use rotors::Rotor;
use shard::Shard;

/// The unit file's size.
pub(super) const RADIUS: f32 = 125.0;
pub(super) const HEIGHT: f32 = 240.0;
/// The ring wall: inner and outer radius, and its top.
const RING_IN: f32 = 119.0;
const RING_OUT: f32 = 125.0;
const RING_TOP: f32 = 9.0;

/// Where each design holds its energy, model metres at the unit file's size: the middle of
/// the open room between its shards, over the well's middle.
pub(super) const CORE: [Vec3; 3] = [
    Vec3::new(0.0, 0.0, 105.0),
    Vec3::new(0.0, 0.0, 122.0),
    Vec3::new(0.0, 0.0, 112.0),
];
/// How far round each core the model leaves open for the energy.
#[cfg(test)]
pub(super) const CORE_CLEAR: f32 = 20.0;

/// A number from -1 to 1 for `seed`.
fn jitter(seed: u32) -> f32 {
    hash_unit(0x6a71, seed) * 2.0 - 1.0
}

/// A block on the ground from `a` to `c` (their heights ignored), `half` its half width,
/// standing `h.0` high at `a` and `h.1` at `c`: footings and anchor blocks.
fn footing(b: &mut MeshBuilder, a: Vec3, c: Vec3, half: f32, h: (f32, f32)) {
    let d = (c - a).with_z(0.0).normalize_or(Vec3::X);
    let side = d.cross(Vec3::Z) * half;
    let end = |p: Vec3, h: f32| {
        let p = p.with_z(0.0);
        vec![
            p - side,
            p + side,
            p + side + Vec3::Z * h,
            p - side + Vec3::Z * h,
        ]
    };
    b.with_facets(|b| b.loft(&[end(a, h.0), end(c, h.1)], true, true));
}

/// The great band round the inside of the shards at `z`, `r` out, turning at `rate`.
fn band(z: f32, r: f32, rate: f32) -> Rotor {
    Rotor {
        z0: z - 5.0,
        z1: z + 5.0,
        outer: r,
        inner: r - 6.0,
        vanes: 18,
        rate,
    }
}

fn dir(deg: f32) -> Vec3 {
    let a = deg.to_radians();
    v3(a.cos(), a.sin(), 0.0)
}

/// What every design ends with: the shards, then the owner's colour far off.
fn finish_coarse(b: &mut MeshBuilder, shards: &[Shard]) {
    for s in shards {
        s.coarse(b);
    }
    b.paint(TEAM);
    b.decal(Vec3::Z * 3.0, Vec2::splat(40.0));
}

// ---- A: the crown ---------------------------------------------------------------------

fn crown_shards() -> Vec<Shard> {
    let tall = [240.0, 222.0, 234.0, 214.0, 230.0, 220.0];
    let mut shards = Vec::new();
    for (k, &height) in tall.iter().enumerate() {
        let a = 60.0 * k as f32 + 15.0;
        shards.push(Shard {
            root: dir(a) * 70.0 + Vec3::Z,
            tip: dir(a + 16.0) * 9.0 + Vec3::Z * height,
            back: dir(a),
            half: 22.0,
            depth: 9.0,
            skew: if k % 2 == 0 { 0.7 } else { -0.5 },
            plates: 5,
            seed: k as u32,
        });
    }
    for k in 0..6 {
        let a = 60.0 * k as f32 + 45.0;
        shards.push(Shard {
            root: dir(a) * 97.0 + Vec3::Z,
            tip: dir(a + 6.0) * 47.0 + Vec3::Z * (132.0 + 18.0 * jitter(40 + k as u32)),
            back: dir(a),
            half: 14.0,
            depth: 6.0,
            skew: if k % 2 == 0 { -0.6 } else { 0.6 },
            plates: 3,
            seed: 10 + k as u32,
        });
    }
    shards
}

/// A: six tall shards closing to a point over the well, six shorter before them.
pub(super) fn crown(b: &mut MeshBuilder, _tech: u8) {
    let shards = crown_shards();
    let masts: Vec<(Vec3, Vec3)> = (0..4)
        .map(|k| {
            let d = dir(45.0 + 90.0 * k as f32);
            (
                d * 154.0 + Vec3::Z * 0.5,
                d * 146.0 + Vec3::Z * (178.0 + 14.0 * jitter(k)),
            )
        })
        .collect();
    if b.coarse() {
        for &(foot, top) in &masts {
            works::mast(b, foot, top, 8.0, 0);
        }
        return finish_coarse(b, &shards);
    }
    site::terrace(b);
    site::ring_wall(b, &[]);
    site::well(b, 34.0, 7.0, 0);
    site::prongs(b, 3, 38.0, 70.0, CORE[0], 0.0);
    let mut turning = rotors::stack(3, 31.0, 60.0, 0.2);
    turning.push(band(62.0, 42.0, -0.15));
    rotors::build(b, &turning);
    for s in &shards {
        s.build(b);
    }
    let (inner, outer) = shards.split_at(6);
    for k in 0..6 {
        works::span(b, &inner[k], &inner[(k + 1) % 6], &[14.0, 32.0, 52.0]);
        let a = 60.0 * k as f32 + 45.0;
        works::coil(b, dir(a) * 80.0 + Vec3::Z * 7.0, dir(a + 90.0), 4.5, 20.0);
        works::conduit(b, dir(a) * 74.0, dir(a) * 40.0, 3, 0.9);
    }
    // Spike masts on the terrace behind the tall shards, stayed to their backs.
    for (k, s) in inner.iter().enumerate() {
        let a = 60.0 * k as f32 + 15.0;
        let foot = dir(a) * 106.0 + Vec3::Z * 1.4;
        let [top, _] = works::mast(b, foot, foot + Vec3::Z * 96.0 - dir(a) * 4.0, 4.5, k as u32);
        works::stay(b, top, s.keel(0.42), 3.0, 0.45);
        works::stay(b, top, outer[k].keel(0.7), 2.0, 0.4);
        works::stay(b, top, outer[(k + 5) % 6].keel(0.7), 2.0, 0.4);
    }
    // The great masts at the lot's corners, outside the wall.
    for (k, &(foot, top)) in masts.iter().enumerate() {
        let [top, housing] = works::mast(b, foot, top, 8.0, k as u32);
        let d = foot.normalize();
        let side = d.cross(Vec3::Z);
        for s in [-1.0, 1.0] {
            let g = foot + side * (s * 26.0) - d * 4.0;
            works::anchor(b, g, foot);
            works::stay(b, top, g + Vec3::Z * 3.0, 1.0, 0.5);
        }
        let near = &outer[(k + 1) % 6];
        works::stay(b, top, near.keel(0.85), 6.0, 0.6);
        works::stay(b, housing, inner[k * 3 / 2].keel(0.3), 4.0, 0.5);
    }
}

// ---- B: the open ring -----------------------------------------------------------------

fn ring_shards() -> Vec<Shard> {
    (0..9)
        .map(|k| {
            let a = 40.0 * k as f32;
            let tall = k % 2 == 0;
            Shard {
                root: dir(a) * 84.0 + Vec3::Z,
                tip: if tall {
                    dir(a + 7.0) * 66.0 + Vec3::Z * (232.0 + 8.0 * jitter(60 + k))
                } else {
                    dir(a - 5.0) * 62.0 + Vec3::Z * (160.0 + 12.0 * jitter(70 + k))
                },
                back: dir(a),
                half: if tall { 17.0 } else { 13.0 },
                depth: if tall { 7.5 } else { 6.0 },
                skew: if tall { 0.5 } else { -0.7 },
                plates: if tall { 6 } else { 4 },
                seed: 20 + k,
            }
        })
        .collect()
}

/// B: nine shards round a deep well open to the sky, six masts strung to their points.
pub(super) fn open_ring(b: &mut MeshBuilder, _tech: u8) {
    let shards = ring_shards();
    let masts: Vec<(Vec3, Vec3, f32)> = (0..6)
        .map(|k| {
            let a = 20.0 + 60.0 * k as f32;
            let tall = k % 2 == 0;
            let h = if tall { 236.0 } else { 168.0 };
            (
                dir(a) * 106.0 + Vec3::Z * 1.4,
                dir(a) * 100.0 + Vec3::Z * h,
                a,
            )
        })
        .collect();
    if b.coarse() {
        for &(foot, top, _) in &masts {
            works::mast(b, foot, top, 6.0, 0);
        }
        return finish_coarse(b, &shards);
    }
    site::terrace(b);
    site::ring_wall(b, &[]);
    site::well(b, 54.0, 16.0, 0);
    site::prongs(b, 3, 58.0, 92.0, CORE[1], TAU / 6.0);
    let mut turning = rotors::stack(4, 51.0, 84.0, 0.16);
    turning.push(band(70.0, 66.0, -0.12));
    rotors::build(b, &turning);
    for s in &shards {
        s.build(b);
    }
    for k in 0..9 {
        let (s, n) = (&shards[k], &shards[(k + 1) % 9]);
        works::span(b, s, n, &[12.0, 30.0]);
    }
    for (k, &(foot, top, a)) in masts.iter().enumerate() {
        let [top, housing] = works::mast(b, foot, top, 6.0, k as u32);
        // Its two neighbours' points, across the gap.
        let left = &shards[((a - 20.0) / 40.0).round() as usize % 9];
        let right = &shards[((a + 20.0) / 40.0).round() as usize % 9];
        works::stay(b, top, left.keel(0.8), 5.0, 0.55);
        works::stay(b, top, right.keel(0.8), 5.0, 0.55);
        let out = dir(a) * 128.0;
        let g = out.clamp(Vec3::splat(-127.0), Vec3::splat(127.0));
        works::anchor(b, g, foot);
        works::stay(b, top, g + Vec3::Z * 3.0, 1.0, 0.55);
        works::stay(b, housing, left.keel(0.3), 3.0, 0.45);
        works::conduit(b, foot - dir(a) * 6.0, dir(a) * 62.0, 2, 1.0);
    }
}

// ---- C: the split pyramid -------------------------------------------------------------

fn pyramid_shards() -> Vec<Shard> {
    let mut shards = Vec::new();
    // The faces: two broad shards each, the cleft between them.
    for f in 0..4 {
        let a = 90.0 * f as f32;
        let (d, side) = (dir(a), dir(a + 90.0));
        for (i, s) in [-1.0f32, 1.0].into_iter().enumerate() {
            shards.push(Shard {
                root: d * 86.0 + side * (s * 37.0) + Vec3::Z,
                tip: d * 46.0 + side * (s * 27.0) + Vec3::Z * (96.0 + 8.0 * jitter(80 + f)),
                back: d,
                half: 23.0,
                depth: 8.0,
                skew: -0.6 * s,
                plates: 4,
                seed: 30 + f * 2 + i as u32,
            });
        }
    }
    // The corners: tall spires standing over the faces, round the held energy.
    let tall = [240.0, 222.0, 234.0, 216.0];
    for (q, &height) in tall.iter().enumerate() {
        let a = 45.0 + 90.0 * q as f32;
        shards.push(Shard {
            root: dir(a) * 102.0 + Vec3::Z,
            tip: dir(a + 4.0) * 56.0 + Vec3::Z * height,
            back: dir(a),
            half: 18.0,
            depth: 9.0,
            skew: if q % 2 == 0 { 0.6 } else { -0.6 },
            plates: 6,
            seed: 40 + q as u32,
        });
    }
    shards
}

/// C: a pyramid of eight broad shards split by two clefts, four tall shards over it.
pub(super) fn split_pyramid(b: &mut MeshBuilder, _tech: u8) {
    let shards = pyramid_shards();
    let masts: Vec<(Vec3, Vec3)> = (0..4)
        .map(|f| {
            let d = dir(90.0 * f as f32);
            (
                d * 112.0 + Vec3::Z * 1.4,
                d * 106.0 + Vec3::Z * (172.0 + 10.0 * jitter(90 + f)),
            )
        })
        .collect();
    if b.coarse() {
        for &(foot, top) in &masts {
            works::mast(b, foot, top, 6.0, 0);
        }
        return finish_coarse(b, &shards);
    }
    site::terrace(b);
    site::ring_wall(b, &[]);
    site::well(b, 30.0, 6.0, 0);
    site::prongs(b, 4, 34.0, 66.0, CORE[2], TAU / 8.0);
    let mut turning = rotors::stack(3, 27.0, 66.0, 0.2);
    turning.push(band(64.0, 40.0, -0.15));
    rotors::build(b, &turning);
    for s in &shards {
        s.build(b);
    }
    let (faces, corners) = shards.split_at(8);
    for f in 0..4 {
        let (l, r) = (&faces[f * 2], &faces[f * 2 + 1]);
        // Gantries bridging the cleft, and from each face to the corner shards beside it.
        works::span(b, l, r, &[18.0, 42.0, 66.0]);
        works::span(b, r, &corners[f], &[16.0, 40.0]);
        works::span(b, &corners[(f + 3) % 4], l, &[16.0, 40.0]);
        let d = dir(90.0 * f as f32);
        works::conduit(b, d * 104.0, d * 36.0, 3, 1.0);
        let [top, housing] = works::mast(b, masts[f].0, masts[f].1, 6.0, f as u32);
        works::stay(b, top, l.keel(0.75), 5.0, 0.55);
        works::stay(b, top, r.keel(0.75), 5.0, 0.55);
        works::stay(b, top, corners[f].keel(0.6), 6.0, 0.5);
        let side = dir(90.0 * f as f32 + 90.0);
        for s in [-1.0, 1.0] {
            let g = d * 127.0 + side * (s * 24.0);
            works::anchor(b, g, masts[f].0);
            works::stay(b, housing, g + Vec3::Z * 3.0, 1.0, 0.45);
        }
        works::coil(
            b,
            dir(90.0 * f as f32 + 45.0) * 76.0 + Vec3::Z * 7.0,
            dir(90.0 * f as f32 + 135.0),
            4.5,
            18.0,
        );
    }
}
