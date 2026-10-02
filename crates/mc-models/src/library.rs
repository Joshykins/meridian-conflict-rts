//! The model catalogue: every mesh key, the size it is authored at, and the
//! assembly of its three levels of detail into a [`Model`].

use glam::{Affine3A, Vec3};

use super::builder::MeshBuilder;
use super::{aster, part, props, regency, rig, MeshLod, Model, Pit, LOD_COUNT};

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
    /// It has a far level of its own ([`MeshBuilder::far`]).
    pub far: bool,
}

impl ModelDef {
    pub(super) const fn new(
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
            far: false,
        }
    }

    pub(super) const fn tiered(
        key: &'static str,
        nominal: [(f32, f32); MAX_TECH as usize],
        build: fn(&mut MeshBuilder, u8),
    ) -> Self {
        ModelDef {
            key,
            nominal,
            build,
            max_tech: MAX_TECH,
            far: false,
        }
    }

    /// The model has a far level, cheaper than its coarse one, for when it is only a
    /// few pixels across: props there are by the hundred thousand.
    pub(super) const fn with_far(mut self) -> Self {
        self.far = true;
        self
    }

    /// The model also has a tier 4 of its own, at its tech 3 size.
    pub(super) const fn with_tier_4(mut self) -> Self {
        self.max_tech = 4;
        self
    }
}

fn catalogue() -> impl Iterator<Item = &'static ModelDef> {
    aster::MODELS
        .iter()
        .chain(regency::MODELS.iter())
        .chain(regency::naval::attack_boat::MODELS.iter())
        .chain(regency::naval::submarine::MODELS.iter())
        .chain(regency::naval::frigate::MODELS.iter())
        .chain(regency::naval::destroyer::MODELS.iter())
        .chain(regency::naval::cruiser::MODELS.iter())
        .chain(regency::naval::battleship::MODELS.iter())
        .chain(regency::naval::carrier::MODELS.iter())
        .chain(regency::naval::assault_submarine::MODELS.iter())
        .chain(regency::air::flechette::MODELS.iter())
        .chain(regency::air::quarrel::MODELS.iter())
        .chain(regency::air::petard::MODELS.iter())
        .chain(regency::air::coffer::MODELS.iter())
        .chain(regency::air::sickle::MODELS.iter())
        .chain(regency::gunships::quiver::MODELS.iter())
        .chain(regency::gunships::wick::MODELS.iter())
        .chain(regency::gunships::reaper::MODELS.iter())
        .chain(props::MODELS.iter())
        .chain(super::desert::MODELS.iter())
        .chain(super::dam::MODELS.iter())
        .chain(super::dam_works::MODELS.iter())
        .chain(super::replicator::MODELS.iter())
        .chain(super::precursor::MODELS.iter())
        .chain(super::precursor_mega::MODELS.iter())
        .chain(super::precursor_tower::MODELS.iter())
        .chain(super::precursor_polar::MODELS.iter())
        .chain(super::precursor_forge::MODELS.iter())
        .chain(super::precursor_sky::MODELS.iter())
        .chain(super::precursor_gate::MODELS.iter())
        .chain(super::precursor_citadel::MODELS.iter())
}

pub(super) fn find(key: &str) -> Option<&'static ModelDef> {
    catalogue().find(|def| def.key == key)
}

/// Every key [`build_model`] understands: units and structures first, then map props.
pub fn all_model_keys() -> Vec<&'static str> {
    catalogue().map(|def| def.key).collect()
}

/// The tech 1 (collision radius, height) `key` is authored at. Points given in
/// model space (exhausts, nacelle pivots) scale by a blueprint's size over this.
pub fn authored_size(key: &str) -> Option<(f32, f32)> {
    catalogue()
        .find(|def| def.key == key)
        .map(|def| def.nominal[0])
}

/// Builds `key` at its authored (tech 1 blueprint) size.
pub fn build_model(key: &str) -> Option<Model> {
    crate::remote::through(crate::remote::Call::prop(key), || build_model_made(key))
}

fn build_model_made(key: &str) -> Option<Model> {
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
    let call = crate::remote::Call {
        key: key.to_owned(),
        radius,
        height,
        tech,
        modules: modules.iter().map(|m| (*m).to_owned()).collect(),
        prop: false,
    };
    crate::remote::through(call, || {
        build_fitted_made(key, radius, height, tech, modules)
    })
}

