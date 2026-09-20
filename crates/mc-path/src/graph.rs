//! Sector portals, the abstract graph over them and the corridor search.
//!
//! A portal is a maximal run of cell pairs that face each other across a
//! sector edge and are both passable for a (layer, size class). Portals are the
//! nodes; two portals of one sector are joined by the in-sector path cost
//! between their centre cells. Graphs are derived lazily, per sector, inside
//! background builds and memoised in `GraphCache`. An entry is valid only while
//! the versions of the sector and its four neighbours match, so a blocker
//! change invalidates exactly the touched sectors and their neighbours. The
//! cache is semantically transparent: a hit and a rebuild give the same graph,
//! so it may be shared between threads and evicted at will.

use crate::grid::{LayerSector, NavGrid, SectorKind, SECTOR, SECTOR_AREA};
use crate::{Cell, MoveLayer, SizeClass, SECTOR_CELLS};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub(crate) const INF: u32 = u32::MAX;
pub(crate) const STRAIGHT: u32 = 10;
pub(crate) const DIAGONAL: u32 = 14;

/// Counter-clockwise from +X, like `Angle`.
pub(crate) const DIRS: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

#[inline]
pub(crate) fn step_cost(dir: usize) -> u32 {
    if dir & 1 == 0 {
        STRAIGHT
    } else {
        DIAGONAL
    }
}

#[inline]
pub(crate) fn octile(dx: i32, dy: i32) -> u32 {
    let (a, b) = (dx.unsigned_abs(), dy.unsigned_abs());
    STRAIGHT * a.max(b) + (DIAGONAL - STRAIGHT) * a.min(b)
}

/// Multiplicative hasher for integer keys. The maps using it are lookup-only,
/// so iteration order never leaks into results.
#[derive(Default)]
pub(crate) struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100_0000_01B3);
        }
    }
    fn write_u32(&mut self, v: u32) {
        let x = (v as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 = x ^ (x >> 29);
    }
}

pub(crate) type IdMap<V> = HashMap<u32, V, BuildHasherDefault<IdHasher>>;

/// Flood window: a sector plus a one-cell frame.
pub(crate) const WIN_W: usize = SECTOR + 2;
pub(crate) const WIN: usize = WIN_W * WIN_W;

#[inline]
pub(crate) fn win(x: i32, y: i32) -> usize {
    (y + 1) as usize * WIN_W + (x + 1) as usize
}

/// Shortest-path costs over a window. Every finite entry of `cost` is a seed;
/// frame cells are seeds only and are never relaxed. Diagonal steps need both
/// orthogonal neighbours passable, so nothing cuts a corner.
///
/// Dial's bucket queue: steps cost 10 or 14, so 16 circular buckets order the
/// frontier without a heap. Final costs do not depend on processing order.
pub(crate) fn flood(cost: &mut [u32; WIN], pass: &[bool; WIN]) {
    const OFFSETS: [isize; 8] = [
        1,
        WIN_W as isize + 1,
        WIN_W as isize,
        WIN_W as isize - 1,
        -1,
        -(WIN_W as isize) - 1,
        -(WIN_W as isize),
        -(WIN_W as isize) + 1,
    ];
    let mut buckets: [Vec<u16>; 16] = Default::default();
    let mut queued = 0usize;
    let mut level = INF;
    for (i, &c) in cost.iter().enumerate() {
        if c != INF {
            buckets[(c & 15) as usize].push(i as u16);
            queued += 1;
            level = level.min(c);
        }
    }
    let mut scratch: Vec<u16> = Vec::new();
    let mut idle = 0;
    while queued > 0 {
        let b = (level & 15) as usize;
        std::mem::swap(&mut scratch, &mut buckets[b]);
        let mut worked = false;
        for &entry in &scratch {
            let i = entry as usize;
            if cost[i] > level {
                // A seed from a later lap of the wheel.
                buckets[b].push(entry);
                continue;
            }
            queued -= 1;
            if cost[i] < level {
                continue;
            }
            worked = true;
            for (d, off) in OFFSETS.iter().enumerate() {
                let n = i as isize + off;
                if n < 0 || n >= WIN as isize {
                    continue;
                }
                let n = n as usize;
                let (nx, ny) = (n % WIN_W, n / WIN_W);
                let interior = (1..=SECTOR).contains(&nx) && (1..=SECTOR).contains(&ny);
                if !interior || !pass[n] {
                    continue;
                }
                if d & 1 == 1 && !(pass[ny * WIN_W + i % WIN_W] && pass[(i / WIN_W) * WIN_W + nx]) {
                    continue;
                }
                let nc = level + step_cost(d);
                if nc < cost[n] {
                    cost[n] = nc;
                    buckets[(nc & 15) as usize].push(n as u16);
                    queued += 1;
                }
            }
        }
        scratch.clear();
        level += 1;
        idle = if worked { 0 } else { idle + 1 };
        if idle >= 16 && queued > 0 {
            // Nothing within a lap: jump to the next seed instead of stepping to it.
            level = buckets
                .iter()
                .flatten()
                .map(|&e| cost[e as usize])
                .filter(|&c| c >= level)
                .min()
                .unwrap_or(level);
            idle = 0;
        }
    }
}

