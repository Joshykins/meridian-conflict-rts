//! Adjacency links for the interface and the ground conduits (`crate::adjacency`), and
//! how hard a fabricator works, for its animation.

use super::World;
use crate::adjacency::{self, Resource};
use mc_data::BlueprintId;

/// One provider saving one neighbour one resource, as the viewer sees it.
#[derive(Clone, Copy, Debug)]
pub struct LinkView {
    pub provider: u32,
    pub consumer: u32,
    pub provider_blueprint: BlueprintId,
    pub consumer_blueprint: BlueprintId,
    pub owner: u8,
    pub resource: Resource,
    /// Share of the neighbour's use this provider saves on its own, before the cap.
    pub share: f32,
    /// The stretch of lot edge the two share, end to end, metres.
    pub edge: [[f32; 2]; 2],
    /// The provider's lot centre and the neighbour's: the conduit runs from one to the other.
    pub from: [f32; 2],
    pub to: [f32; 2],
    /// The two go down together (`adjacency::bound`).
    pub bound: bool,
}

impl World {
    /// A finished material fabricator's work this tick, for its animation: 1 at full
    /// output, between when its side is short of energy, 0 paused or with none.
    /// Anyone who sees it may see this (a stalled plant visibly falters); only its own
    /// side sees the pause mark that tells a pause from a blackout.
    pub(super) fn fabricator_work(&self, row: usize) -> Option<f32> {
        let f = self.bp(row).fabricator?;
        if !self.state.units.is_active(row) {
            return Some(0.0);
        }
        let full = f.mass / mc_core::TICKS_PER_SECOND as i32;
        let made = self
            .flows
            .get(row)
            .map_or(mc_core::Fx::ZERO, |fl| fl.made[0]);
        Some(if full > mc_core::Fx::ZERO {
            (made / full).to_f32().clamp(0.0, 1.0)
        } else {
            0.0
        })
    }

    /// This tick's links that `viewer` may see: its own side's and its allies'. An
    /// enemy's are left out, as its economy is.
    pub(super) fn write_links(&self, viewer: Option<u8>, out: &mut Vec<LinkView>) {
        out.clear();
        let units = &self.state.units;
        for l in &self.adjacency.links {
            let (Some(p), Some(c)) = (units.row(l.provider), units.row(l.consumer)) else {
                continue;
            };
            let owner = units.owner[p];
            if viewer.is_some_and(|v| self.are_enemies(v, owner)) {
                continue;
            }
            let (pbp, cbp) = (self.bp(p), self.bp(c));
            out.push(LinkView {
                provider: l.provider.0,
                consumer: l.consumer.0,
                provider_blueprint: pbp.id,
                consumer_blueprint: cbp.id,
                owner,
                resource: l.resource,
                share: l.share.to_f32(),
                edge: [l.edge.0.to_f32(), l.edge.1.to_f32()],
                from: units.pos[p].to_f32(),
                to: units.pos[c].to_f32(),
                bound: adjacency::bound(pbp, cbp),
            });
        }
    }
}

/// What all the links into `consumer` save it of `resource`, capped (`adjacency::MAX_SAVING`).
pub fn link_saving(links: &[LinkView], consumer: u32, resource: Resource) -> f32 {
    let sum: f32 = links
        .iter()
        .filter(|l| l.consumer == consumer && l.resource == resource)
        .map(|l| l.share)
        .sum();
    sum.min(adjacency::MAX_SAVING[resource as usize].to_f32())
}
