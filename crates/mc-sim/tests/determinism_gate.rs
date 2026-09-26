//! Keeps the determinism gate in place. The gate itself is clippy (each sim crate's
//! clippy.toml and the `#![warn(...)]` line in its lib.rs; CLAUDE.md section 3); this
//! fails if either is taken away, or if a whole module is exempted from it outside the
//! modules whose job is presentation.

use std::path::{Path, PathBuf};

const GATE: &str =
    "#![warn(clippy::disallowed_types, clippy::disallowed_methods, clippy::float_arithmetic)]";

/// What each clippy.toml must ban, at least.
const BANNED: &[&str] = &[
    "\"f32\"",
    "\"f64\"",
    "std::collections::HashMap",
    "std::collections::HashSet",
    "std::time::Instant",
    "std::time::SystemTime",
    "mc_core::Fx::to_f32",
    "mc_core::Fx::from_f32",
    "std::time::Instant::now",
    "std::thread::current",
    "std::thread::available_parallelism",
    "slice::sort_unstable_by_key",
    "slice::sort_unstable_by",
];

/// The only modules exempt as a whole: they exist to turn sim state into what the
/// renderer draws.
const PRESENTATION_MODULES: &[&str] = &["mirror.rs", "print_heads.rs"];

fn crates() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn every_sim_crate_keeps_its_gate() {
    for krate in ["mc-sim", "mc-path"] {
        let lib = read(&crates().join(krate).join("src/lib.rs"));
        assert!(
            lib.lines().any(|l| l.trim() == GATE),
            "{krate}/src/lib.rs lost its determinism gate line:\n{GATE}"
        );
        let config = read(&crates().join(krate).join("clippy.toml"));
        for banned in BANNED {
            assert!(
                config.contains(banned),
                "{krate}/clippy.toml no longer bans {banned}"
            );
        }
    }
}

#[test]
fn only_presentation_modules_are_exempt_as_a_whole() {
    for krate in ["mc-sim", "mc-path"] {
        let mut stack = vec![crates().join(krate).join("src")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let name = path.file_name().unwrap().to_str().unwrap();
                if !name.ends_with(".rs") || PRESENTATION_MODULES.contains(&name) {
                    continue;
                }
                for (n, line) in read(&path).lines().enumerate() {
                    let line = line.trim();
                    let exempts_module = (line.starts_with("#![expect(")
                        || line.starts_with("#![allow("))
                        && ["float_arithmetic", "disallowed_types", "disallowed_methods"]
                            .iter()
                            .any(|lint| line.contains(lint));
                    assert!(
                        !exempts_module,
                        "{}:{}: a whole module is exempted from the determinism gate; \
                         exempt the one presentation item instead",
                        path.display(),
                        n + 1
                    );
                }
            }
        }
    }
}
