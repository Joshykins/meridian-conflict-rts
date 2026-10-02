//! Army salvagers: a salvage unit whose reclaimer follows the army
//! (`Reclaimer::follows_army`, the Regency's Scythe) is not sent to wreck fields near home.
//! The AI makes one for each of its big ground operations, up to `MOST_ESCORTS`, and puts
//! it on guard round the operation's lead unit, so it goes with the army and takes apart
//! what falls round it as the fight moves (`area_work.rs`).

use super::commander::state::OpKind;
use super::Census;
use crate::command::Command;
use crate::tables::{OrderKind, UnitId};
use crate::World;
use mc_core::Fx;
use mc_data::BlueprintId;

/// Most army salvagers the AI keeps, one an operation.
const MOST_ESCORTS: usize = 2;
/// Mass a ground operation holds before an army salvager goes with it.
const ESCORTED_ARMY: i32 = 2000;
/// The ring an army salvager works round the unit it follows.
const ESCORT_RADIUS: i32 = 600;

impl World {
    /// The lead unit of each ground operation big enough to take an army salvager,
    /// biggest first, at most `MOST_ESCORTS` (a deliberate cap: more would trail the
    /// smaller groups for little).
    fn escorted_armies(&self, player: u8) -> Vec<UnitId> {
        let units = &self.state.units;
        let mut armies: Vec<(Fx, u32, UnitId)> = self.state.ai[player as usize]
            .commander
            .ops
            .iter()
            .filter(|o| matches!(o.kind, OpKind::Army | OpKind::Siege | OpKind::Defend))
            .filter(|o| o.mass() >= Fx::from_int(ESCORTED_ARMY))
            .filter_map(|o| {
                let lead = o
                    .units
                    .iter()
                    .map(|(id, _)| *id)
                    .find(|&id| units.row(id).is_some())?;
                Some((o.mass(), o.id, lead))
            })
            .collect();
        // Biggest first; the operation's id breaks a tie.
        armies.sort_by_key(|&(mass, id, _)| (std::cmp::Reverse(mass), id));
        armies
            .into_iter()
            .take(MOST_ESCORTS)
            .map(|(_, _, lead)| lead)
            .collect()
    }

    /// What an idle factory makes to go with the army, if an army has none yet: the
    /// cheapest army salvager it builds.
    pub(super) fn escort_product(
        &self,
        player: u8,
        factory: usize,
        census: &Census,
        planned: usize,
    ) -> Option<BlueprintId> {
        if census.escorts.len() + planned >= self.escorted_armies(player).len() {
            return None;
        }
        let builder = self.bp(factory).builder.as_ref()?;
        builder
            .builds
            .iter()
            .copied()
            .filter(|b| follows_army(self.blueprints.unit(*b)))
            .min_by_key(|b| (self.blueprints.unit(*b).cost_mass, b.0))
    }

    /// Puts each army salvager on guard round the lead of an army, unless it already
    /// guards a unit of it.
    pub(super) fn direct_escorts(&self, player: u8, census: &Census, out: &mut Vec<Command>) {
        let armies = self.escorted_armies(player);
        if armies.is_empty() {
            return;
        }
        let units = &self.state.units;
        let ops = &self.state.ai[player as usize].commander.ops;
        for (i, &row) in census.escorts.iter().enumerate() {
            let lead = armies[i % armies.len()];
            let army = ops.iter().find(|o| o.has(lead));
            let guarding = self
                .state
                .orders
                .iter(units, row)
                .find(|o| o.kind == OrderKind::Guard)
                .is_some_and(|o| o.target == lead || army.is_some_and(|a| a.has(o.target)));
            if guarding {
                continue;
            }
            let Some(at) = units.row(lead) else {
                continue;
            };
            out.push(Command::Guard {
                units: vec![units.id(row)],
                pos: units.pos[at],
                target: lead,
                radius: Fx::from_int(ESCORT_RADIUS),
                queue: false,
            });
        }
    }
}

/// A salvage unit made to go with the army.
pub(super) fn follows_army(bp: &mc_data::UnitBlueprint) -> bool {
    bp.is_salvager() && bp.reclaimer.is_some_and(|r| r.follows_army)
}

#[cfg(test)]
#[path = "escorts_tests.rs"]
mod tests;
