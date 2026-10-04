use super::*;
use crate::tables::Controller;
use crate::world::MapData;
use crate::{AiConfig, MatchConfig, PlayerSetup};
use mc_core::Angle;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use std::{path::Path, sync::Arc};

fn world() -> World {
    let data = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let config = MatchConfig {
        seed: 11,
        cheats: true,
        fog: false,
        spawn_commanders: false,
        players: (0..2)
            .map(|i| PlayerSetup {
                name: format!("AI {i}"),
                faction: "Aster".into(),
                team: i,
                controller: Controller::Human,
                start: i,
                ai: AiConfig::default(),
            })
            .collect(),
    };
    World::with_terrain(
        Heightfield::flat(256, 256, Fx::from_int(20)),
        MapData {
            name: "salvage".into(),
            content_id: 11,
            ore: Vec::new(),
            starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
            props: vec![],
        },
        Arc::new(data),
        Arc::new(Pool::new(0)),
        &config,
    )
    .unwrap()
}

fn spawn(w: &mut World, key: &str, x: i32, y: i32) -> usize {
    w.spawn_unit(
        w.blueprints.id_of(key).unwrap(),
        0,
        FxVec2::from_ints(x, y),
        Angle::ZERO,
        true,
    )
    .unwrap()
}

/// A field of tank wrecks, `n` of them 100 mass each, round (x, y).
fn field(w: &mut World, x: i32, y: i32, n: i32) {
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    for i in 0..n {
        w.state
            .wrecks
            .spawn(
                tank,
                FxVec2::from_ints(x + (i % 4) * 30, y + (i / 4) * 30),
                Fx::from_int(20),
                Angle::ZERO,
                Fx::from_int(100),
                0,
            )
            .unwrap();
    }
}

#[test]
fn a_field_near_home_gets_a_tower_and_a_covered_one_does_not() {
    let mut w = world();
    field(&mut w, 700, 500, 8);
    field(&mut w, 1600, 900, 2);
    let engineer = spawn(&mut w, "aster_t1_engineer", 320, 320);
    let start = w.state.players[0].start;
    let census = w.survey_own(0);
    let intel = Intel::default();
    let mut planned = w.plan_counts(0, &census);
    planned.salvage = w.wreck_fields(start, &intel);
    assert_eq!(planned.salvage.len(), 2);
    assert_eq!(planned.salvage[0].mass, Fx::from_int(800), "richest first");
    let job = w
        .salvage_job(engineer, &planned, &|_| true)
        .expect("a tower for the big field");
    assert_eq!(w.blueprints.unit(job.blueprint).key, "aster_t1_reclaimer");
    assert!(job.near.distance(FxVec2::from_ints(740, 530)) < Fx::from_int(250));
    // A tower that reaches the big field: the small one is not worth another.
    planned
        .towers
        .push((FxVec2::from_ints(800, 400), Fx::from_int(640)));
    assert!(w.salvage_job(engineer, &planned, &|_| true).is_none());
}

#[test]
fn factories_make_a_cheap_salvager_while_wrecks_lie_about_and_idle_ones_go_to_them() {
    let mut w = world();
    let air = spawn(&mut w, "aster_t2_air_factory", 400, 300);
    let land = spawn(&mut w, "aster_t1_land_factory", 300, 420);
    let naval = spawn(&mut w, "aster_t3_naval_factory", 500, 500);
    let census = w.survey_own(0);
    assert!(
        w.salvage_product(air, &census, 0, &[]).is_none(),
        "no wrecks, no salvager"
    );
    field(&mut w, 900, 900, 6);
    let fields = w.wreck_fields(w.state.players[0].start, &Intel::default());
    let key = |id: Option<BlueprintId>| id.map(|b| w.blueprints.unit(b).key.clone());
    // Every factory, of any tier, makes the cheapest: the Gleaner (Reclaimer I).
    for factory in [air, land, naval] {
        assert_eq!(
            key(w.salvage_product(factory, &census, 0, &fields)).as_deref(),
            Some("aster_t1_mobile_reclaimer")
        );
    }
    assert!(
        w.salvage_product(land, &census, 1, &fields).is_none(),
        "one is enough for 600 mass"
    );
    // A Gleaner idle at home, out of reach of the field, is sent to it.
    let reclaimer = spawn(&mut w, "aster_t1_mobile_reclaimer", 320, 320);
    let census = w.survey_own(0);
    assert_eq!(census.salvagers_idle, vec![reclaimer]);
    let mut out = Vec::new();
    w.direct_salvagers(&census, &fields, &mut out);
    assert!(matches!(
        out.as_slice(),
        [crate::Command::Move { target, .. }] if target.distance(FxVec2::from_ints(930, 930)) < Fx::from_int(100)
    ));
}

/// The Regency's builders put up the Crucible, their own tower: the tech 1 one, the first in
/// every list, which upgrades in place.
#[test]
fn a_regency_builder_puts_up_its_own_tower() {
    let mut w = world();
    for builder in [
        "regency_t1_engineer",
        "regency_t2_engineer",
        "regency_t3_engineer",
    ] {
        let row = spawn(&mut w, builder, 320, 320);
        let pick = w.pick_reclaimer(row).expect(builder);
        assert_eq!(
            w.blueprints.unit(pick).key,
            "regency_t1_reclaimer",
            "{builder}"
        );
    }
}
