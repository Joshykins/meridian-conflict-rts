//! Compiles `shaders/*.wgsl` to SPIR-V with naga, so building the game needs
//! no Vulkan SDK or external shader compiler on any platform.
//!
//! WGSL has no include mechanism, so `shaders/common.wgsl` is prepended to
//! every shader, and `shaders/bindings.wgsl` to those containing the line
//! `//!use bindings`. GPU structs and set 0 then match across all passes.
//! `shaders/surface.wgsl` (what is drawn on a unit's faces) follows for those
//! containing `//!use surface`.

use std::path::Path;

fn main() {
    let shader_dir = Path::new("shaders");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    println!("cargo:rerun-if-changed=shaders");

    let common = std::fs::read_to_string(shader_dir.join("common.wgsl")).unwrap();
    let bindings = std::fs::read_to_string(shader_dir.join("bindings.wgsl")).unwrap();
    // Local lights (lights.rs) ride along with set 0.
    let bindings = format!("{bindings}\n{}", std::fs::read_to_string(shader_dir.join("lights.wgsl")).unwrap());
    let surface = std::fs::read_to_string(shader_dir.join("surface.wgsl")).unwrap();
    check_common_layouts(&common);
    let mut failed = false;
    let mut entries: Vec<_> = std::fs::read_dir(shader_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.extension().is_none_or(|e| e != "wgsl")
            || matches!(
                path.file_stem().unwrap().to_str(),
                Some("common" | "bindings" | "surface" | "lights")
            )
        {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let body = std::fs::read_to_string(&path).unwrap();
        let mut prelude = if body.lines().any(|l| l.trim() == "//!use bindings") {
            format!("{common}\n{bindings}")
        } else {
            common.clone()
        };
        if body.lines().any(|l| l.trim() == "//!use surface") {
            prelude = format!("{prelude}\n{surface}");
        }
        let source = format!("{prelude}\n{body}");
        match compile(&source) {
            Ok(words) => {
                if name == "shields" {
                    check_strides(&source, &[("Shield", 128), ("ShieldHit", 32)]);
                }
                match name.as_str() {
                    "shockwaves" | "screen" => check_strides(&source, &[("Shockwave", 64), ("EffectBarrier", 32)]),
                    "puffs" => check_strides(&source, &[("Puff", 80)]),
                    "sprites" => check_strides(&source, &[("Effect", 48)]),
                    "terrain" => check_strides(&source, &[("Light", 64)]),
                    _ => {}
                }
                let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
                std::fs::write(Path::new(&out_dir).join(format!("{name}.spv")), bytes).unwrap();
            }
            Err(e) => {
                // Line numbers in the message count from the top of the prelude.
                let offset = prelude.lines().count() + 1;
                for line in format!("{name}.wgsl (its line 1 is line {offset} below): {e}").lines()
                {
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

/// CPU `UnitInstance` and `ModelInfo` are 160 bytes each. A `vec3` pad
/// here would align to 16 and stretch the GPU record; every entity after the
/// first would then be read from the middle of another, so models jump and
/// vanish each frame.
fn check_common_layouts(common: &str) {
    check_strides(common, &[("Entity", 192), ("ModelInfo", 432), ("HousePose", 96), ("Atmosphere", 496)]);
}

fn check_strides(source: &str, want: &[(&str, u32)]) {
    let module = naga::front::wgsl::parse_str(source).unwrap_or_else(|e| {
        panic!("{}", e.emit_to_string(source));
    });
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(module.to_ctx()).expect("wgsl layout");
    for (handle, ty) in module.types.iter() {
        let Some(name) = ty.name.as_deref() else {
            continue;
        };
        let Some((_, size)) = want.iter().find(|(n, _)| *n == name) else {
            continue;
        };
        let layout = &layouter[handle];
        assert_eq!(
            (layout.size, layout.to_stride()),
            (*size, *size),
            "{name} GPU stride must match the CPU record ({size} bytes); a vec3 member after a scalar will pad it"
        );
    }
}

fn compile(source: &str) -> Result<Vec<u32>, String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| e.emit_to_string(source))?;
    let options = naga::back::spv::Options {
        lang_version: (1, 3),
        ..Default::default()
    };
    naga::back::spv::write_vec(&module, &info, &options, None).map_err(|e| e.to_string())
}
