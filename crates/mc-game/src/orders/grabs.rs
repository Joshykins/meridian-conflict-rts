//! The orders a press with shift could pick up (`OrderMap::pick`), each once. A group's
//! members all carry its waypoints: tens of thousands of copies of a few orders, each put
//! on screen to measure it against the pointer every frame shift was held. Now the
//! copies are set aside once per pull, and only the different orders are measured.

use super::{draggable, key_of, Key};
use crate::fast_hash::IdSet;
use crate::game::View;
use mc_data::BlueprintId;
use mc_sim::tables::OrderKind;

#[derive(Default)]
pub(super) struct Grabs {
    /// The pull they were found for.
    at: Option<u64>,
    /// Each draggable order once, with where it is drawn, in the order first met.
    orders: Vec<(Key, [f32; 2])>,
}

impl Grabs {
    /// The draggable orders of every queue, each `(key, pos)` once and in the order a pass
    /// over the queues meets them first; a structure that has been begun is on the map,
    /// not in the plans, and is left out.
    pub(super) fn orders(&mut self, view: &View) -> &[(Key, [f32; 2])] {
        let pulls = view.pulls();
        // Never pulled (a view filled by hand): every time, as it may have changed.
        if pulls == 0 || self.at != Some(pulls) {
            self.at = Some(pulls);
            self.orders.clear();
            let planned: IdSet<(mc_core::FxVec2, BlueprintId)> = (view.status.plans.iter())
                .map(|p| (p.at, p.blueprint))
                .collect();
            let mut seen: IdSet<(u8, mc_core::FxVec2, BlueprintId, [u32; 2])> = IdSet::default();
            for order in (view.status.queues.iter())
                .flat_map(|q| q.route())
                .filter(|o| draggable(o.kind))
            {
                let key = key_of(order);
                if order.kind == OrderKind::Build && !planned.contains(&(key.at, key.blueprint)) {
                    continue;
                }
                let pos = order.pos.map(f32::to_bits);
                if seen.insert((key.kind as u8, key.at, key.blueprint, pos)) {
                    self.orders.push((key, order.pos));
                }
            }
        }
        &self.orders
    }
}
