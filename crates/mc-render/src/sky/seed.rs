//! Storm clouds handing themselves to the weather (clouds_sim.wgsl `cs_force`, kind seed):
//! a Nimbus's spent thunderhead rises into the layer and its water joins it, so a run
//! leaves the sky over its track a little more overcast, carried off by the wind and
//! healing back to the day's weather in its own time.
use super::*;
use crate::gpu_consts::disturber::{SEED, SEED_FEEDS};

/// At most this many seeds feed the weather at once: a cosmetic cap, the newest past it
/// leave the layer alone (a few Nimbus runs at once lay this many).
const MOST_SEEDS: usize = 48;

impl Sky {
    /// A storm cloud `reach` metres round `at` joining the cloud layer: for `SEED_FEEDS`
    /// seconds the weather there gathers toward full cover, most at its middle.
    pub fn seed(&mut self, at: Vec2, reach: f32, time: f32) {
        if reach > 0.0 && reach.is_finite() && at.is_finite() && self.seeds.len() < MOST_SEEDS {
            self.seeds.push((at, reach, time));
        }
    }

    /// The seeds still feeding the layer at `now`, as disturbers, up to `room` of them (the
    /// disturber buffer's room after the wakes and blasts: past it, a cosmetic cap, the
    /// newest seeds wait out of the weather).
    pub(super) fn seed_disturbers(&mut self, now: f32, room: usize) -> Vec<Disturber> {
        self.seeds.retain(|s| now - s.2 < SEED_FEEDS);
        self.seeds
            .iter()
            .take(room)
            .map(|&(at, reach, start)| Disturber {
                a: [at.x, at.y, reach, SEED],
                b: [0.0, 0.0, 1.0, now - start],
                c: [0.0; 4],
            })
            .collect()
    }
}
