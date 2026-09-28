//! The Skyguard's cells: the model, the shader's cell numbering and the unit file's
//! muzzles agree (`gpu_consts::cells`).

use super::triangles;
use crate::gpu_consts::cells::{DECK, HALF, OFFSET};
use crate::{build_model, material, part};

/// The cell the shader numbers a point in (entity.wgsl): corner to opposite corner.
fn cell(x: f32, y: f32) -> u32 {
    let front = x >= 0.0;
    (if front != (y >= 0.0) { 2 } else { 0 }) + u32::from(front)
}

#[test]
fn skyguard_muzzles_stand_in_the_cells_the_shader_numbers() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let bps = mc_data::Blueprints::load(&data).unwrap();
    let bp = bps
        .units
        .iter()
        .find(|u| u.visual.mesh == "aa_sam")
        .expect("the Skyguard");
    let weapon = &bp.weapons[0];
    assert!(weapon.hatch_ticks > 0 && weapon.boost_ticks > 0 && weapon.split);
    assert_eq!(weapon.muzzles.len(), 4);
    assert!(weapon.salvo as usize >= weapon.muzzles.len());
    for (k, m) in weapon.muzzles.iter().enumerate() {
        let [x, y, z] = [m.x.to_f32(), m.y.to_f32(), m.z.to_f32()];
        assert_eq!(cell(x, y), k as u32, "muzzle {k} is in another cell");
        assert!(
            (x.abs() - OFFSET).abs() < 0.05 && (y.abs() - OFFSET).abs() < 0.05,
            "muzzle {k} is off its cell's centre"
        );
        // The missile's body (caliber / 0.28 each way) fills the cell under the deck.
        let half = weapon.caliber / 0.28;
        assert!(
            z + half <= DECK + 0.2 && z - half >= 0.8,
            "muzzle {k} at {z}"
        );
    }
}

#[test]
fn skyguard_hatches_and_rounds_sit_on_their_cells() {
    for key in ["aa_sam", "aa_sam~array", "aa_sam~dome"] {
        let model = build_model(key).unwrap();
        for (level, lod) in model.lods.iter().enumerate() {
            assert!(
                lod.vertices.iter().all(|v| v.part != part::TURRET),
                "{key} LOD{level}: nothing yaws"
            );
            let mut hatches = [false; 4];
            for v in lod.vertices.iter().filter(|v| v.part == part::CELL_HATCH) {
                // Each hatch lies on the deck over its own cell, clear of the hinge line.
                let [x, y, z] = v.pos;
                assert!(
                    (DECK - 0.1..DECK + 0.6).contains(&z),
                    "{key} LOD{level}: hatch at {z}"
                );
                assert!(
                    x.abs() <= OFFSET + HALF + 0.3,
                    "{key} LOD{level}: hatch past its hinge"
                );
                hatches[cell(x, y) as usize] = true;
            }
            assert_eq!(hatches, [true; 4], "{key} LOD{level}: a cell has no hatch");
            let rounds = lod.vertices.iter().filter(|v| v.part == part::CELL_ROUND);
            for v in rounds.clone() {
                assert!(
                    v.pos[2] < DECK,
                    "{key} LOD{level}: a round stands out of its cell"
                );
                assert!(
                    (v.pos[0].abs() - OFFSET).abs() <= HALF
                        && (v.pos[1].abs() - OFFSET).abs() <= HALF,
                    "{key} LOD{level}: a round is outside its cell"
                );
            }
            if level == 0 {
                assert!(rounds.count() > 0, "{key}: no missiles in the cells");
            }
            assert!(
                lod.vertices.iter().any(|v| v.material == material::TEAM),
                "{key} LOD{level}: no team colour"
            );
            assert!(
                lod.vertices.iter().any(|v| v.pos[0].hypot(v.pos[1]) > 9.0),
                "{key} LOD{level}: pad is too small"
            );
        }
        let [full, mid, coarse] = [0, 1, 2].map(|lod| triangles(&model.lods[lod]));
        assert!(
            full >= mid && mid >= coarse && coarse < 80 && full <= 2600,
            "{key} lod triangles {full}/{mid}/{coarse}"
        );
    }
}
