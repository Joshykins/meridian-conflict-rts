//! Guns that choose their own target by worth rather than by distance
//! (`TargetPick::Costliest`): the map guns, whose reach is most of the map, so a grid
//! search round the gun would cover the whole map anyway.

use mc_data::Weapon;

use crate::world::World;

/// Ticks between looks for a new target while a costliest-pick gun has none. A look
/// goes over every unit on the map, and a gun that reloads in seconds loses nothing
/// by finding its next target up to this late.
const LOOK_EVERY: u32 = 10;

impl World {
    /// The enemy that cost its owner the most mass among those `weapon` on `row` may
    /// shoot now and of the kinds in `prefer` (all kinds if zero); the nearest of
    /// those on a tie, then the lowest row. `None` between looks (`LOOK_EVERY`).
    pub(crate) fn costliest_target(
        &self,
        row: usize,
        weapon: &Weapon,
        prefer: u32,
    ) -> Option<usize> {
        let units = &self.state.units;
        // Guns take their turns to look on different ticks.
        let turn = self.state.tick.wrapping_add(row as u32);
        if !turn.is_multiple_of(LOOK_EVERY) {
            return None;
        }
        let from = units.pos[row];
        let owner = units.owner[row];
        let mut best: Option<(usize, mc_core::Fx, mc_core::Fx)> = None;
        for t in 0..units.slots.rows() {
            // The cheap tests first: most of the map is friendly or out of kind.
            if !units.slots.is_alive(t)
                || !self.are_enemies(owner, units.owner[t])
                || !self.weapon_reaches(t, weapon)
                || (prefer != 0 && !self.hittable(t, prefer))
                || !self.is_valid_target(row, t, weapon)
            {
                continue;
            }
            let worth = self.bp(t).cost_mass;
            let gap = from.distance(units.pos[t]);
            let better = best.is_none_or(|(_, w, g)| worth > w || (worth == w && gap < g));
            if better {
                best = Some((t, worth, gap));
            }
        }
        best.map(|(t, _, _)| t)
    }
}
