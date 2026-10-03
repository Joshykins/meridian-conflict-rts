//! The Material Fabricator (mesh `fabricator`): one heavy machine on a 2x2 lot, built at
//! tech 2 and upgraded in place to tech 3. It is drawn in the fusion plants' manner, a
//! four-fold foundation building up to its middle, in plain plate: the matter it makes
//! shows only as small windows in the Materials colour, flashing at each stroke, and its
//! status as small lamps. It works in beats (`gpu_consts::fab`): a press comes down, the
//! matter flashes, the press lifts and an indexer turns on a quarter, faltering when its
//! side is short of energy, at rest with only its lamps lit when paused or without power.
//! Tech 3 is the tech 2 machine with more on it: those pieces ride the tech 2 model as
//! upgrade pieces (`structures::kit`), going up during the refit.
//!
//! The machine (`carousel`): a carousel of eight cells turning round a finned hub, a
//! hammer dropping on the hub's head under a capped crown, built from the kit here.

pub(super) mod carousel;

use glam::Vec3;

use super::parts::*;
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// The (radius, height) of each tier, the blueprints': tech 1 is never built and drawn
/// as tech 2.
pub(super) const SIZES: [(f32, f32); 3] = [(11.0, 16.0), (11.0, 16.0), (11.0, 22.0)];
/// Every tier's lot, in build cells a side.
#[cfg(test)]
const LOT: u32 = 2;

/// Half the plinth, its corner cut, and the top of its deck.
pub(super) const HALF: f32 = 11.6;
const CHAMFER: f32 = 2.6;
pub(super) const DECK: f32 = 0.9;

/// The tier a model drawn at `tech` has: 2 or 3.
pub(super) fn tier_of(tech: u8) -> u8 {
    tech.clamp(2, 3)
}

/// Tech 3's machinery on a building drawn at `tech`: built at tech 3, an upgrade piece
/// going up `at` of the way through the refit at tech 2.
pub(super) fn tech_3(b: &mut MeshBuilder, tech: u8, at: f32, f: impl FnOnce(&mut MeshBuilder)) {
    super::structures::kit(b, tier_of(tech), 3, at, f);
}

/// The plinth out to the lot's edge, the owner's colour on its two flanks (±y).
pub(super) fn plinth(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(v3(0.0, 0.0, DECK * 0.5), v3(HALF * 2.0, HALF * 2.0, DECK));
        team_panel(b, v3(0.0, HALF - 1.2, DECK), v2(4.0, 0.9));
        return;
    }
    let plan = chamfered_rect(v2(HALF, HALF), CHAMFER);
    if !b.fine() {
        b.paint(PLATING);
        b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(DECK, 0.96)]);
        b.mirror_y(|b| team_panel(b, v3(0.0, HALF - 1.2, DECK), v2(4.0, 0.9)));
        return;
    }
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(0.0, 1.02), Section::new(DECK * 0.4, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(DECK * 0.4, 0.99), Section::new(DECK, 0.96)],
    );
    b.mirror_y(|b| team_panel(b, v3(0.0, HALF - 1.2, DECK), v2(4.0, 0.9)));
}

/// A box of matter in the Materials colour, `size` at `center`: lit by the work, flashing
/// at each stroke (`pattern::FAB_MATTER`).
pub(super) fn matter(b: &mut MeshBuilder, center: Vec3, size: Vec3) {
    b.paint(GLOW_MATERIALS).pattern(pattern::FAB_MATTER);
    b.cuboid(center, size);
}

/// A status lamp, `size` at `center`: green at work, blinking amber short of energy, a
/// slow standby glow at rest (`pattern::FAB_LAMP`). Not drawn far off.
pub(super) fn lamp(b: &mut MeshBuilder, center: Vec3, size: Vec3) {
    if b.coarse() {
        return;
    }
    b.paint(GLOW_MATERIALS).pattern(pattern::FAB_LAMP);
    b.cuboid(center, size);
}

#[cfg(test)]
mod tests {
    use super::{LOT, SIZES};
    use crate::{build_model_scaled, part, rig};

    /// Tech 2 and 3 stand on one 2x2 lot; tech 2 carries tech 3's machinery as upgrade
    /// pieces (only those reach over its height), tech 3 stands taller, and every one has
    /// a press and an indexer to work in beats.
    #[test]
    fn tech_3_is_tech_2_upgraded_on_one_lot() {
        let half = LOT as f32 * 6.0;
        {
            let key = "fabricator";
            let mut tops = Vec::new();
            for tech in [2u8, 3] {
                let (r, h) = SIZES[tech as usize - 1];
                let model = build_model_scaled(key, r, h, tech).unwrap();
                for (lod, mesh) in model.lods.iter().enumerate() {
                    let reach = mesh
                        .vertices
                        .iter()
                        .map(|v| v.pos[0].abs().max(v.pos[1].abs()))
                        .fold(0.0, f32::max);
                    assert!(
                        reach <= half && reach >= half * 0.8,
                        "{key} T{tech} lod{lod}: reach {reach}"
                    );
                }
                let fine = &model.lods[0];
                for p in [part::FAB_INDEX, part::FAB_PRESS] {
                    assert!(fine.vertices.iter().any(|v| v.part == p), "{key}: part {p}");
                }
                let upgrades = fine
                    .vertices
                    .iter()
                    .filter(|v| v.rig & rig::UPGRADE != 0)
                    .count();
                assert_eq!(upgrades > 0, tech == 2, "{key} T{tech}: upgrade pieces");
                let top = fine
                    .vertices
                    .iter()
                    .filter(|v| v.rig & rig::UPGRADE == 0)
                    .map(|v| v.pos[2])
                    .fold(0.0, f32::max);
                assert!(top <= h + 0.05, "{key} T{tech}: {top} m tall, not {h}");
                tops.push(top);
            }
            assert!(tops[1] > tops[0] + 4.0, "{key}: tech 3 stands taller");
        }
    }
}
