//! An Unmaker's warheads (Shapers on `SiteKind::Nuke` sites). An Unmaker raises silos
//! behind itself (`progenitor_silo`, `PieceKind::Silo`, more as it levels:
//! `shaper_silos`), and each silo is an ordinary nuclear launcher (`nukes.rs`) that
//! grows with its Unmaker's level by its blueprint's `growth` block: more warheads held,
//! each assembled sooner, a bigger blast. Whenever a silo holds a warhead and the
//! Unmaker's launch gap (`launch_gap_ticks`) has run, the Unmaker gives it a mark: the
//! defenders' densest knot of structures (else of units on the ground) that no warhead
//! of its is already bound for. From there the warhead is any other: the defenders'
//! anti-nukes shoot it down as they would any enemy's, and its mark is shown to them
//! from launch (the interface reveals the Progenitor's warheads).

use super::rules::launch_gap_ticks;
use super::{PieceKind, HZ};
use crate::nukes::MissileKind;
use crate::{SimError, UnitId};
use mc_core::{Fx, FxVec2, Rng};
use mc_data::survival::SiteKind;
use mc_data::MoveLayer;

/// Places tried as a warhead's mark; the densest wins. A bound on the search.
const CANDIDATES: usize = 48;
/// Ticks an Unmaker with nothing to launch (or nothing to aim at) waits to look again.
const RETRY_TICKS: u32 = 2 * HZ;

impl crate::World {
    /// Unmakers whose launch gap has run give a silo holding a warhead its mark.
    pub(super) fn survival_warheads(&mut self, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let due: Vec<usize> = (0..s.nodes.len())
            .filter(|&i| {
                let n = &s.nodes[i];
                n.raised && now >= n.next_fire && n.kind == SiteKind::Nuke
            })
            .collect();
        for i in due {
            let s = self.state.survival.as_ref().unwrap();
            let side = s.config.engine_player;
            let level = s.nodes[i].level;
            let silo = self.ready_silo(i);
            let node = &mut self.state.survival.as_mut().unwrap().nodes[i];
            node.next_fire = now + RETRY_TICKS;
            let Some((silo, radius)) = silo else {
                continue;
            };
            let mut rng = self.state.survival.as_ref().unwrap().warp_ins.rng;
            let mark = self.warhead_mark(&mut rng, radius);
            self.state.survival.as_mut().unwrap().warp_ins.rng = rng;
            let Some(mark) = mark else {
                continue;
            };
            self.launch_nuke(side, &[silo], mark);
            self.state.survival.as_mut().unwrap().nodes[i].next_fire =
                now + launch_gap_ticks(level);
        }
        Ok(())
    }

    /// The standing silo of Unmaker `i` with the most warheads free (ties: the lowest id),
    /// and its blast's reach; None when none holds one.
    fn ready_silo(&self, i: usize) -> Option<(UnitId, Fx)> {
        let n = &self.state.survival.as_ref()?.nodes[i];
        n.pieces
            .iter()
            .filter(|p| p.kind == PieceKind::Silo && p.raised)
            .filter_map(|p| {
                let row = self.state.units.row(p.unit)?;
                let free = self.state.strategic.launchers.get(&p.unit)?.free();
                let radius = self.launcher_spec(row)?.blast?.radius;
                (free > 0).then_some((std::cmp::Reverse(free), p.unit, radius))
            })
            .min_by_key(|(free, id, _)| (*free, *id))
            .map(|(_, id, radius)| (id, radius))
    }

    /// Where a warhead of `radius` does the defenders most harm: of up to `CANDIDATES`
    /// of their structures (else their units on the ground), the one with the most of
    /// them within the blast, none within a blast of a mark the Progenitor's warheads are
    /// already bound for.
    fn warhead_mark(&self, rng: &mut Rng, radius: Fx) -> Option<FxVec2> {
        let side = self.state.survival.as_ref()?.config.engine_player;
        let u = &self.state.units;
        let (mut structures, mut armies) = (Vec::new(), Vec::new());
        for r in u.slots.iter() {
            if !u.is_active(r) || !self.are_enemies(side, u.owner[r]) {
                continue;
            }
            match self.bp(r).motion {
                None => structures.push(u.pos[r]),
                Some(m) if m.layer != MoveLayer::Air => armies.push(u.pos[r]),
                Some(_) => {}
            }
        }
        let pool = if structures.is_empty() {
            armies
        } else {
            structures
        };
        // Marks already given: warheads in the air, and those waiting in a silo.
        let st = &self.state.strategic;
        let mut taken: Vec<FxVec2> = st
            .missiles
            .iter()
            .filter(|m| m.kind == MissileKind::Warhead && m.owner == side)
            .map(|m| m.mark.xy())
            .collect();
        for (id, l) in &st.launchers {
            if u.row(*id).is_some_and(|r| u.owner[r] == side) {
                taken.extend(l.targets.iter().map(|t| t.0));
            }
        }
        let open: Vec<FxVec2> = pool
            .iter()
            .copied()
            .filter(|p| taken.iter().all(|t| t.distance(*p) > radius))
            .collect();
        if open.is_empty() {
            return None;
        }
        let picks: Vec<FxVec2> = if open.len() <= CANDIDATES {
            open.clone()
        } else {
            (0..CANDIDATES)
                .map(|_| open[rng.below(open.len() as u32) as usize])
                .collect()
        };
        let reach = radius * radius;
        picks
            .iter()
            .enumerate()
            .map(|(k, c)| {
                let hits = pool.iter().filter(|p| p.distance_sq(*c) <= reach).count();
                (std::cmp::Reverse(hits), k, *c)
            })
            .min_by_key(|(hits, k, _)| (*hits, *k))
            .map(|(_, _, c)| c)
    }
}
