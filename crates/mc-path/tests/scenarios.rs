//! End-to-end scenarios through the public API: a unit is a position stepped
//! along whatever `sample` says, exactly as the sim would drive it.

use mc_core::{Fx, FxVec2, StateHasher};
use mc_path::terrain::*;
use mc_path::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const SPEED: i32 = 5;

#[derive(Debug)]
struct Walk {
    end: Sample,
    /// Cells visited, consecutive duplicates removed.
    cells: Vec<Cell>,
    ticks: u64,
}

/// Steps one unit until it arrives, is told it cannot, or `max_ticks` pass.
/// Panics if the unit is ever steered onto a cell it cannot stand on.
fn walk(
    nav: &mut Nav,
    tick: &mut u64,
    id: FieldId,
    layer: MoveLayer,
    size: SizeClass,
    start: FxVec2,
    max_ticks: u64,
) -> Walk {
    let mut pos = start;
    let mut cells = vec![Cell::from_pos(pos)];
    let first = *tick;
    loop {
        *tick += 1;
        nav.begin_tick(*tick);
        let s = nav.sample(id, pos);
        match s {
            Sample::Direction(d) => {
                assert!(
                    (d.length() - Fx::ONE).abs() < Fx::ratio(1, 100),
                    "not a unit vector: {d:?}"
                );
                pos += d * Fx::from_int(SPEED);
                let c = Cell::from_pos(pos);
                assert!(
                    nav.is_passable(layer, size, c),
                    "steered onto impassable {c:?} at tick {tick}"
                );
                if *cells.last().unwrap() != c {
                    cells.push(c);
                }
            }
            Sample::NeedsExtend => nav.extend(id, pos).unwrap(),
            Sample::Pending => {}
            Sample::Arrived | Sample::Unreachable | Sample::Failed(_) => {
                return Walk {
                    end: s,
                    cells,
                    ticks: *tick - first,
                }
            }
        }
        assert!(
            *tick - first < max_ticks,
            "still walking after {max_ticks} ticks at {:?}",
            Cell::from_pos(pos)
        );
    }
}

fn nav_from(w: i32, h: i32, f: impl FnMut(i32, i32) -> u8) -> Nav {
    Nav::new(
        NavGrid::from_fn(w, h, f).unwrap(),
        NavConfig::default(),
        Arc::new(InlineSpawner),
    )
}

fn at(x: i32, y: i32) -> FxVec2 {
    Cell::new(x, y).center()
}

fn rect(x0: i32, y0: i32, x1: i32, y1: i32) -> CellRect {
    CellRect::new(Cell::new(x0, y0), Cell::new(x1, y1))
}

/// 2048 x 2048: a lake with a shallow rim in the middle, ridges with passes
/// across the land, a rock field in one corner.
fn continent(x: i32, y: i32) -> u8 {
    let (dx, dy) = ((x - 1024) as i64, (y - 1024) as i64);
    let r2 = dx * dx + dy * dy;
    if r2 < 380 * 380 {
        return DEEP;
    }
    if r2 < 400 * 400 {
        return SHALLOW;
    }
    let ridge_v = x % 500 == 250 && y % 700 > 80;
    let ridge_h = y % 600 == 300 && x % 900 > 120;
    let rocks = x < 600 && y > 1400 && (x * 31 + y * 17) % 53 == 0;
    if ridge_v || ridge_h || rocks {
        LAND | STEEP
    } else {
        LAND
    }
}

#[test]
fn reaches_goal_around_obstacles_on_a_small_grid() {
    // Two staggered walls make an S-bend.
    let mut nav = nav_from(64, 64, |x, y| {
        if (x == 20 && y < 50) || (x == 40 && y > 14) {
            0
        } else {
            LAND
        }
    });
    let mut tick = 0;
    nav.begin_tick(tick);
    let start = at(4, 4);
    let id = nav
        .request(MoveLayer::Land, SizeClass::SMALL, at(60, 60), &[start])
        .unwrap();
    let w = walk(
        &mut nav,
        &mut tick,
        id,
        MoveLayer::Land,
        SizeClass::SMALL,
        start,
        2000,
    );
    assert_eq!(w.end, Sample::Arrived);
    assert!(w.cells.iter().any(|c| c.x == 20 && c.y >= 50));
    assert!(w.cells.iter().any(|c| c.x == 40 && c.y <= 14));
}

