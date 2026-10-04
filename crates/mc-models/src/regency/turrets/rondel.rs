//! The Rondel (tech 2 missile defence, a 1 x 1 lot), upgrading in place to Rondel II:
//! gravity lenses that each throw a lens onto a missile and squeeze it until it folds
//! (the faction's `anti_missile_look`). Each lens is one of the unit file's
//! `anti_missile_mounts` (`data/factions/regency/units/structures.ron`): keep [`T2_LENSES`],
//! [`T3_LENSES`] and the data in step. Nothing on it turns that carries a lens, since the
//! sim fires from the mounts as laid.
//!
//! Three designs to pick from (`regency_missile_defense` and its `~` variants):
//! - the disc: a plated drum under a broad tilted rondel that turns slowly, the lenses on
//!   swept cheeks either side;
//! - `~petals`: four swept plates rising round a short core in a star, the lenses at the
//!   tips of two;
//! - `~hover`: a squat plinth and a gravity pylon, the lenses floating over it in open
//!   swept collars, held by nothing.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::sunspear::coil_light;

/// The tech 2 lenses, either side across y.
const T2_LENSES: [Vec3; 2] = [Vec3::new(0.0, 3.0, 10.0), Vec3::new(0.0, -3.0, 10.0)];
/// The pair Rondel II adds, higher and across x.
const T3_LENSES: [Vec3; 2] = [Vec3::new(3.0, 0.0, 13.0), Vec3::new(-3.0, 0.0, 13.0)];
/// A lens's face radius.
const LENS_R: f32 = 0.72;
/// The plinth: its foot's circumradius and its top.
const PLINTH_R: f32 = 5.0;
const PLINTH_TOP: f32 = 1.2;

/// Where a lens at `at` looks: out from the middle and up, so it covers the sky round it.
fn facing(at: Vec3) -> Vec3 {
    (v3(at.x, at.y, 0.0).normalize_or_zero() + Vec3::Z).normalize()
}

