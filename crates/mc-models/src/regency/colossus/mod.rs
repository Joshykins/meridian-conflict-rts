//! Regency T4 siege quadruped: a broad, low carapace on four outward-set armored
//! limbs. Recessed rotary bearings and broad greaves carry its mass; charcoal and
//! pale iridescent plates, red surface insets and exposed graphite machinery keep
//! the established faction finish. The jawed siege aperture pitches independently,
//! with two chin guns and an eight-cell dorsal rocket pack on separate mounts.
//!
//! Authored at 40 m radius by 37 m high and built 3.6 times larger. The blueprint's
//! dimensions and weapon origins are checked against this model's rig.

mod body;
mod detail;
mod guns;
mod lance;
mod legs;
mod rockets;
#[cfg(test)]
mod tests;

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::part;

use super::kit::v3;

/// Authored size (the unit file's is `SCALE` times it).
pub(super) const RADIUS: f32 = 40.0;
pub(super) const HEIGHT: f32 = 37.0;
/// How much bigger the unit file builds it.
#[cfg(test)]
const SCALE: f32 = 3.6;

/// Where the carapace turns on the crown: the top of the race, the carapace's belly.
const RACE: f32 = 19.2;
/// The deck the carapace's top stands at.
const DECK: f32 = 27.0;

/// The lance's trunnion, and the middle of its charge: the unit file's weapon 0 `pivot`
/// and `muzzle`. The charge forms [`lance::MOUTH`] ahead of the trunnion, past the jaws' tips.
const LANCE_PIVOT: Vec3 = Vec3::new(10.0, 0.0, 26.2);
#[cfg(test)]
const LANCE_MUZZLE: Vec3 = Vec3::new(LANCE_PIVOT.x + lance::MOUTH, 0.0, LANCE_PIVOT.z);
/// How far round the charge the jaws stand: none of them closer than 0.4 of it.
#[cfg(test)]
// The compact charge core is under one authored metre across at full charge.
const HOLD: f32 = 2.5;

/// The left chin gun's trunnion and the middle of its barrel's mouth: the unit file's
/// weapon 1 `pivot` and `muzzle`. The right gun (weapon 2) is their mirror.
const GUN_PIVOT: Vec3 = Vec3::new(16.2, 8.6, 22.3);
#[cfg(test)]
const GUN_MUZZLE: Vec3 = Vec3::new(GUN_PIVOT.x + guns::LEN, GUN_PIVOT.y, GUN_PIVOT.z);

/// The rocket pack's hinge, the middle of its face and its eight tube mouths, as drawn at
/// rest (level, facing forward): the unit file's weapon 3 `pivot`, `muzzle` and `muzzles`.
/// The sim raises it to the weapon's `rake` about the hinge.
const ROCKET_PIVOT: Vec3 = Vec3::new(-6.5, 0.0, 32.0);
const ROCKET_MUZZLE: Vec3 = Vec3::new(-5.5, 0.0, 34.7);

pub(super) fn colossus(b: &mut MeshBuilder, _tech: u8) {
    build(b, &legs::SIEGE);
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
