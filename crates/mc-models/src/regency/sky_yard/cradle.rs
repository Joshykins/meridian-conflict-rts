//! Design B, the cradle: four great plated arms rise from the lot's corners and lean in,
//! their points standing apart, their claws holding a wide lens ring that turns, lit
//! violet, round the emitter's crown, which their fingers carry in over it; the beam
//! leaves up through it. In
//! the bed under it a stack of rotor drums turns, the intakes' conduits running in.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::gravitic_engine::rotors::{self, Rotor};
use super::super::kit::dark_plate;
use super::super::machine::{armour, collar, strut, swept, Frame};
use super::ground::{apron, dir, emitter_crown, intakes, limb};
use super::EMITTER;

/// The lens ring the claws hold.
const LENS: Rotor = Rotor {
    z0: 266.0,
    z1: 276.0,
    outer: 60.0,
    inner: 48.0,
    vanes: 16,
    rate: 0.18,
    lit: true,
};

pub(in crate::regency) fn cradle(b: &mut MeshBuilder, _tech: u8) {
    apron(b);
    for k in 0..4 {
        let a = 45.0 + 90.0 * k as f32;
        let d = dir(a);
        let side = dir(a + 90.0);
        limb(
            b,
            &[
                (d * 120.0 + Vec3::Z * 0.5, 13.0),
                (d * 125.0 + Vec3::Z * 90.0, 11.5),
                (d * 114.0 + Vec3::Z * 180.0, 10.0),
                (d * 92.0 + Vec3::Z * 248.0, 8.0),
                (d * 76.0 + Vec3::Z * 288.0, 5.5),
                (d * 72.0 + Vec3::Z * 304.0, 3.0),
            ],
            d + Vec3::Z * 0.6,
        );
        if b.coarse() {
            continue;
        }
        // The plated boot it stands in, an optic looking out of it.
        dark_plate(b);
        b.with_facets(|b| b.prism(d * 118.0, 6, 18.0, 13.0, 12.0));
        // The claw: two plates either side of the ring's rim, points in.
        for s in [-1.0f32, 1.0] {
            dark_plate(b);
            let f = Frame::new(
                d * 74.0 + side * (s * 5.0) + Vec3::Z * 270.0,
                -d + Vec3::Z * 0.1 * s,
                side * s + Vec3::Z * 0.3,
            );
            b.with_facets(|b| armour(b, &f, &swept(16.0, 5.0, -0.5 * s, 0.4), 1.2));
        }
        // Its finger in over the ring to the crown, above the ring's top.
        strut(
            b,
            d * 80.0 + Vec3::Z * 284.0,
            d * 15.0 + EMITTER - Vec3::Z * 5.0,
            1.8,
        );
        if b.fine() {
            b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
            let f = Frame::new(d * 135.0 + Vec3::Z * 2.0, Vec3::Z + d * 0.3, d);
            armour(b, &f, &swept(10.0, 4.5, 0.0, 0.7), 0.5);
        }
    }
    if b.coarse() {
        return;
    }
    intakes(b, 40.0, 30.0);
    let mut turning = rotors::stack(3, 32.0, 54.0, 0.14);
    turning.push(LENS);
    rotors::build(b, &turning);
    // The ring's bearing blocks in the claws.
    for k in 0..4 {
        let d = dir(45.0 + 90.0 * k as f32);
        collar(b, d * 63.0 + Vec3::Z * 271.0, d, 3.2, 3.0);
    }
    emitter_crown(b, 6, 13.0);
}
