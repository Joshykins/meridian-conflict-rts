//! A builder's line: the tiers it climbs through, and what it will be once the
//! upgrades and refits in its queue are done.

use crate::hud::Scene;
use mc_data::{Blueprints, UnitBlueprint};
use mc_sim::mirror::{UnitInstance, UnitOrders};
use mc_sim::tables::OrderKind;

/// What `bp` will be once the tier upgrades and refits in `queue` are done.
pub(crate) fn loadout_after_queue<'a>(
    blueprints: &'a Blueprints,
    queue: Option<&UnitOrders>,
    bp: &'a UnitBlueprint,
) -> &'a UnitBlueprint {
    let mut at = bp;
    for o in queue.iter().flat_map(|q| &q.orders) {
        if o.kind != OrderKind::Upgrade {
            continue;
        }
        if blueprints.kit(o.blueprint).is_some() {
            if let Ok(next) = blueprints.refit_result(at.id, o.blueprint) {
                at = blueprints.unit(next);
            }
        } else if at.upgrades_to == Some(o.blueprint) {
            at = blueprints.unit(o.blueprint);
        }
    }
    at
}

/// What the unit `u` (of `bp`) will be once its queued upgrades are done.
pub(super) fn planned<'a>(
    s: &Scene<'a>,
    u: &UnitInstance,
    bp: &'a UnitBlueprint,
) -> &'a UnitBlueprint {
    loadout_after_queue(s.blueprints, s.queue_of(u), bp)
}

/// The tiers `bp` upgrades through, one after another.
pub(super) fn successors<'a>(s: &Scene<'a>, bp: &UnitBlueprint) -> Vec<&'a UnitBlueprint> {
    let blueprints = s.blueprints;
    let mut line: Vec<&'a UnitBlueprint> = Vec::new();
    let mut next = bp.upgrades_to;
    while let Some(id) = next {
        if line.iter().any(|b| b.id == id) {
            break;
        }
        let b = blueprints.unit(id);
        line.push(b);
        next = b.upgrades_to;
    }
    line
}