/// A gravity lens head at `at` looking along `toward`: a bronze housing in a dark plated
/// collar, the missile defence's red on its rim and in its face, three bronze vanes round
/// it close up. The Regency's emitter, on structures and warships alike.
pub(in crate::regency) fn crush_lens(b: &mut MeshBuilder, at: Vec3, toward: Vec3, r: f32) {
    let d = toward.normalize();
    let sides = b.sides(10);
    metal(b);
    b.cylinder_between(at - d * r * 1.1, at - d * r * 0.1, r * 0.95, r * 1.1, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(at - d * r * 0.1, at + d * 0.04, r * 0.42, r * 0.36, sides);
    if !b.fine() {
        return;
    }
    dark_plate(b);
    hoop_on(b, at - d * r * 0.3, d, r * 1.2, r * 0.4, r * 0.5, sides);
    b.paint(GLOW_LASER);
    hoop_on(b, at, d, r * 0.95, r * 0.14, r * 0.12, sides);
    let e1 = d.any_orthonormal_vector();
    let e2 = d.cross(e1);
    metal(b);
    for k in 0..3 {
        let a = TAU * k as f32 / 3.0;
        let e = e1 * a.cos() + e2 * a.sin();
        b.beam(
            at + e * r * 1.15 - d * r * 0.8,
            at + e * r * 0.9 + d * r * 0.3,
            Vec2::new(0.1, 0.24),
            Vec2::new(0.08, 0.14),
        );
    }
}

/// The lenses drawn at `tech`: the tech 2 pair, and Rondel II's waiting on it as an
/// upgrade piece (or drawn, at tech 3).
fn lenses(b: &mut MeshBuilder, tech: u8) {
    for at in T2_LENSES {
        crush_lens(b, at, facing(at), LENS_R);
    }
    tier(b, tech, 3, 0.5, |b| {
        for at in T3_LENSES {
            crush_lens(b, at, facing(at), LENS_R);
        }
    });
}

/// Far off: a red point where each lens is.
fn coarse_lenses(b: &mut MeshBuilder, tech: u8) {
    b.paint(GLOW_LASER);
    let pairs: &[[Vec3; 2]] = if tech >= 3 {
        &[T2_LENSES, T3_LENSES]
    } else {
        &[T2_LENSES]
    };
    for at in pairs.iter().flatten() {
        let (x, y) = (Vec3::X * 0.5, Vec3::Y * 0.5);
        b.face(&[*at - x - y, *at + x - y, *at + x + y, *at - x + y]);
    }
}

/// The plinth every design stands on: a plated octagon stepping in, a seam round it, the
/// owner's colour on its diagonals.
fn plinth(b: &mut MeshBuilder) {
    dark_plate(b);
    if b.coarse() {
        b.prism(Vec3::ZERO, 4, PLINTH_R, PLINTH_R - 0.8, PLINTH_TOP);
        tabs(b, 2);
        return;
    }
    b.prism(Vec3::ZERO, 8, PLINTH_R, PLINTH_R - 0.4, 0.7);
    seam(b);
    b.prism(Vec3::Z * 0.7, 8, PLINTH_R - 0.55, PLINTH_R - 0.55, 0.15);
    dark_plate(b);
    b.prism(
        Vec3::Z * 0.85,
        8,
        PLINTH_R - 0.7,
        PLINTH_R - 1.0,
        PLINTH_TOP - 0.85,
    );
    tabs(b, 4);
}

/// The owner's colour on `n` tabs round the plinth's top, on the diagonals.
fn tabs(b: &mut MeshBuilder, n: usize) {
    b.paint(TEAM);
    for k in 0..n {
        let a = FRAC_PI_4 + k as f32 * TAU / n as f32;
        let d = v3(a.cos(), a.sin(), 0.0);
        let at = d * (PLINTH_R - 1.6) + Vec3::Z * (PLINTH_TOP + 0.02);
        let along = d * 0.5;
        let across = Vec3::Z.cross(along);
        b.face(&[
            at - along - across,
            at + along - across,
            at + along + across,
            at - along + across,
        ]);
    }
}

/// A swept plate standing from `foot` up to `tip`, its face turned to `out`, `half` wide.
fn plate(b: &mut MeshBuilder, foot: Vec3, tip: Vec3, out: Vec3, half: f32, thick: f32) {
    dark_plate(b);
    armour(
        b,
        &Frame::new(foot, tip - foot, out),
        &swept(foot.distance(tip), half, 0.0, 0.7),
        thick,
    );
}

// ---- A: the disc ------------------------------------------------------------------------

/// The disc: a plated drum, a broad rondel tilted over it turning slowly, and a swept
/// cheek either side up to each lens.
pub(in crate::regency) fn disc(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(2, 3);
    plinth(b);
    dark_plate(b);
    let core = if b.coarse() { 4 } else { 8 };
    b.prism(Vec3::Z * PLINTH_TOP, core, 3.2, 2.6, 5.0);
    if b.coarse() {
        coarse_lenses(b, tech);
        return;
    }
    seam(b);
    b.prism(Vec3::Z * 6.2, 8, 2.6, 2.6, 0.25);
    // The rondel: a thick plated disc on a bronze hub, tilted, turning.
    let hub = Vec3::Z * 7.6;
    b.set_spinner_pivot(hub);
    shaft(b, Vec3::Z * 6.4, hub, 0.5);
    b.with_part(part::SPINNER, |b| {
        let n = v3(0.35, 0.0, 1.0).normalize();
        dark_plate(b);
        let sides = b.sides(16);
        b.cylinder_between(hub - n * 0.3, hub + n * 0.3, 2.3, 2.1, sides);
        if b.fine() {
            seam(b);
            hoop_on(b, hub, n, 2.3, 0.18, 0.66, sides);
        }
        b.paint(GLOW_LASER);
        hoop_on(b, hub + n * 0.32, n, 1.2, 0.12, 0.04, sides);
    });
    for at in T2_LENSES {
        let d = v3(0.0, at.y.signum(), 0.0);
        plate(
            b,
            d * 2.9 + Vec3::Z * 3.0,
            at - d * 0.6 - Vec3::Z * 0.4,
            d,
            0.9,
            0.4,
        );
        if b.fine() {
            red_slot(b, d * 3.25 + Vec3::Z * 5.0, d, Vec3::Z, 2.4, 0.12);
        }
    }
    tier(b, tech, 3, 0.4, |b| {
        for at in T3_LENSES {
            let d = v3(at.x.signum(), 0.0, 0.0);
            plate(
                b,
                d * 2.9 + Vec3::Z * 3.0,
                at - d * 0.6 - Vec3::Z * 0.4,
                d,
                0.8,
                0.4,
            );
        }
    });
    lenses(b, tech);
}

// ---- B: the petals ----------------------------------------------------------------------

/// The petals: a short core, four swept plates rising round it in a star, a lens at the
/// tips of the pair across y; Rondel II raises the pair across x into a crown with lenses.
pub(in crate::regency) fn petals(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(2, 3);
    plinth(b);
    dark_plate(b);
    let core = if b.coarse() { 4 } else { 8 };
    b.prism(Vec3::Z * PLINTH_TOP, core, 2.3, 1.7, 6.6);
    if b.coarse() {
        coarse_lenses(b, tech);
        return;
    }
    seam(b);
    b.prism(Vec3::Z * 7.8, 8, 1.7, 1.2, 0.3);
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2;
        let d = v3(a.cos(), a.sin(), 0.0);
        if k % 2 == 1 {
            // Across y: up to the lens.
            plate(
                b,
                d * 3.9 + Vec3::Z * 1.0,
                d * 3.0 + Vec3::Z * 9.4,
                d,
                1.2,
                0.45,
            );
            if b.fine() {
                strut(b, d * 1.9 + Vec3::Z * 4.0, d * 3.5 + Vec3::Z * 5.6, 0.25);
            }
        } else {
            // Across x: a lower petal, raised by Rondel II.
            plate(
                b,
                d * 3.9 + Vec3::Z * 1.0,
                d * 2.6 + Vec3::Z * 7.4,
                d,
                1.1,
                0.4,
            );
            if b.fine() {
                red_slot(
                    b,
                    d * 3.7 + Vec3::Z * 3.4,
                    d,
                    v3(-d.x * 0.2, 0.0, 1.0),
                    2.2,
                    0.12,
                );
            }
        }
    }
    tier(b, tech, 3, 0.4, |b| {
        for at in T3_LENSES {
            let d = v3(at.x.signum(), 0.0, 0.0);
            plate(
                b,
                d * 2.0 + Vec3::Z * 6.0,
                at - d * 0.4 - Vec3::Z * 0.5,
                d,
                0.9,
                0.4,
            );
        }
    });
    lenses(b, tech);
}