#[test]
fn crosses_many_sectors_and_layers_respect_water() {
    let mut nav = nav_from(2048, 2048, continent);
    let mut tick = 0;
    nav.begin_tick(tick);
    let (start, goal) = (at(40, 1000), at(2000, 1040));
    let water = |nav: &Nav, c: &Cell| nav.grid().terrain_class(*c) & (SHALLOW | DEEP) != 0;

    let land = nav
        .request(MoveLayer::Land, SizeClass::SMALL, goal, &[start])
        .unwrap();
    let w = walk(
        &mut nav,
        &mut tick,
        land,
        MoveLayer::Land,
        SizeClass::SMALL,
        start,
        20_000,
    );
    assert_eq!(w.end, Sample::Arrived);
    assert!(!w.cells.iter().any(|c| water(&nav, c)), "land unit got wet");
    let land_ticks = w.ticks;

    let amph = nav
        .request(MoveLayer::Amphibious, SizeClass::SMALL, goal, &[start])
        .unwrap();
    let w = walk(
        &mut nav,
        &mut tick,
        amph,
        MoveLayer::Amphibious,
        SizeClass::SMALL,
        start,
        20_000,
    );
    assert_eq!(w.end, Sample::Arrived);
    assert!(
        w.cells.iter().any(|c| water(&nav, c)),
        "amphibious unit walked round the lake"
    );
    assert!(w.ticks < land_ticks);

    let hover = nav
        .request(MoveLayer::Hover, SizeClass::MEDIUM, goal, &[start])
        .unwrap();
    let w = walk(
        &mut nav,
        &mut tick,
        hover,
        MoveLayer::Hover,
        SizeClass::MEDIUM,
        start,
        20_000,
    );
    assert_eq!(w.end, Sample::Arrived);

    let (dock, across) = (at(1024 - 300, 1024), at(1024 + 250, 1024 + 150));
    let naval = nav
        .request(MoveLayer::Naval, SizeClass::LARGE, across, &[dock])
        .unwrap();
    let w = walk(
        &mut nav,
        &mut tick,
        naval,
        MoveLayer::Naval,
        SizeClass::LARGE,
        dock,
        20_000,
    );
    assert_eq!(w.end, Sample::Arrived);
    assert!(w.cells.iter().all(|c| nav.grid().terrain_class(*c) == DEEP));
    assert_eq!(
        nav.request(MoveLayer::Naval, SizeClass::SMALL, goal, &[dock]),
        Err(PathError::GoalImpassable)
    );
    // Fields are corridors, not maps: far fewer tiles than the 4096 sectors.
    assert!(
        nav.field_tiles(land) < 600,
        "{} tiles",
        nav.field_tiles(land)
    );
}

#[test]
fn big_units_refuse_a_gap_small_ones_take() {
    // Wall at x = 100 with a 2-cell gap at y = 40 and a wide opening at the far top.
    let wall = |x: i32, y: i32| {
        if x == 100 && !(40..42).contains(&y) && y < 230 {
            0
        } else {
            LAND
        }
    };
    let mut nav = nav_from(256, 256, wall);
    let mut tick = 0;
    nav.begin_tick(tick);
    let (start, goal) = (at(60, 40), at(140, 40));
    let through_gap = |w: &Walk| {
        w.cells
            .iter()
            .any(|c| c.x == 100 && (40..42).contains(&c.y))
    };
    for (size, expect_gap) in [
        (SizeClass::SMALL, true),
        (SizeClass::MEDIUM, true),
        (SizeClass::LARGE, false),
        (SizeClass::HUGE, false),
    ] {
        let id = nav.request(MoveLayer::Land, size, goal, &[start]).unwrap();
        let w = walk(&mut nav, &mut tick, id, MoveLayer::Land, size, start, 5000);
        assert_eq!(w.end, Sample::Arrived, "{size:?}");
        assert_eq!(through_gap(&w), expect_gap, "{size:?}");
        nav.release(id).unwrap();
    }
}

