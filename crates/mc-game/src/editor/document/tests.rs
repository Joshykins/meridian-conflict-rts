use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
fn fixture() -> (PathBuf, Document, mc_data::Blueprints) {
    let folder = std::env::temp_dir().join(format!(
        "mc-editor-test-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("base.mcmap");
    mc_map::bake::bake(
        &mc_map::bake::BakeParams::square("Editor test", 2, 7),
        &path,
    )
    .unwrap();
    let bp = mc_data::Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
        .unwrap();
    let doc = Document::open(&path).unwrap();
    (folder, doc, bp)
}
fn unit(name: &str) -> Command {
    Command::Unit {
        name: name.into(),
        group: "company".into(),
        blueprint: "aster_t1_tank".into(),
        owner: 0,
        at: [500.0, 500.0],
        heading: 0.0,
    }
}
#[test]
fn invalid_batch_is_atomic_and_undo_redo_keep_stable_ids() {
    let (folder, mut doc, bp) = fixture();
    let heights = doc.map.samples().to_vec();
    let invalid = Command::Unit {
        name: "bad".into(),
        group: "company".into(),
        blueprint: "unknown".into(),
        owner: 0,
        at: [500.0, 500.0],
        heading: 0.0,
    };
    assert!(doc
        .apply(
            &[
                Command::Terrain {
                    at: [500.0, 500.0],
                    radius: 64.0,
                    strength: 10.0,
                    brush: Brush::Raise
                },
                unit("tank"),
                invalid
            ],
            &bp
        )
        .is_err());
    assert_eq!(doc.map.samples(), heights);
    assert!(doc.placed.units.is_empty());
    assert!(!doc.dirty);
    doc.apply(&[unit("tank")], &bp).unwrap();
    let id = doc.placed.units[0].id;
    doc.apply(
        &[Command::Transform {
            id,
            at: [700.0, 700.0],
            heading: 90.0,
        }],
        &bp,
    )
    .unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.placed.units[0].at, FxVec2::from_ints(500, 500));
    doc.redo().unwrap();
    assert_eq!(doc.placed.units[0].id, id);
    assert_eq!(doc.placed.units[0].at, FxVec2::from_ints(700, 700));
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn save_as_export_and_failed_save_preserve_assets_and_document() {
    let (folder, mut doc, bp) = fixture();
    doc.apply(&[unit("tank")], &bp).unwrap();
    let saved = folder.join("saved");
    doc.save(&saved).unwrap();
    std::fs::write(saved.join("strings.en.ron"), "{text}").unwrap();
    let old = MapFile::open(saved.join("terrain.mcmap"))
        .unwrap()
        .content_id();
    doc.apply(&[unit("second")], &bp).unwrap();
    let before = doc.placed.clone();
    doc.export(&folder.join("playtest")).unwrap();
    assert!(doc.dirty);
    assert_eq!(doc.placed, before);
    assert_eq!(
        std::fs::read_to_string(folder.join("playtest/strings.en.ron")).unwrap(),
        "{text}"
    );
    doc.map.starts.push(FxVec2::from_ints(-1, -1));
    assert!(doc.save(&saved).is_err());
    assert_eq!(
        MapFile::open(saved.join("terrain.mcmap"))
            .unwrap()
            .content_id(),
        old
    );
    assert_eq!(
        std::fs::read_to_string(saved.join("strings.en.ron")).unwrap(),
        "{text}"
    );
    doc.map.starts.pop();
    assert!(doc.save(&folder.join("playtest")).is_err());
    doc.save(&saved).unwrap();
    let opened = Document::open(&saved.join("terrain.mcmap")).unwrap();
    assert_eq!(opened.placed, before);
    assert!(!doc.dirty);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn editing_properties_preserves_identity_and_rejects_duplicate_names_atomically() {
    let (folder, mut doc, bp) = fixture();
    doc.apply(&[unit("first"), unit("second")], &bp).unwrap();
    let id = doc.placed.units[0].id;
    let edit = |name: &str| Command::Properties {
        id,
        name: name.into(),
        group: Some("guards".into()),
        owner: Some(2),
        blueprint: None,
    };
    let before = doc.placed.clone();
    assert!(doc.apply(&[edit("second")], &bp).is_err());
    assert_eq!(doc.placed, before);
    doc.apply(&[edit("renamed")], &bp).unwrap();
    assert_eq!(doc.placed.units[0].id, id);
    assert_eq!(doc.placed.units[0].name, "renamed");
    assert_eq!(doc.placed.units[0].owner, 2);
    doc.undo().unwrap();
    assert_eq!(doc.placed, before);
    doc.redo().unwrap();
    assert_eq!(doc.placed.units.len(), 2);
    assert_eq!(doc.placed.units[0].id, id);
    std::fs::remove_dir_all(folder).unwrap();
}
