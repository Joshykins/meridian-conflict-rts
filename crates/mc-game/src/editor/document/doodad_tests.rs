use super::*;
fn prop(name: &str, at: [f32; 2]) -> Command {
    Command::Doodad {
        name: name.into(),
        kind: "CityBillboard".into(),
        at,
        heading: 30.0,
        scale: 1.5,
        tint: [240, 70, 30],
        blocking: true,
        health: 500,
        reclaim: 20,
        footprint: Some([8.0, 2.0, 15.0]),
    }
}
#[test]
fn component_edits_remap_delete_undo_and_sorted_save_without_losing_identity() {
    let (folder, mut doc, bp) = tests::fixture();
    let before = doc.map.props.clone();
    doc.apply(
        &[prop("east", [1200.0, 1200.0]), prop("west", [400.0, 400.0])],
        &bp,
    )
    .unwrap();
    let east = doc.placed.doodads[0].id;
    let west = doc.placed.doodads[1].id;
    let update = doc.apply(&[Command::Delete { id: east }], &bp).unwrap();
    assert!(update.props);
    assert_eq!(doc.map.props.len(), before.len() + 1);
    assert_eq!(doc.placed.doodads[0].id, west);
    assert_eq!(doc.placed.doodads[0].prop_index as usize, before.len());
    doc.undo().unwrap();
    assert_eq!(doc.placed.doodads[0].id, east);
    assert_eq!(doc.placed.doodads[1].id, west);
    doc.apply(
        &[Command::Transform {
            id: west,
            at: [600.0, 600.0],
            heading: 90.0,
        }],
        &bp,
    )
    .unwrap();
    let folder_out = folder.join("authored");
    doc.save(&folder_out).unwrap();
    let opened = Document::open(&folder_out.join("terrain.mcmap")).unwrap();
    let d = opened.placed.doodads.iter().find(|d| d.id == west).unwrap();
    let p = opened.map.props[d.prop_index as usize];
    assert_eq!(p.pos, FxVec2::from_ints(600, 600));
    assert_eq!(p.heading, Angle::QUARTER_TURN);
    assert_eq!(p.scale_milli, 1500);
    assert_eq!((d.health, d.reclaim, d.tint), (500, 20, [240, 70, 30]));
    assert!(d.blocking);
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.map.props, before);
    assert!(doc.placed.doodads.is_empty());
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn invalid_component_batch_changes_neither_props_nor_other_objects() {
    let (folder, mut doc, bp) = tests::fixture();
    let before = doc.map.props.clone();
    let mut invalid = prop("bad", [500.0, 500.0]);
    if let Command::Doodad { footprint, .. } = &mut invalid {
        *footprint = None;
    }
    assert!(doc
        .apply(&[prop("okay", [400.0, 400.0]), invalid], &bp)
        .is_err());
    assert_eq!(doc.map.props, before);
    assert!(doc.placed.doodads.is_empty());
    assert!(!doc.can_undo());
    doc.apply(&[prop("sign", [400.0, 400.0])], &bp).unwrap();
    let id = doc.placed.doodads[0].id;
    let mut renamed = prop("renamed", [500.0, 600.0]);
    if let Command::Doodad { tint, health, .. } = &mut renamed {
        *tint = [0, 0, 0];
        *health = 0;
    }
    doc.apply(
        &[
            Command::Properties {
                id,
                name: "renamed".into(),
                group: None,
                owner: None,
                blueprint: None,
            },
            renamed,
        ],
        &bp,
    )
    .unwrap();
    assert_eq!(doc.placed.doodads.len(), 1);
    assert_eq!(doc.placed.doodads[0].id, id);
    assert_eq!(doc.placed.doodads[0].tint, [0, 0, 0]);
    assert_eq!(doc.placed.doodads[0].health, 0);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn scale_and_move_preserve_components_and_undo_atomically() {
    let (folder, mut doc, bp) = super::tests::fixture();
    doc.apply(
        &[Command::Doodad {
            name: "sign".into(),
            kind: "CityBillboard".into(),
            at: [400.0, 400.0],
            heading: 0.0,
            scale: 1.0,
            tint: [100, 200, 255],
            blocking: true,
            health: 120,
            reclaim: 80,
            footprint: Some([8.0, 2.0, 15.0]),
        }],
        &bp,
    )
    .unwrap();
    let before = doc.placed.clone();
    let prop = doc.map.props[before.doodads[0].prop_index as usize];
    let id = before.doodads[0].id;
    doc.apply(
        &[
            Command::Transform {
                id,
                at: [416.0, 432.0],
                heading: 90.0,
            },
            Command::Scale { id, factor: 2.0 },
        ],
        &bp,
    )
    .unwrap();
    assert_eq!(doc.placed, before);
    let changed = doc.map.props[before.doodads[0].prop_index as usize];
    assert_eq!(changed.scale_milli, 2000);
    assert_eq!(changed.pos.to_f32(), [416.0, 432.0]);
    assert!(doc
        .apply(
            &[
                Command::Transform {
                    id,
                    at: [500.0, 500.0],
                    heading: 0.0
                },
                Command::Scale { id, factor: 20.0 }
            ],
            &bp
        )
        .is_err());
    assert_eq!(
        doc.map.props[before.doodads[0].prop_index as usize],
        changed
    );
    doc.undo().unwrap();
    assert_eq!(doc.map.props[before.doodads[0].prop_index as usize], prop);
    doc.redo().unwrap();
    assert_eq!(
        doc.map.props[before.doodads[0].prop_index as usize],
        changed
    );
    std::fs::remove_dir_all(folder).unwrap();
}
