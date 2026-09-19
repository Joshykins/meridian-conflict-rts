//! Map props: trees, rocks and city buildings. Authored at scale 1; the
//! renderer applies each prop's own scale and heading.

use glam::{Vec2, Vec3};

use super::builder::{chamfered_rect, hash_unit, MeshBuilder};
use super::library::ModelDef;
use super::material::*;

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("tree_conifer", 3.4, 14.0, tree_conifer),
    ModelDef::new("tree_pine", 3.0, 18.0, tree_pine),
    ModelDef::new("tree_broadleaf", 5.0, 12.0, tree_broadleaf),
    ModelDef::new("tree_dead", 2.6, 9.0, tree_dead),
    ModelDef::new("rock_small", 2.2, 1.8, rock_small),
    ModelDef::new("rock_large", 6.0, 5.0, rock_large),
    ModelDef::new("building_small", 12.0, 9.0, building_small),
    ModelDef::new("building_medium", 17.0, 20.0, building_medium),
    ModelDef::new("building_wide", 23.0, 14.0, building_wide),
    ModelDef::new("building_tower", 15.0, 64.0, building_tower),
];

fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

// ---- trees -------------------------------------------------------------------

/// Tapered trunk from the ground to `top`.
fn trunk(b: &mut MeshBuilder, top: Vec3, radius: f32) {
    b.paint(BARK);
    let sides = if b.coarse() { 3 } else { b.sides(6) };
    // Rooted a metre down so trees on slopes never float.
    b.cylinder_between(v3(0.0, 0.0, -1.0), top, radius * 1.1, radius * 0.45, sides);
}

/// Stacked, slightly offset foliage cones from `z0` up to `z1`.
fn conifer_tiers(b: &mut MeshBuilder, tiers: usize, z0: f32, z1: f32, radius: f32, seed: u32) {
    b.paint(FOLIAGE);
    if b.coarse() {
        b.prism(v3(0.0, 0.0, z0), 5, radius, 0.0, z1 - z0);
        return;
    }
    let tiers = if b.fine() { tiers } else { tiers.div_ceil(2) };
    let step = (z1 - z0) / (tiers as f32 + 0.6);
    for i in 0..tiers {
        let t = i as f32 / tiers as f32;
        let r = radius * (1.0 - 0.72 * t);
        let lean = Vec2::new(hash_unit(seed, i as u32) - 0.5, hash_unit(seed, 100 + i as u32) - 0.5) * 0.25 * radius;
        let base = v3(lean.x * t, lean.y * t, z0 + step * i as f32);
        b.yawed(base, hash_unit(seed, 200 + i as u32) * 3.0, |b| {
            // A shallow skirt under each cone reads as drooping boughs.
            b.prism(Vec3::ZERO, 7, r * 0.55, r, step * 0.28);
            b.prism(v3(0.0, 0.0, step * 0.28), 7, r, if i + 1 == tiers { 0.0 } else { r * 0.22 }, step * 1.5);
        });
    }
}

fn tree_conifer(b: &mut MeshBuilder, _tech: u8) {
    trunk(b, v3(0.0, 0.0, 6.0), 0.45);
    conifer_tiers(b, 4, 2.2, 14.0, 3.3, 11);
}

fn tree_pine(b: &mut MeshBuilder, _tech: u8) {
    trunk(b, v3(0.3, 0.1, 15.0), 0.5);
    conifer_tiers(b, 3, 9.5, 18.0, 2.9, 23);
    if b.fine() {
        // Dead lower branch stubs.
        b.paint(BARK);
        b.cylinder_between(v3(0.1, 0.0, 6.0), v3(1.5, 0.5, 6.8), 0.12, 0.05, 4);
        b.cylinder_between(v3(0.15, 0.0, 7.6), v3(-1.0, -0.9, 8.3), 0.11, 0.05, 4);
    }
}

fn tree_broadleaf(b: &mut MeshBuilder, _tech: u8) {
    trunk(b, v3(0.2, 0.0, 6.5), 0.6);
    b.paint(FOLIAGE);
    if b.coarse() {
        b.lumpy_spheroid(v3(0.0, 0.0, 8.0), v3(4.6, 4.6, 3.9), 5, 3, 0.0, 0);
        return;
    }
    let crowns: &[(Vec3, Vec3)] = &[
        (v3(0.2, 0.0, 8.3), v3(3.6, 3.6, 3.5)),
        (v3(2.2, 1.2, 7.0), v3(2.5, 2.4, 2.1)),
        (v3(-2.0, 1.6, 7.3), v3(2.4, 2.5, 2.2)),
        (v3(-0.6, -2.4, 7.1), v3(2.6, 2.4, 2.2)),
    ];
    let (sides, rings, count) = if b.fine() { (7, 4, crowns.len()) } else { (5, 3, 2) };
    for (i, &(center, radii)) in crowns.iter().take(count).enumerate() {
        let radii = if b.fine() { radii } else { radii * 1.25 };
        b.lumpy_spheroid(center, radii, sides, rings, 0.16, 40 + i as u32);
    }
    if b.fine() {
        b.paint(BARK);
        b.cylinder_between(v3(0.1, 0.0, 4.2), v3(2.0, 1.0, 6.4), 0.25, 0.12, 5);
        b.cylinder_between(v3(0.1, 0.0, 4.6), v3(-1.8, 1.4, 6.6), 0.25, 0.12, 5);
        b.cylinder_between(v3(0.1, 0.0, 4.4), v3(-0.5, -2.0, 6.4), 0.22, 0.12, 5);
    }
}

