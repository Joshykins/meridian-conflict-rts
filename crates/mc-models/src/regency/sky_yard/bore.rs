//! Design C, the bore: a low wall rings a bore in the apron; eight blade-shards stand
//! inside it, flaring outward as they rise like a crown opened to the sky; out of the
//! bore rises a mast of rotor drums, each smaller and higher, neighbours turning
//! opposite ways, carrying the emitter's crown at its top.

use std::f32::consts::TAU;

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::gravitic_engine::rotors;
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, collar, swept, Frame};
use super::super::shard::{jitter, Shard};
use super::ground::{apron, dir, emitter_crown, intakes, spokes};
use super::EMITTER;

/// The ring wall: inner and outer radius, and its top.
const WALL: (f32, f32, f32) = (98.0, 106.0, 18.0);

fn shards() -> Vec<Shard> {
    (0..8)
        .map(|k| {
            let a = 22.5 + 45.0 * k as f32;
            let skew = if k % 2 == 0 { 0.5 } else { -0.5 };
            Shard {
                root: dir(a) * 84.0 + Vec3::Z,
                tip: dir(a + 6.0 * skew) * 126.0 + Vec3::Z * (196.0 + 14.0 * jitter(k)),
                back: dir(a),
                half: 15.0,
                depth: 8.0,
                skew,
                plates: 4,
                seed: 20 + k,
                scale: 1.0,
            }
        })
        .collect()
}

/// The mast's plated core: its radius at `z`.
fn mast_r(z: f32) -> f32 {
    12.0 - 5.0 * (z / EMITTER.z).clamp(0.0, 1.0)
}

pub(in crate::regency) fn bore(b: &mut MeshBuilder, _tech: u8) {
    apron(b);
    let shards = shards();
    if b.coarse() {
        for s in shards.iter().step_by(2) {
            s.coarse(b);
        }
        dark_plate(b);
        b.prism(Vec3::Z * 2.0, 4, 16.0, 6.0, EMITTER.z - 2.0);
        return;
    }
    wall(b);
    // The bore's floor: graphite, a seam-dark rim round it.
    seam(b);
    b.with_facets(|b| b.prism(Vec3::Z * 3.2, 24, 44.0, 42.0, 0.6));
    metal(b);
    b.prism(Vec3::Z * 3.2, 16, 30.0, 30.0, 0.9);
    intakes(b, 46.0, 14.0);
    for s in &shards {
        s.build(b);
    }
    // The mast: its core, collared where each drum turns on it.
    let sides = b.sides(8);
    dark_plate(b);
    b.with_facets(|b| {
        b.cylinder_between(
            Vec3::Z * 3.0,
            Vec3::Z * 272.0,
            mast_r(0.0),
            mast_r(272.0),
            sides,
        );
    });
    let drums = rotors::stack(5, 40.0, 250.0, 0.18);
    rotors::build(b, &drums);
    for r in &drums {
        let z = (r.z0 + r.z1) * 0.5;
        b.with_orbit(Vec3::Z, r.rate, |b| spokes(b, r, mast_r(z)));
        collar(
            b,
            Vec3::Z * z,
            Vec3::Z,
            mast_r(z) + 2.5,
            ((r.z1 - r.z0) * 0.5).min(12.0),
        );
    }
    metal(b);
    b.cylinder_between(Vec3::Z * 268.0, EMITTER - Vec3::Z * 4.0, 7.0, 6.0, sides);
    emitter_crown(b, 8, 16.0);
}

/// The low wall round the bore: plated segments leaning in, a swept plate up the outer
/// face of each, its point a spike over the top; an optic in every fourth.
fn wall(b: &mut MeshBuilder) {
    let (inner, outer, top) = WALL;
    let count = if b.fine() { 32 } else { 16 };
    let step = TAU / count as f32;
    for k in 0..count {
        let a0 = k as f32 * step + 0.006 * TAU;
        let a1 = (k + 1) as f32 * step - 0.006 * TAU;
        let mid = (a0 + a1) * 0.5;
        // Gaps where the intakes' conduits and the helpers' streams come through.
        let gap = (0..4).any(|g| {
            let d = (mid - g as f32 * TAU / 4.0).rem_euclid(TAU);
            d.min(TAU - d) < step * 0.8
        });
        if gap {
            continue;
        }
        let section = |a: f32| -> Vec<Vec3> {
            let d = v3(a.cos(), a.sin(), 0.0);
            vec![
                d * outer,
                d * (outer - 1.6) + Vec3::Z * top,
                d * (inner + 0.6) + Vec3::Z * (top + 0.6),
                d * inner,
            ]
        };
        dark_plate(b);
        b.with_facets(|b| b.loft(&[section(a0), section(a1)], false, true));
        if !b.fine() {
            continue;
        }
        let d = v3(mid.cos(), mid.sin(), 0.0);
        let n = (d + Vec3::Z * 0.14).normalize();
        let f = Frame::new(d * (outer + 0.15) + Vec3::Z, Vec3::Z - d * 0.14, n);
        let half = outer * step * 0.34;
        if k % 4 == 1 {
            b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
        } else {
            dark_plate(b);
        }
        let long = if k % 3 == 0 { 1.5 } else { 1.0 };
        armour(
            b,
            &f,
            &swept(top * long, half, 0.4 * jitter(k as u32), 0.55),
            0.8,
        );
    }
}
