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
            let mut out = String::from("// Generated from src/gpu_consts.rs by build.rs. Do not edit.\n");
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

    /// Strategic icons (icons.wgsl), and the click reach that matches them (mc-game pick.rs).
    pub mod icon as "ICON_" {
        /// A tier-5 titan's icon, drawn wider than anything else's: output pixels across.
        pub const TITAN_PX: f32 = 34.0;
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
        /// Concrete: a road's asphalt.
        pub const CONCRETE_ROAD: u32 = 4;
        /// Concrete: road paint, yellow.
        pub const CONCRETE_LINE: u32 = 5;
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
}