/// In-sector path costs from `start` to every cell, for units needing clearance `need`.
pub(crate) fn local_costs(sector: &LayerSector, need: u8, start: usize) -> Box<[u32; SECTOR_AREA]> {
    let mut out = Box::new([INF; SECTOR_AREA]);
    let (sx, sy) = ((start % SECTOR) as i32, (start / SECTOR) as i32);
    match &sector.kind {
        SectorKind::Blocked => {}
        SectorKind::Open => {
            for (i, c) in out.iter_mut().enumerate() {
                *c = octile((i % SECTOR) as i32 - sx, (i / SECTOR) as i32 - sy);
            }
        }
        SectorKind::Mixed(caps) => {
            if caps[start] < need {
                return out;
            }
            let mut pass = [false; WIN];
            let mut cost = [INF; WIN];
            for y in 0..SECTOR {
                for x in 0..SECTOR {
                    pass[(y + 1) * WIN_W + x + 1] = caps[y * SECTOR + x] >= need;
                }
            }
            cost[win(sx, sy)] = 0;
            flood(&mut cost, &pass);
            for y in 0..SECTOR {
                out[y * SECTOR..(y + 1) * SECTOR]
                    .copy_from_slice(&cost[(y + 1) * WIN_W + 1..(y + 1) * WIN_W + 1 + SECTOR]);
            }
        }
    }
    out
}

pub(crate) const SIDE_W: u8 = 0;
pub(crate) const SIDE_E: u8 = 1;
pub(crate) const SIDE_S: u8 = 2;
pub(crate) const SIDE_N: u8 = 3;
const SIDE_STEP: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Portal {
    pub side: u8,
    pub start: u8,
    pub len: u8,
}

impl Portal {
    /// Centre cell on this sector's side of the edge, local coordinates.
    pub fn center(self) -> (i32, i32) {
        let mid = (self.start + self.len / 2) as i32;
        match self.side {
            SIDE_W => (0, mid),
            SIDE_E => (SECTOR_CELLS - 1, mid),
            SIDE_S => (mid, 0),
            _ => (mid, SECTOR_CELLS - 1),
        }
    }

    /// Midpoint of the run on the sector boundary itself, in half cells
    /// (cell centre `x` is `2x`, so the west boundary is -1). Measuring
    /// between these keeps abstract costs exactly octile across open sectors,
    /// which is what lets the search tie-break toward the straight line.
    pub fn edge_point(self) -> (i32, i32) {
        let mid = 2 * self.start as i32 + self.len as i32 - 1;
        match self.side {
            SIDE_W => (-1, mid),
            SIDE_E => (2 * SECTOR_CELLS - 1, mid),
            SIDE_S => (mid, -1),
            _ => (mid, 2 * SECTOR_CELLS - 1),
        }
    }

    fn center_index(self) -> usize {
        let (x, y) = self.center();
        y as usize * SECTOR + x as usize
    }
}

pub(crate) struct SectorGraph {
    /// Sorted by (side, start).
    pub portals: Vec<Portal>,
    /// `portals.len()` squared, row-major, `INF` where not connected inside the
    /// sector. Boundary to boundary, so one edge crossing is included.
    pub costs: Vec<u32>,
    /// Versions of the sector and its W, E, S, N neighbours this was derived from.
    versions: [u32; 5],
}

