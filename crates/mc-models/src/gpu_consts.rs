//! Numbers the CPU and the shaders must agree on, written once, here.
//!
//! `build.rs` compiles this same file and prepends every constant to every shader
//! as `const <PREFIX><NAME>: <type> = <value>;` (so `pass::SHADOW` is `PASS_SHADOW`
//! in WGSL). A shader never writes one of these numbers itself, and Rust never
//! keeps a second copy (CLAUDE.md section 4).
//!
//! This file may use nothing but `core`: the build script compiles it on its own.

/// Declares a module of shared constants and its WGSL spelling.
macro_rules! shared {
    ($(
        $(#[$doc:meta])*
        pub mod $module:ident as $prefix:literal {
            $($(#[$cdoc:meta])* pub const $name:ident: $ty:ident = $value:expr;)*
        }
    )*) => {
        $(
            $(#[$doc])*
            pub mod $module {
                $($(#[$cdoc])* pub const $name: $ty = $value;)*

                /// This module's constants as WGSL declarations.
                pub fn wgsl(out: &mut String) {
                    $(
                        out.push_str(&format!(
                            "const {}{}: {} = {};\n",
                            $prefix,
                            stringify!($name),
                            stringify!($ty),
                            super::Literal::wgsl($name),
                        ));
                    )*
                }
            }
        )*

        /// Every shared constant as WGSL: the prelude build.rs puts before every shader.
        pub fn wgsl() -> String {
            let mut out = String::from("// Generated from mc-models/src/gpu_consts.rs by mc-render/build.rs. Do not edit.\n");
            $($module::wgsl(&mut out);)*
            out
        }
    };
}

/// A value written as a WGSL literal of its own type.
trait Literal {
    fn wgsl(self) -> String;
}

impl Literal for u32 {
    fn wgsl(self) -> String {
        format!("{self}u")
    }
}

impl Literal for i32 {
    fn wgsl(self) -> String {
        format!("{self}i")
    }
}

impl Literal for f32 {
    fn wgsl(self) -> String {
        // Debug keeps a decimal point ("1.0"), which WGSL needs to read a float.
        format!("{self:?}")
    }
}

shared! {
    /// What a draw is for, in the low byte of the entity and terrain push constant
    /// `pass_kind`; a shadow pass puts its cascade in the next byte.
    pub mod pass as "PASS_" {
        pub const MAIN: u32 = 0;
        pub const SHADOW: u32 = 1;
        /// A hull field: the posed mesh pushed out along its skin (shield depth, water).
        pub const HULL: u32 = 2;
        /// The depth pre-pass, above the kind byte so no kind test matches it.
        pub const PREPASS: u32 = 0x10000;
        pub const KIND_MASK: u32 = 0xff;
        pub const CASCADE_SHIFT: u32 = 8;
    }

    /// The draw lists the GPU cull builds each frame (cull.wgsl), each a full set of
    /// per-slot indirect commands: the colour pass's, the depth pre-pass's (no props
    /// too small to hide anything) and one per shadow cascade (only what can cast
    /// into it). Each pass draws only its own list, so a tree outside a cascade
    /// never runs the vertex shader there.
    pub mod cull_list as "CULL_LIST_" {
        pub const MAIN: u32 = 0;
        pub const PREPASS: u32 = 1;
        /// The nearest cascade's list; cascade `c` is `SHADOW + c`.
        pub const SHADOW: u32 = 2;
        pub const COUNT: u32 = 5;
    }

    /// Levels of detail past a model's own (`models::LOD_COUNT` of them), as draw slots
    /// after its first.
    pub mod lod as "LOD_" {
        /// A prop's far level (`models::Model::far`), when it is a few pixels across.
        pub const FAR: u32 = 3;
        /// Projected radius, in pixels, under which a prop draws its far level.
        pub const FAR_PX: f32 = 5.0;
    }

    /// Which side of the clouds a shot draw is (sprites.wgsl `push.layer`): the
    /// tracers and dots go under them up close, and over them, with the
    /// strategic icons, once they are yellow markers.
    pub mod sprite_layer as "SPRITE_LAYER_" {
        pub const UNDER_CLOUD: u32 = 0;
        pub const OVER_CLOUD: u32 = 1;
    }

    /// A giant's footprint (ground.wgsl `vs_print`, renderer `titan_fx`): drawn as a grid of
    /// this many cells a side, each vertex on the ground, so it lies over hills and hollows.
    pub mod print as "PRINT_" {
        pub const GRID: u32 = 24;
    }

    /// Strategic icons (icons.wgsl), and the click reach that matches them (mc-game pick.rs).
    pub mod icon as "ICON_" {
        /// A tier-5 titan's icon, drawn wider than anything else's: output pixels across.
        pub const TITAN_PX: f32 = 34.0;
        /// `ModelInfo::icon` bit: the model is a mobile unit.
        pub const MOBILE: u32 = 0x10000;
        /// `ModelInfo::icon` bit: the unit flies.
        pub const AIR: u32 = 0x40000;
    }

    /// The selection mark (icons.wgsl `vs_ring`).
    pub mod ring as "RING_" {
        /// Cells along each side of a mark's ground grid: a mark is drawn as
        /// `GRID * GRID` quad instances so it drapes over the terrain.
        pub const GRID: u32 = 12;
    }

    /// Desert map scenery the entity shader dresses by a face's pattern byte
    /// (models/desert.rs, models/dam.rs, scenery.wgsl). Patterns only mean panel
    /// detail on plated materials, so leaves, bark, rock and concrete use the
    /// byte for looks of their own.
    pub mod scenery as "SCENERY_" {
        /// Leaf-card pattern: the desert leaf atlas (juniper, pinyon, cottonwood).
        pub const LEAF_DESERT: u32 = 2;
        /// The desert atlas's layer after FOLIAGE_BASE (foliage.rs).
        pub const FOLIAGE_DESERT: i32 = 7;
        /// Bark: a juniper's shaggy, stringy, silver-grey bark.
        pub const BARK_SHAGGY: u32 = 4;
        /// Rock: bedded red sandstone, its laminae level in the model.
        pub const ROCK_BEDDED: u32 = 1;
        /// Concrete: cast mass concrete, its lifts and block joints showing.
        pub const CONCRETE_CAST: u32 = 1;
        /// Concrete: the white mineral ring a drawn-down reservoir leaves.
        pub const CONCRETE_RING: u32 = 2;
        /// Concrete: dark and wet, just over the water.
        pub const CONCRETE_WET: u32 = 3;
        // retired: 4 (a road's asphalt), 5 (road paint)
        /// Concrete: a deep opening's dark mouth.
        pub const CONCRETE_SHADOW: u32 = 6;
        /// Painted steel on scenery: red and white (the dam's cranes).
        pub const CONCRETE_RED: u32 = 7;
        pub const CONCRETE_WHITE: u32 = 8;
        /// Concrete: a dry spillway chute, stained dark and rust-streaked.
        pub const CONCRETE_CHUTE: u32 = 9;
        /// A pale blue-grey painted metal roof.
        pub const CONCRETE_ROOF: u32 = 10;
    }

    /// A gun's breech door (`rig::BREECH`, `Model::breech`): swings open on its hinge as
    /// the gun kicks and shuts as it runs out (entity.wgsl `breech_open`).
    pub mod breech as "BREECH_" {
        /// The rig bit. It borrows the lowest `UPGRADE_AT` bit, which only means anything
        /// on refit pieces; a breech door never is one.
        pub const RIG: u32 = 0x10000;
    }

    /// A wall section that joins its neighbours (`models::wall`): each quarter of
    /// its lot holds every piece that quarter could need, and the entity shader draws the
    /// one its neighbours call for. The neighbours are `status[2]`'s bits
    /// (`mc_sim::mirror::join_walls`): bit k is the cell k × 45° counter-clockwise from
    /// the section's own +x. Quarter q lies between side bits 2q and 2q + 2, with the
    /// corner bit 2q + 1 between them.
    pub mod wall as "WALL_" {
        /// The first piece's part: quarter q's piece `case` is `PART_FIRST + q * CASES + case`.
        pub const PART_FIRST: u32 = 26;
        pub const CASES: u32 = 5;
        /// Neither side joined: a quarter of the pillar that ends or turns a wall.
        pub const CAP: u32 = 0;
        /// Only the side at bit 2q joined: the wall running out that way.
        pub const RUN_A: u32 = 1;
        /// Only the side at bit 2q + 2 joined.
        pub const RUN_B: u32 = 2;
        /// Both sides, not the corner: the inside of a turn.
        pub const JOIN: u32 = 3;
        /// Both sides and the corner: the quarter is filled, a block of walls is one
        /// thick wall.
        pub const FULL: u32 = 4;
    }

    /// Grass round the eye (renderer/grass.rs, grass_gen.wgsl, grass.wgsl).
    pub mod grass as "GRASS_" {
        /// A candidate tuft per this many metres each way.
        pub const CELL_M: f32 = 0.28;
        /// Below this many pixels a cell grows no grass: the ground's own colour
        /// carries on from there (terrain.wgsl).
        pub const MIN_PX: f32 = 1.4;
        /// The trample map's side in metres, a texel each.
        pub const WINDOW: i32 = 512;
        /// Presses (units, props, lots, scorches, track marks) gathered at most.
        pub const MAX_PRESS: u32 = 4096;
        /// Blades per tuft and segments per blade in the near, middle and far band.
        pub const NEAR_BLADES: u32 = 24;
        pub const NEAR_SEGMENTS: u32 = 4;
        pub const MID_BLADES: u32 = 14;
        pub const MID_SEGMENTS: u32 = 3;
        pub const FAR_BLADES: u32 = 8;
        pub const FAR_SEGMENTS: u32 = 2;
        /// Tufts each band holds, laid out near, middle, far in the tuft buffer.
        pub const NEAR_CAP: u32 = 98304;
        pub const MID_CAP: u32 = 262144;
        pub const FAR_CAP: u32 = 524288;
    }

    /// Puff kinds (puffs.wgsl, renderer `push_puff`). The older kinds are still
    /// spelled out on both sides; new ones are declared here.
    pub mod puff as "PUFF_" {
        /// Burning gas off a unit or a shell going up (renderer/blast_fx.rs): an
        /// opaque, billowing ball, white-yellow inside, that cools to soot from the
        /// rim in and rises as smoke.
        pub const BLAST: u32 = 38;
        /// A shard of a flak shell's casing (renderer/flak_fx.rs): a hot metal streak
        /// as long as it is fast, flung out to the burst's splash and slowed by the air.
        pub const SHRAPNEL: u32 = 37;
        /// A flak burst's smoke (renderer/flak_fx.rs): the charge burning inside a
        /// hard-edged black ball that hangs on the wind for seconds.
        pub const FLAK: u32 = 39;
    }

    /// A strategic launcher (models/aster/strategic.rs, entity.wgsl). The rounds word
    /// the sim writes in `status[2]` (`mc_sim::nukes::LAUNCHER_*`; a test holds them
    /// equal), and the plant that works a load cycle while a round is assembling: a hatch
    /// lid over the store slides open, a hoist block goes down into it and back up, the
    /// lid shuts.
    pub mod launcher as "LAUNCHER_" {
        pub const STOCK_MASK: u32 = 0xFF;
        pub const CAPACITY_SHIFT: u32 = 16;
        pub const MARK: u32 = 0x2000000;
        /// Auto-build off: it assembles only rounds queued by hand.
        pub const MANUAL: u32 = 0x4000000;
        pub const QUEUED_SHIFT: u32 = 27;
        /// The Sunfall's icon (`IconKind::Silo`); every other launcher is an array.
        pub const ICON_SILO: u32 = 25;
        /// The store's hatch lid: slides open along -x on the silo, -y on the array.
        pub const PART_LID: u32 = 23;
        /// The hoist block and the lower half of its cables: let down and hauled up.
        pub const PART_HOIST: u32 = 46;
        /// Seconds in one load cycle.
        pub const CYCLE_S: f32 = 13.0;
        pub const SILO_LID_TRAVEL: f32 = 8.6;
        pub const SILO_HOIST_DROP: f32 = 3.7;
        /// Hoist vertices above this height are the cables' tops, held at the trolley.
        pub const SILO_HOIST_SPLIT: f32 = 16.0;
        pub const ARRAY_LID_TRAVEL: f32 = 2.4;
        pub const ARRAY_HOIST_DROP: f32 = 1.2;
        pub const ARRAY_HOIST_SPLIT: f32 = 7.8;
    }

    /// Work-beam kinds (beams.wgsl) that the sim writes into `BeamInstance::kind`
    /// (`mc_sim::reclaim`). The older kinds are still spelled out in the shader.
    pub mod beam as "BEAM_" {
        /// A Naga builder's nanite stream, emitter to weld (`reclaim::BEAM_NANITE`).
        pub const NANITE: u32 = 1;
        /// A Naga site being fed: its rings and rising filaments (`reclaim::BEAM_NANITE_SITE`).
        pub const NANITE_SITE: u32 = 6;
    }

    /// Materials (the sim's mass) as a light: the HUD's Materials red-orange
    /// (`hud::MASS`, 0xFF6B3D) in linear RGB. Reclaim's beams, the stream down a reclaim
    /// tower's chutes and anything else that shows mass on the move share it.
    pub mod mass as "MASS_" {
        pub const R: f32 = 1.0;
        pub const G: f32 = 0.147;
        pub const B: f32 = 0.0467;
        /// `pattern::MASS_FLOW`: a chute with material falling down it while the unit reclaims.
        pub const FLOW_PATTERN: u32 = 30;
        /// How fast the clumps fall down a chute, metres a second.
        pub const FLOW_SPEED: f32 = 7.0;
        /// `material::GLOW_MATERIALS`: a reclaim emitter, lit in this colour while the unit
        /// reclaims (`unit_flag::RECLAIMING`) and banked low while it does not.
        pub const GLOW_MATERIAL: u32 = 27;
    }

    /// Bits of a selection mark's `kind` (renderer `Mark`, icons.wgsl `fs_ring`).
    pub mod mark as "MARK_" {
        /// Hovered; without it the mark is a selection.
        pub const HOVER: u32 = 1;
        /// Hostile: the ground brackets go red.
        pub const ENEMY: u32 = 2;
        /// No selection ring, only the status bars (work under way, seen unselected).
        pub const BARS_ONLY: u32 = 4;
        /// What a click would take apart: ringed in the Materials red-orange (`MASS_*`).
        pub const RECLAIM: u32 = 8;
    }

    /// Effect kinds (sprites.wgsl `Effect::params.z`, lights.rs `effect`). The older kinds
    /// are still spelled out on both sides; new ones are declared here.
    pub mod effect as "EFFECT_" {
        /// The last of a hull going up a reclaim beam: a flare in the Materials red-orange.
        pub const MATERIALS: u32 = 10;
    }

    /// Bits of `UnitInstance::owner_flags` (`owner | flags << 8`) the shaders read that the
    /// older hand-written `FLAG_*` list in common.wgsl does not hold.
    pub mod unit_flag as "UNIT_FLAG_" {
        /// `mc_sim::tables::flag::RECLAIMING << 8`: pulling mass out of a wreck this tick.
        pub const RECLAIMING: u32 = 0x10000;
    }
}
