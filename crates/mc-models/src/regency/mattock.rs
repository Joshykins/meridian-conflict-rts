//! The Regency's tech 1 artillery, the Mattock: a low four-legged walker carrying a
//! Plasmeric Mortar in a turning house on its back, which lobs a charge of plasma high
//! onto what it is told to strike (the unit file's `Plasmeric Mortar`).
//!
//! The body is a faceted plated hull slung low between four long legs, a sensor head with
//! red optics at its nose. Each leg is a plated thigh up from a bronze hip drum to a high
//! knee, a keeled shin and an armoured boot (no toes). The house on its back is swept
//! back to a point; between its cheeks the mortar pitches on a bronze trunnion: a short
//! fat plated tube under a keeled jacket, a seam-dark collar at the mouth and the plasma's
//! heat glowing red down the bore.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, seams dark, dark bronze on the
//! joints and the trunnion, red optics and weapon heat. No violet: it does not build.
//!
//! Rig: the legs are two bones each, posed by `entity.wgsl` `crawl_leg`
//! (`MeshBuilder::set_crawl_legs`), the four feet in diagonal pairs. The house is the
//! turret and turns about the unit's middle; the mortar pitches about its trunnion
//! (`rig::ARM_GUN`, `set_arm_pivot`). Authored at blueprint scale (radius 4.2, height
//! 3.2): `MUZZLE` and `PIVOT` are `regency_t1_artillery`'s weapon in
//! `data/factions/regency/units/land.ron`.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::breaker::sensor_head;
use super::commander::form::{blade, ring, sleeve, Ring, KEEL, OCT};
use super::kit::{dark_plate, metal, seam, v3};
use super::machine::red_slot;

/// The mortar's mouth and its trunnion: the unit file's weapon `muzzle` and `pivot`.
pub(super) const MUZZLE: Vec3 = Vec3::new(1.13, 0.0, 3.45);
pub(super) const PIVOT: Vec3 = Vec3::new(-0.3, 0.0, 2.45);
/// Where the house turns on the hull.
const TURRET: Vec3 = Vec3::new(0.0, 0.0, 2.05);

/// Left legs, front to back: hip, knee, the foot's tip, and where in the cycle it lifts.
/// The right legs are half a cycle on, so diagonal feet step together.
const LEGS: [(Vec3, Vec3, Vec3, f32); 2] = [
    (
        Vec3::new(1.0, 0.7, 1.45),
        Vec3::new(1.85, 1.7, 2.35),
        Vec3::new(2.65, 2.6, 0.0),
        0.0,
    ),
    (
        Vec3::new(-1.0, 0.7, 1.45),
        Vec3::new(-1.8, 1.75, 2.35),
        Vec3::new(-2.6, 2.65, 0.0),
        0.5,
    ),
];
pub(super) fn mattock(b: &mut MeshBuilder, _tech: u8) {
    b.set_crawl_legs(&LEGS, 8.0, 0.6, 0.6);
    b.set_turret_pivot(TURRET);
    b.set_arm_pivot(PIVOT);
    b.set_dust_line(1.2);
    if b.coarse() {
        coarse(b);
        return;
    }
    hull(b);
    b.mirror_y(|b| {
        for (i, &(hip, knee, foot, _)) in LEGS.iter().enumerate() {
            b.with_pair(i, |b| leg(b, hip, knee, foot));
        }
    });
    b.with_part(part::TURRET, |b| {
        house(b);
        b.with_limb(rig::ARM_GUN, tube);
    });
}

