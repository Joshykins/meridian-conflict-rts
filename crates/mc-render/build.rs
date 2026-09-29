//! Compiles `shaders/*.wgsl` to SPIR-V with naga, so building the game needs
//! no Vulkan SDK. macOS additionally optimises the SPIR-V with spirv-opt for
//! MoltenVK's Metal translator.
//!
//! WGSL has no include mechanism, so `shaders/common.wgsl` is prepended to
//! every shader, and `shaders/bindings.wgsl` to those containing the line
//! `//!use bindings`. GPU structs and set 0 then match across all passes.
//! `shaders/surface.wgsl` (what is drawn on a unit's faces) follows for those
//! containing `//!use surface`, and `shaders/habitat.wgsl` (where things grow,
//! the air near the ground) to those containing `//!use habitat`, and
//! `shaders/scenery.wgsl` (desert bark, rock and the dam's concrete) after
//! surface to those containing `//!use scenery`; `shaders/desert.wgsl`
//! (canyon-country desert ground) after habitat for `//!use desert`;
//! `shaders/warp_hull.wgsl` and `shaders/warp_puffs.wgsl` (a capital ship's warp)
//! for `//!use warp_hull` and `//!use warp_puffs`; `shaders/emp.wgsl` and
//! `shaders/wreck.wgsl` (how a wreck lies and burns out) for their own. In front of
//! all of it go the numbers the CPU shares with the shaders, generated from
//! `mc-models/src/gpu_consts.rs`.
//!
//! **CPU-GPU layout contracts.** A WGSL struct that the CPU also writes is
//! marked with the Rust type it mirrors, on the line above it:
//!
//! ```wgsl
//! //!rust crate::renderer::Globals
//! struct Globals { ... }
//! ```
//!
//! For each one this writes a test (`OUT_DIR/gpu_layout.rs`, included by
//! `src/gpu_layout.rs`) that the Rust type has the WGSL size and that every
//! WGSL member sits at the same offset as the Rust field of the same name. A
//! marked struct may be defined in one file only; its members and the Rust
//! fields share their names, and the test does not compile otherwise.

#[path = "../mc-models/src/gpu_consts.rs"]
#[expect(
    unreachable_pub,
    reason = "the library's public gpu_consts module, compiled here as a private one"
)]
mod gpu_consts;

#[path = "src/shader_prelude.rs"]
mod shader_prelude;

