//! Which queues' order lines are drawn this frame (`OrderMap::draw`).
//!
//! The members of a command group share every leg and waypoint of their line, so after
//! the first of them the rest draw nothing more; with tens of thousands in a group, the
//! pass that found that out leg by leg was most of the interface's frame. Here the queues
//! are sorted out once per pull (and change of selection or shift): one queue for each
//! group's route stands for all of its members, and the frames between draw only those.
//!
//! Units walking on their own each have a line of their own. Every route's first unit is
//! drawn, so every waypoint shows, but past `MAX_LONE` units the rest are drawn only for
//! those nearest the middle of the view.

use crate::fast_hash::IdMap;
use crate::game::View;
use crate::unit_rows::Picked;
use glam::Vec2;
use mc_sim::mirror::{QueuedOrder, UnitOrders};
use mc_sim::tables::OrderKind;
use std::hash::{Hash, Hasher};

/// Most units walking on their own whose lines are drawn besides each route's first: those
/// nearest the middle of the view. A deliberate cosmetic cap: tens of thousands of lines
/// are a carpet that the line budget (`MAX_LINES`) cuts short anyway, and working each of
/// them out cost more than the rest of the interface together.
const MAX_LONE: usize = 2048;

/// The orders a queue's line runs through: production and refits hold up what is queued
/// behind them; a factory's standing orders are its products' way out.
pub(super) fn drawn_route(queue: &UnitOrders) -> impl Iterator<Item = &QueuedOrder> {
    let held_up = |o: &&QueuedOrder| !matches!(o.kind, OrderKind::Produce | OrderKind::Upgrade);
    (queue.orders.iter().take_while(held_up))
        .chain(&queue.standing)
        .take(24)
}

/// A patrol's posts: its line loops back from the last to the first.
fn posts(queue: &UnitOrders) -> impl Iterator<Item = &QueuedOrder> {
    queue.route().filter(|o| o.kind == OrderKind::Patrol)
}

/// What the line of an order is drawn from, but for where the unit itself stands and its
/// place in its group (`offset`), which differ from unit to unit.
fn same_order(a: &QueuedOrder, b: &QueuedOrder) -> bool {
    a.kind == b.kind
        && a.at == b.at
        && a.pos == b.pos
        && a.formation == b.formation
        && a.radius.to_bits() == b.radius.to_bits()
        && a.blueprint == b.blueprint
}

/// Two queues whose lines run the same way (`same_order`), from the same group.
fn same_route(a: &UnitOrders, b: &UnitOrders) -> bool {
    a.orders.first().map(|o| o.formation) == b.orders.first().map(|o| o.formation)
        && drawn_route(a).count() == drawn_route(b).count()
        && drawn_route(a)
            .zip(drawn_route(b))
            .all(|(x, y)| same_order(x, y))
        && posts(a).count() == posts(b).count()
        && posts(a).zip(posts(b)).all(|(x, y)| same_order(x, y))
}

fn route_hash(queue: &UnitOrders) -> u64 {
    let mut h = crate::fast_hash::IdHasher::default();
    queue.orders.first().map(|o| o.formation).hash(&mut h);
    for o in drawn_route(queue) {
        (o.kind as u8, o.at, o.formation, o.blueprint.0).hash(&mut h);
    }
    for o in posts(queue) {
        o.at.hash(&mut h);
    }
    h.finish()
}

/// How a queue's line is drawn.
enum Kind {
    /// From its group's middle, through its group's waypoints: shared by the group.
    Group,
    /// From the unit itself, through waypoints of its own.
    Lone,
    /// Anything else (a factory batch's, or a group's line a unit carries on alone).
    Other,
}

fn kind_of(queue: &UnitOrders) -> Kind {
    if queue.batch.is_some() {
        return Kind::Other;
    }
    let group = queue.orders.first().is_some_and(|o| o.formation != 0);
    let all = |grouped: bool| drawn_route(queue).all(|o| (o.formation != 0) == grouped);
    match (group, all(group)) {
        (true, true) => Kind::Group,
        (false, true) => Kind::Lone,
        _ => Kind::Other,
    }
}

#[derive(Default)]
pub(super) struct Routes {
    /// The `Picked::stamp` and shift they were sorted out for.
    key: Option<(u64, bool)>,
    /// Queues drawn every frame, by index in `status.queues`.
    always: Vec<u32>,
    /// The other units walking on their own: their queue, and where the unit stands.
    lone: Vec<(u32, Vec2)>,
}

