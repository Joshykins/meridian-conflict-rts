//! Compiles `shaders/*.wgsl` to SPIR-V with naga, so building the game needs
//! no Vulkan SDK or external shader compiler on any platform.
//!
//! WGSL has no include mechanism, so `shaders/common.wgsl` is prepended to
//! every shader, and `shaders/bindings.wgsl` to those containing the line
//! `//!use bindings`. GPU structs and set 0 then match across all passes.

use std::path::Path;

fn main() {
    let shader_dir = Path::new("shaders");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    println!("cargo:rerun-if-changed=shaders");

    let common = std::fs::read_to_string(shader_dir.join("common.wgsl")).unwrap();
    let bindings = std::fs::read_to_string(shader_dir.join("bindings.wgsl")).unwrap();
    let mut failed = false;
    let mut entries: Vec<_> = std::fs::read_dir(shader_dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        if path.extension().is_none_or(|e| e != "wgsl") || matches!(path.file_stem().unwrap().to_str(), Some("common" | "bindings")) {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let body = std::fs::read_to_string(&path).unwrap();
        let prelude = if body.lines().any(|l| l.trim() == "//!use bindings") { format!("{common}\n{bindings}") } else { common.clone() };
        let source = format!("{prelude}\n{body}");
        match compile(&source) {
            Ok(words) => {
                let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
                std::fs::write(Path::new(&out_dir).join(format!("{name}.spv")), bytes).unwrap();
            }
            Err(e) => {
                // Line numbers in the message count from the top of the prelude.
                let offset = prelude.lines().count() + 1;
                for line in format!("{name}.wgsl (its line 1 is line {offset} below): {e}").lines() {
                    println!("cargo:warning={line}");
                }
                failed = true;
            }
        }
    }
    if failed {
        panic!("shader compilation failed");
    }
}

fn compile(source: &str) -> Result<Vec<u32>, String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .map_err(|e| e.emit_to_string(source))?;
    let options = naga::back::spv::Options { lang_version: (1, 3), ..Default::default() };
    naga::back::spv::write_vec(&module, &info, &options, None).map_err(|e| e.to_string())
}