impl SectorGraph {
    fn find(&self, side: u8, start: u8) -> Option<usize> {
        self.portals
            .binary_search_by_key(&(side, start), |p| (p.side, p.start))
            .ok()
    }
}

fn versions(grid: &NavGrid, layer: MoveLayer, sx: i32, sy: i32) -> [u32; 5] {
    let v = |dx: i32, dy: i32| {
        grid.sector_index(sx + dx, sy + dy)
            .map_or(0, |_| grid.layer_sector(layer, sx + dx, sy + dy).version)
    };
    [v(0, 0), v(-1, 0), v(1, 0), v(0, -1), v(0, 1)]
}

fn build_graph(grid: &NavGrid, layer: MoveLayer, size: SizeClass, sx: i32, sy: i32) -> SectorGraph {
    let need = size.cells();
    let me = grid.layer_sector(layer, sx, sy);
    let mut portals = Vec::new();
    for side in 0..4u8 {
        let (dx, dy) = SIDE_STEP[side as usize];
        if grid.sector_index(sx + dx, sy + dy).is_none() {
            continue;
        }
        let other = grid.layer_sector(layer, sx + dx, sy + dy);
        let last = SECTOR - 1;
        let pair = |i: usize| -> bool {
            let (a, b) = match side {
                SIDE_W => (i * SECTOR, i * SECTOR + last),
                SIDE_E => (i * SECTOR + last, i * SECTOR),
                SIDE_S => (i, last * SECTOR + i),
                _ => (last * SECTOR + i, i),
            };
            me.cap(a) >= need && other.cap(b) >= need
        };
        let mut i = 0;
        while i < SECTOR {
            if !pair(i) {
                i += 1;
                continue;
            }
            let start = i;
            while i < SECTOR && pair(i) {
                i += 1;
            }
            portals.push(Portal {
                side,
                start: start as u8,
                len: (i - start) as u8,
            });
        }
    }
    let n = portals.len();
    let mut costs = vec![INF; n * n];
    for (a, p) in portals.iter().enumerate() {
        let local = local_costs(me, need, p.center_index());
        for (b, q) in portals.iter().enumerate() {
            costs[a * n + b] = local[q.center_index()].saturating_add(STRAIGHT);
        }
    }
    SectorGraph {
        portals,
        costs,
        versions: versions(grid, layer, sx, sy),
    }
}

/// Graph of an open sector whose four neighbours are open too: one full-edge portal per side.
fn open_graph() -> Arc<SectorGraph> {
    static OPEN: OnceLock<Arc<SectorGraph>> = OnceLock::new();
    OPEN.get_or_init(|| {
        let portals: Vec<Portal> = (0..4)
            .map(|side| Portal {
                side,
                start: 0,
                len: SECTOR as u8,
            })
            .collect();
        let mut costs = vec![0; 16];
        for (a, p) in portals.iter().enumerate() {
            for (b, q) in portals.iter().enumerate() {
                let ((px, py), (qx, qy)) = (p.edge_point(), q.edge_point());
                costs[a * 4 + b] = octile(px - qx, py - qy) / 2;
            }
        }
        Arc::new(SectorGraph {
            portals,
            costs,
            versions: [0; 5],
        })
    })
    .clone()
}

const SHARDS: usize = 64;
const SHARD_CAP: usize = 1024;

pub(crate) struct GraphCache {
    shards: Vec<Mutex<IdMap<Arc<SectorGraph>>>>,
    /// Graphs derived from cell data (cache misses). Diagnostic; depends on thread timing.
    pub built: AtomicU64,
}

impl GraphCache {
    pub fn new() -> GraphCache {
        GraphCache {
            shards: (0..SHARDS).map(|_| Mutex::new(IdMap::default())).collect(),
            built: AtomicU64::new(0),
        }
    }

