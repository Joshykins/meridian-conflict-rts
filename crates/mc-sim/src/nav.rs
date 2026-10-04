//! The simulation's view of `mc-path`: terrain classification, field handles
//! that fit in a table column, and background builds on the job pool.

use crate::SimError;
use mc_core::{Fx, FxVec2, StateHasher};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_path::{
    terrain, Cell, CellRect, FieldId, NavConfig, NavGrid, PathError, Sample, SizeClass, Spawner,
};
use std::sync::Arc;

mod pockets;

/// Steeper than this (rise over run) and ground units cannot cross the cell.
const MAX_SLOPE: Fx = Fx::ratio(1, 2);
/// Water shallower than this is wadeable but not navigable by ships.
const SHALLOW_DEPTH: Fx = Fx::from_int(6);
/// How far from an unreachable goal (inside a structure, in a lake) to look for a usable one.
const GOAL_SEARCH_CELLS: i32 = 40;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Steer {
    Direction(FxVec2),
    Arrived,
    /// The field is not due yet on this tick. Identical on every machine.
    Pending,
    /// The unit strayed outside the built corridor; ask for more.
    NeedsExtend,
    /// Something was built on the goal after the field was asked for: drop the
    /// field and ask again, and the new one leads to the nearest open ground.
    Rebuilt,
    Unreachable,
}

struct PoolSpawner(Arc<Pool>);

impl Spawner for PoolSpawner {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        // Detached: the result comes back through mc-path's own slot.
        drop(self.0.spawn_background("flow-field", task));
    }
}

/// What a field request came to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route {
    /// A handle for the unit's field column.
    Field(u32),
    /// Nowhere near the goal can be stood on; the order cannot be carried out.
    Unreachable,
    /// Pathing is at a budget (every field slot or corridor anchor in use).
    /// Nothing is held; ask again on a later tick.
    Busy,
}

pub struct Nav {
    inner: mc_path::Nav,
    /// Table handles -> path fields. One entry per unit request, so releasing is symmetrical.
    handles: Vec<Option<FieldId>>,
    free: Vec<u32>,
    /// Path cells inside a structure's lot, standing or begun. A structure blocks
    /// only its hull for pathing; this keeps the rest of the lot, the walkable
    /// apron, from being built over. Derived from the units: never snapshotted.
    lots: Vec<bool>,
    lots_w: u32,
    pockets: pockets::Pockets,
}

fn layer(l: mc_data::MoveLayer) -> mc_path::MoveLayer {
    match l {
        mc_data::MoveLayer::Land => mc_path::MoveLayer::Land,
        mc_data::MoveLayer::Amphibious => mc_path::MoveLayer::Amphibious,
        mc_data::MoveLayer::Naval => mc_path::MoveLayer::Naval,
        mc_data::MoveLayer::Hover | mc_data::MoveLayer::Air => mc_path::MoveLayer::Hover,
    }
}

fn size(class: u8) -> SizeClass {
    SizeClass::new(class.min(mc_path::SIZE_CLASSES - 1)).expect("clamped to a valid class")
}

fn path_error(e: PathError) -> SimError {
    SimError::Path(e.to_string())
}

/// The part of a snapshot that restores pathing exactly: `mc-path`'s own state
/// plus the handle table the unit columns index into.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct NavSnapshot {
    path_state: Vec<u8>,
    handles: Vec<Option<u64>>,
    free: Vec<u32>,
    pockets: Vec<(u8, u8, i32, i32)>,
}

/// A cell's terrain class (`mc_path::terrain`) from the heightfield alone:
/// land, shallow or deep water by its lowest corner, and steep past `MAX_SLOPE`.
pub fn cell_class(terrain: &Heightfield, water: Fx, cx: u32, cy: u32) -> u8 {
    let low = terrain
        .sample_height(cx, cy)
        .min(terrain.sample_height(cx + 1, cy))
        .min(terrain.sample_height(cx, cy + 1))
        .min(terrain.sample_height(cx + 1, cy + 1));
    let depth = water - low;
    let mut c = if depth <= Fx::ZERO {
        terrain::LAND
    } else if depth < SHALLOW_DEPTH {
        terrain::SHALLOW
    } else {
        terrain::DEEP
    };
    if terrain.cell_slope(cx, cy) > MAX_SLOPE {
        c |= terrain::STEEP;
    }
    c
}

impl Nav {
    pub fn new(terrain: &Heightfield, pool: Arc<Pool>) -> Result<Nav, SimError> {
        let grid = Self::base_grid(terrain, &pool)?;
        let inner = mc_path::Nav::new(grid, NavConfig::default(), Arc::new(PoolSpawner(pool)));
        let (w, h) = terrain.size_cells();
        Ok(Nav {
            inner,
            handles: Vec::new(),
            free: Vec::new(),
            lots: vec![false; (w * h) as usize],
            lots_w: w,
            pockets: pockets::Pockets::default(),
        })
    }

