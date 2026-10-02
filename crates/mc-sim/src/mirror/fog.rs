//! What the presentation sees of the fog of war: the viewer's grid, for what
//! has been explored, and every vision disc its side sees through, which the
//! renderer draws round at each unit's place between ticks (renderer/fog_field.rs).

use super::RenderFrame;
use crate::World;
use bytemuck::{Pod, Zeroable};

/// A vision disc for one tick: drawn at `start` at the tick's start and `end` at its end,
/// as the unit seeing it is.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
pub struct VisionDisc {
    pub start: [f32; 2],
    pub end: [f32; 2],
    /// Metres: the unit's vision.
    pub radius: f32,
    pub _pad: [f32; 3],
}

impl World {
    pub(super) fn write_fog(&self, viewer: Option<u8>, frame: &mut RenderFrame) {
        frame.fog.clear();
        frame.vision.clear();
        frame.fog_dims = self.fog.dims();
        frame.fog_mask = 0;
        let (Some(v), true) = (viewer, self.state.fog_enabled) else {
            return;
        };
        let mask = self.team_mask(v);
        frame.fog_mask = mask;
        let (visible, explored) = (self.fog.visible_cells(), self.fog.explored_cells());
        frame.fog.reserve(visible.len() * 2);
        for i in 0..visible.len() {
            // Radar paints blips, not the ground: only vision lights a cell.
            frame.fog.push(if visible[i] & mask != 0 { 255 } else { 0 });
            frame
                .fog
                .push(if explored[i] & mask != 0 { 255 } else { 0 });
        }
        let units = &self.state.units;
        frame.vision.extend(
            self.fog
                .sights()
                .iter()
                .filter(|s| s.mask & mask != 0)
                .map(|s| VisionDisc {
                    start: s.row.map_or(s.pos, |row| units.prev_pos[row]).to_f32(),
                    end: s.pos.to_f32(),
                    radius: s.vision.to_f32(),
                    _pad: [0.0; 3],
                }),
        );
    }
}
