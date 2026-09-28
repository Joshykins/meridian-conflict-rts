//! Walls that join: which of a section's pieces its neighbours call for. The entity
//! shader makes the same choice for every vertex (`wall_piece_shown` in entity.wgsl);
//! this copy draws the portraits and checks the meshes.

use super::part;
use crate::gpu_consts::wall;

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