    /// Exports pathing state. Call between ticks; may wait for builds in flight.
    pub fn snapshot(&mut self) -> NavSnapshot {
        NavSnapshot {
            path_state: self.inner.export_state(),
            handles: self
                .handles
                .iter()
                .map(|h| h.map(FieldId::to_bits))
                .collect(),
            free: self.free.clone(),
            pockets: self.pockets.export(),
        }
    }

    /// Rebuilds pathing from a snapshot over the map's unedited terrain classes.
    pub fn restore(
        terrain: &Heightfield,
        pool: Arc<Pool>,
        snapshot: &NavSnapshot,
    ) -> Result<Nav, SimError> {
        let grid = Self::base_grid(terrain, &pool)?;
        let inner = mc_path::Nav::import_state(
            grid,
            NavConfig::default(),
            Arc::new(PoolSpawner(pool)),
            &snapshot.path_state,
        )
        .map_err(path_error)?;
        Ok(Nav {
            inner,
            handles: snapshot
                .handles
                .iter()
                .map(|h| h.map(FieldId::from_bits))
                .collect(),
            free: snapshot.free.clone(),
            lots: vec![false; (terrain.size_cells().0 * terrain.size_cells().1) as usize],
            lots_w: terrain.size_cells().0,
            pockets: pockets::Pockets::import(&snapshot.pockets),
        })
    }

    /// Terrain classes from the heightfield alone: no structures, no props.
    fn base_grid(terrain: &Heightfield, pool: &Arc<Pool>) -> Result<NavGrid, SimError> {
        let (w, h) = terrain.size_cells();
        let water = terrain.water_level();
        let mut classes = vec![0u8; (w * h) as usize];
        // Row-parallel: every cell is classified from the heightfield alone.
        pool.parallel_chunks_mut(&mut classes, w as usize, |y, row| {
            for (x, class) in row.iter_mut().enumerate() {
                *class = cell_class(terrain, water, x as u32, y as u32);
            }
        });
        NavGrid::from_cells(w as i32, h as i32, &classes).map_err(path_error)
    }

    pub fn begin_tick(&mut self, tick: u32) -> Result<(), SimError> {
        self.inner.begin_tick(tick as u64);
        Ok(())
    }

    /// Requests the shared field to `goal`.
    pub fn request(
        &mut self,
        l: mc_data::MoveLayer,
        size_class: u8,
        goal: FxVec2,
        from: FxVec2,
    ) -> Result<Route, SimError> {
        let (l, s) = (layer(l), size(size_class));
        let Some(goal_cell) = self.nearest_cell(l, s, goal) else {
            return Ok(Route::Unreachable);
        };
        let goal = if Cell::from_pos(goal) == goal_cell {
            goal
        } else {
            goal_cell.center()
        };
        let from = self.nearest_cell(l, s, from).map_or(from, |c| {
            if Cell::from_pos(from) == c {
                from
            } else {
                c.center()
            }
        });
        let id = match self.inner.request(l, s, goal, &[from]) {
            Ok(id) => id,
            Err(PathError::GoalImpassable | PathError::OutOfMap | PathError::Impassable) => {
                return Ok(Route::Unreachable)
            }
            // A crowded late game can hold every field at once. Units wait for
            // a slot; ending the match over it is worse.
            Err(PathError::TooManyFields | PathError::TooManyAnchors) => return Ok(Route::Busy),
            Err(e) => return Err(path_error(e)),
        };
        let handle = match self.free.pop() {
            Some(h) => {
                self.handles[h as usize] = Some(id);
                h
            }
            None => {
                self.handles.push(Some(id));
                self.handles.len() as u32 - 1
            }
        };
        Ok(Route::Field(handle))
    }

    pub fn release(&mut self, handle: u32) {
        if let Some(id) = self.handles.get_mut(handle as usize).and_then(Option::take) {
            // The handle table is the only caller, so a stale id here is a bug worth hearing about.
            if let Err(e) = self.inner.release(id) {
                debug_assert!(false, "released a dead field: {e}");
            }
            self.free.push(handle);
        }
    }

    pub fn sample(&self, handle: u32, pos: FxVec2) -> Steer {
        let Some(Some(id)) = self.handles.get(handle as usize) else {
            return Steer::Unreachable;
        };
        match self.inner.sample(*id, pos) {
            Sample::Direction(d) => Steer::Direction(d),
            Sample::Arrived => Steer::Arrived,
            Sample::Pending => Steer::Pending,
            Sample::NeedsExtend => Steer::NeedsExtend,
            Sample::GoalBlocked => Steer::Rebuilt,
            Sample::Unreachable | Sample::Failed(_) => Steer::Unreachable,
        }
    }

