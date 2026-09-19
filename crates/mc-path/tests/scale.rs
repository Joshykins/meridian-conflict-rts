//! Full-size smoke test: an 80 km map, one request from corner to corner.
//! Its own test binary so a counting allocator can measure the real heap.

use mc_core::Fx;
use mc_path::terrain::*;
use mc_path::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

const MB: usize = 1 << 20;

/// Cheap integer terrain: a lattice of lakes with shallow rims, ridge lines
/// with passes, and rock fields over a third of the map.
fn terrain(x: i32, y: i32) -> u8 {
    let (tx, ty) = (x / 1024, y / 1024);
    let radius = 120 + ((tx * 7 + ty * 13) % 5) * 50;
    let (dx, dy) = (x % 1024 - 512, y % 1024 - 512);
    let r2 = dx * dx + dy * dy;
    if r2 < radius * radius {
        return DEEP;
    }
    if r2 < (radius + 12) * (radius + 12) {
        return SHALLOW;
    }
    let ridge = (x % 640 == 300 && y % 512 > 60) || (y % 768 == 400 && x % 512 > 90);
    let rocks = (tx + ty) % 3 == 0 && (x * 31 + y * 17) % 97 == 0;
    if ridge || rocks {
        LAND | STEEP
    } else {
        LAND
    }
}

#[test]
fn full_map_cross_request_stays_in_budget() {
    let t = Instant::now();
    let grid = NavGrid::from_fn(MAX_MAP_CELLS, MAX_MAP_CELLS, terrain).unwrap();
    let grid_ms = t.elapsed().as_millis();
    let grid_heap = LIVE.load(Ordering::Relaxed);
    println!("grid: {grid_ms} ms, heap {} MB (self-reported {} MB)", grid_heap / MB, grid.memory_bytes() / MB);
    // A dense byte per cell per layer would be 400 MB before any clearance data.
    assert!(grid_heap < 160 * MB);

    let mut nav = Nav::new(grid, NavConfig::default(), Arc::new(InlineSpawner));
    let (land, size) = (MoveLayer::Land, SizeClass::SMALL);
    let start = nav.nearest_passable(land, size, Cell::new(30, 40).center(), 32).unwrap().center();
    let goal = nav.nearest_passable(land, size, Cell::new(10_200, 10_190).center(), 32).unwrap().center();
    nav.begin_tick(0);
    let t = Instant::now();
    let id = nav.request(land, size, goal, &[start]).unwrap();
    let build_ms = t.elapsed().as_millis();
    let ready = nav.ready_tick(id).unwrap();
    let stats = nav.stats();
    println!("cross-map build: {build_ms} ms for a {ready}-tick ({} ms) deadline, graphs derived {}", ready * 100, stats.graphs_built);
    nav.begin_tick(ready);
    println!("field: {} tiles, {:?}", nav.field_tiles(id), nav.field_stats(id).unwrap());
    assert!(nav.field_tiles(id) < 4096);

    // Walk the whole 115 km. 7 m a tick keeps every step inside the neighbouring cells.
    let t = Instant::now();
    let (mut pos, mut tick, mut steps) = (start, ready, 0u32);
    loop {
        tick += 1;
        nav.begin_tick(tick);
        match nav.sample(id, pos) {
            Sample::Direction(d) => {
                pos += d * Fx::from_int(7);
                assert!(nav.is_passable(land, size, Cell::from_pos(pos)));
            }
            Sample::Arrived => break,
            Sample::NeedsExtend => nav.extend(id, pos).unwrap(),
            Sample::Pending => {}
            other => panic!("{other:?} at {:?}", Cell::from_pos(pos)),
        }
        steps += 1;
        assert!(steps < 40_000, "lost at {:?}", Cell::from_pos(pos));
    }
    println!("walk: {steps} ticks sampled in {} ms ({} extends)", t.elapsed().as_millis(), nav.stats().extends_scheduled);

    // A structure on the route: the tick-side cost is what the sim pays.
    let c = Cell::from_pos(Cell::new(5000, 5000).center());
    let site = nav.nearest_passable(land, SizeClass::HUGE, c.center(), 32).unwrap();
    let r = CellRect::new(Cell::new(site.x & !1, site.y & !1), Cell::new((site.x & !1) + 4, (site.y & !1) + 4));
    let t = Instant::now();
    nav.block_rect(r).unwrap();
    println!("block_rect incl. inline repair build: {} ms, repair due at tick {:?}, {:?}", t.elapsed().as_millis(), nav.ready_tick(id), nav.stats());

    let peak = PEAK.load(Ordering::Relaxed);
    println!("heap now {} MB, peak {} MB", LIVE.load(Ordering::Relaxed) / MB, peak / MB);
    assert!(peak < 256 * MB);
}
