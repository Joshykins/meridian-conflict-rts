//! Engineer hunts (`Gambit::Hunt`): a fast squad from the staging point goes
//! after the enemy's builders and its mines out on the edges, to keep it from
//! building, while the main army gathers for its wave.
use super::strategy::Gambit;
use super::*;

/// Ticks between squads sent: a minute.
const HUNT_EVERY: u32 = 600;
/// A squad: the fastest this many of those gathered, and no fewer than `HUNT_MIN`.
const HUNT_SQUAD: usize = 6;
const HUNT_MIN: usize = 4;
/// An engineer seen within this many ticks is still near where it was seen.
const FRESH: u32 = 300;
/// Guns within this of a target count against it.
const GUN_REACH: Fx = Fx::from_int(500);
/// Its owner's start at least this far off: out on the edges.
const OUTSKIRTS: Fx = Fx::from_int(900);

impl World {
    /// Sends a hunting squad out of `at_stage` when one is due and there is
    /// something on the enemy's edges to hunt.
    pub(super) fn direct_hunt(
        &mut self,
        player: u8,
        at_stage: &mut Vec<usize>,
        staging: FxVec2,
        out: &mut Vec<Command>,
    ) {
        let tick = self.state.tick;
        if !self.holds(player, Gambit::Hunt)
            || tick < self.state.ai[player as usize].next_hunt
            || at_stage.len() < HUNT_MIN
        {
            return;
        }
        let Some(target) = self.hunt_target(player, staging) else {
            return;
        };
        let units = &self.state.units;
        let speed = |r: usize| self.bp(r).motion.map_or(Fx::ZERO, |m| m.speed);
        let mut fast: Vec<usize> = at_stage
            .iter()
            .copied()
            .filter(|&r| !self.bp(r).has(cat::ARTILLERY))
            .collect();
        fast.sort_by_key(|&r| (std::cmp::Reverse(speed(r)), units.id(r)));
        fast.truncate(HUNT_SQUAD);
        if fast.len() < HUNT_MIN {
            return;
        }
        out.push(Command::AttackMove {
            units: self.ids_of(&fast),
            target,
            queue: false,
        });
        at_stage.retain(|r| !fast.contains(r));
        let ai = &mut self.state.ai[player as usize];
        ai.next_hunt = tick + HUNT_EVERY;
        ai.raids += 1;
    }

    /// A builder of the enemy's seen lately out on its edges, else a mine
    /// there, the one with the fewest guns near it for the walk.
    fn hunt_target(&self, player: u8, from: FxVec2) -> Option<FxVec2> {
        let tick = self.state.tick;
        let contacts = &self.state.ai[player as usize].contacts;
        let guns = |at: FxVec2| -> i64 {
            contacts
                .iter()
                .filter(|c| c.pos.distance(at) <= GUN_REACH)
                .map(|c| self.blueprints.unit(c.blueprint))
                .filter(|bp| !bp.weapons.is_empty() && !bp.has(cat::ENGINEER))
                .map(adaptive::strength)
                .sum()
        };
        contacts
            .iter()
            .filter(|c| {
                let bp = self.blueprints.unit(c.blueprint);
                let engineer = bp.has(cat::ENGINEER)
                    && !bp.has(cat::COMMANDER)
                    && tick.saturating_sub(c.seen) <= FRESH;
                (engineer || bp.has(cat::EXTRACTOR))
                    && self
                        .enemy_start_near(player, c.pos)
                        .is_some_and(|s| s.distance(c.pos) > OUTSKIRTS)
            })
            .map(|c| {
                // An engineer is worth a longer walk than a mine.
                let builder = self.blueprints.unit(c.blueprint).has(cat::ENGINEER) as i64;
                let cost =
                    c.pos.distance(from).floor_int() as i64 / 4 + guns(c.pos) * 2 - builder * 300;
                (cost, c.pos)
            })
            .min_by_key(|&(cost, p)| (cost, p.x, p.y))
            .map(|(_, p)| p)
    }
}
