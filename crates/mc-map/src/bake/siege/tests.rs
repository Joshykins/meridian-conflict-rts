use super::grid::y_at;
use super::*;
use crate::bake::test_maps::HALCYON;

fn wall_piece(kind: PropKind) -> bool {
    matches!(
        kind,
        PropKind::CityWall | PropKind::CityWallTower | PropKind::CityGate
    )
}

/// Every building stands on ground cut level to it (the wall's curtain
/// follows the land).
#[test]
fn lots_stand_on_level_ground() {
    let t = &*HALCYON;
    let mut worst = (0.0_f64, (0.0, 0.0));
    for lot in t
        .siege
        .lots
        .iter()
        .filter(|l| l.kind != PropKind::CityWall)
        .step_by(7)
    {
        for p in lot.rect.corners().into_iter().chain([lot.rect.c]) {
            let off = (t.height(p.0, p.1) - lot.level).abs();
            if off > worst.0 {
                worst = (off, p);
            }
        }
    }
    assert!(
        worst.0 < 0.3,
        "a lot's ground is {:.2} m off level at {:?}",
        worst.0,
        worst.1
    );
}

/// Nothing is built on a road or its pavement, but the wall's gates over
/// their own roads.
#[test]
fn lots_keep_off_the_roads() {
    let t = &*HALCYON;
    let index = t.siege.road_index.as_ref().unwrap();
    for lot in &t.siege.lots {
        if wall_piece(lot.kind) || lot.kind == PropKind::CityRubble {
            continue;
        }
        let r = lot.rect.hx.hypot(lot.rect.hy);
        for i in index.near(lot.rect.c, r + 40.0) {
            let s = t.siege.roads[i as usize];
            let d = lot.rect.distance_to_segment(s.a, s.b);
            assert!(
                d >= s.reach(),
                "{:?} at {:?} is {d:.1} m from a road reaching {:.1} m",
                lot.kind,
                lot.at,
                s.reach()
            );
        }
    }
}

/// The wall runs unbroken from the west edge to the east edge, but for its
/// gates' passages and its breaches.
#[test]
fn the_wall_runs_edge_to_edge() {
    let t = &*HALCYON;
    let pieces: Vec<&Lot> = t.siege.lots.iter().filter(|l| wall_piece(l.kind)).collect();
    let mut x = 8.0;
    while x < SIZE - 8.0 {
        let p = (x, y_at(WALL, x));
        let covered = pieces.iter().any(|l| l.rect.outside(p) <= 0.0);
        let passage = GATES_X.iter().any(|&g| (x - g).abs() < 30.0);
        let breach = BREACHES_X
            .iter()
            .any(|&b| (x - b).abs() < BREACH_HALF + 2.0);
        assert!(covered || passage || breach, "the wall is open at x {x:.0}");
        x += 4.0;
    }
}

/// The bases' pads are level and well clear of every building.
#[test]
fn bases_are_open_and_level() {
    let t = &*HALCYON;
    for &s in &t.starts {
        let h = t.height(s.0, s.1);
        for k in 0..16 {
            let a = k as f64 / 16.0 * std::f64::consts::TAU;
            let p = (
                s.0 + 0.9 * BASE_CORE * a.cos(),
                s.1 + 0.9 * BASE_CORE * a.sin(),
            );
            assert!(
                (t.height(p.0, p.1) - h).abs() < 0.5,
                "start {s:?} is not level"
            );
        }
        assert!(
            t.siege.lots.iter().all(|l| dist(l.rect.c, s) > BASE_OPEN),
            "a building in base {s:?}"
        );
    }
}
