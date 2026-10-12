//! The Regency mobile shield, the Targe (`regency_t2_mobile_shield`): a craft on gravity
//! lift, no legs, so it crosses water as it crosses land, and on its back the projector
//! that holds a field over the army round it.
//!
//! - The hull (`hull`): a broad plated lozenge, a sponson either side lapped in swept
//!   plates, a lift bell under each end of each sponson and one under the body, a sensor
//!   head with red optics at the nose, a plated plinth over its middle.
//! - The projector (`projector`): four swept plates rising off the plinth's corners and
//!   curling in over a graphite emitter at the unit's middle, where the field's jet
//!   leaves (the unit file's `shield_projector`): the dome's shaft always rises from the
//!   unit's origin.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, graphite machinery, red slots
//! let into the plates and the Ward's prism light in the emitter's crown. No violet: it
//! does not build. No star: a Targe that dies does not go nova.
//!
//! Rig: a hovercraft (`MeshBuilder::set_hover`): the shader heaves the hull over its lift
//! bells. Nothing turns.

mod hull;
mod projector;

#[cfg(test)]
mod tests;

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::kit::{dark_plate, metal, seam, v3};

/// The deck, and the top of the plinth the projector stands on.
const DECK: f32 = 2.1;
const PLINTH: f32 = 3.0;

pub(super) fn shield_hover(b: &mut MeshBuilder, _tech: u8) {
    b.set_hover();
    b.set_dust_line(0.7);
    if b.coarse() {
        coarse(b);
        return;
    }
    hull::hull(b);
    projector::crown(b);
}

/// Far off: a wedge of a hull with the team colour on its back, the lift under it, and
/// the crown as four plates round a graphite emitter.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.frustum(
        v3(0.0, 0.0, 0.6),
        Vec2::new(8.6, 6.4),
        Vec2::new(6.0, 3.2),
        DECK - 0.6,
        Vec2::new(-0.2, 0.0),
    );
    hull::team(b);
    b.with_part(crate::part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[
            v3(3.0, 0.0, 0.45),
            v3(-3.4, -2.8, 0.45),
            v3(-3.4, 2.8, 0.45),
        ]);
    });
    let (crown, top) = projector::CROWN;
    metal(b);
    b.cylinder_between(v3(0.0, 0.0, DECK), v3(0.0, 0.0, crown), 1.2, 0.4, 4);
    dark_plate(b);
    for k in 0..4 {
        let a = FRAC_PI_4 + k as f32 * FRAC_PI_2;
        let d = v3(a.cos(), a.sin(), 0.0);
        let across = Vec3::Z.cross(d) * 0.6;
        let quad = [
            d * 1.6 - across + Vec3::Z * DECK,
            d * 1.6 + across + Vec3::Z * DECK,
            d * 0.9 + across * 0.6 + Vec3::Z * top,
            d * 0.9 - across * 0.6 + Vec3::Z * top,
        ];
        b.face(&quad);
        b.face(&[quad[3], quad[2], quad[1], quad[0]]);
    }
}
