//! The way a conduit runs from the provider's lot to the neighbour's, by its faction's
//! `mc_data::LinePath`, as a polyline the shader lays the cable along. It is laid out
//! centre to centre and cut where it is `INSET_M` inside each lot, so it ducks under the
//! buildings' edges rather than running across a flat one to its middle.

use crate::gpu_consts::link;
use glam::Vec2;
use mc_data::LinePath;

/// A conduit's path: `count` points from the provider to the neighbour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Route {
    pub(super) points: [[f32; 2]; link::POINTS as usize],
    pub(super) count: u32,
    /// Metres end to end, along the path.
    pub(super) length: f32,
    /// Its inner points are turns (`link::TURNS`), not points along a curve.
    pub(super) turns: bool,
}

/// A curve bows out to the side by this share of its length.
const BOW: f32 = 0.16;
/// Points closer than this are one.
const SAME_M: f32 = 0.25;
/// How far into each lot, from the seam, a conduit runs.
const INSET_M: f32 = 6.0;

/// How a conduit of `path` runs between `from` and `to`, two lot centres whose lots share
/// the axis-aligned stretch of edge `edge`, ending `INSET_M` inside each lot (or at the
/// centre, for a lot shallower than that). `bow` (+1 or -1) is the side a curve bows to.
pub(super) fn route(path: LinePath, from: Vec2, to: Vec2, edge: [Vec2; 2], bow: f32) -> Route {
    // The seam's normal axis, and the other: the lots meet across x when the edge runs
    // along y.
    let (normal, lateral) = if (edge[0].x - edge[1].x).abs() < SAME_M {
        (Vec2::X, Vec2::Y)
    } else {
        (Vec2::Y, Vec2::X)
    };
    let d = to - from;
    let mut points: Vec<Vec2> = match path {
        LinePath::Straight => {
            // Out across the seam, along it where the two are not in line, and on in.
            let seam = edge[0].dot(normal);
            let at = |n: f32, t: f32| normal * n + lateral * t;
            vec![
                from,
                at(seam, from.dot(lateral)),
                at(seam, to.dot(lateral)),
                to,
            ]
        }
        LinePath::Diagonal => {
            // Along the longer axis, 45 degrees across the shorter, and along again.
            let (major, minor) = if d.x.abs() >= d.y.abs() {
                (Vec2::new(d.x, 0.0), Vec2::new(0.0, d.y))
            } else {
                (Vec2::new(0.0, d.y), Vec2::new(d.x, 0.0))
            };
            let run = (major.length() - minor.length()) * 0.5;
            let a = from + major.normalize_or_zero() * run;
            let b = a + major.normalize_or_zero() * minor.length() + minor;
            vec![from, a, b, to]
        }
        LinePath::Curve => {
            // A cubic bowed to one side, its ends leaving along the line between them.
            let side = d.perp().normalize_or_zero() * d.length() * BOW * bow;
            let (c1, c2) = (from + d * 0.3 + side, from + d * 0.7 + side);
            let n = link::POINTS as usize;
            (0..n)
                .map(|i| {
                    let t = i as f32 / (n - 1) as f32;
                    let u = 1.0 - t;
                    from * (u * u * u)
                        + c1 * (3.0 * u * u * t)
                        + c2 * (3.0 * u * t * t)
                        + to * (t * t * t)
                })
                .collect()
        }
    };
    let seam = edge[0].dot(normal);
    inset(&mut points, normal, seam);
    points.reverse();
    inset(&mut points, normal, seam);
    points.reverse();
    points.dedup_by(|b, a| a.distance(*b) < SAME_M);
    if path != LinePath::Curve {
        // A run that goes straight on through a point does not turn there.
        let mut i = 1;
        while i + 1 < points.len() {
            let (a, b) = (points[i] - points[i - 1], points[i + 1] - points[i]);
            if a.perp_dot(b).abs() < 1e-3 * a.length() * b.length() && a.dot(b) > 0.0 {
                points.remove(i);
            } else {
                i += 1;
            }
        }
    }
    let mut out = Route {
        points: [[0.0; 2]; link::POINTS as usize],
        count: points.len() as u32,
        length: points.windows(2).map(|w| w[0].distance(w[1])).sum(),
        turns: path != LinePath::Curve,
    };
    for (slot, p) in out.points.iter_mut().zip(&points) {
        *slot = p.to_array();
    }
    out
}

