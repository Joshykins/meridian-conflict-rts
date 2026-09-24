//! The model catalogue: every mesh key, the size it is authored at, and the
//! assembly of its three levels of detail into a [`Model`].

use glam::{Affine3A, Vec3};

use super::builder::MeshBuilder;
use super::{aster, part, props, rig, MeshLod, Model, Pit, LOD_COUNT};

/// Highest tech level a model distinguishes.
pub(super) const MAX_TECH: u8 = 3;

/// One entry of the catalogue.
pub(super) struct ModelDef {
    pub key: &'static str,
    /// (collision radius, height) the model is authored at, per tech level.
    /// Most models have one authored size; structures that grow with tech
    /// (factory, extractor, radar, shield) list a taller one per tier.
    pub nominal: [(f32, f32); MAX_TECH as usize],
    /// Emits the model at the builder's level of detail for tech level `1..=max_tech`.
    pub build: fn(&mut MeshBuilder, u8),
    /// The highest tech with a look of its own: [`MAX_TECH`], or 4 for a model with a
    /// tier 4 (drawn at the tech 3 size).
    pub max_tech: u8,
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
            max_tech: MAX_TECH,
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
            max_tech: MAX_TECH,
        }
    }

    /// The model also has a tier 4 of its own, at its tech 3 size.
    pub const fn with_tier_4(mut self) -> Self {
        self.max_tech = 4;
        self
    }
}

fn catalogue() -> impl Iterator<Item = &'static ModelDef> {
    aster::MODELS.iter().chain(props::MODELS.iter()).chain(super::replicator::MODELS.iter())
}

pub(super) fn find(key: &str) -> Option<&'static ModelDef> {
    catalogue().find(|def| def.key == key)
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
/// Higher `tech` (1..=3) adds modules, antennae and glow emitters; tech 4 and 5
/// are drawn as tech 3 until they get looks of their own.
pub fn build_model_scaled(key: &str, radius: f32, height: f32, tech: u8) -> Option<Model> {
    build_model_fitted(key, radius, height, tech, &[])
}

