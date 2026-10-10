//! The ground the engine stands on: a stepped terrace, the low ring wall round the site,
//! the well under the held energy with the owner's colour round its rim, and the prongs
//! that anchor the energy from below.

use std::f32::consts::TAU;

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::TEAM;

use super::super::kit::{dark_plate, metal, nanite_plate, seam, v3};
use super::super::machine::{armour, collar, hoop, swept, Frame};
use super::{footing, jitter, RING_IN, RING_OUT, RING_TOP};

/// The terrace: two steps of 24 sides, outer radius and top of each.
pub(super) const TERRACE: [(f32, f32); 2] = [(114.0, 1.4), (102.0, 2.8)];

/// The terrace the shards stand on.
pub(super) fn terrace(b: &mut MeshBuilder) {
    let sides = if b.coarse() { 8 } else { 24 };
    let mut z = 0.0;
    for (n, &(r, top)) in TERRACE.iter().enumerate() {
        if n == 0 {
            dark_plate(b);
        } else {
            seam(b);
        }
        b.prism(Vec3::Z * z, sides, r, r - 2.0, top - z);
        z = top;
    }
}

/// The low ring wall round the site: plated segments leaning in, a swept plate up the
/// outer face of each whose point stands over the top, gaps at `gates` (bearings).
pub(super) fn ring_wall(b: &mut MeshBuilder, gates: &[f32]) {
    let count = 36;
    let step = TAU / count as f32;
    for k in 0..count {
        let a0 = k as f32 * step + 0.004 * TAU;
        let a1 = (k + 1) as f32 * step - 0.004 * TAU;
        let mid = (a0 + a1) * 0.5;
        if gates.iter().any(|g| angle_gap(mid, *g) < step * 0.9) {
            continue;
        }
        let section = |a: f32| -> Vec<Vec3> {
            let d = v3(a.cos(), a.sin(), 0.0);
            vec![
                d * RING_OUT,
                d * (RING_OUT - 1.4) + Vec3::Z * RING_TOP,
                d * (RING_IN + 0.6) + Vec3::Z * (RING_TOP + 0.6),
                d * RING_IN,
            ]
        };
        dark_plate(b);
        b.with_facets(|b| b.loft(&[section(a0), section(a1)], false, true));
        if b.coarse() {
            continue;
        }
        let d = v3(mid.cos(), mid.sin(), 0.0);
        let side = v3(-mid.sin(), mid.cos(), 0.0);
        // The plate up its face: its point a spike over the wall.
        let n = (d + Vec3::Z * 0.14).normalize();
        let f = Frame::new(d * (RING_OUT + 0.15) + Vec3::Z * 1.0, Vec3::Z - d * 0.14, n);
        let half = RING_OUT * step * 0.34;
        let long = if k % 3 == 0 { 1.5 } else { 1.0 };
        armour(
            b,
            &f,
            &swept(RING_TOP * long, half, 0.4 * jitter(k as u32), 0.55),
            0.7,
        );
        // A graphite rib along its inner foot.
        if b.fine() && k % 2 == 0 {
            metal(b);
            b.cylinder_between(
                d * (RING_IN - 0.6) + side * (half * 1.1) + Vec3::Z * 1.0,
                d * (RING_IN - 0.6) - side * (half * 1.1) + Vec3::Z * 1.0,
                0.7,
                0.7,
                6,
            );
        }
    }
}

fn angle_gap(a: f32, c: f32) -> f32 {
    let d = (a - c).rem_euclid(TAU);
    d.min(TAU - d)
}

/// The well: an open floor of `r` under the energy, its rim wall `rim` high with the
/// owner's colour round its top, stepping down inside to the floor in `steps`.
pub(super) fn well(b: &mut MeshBuilder, r: f32, rim: f32, steps: usize) {
    let segs = if b.fine() { 32 } else { 16 };
    dark_plate(b);
    b.with_facets(|b| {
        hoop(b, Vec3::Z * (rim * 0.5), r + 4.0, 8.0, rim, segs);
        if b.coarse() {
            return;
        }
        // Steps down inside the rim, each lower and further in.
        let tread = r * 0.5 / steps as f32;
        for i in 0..steps {
            let h = rim * (1.0 - (i + 1) as f32 / (steps + 1) as f32);
            if i % 2 == 0 {
                seam(b);
            } else {
                dark_plate(b);
            }
            hoop(
                b,
                Vec3::Z * (h * 0.5),
                r - tread * (i as f32 + 0.5),
                tread + 0.4,
                h,
                segs,
            );
        }
    });
    b.paint(TEAM);
    hoop(b, Vec3::Z * (rim + 0.1), r + 4.0, 5.0, 0.2, segs);
    if b.coarse() {
        return;
    }
    // The floor: graphite, glazed plates round it the energy shows in.
    metal(b);
    b.prism(Vec3::ZERO, segs, r * 0.5 + 0.5, r * 0.5, 0.6);
    nanite_plate(b);
    let n = 12;
    for k in 0..n {
        let a = TAU * (k as f32 + 0.5) / n as f32;
        let d = v3(a.cos(), a.sin(), 0.0);
        let f = Frame::new(d * (r * 0.07) + Vec3::Z * 0.6, d, Vec3::Z);
        armour(b, &f, &swept(r * 0.4, r * 0.045, 0.0, 0.7), 0.5);
    }
}

/// `count` prongs standing round the well at `r`, their tips leaning in to hold the
/// energy at `core` from below: plated, a graphite knuckle at each bend, glazed down their
/// inner faces.
pub(super) fn prongs(
    b: &mut MeshBuilder,
    count: usize,
    r: f32,
    reach: f32,
    core: Vec3,
    phase: f32,
) {
    for k in 0..count {
        let a = TAU * k as f32 / count as f32 + phase;
        let d = v3(a.cos(), a.sin(), 0.0);
        let side = v3(-a.sin(), a.cos(), 0.0);
        let foot = d * r;
        let knee = d * (r * 0.92) + Vec3::Z * (reach * 0.55);
        let tip = knee + (core - knee).normalize() * (reach * 0.55);
        let w = r * 0.13;
        let ring = |c: Vec3, w: f32, toward: Vec3| -> Vec<Vec3> {
            let n = (c - toward).with_z(0.0).normalize_or(d);
            vec![
                c + n * (w * 0.7),
                c + side * w,
                c - n * (w * 0.5),
                c - side * w,
            ]
        };
        dark_plate(b);
        b.with_facets(|b| {
            b.loft(
                &[
                    ring(foot, w * 1.3, core),
                    ring(knee, w, core),
                    ring(tip.lerp(knee, 0.04), w * 0.4, core),
                    vec![tip; 4],
                ],
                true,
                false,
            );
        });
        if b.coarse() {
            continue;
        }
        collar(b, knee, side, w * 0.55, w * 2.4);
        if b.fine() {
            nanite_plate(b);
            let u = (tip - knee).normalize();
            let n = (core - knee).with_z(0.0).normalize_or(-d);
            let f = Frame::new(knee + n * (w * 0.3) - u * (w * 1.2), u, n);
            armour(
                b,
                &f,
                &swept((tip - knee).length() * 0.8, w * 0.32, 0.0, 0.7),
                0.4,
            );
            // A plated foot block.
            dark_plate(b);
            footing(
                b,
                foot - d * (w * 1.6),
                foot + d * (w * 1.6),
                w * 1.5,
                (4.0, 2.0),
            );
        }
    }
}
