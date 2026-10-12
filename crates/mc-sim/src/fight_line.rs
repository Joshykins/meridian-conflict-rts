//! The firing line of a block on attack-move: members that cannot hurt what the
//! block is fighting (anti-air, shields, radar, engineers) stop behind the
//! members that are firing instead of marching on past them into the enemy.
//!
//! A member holds on attack-move while it has a clear shot (`run_move`). One with
//! no weapon for the fight never has one, so it used to walk on to the goal, and
//! the block's anchor, kept by the members still marching, went with it. Now each
//! tick the firing members of every ground block are found first: how near the
//! goal the foremost of them stands, and what they are shooting at. A member with
//! no weapon for ground or sea (unarmed, anti-air, interceptors only) that could
//! hit none of those targets holds once it is level with that line. Every other
//! member walks on into the fight: it is only short of range, or after other prey.

use crate::tables::*;
use crate::World;
use mc_core::Fx;
use mc_data::{cat, BlueprintId};
use std::collections::BTreeMap;

/// How far behind the foremost firing member, beyond its own radius, a member
/// that cannot join the fight keeps.
const GAP: Fx = Fx::from_int(10);

/// One block's fight this tick.
pub(crate) struct FightLine {
    /// Distance to the order's goal of the firing member nearest it.
    front: Fx,
    /// What the firing members are shooting at.
    targets: Vec<BlueprintId>,
}

/// The fights of every ground block on attack-move, by formation.
pub(crate) type FightLines = BTreeMap<u64, FightLine>;

impl World {
    /// Each ground block on attack-move with a member firing: its firing line.
    pub(crate) fn fight_lines(&self) -> FightLines {
        let units = &self.state.units;
        let mut lines = FightLines::new();
        for row in units.slots.iter() {
            if !units.is_active(row) || self.is_air(row) {
                continue;
            }
            let Some(o) = self.state.orders.front(units, row) else {
                continue;
            };
            if o.kind != OrderKind::AttackMove || o.formation == 0 || !self.has_clear_target(row) {
                continue;
            }
            let near = units.pos[row].distance(o.pos);
            let line = lines.entry(o.formation).or_insert(FightLine {
                front: near,
                targets: Vec::new(),
            });
            line.front = line.front.min(near);
            for t in units.weapon_target[row]
                .iter()
                .filter_map(|&t| units.row(t))
            {
                let bp = units.blueprint[t];
                if !line.targets.contains(&bp) {
                    line.targets.push(bp);
                }
            }
        }
        lines
    }

    /// Whether `row`, marching on attack-move order `o`, stops at its block's
    /// firing line: it has nothing to shoot, could hurt none of what the block is
    /// fighting, and has come level with the foremost member firing.
    pub(crate) fn holds_behind_fight(&self, row: usize, o: &Order, lines: &FightLines) -> bool {
        let Some(line) = lines.get(&o.formation) else {
            return false;
        };
        let bp = self.bp(row);
        // A fighter goes on to the fight, even one whose prey is not what the
        // others are shooting (a submarine, say, behind ships firing on the shore).
        let fights_surface = bp
            .weapons
            .iter()
            .any(|w| !w.intercepts && w.target_mask & !cat::AIR != 0);
        if fights_surface
            || line
                .targets
                .iter()
                .any(|&t| bp.can_attack(self.blueprints.unit(t)))
        {
            return false;
        }
        self.state.units.pos[row].distance(o.pos) <= line.front + bp.radius + GAP
    }
}
