//! The commander unit, the one unit the side moves by itself, the way a player
//! guards their king (`docs/AI_COMMANDER.md`, "Rules every layer keeps"): under
//! fire it cannot answer, it walks out of reach of what is shooting it.
use super::profile::Target;
use super::Ctx;
use crate::ai::offset_toward;
use crate::command::Command;
use crate::World;
use mc_core::{Angle, Fx, FxVec2};

/// Health lost since the last think, as a share of the whole, that says it is under fire.
const HIT: Fx = Fx::ratio(1, 50);
/// Ticks it keeps away once it has moved off (half a minute): going home at once
/// walked it back under the guns.
const AWAY: u32 = 300;
/// Contacts seen this many ticks ago or less may be what is shooting.
const FRESH: u32 = 50;
/// Metres past the shooters' reach it walks to.
const MARGIN: i32 = 300;
/// Mass a commander counts for when it can shoot back: about a tech 2 army's worth.
const KING_MASS: i32 = 2000;

impl World {
    /// Moves the commander out of reach of what is shooting it when the side cannot
    /// answer it there. Corvettes warped in and shot one from a kilometre with rails
    /// no anti-air in its base could reach, and the match was lost with it.
    pub(in crate::ai) fn guard_commander(&mut self, ctx: &Ctx, out: &mut Vec<Command>) {
        let player = ctx.player as usize;
        let tick = self.state.tick;
        let units = &self.state.units;
        let Some(r) = units
            .row(self.state.players[player].commander)
            .filter(|&r| units.is_active(r))
        else {
            return;
        };
        let at = units.pos[r];
        let health = units.health[r];
        let full = self.bp(r).health.max(Fx::ONE);
        let king = ctx.profiles.get(units.blueprint[r]);
        let was = std::mem::replace(&mut self.state.ai[player].commander.king_health, health);
        if was == Fx::ZERO || was - health < full * HIT {
            return;
        }
        // What can be shooting it: armed enemies seen lately with it in their reach.
        let mut shooters: Vec<(FxVec2, Option<Target>)> = Vec::new();
        let mut theirs = Fx::ZERO;
        let mut sum = FxVec2::ZERO;
        let mut reach = Fx::ZERO;
        for c in &self.state.ai[player].contacts {
            let p = ctx.profiles.get(c.blueprint);
            let range = p.reach[Target::Land as usize];
            if tick.saturating_sub(c.seen) > FRESH
                || !p.hits(Target::Land)
                || c.pos.distance(at) > range + Fx::from_int(MARGIN)
            {
                continue;
            }
            theirs += p.mass;
            sum += c.pos - at;
            reach = reach.max(range);
            shooters.push((c.pos, p.is));
        }
        if shooters.is_empty() {
            return;
        }
        // What of the side can shoot back from where it stands: the commander, and
        // armed units with a shooter in their own reach. Anti-air beside it that
        // could hit a corvette but not a kilometre out counted, and it stood there.
        let answers = |p: &super::profile::Profile, from: FxVec2| {
            shooters.iter().any(|&(s, t)| {
                t.is_some_and(|t| p.hits(t) && s.distance(from) <= p.reach[t as usize])
            })
        };
        let mut ours = if answers(king, at) {
            Fx::from_int(KING_MASS)
        } else {
            Fx::ZERO
        };
        let units = &self.state.units;
        for row in units.slots.iter() {
            if row == r || units.owner[row] != ctx.player || !units.is_active(row) {
                continue;
            }
            let p = ctx.profiles.get(units.blueprint[row]);
            if p.armed()
                && units.pos[row].distance(at) <= reach + Fx::from_int(MARGIN * 2)
                && answers(p, units.pos[row])
            {
                ours += p.mass;
            }
        }
        // Worn down past three fifths it goes either way: a commander at half health
        // stood beside fourteen thousand mass of its own and died to five corvettes.
        let worn = health * 5 < full * 3;
        if ours >= theirs && !worn {
            return;
        }
        // Away from them, past their reach: straight back from the shooters, else
        // a little to either side, else home.
        let n = shooters.len() as i32;
        let centre = at + FxVec2::new(sum.x / n, sum.y / n);
        let dist = reach + Fx::from_int(MARGIN) - centre.distance(at).min(reach);
        let back = at + (at - centre);
        let spot = [0, 50, -50, 90, -90]
            .into_iter()
            .map(|deg| {
                let dir = (back - at).rotate(Angle::from_degrees(deg));
                self.clamp_to_map(offset_toward(at, at + dir, dist))
            })
            .find(|&p| ctx.can_walk(p))
            .unwrap_or(ctx.start);
        self.state.ai[player].commander.king_fled = tick + AWAY;
        out.push(Command::Move {
            units: vec![self.state.units.id(r)],
            target: spot,
            queue: false,
        });
    }
}
