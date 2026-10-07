//! Executioner's layered arrowhead turret and enclosed, segmented fusion lance.
use super::super::kit::v3;
use super::executioner_chassis::{body, metal, plate, wedge};
use super::Line;
use crate::builder::MeshBuilder;
use crate::{part, pattern};
use glam::Vec3;

const LINE: Line = Line {
    pivot: Vec3::new(1.5, 0.0, 5.3),
    pitch_deg: 0.0,
    len: 8.5,
};

pub(in crate::regency) fn executioner(b: &mut MeshBuilder, _tech: u8) {
    LINE.rig(b, 3.6, 0.35);
    body(b);
    if b.coarse() {
        b.with_part(part::TURRET, |b| {
            plate(b);
            b.prism(v3(-0.9, 0.0, 3.8), 4, 4.3, 3.1, 2.7);
            super::team_patch(b, -1.8, -0.6, 0.6, 6.51);
            LINE.gun(b, |b| {
                plate(b);
                b.cylinder_between(Vec3::ZERO, v3(8.5, 0.0, 0.0), 0.8, 0.5, 3);
            });
        });
        return;
    }
    b.with_part(part::TURRET, |b| {
        turret(b);
        LINE.gun(b, cannon);
        if !b.coarse() {
            rockets(b);
        }
    });
}

fn turret(b: &mut MeshBuilder) {
    metal(b);
    b.prism(v3(-0.8, 0.0, 3.55), b.sides(16), 2.4, 2.4, 0.65);
    // Three overlapping roof courses, each ending in a sharp beak. The highest
    // plate extends aft into the reference's paired, swept crown fins.
    for (nose, tail, z, half, rise) in [
        (3.6, -5.4, 4.0, 3.0, 1.05),
        (2.9, -5.7, 4.9, 2.85, 1.05),
        (1.4, -5.65, 5.85, 2.45, 0.85),
    ] {
        wedge(b, nose, tail, 0.0, half, z, rise, false);
    }
    b.mirror_y(|b| {
        wedge(b, 0.5, -5.9, 3.35, 1.1, 4.2, 1.3, false);
        // Cheek plate outside the launch cells, carried into a swept rear fin.
        wedge(b, -0.1, -6.4, 3.8, 0.55, 5.4, 1.1, false);
        plate(b);
        b.extrude_y(&[[-2.2, 6.25], [-5.9, 7.0], [-5.45, 6.0]], 1.45, 1.95);
        if !b.coarse() {
            // The visor is the front bevel itself, carrying a recessed red slot.
            plate(b);
            b.pattern(pattern::REG_SENTRY_SLOT);
            b.face(&[
                v3(1.42, 0.08, 5.9),
                v3(0.75, 1.65, 5.99),
                v3(0.75, 1.65, 6.24),
                v3(1.42, 0.08, 6.15),
            ]);
        }
    });
}

fn barrel_ring(x: f32, y: f32, half: f32, height: f32) -> Vec<Vec3> {
    [
        (-0.65, -1.0),
        (0.65, -1.0),
        (1.0, -0.65),
        (1.0, 0.65),
        (0.65, 1.0),
        (-0.65, 1.0),
        (-1.0, 0.65),
        (-1.0, -0.65),
    ]
    .into_iter()
    .map(|(a, z)| v3(x, y + a * half, z * height))
    .collect()
}

fn cannon(b: &mut MeshBuilder) {
    // A continuous dark pressure tube beneath six separate octagonal jackets.
    metal(b);
    b.cylinder_between(Vec3::ZERO, v3(8.5, 0.0, 0.0), 0.43, 0.33, 8);
    let count = if b.fine() { 6 } else { 3 };
    for i in 0..count {
        let t = i as f32 / count as f32;
        let x = t * 8.1;
        let length = 8.1 / count as f32 - 0.16;
        let w = 0.87 - t * 0.36;
        plate(b);
        b.loft(
            &[
                barrel_ring(x, 0.0, w * 0.88, w * 0.86),
                barrel_ring(x + 0.2, 0.0, w, w),
                barrel_ring(x + length, 0.0, w * 0.86, w * 0.86),
            ],
            true,
            true,
        );
        if !b.coarse() && i % 2 == 1 {
            // A real jacket end face with a hot gasket drawn in its surface.
            b.pattern(pattern::REG_SENTRY_RIM);
            b.face(&barrel_ring(x + length + 0.006, 0.0, w * 0.86, w * 0.86));
        }
    }
    plate(b);
    b.loft(
        &[
            barrel_ring(7.65, 0.0, 0.65, 0.59),
            barrel_ring(8.5, 0.0, 0.58, 0.55),
        ],
        true,
        false,
    );
    // Recessed muzzle: a black cavity, bounded by an armoured octagonal lip.
    let outer = barrel_ring(8.5, 0.0, 0.58, 0.55);
    let inner = barrel_ring(8.5, 0.0, 0.36, 0.34);
    b.pattern(pattern::REG_SENTRY_RIM);
    for k in 0..8 {
        let n = (k + 1) % 8;
        b.face(&[outer[k], outer[n], inner[n], inner[k]]);
    }
    metal(b);
    b.loft(&[barrel_ring(8.05, 0.0, 0.25, 0.25), inner], true, false);
    if !b.coarse() {
        // Short repeater to the right of the long lance; the muzzle and rig stay
        // aligned with the existing gameplay weapon coordinates.
        metal(b);
        b.cylinder_between(v3(0.2, -2.4, 0.0), v3(4.3, -2.4, 0.0), 0.32, 0.24, 8);
        for &x in if b.fine() {
            &[0.45, 1.5, 2.6, 3.6][..]
        } else {
            &[][..]
        } {
            plate(b);
            b.loft(
                &[
                    barrel_ring(x, -2.4, 0.43, 0.39),
                    barrel_ring(x + 0.55, -2.4, 0.4, 0.36),
                ],
                true,
                true,
            );
        }
    }
}

fn rockets(b: &mut MeshBuilder) {
    // The six existing rocket muzzles are tucked into the turret cheeks.
    b.mirror_y(|b| {
        for y in [3.1, 3.6, 4.1] {
            metal(b);
            b.cylinder_between(v3(-0.6, y, 6.0), v3(0.6, y, 6.0), 0.24, 0.23, 6);
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