    pub fn extend(&mut self, handle: u32, pos: FxVec2) -> Result<(), SimError> {
        let Some(Some(id)) = self.handles.get(handle as usize) else {
            return Ok(());
        };
        match self.inner.extend(*id, pos) {
            // Unresolved: the cell was already routed and still has no flow.
            // TooManyAnchors: the shared field cannot grow to reach this unit.
            // Either way the unit is stuck; killing the match over it is worse.
            Ok(())
            | Err(
                PathError::Impassable
                | PathError::OutOfMap
                | PathError::Unresolved
                | PathError::TooManyAnchors,
            ) => Ok(()),
            Err(e) => Err(path_error(e)),
        }
    }

    pub fn passable(&self, l: mc_data::MoveLayer, size_class: u8, pos: FxVec2) -> bool {
        if l == mc_data::MoveLayer::Air {
            return true;
        }
        let (l, s, c) = (layer(l), size(size_class), Cell::from_pos(pos));
        self.inner.is_passable(l, s, c) && !self.pockets.contains(l, s, c)
    }

    /// Closest standable point to `pos` for this hull. Air may stand anywhere.
    pub fn clear_segment(&self, l: mc_data::MoveLayer, size: u8, from: FxVec2, to: FxVec2) -> bool {
        if l == mc_data::MoveLayer::Air {
            return true;
        }
        let length = from.distance(to);
        if length > Fx::from_int(512) {
            return false;
        }
        let steps = (length / 4).ceil_int().max(1);
        // A step in the cell the last was in has its answer already, and so
        // has one in a stretch of sector found open all through.
        let mut last = None;
        let mut open: Option<CellRect> = None;
        fractions(steps).all(|t| {
            let at = from.lerp(to, t);
            let cell = Cell::from_pos(at);
            if last == Some(cell) || open.is_some_and(|r| r.contains(cell)) {
                return true;
            }
            last = Some(cell);
            if !self.passable(l, size, at) {
                return false;
            }
            open = self.inner.open_around(layer(l), cell).or(open);
            true
        })
    }

    pub fn nearest_passable(
        &self,
        l: mc_data::MoveLayer,
        size_class: u8,
        pos: FxVec2,
    ) -> Option<FxVec2> {
        if l == mc_data::MoveLayer::Air {
            return Some(pos);
        }
        self.nearest_cell(layer(l), size(size_class), pos).map(|c| {
            if Cell::from_pos(pos) == c {
                pos
            } else {
                c.center()
            }
        })
    }

    fn rect(min: (u32, u32), max_inclusive: (u32, u32)) -> CellRect {
        CellRect::new(
            Cell::new(min.0 as i32, min.1 as i32),
            Cell::new(max_inclusive.0 as i32 + 1, max_inclusive.1 as i32 + 1),
        )
    }

    /// Land terrain only; structure blockers are ignored.
    pub fn passable_terrain(&self, min: (u32, u32), max_inclusive: (u32, u32)) -> bool {
        self.inner
            .passable_terrain(Self::rect(min, max_inclusive), mc_path::MoveLayer::Land)
    }

    /// Floating foundations accept water but still reject cliffs.
    pub fn passable_floating_terrain(&self, min: (u32, u32), max: (u32, u32)) -> bool {
        self.inner
            .passable_terrain(Self::rect(min, max), mc_path::MoveLayer::Hover)
    }

    /// Water deep enough for ships over the whole rect: where a naval yard may stand.
    pub fn passable_naval_terrain(&self, min: (u32, u32), max: (u32, u32)) -> bool {
        self.inner
            .passable_terrain(Self::rect(min, max), mc_path::MoveLayer::Naval)
    }

    /// No structure or city blocker in these cells.
    pub fn no_blockers(&self, min: (u32, u32), max_inclusive: (u32, u32)) -> bool {
        self.inner.no_blockers(Self::rect(min, max_inclusive))
    }

    /// Marks (or clears) a structure's lot interior as taken.
    pub fn set_lot(&mut self, min: (u32, u32), max_inclusive: (u32, u32), taken: bool) {
        let w = self.lots_w;
        let h = self.lots.len() as u32 / w.max(1);
        for y in min.1..=max_inclusive.1.min(h.saturating_sub(1)) {
            for x in min.0..=max_inclusive.0.min(w.saturating_sub(1)) {
                self.lots[(y * w + x) as usize] = taken;
            }
        }
    }

