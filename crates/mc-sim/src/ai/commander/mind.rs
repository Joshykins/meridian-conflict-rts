//! What a Commander is thinking, as plain data for the observer overlay
//! (`docs/AI_COMMANDER.md`, "Overlay"): its plans with their stakes and appeal,
//! each operation's group, target, phase and trade, what has hurt it, and its
//! last decisions with why. Read-only: nothing here goes back into the sim.
use super::state::{OpKind, Phase, PLANS};
use crate::{Brain, World};
use mc_core::Fx;

/// One way to win the side weighs.
#[derive(Clone, Debug, PartialEq)]
pub struct MindPlan {
    pub name: &'static str,
    /// "off", "probe", "invest" or "all-in".
    pub stake: &'static str,
    /// 0 (off) to 3 (all-in).
    pub level: u8,
    /// How good it looked at the last review.
    pub appeal: i32,
}

/// One operation: a group with a job.
#[derive(Clone, Debug, PartialEq)]
pub struct MindOp {
    pub id: u32,
    pub kind: &'static str,
    pub phase: &'static str,
    /// It is moving on its target, not gathering or coming home.
    pub engaged: bool,
    pub units: u32,
    /// Mass in the group, and the mass it wants.
    pub mass: i32,
    pub want: i32,
    /// Where the group stands (metres), when any of it is alive.
    pub at: Option<(i32, i32)>,
    pub target: (i32, i32),
    pub rally: (i32, i32),
    /// Enemy mass it has killed and own mass it has lost.
    pub killed: i32,
    pub lost: i32,
}

/// One decision, newest last.
#[derive(Clone, Debug, PartialEq)]
pub struct MindNote {
    pub minute: u32,
    pub plan: &'static str,
    pub stake: &'static str,
    pub why: &'static str,
}

/// A Commander's mind, for the overlay.
#[derive(Clone, Debug, PartialEq)]
pub struct AiMind {
    /// Every plan, held ones first, then by appeal.
    pub plans: Vec<MindPlan>,
    pub ops: Vec<MindOp>,
    pub notes: Vec<MindNote>,
    /// Recent losses by what killed them (mass, fading), named.
    pub hurt: Vec<(&'static str, i32)>,
    /// Minutes since each kind of force was last seen; `None`: never.
    pub seen: Vec<(&'static str, Option<u32>)>,
    /// Where the waves gather.
    pub rally: Option<(i32, i32)>,
}

const HURT_NAMES: [&str; 6] = ["land", "artillery", "air", "space", "sea", "turrets"];

impl World {
    /// What player `player`'s AI is thinking, when it is a Commander.
    pub fn ai_mind(&self, player: u8) -> Option<AiMind> {
        let ai = self.state.ai.get(player as usize)?;
        if ai.config.brain != Brain::Commander || ai.commander.plans.is_empty() {
            return None;
        }
        let c = &ai.commander;
        let xy = |p: mc_core::FxVec2| (p.x.floor_int(), p.y.floor_int());
        let mut plans: Vec<MindPlan> = PLANS
            .iter()
            .map(|&k| {
                let stake = c.plan(k);
                MindPlan {
                    name: k.name(),
                    stake: stake.name(),
                    level: stake as u8,
                    appeal: i32::from(c.appeal[k as usize]),
                }
            })
            .collect();
        plans.sort_by_key(|p| (std::cmp::Reverse(p.level), std::cmp::Reverse(p.appeal)));
        let ops = c
            .ops
            .iter()
            .map(|o| {
                let rows = self.op_rows(o);
                MindOp {
                    id: o.id,
                    kind: o.kind.name(),
                    phase: o.phase.name(),
                    engaged: o.phase == Phase::Executing && o.kind != OpKind::Guard,
                    units: rows.len() as u32,
                    mass: o.mass().floor_int(),
                    want: o.want.floor_int(),
                    at: self.centre_of(&rows).map(xy),
                    target: xy(o.target),
                    rally: xy(o.rally),
                    killed: o.ledger.killed.floor_int(),
                    lost: o.ledger.lost.floor_int(),
                }
            })
            .collect();
        let notes = c
            .notes
            .iter()
            .map(|n| MindNote {
                minute: n.tick / 600,
                plan: n.plan.name(),
                stake: n.stake.name(),
                why: n.why.name(),
            })
            .collect();
        let hurt = HURT_NAMES
            .iter()
            .zip(c.hurt.iter())
            .filter(|(_, &m)| m > Fx::ZERO)
            .map(|(&n, m)| (n, m.floor_int()))
            .collect();
        let tick = self.state.tick;
        let ago = |t: u32| (t > 0).then(|| tick.saturating_sub(t) / 600);
        let s = &c.sticky;
        let seen = vec![
            ("air", ago(s.air_seen)),
            ("navy", ago(s.navy_seen)),
            ("subs", ago(s.subs_seen)),
            ("space", ago(s.space_seen)),
            ("nukes", ago(s.nukes_seen)),
        ];
        Some(AiMind {
            plans,
            ops,
            notes,
            hurt,
            seen,
            rally: c.rally.map(xy),
        })
    }
}
