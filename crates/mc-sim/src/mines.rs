//! Core mines: materials out of the map's mine points.
//!
//! A map has a fixed set of mine points, one to each of its ore fields
//! ([`World::mine_points`]). A mine may stand only on a point, one mine to a
//! point, anyone's, and it digs a fixed `rate` a second from the moment it is
//! finished. The tiers are the investment: an upgrade raises the rate.

use crate::nav::cell_class;
use crate::placement::{check_cells, CITY};
use crate::tables::UnitId;
use crate::world::{prop_cells, snap_to_build_grid};
use crate::World;
use mc_core::{Fx, FxVec2, StateHasher, TICKS_PER_SECOND};
use mc_data::UnitBlueprint;
use mc_map::{Heightfield, OreRegion, Prop};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const DT: i32 = TICKS_PER_SECOND as i32;
/// The share of its output a mine still makes with none of its energy upkeep
/// paid, as a fraction: enough to climb out of a stall, never to live on.
pub const UNPOWERED: (i64, i64) = (1, 4);
/// Metres from a mine point within which a build order for a mine goes onto
/// that point; farther off, the order is refused.
pub const POINT_SNAP_M: i32 = 60;
/// Metres a point may move off its ore field's middle to find ground a mine
/// can stand on.
const POINT_SEARCH_M: i32 = 96;

/// One mine's own state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MineState {
    /// Ticks since it was finished. Kept through an upgrade.
    pub age: u32,
}

/// Every live, finished mine, by unit id.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mines {
    pub by_unit: BTreeMap<UnitId, MineState>,
}

impl Mines {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.by_unit.len() as u64);
        for (id, m) in &self.by_unit {
            h.write_u64(id.0 as u64 | (m.age as u64) << 32);
        }
    }
}

/// The share of its full output a mine makes when `powered` of its side's
/// upkeep is paid: [`UNPOWERED`] with no energy at all, rising to all of it.
pub fn mine_power(powered: Fx) -> Fx {
    let unpowered = Fx::ratio(UNPOWERED.0, UNPOWERED.1);
    unpowered + (Fx::ONE - unpowered) * powered.clamp(Fx::ZERO, Fx::ONE)
}

/// Ticks between blows of a core mine's hammer at `tech`. The heavier rigs strike
/// slower. Only the look and sound keep this beat: output is paid every tick.
pub fn hammer_ticks(tech: u8) -> u32 {
    match tech {
        0 | 1 => 28,
        2 => 32,
        3 => 36,
        _ => 48,
    }
}

/// A mine's hammer as the mirror publishes it in `UnitInstance::gait`: blows struck since
/// it was finished (wrapping at 256), and what one tick adds to that, twice. So a blow
/// lands where the count passes a whole number. The renderer drops the hammer and the
/// interface hears it by this, the way a walker's feet go down.
#[expect(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    reason = "presentation: the mirror publishes it as UnitInstance::gait for the renderer and audio"
)]
pub fn hammer_gait(age: u32, tech: u8) -> [f32; 3] {
    let period = hammer_ticks(tech);
    let step = 1.0 / period as f32;
    [(age % (period * 256)) as f32 * step, step, step]
}

/// A map's mine points: one to each ore field, at the lot of `bp` (a core mine)
/// nearest the field's middle that a mine can stand on (land, or water for a mine
/// that floats; never a city block), and never overlapping an earlier point's lot.
/// A field with no such lot within [`POINT_SEARCH_M`] has no point. In map order.
/// From the map alone, so the sim and the interface agree on them.
pub fn mine_points(
    ground: &Heightfield,
    props: &[Prop],
    ore: &[OreRegion],
    bp: &UnitBlueprint,
) -> Vec<FxVec2> {
    let size = ground.size_metres();
    let water = ground.water_level();
    let cell = mc_map::BUILD_CELL_M;
    let lot = Fx::from_int(bp.footprint.0.max(bp.footprint.1) as i32 * cell);
    let rings = POINT_SEARCH_M / cell;
    let mut points: Vec<FxVec2> = Vec::new();
    for field in ore {
        let middle = snap_to_build_grid(bp, field.centre());
        // City blocks near enough to touch a lot the search may try.
        let near = Fx::from_int(POINT_SEARCH_M + 200);
        let mut city: Vec<((u32, u32), (u32, u32))> = Vec::new();
        for p in props
            .iter()
            .filter(|p| p.pos.distance_sq(middle) < near * near)
        {
            city.extend(prop_cells(p, size));
        }
        let class = |x: u32, y: u32| {
            let (w, h) = ground.size_cells();
            if x >= w || y >= h {
                return 0;
            }
            let in_city = city
                .iter()
                .any(|&(lo, hi)| x >= lo.0 && x <= hi.0 && y >= lo.1 && y <= hi.1);
            cell_class(ground, water, x, y) | if in_city { CITY } else { 0 }
        };
        let mut best = None;
        'search: for ring in 0..=rings {
            // Each ring of lots in turn, nearest first within it.
            let mut spots: Vec<FxVec2> = (-ring..=ring)
                .flat_map(|dy| (-ring..=ring).map(move |dx| (dx, dy)))
                .filter(|&(dx, dy)| dx.abs().max(dy.abs()) == ring)
                .map(|(dx, dy)| middle + FxVec2::from_ints(dx * cell, dy * cell))
                .collect();
            spots.sort_by_key(|p| (p.distance_sq(middle), p.x, p.y));
            for p in spots {
                let clear = points
                    .iter()
                    .all(|q| (q.x - p.x).abs() >= lot || (q.y - p.y).abs() >= lot);
                if clear && check_cells(bp, p, size, class).is_ok() {
                    best = Some(p);
                    break 'search;
                }
            }
        }
        points.extend(best);
    }
    points
}

