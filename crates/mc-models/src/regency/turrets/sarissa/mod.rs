//! The Sarissa (`regency_t4_anti_ship`, an 8 x 8 lot, the Zenith's size): the Regency's
//! anti-ship lance, a Skyward Pinch-fusion Lance that holds a stream of fusion on a
//! warship in the upper air. The `muzzle` is the lens the lance leaves from.
//!
//! A long turret (`house`) turns on a race over a stepped plinth (`base`) with a heat-sink
//! pylon on each corner. The gun (`gun`) is a split bore laid level at rest and only ever
//! raised: the ships it shoots hang over the field. Through the charge its jaws part off
//! the bore (`charge_gear::PART`), the slots in them rippling toward the lens; they stay
//! open through the burn and shut when it ends, and the pylons' and the turret's lids
//! stand up with the heat (`charge_gear::VENT`). While it burns the core runs back into
//! its jacket (`with_recoil`, the beam's brace).
//!
//! The turret and gun are authored on a deck at [`DECK`] and drawn [`SCALE`] times that
//! size on the plinth's top at [`PLINTH`], so they rule the outline. Their pivot and muzzle
//! as drawn ([`DRAWN`]) are the unit file's (`data/factions/regency/units/strategic.ron`).

mod base;
mod gun;
mod house;

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::{part, rig};

use super::*;

/// The blueprint's radius and height.
pub(in crate::regency) const RADIUS: f32 = 42.5;
pub(in crate::regency) const HEIGHT: f32 = 52.0;
/// The gun's line as authored: the trunnion over the turret's middle, the lens 58 m down
/// the bore.
const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 40.0), Vec3::new(58.0, 0.0, 40.0));
/// The race the turret turns on, and the deck the turret is authored on.
const RACE: f32 = 17.0;
const DECK: f32 = 26.0;
/// The turret roof under the gun, and the cheeks' inner faces off the bore.
const ROOF: f32 = 32.0;
const CHEEK: f32 = 7.2;
/// How much bigger the turret and gun are drawn than authored, and the plinth's top they
/// stand on.
const SCALE: f32 = 1.25;
const PLINTH: f32 = 22.0;
/// The gun's line as drawn.
#[cfg(test)]
const DRAWN: Line = Line::new(
    Vec3::new(0.0, 0.0, PLINTH + (40.0 - DECK) * SCALE),
    Vec3::new(58.0 * SCALE, 0.0, PLINTH + (40.0 - DECK) * SCALE),
);
/// How far round the lens the jaws' heads stand, as drawn: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 4.0;

/// The frame the turret is drawn in: its authored deck on the plinth's top, scaled up.
fn turret_frame() -> Affine3A {
    Affine3A::from_translation(Vec3::Z * PLINTH)
        * Affine3A::from_scale(Vec3::splat(SCALE))
        * Affine3A::from_translation(-Vec3::Z * DECK)
}

/// The Sarissa, its gun laid level.
pub(in crate::regency) fn sarissa(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        coarse(b);
        return;
    }
    base::draw(b);
    b.with(turret_frame(), |b| {
        b.set_turret_pivot(Vec3::Z * DECK);
        b.set_arm_pivot(LINE.pivot);
        b.set_recoil(LINE.pivot, LINE.muzzle, 1.6);
        // The jaws part off the bore's line through the trunnion; gear travels are drawn
        // at three times their metres (and the turret's scale): the jaws part 2 m, the
        // lids lift 3.4 m.
        b.set_charge_gear(LINE.pivot, 3.0);
        b.with_part(part::TURRET, |b| {
            house::draw(b);
            b.with_limb(rig::ARM_GUN, |b| {
                gun_frame(b, &LINE, gun::draw);
            });
        });
    });
}

/// Far off: the plinth, the turret's block and the gun's bar.
fn coarse(b: &mut MeshBuilder) {
    step(b, 8, 38.0, 22.0, 0.0, PLINTH);
    b.with(turret_frame(), |b| {
        b.set_turret_pivot(Vec3::Z * DECK);
        b.set_arm_pivot(LINE.pivot);
        b.set_recoil(LINE.pivot, LINE.muzzle, 1.6);
        b.with_part(part::TURRET, |b| {
            dark_plate(b);
            b.block(v3(-26.0, -11.0, DECK), v3(12.0, 11.0, ROOF + 4.0));
            team_patch(b, -20.0, -8.0, 5.0, ROOF + 4.01);
            b.with_limb(rig::ARM_GUN, |b| {
                gun_frame(b, &LINE, |b| {
                    dark_plate(b);
                    b.beam(
                        v3(-6.0, 0.0, 0.0),
                        v3(LINE.len() - 0.5, 0.0, 0.0),
                        Vec2::new(10.0, 10.0),
                        Vec2::new(4.0, 4.0),
                    );
                });
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sarissa_holds_its_lens() {
        super::super::super::check_charge(
            "regency_sarissa",
            RADIUS,
            HEIGHT,
            Some(8),
            &[DRAWN.muzzle.to_array()],
            HOLD,
        );
    }

    /// The unit file's pivot and muzzle are the model's own.
    #[test]
    fn sarissa_line_is_the_unit_files() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("../../data"),
        )
        .unwrap();
        let unit = bp.unit(bp.id_of("regency_t4_anti_ship").unwrap());
        assert_eq!(unit.visual.mesh, "regency_sarissa");
        let w = &unit.weapons[0];
        let v = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
        assert!(v(w.muzzle).distance(DRAWN.muzzle) < 0.02);
        assert!(v(w.pivot.unwrap()).distance(DRAWN.pivot) < 0.02);
        assert!((unit.radius.to_f32() - RADIUS).abs() < 0.01);
        assert!((unit.height.to_f32() - HEIGHT).abs() < 0.01);
    }
}
