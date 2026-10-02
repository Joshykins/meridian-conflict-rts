//! How a shader's WGSL becomes SPIR-V: the prelude files put in front of it,
//! and the naga compile. `build.rs` includes this file to build the shaders
//! into the game, and `shader_reload` uses it to rebuild them while the
//! process runs, so the two always assemble a shader the same way.
//!
//! WGSL has no include mechanism, so `common.wgsl` goes in front of every
//! shader (after the numbers the CPU shares with the shaders, generated from
//! `gpu_consts.rs`), and `bindings.wgsl` with `regions.wgsl` and `lights.wgsl` in
//! front of those containing the line `//!use bindings` (`regions` alone for a
//! shader that binds the atmosphere itself and says `//!use regions`); then `shore`, `habitat`, `desert`, `rock`, `surface`,
//! `regency`, `scenery`, `warp_hull` and `warp_puffs` for their own `//!use` lines, in that order,
//! then `emp` (an EMP stun's look on a model) and `wreck` (how a wreck lies and looks).

/// Files put in front of shaders, never compiled on their own.
pub(crate) const PRELUDES: [&str; 17] = [
    "common",
    "bindings",
    "regions",
    "shore",
    "surface",
    "regency",
    "lights",
    "habitat",
    "scenery",
    "desert",
    "rock",
    "warp_hull",
    "warp_puffs",
    "plasma_puffs",
    "emp",
    "wreck",
    "nova",
];

/// The prelude files' text.
pub(crate) struct Preludes {
    common: String,
    bindings: String,
    regions: String,
    shore: String,
    surface: String,
    regency: String,
    habitat: String,
    scenery: String,
    desert: String,
    rock: String,
    warp_hull: String,
    warp_puffs: String,
    plasma_puffs: String,
    emp: String,
    wreck: String,
    nova: String,
}

impl Preludes {
    /// `read` gives a prelude file's text by name; `consts` is `gpu_consts::wgsl()`.
    pub(crate) fn new(consts: &str, read: &dyn Fn(&str) -> String) -> Preludes {
        Preludes {
            common: format!("{consts}\n{}", read("common")),
            // A map's regions (they read set 0's atmosphere) and the local lights
            // (lights.rs) ride along with set 0.
            bindings: format!(
                "{}\n{}\n{}",
                read("bindings"),
                read("regions"),
                read("lights")
            ),
            regions: read("regions"),
            shore: read("shore"),
            surface: read("surface"),
            regency: read("regency"),
            habitat: read("habitat"),
            scenery: read("scenery"),
            desert: read("desert"),
            rock: read("rock"),
            warp_hull: read("warp_hull"),
            warp_puffs: read("warp_puffs"),
            plasma_puffs: read("plasma_puffs"),
            emp: read("emp"),
            wreck: read("wreck"),
            nova: read("nova"),
        }
    }

    /// The whole source of a shader whose own text is `body`, and the number of
    /// prelude lines in front of it (for error messages).
    pub(crate) fn assemble(&self, body: &str) -> (String, usize) {
        let uses = |what: &str| body.lines().any(|l| l.trim() == format!("//!use {what}"));
        let mut prelude = if uses("bindings") {
            format!("{}\n{}", self.common, self.bindings)
        } else if uses("regions") {
            // A map's regions for a shader with an atmosphere of its own (clouds_sim.wgsl).
            format!("{}\n{}", self.common, self.regions)
        } else {
            self.common.clone()
        };
        // Waves on the shore (needs bindings).
        if uses("shore") {
            prelude = format!("{prelude}\n{}", self.shore);
        }
        // Where things grow and the air near the ground (needs bindings).
        if uses("habitat") {
            prelude = format!("{prelude}\n{}", self.habitat);
        }
        // Canyon-country desert colours (needs habitat).
        if uses("desert") {
            prelude = format!("{prelude}\n{}", self.desert);
        }
        // Cliff stone, for the walls and the rock on them (needs desert).
        if uses("rock") {
            prelude = format!("{prelude}\n{}", self.rock);
        }
        if uses("surface") {
            prelude = format!("{prelude}\n{}", self.surface);
        }
        // Regency plate and bronze (needs surface).
        if uses("regency") {
            prelude = format!("{prelude}\n{}", self.regency);
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
        // The Regency's squeezed plasma as light (puffs.wgsl; needs bindings).
        if uses("plasma_puffs") {
            prelude = format!("{prelude}\n{}", self.plasma_puffs);
        }
        // An EMP stun's look on a model (needs common).
        if uses("emp") {
            prelude = format!("{prelude}\n{}", self.emp);
        }
        // How a wreck lies, breaks up and burns out (entity.wgsl; needs bindings).
        if uses("wreck") {
            prelude = format!("{prelude}\n{}", self.wreck);
        }
        // A Regency warhead's plasma nova (nuke.wgsl; uses its types and billows).
        if uses("nova") {
            prelude = format!("{prelude}\n{}", self.nova);
        }
        let lines = prelude.lines().count() + 1;
        (format!("{prelude}\n{body}"), lines)
    }
}

/// Parses, validates and writes SPIR-V 1.3 with bounds checks; the error is naga's, with
/// source lines.
pub(crate) fn compile(source: &str) -> Result<(naga::Module, Vec<u32>), String> {
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| e.emit_to_string(source))?;
    // Every index, buffer access and texel load clamped in range. naga's default is
    // unchecked: one index past the end then reads unmapped memory, which NVIDIA
    // shrugs off and AMD answers with a page fault, the device lost (a friend's
    // RX 7900 XTX, 2026-09-29).
    let restrict = naga::proc::BoundsCheckPolicy::Restrict;
    let options = naga::back::spv::Options {
        lang_version: (1, 3),
        bounds_check_policies: naga::proc::BoundsCheckPolicies {
            index: restrict,
            buffer: restrict,
            image_load: restrict,
            binding_array: restrict,
        },
        ..Default::default()
    };
    let words =
        naga::back::spv::write_vec(&module, &info, &options, None).map_err(|e| e.to_string())?;
    Ok((module, words))
}
