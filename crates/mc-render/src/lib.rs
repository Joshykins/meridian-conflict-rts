//! Raw Vulkan renderer. GPU-driven: the CPU uploads the sim's render mirror
//! once per tick; interpolation, culling, LOD selection and draw generation
//! all happen on the GPU.

pub mod camera;
pub mod gpu;
pub mod models;
pub mod overlay;
pub mod pipelines;
pub mod renderer;
pub mod terrain;
pub mod textures;

pub use camera::Camera;
pub use gpu::GpuError;
pub use overlay::Overlay;
pub use renderer::{FrameInput, FrameStats, Mark, Renderer, SceneDesc, Target};
