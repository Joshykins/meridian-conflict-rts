//! The Regency's tech 1 mobile anti-air, the Brazier, the counterpart of ARC's Gnat: a
//! stocky six-legged walker with the Outrider's gait, carrying a Plasmeric AA Repeater on a
//! turret over its back: the Canopy's organ made small (three short tubes in one
//! clamped block), drawn level and held up at the sky by the sim at rest so it reads as
//! anti-air from any angle,
//! between two plated cheeks swept back into spikes.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plate over dark bronze, red optics and
//! red lips on the tubes. No violet: it does not build.
//!
//! Rig: each leg is two bones posed by `entity.wgsl` `crawl_leg` (`set_crawl_legs`), the
//! six feet in two alternating tripods. The turret turns about the unit's middle; the organ
//! pitches about its trunnion (`rig::ARM_GUN`), and the middle tube's mouth is the unit
//! file's `muzzle` (`regency_t1_mobile_aa` in `data/factions/regency/units/land.ron`).

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::chassis::{coarse_crawl_leg, crawl_leg};
use super::commander::form::{blade, ring, sleeve, OCT};
use super::guns::flak_organ;
use super::kit::{dark_plate, metal, v3};
use super::machine::{armour, red_slot, swept, Frame};

/// The organ's trunnion and its middle tube's mouth.
#[derive(Clone, Copy, Debug)]
pub(super) struct Line {
    pub(super) pivot: Vec3,
    pub(super) muzzle: Vec3,
}

impl Line {
    fn len(&self) -> f32 {
        self.pivot.distance(self.muzzle)
    }

    fn rig(&self, b: &mut MeshBuilder) {
        b.set_turret_pivot(self.pivot);
        b.set_arm_pivot(self.pivot);
    }
}

/// The organ's trunnion and middle mouth: the unit file's `pivot` and `muzzle`.
pub(super) const LINE: Line = Line {
    pivot: Vec3::new(0.0, 0.0, 3.2),
    muzzle: Vec3::new(2.2, 0.0, 3.2),
};
/// The tubes across the bore, and each one's radius.
const TUBES: [f32; 3] = [-0.48, 0.0, 0.48];
const TUBE_R: f32 = 0.2;
/// How far either side of the bore the cheeks stand.
const CHEEK: f32 = 0.95;

/// The left legs, front to back: hip, knee, the foot's tip, where in the cycle
/// it lifts (two alternating tripods).
const CRAWL: [(Vec3, Vec3, Vec3, f32); 3] = [
    (
        Vec3::new(1.3, 0.65, 1.55),
        Vec3::new(2.2, 1.75, 2.6),
        Vec3::new(2.9, 2.85, 0.0),
        0.0,
    ),
    (
        Vec3::new(0.0, 0.8, 1.55),
        Vec3::new(0.15, 2.05, 2.75),
        Vec3::new(0.25, 3.2, 0.0),
        0.5,
    ),
    (
        Vec3::new(-1.3, 0.7, 1.6),
        Vec3::new(-2.1, 1.85, 2.6),
        Vec3::new(-2.8, 2.9, 0.0),
        0.06,
    ),
];

/// A stocky body on six legs, the turret on its back.
pub(super) fn mobile_aa(b: &mut MeshBuilder, _tech: u8) {
    b.set_crawl_legs(&CRAWL, 8.0, 0.55, 0.6);
    LINE.rig(b);
    b.set_dust_line(0.8);
    if b.coarse() {
        b.mirror_y(|b| {
            for &(hip, knee, foot, _) in &CRAWL {
                coarse_crawl_leg(b, hip, knee, foot, 1.3);
            }
        });
        dark_plate(b);
        b.frustum_open(
            v3(0.0, 0.0, 1.15),
            Vec2::new(3.6, 1.8),
            Vec2::new(3.0, 1.4),
            0.9,
            Vec2::ZERO,
        );
        coarse_turret(b, &LINE, 2.05);
        return;
    }
    b.mirror_y(|b| {
        for (i, &(hip, knee, foot, _)) in CRAWL.iter().enumerate() {
            b.with_pair(i, |b| crawl_leg(b, hip, knee, foot, 1.3));
        }
    });
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(2.1, 0.0, 1.5), Vec3::Z, 0.3, 0.22),
            ring(v3(1.5, 0.0, 1.62), Vec3::Z, 0.82, 0.45),
            ring(v3(-1.2, 0.0, 1.66), Vec3::Z, 0.9, 0.48),
            ring(v3(-2.1, 0.0, 1.7), Vec3::Z, 0.45, 0.3),
        ],
        &OCT,
    );
    b.mirror_y(|b| {
        // The hip drums, up close: the body's flank hides them further off.
        let drums: &[_] = if b.fine() { &CRAWL } else { &[] };
        for &(hip, _, foot, _) in drums {
            let out = (foot - hip).truncate().extend(0.0).normalize();
            metal(b);
            b.cylinder_between(hip - out * 0.3, hip + out * 0.16, 0.34, 0.3, 8);
        }
        red_slot(
            b,
            v3(2.0, 0.22, 1.55),
            v3(0.7, 0.7, 0.0),
            Vec3::Y,
            0.18,
            0.08,
        );
    });
    b.with_part(part::TURRET, |b| turret(b, &LINE, 2.1));
}

