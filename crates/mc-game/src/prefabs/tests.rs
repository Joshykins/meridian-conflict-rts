use super::*;

fn blueprints() -> Blueprints {
    Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

fn at(x: i32, y: i32) -> FxVec2 {
    FxVec2::from_ints(x, y)
}

/// A structure of `key` at `p`, unturned.
fn laid(b: &Blueprints, key: &str, p: FxVec2) -> Laid {
    let blueprint = b.id_of(key).unwrap();
    Laid {
        blueprint,
        at: p,
        heading: b.unit(blueprint).build_heading(),
    }
}

/// A factory (8x8) with two generators (2x2) and a storage (3x3) beside it.
fn base(b: &Blueprints) -> Vec<Laid> {
    vec![
        laid(b, "aster_t1_land_factory", at(600, 600)),
        laid(b, "aster_t1_power", at(660, 600)),
        laid(b, "aster_t1_power", at(660, 624)),
        laid(b, "aster_materials_storage", at(666, 570)),
    ]
}

/// Where the laid-out members stand, sorted, without which way they face.
fn spots(sites: &[Laid]) -> Vec<(BlueprintId, FxVec2)> {
    let mut out: Vec<_> = sites.iter().map(|s| (s.blueprint, s.at)).collect();
    out.sort_by_key(|(id, p)| (id.0, p.x, p.y));
    out
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
    let placed = layout(&b, &members, middle, Turn::default());
    let want: Vec<_> = sites
        .iter()
        .map(|s| Laid {
            at: mc_sim::world::snap_to_build_grid(b.unit(s.blueprint), s.at),
            ..*s
        })
        .collect();
    assert_eq!(spots(&placed), spots(&want));
}

#[test]
fn moved_anywhere_it_keeps_its_shape() {
    let b = blueprints();
    let p = capture(&b, &base(&b)).unwrap();
    let (members, _) = p.resolve(&b);
    let here = layout(&b, &members, at(1000, 1000), Turn::default());
    // Any point snaps to a cell corner first, so the shape never shifts by a cell.
    for q in [at(2003, 517), at(1210, 1195), at(15, 4000)] {
        let there = layout(&b, &members, q, Turn::default());
        let d = there[0].at - here[0].at;
        for (a, b) in here.iter().zip(&there) {
            assert_eq!(b.at - a.at, d, "member moved apart from the others");
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
    for (quarters, mirrored) in (0..4).flat_map(|q| [(q, false), (q, true)]) {
        let q = Turn { quarters, mirrored };
        let sites = layout(&b, &members, at(1200, 1200), q);
        for (i, sa) in sites.iter().enumerate() {
            for sb in &sites[i + 1..] {
                let fa = b.unit(sa.blueprint).footprint_at(sa.heading);
                let fb = b.unit(sb.blueprint).footprint_at(sb.heading);
                let (pa, pb) = (sa.at, sb.at);
                let reach = |x: u8, y: u8| (x as i32 + y as i32) * mc_map::BUILD_CELL_M / 2;
                let d = pa - pb;
                let apart = d.x.abs().round_int() >= reach(fa.0, fb.0)
                    || d.y.abs().round_int() >= reach(fa.1, fb.1);
                assert!(apart, "{q:?}: lots overlap");
            }
        }
    }
}

#[test]
fn capture_refuses_nothing_and_too_much() {
    let b = blueprints();
    assert_eq!(capture(&b, &[]), Err(Uncaptured::Nothing));
    let commander = b.factions[0].commander;
    let key = |id: BlueprintId| b.unit(id).key.clone();
    assert_eq!(
        capture(&b, &[laid(&b, &key(commander), at(100, 100))]),
        Err(Uncaptured::Nothing)
    );
    let many: Vec<_> = (0..MAX_MEMBERS as i32 + 1)
        .map(|i| {
            laid(
                &b,
                "aster_t1_power",
                at(100 + 24 * (i % 10), 100 + 24 * (i / 10)),
            )
        })
        .collect();
    assert_eq!(
        capture(&b, &many),
        Err(Uncaptured::TooMany(MAX_MEMBERS + 1))
    );
    // The same structure on the same lot twice (built and planned) counts once.
    let power = laid(&b, "aster_t1_power", at(100, 100));
    let twice = [power, power];
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
            turn: 0,
        }],
        category: None,
        icon: None,
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
        heading: Angle::ZERO,
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

/// A layout with an odd-sized lot (the storage, 3x3) off the middle, a power plant
/// turned a quarter, and a factory unturned.
fn turned_base(b: &Blueprints) -> Vec<Resolved> {
    let mut sites = base(b);
    sites[1].heading = sites[1].heading + Angle::QUARTER_TURN;
    let (members, _) = capture(b, &sites).unwrap().resolve(b);
    members
}

#[test]
fn mirrored_and_turned_lots_land_on_the_grid_and_face_their_mirrored_way() {
    let b = blueprints();
    let members = turned_base(&b);
    assert!(members.iter().any(|m| m.turn == 1), "the turn is kept");
    let anchor = at(1200, 1200);
    let plain = layout(&b, &members, anchor, Turn::default());
    for quarters in 0..4 {
        let turn = Turn {
            quarters,
            mirrored: true,
        };
        let sites = layout(&b, &members, anchor, turn);
        for (m, (s, p)) in members.iter().zip(sites.iter().zip(&plain)) {
            let bp = b.unit(m.blueprint);
            // On the grid as the sim snaps it, and facing a quarter turn of its build heading.
            assert_eq!(
                mc_sim::world::snap_turned_to_build_grid(bp, s.at, s.heading),
                s.at,
                "{}",
                bp.key
            );
            assert_eq!(bp.lot_heading(s.heading), s.heading);
            // The mirror of the unturned layout, then turned: x flipped about the
            // middle, a heading h now a half turn less h.
            let snapped = snap_anchor(anchor);
            let (dx, dy) = (
                (p.at.x - snapped.x).round_int(),
                (p.at.y - snapped.y).round_int(),
            );
            let (x, y) = rotate(-dx, dy, quarters);
            assert_eq!(
                s.at,
                FxVec2::new(snapped.x + Fx::from_int(x), snapped.y + Fx::from_int(y)),
                "{}",
                bp.key
            );
            let want =
                Angle::HALF_TURN - p.heading + Angle(Angle::QUARTER_TURN.0 * quarters as u16);
            assert_eq!(s.heading, bp.lot_heading(want), "{}", bp.key);
        }
    }
}

#[test]
fn mirroring_twice_gives_the_layout_back() {
    let b = blueprints();
    let members = turned_base(&b);
    for quarters in 0..4 {
        let plain = Turn {
            quarters,
            mirrored: false,
        };
        let mut twice = plain;
        twice.mirrored = !twice.mirrored;
        twice.mirrored = !twice.mirrored;
        assert_eq!(twice, plain);
        // And mirrored about the middle, a mirrored layout mirrored again is the original:
        // a member's offset and heading both come back.
        for m in &members {
            let h = m.heading(&b);
            let once = Turn {
                quarters: 0,
                mirrored: true,
            }
            .place(m.x, m.y, h);
            let back = Turn {
                quarters: 0,
                mirrored: true,
            }
            .place(once.0, once.1, once.2);
            assert_eq!(back, (m.x, m.y, h));
        }
    }
}

#[test]
fn place_turn_reads_quarters_and_a_mirror() {
    assert_eq!(Turn::parse("0"), Some(Turn::default()));
    assert_eq!(
        Turn::parse("3m"),
        Some(Turn {
            quarters: 3,
            mirrored: true
        })
    );
    assert_eq!(Turn::parse("4"), None);
    assert_eq!(Turn::parse("m"), None);
}

#[test]
fn members_keep_the_order_they_were_built_in() {
    let b = blueprints();
    // Built storage first, factory last: not the order a page reads them in.
    let mut sites = base(&b);
    sites.reverse();
    let p = capture(&b, &sites).unwrap();
    let keys: Vec<&str> = p.members.iter().map(|m| m.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "aster_materials_storage",
            "aster_t1_power",
            "aster_t1_power",
            "aster_t1_land_factory"
        ]
    );
    // Laid out, they come in the same order.
    let (members, _) = p.resolve(&b);
    let laid = layout(&b, &members, at(1000, 1000), Turn::default());
    let ids: Vec<BlueprintId> = laid.iter().map(|l| l.blueprint).collect();
    let want: Vec<BlueprintId> = sites.iter().map(|s| s.blueprint).collect();
    assert_eq!(ids, want);
}

#[test]
fn build_order_ranks_by_first_seen_and_a_reused_id_is_new() {
    use mc_sim::mirror::UnitInstance;
    let b = blueprints();
    let power = b.id_of("aster_t1_power").unwrap();
    let unit = |id: u32| UnitInstance {
        unit_id: id,
        blueprint: power.0 as u32,
        ..bytemuck::Zeroable::zeroed()
    };
    let mut order = BuildOrder::default();
    // Slot 9 first, then slot 2: ids alone would rank 2 first.
    order.observe(&[unit(9)], 0, &b);
    order.observe(&[unit(9), unit(2)], 0, &b);
    assert!(order.rank(9) < order.rank(2));
    // Another player's, and one never seen, rank after.
    let mut theirs = unit(5);
    theirs.owner_flags = 1;
    order.observe(&[unit(9), unit(2), theirs], 0, &b);
    assert_eq!(order.rank(5), u64::MAX);
    // Gone and back (a new unit in the slot): begun after the others.
    order.observe(&[unit(2)], 0, &b);
    order.observe(&[unit(2), unit(9)], 0, &b);
    assert!(order.rank(2) < order.rank(9));
}

#[test]
fn shelf_icon_and_tier_come_from_the_costliest_structure_unless_picked() {
    let b = blueprints();
    let p = capture(&b, &base(&b)[..1]).unwrap();
    assert_eq!(p.category(&b), Category::Factories);
    assert_eq!(p.badge(&b), Badge::Factory);
    assert_eq!(p.tech(&b), 1);
    // The base's generators and storage together cost more than its factory, but the
    // factory is its costliest structure: a factory prefab.
    let whole = capture(&b, &base(&b)).unwrap();
    assert_eq!(whole.category(&b), Category::Factories);
    assert_eq!(whole.badge(&b), Badge::Factory);
    let mut picked = p.clone();
    picked.category = Some(Category::Economy);
    picked.icon = Some(Badge::Power);
    assert_eq!(picked.category(&b), Category::Economy);
    assert_eq!(picked.badge(&b), Badge::Power);
    // Power alone goes on the economy shelf.
    let power = capture(&b, &[laid(&b, "aster_t1_power", at(600, 600))]).unwrap();
    assert_eq!(power.category(&b), Category::Economy);
    assert_eq!(power.badge(&b), Badge::Power);
}

#[test]
fn a_shelf_and_icon_picked_are_saved_and_old_files_still_read() {
    let b = blueprints();
    let dir = std::env::temp_dir().join(format!("mc-prefab-shelf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prefabs.ron");
    let mut lib = Library {
        path: Some(path.clone()),
        ..Library::default()
    };
    let mut p = capture(&b, &base(&b)).unwrap();
    p.category = Some(Category::Defense);
    p.icon = Some(Badge::Shield);
    lib.keep(p);
    lib.save().unwrap();
    let read = read_file(&path).unwrap().unwrap();
    assert_eq!(read[0].category, Some(Category::Defense));
    assert_eq!(read[0].icon, Some(Badge::Shield));
    // A file from before shelves and icons: its prefabs take their own.
    std::fs::write(
        &path,
        "(prefabs: [(name: \"Old\", members: [(key: \"aster_t1_power\", x: 0, y: 0)])])",
    )
    .unwrap();
    let read = read_file(&path).unwrap().unwrap();
    assert_eq!((read[0].category, read[0].icon), (None, None));
    assert_eq!(read[0].category(&b), Category::Economy);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_edit_keeps_its_place_and_refuses_another_prefabs_name() {
    let b = blueprints();
    let mut lib = Library::default();
    let mut a = capture(&b, &base(&b)).unwrap();
    a.name = "A".into();
    let mut c = a.clone();
    c.name = "C".into();
    let Some(Kept::Added(a_id)) = lib.keep(a) else {
        panic!("kept")
    };
    lib.keep(c);
    let mut edit = lib.get(a_id).unwrap().clone();
    edit.name = "c".into();
    assert!(!lib.update(edit.clone()), "C is taken");
    edit.name = "Base".into();
    edit.category = Some(Category::Intel);
    assert!(lib.update(edit));
    let names: Vec<&str> = lib.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["C", "Base"]);
    assert_eq!(lib.get(a_id).unwrap().category, Some(Category::Intel));
}
