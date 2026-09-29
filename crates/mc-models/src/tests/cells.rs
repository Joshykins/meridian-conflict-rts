//! Hatched missile cells (`CellBlock`): the model's blocks, the shader's numbering and
//! the unit file's muzzles agree, and each hatch and round sits on its own cell.

use super::triangles;
use crate::{build_model_scaled, material, part, CellBlock, Model};

/// Every unit with a hatched weapon, built as the game draws it, with that weapon.
fn launchers() -> Vec<(String, Model, mc_data::Weapon)> {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let bps = mc_data::Blueprints::load(&data).unwrap();
    bps.units
        .iter()
        .filter_map(|u| {
            let w = u.weapons.iter().find(|w| w.hatch_ticks > 0)?;
            let model =
                build_model_scaled(&u.visual.mesh, u.radius.to_f32(), u.height.to_f32(), u.tech)
                    .unwrap_or_else(|| panic!("{} builds", u.visual.mesh));
            Some((u.key.clone(), model, w.clone()))
        })
        .collect()
}

/// The block and grid cell (i, j) a point on the model stands in, as entity.wgsl finds it:
/// the nearer block's middle, then the nearest cell of its grid.
fn cell_of(blocks: &[CellBlock], x: f32, y: f32) -> (usize, usize, usize) {
    let b = (0..blocks.len())
        .min_by(|&a, &b| {
            let d = |k: usize| (x - blocks[k].centre[0]).hypot(y - blocks[k].centre[1]);
            d(a).total_cmp(&d(b))
        })
        .unwrap();
    let k = &blocks[b];
    let at = |v: f32, c: f32, n: u8| {
        ((v - c) / k.pitch + (n as f32 - 1.0) * 0.5)
            .round()
            .clamp(0.0, n as f32 - 1.0) as usize
    };
    (b, at(x, k.centre[0], k.nx), at(y, k.centre[1], k.ny))
}

#[test]
fn cell_muzzles_stand_in_the_cells_their_model_declares() {
    let launchers = launchers();
    assert!(
        launchers.len() >= 2,
        "the Skyguard and the Atoll launch from cells"
    );
    for (key, model, weapon) in &launchers {
        let blocks = &model.cells;
        assert!(
            !blocks.is_empty(),
            "{key}: a hatched weapon on a model with no cells"
        );
        let total: usize = blocks.iter().map(|b| b.nx as usize * b.ny as usize).sum();
        assert_eq!(weapon.muzzles.len(), total, "{key}: a muzzle per cell");
        assert!(
            weapon.salvo as usize >= total,
            "{key}: a salvo empties every cell"
        );
        assert!(weapon.boost_ticks > 0 && weapon.split, "{key}");
        let bits = CellBlock::gpu(blocks).1;
        for (k, m) in weapon.muzzles.iter().enumerate() {
            let [x, y, z] = [m.x.to_f32(), m.y.to_f32(), m.z.to_f32()];
            let (b, i, j) = cell_of(blocks, x, y);
            let block = &blocks[b];
            let c = i * block.ny as usize + j;
            assert_eq!(
                (bits[2 + b] >> (4 * c)) & 0xF,
                k as u32,
                "{key}: muzzle {k} is in another cell"
            );
            let centre = block.grid_centre(i, j);
            assert!(
                (x - centre[0]).hypot(y - centre[1]) < 0.05,
                "{key}: muzzle {k} at ({x}, {y}) is off its cell's centre {centre:?}"
            );
            // The missile's body (caliber / 0.28 each way) stands in the cell under the deck.
            let half = weapon.caliber / 0.28;
            assert!(z + half <= block.deck + 0.2, "{key}: muzzle {k} at {z}");
        }
    }
}

#[test]
fn hatches_and_rounds_sit_on_their_cells() {
    for (key, model, _) in launchers() {
        let blocks = &model.cells;
        let cells: usize = blocks.iter().map(|b| b.nx as usize * b.ny as usize).sum();
        for (level, lod) in model.lods.iter().enumerate() {
            let mut hatches = vec![false; cells];
            let index = |b: usize, i: usize, j: usize| {
                blocks[..b]
                    .iter()
                    .map(|k| k.nx as usize * k.ny as usize)
                    .sum::<usize>()
                    + i * blocks[b].ny as usize
                    + j
            };
            for v in lod.vertices.iter().filter(|v| v.part == part::CELL_HATCH) {
                let [x, y, z] = v.pos;
                let (b, i, j) = cell_of(blocks, x, y);
                let block = &blocks[b];
                assert!(
                    (block.deck - 0.1..block.deck + 0.8).contains(&z),
                    "{key} LOD{level}: hatch at {z}"
                );
                // Clear of its hinge line and inside its cell.
                let c = block.grid_centre(i, j);
                let (dx, dy) = ((x - c[0]).abs(), (y - c[1]).abs());
                assert!(
                    dx <= block.pitch * 0.5 && dy <= block.pitch * 0.5,
                    "{key} LOD{level}: hatch spills out of its cell"
                );
                hatches[index(b, i, j)] = true;
            }
            if lod.vertices.iter().any(|v| v.part == part::CELL_HATCH) {
                assert!(
                    hatches.iter().all(|&h| h),
                    "{key} LOD{level}: a cell has no hatch"
                );
            }
            let rounds = lod.vertices.iter().filter(|v| v.part == part::CELL_ROUND);
            for v in rounds.clone() {
                let (b, i, j) = cell_of(blocks, v.pos[0], v.pos[1]);
                let c = blocks[b].grid_centre(i, j);
                assert!(
                    v.pos[2] < blocks[b].deck,
                    "{key} LOD{level}: a round stands out"
                );
                assert!(
                    (v.pos[0] - c[0]).abs() <= blocks[b].half
                        && (v.pos[1] - c[1]).abs() <= blocks[b].half,
                    "{key} LOD{level}: a round is outside its cell"
                );
            }
            if level == 0 {
                assert!(rounds.count() > 0, "{key}: no missiles in the cells");
                assert!(hatches.iter().all(|&h| h), "{key}: a cell has no hatch");
            }
        }
    }
}

#[test]
fn skyguard_is_a_fixed_site_on_a_wide_pad() {
    let model = crate::build_model("aa_sam").unwrap();
    for (level, lod) in model.lods.iter().enumerate() {
        assert!(
            lod.vertices.iter().all(|v| v.part != part::TURRET),
            "LOD{level}: nothing yaws"
        );
        assert!(
            lod.vertices.iter().any(|v| v.material == material::TEAM),
            "LOD{level}: no team colour"
        );
        assert!(
            lod.vertices.iter().any(|v| v.pos[0].hypot(v.pos[1]) > 9.0),
            "LOD{level}: pad is too small"
        );
    }
    let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
    assert!(
        full >= mid && mid >= coarse && coarse < 80 && full <= 2600,
        "lod triangles {full}/{mid}/{coarse}"
    );
}
