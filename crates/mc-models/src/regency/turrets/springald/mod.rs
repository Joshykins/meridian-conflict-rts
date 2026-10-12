//! The Springald (`regency_t4_artillery`, an 8 x 8 lot, the Culverin's and a Power Generator III's size): the
//! Regency's map gun, a Triune Pinch-fusion Howitzer. Like the Sunspear it gathers its
//! charge in front of the bore, here three knots of fusion at once, one at each rail's
//! lens, and lobs them high as one shell. The `muzzle` is the middle of the charge.
//!
//! A long turret turns on a toothed race (`house`) over its drum and outriggers (`base`): plated cheeks
//! either side of the trunnion, the fusion plant that feeds the gun on its back, three
//! capacitor cells (`back`), and the gun (`rails`) laid 45 degrees up at rest, as a map gun
//! stands, and never lower: it lobs higher still (`keeps_aim` holds it where it last
//! fired). Through the charge the gun's three blades draw back off the bore
//! (`charge_gear::FLARE`); on the shot they slam shut, the bore runs back into its jacket,
//! and the radiator lids and the cells' caps stand up with the heat (`charge_gear::VENT`).
//!
//! Its pivot and muzzle ([`LINE`]) are the unit file's
//! (`data/factions/regency/units/strategic.ron`).

mod back;
mod base;
mod house;
mod rails;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::{part, rig};

use super::*;

/// The blueprint's radius and height.
pub(in crate::regency) const RADIUS: f32 = 42.5;
pub(in crate::regency) const HEIGHT: f32 = 48.0;
/// The gun's line: the trunnion over the turret's middle, the charge 52 m out down the bore
/// laid 45 degrees up (52 cos 45 = 36.77 out, 52 sin 45 = 36.77 up).
pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 18.0), Vec3::new(36.77, 0.0, 54.77));
/// How far round the charge its projectors stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 5.5;
/// The race the turret turns on, and its deck.
const RACE: f32 = 15.0;
const DECK: f32 = 8.0;
/// The turret roof.
const ROOF: f32 = 13.0;
/// The cheeks' inner faces, off the bore.
const CHEEK: f32 = 6.4;

/// The Springald, its gun laid 45 degrees up.
pub(in crate::regency) fn springald(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(Vec3::Z * DECK);
    b.set_arm_pivot(LINE.pivot);
    b.set_recoil(LINE.pivot, LINE.muzzle, 4.0);
    if b.coarse() {
        coarse(
            b,
            &LINE,
            &Coarse {
                base_r: 44.0,
                base_h: DECK,
                x0: -22.0,
                x1: 10.0,
                half: 9.0,
                // Up to the plant's top.
                top: ROOF + 9.0,
                gun: Vec2::new(3.8, 3.8),
                tip: Vec2::splat(3.0),
                mouth: Some((44.0, 3.6)),
            },
        );
        return;
    }
    // The blades flare off the bore's line through the trunnion; gear travels are drawn
    // at twice their metres: the blades draw back 1 m, the lids lift 1.8 m.
    b.set_charge_gear(LINE.pivot, 2.0);
    base::draw(b);
    b.with_part(part::TURRET, |b| {
        house::draw(b);
        back::draw(b);
        b.with_limb(rig::ARM_GUN, |b| {
            gun_frame(b, &LINE, |b| {
                rails::breech(b);
                b.with_recoil(rails::bore);
            })
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn springald_holds_its_charge() {
        super::super::super::check_charge(
            "regency_springald",
            RADIUS,
            HEIGHT,
            Some(8),
            &[LINE.muzzle.to_array()],
            HOLD,
        );
    }

    /// The unit file's pivot and muzzle are the model's own.
    #[test]
    fn springald_line_is_the_unit_files() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let unit = bp.unit(bp.id_of("regency_t4_artillery").unwrap());
        assert_eq!(unit.visual.mesh, "regency_springald");
        let w = &unit.weapons[0];
        let v = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
        assert!(v(w.muzzle).distance(LINE.muzzle) < 0.02);
        assert!(v(w.pivot.unwrap()).distance(LINE.pivot) < 0.02);
        assert!((unit.radius.to_f32() - RADIUS).abs() < 0.01);
        assert!((unit.height.to_f32() - HEIGHT).abs() < 0.01);
    }
}
