//! Runtime scenery and cloud quality, shared by menu and match rendering.

use super::shadow_map::ShadowMap;
use super::{Antialiasing, GpuError, Renderer};
use crate::gpu_consts::quality as bits;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneQuality {
    /// Minimum prop radius in pixels, LOD bias, minimum shadow radius in pixels.
    pub prop_detail: [f32; 3],
    /// Cloud march resolution is window resolution divided by this, in each axis.
    pub cloud_divisor: u32,
    /// Single-patch terrain textures, cheaper shadow filtering and staggered cloud shade.
    pub simple_shading: bool,
    /// Texels along each side of every sun shadow cascade.
    pub shadow_size: u32,
    /// Camera distance in metres at which sun shadows have faded out and the shadow
    /// pass stops drawing; they start fading at 3/8 of it.
    pub shadow_distance: f32,
    /// Ground-truth ambient occlusion (gtao.rs).
    pub ambient_occlusion: bool,
    /// Grass tufts grown, as a fraction of the full field; 0 grows none (grass.rs).
    pub grass_density: f32,
    /// Screen-space reflections on water.
    pub water_reflections: bool,
}

impl Default for SceneQuality {
    fn default() -> Self {
        Self {
            prop_detail: [1.2, 2.0, 0.0],
            cloud_divisor: 3,
            simple_shading: false,
            shadow_size: 2048,
            shadow_distance: 8000.0,
            ambient_occlusion: true,
            grass_density: 1.0,
            water_reflections: true,
        }
    }
}

/// `MERIDIAN_<name>` parsed, when it is set and parses.
fn env<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(format!("MERIDIAN_{name}"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// `MERIDIAN_<name>` as a switch: `0` off, anything else on.
fn env_switch(name: &str) -> Option<bool> {
    std::env::var(format!("MERIDIAN_{name}"))
        .ok()
        .map(|v| v.trim() != "0")
}

impl SceneQuality {
    /// The default quality under the capture tools' explicit overrides.
    pub(super) fn from_env() -> Self {
        Self::default().with_env()
    }

    /// Any `MERIDIAN_*` override set in the environment wins over a preset, so an
    /// A/B measurement holds whatever the settings say (docs/SWITCHES.md).
    fn with_env(mut self) -> Self {
        if let Ok(value) = std::env::var("MERIDIAN_PROP_DETAIL") {
            for (slot, value) in self.prop_detail.iter_mut().zip(value.split(',')) {
                if let Ok(value) = value.trim().parse() {
                    *slot = value;
                }
            }
        }
        if let Some(value) = env("CLOUD_RES") {
            self.cloud_divisor = value;
        }
        if let Some(value) = env_switch("SIMPLE_SHADING") {
            self.simple_shading = value;
        }
        if let Some(value) = env("SHADOW_SIZE") {
            self.shadow_size = value;
        }
        if let Some(value) = env("SHADOW_DISTANCE") {
            self.shadow_distance = value;
        }
        if let Some(value) = env_switch("GTAO") {
            self.ambient_occlusion = value;
        }
        if env_switch("GRASS") == Some(false) {
            self.grass_density = 0.0;
        } else if let Some(value) = env("GRASS_DENSITY") {
            self.grass_density = value;
        }
        if let Some(value) = env_switch("WATER_REFLECTIONS") {
            self.water_reflections = value;
        }
        self.normalised()
    }

    fn normalised(mut self) -> Self {
        let fallback = Self::default();
        for (value, fallback) in self.prop_detail.iter_mut().zip(fallback.prop_detail) {
            if !value.is_finite() || *value < 0.0 {
                *value = fallback;
            }
        }
        self.prop_detail[1] = self.prop_detail[1].max(0.1);
        self.cloud_divisor = self.cloud_divisor.clamp(1, 4);
        self.shadow_size = self.shadow_size.clamp(512, 4096).next_power_of_two();
        if !self.shadow_distance.is_finite() {
            self.shadow_distance = fallback.shadow_distance;
        }
        self.shadow_distance = self.shadow_distance.clamp(1000.0, 30000.0);
        if !self.grass_density.is_finite() {
            self.grass_density = fallback.grass_density;
        }
        self.grass_density = self.grass_density.clamp(0.0, 1.0);
        self
    }

    /// How strongly the sun's shadows show with the camera `distance` metres from its
    /// focus: 1 near, fading to 0 (and no shadow pass) at `shadow_distance`.
    pub(super) fn shadow_strength(&self, distance: f32) -> f32 {
        let start = self.shadow_distance * 0.375;
        1.0 - ((distance - start) / (self.shadow_distance - start)).clamp(0.0, 1.0)
    }

    /// The `QUALITY_*` bits the shaders read from `Globals.detail.w`.
    pub(super) fn bits(&self) -> u32 {
        let mut out = 0;
        if self.simple_shading {
            out |= bits::SIMPLE_SHADING;
        }
        if self.water_reflections {
            out |= bits::WATER_REFLECTIONS;
        }
        out
    }
}

impl Renderer {
    /// Change resolution and edge smoothing without changing scenery quality
    /// (also used temporarily for loading transitions).
    pub fn set_render_quality(&mut self, scale: f32, aa: Antialiasing) -> Result<(), GpuError> {
        self.set_graphics_quality(scale, aa, self.quality)
    }

    /// Apply a complete quality choice. Rebuild targets only for resolution,
    /// anti-aliasing, cloud-resolution or shadow-size changes, after waiting for the GPU.
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
        let quality = quality.with_env();
        let rebuild = scale != self.render_scale
            || aa != self.antialiasing
            || quality.cloud_divisor != self.quality.cloud_divisor;
        if quality.shadow_size != self.shadow.size() {
            // SAFETY: the device is live; waiting has no other requirement.
            unsafe { self.gpu.device.device_wait_idle() }?;
            let map = ShadowMap::new(
                &self.gpu,
                self.passes.shadow,
                self.scene_set,
                quality.shadow_size,
            )?;
            std::mem::replace(&mut self.shadow, map).destroy(&self.gpu);
        }
        self.render_scale = scale;
        self.antialiasing = aa;
        self.quality = quality;
        self.gtao.enabled = quality.ambient_occlusion;
        self.grass.set_density(quality.grass_density);
        if rebuild {
            self.create_size_dependent()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadows_fade_out_at_their_distance() {
        let quality = SceneQuality::default();
        assert_eq!(quality.shadow_strength(0.0), 1.0);
        assert_eq!(quality.shadow_strength(3000.0), 1.0);
        assert!(quality.shadow_strength(5500.0) > 0.0);
        assert_eq!(quality.shadow_strength(8000.0), 0.0);
    }

    #[test]
    fn odd_values_are_brought_into_range() {
        let quality = SceneQuality {
            shadow_size: 3000,
            shadow_distance: f32::NAN,
            grass_density: 4.0,
            cloud_divisor: 0,
            ..SceneQuality::default()
        }
        .normalised();
        assert_eq!(quality.shadow_size, 4096);
        assert_eq!(quality.shadow_distance, 8000.0);
        assert_eq!(quality.grass_density, 1.0);
        assert_eq!(quality.cloud_divisor, 1);
    }
}
