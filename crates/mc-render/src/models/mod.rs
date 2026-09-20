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
    /// Construction emitters: the yellow-orange of a build beam. Emissive.
    pub const GLOW_AMBER: u32 = 13;
    /// Armour painted dark: the faction's plating colour taken down to graphite.
    pub const PLATING_DARK: u32 = 14;
}

/// Which rigid part of the model a vertex belongs to. The vertex shader
/// animates parts; the mesh itself is static.
pub mod part {
    pub const HULL: u32 = 0;
    /// Yaws around `Model::turret_pivot` by the unit's turret angle.
    pub const TURRET: u32 = 1;
    /// Spins continuously around `Model::spinner_pivot` (radar dishes, extractor intakes).
    pub const SPINNER: u32 = 2;
    /// Tread / leg surfaces: the shader scrolls or bobs these with distance travelled.
    pub const LOCOMOTION: u32 = 3;
}

/// How a vertex is rigged beyond its part: which bone of a walking leg it
/// rides, and whether it belongs to the unit's next upgrade.
pub mod rig {
    /// Low bits: the leg bone. The vertex shader poses legs by two-bone IK
    /// from `Model::legs`; the side comes from the sign of the vertex's y.
    pub const THIGH: u32 = 1;
    pub const SHIN: u32 = 2;
    pub const FOOT: u32 = 3;
    /// A forearm that pitches about `Model::arm_pivot` to point up or down at what
    /// it aims at: the first weapon's arm, and the build arm.
    pub const ARM_GUN: u32 = 4;
    pub const ARM_TOOL: u32 = 5;
    pub const LIMB_MASK: u32 = 0xF;
    /// Part of what the unit's upgrade adds: not drawn until the refit is under
    /// way, then a hologram, then built. Bits 16..24 say when in the refit it
    /// goes up (0..=255 of the way through).
    pub const UPGRADE: u32 = 1 << 8;
    pub const UPGRADE_AT_SHIFT: u32 = 16;
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
    /// `rig` bits.
    pub rig: u32,
}

#[derive(Clone, Debug, Default)]
pub struct MeshLod {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

pub const LOD_COUNT: usize = 3;

/// Where a tracked model touches the ground, in model space (metres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Treads {
    /// Distance from the centre line to the middle of each track.
    pub half_gauge: f32,
    /// Width of one track.
    pub width: f32,
    /// x of the tracks' rear end, where the dust comes off.
    pub rear: f32,
}

/// A walker's legs, in model space (metres): the joints of the left (+y) leg
/// standing at rest. The right leg is its mirror image, half a cycle behind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Legs {
    pub hip: [f32; 3],
    pub knee: [f32; 3],
    pub ankle: [f32; 3],
    /// Ground covered by one full cycle (both feet). A power of two, so the
    /// sim's wrapping distance counter never breaks the stride.
    pub stride: f32,
    /// Share of the cycle a foot spends planted. Over a half is a walk; under it
    /// is a run, with both feet off the ground between steps.
    pub stance: f32,
    /// How high a foot is lifted on its way forward.
    pub lift: f32,
    /// Sole in model space: metres behind the ankle, ahead of it, and the
    /// sole's width. Zero if this walker does not stamp the ground.
    pub foot: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct Model {
    pub key: String,
    /// Full detail, reduced, and a handful of boxes.
    pub lods: [MeshLod; LOD_COUNT],
    pub turret_pivot: [f32; 3],
    pub spinner_pivot: [f32; 3],
    /// Radius of the bounding sphere around the model origin.
    pub bounds_radius: f32,
    /// Set for tracked vehicles: they mark the ground and raise dust.
    pub treads: Option<Treads>,
    /// Set for walkers: their legs are posed by the vertex shader.
    pub legs: Option<Legs>,
    /// The left (+y) elbow, for models whose forearms pitch (`rig::ARM_GUN`, `ARM_TOOL`);
    /// the right one is its mirror image. The unit file's `pivot`s say the same.
    pub arm_pivot: Option<[f32; 3]>,
}

mod aster;
pub mod builder;
mod library;
#[cfg(test)]
mod preview;
mod props;
#[cfg(test)]
mod tests;

pub use library::{all_model_keys, build_model, build_model_scaled, prop_model_key};
