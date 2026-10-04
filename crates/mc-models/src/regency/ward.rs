//! The Ward, the Regency shield generator, on its 3 x 3 lot: a small caged star that throws
//! the field. Tech 2, upgrading in place to Ward II. The star is light, as a power
//! generator's is (`Model::star_core`, renderer `star_core_fx.rs`), held at the height
//! an Aegis's crystal sits at (`mc_data::SHIELD_PROJECTOR_HEIGHT`). The white-hot jet that
//! climbs to the veil's crown (shields.wgsl `column_shade`) is born lower, in the emitter
//! cone on the pinch block (the unit file's `shield_projector`), and runs up through it.
//!
//! The fork: on a plated octagonal plinth, two tall plated prongs stand either side of the
//! star, each a course of two swept plates braced off the plinth by a plated strut, a
//! gravity lens lit in the prism on its inner face aimed at the star, a red line up it.
//! Two bronze gravity rings tumble round the star. Ward II stands a second, taller pair
//! of prongs across the first and adds a third ring.

use std::f32::consts::{FRAC_PI_4, TAU};

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::*;

/// The star, at an Aegis crystal's height (`mc_data::SHIELD_PROJECTOR_HEIGHT`).
const CORE: Vec3 = Vec3::new(0.0, 0.0, 16.0);
/// The star's face, and Ward II's.
/// (Both small, so a breached Ward's nova keeps time with `regency_supernova_small`.)
const CORE_R: [f32; 2] = [1.5, 1.6];
/// The plinth: its foot's radius and its top.
const PLINTH_R: f32 = 12.6;
const PLINTH_TOP: f32 = 3.0;

/// The direction at bearing `a` (radians) round z.
fn out(a: f32) -> Vec3 {
    v3(a.cos(), a.sin(), 0.0)
}

/// The star at `at` (recorded for the renderer to draw as light), the turning pivot, and
/// a small white-hot heart in the mesh for portraits and far off.
fn core(b: &mut MeshBuilder, tech: u8) {
    let r = CORE_R[usize::from(tech >= 3)];
    b.set_spinner_pivot(CORE);
    b.set_star_core(CORE, r);
    b.paint(GLOW_PRISM);
    let h = r * 0.22;
    if b.coarse() {
        b.cylinder_between(CORE - Vec3::Z * h, CORE + Vec3::Z * h, h, h * 0.6, 3);
        return;
    }
    let sides = b.sides(10);
    b.spheroid(CORE, Vec3::splat(h), sides, if b.fine() { 6 } else { 4 });
}

/// A gravity ring round the star: a bronze band of radius `r` square to `normal`,
/// tumbling about `spin` at `rate` rad/s. Close up only.
fn ring(b: &mut MeshBuilder, normal: Vec3, spin: Vec3, rate: f32, r: f32) {
    if !b.fine() {
        return;
    }
    b.with_orbit(spin, rate, |b| {
        metal(b);
        hoop_on(b, CORE, normal, r, 0.24, 0.18, 28);
    });
}

/// The rings round the star at `tech`: two, and a third at tech 3.
fn rings(b: &mut MeshBuilder, tech: u8) {
    ring(b, v3(0.0, 0.6, 1.0), Vec3::X, 0.8, 2.5);
    ring(b, v3(0.6, 0.0, 1.0), v3(0.0, 1.0, 0.4), -1.2, 3.1);
    tier(b, tech, 3, 0.6, |b| {
        ring(b, v3(-0.5, 0.5, 0.6), v3(1.0, 1.0, 0.2), 1.5, 3.7)
    });
}

/// The plinth: a plated octagonal drum stepping in to its top, a lit seam round it, the
/// owner's colour on four tabs at its shoulders.
fn plinth(b: &mut MeshBuilder) {
    dark_plate(b);
    if b.coarse() {
        // Far off: one drum to the top.
        b.prism(Vec3::ZERO, 6, PLINTH_R, PLINTH_R - 3.6, PLINTH_TOP);
        return;
    }
    b.prism(Vec3::ZERO, 8, PLINTH_R, PLINTH_R - 1.0, 1.3);
    seam(b);
    b.prism(v3(0.0, 0.0, 1.3), 8, PLINTH_R - 1.6, PLINTH_R - 1.6, 0.35);
    dark_plate(b);
    b.prism(
        v3(0.0, 0.0, 1.65),
        8,
        PLINTH_R - 2.0,
        PLINTH_R - 3.6,
        PLINTH_TOP - 1.65,
    );
    for k in 0..4 {
        let d = out(FRAC_PI_4 + k as f32 * TAU / 4.0);
        tab(b, d * (PLINTH_R - 1.3) + v3(0.0, 0.0, 1.32), d, 0.7);
    }
}

