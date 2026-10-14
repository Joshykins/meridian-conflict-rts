use super::*;
#[test]
fn resize_keeps_named_identity_and_complete_undo_history() {
    let (folder, mut doc, bp) = tests::fixture();
    doc.apply(
        &[
            Command::Point {
                name: "marker".into(),
                at: [500.0, 600.0],
            },
            Command::Camera {
                name: "view".into(),
                at: [500.0, 600.0],
                distance: 500,
                yaw: 0.0,
                height: Some(120.0),
                pitch: Some(45.0),
                free: true,
            },
            Command::Doodad {
                name: "rock".into(),
                kind: "RockSmall".into(),
                at: [500.0, 600.0],
                heading: 45.0,
                scale: 1.0,
                tint: [12, 34, 56],
                blocking: true,
                health: 100,
                reclaim: 20,
                footprint: Some([2.0, 2.0, 3.0]),
            },
        ],
        &bp,
    )
    .unwrap();
    let placed = doc.placed.clone();
    let samples = doc.map.samples().to_vec();
    let tiles = [doc.map.info.tiles_w, doc.map.info.tiles_h];
    let update = doc
        .apply(
            &[Command::Bounds {
                origin: [-1, -1],
                tiles: [tiles[0] + 2, tiles[1] + 2],
                fill: -20.0,
            }],
            &bp,
        )
        .unwrap();
    assert_eq!(update.reframe, Some([-2048.0, -2048.0]));
    assert_eq!(doc.placed.points[0].id, placed.points[0].id);
    assert_eq!(doc.placed.points[0].at, FxVec2::from_ints(2548, 2648));
    assert_eq!(doc.placed.cameras[0].pose, placed.cameras[0].pose);
    assert!(doc.history_reframes(false));
    doc.undo().unwrap();
    assert_eq!(doc.placed, placed);
    assert_eq!(doc.map.samples(), samples);
    assert!(doc.history_reframes(true));
    doc.redo().unwrap();
    let saved = folder.join("resized");
    doc.save(&saved).unwrap();
    let reopened = Document::open(&saved.join("terrain.mcmap")).unwrap();
    assert_eq!(reopened.map.info.tiles_w, tiles[0] + 2);
    assert_eq!(reopened.placed.points, doc.placed.points);
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert!(doc.placed.points.is_empty());
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn invalid_resize_does_not_mutate_the_document() {
    let (folder, mut doc, bp) = tests::fixture();
    let samples = doc.map.samples().to_vec();
    for command in [
        Command::Bounds {
            origin: [0, 0],
            tiles: [0, 1],
            fill: 0.0,
        },
        Command::Bounds {
            origin: [0, 0],
            tiles: [1, 1],
            fill: f32::NAN,
        },
        Command::Bounds {
            origin: [i32::MAX, 0],
            tiles: [1, 1],
            fill: 0.0,
        },
    ] {
        assert!(doc.apply(&[command], &bp).is_err());
        assert_eq!(doc.map.samples(), samples);
    }
    assert!(!doc.can_undo());
    assert!(doc
        .apply(
            &[
                Command::Time { hour: 17.0 },
                Command::Bounds {
                    origin: [0, 0],
                    tiles: [2, 2],
                    fill: 0.0
                }
            ],
            &bp
        )
        .is_err());
    assert!(!doc.can_undo());
    std::fs::remove_dir_all(folder).unwrap();
}