/// [`build_model_scaled`] for a unit with refit modules: `modules` are their keys in
/// look-bit order, and the model carries every module's pieces, tagged for the shader.
pub fn build_model_fitted(
    key: &str,
    radius: f32,
    height: f32,
    tech: u8,
    modules: &[&str],
) -> Option<Model> {
    let def = catalogue().find(|def| def.key == key)?;
    let tech = tech.clamp(1, def.max_tech);
    let (nominal_radius, nominal_height) = def.nominal[tech.min(MAX_TECH) as usize - 1];
    let horizontal = radius / nominal_radius;
    let root = Affine3A::from_scale(Vec3::new(horizontal, horizontal, height / nominal_height));

    let mut pivots = (Vec3::ZERO, Vec3::ZERO);
    let mut treads = None;
    let mut dust_line = None;
    let mut legs = None;
    let mut hover = false;
    let mut arm_pivot = None;
    let mut arm_boom = false;
    let mut recoil = None;
    let mut fold = None;
    let mut fold_wrist = None;
    let mut neck = None;
    let mut mount = None;
    let mut houses = Vec::new();
    let mut spins = Vec::new();
    let mut pit = None;
    let lods: [MeshLod; LOD_COUNT] = std::array::from_fn(|lod| {
        let mut builder = MeshBuilder::new(lod, root);
        builder.set_modules(modules);
        (def.build)(&mut builder, tech);
        if lod == 0 {
            pivots = (builder.turret_pivot(), builder.spinner_pivot());
            treads = builder.treads();
            dust_line = builder.dust_line();
            legs = builder.legs();
            hover = builder.hover();
            arm_pivot = builder.arm_pivot();
            arm_boom = builder.arm_boom();
            recoil = builder.recoil();
            fold = builder.fold();
            fold_wrist = builder.fold_wrist();
            neck = builder.neck();
            mount = builder.mount();
            houses = builder.houses();
            spins = builder.spins();
            pit = builder.pit();
        }
        builder.finish()
    });
    let surface_reach = bounds_radius(&lods[0], pivots.0, pivots.1, None, None, pit);
    let dust_line = dust_line.unwrap_or_else(|| {
        0.62 * lods[0].vertices.iter().map(|v| v.pos[2]).fold(0.0f32, f32::max)
    });
    let bounds_radius = lods
        .iter()
        .map(|lod| {
            bounds_radius(lod, pivots.0, pivots.1, arm_pivot.map(Vec3::from), fold.map(|f| Vec3::new(f[0], f[1], f[2])), pit)
        })
        .fold(0.0, f32::max);
    Some(Model {
        key: key.to_owned(),
        lods,
        turret_pivot: pivots.0.to_array(),
        spinner_pivot: pivots.1.to_array(),
        bounds_radius,
        surface_reach,
        dust_line,
        treads,
        legs,
        hover,
        arm_pivot,
        arm_boom,
        recoil,
        fold,
        fold_wrist,
        neck,
        mount,
        houses,
        spins,
        pit,
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

/// Distance from the origin that holds `p` with its turret or spinner at any yaw.
fn yawed_reach(p: Vec3, part: u32, turret_pivot: Vec3, spinner_pivot: Vec3) -> f32 {
    let horizontal = match part {
        part::TURRET => turret_pivot.truncate().length() + (p - turret_pivot).truncate().length(),
        part::SPINNER => {
            spinner_pivot.truncate().length() + (p - spinner_pivot).truncate().length()
        }
        _ => p.truncate().length(),
    };
    horizontal.hypot(p.z)
}

/// Radius around the origin that holds the mesh with its turret and spinner at
/// any yaw, and a howitzer tube elevated straight up.
fn bounds_radius(
    mesh: &MeshLod,
    turret_pivot: Vec3,
    spinner_pivot: Vec3,
    arm_pivot: Option<Vec3>,
    fold: Option<Vec3>,
    pit: Option<Pit>,
) -> f32 {
    mesh.vertices
        .iter()
        .map(|v| {
            let mut p = Vec3::from(v.pos);
            // What is down a pit is only ever seen through its opening, which the rest holds.
            if let Some(pit) = pit {
                // Raised onto its stilts on water, the driver hauled up, the next section
                // waiting raised at the rack.
                if v.part != part::AFLOAT {
                    p.z += pit.afloat_lift;
                }
                if v.part == part::RAM {
                    p.z += pit.stroke;
                }
                // Down the bore is seen only through the opening, which the rest holds;
                // a stilt under the sea is hidden by it.
                if p.truncate().length() <= pit.radius && p.z < pit.open || v.part == part::AFLOAT {
                    p.z = p.z.max(pit.open);
                }
            }
            let rest = yawed_reach(p, v.part, turret_pivot, spinner_pivot);
            let elevated = match (arm_pivot, v.rig & rig::LIMB_MASK) {
                (Some(pivot), rig::ARM_GUN | rig::ARM_TOOL) => {
                    // 90° up about the elbow: (x, z) → (−z, x) relative to it.
                    let r = p - pivot;
                    let up = Vec3::new(pivot.x - r.z, p.y, pivot.z + r.x);
                    let elbow = yawed_reach(up, v.part, turret_pivot, spinner_pivot);
                    // A two-bone arm also folds the forearm about the shoulder.
                    let rs = p - turret_pivot;
                    let folded = Vec3::new(turret_pivot.x - rs.z, p.y, turret_pivot.z + rs.x);
                    elbow.max(yawed_reach(folded, v.part, turret_pivot, spinner_pivot))
                }
                (_, rig::FOLD | rig::FOLD_HEAD) => {
                    // Folding gear swings anywhere round its hinge.
                    let hinge = fold.unwrap_or(turret_pivot);
                    yawed_reach(hinge, v.part, turret_pivot, spinner_pivot) + (p - hinge).length()
                }
                (_, rig::ARM_BOOM) => {
                    let r = p - turret_pivot;
                    let up = Vec3::new(turret_pivot.x - r.z, p.y, turret_pivot.z + r.x);
                    yawed_reach(up, v.part, turret_pivot, spinner_pivot)
                }
                _ => 0.0,
            };
            rest.max(elevated)
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
