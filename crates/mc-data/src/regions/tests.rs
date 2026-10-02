use super::*;

fn wall(line: &[(f32, f32)], left: usize, right: usize) -> Wall {
    Wall {
        line: line.to_vec(),
        left,
        right,
    }
}

/// Frostline's wall: desert (0) west of it, Alaska (1) east.
fn two() -> Walls {
    let line = [
        (6592.0, 0.0),
        (6592.0, 3400.0),
        (8192.0, 5000.0),
        (8192.0, 11384.0),
        (9792.0, 12984.0),
        (9792.0, 16384.0),
    ];
    Walls::new(vec![wall(&line, 0, 1)], 2).unwrap()
}

/// Three regions on an 8 km square: 0 west of x = 4000, and east of it 1 south of
/// y = 4000 and 2 north of it. The east side's wall ends on the first.
fn tee() -> Walls {
    Walls::new(
        vec![
            wall(&[(4000.0, 0.0), (4000.0, 4000.0)], 0, 1),
            wall(&[(4000.0, 4000.0), (4000.0, 8000.0)], 0, 2),
            wall(&[(4000.0, 4000.0), (8000.0, 4000.0)], 2, 1),
        ],
        3,
    )
    .unwrap()
}

fn tee_region(x: f32, y: f32) -> usize {
    if x < 4000.0 {
        0
    } else if y < 4000.0 {
        1
    } else {
        2
    }
}

/// Four quarters of an 8 km square, meeting in its middle: 0 south-west, 1
/// south-east, 2 north-east, 3 north-west. Two of the walls are written towards
/// the middle and two away from it.
fn quarters() -> Walls {
    Walls::new(
        vec![
            wall(&[(4000.0, 0.0), (4000.0, 4000.0)], 0, 1),
            wall(&[(4000.0, 8000.0), (4000.0, 4000.0)], 2, 3),
            wall(&[(0.0, 4000.0), (4000.0, 4000.0)], 3, 0),
            wall(&[(4000.0, 4000.0), (8000.0, 4000.0)], 2, 1),
        ],
        4,
    )
    .unwrap()
}

fn quarter(x: f32, y: f32) -> usize {
    match (x < 4000.0, y < 4000.0) {
        (true, true) => 0,
        (false, true) => 1,
        (false, false) => 2,
        (true, false) => 3,
    }
}

/// The weights at every step of a walk from `from` to `to`: they always sum to 1,
/// and never jump (a step of `step` metres changes none by more than a smooth
/// hand-over within `half` could).
fn walk_is_smooth(walls: &Walls, from: (f32, f32), to: (f32, f32), half: f32) {
    let length = (to.0 - from.0).hypot(to.1 - from.1);
    let step = half / 40.0;
    let steps = (length / step).ceil() as usize;
    let mut last: Option<[f32; MAX_REGIONS]> = None;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let (x, y) = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        let w = walls.weights(x, y, half);
        let sum: f32 = w.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "{x}, {y}: {w:?}");
        assert!(w.iter().all(|v| (0.0..=1.0).contains(v)), "{x}, {y}: {w:?}");
        if let Some(last) = last {
            for r in 0..MAX_REGIONS {
                assert!(
                    (w[r] - last[r]).abs() < 0.12,
                    "region {r} jumps at {x}, {y}: {} to {}",
                    last[r],
                    w[r]
                );
            }
        }
        last = Some(w);
    }
}

