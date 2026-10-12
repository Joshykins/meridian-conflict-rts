//! The Targe's projector over the plinth: the emitter at the unit's middle that the
//! field's jet leaves from (the unit file's `shield_projector`), and the crown of plates
//! round it.

use std::f32::consts::{FRAC_PI_4, TAU};

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, swept, Frame};
use super::super::parts::inlay;
use super::PLINTH;

/// The direction at bearing `a` (radians) round z.
fn out(a: f32) -> Vec3 {
    v3(a.cos(), a.sin(), 0.0)
}

/// The emitter: a graphite cone up the unit's middle to `top`, a prism lens in its crown.
fn emitter(b: &mut MeshBuilder, top: f32) {
    let sides = b.sides(10);
    metal(b);
    b.cylinder_between(
        v3(0.0, 0.0, PLINTH - 0.05),
        v3(0.0, 0.0, top - 0.3),
        0.75,
        0.38,
        sides,
    );
    seam(b);
    b.cylinder_between(
        v3(0.0, 0.0, top - 0.3),
        v3(0.0, 0.0, top - 0.18),
        0.42,
        0.4,
        sides,
    );
    b.paint(GLOW_PRISM);
    b.cylinder_between(
        v3(0.0, 0.0, top - 0.18),
        v3(0.0, 0.0, top),
        0.34,
        0.18,
        sides,
    );
}

/// A red slot let into a plate's face: a strip `len` along `f.u` at `s`, on the face at
/// height `h` turned toward `n`.
fn slot(b: &mut MeshBuilder, f: &Frame, s: f32, h: f32, n: Vec3, len: f32, w: f32) {
    let c = f.at(s, 0.0, h);
    let (u, t) = (f.u * (len * 0.5), f.v * (w * 0.5));
    inlay(
        b,
        &[c - u - t, c + u - t, c + u + t, c - u + t],
        n,
        pattern::EMBER_SLOT,
    );
}

/// A blade of two swept plates from `foot` by `knee` to `tip`, faced toward `n`, `half`
/// wide at the foot; a red slot down the inner face of its upper plate.
fn blade(b: &mut MeshBuilder, foot: Vec3, knee: Vec3, tip: Vec3, n: Vec3, half: f32) {
    let f1 = Frame::new(foot, knee - foot, n);
    let f2 = Frame::new(knee, tip - knee, n);
    let (l1, l2) = ((knee - foot).length(), (tip - knee).length());
    dark_plate(b);
    armour(b, &f1, &swept(l1 + 0.35, half, 0.0, 0.85), 0.3);
    armour(b, &f2, &swept(l2, half * 0.8, 0.4, 0.55), 0.26);
    if b.fine() {
        slot(b, &f2, l2 * 0.3, 0.0, -f2.n, l2 * 0.4, 0.14);
    }
}

/// The crown's emitter and its plates' tips.
pub(super) const CROWN: (f32, f32) = (5.0, 6.6);

/// The crown: four swept plates rising off the plinth's corners and curling in over the
/// emitter like a split bore turned to the sky, the field's jet gathered between their
/// tips; a red slot down the inside of each.
pub(super) fn crown(b: &mut MeshBuilder) {
    let (crown, top) = CROWN;
    emitter(b, crown);
    for k in 0..4 {
        let d = out(FRAC_PI_4 + k as f32 * TAU / 4.0);
        blade(
            b,
            d * 1.45 + Vec3::Z * (PLINTH - 0.1),
            d * 1.75 + Vec3::Z * 4.5,
            d * 0.8 + Vec3::Z * top,
            d,
            0.6,
        );
    }
}
