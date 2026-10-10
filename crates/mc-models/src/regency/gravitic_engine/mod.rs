//! The Fallen (`regency_t5_gravitic_engine`, mesh "regency_gravitic_engine"): the
//! Regency's tech 5 gravitic engine, authored at its unit file's size (metres; a 22 x 22
//! lot of 12 m cells, 264 m square, 240 m high).
//!
//! A crown of six great black blade-shards standing round a well, leaning in and twisting
//! as they rise to close to a point high over it, open gaps between them all the way up
//! and between their points; six short shards stand low before the gaps. Each shard
//! (`shard`) is a faceted graphite core under courses of dark plate, its points the
//! plates' swept edges, its inner faces glazed so the held energy shows through them.
//! Between the shards' feet stand the works (`works`): gantries spanning blade to blade,
//! coil drums, conduits running in to the well; round them tall spike masts, their stays
//! strung down to the ground and across to the shards. A low ring wall closes the site
//! (`site`).
//!
//! Inside, machinery turns about the z axis (`rotors`), seen through the gaps: a stack of
//! rotor drums rising out of the well, neighbours turning opposite ways, and two great
//! bands round the inside of the shards. They are orbit pieces
//! (`MeshBuilder::with_orbit`).
//!
//! The energy itself is not part of the model: the renderer draws it, violet, held in the
//! open room between the shards at [`CORE`] (model metres, at the unit file's size), with
//! prongs standing up off the well's rim to anchor it from below. Nothing of the model
//! comes within `CORE_CLEAR` of that point.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plate over dark graphite works, red
//! optics in the shards' boots, light only in the plates (`pattern::NANITE_SLOT`,
//! `NANITE_RIM`, `EMBER_RIM`, `EMBER_SLOT`), never a lit piece of mesh.

mod rotors;
mod shard;
mod site;
#[cfg(test)]
mod tests;
mod works;

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

/// Where the engine holds its energy, model metres at the unit file's size: the middle of
/// the open room between its shards, over the well's middle.
pub(super) const CORE: Vec3 = Vec3::new(0.0, 0.0, 108.0);
/// How far round the core the model leaves open for the energy.
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

/// A great band round the inside of the shards at `z`, `r` out, turning at `rate`.
fn band(z: f32, r: f32, rate: f32) -> Rotor {
    Rotor {
        z0: z - 6.0,
        z1: z + 6.0,
        outer: r,
        inner: r - 7.0,
        vanes: 18,
        rate,
        lit: true,
    }
}

fn dir(deg: f32) -> Vec3 {
    let a = deg.to_radians();
    v3(a.cos(), a.sin(), 0.0)
}

/// Far off: the shards, then the owner's colour.
fn finish_coarse(b: &mut MeshBuilder, shards: &[Shard]) {
    for s in shards {
        s.coarse(b);
    }
    b.paint(TEAM);
    b.decal(Vec3::Z * 3.0, Vec2::splat(40.0));
}

fn crown_shards() -> Vec<Shard> {
    let tall = [240.0, 222.0, 234.0, 214.0, 230.0, 220.0];
    let mut shards = Vec::new();
    for (k, &height) in tall.iter().enumerate() {
        let a = 60.0 * k as f32 + 15.0;
        shards.push(Shard {
            root: dir(a) * 70.0 + Vec3::Z,
            tip: dir(a + 18.0) * 20.0 + Vec3::Z * height,
            back: dir(a),
            half: 16.0,
            depth: 8.0,
            skew: if k % 2 == 0 { 0.7 } else { -0.5 },
            plates: 5,
            seed: k as u32,
        });
    }
    for k in 0..6 {
        let a = 60.0 * k as f32 + 45.0;
        shards.push(Shard {
            root: dir(a) * 100.0 + Vec3::Z,
            tip: dir(a + 5.0) * 78.0 + Vec3::Z * (80.0 + 10.0 * jitter(40 + k as u32)),
            back: dir(a),
            half: 9.0,
            depth: 5.0,
            skew: if k % 2 == 0 { -0.6 } else { 0.6 },
            plates: 2,
            seed: 10 + k as u32,
        });
    }
    shards
}

/// Six tall shards closing to a point over the well, six short ones before the gaps.
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
    site::well(b, 34.0, 7.0);
    site::prongs(b, 3, 38.0, 70.0, CORE);
    let mut turning = rotors::stack(4, 33.0, 70.0, 0.2);
    turning.push(band(62.0, 46.0, -0.15));
    turning.push(band(88.0, 37.0, 0.12));
    rotors::build(b, &turning);
    b.set_gravity_core(CORE, CORE_CLEAR);
    for s in &shards {
        s.build(b);
    }
    let (inner, outer) = shards.split_at(6);
    for k in 0..6 {
        works::span(b, &inner[k], &inner[(k + 1) % 6], &[16.0, 40.0]);
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
