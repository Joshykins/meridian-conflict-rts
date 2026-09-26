//! Wall sections that join the sections beside them into one wall (`cat::WALL`).
//!
//! Presentation only: the neighbours come from what the frame draws, so a section seen
//! at the edge of the fog gives nothing away about the ones behind it. The renderer
//! draws each quarter of a section as the piece its neighbours call for
//! (`mc_render::gpu_consts::wall`).

use std::f32::consts::FRAC_PI_2;

use super::{UnitInstance, KIND_PROP, KIND_WRECK};
use mc_data::{cat, Blueprints};

/// `status[2]` of a joining wall section: bit k is set when the cell k × 45°
/// counter-clockwise from the section's own +x holds a section it joins. The sides are
/// the even bits; a corner bit fills the corner in only with both sides beside it set,
/// so a block of sections is one thick wall.
pub const WALL_JOINS: u32 = 0xFF;

/// The cells round a section in bit order: east, then counter-clockwise.
const AROUND: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

/// Whether `u` is a section that joins others: a one-cell wall.
fn joins(blueprints: &Blueprints, u: &UnitInstance) -> bool {
    u.owner_flags & KIND_PROP == 0
        && blueprints
            .units
            .get(u.blueprint as usize)
            .is_some_and(|b| b.has(cat::WALL) && b.footprint == (1, 1))
}

/// Which sections join each other: one player's (with the previews of the ones it
/// plans), or wrecks.
fn side(u: &UnitInstance) -> u32 {
    u.owner_flags & (0xFF | KIND_WRECK)
}

/// The build cell a section stands in.
fn cell(u: &UnitInstance) -> (i32, i32) {
    let size = mc_map::BUILD_CELL_M as f32;
    (
        (u.pos[0] / size).floor() as i32,
        (u.pos[1] / size).floor() as i32,
    )
}

/// Writes each wall section's neighbours in `units` into its `status[2]`
/// ([`WALL_JOINS`]). `others` are sections it may join that are not written: the walls
/// already standing behind a placement preview.
pub fn join_walls(blueprints: &Blueprints, units: &mut [UnitInstance], others: &[UnitInstance]) {
    let mut cells: Vec<(u32, i32, i32)> = units
        .iter()
        .chain(others)
        .filter(|u| joins(blueprints, u))
        .map(|u| {
            let (x, y) = cell(u);
            (side(u), x, y)
        })
        .collect();
    if cells.is_empty() {
        return;
    }
    cells.sort_unstable();
    for u in units.iter_mut().filter(|u| joins(blueprints, u)) {
        let (x, y) = cell(u);
        let side = side(u);
        let world = AROUND
            .iter()
            .enumerate()
            .filter(|(_, (dx, dy))| cells.binary_search(&(side, x + dx, y + dy)).is_ok())
            .fold(0u32, |bits, (k, _)| bits | 1 << k);
        // Into the section's own frame: its heading in quarter turns, two bits a turn.
        let turn = 2 * ((u.heading / FRAC_PI_2).round() as i32).rem_euclid(4) as u32;
        u.status[2] = ((world >> turn) | (world << (8 - turn))) & WALL_JOINS;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn blueprints() -> Blueprints {
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
    }

    /// A section of `key` for `owner` in build cell (x, y), facing south as structures do.
    fn section(bp: &Blueprints, key: &str, owner: u32, x: i32, y: i32) -> UnitInstance {
        let size = mc_map::BUILD_CELL_M as f32;
        let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
        u.blueprint = bp.unit_by_key(key).unwrap().id.0 as u32;
        u.owner_flags = owner;
        u.pos = [(x as f32 + 0.5) * size, (y as f32 + 0.5) * size, 0.0];
        u.heading = 3.0 * FRAC_PI_2;
        u
    }

    #[test]
    fn neighbours_are_in_the_sections_own_frame() {
        let bp = blueprints();
        // An L: (5, 5) with a section east of it and one north.
        let mut units = vec![
            section(&bp, "aster_wall", 1, 5, 5),
            section(&bp, "aster_wall", 1, 6, 5),
            section(&bp, "aster_wall", 1, 5, 6),
        ];
        join_walls(&bp, &mut units, &[]);
        // Facing south, the section's +x is the world's south: east is its +y (bit 2),
        // north its -x (bit 4).
        assert_eq!(units[0].status[2], 1 << 2 | 1 << 4);
        // Facing east instead, the bits are the world's.
        units[0].heading = 0.0;
        join_walls(&bp, &mut units, &[]);
        assert_eq!(units[0].status[2], 1 | 1 << 2);
    }

    #[test]
    fn only_one_sides_walls_join() {
        let bp = blueprints();
        let mut units = vec![
            section(&bp, "aster_wall", 1, 5, 5),
            section(&bp, "aster_wall", 2, 6, 5),
            section(&bp, "aster_t1_radar", 1, 4, 5),
        ];
        units[0].heading = 0.0;
        join_walls(&bp, &mut units, &[]);
        assert_eq!(units[0].status[2], 0);
        // A preview joins the standing walls behind it, which are not written.
        let standing = [section(&bp, "aster_wall", 1, 5, 4)];
        let mut ghost = [section(
            &bp,
            "aster_wall",
            1 | super::super::KIND_GHOST,
            5,
            5,
        )];
        ghost[0].heading = 0.0;
        join_walls(&bp, &mut ghost, &standing);
        assert_eq!(ghost[0].status[2], 1 << 6);
    }

    #[test]
    fn a_block_fills_its_corners() {
        let bp = blueprints();
        let mut units: Vec<UnitInstance> = (0..4)
            .map(|i| section(&bp, "aster_wall", 1, 5 + i % 2, 5 + i / 2))
            .collect();
        for u in &mut units {
            u.heading = 0.0;
        }
        join_walls(&bp, &mut units, &[]);
        // The south-west section: east, north-east and north.
        assert_eq!(units[0].status[2], 0b111);
    }
}
