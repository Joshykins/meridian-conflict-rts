use super::shape::{in_harbour, HARBOUR, ISLANDS};
use super::*;
use crate::bake::test_maps::VELA;

/// Every building stands on ground cut level to it (fences follow the land).
#[test]
fn lots_stand_on_level_ground() {
    let t = &*VELA;
    let mut worst = (0.0_f64, (0.0, 0.0), PropKind::ColonyHab);
    for lot in t
        .vela
        .lots
        .iter()
        .filter(|l| l.kind != PropKind::ColonyFence)
    {
        for p in lot.rect.corners().into_iter().chain([lot.rect.c]) {
            let off = (t.height(p.0, p.1) - lot.level).abs();
            if off > worst.0 {
                worst = (off, p, lot.kind);
            }
        }
    }
    assert!(
        worst.0 < 0.3,
        "a {:?}'s ground is {:.2} m off level at {:?}",
        worst.2,
        worst.0,
        worst.1
    );
}

/// Nothing is built on a road.
#[test]
fn lots_keep_off_the_roads() {
    let t = &*VELA;
    let index = t.vela.road_index.as_ref().unwrap();
    for lot in &t.vela.lots {
        let r = lot.rect.hx.hypot(lot.rect.hy);
        for i in index.near(lot.rect.c, r + 40.0) {
            let s = t.vela.roads[i as usize];
            let d = lot.rect.distance_to_segment(s.a, s.b);
            assert!(
                d >= s.reach(),
                "{:?} at {:?} is {d:.1} m from a road",
                lot.kind,
                lot.at
            );
        }
    }
}

/// The roads run on dry land, but where the bridge carries the coast road
/// over the river; and nowhere steeper than a laden truck climbs.
#[test]
fn roads_are_dry_and_drivable() {
    let t = &*VELA;
    for s in &t.vela.roads {
        let n = (dist(s.a, s.b) / 8.0).ceil().max(1.0) as usize;
        for k in 0..=n {
            let f = k as f64 / n as f64;
            let p = (s.a.0 + (s.b.0 - s.a.0) * f, s.a.1 + (s.b.1 - s.a.1) * f);
            // On the deck, or at its ends where the bank falls away under it.
            let on_bridge =
                (p.0 - BRIDGE_X).abs() < BRIDGE_HALF_SPAN + 12.0 && (p.1 - ROAD_Y).abs() < 10.0;
            if on_bridge {
                continue;
            }
            let h = t.height(p.0, p.1);
            assert!(h > 0.5, "a road is under water at {p:?} ({h:.1} m)");
            let slope = t.slope(p.0, p.1);
            assert!(slope < 0.2, "a road climbs {slope:.2} at {p:?}");
        }
    }
}

/// The bridge's deck meets the road at both ends, over the river.
#[test]
fn the_bridge_lands_on_the_road() {
    let t = &*VELA;
    for x in [BRIDGE_X - BRIDGE_HALF_SPAN, BRIDGE_X + BRIDGE_HALF_SPAN] {
        let h = t.height(x, ROAD_Y);
        assert!(
            (h - DECK_TOP).abs() < 0.6,
            "the bridge's end at x {x} stands on {h:.2} m"
        );
    }
    assert!(
        t.height(BRIDGE_X, ROAD_Y) < -1.0,
        "no river under the bridge"
    );
}

/// The harbour is water, the islands are land, the beach is dry above the
/// tide and wet below it.
#[test]
fn the_coast_is_where_the_script_wants_it() {
    let t = &*VELA;
    let (lo, hi) = HARBOUR;
    let mid = ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0);
    assert!(in_harbour(mid));
    assert!(
        t.height(mid.0, mid.1) < -3.0,
        "the harbour is dry: {:.1}",
        t.height(mid.0, mid.1)
    );
    for (c, _, _) in ISLANDS {
        assert!(
            t.height(c.0, c.1) > 2.0,
            "the island at {c:?} is under water"
        );
    }
    assert!(t.height(BEACH_START.0, BEACH_START.1) > 1.0);
}
