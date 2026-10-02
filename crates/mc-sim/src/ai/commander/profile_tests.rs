use super::*;
use std::path::Path;

fn roster() -> Blueprints {
    Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
}

fn profile(b: &Blueprints, key: &str) -> Profile {
    Profile::of(b.unit(b.id_of(key).unwrap()))
}

#[test]
fn roles_come_from_what_a_unit_can_do() {
    let b = roster();
    let has = |key: &str, r: u32| profile(&b, key).has(r);
    assert!(has("aster_t1_tank", role::LINE));
    assert!(has("aster_t1_mobile_aa", role::ANTI_AIR));
    assert!(has("aster_t1_lift_ship", role::TRANSPORT | role::WARP));
    assert!(has("aster_t2_lift_ship", role::TRANSPORT | role::PROJECT));
    assert!(has("aster_t1_sensor_ship", role::SENSOR | role::WARP));
    assert!(has("aster_t4_artillery", role::MAP_GUN | role::PROJECT));
    assert!(has("aster_t4_nuke_silo", role::STRATEGIC));
    assert!(has("aster_t3_nuke_defense", role::INTERCEPTOR));
    assert!(has("aster_t1_submarine", role::HUNTER | role::ANTI_SHIP));
    assert!(has("aster_t1_bomber", role::STRIKE));
    // The Regency's gunships: a drone carrier whose Wicks are its shells, and a beam craft.
    assert!(has("regency_t2_drone_carrier", role::STRIKE));
    assert!(has("regency_t3_assault_aircraft", role::STRIKE));
    assert!(has("aster_t4_anti_ship", role::ANTI_SPACE));
    // A mobile gun that shoots only at spacecraft is not marched with the line.
    assert!(has("regency_t3_mobile_aa", role::ANTI_SPACE));
    for r in [role::LINE, role::RAIDER, role::ANTI_AIR] {
        assert!(!has("regency_t3_mobile_aa", r));
    }
    assert!(has("regency_t3_artillery", role::ARTILLERY | role::SIEGE));
    assert!(!has("aster_t1_tank", role::ANTI_AIR));
    let sub = profile(&b, "aster_t1_submarine");
    assert_eq!(sub.is, Some(Target::Submerged));
    assert_eq!(sub.domain, Some(Domain::Sub));
}

/// The Regency's tech 2 aircraft take the same places in the plans as ARC's: the Pilum
/// hunts aircraft, the Trident ships under and on the water, the Voulge strikes the
/// ground, the Winnow salvages, and Skyforge II builds each of them.
#[test]
fn regency_tech_2_aircraft_take_arc_places() {
    let b = roster();
    let has = |key: &str, r: u32| profile(&b, key).has(r);
    for (regency, arc, r) in [
        (
            "regency_t2_interceptor",
            "aster_t2_interceptor",
            role::ANTI_AIR,
        ),
        (
            "regency_t2_torpedo_bomber",
            "aster_t2_torpedo_bomber",
            role::HUNTER | role::ANTI_SHIP,
        ),
        ("regency_t2_strike_drone", "aster_t1_bomber", role::STRIKE),
    ] {
        assert!(has(arc, r), "{arc}");
        assert!(has(regency, r), "{regency}");
    }
    let winnow = b.unit(b.id_of("regency_t2_reclaim_carrier").unwrap());
    assert!(winnow.is_salvager(), "the Winnow salvages");
    let skyforge = b.unit(b.id_of("regency_t2_air_factory").unwrap());
    let builds = &skyforge.builder.as_ref().unwrap().builds;
    for key in [
        "regency_t2_interceptor",
        "regency_t2_strike_drone",
        "regency_t2_torpedo_bomber",
        "regency_t2_reclaim_carrier",
    ] {
        assert!(
            builds.contains(&b.id_of(key).unwrap()),
            "Skyforge II builds {key}"
        );
    }
}

/// Every armed unit any race can field has a place in the Commander's plans: it
/// fights on the ground, in the air, at sea, under it, or shells from afar. A unit
/// that fits none would never be built. Run with `--nocapture` for the whole table.
#[test]
fn every_armed_unit_of_every_race_has_a_role() {
    let b = roster();
    let combat = role::LINE
        | role::RAIDER
        | role::ARTILLERY
        | role::ANTI_AIR
        | role::ANTI_SHIP
        | role::HUNTER
        | role::SIEGE
        | role::MAP_GUN
        | role::DEFENSE
        | role::STRIKE
        | role::ANTI_SPACE;
    let mut missing = vec![];
    for bp in &b.units {
        let p = Profile::of(bp);
        println!(
            "{:<32} t{} {:?} roles {:#07x} cost {:>7} dps land {:>6} air {:>6} ship {:>6} sub {:>6}",
            bp.key,
            p.tech,
            p.domain,
            p.roles,
            p.cost.floor_int(),
            p.dps[0].floor_int(),
            p.dps[2].floor_int(),
            p.dps[3].floor_int(),
            p.dps[4].floor_int(),
        );
        // Armed spaceships fight as a fleet of their own.
        let warship = p.domain == Some(Domain::Space) && p.armed();
        if p.armed() && p.roles & combat == 0 && !warship && !bp.has(cat::COMMANDER) {
            missing.push(bp.key.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "armed units no plan can use: {missing:?}"
    );
}

/// A Commander playing the Regency fields its own air force: its Skyforge makes a
/// fighter for the air guard, a bomber for strikes, an air scout and a salvage drone, and
/// its Exarch raises a transport for landings, all of them the Regency's own.
#[test]
fn the_regency_builds_its_own_air_force() {
    let b = roster();
    let made_by = |key: &str| -> Vec<BlueprintId> {
        b.unit(b.id_of(key).unwrap())
            .builder
            .as_ref()
            .unwrap()
            .builds
            .clone()
    };
    for factory in [
        "regency_t1_air_factory",
        "regency_t2_air_factory",
        "regency_t3_air_factory",
    ] {
        let made = made_by(factory);
        let regency_air = |want: &dyn Fn(&Profile) -> bool| {
            made.iter().any(|&id| {
                let bp = b.unit(id);
                let p = Profile::of(bp);
                bp.key.starts_with("regency_") && p.domain == Some(Domain::Air) && want(&p)
            })
        };
        assert!(
            regency_air(&|p| p.has(role::ANTI_AIR)),
            "{factory}: fighter"
        );
        assert!(regency_air(&|p| p.has(role::STRIKE)), "{factory}: bomber");
        assert!(regency_air(&|p| p.has(role::SCOUT)), "{factory}: scout");
        assert!(
            made.iter()
                .any(|&id| b.unit(id).key == "regency_t1_air_reclaimer"),
            "{factory}: salvage drone"
        );
        assert!(
            !made
                .iter()
                .any(|&id| b.unit(id).key == "aster_t1_rotor_gunship"),
            "{factory}: no tech 1 gunship"
        );
    }
    for builder in ["regency_commander", "regency_t1_engineer"] {
        assert!(
            made_by(builder).iter().any(|&id| {
                b.unit(id).key.starts_with("regency_")
                    && profile(&b, &b.unit(id).key).has(role::TRANSPORT)
            }),
            "{builder}: transport"
        );
    }
}