impl Routes {
    /// The queues whose lines are drawn this frame, in `status.queues` order, with the
    /// middle of the view at `focus`.
    pub(super) fn drawn(&mut self, view: &View, focus: Vec2) -> Vec<u32> {
        let picked = view.picked();
        let key = (picked.stamp(), view.shift);
        // Never pulled (a view filled by hand): every time, as it may have changed.
        if view.pulls() == 0 || self.key != Some(key) {
            self.key = Some(key);
            self.sort_out(view, &picked);
        }
        let mut out = self.always.clone();
        if self.lone.len() <= MAX_LONE {
            out.extend(self.lone.iter().map(|l| l.0));
        } else {
            let mut near: Vec<(f32, u32)> = (self.lone.iter())
                .map(|&(queue, at)| (at.distance_squared(focus), queue))
                .collect();
            near.select_nth_unstable_by(MAX_LONE, |a, b| a.0.total_cmp(&b.0));
            out.extend(near[..MAX_LONE].iter().map(|n| n.1));
        }
        out.sort_unstable();
        out
    }

    fn sort_out(&mut self, view: &View, picked: &Picked) {
        self.always.clear();
        self.lone.clear();
        let queues = &view.status.queues;
        // Routes met so far, by hash, and the queue first met on each; and the last one met,
        // which the next queue (the next member of the same group, most often) is likely on.
        let mut groups: IdMap<u64, Vec<u32>> = IdMap::default();
        let mut lone: IdMap<u64, Vec<u32>> = IdMap::default();
        let (mut last_group, mut last_lone) = (None::<u32>, None::<u32>);
        let met = |seen: &mut IdMap<u64, Vec<u32>>, last: &mut Option<u32>, i: u32| {
            let queue = &queues[i as usize];
            if let Some(first) = last.filter(|&f| same_route(&queues[f as usize], queue)) {
                return Some(first);
            }
            let firsts = seen.entry(route_hash(queue)).or_default();
            let first = (firsts.iter().copied()).find(|&f| same_route(&queues[f as usize], queue));
            if first.is_none() {
                firsts.push(i);
            }
            *last = Some(first.unwrap_or(i));
            first
        };
        for (i, queue) in queues.iter().enumerate() {
            let Some(row) = view.unit_index(queue.unit_id) else {
                continue;
            };
            if !picked.has(row) && !view.shift {
                continue;
            }
            // No orders, no line.
            if queue.orders.is_empty() && queue.standing.is_empty() {
                continue;
            }
            let i = i as u32;
            match kind_of(queue) {
                Kind::Other => self.always.push(i),
                Kind::Group => {
                    if met(&mut groups, &mut last_group, i).is_none() {
                        self.always.push(i);
                    }
                }
                Kind::Lone => {
                    if met(&mut lone, &mut last_lone, i).is_none() {
                        self.always.push(i);
                    } else {
                        let u = &view.frame.units[row];
                        self.lone.push((i, Vec2::new(u.pos[0], u.pos[1])));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::FxVec2;

    fn order(formation: u64, x: i32) -> QueuedOrder {
        QueuedOrder {
            formation,
            offset: [x as f32, 0.0],
            moving_slot: None,
            formation_phase: 0,
            kind: OrderKind::Move,
            pos: [100.0, 50.0],
            at: FxVec2::from_ints(100, 50),
            blueprint: mc_data::BlueprintId(0),
            radius: 0.0,
        }
    }

    /// A side of `n` units, `formation` for each by its index, all selected.
    fn side(n: u32, formation: impl Fn(u32) -> Option<u64>) -> View {
        let mut view = View::new(0, [[1.0; 3]; mc_core::MAX_PLAYERS], false);
        for id in 0..n {
            let mut u: mc_sim::mirror::UnitInstance = bytemuck::Zeroable::zeroed();
            u.unit_id = id;
            u.pos = [id as f32, 0.0, 0.0];
            view.frame.units.push(u);
            view.index_of.insert(id, id as usize);
            view.status.queues.push(UnitOrders {
                unit_id: id,
                orders: formation(id)
                    .map(|f| order(f, id as i32))
                    .into_iter()
                    .collect(),
                ..Default::default()
            });
        }
        view.selection = (0..n).collect();
        view
    }

    #[test]
    fn a_group_is_drawn_once_and_lone_units_each() {
        // Two groups and their members, interleaved; idle units draw nothing.
        let view = side(10, |id| match id % 3 {
            0 => Some(7),
            1 => Some(9),
            _ => (id < 6).then_some(0),
        });
        let mut routes = Routes::default();
        // 0 and 1 stand for their groups; 2 and 5 walk alone.
        assert_eq!(routes.drawn(&view, Vec2::ZERO), [0, 1, 2, 5]);
    }

    #[test]
    fn past_the_cap_the_lone_units_nearest_the_view_are_drawn() {
        let n = MAX_LONE as u32 + 100;
        let view = side(n, |_| Some(0));
        let mut routes = Routes::default();
        let drawn = routes.drawn(&view, Vec2::new(n as f32, 0.0));
        assert_eq!(drawn.len(), MAX_LONE + 1);
        // The route's first unit, then those nearest the far end, where the view is.
        assert_eq!(drawn[0], 0);
        assert_eq!(drawn[1], n - MAX_LONE as u32);
        assert_eq!(*drawn.last().unwrap(), n - 1);
    }
}
