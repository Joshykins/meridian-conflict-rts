//! The engine's machinery, turning about its z axis under and round the held energy:
//! a stack of rotor drums rising out of the well, each smaller and higher than the one
//! below, neighbours turning opposite ways, and a great band round the inside of the
//! shards, seen through the gaps between them. Every one is a ring of plate about the
//! vertical axis, never a cage round the energy: no gimbals, no teeth, no rams.
//!
//! Rig: each ring is an orbit piece (`MeshBuilder::with_orbit`, `part::ORBIT`): it turns
//! about the z axis through the spinner pivot (the model's origin) at its own rate while
//! the structure stands powered and built (entity.wgsl, `gpu_consts::orbit`).

use std::f32::consts::TAU;

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::kit::{dark_plate, metal, nanite_plate, seam, v3};
use super::super::machine::{armour, hoop, swept, Frame};

/// One ring: its foot and top, outer and inner radius, the plated vanes standing on it,
/// and how fast it turns (rad/s; the sign is the way).
#[derive(Clone, Copy, Debug)]
pub(super) struct Rotor {
    pub(super) z0: f32,
    pub(super) z1: f32,
    pub(super) outer: f32,
    pub(super) inner: f32,
    pub(super) vanes: usize,
    pub(super) rate: f32,
    /// Its drum's faces are rimmed in violet (`pattern::NANITE_RIM`), so it reads turning
    /// through the gaps; it is drawn in fewer, broader panels.
    pub(super) lit: bool,
}

/// A stack of `count` rotors rising out of a well of radius `r` to `top`, each a step
/// smaller and higher, the lowest turning at `rate`, every other one the other way and a
/// little quicker.
pub(super) fn stack(count: usize, r: f32, top: f32, rate: f32) -> Vec<Rotor> {
    let rise = top / count as f32;
    (0..count)
        .map(|k| {
            let s = 1.0 - 0.6 * k as f32 / count as f32;
            Rotor {
                z0: 0.6 + rise * k as f32,
                z1: rise * (k as f32 + 0.78),
                outer: r * s,
                inner: r * s - (r * 0.26).max(5.0),
                vanes: 8 + 4 * (count - k),
                rate: rate * (1.0 + 0.45 * k as f32) * if k % 2 == 0 { 1.0 } else { -1.0 },
                lit: k % 2 == 1,
            }
        })
        .collect()
}

/// Builds the rotors, and sets the axis they turn about.
pub(super) fn build(b: &mut MeshBuilder, rotors: &[Rotor]) {
    b.set_spinner_pivot(Vec3::ZERO);
    for (k, r) in rotors.iter().enumerate() {
        b.with_orbit(Vec3::Z, r.rate, |b| ring(b, r, k));
    }
}

fn ring(b: &mut MeshBuilder, r: &Rotor, k: usize) {
    let segs = match (r.lit, b.fine()) {
        (true, _) => 12,
        (false, true) => 36,
        (false, false) => 18,
    };
    let (mid, w, h) = ((r.outer + r.inner) * 0.5, r.outer - r.inner, r.z1 - r.z0);
    b.with_facets(|b| {
        if r.lit {
            b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
        } else {
            dark_plate(b);
        }
        hoop(b, Vec3::Z * (r.z0 + h * 0.5), mid, w, h, segs);
        if b.coarse() {
            return;
        }
        // A seam band round its outer face, and its graphite race inside.
        seam(b);
        hoop(
            b,
            Vec3::Z * (r.z0 + h * 0.3),
            r.outer + 0.3,
            1.2,
            h * 0.16,
            segs,
        );
        metal(b);
        hoop(
            b,
            Vec3::Z * (r.z0 + h * 0.5),
            r.inner - 0.6,
            1.6,
            h * 0.8,
            segs,
        );
    });
    if b.coarse() {
        return;
    }
    // Vanes: plates standing on its top, raked the way it turns, their points its spikes.
    let vanes = if b.fine() { r.vanes } else { r.vanes / 2 };
    let way = r.rate.signum();
    for i in 0..vanes {
        let a = TAU * (i as f32 + 0.5 * (k % 2) as f32) / vanes as f32;
        let d = v3(a.cos(), a.sin(), 0.0);
        let along = v3(-a.sin(), a.cos(), 0.0) * way;
        if i % 2 == 0 {
            nanite_plate(b);
        } else {
            dark_plate(b);
        }
        let f = Frame::new(
            d * (mid - w * 0.05) + Vec3::Z * r.z1 - along * (w * 0.5),
            along + Vec3::Z * 0.55,
            -d,
        );
        let len = (TAU * mid / vanes as f32 * 1.1).min(h * 2.4);
        b.with_facets(|b| armour(b, &f, &swept(len, w * 0.34, 0.6, 0.45), (w * 0.12).min(1.4)));
    }
}
