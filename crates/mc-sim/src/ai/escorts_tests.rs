use super::*;
use crate::ai::commander::state::PlanKind;
use crate::tables::Controller;
use crate::world::MapData;
use crate::{AiConfig, MatchConfig, PlayerCommand, PlayerSetup};
use mc_core::{Angle, FxVec2};
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
                faction: "Regency".into(),
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
            name: "escorts".into(),
            content_id: 12,
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

#[test]
fn a_big_army_gets_a_scythe_that_guards_its_lead() {
    let mut w = world();
    let skyforge = spawn(&mut w, "regency_t3_air_factory", 400, 300);
    let key = |w: &World, id: Option<BlueprintId>| id.map(|b| w.blueprints.unit(b).key.clone());
    let census = w.survey_own(0);
    assert!(
        w.escort_product(0, skyforge, &census, 0).is_none(),
        "no army, no scavenger to go with it"
    );
    // An army of eight tanks.
    let op = w.open_op(
        0,
        OpKind::Army,
        PlanKind::Pressure,
        FxVec2::from_ints(800, 800),
        FxVec2::from_ints(1600, 1600),
        Fx::ZERO,
    );
    let tanks: Vec<UnitId> = (0..8)
        .map(|i| {
            let r = spawn(&mut w, "regency_t1_tank", 800 + i * 20, 800);
            w.state.units.id(r)
        })
        .collect();
    let army = w.state.ai[0]
        .commander
        .ops
        .iter_mut()
        .find(|o| o.id == op)
        .unwrap();
    army.units = tanks.iter().map(|&t| (t, Fx::from_int(300))).collect();
    let census = w.survey_own(0);
    assert_eq!(
        key(&w, w.escort_product(0, skyforge, &census, 0)).as_deref(),
        Some("regency_t3_scavenger")
    );
    assert!(
        w.escort_product(0, skyforge, &census, 1).is_none(),
        "one an army"
    );
    // It is not a field salvager: a wreck field near home never asks for one.
    let field = [super::super::salvage::Field {
        at: FxVec2::from_ints(600, 600),
        mass: Fx::from_int(3000),
    }];
    assert_ne!(
        key(&w, w.salvage_product(skyforge, &census, 0, &field)).as_deref(),
        Some("regency_t3_scavenger")
    );
    // Built, it goes on guard round the army's lead, and stays on it.
    let scythe = spawn(&mut w, "regency_t3_scavenger", 420, 320);
    let census = w.survey_own(0);
    assert_eq!(census.escorts, vec![scythe]);
    assert_eq!(census.salvagers, 0);
    let mut out = Vec::new();
    w.direct_escorts(0, &census, &mut out);
    assert!(
        matches!(out.as_slice(), [Command::Guard { target, .. }] if *target == tanks[0]),
        "{out:?}"
    );
    let commands: Vec<PlayerCommand> = out
        .into_iter()
        .map(|command| PlayerCommand { player: 0, command })
        .collect();
    w.tick(&commands).unwrap();
    let census = w.survey_own(0);
    let mut again = Vec::new();
    w.direct_escorts(0, &census, &mut again);
    assert!(again.is_empty(), "already with the army: {again:?}");
}