/// The owner's colour on a flat top: a square `half` across at `at`.
fn tab(b: &mut MeshBuilder, at: Vec3, along: Vec3, half: f32) {
    let along = along.normalize() * half;
    let across = Vec3::Z.cross(along);
    b.paint(TEAM);
    b.face(&[
        at - along - across,
        at + along - across,
        at + along + across,
        at - along + across,
    ]);
}

pub(super) fn ward(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(2, 3);
    plinth(b);
    core(b, tech);
    if b.coarse() {
        coarse_fork(b, tech);
        return;
    }
    // The pinch block the prongs stand on, and the emitter cone the jet leaves from.
    dark_plate(b);
    b.prism(v3(0.0, 0.0, PLINTH_TOP), 8, 5.6, 4.4, 3.4);
    metal(b);
    b.cylinder_between(v3(0.0, 0.0, 6.4), v3(0.0, 0.0, 9.6), 1.6, 0.8, 10);
    prongs(b, 0.0, 26.0);
    rings(b, tech);
    tier(b, tech, 3, 0.4, |b| prongs(b, TAU / 4.0, 31.6));
}

/// A pair of prongs across bearing `a`, up to `top`.
fn prongs(b: &mut MeshBuilder, a: f32, top: f32) {
    for side in [1.0, -1.0] {
        let d = out(a) * side;
        let foot = d * 8.6 + Vec3::Z * 1.4;
        let knee = d * 5.6 + Vec3::Z * 12.0;
        let tip = d * 4.2 + Vec3::Z * top;
        let f1 = Frame::new(foot, knee - foot, d);
        let f2 = Frame::new(knee, tip - knee, d);
        dark_plate(b);
        armour(
            b,
            &f1,
            &swept((knee - foot).length() + 1.2, 1.7, 0.0, 0.85),
            0.6,
        );
        armour(b, &f2, &swept((tip - knee).length(), 1.4, 0.5, 0.6), 0.55);
        // A gravity lens on its inner face, aimed at the star.
        let lens = d * 3.6 + Vec3::Z * CORE.z;
        metal(b);
        b.cylinder_between(lens + d * 1.2, lens, 1.25, 1.0, b.sides(10));
        b.paint(GLOW_PRISM);
        b.cylinder_between(lens, lens - d * 0.12, 0.7, 0.6, b.sides(10));
        if b.fine() {
            strut(b, d * 11.0 + Vec3::Z * 1.3, d * 6.8 + Vec3::Z * 8.0, 0.45);
            red_slot(b, f2.at(3.0, 0.0, -0.3), -d, f2.u, 4.0, 0.14);
        }
    }
}

/// The owner's colour on the plinth far off, two tabs.
fn coarse_tabs(b: &mut MeshBuilder) {
    for k in 0..2 {
        let d = out(FRAC_PI_4 + k as f32 * TAU / 2.0);
        tab(
            b,
            d * (PLINTH_R - 2.6) + v3(0.0, 0.0, PLINTH_TOP + 0.02),
            d,
            0.9,
        );
    }
}

/// Far off: each prong a plate, the owner's colour on the plinth, the star.
fn coarse_fork(b: &mut MeshBuilder, tech: u8) {
    coarse_tabs(b);
    dark_plate(b);
    let pairs = if tech >= 3 { 2 } else { 1 };
    for k in 0..pairs {
        for side in [1.0, -1.0] {
            let d = out(k as f32 * TAU / 4.0) * side;
            let across = Vec3::Z.cross(d) * 1.4;
            let top = if k == 0 { 26.0 } else { 31.6 };
            let quad = [
                d * 8.6 - across,
                d * 8.6 + across,
                d * 4.2 + across + Vec3::Z * top,
                d * 4.2 - across + Vec3::Z * top,
            ];
            b.face(&quad);
            b.face(&[quad[3], quad[2], quad[1], quad[0]]);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ward() {
        super::super::check_at("regency_ward", 2, 13.9, 26.0, Some(3), &[]);
        super::super::check_at("regency_ward", 3, 13.9, 32.0, Some(3), &[]);
    }

    /// The star sits at an Aegis crystal's height.
    #[test]
    fn the_star_is_the_projector() {
        let model = crate::build_model_scaled("regency_ward", 13.9, 26.0, 2).unwrap();
        let star = model.star_core.expect("a star");
        assert!(
            (star[2] - mc_data::SHIELD_PROJECTOR_HEIGHT).abs() < 0.1,
            "{star:?}"
        );
    }
}
