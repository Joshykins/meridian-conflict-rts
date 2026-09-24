//! Combat rank: a unit keeps a kill count, and five ranks of veterancy.
//!
//! Rank is earned from the share of damage a unit dealt to something that
//! then died. Each rank raises max health and, slightly, how fast it heals.

use crate::tables::*;
use crate::World;
use mc_core::{Fx, TICKS_PER_SECOND};

const DT: i32 = TICKS_PER_SECOND as i32;

/// Highest rank a unit can hold.
pub const VETERANCY_MAX: u8 = 5;

/// Extra max health at this rank: one tenth of the blueprint per rank.
pub fn veterancy_health(base: Fx, level: u8) -> Fx {
    base * (10 + level.min(VETERANCY_MAX) as i32) / 10
}

/// Health regenerated per second at this rank: the blueprint's regen, a little
/// more of it per rank, and a thin sliver of the unit's own hit points.
pub fn veterancy_regen(base_regen: Fx, base_health: Fx, level: u8) -> Fx {
    let level = level.min(VETERANCY_MAX) as i32;
    base_regen * (10 + level) / 10 + base_health * level / 4000
}

/// Kill-equivalents needed to leave `level` for the next. Grows by one each rank.
pub fn veterancy_need(level: u8) -> Fx {
    Fx::from_int(level.min(VETERANCY_MAX.saturating_sub(1)) as i32 + 1)
}

impl World {
    /// Full hit points the unit in `row` can hold, rank included.
    #[inline]
    pub fn unit_max_health(&self, row: usize) -> Fx {
        veterancy_health(self.bp(row).health, self.state.units.veterancy[row])
    }

    pub(crate) fn run_regen(&mut self) {
        for row in self.state.units.slots.iter() {
            if !self.state.units.is_active(row)
                || self.state.units.health[row] <= Fx::ZERO
                || self.state.units.has_flag(row, flag::HURT)
            {
                continue;
            }
            let bp = self.bp(row);
            let level = self.state.units.veterancy[row];
            let regen = veterancy_regen(bp.regen, bp.health, level);
            if regen <= Fx::ZERO {
                continue;
            }
            let max = veterancy_health(bp.health, level);
            let health = &mut self.state.units.health[row];
            if *health < max {
                *health = (*health + regen / DT).min(max);
            }
        }
    }

    pub(crate) fn record_damage(&mut self, victim: usize, source: UnitId, amount: Fx) {
        if amount <= Fx::ZERO {
            return;
        }
        let credits = &mut self.state.units.damage[victim];
        if let Some((_, have)) = credits.iter_mut().find(|(id, _)| *id == source) {
            *have += amount;
        } else {
            credits.push((source, amount));
        }
    }

    /// The victim is dead: the killer takes the kill, and everyone who hurt it
    /// is paid their share of one kill-equivalent.
    pub(crate) fn settle_kill(&mut self, victim: usize, killer: UnitId, killer_player: u8) {
        if (killer_player as usize) < self.state.players.len() {
            self.state.players[killer_player as usize].units_killed += 1;
        }
        let victim_owner = self.state.units.owner[victim];
        if let Some(row) = self.state.units.row(killer) {
            if self.state.units.health[row] > Fx::ZERO
                && self.are_enemies(self.state.units.owner[row], victim_owner)
            {
                self.state.units.kills[row] = self.state.units.kills[row].saturating_add(1);
            }
        }
        let credits = std::mem::take(&mut self.state.units.damage[victim]);
        let total = credits.iter().fold(Fx::ZERO, |a, &(_, d)| a + d);
        if total <= Fx::ZERO {
            return;
        }
        let mut awards: Vec<(usize, Fx)> = credits
            .into_iter()
            .filter(|(id, _)| !id.is_none())
            .filter_map(|(id, dmg)| {
                let row = self.state.units.row(id)?;
                if self.state.units.health[row] <= Fx::ZERO
                    || !self.are_enemies(self.state.units.owner[row], victim_owner)
                {
                    return None;
                }
                Some((row, dmg / total))
            })
            .collect();
        awards.sort_unstable_by_key(|&(row, _)| row);
        for (row, share) in awards {
            self.grant_veterancy(row, share);
        }
    }

