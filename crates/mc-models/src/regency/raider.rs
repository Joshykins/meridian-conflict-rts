//! The Regency's tech 1 raider, the Marauder, the counterpart of ARC's Lancer: a low,
//! fast four-legged walker, its legs splayed wide, carrying a Twin Plasmeric Repeater on a
//! turret over its back: a repeater pod either side of a keeled housing on plated pylons,
//! red optics at the housing's front and the owner's colour behind.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plate over dark bronze joints, red optics
//! and red lenses in the guns' mouths. No violet: it does not build.
//!
//! Rig: each leg is two bones posed by `entity.wgsl` `crawl_leg` (`set_crawl_legs`), the
//! front and back pairs half a cycle apart. The turret turns about the unit's middle at
//! `TURRET`; the twin pods' mouths are the unit file's `muzzles` (`regency_t1_raider` in
//! `data/factions/regency/units/land.ron`).

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::part;

use super::chassis::{coarse_crawl_leg, crawl_leg};
use super::commander::form::{blade, ring, sleeve, KEEL, OCT};
use super::guns::repeater;
use super::kit::{dark_plate, metal, v3};
use super::machine::{armour, red_slot, swept, Frame};

/// The left pod's muzzle (the right is its mirror): the unit file's `muzzles`.
pub(super) const MUZZLE: Vec3 = Vec3::new(2.1, 0.95, 2.7);
/// Where the turret turns.
pub(super) const TURRET: Vec3 = Vec3::new(0.0, 0.0, 2.4);
/// Where each pod's breech starts.
const BREECH: f32 = -0.55;

/// The left legs, front and back: hip, knee, the foot's tip, where in the
/// cycle it lifts.
const CRAWL: [(Vec3, Vec3, Vec3, f32); 2] = [
    (
        Vec3::new(0.6, 0.5, 1.5),
        Vec3::new(1.3, 1.4, 2.25),
        Vec3::new(2.0, 2.2, 0.0),
        0.0,
    ),
    (
        Vec3::new(-0.6, 0.5, 1.5),
        Vec3::new(-1.3, 1.4, 2.25),
        Vec3::new(-2.0, 2.2, 0.0),
        0.5,
    ),
];

/// A low body slung between four splayed legs, the pods on a low turret over its back.
pub(super) fn raider(b: &mut MeshBuilder, _tech: u8) {
    b.set_crawl_legs(&CRAWL, 4.0, 0.55, 0.4);
    b.set_turret_pivot(TURRET);
    b.set_dust_line(0.6);
    if b.coarse() {
        b.mirror_y(|b| {
            for &(hip, knee, foot, _) in &CRAWL {
                coarse_crawl_leg(b, hip, knee, foot, 1.0);
            }
        });
        coarse_upper(b, 2.0);
        return;
    }
    b.mirror_y(|b| {
        for (i, &(hip, knee, foot, _)) in CRAWL.iter().enumerate() {
            b.with_pair(i, |b| crawl_leg(b, hip, knee, foot, 1.0));
        }
    });
    // The body: a keeled hull, hip drums at its flanks.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-1.5, 0.0, 1.62), Vec3::Z, 0.3, 0.22),
            ring(v3(-0.9, 0.0, 1.62), Vec3::Z, 0.62, 0.38),
            ring(v3(0.8, 0.0, 1.6), Vec3::Z, 0.62, 0.4),
            ring(v3(1.55, 0.0, 1.52), Vec3::Z, 0.28, 0.2),
        ],
        &OCT,
    );
    b.mirror_y(|b| {
        for &(hip, _, foot, _) in &CRAWL {
            let out = (foot - hip).truncate().extend(0.0).normalize();
            metal(b);
            b.cylinder_between(hip - out * 0.2, hip + out * 0.12, 0.26, 0.22, b.sides(8));
        }
        red_slot(
            b,
            v3(1.42, 0.2, 1.55),
            v3(0.7, 0.7, 0.0),
            Vec3::Y,
            0.16,
            0.08,
        );
    });
    b.with_part(part::TURRET, |b| twin_turret(b, 2.0));
}

/// A flat plate across the back at `z`, swept back into two points, the owner's colour
/// on it as a chevron.
fn back_plate(b: &mut MeshBuilder, z: f32) {
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(0.55, 0.0, z), -Vec3::X, Vec3::Z),
        &[
            [0.0, -0.3],
            [0.0, 0.3],
            [0.9, 0.42],
            [1.35, 0.2],
            [1.05, 0.0],
            [1.35, -0.2],
            [0.9, -0.42],
        ],
        0.1,
    );
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            v3(0.35, 0.0, z + 0.115),
            v3(-0.25, 0.3, z + 0.115),
            v3(-0.45, 0.24, z + 0.115),
            v3(0.12, 0.0, z + 0.115),
        ])
    });
}

/// A plated pylon from `root` on the hull out to the left pod.
fn pylon(b: &mut MeshBuilder, root: Vec3) {
    dark_plate(b);
    b.beam(
        root,
        v3(-0.15, MUZZLE.y - 0.2, MUZZLE.z),
        Vec2::new(0.5, 0.26),
        Vec2::new(0.42, 0.22),
    );
}

