//! The Regency's gravity lift, for any of their machines that float (docs/STYLE.md "The
//! Regency suite"): a bronze collar let into the hull, and hung in it a lift bell
//! (`part::LOCOMOTION`, so a hovercraft's hull heaves over it while it stays low), a dark
//! core ringed in red where the lift leaves it. Each bell's mouth is marked
//! (`MeshBuilder::add_lift`): red plasma crackles under it while the machine is up
//! (renderer `lift_fx.rs`).

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;

use super::kit::{metal, seam};
use super::machine::hoop;

/// A lift bell of radius `r` whose mouth opens at `mouth` (its middle, facing down), its
/// collar reaching up `depth` into the hull above it.
pub(super) fn bell(b: &mut MeshBuilder, mouth: Vec3, r: f32, depth: f32) {
    b.add_lift(mouth, r * 0.7);
    let sides = b.sides(10);
    let collar = mouth.z + 0.3 * depth.max(0.4);
    metal(b);
    b.prism(
        mouth.with_z(collar),
        sides,
        r,
        r * 0.95,
        depth - (collar - mouth.z),
    );
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.prism(
            mouth.with_z(mouth.z + 0.06),
            sides,
            r * 0.86,
            r * 0.84,
            collar - mouth.z + 0.2,
        );
        if b.fine() {
            b.prism(mouth, sides, r * 0.55, r * 0.7, 0.08);
            b.paint(GLOW_LASER);
            hoop(
                b,
                mouth.with_z(mouth.z + 0.1),
                r * 0.7,
                0.08 * r.max(0.6),
                0.05,
                12,
            );
        }
    });
}