/// Far off: a wedge of a hull, flat legs that do not walk, the house a block with the team
/// colour on its roof and the gun a bar that still pitches.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.frustum(
        v3(0.0, 0.0, 1.1),
        Vec2::new(4.6, 1.8),
        Vec2::new(3.6, 1.4),
        0.95,
        Vec2::new(-0.1, 0.0),
    );
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            for &(hip, knee, foot, _) in &LEGS {
                b.face(&[hip, foot, knee]);
                b.face(&[hip, knee, foot]);
            }
        });
    });
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.frustum(
            v3(-0.3, 0.0, TURRET.z),
            Vec2::new(2.0, 1.4),
            Vec2::new(1.6, 1.1),
            0.55,
            Vec2::ZERO,
        );
        b.paint(TEAM);
        b.face(&[
            v3(0.2, 0.0, TURRET.z + 0.57),
            v3(-0.8, 0.45, TURRET.z + 0.57),
            v3(-0.8, -0.45, TURRET.z + 0.57),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            b.beam(PIVOT, MUZZLE, Vec2::new(0.55, 0.55), Vec2::new(0.5, 0.5));
        });
    });
}

/// The hull: a faceted plated body over a seam-dark keel, the sensor head at its nose and
/// the bronze ring the house turns on.
fn hull(b: &mut MeshBuilder) {
    let (z, k) = (1.6, 1.0);
    seam(b);
    sleeve(
        b,
        &[
            ring(v3(1.9, 0.0, z - 0.25), Vec3::Z, 0.45 * k, 0.2),
            ring(v3(-1.6, 0.0, z - 0.25), Vec3::Z, 0.55 * k, 0.22),
        ],
        &OCT,
    );
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(2.45, 0.0, z - 0.1), Vec3::Z, 0.22, 0.15),
            ring(v3(1.75, 0.0, z), Vec3::Z, 0.78 * k, 0.42),
            ring(v3(-0.5, 0.0, z + 0.02), Vec3::Z, 0.95 * k, 0.5),
            ring(v3(-1.85, 0.0, z - 0.05), Vec3::Z, 0.62 * k, 0.38),
            ring(v3(-2.45, 0.0, z - 0.1), Vec3::Z, 0.22 * k, 0.16),
        ],
        &OCT,
    );
    sensor_head(b, v3(2.5, 0.0, z - 0.2), 0.6);
    // Swept plates down the flanks, their trailing edges the spikes.
    b.mirror_y(|b| {
        dark_plate(b);
        blade(
            b,
            v3(1.3, 0.72 * k, z + 0.25),
            v3(-1.0, 0.18, -0.1),
            v3(0.0, 0.6, 1.0),
            2.6,
            0.32,
            -0.6,
            0.14,
        );
        if b.fine() {
            red_slot(b, v3(-0.6, 0.9 * k, z + 0.05), Vec3::Y, Vec3::X, 0.6, 0.05);
        }
    });
    metal(b);
    let sides = b.sides(12);
    b.prism(v3(0.0, 0.0, TURRET.z - 0.12), sides, 0.8, 0.76, 0.14);
}

/// One leg (left side): a bronze hip drum, a plated thigh up to a high knee with a swept
/// plate over its top, a bronze knee drum, a keeled shin down to an armoured boot.
fn leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, foot: Vec3) {
    let out = (foot - hip).truncate().extend(0.0).normalize();
    let axis = out.cross(Vec3::Z);
    let outside = (Vec3::Z + out * 0.4).normalize();
    let ankle = foot + Vec3::Z * 0.55 - out * 0.12;
    metal(b);
    let sides = b.sides(8);
    b.cylinder_between(hip - out * 0.3, hip + out * 0.12, 0.3, 0.27, sides);
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            dark_plate(b);
            let thigh = knee - hip;
            {
                let r = lean(
                    b,
                    &[
                        ring(hip + thigh * 0.1, outside, 0.27, 0.32),
                        ring(hip + thigh * 0.55, outside, 0.32, 0.4),
                        ring(knee - thigh * 0.08, outside, 0.25, 0.3),
                    ],
                );
                sleeve(b, &r, &KEEL);
            }
            blade(
                b,
                hip + thigh * 0.35 + outside * 0.38,
                thigh,
                outside,
                thigh.length() * 0.75,
                0.2,
                0.0,
                0.1,
            );
        });
        b.with_limb(rig::SHIN, |b| {
            metal(b);
            let sides = if b.fine() { 8 } else { 5 };
            b.cylinder_between(knee - axis * 0.2, knee + axis * 0.3, 0.28, 0.28, sides);
            dark_plate(b);
            let shin = ankle - knee;
            {
                let r = lean(
                    b,
                    &[
                        ring(knee + shin * 0.06, out + Vec3::Z, 0.26, 0.3),
                        ring(knee + shin * 0.5, out + Vec3::Z, 0.22, 0.27),
                        ring(ankle, out + Vec3::Z, 0.17, 0.2),
                    ],
                );
                sleeve(b, &r, &KEEL);
            }
            // The boot: a faceted wedge down to the ground, its toe out along the leg.
            seam(b);
            {
                let r = lean(
                    b,
                    &[
                        ring(ankle + Vec3::Z * 0.05, out, 0.22, 0.26),
                        ring(foot + Vec3::Z * 0.2, out, 0.27, 0.33),
                        ring(foot + Vec3::Z * 0.05, out, 0.12, 0.18),
                    ],
                );
                sleeve(b, &r, &OCT);
            }
        });
    });
}

