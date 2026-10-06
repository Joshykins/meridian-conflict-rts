//! The Carillon (`regency_t4_area_defense`, a 4 x 4 lot): the Regency's area defence, a
//! tall spire that makes plasma orbs and holds them floating round its shaft until
//! something comes into reach (`Weapon::orbs`). The orbs are not part of the model: they
//! are plasma drawn by the renderer (`renderer/orb_fx.rs`), one in each cradle the model
//! leaves empty, so a spire with no orbs stands bare and one with twenty is ringed in
//! red light.
//!
//! - A stepped octagonal plinth under a drum, buttress blades sweeping out from the drum
//!   to the plinth's rim, a team band round the drum.
//! - The shaft rises in five tapering octagonal stages to a spike, seams between them lit
//!   red, plated fins raked up its flanks between the orbs' bearings.
//! - Five rings of cradles, a batch to a ring (ring 0 the lowest, filled first). At each
//!   ring's height a floating bronze hoop stands off the shaft on four studs, and from
//!   each stud a plated arm reaches out to a cradle: a dish and three prongs holding the
//!   place of an orb ([`orbs`] are the unit file's muzzles). Each ring is turned 22.5
//!   degrees off the one below, so the orbs spiral round the shaft.
//! - A crown of two floating hoops under the spike.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::kit::{blade, dark_plate, metal, seam, spike, v3};
use super::machine::*;

/// The blueprint's radius and height.
pub(super) const RADIUS: f32 = 18.75;
pub(super) const HEIGHT: f32 = 66.0;
/// How far from the shaft the orbs float, the height of each ring, and the orbs in a ring.
const ORB_RADIUS: f32 = 11.5;
const RING_Z: [f32; 5] = [20.0, 29.0, 38.0, 47.0, 56.0];
const BATCH: usize = 4;
/// The shaft's stages: foot and top height, and the half width there.
const STAGES: [(f32, f32, f32, f32); 5] = [
    (3.2, 18.0, 5.0, 4.1),
    (18.0, 34.0, 4.1, 3.2),
    (34.0, 49.0, 3.2, 2.4),
    (49.0, 61.0, 2.4, 1.6),
    (61.0, 66.0, 1.6, 0.15),
];
/// The cradle's dish radius at its rim, and how far under its orb it sits.
const DISH: f32 = 2.5;
const DROP: f32 = 2.9;

/// Where the orbs float: the unit file's `muzzles`, slot by slot (ring `k`, orb `i`).
pub(super) fn orbs() -> Vec<Vec3> {
    (0..RING_Z.len() * BATCH)
        .map(|slot| {
            let (k, i) = (slot / BATCH, slot % BATCH);
            let a = (k as f32 * 22.5 + i as f32 * 90.0).to_radians();
            v3(ORB_RADIUS * a.cos(), ORB_RADIUS * a.sin(), RING_Z[k])
        })
        .collect()
}

pub(super) fn carillon(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        coarse(b);
        return;
    }
    b.with_facets(|b| {
        plinth(b);
        shaft(b);
        fins(b);
        crown(b);
    });
    let places = orbs();
    for (k, &z) in RING_Z.iter().enumerate() {
        ring(b, k, z, &places[k * BATCH..(k + 1) * BATCH]);
    }
}

/// Far off: the plinth and the shaft as two square cones, the owner's colour on the
/// plinth's top, a flat diamond of plate under each ring of orbs.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, 17.0, 13.0, 3.2);
    b.prism(Vec3::Z * 3.2, 4, 5.4, 0.3, HEIGHT - 3.2);
    b.paint(TEAM);
    let top = 3.25;
    b.face(&[
        v3(-9.0, 0.0, top),
        v3(0.0, -9.0, top),
        v3(9.0, 0.0, top),
        v3(0.0, 9.0, top),
    ]);
    metal(b);
    for &z in &RING_Z {
        let z = z - DROP;
        let r = ORB_RADIUS - 1.0;
        let ring = [v3(-r, 0.0, z), v3(0.0, -r, z), v3(r, 0.0, z), v3(0.0, r, z)];
        b.face(&ring);
        b.face(&[ring[3], ring[2], ring[1], ring[0]]);
    }
}

