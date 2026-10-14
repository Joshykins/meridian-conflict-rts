//! Authored objects are immutable loaded inputs, bound to replay and netplay identity.
use mc_core::FxVec2;
use mc_map::{
    bake::{bake, BakeParams},
    placed::{Placed, Point},
    MapFile,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
fn fixture() -> (PathBuf, PathBuf) {
    let folder = std::env::temp_dir().join(format!(
        "mc-placed-map-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("terrain.mcmap");
    bake(&BakeParams::square("Placed", 1, 7), &path).unwrap();
    (folder, path)
}
#[test]
fn identity_includes_placements_and_open_maps_keep_their_snapshot() {
    let (folder, path) = fixture();
    let baseline = MapFile::open(&path).unwrap();
    let terrain_id = baseline.content_id();
    let mut placed = Placed {
        next_id: 2,
        points: vec![Point {
            id: 1,
            name: "landing".into(),
            at: FxVec2::from_ints(400, 400),
        }],
        ..Default::default()
    };
    placed
        .write(
            &folder.join("placed.ron"),
            baseline.info(),
            baseline.props().len(),
        )
        .unwrap();
    let authored = MapFile::open(&path).unwrap();
    assert_ne!(authored.content_id(), terrain_id);
    assert_eq!(authored.placed(), &placed);
    authored.verify().unwrap(); // binary integrity remains independent of authoring identity
    std::fs::write(
        folder.join("placed.ron"),
        ron::ser::to_string(&placed).unwrap(),
    )
    .unwrap();
    assert_eq!(
        MapFile::open(&path).unwrap().content_id(),
        authored.content_id()
    );
    placed.points[0].at.x += mc_core::Fx::ONE;
    placed
        .write(
            &folder.join("placed.ron"),
            baseline.info(),
            baseline.props().len(),
        )
        .unwrap();
    let edited = MapFile::open(&path).unwrap();
    assert_ne!(edited.content_id(), authored.content_id());
    assert_ne!(edited.placed(), authored.placed());
    assert!(baseline.placed().points.is_empty());
    drop((edited, authored, baseline));
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn empty_authoring_keeps_legacy_identity_but_invalid_authoring_refuses_to_load() {
    let (folder, path) = fixture();
    let map = MapFile::open(&path).unwrap();
    let id = map.content_id();
    Placed::default()
        .write(&folder.join("placed.ron"), map.info(), map.props().len())
        .unwrap();
    drop(map);
    assert_eq!(MapFile::open(&path).unwrap().content_id(), id);
    std::fs::write(folder.join("placed.ron"), "(version: 999)").unwrap();
    assert!(MapFile::open(&path).is_err());
    // A flat baked map never adopts another map's adjacent folder sidecar.
    let flat = folder.join("legacy.mcmap");
    std::fs::copy(&path, &flat).unwrap();
    assert_eq!(MapFile::open(flat).unwrap().content_id(), id);
    std::fs::remove_dir_all(folder).unwrap();
}
