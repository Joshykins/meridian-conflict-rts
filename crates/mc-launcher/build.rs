//! Builds the program icon and name (`launcher.rc`) into MeridianConflict.exe
//! when building for Windows with a resource compiler at hand.

use embed_resource::CompilationResult;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
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
