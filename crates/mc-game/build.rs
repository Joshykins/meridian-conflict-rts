//! At build time:
//!
//! - Builds the program icon and name (`meridian.rc`) into meridian.exe when
//!   building for Windows, so Explorer, the taskbar and a pinned shortcut show
//!   them. The icon is drawn by `ui/emblem/monogram.rs`. Also its manifest
//!   (`meridian.manifest`), which the crash window needs.
//! - Stamps the build with its channel and commit: `MERIDIAN_BUILD` is
//!   `<version>+<short hash>` for a release, `<version>-<channel>+<short hash>`
//!   otherwise. The channel comes from `MERIDIAN_CHANNEL` (dev, playtest or
//!   release; dev when unset) and is `MERIDIAN_CHANNEL` again for the game.
//!   Network players must run the same simulation code, which the map and unit
//!   data hashes do not cover; the relay refuses players whose build differs.
//!   Outside a git checkout (a source archive) the build has no commit.
//!   `MERIDIAN_COMMIT` is the full commit hash, empty outside a checkout.
//! - Fingerprints the simulation: `MERIDIAN_SIM` is a hash of the sources of
//!   every crate the simulation is built from (`sim_crates.txt`), with
//!   `Cargo.lock` and the toolchain pin. Two builds with the same fingerprint
//!   play each other's replays (given the same unit data and map), so an update
//!   that only touches drawing, sound or the interface keeps old replays
//!   playing (docs/RELEASES.md).
//! - Numbers the build: `MERIDIAN_BUILD_NUMBER` is the count of commits up to
//!   HEAD, shown as "Build N" on the menu and the opening screen. Empty outside
//!   a git checkout.

use embed_resource::CompilationResult;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn stamp_build() {
    println!("cargo:rerun-if-env-changed=MERIDIAN_CHANNEL");
    let channel = std::env::var("MERIDIAN_CHANNEL").unwrap_or_else(|_| "dev".to_owned());
    if !["dev", "playtest", "release"].contains(&channel.as_str()) {
        eprintln!("MERIDIAN_CHANNEL={channel}: must be dev, playtest or release");
        std::process::exit(1);
    }
    println!("cargo:rustc-env=MERIDIAN_CHANNEL={channel}");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let version = match channel.as_str() {
        "release" => version,
        _ => format!("{version}-{channel}"),
    };
    let build = match git(&["rev-parse", "--short=10", "HEAD"]) {
        Some(hash) => format!("{version}+{hash}"),
        None => version,
    };
    println!("cargo:rustc-env=MERIDIAN_BUILD={build}");
    let commit = git(&["rev-parse", "HEAD"]).unwrap_or_default();
    println!("cargo:rustc-env=MERIDIAN_COMMIT={commit}");
    let number = git(&["rev-list", "--count", "HEAD"]).unwrap_or_default();
    println!("cargo:rustc-env=MERIDIAN_BUILD_NUMBER={number}");
    // A new commit (or checkout) changes the stamp; nothing else needs a rebuild for it.
    for dir in [
        git(&["rev-parse", "--absolute-git-dir"]),
        git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
    ]
    .into_iter()
    .flatten()
    {
        println!("cargo:rerun-if-changed={dir}/HEAD");
        println!("cargo:rerun-if-changed={dir}/refs/heads");
        println!("cargo:rerun-if-changed={dir}/packed-refs");
    }
}

/// FNV-1a over every file the simulation is built from, in path order. Not a
/// defence against anyone: it only has to change when the sources do.
fn stamp_sim() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../..");
    println!("cargo:rerun-if-changed=sim_crates.txt");
    let list = std::fs::read_to_string(manifest.join("sim_crates.txt")).unwrap();
    let mut files = Vec::new();
    for name in ["Cargo.lock", "rust-toolchain.toml"] {
        files.push(root.join(name));
    }
    for krate in list
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let dir = root.join("crates").join(krate);
        println!("cargo:rerun-if-changed={}", dir.join("src").display());
        files.push(dir.join("Cargo.toml"));
        walk(&dir.join("src"), &mut files);
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h = (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for f in &files {
        println!("cargo:rerun-if-changed={}", f.display());
        let rel = f.strip_prefix(&root).unwrap_or(f);
        let bytes = std::fs::read(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        eat(rel.to_string_lossy().replace('\\', "/").as_bytes());
        // Line endings follow the checkout (Windows or WSL), not the source.
        let text: Vec<u8> = bytes.into_iter().filter(|&b| b != b'\r').collect();
        eat(&(text.len() as u64).to_le_bytes());
        eat(&text);
    }
    println!("cargo:rustc-env=MERIDIAN_SIM={h:016x}");
}

fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    stamp_build();
    stamp_sim();
    println!("cargo:rerun-if-changed=meridian.rc");
    println!("cargo:rerun-if-changed=assets/meridian.ico");
    println!("cargo:rerun-if-changed=meridian.manifest");
    let version = |part: &str| {
        let value = std::env::var(format!("CARGO_PKG_VERSION_{part}")).unwrap_or_default();
        format!("VERSION_{part}={value}")
    };
    let macros = [version("MAJOR"), version("MINOR"), version("PATCH")];
    match embed_resource::compile("meridian.rc", &macros) {
        CompilationResult::NotWindows | CompilationResult::Ok => {}
        // No resource compiler (rc.exe comes with the Windows SDK): the game
        // still builds and runs, with the system's icon.
        CompilationResult::NotAttempted(why) => {
            println!("cargo:warning=meridian.exe built without its icon: {why}")
        }
        CompilationResult::Failed(why) => {
            eprintln!("meridian.rc: {why}");
            std::process::exit(1);
        }
    }
}