#[test]
fn island_goal_is_unreachable_by_land() {
    let moat = |x: i32, y: i32| {
        if ((x - 128).abs().max((y - 128).abs())) / 4 == 5 {
            DEEP
        } else {
            LAND
        }
    };
    let mut nav = nav_from(256, 256, moat);
    let mut tick = 0;
    nav.begin_tick(tick);
    let start = at(10, 10);
    let id = nav
        .request(MoveLayer::Land, SizeClass::SMALL, at(128, 128), &[start])
        .unwrap();
    let w = walk(
        &mut nav,
        &mut tick,
        id,
        MoveLayer::Land,
        SizeClass::SMALL,
        start,
        100,
    );
    assert_eq!(w.end, Sample::Unreachable);
    // Someone already on the island is fine, and hovercraft do not care.
    assert!(matches!(nav.sample(id, at(120, 130)), Sample::Direction(_)));
    let hover = nav
        .request(MoveLayer::Hover, SizeClass::SMALL, at(128, 128), &[start])
        .unwrap();
    assert_eq!(
        walk(
            &mut nav,
            &mut tick,
            hover,
            MoveLayer::Hover,
            SizeClass::SMALL,
            start,
            5000
        )
        .end,
        Sample::Arrived
    );
}

#[test]
fn blocking_the_only_pass_reroutes_after_repair() {
    // Wall at x = 512 with a pass at y = 96..104 and another far away at y = 900..908.
    let wall = |x: i32, y: i32| {
        if x == 512 && !(96..104).contains(&y) && !(900..908).contains(&y) {
            0
        } else {
            LAND
        }
    };
    let mut nav = nav_from(1024, 1024, wall);
    let mut tick = 0;
    nav.begin_tick(tick);
    let (start, goal) = (at(300, 100), at(700, 130));
    let id = nav
        .request(MoveLayer::Land, SizeClass::SMALL, goal, &[start])
        .unwrap();
    let w = walk(
        &mut nav,
        &mut tick,
        id,
        MoveLayer::Land,
        SizeClass::SMALL,
        start,
        5000,
    );
    assert_eq!(w.end, Sample::Arrived);
    assert!(w
        .cells
        .iter()
        .any(|c| c.x == 512 && (96..104).contains(&c.y)));

    let before = nav.stats();
    let recomputed = nav.grid().sectors_recomputed();
    nav.block_rect(rect(510, 94, 516, 106)).unwrap();
    // The synchronous part touched the 2 x 2 sectors the structure straddles (x4 layers).
    assert_eq!(nav.grid().sectors_recomputed() - recomputed, 16);
    assert_eq!(nav.stats().repairs_scheduled, before.repairs_scheduled + 1);
    let w = walk(
        &mut nav,
        &mut tick,
        id,
        MoveLayer::Land,
        SizeClass::SMALL,
        start,
        20_000,
    );
    assert_eq!(w.end, Sample::Arrived);
    assert!(w
        .cells
        .iter()
        .any(|c| c.x == 512 && (900..908).contains(&c.y)));
    // A different corridor altogether, so nearly every tile is new; what is
    // shared is the goal's own tile. (Locality has its own test below.)
    let s = nav.field_stats(id).unwrap();
    assert!(s.tiles_reused >= 1, "{s:?}");

    // Opening the pass again brings the short way back.
    nav.unblock_rect(rect(510, 94, 516, 106)).unwrap();
    let w = walk(
        &mut nav,
        &mut tick,
        id,
        MoveLayer::Land,
        SizeClass::SMALL,
        start,
        5000,
    );
    assert!(w
        .cells
        .iter()
        .any(|c| c.x == 512 && (96..104).contains(&c.y)));
}

