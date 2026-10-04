//! Two things at build time:
//!
//! - Builds the program icon and name (`meridian.rc`) into meridian.exe when
//!   building for Windows, so Explorer, the taskbar and a pinned shortcut show
//!   them. The icon is drawn by `ui/emblem/monogram.rs`. Also its manifest
//!   (`meridian.manifest`), which the crash window needs.
//! - Stamps the build with its commit: `MERIDIAN_BUILD` is `<version>+<short hash>`.
//!   Network players must run the same simulation code, which the map and unit
//!   data hashes do not cover; the relay refuses players whose build differs.
//!   Outside a git checkout (a source archive) the build is the version alone.
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
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let build = match git(&["rev-parse", "--short=10", "HEAD"]) {
        Some(hash) => format!("{version}+{hash}"),
        None => version,
    };
    println!("cargo:rustc-env=MERIDIAN_BUILD={build}");
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

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    stamp_build();
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