#[test]
fn a_wall_parts_two_regions() {
    let walls = two();
    assert_eq!((walls.regions(), walls.segments().len()), (2, 5));
    // West of the line region 0, east of it region 1; a point on it is on its right.
    assert_eq!(walls.region_at(1000.0, 1000.0), 0);
    assert_eq!(walls.region_at(12000.0, 1000.0), 1);
    assert_eq!(walls.region_at(7391.0, 4200.0), 0);
    assert_eq!(walls.region_at(7393.0, 4200.0), 1);
    assert_eq!(walls.region_at(8192.0, 8000.0), 1);
    // Distance is across the wall, not along x: shorter on the diagonal.
    assert!((walls.wall_distance(8292.0, 8000.0) - 100.0).abs() < 1e-3);
    assert!((walls.wall_distance(8092.0, 8000.0) - 100.0).abs() < 1e-3);
    let across = walls.wall_distance(7492.0, 4200.0);
    assert!(
        (across - 100.0 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-2,
        "{across}"
    );
    // How far along the wall: its length up to the nearest point.
    let along = walls.probe(8292.0, 8000.0).along;
    let expect = 3400.0 + 1600.0 * std::f32::consts::SQRT_2 + 3000.0;
    assert!((along - expect).abs() < 0.1, "{along}");
    // Half the map each.
    let shares = walls.shares(16384.0, 16384.0);
    assert!((shares[0] - 0.5).abs() < 0.02 && (shares[1] - 0.5).abs() < 0.02);
    assert_eq!(shares[2..], [0.0; 6]);
    // Either side of its corners the side is the same as along its straights.
    for (corner, turn) in [((6592.0, 3400.0), 1.0), ((8192.0, 5000.0), -1.0)] {
        for i in 0..64 {
            let a = i as f32 / 64.0 * std::f32::consts::TAU;
            for r in [0.5, 1.0, 30.0] {
                let (x, y) = (corner.0 + a.cos() * r, corner.1 + a.sin() * r);
                // The wall here: straight north below the first corner and above
                // the second, north-east between them.
                let on_diagonal = if turn > 0.0 {
                    y > corner.1
                } else {
                    y < corner.1
                };
                let line_x = if on_diagonal {
                    corner.0 + (y - corner.1)
                } else {
                    corner.0
                };
                if (x - line_x).abs() < 0.2 {
                    continue;
                }
                assert_eq!(walls.region_at(x, y), (x > line_x) as usize, "{x}, {y}");
            }
        }
    }
}

#[test]
fn a_wall_may_run_any_way() {
    // The same wall written north to south, its hands swapped: the same regions.
    let up = two();
    let mut line = up.walls()[0].line.clone();
    line.reverse();
    let down = Walls::new(vec![wall(&line, 1, 0)], 2).unwrap();
    for i in 0..40 {
        for j in 0..40 {
            let (x, y) = (i as f32 * 420.0 + 3.0, j as f32 * 420.0 - 100.0);
            assert_eq!(up.region_at(x, y), down.region_at(x, y), "{x}, {y}");
            assert!((up.wall_distance(x, y) - down.wall_distance(x, y)).abs() < 1e-2);
        }
    }
    // West to east: north is on the left.
    let across = Walls::new(vec![wall(&[(0.0, 500.0), (900.0, 500.0)], 1, 0)], 2).unwrap();
    assert_eq!(across.region_at(300.0, 700.0), 1);
    assert_eq!(across.region_at(300.0, 200.0), 0);
}

#[test]
fn two_regions_hand_over_as_one_smooth_step() {
    let walls = two();
    for half in [6.0, 160.0] {
        // Deep inside a region: all of it, exactly.
        let mut west = [0.0; MAX_REGIONS];
        west[0] = 1.0;
        let mut east = [0.0; MAX_REGIONS];
        east[1] = 1.0;
        assert_eq!(walls.weights(8192.0 - half, 8000.0, half), west);
        assert_eq!(walls.weights(8192.0 + half, 8000.0, half), east);
        assert_eq!(walls.weights(100.0, 100.0, half), west);
        // Across the wall: the smooth step from one side to the other.
        for i in -20..=20 {
            let d = half * i as f32 / 20.0;
            let w = walls.weights(8192.0 + d, 8000.0, half);
            let k = smoothstep(-half, half, d);
            assert!(
                (w[1] - k).abs() < 1e-4 && (w[0] - (1.0 - k)).abs() < 1e-4,
                "{d}: {w:?}"
            );
        }
        walk_is_smooth(
            &walls,
            (8192.0 - 2.0 * half, 8000.0),
            (8192.0 + 2.0 * half, 8000.0),
            half,
        );
        // And across its corners.
        walk_is_smooth(
            &walls,
            (6592.0 - half, 3400.0 + half),
            (6592.0 + 2.0 * half, 3400.0 - half),
            half,
        );
    }
    // Without walls, region 0 everywhere.
    let none = Walls::default();
    assert_eq!((none.regions(), none.is_empty()), (1, true));
    assert_eq!(none.region_at(5.0, 5.0), 0);
    assert_eq!(none.wall_distance(5.0, 5.0), f32::INFINITY);
    assert_eq!(none.weights(5.0, 5.0, 160.0)[0], 1.0);
    assert_eq!(none.shares(100.0, 100.0)[0], 1.0);
}

#[test]
fn three_regions_meet_where_a_wall_ends_on_another() {
    let walls = tee();
    for i in 0..80 {
        for j in 0..80 {
            let (x, y) = (i as f32 * 100.0 + 37.0, j as f32 * 100.0 + 41.0);
            assert_eq!(walls.region_at(x, y), tee_region(x, y), "{x}, {y}");
        }
    }
    // Right round the junction, from a foot out.
    for r in [0.3, 5.0, 150.0] {
        for i in 0..360 {
            let a = (i as f32 + 0.37).to_radians();
            let (x, y) = (4000.0 + a.cos() * r, 4000.0 + a.sin() * r);
            assert_eq!(walls.region_at(x, y), tee_region(x, y), "{x}, {y}");
        }
    }
    let shares = walls.shares(8000.0, 8000.0);
    assert!((shares[0] - 0.5).abs() < 0.02, "{shares:?}");
    assert!((shares[1] - 0.25).abs() < 0.02 && (shares[2] - 0.25).abs() < 0.02);
    for half in [6.0, 160.0] {
        // Deep inside each region, all of it exactly.
        for (x, y) in [(1000.0, 4000.0), (6000.0, 1000.0), (6000.0, 7000.0)] {
            let w = walls.weights(x, y, half);
            assert_eq!(w[tee_region(x, y)], 1.0);
            assert_eq!(w.iter().sum::<f32>(), 1.0);
        }
        // By a wall well away from the junction, only its two regions.
        let w = walls.weights(4000.0 + half * 0.2, 1000.0, half);
        assert!(w[0] > 0.0 && w[1] > w[0] && w[2] == 0.0, "{w:?}");
        // At the junction, a third of each.
        let w = walls.weights(4000.0, 4000.0, half);
        for share in &w[..3] {
            assert!((share - 1.0 / 3.0).abs() < 1e-3, "{w:?}");
        }
        // Smooth across every wall, through the junction and round it.
        walk_is_smooth(
            &walls,
            (4000.0 - 2.0 * half, 1000.0),
            (4000.0 + 2.0 * half, 1000.0),
            half,
        );
        walk_is_smooth(
            &walls,
            (6000.0, 4000.0 - 2.0 * half),
            (6000.0, 4000.0 + 2.0 * half),
            half,
        );
        walk_is_smooth(
            &walls,
            (4000.0 - 2.0 * half, 4000.0 - half),
            (4000.0 + 2.0 * half, 4000.0 + half),
            half,
        );
        walk_is_smooth(
            &walls,
            (4000.0 - 2.0 * half, 4000.0),
            (4000.0 + 2.0 * half, 4000.0),
            half,
        );
        for r in [half * 0.1, half * 0.5, half, half * 1.5] {
            let mut last = (4000.0 + r, 4000.0);
            for i in 1..=72 {
                let a = (i as f32 * 5.0).to_radians();
                let next = (4000.0 + a.cos() * r, 4000.0 + a.sin() * r);
                walk_is_smooth(&walls, last, next, half);
                last = next;
            }
        }
    }
}

#[test]
fn four_regions_meet_at_a_point() {
    let walls = quarters();
    for i in 0..80 {
        for j in 0..80 {
            let (x, y) = (i as f32 * 100.0 + 37.0, j as f32 * 100.0 + 41.0);
            assert_eq!(walls.region_at(x, y), quarter(x, y), "{x}, {y}");
        }
    }
    for r in [0.3, 5.0, 150.0] {
        for i in 0..360 {
            let a = (i as f32 + 0.37).to_radians();
            let (x, y) = (4000.0 + a.cos() * r, 4000.0 + a.sin() * r);
            assert_eq!(walls.region_at(x, y), quarter(x, y), "{x}, {y}");
        }
    }
    let shares = walls.shares(8000.0, 8000.0);
    assert!(
        shares[..4].iter().all(|s| (s - 0.25).abs() < 0.02),
        "{shares:?}"
    );
    for half in [6.0, 160.0] {
        for (x, y) in [
            (1000.0, 1000.0),
            (7000.0, 1000.0),
            (7000.0, 7000.0),
            (1000.0, 7000.0),
        ] {
            assert_eq!(walls.weights(x, y, half)[quarter(x, y)], 1.0);
        }
        // At the middle a quarter of each; on an arm, half of each of its two.
        let w = walls.weights(4000.0, 4000.0, half);
        assert!(w[..4].iter().all(|s| (s - 0.25).abs() < 1e-3), "{w:?}");
        let w = walls.weights(4000.0, 1000.0, half);
        assert!(
            (w[0] - 0.5).abs() < 1e-3 && (w[1] - 0.5).abs() < 1e-3,
            "{w:?}"
        );
        assert_eq!((w[2], w[3]), (0.0, 0.0));
        // Smooth across the arms, through the middle and round it.
        walk_is_smooth(
            &walls,
            (4000.0 - 2.0 * half, 7000.0),
            (4000.0 + 2.0 * half, 7000.0),
            half,
        );
        walk_is_smooth(
            &walls,
            (4000.0 - 2.0 * half, 4000.0 - 2.0 * half),
            (4000.0 + 2.0 * half, 4000.0 + 2.0 * half),
            half,
        );
        walk_is_smooth(
            &walls,
            (4000.0 - 2.0 * half, 4000.0 + half * 0.3),
            (4000.0 + 2.0 * half, 4000.0 - half * 0.2),
            half,
        );
        for r in [half * 0.1, half * 0.5, half, half * 1.5] {
            let mut last = (4000.0 + r, 4000.0);
            for i in 1..=72 {
                let a = (i as f32 * 5.0).to_radians();
                let next = (4000.0 + a.cos() * r, 4000.0 + a.sin() * r);
                walk_is_smooth(&walls, last, next, half);
                last = next;
            }
        }
    }
}

#[test]
fn bad_walls_are_refused() {
    let line = [(0.0, 0.0), (0.0, 900.0)];
    assert!(Walls::new(vec![wall(&line, 0, 1)], 2).is_ok());
    assert_eq!(
        Walls::new(vec![wall(&line[..1], 0, 1)], 2),
        Err(RegionError::Points { wall: 0, points: 1 })
    );
    assert_eq!(
        Walls::new(vec![wall(&line, 0, 1), wall(&[], 0, 1)], 2),
        Err(RegionError::Points { wall: 1, points: 0 })
    );
    assert_eq!(
        Walls::new(vec![wall(&[(0.0, 0.0), (f32::NAN, 5.0)], 0, 1)], 2),
        Err(RegionError::NotFinite { wall: 0, point: 1 })
    );
    assert_eq!(
        Walls::new(vec![wall(&[(0.0, 0.0), (3.0, 5.0), (3.0, 5.0)], 0, 1)], 2),
        Err(RegionError::Repeated { wall: 0, point: 2 })
    );
    assert_eq!(
        Walls::new(vec![wall(&line, 1, 1)], 2),
        Err(RegionError::SameSides { wall: 0 })
    );
    assert_eq!(
        Walls::new(vec![wall(&line, 0, 2)], 2),
        Err(RegionError::NoRegion { wall: 0, region: 2 })
    );
    // 32 segments between them, and no more.
    let zigzag = |points: usize| -> Vec<(f32, f32)> {
        (0..points)
            .map(|i| ((i % 2) as f32 * 50.0, i as f32 * 100.0))
            .collect()
    };
    let full = vec![wall(&zigzag(17), 0, 1), wall(&zigzag(17), 1, 2)];
    assert_eq!(Walls::new(full, 3).unwrap().segments().len(), 32);
    let over = vec![wall(&zigzag(17), 0, 1), wall(&zigzag(18), 1, 2)];
    assert_eq!(Walls::new(over, 3), Err(RegionError::Segments(33)));
}
