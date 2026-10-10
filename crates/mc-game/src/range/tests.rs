use super::*;

fn blueprints() -> Blueprints {
    Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

/// Every scenario either stages or says why not, for every unit there is.
#[test]
fn every_unit_can_be_the_subject() {
    let b = blueprints();
    let pad = FxVec2::from_ints(2000, 2000);
    for bp in b.units.iter().filter(|bp| b.is_listed(bp.id)) {
        for s in Scenario::ALL {
            match stage(s, &b, pad, bp.id) {
                Ok((commands, owed)) => assert!(
                    !commands.is_empty() || owed.is_some(),
                    "{} / {s:?} staged nothing",
                    bp.key
                ),
                Err(why) => assert!(!why.is_empty()),
            }
        }
    }
    let tank = b.id_of(DEFAULT_SUBJECT).unwrap();
    let cannot: Vec<Scenario> = Scenario::ALL
        .into_iter()
        .filter(|s| stage(*s, &b, pad, tank).is_err())
        .collect();
    assert_eq!(
        cannot,
        [
            Scenario::AtWork,
            Scenario::Salvage,
            Scenario::Refit,
            Scenario::Lift,
            Scenario::Warp,
            Scenario::WarpDampened,
            Scenario::Short,
            Scenario::Elevator,
            Scenario::Dash,
            Scenario::Pack,
            Scenario::Fracture,
            Scenario::FractureLetGo
        ],
        "the default subject supports everything a tank can do"
    );
    let commander = b.unit_by_key("aster_commander").unwrap().id;
    let cannot: Vec<Scenario> = Scenario::ALL
        .into_iter()
        .filter(|s| stage(*s, &b, pad, commander).is_err())
        .collect();
    assert_eq!(
        cannot,
        [
            Scenario::BuildIt,
            Scenario::Lift,
            Scenario::Warp,
            Scenario::WarpDampened,
            Scenario::Short,
            Scenario::Elevator,
            Scenario::Dash,
            Scenario::Pack,
            Scenario::Fracture,
            Scenario::FractureLetGo
        ],
        "nothing builds a commander, and it draws no energy; it does everything else"
    );
}

#[test]
fn attackers_and_targets_stand_inside_weapon_range() {
    let b = blueprints();
    let pad = FxVec2::from_ints(2000, 2000);
    let tank = b.unit(b.id_of(DEFAULT_SUBJECT).unwrap());
    let (commands, _) = stage(Scenario::Targets, &b, pad, tank.id).unwrap();
    for c in commands {
        let Command::DebugSpawn { pos, .. } = c else {
            panic!()
        };
        assert!(pos.distance(pad) < tank.weapons[0].range_max);
    }
}

#[test]
fn a_map_gun_gets_targets_a_few_kilometres_out() {
    let b = blueprints();
    let pad = FxVec2::from_ints(2000, 2000);
    let gun = b.unit(b.id_of("aster_t4_artillery").unwrap());
    let (commands, _) = stage(Scenario::Targets, &b, pad, gun.id).unwrap();
    assert!(!commands.is_empty());
    for c in commands {
        let Command::DebugSpawn { pos, blueprint, .. } = c else {
            panic!()
        };
        assert!(b.unit(blueprint).categories & gun.weapons[0].target_mask != 0);
        let gap = pos.distance(pad);
        assert!(gap > gun.weapons[0].range_min && gap < Fx::from_int(4000));
    }
}

#[test]
fn spawn_subject_places_the_subject() {
    let b = blueprints();
    let shield = b.id_of("aster_t2_shield").unwrap();
    assert_ne!(shield, b.id_of(DEFAULT_SUBJECT).unwrap());
    let range = Range::new(FxVec2::from_ints(2000, 2000), shield);
    let Command::DebugSpawn { blueprint, .. } = range.spawn_at(range.pad, &b) else {
        panic!()
    };
    assert_eq!(blueprint, shield);
}

#[test]
fn line_up_places_each_unit_once_inside_the_map() {
    let b = blueprints();
    let tank = b.id_of(DEFAULT_SUBJECT).unwrap();
    let mut range = Range::new(FxVec2::from_ints(100, 100), tank);
    let all: Vec<BlueprintId> = b
        .units
        .iter()
        .filter(|u| b.is_listed(u.id))
        .map(|u| u.id)
        .collect();
    let bounds = FxVec2::from_ints(4096, 4096);
    let wet = |id: BlueprintId| {
        let u = b.unit(id);
        u.water_only()
            || u.motion
                .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval)
    };
    let shown = |l: &LineUp| -> Vec<(BlueprintId, [f32; 2])> {
        l.commands
            .iter()
            .filter_map(|c| match c {
                Command::DebugSpawn {
                    blueprint,
                    pos,
                    count: 1,
                    owner: BLUE,
                    ..
                } => Some((*blueprint, pos.to_f32())),
                Command::DebugSpawn { .. } => panic!("one Blue unit each"),
                _ => None,
            })
            .collect()
    };
    let dry = range.line_up(&b, &all, None, bounds);
    assert!(matches!(dry.commands[0], Command::DebugClear));
    let placed = shown(&dry);
    assert_eq!(dry.stranded, all.iter().filter(|&&id| wet(id)).count());
    assert_eq!(placed.len() + dry.stranded, all.len());
    // Near the map's corner, the block is pushed back inside it.
    assert!(placed
        .iter()
        .all(|(_, [x, y])| *x > 0.0 && *y > 0.0 && *x < 4096.0 && *y < 4096.0));
    for (i, (_, p)) in placed.iter().enumerate() {
        for (_, q) in &placed[i + 1..] {
            assert!(
                (p[0] - q[0]).hypot(p[1] - q[1]) > 4.0,
                "two units on one spot"
            );
        }
    }
    let wet_sea = range.line_up(&b, &all, Some(FxVec2::from_ints(3000, 3000)), bounds);
    assert_eq!(wet_sea.stranded, 0);
    assert_eq!(shown(&wet_sea).len(), all.len());
}

