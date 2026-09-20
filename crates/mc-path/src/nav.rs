//! `Nav`: the tick-side face of the pathfinder and its background contract.
//!
//! What the sim observes through `sample` on a given tick must be identical on
//! every machine, whatever the thread count or timing. Three rules give that:
//!
//! 1. A build is scheduled with a snapshot of the grid (copy-on-write, so it
//!    is a few refcounts) and of the field's previous data. Its output is a
//!    pure function of that input; where and when it runs is irrelevant.
//! 2. Its adoption tick is fixed when it is scheduled, from the request alone.
//!    `begin_tick` adopts on exactly that tick and blocks if the task is late.
//! 3. A field runs one build at a time. Anything that happens while a build is
//!    in flight (blockers changing, `extend` calls) is recorded on the field
//!    and acted on at adoption: the in-flight result is adopted as computed,
//!    against its old snapshot, and if the recorded changes overlap its tiles
//!    a repair is scheduled right then, with its own ready tick. Nothing is
//!    cancelled, so steady construction cannot starve a field, and nothing
//!    inspects a result before its tick.
//!
//! Until a repair lands `sample` keeps serving the old tiles, but checks the
//! step it hands out against the live grid and answers `Pending` rather than
//! steer a unit into a cell that has just been built on.

use crate::field::{
    build, BuildInput, BuildStats, FieldData, CODE_GOAL, CODE_MASK, CODE_UNKNOWN, FLAG_ESCAPE,
    FLAG_LOS,
};
use crate::graph::{GraphCache, DIRS};
use crate::grid::{NavGrid, Touched, SECTOR};
use crate::{Cell, CellRect, MoveLayer, PathError, SizeClass, SECTOR_CELLS};
use mc_core::{Fx, FxVec2, StateHasher};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};

mod snapshot;

/// Runs background builds. Implement it over the job system; the crate itself
/// only ships the two trivial ones.
pub trait Spawner: Send + Sync {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>);
}

/// Runs the task on the calling thread before `spawn` returns. For tests and tools.
pub struct InlineSpawner;

impl Spawner for InlineSpawner {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        task();
    }
}

/// One detached OS thread per task.
pub struct ThreadSpawner;

impl Spawner for ThreadSpawner {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        std::thread::spawn(task);
    }
}

#[derive(Clone, Debug)]
pub struct NavConfig {
    /// Fields alive at once, counting released ones still cached.
    pub max_fields: usize,
    /// Corridor anchors (distinct origin cells) per field.
    pub max_anchors: usize,
    pub max_tiles_per_field: usize,
    /// Tiles over all fields, about 1.6 KB each.
    pub max_total_tiles: usize,
    pub max_search_nodes: u32,
    /// Ticks from scheduling a build to adopting it: `base_latency` plus one
    /// per `sectors_per_latency_tick` sectors between the goal and the
    /// farthest anchor being routed.
    pub base_latency: u32,
    pub sectors_per_latency_tick: u32,
    /// Sectors added around the abstract route on every side.
    pub corridor_margin: i32,
    /// Cells around the goal that get a line-of-sight test.
    pub los_radius: i32,
}

