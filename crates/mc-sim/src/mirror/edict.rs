//! The Edicts at work, for the renderer, the sound and the interface
//! (`RenderFrame::edicts`, `crate::edict`): each ship gathering, or its shot fired, its
//! wave burning and its reload.

use super::World;
use crate::edict::EdictPhase;
use mc_core::{FxVec2, TICKS_PER_SECOND};
use mc_data::BlueprintId;

/// Where a ship's Edict is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdictStage {
    /// Gathering along the hull; it fires when `progress` reaches one.
    Gathering,
    /// Fired `seconds` ago: the blast, the wave and the reload.
    Fired,
}

/// One ship's Edict as the viewer may see it.
#[derive(Clone, Copy, Debug)]
pub struct EdictView {
    pub unit_id: u32,
    pub owner: u8,
    pub blueprint: BlueprintId,
    pub stage: EdictStage,
    /// Gathering: the share gathered. Fired: one.
    pub progress: f32,
    /// Seconds into this stage.
    pub seconds: f32,
    /// Gathering: seconds left at full power. Fired: seconds until it is ready again.
    pub left: f32,
    /// The firing point (world, on the keel), and how far forward of the ship's origin it
    /// lies along the hull, metres (the model's +X).
    pub from: [f32; 3],
    pub along: f32,
    /// The mark, on the ground (the sea's surface over water).
    pub mark: [f32; 3],
    /// The blast's core and reach round the mark, metres.
    pub core: f32,
    pub radius: f32,
    /// The molten wave's full reach, how far it has rolled, and seconds it burns from the
    /// shot (rolling and lingering).
    pub wave: f32,
    pub wave_reach: f32,
    pub wave_life: f32,
    /// Seconds after the shot the ship's other guns stay silent.
    pub quiet: f32,
    /// Its side pays for the gather: a stalled one stands still (energy, paused, stunned).
    pub powered: bool,
}

impl World {
    /// Every Edict at work the viewer may see: its own side's and its allies' always, an
    /// enemy's where the viewer sees the ship, the firing point or the mark.
    pub(super) fn write_edicts(&self, viewer: Option<u8>, out: &mut Vec<EdictView>) {
        out.clear();
        let s = &self.state;
        let tps = TICKS_PER_SECOND as f32;
        for c in &s.edicts {
            let Some(e) = self.blueprints.unit(c.blueprint).edict.as_ref() else {
                continue;
            };
            let row = s.units.row(c.source);
            if let Some(v) = viewer.filter(|&v| self.are_enemies(v, c.owner)) {
                let mask = self.team_mask(v);
                let seen = !s.fog_enabled
                    || self.fog.is_detected(c.from.xy(), mask)
                    || self.fog.is_detected(c.mark, mask)
                    || row.is_some_and(|r| self.detects_for_team(v, r));
                if !seen {
                    continue;
                }
            }
            let (stage, progress, seconds, left, age) = match c.phase {
                EdictPhase::Gathering { left } => {
                    let all = e.gather_ticks.max(1) as f32;
                    let done = all - left as f32;
                    (
                        EdictStage::Gathering,
                        done / all,
                        done / tps,
                        left as f32 / tps,
                        0,
                    )
                }
                EdictPhase::Fired { age } => (
                    EdictStage::Fired,
                    1.0,
                    age as f32 / tps,
                    e.reload_ticks.saturating_sub(age) as f32 / tps,
                    age,
                ),
            };
            let along = row.map_or(0.0, |r| {
                let fwd = FxVec2::from_angle(s.units.heading[r]);
                (c.from.xy() - s.units.pos[r]).dot(fwd).to_f32()
            });
            let wave_reach = match stage {
                EdictStage::Gathering => 0.0,
                EdictStage::Fired => e.wave.reach(age).to_f32(),
            };
            out.push(EdictView {
                unit_id: c.source.0,
                owner: c.owner,
                blueprint: c.blueprint,
                stage,
                progress,
                seconds,
                left,
                from: c.from.to_f32(),
                along,
                mark: c.mark.extend(c.z).to_f32(),
                core: e.blast.core.to_f32(),
                radius: e.blast.radius.to_f32(),
                wave: e.wave.radius.to_f32(),
                wave_reach,
                wave_life: e.wave.life_ticks() as f32 / tps,
                quiet: e.quiet_ticks as f32 / tps,
                powered: c.powered,
            });
        }
    }
}