    pub fn get(
        &self,
        grid: &NavGrid,
        layer: MoveLayer,
        size: SizeClass,
        sector: u32,
    ) -> Arc<SectorGraph> {
        let (sx, sy) = grid.sector_xy(sector);
        let (sw, sh) = grid.sector_dims();
        let open = |dx: i32, dy: i32| {
            matches!(
                grid.layer_sector(layer, sx + dx, sy + dy).kind,
                SectorKind::Open
            )
        };
        if sx > 0
            && sy > 0
            && sx + 1 < sw
            && sy + 1 < sh
            && open(0, 0)
            && open(-1, 0)
            && open(1, 0)
            && open(0, -1)
            && open(0, 1)
        {
            return open_graph();
        }
        let key = sector | (layer.index() as u32) << 20 | (size.index() as u32) << 22;
        let want = versions(grid, layer, sx, sy);
        let shard = &self.shards[(sector as usize).wrapping_mul(0x9E37) % SHARDS];
        if let Some(g) = shard.lock().unwrap().get(&key) {
            if g.versions == want {
                return g.clone();
            }
        }
        // Built outside the lock: two threads may race to the same graph, which only costs time.
        let graph = Arc::new(build_graph(grid, layer, size, sx, sy));
        self.built.fetch_add(1, Ordering::Relaxed);
        let mut map = shard.lock().unwrap();
        if map.len() >= SHARD_CAP {
            map.clear();
        }
        map.insert(key, graph.clone());
        graph
    }
}

/// Canonical id of a portal: the edge belongs to the sector on its west/south side.
fn node_id(grid: &NavGrid, sector: u32, p: Portal) -> u32 {
    let (sx, sy) = grid.sector_xy(sector);
    let (owner, north) = match p.side {
        SIDE_E => (sector, 0),
        SIDE_N => (sector, 1),
        SIDE_W => (
            grid.sector_index(sx - 1, sy)
                .expect("portal implies neighbour"),
            0,
        ),
        _ => (
            grid.sector_index(sx, sy - 1)
                .expect("portal implies neighbour"),
            1,
        ),
    };
    owner << 6 | north << 5 | p.start as u32
}

/// The two (sector, side) views of a node.
fn node_sides(grid: &NavGrid, node: u32) -> [(u32, u8, u8); 2] {
    let (owner, north, start) = (node >> 6, node >> 5 & 1, (node & 31) as u8);
    let (sx, sy) = grid.sector_xy(owner);
    if north == 1 {
        [
            (owner, SIDE_N, start),
            (
                grid.sector_index(sx, sy + 1)
                    .expect("node implies neighbour"),
                SIDE_S,
                start,
            ),
        ]
    } else {
        [
            (owner, SIDE_E, start),
            (
                grid.sector_index(sx + 1, sy)
                    .expect("node implies neighbour"),
                SIDE_W,
                start,
            ),
        ]
    }
}

pub(crate) enum Route {
    /// Sectors the route passes through, in no particular order, possibly with repeats.
    Found(Vec<u32>),
    Unreachable,
    /// `max_nodes` expansions were not enough.
    Limit,
}

const GOAL_NODE: u32 = u32::MAX;

/// Search priority beyond g: the octile heuristic weighted 1.5, plus 2 per half
/// cell of distance from the straight line. The search only has to pick a
/// corridor (cell costs inside it are integrated exactly), so a modest detour
/// is a good trade for not flooding sideways through rough terrain, where
/// every expansion may derive a sector graph. The line term makes the corridor
/// track the straight line across open country, where octile costs tie over a
/// whole parallelogram of staircases.
#[inline]
fn weigh((dev, h): (u32, u32)) -> u32 {
    h + h / 2 + 2 * dev
}

