//! How the map's ground and sea are drawn (its `MapLook`, from `maps/<stem>.ron`): its
//! climate, and on a map split in two by a climate divide the line and the climate
//! east of it, as the shaders read them (`Globals::climate`, `divide`, `divide_info`;
//! bindings.wgsl `climate_at`).

use super::*;
use crate::gpu_consts::divide;
use mc_data::weather::{Climate, ClimateDivide, MapLook};

/// `MERIDIAN_CLIMATE=temperate|tropical|desert`: draws any map in that climate, all of
/// it (`Renderer::set_map_look`).
fn climate_override() -> Option<Climate> {
    std::env::var("MERIDIAN_CLIMATE")
        .ok()
        .and_then(|v| Climate::from_name(&v))
}

/// The climate as the shaders read it (`Globals::climate.x`, `divide_info.y`;
/// `tropical()` and `desert()` in bindings.wgsl).
fn climate_code(climate: Climate) -> f32 {
    match climate {
        Climate::Temperate => 0.0,
        Climate::Tropical => 1.0,
        Climate::Desert => 2.0,
    }
}

/// What a renderer draws until it is told its map's look.
pub(super) fn initial() -> MapLook {
    climate_override().map(MapLook::single).unwrap_or_default()
}

/// The climate part of a frame's `Globals`.
pub(super) struct ClimateGlobals {
    /// `Globals::climate.x`.
    pub(super) climate: f32,
    pub(super) divide: [[f32; 4]; divide::POINTS as usize],
    pub(super) divide_info: [f32; 4],
}

impl Renderer {
    /// How the map's ground and sea are drawn (its `MapConfig::look`): the climate's
    /// palette, and on a map with a climate divide the line, the climate east of it
    /// and the light along the wall's foot. `MERIDIAN_CLIMATE=temperate|tropical|desert`
    /// draws the whole map in that climate instead, with no divide, for shots and tests.
    pub fn set_map_look(&mut self, look: &MapLook) {
        self.look = match climate_override() {
            Some(climate) => MapLook {
                climate,
                strata_lift: look.strata_lift,
                divide: None,
            },
            None => look.clone(),
        };
        // A map's file checks its line as it is read; one built in code may not have.
        if let Some(Err(e)) = self.look.divide.as_ref().map(ClimateDivide::validate) {
            log::warn!("climate divide left out: {e}");
            self.look.divide = None;
        }
    }

    pub(super) fn climate_globals(&self) -> ClimateGlobals {
        let (divide, points) = crate::sky::divide_points(self.look.divide.as_ref());
        let east = self
            .look
            .divide
            .as_ref()
            .map_or(self.look.climate, |d| d.climate);
        ClimateGlobals {
            climate: climate_code(self.look.climate),
            divide,
            divide_info: [points, climate_code(east), self.look.strata_lift, 0.0],
        }
    }

    /// What the ambient sound listens for (mc-game's ambience.rs): how dark it is,
    /// 0 in daylight to 1 at night; the wind over the ground, metres a second; and
    /// the climate the map is drawn in at `focus`, where the listener is.
    pub fn ambience_cues(&self, focus: glam::Vec2) -> (f32, f32, Climate) {
        (
            self.sky.darkness(),
            self.sky.wind_speed(),
            self.look.climate_at(focus.x, focus.y),
        )
    }
}
