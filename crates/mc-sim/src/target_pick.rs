//! How the map guns find their nearest target: their reach is most of the map, so a
//! grid search round the gun would cover the whole map anyway, and a look over the
//! unit table is far cheaper than one over the ~100k grid cells a 22 km reach spans.

use mc_core::Fx;
use mc_data::Weapon;

use crate::world::World;

/// A gun that reaches further than this is a map gun: it looks over the unit table
/// (`nearest_on_map`) rather than the grid round it. It reaches a whole base wherever
/// it stands, so the AI's builders do not keep out of its reach either (`ai/danger.rs`).
pub(crate) const MAP_GUN_REACH: Fx = Fx::from_int(5000);

/// Ticks between looks for a new target while a map gun has none. A look goes over
/// every unit on the map, and a gun that reloads in seconds loses nothing by finding
/// its next target up to this late.
const LOOK_EVERY: u32 = 10;

impl World {
    /// The nearest enemy `weapon` on `row` may shoot now, of the kinds in `prefer` (all
    /// kinds if zero), the lowest row on a tie: what the grid search finds for a gun of
    /// ordinary reach. `None` between looks (`LOOK_EVERY`).
    pub(crate) fn nearest_on_map(&self, row: usize, weapon: &Weapon, prefer: u32) -> Option<usize> {
        let units = &self.state.units;
        // Guns take their turns to look on different ticks.
        let turn = self.state.tick.wrapping_add(row as u32);
        if !turn.is_multiple_of(LOOK_EVERY) {
            return None;
        }
        let from = units.pos[row];
        let owner = units.owner[row];
        let mut best: Option<(usize, Fx)> = None;
        for t in 0..units.slots.rows() {
            // The cheap tests first: most of the map is friendly or out of kind.
            if !units.slots.is_alive(t)
                || !self.are_enemies(owner, units.owner[t])
                || !self.weapon_reaches(t, weapon)
                || (prefer != 0 && !self.hittable(t, prefer))
            {
                continue;
            }
            let gap = from.distance_sq(units.pos[t]);
            if best.is_none_or(|(_, g)| gap < g) && self.is_valid_target(row, t, weapon) {
                best = Some((t, gap));
            }
        }
        best.map(|(t, _)| t)
    }
}
