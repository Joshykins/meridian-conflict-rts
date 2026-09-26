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
}
