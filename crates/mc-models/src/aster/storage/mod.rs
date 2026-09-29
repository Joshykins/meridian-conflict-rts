//! The storage structures: the Capacitor Bank and the Materials Vault. Each shows its
//! side's store (`gpu_consts::store`): fill pieces light as the store fills, and status
//! lamps go amber while it drains, red when it is dry and green when it is full.

pub(super) mod bank;
pub(super) mod vault;

use glam::{Vec2, Vec3};

use super::parts::v3;
use crate::builder::MeshBuilder;
use crate::gpu_consts::store;
use crate::material::*;
use crate::part;

/// Emits `f` as a fill piece: lit while the side's store is at least `at` full (0 to 1),
/// dark below that. Its authored material is how it looks when the store is not known.
fn fill(b: &mut MeshBuilder, at: f32, f: impl FnOnce(&mut MeshBuilder)) {
    let top = (store::FILL_LEVELS - 1) as f32;
    let level = (at * store::FILL_LEVELS as f32 - 0.5)
        .round()
        .clamp(0.0, top) as u32;
    b.with_part(part::STORE_FILL_FIRST + level, f);
}

/// A status lamp standing at `at`: a dark housing under a domed lens of radius `r`,
/// dark glass until the store lights it. Far off, the lens alone as a disc.
fn status_lamp(b: &mut MeshBuilder, at: Vec3, r: f32) {
    if b.coarse() {
        b.with_part(part::STORE_LAMP, |b| {
            b.paint(GLASS);
            b.decal(at + v3(0.0, 0.0, 0.1), Vec2::splat(r * 2.0));
        });
        return;
    }
    let sides = b.sides(8);
    b.paint(ACCENT);
    b.prism(at, sides, r * 1.35, r * 1.2, r * 0.7);
    b.with_part(part::STORE_LAMP, |b| {
        b.paint(GLASS);
        b.prism(at + v3(0.0, 0.0, r * 0.7), sides, r, r * 0.55, r);
    });
}

#[cfg(test)]
mod tests {
    use crate::gpu_consts::store;
    use crate::{build_model_scaled, part};
    use mc_sim::store_lights as s;

    /// The shader reads the mirror's store word by `gpu_consts::store`'s copy of it.
    #[test]
    fn the_store_word_matches_the_sim() {
        assert_eq!(store::FILL_MASK, s::STORE_FILL_MASK);
        assert_eq!(store::STATE_SHIFT, s::STORE_STATE_SHIFT);
        assert_eq!(store::STATE_MASK, s::STORE_STATE_MASK);
        assert_eq!(store::MARK, s::STORE_MARK);
        assert_eq!(store::NEUTRAL, s::StoreState::Neutral as u32);
        assert_eq!(store::DRAINING, s::StoreState::Draining as u32);
        assert_eq!(store::EMPTY, s::StoreState::Empty as u32);
        assert_eq!(store::FULL, s::StoreState::Full as u32);
        assert_eq!(
            store::PART_LAMP,
            store::PART_FILL_FIRST + store::FILL_LEVELS
        );
        const { assert!(part::CELL_ROUND < store::PART_FILL_FIRST) };
    }

    /// Every tier of both stores has status lamps and a fill gauge whose levels climb
    /// from near empty to near full, at the full and middle levels of detail.
    #[test]
    fn every_tier_has_lamps_and_a_climbing_gauge() {
        for (key, r, heights) in [
            ("storage_energy", 14.2, [10.2, 14.3, 19.4]),
            ("storage_mass", 12.9, [6.3, 10.2, 14.9]),
        ] {
            for (i, h) in heights.into_iter().enumerate() {
                let model = build_model_scaled(key, r, h, i as u8 + 1).unwrap();
                for lod in &model.lods[..2] {
                    let parts: Vec<u32> = lod
                        .vertices
                        .iter()
                        .filter(|v| v.rig & crate::rig::UPGRADE == 0)
                        .map(|v| v.part)
                        .collect();
                    let tag = format!("{key} T{}", i + 1);
                    assert!(parts.contains(&part::STORE_LAMP), "{tag}: no lamps");
                    let levels: Vec<u32> = parts
                        .iter()
                        .filter(|&&p| (part::STORE_FILL_FIRST..part::STORE_LAMP).contains(&p))
                        .map(|p| p - part::STORE_FILL_FIRST)
                        .collect();
                    let (lo, hi) = (levels.iter().min(), levels.iter().max());
                    assert!(
                        lo.is_some_and(|&l| l <= 2) && hi.is_some_and(|&l| l >= 12),
                        "{tag}: gauge levels {lo:?}..{hi:?}"
                    );
                }
            }
        }
    }
}
