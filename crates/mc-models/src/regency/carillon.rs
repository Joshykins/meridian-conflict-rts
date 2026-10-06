//! The Carillon (`regency_t4_area_defense`, a 4 x 4 lot): the Regency's area defence, a
//! tall spire that makes plasma orbs and holds them drifting round its shaft until
//! something comes into reach (`Weapon::orbs`). The orbs are not part of the model: they
//! are plasma drawn by the renderer (`renderer/orb_fx.rs`) floating free round the spire
//! like fireflies, so a spire with no orbs stands bare and one with thirty-two sits in a
//! swarm of red light.
//!
//! - A stepped octagonal plinth under a drum, buttress blades sweeping out from the drum
//!   to the plinth's rim, a team band round the drum.
//! - The shaft rises in five tapering octagonal stages to a spike, seams between them lit
//!   red, plated fins raked up its flanks.
//! - A crown of two floating hoops under the spike.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::kit::{blade, dark_plate, metal, seam, v3};
use super::machine::*;

/// The blueprint's radius and height.
pub(super) const RADIUS: f32 = 18.75;
pub(super) const HEIGHT: f32 = 66.0;
/// The shaft's stages: foot and top height, and the half width there.
const STAGES: [(f32, f32, f32, f32); 5] = [
    (3.2, 18.0, 5.0, 4.1),
    (18.0, 34.0, 4.1, 3.2),
    (34.0, 49.0, 3.2, 2.4),
    (49.0, 61.0, 2.4, 1.6),
    (61.0, 66.0, 1.6, 0.15),
];
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
}

/// Far off: the plinth and the shaft as two square cones, the owner's colour on the
/// plinth's top.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carillon_fits_its_lot() {
        // No turret: the orbs are plasma, so there is no muzzle for the check to find.
        super::super::check("regency_carillon", RADIUS, HEIGHT, None, &[]);
    }

    /// The unit file's muzzles are the orbs' places, drifting about them (`mc_sim::orbs`):
    /// whole batches, and every place stands clear of the spire's mesh to drift in.
    #[test]
    fn carillon_orbs_float_clear_of_the_spire() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let unit = bp.unit(bp.id_of("regency_t4_area_defense").unwrap());
        assert_eq!(unit.visual.mesh, "regency_carillon");
        let w = &unit.weapons[0];
        assert_eq!(w.muzzles.len() % w.orbs as usize, 0);
        assert!((unit.radius.to_f32() - RADIUS).abs() < 0.01);
        assert!((unit.height.to_f32() - HEIGHT).abs() < 0.01);
        let model = crate::build_model_scaled("regency_carillon", RADIUS, HEIGHT, 1).unwrap();
        for m in &w.muzzles {
            let orb = Vec3::from(m.to_f32());
            let near = model.lods[0]
                .vertices
                .iter()
                .map(|v| Vec3::from(v.pos).distance(orb))
                .fold(f32::MAX, f32::min);
            assert!(near > 4.0, "something {near} m from the orb at {orb}");
        }
    }
}
