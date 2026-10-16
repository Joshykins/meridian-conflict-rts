//! Laying a bore that stands off the point its gun turns and pitches about. A giant's
//! arm (`Weapon::sway`: the Breacher's gatlings, the Behemoth's arms) turns about the
//! torso's axis and pitches about the shoulder, but its barrels hang well beside and
//! below that: laid along the line from the shoulder to the mark, the barrels pointed
//! that far beside and under it, while the shells (fired from the muzzle at the mark)
//! left them at an angle. Laid by these, the bore's own line runs through the mark,
//! so the barrels point where the shells go.

use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::Weapon;

/// The yaw that lays the bore of `weapon`, turning about the unit's middle, onto a mark
/// at `bearing`, `dist` metres off: its bore stands `muzzle.y` to the left of the line
/// it turns along, so it turns that much the other way.
pub(crate) fn yaw(weapon: &Weapon, bearing: Angle, dist: Fx) -> Angle {
    bearing - off_line(weapon.muzzle.y, dist)
}

/// How far ahead of `pivot`, along the line the bore of `weapon` is turned to, lies a
/// mark `dist` metres from the unit's middle: the bore stands `muzzle.y` beside that
/// middle, so the mark is that much nearer along it.
pub(crate) fn run(weapon: &Weapon, pivot: FxVec3, dist: Fx) -> Fx {
    let side = weapon.muzzle.y;
    (dist * dist - side * side).max(Fx::ZERO).sqrt() - pivot.x
}

/// The pitch, within what an arm can do, that lays the bore of `weapon` (pitching about
/// `pivot`, the bore level at no pitch) onto a mark `run` metres ahead of the pivot along
/// the bore's turned line and `rise` above it.
pub(crate) fn pitch(weapon: &Weapon, pivot: FxVec3, run: Fx, rise: Fx) -> Angle {
    let line = FxVec2::new(run.max(Fx::ONE), rise);
    let lay = Angle::ZERO.delta_to(line.angle()) as i32
        - Angle::ZERO.delta_to(off_line(weapon.muzzle.z - pivot.z, line.length())) as i32;
    let limit = crate::world::ARM_PITCH_LIMIT as i32;
    Angle(lay.clamp(-limit, limit) as i16 as u16)
}

/// How far a line standing `offset` off a point must turn from the bearing of a mark
/// `dist` away to run through it: the arcsine of `offset / dist` (none when the mark
/// is that close).
fn off_line(offset: Fx, dist: Fx) -> Angle {
    if dist <= offset.abs() {
        return Angle::ZERO;
    }
    FxVec2::new((dist * dist - offset * offset).sqrt(), offset).angle()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: i32, y: i32, z: i32) -> FxVec3 {
        FxVec3::new(Fx::from_int(x), Fx::from_int(y), Fx::from_int(z))
    }

    /// A bore 20 m to the left of the line it turns along, laid on a mark 200 m out dead
    /// ahead, turns right by asin(0.1); one 20 m under the pivot pitches up by as much.
    #[test]
    fn a_bore_off_its_pivot_lays_through_the_mark() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("../../data"),
        )
        .unwrap();
        let mut w = bp.unit(bp.id_of("aster_t4_breacher").unwrap()).weapons[1].clone();
        let asin = Angle::ZERO.delta_to(
            FxVec2::new(Fx::from_int(200 * 200 - 20 * 20).sqrt(), Fx::from_int(20)).angle(),
        );
        w.muzzle = v(40, 20, 30);
        let turned = yaw(&w, Angle::ZERO, Fx::from_int(200));
        assert_eq!(Angle::ZERO.delta_to(turned), -asin);
        w.muzzle = v(40, 0, 30);
        let pivot = v(0, 0, 50);
        let laid = pitch(&w, pivot, run(&w, pivot, Fx::from_int(200)), Fx::ZERO);
        assert!((Angle::ZERO.delta_to(laid) - asin).abs() <= 2);
    }
}
