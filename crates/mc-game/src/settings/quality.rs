//! Quality presets own the complete bundle; manual resolution/AA edits retain
//! their base scenery settings and are shown as Custom. Auto picks the preset for
//! the graphics card (`Quality::detect`).

use super::{Antialiasing, Settings};
use mc_render::gpu::{Adapter, DeviceKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    Low,
    #[serde(alias = "Balanced")]
    Medium,
    High,
    Ultra,
}

impl Default for Quality {
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            Self::Medium
        } else {
            Self::High
        }
    }
}

/// Above this many output pixels (4K) a preset costs a step more than the card's
/// memory suggests.
const HUGE_SCREEN_PIXELS: u64 = 3840 * 2160;

impl Quality {
    pub const ALL: [Self; 4] = [Self::Low, Self::Medium, Self::High, Self::Ultra];

    /// `low`, `medium`, `high` or `ultra`, any case.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|q| q.label().eq_ignore_ascii_case(name.trim()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Ultra => "Ultra",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Low => "Half resolution, no grass or ambient occlusion, near shadows",
            Self::Medium => "75% resolution, thin grass, simple shading",
            Self::High => "Full resolution, full grass, ambient occlusion",
            Self::Ultra => "Supersampled, sharp far shadows, fine clouds",
        }
    }

    /// The preset to start with on `adapter` drawing `pixels` output pixels:
    /// integrated and software devices get Low, a discrete card goes by its memory.
    /// Only a player picks Ultra.
    pub fn detect(adapter: &Adapter, pixels: u64) -> Self {
        let by_device = match adapter.kind {
            // Shared memory says nothing about the GPU; docs/PERFORMANCE-MAC.md
            // measured an M3 Pro short of 60 fps even on Low.
            DeviceKind::Software | DeviceKind::Integrated => Self::Low,
            DeviceKind::Discrete | DeviceKind::Other => match adapter.vram_mib {
                0..3500 => Self::Low,
                3500..7000 => Self::Medium,
                _ => Self::High,
            },
        };
        if pixels > HUGE_SCREEN_PIXELS {
            by_device.lower()
        } else {
            by_device
        }
    }

    fn lower(self) -> Self {
        match self {
            Self::Low | Self::Medium => Self::Low,
            Self::High => Self::Medium,
            Self::Ultra => Self::High,
        }
    }

    pub fn render_scale(self) -> f32 {
        match self {
            Self::Low => 0.5,
            Self::Medium => 0.75,
            Self::High => 1.0,
            Self::Ultra => 1.5,
        }
    }

    pub fn antialiasing(self) -> Antialiasing {
        match self {
            Self::Low => Antialiasing::Off,
            _ => Antialiasing::Smaa,
        }
    }

    pub fn scene(self) -> mc_render::SceneQuality {
        match self {
            Self::Low => mc_render::SceneQuality {
                prop_detail: [6.0, 4.0, 8.0],
                cloud_divisor: 4,
                simple_shading: true,
                shadow_size: 1024,
                shadow_distance: 4000.0,
                ambient_occlusion: false,
                grass_density: 0.0,
                water_reflections: false,
            },
            Self::Medium => mc_render::SceneQuality {
                prop_detail: [3.0, 3.0, 5.0],
                cloud_divisor: 4,
                simple_shading: true,
                shadow_size: 2048,
                shadow_distance: 6000.0,
                ambient_occlusion: false,
                grass_density: 0.5,
                water_reflections: true,
            },
            Self::High => mc_render::SceneQuality {
                prop_detail: [1.2, 2.0, 0.0],
                cloud_divisor: 3,
                simple_shading: false,
                shadow_size: 2048,
                shadow_distance: 8000.0,
                ambient_occlusion: true,
                grass_density: 1.0,
                water_reflections: true,
            },
            Self::Ultra => mc_render::SceneQuality {
                prop_detail: [1.2, 1.0, 0.0],
                cloud_divisor: 2,
                simple_shading: false,
                shadow_size: 4096,
                shadow_distance: 12000.0,
                ambient_occlusion: true,
                grass_density: 1.0,
                water_reflections: true,
            },
        }
    }
}

impl Settings {
    /// Apply the settings to `renderer`, first choosing the preset for its device when
    /// `auto_quality` is on.
    pub fn apply_graphics(
        &mut self,
        renderer: &mut mc_render::Renderer,
    ) -> Result<(), mc_render::GpuError> {
        if self.auto_quality {
            let adapter = renderer.adapter();
            let (w, h) = renderer.size();
            let quality = Quality::detect(&adapter, u64::from(w) * u64::from(h));
            if quality != self.quality {
                log::info!(
                    "graphics quality {} for {} ({:?}, {} MiB) at {w}x{h}",
                    quality.label(),
                    adapter.name,
                    adapter.kind,
                    adapter.vram_mib
                );
            }
            self.set_preset(quality);
        }
        let (w, h) = renderer.size();
        crate::crash::context(
            "graphics",
            format!(
                "{} ({}{}), render scale {}, {:?}, output {w}x{h}",
                self.quality_label(),
                self.quality.label(),
                if self.auto_quality {
                    ", chosen for this card"
                } else {
                    ""
                },
                self.render_scale,
                self.antialiasing,
            ),
        );
        renderer.set_graphics_quality(
            self.render_scale,
            self.antialiasing.to_renderer(),
            self.quality.scene(),
        )
    }

