//! Flow field construction. Runs on background threads; everything here is a
//! pure function of `BuildInput`.
//!
//! A field is a corridor of sector tiles. The abstract search picks the
//! sectors (routes from every anchor to the goal, widened by a margin), then
//! tiles are integrated in Dijkstra order of their cheapest incoming seed, each
//! seeded from the border costs of the tiles finished before it. Costs strictly
//! fall along every stored direction, including across tile borders, so
//! following the field always ends at the goal.
//!
//! A rebuild reuses a previous tile when its sector version and its seeds are
//! unchanged, which is what keeps repairs local: tiles toward the goal from a
//! change, and tiles whose incoming costs did not move, are shared as-is.

use crate::graph::{find_route, flood, local_costs, step_cost, win, GraphCache, IdMap, Route, DIRS, INF, WIN, WIN_W as W};
use crate::grid::{NavGrid, SectorKind, SECTOR, SECTOR_AREA};
use crate::{Cell, MoveLayer, PathError, SizeClass, SECTOR_CELLS};
use mc_core::StateHasher;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, VecDeque};
use std::sync::Arc;

pub(crate) const CODE_MASK: u8 = 0x0F;
/// Codes 0..8 are indices into `DIRS`.
pub(crate) const CODE_GOAL: u8 = 8;
/// Passable but not integrated: the corridor does not cover this cell's region yet.
pub(crate) const CODE_UNKNOWN: u8 = 9;
/// Region of an anchor the abstract search proved cut off from the goal.
pub(crate) const CODE_UNREACHABLE: u8 = 10;
/// Impassable with no passable cell to escape to inside the tile.
pub(crate) const CODE_STUCK: u8 = 11;
/// A straight line to the goal is clear; steer at the goal, not along the grid.
pub(crate) const FLAG_LOS: u8 = 0x10;
/// Impassable cell; the direction leads to the nearest integrated cell.
pub(crate) const FLAG_ESCAPE: u8 = 0x20;

const EDGE_W: usize = 0;
const EDGE_E: usize = 1;
const EDGE_S: usize = 2;
const EDGE_N: usize = 3;

#[derive(Clone)]
pub(crate) struct Tile {
    pub dirs: [u8; SECTOR_AREA],
    /// Integration costs of the border cells (W, E, S, N), the seeds for neighbours.
    pub edges: [[u32; SECTOR]; 4],
    /// Sector version and seed digest this tile was integrated from; the reuse test.
    pub(crate) version: u32,
    pub(crate) seed_hash: u64,
    /// Cheapest seed. Seeds are hashed relative to it: a uniform shift of every
    /// seed moves the costs but not one direction, so such a tile is reused
    /// with its border costs shifted.
    pub(crate) base: u32,
    /// Re-integrated by a fix-up pass, so `seed_hash` does not describe it. Never reused.
    pub(crate) patched: bool,
    pub(crate) marked: bool,
}

/// Work done by one build. Deterministic, unlike cache counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuildStats {
    pub tiles_built: u32,
    pub tiles_reused: u32,
    pub search_nodes: u32,
}

pub(crate) struct FieldData {
    /// Sorted by sector id.
    pub tiles: Vec<(u32, Arc<Tile>)>,
    /// Anchor cells with no route to the goal.
    pub unreachable: Vec<u32>,
    /// Inclusive sector bounding box of the tiles: (x0, y0, x1, y1).
    pub bounds: (i32, i32, i32, i32),
    pub stats: BuildStats,
}

impl FieldData {
    #[inline]
    pub fn tile(&self, sector: u32) -> Option<&Arc<Tile>> {
        self.tiles.binary_search_by_key(&sector, |t| t.0).ok().map(|i| &self.tiles[i].1)
    }
}

pub(crate) struct BuildInput {
    pub grid: NavGrid,
    pub cache: Arc<GraphCache>,
    pub layer: MoveLayer,
    pub size: SizeClass,
    pub goal: Cell,
    /// Anchors to route. All of the field's anchors when `full`, else only the new ones.
    pub anchors: Vec<Cell>,
    /// Repair: route everything again and drop tiles that fall outside the new corridor.
    pub full: bool,
    pub prev: Option<Arc<FieldData>>,
    pub margin: i32,
    pub los_radius: i32,
    pub max_tiles: usize,
    pub max_nodes: u32,
}

