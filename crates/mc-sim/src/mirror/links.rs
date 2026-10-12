//! Adjacency links for the interface and the ground conduits (`crate::adjacency`), and
//! how hard a fabricator works, for its animation.

use super::World;
use crate::adjacency::Resource;
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
    /// Share of the neighbour's use this provider saves across their shared edge; `None`
    /// on an enemy's link, whose economy the viewer is not told.
    pub share: Option<f32>,
    /// The stretch of lot edge the two share, end to end, metres.
    pub edge: [[f32; 2]; 2],
    /// The provider's lot centre and the neighbour's: the conduit runs from one to the other.
    pub from: [f32; 2],
    pub to: [f32; 2],
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
        let full = f.materials / mc_core::TICKS_PER_SECOND as i32;
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

    /// This tick's links that `viewer` may see: its own side's and its allies', and an
    /// enemy's whose two buildings its team sees (the conduit is on the ground for
    /// anyone to look at), without the share, as an enemy's economy is hidden.
    pub(super) fn write_links(&self, viewer: Option<u8>, out: &mut Vec<LinkView>) {
        out.clear();
        let units = &self.state.units;
        for l in &self.adjacency.links {
            let (Some(p), Some(c)) = (units.row(l.provider), units.row(l.consumer)) else {
                continue;
            };
            let owner = units.owner[p];
            let enemy = viewer.filter(|&v| self.are_enemies(v, owner));
            if let Some(v) = enemy {
                let mask = self.team_mask(v);
                if self.state.fog_enabled && !(self.seen_by(p, mask) && self.seen_by(c, mask)) {
                    continue;
                }
            }
            let (pbp, cbp) = (self.bp(p), self.bp(c));
            out.push(LinkView {
                provider: l.provider.0,
                consumer: l.consumer.0,
                provider_blueprint: pbp.id,
                consumer_blueprint: cbp.id,
                owner,
                resource: l.resource,
                share: enemy.is_none().then(|| l.share.to_f32()),
                edge: [l.edge.0.to_f32(), l.edge.1.to_f32()],
                from: units.pos[p].to_f32(),
                to: units.pos[c].to_f32(),
            });
        }
    }
}
