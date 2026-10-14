//! Who speaks for the selection on the construction panel (`build::draw`).

use super::line::planned;
use crate::hud::{has_flag, Hud, Scene};
use mc_data::{cat, UnitBlueprint};
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::flag;

pub(super) struct Speakers<'a> {
    /// The builder whose menu the panel shows.
    pub(super) builder: Option<(&'a UnitInstance, &'a UnitBlueprint)>,
    /// The first unit that climbs a tier or refits.
    pub(super) upgrader: Option<(&'a UnitInstance, &'a UnitBlueprint)>,
    /// Whether any of them could be a carrier (`wings::carrier_of`).
    pub(super) carriers: bool,
}

pub(super) fn speakers<'a>(
    hud: &mut Hud,
    s: &'a Scene,
    units: &[&'a UnitInstance],
) -> Speakers<'a> {
    // The highest tier of builder in the selection speaks for it, among builders of the
    // first one's kind (engineers, or factories): what it can start, the others help
    // raise. A factory still going up takes its queue already: it starts on it once it
    // stands. Each pass over the units below is made only when one of their blueprints
    // could answer it: thousands of fighters have none of these.
    let builds = (hud.kept.deck).any_kind(s, units, |bp| {
        bp.builder.as_ref().is_some_and(|b| !b.builds.is_empty())
    });
    let upgrades = (hud.kept.deck).any_kind(s, units, |bp| {
        bp.upgrades_to.is_some() || s.blueprints.refit_set(bp.id).is_some()
    });
    let carriers = (hud.kept.deck).any_kind(s, units, |bp| bp.air_wings.is_some());
    let builders: &[&UnitInstance] = if builds { units } else { &[] };
    let mut builder_units = builders.iter().map(|u| (*u, s.bp(u))).filter(|(u, bp)| {
        bp.builder.as_ref().is_some_and(|b| !b.builds.is_empty())
            && (!has_flag(u, flag::UNDER_CONSTRUCTION) || bp.has(cat::FACTORY))
    });
    let builder_unit = builder_units.next().map(|first| {
        let kind = first.1.has(cat::FACTORY);
        let tier = |(u, bp): (&UnitInstance, &UnitBlueprint)| planned(s, u, bp).tech;
        builder_units
            .filter(|(_, bp)| bp.has(cat::FACTORY) == kind)
            .fold(first, |best, b| if tier(b) > tier(best) { b } else { best })
    });
    // A structure still going up queues its tiers already, to start once it stands.
    let upgraders: &[&UnitInstance] = if upgrades { units } else { &[] };
    let upgrader = upgraders.iter().map(|u| (*u, s.bp(u))).find(|(u, bp)| {
        (bp.upgrades_to.is_some() || s.blueprints.refit_set(bp.id).is_some())
            && (!has_flag(u, flag::UNDER_CONSTRUCTION) || !bp.is_mobile())
    });
    Speakers {
        builder: builder_unit,
        upgrader,
        carriers,
    }
}
