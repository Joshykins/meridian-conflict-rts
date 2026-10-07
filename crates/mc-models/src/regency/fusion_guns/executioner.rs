//! Executioner: a shielded tracked assault hull, protected fusion rails, coaxial
//! repeater and six rocket cells in two turret shoulder racks.
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::turrets::sunspear;
use super::executioner_chassis::body;
use super::{coarse_gun, fusion_core, team_patch, turret_hull, Line};
use crate::builder::MeshBuilder;
use crate::part;
use glam::{Affine3A, Vec2, Vec3};

const LINE: Line = Line {
    pivot: Vec3::new(1.5, 0.0, 5.3),
    pitch_deg: 0.0,
    len: 8.5,
};
pub(in crate::regency) fn executioner(b: &mut MeshBuilder, _tech: u8) {
    LINE.rig(b, 3.6, 0.35);
    body(b);
    if b.coarse() {
        coarse_gun(b, &LINE, 3.6, 6.4, 4.5, 1.1);
        return;
    }
    b.with_part(part::TURRET, |b| {
        let shoulder = 3.1;
        turret_hull(
            b,
            &[[3.5, 0.5], [0.8, shoulder], [-3.2, shoulder], [-5.4, 0.0]],
            3.7,
            6.2,
        );
        team_patch(b, -1.2, 1.0, 0.8, 6.22);
        fusion_core(b, v3(-3.8, 0.0, 6.1), 0.18, 0.18);
        LINE.gun(b, |b| {
            b.with(Affine3A::from_scale(Vec3::splat(0.34)), |b| {
                sunspear::rails(b, 25.0);
                sunspear::pinch_coils(b);
            });
            // Armoured shrouds protect the coil assembly while leaving the charge clear.
            dark_plate(b);
            for y in [-1.25, 1.25] {
                b.beam(
                    v3(0.0, y, 0.0),
                    v3(6.0, y, 0.0),
                    Vec2::new(0.65, 1.0),
                    Vec2::new(0.4, 0.65),
                );
            }
            // Coaxial plasma repeater: same yaw and pitch assembly as the fusion gun.
            metal(b);
            b.cylinder_between(v3(0.5, -2.4, 0.0), v3(4.3, -2.4, 0.0), 0.38, 0.28, 8);
            seam(b);
            b.cylinder_between(v3(4.28, -2.4, 0.0), v3(4.32, -2.4, 0.0), 0.22, 0.22, 8);
        });
        for sign in [-1.0, 1.0] {
            dark_plate(b);
            let rear = -2.7;
            b.beam(
                v3(rear, sign * 3.6, 5.6),
                v3(0.45, sign * 3.6, 5.9),
                Vec2::new(1.7, 1.05),
                Vec2::new(1.65, 0.95),
            );
            for y in [3.1, 3.6, 4.1] {
                metal(b);
                b.cylinder_between(
                    v3(0.45, sign * y, 6.0),
                    v3(0.6, sign * y, 6.0),
                    0.23,
                    0.23,
                    if b.fine() { 8 } else { 4 },
                );
                seam(b);
                b.cylinder_between(
                    v3(0.6, sign * y, 6.0),
                    v3(0.62, sign * y, 6.0),
                    0.17,
                    0.17,
                    if b.fine() { 8 } else { 4 },
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executioner_weapons_match_the_model() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let bp = mc_data::Blueprints::load(&dir).unwrap();
        let id = bp.id_of("regency_t3_assault_tank").unwrap();
        let unit = bp.unit(id);
        let factory = bp.unit(bp.id_of("regency_t3_land_factory").unwrap());
        assert!(factory.builder.as_ref().unwrap().builds.contains(&id));
        assert_eq!(unit.visual.mesh, "regency_executioner");
        let gun = &unit.weapons[0];
        assert_eq!(gun.muzzle.x.to_f32(), LINE.muzzle().x);
        assert!((gun.muzzle.z.to_f32() - LINE.muzzle().z).abs() < 0.001);
        for key in ["regency_executioner"] {
            let model = crate::build_model(key).unwrap();
            for mesh in &model.lods {
                assert!(!mesh.indices.is_empty());
                assert!(mesh
                    .vertices
                    .iter()
                    .all(|v| v.pos[2] >= -0.01 && v.pos[2] <= 8.75));
                assert!(mesh
                    .vertices
                    .iter()
                    .any(|v| v.material == crate::material::TEAM));
            }
            let mesh = &model.lods[0];
            for muzzle in unit.weapons[1..].iter().flat_map(|w| {
                if w.muzzles.is_empty() {
                    vec![w.muzzle]
                } else {
                    w.muzzles.clone()
                }
            }) {
                let at = v3(muzzle.x.to_f32(), muzzle.y.to_f32(), muzzle.z.to_f32());
                let near = mesh
                    .vertices
                    .iter()
                    .filter(|v| v.part == part::TURRET)
                    .map(|v| Vec3::from(v.pos).distance(at))
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    near < 0.4,
                    "{key}: barrel missing at {at:?}, nearest {near}"
                );
            }
        }
    }
}
