//! Design C (`regency_fabricator~c`), the pyramid: a stepped octagonal pyramid of dark
//! plate, a swept plate laid up every face of every tier with its spike pointing up, the
//! matter glowing in the recessed gap under each tier; red slots high on the middle tier;
//! a graphite crown.
//! - Tech 2: three tiers and the crown.
//! - Tech 3: a fourth tier on the crown under a taller crown.

use std::f32::consts::{FRAC_PI_4, PI, TAU};

use glam::Vec3;

use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::*;

/// A tier: foot, top, radius (to its corners) at foot and top. The glowing gap under it
/// runs from the tier below's top up to its foot.
type Tier = (f32, f32, f32, f32);
const TIERS: [Tier; 3] = [
    (0.0, 4.2, 11.2, 8.8),
    (4.9, 9.2, 8.2, 6.0),
    (9.8, 13.4, 5.6, 3.4),
];
const HIGH_TIER: Tier = (14.0, 18.0, 3.6, 2.2);
/// A crown on a tier's top: its graphite drum's top, the cap's top, its radius.
const CROWNS: [(f32, f32, f32); 2] = [(15.0, 16.0, 2.4), (20.8, 22.0, 1.6)];

pub(in crate::regency) fn build(b: &mut MeshBuilder, tech: u8) {
    let mut below = None;
    for (k, t) in TIERS.iter().enumerate() {
        tier_of(b, *t, below, k == 1);
        below = Some(*t);
    }
    crown(b, TIERS[2], CROWNS[0]);

    tech_3(b, tech, 0.4, |b| {
        tier_of(b, HIGH_TIER, Some(TIERS[2]), false);
        crown(b, HIGH_TIER, CROWNS[1]);
    });
}

/// A tier: the gap of hot matter under it (over `below`), the plated frustum, a swept
/// plate up each face; red slots high on alternate faces if `slots`.
fn tier_of(b: &mut MeshBuilder, (z0, z1, r0, r1): Tier, below: Option<Tier>, slots: bool) {
    let coarse = b.coarse();
    let sides = if coarse { 4 } else { 8 };
    if coarse {
        dark_plate(b);
        b.prism(v3(0.0, 0.0, z0), sides, r0 * 1.25, r1 * 1.25, z1 - z0);
        team_tab(b, v3(0.0, 0.0, z1), r1 * 0.5);
        return;
    }
    let plan = ngon(sides, 1.0);
    if let Some((_, top, _, rt)) = below {
        b.paint(GLOW_ORANGE);
        b.loft_z(
            &plan,
            &[Section::new(top, rt * 0.86), Section::new(z0, r0 * 0.86)],
        );
    }
    seam(b);
    b.loft_z(
        &plan,
        &[Section::new(z0, r0 * 0.97), Section::new(z0 + 0.35, r0)],
    );
    dark_plate(b);
    b.with_facets(|b| b.loft_z(&plan, &[Section::new(z0 + 0.35, r0), Section::new(z1, r1)]));
    // The faces look along k * 45 degrees (the corners sit half a side round).
    let (f0, f1) = (r0 * (PI / 8.0).cos(), r1 * (PI / 8.0).cos());
    let rise = z1 - z0 - 0.35;
    let slope = (f1 - f0).hypot(rise);
    for k in 0..sides {
        let a = TAU * k as f32 / sides as f32;
        let d = v3(a.cos(), a.sin(), 0.0);
        let foot = d * f0 + Vec3::Z * (z0 + 0.35);
        let up = d * (f1 - f0) + Vec3::Z * rise;
        let out = d * rise + Vec3::Z * (f0 - f1);
        let f = Frame::new(foot + up * 0.12, up, out);
        let side = 2.0 * r0 * (PI / 8.0).sin();
        dark_plate(b);
        armour(
            b,
            &f,
            &swept(slope * 0.98, side * 0.32, 0.0, 0.62),
            THICK * 0.8,
        );
        if slots && k % 2 == 1 && b.fine() {
            let n = out.normalize();
            let along = v3(-d.y, d.x, 0.0);
            for s in [-1.0f32, 1.0] {
                red_slot(
                    b,
                    foot + up * 0.66 + n * 0.02 + along * (s * side * 0.36),
                    n,
                    up,
                    slope * 0.3,
                    0.2,
                );
            }
        }
    }
}

/// The crown on a tier's top: a graphite drum up to `drum`, a plated cap on it to `top`,
/// a seam between, the owner's colour on the cap.
fn crown(b: &mut MeshBuilder, (_, z, _, _): Tier, (drum, top, r): (f32, f32, f32)) {
    if b.coarse() {
        return;
    }
    metal(b);
    b.prism(v3(0.0, 0.0, z), b.sides(8), r, r * 0.85, drum - z);
    if b.fine() {
        seam(b);
        hoop(b, Vec3::Z * (z + (drum - z) * 0.5), r * 0.94, 0.4, 0.4, 8);
    }
    dark_plate(b);
    b.prism(v3(0.0, 0.0, drum), 8, r * 1.1, r * 0.7, top - drum);
    b.paint(TEAM);
    let t = r * 0.45;
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.face(&[
            v3(t, 0.0, top + 0.02),
            v3(0.0, t, top + 0.02),
            v3(-t, 0.0, top + 0.02),
            v3(0.0, -t, top + 0.02),
        ])
    });
}
