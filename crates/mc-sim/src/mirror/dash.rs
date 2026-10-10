//! The dashes under way, for the interface (`RenderFrame::dashes`, mc-game `dash_marks.rs`):
//! each unit charging a dash, gone in one, or just come down with its wave still rolling.

use super::World;
use crate::dash::DashPhase;
use mc_core::TICKS_PER_SECOND;
use mc_data::BlueprintId;

/// Where a dash shown to the interface is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashStage {
    /// Charging where it stands.
    Charging,
    /// Gone, out of the world, until it lands.
    Gone,
    /// Down, its wave rolling out from where it landed.
    Landed,
}

/// One dash under way as the viewer may see it.
#[derive(Clone, Copy, Debug)]
pub struct DashView {
    pub unit_id: u32,
    pub owner: u8,
    pub blueprint: BlueprintId,
    pub stage: DashStage,
    /// Ticks into this stage, and how many it lasts.
    pub ticks: u16,
    pub length: u16,
    /// Where it left from and where it comes down, on the ground.
    pub from: [f32; 3],
    pub to: [f32; 3],
    /// How far its landing's wave reaches (its farthest `dash_only` weapon), metres, and
    /// how fast it rolls, metres a second. Zero for a dash with no wave.
    pub wave: f32,
    pub wave_speed: f32,
}

impl World {
    /// Every dash the viewer may see: its own side's always, an enemy's where the viewer
    /// sees the dasher or the ground it comes down on.
    pub(super) fn write_dashes(&self, viewer: Option<u8>, out: &mut Vec<DashView>) {
        out.clear();
        let s = &self.state;
        for row in s.units.slots.iter() {
            let bp = self.bp(row);
            let Some(dash) = bp.dash else {
                continue;
            };
            let d = &s.units.dash[row];
            let (wave, wave_speed) = bp
                .weapons
                .iter()
                .filter(|w| w.dash_only)
                .filter_map(|w| Some((w.range_max.to_f32(), w.cone?.speed.to_f32())))
                .fold((0.0f32, 0.0f32), |a, b| if b.0 > a.0 { b } else { a });
            let rolled = if wave_speed > 0.0 {
                (wave / wave_speed * TICKS_PER_SECOND as f32).ceil() as u16 + 2
            } else {
                0
            };
            let since = dash.cooldown_ticks.saturating_sub(d.cooldown);
            let (stage, ticks, length) = match d.phase {
                DashPhase::Windup => (DashStage::Charging, d.ticks, dash.windup_ticks),
                DashPhase::Transit => (DashStage::Gone, d.ticks, dash.transit_ticks),
                // Only while its wave is rolling, after a landing (`to` set).
                DashPhase::Ready if d.cooldown > 0 && since <= rolled && rolled > 0 => {
                    (DashStage::Landed, since, rolled)
                }
                DashPhase::Ready => continue,
            };
            let owner = s.units.owner[row];
            if let Some(v) = viewer.filter(|&v| self.are_enemies(v, owner)) {
                let mask = self.team_mask(v);
                let seen = !s.fog_enabled
                    || self.fog.is_detected(d.to, mask)
                    || (stage == DashStage::Charging && self.detects_for_team(v, row));
                if !seen {
                    continue;
                }
            }
            let ground = |p: mc_core::FxVec2| {
                let [x, y] = p.to_f32();
                let z = self
                    .terrain
                    .height_at(p)
                    .max(self.terrain.water_level())
                    .to_f32();
                [x, y, z]
            };
            out.push(DashView {
                unit_id: s.units.id(row).0,
                owner,
                blueprint: s.units.blueprint[row],
                stage,
                ticks,
                length,
                from: ground(d.from),
                to: ground(d.to),
                wave,
                wave_speed,
            });
        }
    }
}
