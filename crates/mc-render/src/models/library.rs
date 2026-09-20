//! The model catalogue: every mesh key, the size it is authored at, and the
//! assembly of its three levels of detail into a [`Model`].

use glam::{Affine3A, Vec3};

use super::builder::MeshBuilder;
use super::{aster, part, props, MeshLod, Model, LOD_COUNT};

/// Highest tech level a model distinguishes.
pub(super) const MAX_TECH: u8 = 3;

/// One entry of the catalogue.
pub(super) struct ModelDef {
    pub key: &'static str,
    /// (collision radius, height) the model is authored at, per tech level.
    /// Most models have one authored size; structures that grow with tech
    /// (factory, extractor) list a taller one per tier.
    pub nominal: [(f32, f32); MAX_TECH as usize],
    /// Emits the model at the builder's level of detail for tech level `1..=3`.
    pub build: fn(&mut MeshBuilder, u8),
}

impl ModelDef {
    pub const fn new(
        key: &'static str,
        radius: f32,
        height: f32,
        build: fn(&mut MeshBuilder, u8),
    ) -> Self {
        ModelDef {
            key,
            nominal: [(radius, height); MAX_TECH as usize],
            build,
        }
    }

    pub const fn tiered(
        key: &'static str,
        nominal: [(f32, f32); MAX_TECH as usize],
        build: fn(&mut MeshBuilder, u8),
    ) -> Self {
        ModelDef {
            key,
            nominal,
            build,
        }
    }
}

fn catalogue() -> impl Iterator<Item = &'static ModelDef> {
    aster::MODELS.iter().chain(props::MODELS.iter())
}

/// Every key [`build_model`] understands: units and structures first, then map props.
pub fn all_model_keys() -> Vec<&'static str> {
    catalogue().map(|def| def.key).collect()
}

/// Builds `key` at its authored (tech 1 blueprint) size.
pub fn build_model(key: &str) -> Option<Model> {
    let (radius, height) = catalogue().find(|def| def.key == key)?.nominal[0];
    build_model_scaled(key, radius, height, 1)
}

/// Builds `key` fitted to a blueprint's collision `radius` and `height`.
/// Higher `tech` (1..=3) adds modules, antennae and glow emitters.
pub fn build_model_scaled(key: &str, radius: f32, height: f32, tech: u8) -> Option<Model> {
    let def = catalogue().find(|def| def.key == key)?;
    let tech = tech.clamp(1, MAX_TECH);
    let (nominal_radius, nominal_height) = def.nominal[tech as usize - 1];
    let horizontal = radius / nominal_radius;
    let root = Affine3A::from_scale(Vec3::new(horizontal, horizontal, height / nominal_height));

    let mut pivots = (Vec3::ZERO, Vec3::ZERO);
    let mut treads = None;
    let mut legs = None;
    let mut arm_pivot = None;
    let lods: [MeshLod; LOD_COUNT] = std::array::from_fn(|lod| {
        let mut builder = MeshBuilder::new(lod, root);
        (def.build)(&mut builder, tech);
        if lod == 0 {
            pivots = (builder.turret_pivot(), builder.spinner_pivot());
            treads = builder.treads();
            legs = builder.legs();
            arm_pivot = builder.arm_pivot();
        }
        builder.finish()
    });
    let bounds_radius = lods
        .iter()
        .map(|lod| bounds_radius(lod, pivots.0, pivots.1))
        .fold(0.0, f32::max);
    Some(Model {
        key: key.to_owned(),
        lods,
        turret_pivot: pivots.0.to_array(),
        spinner_pivot: pivots.1.to_array(),
        bounds_radius,
        treads,
        legs,
        arm_pivot,
    })
}

/// One level of detail of `key`, still in its builder, so tests can inspect the solids.
#[cfg(test)]
pub(super) fn build_lod(key: &str, lod: usize, tech: u8) -> MeshBuilder {
    let def = catalogue().find(|def| def.key == key).expect("known key");
    let mut builder = MeshBuilder::new(lod, Affine3A::IDENTITY);
    (def.build)(&mut builder, tech);
    builder
}

/// Radius around the origin that holds the mesh with its turret and spinner at any yaw.
fn bounds_radius(mesh: &MeshLod, turret_pivot: Vec3, spinner_pivot: Vec3) -> f32 {
    mesh.vertices
        .iter()
        .map(|v| {
            let p = Vec3::from(v.pos);
            let horizontal = match v.part {
                part::TURRET => {
                    turret_pivot.truncate().length() + (p - turret_pivot).truncate().length()
                }
                part::SPINNER => {
                    spinner_pivot.truncate().length() + (p - spinner_pivot).truncate().length()
                }
                _ => p.truncate().length(),
            };
            horizontal.hypot(p.z)
        })
        .fold(0.0, f32::max)
}

/// Model key for a map prop, from `mc_map::PropKind::raw()`. Unknown kinds
/// fall back by family (tree, rock, building) so new kinds still draw.
pub fn prop_model_key(kind_raw: u16) -> &'static str {
    match kind_raw {
        0 => "tree_broadleaf",
        1 => "tree_conifer",
        2 => "tree_pine",
        3 => "tree_dead",
        4..=15 => "tree_broadleaf",
        16 => "rock_small",
        17 => "rock_large",
        18..=31 => "rock_small",
        32 => "building_small",
        33 => "building_medium",
        34 => "building_wide",
        35 => "building_tower",
        _ => "building_small",
    }
}