// ---- C: the hover -----------------------------------------------------------------------

/// The hover: a squat plinth and a gravity pylon lit in the prism, the lenses floating
/// over it each in an open collar of two swept plates, held by nothing.
pub(in crate::regency) fn hover(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(2, 3);
    plinth(b);
    dark_plate(b);
    let core = if b.coarse() { 4 } else { 8 };
    b.prism(Vec3::Z * PLINTH_TOP, core, 2.8, 2.2, 3.0);
    if b.coarse() {
        coarse_lenses(b, tech);
        return;
    }
    // The pylon: a bronze shaft up to a plated cap, the prism lens on top that holds the
    // lenses up.
    metal(b);
    b.prism(Vec3::Z * 4.2, 8, 1.0, 0.8, 2.6);
    dark_plate(b);
    b.prism(Vec3::Z * 6.8, 8, 1.4, 0.9, 0.6);
    coil_light(b, 6);
    b.prism(Vec3::Z * 7.4, 8, 0.7, 0.5, 0.12);
    for at in T2_LENSES {
        collar_plates(b, at);
    }
    tier(b, tech, 3, 0.4, |b| {
        metal(b);
        b.prism(Vec3::Z * 7.5, 8, 0.6, 0.45, 3.0);
        coil_light(b, 6);
        b.prism(Vec3::Z * 10.5, 8, 0.45, 0.3, 0.12);
        for at in T3_LENSES {
            collar_plates(b, at);
        }
    });
    lenses(b, tech);
}

/// An open collar round the floating lens at `at`: two swept plates curling back from
/// beside it, their tips trailing under it.
fn collar_plates(b: &mut MeshBuilder, at: Vec3) {
    let d = facing(at);
    let side = d.cross(Vec3::Z).normalize();
    for s in [1.0, -1.0] {
        let e = side * s;
        plate(
            b,
            at + e * 1.15 + d * 0.3,
            at + e * 0.8 - d * 1.5 - Vec3::Z * 0.3,
            e,
            0.5,
            0.25,
        );
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    /// The lenses are the unit file's anti-missile mounts, at both tiers.
    #[test]
    fn the_unit_files_mounts_are_the_lenses() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let bps = mc_data::Blueprints::load(&dir).unwrap();
        for (key, lenses) in [
            ("regency_t2_missile_defense", super::T2_LENSES.to_vec()),
            (
                "regency_t3_missile_defense",
                [super::T2_LENSES, super::T3_LENSES].concat(),
            ),
        ] {
            let unit = bps.unit_by_key(key).unwrap();
            assert_eq!(unit.visual.mesh, "regency_missile_defense");
            let mounts: Vec<Vec3> = unit
                .anti_missile_mounts
                .iter()
                .map(|m| Vec3::new(m.x.to_f32(), m.y.to_f32(), m.z.to_f32()))
                .collect();
            assert_eq!(mounts.len(), lenses.len(), "{key}");
            for (m, l) in mounts.iter().zip(&lenses) {
                assert!(m.distance(*l) < 1e-3, "{key}: mount {m} vs lens {l}");
            }
        }
    }

    #[test]
    fn rondel() {
        for key in [
            "regency_missile_defense",
            "regency_missile_defense~petals",
            "regency_missile_defense~hover",
        ] {
            super::super::super::check_at(key, 2, 5.25, 12.0, Some(1), &[]);
            super::super::super::check_at(key, 3, 5.25, 15.0, Some(1), &[]);
        }
    }
}