fn build_fitted_made(
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
    let mut breech = None;
    let mut charge_gear = None;
    let mut fold_wrist = None;
    let mut neck = None;
    let mut shield_emitter = None;
    let mut mount = None;
    let mut houses = Vec::new();
    let mut cells = Vec::new();
    let mut spins = Vec::new();
    let mut pit = None;
    let mut excavation = None;
    let mut star_core = None;
    let mut scans = false;
    let mut exhausts = Vec::new();
    let mut lifts = Vec::new();
    let mut discharge = None;
    let mut vtol = None;
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
            scans = builder.spinner_scans();
            arm_pivot = builder.arm_pivot();
            arm_boom = builder.arm_boom();
            recoil = builder.recoil();
            fold = builder.fold();
            breech = builder.breech();
            charge_gear = builder.charge_gear();
            fold_wrist = builder.fold_wrist();
            neck = builder.neck();
            shield_emitter = builder.shield_emitter();
            mount = builder.mount();
            houses = builder.houses();
            cells = builder.cells();
            spins = builder.spins();
            pit = builder.pit();
            excavation = builder.excavation();
            star_core = builder.star_core();
            exhausts = builder.exhausts();
            lifts = builder.lifts();
            discharge = builder.discharge();
            vtol = builder.vtol();
        }
        builder.finish()
    });
    let surface_reach = bounds_radius(&lods[0], pivots.0, pivots.1, None, None, pit);
    let dust_line = dust_line.unwrap_or_else(|| {
        0.62 * lods[0]
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(0.0f32, f32::max)
    });
    let bounds_radius = lods
        .iter()
        .map(|lod| {
            bounds_radius(
                lod,
                pivots.0,
                pivots.1,
                arm_pivot.map(Vec3::from),
                fold.map(|f| Vec3::new(f[0], f[1], f[2])),
                pit,
            )
        })
        .fold(0.0, f32::max);
    let far = def.far.then(|| {
        let mut builder = MeshBuilder::new_far(root);
        builder.set_modules(modules);
        (def.build)(&mut builder, tech);
        builder.finish()
    });
    Some(Model {
        key: key.to_owned(),
        lods,
        far,
        turret_pivot: pivots.0.to_array(),
        spinner_pivot: pivots.1.to_array(),
        spinner_scans: scans,
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
        breech,
        charge_gear,
        fold_wrist,
        neck,
        shield_emitter,
        mount,
        houses,
        cells,
        spins,
        pit,
        excavation,
        star_core,
        exhausts,
        lifts,
        discharge,
        vtol,
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
        // A gyroscope's ring may turn anywhere round the pivot.
        _ if part & part::ORBIT_MASK == part::ORBIT => {
            return spinner_pivot.length() + (p - spinner_pivot).length();
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
                if !part::afloat_only(v.part) {
                    p.z += pit.afloat_lift;
                }
                if v.part == part::RAM {
                    p.z += pit.stroke;
                }
                // Down the bore is seen only through the opening, which the rest holds;
                // a stilt under the sea is hidden by it.
                if p.truncate().length() <= pit.radius && p.z < pit.open
                    || part::afloat_only(v.part)
                {
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
/// fall back by family (tree, rock, building, precursor) so new kinds still draw.
pub fn prop_model_key(kind_raw: u16) -> &'static str {
    match kind_raw {
        0 => "tree_broadleaf",
        1 => "tree_conifer",
        2 => "tree_pine",
        3 => "tree_dead",
        4 => "tree_palm",
        5 => "tree_jungle",
        6 => "tree_juniper",
        7 => "tree_pinyon",
        8 => "tree_cottonwood",
        9..=15 => "tree_broadleaf",
        16 => "rock_small",
        17 => "rock_large",
        18 => "rock_slab",
        19..=31 => "rock_small",
        32 => "building_small",
        33 => "building_medium",
        34 => "building_wide",
        35 => "building_tower",
        36..=47 => "building_small",
        48 => "precursor_spire",
        49 => "precursor_pylon",
        50 => "precursor_arch",
        51 => "precursor_ring",
        52 => "precursor_shard",
        53 => "precursor_wall",
        54 => "precursor_beacon",
        55 => "precursor_conduit",
        56 => "precursor_fragment",
        57 => "precursor_bastion",
        58 => "precursor_boom",
        59 => "precursor_tower",
        60 => "precursor_span",
        61 => "precursor_viaduct",
        62 => "precursor_pier",
        63 => "precursor_seaway",
        64 => "precursor_vault",
        65 => "precursor_axis",
        66 => "precursor_terrace",
        67 => "precursor_lining",
        68 => "precursor_forge",
        69 => "precursor_cradle",
        70 => "precursor_heart",
        71 => "precursor_halo",
        72 => "precursor_monolith",
        73 => "precursor_seagate",
        74 => "precursor_platform",
        75 => "precursor_gate",
        76 => "precursor_needle",
        77 => "precursor_rampart",
        78 => "precursor_floor",
        79 => "precursor_citadel",
        80 => "landmark_dam",
        81 => "landmark_switchyard",
        82 => "landmark_pylon",
        83 => "landmark_town",
        84 => "landmark_span",

        _ => "building_small",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn exhausts_are_recorded_once_per_port_mirrored_and_scaled() {
        let model = super::build_model("assault_tank").expect("the Fulgur");
        assert_eq!(model.exhausts.len(), 2, "one per vent");
        let [a, b] = [model.exhausts[0], model.exhausts[1]];
        assert!((a.at[1] + b.at[1]).abs() < 1e-3, "mirrored: {a:?} {b:?}");
        assert!(a.toward[2] > 0.9, "the gas leaves upwards: {a:?}");
        let big = super::build_model_scaled("assault_tank", 38.0, 30.0, 1).expect("scaled");
        let ratio = big.exhausts[0].radius / a.radius;
        assert!(
            (ratio - 2.0).abs() < 0.05,
            "the ports scale with the hull: {ratio}"
        );
        assert!(super::build_model("tank_light").is_some_and(|m| m.exhausts.is_empty()));
    }
}