/// The core mine that mine points are laid out for: the first one in the data.
/// Every mine has the same 3x3 lot, so the points suit them all.
pub fn point_blueprint(blueprints: &mc_data::Blueprints) -> Option<&UnitBlueprint> {
    blueprints.units.iter().find(|b| b.mine.is_some())
}

impl World {
    /// This map's mine points ([`mine_points`]).
    pub(crate) fn find_mine_points(&self) -> Vec<FxVec2> {
        point_blueprint(&self.blueprints).map_or_else(Vec::new, |bp| {
            mine_points(&self.terrain, &self.map.props, &self.map.ore, bp)
        })
    }

    /// The mine point a mine's lot centred at `pos` stands on, if any.
    pub fn mine_point_at(&self, pos: FxVec2) -> Option<usize> {
        self.mine_points.iter().position(|&p| p == pos)
    }

    /// The mine point nearest `pos` within [`POINT_SNAP_M`], if any.
    pub fn mine_point_near(&self, pos: FxVec2) -> Option<FxVec2> {
        let reach = Fx::from_int(POINT_SNAP_M);
        self.mine_points
            .iter()
            .copied()
            .filter(|p| p.distance_sq(pos) <= reach * reach)
            .min_by_key(|p| (p.distance_sq(pos), p.x, p.y))
    }

    /// A mine, finished or a site, anyone's, standing on the point at `point`.
    pub fn mine_on_point(&self, point: FxVec2) -> Option<usize> {
        let units = &self.state.units;
        units
            .slots
            .iter()
            .find(|&row| self.bp(row).mine.is_some() && units.pos[row] == point)
    }

    /// Adds finished mines and drops dead ones, and ages them.
    pub(crate) fn run_mines(&mut self) {
        let units = &self.state.units;
        let mines = &mut self.state.mines.by_unit;
        mines.retain(|id, _| units.row(*id).is_some_and(|row| units.is_active(row)));
        for m in mines.values_mut() {
            m.age = m.age.saturating_add(1);
        }
        for row in units.slots.iter() {
            if units.is_active(row) && self.blueprints.unit(units.blueprint[row]).mine.is_some() {
                mines.entry(units.id(row)).or_default();
            }
        }
    }

    /// Mines' output for this tick: into `income` (per player, per tick) and each mine's flow.
    /// Adds every mine's output to its owner's mass income, scaled by `powered`,
    /// the share of its side's energy that is covered. Returns the mass per tick
    /// each side's mines fell short of their full output by.
    pub(crate) fn mine_income(&mut self, income: &mut [(Fx, Fx)], powered: &[Fx]) -> Vec<Fx> {
        let mut lost = vec![Fx::ZERO; income.len()];
        for &id in self.state.mines.by_unit.keys() {
            let Some(row) = self.state.units.row(id) else {
                continue;
            };
            let Some(bp) = self.blueprints.unit(self.state.units.blueprint[row]).mine else {
                continue;
            };
            let p = self.state.units.owner[row] as usize;
            let full = bp.rate / DT;
            let made = full * mine_power(powered[p]);
            income[p].0 += made;
            lost[p] += full - made;
            if row >= self.flows.len() {
                self.flows.resize(row + 1, Default::default());
            }
            self.flows[row].made[0] += made;
        }
        lost
    }

    /// Material fabricators' output for this tick, into `income` and each one's flow:
    /// all of it with `powered` (its side's energy share) whole, none with no energy.
    pub(crate) fn fabricator_income(&mut self, income: &mut [(Fx, Fx)], powered: &[Fx]) {
        for row in self.state.units.slots.iter() {
            if !self.state.units.is_active(row) {
                continue;
            }
            let Some(f) = self.bp(row).fabricator else {
                continue;
            };
            let p = self.state.units.owner[row] as usize;
            let made = f.mass / DT * powered[p].clamp(Fx::ZERO, Fx::ONE);
            income[p].0 += made;
            if row >= self.flows.len() {
                self.flows.resize(row + 1, Default::default());
            }
            self.flows[row].made[0] += made;
        }
    }
}