impl Default for NavConfig {
    fn default() -> Self {
        NavConfig {
            max_fields: 256,
            max_anchors: 64,
            max_tiles_per_field: 4096,
            max_total_tiles: 1 << 16,
            max_search_nodes: 1 << 20,
            base_latency: 2,
            sectors_per_latency_tick: 32,
            corridor_margin: 1,
            los_radius: 20,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FieldId {
    index: u32,
    generation: u32,
}

impl FieldId {
    /// Stable packing for the sim's tables and snapshots: slot index in the high half, generation in the low.
    #[inline]
    pub fn to_bits(self) -> u64 {
        (self.index as u64) << 32 | self.generation as u64
    }

    #[inline]
    pub fn from_bits(bits: u64) -> FieldId {
        FieldId {
            index: (bits >> 32) as u32,
            generation: bits as u32,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sample {
    /// Unit vector to steer along.
    Direction(FxVec2),
    /// In the goal cell. The final approach is the sim's business.
    Arrived,
    /// No usable data on this tick: the first build is not due yet, or the next
    /// step was built over and the repair is not due yet. Hold position.
    Pending,
    /// Outside the built corridor. Call `extend` and keep sampling.
    NeedsExtend,
    Unreachable,
    /// The field hit a limit; the error is the same on every machine.
    Failed(PathError),
}

/// Diagnostics. Everything except `late_joins` and `graphs_built` is deterministic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavStats {
    pub builds_scheduled: u64,
    pub builds_adopted: u64,
    pub repairs_scheduled: u64,
    pub extends_scheduled: u64,
    pub tiles_built: u64,
    pub tiles_reused: u64,
    pub search_nodes: u64,
    pub fields_evicted: u64,
    pub live_fields: usize,
    pub total_tiles: usize,
    /// Times `begin_tick` had to wait for a build. Depends on wall time.
    pub late_joins: u64,
    /// Sector graphs derived from cell data. Depends on thread timing.
    pub graphs_built: u64,
}

struct ResultSlot {
    value: Mutex<Option<Result<FieldData, PathError>>>,
    ready: Condvar,
}

/// Fills the slot with an error if the task unwinds, so `begin_tick` cannot hang.
struct SlotGuard(Arc<ResultSlot>);

impl Drop for SlotGuard {
    fn drop(&mut self) {
        let mut v = self.0.value.lock().unwrap_or_else(|e| e.into_inner());
        if v.is_none() {
            *v = Some(Err(PathError::BuildPanicked));
        }
        self.0.ready.notify_all();
    }
}

/// (sector, cell index). Sorted, so anchors group by sector.
type Anchor = (u32, u32);

struct Pending {
    ready_tick: u64,
    slot: Arc<ResultSlot>,
    /// Anchors this build routes; `extend` from the same sectors waits for it.
    routing: Vec<Anchor>,
    /// Sectors (of this field's layer) changed while the build was in flight.
    dirty: Vec<u32>,
    dirty_all: bool,
    unblocked: bool,
}

const MAX_DIRTY: usize = 256;

struct Field {
    layer: MoveLayer,
    size: SizeClass,
    goal: Cell,
    key: (u8, u8, u32),
    refcount: u32,
    released_tick: u64,
    anchors: Vec<Anchor>,
    data: Option<Arc<FieldData>>,
    error: Option<PathError>,
    pending: Option<Pending>,
    queued_anchors: Vec<Anchor>,
    queued_repair: bool,
}

#[derive(Default)]
struct Slot {
    generation: u32,
    field: Option<Field>,
}

pub struct Nav {
    grid: NavGrid,
    cfg: NavConfig,
    spawner: Arc<dyn Spawner>,
    cache: Arc<GraphCache>,
    slots: Vec<Slot>,
    free: Vec<u32>,
    by_key: BTreeMap<(u8, u8, u32), u32>,
    tick: u64,
    total_tiles: usize,
    stats: NavStats,
}

const DIAG: Fx = Fx(46_341);
const DIR_VECS: [FxVec2; 8] = [
    FxVec2::new(Fx::ONE, Fx::ZERO),
    FxVec2::new(DIAG, DIAG),
    FxVec2::new(Fx::ZERO, Fx::ONE),
    FxVec2::new(Fx(-DIAG.0), DIAG),
    FxVec2::new(Fx(-Fx::ONE.0), Fx::ZERO),
    FxVec2::new(Fx(-DIAG.0), Fx(-DIAG.0)),
    FxVec2::new(Fx::ZERO, Fx(-Fx::ONE.0)),
    FxVec2::new(DIAG, Fx(-DIAG.0)),
];

fn insert_sorted(v: &mut Vec<Anchor>, a: Anchor) {
    if let Err(i) = v.binary_search(&a) {
        v.insert(i, a);
    }
}

/// Whether `sector` lies in the bounding box of the field's tiles.
fn in_bounds(grid: &NavGrid, data: &FieldData, sector: u32) -> bool {
    let (sx, sy) = grid.sector_xy(sector);
    let (x0, y0, x1, y1) = data.bounds;
    (x0..=x1).contains(&sx) && (y0..=y1).contains(&sy)
}

fn has_sector(v: &[Anchor], sector: u32) -> bool {
    let i = v.partition_point(|a| a.0 < sector);
    v.get(i).is_some_and(|a| a.0 == sector)
}

impl Nav {
    pub fn new(grid: NavGrid, cfg: NavConfig, spawner: Arc<dyn Spawner>) -> Nav {
        Nav {
            grid,
            cfg,
            spawner,
            cache: Arc::new(GraphCache::new()),
            slots: Vec::new(),
            free: Vec::new(),
            by_key: BTreeMap::new(),
            tick: 0,
            total_tiles: 0,
            stats: NavStats::default(),
        }
    }

    #[inline]
    pub fn grid(&self) -> &NavGrid {
        &self.grid
    }

    #[inline]
    pub fn config(&self) -> &NavConfig {
        &self.cfg
    }

    pub fn stats(&self) -> NavStats {
        let mut s = self.stats;
        s.live_fields = self.by_key.len();
        s.total_tiles = self.total_tiles;
        s.graphs_built = self.cache.built.load(Ordering::Relaxed);
        s
    }

    /// Work done by the build `id` currently serves.
    pub fn field_stats(&self, id: FieldId) -> Option<BuildStats> {
        self.field(id).ok()?.data.as_ref().map(|d| d.stats)
    }

    /// Sectors `id` currently has tiles for.
    pub fn field_tiles(&self, id: FieldId) -> usize {
        self.field(id)
            .ok()
            .and_then(|f| f.data.as_ref())
            .map_or(0, |d| d.tiles.len())
    }

    /// Tick on which the build in flight for `id` will be adopted, if any.
    pub fn ready_tick(&self, id: FieldId) -> Option<u64> {
        self.field(id).ok()?.pending.as_ref().map(|p| p.ready_tick)
    }

    #[inline]
    pub fn is_passable(&self, layer: MoveLayer, size: SizeClass, cell: Cell) -> bool {
        self.grid.is_passable(layer, size, cell)
    }

    #[inline]
    pub fn can_place(&self, rect: CellRect, layer: MoveLayer) -> bool {
        self.grid.can_place(rect, layer)
    }

    #[inline]
    pub fn passable_terrain(&self, rect: CellRect, layer: MoveLayer) -> bool {
        self.grid.passable_terrain(rect, layer)
    }

    #[inline]
    pub fn no_blockers(&self, rect: CellRect) -> bool {
        self.grid.no_blockers(rect)
    }

    #[inline]
    pub fn nearest_passable(
        &self,
        layer: MoveLayer,
        size: SizeClass,
        pos: FxVec2,
        max_radius_cells: i32,
    ) -> Option<Cell> {
        self.grid
            .nearest_passable(layer, size, pos, max_radius_cells)
    }

    fn field(&self, id: FieldId) -> Result<&Field, PathError> {
        let slot = self
            .slots
            .get(id.index as usize)
            .ok_or(PathError::InvalidField)?;
        slot.field
            .as_ref()
            .filter(|_| slot.generation == id.generation)
            .ok_or(PathError::InvalidField)
    }

    fn field_mut(&mut self, id: FieldId) -> Result<&mut Field, PathError> {
        let slot = self
            .slots
            .get_mut(id.index as usize)
            .ok_or(PathError::InvalidField)?;
        let generation = slot.generation;
        slot.field
            .as_mut()
            .filter(|_| generation == id.generation)
            .ok_or(PathError::InvalidField)
    }

    /// Start of a sim tick: adopts every build due at or before `tick`, in slot
    /// order, waiting for any that has not finished.
    pub fn begin_tick(&mut self, tick: u64) {
        self.tick = tick;
        for index in 0..self.slots.len() {
            let Some(field) = self.slots[index].field.as_mut() else {
                continue;
            };
            if field.pending.as_ref().is_none_or(|p| p.ready_tick > tick) {
                continue;
            }
            let pending = field.pending.take().unwrap();
            let result = {
                let mut v = pending.slot.value.lock().unwrap_or_else(|e| e.into_inner());
                if v.is_none() {
                    self.stats.late_joins += 1;
                }
                while v.is_none() {
                    v = pending
                        .slot
                        .ready
                        .wait(v)
                        .unwrap_or_else(|e| e.into_inner());
                }
                v.take().unwrap()
            };
            self.stats.builds_adopted += 1;
            self.total_tiles -= field.data.as_ref().map_or(0, |d| d.tiles.len());
            match result {
                Ok(data) => {
                    self.stats.tiles_built += data.stats.tiles_built as u64;
                    self.stats.tiles_reused += data.stats.tiles_reused as u64;
                    self.stats.search_nodes += data.stats.search_nodes as u64;
                    self.total_tiles += data.tiles.len();
                    let stale = pending.dirty_all
                        || (pending.unblocked && !data.unreachable.is_empty())
                        || pending.dirty.iter().any(|&s| {
                            data.tile(s).is_some()
                                || (pending.unblocked && in_bounds(&self.grid, &data, s))
                        });
                    field.queued_repair |= stale;
                    field.data = Some(Arc::new(data));
                }
                Err(e) => {
                    field.data = None;
                    field.error = Some(e);
                }
            }
            while self.total_tiles > self.cfg.max_total_tiles {
                if !self.evict_one(index as u32) {
                    let field = self.slots[index].field.as_mut().unwrap();
                    self.total_tiles -= field.data.take().map_or(0, |d| d.tiles.len());
                    field.error = Some(PathError::TileBudget);
                }
            }
            let field = self.slots[index].field.as_mut().unwrap();
            if field.error.is_some() {
                field.queued_anchors.clear();
                field.queued_repair = false;
                if field.refcount == 0 {
                    self.remove(index as u32);
                }
                continue;
            }
            if field.queued_repair || !field.queued_anchors.is_empty() {
                let new = std::mem::take(&mut field.queued_anchors);
                let full = std::mem::take(&mut field.queued_repair);
                for &a in &new {
                    insert_sorted(&mut field.anchors, a);
                }
                self.schedule(index as u32, new, full);
            }
        }
    }

    /// Flow field toward `goal` for the units at `from`. Requests for the same
    /// (layer, size, goal cell) share one refcounted field; positions in
    /// sectors it does not cover yet extend it. Cheap: the work is scheduled
    /// on the spawner and adopted `latency` ticks from now.
    pub fn request(
        &mut self,
        layer: MoveLayer,
        size: SizeClass,
        goal: FxVec2,
        from: &[FxVec2],
    ) -> Result<FieldId, PathError> {
        let goal_cell = self.grid.cell_of(goal).ok_or(PathError::OutOfMap)?;
        if !self.grid.is_passable(layer, size, goal_cell) {
            return Err(PathError::GoalImpassable);
        }
        // One anchor per sector: the lowest passable cell. Units elsewhere in
        // the sector find out through `NeedsExtend` if they are walled off from it.
        let mut anchors: Vec<Anchor> = from
            .iter()
            .filter_map(|&p| self.grid.cell_of(p))
            .filter(|&c| self.grid.is_passable(layer, size, c))
            .map(|c| (self.grid.sector_of(c), self.grid.cell_index(c)))
            .collect();
        anchors.sort_unstable();
        anchors.dedup_by_key(|a| a.0);
        let key = (layer as u8, size.index(), self.grid.cell_index(goal_cell));

        if let Some(&index) = self.by_key.get(&key) {
            let field = self.slots[index as usize].field.as_ref().unwrap();
            let new: Vec<Anchor> = anchors
                .into_iter()
                .filter(|a| {
                    !has_sector(&field.anchors, a.0) && !has_sector(&field.queued_anchors, a.0)
                })
                .filter(|a| field.data.as_ref().is_none_or(|d| d.tile(a.0).is_none()))
                .collect();
            if field.error.is_none()
                && field.anchors.len() + field.queued_anchors.len() + new.len()
                    > self.cfg.max_anchors
            {
                return Err(PathError::TooManyAnchors);
            }
            let id = FieldId {
                index,
                generation: self.slots[index as usize].generation,
            };
            let field = self.slots[index as usize].field.as_mut().unwrap();
            field.refcount += 1;
            if field.error.is_none() && !new.is_empty() {
                self.add_anchors(index, new);
            }
            return Ok(id);
        }

        if anchors.len() > self.cfg.max_anchors {
            return Err(PathError::TooManyAnchors);
        }
        let index = match self.free.pop() {
            Some(i) => i,
            None if self.slots.len() < self.cfg.max_fields => {
                self.slots.push(Slot::default());
                self.slots.len() as u32 - 1
            }
            None => {
                if !self.evict_one(u32::MAX) {
                    return Err(PathError::TooManyFields);
                }
                self.free.pop().unwrap()
            }
        };
        self.slots[index as usize].field = Some(Field {
            layer,
            size,
            goal: goal_cell,
            key,
            refcount: 1,
            released_tick: 0,
            anchors: anchors.clone(),
            data: None,
            error: None,
            pending: None,
            queued_anchors: Vec::new(),
            queued_repair: false,
        });
        self.by_key.insert(key, index);
        self.schedule(index, anchors, true);
        Ok(FieldId {
            index,
            generation: self.slots[index as usize].generation,
        })
    }

    /// Grows the corridor to cover `pos` after a `NeedsExtend`. Idempotent
    /// while the sector is already being routed, so every unit of a group may
    /// call it every tick.
    pub fn extend(&mut self, id: FieldId, pos: FxVec2) -> Result<(), PathError> {
        let cell = self.grid.cell_of(pos).ok_or(PathError::OutOfMap)?;
        let max_anchors = self.cfg.max_anchors;
        let anchor = (self.grid.sector_of(cell), self.grid.cell_index(cell));
        let passable = |f: &Field| self.grid.is_passable(f.layer, f.size, cell);
        let field = self.field(id)?;
        if let Some(e) = field.error {
            return Err(e);
        }
        if !passable(field) {
            return Err(PathError::Impassable);
        }
        let in_flight = field
            .pending
            .as_ref()
            .is_some_and(|p| has_sector(&p.routing, anchor.0));
        if in_flight || has_sector(&field.queued_anchors, anchor.0) {
            return Ok(());
        }
        if field.anchors.binary_search(&anchor).is_ok() {
            // Already routed. If the cell still has no data, asking again cannot help.
            let unknown = field
                .data
                .as_deref()
                .and_then(|d| self.code_at(d, cell))
                .is_some_and(|c| c & CODE_MASK == CODE_UNKNOWN);
            return if unknown && field.pending.is_none() {
                Err(PathError::Unresolved)
            } else {
                Ok(())
            };
        }
        if field.anchors.len() + field.queued_anchors.len() >= max_anchors {
            return Err(PathError::TooManyAnchors);
        }
        self.add_anchors(id.index, vec![anchor]);
        Ok(())
    }

    fn add_anchors(&mut self, index: u32, new: Vec<Anchor>) {
        let field = self.slots[index as usize].field.as_mut().unwrap();
        if field.pending.is_some() {
            for a in new {
                insert_sorted(&mut field.queued_anchors, a);
            }
        } else {
            for &a in &new {
                insert_sorted(&mut field.anchors, a);
            }
            self.stats.extends_scheduled += 1;
            self.schedule(index, new, false);
        }
    }

    /// Drops one reference. An unreferenced field stays cached, and hashed,
    /// until it is evicted (oldest release first, then lowest slot) to make room.
    pub fn release(&mut self, id: FieldId) -> Result<(), PathError> {
        let tick = self.tick;
        let field = self.field_mut(id)?;
        if field.refcount == 0 {
            return Err(PathError::InvalidField);
        }
        field.refcount -= 1;
        if field.refcount == 0 {
            field.released_tick = tick;
            if field.error.is_some() {
                self.remove(id.index);
            }
        }
        Ok(())
    }

    fn remove(&mut self, index: u32) {
        let slot = &mut self.slots[index as usize];
        if let Some(field) = slot.field.take() {
            slot.generation += 1;
            self.total_tiles -= field.data.as_ref().map_or(0, |d| d.tiles.len());
            self.by_key.remove(&field.key);
            self.free.push(index);
        }
    }

    fn evict_one(&mut self, keep: u32) -> bool {
        let victim = self
            .slots
            .iter()
            .enumerate()
            .filter(|(i, s)| *i as u32 != keep && s.field.as_ref().is_some_and(|f| f.refcount == 0))
            .min_by_key(|(i, s)| (s.field.as_ref().unwrap().released_tick, *i))
            .map(|(i, _)| i as u32);
        if let Some(v) = victim {
            self.remove(v);
            self.stats.fields_evicted += 1;
        }
        victim.is_some()
    }

    /// Hands a build to the spawner. `new` are the anchors to route when the
    /// build is incremental; a `full` build routes every anchor again.
    fn schedule(&mut self, index: u32, new: Vec<Anchor>, full: bool) {
        let field = self.slots[index as usize].field.as_mut().unwrap();
        let routing = if full { field.anchors.clone() } else { new };
        let (gx, gy) = (field.goal.x / SECTOR_CELLS, field.goal.y / SECTOR_CELLS);
        let reach = routing.iter().map(|a| {
            let (sx, sy) = self.grid.sector_xy(a.0);
            (sx - gx).abs().max((sy - gy).abs()) as u32
        });
        let latency = self.cfg.base_latency
            + reach.max().unwrap_or(0) / self.cfg.sectors_per_latency_tick.max(1);
        let input = BuildInput {
            grid: self.grid.clone(),
            cache: self.cache.clone(),
            layer: field.layer,
            size: field.size,
            goal: field.goal,
            anchors: routing
                .iter()
                .map(|a| self.grid.cell_from_index(a.1))
                .collect(),
            full,
            prev: field.data.clone(),
            margin: self.cfg.corridor_margin,
            los_radius: self.cfg.los_radius,
            max_tiles: self.cfg.max_tiles_per_field,
            max_nodes: self.cfg.max_search_nodes,
        };
        let slot = Arc::new(ResultSlot {
            value: Mutex::new(None),
            ready: Condvar::new(),
        });
        field.pending = Some(Pending {
            ready_tick: self.tick + latency as u64,
            slot: slot.clone(),
            routing,
            dirty: Vec::new(),
            dirty_all: false,
            unblocked: false,
        });
        self.stats.builds_scheduled += 1;
        self.spawner.spawn(Box::new(move || {
            let guard = SlotGuard(slot);
            let out = build(&input);
            *guard.0.value.lock().unwrap_or_else(|e| e.into_inner()) = Some(out);
        }));
    }

    /// Structure placed. Takes effect on the grid now; see the module docs for
    /// what happens to fields.
    pub fn block_rect(&mut self, rect: CellRect) -> Result<(), PathError> {
        let touched = self.grid.block_rect(rect)?;
        self.grid_changed(&touched, false);
        Ok(())
    }

    /// Structure removed. Besides fields with tiles on the spot this re-routes
    /// fields whose tile bounding box contains it (the opening may be a
    /// shortcut) and fields holding cut-off anchors (it may be anywhere).
    pub fn unblock_rect(&mut self, rect: CellRect) -> Result<(), PathError> {
        let touched = self.grid.unblock_rect(rect)?;
        self.grid_changed(&touched, true);
        Ok(())
    }

    fn grid_changed(&mut self, touched: &Touched, unblock: bool) {
        for index in 0..self.slots.len() {
            let Some(field) = self.slots[index].field.as_mut() else {
                continue;
            };
            let sectors = &touched.layers[field.layer.index()];
            if sectors.is_empty() || field.error.is_some() {
                continue;
            }
            if let Some(p) = field.pending.as_mut() {
                p.unblocked |= unblock;
                p.dirty.extend_from_slice(sectors);
                if p.dirty.len() > MAX_DIRTY {
                    p.dirty.clear();
                    p.dirty_all = true;
                }
            } else if let Some(data) = &field.data {
                let grid = &self.grid;
                if (unblock && !data.unreachable.is_empty())
                    || sectors
                        .iter()
                        .any(|&s| data.tile(s).is_some() || (unblock && in_bounds(grid, data, s)))
                {
                    self.stats.repairs_scheduled += 1;
                    self.schedule(index as u32, Vec::new(), true);
                }
            }
        }
    }

    fn code_at(&self, data: &FieldData, cell: Cell) -> Option<u8> {
        if !self.grid.contains(cell) {
            return None;
        }
        let tile = data.tile(self.grid.sector_of(cell))?;
        Some(
            tile.dirs[(cell.y % SECTOR_CELLS) as usize * SECTOR + (cell.x % SECTOR_CELLS) as usize],
        )
    }

    /// Steering for a unit at `pos`. Read-only and cheap: a few binary searches.
    pub fn sample(&self, id: FieldId, pos: FxVec2) -> Sample {
        let field = match self.field(id) {
            Ok(f) => f,
            Err(e) => return Sample::Failed(e),
        };
        if let Some(e) = field.error {
            return Sample::Failed(e);
        }
        let Some(data) = field.data.as_deref() else {
            return Sample::Pending;
        };
        let Some(cell) = self.grid.cell_of(pos) else {
            return Sample::Unreachable;
        };
        if cell == field.goal {
            return Sample::Arrived;
        }
        let Some(code) = self.code_at(data, cell) else {
            return Sample::NeedsExtend;
        };
        let dir = match code & CODE_MASK {
            CODE_GOAL => return Sample::Arrived,
            CODE_UNKNOWN => {
                let anchor = (self.grid.sector_of(cell), self.grid.cell_index(cell));
                return if field.anchors.binary_search(&anchor).is_ok() && field.pending.is_none() {
                    Sample::Unreachable
                } else {
                    Sample::NeedsExtend
                };
            }
            d if d < 8 => d as usize,
            _ => return Sample::Unreachable,
        };
        if code & FLAG_ESCAPE != 0 {
            return Sample::Direction(DIR_VECS[dir]);
        }
        let los = code & FLAG_LOS != 0;
        let to_goal = field.goal.center() - pos;
        let step = if los {
            (to_goal.angle().0.wrapping_add(0x1000) >> 13) as usize & 7
        } else {
            dir
        };
        let next = Cell::new(cell.x + DIRS[step].0, cell.y + DIRS[step].1);
        let passable = |c: Cell| self.grid.is_passable(field.layer, field.size, c);
        if passable(cell) && !passable(next) && (field.pending.is_some() || field.queued_repair) {
            return Sample::Pending;
        }
        if los {
            return Sample::Direction(to_goal.normalize());
        }
        Sample::Direction(self.blend(data, cell, pos).unwrap_or(DIR_VECS[dir]))
    }

    /// Bilinear mix of the four nearest cells' directions, which rounds off the
    /// 45-degree kinks. Only in the open: any neighbour without a plain
    /// direction, or a mix that nearly cancels, falls back to the cell's own.
    /// Safe for steps up to one cell (8 m) per tick; faster movers sub-step.
    fn blend(&self, data: &FieldData, cell: Cell, pos: FxVec2) -> Option<FxVec2> {
        let off = pos - cell.center();
        let (ox, oy) = (
            if off.x.0 >= 0 { 1 } else { -1 },
            if off.y.0 >= 0 { 1 } else { -1 },
        );
        let (wx, wy) = (off.x.abs() / 8, off.y.abs() / 8);
        let mut sum = FxVec2::ZERO;
        for (dx, dy, w) in [
            (0, 0, (Fx::ONE - wx) * (Fx::ONE - wy)),
            (ox, 0, wx * (Fx::ONE - wy)),
            (0, oy, (Fx::ONE - wx) * wy),
            (ox, oy, wx * wy),
        ] {
            let code = self.code_at(data, Cell::new(cell.x + dx, cell.y + dy))?;
            if code & CODE_MASK >= 8 || code & FLAG_ESCAPE != 0 {
                return None;
            }
            sum += DIR_VECS[(code & CODE_MASK) as usize] * w;
        }
        if sum.length_sq() < Fx::ratio(1, 4) {
            return None;
        }
        // The mix can aim between the four cells at one none of them leads to
        // (a rock diagonally ahead). Probe one cell length along it; the grid
        // direction is safe by construction, so fall back to it on any doubt.
        let v = sum.normalize();
        for metres in [2, 4, 6, 8] {
            let probe = Cell::from_pos(pos + v * Fx::from_int(metres));
            if probe != cell {
                let code = self.code_at(data, probe)?;
                if code & CODE_MASK > CODE_GOAL || code & FLAG_ESCAPE != 0 {
                    return None;
                }
            }
        }
        Some(v)
    }

    /// Logical state only: blockers and every field's identity, refcount,
    /// anchors and schedule. Tiles and graph caches are derived and left out.
    /// (Named like the sim's other `hash` methods; it is not `std::hash::Hash`.)
    #[allow(clippy::should_implement_trait)]
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.tick);
        h.write_u64(self.grid.blocker_hash());
        h.write_u64(self.grid.blocked_cell_count());
        h.write_u32(self.grid.version());
        h.write_u64(self.by_key.len() as u64);
        for (index, slot) in self.slots.iter().enumerate() {
            let Some(f) = &slot.field else { continue };
            h.write_u32(index as u32);
            h.write_u32(slot.generation);
            h.write_u32(f.key.0 as u32 | (f.key.1 as u32) << 8);
            h.write_u32(f.key.2);
            h.write_u32(f.refcount);
            h.write_u64(f.released_tick);
            for list in [&f.anchors, &f.queued_anchors] {
                h.write_u64(list.len() as u64);
                for a in list {
                    h.write_u32(a.1);
                }
            }
            h.write_u32(
                f.data.is_some() as u32
                    | (f.queued_repair as u32) << 1
                    | (f.error.map_or(0, |e| e.code() as u32 + 1)) << 8,
            );
            h.write_u64(f.pending.as_ref().map_or(u64::MAX, |p| p.ready_tick));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::*;

    fn nav(w: i32, cfg: NavConfig) -> Nav {
        Nav::new(
            NavGrid::from_fn(w, w, |_, _| LAND).unwrap(),
            cfg,
            Arc::new(InlineSpawner),
        )
    }

    fn at(x: i32, y: i32) -> FxVec2 {
        Cell::new(x, y).center()
    }

    const L: MoveLayer = MoveLayer::Land;
    const S: SizeClass = SizeClass::SMALL;

    #[test]
    fn results_appear_on_the_ready_tick_even_when_built_early() {
        let mut n = nav(256, NavConfig::default());
        n.begin_tick(10);
        let id = n.request(L, S, at(200, 200), &[at(10, 10)]).unwrap();
        assert_eq!(n.ready_tick(id), Some(12));
        assert_eq!(n.sample(id, at(10, 10)), Sample::Pending);
        n.begin_tick(11);
        assert_eq!(n.sample(id, at(10, 10)), Sample::Pending);
        n.begin_tick(12);
        assert!(matches!(n.sample(id, at(10, 10)), Sample::Direction(_)));
        assert_eq!(n.sample(id, at(200, 200)), Sample::Arrived);
    }

    #[test]
    fn latency_grows_with_distance_only() {
        let cfg = NavConfig {
            sectors_per_latency_tick: 4,
            ..NavConfig::default()
        };
        let mut n = nav(1024, cfg);
        n.begin_tick(0);
        let id = n
            .request(L, S, at(1000, 10), &[at(10, 10), at(500, 10)])
            .unwrap();
        assert_eq!(n.ready_tick(id), Some(2 + 31 / 4));
    }

    #[test]
    fn same_goal_cell_shares_one_field() {
        let mut n = nav(256, NavConfig::default());
        n.begin_tick(0);
        let a = n.request(L, S, at(200, 200), &[at(10, 10)]).unwrap();
        let b = n
            .request(L, S, at(200, 200) + FxVec2::from_ints(1, 1), &[at(12, 10)])
            .unwrap();
        let c = n.request(L, SizeClass::MEDIUM, at(200, 200), &[]).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(n.stats().builds_scheduled, 2);
        n.release(a).unwrap();
        n.release(b).unwrap();
        assert_eq!(n.release(a), Err(PathError::InvalidField));
        // Still cached: a new request revives it without a build.
        let d = n.request(L, S, at(200, 200), &[at(10, 10)]).unwrap();
        assert_eq!(a, d);
        assert_eq!(n.stats().builds_scheduled, 2);
    }

    #[test]
    fn limits_are_errors() {
        let cfg = NavConfig {
            max_fields: 2,
            max_anchors: 2,
            ..NavConfig::default()
        };
        let mut n = nav(256, cfg);
        n.begin_tick(0);
        assert_eq!(n.request(L, S, at(300, 0), &[]), Err(PathError::OutOfMap));
        assert_eq!(
            n.request(MoveLayer::Naval, S, at(5, 5), &[]),
            Err(PathError::GoalImpassable)
        );
        assert_eq!(
            n.request(L, S, at(5, 5), &[at(40, 40), at(80, 80), at(120, 120)]),
            Err(PathError::TooManyAnchors)
        );
        let a = n.request(L, S, at(5, 5), &[]).unwrap();
        let _b = n.request(L, S, at(6, 5), &[]).unwrap();
        assert_eq!(
            n.request(L, S, at(7, 5), &[]),
            Err(PathError::TooManyFields)
        );
        // A released field is evicted to make room and its id goes stale.
        n.release(a).unwrap();
        let c = n.request(L, S, at(7, 5), &[]).unwrap();
        assert_eq!(
            n.sample(a, at(1, 1)),
            Sample::Failed(PathError::InvalidField)
        );
        assert_eq!(n.stats().fields_evicted, 1);
        assert_eq!(n.extend(c, at(300, 300)), Err(PathError::OutOfMap));
    }

    #[test]
    fn tile_budget_fails_the_field_not_the_tick() {
        let cfg = NavConfig {
            max_total_tiles: 6,
            ..NavConfig::default()
        };
        let mut n = nav(512, cfg);
        n.begin_tick(0);
        let id = n.request(L, S, at(500, 500), &[at(5, 5)]).unwrap();
        n.begin_tick(2);
        assert_eq!(
            n.sample(id, at(5, 5)),
            Sample::Failed(PathError::TileBudget)
        );
        assert_eq!(n.stats().total_tiles, 0);
        n.release(id).unwrap();
        assert_eq!(n.stats().live_fields, 0);
    }

    #[test]
    fn straying_units_extend_the_corridor() {
        let mut n = nav(512, NavConfig::default());
        n.begin_tick(0);
        let id = n.request(L, S, at(500, 20), &[at(10, 20)]).unwrap();
        n.begin_tick(2);
        let stray = at(20, 400);
        assert_eq!(n.sample(id, stray), Sample::NeedsExtend);
        let tiles = n.field_tiles(id);
        n.extend(id, stray).unwrap();
        n.extend(id, stray).unwrap();
        assert_eq!(n.stats().extends_scheduled, 1);
        assert_eq!(n.sample(id, stray), Sample::NeedsExtend);
        n.begin_tick(4);
        assert!(matches!(n.sample(id, stray), Sample::Direction(_)));
        assert!(n.field_tiles(id) > tiles);
        // The extension kept every old tile instead of integrating it again.
        assert!(n.field_stats(id).unwrap().tiles_reused as usize >= tiles - 8);
    }

    /// Sector (1,1) split by a wall. The first request lives in the east
    /// pocket with the goal; the west pocket is only reachable around it.
    /// `extend` used to reuse the east tile, leave the west cells unknown,
    /// and fail the next call with `PathError::Unresolved`.
    #[test]
    fn same_sector_pocket_integrates_on_extend() {
        let mut n = Nav::new(
            NavGrid::from_fn(96, 96, |x, y| {
                if x == 48 && (32..64).contains(&y) {
                    0
                } else {
                    LAND
                }
            })
            .unwrap(),
            NavConfig {
                corridor_margin: 0,
                ..NavConfig::default()
            },
            Arc::new(InlineSpawner),
        );
        n.begin_tick(0);
        let id = n.request(L, S, at(60, 40), &[at(55, 40)]).unwrap();
        n.begin_tick(2);
        assert!(matches!(
            n.sample(id, at(55, 40)),
            Sample::Direction(_) | Sample::Arrived
        ));
        let west = at(40, 40);
        assert_eq!(n.sample(id, west), Sample::NeedsExtend);
        n.extend(id, west).unwrap();
        n.begin_tick(4);
        assert!(
            matches!(n.sample(id, west), Sample::Direction(_)),
            "west pocket should integrate on extend, got {:?}",
            n.sample(id, west)
        );
        n.extend(id, west).unwrap();
    }

    /// A sealed room in a sector the field already covers. Extending from
    /// inside must become `Unreachable`, not `PathError::Unresolved`.
    #[test]
    fn sealed_pocket_is_unreachable_not_unresolved() {
        let mut n = Nav::new(
            NavGrid::from_fn(96, 96, |x, y| {
                let wall = ((x == 36 || x == 41) && (36..42).contains(&y))
                    || ((y == 36 || y == 41) && (36..42).contains(&x));
                if wall {
                    0
                } else {
                    LAND
                }
            })
            .unwrap(),
            NavConfig {
                corridor_margin: 0,
                ..NavConfig::default()
            },
            Arc::new(InlineSpawner),
        );
        n.begin_tick(0);
        let id = n.request(L, S, at(60, 40), &[at(55, 40)]).unwrap();
        n.begin_tick(2);
        let inside = at(38, 38);
        assert!(n.is_passable(L, S, Cell::from_pos(inside)));
        assert_eq!(n.sample(id, inside), Sample::NeedsExtend);
        n.extend(id, inside).unwrap();
        n.begin_tick(4);
        assert_eq!(n.sample(id, inside), Sample::Unreachable);
        assert_eq!(n.extend(id, inside), Ok(()));
    }

    #[test]
    fn block_during_flight_adopts_then_repairs() {
        let mut n = nav(256, NavConfig::default());
        n.begin_tick(0);
        let id = n.request(L, S, at(200, 20), &[at(10, 20)]).unwrap();
        n.begin_tick(1);
        // Lands on the corridor while the first build is still pending.
        n.block_rect(CellRect::new(Cell::new(100, 16), Cell::new(104, 24)))
            .unwrap();
        assert_eq!(n.stats().repairs_scheduled, 0);
        n.begin_tick(2);
        // Old result adopted on schedule, repair now in flight.
        assert_eq!(n.ready_tick(id), Some(4));
        assert!(matches!(n.sample(id, at(10, 20)), Sample::Direction(_)));
        // The stale tile still points into the structure: hold instead.
        assert_eq!(n.sample(id, at(99, 20)), Sample::Pending);
        n.begin_tick(4);
        assert!(matches!(n.sample(id, at(99, 20)), Sample::Direction(_)));
        assert_eq!(n.ready_tick(id), None);
        // Far from every field: nothing to repair.
        n.block_rect(CellRect::new(Cell::new(100, 200), Cell::new(104, 204)))
            .unwrap();
        assert_eq!(n.ready_tick(id), None);
    }
}
