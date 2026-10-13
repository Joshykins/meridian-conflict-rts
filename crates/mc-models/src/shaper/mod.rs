//! Survival's Shapers (mc-sim `survival`): Precursor spires in the Heart's language
//! (`precursor_sky.rs`) that grow with their level, taller and broader at once.
//!
//! - `replication_node`, the printing Shaper: the braced blades (`printer.rs`), a sheer
//!   core telescoping up between leaning slab blades, the Lens on its head.
//! - `progenitor_seedcaster` and `progenitor_unmaker`, the auxiliary Shapers: the stacked
//!   spire (`auxiliary.rs`), stages hovering over each other on necks with collars round
//!   them, the Lens near the top, under an artillery bore, a silo crown or, for the
//!   Gatewright (`progenitor_gatewright`, fleets), a great upright gate ring.
//!
//! Round them stand the pieces of their bases (`base.rs`): print platforms and slips,
//! shield pylons.
//!
//! Every one carries its anti-nuke as the halo round its Lens (`halo.rs`), a blade to an
//! interceptor held.
//!
//! Each is drawn by stage: a mesh for each level in [`STAGE_LEVELS`], keyed `<mesh>` at
//! level 1 (the blueprint's own) and `<mesh>@<level>` past it, authored at its true size
//! (its `ModelDef` radius is the reach of its foot, [`reach`]). The renderer draws a
//! Shaper with the last stage its level has reached, cut off at its built height so the
//! newest part is seen printing in (mc-render `shaper_growth.rs`).
//!
//! Model space as for every unit: x forward (the side its platforms are on), y left,
//! z up, the origin on the ground at its middle.

use super::library::ModelDef;

/// How many stages each Shaper has a mesh for.
pub const STAGES: usize = 16;

/// The levels with a mesh of their own: every level while a level still adds a lot of
/// height, then further apart as the height closes on its limit. Past the last the
/// Shaper keeps its last stage (it is within 1% of its height's limit).
pub const STAGE_LEVELS: [u16; STAGES] = [1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 25, 30, 40];

/// Metres a Shaper of `level` stands when fully grown: mc-sim `survival::shaper_height`
/// (a test in mc-render holds the two equal), `60 + 1100 (1 - 0.9^(level - 1))`.
pub const fn height(level: u16) -> f32 {
    let mut left = 1.0f32;
    let mut i = 1;
    while i < level && i < 200 {
        left *= 0.9;
        i += 1;
    }
    60.0 + 1100.0 * (1.0 - left)
}

/// How far a Shaper has grown toward its height's limit, 0 at level 1 toward 1.
const fn grown(level: u16) -> f32 {
    (height(level) - 60.0) / 1100.0
}

/// A Shaper's visual tier: mc-sim `survival::shaper_tier` (held equal by the same test),
/// one every four levels, each a section, stage or collar more.
pub const fn tier(level: u16) -> usize {
    1 + level.saturating_sub(1) as usize / 4
}

/// Metres from a Shaper's middle to the outer edge of its foot at `level`, by `kind`
/// (`ShaperInstance::kind`: 0 a printer, else an auxiliary): mc-sim
/// `survival::shaper_foot` at the level's height, which the sim blocks (a test in
/// mc-render holds the two equal). The blades' feet stand out to it.
pub const fn reach(kind: u8, level: u16) -> f32 {
    if kind == 0 {
        30.0 + 170.0 * grown(level)
    } else {
        28.0 + 150.0 * grown(level)
    }
}

/// The Lens and halo of the stage a Shaper of `kind` (`ShaperInstance::kind`) and
/// `level` is drawn with.
fn mount(kind: u8, level: u16) -> halo::Mount {
    let g = Growth::at(kind, STAGE_LEVELS[stage(level)]);
    match kind {
        0 => printer::mount(g),
        _ => auxiliary::mount(g, kind),
    }
}

/// Metres over a Shaper's foot of the middle of its Lens, where its Lance charges and
/// strikes from, as its stage is drawn: the printer's on its core's head (printer.rs), a
/// Seedcaster's under its bore and an Unmaker's under its silo crown (auxiliary.rs). The
/// renderer's hard light and the game's sounds put the Lens here.
pub fn lens_height(kind: u8, level: u16) -> f32 {
    mount(kind, level).lens_mid
}

/// Slots in the halo a Shaper of `kind` and `level` is drawn with.
pub fn halo_slot_count(kind: u8, level: u16) -> usize {
    mount(kind, level).g.slots.max(1)
}

/// Metres long an interceptor blade hangs in the halo of a Shaper of `kind` and `level`.
pub fn halo_blade(kind: u8, level: u16) -> f32 {
    mount(kind, level).len
}