pub(crate) fn build(input: &BuildInput) -> Result<FieldData, PathError> {
    let BuildInput { grid, layer, size, goal, .. } = input;
    let (layer, size, goal) = (*layer, *size, *goal);
    let mut stats = BuildStats::default();
    let prev = input.prev.as_deref();

    // 1. Corridor.
    let goal_sector = grid.sector_of(goal);
    let mut route: BTreeSet<u32> = BTreeSet::from([goal_sector]);
    let mut pinned: BTreeSet<u32> = BTreeSet::new();
    let mut unreachable: Vec<u32> = if input.full { Vec::new() } else { prev.map_or(Vec::new(), |p| p.unreachable.clone()) };
    let mut known = IdMap::default();
    for &anchor in &input.anchors {
        match find_route(grid, &input.cache, layer, size, anchor, goal, &mut known, input.max_nodes, &mut stats.search_nodes) {
            Route::Found(sectors) => route.extend(sectors),
            Route::Unreachable => unreachable.push(grid.cell_index(anchor)),
            Route::Limit => return Err(PathError::SearchLimit),
        }
    }
    unreachable.sort_unstable();
    unreachable.dedup();
    for &cell in &unreachable {
        pinned.insert(grid.sector_of(grid.cell_from_index(cell)));
    }
    let mut corridor: BTreeSet<u32> = pinned;
    for &s in &route {
        let (sx, sy) = grid.sector_xy(s);
        for dy in -input.margin..=input.margin {
            for dx in -input.margin..=input.margin {
                if let Some(n) = grid.sector_index(sx + dx, sy + dy) {
                    if !matches!(grid.layer_sector(layer, sx + dx, sy + dy).kind, SectorKind::Blocked) {
                        corridor.insert(n);
                    }
                }
            }
        }
    }
    if let (false, Some(p)) = (input.full, prev) {
        corridor.extend(p.tiles.iter().map(|t| t.0));
    }
    if corridor.len() > input.max_tiles {
        return Err(PathError::CorridorTooLarge);
    }
    let corridor: Vec<u32> = corridor.into_iter().collect();

    // 2. Tiles, cheapest seed first. An incremental build has no grid change
    // to account for (that would have made it a repair), so it keeps every
    // previous tile and only grows outward from them.
    let mut done: Vec<Option<Arc<Tile>>> = vec![None; corridor.len()];
    let mut fresh = vec![false; corridor.len()];
    let slot = |s: u32| corridor.binary_search(&s).ok();
    let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();
    heap.push(Reverse((0, goal_sector)));
    if let (false, Some(p)) = (input.full, prev) {
        for (s, tile) in &p.tiles {
            done[slot(*s).expect("corridor includes previous tiles")] = Some(tile.clone());
            stats.tiles_reused += 1;
        }
        for (i, &sector) in corridor.iter().enumerate() {
            if done[i].is_some() {
                continue;
            }
            let (sx, sy) = grid.sector_xy(sector);
            for (d, &(dx, dy)) in DIRS.iter().enumerate() {
                let from = grid.sector_index(sx + dx, sy + dy).and_then(slot).and_then(|j| done[j].as_deref());
                let seed = from.map_or(INF, |t| facing_min(t, -dx, -dy));
                if seed != INF {
                    heap.push(Reverse((seed + step_cost(d), sector)));
                }
            }
        }
    }
    let ctx = Ctx { grid, layer, need: size.cells(), goal };
    while let Some(Reverse((_, sector))) = heap.pop() {
        let Some(i) = slot(sector) else { continue };
        if done[i].is_some() {
            continue;
        }
        let ring = ctx.ring(sector, &corridor, &done);
        let built = stats.tiles_built;
        let tile = ctx.tile(sector, &ring, prev, false, &mut stats);
        fresh[i] = stats.tiles_built != built;
        let (sx, sy) = grid.sector_xy(sector);
        for (d, &(dx, dy)) in DIRS.iter().enumerate() {
            let Some(n) = grid.sector_index(sx + dx, sy + dy) else { continue };
            if slot(n).is_none_or(|j| done[j].is_some()) {
                continue;
            }
            let seed = facing_min(&tile, dx, dy);
            if seed != INF {
                heap.push(Reverse((seed + step_cost(d), n)));
            }
        }
        done[i] = Some(tile);
    }

    // 3. Fix-up: regions of a finished tile that are only reachable through a
    // tile finished later. Re-integrating with a superset of seeds can only
    // lower costs, so directions already handed to neighbours stay valid.
    // Only tiles next to something integrated in the previous round can gain.
    for _ in 0..MAX_FIXUP_PASSES {
        let mut next = vec![false; corridor.len()];
        for (i, &sector) in corridor.iter().enumerate() {
            let (sx, sy) = grid.sector_xy(sector);
            let near_fresh = DIRS.iter().any(|&(dx, dy)| grid.sector_index(sx + dx, sy + dy).and_then(slot).is_some_and(|j| fresh[j]));
            if !near_fresh || done[i].as_deref().is_some_and(|t| !ctx.may_gain(t, sector, &corridor, &done)) {
                continue;
            }
            let ring = ctx.ring(sector, &corridor, &done);
            let wants = match &done[i] {
                None => ring.cost.iter().any(|&c| c != INF),
                Some(t) => ctx.has_fillable_border(t, &ring),
            };
            if !wants {
                continue;
            }
            let tile = ctx.tile(sector, &ring, None, true, &mut stats);
            next[i] = done[i].as_ref().is_none_or(|old| old.edges != tile.edges);
            done[i] = Some(tile);
        }
        if !next.contains(&true) {
            break;
        }
        fresh = next;
    }

    // 4. Marks for cut-off anchors, then line-of-sight flags around the goal.
    for &cell in &unreachable {
        let c = grid.cell_from_index(cell);
        let sector = grid.sector_of(c);
        let Some(i) = slot(sector) else { continue };
        let (sx, sy) = grid.sector_xy(sector);
        let local = (c.y % SECTOR_CELLS) as usize * SECTOR + (c.x % SECTOR_CELLS) as usize;
        let region = local_costs(grid.layer_sector(layer, sx, sy), size.cells(), local);
        let ring = Ring { cost: [INF; WIN], pass: ctx.pass_window(sector) };
        let tile = Arc::make_mut(done[i].get_or_insert_with(|| ctx.tile(sector, &ring, None, true, &mut stats)));
        for (code, &r) in tile.dirs.iter_mut().zip(region.iter()) {
            if r != INF && *code & CODE_MASK == CODE_UNKNOWN {
                *code = CODE_UNREACHABLE;
                tile.marked = true;
            }
        }
    }
    ctx.flag_line_of_sight(&corridor, &mut done, input.los_radius);

    let mut bounds = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (&s, _) in corridor.iter().zip(&done).filter(|(_, t)| t.is_some()) {
        let (x, y) = grid.sector_xy(s);
        bounds = (bounds.0.min(x), bounds.1.min(y), bounds.2.max(x), bounds.3.max(y));
    }
    let tiles = corridor.into_iter().zip(done).filter_map(|(s, t)| t.map(|t| (s, t))).collect();
    Ok(FieldData { tiles, unreachable, bounds, stats })
}

