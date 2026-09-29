//! How a shader's WGSL becomes SPIR-V: the prelude files put in front of it,
//! and the naga compile. `build.rs` includes this file to build the shaders
//! into the game, and `shader_reload` uses it to rebuild them while the
//! process runs, so the two always assemble a shader the same way.
//!
//! WGSL has no include mechanism, so `common.wgsl` goes in front of every
//! shader (after the numbers the CPU shares with the shaders, generated from
//! `gpu_consts.rs`), and `bindings.wgsl` with `lights.wgsl` in front of those
//! containing the line `//!use bindings`; then `habitat`, `desert`, `surface`,
//! `scenery`, `warp_hull` and `warp_puffs` for their own `//!use` lines, in that order.

/// Files put in front of shaders, never compiled on their own.
pub(crate) const PRELUDES: [&str; 9] = [
    "common",
    "bindings",
    "surface",
    "lights",
    "habitat",
    "scenery",
    "desert",
    "warp_hull",
    "warp_puffs",
];

/// The prelude files' text.
pub(crate) struct Preludes {
    common: String,
    bindings: String,
    surface: String,
    habitat: String,
    scenery: String,
    desert: String,
    warp_hull: String,
    warp_puffs: String,
}

impl Preludes {
    /// `read` gives a prelude file's text by name; `consts` is `gpu_consts::wgsl()`.
    pub(crate) fn new(consts: &str, read: &dyn Fn(&str) -> String) -> Preludes {
        Preludes {
            common: format!("{consts}\n{}", read("common")),
            // Local lights (lights.rs) ride along with set 0.
            bindings: format!("{}\n{}", read("bindings"), read("lights")),
            surface: read("surface"),
            habitat: read("habitat"),
            scenery: read("scenery"),
            desert: read("desert"),
            warp_hull: read("warp_hull"),
            warp_puffs: read("warp_puffs"),
        }
    }

    /// The whole source of a shader whose own text is `body`, and the number of
    /// prelude lines in front of it (for error messages).
    pub(crate) fn assemble(&self, body: &str) -> (String, usize) {
        let uses = |what: &str| body.lines().any(|l| l.trim() == format!("//!use {what}"));
        let mut prelude = if uses("bindings") {
            format!("{}\n{}", self.common, self.bindings)
        } else {
            self.common.clone()
        };
        // Where things grow and the air near the ground (needs bindings).
        if uses("habitat") {
            prelude = format!("{prelude}\n{}", self.habitat);
        }
        // Canyon-country desert colours (needs habitat).
        if uses("desert") {
            prelude = format!("{prelude}\n{}", self.desert);
        }
        if uses("surface") {
            prelude = format!("{prelude}\n{}", self.surface);
        }
        // Desert scenery's looks (needs surface).
        if uses("scenery") {
            prelude = format!("{prelude}\n{}", self.scenery);
        }
        // A capital ship's warp: its hull drawn as a streak (entity.wgsl), and the light
        // round a jump (puffs.wgsl; needs bindings).
        if uses("warp_hull") {
            prelude = format!("{prelude}\n{}", self.warp_hull);
        }
        if uses("warp_puffs") {
            prelude = format!("{prelude}\n{}", self.warp_puffs);
        }
        let lines = prelude.lines().count() + 1;
        (format!("{prelude}\n{body}"), lines)
    }
}

/// Parses, validates and writes SPIR-V 1.3; the error is naga's, with source lines.
pub(crate) fn compile(source: &str) -> Result<(naga::Module, Vec<u32>), String> {
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
    let words =
        naga::back::spv::write_vec(&module, &info, &options, None).map_err(|e| e.to_string())?;
    Ok((module, words))
}
