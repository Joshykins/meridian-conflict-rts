//! The Colossus, the Regency's tech 5 siege walker: a long low plated hull carried 34 m up
//! on four legs splayed far out to the sides, as wide as the hull is long, that step over
//! hills and houses alike. Its head is one Heavy
//! Pinch-fusion Lance, a wedge prow whose two jaws part along their seam through the
//! charge and hold open while the star burns between them; two Rapid Pinch-fusion Guns
//! hang under the chin on houses of their own, and a battery of eight rocket tubes lies
//! on the carapace's back, hinged to rise when it fires.
//!
//! - `legs`: the crown the hips meet at, and the four legs (a thigh out to a knee under
//!   its guard, a long keeled shin, an armoured hoof).
//! - `body`: the hull the head turns on (one swept loft with lames), the chin pods, the dorsal fin.
//! - `lance`: the cannon head in its trunnion at the carapace's front, the jaws, the
//!   bore's pinch coils and prism.
//! - `guns`: the two chin guns (weapons 1 and 2, `rig::HOUSE`).
//! - `rockets`: the rocket pack (weapon 3, a house, its tubes pitched by the sim to the
//!   unit file's `rake`).
//!
//! Rig: four legs walking as a quadruped (`MeshBuilder::set_crawl_legs`), two mirrored
//! pairs, each foot lifting in turn (rear left, front right, rear right, front left) so
//! three always stand. The carapace is the turret and turns about the crown. The lance
//! pitches about its trunnion (`rig::ARM_GUN`, `set_arm_pivot`); through the charge its jaws
//! part (`charge_gear::SPREAD`), the bore's prism sliding out between them
//! (`charge_gear::EXTEND`) as the coils light.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, seam-dark joints, graphite
//! machinery, red optics, fusion lit in the prism. No violet: it does not build.
//!
//! Authored 32 m by 50 m and built [`SCALE`] times bigger: every number for it in
//! `data/factions/regency/units/experimental.ron` (`regency_t5_colossus`) is this
//! file's times `SCALE` (tests).

mod body;
mod guns;
mod lance;
mod legs;
mod rockets;
#[cfg(test)]
mod tests;
mod variant_b;

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::part;

use super::kit::v3;

pub(super) use variant_b::colossus_b;

/// Authored size (the unit file's is `SCALE` times it).
pub(super) const RADIUS: f32 = 32.0;
pub(super) const HEIGHT: f32 = 57.0;
/// How much bigger the unit file builds it.
#[cfg(test)]
const SCALE: f32 = 6.0;

/// Where the carapace turns on the crown: the top of the race, the carapace's belly.
const RACE: f32 = 39.2;
/// The deck the carapace's top stands at.
const DECK: f32 = 47.0;

/// The lance's trunnion, and the middle of its charge: the unit file's weapon 0 `pivot`
/// and `muzzle`. The charge forms [`lance::MOUTH`] ahead of the trunnion, past the jaws' tips.
const LANCE_PIVOT: Vec3 = Vec3::new(10.0, 0.0, 45.2);
#[cfg(test)]
const LANCE_MUZZLE: Vec3 = Vec3::new(LANCE_PIVOT.x + lance::MOUTH, 0.0, LANCE_PIVOT.z);
/// How far round the charge the jaws stand: none of them closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 5.0;

/// The left chin gun's trunnion and the middle of its barrel's mouth: the unit file's
/// weapon 1 `pivot` and `muzzle`. The right gun (weapon 2) is their mirror.
const GUN_PIVOT: Vec3 = Vec3::new(16.2, 8.6, 42.3);
#[cfg(test)]
const GUN_MUZZLE: Vec3 = Vec3::new(GUN_PIVOT.x + guns::LEN, GUN_PIVOT.y, GUN_PIVOT.z);

/// The rocket pack's hinge, the middle of its face and its eight tube mouths, as drawn at
/// rest (level, facing forward): the unit file's weapon 3 `pivot`, `muzzle` and `muzzles`.
/// The sim raises it to the weapon's `rake` about the hinge.
const ROCKET_PIVOT: Vec3 = Vec3::new(-6.5, 0.0, 45.4);
const ROCKET_MUZZLE: Vec3 = Vec3::new(-5.5, 0.0, 48.1);

pub(super) fn colossus(b: &mut MeshBuilder, _tech: u8) {
    build(b, &legs::ARCHED);
}

/// The model with legs in `style`.
fn build(b: &mut MeshBuilder, style: &legs::Style) {
    let legs = legs::legs(style);
    legs::rig(b, style, &legs);
    b.set_turret_pivot(v3(0.0, 0.0, RACE));
    b.set_arm_pivot(LANCE_PIVOT);
    b.set_dust_line(2.0);
    b.set_shield_emitter(body::EMITTER);
    if b.coarse() {
        body::coarse(b);
        legs::coarse(b, &legs);
        return;
    }
    legs::pelvis(b, style);
    let (front, rear) = (legs[0], legs[1]);
    b.mirror_y(|b| b.with_pair(0, |b| legs::leg(b, style, front)));
    b.mirror_y(|b| b.with_pair(1, |b| legs::leg(b, style, rear)));
    b.with_part(part::TURRET, |b| {
        body::carapace(b);
        lance::lance(b);
        guns::guns(b);
        rockets::pack(b);
    });
}