#[test]
fn a_structure_in_the_open_rebuilds_only_nearby_tiles() {
    let mut nav = nav_from(2048, 2048, |_, _| LAND);
    let mut tick = 0;
    nav.begin_tick(tick);
    let (start, goal) = (at(100, 100), at(1900, 1500));
    let id = nav
        .request(MoveLayer::Land, SizeClass::SMALL, goal, &[start])
        .unwrap();
    tick = nav.ready_tick(id).unwrap();
    nav.begin_tick(tick);
    let tiles = nav.field_tiles(id);
    assert!(tiles > 100);
    let graphs = nav.stats().graphs_built;

    // Mid-corridor, in the middle of sector (31, 24).
    assert!(matches!(
        nav.sample(id, at(1000, 770)),
        Sample::Direction(_)
    ));
    nav.block_rect(rect(1004, 780, 1008, 784)).unwrap();
    tick = nav.ready_tick(id).expect("repair scheduled");
    nav.begin_tick(tick);
    let s = nav.field_stats(id).unwrap();
    // Only the tiles around the structure and the few behind it whose incoming
    // costs really moved are integrated again; over 90 % are shared as they were.
    assert!(
        s.tiles_built >= 1 && s.tiles_built <= 32 && s.tiles_reused * 10 >= tiles as u32 * 9,
        "{s:?}"
    );
    assert_eq!(s.tiles_built + s.tiles_reused, tiles as u32);
    // Portals and abstract edges: the touched sector and its four neighbours at most.
    assert!(nav.stats().graphs_built - graphs <= 5);
    let w = walk(
        &mut nav,
        &mut tick,
        id,
        MoveLayer::Land,
        SizeClass::SMALL,
        at(990, 775),
        20_000,
    );
    assert_eq!(w.end, Sample::Arrived);

    // A structure nowhere near the corridor schedules nothing.
    nav.block_rect(rect(100, 1800, 104, 1804)).unwrap();
    assert_eq!(nav.ready_tick(id), None);
}

/// Threads that start late and finish in a scrambled order.
struct JitterSpawner {
    state: AtomicU64,
}

impl Spawner for JitterSpawner {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        let s = self
            .state
            .fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
        let ms = ((s ^ (s >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9) >> 40) % 8;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            task();
        });
    }
}

/// (nav hash, ids handed out by this tick's requests, every unit's sample)
type TickLog = (u64, Vec<u64>, Vec<Sample>);

fn script_terrain(x: i32, y: i32) -> u8 {
    let lake = (x - 300) * (x - 300) + (y - 200) * (y - 200) < 60 * 60;
    let ridge = x % 128 == 70 && y % 160 > 30;
    if lake {
        DEEP
    } else if ridge || (x * 13 + y * 7) % 61 == 0 {
        LAND | STEEP
    } else {
        LAND
    }
}

const SCRIPT_TICKS: u64 = 700;

/// A field table this small forces evictions, so slots are recycled and
/// generations move; the tile cap makes the two map-wide requests fail.
fn script_config() -> NavConfig {
    NavConfig {
        max_fields: 6,
        max_tiles_per_field: 150,
        ..NavConfig::default()
    }
}

const SCRIPT_GOALS: [(i32, i32); 4] = [(480, 470), (30, 480), (470, 40), (256, 300)];

/// A fixed script of requests, moves, extends, blocks and releases. `units`
/// and `ids` stand in for the sim's own tables: a snapshot copies them as
/// plain data, field ids as bits.
struct Script {
    nav: Nav,
    units: Vec<(FxVec2, usize)>,
    ids: Vec<Option<FieldId>>,
    extra: Vec<FieldId>,
    built: Vec<CellRect>,
}

impl Script {
    fn new(spawner: Arc<dyn Spawner>) -> Script {
        let nav = Nav::new(
            NavGrid::from_fn(512, 512, script_terrain).unwrap(),
            script_config(),
            spawner,
        );
        let units = (0..24)
            .map(|i| {
                let want = at(20 + (i * 37) % 200, 20 + (i * 53) % 150);
                (
                    nav.nearest_passable(MoveLayer::Land, SizeClass::SMALL, want, 16)
                        .unwrap()
                        .center(),
                    i as usize % SCRIPT_GOALS.len(),
                )
            })
            .collect();
        Script {
            nav,
            units,
            ids: vec![None; SCRIPT_GOALS.len()],
            extra: Vec::new(),
            built: Vec::new(),
        }
    }

    /// What a joining client does: game tables copied, `Nav` rebuilt from the blob over a fresh base grid.
    fn restore(&self, blob: &[u8], spawner: Arc<dyn Spawner>) -> Script {
        let base = NavGrid::from_fn(512, 512, script_terrain).unwrap();
        let nav = Nav::import_state(base, script_config(), spawner, blob).unwrap();
        let ids = self
            .ids
            .iter()
            .map(|id| id.map(|id| FieldId::from_bits(id.to_bits())))
            .collect();
        Script {
            nav,
            units: self.units.clone(),
            ids,
            extra: self
                .extra
                .iter()
                .map(|id| FieldId::from_bits(id.to_bits()))
                .collect(),
            built: self.built.clone(),
        }
    }

