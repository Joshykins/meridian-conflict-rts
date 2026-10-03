//! Where the AI's next core mine goes: the nearest free mine point.
use super::*;

impl World {
    /// The nearest free mine point within `range` of `from`, staying off the
    /// enemy's doorstep until the army can contest it. Its lot centre.
    pub(super) fn free_deposit(
        &self,
        from: FxVec2,
        claimed: &[Claim],
        range: Fx,
        intel: &Intel,
    ) -> Option<FxVec2> {
        self.free_ores(from, claimed, range, intel)
            .into_iter()
            .next()
    }

    /// Mine points with no mine on them and none planned, within `range` of
    /// `from`, nearest first (`free_deposit`'s rule).
    pub(super) fn free_ores(
        &self,
        from: FxVec2,
        claimed: &[Claim],
        range: Fx,
        intel: &Intel,
    ) -> Vec<FxVec2> {
        let mut free: Vec<FxVec2> = self
            .mine_points
            .iter()
            .copied()
            .filter(|&d| self.deposit_open(d, from, range, claimed, intel))
            .collect();
        free.sort_by_key(|d| (d.distance_sq(from), d.x, d.y));
        free
    }

    /// Whether a mine may go on the point `d`: in range, no mine on it (built or
    /// planned), off the enemy's doorstep, and out of danger.
    fn deposit_open(
        &self,
        d: FxVec2,
        from: FxVec2,
        range: Fx,
        claimed: &[Claim],
        intel: &Intel,
    ) -> bool {
        d.distance(from) <= range
            && self.mine_on_point(d).is_none()
            // A planned mine counts like a built one.
            && !claimed.iter().any(|c| c.pos.distance(d) < Fx::from_int(32))
            && intel.enemy_start.is_none_or(|e| {
                d.distance(e) > Fx::from_int(480) || d.distance(from) < d.distance(e)
            })
            && !intel.danger.hot(d)
    }
}
