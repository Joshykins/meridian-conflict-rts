//! Where a shot passes a hull: a disc of the unit's radius, or, for a long hull, its
//! `Body` capsule laid along its heading. A 570 m warship tested as a disc took shells
//! 200 m out from its flanks, bursting in the air beside it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::UnitBlueprint;

/// The hull's spine in the world plane, its two ends, and its half width about it: a
/// `Body`'s capsule, or the disc of `radius` (a spine of no length).
pub(crate) fn spine(bp: &UnitBlueprint, pos: FxVec2, heading: Angle) -> (FxVec2, FxVec2, Fx) {
    match bp.body {
        Some(b) => {
            let nose = FxVec2::from_angle(heading);
            (
                pos + nose * (b.aft + b.beam),
                pos + nose * (b.fore - b.beam),
                b.beam,
            )
        }
        None => (pos, pos, bp.radius),
    }
}

/// Where along the shot `from + vel * t`, `t` in `enter..=leave`, it passes nearest the
/// spine `a..b`, and the square of how far off it is there. A disc's spine (`a == b`)
/// takes the shot's closest pass to its centre.
pub(crate) fn closest_pass(
    from: FxVec2,
    vel: FxVec2,
    enter: Fx,
    leave: Fx,
    a: FxVec2,
    b: FxVec2,
) -> (Fx, Fx) {
    let len_sq = vel.length_sq().max(Fx::EPSILON);
    let along = |p: FxVec2| ((p - from).dot(vel) / len_sq).clamp(enter, leave);
    if a == b {
        let t = along(a);
        return (t, (from + vel * t).distance_sq(a));
    }
    let off = |t: Fx| (t, to_segment_sq(from + vel * t, a, b));
    // The shot crosses the spine: it runs into the hull's middle.
    let d = b - a;
    let denom = vel.cross(d);
    if denom != Fx::ZERO {
        let t = (a - from).cross(d) / denom;
        let u = (a - from).cross(vel) / denom;
        if t >= enter && t <= leave && u >= Fx::ZERO && u <= Fx::ONE {
            return (t, Fx::ZERO);
        }
    }
    // Two segments that do not cross are nearest at an end of one of them.
    [off(enter), off(leave), off(along(a)), off(along(b))]
        .into_iter()
        .min_by_key(|&(t, d)| (d, t))
        .unwrap_or((enter, Fx::MAX))
}

/// The square of how far `p` is from the segment `a..b`.
fn to_segment_sq(p: FxVec2, a: FxVec2, b: FxVec2) -> Fx {
    let d = b - a;
    let u = ((p - a).dot(d) / d.length_sq().max(Fx::EPSILON)).clamp(Fx::ZERO, Fx::ONE);
    p.distance_sq(a + d * u)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: i32, y: i32) -> FxVec2 {
        FxVec2::from_ints(x, y)
    }

    #[test]
    fn a_disc_takes_the_closest_pass_to_its_centre() {
        let (t, d) = closest_pass(v(0, 10), v(100, 0), Fx::ZERO, Fx::ONE, v(40, 0), v(40, 0));
        assert_eq!(t, Fx::ratio(4, 10));
        assert_eq!(d, Fx::from_int(100));
    }

    #[test]
    fn a_shot_across_the_spine_meets_it() {
        // Spine along x from -200 to 200; the shot comes down across it at x = 150.
        let (t, d) = closest_pass(
            v(150, 100),
            v(0, -200),
            Fx::ZERO,
            Fx::ONE,
            v(-200, 0),
            v(200, 0),
        );
        assert_eq!(d, Fx::ZERO);
        assert_eq!(t, Fx::HALF);
    }

    #[test]
    fn a_shot_beside_the_spine_passes_it_at_its_side() {
        // Running alongside, 50 m out from a spine 400 m long.
        let (_, d) = closest_pass(
            v(-300, 50),
            v(600, 0),
            Fx::ZERO,
            Fx::ONE,
            v(-200, 0),
            v(200, 0),
        );
        assert_eq!(d, Fx::from_int(2500));
        // Past the end, it is measured from the end.
        let (t, d) = closest_pass(
            v(260, -100),
            v(0, 200),
            Fx::ZERO,
            Fx::ONE,
            v(-200, 0),
            v(200, 0),
        );
        assert_eq!(t, Fx::HALF);
        assert_eq!(d, Fx::from_int(3600));
    }
}
