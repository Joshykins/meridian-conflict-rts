//! Ground lots under structures, drawn as stains.

use super::*;
use std::collections::HashMap;

/// Two lots closer than this on both axes are the same lot, metres.
const SAME: f32 = 0.5;

/// A lot's cell in a grid of `SAME`-metre cells: two lots within `SAME` of each other
/// on both axes are in the same or neighbouring cells.
fn cell(pos: [f32; 2]) -> (i32, i32) {
    (
        (pos[0] / SAME).floor() as i32,
        (pos[1] / SAME).floor() as i32,
    )
}

impl Renderer {
    /// Ground lots for structures: persisted pours first, then any live site,
    /// ghost or wreck that is not already on that centre. Death does not
    /// remove a lot; the scorch stain is drawn on top of it.
    pub(super) fn structure_pads(
        &self,
        units: &[UnitInstance],
        persisted: &[StainInstance],
        room: usize,
    ) -> Vec<StainInstance> {
        let mut pads = persisted.iter().copied().take(room).collect::<Vec<_>>();
        // Lots already down, by cell: "is there one on this centre?" looks at nine
        // cells, not every lot on the map.
        let mut taken: HashMap<(i32, i32), Vec<[f32; 2]>> = HashMap::new();
        for p in &pads {
            taken.entry(cell(p.pos)).or_default().push(p.pos);
        }
        for u in units {
            if pads.len() >= room || u.owner_flags & (KIND_PROP | STATE_RADAR) != 0 {
                continue;
            }
            let Some(bp) = self.blueprints.units.get(u.blueprint as usize) else {
                continue;
            };
            if !bp.poured_lot() {
                continue;
            }
            let pos = [u.pos[0], u.pos[1]];
            let (cx, cy) = cell(pos);
            let on_it = (-1..=1).any(|dx| {
                (-1..=1).any(|dy| {
                    taken.get(&(cx + dx, cy + dy)).is_some_and(|lots| {
                        lots.iter()
                            .any(|p| (p[0] - pos[0]).abs() < SAME && (p[1] - pos[1]).abs() < SAME)
                    })
                })
            });
            if on_it {
                continue;
            }
            let half = bp.footprint.0.max(bp.footprint.1) as f32 * (BUILD_CELL_M as f32 * 0.5);
            let ghost = u.owner_flags & KIND_GHOST != 0;
            let build = if u.owner_flags & KIND_WRECK != 0 {
                255
            } else {
                (u.build.clamp(0.0, 1.0) * 255.0) as u32
            };
            // A Regency structure stands on a lot of dark machined plate, not a paved one.
            let nanite = self
                .blueprints
                .factions
                .get(bp.faction.0 as usize)
                .is_some_and(|f| f.construction == mc_data::Construction::Nanite);
            taken.entry((cx, cy)).or_default().push(pos);
            pads.push(StainInstance {
                pos,
                radius: half,
                strength_seed: mc_sim::pack_structure_pad(
                    (u.owner_flags & crate::gpu_consts::owner::MASK) as u8,
                    build as u8,
                    u.blueprint as u16,
                    ghost,
                ) | if nanite { mc_sim::PAD_NANITE } else { 0 },
            });
        }
        pads
    }
}
