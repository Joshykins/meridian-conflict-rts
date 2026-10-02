//! How the map's ground and sea are drawn (its `MapLook`, from `maps/<stem>.ron`): its
//! climate, and on a map with regions each region's climate, as the shaders read
//! them (`Globals::climate`, `region_climate`, `map_look`; bindings.wgsl
//! `climate_at`). The climate walls between the regions go to the sky, which holds
//! them for every shader (`Atmosphere::walls`, sky/regions.rs).

use super::*;
use crate::gpu_consts::regions;
use mc_data::weather::{Climate, MapLook};

/// `MERIDIAN_CLIMATE=temperate|tropical|desert`: draws any map in that climate, all of
/// it (`Renderer::set_map_look`).
fn climate_override() -> Option<Climate> {
    std::env::var("MERIDIAN_CLIMATE")
        .ok()
        .and_then(|v| Climate::from_name(&v))
}

/// The climate as the shaders read it (`Globals::climate.x`; `tropical()` and
/// `desert()` in bindings.wgsl).
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
    pub(super) region_climate: [[f32; 4]; regions::MAX as usize],
    pub(super) map_look: [f32; 4],
}

impl Renderer {
    /// How the map's ground and sea are drawn (its `MapConfig::look`): the climate's
    /// palette, and on a map with regions each region's, the climate walls between
    /// them and the light along the walls' foot. `MERIDIAN_CLIMATE=temperate|tropical|desert`
    /// draws the whole map in that climate instead, with no regions, for shots and tests.
    pub fn set_map_look(&mut self, look: &MapLook) {
        self.look = match climate_override() {
            Some(climate) => {
                let mut one = MapLook::single(climate);
                one.strata_lift = look.strata_lift;
                one
            }
            None => look.clone(),
        };
        self.sky.set_walls(self.look.walls().clone());
    }

    /// How the map is drawn now: its look as `set_map_look` took it.
    pub fn map_look(&self) -> &MapLook {
        &self.look
    }

    pub(super) fn climate_globals(&self) -> ClimateGlobals {
        let climates = self.look.climates();
        let mut region_climate = [[0.0; 4]; regions::MAX as usize];
        for (slot, climate) in region_climate.iter_mut().zip(climates) {
            *slot = [
                (*climate == Climate::Tropical) as u32 as f32,
                (*climate == Climate::Desert) as u32 as f32,
                0.0,
                0.0,
            ];
        }
        ClimateGlobals {
            climate: climate_code(climates[0]),
            region_climate,
            map_look: [self.look.strata_lift, 0.0, 0.0, 0.0],
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
