//! Gravitic lenses at work, for the renderer (`RenderFrame::lenses`, `crate::lens`): each
//! lens's front running out to its target. Its strike is the `LensStruck` event.

use super::World;
use mc_core::{FxVec2, TICKS_PER_SECOND};
use mc_data::BlueprintId;

/// One lens's front as the viewer may see it.
#[derive(Clone, Copy, Debug)]
pub struct LensView {
    pub unit_id: u32,
    pub owner: u8,
    pub blueprint: BlueprintId,
    pub weapon: u8,
    /// Seconds since the front was let go.
    pub seconds: f32,
    /// The lens's emitter (world), where the front left.
    pub emitter: [f32; 3],
    /// Where the front has got to.
    pub front: [f32; 3],
    /// The ground under the target, where the well will open (the sea's surface over
    /// water).
    pub ground: [f32; 3],
}

impl World {
    /// Every lens's work the viewer may see: its own side's always, an enemy's where the
    /// viewer sees the lens or the ground under the front.
    pub(super) fn write_lenses(&self, viewer: Option<u8>, out: &mut Vec<LensView>) {
        out.clear();
        let s = &self.state;
        let tps = TICKS_PER_SECOND as f32;
        for t in &s.lenses {
            let Some(row) = s.units.row(t.source) else {
                continue;
            };
            let Some(weapon) = self.bp(row).weapons.get(t.weapon as usize) else {
                continue;
            };
            if weapon.lens.is_none() {
                continue;
            }
            let front = t.front.xy();
            if let Some(v) = viewer.filter(|&v| self.are_enemies(v, t.owner)) {
                let mask = self.team_mask(v);
                let seen = !s.fog_enabled
                    || self.fog.is_detected(front, mask)
                    || self.detects_for_team(v, row);
                if !seen {
                    continue;
                }
            }
            let fwd = FxVec2::from_angle(s.units.heading[row]);
            let m = weapon.muzzle;
            let at = s.units.pos[row] + fwd * m.x + fwd.perp() * m.y;
            let [ex, ey] = at.to_f32();
            let foot = s.units.row(t.target).map_or(front, |r| s.units.pos[r]);
            let [gx, gy] = foot.to_f32();
            let gz = self
                .terrain
                .height_at(foot)
                .max(self.terrain.water_level())
                .to_f32();
            out.push(LensView {
                unit_id: t.source.0,
                owner: t.owner,
                blueprint: s.units.blueprint[row],
                weapon: t.weapon,
                seconds: t.age as f32 / tps,
                emitter: [ex, ey, (s.units.z[row] + m.z).to_f32()],
                front: t.front.to_f32(),
                ground: [gx, gy, gz],
            });
        }
    }
}
