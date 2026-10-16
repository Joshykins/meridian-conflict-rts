//! The Gorget (tech 2 anti-air, a 2 x 2 lot): a Twin Pinched-plasmeric Airburst Repeater.
//! Two barrels, one over the other, fire together, each shot a pinched bolt fused to burst
//! where it was laid. A gorget of lapped bands guards the breech's front, a spine blade
//! runs between the barrels, heat-sink fins stand along each (`gun`). The guns are drawn
//! level; the sim holds them up at the sky at rest.
//!
//! The base (`base`): a hexagonal keep on a footing braced by three keeled buttresses, a
//! bay between each two, a race the turret rides; the turret's deck has a wall either side
//! of the gun and a bustle behind it carrying the owner's colour (a surface mark: no ring).
//!
//! Its pivot and muzzles are the unit file's (`data/factions/regency/units/structures.ron`):
//! `muzzle` is the middle between the two mouths, `muzzles` the mouths, upper first.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::{part, rig};

use super::*;

mod base;
mod gun;

/// The guns' trunnion and the middle between their mouths, the bore level.
pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 7.0), Vec3::new(8.0, 0.0, 7.0));
/// The barrels over and under the line.
pub(super) const BARRELS: [f32; 2] = [0.6, -0.6];

pub(crate) fn gorget(b: &mut MeshBuilder, _tech: u8) {
    LINE.rig(b, 0.9);
    if b.coarse() {
        coarse(
            b,
            &LINE,
            &Coarse {
                base_r: 10.0,
                base_h: 5.4,
                x0: -3.2,
                x1: 2.0,
                half: 2.4,
                top: LINE.pivot.z + 1.4,
                gun: Vec2::new(1.4, 2.6),
                tip: Vec2::new(0.6, 1.4),
                mouth: None,
            },
        );
        return;
    }
    base::keep(b);
    b.with_part(part::TURRET, |b| {
        base::deck(b);
        b.with_limb(rig::ARM_GUN, |b| {
            gun_frame(b, &LINE, |b| gun::gun(b, LINE.len()))
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gorget_fits() {
        let mouths: Vec<[f32; 3]> = BARRELS
            .iter()
            .map(|&z| (LINE.muzzle + Vec3::Z * z).to_array())
            .collect();
        super::super::super::check("regency_airburst_repeater", 10.0, 9.0, Some(2), &mouths);
    }

    /// The unit file's muzzles are the two mouths, over and under its muzzle.
    #[test]
    fn the_unit_files_muzzles_are_the_barrels() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("../../data"),
        )
        .unwrap();
        let unit = bp
            .units
            .iter()
            .find(|u| u.visual.mesh == "regency_airburst_repeater")
            .unwrap();
        let mouths: Vec<Vec3> = unit.weapons[0]
            .muzzles
            .iter()
            .map(|p| Vec3::from(p.to_f32()))
            .collect();
        assert_eq!(mouths.len(), BARRELS.len());
        for (m, z) in mouths.iter().zip(BARRELS) {
            assert!(
                m.distance(LINE.muzzle + Vec3::Z * z) < 0.02,
                "barrel {z}: {m}"
            );
        }
    }
}
