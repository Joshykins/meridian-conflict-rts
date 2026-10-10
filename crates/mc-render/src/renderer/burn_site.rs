//! What the fire, smoke and wreck effects need to know about a blueprint's model.

use crate::models::{self, Model};
use glam::Vec3;

/// What `damage_fires`, `wreck_smoke` and `turret_landings` need to know about a
/// blueprint's model.
#[derive(Clone)]
pub(super) struct BurnSite {
    pub(super) grid: models::burns::BurnGrid,
    pub(super) reach: f32,
    /// The model's bounds radius: what a wreck section's stretch is in shares of.
    pub(super) bounds: f32,
    pub(super) height: f32,
    pub(super) turret_pivot: Vec3,
    /// It has a turret, which its wreck throws off (`wreck_throw.rs`).
    pub(super) throws_turret: bool,
}

impl BurnSite {
    /// `height`: the model's, as the renderer measures it for the GPU.
    pub(super) fn of(model: &Model, height: f32) -> BurnSite {
        BurnSite {
            grid: models::burns::BurnGrid::bake(&model.lods[0]),
            // As `entity.wgsl` hands them to the surface shader, so the marks agree.
            reach: model.surface_reach.max(1.0),
            bounds: model.bounds_radius.max(1.0),
            height: height.max(1.0),
            turret_pivot: Vec3::from(model.turret_pivot),
            throws_turret: model.lods[0]
                .vertices
                .iter()
                .any(|v| v.part == models::part::TURRET),
        }
    }
}
