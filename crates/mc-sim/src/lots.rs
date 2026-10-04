//! What a lot and a solid map prop keep units off: the path cells they block,
//! and how a structure's lot is taken, given back and re-blocked round its
//! neighbours. Moved out of `world.rs`.

use crate::nav::Nav;
use crate::tables::*;
use crate::world::{hull_cells, place_cells_of};
use crate::World;
use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::{BlueprintId, UnitBlueprint};
use mc_map::Prop;

/// City buildings and precursor artifacts are solid. Their cells are blocked,
/// once, before the match starts.
pub(crate) fn block_buildings(nav: &mut Nav, props: &[Prop], map_size: FxVec2) {
    for p in props {
        for (min, max) in prop_cells(p, map_size) {
            nav.block_cells(min, max);
        }
    }
}

/// The cell rectangles a map prop makes solid: a city building's lot on the
/// build grid, or a precursor artifact's or a landmark's solid parts
/// (`Prop::solid_runs`), a row at a time. Nothing for trees and rocks.
pub(crate) fn prop_cells(p: &Prop, map_size: FxVec2) -> Vec<((u32, u32), (u32, u32))> {
    if !p.kind.solid_plan().is_empty() {
        let cell = mc_map::CELL_SIZE_M;
        let cells = (
            (map_size.x.floor_int() / cell) as u32,
            (map_size.y.floor_int() / cell) as u32,
        );
        return p
            .solid_runs(cells)
            .into_iter()
            .map(|(y, a, b)| ((a, y), (b, y)))
            .collect();
    }
    building_cells(p, map_size).into_iter().collect()
}

pub(crate) fn building_cells(p: &Prop, map_size: FxVec2) -> Option<((u32, u32), (u32, u32))> {
    if !p.kind.is_building() {
        return None;
    }
    let grid = mc_map::BUILD_CELL_M;
    let half = 20 * p.scale_milli as i32 / 1000 + 2;
    let (x, y) = (p.pos.x.round_int(), p.pos.y.round_int());
    let lo = |v: i32| ((v - half).div_euclid(grid) * grid).max(0);
    let hi = |v: i32, limit: i32| (((v + half + grid - 1).div_euclid(grid)) * grid).min(limit);
    let (x0, y0) = (lo(x), lo(y));
    let (x1, y1) = (hi(x, map_size.x.floor_int()), hi(y, map_size.y.floor_int()));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let cell = mc_map::CELL_SIZE_M;
    Some((
        ((x0 / cell) as u32, (y0 / cell) as u32),
        ((x1 / cell - 1) as u32, (y1 / cell - 1) as u32),
    ))
}

pub(crate) fn cells_overlap(a: ((u32, u32), (u32, u32)), b: ((u32, u32), (u32, u32))) -> bool {
    a.0 .0 <= b.1 .0 && b.0 .0 <= a.1 .0 && a.0 .1 <= b.1 .1 && b.0 .1 <= a.1 .1
}

impl World {
    /// Drops a structure's hull blockers and frees its lot, then puts back any
    /// blocker still needed by a neighbour or a city.
    pub(crate) fn release_lot(
        &mut self,
        except: usize,
        bp: &UnitBlueprint,
        pos: FxVec2,
        heading: Angle,
    ) {
        let released = hull_cells(bp, pos, heading);
        for &(min, max) in &released {
            self.nav.unblock_cells(min, max);
        }
        let lot = place_cells_of(bp.footprint, pos);
        self.nav.set_lot(lot.0, lot.1, false);
        let restore = {
            let units = &self.state.units;
            let mut restore = Vec::new();
            for row in units.slots.iter() {
                if row == except {
                    continue;
                }
                let bp = self.blueprints.unit(units.blueprint[row]);
                if !bp.is_structure()
                    && !(bp.is_site_built_unit() && units.has_flag(row, flag::UNDER_CONSTRUCTION))
                {
                    continue;
                }
                let other_lot = place_cells_of(bp.footprint, units.pos[row]);
                if !cells_overlap(lot, other_lot) {
                    continue;
                }
                // A successor assembled inside this lot keeps it.
                self.nav.set_lot(other_lot.0, other_lot.1, true);
                for other in hull_cells(bp, units.pos[row], units.heading[row]) {
                    if released.iter().any(|&r| cells_overlap(r, other)) {
                        restore.push(other);
                    }
                }
            }
            let map_size = self.terrain.size_metres();
            for (i, p) in self.map.props.iter().enumerate() {
                // A city block that came down left rubble its cells stay open over.
                if !(p.kind.is_building() || !p.kind.solid_plan().is_empty())
                    || !self.is_prop_alive(i)
                {
                    continue;
                }
                for cells in prop_cells(p, map_size) {
                    if released.iter().any(|&r| cells_overlap(r, cells)) {
                        restore.push(cells);
                    }
                }
            }
            restore
        };
        for (min, max) in restore {
            self.nav.block_cells(min, max);
        }
    }