    fn grant_veterancy(&mut self, row: usize, share: Fx) {
        if share <= Fx::ZERO {
            return;
        }
        let base = self.bp(row).health;
        let units = &mut self.state.units;
        let mut level = units.veterancy[row];
        if level >= VETERANCY_MAX || units.health[row] <= Fx::ZERO {
            return;
        }
        let mut progress = units.veterancy_progress[row] + share;
        while level < VETERANCY_MAX {
            let need = veterancy_need(level);
            if progress < need {
                break;
            }
            progress -= need;
            let old_max = veterancy_health(base, level);
            level += 1;
            let new_max = veterancy_health(base, level);
            units.health[row] = (units.health[row] + new_max - old_max).min(new_max);
        }
        units.veterancy[row] = level;
        units.veterancy_progress[row] = if level >= VETERANCY_MAX {
            Fx::ZERO
        } else {
            progress
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Controller;
    use crate::world::MapData;
    use crate::{MatchConfig, PlayerSetup};
    use mc_core::{Angle, FxVec2};
    use mc_data::Blueprints;
    use mc_jobs::Pool;
    use mc_map::Heightfield;
    use std::path::Path;
    use std::sync::Arc;

    fn test_world() -> World {
        let blueprints = Arc::new(
            Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
        );
        let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
        let map = MapData {
            name: "vet".into(),
            content_id: 1,
            ore: Vec::new(),
            starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
            props: Vec::new(),
        };
        let player = |name: &str, team| PlayerSetup {
            name: name.into(),
            faction: "Aster".into(),
            ai: Default::default(),
            team,
            controller: Controller::Human,
            start: team,
        };
        let config = MatchConfig {
            seed: 3,
            players: vec![player("you", 0), player("hostile", 1)],
            cheats: true,
            fog: false,
            spawn_commanders: false,
        };
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
    }

    #[test]
    fn five_ranks_raise_health_and_regen() {
        let base = Fx::from_int(1000);
        assert_eq!(veterancy_health(base, 0), base);
        assert_eq!(veterancy_health(base, 5), Fx::from_int(1500));
        assert_eq!(veterancy_health(base, 9), Fx::from_int(1500));
        assert_eq!(veterancy_regen(Fx::ZERO, base, 0), Fx::ZERO);
        assert!(veterancy_regen(Fx::ZERO, base, 5) > Fx::ZERO);
        assert!(
            veterancy_regen(Fx::from_int(10), base, 5) > veterancy_regen(Fx::from_int(10), base, 0)
        );
    }

    #[test]
    fn rank_cost_grows() {
        assert_eq!(veterancy_need(0), Fx::ONE);
        assert_eq!(veterancy_need(4), Fx::from_int(5));
        assert_eq!(veterancy_need(5), Fx::from_int(5));
    }

    #[test]
    fn a_full_kill_promotes_and_a_share_does_not() {
        let mut w = test_world();
        let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
        let a = w
            .spawn_unit(tank, 0, FxVec2::from_ints(500, 512), Angle::ZERO, true)
            .unwrap();
        let b = w
            .spawn_unit(tank, 0, FxVec2::from_ints(540, 512), Angle::ZERO, true)
            .unwrap();
        let victim = w
            .spawn_unit(tank, 1, FxVec2::from_ints(700, 512), Angle::ZERO, true)
            .unwrap();
        let (a_id, b_id) = (w.state.units.id(a), w.state.units.id(b));
        let hp = w.state.units.health[victim];
        w.damage_unit(victim, hp * Fx::ratio(2, 5), 0, a_id);
        w.damage_unit(victim, hp, 0, b_id);
        assert_eq!(w.state.units.kills[b], 1);
        assert_eq!(w.state.units.kills[a], 0);
        assert_eq!(w.state.units.veterancy[a], 0);
        assert_eq!(w.state.units.veterancy[b], 0);
        let (pa, pb) = (
            w.state.units.veterancy_progress[a],
            w.state.units.veterancy_progress[b],
        );
        assert!(
            pb > pa,
            "the larger share should be ahead: {pa:?} vs {pb:?}"
        );
        assert_eq!(pa + pb, Fx::ONE);
        assert_eq!(w.state.players[0].units_killed, 1);
    }

    #[test]
    fn five_solo_kills_reach_the_top_rank_and_raise_health() {
        let mut w = test_world();
        let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
        let shooter = w
            .spawn_unit(tank, 0, FxVec2::from_ints(500, 512), Angle::ZERO, true)
            .unwrap();
        let shooter_id = w.state.units.id(shooter);
        let base = w.bp(shooter).health;
        assert_eq!(w.state.units.health[shooter], base);
        // Triangular: 1+2+3+4+5 = 15 kill-equivalents to rank 5.
        for i in 0..15 {
            let victim = w
                .spawn_unit(
                    tank,
                    1,
                    FxVec2::from_ints(800 + i * 20, 512),
                    Angle::ZERO,
                    true,
                )
                .unwrap();
            let hp = w.state.units.health[victim];
            w.damage_unit(victim, hp, 0, shooter_id);
        }
        assert_eq!(w.state.units.kills[shooter], 15);
        assert_eq!(w.state.units.veterancy[shooter], VETERANCY_MAX);
        assert_eq!(
            w.state.units.health[shooter],
            veterancy_health(base, VETERANCY_MAX)
        );
    }

    #[test]
    fn regen_closes_a_wound() {
        let mut w = test_world();
        let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
        let row = w
            .spawn_unit(tank, 0, FxVec2::from_ints(500, 512), Angle::ZERO, true)
            .unwrap();
        w.state.units.veterancy[row] = VETERANCY_MAX;
        w.state.units.health[row] = w.unit_max_health(row) / 2;
        let before = w.state.units.health[row];
        w.run_regen();
        assert!(w.state.units.health[row] > before);
        assert!(w.state.units.health[row] <= w.unit_max_health(row));
    }
}
