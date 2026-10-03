//! Mines on a coast. A mine on land works only land and one in the sea only the sea,
//! so the survey draws a mine's territory on its own side of the shoreline, its edge
//! following the shore.

use super::mine_marks::{overview_height, TERRITORY_SEGMENTS};
use super::Scene;
use glam::Vec2;
use mc_map::MapFile;
use mc_sim::mines::{OreGrid, GROUND_CELL_M};
use std::collections::BTreeMap;
use std::f32::consts::TAU;

/// Metres between the samples a ray takes looking for the shore.
const STEP: f32 = 12.0;
/// A stretch of the other kind of ground a ray looks past: a rock off the coast, or a
/// pond inland. The sim still counts the ground beyond it, cell by cell; cutting the
/// outline there would leave a wedge of it undrawn.
const GAP: f32 = 240.0;
/// Sites remembered before the memory is cleared: every mine, and wherever the
/// pointer has stood with one in hand.
const MOST_SITES: usize = 512;

/// The map rasterised as the sim counts it, and each mine site's shore, worked out
/// once: it only changes with where a mine stands and its reach.
#[derive(Default)]
pub struct Survey {
    grid: Option<OreGrid>,
    /// Metres out along each of the territory's rays (`mine_marks::territory`) to the
    /// shore (or the map's edge), by site and reach.
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
    /// wherever the other side of it comes in first.
    pub(super) fn on_own_ground(
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

/// Whether a mine at `at` stands in the sea, as the sim's grid (and [`Survey::grid`])
/// counts it: by the ground cell it is in.
pub(super) fn at_sea(map: &MapFile, at: Vec2) -> bool {
    let pitch = GROUND_CELL_M as f32;
    let size = Vec2::from(map.info().size_metres().to_f32());
    let cell = (at / pitch).floor();
    let centre = cell * pitch + pitch * 0.5;
    let inside = cell.x >= 0.0 && cell.y >= 0.0 && centre.x < size.x && centre.y < size.y;
    !inside || overview_height(map, centre) <= map.info().water_level.to_f32()
}

/// Metres out from `at` along each of the territory's rays to where the ground turns
/// to the other kind (land to sea, or sea to land) or the map ends, no further than `most`.
fn shore(s: &Scene, at: Vec2, most: f32) -> Vec<f32> {
    let water = s.map.info().water_level.to_f32();
    let size = Vec2::from(s.map.info().size_metres().to_f32());
    let dry = |p: Vec2| overview_height(s.map, p) > water;
    let home = dry(at);
    let ours = |p: Vec2| p.cmpge(Vec2::ZERO).all() && p.cmplt(size).all() && dry(p) == home;
    (0..=TERRITORY_SEGMENTS)
        .map(|i| {
            let u = Vec2::from_angle(i as f32 / TERRITORY_SEGMENTS as f32 * TAU);
            let mut t = STEP;
            while t < most {
                if !ours(at + u * t)
                    && !(1..=(GAP / STEP) as i32).any(|k| {
                        let ahead = t + k as f32 * STEP;
                        ahead < most && ours(at + u * ahead)
                    })
                {
                    // Between the last sample on our side and this one: halve in on the shore.
                    let (mut lo, mut hi) = (t - STEP, t);
                    for _ in 0..4 {
                        let mid = (lo + hi) * 0.5;
                        if ours(at + u * mid) {
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
