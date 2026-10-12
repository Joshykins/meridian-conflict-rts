//! Nanites at work on hulls, for the renderer (`RenderFrame::mends`, `repair_fields`): a
//! unit mending itself with nanite regen (`UnitBlueprint::nanite_regen`), and each repair
//! field with the friends it mends this tick, as `repair_field.rs` picks them.

use super::{MendView, RepairFieldView};
use crate::spatial::kind;
use crate::World;
use mc_core::Fx;

impl World {
    /// What `viewer` may see of it: its own side's and its allies', and an enemy's it
    /// detects.
    fn mend_seen(&self, viewer: Option<u8>, row: usize) -> bool {
        viewer.is_none_or(|v| {
            !self.are_enemies(v, self.state.units.owner[row]) || self.detects_for_team(v, row)
        })
    }

    fn mend_view(&self, row: usize, from: Option<[f32; 3]>) -> MendView {
        let s = &self.state;
        let bp = self.bp(row);
        let [x, y] = s.units.pos[row].to_f32();
        MendView {
            unit_id: s.units.id(row).0,
            pos: [x, y, s.units.z[row].to_f32()],
            radius: bp.radius.to_f32(),
            height: bp.height.to_f32(),
            from,
        }
    }

    pub(super) fn write_mends(
        &self,
        viewer: Option<u8>,
        mends: &mut Vec<MendView>,
        fields: &mut Vec<RepairFieldView>,
    ) {
        mends.clear();
        fields.clear();
        let s = &self.state;
        for row in s.units.slots.iter() {
            let bp = self.bp(row);
            if !(bp.nanite_regen || bp.repair_field.is_some())
                || !s.units.is_active(row)
                || s.units.health[row] <= Fx::ZERO
                || !self.mend_seen(viewer, row)
            {
                continue;
            }
            // As `run_regen` mends it: hurt, hit this tick or not.
            if bp.nanite_regen
                && bp.regen > Fx::ZERO
                && s.units.health[row] < self.unit_max_health(row)
            {
                mends.push(self.mend_view(row, None));
            }
            let Some(field) = bp.repair_field else {
                continue;
            };
            let carrier = self.mend_view(row, None);
            fields.push(RepairFieldView {
                unit_id: carrier.unit_id,
                pos: carrier.pos,
                radius: field.radius.to_f32(),
            });
            // The nanites leave from high on the carrier.
            let from = [
                carrier.pos[0],
                carrier.pos[1],
                carrier.pos[2] + carrier.height * 0.8,
            ];
            let (pos, owner) = (s.units.pos[row], s.units.owner[row]);
            self.index
                .query(pos, field.radius, kind::UNIT | kind::AIRCRAFT, |e| {
                    let t = e.row as usize;
                    // As `run_repair_fields` picks them.
                    if (t != row || field.mends_self)
                        && self.unit_entry_is_current(e)
                        && s.units.is_active(t)
                        && s.units.health[t] > Fx::ZERO
                        && e.pos.distance(pos) <= field.radius
                        && !self.are_enemies(owner, s.units.owner[t])
                        && s.units.health[t] < self.unit_max_health(t)
                    {
                        mends.push(self.mend_view(t, Some(from)));
                    }
                    true
                });
        }
    }
}
