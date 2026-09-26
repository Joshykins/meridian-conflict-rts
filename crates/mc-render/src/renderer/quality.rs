//! Runtime scenery and cloud quality, shared by menu and match rendering.

use super::{Antialiasing, GpuError, Renderer};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneQuality {
    /// Minimum prop radius in pixels, LOD bias, minimum shadow radius in pixels.
    pub prop_detail: [f32; 3],
    /// Cloud march resolution is window resolution divided by this, in each axis.
    pub cloud_divisor: u32,
}

impl Default for SceneQuality {
    fn default() -> Self {
        Self {
            prop_detail: [1.2, 2.0, 0.0],
            cloud_divisor: 3,
        }
    }
}

impl SceneQuality {
    /// Capture tools retain their explicit overrides. Interactive settings replace
    /// these together when the renderer takes the window or the preset changes.
    pub(super) fn from_env() -> Self {
        let mut quality = Self::default();
        if let Ok(value) = std::env::var("MERIDIAN_PROP_DETAIL") {
            for (slot, value) in quality.prop_detail.iter_mut().zip(value.split(',')) {
                if let Ok(value) = value.trim().parse() {
                    *slot = value;
                }
            }
        }
        if let Ok(value) = std::env::var("MERIDIAN_CLOUD_RES") {
            if let Ok(value) = value.parse() {
                quality.cloud_divisor = value;
            }
        }
        quality.normalised()
    }

    fn normalised(mut self) -> Self {
        for (value, fallback) in self.prop_detail.iter_mut().zip(Self::default().prop_detail) {
            if !value.is_finite() || *value < 0.0 {
                *value = fallback;
            }
        }
        self.prop_detail[1] = self.prop_detail[1].max(0.1);
        self.cloud_divisor = self.cloud_divisor.clamp(1, 4);
        self
    }
}

impl Renderer {
    /// Change resolution and edge smoothing without changing scenery quality
    /// (also used temporarily for loading transitions).
    pub fn set_render_quality(&mut self, scale: f32, aa: Antialiasing) -> Result<(), GpuError> {
        self.set_graphics_quality(scale, aa, self.quality)
    }

    /// Apply a complete quality choice. Rebuild targets only for resolution,
    /// anti-aliasing or cloud-resolution changes, after waiting for the GPU.
    pub fn set_graphics_quality(
        &mut self,
        scale: f32,
        aa: Antialiasing,
        quality: SceneQuality,
    ) -> Result<(), GpuError> {
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 2.0)
        } else {
            1.0
        };
        let quality = quality.normalised();
        let rebuild = scale != self.render_scale
            || aa != self.antialiasing
            || quality.cloud_divisor != self.quality.cloud_divisor;
        self.render_scale = scale;
        self.antialiasing = aa;
        self.quality = quality;
        if rebuild {
            self.create_size_dependent()?;
        }
        Ok(())
    }
}
