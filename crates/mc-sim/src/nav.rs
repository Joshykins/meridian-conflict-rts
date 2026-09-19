//! The simulation's view of `mc-path`: terrain classification, field handles
//! that fit in a table column, and background builds on the job pool.

use crate::SimError;
use mc_core::{Fx, FxVec2, StateHasher};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_path::{terrain, Cell, CellRect, FieldId, NavConfig, NavGrid, PathError, Sample, SizeClass, Spawner};
use std::sync::Arc;

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
    Unreachable,
}

struct PoolSpawner(Arc<Pool>);

impl Spawner for PoolSpawner {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        // Detached: the result comes back through mc-path's own slot.
        drop(self.0.spawn_background("flow-field", task));
    }
}

pub struct Nav {
    inner: mc_path::Nav,
    /// Table handles -> path fields. One entry per unit request, so releasing is symmetrical.
    handles: Vec<Option<FieldId>>,
    free: Vec<u32>,
}

fn layer(l: mc_data::MoveLayer) -> mc_path::MoveLayer {
    match l {
        mc_data::MoveLayer::Land => mc_path::MoveLayer::Land,
        mc_data::MoveLayer::Amphibious => mc_path::MoveLayer::Amphibious,
        mc_data::MoveLayer::Naval => mc_path::MoveLayer::Naval,
        mc_data::MoveLayer::Hover => mc_path::MoveLayer::Hover,
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
}

impl Nav {
    pub fn new(terrain: &Heightfield, pool: Arc<Pool>) -> Result<Nav, SimError> {
        let grid = Self::base_grid(terrain, &pool)?;
        let inner = mc_path::Nav::new(grid, NavConfig::default(), Arc::new(PoolSpawner(pool)));
        Ok(Nav { inner, handles: Vec::new(), free: Vec::new() })
    }

    /// Exports pathing state. Call between ticks; may wait for builds in flight.
    pub fn snapshot(&mut self) -> NavSnapshot {
        NavSnapshot {
            path_state: self.inner.export_state(),
            handles: self.handles.iter().map(|h| h.map(FieldId::to_bits)).collect(),
            free: self.free.clone(),
        }
    }

    /// Rebuilds pathing from a snapshot over the map's unedited terrain classes.
    pub fn restore(terrain: &Heightfield, pool: Arc<Pool>, snapshot: &NavSnapshot) -> Result<Nav, SimError> {
        let grid = Self::base_grid(terrain, &pool)?;
        let inner = mc_path::Nav::import_state(grid, NavConfig::default(), Arc::new(PoolSpawner(pool)), &snapshot.path_state).map_err(path_error)?;
        Ok(Nav { inner, handles: snapshot.handles.iter().map(|h| h.map(FieldId::from_bits)).collect(), free: snapshot.free.clone() })
    }

    /// Terrain classes from the heightfield alone: no structures, no props.
    fn base_grid(terrain: &Heightfield, pool: &Arc<Pool>) -> Result<NavGrid, SimError> {
        let (w, h) = terrain.size_cells();
        let water = terrain.water_level();
        let mut classes = vec![0u8; (w * h) as usize];
        // Row-parallel: every cell is classified from the heightfield alone.
        pool.parallel_chunks_mut(&mut classes, w as usize, |y, row| {
            for (x, class) in row.iter_mut().enumerate() {
                let (cx, cy) = (x as u32, y as u32);
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
                *class = c;
            }
        });
        NavGrid::from_cells(w as i32, h as i32, &classes).map_err(path_error)
    }

    pub fn begin_tick(&mut self, tick: u32) -> Result<(), SimError> {
        self.inner.begin_tick(tick as u64);
        Ok(())
    }

    /// Requests the shared field to `goal`. `Ok(None)`: nowhere near the goal
    /// can be stood on, so the order cannot be carried out.
    pub fn request(&mut self, l: mc_data::MoveLayer, size_class: u8, goal: FxVec2, from: FxVec2) -> Result<Option<u32>, SimError> {
        let (l, s) = (layer(l), size(size_class));
        let Some(goal_cell) = self.inner.nearest_passable(l, s, goal, GOAL_SEARCH_CELLS) else { return Ok(None) };
        let goal = if Cell::from_pos(goal) == goal_cell { goal } else { goal_cell.center() };
        let from = self.inner.nearest_passable(l, s, from, GOAL_SEARCH_CELLS).map_or(from, |c| if Cell::from_pos(from) == c { from } else { c.center() });
        let id = match self.inner.request(l, s, goal, &[from]) {
            Ok(id) => id,
            Err(PathError::GoalImpassable | PathError::OutOfMap | PathError::Impassable) => return Ok(None),
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
        Ok(Some(handle))
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
        let Some(Some(id)) = self.handles.get(handle as usize) else { return Steer::Unreachable };
        match self.inner.sample(*id, pos) {
            Sample::Direction(d) => Steer::Direction(d),
            Sample::Arrived => Steer::Arrived,
            Sample::Pending => Steer::Pending,
            Sample::NeedsExtend => Steer::NeedsExtend,
            Sample::Unreachable | Sample::Failed(_) => Steer::Unreachable,
        }
    }

    pub fn extend(&mut self, handle: u32, pos: FxVec2) -> Result<(), SimError> {
        let Some(Some(id)) = self.handles.get(handle as usize) else { return Ok(()) };
        match self.inner.extend(*id, pos) {
            Ok(()) | Err(PathError::Impassable | PathError::OutOfMap) => Ok(()),
            Err(e) => Err(path_error(e)),
        }
    }

    pub fn passable(&self, l: mc_data::MoveLayer, size_class: u8, pos: FxVec2) -> bool {
        self.inner.is_passable(layer(l), size(size_class), Cell::from_pos(pos))
    }

    fn rect(min: (u32, u32), max_inclusive: (u32, u32)) -> CellRect {
        CellRect::new(Cell::new(min.0 as i32, min.1 as i32), Cell::new(max_inclusive.0 as i32 + 1, max_inclusive.1 as i32 + 1))
    }

    /// True when a land structure may occupy these cells.
    pub fn can_place(&self, min: (u32, u32), max_inclusive: (u32, u32)) -> bool {
        self.inner.can_place(Self::rect(min, max_inclusive), mc_path::MoveLayer::Land)
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

    pub fn stats(&self) -> mc_path::NavStats {
        self.inner.stats()
    }

    pub fn hash(&self, h: &mut StateHasher) {
        self.inner.hash(h);
        h.write_u32s(&self.free);
        h.write_u64(self.handles.len() as u64);
    }
}