/// The footing: two steps of octagon, the drum the shaft stands in, buttress blades swept
/// out from the drum to the plinth's rim, and the team band.
fn plinth(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(Vec3::ZERO, 8, 17.6, 16.2, 1.6);
    b.prism(Vec3::Z * 1.6, 8, 15.0, 12.8, 1.6);
    seam(b);
    b.prism(Vec3::Z * 3.15, 8, 12.9, 12.6, 0.2);
    dark_plate(b);
    b.prism(Vec3::Z * 3.2, 8, 7.4, 5.6, 9.0);
    b.paint(TEAM);
    b.prism(Vec3::Z * 9.6, 8, 5.95, 5.9, 0.9);
    dark_plate(b);
    for k in 0..8 {
        let a = TAU * (k as f32 + 0.5) / 8.0;
        let d = v3(a.cos(), a.sin(), 0.0);
        // Plates lapped down the drum, each a spike over the step below.
        let long = if k % 2 == 0 { 1.0 } else { 0.8 };
        let f = Frame::new(
            d * 6.4 + Vec3::Z * 12.2,
            d * 0.3 - Vec3::Z,
            d + Vec3::Z * 0.2,
        );
        Course {
            count: if b.fine() { 2 } else { 1 },
            step: 2.2,
            len: 3.4 * long,
            half: 1.5,
            tip: 0.0,
            thick: 0.35,
            tail: 1.2,
        }
        .lay(b, &f);
    }
    for k in 0..4 {
        let a = TAU * k as f32 / 4.0;
        let d = v3(a.cos(), a.sin(), 0.0);
        let side = v3(-a.sin(), a.cos(), 0.0);
        blade(
            b,
            d * 6.0 + Vec3::Z * 14.0,
            d * 15.2 + Vec3::Z * 3.0,
            1.6,
            side,
        );
    }
}

/// The shaft, a stage at a time, a lit seam between them.
fn shaft(b: &mut MeshBuilder) {
    for (n, &(z0, z1, r0, r1)) in STAGES.iter().enumerate() {
        dark_plate(b);
        b.prism(Vec3::Z * z0, 8, r0, r1, z1 - z0);
        if n + 1 < STAGES.len() {
            seam(b);
            b.prism(Vec3::Z * (z1 - 0.25), 8, r1 + 0.18, r1 + 0.14, 0.5);
        }
    }
}

/// Four plated fins raked up the shaft's flanks, between the orbs' bearings.
fn fins(b: &mut MeshBuilder) {
    dark_plate(b);
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let side = v3(-a.sin(), a.cos(), 0.0);
        blade(
            b,
            d * 4.6 + Vec3::Z * 12.0,
            d * 7.4 + Vec3::Z * 30.0,
            1.1,
            side,
        );
        blade(
            b,
            d * 3.5 + Vec3::Z * 31.0,
            d * 5.6 + Vec3::Z * 50.0,
            0.8,
            side,
        );
    }
}

/// Two hoops floating under the spike, each held off the shaft on four studs.
fn crown(b: &mut MeshBuilder) {
    for (z, r) in [(58.5, 3.4), (62.5, 2.5)] {
        metal(b);
        hoop(b, Vec3::Z * z, r, 0.7, 0.55, if b.fine() { 16 } else { 8 });
        for k in 0..4 {
            let a = TAU * k as f32 / 4.0 + 0.4;
            let d = v3(a.cos(), a.sin(), 0.0);
            b.beam(
                d * (r - 0.35) + Vec3::Z * z,
                d * 1.6 + Vec3::Z * z,
                Vec2::splat(0.3),
                Vec2::splat(0.4),
            );
        }
    }
}

