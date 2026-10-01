//! Self-destruct. Ordered at once (`Command::SelfDestruct` with `timed` off) a unit
//! blows up this tick; timed, it counts down [`COUNTDOWN`] ticks first, in the open for
//! the interface to show round it (`RenderFrame::destructs`), and the same order given
//! again calls it off.

use crate::tables::UnitId;
use crate::World;
use mc_core::{Fx, TICKS_PER_SECOND};

/// Ticks a timed self-destruct counts down: five seconds.
pub(crate) const COUNTDOWN: u16 = 5 * TICKS_PER_SECOND as u16;

impl World {
    /// `Command::SelfDestruct`: the player's units among `ids` blow up now, or start
    /// counting down. A timed order to units of which any is counting down already
    /// stops every countdown among them, so the same key both arms and disarms.
    pub(crate) fn self_destruct(&mut self, player: u8, ids: &[UnitId], timed: bool) {
        let rows = self.owned(player, ids, 0);
        let units = &mut self.state.units;
        if !timed {
            for row in rows {
                units.health[row] = Fx::ZERO;
            }
            return;
        }
        let disarm = rows.iter().any(|&row| units.destruct[row] > 0);
        for row in rows {
            units.destruct[row] = if disarm { 0 } else { COUNTDOWN };
        }
    }

    /// Counts every armed self-destruct down one tick; a unit whose count runs out
    /// blows up, as if ordered at once.
    pub(crate) fn run_destructs(&mut self) {
        let units = &mut self.state.units;
        for row in units.slots.iter() {
            match units.destruct[row] {
                0 => {}
                1 => {
                    units.destruct[row] = 0;
                    units.health[row] = Fx::ZERO;
                }
                n => units.destruct[row] = n - 1,
            }
        }
    }
}
