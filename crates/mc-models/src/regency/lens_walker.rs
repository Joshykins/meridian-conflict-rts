//! The Regency's tech 2 mobile missile defence, the Pauldron, the counterpart of ARC's
//! Aureole: the Rondel's gravity lenses carried on a six-legged walker with the Brazier's
//! gait, so the army has the tower's cover on the march. It walks: not every Regency hull
//! rides on lift.
//!
//! Each lens is one of the unit file's `anti_missile_mounts`
//! (`regency_t2_mobile_missile_defense` in `data/factions/regency/units/land_t2.ron`):
//! keep [`LENSES`] and the data in step. Nothing that carries a lens turns, since the sim
//! fires from the mounts as laid (turned with the hull).
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plate over dark bronze, red optics in
//! the plate (`pattern::EMBER_SLOT`), the missile defence's red in the lenses. No violet: it does not build.
//!
//! Rig: each leg is two bones posed by `entity.wgsl` `crawl_leg` (`set_crawl_legs`), the
//! six feet in two alternating tripods.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

use super::chassis::{coarse_crawl_leg, crawl_leg};
use super::commander::form::{ring, sleeve, OCT};
use super::kit::{dark_plate, metal, v3};
use super::machine::{armour, shaft, swept, Frame};
use super::parts::inlay;
use super::turrets::rondel::crush_lens;

/// The lenses, either side across y: the unit file's `anti_missile_mounts`.
const LENSES: [Vec3; 2] = [Vec3::new(-0.4, 1.4, 4.8), Vec3::new(-0.4, -1.4, 4.8)];
/// A lens's face radius.
const LENS_R: f32 = 0.6;
/// The top of the body, where whatever holds the lenses stands.
const BACK: f32 = 2.45;

/// The left legs, front to back: hip, knee, the foot's tip, where in the cycle it lifts
/// (two alternating tripods). The Brazier's, a quarter larger.
const CRAWL: [(Vec3, Vec3, Vec3, f32); 3] = [
    (
        Vec3::new(1.65, 0.8, 1.95),
        Vec3::new(2.75, 2.2, 3.25),
        Vec3::new(3.6, 3.55, 0.0),
        0.0,
    ),
    (
        Vec3::new(0.0, 1.0, 1.95),
        Vec3::new(0.2, 2.55, 3.45),
        Vec3::new(0.3, 4.0, 0.0),
        0.5,
    ),
    (
        Vec3::new(-1.65, 0.88, 2.0),
        Vec3::new(-2.65, 2.3, 3.25),
        Vec3::new(-3.5, 3.6, 0.0),
        0.06,
    ),
];

/// Where a lens at `at` looks: out to its side and up, so the pair covers the sky.
fn facing(at: Vec3) -> Vec3 {
    (Vec3::Y * at.y.signum() * 0.6 + Vec3::Z).normalize()
}

/// The walker and, as its name says, two great shoulder plates curving up off the body's
/// flanks, each holding a lens at its crown on a bronze collar.
pub(super) fn lens_walker(b: &mut MeshBuilder, _tech: u8) {
    if walker(b) {
        return;
    }
    for lens in LENSES {
        let s = lens.y.signum();
        let side = Vec3::Y * s;
        // The plate: from the flank up and out, curling in under the lens.
        let foot = v3(0.9, 0.95 * s, BACK - 0.3);
        let crown = lens - side * 0.15 - Vec3::Z * 0.75;
        dark_plate(b);
        armour(
            b,
            &Frame::new(foot, crown - foot, side + Vec3::Z * 0.4),
            &swept(foot.distance(crown) + 0.4, 1.0, 0.4, 0.55),
            0.24,
        );
        // A lower lame lapped under it, the way a pauldron's plates lie.
        let low = v3(-1.4, 1.05 * s, BACK - 0.35);
        armour(
            b,
            &Frame::new(low, crown - low, side + Vec3::Z * 0.2),
            &swept(low.distance(crown) * 0.8, 0.6, 0.0, 0.6),
            0.18,
        );
        shaft(b, crown, lens - facing(lens) * LENS_R, 0.22);
        crush_lens(b, lens, facing(lens), LENS_R);
    }
}