/// Ring `k` of cradles at height `z`: a hoop floating off the shaft on studs, and an arm
/// out to each cradle.
fn ring(b: &mut MeshBuilder, k: usize, z: f32, places: &[Vec3]) {
    let under = z - DROP;
    let r = STAGES
        .iter()
        .find(|s| z >= s.0 && z <= s.1)
        .map_or(3.0, |s| s.2 + (s.3 - s.2) * (z - s.0) / (s.1 - s.0));
    metal(b);
    hoop(
        b,
        Vec3::Z * under,
        7.0,
        0.6,
        0.5,
        if b.fine() { 16 } else { 8 },
    );
    for (i, p) in places.iter().enumerate() {
        let a = p.y.atan2(p.x);
        let d = v3(a.cos(), a.sin(), 0.0);
        // The stud and the arm: plated, from the shaft out to under the dish.
        strut(
            b,
            d * (r + 0.1) + Vec3::Z * (under + 0.3),
            d * 7.0 + Vec3::Z * under,
            0.4,
        );
        strut(
            b,
            d * 7.0 + Vec3::Z * under,
            d * (ORB_RADIUS - DISH * 0.6) + Vec3::Z * (under - 0.15),
            0.5,
        );
        cradle(b, *p, d, (k + i) % 2 == 0);
    }
}

/// An orb's cradle: a shallow dish under the place and three prongs curving up round it,
/// the dish lit red by the surface (`seam`'s ember pattern).
fn cradle(b: &mut MeshBuilder, at: Vec3, out: Vec3, _odd: bool) {
    let base = at - Vec3::Z * DROP;
    dark_plate(b);
    b.prism(base - Vec3::Z * 0.5, 6, 1.0, DISH, 0.9);
    seam(b);
    b.prism(base + Vec3::Z * 0.38, 6, DISH * 0.82, DISH * 0.78, 0.14);
    if !b.fine() {
        return;
    }
    dark_plate(b);
    for p in 0..3 {
        // One prong points outward, the other two sweep back toward the shaft.
        let a = out.y.atan2(out.x) + TAU * p as f32 / 3.0;
        let d = v3(a.cos(), a.sin(), 0.0);
        spike(
            b,
            base + d * DISH * 0.85 + Vec3::Z * 0.2,
            at + d * (DISH * 1.05) + Vec3::Z * 1.3,
            0.28,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carillon_fits_its_lot_and_holds_its_orbs_clear() {
        // No turret: the orbs are plasma, so there is no muzzle for the check to find.
        super::super::check("regency_carillon", RADIUS, HEIGHT, None, &[]);
        let model = crate::build_model_scaled("regency_carillon", RADIUS, HEIGHT, 1).unwrap();
        for orb in orbs() {
            let near = model.lods[0]
                .vertices
                .iter()
                .map(|v| Vec3::from(v.pos).distance(orb))
                .fold(f32::MAX, f32::min);
            assert!(near > 2.4, "something {near} m from the orb at {orb}");
        }
    }

    /// The unit file's muzzles are the orbs' places, one orb to a muzzle, a ring a batch.
    #[test]
    fn carillon_orbs_are_the_unit_files() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let unit = bp.unit(bp.id_of("regency_t4_area_defense").unwrap());
        assert_eq!(unit.visual.mesh, "regency_carillon");
        let w = &unit.weapons[0];
        assert_eq!(w.orbs as usize, BATCH);
        assert_eq!(w.muzzles.len(), orbs().len());
        for (m, o) in w.muzzles.iter().zip(orbs()) {
            assert!(Vec3::from(m.to_f32()).distance(o) < 0.02, "{m:?} vs {o}");
        }
        assert!((unit.radius.to_f32() - RADIUS).abs() < 0.01);
        assert!((unit.height.to_f32() - HEIGHT).abs() < 0.01);
    }
}