    /// No structure's lot in these cells.
    pub fn lots_free(&self, min: (u32, u32), max_inclusive: (u32, u32)) -> bool {
        let w = self.lots_w;
        let h = self.lots.len() as u32 / w.max(1);
        (min.1..=max_inclusive.1.min(h.saturating_sub(1))).all(|y| {
            (min.0..=max_inclusive.0.min(w.saturating_sub(1)))
                .all(|x| !self.lots[(y * w + x) as usize])
        })
    }

    pub fn block_cells(&mut self, min: (u32, u32), max_inclusive: (u32, u32)) {
        if let Err(e) = self.inner.block_rect(Self::rect(min, max_inclusive)) {
            debug_assert!(false, "structure footprint rejected by the nav grid: {e}");
        }
    }

    pub fn unblock_cells(&mut self, min: (u32, u32), max_inclusive: (u32, u32)) {
        if let Err(e) = self.inner.unblock_rect(Self::rect(min, max_inclusive)) {
            debug_assert!(false, "structure footprint rejected by the nav grid: {e}");
        }
    }

    /// Works out the pockets round cells a structure just took or gave back.
    /// Only structures and city blocks coming down call this: the map's own
    /// city, blocked before the match, is full of shut courtyards no unit is in.
    pub fn reseal(&mut self, min: (u32, u32), max_inclusive: (u32, u32)) {
        self.pockets
            .update(&self.inner, Self::rect(min, max_inclusive));
    }

    /// Path cells inside pockets, summed over every layer and size class.
    pub fn pocket_cells(&self) -> usize {
        self.pockets.len()
    }

    /// The standable cell nearest `pos`: its own if it can stand there. A
    /// pocket's cells are left out, so this is the way out of one.
    fn nearest_cell(&self, l: mc_path::MoveLayer, s: SizeClass, pos: FxVec2) -> Option<Cell> {
        let found = self.inner.nearest_passable(l, s, pos, GOAL_SEARCH_CELLS)?;
        if !self.pockets.contains(l, s, found) {
            return Some(found);
        }
        let origin = Cell::from_pos(pos);
        let open = |c: Cell| self.inner.is_passable(l, s, c) && !self.pockets.contains(l, s, c);
        // Ring by ring, as `mc_path`'s own search: nearest centre, then lowest y, then x.
        let mut best: Option<(i64, Cell)> = None;
        for r in 1..=GOAL_SEARCH_CELLS {
            if let Some((d, _)) = best {
                let reach = i64::from(r * mc_path::CELL_SIZE - mc_path::CELL_SIZE / 2)
                    << (Fx::FRAC_BITS - 8);
                if reach * reach >= d {
                    break;
                }
            }
            let ring = (-r..=r)
                .flat_map(|i| {
                    [
                        Cell::new(origin.x + i, origin.y - r),
                        Cell::new(origin.x + i, origin.y + r),
                    ]
                })
                .chain(((1 - r)..r).flat_map(|i| {
                    [
                        Cell::new(origin.x - r, origin.y + i),
                        Cell::new(origin.x + r, origin.y + i),
                    ]
                }));
            for c in ring.filter(|&c| open(c)) {
                let d = c.center() - pos;
                let (dx, dy) = (d.x.0 >> 8, d.y.0 >> 8);
                let key = (dx * dx + dy * dy, c);
                if best.is_none_or(|b| (key.0, key.1.y, key.1.x) < (b.0, b.1.y, b.1.x)) {
                    best = Some(key);
                }
            }
        }
        best.map(|(_, c)| c)
    }

    pub fn stats(&self) -> mc_path::NavStats {
        self.inner.stats()
    }

    pub fn hash(&self, h: &mut StateHasher) {
        self.inner.hash(h);
        h.write_u32s(&self.free);
        self.pockets.hash(h);
        h.write_u64(self.handles.len() as u64);
    }
}

/// `Fx::ratio(i, steps)` for `i` in `0..=steps`, the ratio carried as quotient
/// and remainder instead of divided out at every step.
fn fractions(steps: i32) -> impl Iterator<Item = Fx> {
    let steps = steps.max(1) as i64;
    let one = 1i64 << Fx::FRAC_BITS;
    let (whole, part) = (one / steps, one % steps);
    (0..=steps).scan((0i64, 0i64), move |(t, rest), _| {
        let at = Fx(*t);
        *t += whole;
        *rest += part;
        if *rest >= steps {
            *t += 1;
            *rest -= steps;
        }
        Some(at)
    })
}

#[cfg(test)]
mod fraction_tests {
    use super::*;

    #[test]
    fn fractions_are_the_ratios() {
        for steps in 1..300 {
            let want: Vec<Fx> = (0..=steps as i64)
                .map(|i| Fx::ratio(i, steps as i64))
                .collect();
            assert_eq!(fractions(steps).collect::<Vec<_>>(), want, "{steps} steps");
        }
    }
}
