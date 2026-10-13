//! What every design of the yard shares: the plated apron it stands on with the owner's
//! colour round it, the four intakes helpers pour their work into, the crown of plates
//! the great beam leaves from, and the plated limbs its arms and buttresses are made of.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::{PLATING_DARK, TEAM};
use crate::pattern;

use super::super::gravitic_engine::rotors::Rotor;
use super::super::gravitic_engine::works::conduit;
use super::super::kit::{dark_plate, metal, nanite_plate, seam, v3};
use super::super::machine::{armour, collar, hoop, hoop_on, swept, Frame};
use super::{EMITTER, INTAKE_R, INTAKE_Z};

/// A level unit vector `deg` degrees round from +x.
pub(super) fn dir(deg: f32) -> Vec3 {
    let a = deg.to_radians();
    v3(a.cos(), a.sin(), 0.0)
}

/// The apron: two plated steps, a course of swept plates round the outer one, and the
/// owner's colour round the inner. Far off, one low step and the colour.
pub(super) fn apron(b: &mut MeshBuilder) {
    dark_plate(b);
    if b.coarse() {
        b.prism(Vec3::ZERO, 6, 136.0, 132.0, 2.0);
        b.paint(TEAM);
        b.decal(Vec3::Z * 2.2, Vec2::splat(60.0));
        return;
    }
    let sides = if b.fine() { 24 } else { 12 };
    b.with_facets(|b| b.prism(Vec3::ZERO, sides, 136.0, 132.0, 1.8));
    seam(b);
    b.with_facets(|b| b.prism(Vec3::Z * 1.8, sides, 124.0, 120.0, 1.6));
    b.paint(TEAM);
    hoop(b, Vec3::Z * 3.5, 112.0, 5.0, 0.3, sides);
    if !b.fine() {
        return;
    }
    // Swept plates round the outer step, their points running round it: from above the
    // apron reads swept, not squared.
    dark_plate(b);
    let n = 28;
    for k in 0..n {
        let a = 360.0 * k as f32 / n as f32;
        let d = dir(a);
        let along = dir(a + 90.0);
        let f = Frame::new(d * 128.0 + Vec3::Z * 1.85, along, Vec3::Z + d * 0.1);
        armour(b, &f, &swept(24.0, 3.2, 0.6, 0.45), 0.6);
    }
}

/// The four intakes round the foot, on the axes: plated houses sloping down outward, the
/// mouth on each (the unit file's emitters 2 to 5) a graphite collar rimmed in violet,
/// a red optic under it, plates swept back over its flanks, and conduits in to the middle.
pub(super) fn intakes(b: &mut MeshBuilder, from: f32, inner: f32) {
    if b.coarse() {
        return;
    }
    for k in 0..4 {
        let a = 90.0 * k as f32;
        let d = dir(a);
        let side = d.cross(Vec3::Z);
        let section = |r: f32, h: f32, w: f32| {
            vec![
                d * r - side * w,
                d * r + side * w,
                d * r + side * (w * 0.7) + Vec3::Z * h,
                d * r - side * (w * 0.7) + Vec3::Z * h,
            ]
        };
        // Its top passes just under the mouth.
        dark_plate(b);
        b.with_facets(|b| {
            b.loft(
                &[
                    section(from, 60.0, 15.0),
                    section(INTAKE_R, INTAKE_Z - 3.0, 14.0),
                    section(80.0, 22.0, 12.0),
                ],
                true,
                true,
            );
        });
        // The mouth: a collar facing up and out, rimmed violet, working while it builds.
        let mouth = d * INTAKE_R + Vec3::Z * INTAKE_Z;
        let axis = (d * 0.55 + Vec3::Z).normalize();
        metal(b);
        let sides = b.sides(16);
        b.cylinder_between(mouth - axis * 4.0, mouth, 8.5, 7.5, sides);
        b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
        b.with_facets(|b| hoop_on(b, mouth + axis * 0.4, axis, 7.6, 2.4, 1.2, sides));
        if !b.fine() {
            continue;
        }
        // An optic looking out under the mouth, in a plate of its own.
        b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
        let f = Frame::new(
            d * 79.0 + Vec3::Z * 6.0,
            Vec3::Z + d * 0.4,
            d + Vec3::Z * 0.3,
        );
        armour(b, &f, &swept(14.0, 6.0, 0.0, 0.7), 0.6);
        // Plates swept back up its flanks toward the middle, points standing proud.
        dark_plate(b);
        for s in [-1.0f32, 1.0] {
            for (j, r) in [74.0f32, 62.0, from + 10.0].into_iter().enumerate() {
                let h = 22.0 + (r - 80.0) / (from - 80.0) * 38.0;
                let o = d * r + side * (s * 13.5) + Vec3::Z * (h * 0.35);
                let f = Frame::new(o, -d + Vec3::Z * 0.55, side * s);
                armour(
                    b,
                    &f,
                    &swept(18.0 + 3.0 * j as f32, h * 0.32, 0.5, 0.4),
                    0.7,
                );
            }
        }
        conduit(b, d * from, d * inner, 3, 1.1);
    }
}

/// The crown the great beam leaves from: `petals` plates splayed up and out round the
/// emitter, glazed violet inside, on a graphite collar `r` round with a violet rim.
pub(super) fn emitter_crown(b: &mut MeshBuilder, petals: usize, r: f32) {
    let sides = if b.fine() { 24 } else { 12 };
    metal(b);
    hoop(b, EMITTER - Vec3::Z * 7.0, r, 3.0, 8.0, sides);
    b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
    b.with_facets(|b| hoop(b, EMITTER - Vec3::Z * 2.0, r * 0.72, 2.6, 2.4, sides));
    for k in 0..petals {
        let a = 360.0 * k as f32 / petals as f32 + 15.0;
        let d = dir(a);
        let up = Vec3::Z + d * 0.5;
        let foot = EMITTER - Vec3::Z * 26.0 + d * (r * 0.9);
        dark_plate(b);
        let f = Frame::new(foot, up, d);
        b.with_facets(|b| armour(b, &f, &swept(36.0, r * 0.42, 0.0, 0.55), 1.4));
        if b.fine() {
            // Its inner face, glazed: the violet of the work showing through.
            nanite_plate(b);
            let g = Frame::new(foot - d * 0.3 + up.normalize() * 3.0, up, -d);
            armour(b, &g, &swept(26.0, r * 0.3, 0.0, 0.6), 0.5);
        }
    }
}

/// Spokes from a turning ring in to its race on a mast of radius `hub` at the ring's
/// height, turning with it: plated, a graphite knuckle at the ring.
pub(super) fn spokes(b: &mut MeshBuilder, r: &Rotor, hub: f32) {
    let z = (r.z0 + r.z1) * 0.5;
    let n = if b.fine() { 6 } else { 3 };
    for k in 0..n {
        let d = dir(360.0 * k as f32 / n as f32 + 10.0);
        dark_plate(b);
        b.beam(
            d * (hub + 4.0) + Vec3::Z * z,
            d * (r.inner + 1.0) + Vec3::Z * z,
            Vec2::new(3.0, (r.z1 - r.z0) * 0.5),
            Vec2::new(2.2, (r.z1 - r.z0) * 0.6),
        );
        if b.fine() {
            collar(b, d * (r.inner - 1.5) + Vec3::Z * z, d, 2.2, 3.0);
        }
    }
}