/// The house (turret): a plated block swept back to a point, an armoured cheek either side
/// of the trunnion, the team colour on its roof and a red slot under its lip.
fn house(b: &mut MeshBuilder) {
    let z = TURRET.z;
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [0.45, 0.48],
                [-0.95, 0.62],
                [-1.65, 0.0],
                [-0.95, -0.62],
                [0.45, -0.48],
            ],
            &[
                Section::new(z, 1.0),
                Section::new(z + 0.4, 0.97),
                Section::new(z + 0.6, 0.78).shifted(-0.18, 0.0),
            ],
        )
    });
    b.mirror_y(|b| {
        dark_plate(b);
        b.with_facets(|b| {
            b.extrude_y(
                &[
                    [PIVOT.x - 0.55, z + 0.05],
                    [PIVOT.x + 0.55, z + 0.05],
                    [PIVOT.x + 0.45, PIVOT.z + 0.2],
                    [PIVOT.x - 0.05, PIVOT.z + 0.45],
                    [PIVOT.x - 0.7, PIVOT.z + 0.15],
                ],
                0.58,
                0.74,
            )
        });
    });
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            v3(-0.35, 0.0, z + 0.62),
            v3(-0.95, 0.25, z + 0.62),
            v3(-1.05, 0.14, z + 0.62),
            v3(-0.55, 0.0, z + 0.62),
        ])
    });
    if b.fine() {
        red_slot(b, v3(0.47, 0.0, z + 0.22), Vec3::X, Vec3::Y, 0.6, 0.06);
    }
}

