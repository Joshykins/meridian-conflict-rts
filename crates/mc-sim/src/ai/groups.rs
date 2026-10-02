//! Helpers for groups of units: their ids for one order, and where a fleet can
//! reach the enemy by water (`commander/ops_air_sea.rs`).
use super::*;

impl World {
    pub(super) fn ids_of(&self, rows: &[usize]) -> Vec<UnitId> {
        rows.iter()
            .take(MAX_COMMAND_UNITS)
            .map(|&r| self.state.units.id(r))
            .collect()
    }

    /// The nearest remembered contact some ship of `fleet` can reach within
    /// weapon range of, on water `sea` reaches.
    pub(super) fn fleet_target(
        &self,
        player: u8,
        fleet: &[usize],
        sea: &super::sea::SeaReach,
        from: FxVec2,
    ) -> Option<FxVec2> {
        self.state.ai[player as usize]
            .contacts
            .iter()
            .filter_map(|c| {
                let enemy = self.blueprints.unit(c.blueprint);
                fleet.iter().find_map(|&row| {
                    let bp = self.bp(row);
                    let motion = bp.motion?;
                    let range = bp
                        .weapons
                        .iter()
                        .filter(|w| w.target_mask & enemy.target_categories() != 0)
                        .map(|w| w.range_max)
                        .max()?;
                    let goal = self
                        .nav
                        .nearest_passable(motion.layer, motion.size_class, c.pos)?;
                    (goal.distance(c.pos) <= range && sea.reaches(goal)).then_some(goal)
                })
            })
            .min_by_key(|p| (p.distance_sq(from), p.x, p.y))
    }

    /// With no contact, the fleet's own water nearest an enemy start.
    pub(super) fn enemy_water(
        &self,
        player: u8,
        sea: &super::sea::SeaReach,
        from: FxVec2,
    ) -> Option<FxVec2> {
        self.state
            .players
            .iter()
            .enumerate()
            .filter(|(i, p)| !p.defeated && self.are_enemies(player, *i as u8))
            .map(|(_, p)| p.start)
            .min_by_key(|s| (s.distance_sq(from), s.x, s.y))
            .and_then(|s| sea.nearest(s))
    }
}
