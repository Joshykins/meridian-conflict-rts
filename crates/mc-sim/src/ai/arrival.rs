//! Units that are where they were sent, though their order has not ended.
//!
//! A big group sent to one point never finishes the move: the crowd keeps the
//! units on its edge a hundred metres or more short of the point, pushing, so
//! they never go idle. The army counted only idle units as gathered, so on
//! Serac Divide 190 of 256 units sat in the staging blob under a move order,
//! the staging point looked empty, and no full wave left for twenty minutes.
//! A fleet gathering at its anchorage did the same: on The Axis fifty ships
//! stood within 90 m of it under orders they never finished, and never sailed.
use super::*;
use std::collections::BTreeMap;

/// Metres short of its target a unit sent alone counts as there.
const ARRIVED_SLACK: i32 = 40;
/// More metres per unit of the square root of the group sent to that point:
/// the blob's radius grows with it.
const ARRIVED_PER_ROOT: i32 = 10;

impl World {
    /// Rows of `player`'s land army and fleet with a move or attack-move order whose
    /// target they have reached, as near as the crowd sent there lets them.
    /// Sorted, for a binary search.
    pub(super) fn arrived_army(&self, player: u8) -> Vec<usize> {
        let units = &self.state.units;
        let moving: Vec<(usize, FxVec2)> = units
            .slots
            .iter()
            .filter(|&row| {
                units.owner[row] == player
                    && units.is_active(row)
                    && adaptive::domain(self.bp(row)) != 1
                    && !self.bp(row).weapons.is_empty()
                    && self.bp(row).categories & (cat::ENGINEER | cat::COMMANDER) == 0
            })
            .filter_map(|row| {
                let head = units.order_head[row];
                let o = self.state.orders.order.get(head as usize)?;
                (head != NO_ORDER && matches!(o.kind, OrderKind::Move | OrderKind::AttackMove))
                    .then_some((row, o.pos))
            })
            .collect();
        let mut crowd: BTreeMap<(i64, i64), i32> = BTreeMap::new();
        for &(_, at) in &moving {
            *crowd.entry((at.x.0, at.y.0)).or_insert(0) += 1;
        }
        let mut arrived: Vec<usize> = moving
            .into_iter()
            .filter(|&(row, at)| {
                let n = crowd[&(at.x.0, at.y.0)];
                let slack = ARRIVED_SLACK + ARRIVED_PER_ROOT * n.isqrt();
                units.pos[row].distance(at) <= Fx::from_int(slack)
            })
            .map(|(row, _)| row)
            .collect();
        arrived.sort_unstable();
        arrived
    }
}
