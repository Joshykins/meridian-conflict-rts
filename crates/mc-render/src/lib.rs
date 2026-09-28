//! Raw Vulkan renderer. GPU-driven: the CPU uploads the sim's render mirror
//! once per tick; interpolation, culling, LOD selection and draw generation
//! all happen on the GPU.

#![expect(unsafe_code, reason = "raw Vulkan through ash")]

pub mod camera;
pub mod foliage;
pub mod gpu;
pub mod gpu_consts;
#[cfg(test)]
mod gpu_layout;
pub mod ground_cover;
pub mod keep;
pub mod lights;
pub mod models;
pub mod overlay;
pub mod pipelines;
pub mod renderer;
mod shader_prelude;
pub mod shader_reload;
pub mod sky;
mod splash;
mod swapchain;
pub mod terrain;
pub mod textures;
mod warm;

pub use camera::Camera;
pub use gpu::GpuError;
pub use overlay::{Face, Overlay, Type};
pub use renderer::{
    gpu_scopes_to_perf, Antialiasing, DrawStats, FrameInput, FrameStats, GpuScope, Mark, RangeRing,
    Renderer, SceneDesc, SceneQuality, Shot, Target, MAX_RANGES,
};
pub use splash::Splash;
