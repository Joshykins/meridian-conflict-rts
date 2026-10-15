//! Builds the program icon and name (`launcher.rc`) into MeridianConflict.exe
//! when building for Windows with a resource compiler at hand, and numbers the
//! launcher as the game is numbered: `MERIDIAN_BUILD_NUMBER`, the count of
//! commits up to HEAD (empty outside a git checkout). A launcher swaps itself
//! only for one from a later build (`install.rs`, `stage_launcher`).

use embed_resource::CompilationResult;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn stamp_number() {
    let number = git(&["rev-list", "--count", "HEAD"]).unwrap_or_default();
    println!("cargo:rustc-env=MERIDIAN_BUILD_NUMBER={number}");
    // A new commit (or checkout) changes the number; nothing else needs a rebuild for it.
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
    stamp_number();
    println!("cargo:rerun-if-changed=launcher.rc");
    println!("cargo:rerun-if-changed=../mc-game/assets/meridian.ico");
    let version = |part: &str| {
        let value = std::env::var(format!("CARGO_PKG_VERSION_{part}")).unwrap_or_default();
        format!("VERSION_{part}={value}")
    };
    let macros = [version("MAJOR"), version("MINOR"), version("PATCH")];
    match embed_resource::compile("launcher.rc", &macros) {
        CompilationResult::NotWindows | CompilationResult::Ok => {}
        // No resource compiler (rc.exe comes with the Windows SDK): the
        // launcher still builds and runs, with the system's icon.
        CompilationResult::NotAttempted(why) => {
            println!("cargo:warning=MeridianConflict.exe built without its icon: {why}")
        }
        CompilationResult::Failed(why) => {
            eprintln!("launcher.rc: {why}");
            std::process::exit(1);
        }
    }
}