    /// Opens the cells map prop `prop` (a city structure that came down, its
    /// footprint `reach` round `middle`) blocked, save those a live neighbour
    /// still stands on: another structure, or a solid prop that is not one.
    pub(crate) fn open_prop_cells(&mut self, prop: usize, middle: FxVec3, reach: Fx) {
        let map_size = self.terrain.size_metres();
        let released = prop_cells(&self.map.props[prop], map_size);
        for &(min, max) in &released {
            self.nav.unblock_cells(min, max);
        }
        let mut restore = Vec::new();
        let mut keep = |p: &Prop| {
            for cells in prop_cells(p, map_size) {
                if released.iter().any(|&r| cells_overlap(r, cells)) {
                    restore.push(cells);
                }
            }
        };
        // The cells a part covers have their centres in it: a neighbour sharing
        // one reaches within a cell of this footprint.
        let near = self.city_shapes.near(
            &self.state.city,
            middle,
            reach + Fx::from_int(mc_map::CELL_SIZE_M * 2),
        );
        for (row, _, _) in near {
            keep(&self.map.props[self.state.city.prop[row] as usize]);
        }
        for &other in &self.city_shapes.other_solids {
            if self.is_prop_alive(other as usize) {
                keep(&self.map.props[other as usize]);
            }
        }
        for (min, max) in restore {
            self.nav.block_cells(min, max);
        }
    }

    /// Blocks a structure's hull for pathing and takes its lot.
    pub(crate) fn occupy_lot(&mut self, bp: &UnitBlueprint, pos: FxVec2, heading: Angle) {
        for (min, max) in hull_cells(bp, pos, heading) {
            self.nav.block_cells(min, max);
        }
        let lot = place_cells_of(bp.footprint, pos);
        self.nav.set_lot(lot.0, lot.1, true);
    }

    /// A structure became another in place (an upgrade): re-block if its hull changed.
    pub(crate) fn reshape_lot(&mut self, row: usize, old: BlueprintId, new: BlueprintId) {
        let (pos, heading) = (self.state.units.pos[row], self.state.units.heading[row]);
        let old_bp = self.blueprints.unit(old).clone();
        let new_bp = self.blueprints.unit(new).clone();
        if hull_cells(&old_bp, pos, heading) == hull_cells(&new_bp, pos, heading) {
            return;
        }
        self.release_lot(row, &old_bp, pos, heading);
        self.occupy_lot(&new_bp, pos, heading);
    }

    /// Lot occupancy from the standing structures, after a snapshot restore.
    pub(crate) fn rebuild_lots(&mut self) {
        let lots: Vec<_> = self
            .state
            .units
            .slots
            .iter()
            .filter_map(|row| {
                let bp = self.blueprints.unit(self.state.units.blueprint[row]);
                let site = bp.is_site_built_unit()
                    && self.state.units.has_flag(row, flag::UNDER_CONSTRUCTION);
                (bp.is_structure() || site)
                    .then(|| place_cells_of(bp.footprint, self.state.units.pos[row]))
            })
            .collect();
        for (min, max) in lots {
            self.nav.set_lot(min, max, true);
        }
    }
}
