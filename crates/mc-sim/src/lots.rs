//! What a lot and a solid map prop keep units off: the path cells they block,
//! and how a structure's lot is taken, given back and re-blocked round its
//! neighbours. Moved out of `world.rs`.

use crate::nav::Nav;
use crate::tables::*;
use crate::World;
use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::{cat, BlueprintId, UnitBlueprint};
use mc_map::Prop;

/// Snaps a structure centre to the build grid, facing `build_heading`: odd
/// footprints centre on a cell, even footprints on a cell corner.
pub fn snap_to_build_grid(bp: &UnitBlueprint, pos: FxVec2) -> FxVec2 {
    snap_turned_to_build_grid(bp, pos, bp.build_heading())
}

/// `snap_to_build_grid` for a site turned to `heading` (`UnitBlueprint::footprint_at`).
pub fn snap_turned_to_build_grid(bp: &UnitBlueprint, pos: FxVec2, heading: Angle) -> FxVec2 {
    let footprint = bp.footprint_at(heading);
    let cell = Fx::from_int(mc_map::BUILD_CELL_M);
    let snap = |v: Fx, cells: u8| {
        if cells % 2 == 1 {
            Fx::from_int((v / cell).floor_int()) * cell + cell / 2
        } else {
            Fx::from_int((v / cell).round_int()) * cell
        }
    };
    FxVec2::new(snap(pos.x, footprint.0), snap(pos.y, footprint.1))
}

fn lot_extents(footprint: (u8, u8), pos: FxVec2) -> (i32, i32, i32, i32) {
    let half_w = footprint.0 as i32 * mc_map::BUILD_CELL_M / 2;
    let half_h = footprint.1 as i32 * mc_map::BUILD_CELL_M / 2;
    let cx = pos.x.round_int();
    let cy = pos.y.round_int();
    (cx - half_w, cy - half_h, cx + half_w, cy + half_h)
}

/// Inclusive lot rectangle: a deposit on the edge still counts as covered.
pub fn lot_covers_point(footprint: (u8, u8), pos: FxVec2, point: FxVec2) -> bool {
    let (x0, y0, x1, y1) = lot_extents(footprint, pos);
    let px = point.x.round_int();
    let py = point.y.round_int();
    px >= x0 && px <= x1 && py >= y0 && py <= y1
}

/// Metres of a path cell, across, a hull may cover and still leave the cell
/// walkable. Units brush past a building's walls; a lane between two lots
/// stays open once each hull stands this far back from the lot edge.
const HULL_GRACE_M: i32 = 3;

/// Inclusive path-cell rectangles a structure's hull keeps units off: the lot
/// interior cells the hull (turned to `heading`) covers by more than
/// [`HULL_GRACE_M`] across. The rest of the lot is apron units walk on.
/// Walls fill their lot.
pub fn hull_cells(
    bp: &UnitBlueprint,
    pos: FxVec2,
    heading: Angle,
) -> Vec<((u32, u32), (u32, u32))> {
    let (min, max) = place_cells_of(bp.footprint_at(heading), pos);
    if bp.has(cat::WALL) {
        return vec![(min, max)];
    }
    // A seabed installation stands deep under the keels: ships sail over it.
    if bp.seabed {
        return Vec::new();
    }
    let cell = mc_map::CELL_SIZE_M;
    let grow = Fx::from_int(cell / 2 - HULL_GRACE_M);
    let (hx, hy) = (bp.hull.0 + grow, bp.hull.1 + grow);
    let inside = |x: u32, y: u32| {
        let c = FxVec2::from_ints(x as i32 * cell + cell / 2, y as i32 * cell + cell / 2);
        let local = (c - pos).rotate(-heading);
        local.x.abs() < hx && local.y.abs() < hy
    };
    let mut out: Vec<((u32, u32), (u32, u32))> = Vec::new();
    for y in min.1..=max.1 {
        let mut x = min.0;
        while x <= max.0 {
            if !inside(x, y) {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < max.0 && inside(x + 1, y) {
                x += 1;
            }
            // Grow the rectangle from the row below when it spans the same cells.
            match out
                .iter_mut()
                .find(|r| r.0 .0 == x0 && r.1 .0 == x && r.1 .1 + 1 == y)
            {
                Some(r) => r.1 .1 = y,
                None => out.push(((x0, y), (x, y))),
            }
            x += 1;
        }
    }
    if out.is_empty() {
        // A hull too small to cover any cell still stands on the one under its middle.
        let c = (
            (pos.x.floor_int() / cell).max(0) as u32,
            (pos.y.floor_int() / cell).max(0) as u32,
        );
        out.push((c, c));
    }
    out
}

/// Path cells the lot overlaps when rounded out to 8 m. Flattening and the
/// terrain check use this so a 12 m pad still sits on ground that covers it.
pub(crate) fn path_cells_of(footprint: (u8, u8), pos: FxVec2) -> ((u32, u32), (u32, u32)) {
    let cell = mc_map::CELL_SIZE_M;
    let (x0, y0, x1, y1) = lot_extents(footprint, pos);
    let to_cell = |lo: i32, hi: i32| {
        let a = lo.max(0) / cell;
        let b = ((hi + cell - 1) / cell - 1).max(a);
        (a as u32, b as u32)
    };
    let (xa, xb) = to_cell(x0, x1);
    let (ya, yb) = to_cell(y0, y1);
    ((xa, ya), (xb, yb))
}

/// Path cells whose centres lie strictly inside the lot. Neighbouring 12 m
/// lots share the 8 m cells their edges round into; those stay walkable and
/// must not block placement.
pub(crate) fn place_cells_of(footprint: (u8, u8), pos: FxVec2) -> ((u32, u32), (u32, u32)) {
    let cell = mc_map::CELL_SIZE_M;
    let (x0, y0, x1, y1) = lot_extents(footprint, pos);
    let to_cell = |lo: i32, hi: i32| {
        let first = (lo - cell / 2).div_euclid(cell) + 1;
        let last = (hi - cell / 2 - 1).div_euclid(cell);
        let a = first.max(0);
        let b = last.max(a);
        (a as u32, b as u32)
    };
    let (xa, xb) = to_cell(x0, x1);
    let (ya, yb) = to_cell(y0, y1);
    ((xa, ya), (xb, yb))
}

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
        let lot = place_cells_of(bp.footprint_at(heading), pos);
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
                let other_lot = place_cells_of(bp.footprint_at(units.heading[row]), units.pos[row]);
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
        for (min, max) in released {
            self.nav.reseal(min, max);
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
        for (min, max) in released {
            self.nav.reseal(min, max);
        }
    }

    /// Blocks a structure's hull for pathing and takes its lot.
    pub(crate) fn occupy_lot(&mut self, bp: &UnitBlueprint, pos: FxVec2, heading: Angle) {
        for (min, max) in hull_cells(bp, pos, heading) {
            self.nav.block_cells(min, max);
            self.nav.reseal(min, max);
        }
        let lot = place_cells_of(bp.footprint_at(heading), pos);
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
                let units = &self.state.units;
                let bp = self.blueprints.unit(units.blueprint[row]);
                let site = bp.is_site_built_unit() && units.has_flag(row, flag::UNDER_CONSTRUCTION);
                (bp.is_structure() || site)
                    .then(|| place_cells_of(bp.footprint_at(units.heading[row]), units.pos[row]))
            })
            .collect();
        for (min, max) in lots {
            self.nav.set_lot(min, max, true);
        }
    }
}
