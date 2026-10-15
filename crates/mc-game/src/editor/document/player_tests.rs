use super::*;
use mc_data::map_settings::MapPlayer;
fn slot(name: &str) -> MapPlayer {
    MapPlayer {
        name: name.into(),
        faction: "Aster".into(),
        team: 0,
        start: 0,
        computer: false,
    }
}
#[test]
fn roster_and_mission_roundtrip_and_undo_as_one_request() {
    let (folder, mut doc, bp) = tests::fixture();
    let original = doc.config.clone();
    let update = doc
        .apply(
            &[
                Command::Players {
                    slots: vec![
                        slot("Company"),
                        MapPlayer {
                            faction: "Regency".into(),
                            team: 1,
                            computer: true,
                            ..slot("Enemy")
                        },
                    ],
                },
                Command::Mission {
                    key: Some("vela_m1".into()),
                },
            ],
            &bp,
        )
        .unwrap();
    assert!(update.objects && update.environment);
    let edited = doc.config.clone();
    let saved = folder.join("authored");
    doc.save(&saved).unwrap();
    let opened = Document::open(&saved.join("terrain.mcmap")).unwrap();
    assert_eq!(opened.config, edited);
    let map = MapFile::open(saved.join("terrain.mcmap")).unwrap();
    assert!(crate::campaign::definition(&map).unwrap().is_some());
    doc.undo().unwrap();
    assert_eq!(doc.config, original);
    doc.redo().unwrap();
    assert_eq!(doc.config, edited);
    doc.apply(&[Command::Mission { key: None }], &bp).unwrap();
    doc.save(&saved).unwrap();
    assert!(
        crate::campaign::definition(&MapFile::open(saved.join("terrain.mcmap")).unwrap())
            .unwrap()
            .is_none()
    );
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn invalid_rosters_and_removed_owners_reject_the_entire_batch() {
    let (folder, mut doc, bp) = tests::fixture();
    doc.apply(
        &[Command::Unit {
            name: "tank".into(),
            group: "".into(),
            blueprint: "aster_t1_tank".into(),
            owner: 1,
            at: [500.0, 500.0],
            heading: 0.0,
        }],
        &bp,
    )
    .unwrap();
    let original = doc.config.clone();
    for slots in [
        vec![slot("Only")],
        vec![
            slot("A"),
            MapPlayer {
                faction: "missing".into(),
                ..slot("B")
            },
        ],
        vec![
            slot("A"),
            MapPlayer {
                start: 31,
                ..slot("B")
            },
        ],
        vec![slot("A"), slot("")],
        vec![slot("A"); mc_core::MAX_PLAYERS + 1],
    ] {
        assert!(doc
            .apply(
                &[Command::Time { hour: 17.0 }, Command::Players { slots }],
                &bp
            )
            .is_err());
        assert_eq!(doc.config, original);
    }
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn playtest_uses_chosen_slot_and_authored_roster() {
    let (folder, mut doc, bp) = tests::fixture();
    doc.apply(
        &[Command::Players {
            slots: vec![
                slot("First"),
                MapPlayer {
                    faction: "Regency".into(),
                    team: 2,
                    computer: true,
                    ..slot("Second")
                },
            ],
        }],
        &bp,
    )
    .unwrap();
    let map = super::super::snapshot::map(&doc, "slot-test").unwrap();
    let request = crate::ui::setup::MatchRequest {
        map,
        config: super::super::config(&doc.placed, &doc.config),
        colors: crate::setup::TEAM_COLORS,
        survival: None,
        sky: Default::default(),
        fallen_army: Default::default(),
    };
    let start = crate::app::local_editor_start(request, &bp, None, 1).unwrap();
    assert_eq!(start.local, 1);
    assert!(!start.observing);
    assert_eq!(start.roster[1].name, "Second");
    assert_eq!(start.roster[1].faction, "Regency");
    assert_eq!(start.roster[1].team, 2);
    assert_eq!(
        start.roster[1].controller,
        mc_sim::tables::Controller::Human
    );
    drop(start);
    std::fs::remove_dir_all(folder).unwrap();
}