    /// A preset chosen by hand: Auto is off from now on.
    pub fn apply_quality(&mut self, quality: Quality) {
        self.auto_quality = false;
        self.set_preset(quality);
    }

    fn set_preset(&mut self, quality: Quality) {
        self.quality = quality;
        self.render_scale = quality.render_scale();
        self.antialiasing = quality.antialiasing();
    }

    pub fn quality_label(&self) -> &'static str {
        if self.auto_quality {
            "Auto"
        } else if self.render_scale == self.quality.render_scale()
            && self.antialiasing == self.quality.antialiasing()
        {
            self.quality.label()
        } else {
            "Custom"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(kind: DeviceKind, vram_mib: u64) -> Adapter {
        Adapter {
            name: "test".into(),
            kind,
            vendor_id: 0,
            vram_mib,
        }
    }

    #[test]
    fn presets_restore_the_whole_bundle_and_round_trip() {
        for quality in Quality::ALL {
            let mut settings = Settings::default();
            settings.apply_quality(quality);
            assert!(!settings.auto_quality);
            assert_eq!(settings.quality_label(), quality.label());
            settings.render_scale = 2.0;
            assert_eq!(settings.quality_label(), "Custom");
            let text = ron::to_string(&settings).unwrap();
            let saved: Settings = ron::from_str(&text).unwrap();
            assert_eq!(saved, settings);
            assert_eq!(saved.quality.scene(), quality.scene());
            settings.apply_quality(quality);
            settings.antialiasing = if quality.antialiasing() == Antialiasing::Off {
                Antialiasing::Smaa
            } else {
                Antialiasing::Off
            };
            assert_eq!(settings.quality_label(), "Custom");
            settings.apply_quality(quality);
            assert_eq!(settings.quality_label(), quality.label());
        }
    }

    #[test]
    fn each_step_up_costs_more_in_every_respect() {
        for pair in Quality::ALL.windows(2) {
            let (lo, hi) = (pair[0], pair[1]);
            let (a, b) = (lo.scene(), hi.scene());
            assert!(lo.render_scale() < hi.render_scale(), "{lo:?} {hi:?}");
            assert!(a.prop_detail[0] >= b.prop_detail[0]);
            assert!(a.cloud_divisor >= b.cloud_divisor);
            assert!(a.shadow_size <= b.shadow_size);
            assert!(a.shadow_distance < b.shadow_distance);
            assert!(a.grass_density <= b.grass_density);
            assert!(a.simple_shading >= b.simple_shading);
            assert!(a.ambient_occlusion <= b.ambient_occlusion);
            assert!(a.water_reflections <= b.water_reflections);
        }
    }

    #[test]
    fn detection_follows_the_device_and_the_screen() {
        let fhd = 1920 * 1080;
        let uhd = 3840 * 2160 + 1;
        let cases = [
            (DeviceKind::Software, 0, fhd, Quality::Low),
            (DeviceKind::Integrated, 16384, fhd, Quality::Low),
            (DeviceKind::Discrete, 2048, fhd, Quality::Low),
            (DeviceKind::Discrete, 4096, fhd, Quality::Medium),
            (DeviceKind::Discrete, 6144, fhd, Quality::Medium),
            (DeviceKind::Discrete, 8192, fhd, Quality::High),
            (DeviceKind::Discrete, 24576, fhd, Quality::High),
            (DeviceKind::Discrete, 12288, 5120 * 1440, Quality::High),
            (DeviceKind::Discrete, 12288, uhd, Quality::Medium),
            (DeviceKind::Discrete, 4096, uhd, Quality::Low),
        ];
        for (kind, vram, pixels, want) in cases {
            assert_eq!(
                Quality::detect(&adapter(kind, vram), pixels),
                want,
                "{kind:?} {vram} MiB {pixels} px"
            );
        }
    }

    #[test]
    fn auto_shows_as_auto_until_a_preset_is_picked() {
        let mut settings = Settings::default();
        assert_eq!(settings.quality_label(), "Auto");
        settings.apply_quality(Quality::Medium);
        assert_eq!(settings.quality_label(), "Medium");
    }

    #[test]
    fn balanced_settings_load_as_medium() {
        let settings: Settings = ron::from_str("(quality: Balanced, render_scale: 0.75)").unwrap();
        assert_eq!(settings.quality, Quality::Medium);
        assert_eq!(settings.quality_label(), "Medium");
        assert!(settings.quality.scene().simple_shading);
    }

    #[test]
    fn older_settings_keep_explicit_resolution_and_aa() {
        let settings: Settings = ron::from_str("(render_scale: 1.25, antialiasing: Off)").unwrap();
        assert_eq!(settings.render_scale, 1.25);
        assert_eq!(settings.antialiasing, Antialiasing::Off);
        assert_eq!(settings.quality, Quality::default());
        assert_eq!(settings.quality_label(), "Custom");
    }
}