/// The elevator scenario queues a mixed batch on a space elevator, which its first
/// platform can carry; nothing else can take it.
#[test]
fn the_elevator_scenario_queues_a_mixed_batch() {
    let b = blueprints();
    let pad = FxVec2::from_ints(2000, 2000);
    let elevators: Vec<&UnitBlueprint> = b
        .units
        .iter()
        .filter(|bp| b.is_listed(bp.id) && bp.elevator.is_some())
        .collect();
    assert!(!elevators.is_empty(), "there is a space elevator to try");
    for bp in elevators {
        let Ok((_, Some((who, PendingOrder::ProduceMix { batch, .. })))) =
            stage(Scenario::Elevator, &b, pad, bp.id)
        else {
            panic!("{} stages its batch", bp.key);
        };
        assert_eq!(who, bp.id, "the batch is queued on the elevator itself");
        let payload = bp.elevator.unwrap().payload;
        let mass = batch
            .iter()
            .map(|&(id, n)| b.unit(id).cost_mass * n as i32)
            .fold(Fx::ZERO, |a, m| a + m);
        assert!(mass <= payload, "{}: the batch rides one platform", bp.key);
        assert!(
            batch.iter().any(|&(id, _)| b.unit(id).tech >= 4),
            "a tech-4 machine is in it"
        );
    }
}

#[test]
fn a_dash_pack_stands_where_the_seraph_dashes() {
    let b = blueprints();
    let pad = FxVec2::from_ints(2000, 2000);
    let seraph = b.unit(b.id_of("regency_t5_seraph").unwrap());
    let dash = seraph.dash.as_ref().unwrap();
    let (commands, _) = stage(Scenario::Dash, &b, pad, seraph.id).unwrap();
    assert_eq!(commands.len(), 7);
    for c in commands {
        let Command::DebugSpawn { pos, .. } = c else {
            panic!()
        };
        let d = pos.distance(pad);
        assert!(d > dash.min && d < dash.range, "{d:?}");
    }
}
