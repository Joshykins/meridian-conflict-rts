//! A capital ship's attack (`World::capital_engage`): it holds where its guns reach and
//! closes straight in otherwise, out of `orders.rs`, whose private helpers it shares.

use super::*;

impl World {
    /// A capital ship (`is_capital_ship`) never wheels about a target: its turrets
    /// cover every side. It holds where it is the moment any of its guns can hit the
    /// target, at full range, and otherwise closes straight in until one can. Only
    /// the guns that can strike the target set how far in it comes, so a chase after
    /// something only the turrets take closes to the turrets' reach.
    pub(super) fn capital_engage(&mut self, row: usize, target: usize) -> Result<(), SimError> {
        let bp = self.blueprints.clone();
        let weapons = &bp.unit(self.state.units.blueprint[row]).weapons;
        if weapons
            .iter()
            .any(|weapon| self.is_valid_target(row, target, weapon))
        {
            self.capital_hold(row, self.state.units.pos[target]);
            return Ok(());
        }
        // In on the nearest of its hull: a long hull's side, not its middle a kilometre
        // past it (`World::hull_near`).
        let (at, half) = self.hull_near(self.state.units.pos[row], target);
        let reach = weapons
            .iter()
            .filter(|weapon| self.weapon_reaches(target, weapon))
            .map(|weapon| weapon.range_max)
            .max()
            .unwrap_or(Fx::ZERO)
            + half;
        self.capital_close_in(row, at, reach)
    }

    /// [`Self::capital_engage`] on a point: a warship shelling the ground (`AttackGround`,
    /// or `Bombard` with its shots spread `scatter` about the point) holds once the point
    /// is in reach and lays its guns on it the same way, rather than circling it like a
    /// gunship.
    pub(super) fn capital_engage_at(
        &mut self,
        row: usize,
        at: FxVec2,
        scatter: Fx,
    ) -> Result<(), SimError> {
        let full = self.bp(row).max_weapon_range();
        let reach = (full - scatter).max(full / 2);
        if self.state.units.pos[row].distance(at) <= reach {
            self.capital_hold(row, at);
            return Ok(());
        }
        self.capital_close_in(row, at, reach)
    }

    /// Stops where it is, guns laid on `at`.
    fn capital_hold(&mut self, row: usize, at: FxVec2) {
        self.state.units.flags[row] &= !flag::AIR_RUN;
        self.state.units.air_aim[row] = at;
        if self.state.units.has_flag(row, flag::HAS_FIELD) {
            self.stop_moving(row);
        }
    }

    /// Flies straight at `at`, aiming to come to rest a tenth inside `reach` of it, so
    /// the hull is braking to a stand as the mark comes into reach rather than
    /// overshooting it at cruise. It holds as soon as the mark is in reach.
    fn capital_close_in(&mut self, row: usize, at: FxVec2, reach: Fx) -> Result<(), SimError> {
        self.state.units.flags[row] &= !flag::AIR_RUN;
        self.state.units.air_aim[row] = at;
        let back = (self.state.units.pos[row] - at).normalize();
        let goal = self.clamp_to_map(at + back * (reach * Fx::ratio(9, 10)));
        self.ensure_moving(row, goal, goal)
    }
}