/// The turret on the raider's back at `z0`: a bronze
/// ring, a low keeled housing with red optics at its front and the owner's colour on its
/// back, a sensor blade swept back off it, and the pods either side on pylons.
fn twin_turret(b: &mut MeshBuilder, z0: f32) {
    metal(b);
    b.prism(v3(0.0, 0.0, z0 - 0.05), b.sides(10), 0.55, 0.5, 0.22);
    // The housing stands level with the pods: on a plated plinth over a low back.
    let z = (MUZZLE.z - 0.3).max(z0 + 0.4);
    let top = z + 0.38;
    if z > z0 + 0.45 {
        dark_plate(b);
        b.with_facets(|b| {
            b.loft_z(
                &[
                    [0.6, 0.0],
                    [0.3, 0.45],
                    [-0.6, 0.45],
                    [-0.9, 0.0],
                    [-0.6, -0.45],
                    [0.3, -0.45],
                ],
                &[Section::new(z0 + 0.15, 1.0), Section::new(z - 0.2, 0.85)],
            )
        });
    }
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-1.05, 0.0, z), Vec3::Z, 0.32, 0.25),
            ring(v3(-0.6, 0.0, z + 0.02), Vec3::Z, 0.55, 0.42),
            ring(v3(0.45, 0.0, z), Vec3::Z, 0.55, 0.4),
            ring(v3(1.0, 0.0, z - 0.08), Vec3::Z, 0.22, 0.2),
        ],
        &KEEL,
    );
    b.mirror_y(|b| {
        red_slot(b, v3(0.85, 0.2, z), v3(0.7, 0.7, 0.0), Vec3::X, 0.22, 0.07);
        // A pylon up out of the housing's flank to the pod.
        pylon(b, v3(-0.15, 0.35, z - 0.1));
        pod_only(b);
    });
    back_plate(b, top);
    dark_plate(b);
    blade(
        b,
        v3(-0.5, 0.0, z0 + 0.82),
        v3(-1.0, 0.0, 0.6),
        Vec3::Y,
        0.85,
        0.17,
        0.9,
        0.1,
    );
}

/// The left pod and its shoulder plate, without the hull pylon.
fn pod_only(b: &mut MeshBuilder) {
    let m = MUZZLE;
    repeater(b, v3(BREECH, m.y, m.z), m, 0.3);
    dark_plate(b);
    armour(
        b,
        &Frame::new(
            v3(0.75, m.y - 0.05, m.z + 0.32),
            v3(-1.0, 0.12, 0.12),
            v3(0.0, 0.35, 1.0),
        ),
        &swept(1.95, 0.36, -0.7, 0.55),
        0.1,
    );
}

/// Far off: the upper hull standing on `z0`, a wedge with the owner's colour on its back,
/// and each pod a spike out to its muzzle.
fn coarse_upper(b: &mut MeshBuilder, z0: f32) {
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.frustum(
            v3(-0.1, 0.0, z0),
            Vec2::new(2.6, 1.1),
            Vec2::new(1.8, 0.7),
            0.85,
            Vec2::new(-0.2, 0.0),
        );
        b.paint(TEAM);
        b.face(&[
            v3(0.2, 0.0, z0 + 0.87),
            v3(-0.8, 0.3, z0 + 0.87),
            v3(-0.8, -0.3, z0 + 0.87),
        ]);
        dark_plate(b);
        b.mirror_y(|b| {
            let m = MUZZLE;
            let root = [
                v3(BREECH, m.y - 0.25, m.z - 0.2),
                v3(BREECH, m.y + 0.25, m.z - 0.2),
                v3(BREECH, m.y, m.z + 0.55),
            ];
            b.loft(&[root.to_vec(), vec![m; 3]], true, false);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    fn muzzles() -> [[f32; 3]; 2] {
        [
            MUZZLE.to_array(),
            (MUZZLE * Vec3::new(1.0, -1.0, 1.0)).to_array(),
        ]
    }

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check("regency_raider", 2.6, 4.0, None, &muzzles());
    }

    #[test]
    fn the_unit_files_guns_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t1_raider").unwrap());
        assert_eq!(bp.visual.mesh, "regency_raider");
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let mouths: Vec<Vec3> = bp.weapons[0].muzzles.iter().map(|&m| v(m)).collect();
        for m in muzzles() {
            assert!(
                mouths.iter().any(|p| p.distance(Vec3::from(m)) < 1e-3),
                "{m:?} in {mouths:?}"
            );
        }
        assert!(bp.turret_at.is_none(), "the turret turns about the middle");
        let model = build_model("regency_raider").unwrap();
        assert_eq!(Vec3::from(model.turret_pivot), TURRET);
    }

    #[test]
    fn crawls_on_four_legs_and_does_not_build() {
        let model = build_model("regency_raider").unwrap();
        let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
        assert_eq!(crawl.pairs, 2);
        assert!(!model.hover && model.lifts.is_empty());
        for lod in &model.lods {
            assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
        }
    }
}
