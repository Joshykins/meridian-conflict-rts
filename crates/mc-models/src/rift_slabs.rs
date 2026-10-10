//! The ground a gravitic engine breaks (mc-render `renderer/gravitic_engine_fx`): slabs of
//! bedrock heaved up along its rift and lifted over its well, and the shards of rock it
//! hangs in the air. Scenery only: the renderer poses them as props, none of it is known
//! to the sim.
//!
//! A slab is a plate of rock as the ground breaks it: an uneven polygon in plan, its top
//! the ground's surface at z = 0 (so laid flat it sits flush), its sides broken square,
//! its underside wedged and torn, deeper at one end. A shard is a jagged lump, taller than
//! wide. Each is one convex block cut by fracture planes (`cliffs::Block`), so its faces
//! are flat and its edges sharp, and no two keys alike. Sizes are for scale 1; the
//! renderer scales each one.

use glam::Vec3;

use super::builder::MeshBuilder;
use super::cliffs::{Block, Roll};
use super::library::ModelDef;
use super::material::ROCK;

/// Half a slab's width at scale 1, metres, and its depth under the surface.
pub const SLAB_HALF: f32 = 6.0;
pub const SLAB_DEPTH: f32 = 3.2;
/// A shard's half width and height at scale 1.
pub const SHARD_HALF: f32 = 1.6;
pub const SHARD_HEIGHT: f32 = 4.0;

/// The slabs, then the shards. The renderer's slots for them are these keys, in order.
pub const SLAB_KEYS: [&str; 4] = ["rift_slab_a", "rift_slab_b", "rift_slab_c", "rift_slab_d"];
pub const SHARD_KEYS: [&str; 3] = ["rift_shard_a", "rift_shard_b", "rift_shard_c"];

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new(SLAB_KEYS[0], SLAB_HALF, SLAB_DEPTH, |b, _| slab(b, 7)),
    ModelDef::new(SLAB_KEYS[1], SLAB_HALF, SLAB_DEPTH, |b, _| slab(b, 19)),
    ModelDef::new(SLAB_KEYS[2], SLAB_HALF, SLAB_DEPTH, |b, _| slab(b, 31)),
    ModelDef::new(SLAB_KEYS[3], SLAB_HALF, SLAB_DEPTH, |b, _| slab(b, 53)),
    ModelDef::new(SHARD_KEYS[0], SHARD_HALF, SHARD_HEIGHT, |b, _| shard(b, 3)),
    ModelDef::new(SHARD_KEYS[1], SHARD_HALF, SHARD_HEIGHT, |b, _| shard(b, 11)),
    ModelDef::new(SHARD_KEYS[2], SHARD_HALF, SHARD_HEIGHT, |b, _| shard(b, 29)),
];

/// A plate of broken ground: a box cut round its sides into an uneven polygon, then its
/// underside wedged.
fn slab(b: &mut MeshBuilder, seed: u32) {
    b.paint(ROCK);
    let mut roll = Roll { seed, n: 0 };
    let (hx, hy) = (
        SLAB_HALF * roll.next(0.8, 1.0),
        SLAB_HALF * roll.next(0.6, 0.85),
    );
    let mut block = Block::cuboid(Vec3::new(-hx, -hy, -SLAB_DEPTH), Vec3::new(hx, hy, 0.0));
    // Its outline: vertical breaks at uneven turns round it, each taking off a corner.
    let sides = if b.coarse() { 3 } else { 5 };
    let start = roll.next(0.0, 1.0);
    for k in 0..sides {
        let a =
            std::f32::consts::TAU * (start + (k as f32 + roll.next(-0.25, 0.25)) / sides as f32);
        let n = Vec3::new(a.cos(), a.sin(), roll.next(-0.25, 0.25));
        let reach = hx.min(hy) * roll.next(0.75, 1.05);
        block.cut(n, reach);
    }
    // The underside torn away under a slant, deeper at one end.
    if !b.coarse() {
        for _ in 0..2 {
            let a = roll.next(0.0, std::f32::consts::TAU);
            let n = Vec3::new(
                a.cos() * roll.next(0.3, 0.8),
                a.sin() * roll.next(0.3, 0.8),
                -1.0,
            );
            let n = n.normalize();
            block.cut(n, SLAB_DEPTH * roll.next(0.55, 0.85));
        }
    }
    b.with_facets(|b| block.emit(b, true));
}

/// A lump of rock torn out of the ground: taller than wide, its top broken to a ridge.
fn shard(b: &mut MeshBuilder, seed: u32) {
    b.paint(ROCK);
    let mut roll = Roll { seed, n: 0 };
    let h = SHARD_HEIGHT * roll.next(0.8, 1.0);
    let mut block = Block::cuboid(
        Vec3::new(-SHARD_HALF, -SHARD_HALF * 0.8, 0.0),
        Vec3::new(SHARD_HALF, SHARD_HALF * 0.8, h),
    );
    let cuts = if b.coarse() { 2 } else { 6 };
    for _ in 0..cuts {
        let a = roll.next(0.0, std::f32::consts::TAU);
        let up = roll.next(-0.6, 1.2);
        let n = Vec3::new(a.cos(), a.sin(), up).normalize();
        let centre = Vec3::new(0.0, 0.0, h * 0.5);
        let reach = SHARD_HALF * roll.next(0.5, 0.9) + up.max(0.0) * h * 0.35;
        block.cut(n, n.dot(centre) + reach);
    }
    b.with_facets(|b| block.emit(b, true));
}

#[cfg(test)]
mod tests {
    use super::super::build_model;
    use super::{SHARD_KEYS, SLAB_KEYS};

    /// Every piece builds at every level, and a slab's top stays at the ground (z 0).
    #[test]
    fn every_slab_and_shard_builds_and_slabs_sit_flush() {
        for key in SLAB_KEYS.iter().chain(&SHARD_KEYS) {
            let model = build_model(key).expect("in the catalogue");
            for lod in &model.lods {
                assert!(!lod.indices.is_empty(), "{key}: an empty level");
            }
        }
        for key in SLAB_KEYS {
            let model = build_model(key).expect("in the catalogue");
            let top = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MIN, f32::max);
            assert!(top.abs() < 1e-3, "{key}: its top at {top}");
        }
    }
}
