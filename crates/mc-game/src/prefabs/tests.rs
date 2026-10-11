use super::*;

fn blueprints() -> Blueprints {
    Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

fn at(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_ints(x, y)
}

/// A factory (8x8) with two generators (2x2) and a storage (3x3) beside it.
fn base(b: &Blueprints) -> Vec<(BlueprintId, FxVec2)> {
    let id = |k: &str| b.id_of(k).unwrap();
    vec![
        (id("aster_t1_land_factory"), at(600, 600)),
        (id("aster_t1_power"), at(660, 600)),
        (id("aster_t1_power"), at(660, 624)),
        (id("aster_materials_storage"), at(666, 570)),
    ]
}

#[test]
fn capture_then_layout_puts_everything_back() {
    let b = blueprints();
    let sites = base(&b);
    let p = capture(&b, &sites).unwrap();
    assert_eq!(p.members.len(), 4);
    let (members, gone) = p.resolve(&b);
    assert_eq!(gone, 0);
    // Put down where it was taken from, it stands on the same lots.
    let (lo, hi) = bounds(&b, &sites).unwrap();
    let middle = FxVec2::new(
        Fx::from_f32((lo[0] + hi[0]) * 0.5),
        Fx::from_f32((lo[1] + hi[1]) * 0.5),
    );
    let mut placed = layout(&b, &members, middle, 0);
    let mut want: Vec<_> = sites
        .iter()
        .map(|&(id, p)| (id, mc_sim::world::snap_to_build_grid(b.unit(id), p)))
        .collect();
    placed.sort_by_key(|(id, p)| (id.0, p.x, p.y));
    want.sort_by_key(|(id, p)| (id.0, p.x, p.y));
    assert_eq!(placed, want);
}

#[test]
fn moved_anywhere_it_keeps_its_shape() {
    let b = blueprints();
    let p = capture(&b, &base(&b)).unwrap();
    let (members, _) = p.resolve(&b);
    let here = layout(&b, &members, at(1000, 1000), 0);
    // Any point snaps to a cell corner first, so the shape never shifts by a cell.
    for q in [at(2003, 517), at(1210, 1195), at(15, 4000)] {
        let there = layout(&b, &members, q, 0);
        let d = there[0].1 - here[0].1;
        for (a, b) in here.iter().zip(&there) {
            assert_eq!(b.1 - a.1, d, "member moved apart from the others");
        }
    }
}

#[test]
fn four_quarter_turns_come_back_round() {
    for (x, y) in [(0, 0), (18, -6), (-30, 42)] {
        assert_eq!(rotate(x, y, 4), (x, y));
        assert_eq!(rotate(x, y, 2), (-x, -y));
        let (a, b) = rotate(x, y, 1);
        assert_eq!(rotate(a, b, 3), (x, y));
    }
}

#[test]
fn a_turned_layout_of_square_lots_does_not_overlap_itself() {
    let b = blueprints();
    let p = capture(&b, &base(&b)).unwrap();
    let (members, _) = p.resolve(&b);
    for q in 0..4 {
        let sites = layout(&b, &members, at(1200, 1200), q);
        for (i, &(ba, pa)) in sites.iter().enumerate() {
            for &(bb, pb) in &sites[i + 1..] {
                let (fa, fb) = (b.unit(ba).footprint, b.unit(bb).footprint);
                let reach = |x: u8, y: u8| (x as i32 + y as i32) * mc_map::BUILD_CELL_M / 2;
                let d = pa - pb;
                let apart = d.x.abs().round_int() >= reach(fa.0, fb.0)
                    || d.y.abs().round_int() >= reach(fa.1, fb.1);
                assert!(apart, "turn {q}: lots overlap");
            }
        }
    }
}

#[test]
fn capture_refuses_nothing_and_too_much() {
    let b = blueprints();
    assert_eq!(capture(&b, &[]), Err(Uncaptured::Nothing));
    let commander = b.factions[0].commander;
    assert_eq!(
        capture(&b, &[(commander, at(100, 100))]),
        Err(Uncaptured::Nothing)
    );
    let power = b.id_of("aster_t1_power").unwrap();
    let many: Vec<_> = (0..MAX_MEMBERS as i32 + 1)
        .map(|i| (power, at(100 + 24 * (i % 10), 100 + 24 * (i / 10))))
        .collect();
    assert_eq!(
        capture(&b, &many),
        Err(Uncaptured::TooMany(MAX_MEMBERS + 1))
    );
    // The same structure on the same lot twice (built and planned) counts once.
    let twice = [(power, at(100, 100)), (power, at(100, 100))];
    assert_eq!(capture(&b, &twice).unwrap().members.len(), 1);
}

#[test]
fn the_name_says_what_is_in_it() {
    let b = blueprints();
    let p = capture(&b, &base(&b)).unwrap();
    let power = &b.unit(b.id_of("aster_t1_power").unwrap()).name;
    assert!(p.name.starts_with(&format!("2 {power}")), "{}", p.name);
    assert!(p.name.len() <= MAX_NAME);
}

#[test]
fn names_are_cleaned() {
    assert_eq!(clean_name("  Forward \n Base\t"), "Forward Base");
    assert_eq!(clean_name("   "), "Prefab");
    assert_eq!(clean_name(&"x".repeat(80)).len(), MAX_NAME);
}

#[test]
fn keeping_a_name_again_replaces_it_and_puts_it_first() {
    let b = blueprints();
    let mut lib = Library::default();
    let mut p = capture(&b, &base(&b)).unwrap();
    p.name = "Alpha".into();
    let Some(Kept::Added(alpha)) = lib.keep(p.clone()) else {
        panic!("not added")
    };
    p.name = "Beta".into();
    lib.keep(p.clone());
    assert_eq!(lib.iter().next().unwrap().name, "Beta");
    p.name = "ALPHA".into();
    assert_eq!(lib.keep(p), Some(Kept::Replaced(alpha)));
    assert_eq!(lib.len(), 2);
    assert_eq!(lib.iter().next().unwrap().id, alpha);
    let second = lib.iter().nth(1).unwrap().id;
    assert!(lib.touch(second));
    assert_eq!(lib.iter().next().unwrap().name, "Beta");
    assert!(lib.remove(alpha).is_some());
    assert!(lib.get(alpha).is_none());
}

#[test]
fn a_full_library_refuses_a_new_name_but_takes_a_replacement() {
    let b = blueprints();
    let mut lib = Library::default();
    let mut p = capture(&b, &base(&b)).unwrap();
    for i in 0..MAX_PREFABS {
        p.name = format!("P{i}");
        assert!(lib.keep(p.clone()).is_some());
    }
    p.name = "One more".into();
    assert_eq!(lib.keep(p.clone()), None);
    p.name = "P3".into();
    assert!(matches!(lib.keep(p), Some(Kept::Replaced(_))));
}

#[test]
fn the_file_reads_back_what_was_written_and_survives_a_bad_one() {
    let b = blueprints();
    let dir = std::env::temp_dir().join(format!("mc-prefabs-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prefabs.ron");
    let mut lib = Library {
        path: Some(path.clone()),
        ..Library::default()
    };
    let p = capture(&b, &base(&b)).unwrap();
    lib.keep(p.clone());
    lib.save().unwrap();
    let read = read_file(&path).unwrap().unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].members, p.members);
    assert_eq!(read[0].name, p.name);

    // Rubbish is an error, never a panic.
    std::fs::write(&path, "(prefabs: [(name: 7").unwrap();
    assert!(read_file(&path).is_err());
    // A hand-edited prefab past the limits is left out on loading.
    let far = Prefab {
        name: "Far".into(),
        members: vec![Member {
            key: "aster_t1_power".into(),
            x: MAX_OFFSET + 1,
            y: 0,
        }],
        id: PrefabId::default(),
    };
    let mut lib = Library::default();
    lib.push_back(far);
    assert_eq!(lib.len(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_retired_blueprint_is_left_out() {
    let b = blueprints();
    let mut p = capture(&b, &base(&b)).unwrap();
    p.members[0].key = "aster_long_gone".into();
    let (members, gone) = p.resolve(&b);
    assert_eq!((members.len(), gone), (3, 1));
    assert_eq!(p.faction(&b), Some(b.unit(members[0].blueprint).faction));
}

#[test]
fn the_reason_left_out_is_the_commonest_counted() {
    let b = blueprints();
    let power = b.id_of("aster_t1_power").unwrap();
    let site = |fit, buildable| PrefabSite {
        blueprint: power,
        at: at(0, 0),
        fit,
        buildable,
    };
    let sites = [
        site(Ok(()), true),
        site(Err(Unfit::Taken), true),
        site(Err(Unfit::Taken), true),
        site(Err(Unfit::Water), true),
        site(Ok(()), false),
    ];
    assert_eq!(left_out_reason(&sites), "Lot taken (2), 2 more");
    assert_eq!(left_out_reason(&sites[..1]), "");
    assert_eq!(left_out_reason(&sites[4..]), "Not built by this selection");
}
