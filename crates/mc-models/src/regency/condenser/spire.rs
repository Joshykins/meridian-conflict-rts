//! Design A (`regency_fabricator~a`), the spire: an armoured octagonal vessel on a plated
//! footing, held by four buttresses off the diagonals, field collars round its waist and
//! the matter glowing through slots down its faces; a plated point over its hatch.
//! - Tech 2: the vessel, two collars, the buttresses, the point.
//! - Tech 3: an upper stage on the hatch under its own point, a collar round it, struts
//!   carried on from the buttresses' heads.

use std::f32::consts::FRAC_PI_4;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::*;
use super::*;

/// The footing: radius to its corners, height.
const FOOT: (f32, f32) = (10.8, 1.2);
/// The stages: (radius, foot, top of the straight body), and their collars.
const STAGES: [(f32, f32, f32); 2] = [(5.0, FOOT.1, 9.6), (3.4, 12.42, 17.4)];
const COLLARS: [&[f32]; 2] = [&[4.0, 7.6], &[15.0]];
/// A buttress's foot out on its diagonal and its head on the vessel; tech 3's strut.
const BUTTRESS: (f32, Vec2) = (8.6, Vec2::new(5.6, 10.0));
const STRUT: (Vec2, Vec2) = (Vec2::new(5.6, 10.0), Vec2::new(3.9, 16.6));

pub(in crate::regency) fn build(b: &mut MeshBuilder, tech: u8) {
    footing(b, FOOT.0, FOOT.1);
    let (r, z0, z1) = STAGES[0];
    let hatch = vessel(b, r, z0, z1);
    for &z in COLLARS[0] {
        field_collar(b, r, z);
    }
    point(b, hatch, 1.4, 16.0);
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| buttress(b, BUTTRESS.0, BUTTRESS.1, 0.8))
    });

    tech_3(b, tech, 0.35, |b| {
        let (r, z0, z1) = STAGES[1];
        let hatch = vessel(b, r, z0, z1);
        for &z in COLLARS[1] {
            field_collar(b, r, z);
        }
        point(b, hatch, 1.0, 22.0);
    });
    tech_3(b, tech, 0.7, |b| {
        if b.coarse() {
            return;
        }
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                let (a, c) = STRUT;
                strut(b, v3(a.x, 0.0, a.y), v3(c.x, 0.0, c.y), 0.6);
            })
        });
    });
}

/// A graphite neck on a hatch at `z`, `r` across, and a plated point on it up to `top`,
/// lit red slots at its foot.
fn point(b: &mut MeshBuilder, z: f32, r: f32, top: f32) {
    let neck = z + (top - z) * 0.3;
    if !b.coarse() {
        metal(b);
        b.prism(v3(0.0, 0.0, z), b.sides(8), r, r * 0.85, neck - z);
    }
    dark_plate(b);
    // Far off, three sides: the coarse level's budget.
    let sides = if b.coarse() { 3 } else { 4 };
    b.prism(v3(0.0, 0.0, neck), sides, r * 1.5, r * 0.1, top - neck);
    if b.fine() {
        // A slot low on each face: the faces of a four-sided prism look along the axes.
        let (rb, rt, h) = (
            r * 1.5 * std::f32::consts::FRAC_1_SQRT_2,
            r * 0.1 * std::f32::consts::FRAC_1_SQRT_2,
            top - neck,
        );
        let t = 0.14;
        for k in 0..4 {
            let a = std::f32::consts::FRAC_PI_2 * k as f32;
            let d = v3(a.cos(), a.sin(), 0.0);
            let at = d * (rb + (rt - rb) * t) + Vec3::Z * (neck + h * t);
            let out = d * h + Vec3::Z * (rb - rt);
            red_slot(b, at, out, v3(-d.y, d.x, 0.0), rb * 0.9, 0.22);
        }
    }
}
