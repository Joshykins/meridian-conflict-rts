//! Every CPU struct the shaders read, checked against its WGSL layout: size and each
//! member's offset. The tests are written by build.rs from the `//!rust` marks in
//! shaders/ (see there), so a new shared struct is covered by marking it.

include!(concat!(env!("OUT_DIR"), "/gpu_layout.rs"));
