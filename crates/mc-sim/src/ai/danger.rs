//! Ground the AI's builders stay off for a while: where its buildings were
//! just shot down, and where an enemy it can see has the spot in range.
//!
//! Builders used to be blind to both. A turret or mine killed by a raider was
//! the next job of the nearest idle engineer, which walked up, started it
//! again under the same guns, and lost it (and often itself) again, round
//! after round for as long as the raider stayed.
use super::*;

/// Losses this close to a site count against it.
const HOT_RADIUS: Fx = Fx::from_int(220);
/// How long a loss is remembered at most (3 minutes).
const HOT_MEMORY: u32 = 1800;
/// How long one loss keeps builders away (45 s); every further loss nearby
/// adds as much again, up to `HOT_MEMORY`.
const HOT_PER_LOSS: u32 = 450;
/// An enemy seen this recently still counts as standing where it was (10 s).
const SEEN_RECENTLY: u32 = 100;
/// Slack on an enemy's weapon range: a builder stands off its site.
const RANGE_SLACK: Fx = Fx::from_int(60);
/// Losses kept per side.
const MAX_LOSSES: usize = 32;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(super) struct Loss {
    pub pos: FxVec2,
    pub tick: u32,
}

impl AiState {
    /// A building (finished or not) of this side was destroyed at `pos`.
    pub(crate) fn note_loss(&mut self, pos: FxVec2, tick: u32) {
        self.losses
            .retain(|l| tick.saturating_sub(l.tick) < HOT_MEMORY);
        if self.losses.len() >= MAX_LOSSES {
            self.losses.remove(0);
        }
        self.losses.push(Loss { pos, tick });
    }

    pub(super) fn hash_losses(&self, h: &mut StateHasher) {
        h.write_u64(self.losses.len() as u64);
        for l in &self.losses {
            h.write_i64(l.pos.x.0);
            h.write_i64(l.pos.y.0);
            h.write_u64(l.tick as u64);
        }
    }
}

/// What a side knows of the danger around its build sites, gathered once per think.
#[derive(Default)]
pub(super) struct Danger {
    losses: Vec<Loss>,
    /// Armed enemy ground units and turrets seen lately, with their reach.
    guns: Vec<(FxVec2, Fx)>,
    tick: u32,
}

impl Danger {
    /// Whether builders should keep off `site` for now.
    pub(super) fn hot(&self, site: FxVec2) -> bool {
        if self
            .guns
            .iter()
            .any(|&(at, reach)| at.distance(site) <= reach)
        {
            return true;
        }
        let near = self
            .losses
            .iter()
            .filter(|l| l.pos.distance(site) <= HOT_RADIUS);
        let (count, last) = near.fold((0u32, 0u32), |(n, last), l| (n + 1, last.max(l.tick)));
        count > 0 && self.tick - last < (HOT_PER_LOSS * count).min(HOT_MEMORY)
    }
}

impl World {
    pub(super) fn ai_danger(&self, player: u8) -> Danger {
        let tick = self.state.tick;
        let ai = &self.state.ai[player as usize];
        let guns = ai
            .contacts
            .iter()
            .filter(|c| tick.saturating_sub(c.seen) <= SEEN_RECENTLY)
            .filter_map(|c| {
                let bp = self.blueprints.unit(c.blueprint);
                // Aircraft come and go; anti-air answers them, not the builders.
                if bp.has(cat::AIR) || bp.has(cat::ENGINEER) {
                    return None;
                }
                let reach = bp
                    .weapons
                    .iter()
                    // Buildings count as land to most guns: anything but pure anti-air.
                    .filter(|w| w.target_mask & !cat::AIR != 0)
                    .map(|w| w.range_max)
                    .max()?;
                Some((c.pos, reach + RANGE_SLACK))
            })
            .collect();
        Danger {
            losses: ai
                .losses
                .iter()
                .copied()
                .filter(|l| tick - l.tick < HOT_MEMORY)
                .collect(),
            guns,
            tick,
        }
    }
}
