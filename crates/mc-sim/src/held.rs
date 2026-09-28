//! Orders given while the clock is held. A paused single-player match still takes
//! orders, spawns and the rest at once: they are carried out between two ticks,
//! without time moving, so a spawned unit stands there and an ordered one shows
//! its route before the match resumes. The session records the commands in front
//! of the next tick, and a replay applies them at the same point.

use crate::{PlayerCommand, SimError, World};

impl World {
    /// Carries out `commands` without advancing the tick. Returns the state hash.
    pub fn apply_held(&mut self, commands: &[PlayerCommand]) -> Result<u64, SimError> {
        // What the last tick did has been shown; only what these commands do is new.
        self.events.clear();
        self.spent.clear();
        self.muzzles.clear();
        for c in commands {
            self.apply_command(c)?;
        }
        Ok(self.hash())
    }
}