/// The walker the plates stand on: six legs and a plated body, red optics at its
/// nose. True when it is drawn far off, where the lenses are a red point each and the
/// plates add nothing more.
fn walker(b: &mut MeshBuilder) -> bool {
    b.set_crawl_legs(&CRAWL, 8.4, 0.55, 0.7);
    b.set_dust_line(1.0);
    if b.coarse() {
        b.mirror_y(|b| {
            for &(hip, knee, foot, _) in &CRAWL {
                coarse_crawl_leg(b, hip, knee, foot, 1.6);
            }
        });
        dark_plate(b);
        b.frustum_open(
            v3(0.0, 0.0, 1.45),
            Vec2::new(4.6, 2.3),
            Vec2::new(3.8, 1.8),
            1.0,
            Vec2::ZERO,
        );
        b.paint(TEAM);
        let z = 2.46;
        b.face(&[
            v3(-0.4, -0.5, z),
            v3(-0.4, 0.5, z),
            v3(-1.6, 0.5, z),
            v3(-1.6, -0.5, z),
        ]);
        b.paint(GLOW_LASER);
        for at in LENSES {
            let (x, y) = (Vec3::X * 0.5, Vec3::Y * 0.5);
            b.face(&[at - x - y, at + x - y, at + x + y, at - x + y]);
        }
        return true;
    }
    b.mirror_y(|b| {
        for (i, &(hip, knee, foot, _)) in CRAWL.iter().enumerate() {
            b.with_pair(i, |b| crawl_leg(b, hip, knee, foot, 1.6));
        }
    });
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(2.65, 0.0, 1.9), Vec3::Z, 0.38, 0.28),
            ring(v3(1.9, 0.0, 2.02), Vec3::Z, 1.05, 0.56),
            ring(v3(-1.5, 0.0, 2.08), Vec3::Z, 1.15, 0.6),
            ring(v3(-2.65, 0.0, 2.12), Vec3::Z, 0.58, 0.38),
        ],
        &OCT,
    );
    b.mirror_y(|b| {
        // The hip drums, up close: the body's flank hides them further off.
        let drums: &[_] = if b.fine() { &CRAWL } else { &[] };
        for &(hip, _, foot, _) in drums {
            let out = (foot - hip).truncate().extend(0.0).normalize();
            metal(b);
            b.cylinder_between(hip - out * 0.38, hip + out * 0.2, 0.42, 0.38, 8);
        }
        // The optics, recessed in the nose's flank (the light is the surface's).
        if b.fine() {
            let (c, out) = (v3(2.35, 0.36, 1.98), v3(0.6, 0.8, 0.0));
            let (s, t) = (Vec3::X * 0.18, Vec3::Z * 0.07);
            inlay(
                b,
                &[c - s - t, c + s - t, c + s + t, c - s + t],
                out,
                pattern::EMBER_SLOT,
            );
        }
    });
    // The owner's colour on the back, behind whatever holds the lenses.
    b.paint(TEAM);
    let z = BACK + 0.21;
    b.face(&[
        v3(-1.6, -0.45, z),
        v3(-1.6, 0.45, z),
        v3(-2.4, 0.3, z - 0.06),
        v3(-2.4, -0.3, z - 0.06),
    ]);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check("regency_lens_walker", 5.0, 5.6, None, &[]);
    }

    #[test]
    fn walks_on_six_legs() {
        let model = build_model("regency_lens_walker").unwrap();
        let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
        assert_eq!(crawl.pairs, 3);
        assert!(!model.hover && model.lifts.is_empty());
    }

    /// The lenses are the unit file's anti-missile mounts, and it walks.
    #[test]
    fn the_unit_files_mounts_are_the_lenses() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let bps = mc_data::Blueprints::load(&dir).unwrap();
        let unit = bps
            .unit_by_key("regency_t2_mobile_missile_defense")
            .unwrap();
        assert_eq!(unit.visual.mesh, "regency_lens_walker");
        assert_eq!(unit.motion.map(|m| m.layer), Some(mc_data::MoveLayer::Land));
        let mounts: Vec<Vec3> = unit
            .anti_missile_mounts
            .iter()
            .map(|&m| crate::authored(unit, m))
            .collect();
        assert_eq!(mounts.len(), LENSES.len());
        for (m, l) in mounts.iter().zip(&LENSES) {
            assert!(m.distance(*l) < 1e-3, "mount {m} vs lens {l}");
        }
        let model = build_model("regency_lens_walker").unwrap();
        for lod in &model.lods {
            assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
        }
    }
}
