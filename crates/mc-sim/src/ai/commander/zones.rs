//! Zones (`docs/AI_COMMANDER.md`, "Zones"): the ground the land army can walk,
//! split once around the side's start and every ore field into regions that know
//! their neighbours. Operations pick targets and gauge danger by zone, so one idea
//! of "what this ground is worth" serves raids, defence and scouts alike.
//!
//! The graph depends on the map alone, so it is rebuilt after a snapshot is
//! restored instead of being kept in the state. What changes (fire, enemy mass,
//! what we hold) is read from the world model each think (`ZoneView`).
use super::profile::Target;
use super::reach::{Reach, NODE};
use super::world_model::WorldModel;
use crate::World;
use mc_core::{Fx, FxVec2};
use std::collections::VecDeque;

/// No zone.
pub(in crate::ai) const NONE: u16 = u16::MAX;
/// Seeds nearer than this to a zone already made join it: neighbouring ore fields
/// are one zone.
const MERGE: i32 = 400;
/// Zones a map gets at most (a deliberate cap: a map with more ore fields than this
/// leaves the rest to the zone nearest them).
const MOST_ZONES: usize = 250;
/// Mass a mine point is worth to whoever holds the zone.
pub(in crate::ai) const ORE_VALUE: i32 = 250;

#[derive(Clone, Debug, Default)]
pub(in crate::ai) struct Zone {
    pub centre: FxVec2,
    /// Ore fields in it.
    pub ore: u16,
    /// Neighbouring zones, by id.
    pub adj: Vec<u16>,
}

#[derive(Clone, Debug, Default)]
pub(in crate::ai) struct Zones {
    w: i32,
    h: i32,
    /// The zone of each `reach::NODE` square, `NONE` off the walkable ground.
    id: Vec<u16>,
    pub zones: Vec<Zone>,
}

impl Zones {
    /// Zones seeded at `starts` then `ores` (in that order, so ids are the same on
    /// every machine), grown over the walkable nodes of `reach` a ring at a time.
    pub(in crate::ai) fn build(reach: &Reach, starts: &[FxVec2], ores: &[FxVec2]) -> Zones {
        let (w, h) = reach.dims();
        let n = (w * h).max(0) as usize;
        let mut id = vec![NONE; n];
        let mut zones: Vec<Zone> = Vec::new();
        let mut queue = VecDeque::new();
        for seed in starts.iter().chain(ores) {
            let Some(at) = reach.walkable_near(*seed, 2 * NODE) else {
                continue;
            };
            let node = ((at.y.floor_int() / NODE) * w + at.x.floor_int() / NODE) as usize;
            if id[node] != NONE
                || zones
                    .iter()
                    .any(|z| z.centre.distance(at) < Fx::from_int(MERGE))
            {
                continue;
            }
            if zones.len() >= MOST_ZONES {
                break;
            }
            id[node] = zones.len() as u16;
            zones.push(Zone {
                centre: at,
                ..Zone::default()
            });
            queue.push_back(node);
        }
        while let Some(i) = queue.pop_front() {
            let (x, y) = (i as i32 % w, i as i32 / w);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if !reach.walkable_node(nx, ny) {
                    continue;
                }
                let j = (ny * w + nx) as usize;
                if id[j] == NONE {
                    id[j] = id[i];
                    queue.push_back(j);
                }
            }
        }
        for i in 0..n {
            let (x, y) = (i as i32 % w, i as i32 / w);
            let a = id[i];
            if a == NONE {
                continue;
            }
            for (dx, dy) in [(1, 0), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= w || ny >= h {
                    continue;
                }
                let b = id[(ny * w + nx) as usize];
                if b != NONE && b != a {
                    for (p, q) in [(a, b), (b, a)] {
                        let adj = &mut zones[p as usize].adj;
                        if !adj.contains(&q) {
                            adj.push(q);
                        }
                    }
                }
            }
        }
        for z in &mut zones {
            z.adj.sort_unstable();
        }
        let mut zs = Zones { w, h, id, zones };
        for o in ores {
            if let Some(z) = zs.of(*o) {
                zs.zones[z as usize].ore += 1;
            }
        }
        zs
    }

    /// The zone `p` stands in, or the one beside it when it is on a cliff or shore.
    pub(in crate::ai) fn of(&self, p: FxVec2) -> Option<u16> {
        let (x, y) = (p.x.floor_int() / NODE, p.y.floor_int() / NODE);
        (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter_map(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                (nx >= 0 && ny >= 0 && nx < self.w && ny < self.h)
                    .then(|| self.id[(ny * self.w + nx) as usize])
                    .filter(|&z| z != NONE)
                    .map(|z| (dx.abs() + dy.abs(), dy, dx, z))
            })
            .min()
            .map(|(.., z)| z)
    }
}

/// What the side believes of each zone this think.
pub(in crate::ai) struct ZoneView {
    /// Land fire on the zone's centre.
    pub threat: Vec<Fx>,
    /// Enemy mass remembered in the zone (the commander counted heavily).
    pub enemy: Vec<Fx>,
    /// Mass of our structures standing in the zone: what a nuke would cost us there.
    pub held: Vec<Fx>,
}

