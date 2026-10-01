//! Walls that join: which of a section's pieces its neighbours call for, and the frame
//! every joining wall's pieces are built in (`aster::wall`, `regency::palisade`). The
//! entity shader makes the same choice for every vertex (`wall_piece_shown` in
//! entity.wgsl); this copy draws the portraits and checks the meshes.
//!
//! A section is one 12 m cell. Each quarter of it, from the section's centre out to one
//! corner of its lot, carries every piece that quarter could need (`gpu_consts::wall`).
//! Every piece is authored in the quarter between +x and +y and turned round for the
//! other three ([`pieces`]). A piece draws only its outside faces: where it meets the
//! next quarter or the next section, that one's own piece carries on.

use glam::{Affine3A, Vec3};

use super::part;
use crate::builder::MeshBuilder;
use crate::gpu_consts::wall;

/// Half the lot: every piece runs from the section's centre out to its edge.
pub(crate) const EDGE: f32 = 6.0;

/// Builds a joining wall: `piece(b, case)` draws piece `case` (`gpu_consts::wall`) in
/// the quarter between +x and +y, and it is turned round into each quarter under that
/// quarter's part.
pub(crate) fn pieces(b: &mut MeshBuilder, mut piece: impl FnMut(&mut MeshBuilder, u32)) {
    for q in 0..4 {
        b.with(quarter(q), |b| {
            for case in 0..wall::CASES {
                b.with_part(part::WALL_FIRST + q * wall::CASES + case, |b| {
                    piece(b, case)
                });
            }
        });
    }
}

/// A side profile, bottom to top: how far the face stands out from the wall's middle
/// (`[w, z]`), and the paint of each band from one point to the next, in `pattern`.
/// The top is flat at the last point.
pub(crate) struct Profile {
    pub(crate) points: &'static [[f32; 2]],
    pub(crate) paint: &'static [u32],
    pub(crate) pattern: u32,
}

impl Profile {
    pub(crate) fn top(&self) -> [f32; 2] {
        self.points[self.points.len() - 1]
    }
}

/// A quarter turn `q` times counter-clockwise, exactly: the pieces meet on the lot's edge.
fn quarter(q: u32) -> Affine3A {
    let (c, s) = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][q as usize % 4];
    Affine3A::from_cols(
        Vec3::new(c, s, 0.0).into(),
        Vec3::new(-s, c, 0.0).into(),
        Vec3::Z.into(),
        Vec3::ZERO.into(),
    )
}

/// Mirror across the quarter's diagonal: +x becomes +y and +y becomes +x.
pub(crate) fn diagonal() -> Affine3A {
    Affine3A::from_cols(
        Vec3::Y.into(),
        Vec3::X.into(),
        Vec3::Z.into(),
        Vec3::ZERO.into(),
    )
}

/// The outside faces of a piece: band by band up `profile`, along the plan `outline`
/// the profile's `w` gives. `out` points roughly away from the piece in plan.
pub(crate) fn sides(
    b: &mut MeshBuilder,
    profile: &Profile,
    outline: impl Fn(f32) -> Vec<Vec3>,
    out: Vec3,
) {
    for (band, pair) in profile.points.windows(2).enumerate() {
        let ([w0, z0], [w1, z1]) = (pair[0], pair[1]);
        let (low, high) = (outline(w0), outline(w1));
        b.paint(profile.paint[band]).pattern(profile.pattern);
        for i in 0..low.len() - 1 {
            // This edge's own outward direction in plan, tilted by the band's slope: up
            // for a ledge, down for the underside of the coping.
            let edge = low[i + 1] - low[i];
            let mut side = Vec3::new(edge.y, -edge.x, 0.0).normalize_or_zero();
            if side.dot(out) < 0.0 {
                side = -side;
            }
            let normal = side * (z1 - z0) + Vec3::Z * (w0 - w1);
            face_out(
                b,
                vec![
                    low[i] + Vec3::Z * z0,
                    low[i + 1] + Vec3::Z * z0,
                    high[i + 1] + Vec3::Z * z1,
                    high[i] + Vec3::Z * z1,
                ],
                normal,
            );
        }
    }
}

/// Emits `points` as one flat face, wound so its front faces `out`.
pub(crate) fn face_out(b: &mut MeshBuilder, mut points: Vec<Vec3>, out: Vec3) {
    let mut normal = Vec3::ZERO;
    for (i, p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        normal += p.cross(q);
    }
    if normal.dot(out) < 0.0 {
        points.reverse();
    }
    b.face(&points);
}

