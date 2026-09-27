use super::*;
use crate::file::MapFile;
use crate::heightfield::Heightfield;
use crate::test_util::{baked_4km, baked_islands, temp_path};
use crate::MAX_MAP_TILES;

/// Mean corner of every ore field.
fn ore_centres(map: &MapFile) -> Vec<FxVec2> {
    map.ore_regions()
        .iter()
        .map(|r| {
            let n = r.points.len() as i32;
            let sum = r.points.iter().fold(FxVec2::ZERO, |a, p| a + *p);
            FxVec2::new(sum.x / n, sum.y / n)
        })
        .collect()
}

#[test]
fn ore_fields_are_organic_and_mostly_dry() {
    let map = MapFile::open(baked_4km()).unwrap();
    let hf = Heightfield::load(&map).unwrap();
    for region in map.ore_regions() {
        assert_eq!(region.points.len(), ORE_CORNERS);
        let (lo, hi) = region.bounds();
        assert!(
            (hi - lo).length() > Fx::from_int(80),
            "{:?} is tiny",
            (lo, hi)
        );
        // Sample the inside: dry, and no tree or rock on it.
        let (mut inside, mut dry) = (0, 0);
        let mut y = lo.y;
        while y < hi.y {
            let mut x = lo.x;
            while x < hi.x {
                let p = FxVec2::new(x, y);
                if region.contains(p) {
                    inside += 1;
                    dry += (hf.height_at(p) > hf.water_level()) as i32;
                }
                x += Fx::from_int(8);
            }
            y += Fx::from_int(8);
        }
        assert!(
            inside > 50 && dry * 10 >= inside * 9,
            "{dry} of {inside} dry"
        );
        // Towns keep their buildings off a field; woods may grow over it.
        assert!(!map
            .props()
            .iter()
            .any(|p| p.kind.is_building() && region.contains(p.pos)));
    }
}