impl ZoneView {
    /// Mass the zone is worth: its ore and what the enemy has standing in it.
    pub(in crate::ai) fn value(&self, zones: &Zones, z: u16) -> Fx {
        Fx::from_int(ORE_VALUE * i32::from(zones.zones[z as usize].ore)) + self.enemy[z as usize]
    }

    /// For each zone, the most land fire on any zone along the fewest-hops way to it
    /// from `from`, both ends included (the lower id first on ties); zero where no
    /// way leads.
    pub(in crate::ai) fn fire_along(&self, zones: &Zones, from: u16) -> Vec<Fx> {
        let mut along = vec![Fx::ZERO; zones.zones.len()];
        let mut reached = vec![false; zones.zones.len()];
        let Some(first) = along.get_mut(from as usize) else {
            return along;
        };
        *first = self.threat[from as usize];
        reached[from as usize] = true;
        let mut queue = VecDeque::from([from]);
        while let Some(z) = queue.pop_front() {
            for &a in &zones.zones[z as usize].adj {
                if !reached[a as usize] {
                    reached[a as usize] = true;
                    along[a as usize] = along[z as usize].max(self.threat[a as usize]);
                    queue.push_back(a);
                }
            }
        }
        along
    }
}

impl World {
    /// The zone view for `player` from its world model and what it remembers.
    pub(in crate::ai) fn zone_view(&self, player: u8, zones: &Zones, wm: &WorldModel) -> ZoneView {
        let n = zones.zones.len();
        let ai = &self.state.ai[player as usize];
        let mut enemy = vec![Fx::ZERO; n];
        for c in &ai.contacts {
            if let Some(z) = zones.of(c.pos) {
                let bp = self.blueprints.unit(c.blueprint);
                enemy[z as usize] += if bp.has(mc_data::cat::COMMANDER) {
                    Fx::from_int(5000)
                } else {
                    bp.cost_mass
                };
            }
        }
        let mut held = vec![Fx::ZERO; n];
        let units = &self.state.units;
        for r in units.slots.iter() {
            if units.owner[r] == player && self.bp(r).is_structure() {
                if let Some(z) = zones.of(units.pos[r]) {
                    held[z as usize] += self.bp(r).cost_mass;
                }
            }
        }
        ZoneView {
            threat: zones
                .zones
                .iter()
                .map(|z| wm.threat_at(z.centre, Target::Land))
                .collect(),
            held,
            enemy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i32, y: i32) -> FxVec2 {
        FxVec2::from_ints(x * NODE + NODE / 2, y * NODE + NODE / 2)
    }

    /// Two rooms joined by a corridor, a wall room cut off on the right.
    fn map() -> Reach {
        Reach::from_rows(&[
            "###########.##",
            "###########.##",
            "##.########.##",
            "###########.##",
            "###########.##",
        ])
    }

    #[test]
    fn zones_split_the_ground_between_their_seeds_and_know_their_neighbours() {
        let zs = Zones::build(&map(), &[at(0, 0)], &[at(10, 4)]);
        assert_eq!(zs.zones.len(), 2);
        assert_eq!(zs.of(at(1, 1)), Some(0));
        assert_eq!(zs.of(at(9, 3)), Some(1));
        assert_eq!(zs.zones[0].adj, vec![1]);
        assert_eq!(zs.zones[1].adj, vec![0]);
        // The strip past the wall is cut off from home: no zone.
        assert_eq!(zs.of(at(13, 2)), None);
    }

    #[test]
    fn seeds_close_together_make_one_zone_that_counts_both_ore_fields() {
        let zs = Zones::build(&map(), &[at(0, 0)], &[at(1, 1), at(2, 1)]);
        assert_eq!(zs.zones.len(), 1);
        assert_eq!(zs.zones[0].ore, 2);
    }
}

#[cfg(test)]
mod view_tests {
    use super::*;

    #[test]
    fn fire_along_carries_the_worst_zone_on_the_way() {
        // 0 - 1 - 2, fire on the middle one only.
        let zones = Zones {
            zones: vec![
                Zone {
                    adj: vec![1],
                    ..Zone::default()
                },
                Zone {
                    adj: vec![0, 2],
                    ..Zone::default()
                },
                Zone {
                    adj: vec![1],
                    ..Zone::default()
                },
            ],
            ..Zones::default()
        };
        let view = ZoneView {
            threat: vec![Fx::ZERO, Fx::from_int(7), Fx::ZERO],
            enemy: vec![Fx::ZERO; 3],
            held: vec![Fx::ZERO; 3],
        };
        assert_eq!(
            view.fire_along(&zones, 0),
            vec![Fx::ZERO, Fx::from_int(7), Fx::from_int(7)]
        );
        assert_eq!(view.fire_along(&zones, 1)[0], Fx::from_int(7));
    }
}