/// Cuts the start of `points` where it first comes within `INSET_M` of the seam (the line
/// across `normal` at `seam`) from its own side. A path that starts that close already is
/// left as it is.
fn inset(points: &mut Vec<Vec2>, normal: Vec2, seam: f32) {
    let side = (points[0].dot(normal) - seam).signum();
    let depth = |p: Vec2| (p.dot(normal) - seam) * side;
    let Some(i) = points
        .windows(2)
        .position(|w| depth(w[0]) > INSET_M && depth(w[1]) <= INSET_M)
    else {
        return;
    };
    let (a, b) = (points[i], points[i + 1]);
    let t = (depth(a) - INSET_M) / (depth(a) - depth(b));
    points[i] = a.lerp(b, t);
    points.drain(..i);
}

#[cfg(test)]
mod tests {
    use super::*;

    // A reactor's 96 m lot at the origin and a 24 m lot against its west side, 12 m up.
    const FROM: Vec2 = Vec2::new(0.0, 0.0);
    const TO: Vec2 = Vec2::new(-60.0, 12.0);
    const EDGE: [Vec2; 2] = [Vec2::new(-48.0, 0.0), Vec2::new(-48.0, 24.0)];

    fn points(r: &Route) -> Vec<Vec2> {
        r.points[..r.count as usize]
            .iter()
            .map(|&p| Vec2::from(p))
            .collect()
    }

    #[test]
    fn every_path_runs_from_inside_one_lot_to_inside_the_other() {
        for path in [LinePath::Straight, LinePath::Curve, LinePath::Diagonal] {
            let r = route(path, FROM, TO, EDGE, 1.0);
            let p = points(&r);
            let (a, b) = (p[0], *p.last().unwrap());
            assert!((a.x - (-48.0 + INSET_M)).abs() < 1e-3, "{path:?} {a:?}");
            assert!((b.x - (-48.0 - INSET_M)).abs() < 1e-3, "{path:?} {b:?}");
            assert!(r.length >= a.distance(b) - 0.01, "{path:?}");
            assert!(r.count <= link::POINTS);
        }
    }

    #[test]
    fn a_lot_shallower_than_the_inset_keeps_its_centre() {
        let edge = [Vec2::new(-4.0, -4.0), Vec2::new(-4.0, 4.0)];
        let r = route(LinePath::Straight, FROM, Vec2::new(-8.0, 0.0), edge, 1.0);
        assert_eq!(points(&r), [FROM, Vec2::new(-8.0, 0.0)]);
    }

    #[test]
    fn straight_runs_square_and_turns_on_the_seam() {
        let r = route(LinePath::Straight, FROM, TO, EDGE, 1.0);
        let p = points(&r);
        assert_eq!(
            p,
            [
                Vec2::new(-48.0 + INSET_M, 0.0),
                Vec2::new(-48.0, 0.0),
                Vec2::new(-48.0, 12.0),
                Vec2::new(-48.0 - INSET_M, 12.0),
            ]
        );
        assert!(r.turns);
        // In line: one run straight across.
        let r = route(LinePath::Straight, FROM, Vec2::new(-60.0, 0.0), EDGE, 1.0);
        assert_eq!(r.count, 2);
    }

    #[test]
    fn diagonal_turns_only_at_45_degrees() {
        // Far enough to the side that the 45 degree stretch crosses the seam.
        let r = route(LinePath::Diagonal, FROM, Vec2::new(-60.0, 40.0), EDGE, 1.0);
        let p = points(&r);
        assert_eq!(r.count, 3);
        for w in p.windows(2) {
            let d = w[1] - w[0];
            let (x, y) = (d.x.abs(), d.y.abs());
            assert!(x < 1e-3 || y < 1e-3 || (x - y).abs() < 1e-3, "{d:?}");
        }
    }

    #[test]
    fn a_curve_bows_to_its_side() {
        let r = route(LinePath::Curve, FROM, Vec2::new(-60.0, 0.0), EDGE, 1.0);
        let middle = Vec2::from(r.points[r.count as usize / 2]);
        assert!(middle.y < -3.0, "{middle:?}");
        assert!(!r.turns);
        let other = route(LinePath::Curve, FROM, Vec2::new(-60.0, 0.0), EDGE, -1.0);
        assert!(Vec2::from(other.points[other.count as usize / 2]).y > 3.0);
    }
}
