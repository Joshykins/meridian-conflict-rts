//! Procedural models. There are no authored art assets yet: every unit,
//! structure and prop is generated from code at start-up, with three levels of
//! detail. Below the last level the renderer switches to strategic icons.

/// Surface classes. The fragment shader turns these into PBR parameters, so a
/// faction's palette can change without rebuilding meshes.
pub mod material {
    /// Main armour plating (Aster: stark white).
    pub const PLATING: u32 = 0;
    /// Frame, joints, recesses (Aster: black / dark grey).
    pub const ACCENT: u32 = 1;
    /// Faction highlight emitters (Aster: near-white blue). Emissive.
    pub const GLOW: u32 = 2;
    /// Owner's team colour stripe.
    pub const TEAM: u32 = 3;
    /// Bare gunmetal: barrels, pistons.
    pub const METAL: u32 = 4;
    /// Sensors, canopies.
    pub const GLASS: u32 = 5;
    /// Treads, tyres, feet.
    pub const TREAD: u32 = 6;
    /// Conventional-weapon emitters (orange). Emissive.
    pub const GLOW_ORANGE: u32 = 7;
    pub const BARK: u32 = 8;
    pub const FOLIAGE: u32 = 9;
    pub const ROCK: u32 = 10;
    pub const CONCRETE: u32 = 11;
    /// Lit windows on city buildings. Mildly emissive.
    pub const WINDOWS: u32 = 12;
}

/// Which rigid part of the model a vertex belongs to. The vertex shader
/// animates parts; the mesh itself is static.
pub mod part {
    pub const HULL: u32 = 0;
    /// Yaws around `Model::turret_pivot` by the unit's turret angle.
    pub const TURRET: u32 = 1;
    /// Spins continuously around `Model::spinner_pivot` (radar dishes, extractor rotors).
    pub const SPINNER: u32 = 2;
    /// Tread / leg surfaces: the shader scrolls or bobs these with distance travelled.
    pub const LOCOMOTION: u32 = 3;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    /// Model space, metres: x forward, y left, z up, origin on the ground under the centre.
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    /// Metres along the surface (box-projected); the shader tiles panel-line normal maps with it.
    pub uv: [f32; 2],
    pub material: u32,
    pub part: u32,
}

#[derive(Clone, Debug, Default)]
pub struct MeshLod {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

pub const LOD_COUNT: usize = 3;

#[derive(Clone, Debug)]
pub struct Model {
    pub key: String,
    /// Full detail, reduced, and a handful of boxes.
    pub lods: [MeshLod; LOD_COUNT],
    pub turret_pivot: [f32; 3],
    pub spinner_pivot: [f32; 3],
    /// Radius of the bounding sphere around the model origin.
    pub bounds_radius: f32,
}

mod aster;
pub mod builder;
mod library;
mod props;
#[cfg(test)]
mod preview;
#[cfg(test)]
mod tests;

pub use library::{all_model_keys, build_model, build_model_scaled, prop_model_key};
