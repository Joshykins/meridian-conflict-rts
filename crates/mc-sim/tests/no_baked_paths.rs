//! No crate compiles its checkout's path in (CLAUDE.md section 1). A unit built
//! with `CARGO_MANIFEST_DIR` taken by `env!` differs in every worktree, so a new worktree
//! cannot reuse another's build of it (scripts/target-seed.sh) and compiles it
//! from nothing. Tests and examples read the same variable at run time, which
//! cargo and scripts/check.sh set, with `std::env::var_os`.

use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "target") {
                rust_files(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_crate_compiles_its_checkout_path_in() {
    let crates = Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("..");
    let mut files = Vec::new();
    rust_files(&crates, &mut files);
    assert!(files.len() > 100, "found only {} files", files.len());
    // Built in two halves so this file does not match itself.
    let baked = ["env!(\"CARGO_", "MANIFEST_DIR\")"].concat();
    let mut found: Vec<String> = files
        .iter()
        .filter(|f| std::fs::read_to_string(f).is_ok_and(|text| text.contains(&baked)))
        .map(|f| f.strip_prefix(&crates).unwrap_or(f).display().to_string())
        .collect();
    found.sort();
    assert!(
        found.is_empty(),
        "these compile their checkout's path in; read it at run time, \
         std::env::var_os(\"CARGO_MANIFEST_DIR\"):\n  {}",
        found.join("\n  ")
    );
}
