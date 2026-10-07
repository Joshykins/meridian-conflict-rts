//! Variant `regency_colossus~b`: the same hull, head and weapons on heavier legs, low and
//! crab-like, the knee out beyond the hull below the shoulder, thick limbs, a broader stance.
//! Isolated here so deleting it, once a look is chosen, is deleting this file, its `mod` and
//! `use` lines in `mod.rs` and its catalogue entry in `regency/mod.rs`.

use crate::builder::MeshBuilder;

use super::legs::Style;

const SLUNG: Style = Style {
    hip_z: 30.5,
    hip_r: 7.4,
    knee_out: 19.0,
    knee_up: -5.5,
    front_foot: (13.0, 31.5),
    rear_foot: (-15.0, 31.5),
    thigh: [5.8, 4.8, 4.0, 3.5],
    shin: [4.2, 4.2, 2.5, 2.6],
    knee_r: 5.2,
    foot_r: 4.8,
};

pub(in crate::regency) fn colossus_b(b: &mut MeshBuilder, _tech: u8) {
    super::build(b, &SLUNG);
}
