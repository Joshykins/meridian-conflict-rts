//! Storm clouds in the weather (clouds_sim.wgsl `cs_force`, kind `disturber::SEED`): a
//! Nimbus's seeds are drawn by the sky itself, as cells of its weather (renderer
//! `storm_cloud_fx` says where, how wide and how high each is, every tick). Round each the
//! cover gathers toward its own, and the weather's storm goes below zero: the mark of a
//! seeded cloud, which the clouds draw at the height the bomber flies (`Atmosphere::seeded`)
//! and never thin round the selection. When it lets go, what it left drifts off on the
//! wind, the mark fades and it heals back to the day's weather like any other cloud.
use super::*;
use crate::gpu_consts::disturber::{SEED, SEED_UNDER};

/// At most this many cells stir the weather at once: a cosmetic cap, past it the newest
/// wait (some five Nimbus trails at once).
pub(super) const MOST_SEEDS: usize = 96;

/// One storm cloud as the weather takes it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SeedCell {
    pub(crate) at: Vec2,
    /// Metres round `at` it fills, most at its middle.
    pub(crate) radius: f32,
    /// The cover (0-1) the weather there gathers toward.
    pub(crate) cover: f32,
    /// The height it is let go at, metres: where the bomber flies.
    pub(crate) height: f32,
}

impl Sky {
    /// The storm clouds in the weather now, in place of the last set.
    pub(crate) fn set_seeds(&mut self, mut seeds: Vec<SeedCell>) {
        seeds.retain(|s| s.radius > 0.0 && s.radius.is_finite() && s.at.is_finite());
        seeds.truncate(MOST_SEEDS);
        // Over the cloud floor, the heights of the seeds now; the last kept once they are
        // all spent, for the marks they leave to fade at the same height.
        if !seeds.is_empty() {
            let sum: f32 = seeds.iter().map(|s| s.height - self.floor_at(s.at)).sum();
            self.seeded = (sum / seeds.len() as f32).max(SEED_UNDER);
        }
        self.seeds = seeds;
    }

    /// The storm clouds as disturbers.
    pub(super) fn seed_disturbers(&self) -> impl Iterator<Item = Disturber> + '_ {
        self.seeds.iter().map(|s| Disturber {
            a: [s.at.x, s.at.y, s.radius, SEED],
            b: [s.cover, 1.0, 0.0, 0.0],
            c: [0.0; 4],
        })
    }
}
