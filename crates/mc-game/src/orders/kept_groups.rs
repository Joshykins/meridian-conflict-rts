//! Command groups as the player made them (`OrderMap::kept`): brought up to date when the
//! sim has something new (a pull) or units were ordered, and where each one's members
//! stand. A group of tens of thousands was looked up member by member every frame;
//! now once per pull, with the frames between drawing it from the sums kept.

use super::{badge_radius, muster, Field, Group, OrderMap};
use crate::fast_hash::{IdMap, IdSet};
use crate::game::View;
use glam::{Vec2, Vec3};
use mc_sim::tables::OrderKind;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct GroupsKept {
    /// The pull the groups were last brought up to date for.
    at: Option<u64>,
    /// Counts the times they were: what `found` was found for.
    runs: u64,
    found: RefCell<Found>,
}

/// The kept groups' members as found in the frame.
#[derive(Default)]
struct Found {
    /// The pull, `GroupsKept::runs` and the selection (`Picked::stamp`) it is for.
    key: Option<(u64, u64, u64)>,
    groups: Vec<FoundGroup>,
}

struct FoundGroup {
    formation: u64,
    /// Where its members were at the tick before and are at this one, summed: the
    /// middle between them is the sum's, divided by how many.
    was: Vec3,
    now: Vec3,
    members: Rc<[u32]>,
    selected: bool,
}

impl OrderMap {
    /// `keep_groups`, when there is anything new to bring them up to date with.
    pub(super) fn keep_groups_now(&mut self, view: &View) {
        let pulls = view.pulls();
        // Never pulled (a view filled by hand): every time, as it may have changed.
        if pulls == 0 || self.groups_kept.at != Some(pulls) || !self.ordered.get_mut().is_empty() {
            self.keep_groups(view);
            self.groups_kept.at = Some(pulls);
        }
    }

    /// Brings the kept groups up to date: a unit whose current order belongs to a group is
    /// in that group, and out of any other; one that was ordered elsewhere leaves; an idle
    /// one stays where it was. The dead, and groups of fewer than two, drop away.
    pub(super) fn keep_groups(&mut self, view: &View) {
        self.groups_kept.runs += 1;
        // The group each unit is in (one at most), looked up by unit: a side of thousands
        // in a few big groups costs a pass over its queues, not a search of every group's
        // members for every unit.
        let mut group_of: IdMap<u32, u64> = IdMap::default();
        group_of.reserve(self.kept.values().map(Vec::len).sum());
        for (&formation, members) in &self.kept {
            group_of.extend(members.iter().map(|&id| (id, formation)));
        }
        for id in self.ordered.take() {
            if let Some(formation) = group_of.remove(&id) {
                self.left.insert((id, formation));
            }
        }
        // Units that joined a group just now, in the order met: they go on the end of it.
        let mut joining: Vec<(u64, u32)> = Vec::new();
        for queue in &view.status.queues {
            let Some(front) = queue.orders.first() else {
                continue;
            };
            let id = queue.unit_id;
            let joined = front.formation != 0
                && !matches!(front.kind, OrderKind::Produce | OrderKind::Upgrade)
                && !self.left.contains(&(id, front.formation));
            if joined {
                if group_of.insert(id, front.formation) != Some(front.formation) {
                    joining.push((front.formation, id));
                }
            } else if front.formation == 0 {
                // Walking on its own now: out of every group.
                group_of.remove(&id);
            }
        }
        group_of.retain(|&id, _| view.unit_index(id).is_some());
        for (formation, members) in self.kept.iter_mut() {
            members.retain(|id| group_of.get(id) == Some(formation));
        }
        for (formation, id) in joining {
            if group_of.get(&id) == Some(&formation) {
                self.kept.entry(formation).or_default().push(id);
            }
        }
        self.kept.retain(|_, members| members.len() > 1);
        let live: IdSet<u64> = self.kept.keys().copied().collect();
        self.left
            .retain(|&(id, formation)| live.contains(&formation) && view.unit_index(id).is_some());
    }

    /// Each kept command group: the middle of its members (the world, interpolated) and who
    /// they are; and each batch forming at a factory, a group like any other.
    pub(super) fn groups(&self, field: &Field, alpha: f32) -> Vec<Group> {
        let view = field.view;
        let middle_of = |members: &[u32]| {
            let mut middle = Vec3::ZERO;
            let mut found = Vec::new();
            for &id in members {
                let Some(unit) = view.unit(id) else {
                    continue;
                };
                middle += Vec3::from(unit.prev_pos).lerp(Vec3::from(unit.pos), alpha);
                found.push(id);
            }
            (middle / found.len().max(1) as f32, found)
        };
        let batches = muster::batch_groups(field, &middle_of);
        let mut found = self.groups_kept.found.borrow_mut();
        let key = (view.pulls(), self.groups_kept.runs, view.picked().stamp());
        if view.pulls() == 0 || found.key != Some(key) {
            found.key = Some(key);
            found.groups = self.find_groups(view);
        }
        found
            .groups
            .iter()
            .filter(|g| g.members.len() > 1)
            .map(|g| Group {
                formation: g.formation,
                middle: g.was.lerp(g.now, alpha) / g.members.len() as f32,
                members: g.members.clone(),
                selected: g.selected,
                batch: None,
            })
            .chain(batches)
            .collect()
    }

    /// The kept groups' members found in the frame, and where they are.
    fn find_groups(&self, view: &View) -> Vec<FoundGroup> {
        (self.kept.iter())
            .map(|(&formation, members)| {
                let (mut was, mut now) = (Vec3::ZERO, Vec3::ZERO);
                let mut found = Vec::with_capacity(members.len());
                for &id in members {
                    let Some(unit) = view.unit(id) else {
                        continue;
                    };
                    was += Vec3::from(unit.prev_pos);
                    now += Vec3::from(unit.pos);
                    found.push(id);
                }
                FoundGroup {
                    formation,
                    was,
                    now,
                    selected: found.iter().any(|id| view.selection.contains(id)),
                    members: found.into(),
                }
            })
            .collect()
    }

    /// The group whose count badge is under `cursor` (pixels): who a click there selects.
    pub fn group_at(&self, field: &Field, cursor: Vec2) -> Option<Vec<u32>> {
        let scale = self.scale.max(0.4);
        self.groups(field, 1.0)
            .into_iter()
            .filter(|g| !g.members.is_empty())
            .filter_map(|g| {
                let p = field.camera.project(g.middle)?;
                let d = p.distance(cursor) / scale;
                (d <= badge_radius(g.members.len()) + 3.0).then_some((d, g.members))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, members)| members.to_vec())
    }
}
