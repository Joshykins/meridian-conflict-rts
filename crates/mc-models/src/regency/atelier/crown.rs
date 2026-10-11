//! The Atelier's crown, where the stream leaves (`EMITTER`, the top centre): stacked
//! machined collars on the shaft (`spire::tower`), a slotted point rising from them to the
//! top, and round it a halo of straight plated blades floating on field gaps, turning
//! slowly (`part::SPINNER`); from tech 2 a second, smaller halo above it. Every line in
//! it is straight.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, FRAC_PI_8, TAU};

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::{part, pattern};

use super::super::kit::{metal, nanite_plate, seam, slab, v3};
use super::super::machine::{hoop, tier};
use super::super::parts::face_out;
use super::spire::{tower, SHAFT_TOP};
use super::{facets, machined, red_slot, stage, EMITTER};

/// The collars, stacked on the shaft: (foot, head, corner radius at each).
const COLLARS: [(f32, f32, f32, f32); 2] = [
    (SHAFT_TOP, SHAFT_TOP + 1.0, 6.4, 6.2),
    (SHAFT_TOP + 1.2, SHAFT_TOP + 2.2, 5.4, 5.2),
];
/// The top of the collars, where the crown's own pieces stand.
const SEAT: f32 = SHAFT_TOP + 2.2;

pub(in super::super) fn build(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    tower(b, tech);
    if crowned_far(b) {
        return;
    }
    collars(b);
    halo(b, tech);
}

/// Far off, every crown is a slotted point on the shaft: drawn here, and true.
fn crowned_far(b: &mut MeshBuilder) -> bool {
    if !b.coarse() {
        return false;
    }
    nanite_plate(b);
    let ring = square(6.4, SHAFT_TOP);
    b.loft(&[ring, square(0.02, EMITTER.z)], false, false);
    true
}

/// A square of corners at bearings 45 degrees and on, `r` out and `z` up.
fn square(r: f32, z: f32) -> Vec<Vec3> {
    (0..4)
        .map(|k| {
            let a = FRAC_PI_4 + FRAC_PI_2 * k as f32;
            v3(a.cos() * r, a.sin() * r, z)
        })
        .collect()
}

/// An octagon of corners `r` out and `z` up, flat faces on the axes.
fn oct_ring(r: f32, z: f32) -> Vec<Vec3> {
    (0..8)
        .map(|k| {
            let a = FRAC_PI_8 + FRAC_PI_4 * k as f32;
            v3(a.cos() * r, a.sin() * r, z)
        })
        .collect()
}

/// The collars on the shaft's head: a broad machined collar overhanging the shaft, red
/// slots round it, a lit seam, and a narrower collar over it, a violet gasket in each
/// face and a graphite race round its foot.
fn collars(b: &mut MeshBuilder) {
    for (n, &(z0, z1, r0, r1)) in COLLARS.iter().enumerate() {
        machined(b);
        stage(b, 8, FRAC_PI_8, z0, z1, r0, r1);
        for f in facets(8, FRAC_PI_8, z0, z1, r0, r1) {
            if n == 0 {
                red_slot(b, &f, [0.35, 0.65], [-0.6, 0.6]);
            } else if b.mid() {
                let q = f.patch([0.2, 0.8], [-0.7, 0.7], 0.03);
                b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
                face_out(b, &q, f.out());
            }
        }
    }
    seam(b);
    let (_, z, _, r) = COLLARS[0];
    stage(b, 8, FRAC_PI_8, z, COLLARS[1].0, r - 0.4, r - 0.5);
    if b.fine() {
        metal(b);
        hoop(
            b,
            Vec3::Z * (COLLARS[1].0 + 0.1),
            COLLARS[1].2 + 0.1,
            0.3,
            0.2,
            16,
        );
    }
}

/// The crown: a slotted point on a machined foot, and round it a halo of
/// straight plated blades floating on field gaps, turning slowly (`part::SPINNER`); from
/// tech 2 a second, smaller halo above it.
fn halo(b: &mut MeshBuilder, tech: u8) {
    machined(b);
    b.with_facets(|b| {
        b.loft(
            &[oct_ring(3.2, SEAT), oct_ring(3.2, SEAT + 1.0)],
            false,
            false,
        )
    });
    nanite_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[oct_ring(3.2, SEAT + 1.0), oct_ring(0.02, EMITTER.z)],
            false,
            false,
        )
    });
    blade_ring(b, SEAT + 1.5, 6.4, 8, 1.0);
    tier(b, tech, 2, 0.6, |b| blade_ring(b, SEAT + 3.6, 4.6, 8, 0.8));
}

/// A halo of `count` straight blades `r` out at `z`, each `h` deep, a field gap between
/// each and the next, turning slowly: machined outside, a violet gasket on its inner face.
fn blade_ring(b: &mut MeshBuilder, z: f32, r: f32, count: usize, h: f32) {
    b.with_part(part::SPINNER, |b| {
        let chord = 2.0 * r * (std::f32::consts::PI / count as f32).sin();
        for k in 0..count {
            let a = TAU * k as f32 / count as f32;
            let (out, along) = (v3(a.cos(), a.sin(), 0.0), v3(-a.sin(), a.cos(), 0.0));
            let c = out * (r * (std::f32::consts::PI / count as f32).cos()) + Vec3::Z * z;
            let half = along * (chord * 0.39);
            let quad = [
                c - half - Vec3::Z * (h * 0.5),
                c + half - Vec3::Z * (h * 0.5),
                c + half + Vec3::Z * (h * 0.5),
                c - half + Vec3::Z * (h * 0.5),
            ];
            machined(b);
            b.with_facets(|b| slab(b, quad, out * 0.35));
            if b.mid() {
                b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
                let inner: Vec<Vec3> = quad.iter().map(|&p| p - out * 0.03).collect();
                face_out(b, &inner, -out);
            }
        }
    });
}
