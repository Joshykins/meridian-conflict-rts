use super::*;
use crate::{test_util, TILE_CELLS};

#[test]
fn unchanged_document_preserves_content_id_and_every_layer() {
    let source = MapFile::open(test_util::baked_4km()).unwrap();
    let edit = EditMap::load(&source).unwrap();
    let path = test_util::temp_path("editor-roundtrip");
    assert_eq!(edit.save(&path).unwrap(), source.content_id());
    let saved = MapFile::open(&path).unwrap();
    saved.verify().unwrap();
    assert_eq!(saved.props(), source.props());
    assert_eq!(saved.snow(), source.snow());
    assert_eq!(saved.ways(), source.ways());
    assert_eq!(saved.streets(), source.streets());
    assert_eq!(saved.decks(), source.decks());
    assert_eq!(saved.wrecks(), source.wrecks());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn corner_edit_updates_all_four_tiles_and_can_be_undone() {
    let source = MapFile::open(test_util::baked_4km()).unwrap();
    let mut edit = EditMap::load(&source).unwrap();
    let old = edit.set_sample(TILE_CELLS, TILE_CELLS, 4242).unwrap();
    for (tx, ty, index) in [
        (0, 0, TILE_SAMPLE_COUNT - 1),
        (1, 0, 256 * 257),
        (0, 1, 256),
        (1, 1, 0),
    ] {
        assert_eq!(edit.tile(tx, ty).unwrap()[index], 4242);
    }
    let path = test_util::temp_path("editor-seam");
    edit.save(&path).unwrap();
    EditMap::load(&MapFile::open(&path).unwrap()).unwrap();
    edit.set_sample(TILE_CELLS, TILE_CELLS, old).unwrap();
    assert_eq!(edit.save(&path).unwrap(), source.content_id());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn failed_save_leaves_previous_file_readable() {
    let source = MapFile::open(test_util::baked_4km()).unwrap();
    let mut edit = EditMap::load(&source).unwrap();
    let path = test_util::temp_path("editor-failed-save");
    edit.save(&path).unwrap();
    edit.starts.push(FxVec2::from_ints(-1, -1));
    assert!(edit.save(&path).is_err());
    assert_eq!(
        MapFile::open(&path).unwrap().content_id(),
        source.content_id()
    );
    std::fs::remove_file(path).unwrap();
}
