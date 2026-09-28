//! Small maps baked for the set-up screens' tests, so no test needs the baked
//! maps in `maps/` (which are not checked in). Baked once per test process
//! into a directory of its own, in well under a second.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use mc_map::BakeParams;

/// Two skirmish maps (a 1v1 and an eight-start one) and a survival theatre:
/// a small Threshold with the real map's settings file beside it.
pub(crate) fn paths() -> Vec<PathBuf> {
    static PATHS: OnceLock<Vec<PathBuf>> = OnceLock::new();
    PATHS.get_or_init(bake).clone()
}

/// An 8 km square for the HUD's tests, which stage units near (4000, 4000).
pub(crate) fn field() -> PathBuf {
    static FIELD: OnceLock<PathBuf> = OnceLock::new();
    FIELD
        .get_or_init(|| {
            let path = dir().join("test_field.mcmap");
            mc_map::bake(&BakeParams::square("Test Field", 4, 5), &path).unwrap();
            path
        })
        .clone()
}

fn dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mc_game_test_maps_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn bake() -> Vec<PathBuf> {
    // The set-up screens list every map in a directory, so these get one of
    // their own, apart from the HUD's field.
    let dir = dir().join("setup");
    std::fs::create_dir_all(&dir).unwrap();
    let duel = BakeParams::square("Test Duel", 1, 3);
    let teams = BakeParams {
        players: 8,
        ..BakeParams::square("Test Teams", 2, 3)
    };
    let threshold = BakeParams::threshold("The Threshold", 2, 31);
    let paths: Vec<PathBuf> = [
        ("test_duel", duel),
        ("test_teams", teams),
        ("threshold", threshold),
    ]
    .into_iter()
    .map(|(stem, params)| {
        let path = dir.join(format!("{stem}.mcmap"));
        mc_map::bake(&params, &path).unwrap();
        path
    })
    .collect();
    // The survival block lives in the map's settings file.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::copy(root.join("maps/threshold.ron"), dir.join("threshold.ron")).unwrap();
    paths
}
