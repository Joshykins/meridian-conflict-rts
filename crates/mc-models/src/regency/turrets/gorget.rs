//! The Gorget (tech 2 anti-air, a 2 x 2 lot): a Twin Pinched-plasmeric Airburst Repeater.
//! Two barrels, one over the other, fire in turn, each shot a pinched bolt fused to burst
//! where it was laid. A flat prong either side of the pair reaches past both mouths, red
//! emitters on its inner face. The guns are drawn level; the sim holds them up at the sky
//! at rest.
//!
//! The base: a hexagonal keep braced by three keeled buttresses, a plate down each one's
//! back, plated struts on the faces between them, and the race the turret rides.
//!
//! Its pivot and muzzles are the unit file's (`data/factions/regency/units/structures.ron`):
//! `muzzle` is the middle between the two mouths, `muzzles` the mouths, upper first.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::*;

/// The guns' trunnion and the middle between their mouths, the bore level.
pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 7.0), Vec3::new(8.0, 0.0, 7.0));
/// The barrels over and under the line.
pub(super) const BARRELS: [f32; 2] = [0.6, -0.6];
const THICK: f32 = 0.3;

pub(crate) fn gorget(b: &mut MeshBuilder, _tech: u8) {
    LINE.rig(b, 0.7);
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
    keep(b);
    b.with_part(part::TURRET, |b| {
        step(b, 6, 4.0, 3.5, 5.7, 0.7);
        cheeks(b, [-2.8, 1.6], 2.3, [6.2, 8.9], 0.5);
        team_patch(b, -3.4, -2.4, 0.8, 6.41);
        b.with_limb(rig::ARM_GUN, |b| {
            gun_frame(b, &LINE, |b| stacked(b, LINE.len()))
        });
    });
}

/// A hexagonal keep under a seam course, a keeled buttress off every other face with a
/// plate down its back, a plated strut and a red slot on the faces between, and the
/// owner's colour round the race on top.
fn keep(b: &mut MeshBuilder) {
    let fine = b.fine();
    dark_plate(b);
    b.prism(Vec3::ZERO, 6, 6.2, 5.3, 5.2);
    seam(b);
    b.prism(Vec3::Z * 5.2, 6, 5.3, 5.1, 0.3);
    for k in 0..3 {
        b.yawed(Vec3::ZERO, (120.0 * k as f32).to_radians(), |b| {
            buttress(b, 4.0, 3.2, 4.6, 10.6, 2.0, 0.9);
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3(4.6, 0.0, 4.7), v3(1.0, 0.0, -0.6), v3(0.6, 0.0, 1.0)),
                &swept(6.2, 0.9, 0.0, 0.55),
                THICK,
            );
        });
        let a = (60.0 + 120.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let side = v3(-d.y, d.x, 0.0);
        if fine {
            strut(b, d * 5.6 + Vec3::Z * 0.3, d * 5.0 + Vec3::Z * 4.6, 0.35);
            slit(
                b,
                d * 5.22 + side * 1.6 + Vec3::Z * 2.6,
                d,
                Vec3::Z,
                1.8,
                0.2,
            );
            slit(
                b,
                d * 5.22 - side * 1.6 + Vec3::Z * 2.6,
                d,
                Vec3::Z,
                1.8,
                0.2,
            );
        }
    }
    let segs = if fine { 24 } else { 8 };
    b.paint(TEAM);
    hoop(b, Vec3::Z * 5.52, 4.7, 0.7, 0.05, segs);
    metal(b);
    hoop(b, Vec3::Z * 5.6, 3.9, 1.0, 0.25, segs);
}

/// The breech the two guns share: a trunnion collar across and a keeled block.
fn breech(b: &mut MeshBuilder, w: f32, h: f32) {
    collar(b, Vec3::ZERO, Vec3::Y, 0.8, 4.4);
    dark_plate(b);
    hull_x(b, &[[-2.6, w, h, 0.1], [2.0, w + 0.2, h, 0.0]], &KEELED);
}

/// One barrel over the other in a tall breech, a flat prong either side of the pair
/// reaching past both mouths with red emitters inside, tied under in metal.
fn stacked(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    breech(b, 2.6, 3.0);
    b.mirror_y(|b| {
        dark_plate(b);
        bar_through(
            b,
            &[
                (v3(1.6, 1.0, 0.0), Vec2::new(0.8, 2.8)),
                (v3(len - 1.0, 1.3, 0.0), Vec2::new(0.7, 2.4)),
                (v3(len + 0.4, 1.2, 0.0), Vec2::new(0.5, 1.4)),
            ],
            Vec3::Y,
        );
        for z in BARRELS {
            emitter(b, v3(len - 0.3, 0.95, z), v3(len, 0.0, z), 0.26);
        }
        if fine {
            slit(b, v3(len - 3.0, 1.66, 0.0), Vec3::Y, Vec3::X, 2.4, 0.3);
        }
    });
    for z in BARRELS {
        b.with_recoil(|b| {
            dark_plate(b);
            hull_x(b, &[[2.0, 1.0, 0.95, z], [len, 0.66, 0.66, z]], &CHAMFERED);
            if fine {
                mouth_rim(b, v3(len, 0.0, z), 0.32, 8);
            }
        });
    }
    metal(b);
    bar_through(
        b,
        &[
            (v3(5.2, -1.4, -1.5), Vec2::new(0.35, 0.35)),
            (v3(5.2, 1.4, -1.5), Vec2::new(0.35, 0.35)),
        ],
        Vec3::X,
    );
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
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
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
