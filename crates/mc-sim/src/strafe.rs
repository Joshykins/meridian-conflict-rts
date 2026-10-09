//! Strafing beams (`Weapon::strafe`): held beams fixed in an aircraft's hull that rake the
//! ground on its runs. They do not track the mark. Each lands a set way ahead of the hull
//! along the nose, swung off it only as far as its arc lets it, so as the aircraft flies
//! in, where it lands sweeps over the ground with it: two lines seared through the mark,
//! side by side (`Strafe::gap` apart), striking whatever stands on them.

use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::Strafe;

/// Where a strafing beam's stroke lands this tick, between its muzzles: `ahead` along the
/// nose from `pos`, swung toward `mark`'s line within `half_arc`. `None` while the mark
/// is not under the stroke (more than `half` short of it or past it, or off to the side
/// beyond its swing), or behind the hull. `reach` is how far round the mark counts as on
/// it (its radius).
pub(crate) fn stroke(
    pos: FxVec2,
    heading: Angle,
    strafe: Strafe,
    half_arc: u16,
    mark: FxVec2,
    reach: Fx,
) -> Option<FxVec2> {
    let nose = FxVec2::from_angle(heading);
    let side = nose.perp();
    let to = mark - pos;
    let along = to.dot(nose);
    let across = to.dot(side);
    // How far off the nose's line the beams swing, at the range they land.
    let swing = strafe.ahead * FxVec2::from_angle(Angle(half_arc.min(0x4000))).y;
    let on = along > Fx::ZERO
        && (along - strafe.ahead).abs() <= strafe.half + reach
        && across.abs() <= swing + reach;
    on.then(|| pos + nose * strafe.ahead + side * across.clamp(-swing, swing))
}

/// Where the beam from the muzzle at `local` (unit space, y to the left) lands, the stroke
/// landing at `at` for a hull heading `heading`: half the `gap` to the muzzle's side of it.
pub(crate) fn beam_at(at: FxVec2, heading: Angle, local: FxVec3, gap: Fx) -> FxVec2 {
    let side = if local.y < Fx::ZERO { -gap } else { gap };
    at + FxVec2::from_angle(heading).perp() * (side / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strafe() -> Strafe {
        Strafe {
            ahead: Fx::from_int(200),
            half: Fx::from_int(90),
            gap: Fx::from_int(16),
        }
    }

    /// Flying east at a mark, the stroke comes on 90 m short of it, runs through it at
    /// the hull's own pace and goes out 90 m past it.
    #[test]
    fn the_stroke_rakes_through_the_mark_and_out_past_it() {
        let mark = FxVec2::new(Fx::from_int(1000), Fx::ZERO);
        let at = |x: i32| {
            stroke(
                FxVec2::new(Fx::from_int(x), Fx::ZERO),
                Angle::ZERO,
                strafe(),
                Angle::from_degrees(10).0,
                mark,
                Fx::from_int(4),
            )
        };
        assert_eq!(at(700), None, "too far out");
        let first = at(720).expect("on, short of the mark");
        assert_eq!(first, FxVec2::new(Fx::from_int(920), Fx::ZERO));
        assert_eq!(at(800), Some(mark));
        assert!(at(880).is_some(), "on, past the mark");
        assert_eq!(at(900), None, "gone out past it");
        assert_eq!(at(1100), None, "behind");
    }

    /// A mark off the run's line pulls the beams over as far as their arc lets them, and
    /// no further.
    #[test]
    fn the_beams_swing_only_within_their_arc() {
        let pos = FxVec2::ZERO;
        let arc = Angle::from_degrees(10).0;
        let near = FxVec2::new(Fx::from_int(200), Fx::from_int(20));
        let at = stroke(pos, Angle::ZERO, strafe(), arc, near, Fx::ONE).expect("in its swing");
        assert_eq!(at, near);
        let wide = FxVec2::new(Fx::from_int(200), Fx::from_int(60));
        assert_eq!(stroke(pos, Angle::ZERO, strafe(), arc, wide, Fx::ONE), None);
        // The port beam lands half the gap to the left of the stroke, the starboard to the
        // right.
        let muzzle = |y| FxVec3::new(Fx::ZERO, Fx::from_int(y), Fx::ZERO);
        let gap = strafe().gap;
        let port = beam_at(at, Angle::ZERO, muzzle(3), gap);
        assert_eq!(port, near + FxVec2::new(Fx::ZERO, Fx::from_int(8)));
        let starboard = beam_at(at, Angle::ZERO, muzzle(-3), gap);
        assert_eq!(starboard, near - FxVec2::new(Fx::ZERO, Fx::from_int(8)));
    }
}