/// A* over portals from `from` toward `goal`. Stops early on any node in
/// `known` (a node of an earlier route to the same goal) because the rest of
/// the way is already covered; nodes of a found route are added to `known`.
/// Ties break on (priority, h, node id), so the result is a pure function of the inputs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn find_route(
    grid: &NavGrid,
    cache: &GraphCache,
    layer: MoveLayer,
    size: SizeClass,
    from: Cell,
    goal: Cell,
    known: &mut IdMap<()>,
    max_nodes: u32,
    expanded: &mut u32,
) -> Route {
    let need = size.cells();
    let local = |c: Cell| (c.y % SECTOR_CELLS) as usize * SECTOR + (c.x % SECTOR_CELLS) as usize;
    let (from_sector, goal_sector) = (grid.sector_of(from), grid.sector_of(goal));
    let (gsx, gsy) = grid.sector_xy(goal_sector);
    let goal_costs = local_costs(grid.layer_sector(layer, gsx, gsy), need, local(goal));
    if from_sector == goal_sector && goal_costs[local(from)] != INF {
        return Route::Found(vec![goal_sector]);
    }
    let (fsx, fsy) = grid.sector_xy(from_sector);
    let from_costs = local_costs(grid.layer_sector(layer, fsx, fsy), need, local(from));

    // node -> (best g, parent)
    let mut best: IdMap<(u32, u32)> = IdMap::default();
    let mut heap: BinaryHeap<Reverse<(u32, u32, u32, u32)>> = BinaryHeap::new();
    let (lx, ly) = ((goal.x - from.x) as i64, (goal.y - from.y) as i64);
    let line_len = lx.abs().max(ly.abs()).max(1);
    // (distance from the line in half cells, h)
    let h_of = |sector: u32, p: Portal| {
        let (sx, sy) = grid.sector_xy(sector);
        let (ex, ey) = p.edge_point();
        let (x, y) = (2 * sx * SECTOR_CELLS + ex, 2 * sy * SECTOR_CELLS + ey);
        let cross = lx * (y - 2 * from.y) as i64 - ly * (x - 2 * from.x) as i64;
        (
            (cross.abs() / line_len) as u32,
            octile(x - 2 * goal.x, y - 2 * goal.y) / 2,
        )
    };
    // Sectors are revisited from several portals; skip the shared cache's lock and version check.
    let mut memo: IdMap<Arc<SectorGraph>> = IdMap::default();
    let mut graph_of = |sector: u32| {
        memo.entry(sector)
            .or_insert_with(|| cache.get(grid, layer, size, sector))
            .clone()
    };
    let start_graph = graph_of(from_sector);
    for &p in &start_graph.portals {
        let c = from_costs[p.center_index()].saturating_add(STRAIGHT / 2);
        if c != INF {
            let (node, h) = (node_id(grid, from_sector, p), h_of(from_sector, p));
            best.insert(node, (c, node));
            heap.push(Reverse((c + weigh(h), h.1, node, c)));
        }
    }
    while let Some(Reverse((_, _, node, g))) = heap.pop() {
        if g > best[&node].0 {
            continue;
        }
        if node == GOAL_NODE || known.contains_key(&node) {
            let mut sectors = vec![from_sector, goal_sector];
            let mut n = if node == GOAL_NODE {
                best[&node].1
            } else {
                node
            };
            loop {
                known.insert(n, ());
                sectors.extend(node_sides(grid, n).iter().map(|s| s.0));
                let parent = best[&n].1;
                if parent == n {
                    break;
                }
                n = parent;
            }
            return Route::Found(sectors);
        }
        *expanded += 1;
        if *expanded > max_nodes {
            return Route::Limit;
        }
        for (sector, side, start) in node_sides(grid, node) {
            let graph = graph_of(sector);
            let Some(me) = graph.find(side, start) else {
                continue;
            };
            if sector == goal_sector {
                let c = goal_costs[graph.portals[me].center_index()];
                if c != INF {
                    let total = g + STRAIGHT / 2 + c;
                    if best.get(&GOAL_NODE).is_none_or(|b| total < b.0) {
                        best.insert(GOAL_NODE, (total, node));
                        heap.push(Reverse((total, 0, GOAL_NODE, total)));
                    }
                }
            }
            let n = graph.portals.len();
            for (q, &portal) in graph.portals.iter().enumerate() {
                let c = graph.costs[me * n + q];
                if q == me || c == INF {
                    continue;
                }
                let (next, ng) = (node_id(grid, sector, portal), g + c);
                if best.get(&next).is_none_or(|b| ng < b.0) {
                    let nh = h_of(sector, portal);
                    best.insert(next, (ng, node));
                    heap.push(Reverse((ng + weigh(nh), nh.1, next, ng)));
                }
            }
        }
    }
    Route::Unreachable
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::*;
    use crate::CellRect;

    fn route(grid: &NavGrid, cache: &GraphCache, size: SizeClass, from: Cell, goal: Cell) -> Route {
        find_route(
            grid,
            cache,
            MoveLayer::Land,
            size,
            from,
            goal,
            &mut IdMap::default(),
            1 << 20,
            &mut 0,
        )
    }

    #[test]
    fn portals_split_at_walls_and_respect_size() {
        // A wall along x = 32 with a 2-cell gap at y = 10..12.
        let grid = NavGrid::from_fn(96, 96, |x, y| {
            if x == 32 && !(10..12).contains(&y) {
                LAND | STEEP
            } else {
                LAND
            }
        })
        .unwrap();
        let cache = GraphCache::new();
        let small = cache.get(&grid, MoveLayer::Land, SizeClass::SMALL, 0);
        let east: Vec<_> = small.portals.iter().filter(|p| p.side == SIDE_E).collect();
        assert_eq!(
            east,
            vec![&Portal {
                side: SIDE_E,
                start: 10,
                len: 2
            }]
        );
        let medium = cache.get(&grid, MoveLayer::Land, SizeClass::MEDIUM, 0);
        assert_eq!(
            medium.portals.iter().filter(|p| p.side == SIDE_E).count(),
            1
        );
        let large = cache.get(&grid, MoveLayer::Land, SizeClass::LARGE, 0);
        assert_eq!(large.portals.iter().filter(|p| p.side == SIDE_E).count(), 0);
        assert!(matches!(
            route(
                &grid,
                &cache,
                SizeClass::SMALL,
                Cell::new(5, 5),
                Cell::new(90, 5)
            ),
            Route::Found(_)
        ));
    }

    #[test]
    fn cache_revalidates_only_near_changes() {
        let mut grid = NavGrid::from_fn(
            320,
            320,
            |x, _| if x % 64 == 40 { LAND | STEEP } else { LAND },
        )
        .unwrap();
        let grid0 = grid.clone();
        let cache = GraphCache::new();
        for s in 0..100 {
            cache.get(&grid, MoveLayer::Land, SizeClass::SMALL, s);
        }
        let built = cache.built.load(Ordering::Relaxed);
        grid.block_rect(CellRect::new(Cell::new(150, 150), Cell::new(154, 154)))
            .unwrap();
        for s in 0..100 {
            cache.get(&grid, MoveLayer::Land, SizeClass::SMALL, s);
        }
        // Sector (4,4) changed: it and its four neighbours are re-derived, nothing else.
        assert_eq!(cache.built.load(Ordering::Relaxed) - built, 5);
        // An old snapshot never sees a graph derived from newer data.
        let g = cache.get(&grid0, MoveLayer::Land, SizeClass::SMALL, 44);
        assert_eq!(g.versions, [0; 5]);
    }

    #[test]
    fn search_finds_detours_and_dead_ends() {
        // Wall at x = 100 for y < 200; the only way round is over the top.
        let grid = NavGrid::from_fn(256, 256, |x, y| {
            if x == 100 && y < 200 {
                LAND | STEEP
            } else {
                LAND
            }
        })
        .unwrap();
        let cache = GraphCache::new();
        let Route::Found(sectors) = route(
            &grid,
            &cache,
            SizeClass::SMALL,
            Cell::new(10, 10),
            Cell::new(200, 10),
        ) else {
            panic!("route expected")
        };
        assert!(sectors.iter().any(|&s| grid.sector_xy(s).1 >= 6));
        // An island.
        let ring = NavGrid::from_fn(256, 256, |x, y| {
            if (x - 128).abs().max((y - 128).abs()) == 20 {
                DEEP
            } else {
                LAND
            }
        })
        .unwrap();
        assert!(matches!(
            route(
                &ring,
                &cache_for(&ring),
                SizeClass::SMALL,
                Cell::new(10, 10),
                Cell::new(128, 128)
            ),
            Route::Unreachable
        ));
        let mut n = 0;
        let r = find_route(
            &grid,
            &cache,
            MoveLayer::Land,
            SizeClass::SMALL,
            Cell::new(10, 10),
            Cell::new(200, 10),
            &mut IdMap::default(),
            3,
            &mut n,
        );
        assert!(matches!(r, Route::Limit));
    }

    fn cache_for(_: &NavGrid) -> GraphCache {
        GraphCache::new()
    }
}
