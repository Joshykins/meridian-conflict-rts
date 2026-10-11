use super::*;
use crate::ai::commander::reach::NODE;
use crate::ai::commander::zones::Grid;
use mc_core::FxVec2;

fn at(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_ints(x * NODE + NODE / 2, y * NODE + NODE / 2)
}

/// Land from rows (row 0 first): `#` land, `.` cliff; ore fields at `ores`.
fn zones(rows: &[&str], ores: &[FxVec2]) -> Zones {
    let (w, h) = (rows[0].len() as i32, rows.len() as i32);
    let ground: Vec<Option<Ground>> = rows
        .iter()
        .flat_map(|r| r.chars().map(|c| (c == '#').then_some(Ground::Land)))
        .collect();
    let crosses = |_: usize, _: usize| true;
    Zones::build(
        &Grid {
            w,
            h,
            ground: &ground,
            crosses: &crosses,
        },
        &[at(0, 1), at(w - 1, 1)],
        ores,
    )
}

/// We hold the west end, player 1 the east end: claims of `materials` at each start.
fn claims(zs: &Zones, materials: i32) -> Claims {
    let n = zs.zones.len();
    let mut ours = vec![0; n];
    ours[zs.starts[0] as usize] = materials;
    let mut theirs = vec![0; n];
    theirs[zs.starts[1] as usize] = materials;
    Claims {
        ours,
        our_army: vec![0; n],
        theirs: vec![vec![0; n], theirs],
        their_army: vec![vec![0; n], vec![0; n]],
    }
}

fn corridor() -> Vec<&'static str> {
    let row = "################################################";
    vec![row, row, row]
}

#[test]
fn a_front_runs_where_both_sides_reach_as_soon() {
    let ores: Vec<FxVec2> = (1..8).map(|k| at(k * 6, 1)).collect();
    let zs = zones(&corridor(), &ores);
    let (fronts, records, next) = find(&zs, &claims(&zs, 3000), &[], 0, 10);
    assert_eq!(fronts.len(), 1, "{fronts:?}");
    let f = &fronts[0];
    assert_eq!(f.enemy, 1);
    assert!(!f.water);
    // In the middle: no front zone is either start.
    assert!(!f.zones.contains(&zs.starts[0]));
    assert!(!f.zones.contains(&zs.starts[1]));
    assert_eq!(records.len(), 1);
    assert_eq!(next, 1);
}

#[test]
fn a_front_keeps_its_id_from_one_think_to_the_next() {
    let ores: Vec<FxVec2> = (1..8).map(|k| at(k * 6, 1)).collect();
    let zs = zones(&corridor(), &ores);
    let (_, records, next) = find(&zs, &claims(&zs, 3000), &[], 0, 10);
    // The enemy grows stronger: the border moves toward us, the front stays itself.
    let mut c = claims(&zs, 3000);
    c.theirs[1][zs.starts[1] as usize] = 9000;
    let (fronts, again, next2) = find(&zs, &c, &records, next, 20);
    assert_eq!(fronts.len(), 1);
    assert_eq!(fronts[0].id, records[0].id);
    assert_eq!(again[0].since, 10);
    assert_eq!(next2, next);
}

#[test]
fn a_cliff_between_two_passes_makes_two_fronts() {
    let open = "################################################";
    let wall = "#########............................###########";
    let rows = vec![open, open, wall, wall, wall, open, open];
    let ores = vec![
        at(16, 0),
        at(32, 0),
        at(16, 6),
        at(32, 6),
        at(24, 0),
        at(24, 6),
    ];
    let zs = zones(&rows, &ores);
    let (fronts, _, _) = find(&zs, &claims(&zs, 3000), &[], 0, 10);
    assert_eq!(fronts.len(), 2, "{fronts:?}");
}

#[test]
fn no_enemy_claim_no_front() {
    let zs = zones(&corridor(), &[]);
    let mut c = claims(&zs, 3000);
    c.theirs[1] = vec![0; zs.zones.len()];
    let (fronts, _, _) = find(&zs, &c, &[], 0, 10);
    assert!(fronts.is_empty());
}