/// The mortar tube on the trunnion, laid from it to `MUZZLE`'s elevation and length: a bronze breech drum, the
/// plated tube under a keeled jacket, a seam-dark collar and the mouth with the plasma's
/// heat red down it.
fn tube(b: &mut MeshBuilder) {
    let r = 0.3;
    let run = MUZZLE - PIVOT;
    let (len, angle) = (run.length(), run.z.atan2(run.x));
    b.pitched(PIVOT, angle, |b| {
        let o = Vec3::ZERO;
        metal(b);
        let sides = b.sides(10);
        b.cylinder_between(
            o - Vec3::Y * 0.5,
            o + Vec3::Y * 0.5,
            r * 0.95,
            r * 0.95,
            sides,
        );
        dark_plate(b);
        {
            let r = lean(
                b,
                &[
                    ring(o - Vec3::X * 0.45, Vec3::Z, r * 1.05, r * 1.05),
                    ring(o + Vec3::X * 0.3, Vec3::Z, r * 1.2, r * 1.2),
                    ring(o + Vec3::X * (len - 0.3), Vec3::Z, r * 1.05, r * 1.05),
                ],
            );
            sleeve(b, &r, &OCT);
        }
        // The jacket: a keeled plate along the tube's top, swept back past the breech.
        {
            let r = lean(
                b,
                &[
                    ring(o + v3(len - 0.45, 0.0, r * 1.0), Vec3::Z, r * 0.7, r * 0.35),
                    ring(o + v3(0.2, 0.0, r * 1.15), Vec3::Z, r * 0.9, r * 0.45),
                    ring(o + v3(-0.75, 0.0, r * 0.75), Vec3::Z, r * 0.25, r * 0.15),
                ],
            );
            sleeve(b, &r, &KEEL);
        }
        if b.fine() {
            seam(b);
            b.cylinder_between(
                o + Vec3::X * (len - 0.3),
                o + Vec3::X * (len - 0.12),
                r * 1.25,
                r * 1.3,
                sides,
            );
        }
        dark_plate(b);
        b.cylinder_between(
            o + Vec3::X * (len - 0.12),
            o + Vec3::X * len,
            r * 1.15,
            r * 1.2,
            sides,
        );
        // The heat down the bore, sunk well inside the mouth: a glow seen in it, not a
        // lit disc on its face.
        b.paint(GLOW_LASER);
        b.cylinder_between(
            o + Vec3::X * (len - 0.4),
            o + Vec3::X * (len - 0.12),
            r * 0.5,
            r * 0.55,
            sides,
        );
    });
}

/// A small solid's sections: all of them up close, only its ends below full detail.
fn lean(b: &MeshBuilder, rings: &[Ring]) -> Vec<Ring> {
    if b.fine() || rings.len() <= 2 {
        rings.to_vec()
    } else {
        vec![rings[0], rings[rings.len() - 1]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check("regency_mattock", 4.2, 3.2, None, &[MUZZLE.to_array()]);
    }

    #[test]
    fn the_unit_files_mortar_is_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t1_artillery").unwrap());
        assert_eq!(bp.visual.mesh, "regency_mattock");
        assert!((bp.radius.to_f32() - 4.2).abs() < 1e-3 && (bp.height.to_f32() - 3.2).abs() < 1e-3);
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let w = &bp.weapons[0];
        assert!(v(w.muzzle).distance(MUZZLE) < 1e-3);
        assert!(v(w.pivot.expect("it pitches")).distance(PIVOT) < 1e-3);
        assert_eq!(w.trajectory, mc_data::Trajectory::Ballistic);
        assert!(bp.turret_at.is_none(), "the house turns about the middle");

        let model = build_model("regency_mattock").unwrap();
        assert_eq!(model.arm_pivot, Some(PIVOT.to_array()));
        assert!(Vec3::from(model.turret_pivot).truncate().length() < 1e-4);
        for lod in &model.lods {
            assert!(
                lod.vertices
                    .iter()
                    .any(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::ARM_GUN),
                "the mortar pitches"
            );
            assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
        }
    }

    #[test]
    fn walks_on_four_legs_each_rigged_to_its_pair() {
        let model = build_model("regency_mattock").unwrap();
        let crawl = model.legs.and_then(|l| l.crawl).expect("a walker");
        assert_eq!(crawl.pairs, 2);
        assert!(
            model.lifts.is_empty() && !model.hover,
            "a walker, not on lift"
        );
        for lod in &model.lods[..2] {
            for pair in 0..2u32 {
                let shin = || {
                    lod.vertices.iter().filter(move |v| {
                        v.part == part::LOCOMOTION
                            && v.rig & rig::LIMB_MASK == rig::SHIN
                            && (v.rig & rig::PAIR_MASK) >> rig::PAIR_SHIFT == pair
                    })
                };
                assert!(shin().any(|v| v.pos[1] > 0.0) && shin().any(|v| v.pos[1] < 0.0));
                let low = shin().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
                assert!(low < 0.08, "pair {pair}: boot at {low}");
            }
        }
    }
}