    /// One sim tick. Returns everything the sim could observe.
    fn step(&mut self, tick: u64) -> TickLog {
        let (nav, land) = (&mut self.nav, MoveLayer::Land);
        nav.begin_tick(tick);
        let mut handed_out = Vec::new();
        for (g, &(gx, gy)) in SCRIPT_GOALS.iter().enumerate() {
            // Goal 3 is requested again long after its release, to land on the cached field or a recycled slot.
            if tick == 3 * g as u64 || (g == 3 && tick == 420) {
                let goal = nav
                    .nearest_passable(land, SizeClass::SMALL, at(gx, gy), 16)
                    .unwrap()
                    .center();
                let from: Vec<FxVec2> = self
                    .units
                    .iter()
                    .filter(|u| u.1 == g)
                    .map(|u| u.0)
                    .collect();
                self.ids[g] = Some(nav.request(land, SizeClass::SMALL, goal, &from).unwrap());
                handed_out.push(self.ids[g].unwrap().to_bits());
            }
        }
        // A second group joins field 1 from far away while its first build is
        // in flight: the new anchor has to wait in the queue.
        if tick == 4 {
            let from = nav
                .nearest_passable(land, SizeClass::SMALL, at(440, 120), 16)
                .unwrap()
                .center();
            let goal = nav
                .nearest_passable(
                    land,
                    SizeClass::SMALL,
                    at(SCRIPT_GOALS[1].0, SCRIPT_GOALS[1].1),
                    16,
                )
                .unwrap()
                .center();
            assert_eq!(
                nav.request(land, SizeClass::SMALL, goal, &[from]).unwrap(),
                self.ids[1].unwrap()
            );
        }
        if tick == 30 {
            nav.release(self.ids[1].unwrap()).unwrap();
        }
        // Two requests from all over the map blow the per-field tile cap. Their
        // builds fail, and releasing them leaves two free slots whose order
        // decides which ids come next.
        if tick == 20 || tick == 21 {
            let from: Vec<FxVec2> = (0..40)
                .filter_map(|i| {
                    nav.nearest_passable(
                        MoveLayer::Hover,
                        SizeClass::SMALL,
                        at(30 + (i % 8) * 60, 30 + (i / 8) * 100),
                        8,
                    )
                })
                .map(|c| c.center())
                .collect();
            self.extra.push(
                nav.request(
                    MoveLayer::Hover,
                    SizeClass::SMALL,
                    at(250 + tick as i32, 250),
                    &from,
                )
                .unwrap(),
            );
            handed_out.push(self.extra.last().unwrap().to_bits());
        }
        if tick == 40 || tick == 41 {
            let id = self.extra.remove(0);
            assert_eq!(
                nav.sample(id, at(30, 30)),
                Sample::Failed(PathError::CorridorTooLarge)
            );
            nav.release(id).unwrap();
        }
        // Structures go up in the units' way, some while builds are in flight.
        if (4..200).contains(&tick) && tick % 9 == 4 {
            let k = (tick / 9) as i32;
            let (x, y) = (80 + (k * 46) % 360, 60 + (k * 78) % 380);
            let r = rect(x & !1, y & !1, (x & !1) + 6, (y & !1) + 4);
            if nav.can_place(r, land) {
                nav.block_rect(r).unwrap();
            }
        }
        // And one right in front of a unit, so it lands on tiles of that unit's
        // field; at tick 4 that field's first build is still in flight.
        if (4..200).contains(&tick) && tick % 9 == 4 {
            let (pos, g) = self.units[(tick as usize / 9 + 1) % self.units.len()];
            let ahead = Cell::from_pos(
                pos + (at(SCRIPT_GOALS[g].0, SCRIPT_GOALS[g].1) - pos).normalize()
                    * Fx::from_int(48),
            );
            let r = rect(
                ahead.x & !1,
                ahead.y & !1,
                (ahead.x & !1) + 2,
                (ahead.y & !1) + 2,
            );
            if nav.can_place(r, land) {
                nav.block_rect(r).unwrap();
                self.built.push(r);
            }
        }
        if tick == 120 {
            nav.unblock_rect(rect(80, 60, 440, 440)).unwrap();
            for r in self.built.drain(..) {
                nav.unblock_rect(r).unwrap();
            }
        }
        if tick == 150 {
            nav.release(self.ids[3].take().unwrap()).unwrap();
        }
        // Three more goals: two take the free slots, the third does not fit, so
        // the cached field is evicted and its slot reused under a new
        // generation. The re-request at 420 then evicts one of these.
        if tick == 160 {
            for i in 0..3 {
                let goal = nav
                    .nearest_passable(land, SizeClass::MEDIUM, at(200 + 40 * i, 250), 16)
                    .unwrap()
                    .center();
                self.extra.push(
                    nav.request(land, SizeClass::MEDIUM, goal, &[self.units[0].0])
                        .unwrap(),
                );
                handed_out.push(self.extra.last().unwrap().to_bits());
            }
            assert_eq!(nav.stats().fields_evicted, 1);
        }
        if tick == 200 {
            for id in self.extra.drain(..) {
                nav.release(id).unwrap();
            }
        }
        let mut samples = Vec::new();
        for (pos, g) in self.units.iter_mut() {
            let Some(id) = self.ids[*g] else { continue };
            let s = nav.sample(id, *pos);
            match s {
                Sample::Direction(d) => *pos += d * Fx::from_int(7),
                Sample::NeedsExtend => nav.extend(id, *pos).unwrap(),
                _ => {}
            }
            samples.push(s);
        }
        let mut h = StateHasher::new();
        nav.hash(&mut h);
        (h.finish(), handed_out, samples)
    }
}