#[test]
fn same_seed_same_map_any_thread_count() {
    let (a, b, c) = (temp_path("det-a"), temp_path("det-b"), temp_path("det-c"));
    let params = BakeParams::square("Determinism", 2, 99);
    let first = bake(
        &BakeParams {
            threads: 1,
            ..params.clone()
        },
        &a,
    )
    .unwrap();
    let second = bake(
        &BakeParams {
            threads: 5,
            ..params.clone()
        },
        &b,
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    let other = bake(
        &BakeParams {
            seed: 100,
            ..params
        },
        &c,
    )
    .unwrap();
    assert_ne!(first.content_id, other.content_id);
    for p in [a, b, c] {
        std::fs::remove_file(p).ok();
    }
}

#[test]
fn baked_map_round_trips_and_verifies() {
    let map = MapFile::open(baked_4km()).unwrap();
    assert_eq!(map.name(), "Test Basin");
    assert_eq!(map.size_tiles(), (2, 2));
    assert_eq!(map.info().min_z, DEFAULT_MIN_Z);
    assert_eq!(map.info().z_step, DEFAULT_Z_STEP);
    assert_eq!(map.overview_dims(), (129, 129));
    map.verify().unwrap();

    // The overview is every fourth sample of the full-resolution data.
    let hf = Heightfield::load(&map).unwrap();
    for (ox, oy) in [(0, 0), (128, 128), (64, 17), (63, 64), (5, 99)] {
        assert_eq!(
            map.overview()[(oy * 129 + ox) as usize],
            hf.sample(ox * 4, oy * 4)
        );
    }
    // Props come back grouped by tile, and on their tile.
    let mut seen = 0;
    for ty in 0..2 {
        for tx in 0..2 {
            let props = map.props_in_tile(tx, ty).unwrap();
            assert!(props.iter().all(|p| map.info().tile_of(p.pos) == (tx, ty)));
            seen += props.len();
        }
    }
    assert_eq!(seen, map.props().len());
}

#[test]
fn starts_are_level_dry_and_symmetric() {
    let map = MapFile::open(baked_4km()).unwrap();
    let hf = Heightfield::load(&map).unwrap();
    let starts = map.start_positions();
    assert_eq!(starts.len(), 2);
    let centre = FxVec2::from_ints(2048, 2048);
    assert!((starts[0] + starts[1] - centre - centre).length() <= Fx::from_int(BUILD_CELL_M));
    for &s in starts {
        let z = hf.height_at(s);
        assert!(z > hf.water_level() + Fx::from_int(5));
        for (dx, dy) in [
            (-150, 0),
            (150, 0),
            (0, -150),
            (0, 150),
            (100, 100),
            (-100, 100),
        ] {
            let p = s + FxVec2::from_ints(dx, dy);
            assert!(
                (hf.height_at(p) - z).abs() < Fx::ratio(1, 4),
                "start pad is not level at {p:?}"
            );
            assert!(hf.slope_at(p) < Fx::ratio(1, 50));
        }
    }
    // Mirrored terrain: opposite points are equally high (to within detail
    // lost to the sample grid).
    for (x, y) in [(300, 700), (1500, 1900), (2222, 3333), (4000, 100)] {
        let (a, b) = (
            FxVec2::from_ints(x, y),
            FxVec2::from_ints(4096 - x, 4096 - y),
        );
        assert!((hf.height_at(a) - hf.height_at(b)).abs() < Fx::ratio(1, 10));
    }
}

#[test]
fn ore_fields_and_props_respect_the_rules() {
    let map = MapFile::open(baked_4km()).unwrap();
    let hf = Heightfield::load(&map).unwrap();

    let deposits = &ore_centres(&map);
    assert!(deposits.len() >= 2 * 3 + 4);
    for d in deposits {
        assert!(hf.in_bounds(*d));
        assert!(hf.height_at(*d) > hf.water_level());
        assert!(hf.slope_at(*d) < Fx::ratio(1, 5));
    }
    for s in map.start_positions() {
        let close: Vec<FxVec2> = deposits
            .iter()
            .copied()
            .filter(|d| d.distance(*s) < Fx::from_int(260))
            .collect();
        assert_eq!(close.len(), 3);
        for (i, a) in close.iter().enumerate() {
            for b in &close[i + 1..] {
                assert!(
                    a.distance(*b) > Fx::from_int(200),
                    "start deposits {a:?} and {b:?} crowd each other"
                );
            }
        }
    }

    let props = map.props();
    let trees = props.iter().filter(|p| p.kind.is_tree()).count();
    let buildings = props.iter().filter(|p| p.kind.is_building()).count();
    assert!(trees > 100, "{trees} trees");
    assert!(buildings > 10, "{buildings} buildings");
    for p in props {
        assert!(hf.in_bounds(p.pos));
        assert!((500..=1500).contains(&p.scale_milli));
        if p.kind.is_tree() {
            assert!(hf.height_at(p.pos) > hf.water_level());
            assert!(
                hf.slope_at(p.pos) < Fx::ratio(3, 5),
                "tree on a cliff at {:?}",
                p.pos
            );
            // Starts sit in ragged glades: the woods may come in close on
            // some bearings, never onto the base.
            assert!(map
                .start_positions()
                .iter()
                .all(|s| s.distance(p.pos) > Fx::from_int(120)));
        }
        if p.kind.is_building() {
            assert!(
                hf.slope_at(p.pos) < Fx::ratio(1, 10),
                "building on a slope at {:?}",
                p.pos
            );
        }
    }
}

#[test]
fn islands_bake_is_deterministic_and_leaves_the_basin_alone() {
    let (a, b, c) = (temp_path("isl-a"), temp_path("isl-b"), temp_path("isl-c"));
    let params = BakeParams::islands("Determinism", 3, 99);
    assert_eq!((params.players, params.layout), (2, Layout::Islands));
    let first = bake(
        &BakeParams {
            threads: 1,
            ..params.clone()
        },
        &a,
    )
    .unwrap();
    let second = bake(
        &BakeParams {
            threads: 5,
            ..params.clone()
        },
        &b,
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    let basin = bake(
        &BakeParams {
            layout: Layout::Basin,
            ..params
        },
        &c,
    )
    .unwrap();
    assert_ne!(first.content_id, basin.content_id);
    // `square` is the basin, and the layout is part of neither the file nor its defaults.
    assert_eq!(BakeParams::square("x", 3, 99).layout, Layout::default());
    assert_eq!(
        basin,
        bake(&BakeParams::square("Determinism", 3, 99), &c).unwrap()
    );
    for p in [a, b, c] {
        std::fs::remove_file(p).ok();
    }
}

#[test]
fn islands_have_level_starts_a_lake_towns_and_open_sea() {
    let map = MapFile::open(baked_islands()).unwrap();
    let hf = Heightfield::load(&map).unwrap();
    let (water, size) = (hf.water_level(), 4 * TILE_SIZE_M);
    let centre = FxVec2::from_ints(size / 2, size / 2);

    // Both starts on the main island: level, dry, mirrored, about 0.6 of the map apart.
    let starts = map.start_positions();
    assert_eq!(starts.len(), 2);
    assert!((starts[0] + starts[1] - centre - centre).length() <= Fx::from_int(BUILD_CELL_M));
    let apart = starts[0].distance(starts[1]).to_f64() / size as f64;
    assert!(
        (0.55..0.65).contains(&apart),
        "starts are {apart} of the map apart"
    );
    for &s in starts {
        let z = hf.height_at(s);
        assert!(z > water + Fx::from_int(5));
        for (dx, dy) in [
            (-150, 0),
            (150, 0),
            (0, -150),
            (0, 150),
            (100, 100),
            (-100, 100),
        ] {
            let p = s + FxVec2::from_ints(dx, dy);
            assert!(
                (hf.height_at(p) - z).abs() < Fx::ratio(1, 4),
                "start pad is not level at {p:?}"
            );
            assert!(hf.slope_at(p) < Fx::ratio(1, 50));
        }
    }
    // Mirrored through the centre and across the start axis (the NE-SW diagonal).
    for (x, y) in [
        (2300, 2700),
        (3500, 3900),
        (4222, 5333),
        (6000, 5100),
        (1800, 6300),
    ] {
        let z = hf.height_at(FxVec2::from_ints(x, y));
        assert!((z - hf.height_at(FxVec2::from_ints(size - x, size - y))).abs() < Fx::ratio(1, 10));
        assert!((z - hf.height_at(FxVec2::from_ints(y, x))).abs() < Fx::ratio(1, 10));
    }

    // A lake too deep to ford in the middle, open sea along every edge,
    // and dry, level town islands out on the other diagonal.
    assert!(hf.height_at(centre) < water - Fx::from_int(6));
    let (w, h) = hf.size_cells();
    for i in 0..=w {
        for (cx, cy) in [(i, 0), (i, h), (0, i), (w, i)] {
            assert!(
                hf.sample_height(cx, cy) < water - Fx::from_int(20),
                "land at the map edge ({cx}, {cy})"
            );
        }
    }
    let out = (0.45 * size as f64 / 2f64.sqrt()) as i32;
    for town in [
        centre + FxVec2::from_ints(-out, out),
        centre + FxVec2::from_ints(out, -out),
    ] {
        assert!(hf.height_at(town) > water + Fx::from_int(5));
        assert!(hf.slope_at(town) < Fx::ratio(1, 50));
        let ore = ore_centres(&map);
        assert!(ore
            .iter()
            .any(|d| d.distance(town) < Fx::from_int(4 * BUILD_CELL_M)));
        let near = |p: FxVec2| p.distance(town) < Fx::from_int(size / 8);
        assert_eq!(ore.iter().filter(|d| near(**d)).count(), 3);
        assert!(map
            .props()
            .iter()
            .any(|p| p.kind.is_building() && near(p.pos)));
    }
}

#[test]
fn islands_ore_and_props_stay_on_dry_land() {
    let map = MapFile::open(baked_islands()).unwrap();
    let hf = Heightfield::load(&map).unwrap();
    let centre = FxVec2::from_ints(2 * TILE_SIZE_M, 2 * TILE_SIZE_M);

    // Three at each start, four round the lake, three on each town island, and the scatter.
    let deposits = &ore_centres(&map);
    assert!(
        deposits.len() >= 2 * 3 + 4 + 2 * 3 + 4,
        "{} ore fields",
        deposits.len()
    );
    for d in deposits {
        assert!(hf.in_bounds(*d));
        assert!(
            hf.height_at(*d) > hf.water_level() + Fx::from_int(5),
            "{d:?} is in or by the water"
        );
        assert!(hf.slope_at(*d) < Fx::ratio(1, 5));
        let image = centre + centre - *d;
        assert!(
            deposits
                .iter()
                .any(|o| o.distance(image) <= Fx::from_int(3 * BUILD_CELL_M)),
            "{d:?} has no mirror image"
        );
    }
    for s in map.start_positions() {
        let close: Vec<FxVec2> = deposits
            .iter()
            .copied()
            .filter(|d| d.distance(*s) < Fx::from_int(260))
            .collect();
        assert_eq!(close.len(), 3);
        for (i, a) in close.iter().enumerate() {
            for b in &close[i + 1..] {
                assert!(
                    a.distance(*b) > Fx::from_int(200),
                    "start deposits {a:?} and {b:?} crowd each other"
                );
            }
        }
    }
    assert_eq!(
        deposits
            .iter()
            .filter(|d| d.distance(centre) < Fx::from_int(800))
            .count(),
        4
    );

    let props = map.props();
    assert!(props.iter().filter(|p| p.kind.is_tree()).count() > 100);
    for p in props
        .iter()
        .filter(|p| p.kind.is_tree() || p.kind.is_building())
    {
        assert!(
            hf.height_at(p.pos) > hf.water_level() + Fx::ONE,
            "{:?} in the water at {:?}",
            p.kind,
            p.pos
        );
    }
}

/// Path cells a land unit at `from` can reach: dry, no steeper than 0.5,
/// and not in `blocked`. Returns whether `to` is among them.
fn land_route(
    hf: &Heightfield,
    from: FxVec2,
    to: FxVec2,
    blocked: impl Fn(f64, f64) -> bool,
) -> bool {
    let (w, h) = hf.size_cells();
    let cell = CELL_SIZE_M as f64;
    let open = |cx: u32, cy: u32| {
        let dry = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .all(|&(dx, dy)| hf.sample_height(cx + dx, cy + dy) > hf.water_level());
        dry && hf.cell_slope(cx, cy) <= Fx::ratio(1, 2)
            && !blocked((cx as f64 + 0.5) * cell, (cy as f64 + 0.5) * cell)
    };
    let mut seen = vec![false; (w * h) as usize];
    let mut queue = std::collections::VecDeque::from([hf.cell_at(from)]);
    while let Some((cx, cy)) = queue.pop_front() {
        if cx >= w || cy >= h || seen[(cy * w + cx) as usize] || !open(cx, cy) {
            continue;
        }
        seen[(cy * w + cx) as usize] = true;
        // Four-connected, so a diagonal wall three cells thick holds. (Wrapping below 0 fails the bounds check.)
        queue.extend([
            (cx + 1, cy),
            (cx.wrapping_sub(1), cy),
            (cx, cy + 1),
            (cx, cy.wrapping_sub(1)),
        ]);
    }
    let (tx, ty) = hf.cell_at(to);
    seen[(ty * w + tx) as usize]
}

/// The starts are joined by land both ways round the lake, and only so.
fn assert_two_land_routes(map: &MapFile) {
    let hf = Heightfield::load(map).unwrap();
    let starts = map.start_positions();
    let size = hf.size_metres().x.to_f64();
    // A wall across one passage: from the lake's middle out to the map's
    // corner along the diagonal the town islands are on.
    let wall = |x: f64, y: f64, north_west: bool| {
        let (along, across) = ((x + y - size) / 2f64.sqrt(), (y - x) / 2f64.sqrt());
        along.abs() < 20.0 && (across > 0.0) == north_west
    };
    assert!(
        land_route(&hf, starts[0], starts[1], |_, _| false),
        "no land route between the starts"
    );
    assert!(
        land_route(&hf, starts[0], starts[1], |x, y| wall(x, y, true)),
        "no route through the south-east passage"
    );
    assert!(
        land_route(&hf, starts[0], starts[1], |x, y| wall(x, y, false)),
        "no route through the north-west passage"
    );
    assert!(
        !land_route(&hf, starts[0], starts[1], |x, y| wall(x, y, true)
            || wall(x, y, false)),
        "the lake is fordable"
    );
    // No causeways: the town islands are real islands.
    for d in ore_centres(map)
        .iter()
        .filter(|d| d.distance(starts[0]).min(d.distance(starts[1])).to_f64() > 0.4 * size)
    {
        assert!(
            !land_route(&hf, starts[0], *d, |_, _| false),
            "a land bridge to the town island at {d:?}"
        );
    }
}

#[test]
fn islands_starts_are_joined_by_land_on_both_sides_of_the_lake() {
    assert_two_land_routes(&MapFile::open(baked_islands()).unwrap());
    // The smallest size the layout is meant for.
    let path = temp_path("isl-6km");
    bake(&BakeParams::islands("Small Shoals", 3, 7), &path).unwrap();
    assert_two_land_routes(&MapFile::open(&path).unwrap());
    std::fs::remove_file(path).ok();
}

/// `MC_CHECK_ISLANDS=$PWD/maps/twin_shoals.mcmap cargo test --release -p mc-map -- --ignored checks_a_baked`
/// runs the route check on a map baked with `mc-bake --layout islands`.
#[test]
#[ignore = "needs MC_CHECK_ISLANDS=<file.mcmap>"]
fn checks_a_baked_islands_map() {
    let path = std::env::var("MC_CHECK_ISLANDS").expect("MC_CHECK_ISLANDS is not set");
    assert_two_land_routes(&MapFile::open(std::path::Path::new(&path)).unwrap());
}

#[test]
fn bad_parameters_are_errors() {
    let path = temp_path("bad-params");
    assert!(bake(
        &BakeParams {
            players: 4,
            ..BakeParams::islands("x", 3, 1)
        },
        &path
    )
    .is_err());
    assert!(bake(
        &BakeParams {
            players: 9,
            ..BakeParams::square("x", 2, 1)
        },
        &path
    )
    .is_err());
    assert!(bake(
        &BakeParams {
            players: 0,
            ..BakeParams::square("x", 2, 1)
        },
        &path
    )
    .is_err());
    assert!(bake(&BakeParams::square("x", MAX_MAP_TILES + 1, 1), &path).is_err());
    assert!(bake(&BakeParams::square(&"n".repeat(65), 2, 1), &path).is_err());
}
