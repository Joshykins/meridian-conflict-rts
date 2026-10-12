//! Survival's Shapers drawn as they grow (mc-sim `survival::ShaperInstance`,
//! mc-models `shaper`). A Shaper's blueprint has one model, its level 1 stage; this
//! draws it instead with the stage its level has reached (a model slot of its own each,
//! built at its true size), and while it climbs toward that stage's height, cut at the
//! height it is built up to: the top still to come shows as a lattice of replication
//! light over a band hot from the print (entity.wgsl `growth_keeps`). A Shaper reseeded
//! straight to a high level is seen printing up from the ground.
//!
//! Presentation only: it changes the copy of the instances the GPU draws, never the
//! mirror.

use std::borrow::Cow;

use bytemuck::Zeroable;

use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK};
use mc_sim::survival::ShaperInstance;

use crate::models::shaper::{self, STAGES, STAGE_LEVELS};

/// The kinds of Shaper (`ShaperInstance::kind`): printer, artillery, nuke.
pub(super) const KINDS: usize = 3;

pub(super) struct ShaperGrowth {
    /// The model slot (index into `ModelInfo`s) of each kind's first stage; its later
    /// stages follow it in `STAGE_LEVELS` order.
    first: [u32; KINDS],
    /// Each Shaper's line last tick, so it glides between ticks: (unit id, metres).
    lines: Vec<(u32, f32)>,
    /// A stand-in for each stage slot drawn this tick, so the draw lists keep it
    /// (`ActiveDraws::update`).
    drawn: Vec<UnitInstance>,
}

impl ShaperGrowth {
    pub(super) fn new(first: [u32; KINDS]) -> Self {
        ShaperGrowth {
            first,
            lines: Vec::new(),
            drawn: Vec::new(),
        }
    }

    /// Each tick, before the instances go to the GPU: every standing Shaper drawn with
    /// its stage, and cut at its built height while it grows into it.
    pub(super) fn patch<'a>(
        &mut self,
        units: Cow<'a, [UnitInstance]>,
        shapers: &[ShaperInstance],
    ) -> Cow<'a, [UnitInstance]> {
        self.drawn.clear();
        if shapers.is_empty() {
            self.lines.clear();
            return units;
        }
        let mut units = units;
        let mut lines = Vec::with_capacity(shapers.len());
        let list = units.to_mut();
        for s in shapers {
            let Some(u) = list
                .iter_mut()
                .find(|u| u.unit_id == s.unit && u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0)
            else {
                continue;
            };
            let kind = usize::from(s.kind).min(KINDS - 1);
            let stage = shaper::stage(s.level);
            u.blueprint = self.first[kind] + stage as u32;
            let mut stand_in = UnitInstance::zeroed();
            stand_in.blueprint = u.blueprint;
            self.drawn.push(stand_in);
            // Built up to its stage's height: whole. Below it: cut at the line.
            let top = shaper::height(STAGE_LEVELS[stage]);
            let line = s.height.clamp(0.0, top);
            if line >= top - 0.25 || s.raised < 1.0 {
                u.drive_swing = [0.0; 2];
                continue;
            }
            let last = self
                .lines
                .iter()
                .find(|l| l.0 == s.unit)
                .map_or(line, |l| l.1);
            u.drive_swing = [last.max(0.01), line.max(0.01)];
            lines.push((s.unit, line));
        }
        self.lines = lines;
        units
    }

    /// Stand-ins for the stage slots drawn this tick.
    pub(super) fn drawn(&self) -> &[UnitInstance] {
        &self.drawn
    }
}

const _: () = assert!(STAGES == STAGE_LEVELS.len());

#[cfg(test)]
mod tests {
    use super::*;
    use mc_data::survival::SiteKind;

    #[test]
    fn stages_follow_the_sims_heights_feet_and_tiers() {
        for level in 1..=80u16 {
            let sim = mc_sim::survival::shaper_height(level).to_f32();
            assert!(
                (shaper::height(level) - sim).abs() < 0.5,
                "level {level}: {} against the sim's {sim}",
                shaper::height(level)
            );
            for (kind, site) in [
                (0u8, SiteKind::Printer),
                (1, SiteKind::Artillery),
                (2, SiteKind::Nuke),
            ] {
                let foot =
                    mc_sim::survival::shaper_foot(site, mc_sim::survival::shaper_height(level))
                        .to_f32();
                assert!(
                    (shaper::reach(kind, level) - foot).abs() < 0.5,
                    "level {level} {site:?}: the model's foot {} against the sim's {foot}",
                    shaper::reach(kind, level)
                );
            }
            assert_eq!(
                shaper::tier(level),
                usize::from(mc_sim::survival::shaper_tier(level)),
                "level {level}"
            );
        }
    }

    #[test]
    fn a_shaper_is_drawn_with_its_stage_cut_at_its_height() {
        let mut growth = ShaperGrowth::new([100, 200, 300]);
        let mut u = UnitInstance::zeroed();
        u.unit_id = 7;
        u.blueprint = 3;
        let units = vec![u];
        let mut s = ShaperInstance {
            unit: 7,
            kind: 1,
            level: 10,
            height: 500.0,
            raised: 1.0,
            ..Default::default()
        };
        let drawn = growth.patch(Cow::Borrowed(&units), std::slice::from_ref(&s));
        let stage = shaper::stage(10) as u32;
        assert_eq!(drawn[0].blueprint, 200 + stage);
        assert_eq!(drawn[0].drive_swing, [500.0, 500.0]);
        assert_eq!(growth.drawn().len(), 1);
        // Grown to its stage's height: whole.
        s.height = shaper::height(10);
        let drawn = growth.patch(Cow::Borrowed(&units), std::slice::from_ref(&s));
        assert_eq!(drawn[0].drive_swing, [0.0; 2]);
        // The mirror's own copy is never touched.
        assert_eq!(units[0].blueprint, 3);
    }
}