fn scripted_run(spawner: Arc<dyn Spawner>) -> Vec<TickLog> {
    let mut script = Script::new(spawner);
    let log: Vec<TickLog> = (0..SCRIPT_TICKS).map(|t| script.step(t)).collect();
    let arrived = log
        .last()
        .unwrap()
        .2
        .iter()
        .filter(|s| **s == Sample::Arrived)
        .count();
    assert!(arrived >= 12, "only {arrived} units arrived");
    assert!(script.nav.stats().repairs_scheduled > 3);
    log
}

#[test]
fn observations_do_not_depend_on_threads_or_timing() {
    let inline = scripted_run(Arc::new(InlineSpawner));
    let threads = scripted_run(Arc::new(ThreadSpawner));
    let jitter_a = scripted_run(Arc::new(JitterSpawner {
        state: AtomicU64::new(1),
    }));
    let jitter_b = scripted_run(Arc::new(JitterSpawner {
        state: AtomicU64::new(0xDEAD_BEEF),
    }));
    for (name, other) in [
        ("threads", &threads),
        ("jitter a", &jitter_a),
        ("jitter b", &jitter_b),
    ] {
        for (tick, (a, b)) in inline.iter().zip(other.iter()).enumerate() {
            assert_eq!(a, b, "{name} diverged from inline at tick {tick}");
        }
    }
}

/// Ticks after which a snapshot is taken: first builds in flight (1), one
/// with a block and a queued anchor recorded against it (4), a repair in
/// flight (13, 14), failing builds in flight (21), two free slots (41), mid-game, just after the mass unblock (121), a
/// released field sitting in the cache (151), a recycled slot (161), late game,
/// after the re-request that recycles another (421).
const EXPORT_AFTER: [u64; 12] = [1, 4, 13, 14, 21, 41, 60, 121, 151, 161, 300, 421];

type MakeSpawner = fn(u64) -> Arc<dyn Spawner>;

