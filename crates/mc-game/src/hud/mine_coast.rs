//! Mines on a coast. The sea pays nothing, so the survey draws a mine's territory on
//! the land only, its edge following the shoreline.

use super::mine_marks::{overview_height, TERRITORY_SEGMENTS};
use super::Scene;
use glam::Vec2;
use mc_sim::mines::OreGrid;
use std::collections::BTreeMap;
use std::f32::consts::TAU;

/// Metres between the samples a ray takes looking for the shore.
const STEP: f32 = 12.0;
/// Sites remembered before the memory is cleared: every mine, and wherever the
/// pointer has stood with one in hand.
const MOST_SITES: usize = 512;

/// The map rasterised as the sim counts it, and each mine site's shore, worked out
/// once: it only changes with where a mine stands and its reach.
#[derive(Default)]
pub struct Survey {
    grid: Option<OreGrid>,
    /// Metres out along each of the territory's rays (`mine_marks::territory`) to the
    /// first sea, by site and reach; unbounded for a mine that stands in the sea.
    shores: BTreeMap<(u32, u32, u32), Vec<f32>>,
}

impl Survey {
    pub(super) fn grid(&mut self, s: &Scene) -> &OreGrid {
        self.grid.get_or_insert_with(|| {
            let water = s.map.info().water_level.to_f32();
            OreGrid::new(s.map.ore_regions(), s.map.info().size_metres(), |p| {
                overview_height(s.map, Vec2::from(p.to_f32())) > water
            })
        })
    }

    /// `outline` (a territory round `at`, one point a ray) cut back to the shore
    /// wherever the sea comes in first.
    pub(super) fn on_land(
        &mut self,
        s: &Scene,
        at: Vec2,
        reach: f32,
        outline: Vec<Vec2>,
    ) -> Vec<Vec2> {
        let key = (at.x.to_bits(), at.y.to_bits(), reach.to_bits());
        if !self.shores.contains_key(&key) {
            if self.shores.len() >= MOST_SITES {
                self.shores.clear();
            }
            self.shores.insert(key, shore(s, at, reach));
        }
        outline
            .into_iter()
            .zip(&self.shores[&key])
            .map(|(p, &land)| {
                let d = p - at;
                at + d.normalize_or_zero() * d.length().min(land)
            })
            .collect()
    }
}

/// Metres out from `at` to the first sea along each of the territory's rays, no
/// further than `most`. From a point in the sea, every ray runs to `most`.
fn shore(s: &Scene, at: Vec2, most: f32) -> Vec<f32> {
    let water = s.map.info().water_level.to_f32();
    let dry = |p: Vec2| overview_height(s.map, p) > water;
    if !dry(at) {
        return vec![f32::MAX; TERRITORY_SEGMENTS + 1];
    }
    (0..=TERRITORY_SEGMENTS)
        .map(|i| {
            let u = Vec2::from_angle(i as f32 / TERRITORY_SEGMENTS as f32 * TAU);
            let mut t = STEP;
            while t < most {
                if !dry(at + u * t) {
                    // Between the last dry sample and this one: halve in on the shore.
                    let (mut lo, mut hi) = (t - STEP, t);
                    for _ in 0..4 {
                        let mid = (lo + hi) * 0.5;
                        if dry(at + u * mid) {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    return lo;
                }
                t += STEP;
            }
            most
        })
        .collect()
}
