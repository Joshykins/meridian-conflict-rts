//! Blasts wear wrecks down. Splash, bombs, a volatile unit going up, a warhead's front
//! and an AEB storm take mass off every wreck they reach, as reclaim does, until
//! there is nothing left and the wreck is gone. The renderer shows the loss as it
//! goes (`health`, the share of its mass left, wears it away). A wreck just left takes
//! nothing for its first second (`FRESH_TICKS`), so the blast that killed a unit, or the
//! next shell of the same salvo, does not blow its wreck away at once.
use crate::spatial::kind;
use crate::World;
use mc_core::{Fx, FxVec3, TICKS_PER_SECOND};

/// A wreck takes this many times its unit's full health in blast damage to be blown
/// apart: it is a husk, but a heavy one.
const TOUGHNESS: i32 = 2;

/// How long a newly left wreck takes no blast damage: about a second.
const FRESH_TICKS: u32 = TICKS_PER_SECOND;

impl World {
    /// Wears down every wreck within `radius` of `center` (to its middle, less half its
    /// size), by `damage(distance)` each; a dome between them takes it instead. A wreck
    /// with nothing left goes. One left less than `FRESH_TICKS` ago is spared (one the
    /// map laid never is: it was there before the match).
    pub(crate) fn wear_wrecks(&mut self, center: FxVec3, radius: Fx, damage: impl Fn(Fx) -> Fx) {
        if radius <= Fx::ZERO {
            return;
        }
        let wrecks = &self.state.wrecks;
        let tick = self.state.tick;
        let fresh =
            |w: usize| !wrecks.from_map[w] && tick.saturating_sub(wrecks.born[w]) < FRESH_TICKS;
        let mut hit = Vec::new();
        self.index.query(center.xy(), radius, kind::WRECK, |e| {
            let w = e.row as usize;
            if wrecks.slots.is_alive(w) && wrecks.pos[w] == e.pos && !fresh(w) {
                hit.push(w);
            }
            true
        });
        // Rows in table order, so the outcome does not depend on the index's walk. Rows are unique.
        hit.sort_unstable();
        for w in hit {
            let wrecks = &self.state.wrecks;
            let bp = self.blueprints.unit(wrecks.blueprint[w]);
            let middle = wrecks.pos[w].extend(wrecks.z[w] + bp.height / 2);
            let reach = ((middle - center).length() - bp.radius / 2).max(Fx::ZERO);
            if reach > radius || self.blast_blocker(center, middle, None).is_some() {
                continue;
            }
            let dealt = damage(reach);
            if dealt <= Fx::ZERO {
                continue;
            }
            let wrecks = &mut self.state.wrecks;
            let lost = wrecks.mass_max[w] * dealt / (bp.health * TOUGHNESS).max(Fx::ONE);
            let left = wrecks.mass[w];
            wrecks.mass[w] = left - lost.min(left);
            if wrecks.mass[w] <= Fx::ZERO {
                wrecks.slots.free(w);
            }
        }
    }
}
