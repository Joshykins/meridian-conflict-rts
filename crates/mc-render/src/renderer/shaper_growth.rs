//! Survival's Shapers drawn as they grow (mc-sim `survival::ShaperInstance`,
//! mc-models `shaper`). A Shaper's blueprint has one model, its level 1 stage; this
//! draws it instead with the stage its level has reached (a model slot of its own each,
//! built at its true size), and while it climbs toward that stage's height, cut at the
//! height it is built up to: the top still to come shows as a lattice of replication
//! light over a band hot from the print (entity.wgsl `growth_keeps`). A Shaper reseeded
//! straight to a high level is seen printing up from the ground.
//!
//! Its halo (mc-models `shaper/halo.rs`) shows the interceptors it holds from its unit's
//! launcher word (`status[2]`); this hands the shader the spent ones as they lift off
//! (`gpu_consts::halo`).
//!
//! Presentation only: it changes the copy of the instances the GPU draws, never the
//! mirror.

use std::borrow::Cow;

use bytemuck::Zeroable;

use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK};
use mc_sim::nukes::{LAUNCHER_MARK, LAUNCHER_STOCK_MASK};
use mc_sim::survival::ShaperInstance;

use crate::models::gpu_consts::halo::LIFT_TICKS;
use crate::models::shaper::{self, STAGES, STAGE_LEVELS};

/// The kinds of Shaper (`ShaperInstance::kind`): printer, artillery, nuke, fleet.
pub(super) const KINDS: usize = 4;

pub(super) struct ShaperGrowth {
    /// The model slot (index into `ModelInfo`s) of each kind's first stage; its later
    /// stages follow it in `STAGE_LEVELS` order.
    first: [u32; KINDS],
    /// Each Shaper's line last tick, so it glides between ticks: (unit id, metres).
    lines: Vec<(u32, f32)>,
    /// A stand-in for each stage slot drawn this tick, so the draw lists keep it
    /// (`ActiveDraws::update`).
    drawn: Vec<UnitInstance>,
    /// Each Shaper's halo last tick.
    halos: Vec<Halo>,
}

/// A Shaper's halo: the interceptors it held last tick, and its blades lifting off.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Halo {
    unit: u32,
    stock: u32,
    /// What it held before the spend whose blades are lifting off; 0 none.
    from: u32,
    /// Ticks since that spend.
    lift: u32,
}

impl Halo {
    /// Its halo this tick, from last tick's (`was`) and the interceptors it holds now.
    fn next(was: Option<Halo>, unit: u32, stock: u32) -> Halo {
        let mut h = Halo {
            unit,
            stock,
            from: 0,
            lift: 0,
        };
        let Some(was) = was else {
            return h;
        };
        if stock < was.stock {
            // Spent: from what it held (or what it held when the last lift began) the
            // blades lift off afresh.
            h.from = was.from.max(was.stock);
        } else if was.from > stock && was.lift + 1 < LIFT_TICKS {
            h.from = was.from;
            h.lift = was.lift + 1;
        }
        h
    }

    /// `UnitInstance::twin_spin`: what it held before the spend plus how far the lift
    /// has gone, last tick and this (`gpu_consts::halo`); zero with nothing lifting.
    fn twin_spin(&self) -> [f32; 2] {
        if self.from == 0 {
            return [0.0; 2];
        }
        // At most 255 rounds: exact in f32.
        let base = self.from as f32;
        let step = |t: u32| (t as f32 / LIFT_TICKS as f32).min(0.999);
        [base + step(self.lift), base + step(self.lift + 1)]
    }
}

impl ShaperGrowth {
    pub(super) fn new(first: [u32; KINDS]) -> Self {
        ShaperGrowth {
            first,
            lines: Vec::new(),
            drawn: Vec::new(),
            halos: Vec::new(),
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
            self.halos.clear();
            return units;
        }
        let mut units = units;
        let mut lines = Vec::with_capacity(shapers.len());
        let mut halos = Vec::with_capacity(shapers.len());
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
            if u.status[2] & LAUNCHER_MARK != 0 {
                let was = self.halos.iter().find(|h| h.unit == s.unit).copied();
                let halo = Halo::next(was, s.unit, u.status[2] & LAUNCHER_STOCK_MASK);
                u.twin_spin = halo.twin_spin();
                halos.push(halo);
            }
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
        self.halos = halos;
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
                (3, SiteKind::Fleet),
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
    fn the_halos_slots_follow_the_sims_interceptors() {
        let root =
            std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
        let bps = mc_data::Blueprints::load(&root.join("data")).unwrap();
        for key in [
            "replication_node",
            "progenitor_seedcaster",
            "progenitor_unmaker",
        ] {
            let bp = bps.unit_by_key(key).unwrap();
            for level in 1..=40u16 {
                let sim = mc_sim::growth::grown_launcher(bp, level).unwrap().stock;
                assert_eq!(
                    shaper::interceptors(level),
                    usize::from(sim),
                    "{key} level {level}"
                );
            }
        }
    }

    #[test]
    fn a_spent_interceptor_lifts_off_and_goes() {
        let mut halo = Halo::next(None, 7, 3);
        assert_eq!(halo.twin_spin(), [0.0; 2]);
        // One spent: from 3 it lifts off, a step a tick.
        halo = Halo::next(Some(halo), 7, 2);
        assert_eq!((halo.from, halo.lift), (3, 0));
        let [a, b] = halo.twin_spin();
        assert!(a == 3.0 && b > a && b < 4.0);
        for tick in 1..LIFT_TICKS {
            halo = Halo::next(Some(halo), 7, 2);
            assert_eq!((halo.from, halo.lift), (3, tick));
            let [a, b] = halo.twin_spin();
            assert!(a.floor() == 3.0 && b.floor() == 3.0 && b > a);
        }
        // Gone.
        halo = Halo::next(Some(halo), 7, 2);
        assert_eq!(halo.twin_spin(), [0.0; 2]);
        // Another spent mid-lift starts the lift afresh from what it held before both.
        halo = Halo::next(Some(halo), 7, 1);
        halo = Halo::next(Some(halo), 7, 0);
        assert_eq!((halo.from, halo.lift), (2, 0));
        // Refilled past it, the lift is over.
        halo = Halo::next(Some(halo), 7, 2);
        assert_eq!(halo.from, 0);
    }

    #[test]
    fn a_shaper_is_drawn_with_its_stage_cut_at_its_height() {
        let mut growth = ShaperGrowth::new([100, 200, 300, 400]);
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
