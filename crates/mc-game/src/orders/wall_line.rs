//! A wall's place-drag: a line of one-cell sections, each beside the last along a side,
//! so every section joins the next (`mc_sim::mirror::join_walls`). A slant is laid as
//! steps, not as sections touching only at their corners.

use mc_core::FxVec2;

use super::MAX_DRAG_SITES;

/// Centres of the cells a wall dragged from `from` to `to` stands in, `from`'s first.
pub(super) fn wall_line(from: FxVec2, to: FxVec2) -> Vec<FxVec2> {
    let cell = mc_map::BUILD_CELL_M;
    let index = |p: FxVec2| {
        (
            p.x.floor_int().div_euclid(cell),
            p.y.floor_int().div_euclid(cell),
        )
    };
    let centre = |(x, y): (i32, i32)| FxVec2::from_ints(x * cell + cell / 2, y * cell + cell / 2);
    let (start, end) = (index(from), index(to));
    let (nx, ny) = ((end.0 - start.0).abs(), (end.1 - start.1).abs());
    let step = ((end.0 - start.0).signum(), (end.1 - start.1).signum());
    let mut at = start;
    let (mut ix, mut iy) = (0, 0);
    let mut out = vec![centre(at)];
    // A drag stops at the most sites one may put down, as a straight drag does.
    while (ix < nx || iy < ny) && out.len() < MAX_DRAG_SITES {
        // Step along whichever axis keeps the cells nearer the straight line.
        if (1 + 2 * ix) * ny < (1 + 2 * iy) * nx {
            at.0 += step.0;
            ix += 1;
        } else {
            at.1 += step.1;
            iy += 1;
        }
        out.push(centre(at));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
        let at = |(x, y): (i32, i32)| FxVec2::from_ints(x * 12 + 3, y * 12 + 9);
        wall_line(at(from), at(to))
            .into_iter()
            .map(|p| ((p.x.round_int() - 6) / 12, (p.y.round_int() - 6) / 12))
            .collect()
    }

    #[test]
    fn every_section_touches_the_last_along_a_side() {
        for to in [(9, 5), (5, 9), (9, 9), (1, 8), (8, 2), (5, 5), (-4, 7)] {
            let line = cells((5, 5), to);
            assert_eq!(line.first(), Some(&(5, 5)));
            assert_eq!(line.last(), Some(&to), "reaches {to:?}");
            for pair in line.windows(2) {
                let d = (pair[1].0 - pair[0].0).abs() + (pair[1].1 - pair[0].1).abs();
                assert_eq!(d, 1, "{pair:?} toward {to:?}");
            }
        }
        assert_eq!(cells((5, 5), (8, 5)), vec![(5, 5), (6, 5), (7, 5), (8, 5)]);
    }

    #[test]
    fn a_long_drag_stops_at_the_cap() {
        assert_eq!(cells((0, 0), (200, 150)).len(), MAX_DRAG_SITES);
    }
}
