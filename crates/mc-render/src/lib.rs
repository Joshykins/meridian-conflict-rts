//! Raw Vulkan renderer. GPU-driven: the CPU uploads the sim's render mirror
//! once per tick; interpolation, culling, LOD selection and draw generation
//! all happen on the GPU.

pub mod camera;
pub mod foliage;
pub mod gpu;
pub mod lights;
pub mod ground_cover;
pub mod models;
pub mod overlay;
pub mod pipelines;
pub mod renderer;
pub mod sky;
mod splash;
mod swapchain;
mod warm;
pub mod terrain;
pub mod textures;

pub use camera::Camera;
pub use gpu::GpuError;
pub use overlay::{Face, Overlay, Type};
pub use splash::Splash;
pub use renderer::{
    gpu_scopes_to_perf, DrawStats, GpuScope,
    Antialiasing, FrameInput, FrameStats, Mark, RangeRing, Renderer, SceneDesc, Target, MAX_RANGES,
};