/// The middle of the blade in halo slot `k`, in the Shaper's model space (x forward, y
/// left, z up from its foot): where the interceptor it stands for leaves from.
pub fn halo_slot(kind: u8, level: u16, k: usize) -> [f32; 3] {
    halo::slot_middle(&mount(kind, level), k).to_array()
}

/// Interceptors a Shaper of `level` may hold: its blueprint's `interceptor_stock`, 1
/// and 0.34 a level, rounded down (mc-sim `growth::grown_launcher`; a test in mc-render
/// holds the two equal).
pub const fn interceptors(level: u16) -> usize {
    let level = if level == 0 { 1 } else { level as usize };
    (100 + 34 * (level - 1)) / 100
}

/// Slots in the halo of the stage first drawn at `level`: what the last level drawn with
/// it may hold, so each interceptor has a blade of its own. Past the last stage the
/// slots stand for shares of what it holds (entity.wgsl `halo_slot`).
fn halo_slots(level: u16) -> usize {
    let last = STAGE_LEVELS
        .iter()
        .find(|&&l| l > level)
        .map_or(level, |&next| next - 1);
    interceptors(last)
}

/// Which of [`STAGE_LEVELS`] a Shaper of `level` is drawn with: the last it has reached.
pub fn stage(level: u16) -> usize {
    STAGE_LEVELS
        .iter()
        .rposition(|&l| l <= level.max(1))
        .unwrap_or(0)
}

/// The stage meshes' keys of a Shaper of `kind` (`ShaperInstance::kind`: 0 printer,
/// 1 artillery, 2 nuke, 3 fleet), in [`STAGE_LEVELS`] order.
pub fn stage_keys(kind: u8) -> &'static [&'static str; STAGES] {
    match kind {
        1 => &auxiliary::ARTILLERY_KEYS,
        2 => &auxiliary::NUKE_KEYS,
        3 => &auxiliary::FLEET_KEYS,
        _ => &printer::KEYS,
    }
}

pub(super) const MODELS: [&[ModelDef]; 6] = [
    &[auxiliary::SILO],
    &printer::MODELS,
    &auxiliary::ARTILLERY_MODELS,
    &auxiliary::NUKE_MODELS,
    &base::MODELS,
    &auxiliary::FLEET_MODELS,
];

/// How far a Shaper has grown, as its meshes are drawn.
#[derive(Clone, Copy)]
struct Growth {
    height: f32,
    tier: usize,
    /// The reach of its foot ([`reach`]).
    reach: f32,
    /// Its halo's slots ([`halo_slots`]).
    slots: usize,
}

impl Growth {
    fn at(kind: u8, level: u16) -> Self {
        Growth {
            height: height(level),
            tier: tier(level),
            reach: reach(kind, level),
            slots: halo_slots(level),
        }
    }
}

/// One `ModelDef` and build function per stage of a Shaper: `$draw(b, growth)` draws it.
macro_rules! stage_set {
    ($kind:literal, $keys:ident, $defs:ident, $draw:path; $($f:ident $key:literal $level:literal,)*) => {
        $(fn $f(b: &mut MeshBuilder, _tech: u8) {
            $draw(b, super::Growth::at($kind, $level));
        })*
        pub(super) const $keys: [&str; super::STAGES] = [$($key),*];
        pub(super) const $defs: [ModelDef; super::STAGES] = [
            $(ModelDef::new($key, super::reach($kind, $level), super::height($level), $f),)*
        ];
    };
}

mod auxiliary;
mod base;
pub use base::pad_emitters;
mod halo;
mod kit;
mod printer;

/// The triangle budgets of a Shaper's stage mesh `key` (coarse level, reduced level's
/// share of the full, full level): one to a site, up to a kilometre high, seen from
/// across the map, so its reduced level keeps its rings.
#[cfg(test)]
pub(super) fn budget(key: &str) -> Option<(usize, f32, usize)> {
    if base::MODELS.iter().any(|m| m.key == key) {
        // A base piece (`base.rs`): a great platform is 190 m across.
        Some((700, 0.8, 9_000))
    } else if key.starts_with("replication_node") {
        // Its halo hangs a blade to each interceptor even far off: the stock reads.
        Some((2_000, 0.56, 12_000))
    } else if key.starts_with("progenitor_gatewright") {
        // The Seedcaster's spire under a great gate ring, its segments far off too.
        Some((3_400, 0.56, 28_000))
    } else if key.starts_with("progenitor_seedcaster") || key.starts_with("progenitor_unmaker") {
        // Collars round every neck, and a halo as broad as its Lens.
        Some((3_000, 0.56, 28_000))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
