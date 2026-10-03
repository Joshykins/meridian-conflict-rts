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
//! Design candidates, one per file, all built from the kit here:
//! - `drum` (`fabricator~a`): an indexing drum of four cassettes round a squat spindle,
//!   a cross-head stamping them, heat sinks on wedge feet off the diagonals.
//! - `frame` (`fabricator~b`): a press frame of four upright posts on a heavy bed, a
//!   cross-head ram stamping a die in a turning table, heat sinks down two sides.
//! - `carousel` (`fabricator~c`): a carousel of eight cells turning round a finned hub, a
//!   hammer dropping on the hub's head under a capped crown.

pub(super) mod carousel;
pub(super) mod drum;
pub(super) mod frame;

use std::f32::consts::FRAC_PI_4;

use glam::{Vec2, Vec3};

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

/// A chamfered square step from `z0` to `z1`, `half` across: a dark band at its foot,
/// plate above.
pub(super) fn step(b: &mut MeshBuilder, half: f32, chamfer: f32, z0: f32, z1: f32) {
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(
            v3(0.0, 0.0, (z0 + z1) * 0.5),
            v3(half * 2.0, half * 2.0, z1 - z0),
        );
        return;
    }
    let plan = chamfered_rect(v2(half, half), chamfer);
    if !b.fine() {
        b.paint(PLATING);
        b.loft_z(&plan, &[Section::new(z0, 1.0), Section::new(z1, 0.96)]);
        return;
    }
    let band = ((z1 - z0) * 0.25).min(0.5);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(z0, 1.0), Section::new(z0 + band, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(z0 + band, 0.99), Section::new(z1, 0.96)],
    );
}

/// A heat sink along `along` from `at`, `len` long and `w` across, standing on whatever
/// is at `at`: a dark housing, plate fins tall and short in turn under a clamp bar, end
/// caps. Not drawn far off.
pub(super) fn heat_sink(b: &mut MeshBuilder, at: Vec3, along: Vec3, len: f32, w: f32) {
    if b.coarse() {
        return;
    }
    let base = 0.6;
    let fin = w * 0.55;
    b.yawed(at, along.y.atan2(along.x), |b| {
        b.paint(ACCENT);
        b.cuboid_open(v3(len * 0.5, 0.0, base * 0.5), v3(len, w, base));
        if !b.fine() {
            // The middle distance: the fins as one block.
            b.paint(PLATING);
            b.cuboid_open(
                v3(len * 0.5, 0.0, base + fin * 0.5),
                v3(len * 0.86, w * 0.84, fin),
            );
            return;
        }
        let n = 8;
        let pitch = len * 0.86 / n as f32;
        b.paint(PLATING);
        for k in 0..n {
            let x = len * 0.07 + pitch * (k as f32 + 0.5);
            let tall = if k % 2 == 0 { 1.0 } else { 0.78 };
            b.cuboid_open(
                v3(x, 0.0, base + fin * tall * 0.5),
                v3(pitch * 0.34, w * 0.84, fin * tall),
            );
        }
        {
            b.paint(ACCENT);
            b.cuboid(
                v3(len * 0.5, 0.0, base + fin + 0.1),
                v3(len * 0.9, w * 0.12, 0.2),
            );
            for x in [len * 0.035, len * 0.965] {
                b.cuboid_open(v3(x, 0.0, base + fin * 0.5), v3(len * 0.07, w * 0.94, fin));
            }
        }
    });
}

/// A wedge on +x from `x0` to `x1` standing on `z`, `width` across and `height` tall at
/// each end, its top shoulders bevelled.
pub(super) fn wedge(b: &mut MeshBuilder, x0: f32, x1: f32, z: f32, width: Vec2, height: Vec2) {
    let ring = |x: f32, w: f32, h: f32| -> Vec<Vec3> {
        let (s, e) = (w * 0.5, w * 0.16);
        vec![
            v3(x, -s, z),
            v3(x, s, z),
            v3(x, s, z + h * 0.78),
            v3(x, s - e, z + h),
            v3(x, -s + e, z + h),
            v3(x, -s, z + h * 0.78),
        ]
    };
    b.loft(
        &[ring(x0, width.x, height.x), ring(x1, width.y, height.y)],
        true,
        true,
    );
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

/// A capacitor bastion on the deck at `at`, its long side along y: a dark block with a row
/// of cans on it, the grid's power going in. Not drawn far off.
pub(super) fn bastion(b: &mut MeshBuilder, at: Vec3, len: f32, h: f32) {
    if b.coarse() {
        return;
    }
    b.paint(ACCENT);
    if !b.fine() {
        b.cuboid_open(at + Vec3::Z * (h * 0.5), v3(1.8, len, h));
        return;
    }
    b.chamfered_box(at + Vec3::Z * (h * 0.5), v3(1.8, len, h), 0.4);
    let n = (len / 1.4) as usize;
    for k in 0..n {
        let y = (k as f32 - (n as f32 - 1.0) * 0.5) * 1.4;
        b.paint(PLATING);
        b.prism(at + v3(0.0, y, h), 6, 0.5, 0.45, 0.6);
        b.paint(METAL);
        b.prism(at + v3(0.0, y, h + 0.6), 6, 0.2, 0.15, 0.35);
    }
}

/// A box `size` at `center`, its upright edges cut back `chamfer` close to.
pub(super) fn block(b: &mut MeshBuilder, center: Vec3, size: Vec3, chamfer: f32) {
    if b.fine() {
        b.chamfered_box(center, size, chamfer);
    } else {
        b.cuboid(center, size);
    }
}

/// `f` drawn on +x and turned onto each of the four diagonals.
pub(super) fn diagonals(b: &mut MeshBuilder, f: impl Fn(&mut MeshBuilder)) {
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| b.radial(4, &f));
}

#[cfg(test)]
mod tests {
    use super::{LOT, SIZES};
    use crate::{build_model_scaled, part, rig};

    const DESIGNS: [&str; 3] = ["fabricator~a", "fabricator~b", "fabricator~c"];

    /// Tech 2 and 3 stand on one 2x2 lot; tech 2 carries tech 3's machinery as upgrade
    /// pieces (only those reach over its height), tech 3 stands taller, and every one has
    /// a press and an indexer to work in beats.
    #[test]
    fn tech_3_is_tech_2_upgraded_on_one_lot() {
        let half = LOT as f32 * 6.0;
        for key in DESIGNS {
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
