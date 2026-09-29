//! Blasts wear wrecks down. Splash, bombs, a volatile unit going up, a warhead's front
//! and an AEB storm take mass off every wreck they reach, as reclaim does, until
//! there is nothing left and the wreck is gone. The renderer shows the loss as it
//! goes (`health`, the share of its mass left, wears it away).
use crate::spatial::kind;
use crate::World;
use mc_core::{Fx, FxVec3};

/// A wreck takes this many times its unit's full health in blast damage to be blown
/// apart: it is a husk, but a heavy one.
const TOUGHNESS: i32 = 2;

impl World {
    /// Wears down every wreck within `radius` of `center` (to its middle, less half its
    /// size), by `damage(distance)` each; a dome between them takes it instead. A wreck
    /// with nothing left goes.
    pub(crate) fn wear_wrecks(&mut self, center: FxVec3, radius: Fx, damage: impl Fn(Fx) -> Fx) {
        if radius <= Fx::ZERO {
            return;
        }
        let wrecks = &self.state.wrecks;
        let mut hit = Vec::new();
        self.index.query(center.xy(), radius, kind::WRECK, |e| {
            let w = e.row as usize;
            if wrecks.slots.is_alive(w) && wrecks.pos[w] == e.pos {
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
