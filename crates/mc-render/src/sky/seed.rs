//! Storm clouds in the weather (clouds_sim.wgsl `cs_force`, kind `disturber::SEED`): a
//! Nimbus's seeds are drawn by the sky itself, as cells of its weather (renderer
//! `storm_cloud_fx` says where, how wide and how stormy each is, every tick). Round each the
//! cover and the storm gather toward its own; when it lets go, what it left drifts off on
//! the wind and heals back to the day's weather like any other cloud.
use super::*;
use crate::gpu_consts::disturber::SEED;

/// At most this many cells stir the weather at once: a cosmetic cap, past it the newest
/// wait (some five Nimbus trails at once).
pub(super) const MOST_SEEDS: usize = 96;

/// One storm cloud as the weather takes it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeedCell {
    pub(crate) at: Vec2,
    /// Metres round `at` it fills, most at its middle.
    pub(crate) radius: f32,
    /// The cover (0-1) and storm (0-1) the weather there gathers toward.
    pub(crate) cover: f32,
    pub(crate) storm: f32,
}

impl Sky {
    /// The storm clouds in the weather now, in place of the last set.
    pub(crate) fn set_seeds(&mut self, mut seeds: Vec<SeedCell>) {
        seeds.retain(|s| s.radius > 0.0 && s.radius.is_finite() && s.at.is_finite());
        seeds.truncate(MOST_SEEDS);
        self.seeds = seeds;
    }

    /// The storm clouds as disturbers.
    pub(super) fn seed_disturbers(&self) -> impl Iterator<Item = Disturber> + '_ {
        self.seeds.iter().map(|s| Disturber {
            a: [s.at.x, s.at.y, s.radius, SEED],
            b: [s.cover, s.storm, 0.0, 0.0],
            c: [0.0; 4],
        })
    }
}