/// The neighbours a portrait shows a section with: the end of a wall that runs on
/// toward -x, so it reads as a wall and not a lone pillar.
pub(super) const PORTRAIT: u32 = 1 << 4;

/// The piece quarter `q` draws, given the section's neighbours (`status[2]`).
fn case(q: u32, joins: u32) -> u32 {
    let side = |k: u32| (joins >> (k % 8)) & 1 != 0;
    match (side(2 * q), side(2 * q + 2), side(2 * q + 1)) {
        (false, false, _) => wall::CAP,
        (true, false, _) => wall::RUN_A,
        (false, true, _) => wall::RUN_B,
        (true, true, false) => wall::JOIN,
        (true, true, true) => wall::FULL,
    }
}

/// Whether a vertex of `part` is drawn on a section whose neighbours are `joins`.
/// Anything that is not a wall piece always is.
pub(super) fn shown(part: u32, joins: u32) -> bool {
    let Some(k) = part
        .checked_sub(part::WALL_FIRST)
        .filter(|&k| k < part::WALL_COUNT)
    else {
        return true;
    };
    case(k / wall::CASES, joins) == k % wall::CASES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    /// Triangles drawn with neighbours `joins`.
    fn drawn(mesh: &crate::MeshLod, joins: u32) -> usize {
        mesh.indices
            .chunks(3)
            .filter(|t| shown(mesh.vertices[t[0] as usize].part, joins))
            .count()
    }

    #[test]
    fn each_quarter_draws_one_piece() {
        let model = build_model("wall").unwrap();
        let mesh = &model.lods[0];
        for joins in 0..256 {
            for q in 0..4 {
                let pieces = (0..wall::CASES)
                    .filter(|c| shown(part::WALL_FIRST + q * wall::CASES + c, joins))
                    .count();
                assert_eq!(pieces, 1, "quarter {q} with {joins:08b}");
            }
            assert!(drawn(mesh, joins) > 0);
        }
        for piece in part::WALL_FIRST..part::WALL_FIRST + part::WALL_COUNT {
            assert!(
                mesh.vertices.iter().any(|v| v.part == piece),
                "piece {piece} has a mesh"
            );
        }
    }

    #[test]
    fn a_line_is_a_run_and_a_block_is_filled() {
        let (e, n, w, s, ne) = (1, 1 << 2, 1 << 4, 1 << 6, 1 << 1);
        // East and west: all four quarters run.
        assert_eq!(case(0, e | w), wall::RUN_A);
        assert_eq!(case(1, e | w), wall::RUN_B);
        assert_eq!(case(2, e | w), wall::RUN_A);
        assert_eq!(case(3, e | w), wall::RUN_B);
        // A corner cell of a block: filled toward the block, a pillar's corner outside.
        assert_eq!(case(0, e | n | ne), wall::FULL);
        assert_eq!(case(2, e | n | ne), wall::CAP);
        // A turn without the corner cell is a mitre, however the diagonal lies.
        assert_eq!(case(0, e | n), wall::JOIN);
        assert_eq!(case(3, s | ne), wall::RUN_A);
    }

    /// The pieces meet flush: a run's top is level with the next quarter's, and where a
    /// pillar stands beside a wall it covers the wall's end.
    #[test]
    fn pieces_meet() {
        let model = build_model("wall").unwrap();
        let mesh = &model.lods[0];
        let top = |piece: u32| {
            mesh.vertices
                .iter()
                .filter(|v| v.part == part::WALL_FIRST + piece)
                .map(|v| v.pos[2])
                .fold(0.0f32, f32::max)
        };
        let reach = |piece: u32, axis: usize| {
            mesh.vertices
                .iter()
                .filter(|v| v.part == part::WALL_FIRST + piece)
                .map(|v| v.pos[axis])
                .fold(0.0f32, f32::max)
        };
        assert!(
            top(wall::CAP) > top(wall::RUN_A) + 0.5,
            "pillars stand over the wall"
        );
        assert!(
            (top(wall::FULL) - top(wall::CAP)).abs() < 0.2,
            "a block is pillar high"
        );
        assert!(
            reach(wall::CAP, 1) > reach(wall::RUN_A, 1) + 1.0,
            "pillars are broader"
        );
        // A run reaches the lot's edge, where the next section's carries on.
        assert!((reach(wall::RUN_A, 0) - 6.0).abs() < 1e-3);
        assert!((reach(wall::FULL, 0) - 6.0).abs() < 1e-3);
    }
}
