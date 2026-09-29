//! Quality presets own the complete bundle; manual resolution/AA edits retain
//! their base scenery settings and are shown as Custom.

use super::{Antialiasing, Settings};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    Low,
    Balanced,
    High,
    Ultra,
}

impl Default for Quality {
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            Self::Balanced
        } else {
            Self::High
        }
    }
}

impl Quality {
    pub const ALL: [Self; 4] = [Self::Low, Self::Balanced, Self::High, Self::Ultra];

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Balanced => "Balanced",
            Self::High => "High",
            Self::Ultra => "Ultra",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Low => "Simple shading, less scenery",
            Self::Balanced => "Simple shading, reduced scenery",
            Self::High => "Full scenery, medium clouds",
            Self::Ultra => "Fine scenery, high clouds",
        }
    }

    pub fn render_scale(self) -> f32 {
        match self {
            Self::Low => 0.5,
            Self::Balanced => 0.75,
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
        let (prop_detail, cloud_divisor) = match self {
            Self::Low => ([6.0, 4.0, 8.0], 4),
            Self::Balanced => ([4.0, 3.0, 6.0], 4),
            Self::High => ([1.2, 2.0, 0.0], 3),
            Self::Ultra => ([1.2, 1.0, 0.0], 2),
        };
        mc_render::SceneQuality {
            prop_detail,
            cloud_divisor,
            simple_shading: matches!(self, Self::Low | Self::Balanced),
        }
    }
}

impl Settings {
    pub fn apply_graphics(
        &self,
        renderer: &mut mc_render::Renderer,
    ) -> Result<(), mc_render::GpuError> {
        renderer.set_graphics_quality(
            self.render_scale,
            self.antialiasing.to_renderer(),
            self.quality.scene(),
        )
    }

    pub fn apply_quality(&mut self, quality: Quality) {
        self.quality = quality;
        self.render_scale = quality.render_scale();
        self.antialiasing = quality.antialiasing();
    }

    pub fn quality_label(&self) -> &'static str {
        if self.render_scale == self.quality.render_scale()
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

    #[test]
    fn presets_restore_the_whole_bundle_and_round_trip() {
        for quality in Quality::ALL {
            let mut settings = Settings::default();
            settings.apply_quality(quality);
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
    fn existing_low_settings_gain_simple_shading_without_changing_resolution() {
        let mut settings: Settings = ron::from_str("(quality: Low, render_scale: 0.75)").unwrap();
        assert!(settings.quality.scene().simple_shading);
        assert_eq!(settings.render_scale, 0.75);
        assert_eq!(settings.quality_label(), "Custom");
        settings.apply_quality(Quality::High);
        assert!(!settings.quality.scene().simple_shading);
        settings.apply_quality(Quality::Balanced);
        assert!(settings.quality.scene().simple_shading);
    }

    #[test]
    fn older_settings_keep_explicit_resolution_and_aa() {
        let settings: Settings = ron::from_str("(render_scale: 1.25, antialiasing: Off)").unwrap();
        assert_eq!(settings.render_scale, 1.25);
        assert_eq!(settings.antialiasing, Antialiasing::Off);
        assert_eq!(settings.quality, Quality::default());
        assert_eq!(settings.quality_label(), "Custom");
        assert_eq!(
            Settings::default().quality_label(),
            Quality::default().label()
        );
    }
}