const MAX_FIXUP_PASSES: usize = 8;

/// Cheapest border cost of `tile` on the side facing the neighbour at `(dx, dy)`.
fn facing_min(tile: &Tile, dx: i32, dy: i32) -> u32 {
    let last = SECTOR - 1;
    match (dx, dy) {
        (-1, 0) => *tile.edges[EDGE_W].iter().min().unwrap(),
        (1, 0) => *tile.edges[EDGE_E].iter().min().unwrap(),
        (0, -1) => *tile.edges[EDGE_S].iter().min().unwrap(),
        (0, 1) => *tile.edges[EDGE_N].iter().min().unwrap(),
        (-1, -1) => tile.edges[EDGE_W][0],
        (1, -1) => tile.edges[EDGE_E][0],
        (-1, 1) => tile.edges[EDGE_W][last],
        _ => tile.edges[EDGE_E][last],
    }
}

/// The one-cell frame around a sector: seed costs from finished neighbour
/// tiles, and passability of the whole 34 x 34 window.
struct Ring {
    cost: [u32; WIN],
    pass: [bool; WIN],
}

struct Ctx<'a> {
    grid: &'a NavGrid,
    layer: MoveLayer,
    need: u8,
    goal: Cell,
}

impl Ctx<'_> {
    fn pass_window(&self, sector: u32) -> [bool; WIN] {
        let (sx, sy) = self.grid.sector_xy(sector);
        let mut pass = [false; WIN];
        // Per neighbour sector: the span of window cells it covers and where its local coordinates start.
        let spans = [(-1, -1, SECTOR_CELLS - 1), (0, SECTOR_CELLS - 1, 0), (SECTOR_CELLS, SECTOR_CELLS, 0)];
        for (dy, &(y0, y1, ly0)) in spans.iter().enumerate() {
            for (dx, &(x0, x1, lx0)) in spans.iter().enumerate() {
                let (nx, ny) = (sx + dx as i32 - 1, sy + dy as i32 - 1);
                if self.grid.sector_index(nx, ny).is_none() {
                    continue;
                }
                let caps = match &self.grid.layer_sector(self.layer, nx, ny).kind {
                    SectorKind::Blocked => continue,
                    SectorKind::Open => None,
                    SectorKind::Mixed(c) => Some(c),
                };
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let local = (ly0 + y - y0) as usize * SECTOR + (lx0 + x - x0) as usize;
                        pass[win(x, y)] = caps.is_none_or(|c| c[local] >= self.need);
                    }
                }
            }
        }
        pass
    }

    fn ring(&self, sector: u32, corridor: &[u32], done: &[Option<Arc<Tile>>]) -> Ring {
        let (sx, sy) = self.grid.sector_xy(sector);
        let mut ring = Ring { cost: [INF; WIN], pass: self.pass_window(sector) };
        let n = SECTOR_CELLS;
        let last = SECTOR - 1;
        // Frame cells outside the corridor count as walls. A unit cutting a
        // sector corner diagonally passes through the two sectors beside it,
        // and must not be led through one that has no tile.
        for (dx, dy) in DIRS {
            let inside = self.grid.sector_index(sx + dx, sy + dy).is_some_and(|s| corridor.binary_search(&s).is_ok());
            if !inside {
                let xs = if dx == 0 { 0..n } else if dx < 0 { -1..0 } else { n..n + 1 };
                for x in xs {
                    let ys = if dy == 0 { 0..n } else if dy < 0 { -1..0 } else { n..n + 1 };
                    for y in ys {
                        ring.pass[win(x, y)] = false;
                    }
                }
            }
        }
        let tile_at = |dx: i32, dy: i32| -> Option<&Tile> {
            let s = self.grid.sector_index(sx + dx, sy + dy)?;
            done[corridor.binary_search(&s).ok()?].as_deref()
        };
        for i in 0..SECTOR {
            let v = i as i32;
            if let Some(t) = tile_at(-1, 0) {
                ring.cost[win(-1, v)] = t.edges[EDGE_E][i];
            }
            if let Some(t) = tile_at(1, 0) {
                ring.cost[win(n, v)] = t.edges[EDGE_W][i];
            }
            if let Some(t) = tile_at(0, -1) {
                ring.cost[win(v, -1)] = t.edges[EDGE_N][i];
            }
            if let Some(t) = tile_at(0, 1) {
                ring.cost[win(v, n)] = t.edges[EDGE_S][i];
            }
        }
        if let Some(t) = tile_at(-1, -1) {
            ring.cost[win(-1, -1)] = t.edges[EDGE_E][last];
        }
        if let Some(t) = tile_at(1, -1) {
            ring.cost[win(n, -1)] = t.edges[EDGE_W][last];
        }
        if let Some(t) = tile_at(-1, 1) {
            ring.cost[win(-1, n)] = t.edges[EDGE_E][0];
        }
        if let Some(t) = tile_at(1, 1) {
            ring.cost[win(n, n)] = t.edges[EDGE_W][0];
        }
        // A seed on a cell the current grid blocks is stale data from a reused neighbour.
        for (c, &p) in ring.cost.iter_mut().zip(ring.pass.iter()) {
            if !p {
                *c = INF;
            }
        }
        ring
    }

    /// Cheap screen for `has_fillable_border` from border costs alone: some
    /// unintegrated border cell faces an integrated cell of a neighbour.
    fn may_gain(&self, tile: &Tile, sector: u32, corridor: &[u32], done: &[Option<Arc<Tile>>]) -> bool {
        if tile.edges.iter().flatten().all(|&c| c != INF) {
            return false;
        }
        let (sx, sy) = self.grid.sector_xy(sector);
        let tile_at = |dx: i32, dy: i32| -> Option<&Tile> {
            let s = self.grid.sector_index(sx + dx, sy + dy)?;
            done[corridor.binary_search(&s).ok()?].as_deref()
        };
        let last = SECTOR - 1;
        let sides = [(-1, 0, EDGE_W, EDGE_E), (1, 0, EDGE_E, EDGE_W), (0, -1, EDGE_S, EDGE_N), (0, 1, EDGE_N, EDGE_S)];
        let across = sides.iter().any(|&(dx, dy, mine, theirs)| {
            tile_at(dx, dy).is_some_and(|t| (0..SECTOR).any(|i| tile.edges[mine][i] == INF && (i.saturating_sub(1)..=(i + 1).min(last)).any(|j| t.edges[theirs][j] != INF)))
        });
        let corners = [(-1, -1, EDGE_W, 0, EDGE_E, last), (1, -1, EDGE_E, 0, EDGE_W, last), (-1, 1, EDGE_W, last, EDGE_E, 0), (1, 1, EDGE_E, last, EDGE_W, 0)];
        across || corners.iter().any(|&(dx, dy, mine, i, theirs, j)| tile.edges[mine][i] == INF && tile_at(dx, dy).is_some_and(|t| t.edges[theirs][j] != INF))
    }

    /// Whether a passable, unintegrated border cell of `tile` now touches a seed.
    fn has_fillable_border(&self, tile: &Tile, ring: &Ring) -> bool {
        let n = SECTOR_CELLS;
        // Same move rule as integration, or a diagonal-only contact would re-run the tile forever.
        let seeded = |x: i32, y: i32| {
            DIRS.iter().enumerate().any(|(d, &(dx, dy))| {
                let inside = (0..n).contains(&(x + dx)) && (0..n).contains(&(y + dy));
                !inside && ring.cost[win(x + dx, y + dy)] != INF && (d & 1 == 0 || (ring.pass[win(x + dx, y)] && ring.pass[win(x, y + dy)]))
            })
        };
        for i in 0..SECTOR {
            let v = i as i32;
            let cells = [(0, v, EDGE_W), (n - 1, v, EDGE_E), (v, 0, EDGE_S), (v, n - 1, EDGE_N)];
            for (x, y, e) in cells {
                if tile.edges[e][i] == INF && ring.pass[win(x, y)] && seeded(x, y) {
                    return true;
                }
            }
        }
        false
    }

    fn goal_local(&self, sector: u32) -> Option<usize> {
        (self.grid.sector_of(self.goal) == sector).then(|| (self.goal.y % SECTOR_CELLS) as usize * SECTOR + (self.goal.x % SECTOR_CELLS) as usize)
    }

    /// Reuses the previous tile when nothing it was computed from changed, else integrates.
    fn tile(&self, sector: u32, ring: &Ring, prev: Option<&FieldData>, patched: bool, stats: &mut BuildStats) -> Arc<Tile> {
        let (sx, sy) = self.grid.sector_xy(sector);
        let version = self.grid.layer_sector(self.layer, sx, sy).version;
        // The goal cell is a seed fixed at zero, so its tile is not shift-invariant.
        let base = if self.goal_local(sector).is_some() { 0 } else { ring.cost.iter().copied().min().filter(|&c| c != INF).unwrap_or(0) };
        let mut h = StateHasher::new();
        for &c in ring.cost.iter() {
            h.write_u32(if c == INF { INF } else { c - base });
        }
        for row in ring.pass.chunks(W) {
            h.write_u64(row.iter().enumerate().fold(0u64, |a, (i, &p)| a | (p as u64) << i));
        }
        let seed_hash = h.finish();
        if let Some(old) = prev.and_then(|p| p.tile(sector)) {
            if !patched && !old.patched && old.version == version && old.seed_hash == seed_hash {
                stats.tiles_reused += 1;
                if !old.marked && old.base == base {
                    return old.clone();
                }
                let mut tile = (**old).clone();
                for c in tile.edges.iter_mut().flatten().filter(|c| **c != INF) {
                    *c = *c - old.base + base;
                }
                tile.base = base;
                // Cut-off marks are re-derived by every build; a reused tile must not carry old ones.
                tile.marked = false;
                for code in tile.dirs.iter_mut().filter(|c| **c == CODE_UNREACHABLE) {
                    *code = CODE_UNKNOWN;
                }
                return Arc::new(tile);
            }
        }
        stats.tiles_built += 1;
        let mut tile = self.integrate(sector, ring);
        tile.version = version;
        tile.seed_hash = seed_hash;
        tile.base = base;
        tile.patched = patched;
        Arc::new(tile)
    }

    fn integrate(&self, sector: u32, ring: &Ring) -> Tile {
        let n = SECTOR_CELLS;
        let (sx, sy) = self.grid.sector_xy(sector);
        let pass = &ring.pass;
        let mut cost = ring.cost;
        let goal_local = self.goal_local(sector);
        if let Some(g) = goal_local {
            cost[win((g % SECTOR) as i32, (g / SECTOR) as i32)] = 0;
        }
        flood(&mut cost, pass);
        let interior = |x: i32, y: i32| x >= 0 && y >= 0 && x < n && y < n;

        let mut dirs = [CODE_UNKNOWN; SECTOR_AREA];
        let mut escape: VecDeque<usize> = VecDeque::new();
        for y in 0..n {
            for x in 0..n {
                let local = y as usize * SECTOR + x as usize;
                let here = cost[win(x, y)];
                if !pass[win(x, y)] {
                    dirs[local] = CODE_STUCK;
                    continue;
                }
                if here == INF {
                    continue;
                }
                escape.push_back(local);
                if goal_local == Some(local) {
                    dirs[local] = CODE_GOAL;
                    continue;
                }
                // Steepest descent; ties go to the step best aligned with the
                // bearing to the goal, so equal-cost staircases track the
                // straight line instead of running diagonal-then-straight.
                let (gx, gy) = ((self.goal.x - sx * n - x) as i64, (self.goal.y - sy * n - y) as i64);
                let mut best: Option<(u32, i64, u8)> = None;
                for (d, &(dx, dy)) in DIRS.iter().enumerate() {
                    let (nx, ny) = (x + dx, y + dy);
                    let next = cost[win(nx, ny)];
                    if next >= here || !pass[win(nx, ny)] || (d & 1 == 1 && !(pass[win(nx, y)] && pass[win(x, ny)])) {
                        continue;
                    }
                    let along = (dx as i64 * gx + dy as i64 * gy) * if d & 1 == 1 { 7071 } else { 10_000 };
                    let key = (next + step_cost(d), -along, d as u8);
                    if best.is_none_or(|b| key < b) {
                        best = Some(key);
                    }
                }
                if let Some((_, _, d)) = best {
                    dirs[local] = d;
                }
            }
        }
        // Units shoved onto an impassable cell (or built over) get led back out.
        while let Some(local) = escape.pop_front() {
            let (x, y) = ((local % SECTOR) as i32, (local / SECTOR) as i32);
            for (d, &(dx, dy)) in DIRS.iter().enumerate() {
                let (nx, ny) = (x + dx, y + dy);
                if !interior(nx, ny) {
                    continue;
                }
                let nl = ny as usize * SECTOR + nx as usize;
                if dirs[nl] == CODE_STUCK {
                    dirs[nl] = ((d + 4) % 8) as u8 | FLAG_ESCAPE;
                    escape.push_back(nl);
                }
            }
        }

        let last = n - 1;
        let edges = [
            std::array::from_fn(|i| cost[win(0, i as i32)]),
            std::array::from_fn(|i| cost[win(last, i as i32)]),
            std::array::from_fn(|i| cost[win(i as i32, 0)]),
            std::array::from_fn(|i| cost[win(i as i32, last)]),
        ];
        Tile { dirs, edges, version: 0, seed_hash: 0, base: 0, patched: false, marked: false }
    }

    /// Sets `FLAG_LOS` on cells within `radius` of the goal whose fat line to
    /// the goal is clear. Recomputed by every build because the line may cross
    /// sectors that changed while the cell's own tile was reused.
    fn flag_line_of_sight(&self, corridor: &[u32], done: &mut [Option<Arc<Tile>>], radius: i32) {
        let g = self.goal;
        for y in g.y - radius..=g.y + radius {
            for x in g.x - radius..=g.x + radius {
                let c = Cell::new(x, y);
                if !self.grid.contains(c) {
                    continue;
                }
                let Ok(i) = corridor.binary_search(&self.grid.sector_of(c)) else { continue };
                let Some(tile) = &done[i] else { continue };
                let local = (y % SECTOR_CELLS) as usize * SECTOR + (x % SECTOR_CELLS) as usize;
                let code = tile.dirs[local];
                if code & CODE_MASK >= CODE_GOAL || code & FLAG_ESCAPE != 0 {
                    continue;
                }
                let want = if self.clear_line(c) { code | FLAG_LOS } else { code & !FLAG_LOS };
                if want != code {
                    Arc::make_mut(done[i].as_mut().unwrap()).dirs[local] = want;
                }
            }
        }
    }

    /// Three-cell-wide line walk: units start anywhere in the cell, so the
    /// centre line alone would let them clip a corner.
    fn clear_line(&self, from: Cell) -> bool {
        let (dx, dy) = (self.goal.x - from.x, self.goal.y - from.y);
        let steps = dx.abs().max(dy.abs());
        let ok = |x: i32, y: i32| self.grid.clearance(self.layer, Cell::new(x, y)) >= self.need;
        for i in 1..steps {
            let (x, y) = if dx.abs() >= dy.abs() {
                (from.x + i * dx.signum(), from.y + (2 * i * dy + steps * dy.signum()) / (2 * steps))
            } else {
                (from.x + (2 * i * dx + steps * dx.signum()) / (2 * steps), from.y + i * dy.signum())
            };
            let clear = if dx.abs() >= dy.abs() { ok(x, y - 1) && ok(x, y) && ok(x, y + 1) } else { ok(x - 1, y) && ok(x, y) && ok(x + 1, y) };
            if !clear {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::*;

    fn input(grid: &NavGrid, goal: Cell, anchors: &[Cell]) -> BuildInput {
        BuildInput {
            grid: grid.clone(),
            cache: Arc::new(GraphCache::new()),
            layer: MoveLayer::Land,
            size: SizeClass::SMALL,
            goal,
            anchors: anchors.to_vec(),
            full: true,
            prev: None,
            margin: 1,
            los_radius: 20,
            max_tiles: 4096,
            max_nodes: 1 << 20,
        }
    }

    /// Follows stored directions cell by cell; panics on a loop or a dead end.
    fn walk(grid: &NavGrid, data: &FieldData, from: Cell, goal: Cell) -> u32 {
        let mut c = from;
        for steps in 0..100_000 {
            if c == goal {
                return steps;
            }
            let tile = data.tile(grid.sector_of(c)).unwrap_or_else(|| panic!("left the corridor at {c:?}"));
            let code = tile.dirs[(c.y % SECTOR_CELLS) as usize * SECTOR + (c.x % SECTOR_CELLS) as usize] & CODE_MASK;
            assert!(code < 8, "no direction at {c:?}: code {code}");
            let (dx, dy) = DIRS[code as usize];
            c = Cell::new(c.x + dx, c.y + dy);
            assert!(grid.is_passable(MoveLayer::Land, SizeClass::SMALL, c));
        }
        panic!("loop");
    }

    #[test]
    fn open_ground_walks_are_octile_optimal() {
        let grid = NavGrid::from_fn(256, 256, |_, _| LAND).unwrap();
        let (goal, from) = (Cell::new(200, 40), Cell::new(20, 130));
        let data = build(&input(&grid, goal, &[from])).unwrap();
        assert_eq!(walk(&grid, &data, from, goal), 180);
        assert!(data.unreachable.is_empty());
    }

    #[test]
    fn pockets_behind_later_tiles_get_filled() {
        // Sector (1,1) is split by a wall; its west half is only reachable
        // through sector (0,1), which integrates later than (1,1).
        let grid = NavGrid::from_fn(96, 96, |x, y| if x == 48 && (32..64).contains(&y) { 0 } else { LAND }).unwrap();
        let (goal, from) = (Cell::new(60, 40), Cell::new(40, 40));
        let data = build(&input(&grid, goal, &[from])).unwrap();
        assert!(walk(&grid, &data, from, goal) > 20);
    }

    #[test]
    fn rebuild_reuses_everything_when_nothing_changed() {
        let grid = NavGrid::from_fn(512, 512, |x, y| if x % 11 < 3 && y % 13 < 4 { LAND | STEEP } else { LAND }).unwrap();
        let (goal, from) = (Cell::new(480, 470), Cell::new(10, 12));
        let mut inp = input(&grid, goal, &[from]);
        let first = Arc::new(build(&inp).unwrap());
        walk(&grid, &first, from, goal);
        inp.prev = Some(first.clone());
        let second = build(&inp).unwrap();
        assert_eq!(second.stats.tiles_built, 0);
        assert_eq!(second.stats.tiles_reused as usize, first.tiles.len());
    }

    #[test]
    fn cut_off_anchor_is_marked_and_limits_hold() {
        let grid = NavGrid::from_fn(256, 256, |x, y| if (x - 128).abs().max((y - 128).abs()) == 20 { DEEP } else { LAND }).unwrap();
        let (goal, from) = (Cell::new(128, 128), Cell::new(10, 10));
        let data = build(&input(&grid, goal, &[from])).unwrap();
        assert_eq!(data.unreachable, vec![grid.cell_index(from)]);
        let tile = data.tile(0).unwrap();
        assert_eq!(tile.dirs[10 * SECTOR + 10], CODE_UNREACHABLE);
        let mut small = input(&NavGrid::from_fn(512, 512, |_, _| LAND).unwrap(), Cell::new(500, 500), &[Cell::new(5, 5)]);
        small.max_tiles = 8;
        assert_eq!(build(&small).err(), Some(PathError::CorridorTooLarge));
    }

    #[test]
    fn line_of_sight_stops_at_walls() {
        let grid = NavGrid::from_fn(64, 64, |x, y| if x == 20 && y > 4 { 0 } else { LAND }).unwrap();
        let goal = Cell::new(30, 30);
        let data = build(&input(&grid, goal, &[])).unwrap();
        let code = |c: Cell| data.tile(grid.sector_of(c)).unwrap().dirs[(c.y % 32) as usize * SECTOR + (c.x % 32) as usize];
        assert_ne!(code(Cell::new(40, 25)) & FLAG_LOS, 0);
        assert_eq!(code(Cell::new(15, 30)) & FLAG_LOS, 0);
        assert_eq!(code(goal), CODE_GOAL);
        // The wall cells point back out.
        assert_ne!(code(Cell::new(20, 30)) & FLAG_ESCAPE, 0);
    }
}
