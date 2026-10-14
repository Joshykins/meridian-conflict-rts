//! Non-mission maps place their authored units before the first tick. Missions
//! instead stage named groups through their deterministic event program.
use crate::{SimError, World};
impl World {
    pub fn spawn_placed_units(&mut self, placed: &mc_map::placed::Placed) -> Result<(), SimError> {
        let mut spawns = Vec::with_capacity(placed.units.len());
        for u in &placed.units {
            if usize::from(u.owner) >= self.state.players.len() {
                return Err(SimError::Setup(format!(
                    "{} uses missing player slot {}",
                    u.name,
                    u.owner + 1
                )));
            }
            let bp = self.blueprints.id_of(&u.blueprint).ok_or_else(|| {
                SimError::Setup(format!("{} uses unknown blueprint {}", u.name, u.blueprint))
            })?;
            spawns.push((bp, u.owner, u.at, u.heading));
        }
        for (bp, owner, at, heading) in spawns {
            self.spawn_unit(bp, owner, at, heading, true)?;
        }
        Ok(())
    }
}