fn tree_dead(b: &mut MeshBuilder, _tech: u8) {
    trunk(b, v3(0.4, -0.2, 8.6), 0.5);
    if b.coarse() {
        b.paint(BARK);
        b.cylinder_between(v3(0.15, 0.0, 4.0), v3(2.2, 0.8, 7.0), 0.22, 0.1, 3);
        b.cylinder_between(v3(0.2, 0.0, 5.0), v3(-1.9, -0.8, 7.6), 0.2, 0.1, 3);
        return;
    }
    let limbs: &[(Vec3, Vec3, f32)] = &[
        (v3(0.15, -0.05, 3.6), v3(2.3, 0.9, 6.4), 0.24),
        (v3(0.2, -0.1, 4.8), v3(-1.9, -1.0, 7.4), 0.22),
        (v3(0.3, -0.12, 6.0), v3(0.6, 1.9, 8.2), 0.17),
        (v3(2.3, 0.9, 6.4), v3(2.6, 0.2, 7.9), 0.1),
        (v3(-1.9, -1.0, 7.4), v3(-2.5, -0.2, 8.6), 0.09),
        (v3(1.3, 0.5, 5.1), v3(1.9, 1.9, 5.9), 0.1),
    ];
    let count = if b.fine() { limbs.len() } else { 3 };
    b.paint(BARK);
    for &(from, to, radius) in limbs.iter().take(count) {
        b.cylinder_between(from, to, radius, radius * 0.4, 4);
    }
}

// ---- rocks ---------------------------------------------------------------------

fn boulder(b: &mut MeshBuilder, center: Vec3, radii: Vec3, seed: u32) {
    b.paint(ROCK);
    let (sides, rings) = match b.lod() {
        0 => (8, 5),
        1 => (6, 4),
        _ => (5, 3),
    };
    // Sunk a little so the jagged underside never shows.
    b.lumpy_spheroid(center - Vec3::Z * radii.z * 0.25, radii, sides, rings, 0.22, seed);
}

fn rock_small(b: &mut MeshBuilder, _tech: u8) {
    boulder(b, v3(0.0, 0.0, 0.75), v3(2.0, 1.5, 1.25), 5);
    if b.fine() {
        boulder(b, v3(1.3, -1.1, 0.35), v3(0.8, 0.7, 0.55), 6);
    }
}

fn rock_large(b: &mut MeshBuilder, _tech: u8) {
    boulder(b, v3(-0.4, 0.3, 2.2), v3(4.8, 3.9, 3.4), 7);
    if b.mid() {
        b.yawed(v3(3.3, -2.4, 0.0), 0.8, |b| boulder(b, v3(0.0, 0.0, 1.0), v3(2.4, 1.7, 1.6), 8));
    }
    if b.fine() {
        boulder(b, v3(-3.6, -3.0, 0.5), v3(1.3, 1.1, 0.9), 9);
        boulder(b, v3(1.0, 4.2, 0.6), v3(1.5, 1.1, 1.0), 10);
    }
}

// ---- city buildings --------------------------------------------------------------

/// Concrete block with lit window bands on every storey (every other storey
/// at reduced detail). `base` is the ground-level centre.
fn storeys(b: &mut MeshBuilder, base: Vec3, size: Vec3, chamfer: f32, storey: f32) {
    b.paint(CONCRETE);
    // Ground-level blocks reach 2 m down so buildings on slopes never float.
    let sunk = if base.z == 0.0 { 2.0 } else { 0.0 };
    if b.coarse() {
        b.cuboid_open(base + Vec3::Z * ((size.z - sunk) * 0.5), size + Vec3::Z * sunk);
        return;
    }
    // Corner chamfers only read up close.
    let plan = chamfered_rect(Vec2::new(size.x, size.y) * 0.5, if b.fine() { chamfer } else { 0.0 });
    b.at(base, |b| {
        b.extrude_z(&plan, -sunk, size.z);
        let floors = ((size.z - 1.5) / storey).floor() as usize;
        let (stride, band) = if b.fine() { (1, storey * 0.45) } else { (2, storey * 0.9) };
        // Bands stand 12 cm proud of the wall.
        let grow = Vec2::new(1.0 + 0.24 / size.x, 1.0 + 0.24 / size.y);
        b.paint(WINDOWS);
        for floor in (0..floors).step_by(stride) {
            let z = 1.6 + storey * floor as f32;
            let ring = |z: f32| plan.iter().map(|p| v3(p[0] * grow.x, p[1] * grow.y, z)).collect::<Vec<_>>();
            b.loft(&[ring(z), ring(z + band)], false, false);
        }
    });
}

