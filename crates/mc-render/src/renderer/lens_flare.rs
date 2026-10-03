//! Lens flares on bright points: what a very bright, small light does in a camera's
//! glass: a star of thin spikes and a soft core round the light, and a long flat streak
//! across it. No ghost discs: the user ruled them out. The tone map draws them (`screen.wgsl`, `lens_flare`) over the scene and
//! its bloom, and hides one whose light something solid stands in front of.
//!
//! An effect asks for a `Flare` that flashes and dies away (`flash`). Presentation only.

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::camera::Camera;
use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::lens::MAX_FLARES;

/// One flare as the tone map draws it (`LensFlare` in screen.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuLensFlare {
    /// Where the light is in the picture, 0..1 across and down.
    pub(crate) at: [f32; 2],
    /// How long its spikes reach, output pixels.
    pub(crate) radius: f32,
    /// How far the light is from the eye, metres: the scene nearer than that hides it.
    pub(crate) distance: f32,
    /// Its colour times its brightness (HDR, before exposure).
    pub(crate) color: [f32; 3],
    pub(crate) _pad: f32,
}

/// How a flare looks at its brightest.
#[derive(Clone, Copy)]
pub(super) struct Flare {
    /// Colour times brightness: 1 sits at the scene's white, the core burns far over it.
    pub(super) color: Vec3,
    /// How far the spikes reach in the world, metres, before the pixel limits below.
    pub(super) size: f32,
}

/// The spikes never shrink below this many pixels, so a flare reads at strategic zoom,
/// nor grow past this many close up.
const MIN_PX: f32 = 22.0;
const MAX_PX: f32 = 240.0;
/// Past this many metres from the eye a flare is not drawn.
const REACH_M: f32 = 6000.0;

#[derive(Clone, Copy)]
struct Timed {
    flare: Flare,
    at: Vec3,
    start: f32,
    life: f32,
}

pub(super) struct LensFlares {
    buffer: Buffer,
    timed: Vec<Timed>,
    scratch: Vec<(f32, GpuLensFlare)>,
}

impl LensFlares {
    pub(super) fn new(gpu: &Gpu) -> Result<Self, GpuError> {
        let buffer = gpu.host_buffer(
            (16 + MAX_FLARES as usize * size_of::<GpuLensFlare>()) as u64,
            ash::vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        buffer.write(0, &vec![0u8; buffer.size as usize]);
        Ok(Self {
            buffer,
            timed: Vec::new(),
            scratch: Vec::new(),
        })
    }

    pub(super) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
    }

    /// A flare that strikes at `start` and dies away over `life` seconds.
    pub(super) fn flash(&mut self, at: Vec3, flare: Flare, start: f32, life: f32) {
        self.timed.push(Timed {
            flare,
            at,
            start,
            life: life.max(0.01),
        });
    }

    /// Hands the GPU this frame's flares, the strongest in view, dropping the spent ones.
    pub(super) fn upload(&mut self, time: f32, camera: &Camera) {
        self.timed.retain(|t| time < t.start + t.life);
        let eye = camera.eye();
        let view_proj = camera.view_proj();
        let px_per_m = camera.projection_scale();
        let viewport = camera.viewport;
        self.scratch.clear();
        for t in self.timed.iter().filter(|t| time >= t.start) {
            let (at, flare) = (t.at, t.flare);
            // Struck at full and gone fast, the last of it lingering.
            let fade = (1.0 - (time - t.start) / t.life).powi(2);
            let distance = at.distance(eye);
            if fade <= 0.0 || distance > REACH_M {
                continue;
            }
            let clip = view_proj * at.extend(1.0);
            if clip.w <= 0.0 {
                continue;
            }
            let uv = [clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5];
            let radius = (flare.size * px_per_m / distance.max(1.0)).clamp(MIN_PX, MAX_PX);
            // Off the picture by more than its spikes: nothing of it shows.
            let margin = [radius / viewport.x, radius / viewport.y];
            if uv[0] < -margin[0]
                || uv[0] > 1.0 + margin[0]
                || uv[1] < -margin[1]
                || uv[1] > 1.0 + margin[1]
            {
                continue;
            }
            let color = flare.color * fade;
            self.scratch.push((
                color.max_element(),
                GpuLensFlare {
                    at: uv,
                    radius: radius * fade.sqrt(),
                    distance,
                    color: color.to_array(),
                    _pad: 0.0,
                },
            ));
        }
        // Strongest first; the stable sort keeps the order they came in when they tie.
        self.scratch.sort_by(|a, b| b.0.total_cmp(&a.0));
        // A deliberate cap: past the strongest few, more stars only clutter the picture.
        self.scratch.truncate(MAX_FLARES as usize);
        let flares: Vec<GpuLensFlare> = self.scratch.iter().map(|f| f.1).collect();
        self.buffer
            .write(0, bytemuck::cast_slice(&[flares.len() as u32, 0, 0, 0]));
        self.buffer.write(16, bytemuck::cast_slice(&flares));
    }
}
