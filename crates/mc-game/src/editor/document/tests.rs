use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
pub(in crate::editor) fn fixture() -> (PathBuf, Document, mc_data::Blueprints) {
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
    let bp = mc_data::Blueprints::load(
        &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
    )
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

#[test]
fn ramps_and_heightmap_stamps_are_sparse_atomic_and_undoable() {
    let (folder, mut doc, bp) = fixture();
    let (w, h) = doc.map.info.size_cells();
    for y in 0..=h {
        for x in 0..=w {
            doc.map.set_sample(x, y, 1000).unwrap();
        }
    }
    doc.map.set_sample(32, 16, 2000).unwrap();
    let before = doc.map.samples().to_vec();
    let ramp = Command::Ramp {
        from: [128.0, 128.0],
        to: [256.0, 128.0],
        width: 16.0,
    };
    doc.apply(std::slice::from_ref(&ramp), &bp).unwrap();
    assert_eq!(doc.map.samples()[(16 * (w + 1) + 24) as usize], 1500);
    assert_eq!(doc.map.samples()[(20 * (w + 1) + 24) as usize], 1000);
    doc.undo().unwrap();
    assert_eq!(doc.map.samples(), before);
    std::fs::create_dir(folder.join("stamps")).unwrap();
    std::fs::write(
        folder.join("stamps/bad.json"),
        r#"{"width":2,"height":2,"values":[2,0,0,0]}"#,
    )
    .unwrap();
    let stamp = |asset: &str| Command::Stamp {
        at: [512.0, 512.0],
        radius: 32.0,
        strength: 8.0,
        asset: asset.into(),
    };
    assert!(doc.apply(&[ramp, stamp("bad.json")], &bp).is_err());
    assert_eq!(doc.map.samples(), before);
    std::fs::write(
        folder.join("stamps/good.json"),
        r#"{"width":3,"height":3,"values":[0,0,0,0,1,0,0,0,0]}"#,
    )
    .unwrap();
    doc.apply(&[stamp("good.json")], &bp).unwrap();
    assert!(doc.map.samples()[(64 * (w + 1) + 64) as usize] > 1000);
    assert_eq!(doc.map.samples()[(64 * (w + 1) + 68) as usize], 1000);
    doc.undo().unwrap();
    assert_eq!(doc.map.samples(), before);
    assert!(doc.apply(&[stamp("../good.json")], &bp).is_err());
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn terrain_updates_rebuild_grounded_preview_and_high_player_slots_work() {
    let (folder, mut doc, bp) = fixture();
    let mut tank = unit("high_slot");
    if let Command::Unit { owner, .. } = &mut tank {
        *owner = (mc_core::MAX_PLAYERS - 1) as u8;
    }
    assert!(doc.apply(&[tank], &bp).unwrap().objects);
    let bp = std::sync::Arc::new(bp);
    let pool = std::sync::Arc::new(mc_jobs::Pool::new(0));
    let before = super::super::preview::build(&doc, 0, bp.clone(), pool.clone()).unwrap();
    let update = doc
        .apply(
            &[Command::Terrain {
                at: [500.0, 500.0],
                radius: 128.0,
                strength: 100.0,
                brush: Brush::Cliff,
            }],
            &bp,
        )
        .unwrap();
    assert!(update.terrain.is_some());
    let after = super::super::preview::build(&doc, 0, bp.clone(), pool.clone()).unwrap();
    assert!(after.units[0].pos[2] > before.units[0].pos[2] + 90.0);
    doc.undo().unwrap();
    let restored = super::super::preview::build(&doc, 0, bp, pool).unwrap();
    assert_eq!(before.units[0].pos, restored.units[0].pos);
    std::fs::remove_dir_all(folder).unwrap();
}
