use super::*;
impl World {
    pub(crate) fn damage_unit(&mut self, row: usize, damage: Fx, by: u8, source: UnitId) {
        let units = &mut self.state.units;
        if units.health[row] <= Fx::ZERO
            || units.has_flag(row, flag::INVULNERABLE)
            || self
                .state
                .mission
                .as_ref()
                .is_some_and(|m| m.protected.contains(&units.id(row)))
        {
            return;
        }
        let applied = damage.min(units.health[row]);
        units.health[row] -= applied;
        units.flags[row] |= flag::HURT;
        let killed = units.health[row] <= Fx::ZERO;
        self.record_damage(row, source, applied);
        if killed {
            self.settle_kill(row, source, by);
        }
    }
}