/// Rooftop clutter at full detail: a parapet around `size` and `units` air handlers.
fn rooftop(b: &mut MeshBuilder, center: Vec3, size: Vec2, units: u32, seed: u32) {
    if !b.fine() {
        return;
    }
    b.paint(CONCRETE);
    for side in [-1.0, 1.0] {
        b.cuboid_open(center + v3(side * (size.x - 0.5) * 0.5, 0.0, 0.4), v3(0.5, size.y, 0.8));
        b.cuboid_open(center + v3(0.0, side * (size.y - 0.5) * 0.5, 0.4), v3(size.x - 1.0, 0.5, 0.8));
    }
    b.paint(METAL);
    for i in 0..units {
        let p = Vec2::new(hash_unit(seed, i) - 0.5, hash_unit(seed, 10 + i) - 0.5) * size * 0.6;
        b.cuboid_open(center + v3(p.x, p.y, 0.6), v3(2.2 + hash_unit(seed, 20 + i) * 1.5, 1.8, 1.2));
    }
}

fn building_small(b: &mut MeshBuilder, _tech: u8) {
    storeys(b, Vec3::ZERO, v3(18.0, 14.0, 7.5), 0.0, 3.2);
    // Pitched roof.
    b.paint(ACCENT);
    b.extrude_x(&[[-7.6, 7.5], [7.6, 7.5], [0.0, 9.6]], -9.4, 9.4);
    if b.mid() {
        b.paint(CONCRETE);
        b.cuboid_open(v3(11.5, -2.0, 1.1), v3(5.0, 8.0, 4.2));
    }
    if b.fine() {
        // Chimney, dormers, annex roof lights.
        b.paint(CONCRETE);
        b.block(v3(-5.5, 2.4, 8.0), v3(-4.3, 3.6, 10.6));
        b.paint(ACCENT);
        for x in [-2.0, 3.5] {
            b.extrude_x(&[[-7.2, 7.6], [-4.4, 7.6], [-4.4, 8.4], [-5.8, 9.0], [-7.2, 8.4]], x, x + 2.2);
        }
        b.paint(WINDOWS);
        for x in [-2.0, 3.5] {
            b.block(v3(x + 0.5, -7.3, 7.7), v3(x + 1.7, -7.2, 8.4));
        }
        b.plate(v3(11.5, -2.0, 3.2), Vec2::new(2.0, 4.0), 0.3, 0.15);
    }
}

fn building_medium(b: &mut MeshBuilder, _tech: u8) {
    storeys(b, Vec3::ZERO, v3(26.0, 22.0, 16.0), 1.5, 3.4);
    if b.mid() {
        storeys(b, v3(-4.0, 0.0, 16.0), v3(14.0, 16.0, 4.0), 1.0, 3.4);
    }
    rooftop(b, v3(7.5, 0.0, 16.0), Vec2::new(9.0, 20.0), 3, 3);
}

fn building_wide(b: &mut MeshBuilder, _tech: u8) {
    // Low slab with two wings and a raised atrium.
    storeys(b, Vec3::ZERO, v3(38.0, 20.0, 9.5), 1.0, 3.6);
    storeys(b, v3(-11.0, 0.0, 0.0), v3(12.0, 30.0, 12.5), 1.0, 3.6);
    if b.mid() {
        storeys(b, v3(12.0, 0.0, 0.0), v3(10.0, 28.0, 7.0), 1.0, 3.6);
        b.paint(GLASS);
        b.extrude_x(&[[-5.0, 9.5], [5.0, 9.5], [0.0, 13.0]], -4.0, 6.0);
    }
    rooftop(b, v3(-11.0, 0.0, 12.5), Vec2::new(11.0, 28.0), 4, 5);
}

fn building_tower(b: &mut MeshBuilder, _tech: u8) {
    // Podium, setback shaft, crown and mast.
    storeys(b, Vec3::ZERO, v3(28.0, 28.0, 10.0), 3.0, 3.6);
    storeys(b, v3(0.0, 0.0, 10.0), v3(20.0, 20.0, 34.0), 3.0, 3.8);
    if b.mid() {
        storeys(b, v3(0.0, 0.0, 44.0), v3(14.0, 14.0, 12.0), 2.5, 3.8);
        b.paint(CONCRETE);
        b.prism(v3(0.0, 0.0, 56.0), 4, 6.0, 2.0, 3.0);
    } else {
        b.paint(CONCRETE);
        b.frustum_open(v3(0.0, 0.0, 44.0), Vec2::splat(14.0), Vec2::splat(9.0), 15.0, Vec2::ZERO);
    }
    if b.fine() {
        b.paint(METAL);
        b.cylinder_between(v3(0.0, 0.0, 59.0), v3(0.0, 0.0, 66.0), 0.4, 0.12, 5);
        b.paint(WINDOWS);
        b.cuboid(v3(0.0, 0.0, 66.0), Vec3::splat(0.6));
    }
    rooftop(b, v3(0.0, 0.0, 10.0), Vec2::new(27.0, 27.0), 0, 9);
    rooftop(b, v3(0.0, 0.0, 44.0), Vec2::new(19.0, 19.0), 0, 9);
}