#[test]
fn a_snapshot_restores_to_the_same_future() {
    let baseline = scripted_run(Arc::new(InlineSpawner));
    let spawners: [(&str, MakeSpawner); 2] = [
        ("inline", |_| Arc::new(InlineSpawner)),
        ("jitter", |seed| {
            Arc::new(JitterSpawner {
                state: AtomicU64::new(seed),
            })
        }),
    ];
    for (exporter_kind, exporter_spawner) in spawners {
        let mut exporter = Script::new(exporter_spawner(7));
        let mut joiners: Vec<(u64, &str, Script)> = Vec::new();
        let (mut saw_in_flight, mut saw_cached) = (false, false);
        for tick in 0..SCRIPT_TICKS {
            // The exporter keeps playing; it must never notice that it exported.
            assert_eq!(
                exporter.step(tick),
                baseline[tick as usize],
                "{exporter_kind} exporter diverged at tick {tick}"
            );
            for (joined, kind, joiner) in joiners.iter_mut() {
                assert_eq!(joiner.step(tick), baseline[tick as usize], "{kind} joiner from tick {joined} ({exporter_kind} exporter) diverged at tick {tick}");
            }
            if EXPORT_AFTER.contains(&tick) {
                saw_in_flight |= exporter
                    .ids
                    .iter()
                    .flatten()
                    .any(|&id| exporter.nav.ready_tick(id).is_some());
                saw_cached |=
                    exporter.ids.iter().flatten().count() < exporter.nav.stats().live_fields;
                let blob = exporter.nav.export_state();
                assert_eq!(
                    blob,
                    exporter.nav.export_state(),
                    "export is not repeatable"
                );
                // Inline joiners for the threaded exporter and the other way round.
                for (kind, spawner) in spawners.into_iter().filter(|s| s.0 != exporter_kind) {
                    let joiner = exporter.restore(&blob, spawner(tick));
                    assert_eq!(
                        joiner.nav.stats().total_tiles,
                        exporter.nav.stats().total_tiles
                    );
                    // A snapshot of the restored state is the same snapshot.
                    let mut copy = exporter.restore(&blob, spawner(tick));
                    assert_eq!(copy.nav.export_state(), blob);
                    joiners.push((tick, kind, joiner));
                }
            }
        }
        assert!(saw_in_flight && saw_cached);
        for (_, _, joiner) in &joiners {
            let (mut a, mut b) = (joiner.nav.stats(), exporter.nav.stats());
            (a.late_joins, a.graphs_built, b.late_joins, b.graphs_built) = (0, 0, 0, 0);
            assert_eq!(a, b);
        }
    }
}

#[test]
fn damaged_snapshots_are_rejected_without_panicking() {
    let mut script = Script::new(Arc::new(InlineSpawner));
    for tick in 0..14 {
        script.step(tick);
    }
    let blob = script.nav.export_state();
    let import = |bytes: &[u8], w: i32, cfg: NavConfig| {
        Nav::import_state(
            NavGrid::from_fn(w, w, script_terrain).unwrap(),
            cfg,
            Arc::new(InlineSpawner),
            bytes,
        )
        .map(|_| ())
    };
    assert_eq!(import(&blob, 512, script_config()), Ok(()));
    // Wrong map, wrong config, trailing bytes, wrong magic.
    assert_eq!(
        import(&blob, 256, script_config()),
        Err(PathError::BadSnapshot)
    );
    assert_eq!(
        import(
            &blob,
            512,
            NavConfig {
                base_latency: 3,
                ..script_config()
            }
        ),
        Err(PathError::BadSnapshot)
    );
    let mut longer = blob.clone();
    longer.push(0);
    assert_eq!(
        import(&longer, 512, script_config()),
        Err(PathError::BadSnapshot)
    );
    assert_eq!(
        import(&blob[4..], 512, script_config()),
        Err(PathError::BadSnapshot)
    );
    // Every truncation fails cleanly.
    let base = NavGrid::from_fn(512, 512, script_terrain).unwrap();
    let try_import = |bytes: &[u8]| {
        Nav::import_state(
            base.clone(),
            script_config(),
            Arc::new(InlineSpawner),
            bytes,
        )
    };
    let stride = blob.len() / 400 + 1;
    for cut in (0..blob.len())
        .step_by(stride)
        .chain(blob.len().saturating_sub(64)..blob.len())
    {
        assert_eq!(
            try_import(&blob[..cut]).err(),
            Some(PathError::BadSnapshot),
            "cut at {cut}"
        );
    }
    // Corruption may still decode to something well-formed, but must never panic,
    // and whatever it decodes to must survive being used.
    let mut state = 0x1234_5678_9ABC_DEFFu64;
    for _ in 0..150 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let mut bad = blob.clone();
        // Bias toward the header and tables, where structure lives; tile payload is mostly free-form bytes.
        let at = if state & 1 == 0 {
            (state >> 20) as usize % bad.len().min(4096)
        } else {
            (state >> 20) as usize % bad.len()
        };
        bad[at] ^= 1 << ((state >> 8) % 8);
        if let Ok(mut nav) = try_import(&bad) {
            for tick in 14..17 {
                nav.begin_tick(tick);
                for id in script.ids.iter().flatten() {
                    for unit in &script.units {
                        if let Sample::NeedsExtend = nav.sample(*id, unit.0) {
                            let _ = nav.extend(*id, unit.0);
                        }
                    }
                }
            }
        }
    }
}
