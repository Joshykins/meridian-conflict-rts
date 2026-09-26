//! Builds the program icon and name (`meridian.rc`) into meridian.exe when
//! building for Windows, so Explorer, the taskbar and a pinned shortcut show
//! them. The icon is drawn by `ui/emblem/monogram.rs`.

use embed_resource::CompilationResult;

fn main() {
    println!("cargo:rerun-if-changed=meridian.rc");
    println!("cargo:rerun-if-changed=assets/meridian.ico");
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
