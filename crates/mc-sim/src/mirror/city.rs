//! City structures for the renderer (`crate::city`): how hurt each one is, whether
//! it burns, and when it last changed, so the glass can shatter by damage and the
//! fires and collapses be drawn. Their events are `SimEvent::Impact::on_structure`,
//! `StructureAlight` and `StructureCollapsed`.

use super::World;

/// One city structure that is not whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructureView {
    /// The map prop it is (an index into the map's props, as `props_dead`).
    pub prop: u32,
    /// Health left: 255 whole, 0 down.
    pub health: u8,
    /// `crate::city::BURNING`, `GUTTED` (a fire burned out in it) and `DOWN`.
    pub flags: u8,
    /// The tick it caught fire, its fire went out or it came down (its last
    /// change of `flags`); 0 for one never alight.
    pub since: u32,
}

impl World {
    pub(super) fn write_city(&self, out: &mut Vec<StructureView>) {
        let s = &self.state.city;
        out.clear();
        out.extend(s.touched.iter().map(|&row| {
            let r = row as usize;
            StructureView {
                prop: s.prop[r],
                health: s.health_byte(r),
                flags: s.flags[r]
                    & (crate::city::BURNING | crate::city::GUTTED | crate::city::DOWN),
                since: s.since[r],
            }
        }));
    }
}
