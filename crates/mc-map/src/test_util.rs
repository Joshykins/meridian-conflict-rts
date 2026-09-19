//! Fixtures shared by the module tests.

use crate::bake::{bake, BakeParams};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

fn dir() -> PathBuf {
    let dir = std::env::temp_dir().join("mc-map-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A file name no other test (or test process) uses.
pub fn temp_path(tag: &str) -> PathBuf {
    dir().join(format!("{}-{tag}.mcmap", std::process::id()))
}

/// A 4 km (2 x 2 tile), two-player map, baked once per test process.
pub fn baked_4km() -> &'static Path {
    static MAP: OnceLock<PathBuf> = OnceLock::new();
    MAP.get_or_init(|| {
        // The fixture outlives the tests that share it, so nobody deletes it
        // on the way out; sweep what earlier runs left behind instead.
        for entry in std::fs::read_dir(dir()).unwrap().flatten() {
            let age = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok());
            if age.is_some_and(|a| a > Duration::from_secs(3600)) {
                std::fs::remove_file(entry.path()).ok();
            }
        }
        let path = temp_path("baked-4km");
        bake(&BakeParams::square("Test Basin", 2, 7), &path).unwrap();
        path
    })
}