/// The turret standing at `z0`: a bronze ring, a faceted housing with red optics at its
/// front and the owner's colour behind, a plated cheek either side of the organ swept
/// back into a spike, a sensor blade off its back, and the organ on its trunnion.
fn turret(b: &mut MeshBuilder, line: &Line, z0: f32) {
    let roof = line.pivot.z - 0.25;
    metal(b);
    b.prism(v3(0.0, 0.0, z0 - 0.05), b.sides(12), 0.95, 0.9, 0.2);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [1.1, 0.0],
                [0.7, 0.85],
                [-1.1, 1.05],
                [-1.8, 0.5],
                [-1.8, -0.5],
                [-1.1, -1.05],
                [0.7, -0.85],
            ],
            &[
                Section::new(z0 + 0.1, 1.0),
                Section::new(roof, 0.86).shifted(-0.1, 0.0),
            ],
        )
    });
    b.paint(TEAM);
    b.face(&[
        v3(-0.6, -0.5, roof + 0.005),
        v3(-0.6, 0.5, roof + 0.005),
        v3(-1.4, 0.35, roof + 0.005),
        v3(-1.4, -0.35, roof + 0.005),
    ]);
    dark_plate(b);
    blade(
        b,
        v3(-1.35, 0.0, roof),
        v3(-1.0, 0.0, 0.6),
        Vec3::Y,
        0.9,
        0.17,
        0.9,
        0.1,
    );
    let p = line.pivot;
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(0.85, 0.42, roof - 0.35),
            v3(0.8, 0.6, 0.1),
            Vec3::Y,
            0.2,
            0.08,
        );
        // The cheek: a plated wall beside the organ up to the trunnion, a plate down its
        // outside swept back and up into a spike.
        dark_plate(b);
        b.block(
            v3(-0.75, CHEEK, roof - 0.1),
            v3(0.55, CHEEK + 0.24, p.z + 0.4),
        );
        armour(
            b,
            &Frame::new(
                v3(0.7, CHEEK + 0.24, p.z - 0.1),
                v3(-1.0, 0.0, 0.45),
                Vec3::Y,
            ),
            &swept(2.3, 0.45, 0.2, 0.5),
            0.14,
        );
    });
    b.with_limb(rig::ARM_GUN, |b| {
        b.at(p, |b| {
            flak_organ(b, line.len(), &TUBES, TUBE_R, CHEEK + 0.05);
        });
    });
}

/// Far off: the turret at `z0` as a block with the owner's colour on it, the organ as a
/// bar out to its middle mouth, still pitching.
fn coarse_turret(b: &mut MeshBuilder, line: &Line, z0: f32) {
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        let roof = line.pivot.z - 0.25;
        b.frustum_open(
            v3(-0.3, 0.0, z0),
            Vec2::new(2.8, 2.0),
            Vec2::new(2.4, 1.7),
            roof - z0,
            Vec2::new(-0.1, 0.0),
        );
        b.paint(TEAM);
        b.face(&[
            v3(-0.3, -0.5, roof + 0.01),
            v3(-0.3, 0.5, roof + 0.01),
            v3(-1.3, 0.5, roof + 0.01),
            v3(-1.3, -0.5, roof + 0.01),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            b.at(line.pivot, |b| {
                dark_plate(b);
                b.beam(
                    v3(-0.4, 0.0, 0.0),
                    v3(line.len(), 0.0, 0.0),
                    Vec2::new(1.4, 0.55),
                    Vec2::new(0.3, 0.2),
                );
            })
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    #[test]
    fn fits_the_librarys_checks() {
        super::super::check(
            "regency_mobile_aa",
            4.0,
            4.0,
            None,
            &TUBES.map(|y| (LINE.muzzle + Vec3::Y * y).to_array()),
        );
    }

    #[test]
    fn walks_on_six_legs() {
        let model = build_model("regency_mobile_aa").unwrap();
        let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
        assert_eq!(crawl.pairs, 3);
        assert!(!model.hover && model.lifts.is_empty());
    }

    #[test]
    fn the_unit_files_gun_is_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t1_mobile_aa").unwrap());
        assert_eq!(bp.visual.mesh, "regency_mobile_aa");
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let w = &bp.weapons[0];
        assert!(v(w.muzzle).distance(LINE.muzzle) < 1e-3);
        assert!(v(w.pivot.unwrap()).distance(LINE.pivot) < 1e-3);
        assert!(bp.turret_at.is_none(), "the turret turns about the middle");
        assert_eq!(bp.motion.map(|m| m.layer), Some(mc_data::MoveLayer::Land));
        let model = build_model("regency_mobile_aa").unwrap();
        assert_eq!(model.arm_pivot, Some(LINE.pivot.to_array()));
        for lod in &model.lods {
            assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
        }
    }
}
