//! Gravitic engines at work, for the renderer and the interface (`RenderFrame::wells`,
//! `crate::gravitic`): each engine gathering, its rift running, or the well it holds.

use super::World;
use crate::gravitic::WellPhase;
use mc_core::{FxVec2, TICKS_PER_SECOND};
use mc_data::BlueprintId;

/// Where an engine's work is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WellStage {
    /// Gathering at the engine; the rift leaves when `progress` reaches one.
    Gathering,
    /// The rift running through the ground; `progress` is the share of the way run.
    Running,
    /// The well holding at the mark.
    Holding,
}

/// One engine's work as the viewer may see it.
#[derive(Clone, Copy, Debug)]
pub struct WellView {
    pub unit_id: u32,
    pub owner: u8,
    pub blueprint: BlueprintId,
    pub stage: WellStage,
    /// Gathering: the share gathered. Running: the share of the way run. Holding: one.
    pub progress: f32,
    /// Seconds into this stage (gathering or running: until it ends, `left`).
    pub seconds: f32,
    pub left: f32,
    /// The held gravity in the engine (world), where the rift leaves the ground, the
    /// rift's front now, and the mark, all on the ground (the sea's surface over water).
    pub core: [f32; 3],
    pub from: [f32; 3],
    pub front: [f32; 3],
    pub mark: [f32; 3],
    /// The well's reach round the mark, metres.
    pub radius: f32,
    /// Its side pays for it: a faltering well (energy stalled, paused, stunned) is dim.
    pub powered: bool,
}

impl World {
    /// Every engine's work the viewer may see: its own side's always, an enemy's where the
    /// viewer sees the engine or the ground the rift's front or the well is on.
    pub(super) fn write_wells(&self, viewer: Option<u8>, out: &mut Vec<WellView>) {
        out.clear();
        let s = &self.state;
        let tps = TICKS_PER_SECOND as f32;
        for w in &s.wells {
            let Some(row) = s.units.row(w.source) else {
                continue;
            };
            let Some(g) = self.bp(row).gravitic.as_ref() else {
                continue;
            };
            let front = w.front();
            if let Some(v) = viewer.filter(|&v| self.are_enemies(v, w.owner)) {
                let mask = self.team_mask(v);
                let seen = !s.fog_enabled
                    || self.fog.is_detected(front, mask)
                    || self.detects_for_team(v, row);
                if !seen {
                    continue;
                }
            }
            let ground = |p: FxVec2| {
                let [x, y] = p.to_f32();
                let z = self
                    .terrain
                    .height_at(p)
                    .max(self.terrain.water_level())
                    .to_f32();
                [x, y, z]
            };
            let step = g.step.to_f32().max(1e-3);
            let (stage, progress, seconds, left) = match w.phase {
                WellPhase::Gathering { left } => {
                    let all = g.gather_ticks.max(1) as f32;
                    let done = all - left as f32;
                    (
                        WellStage::Gathering,
                        done / all,
                        done / tps,
                        left as f32 / tps,
                    )
                }
                WellPhase::Running { run, length } => {
                    let (run, length) = (run.to_f32(), length.to_f32().max(1e-3));
                    let p = (run / length).min(1.0);
                    (
                        WellStage::Running,
                        p,
                        run / step / tps,
                        (length - run).max(0.0) / step / tps,
                    )
                }
                WellPhase::Holding { age } => (WellStage::Holding, 1.0, age as f32 / tps, 0.0),
            };
            let pos = s.units.pos[row];
            let fwd = FxVec2::from_angle(s.units.heading[row]);
            let core = pos + fwd * g.core.x + fwd.perp() * g.core.y;
            let [cx, cy] = core.to_f32();
            out.push(WellView {
                unit_id: w.source.0,
                owner: w.owner,
                blueprint: s.units.blueprint[row],
                stage,
                progress,
                seconds,
                left,
                core: [cx, cy, (s.units.z[row] + g.core.z).to_f32()],
                from: ground(w.from),
                front: ground(front),
                mark: [w.mark.x.to_f32(), w.mark.y.to_f32(), w.z.to_f32()],
                radius: g.radius.to_f32(),
                powered: w.powered,
            });
        }
    }
}
