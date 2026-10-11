use super::*;

fn at(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_ints(x * NODE + NODE / 2, y * NODE + NODE / 2)
}

/// A grid from rows (row 0 first): `#` land, `~` water, `.` cliff. Every edge
/// between two nodes of the same ground can be crossed.
fn zones(rows: &[&str], starts: &[FxVec2], ores: &[FxVec2]) -> Zones {
    let (w, h) = (rows[0].len() as i32, rows.len() as i32);
    let ground: Vec<Option<Ground>> = rows
        .iter()
        .flat_map(|r| {
            r.chars().map(|c| match c {
                '#' => Some(Ground::Land),
                '~' => Some(Ground::Water),
                _ => None,
            })
        })
        .collect();
    let crosses = |_: usize, _: usize| true;
    Zones::build(
        &Grid {
            w,
            h,
            ground: &ground,
            crosses: &crosses,
        },
        starts,
        ores,
    )
}

#[test]
fn zones_split_the_ground_between_their_seeds_and_know_their_neighbours() {
    let zs = zones(
        &[
            "###########.##",
            "###########.##",
            "##.########.##",
            "###########.##",
            "###########.##",
        ],
        &[at(0, 0)],
        &[at(10, 4)],
    );
    assert_eq!(zs.of(at(1, 1)), Some(0));
    assert_eq!(zs.of(at(9, 3)), Some(1));
    assert_eq!(zs.zones[0].adj, vec![1]);
    assert_eq!(zs.zones[1].adj, vec![0]);
    // The strip past the cliff is ground no seed reached: a zone of its own, with no
    // way across to the others.
    let strip = zs.of(at(13, 2)).expect("the strip is a zone");
    assert!(strip >= 2);
    assert!(zs.zones[strip as usize].adj.is_empty());
    assert_eq!(zs.starts, vec![0]);
}

#[test]
fn seeds_close_together_make_one_zone_that_counts_both_ore_fields() {
    let zs = zones(
        &["#####", "#####", "#####"],
        &[at(0, 0)],
        &[at(1, 1), at(2, 1)],
    );
    assert_eq!(zs.zones.len(), 1);
    assert_eq!(zs.zones[0].ore, 2);
}

#[test]
fn water_gets_zones_of_its_own_and_the_coast_links_the_two_grounds() {
    let zs = zones(&["####~~~~", "####~~~~", "####~~~~"], &[at(0, 1)], &[]);
    let land = zs.of(at(1, 1)).expect("land");
    let sea = zs.of(at(6, 1)).expect("sea");
    assert_ne!(land, sea);
    assert_eq!(zs.zones[land as usize].ground, Ground::Land);
    assert_eq!(zs.zones[sea as usize].ground, Ground::Water);
    assert!(zs.zones[land as usize].adj.is_empty());
    assert_eq!(zs.zones[land as usize].coast, vec![sea]);
    assert_eq!(zs.zones[sea as usize].coast, vec![land]);
}

#[test]
fn open_ground_far_from_any_seed_is_split_by_the_lattice() {
    // 40 nodes is 5 km: one start cannot hold it all.
    let row = "#".repeat(40);
    let rows: Vec<&str> = (0..8).map(|_| row.as_str()).collect();
    let zs = zones(&rows, &[at(0, 0)], &[]);
    assert!(zs.zones.len() >= 3, "{} zones", zs.zones.len());
    let far = zs.of(at(39, 4)).expect("far end");
    assert_ne!(far, 0);
}

#[test]
fn a_pocket_too_small_for_a_zone_belongs_to_none() {
    let zs = zones(&["###.#", "#####"], &[at(0, 0)], &[]);
    // The lone node is joined to the rest below it, so it is in the start's zone.
    assert_eq!(zs.of(at(4, 0)), Some(0));
    let zs = zones(&["###..#", "###..."], &[at(0, 0)], &[]);
    // Cut off and one node: no zone, though `of` falls back to none beside it.
    let (w, _, id) = zs.grid();
    assert_eq!(id[5], NONE);
    assert_eq!(w, 6);
}

#[test]
fn fire_along_carries_the_worst_zone_on_the_way() {
    // 0 - 1 - 2, fire on the middle one only.
    let zones = Zones {
        zones: vec![
            Zone {
                adj: vec![1],
                ..Zone::default()
            },
            Zone {
                adj: vec![0, 2],
                ..Zone::default()
            },
            Zone {
                adj: vec![1],
                ..Zone::default()
            },
        ],
        ..Zones::default()
    };
    let view = ZoneView {
        threat: vec![Fx::ZERO, Fx::from_int(7), Fx::ZERO],
        enemy: vec![Fx::ZERO; 3],
        held: vec![Fx::ZERO; 3],
    };
    assert_eq!(
        view.fire_along(&zones, 0),
        vec![Fx::ZERO, Fx::from_int(7), Fx::from_int(7)]
    );
    assert_eq!(view.fire_along(&zones, 1)[0], Fx::from_int(7));
}