use shader_prelude::{compile, Preludes, PRELUDES};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let shader_dir = Path::new("shaders");
    let out_dir = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    println!("cargo:rerun-if-changed=shaders");
    println!("cargo:rerun-if-changed=../mc-models/src/gpu_consts.rs");
    println!("cargo:rerun-if-changed=src/shader_prelude.rs");

    let read = |name: &str| {
        std::fs::read_to_string(shader_dir.join(format!("{name}.wgsl")))
            .unwrap_or_else(|e| panic!("shaders/{name}.wgsl: {e}"))
    };
    let preludes = Preludes::new(&gpu_consts::wgsl(), &read);

    let mut contracts = Contracts::default();
    for name in PRELUDES {
        contracts.scan(name, &read(name));
    }

    let mut failed = false;
    let mut entries: Vec<_> = std::fs::read_dir(shader_dir)
        .expect("shaders/")
        .map(|e| e.expect("shaders/ entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let Some(name) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        if path.extension().is_none_or(|e| e != "wgsl") || PRELUDES.contains(&name.as_str()) {
            continue;
        }
        let body = std::fs::read_to_string(&path).expect("shader source");
        contracts.scan(&name, &body);
        let (source, offset) = preludes.assemble(&body);
        let scene = body.lines().any(|l| l.trim() == "//!use bindings");
        match compile(&source) {
            Ok((module, words)) => {
                contracts.measure(&module);
                if scene {
                    contracts.scene_bindings(&name, &module);
                }
                let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
                let path = Path::new(&out_dir).join(format!("{name}.spv"));
                std::fs::write(&path, bytes).expect("write SPIR-V");
                if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
                    optimise_for_metal(&path);
                }
            }
            Err(e) => {
                // Line numbers in the message count from the top of the prelude.
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
    std::fs::write(Path::new(&out_dir).join("gpu_layout.rs"), contracts.tests())
        .expect("write gpu_layout.rs");
}

/// Inline functions and scalarise aggregate copies before SPIRV-Cross sees
/// them. Otherwise MoltenVK can mix native Metal arrays with spvUnsafeArray
/// value wrappers when passing uniform/storage array members by value.
fn optimise_for_metal(path: &Path) {
    let result = std::process::Command::new("spirv-opt")
        .args(["--target-env=spv1.3", "-O"])
        .arg(path)
        .arg("-o")
        .arg(path)
        .output()
        .expect("macOS shader builds need spirv-opt: brew install spirv-tools");
    assert!(
        result.status.success(),
        "spirv-opt failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&result.stderr)
    );
}

/// A WGSL struct marked `//!rust <path>`.
struct Contract {
    rust: String,
    file: String,
    /// Size and `(member, offset)`s, once a compiled module has laid it out.
    layout: Option<(u32, Vec<(String, u32)>)>,
}

#[derive(Default)]
struct Contracts {
    by_name: BTreeMap<String, Contract>,
    /// Scene set (group 0) bindings the shaders declare: binding to its descriptor
    /// type, as `ash::vk::DescriptorType` spells it, and the first shader seen.
    scene: BTreeMap<u32, (&'static str, String)>,
    /// Every file that defines each struct name, to catch a shared struct
    /// written out twice.
    defined_in: BTreeMap<String, Vec<String>>,
}

impl Contracts {
    fn scan(&mut self, file: &str, source: &str) {
        let mut marked: Option<String> = None;
        for line in source.lines() {
            let line = line.trim();
            if let Some(path) = line.strip_prefix("//!rust ") {
                marked = Some(path.trim().to_owned());
                continue;
            }
            let Some(rest) = line.strip_prefix("struct ") else {
                // Doc comments and attributes may sit between the mark and the struct.
                if !line.is_empty() && !line.starts_with("//") && !line.starts_with('@') {
                    marked = None;
                }
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            self.defined_in
                .entry(name.clone())
                .or_default()
                .push(file.to_owned());
            if let Some(rust) = marked.take() {
                let previous = self.by_name.insert(
                    name.clone(),
                    Contract {
                        rust,
                        file: file.to_owned(),
                        layout: None,
                    },
                );
                if let Some(previous) = previous {
                    panic!(
                        "struct {name} is marked //!rust in both {}.wgsl and {file}.wgsl; \
                         define a shared struct once, in a prelude file",
                        previous.file
                    );
                }
            }
        }
    }

    fn measure(&mut self, module: &naga::Module) {
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).expect("wgsl layout");
        for (handle, ty) in module.types.iter() {
            let Some(contract) = ty.name.as_deref().and_then(|n| self.by_name.get_mut(n)) else {
                continue;
            };
            if contract.layout.is_some() {
                continue;
            }
            let naga::TypeInner::Struct { members, span } = &ty.inner else {
                continue;
            };
            let stride = layouter[handle].to_stride();
            assert_eq!(
                stride, *span,
                "{}: its WGSL array stride ({stride}) is not its size ({span}); a vec3 member \
                 after a scalar pads it, and every record after the first would be misread",
                contract.rust
            );
            // Padding (`_`-prefixed) is not matched by name; the size and the offsets of
            // its neighbours pin it. Data never lives in a padding member.
            let members = members
                .iter()
                .filter_map(|m| Some((m.name.clone()?, m.offset)))
                .filter(|(name, _)| !name.starts_with('_'))
                .collect();
            contract.layout = Some((*span, members));
        }
    }

    fn scene_bindings(&mut self, shader: &str, module: &naga::Module) {
        for (_, var) in module.global_variables.iter() {
            let Some(binding) = &var.binding else {
                continue;
            };
            if binding.group != 0 {
                continue;
            }
            let kind = match var.space {
                naga::AddressSpace::Uniform => "UNIFORM_BUFFER",
                naga::AddressSpace::Storage { .. } => "STORAGE_BUFFER",
                _ => match &module.types[var.ty].inner {
                    naga::TypeInner::Sampler { .. } => "SAMPLER",
                    naga::TypeInner::Image {
                        class: naga::ImageClass::Storage { .. },
                        ..
                    } => "STORAGE_IMAGE",
                    naga::TypeInner::Image { .. } => "SAMPLED_IMAGE",
                    other => panic!(
                        "{shader}.wgsl: scene binding {} is a {other:?}",
                        binding.binding
                    ),
                },
            };
            let seen = self
                .scene
                .entry(binding.binding)
                .or_insert((kind, shader.to_owned()));
            assert_eq!(
                seen.0, kind,
                "scene binding {} is a {} in {}.wgsl but a {kind} in {shader}.wgsl",
                binding.binding, seen.0, seen.1
            );
        }
    }

    fn tests(&self) -> String {
        for (name, files) in &self.defined_in {
            if self.by_name.contains_key(name) && files.len() > 1 {
                panic!(
                    "struct {name} is shared with the CPU but defined in {}.wgsl: define it once, \
                     in a prelude file",
                    files.join(".wgsl and ")
                );
            }
        }
        let mut out = String::from(
            "// Generated by build.rs from the //!rust marks in shaders/. Do not edit.\n",
        );
        for (name, c) in &self.by_name {
            let Some((size, members)) = &c.layout else {
                panic!(
                    "struct {name} ({}.wgsl) is marked //!rust but no shader uses it",
                    c.file
                );
            };
            let _ = writeln!(
                out,
                "\n#[test]\nfn {}_matches_wgsl() {{",
                name.to_lowercase()
            );
            let _ = writeln!(
                out,
                "    assert_eq!(::core::mem::size_of::<{}>(), {size}, \"size of {} (WGSL {name}, {}.wgsl)\");",
                c.rust, c.rust, c.file
            );
            for (member, offset) in members {
                let _ = writeln!(
                    out,
                    "    assert_eq!(::core::mem::offset_of!({}, {member}), {offset}, \"{}.{member}\");",
                    c.rust, c.rust
                );
            }
            out.push_str("}\n");
        }
        out.push_str(
            "\n/// The scene set every `//!use bindings` shader declares, against the Vulkan \
             layout (`pipelines::SCENE_SET`).\n#[test]\nfn scene_set_matches_the_shaders() {\n    \
             use ash::vk::DescriptorType as T;\n    let declared: &[(u32, T, &str)] = &[\n",
        );
        for (binding, (kind, shader)) in &self.scene {
            let _ = writeln!(out, "        ({binding}, T::{kind}, \"{shader}.wgsl\"),");
        }
        out.push_str(
            "    ];\n    for &(binding, kind, shader) in declared {\n        \
             let layout = crate::pipelines::SCENE_SET.iter().find(|(b, _)| *b == binding);\n        \
             assert_eq!(layout.map(|l| l.1), Some(kind), \"scene binding {binding} ({shader}): the \
             layout says {layout:?}\");\n    }\n    for (binding, _) in crate::pipelines::SCENE_SET {\n        \
             assert!(declared.iter().any(|d| d.0 == *binding), \"scene binding {binding} is in the \
             layout but no shader declares it\");\n    }\n}\n",
        );
        out
    }
}
