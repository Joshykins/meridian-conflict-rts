use super::*;

fn blueprints() -> Blueprints {
    Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

/// Kind, rank, dead zone and reach of each ring.
fn of(b: &Blueprints, key: &str) -> Vec<(Reach, u8, f32, f32)> {
    projections(b.unit(b.id_of(key).unwrap()))
        .into_iter()
        .map(|p| (p.reach, p.rank, p.inner, p.outer))
        .collect()
}

#[test]
fn rings_follow_the_data() {
    let b = blueprints();
    assert_eq!(
        of(&b, "aster_t1_scout"),
        vec![
            (Reach::Direct, 0, 0.0, 140.0),
            (Reach::Radar, 0, 0.0, 3000.0)
        ]
    );
    assert_eq!(
        of(&b, "aster_t1_tank"),
        vec![(Reach::Direct, 0, 0.0, 300.0)]
    );
    // A howitzer has a dead zone, a missile rack is its own kind.
    assert_eq!(
        of(&b, "aster_t1_artillery"),
        vec![(Reach::Indirect, 0, 60.0, 440.0)]
    );
    assert_eq!(
        of(&b, "aster_t2_missile"),
        vec![(Reach::Missile, 0, 120.0, 1400.0)]
    );
    // The Paladin's twin projectors are one ring; its shin tubes another.
    assert_eq!(
        of(&b, "aster_t3_assault_bot"),
        vec![
            (Reach::Direct, 0, 0.0, 400.0),
            (Reach::Torpedo, 0, 0.0, 600.0)
        ]
    );
    // A Reclaimer's reach is a reclaim ring.
    assert_eq!(
        of(&b, "aster_t1_mobile_reclaimer"),
        vec![(Reach::Reclaim, 0, 0.0, 550.0)]
    );
    assert_eq!(
        of(&b, "aster_t3_support"),
        vec![
            (Reach::Radar, 0, 0.0, 6000.0),
            (Reach::AntiMissile, 0, 0.0, 650.0),
            (Reach::Sonar, 0, 0.0, 900.0),
        ]
    );
    assert_eq!(
        of(&b, "aster_t1_radar"),
        vec![(Reach::Radar, 0, 0.0, 3000.0)]
    );
    assert_eq!(
        of(&b, "aster_t2_radar"),
        vec![(Reach::Radar, 0, 0.0, 6000.0)]
    );
    assert_eq!(
        of(&b, "aster_t3_radar"),
        vec![(Reach::Radar, 0, 0.0, 12000.0)]
    );
    assert_eq!(
        of(&b, "aster_t2_shield"),
        vec![(Reach::Shield, 0, 0.0, 115.0)]
    );
    assert_eq!(
        of(&b, "aster_t3_shield"),
        vec![(Reach::Shield, 0, 0.0, 205.0)]
    );
    // The Undertow's field, where an enemy jump is snagged.
    assert_eq!(
        of(&b, "aster_t2_warp_damper"),
        vec![(Reach::Damper, 0, 0.0, 1600.0)]
    );
    // The Exarch's Nanite Repair Field, where its side's units mend.
    assert!(of(&b, "regency_commander+nano_field").contains(&(Reach::Repair, 0, 0.0, 120.0)));
    assert_eq!(
        of(&b, "aster_commander"),
        vec![
            (Reach::Direct, 0, 0.0, 360.0),
            (Reach::Radar, 0, 0.0, 1500.0),
            (Reach::Build, 0, 0.0, 70.0)
        ]
    );
    // Factories build inside themselves; a reactor reaches nothing.
    assert!(of(&b, "aster_t1_land_factory").is_empty());
    assert!(of(&b, "aster_t1_air_factory").is_empty());
    assert!(of(&b, "aster_t1_power").is_empty());
    assert_eq!(
        of(&b, "aster_t1_interceptor"),
        vec![(Reach::AntiAir, 0, 0.0, 220.0)]
    );
    assert_eq!(
        of(&b, "aster_t1_bomber"),
        vec![(Reach::Indirect, 0, 0.0, 320.0)]
    );
    // A battleship's guns are laid flat (`flat_fire`): lobbed, but direct fire, main
    // batteries and secondaries alike.
    let battleship = of(&b, "aster_t3_battleship");
    assert!(
        battleship.iter().all(|r| r.0 != Reach::Indirect),
        "{battleship:?}"
    );
    assert!(
        battleship.contains(&(Reach::Direct, 0, 150.0, 2520.0)),
        "{battleship:?}"
    );
    assert!(
        battleship
            .iter()
            .any(|r| r.0 == Reach::Direct && r.3 == 780.0),
        "{battleship:?}"
    );
}

/// A second gun of the same kind with less reach is its own ring, one rank down.
#[test]
fn shorter_guns_of_a_kind_rank_below_the_farthest() {
    let b = blueprints();
    let mut bp = b.unit(b.id_of("aster_t1_tank").unwrap()).clone();
    let scout = b.unit(b.id_of("aster_t1_scout").unwrap());
    bp.weapons.push(scout.weapons[0].clone());
    bp.weapons.push(bp.weapons[0].clone());
    let all = projections(&bp);
    let got: Vec<_> = all
        .iter()
        .map(|p| (p.reach, p.rank, p.inner, p.outer))
        .collect();
    assert_eq!(
        got,
        vec![
            (Reach::Direct, 0, 0.0, 300.0),
            (Reach::Direct, 1, 0.0, 140.0)
        ]
    );
    // The twin is not named twice; the ranks merge apart.
    assert!(!all[0].name.contains('\u{b7}'));
    assert_ne!(all[0].group(), all[1].group());
    assert_eq!(kind_of(all[1].group()), Reach::Direct as u32);
}

fn unit_at(x: f32, y: f32) -> UnitInstance {
    UnitInstance {
        prev_pos: [x, y, 0.0],
        prev_heading: 0.0,
        pos: [x, y, 0.0],
        heading: 0.0,
        blueprint: 0,
        owner_flags: 0,
        health: 1.0,
        build: 1.0,
        turret_yaw: 0.0,
        radius: 3.0,
        unit_id: 1,
        packed: 0,
        gait: [0.0; 3],
        upgrade: 0.0,
        arm_pitch: [0.0; 4],
        prev_turret_yaw: 0.0,
        weld: [0.0; 3],
        recoil: 0.0,
        prev_recoil: 0.0,
        weld_first: 0,
        weld_count: 0,
        deploy: 0.0,
        prev_deploy: 0.0,
        _pad2: [0.0; 2],
        refit_modules: 0,
        status: [0; 3],
        mount: [0.0; 4],
        spin_recoil: [0.0; 4],
        fx: [0.0; 4],
        drive_swing: [0.0; 2],
        _pad3: [0.0; 2],
    }
}

/// A warship's spinal gun cannot dive onto what is in under its hull: its ring has a
/// dead zone as wide as the sim's (`combat::spinal_dead_zone`), and a higher hull's is wider.
#[test]
fn a_spinal_gun_shows_the_dead_zone_under_its_hull() {
    let b = blueprints();
    let mut rings = Rings::new(&b);
    let key = "aster_t4_frigate";
    let bp = b.unit(b.id_of(key).unwrap());
    let (range, bore) = (
        bp.weapons[0].range_max.to_f32(),
        bp.weapons[0].muzzle.z.to_f32(),
    );
    let at = |z: f32| {
        let mut u = UnitInstance {
            blueprint: b.id_of(key).unwrap().0 as u32,
            ..unit_at(100.0, 100.0)
        };
        u.pos[2] = z;
        u.prev_pos[2] = z;
        u
    };
    let spinal = |z: f32, rings: &mut Rings| {
        let (all, _) = rings.collect([&at(z)].into_iter(), 1.0, true, &|_| 20.0);
        all.into_iter().find(|r| r.outer == range).unwrap().inner
    };
    let high = spinal(580.0, &mut rings);
    let low = spinal(220.0, &mut rings);
    assert_eq!(high, mc_sim::combat::spinal_dead_zone(560.0, bore).round());
    assert!(high > low && low > 0.0, "{low} {high}");
}

/// A gun that cannot turn all the way round projects a wedge: the Hellkite's tail gun faces aft.
#[test]
fn limited_guns_project_a_wedge() {
    let b = blueprints();
    let hellkite = projections(b.unit(b.id_of("aster_t2_fire_bomber").unwrap()));
    let tail = hellkite.iter().find(|p| p.name == "Tail AA").unwrap();
    let arc = tail.arc.unwrap();
    assert!(
        arc.aft && (arc.half.to_degrees() - 75.0).abs() < 0.5,
        "{arc:?}"
    );
    assert_eq!(arc.label(), "150\u{b0} Aft");
    let dorsal = hellkite.iter().find(|p| p.name == "Dorsal AA").unwrap();
    assert_eq!((dorsal.arc, dorsal.rank), (None, 0));

    // The wedge turns with the hull: a unit facing north puts its aft arc south.
    let mut rings = Rings::new(&b);
    let bp = b.id_of("aster_t2_fire_bomber").unwrap().0 as u32;
    let north = std::f32::consts::FRAC_PI_2;
    let unit = UnitInstance {
        blueprint: bp,
        prev_heading: north,
        heading: north,
        ..unit_at(0.0, 0.0)
    };
    let (all, _) = rings.collect([&unit].into_iter(), 1.0, true, &|_| 0.0);
    let aft = all
        .iter()
        .find(|r| r.half_arc < std::f32::consts::PI)
        .unwrap();
    assert!((aft.facing - (north + std::f32::consts::PI)).abs() < 1e-4);
    assert!(
        all.iter()
            .filter(|r| r.half_arc >= std::f32::consts::PI)
            .count()
            >= 2
    );
}

#[test]
fn rings_sit_between_ticks_and_never_outgrow_the_renderer() {
    let b = blueprints();
    let mut rings = Rings::new(&b);
    let tank = b.id_of("aster_t1_tank").unwrap().0 as u32;
    let unit = UnitInstance {
        prev_pos: [10.0, 20.0, 0.0],
        prev_heading: 0.0,
        pos: [20.0, 40.0, 0.0],
        heading: 0.0,
        blueprint: tank,
        owner_flags: 0,
        health: 1.0,
        build: 1.0,
        turret_yaw: 0.0,
        radius: 3.0,
        unit_id: 1,
        packed: 0,
        gait: [0.0; 3],
        upgrade: 0.0,
        arm_pitch: [0.0; 4],
        prev_turret_yaw: 0.0,
        weld: [0.0; 3],
        recoil: 0.0,
        prev_recoil: 0.0,
        weld_first: 0,
        weld_count: 0,
        deploy: 0.0,
        prev_deploy: 0.0,
        _pad2: [0.0; 2],
        refit_modules: 0,
        status: [0; 3],
        mount: [0.0; 4],
        spin_recoil: [0.0; 4],
        fx: [0.0; 4],
        drive_swing: [0.0; 2],
        _pad3: [0.0; 2],
    };
    let (one, drawn) = rings.collect([&unit].into_iter(), 0.5, true, &|_| 0.0);
    assert_eq!((one.len(), drawn), (1, 1));
    assert_eq!(one[0].center, [15.0, 30.0]);
    assert_eq!(Rings::key(&one), vec![(Reach::Direct, 0, 0.0, 300.0)]);

    let wreck = UnitInstance {
        owner_flags: KIND_WRECK,
        ..unit
    };
    assert!(rings
        .collect([&wreck].into_iter(), 0.5, true, &|_| 0.0)
        .0
        .is_empty());
    let army = vec![unit; MAX_RANGES + 40];
    assert_eq!(
        rings.collect(army.iter(), 0.0, true, &|_| 0.0).0.len(),
        MAX_RANGES
    );
}

/// A block of tanks draws the rings on its edge and keeps the rest as masks.
#[test]
fn rings_buried_in_a_blob_are_not_drawn() {
    let b = blueprints();
    let mut rings = Rings::new(&b);
    let tank = b.id_of("aster_t1_tank").unwrap().0 as u32;
    let at = |x: f32, y: f32| UnitInstance {
        prev_pos: [x, y, 0.0],
        prev_heading: 0.0,
        pos: [x, y, 0.0],
        heading: 0.0,
        blueprint: tank,
        owner_flags: 0,
        health: 1.0,
        build: 1.0,
        turret_yaw: 0.0,
        radius: 3.0,
        unit_id: 1,
        packed: 0,
        gait: [0.0; 3],
        upgrade: 0.0,
        arm_pitch: [0.0; 4],
        prev_turret_yaw: 0.0,
        weld: [0.0; 3],
        recoil: 0.0,
        prev_recoil: 0.0,
        weld_first: 0,
        weld_count: 0,
        deploy: 0.0,
        prev_deploy: 0.0,
        _pad2: [0.0; 2],
        refit_modules: 0,
        status: [0; 3],
        mount: [0.0; 4],
        spin_recoil: [0.0; 4],
        fx: [0.0; 4],
        drive_swing: [0.0; 2],
        _pad3: [0.0; 2],
    };
    let block: Vec<UnitInstance> = (0..15)
        .flat_map(|x| (0..15).map(move |y| at(1000.0 + x as f32 * 20.0, 1000.0 + y as f32 * 20.0)))
        .collect();
    let (all, drawn) = rings.collect(block.iter(), 1.0, true, &|_| 0.0);
    assert_eq!(all.len(), 225);
    assert!((4..120).contains(&drawn), "{drawn} of 225 drawn");
    // The corners are on the outline, and the block's middle is not.
    let is_drawn = |x: f32, y: f32| all[..drawn].iter().any(|r| r.center == [x, y]);
    assert!(is_drawn(1000.0, 1000.0) && is_drawn(1280.0, 1280.0));
    assert!(!is_drawn(1140.0, 1140.0));

    // Spread out of each other's reach, every ring shows.
    let line: Vec<UnitInstance> = (0..40)
        .map(|i| at(1000.0 + i as f32 * 700.0, 1000.0))
        .collect();
    assert_eq!(rings.collect(line.iter(), 1.0, true, &|_| 0.0).1, 40);

    // Between ticks the last answer stands; something being placed is drawn wherever it is.
    assert_eq!(rings.collect(block.iter(), 0.5, false, &|_| 0.0).1, drawn);
    let ghost = UnitInstance {
        owner_flags: KIND_GHOST,
        unit_id: u32::MAX,
        ..at(1140.0, 1140.0)
    };
    let (with_ghost, _) =
        rings.collect([&ghost].into_iter().chain(block.iter()), 1.0, true, &|_| {
            0.0
        });
    assert_eq!(with_ghost[0].center, [1140.0, 1140.0]);
}

/// Placing a radar brings in the radar ring of each of our standing radar posts, one group
/// so the renderer merges them; nothing else's, and nothing while placing something else.
#[test]
fn placing_a_radar_shows_our_radar_network() {
    let b = blueprints();
    let id = |key: &str| b.id_of(key).unwrap();
    let post = |key: &str, x: f32, owner: u32, build: f32| UnitInstance {
        blueprint: id(key).0 as u32,
        owner_flags: owner,
        build,
        ..unit_at(x, 0.0)
    };
    let units = [
        post("aster_t1_radar", 0.0, 0, 1.0),
        post("aster_t3_radar", 100.0, 0, 1.0),
        // An enemy's, one still being built, a wreck and a tank: none of the network.
        post("aster_t1_radar", 200.0, 1, 1.0),
        post("aster_t1_radar", 300.0, 0, 0.5),
        post("aster_t1_radar", 400.0, KIND_WRECK, 1.0),
        post("aster_t1_tank", 500.0, 0, 1.0),
    ];
    let network = cover_network(&b, id("aster_t2_radar"), None, 0, units.iter());
    let reach: Vec<(f32, f32)> = network.iter().map(|r| (r.center[0], r.outer)).collect();
    assert_eq!(reach, [(0.0, 3000.0), (100.0, 12000.0)]);
    assert!(network.iter().all(|r| r.group == Reach::Radar as u32));

    // A headless shot draws no ghost: the site's own ring comes last.
    let with_site = cover_network(&b, id("aster_t2_radar"), Some([9.0, 9.0]), 0, units.iter());
    assert_eq!(
        with_site.last().map(|r| (r.center, r.outer)),
        Some(([9.0, 9.0], 6000.0))
    );

    assert!(cover_network(&b, id("aster_t1_tank"), None, 0, units.iter()).is_empty());
}

/// Sonar buoys and missile defences show their networks the same way, each only its own
/// kind: a radar is not part of the sonar network.
#[test]
fn placing_sonar_or_missile_defence_shows_that_network() {
    let b = blueprints();
    let id = |key: &str| b.id_of(key).unwrap();
    let post = |key: &str, x: f32| UnitInstance {
        blueprint: id(key).0 as u32,
        owner_flags: 0,
        build: 1.0,
        ..unit_at(x, 0.0)
    };
    let units = [
        post("aster_t1_radar", 0.0),
        post("aster_t1_sonar", 100.0),
        post("aster_t3_sonar", 200.0),
        post("aster_t2_missile_defense", 300.0),
    ];
    let reach = |placing: &str| -> Vec<(f32, f32, u32)> {
        cover_network(&b, id(placing), None, 0, units.iter())
            .iter()
            .map(|r| (r.center[0], r.outer, r.group))
            .collect()
    };
    let sonar = Reach::Sonar as u32;
    assert_eq!(
        reach("aster_t2_sonar"),
        [(100.0, 1600.0, sonar), (200.0, 6400.0, sonar)]
    );
    let am = Reach::AntiMissile as u32;
    assert_eq!(reach("aster_t3_missile_defense"), [(300.0, 300.0, am)]);
}
