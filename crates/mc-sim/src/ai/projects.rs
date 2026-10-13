//! Strategic projects: which blueprints are experimental units and strategic
//! structures (`project_kind`; the Commander's plans decide which get built),
//! firing the nukes (`direct_nukes`) and aiming the gravitic engines
//! (`direct_gravitic`).
//!
//! Nothing here names a unit. A project is any blueprint in a builder's menu
//! with the `Experimental`, `Strategic` or `Space` category; its kind comes from
//! its data: a `strategic` launcher (nuke or interceptor), a structure with
//! anti-air (an anti-spaceship gun), one with artillery (a map gun), or a
//! mobile unit.
use super::*;
use crate::gravitic::WellPhase;
use mc_data::strategic::StrategicKind;

/// Enemy value a warhead must be able to reach, in multiples of its own
/// materials cost, to be fired.
const NUKE_WORTH: i64 = 2;
/// A contact seen this recently (ticks) is still where it was seen, for a
/// unit that moves.
const FRESH: u32 = 100;
/// Value put on the enemy commander under a warhead: its loss ends the game.
const COMMANDER_WORTH: i64 = 60_000;
/// Enemy materials a gravity well must hold to be worth tearing a rift for.
const WELL_WORTH: i64 = 1500;
/// Ticks a well holds before its engine looks for better ground (twenty seconds): a
/// rift takes a gather and a run to land, and an engine that turned after every
/// army that moved would never crush anything.
const WELL_HOLD: u32 = 200;

/// What a project is for, from its data.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Project {
    /// Shoots down warheads: answers an enemy silo.
    Interceptor,
    /// A structure that shoots at aircraft: answers enemy spaceships.
    SkyGun,
    Nuke,
    /// A structure that shells from afar, or holds a gravity well anywhere.
    MapGun,
    /// An experimental or a warship, built on a lot and sent with the army.
    Mobile,
    /// Brings units and materials down from orbit: an investment, wanted once the
    /// income can carry it (`choose_wants`).
    Elevator,
}

pub(super) fn project_kind(bp: &UnitBlueprint) -> Option<Project> {
    if bp.categories & (cat::EXPERIMENTAL | cat::STRATEGIC | cat::SPACE) == 0
        || bp.has(cat::REPLICATOR)
        || bp.mine.is_some()
        || bp.transport.is_some()
        || bp.cost_materials <= Fx::ZERO
        // An unarmed ship (the Vigil) is no project: counted as one, each Vigil
        // finished freed the slot for the next, and sides had 30 to 60 of them
        // by forty minutes (`spotter_job` builds the one a side wants).
        || (bp.is_mobile() && bp.weapons.is_empty())
        // A yard that raises a hull aloft (`Builder::aloft`) is a project only for what it
        // raises, and the AI raises no Regent yet.
        || bp.builder.as_ref().is_some_and(|b| b.aloft)
    {
        return None;
    }
    if bp.elevator.is_some() {
        return Some(Project::Elevator);
    }
    Some(match bp.strategic.as_ref().map(|s| s.kind) {
        Some(StrategicKind::Interceptor) => Project::Interceptor,
        Some(StrategicKind::Nuke) => Project::Nuke,
        None if bp.is_mobile() => Project::Mobile,
        None if bp.has(cat::ANTI_AIR) => Project::SkyGun,
        None if bp.has(cat::ARTILLERY) || bp.gravitic.is_some() => Project::MapGun,
        None => Project::Mobile,
    })
}

impl World {
    /// A space elevator of `player`'s going up, and the build power it still wants: as
    /// much as the side's income keeps busy on it. Its first builder alone took most
    /// of an hour while every other engineer went on claiming far mines.
    pub(super) fn elevator_short_of_help(
        &self,
        player: u8,
        census: &Census,
        intel: &Intel,
    ) -> Option<(usize, Fx)> {
        let units = &self.state.units;
        let site = census
            .sites
            .iter()
            .copied()
            .find(|&s| self.bp(s).elevator.is_some() && !intel.danger.hot(units.pos[s]))?;
        let bp = self.bp(site);
        let pl = &self.state.players[player as usize];
        let fed = |income: Fx, cost: Fx| income * (bp.build_time / cost.max(Fx::ONE));
        let want =
            fed(pl.materials_income, bp.cost_materials).min(fed(pl.energy_income, bp.cost_energy));
        let on: Fx = units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && units.is_active(r))
            .filter(|&r| {
                self.state.orders.iter(units, r).next().is_some_and(|o| {
                    (o.kind == OrderKind::Assist && !o.target.is_none() && o.target.index() == site)
                        || (o.kind == OrderKind::Build && o.blueprint == units.blueprint[site])
                })
            })
            .filter_map(|r| self.bp(r).builder.as_ref().map(|b| b.power))
            .sum();
        (on < want).then_some((site, want - on))
    }

    /// Warheads ready in `player`'s silos go at the enemy spot worth most
    /// under a blast: its commander above all, then what stands there.
    pub(super) fn direct_nukes(&self, player: u8, out: &mut Vec<Command>) {
        let units = &self.state.units;
        let silos: Vec<(usize, &mc_data::strategic::Strategic)> = units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && units.is_active(r))
            .filter_map(|r| {
                let s = self.bp(r).strategic.as_ref()?;
                (s.kind == StrategicKind::Nuke).then_some((r, s))
            })
            .collect();
        let free: u32 = silos
            .iter()
            .map(|&(r, _)| {
                self.state
                    .strategic
                    .launchers
                    .get(&units.id(r))
                    .map_or(0, |l| l.free() as u32)
            })
            .sum();
        let Some(&(_, spec)) = silos.first() else {
            return;
        };
        let Some(blast) = spec.blast.as_ref() else {
            return;
        };
        if free == 0 {
            return;
        }
        let tick = self.state.tick;
        let contacts = &self.state.ai[player as usize].contacts;
        let worth = |c: &adaptive::Contact| -> i64 {
            let bp = self.blueprints.unit(c.blueprint);
            if bp.is_mobile() && tick.saturating_sub(c.seen) > FRESH {
                return 0;
            }
            if bp.has(cat::COMMANDER) {
                COMMANDER_WORTH
            } else {
                bp.cost_materials.floor_int() as i64
            }
        };
        // Interceptors cover their ground; a salvo bigger than what they hold
        // gets through, a single warhead does not.
        let guarded = |at: FxVec2| -> u32 {
            contacts
                .iter()
                .filter_map(|c| {
                    let s = self.blueprints.unit(c.blueprint).strategic.as_ref()?;
                    (s.kind == StrategicKind::Interceptor && c.pos.distance(at) <= s.coverage)
                        .then_some(s.stock as u32)
                })
                .sum()
        };
        let own_value = |at: FxVec2| -> i64 {
            units
                .slots
                .iter()
                .filter(|&r| {
                    units.owner[r] == player
                        && units.pos[r].distance(at) <= blast.radius + Fx::from_int(100)
                })
                .map(|r| self.bp(r).cost_materials.floor_int() as i64)
                .sum()
        };
        let best = contacts
            .iter()
            .filter(|c| worth(c) > 0)
            .map(|c| {
                let value: i64 = contacts
                    .iter()
                    .filter(|o| o.pos.distance(c.pos) <= blast.radius)
                    .map(worth)
                    .sum();
                (value, c.pos)
            })
            .filter(|&(value, at)| {
                value >= spec.round_materials.floor_int() as i64 * NUKE_WORTH
                    && own_value(at) * 4 < value
                    && guarded(at) < free
            })
            .max_by_key(|&(value, at)| (value, std::cmp::Reverse((at.x, at.y))));
        let Some((_, at)) = best else {
            return;
        };
        // One warhead, or enough to get past the interceptors there.
        let salvo = (guarded(at) + 1).min(free);
        let ids: Vec<UnitId> = silos.iter().map(|&(r, _)| units.id(r)).collect();
        for _ in 0..salvo {
            out.push(Command::LaunchNuke {
                units: ids.clone(),
                pos: at,
            });
        }
    }

    /// Gravitic engines put their wells on the enemy spot worth most inside one: their
    /// army where it stands, or what is built. An engine looks again once its well has
    /// held `WELL_HOLD`, and turns when what it holds is worth less than half the best
    /// spot. Engines of the side never share a spot.
    pub(super) fn direct_gravitic(&self, player: u8, out: &mut Vec<Command>) {
        let units = &self.state.units;
        let tick = self.state.tick;
        let contacts = &self.state.ai[player as usize].contacts;
        let mut held: Vec<FxVec2> = Vec::new();
        for r in units.slots.iter() {
            if units.owner[r] != player || !units.is_active(r) {
                continue;
            }
            let Some(g) = self.bp(r).gravitic.as_ref() else {
                continue;
            };
            let id = units.id(r);
            // Aircraft take only a share of the crush.
            let worth = |c: &adaptive::Contact| -> i64 {
                let bp = self.blueprints.unit(c.blueprint);
                if bp.is_mobile() && tick.saturating_sub(c.seen) > FRESH {
                    return 0;
                }
                let m = if bp.has(cat::COMMANDER) {
                    COMMANDER_WORTH
                } else {
                    bp.cost_materials.floor_int() as i64
                };
                if bp.has(cat::AIR) {
                    (Fx::from_int(m as i32) * g.air_damage).floor_int() as i64
                } else {
                    m
                }
            };
            let value = |at: FxVec2| -> i64 {
                contacts
                    .iter()
                    .filter(|c| c.pos.distance(at) <= g.radius)
                    .map(worth)
                    .sum()
            };
            let well = self.state.wells.iter().find(|w| w.source == id);
            if let Some(w) = well {
                held.push(w.mark);
            }
            let settled = match well.map(|w| w.phase) {
                None => false,
                Some(WellPhase::Holding { age }) => age < WELL_HOLD,
                Some(_) => true,
            };
            if settled {
                continue;
            }
            let now = well.map_or(0, |w| value(w.mark));
            let best = contacts
                .iter()
                .filter(|c| worth(c) > 0)
                .filter(|c| !held.iter().any(|h| h.distance(c.pos) <= g.radius * 2))
                .map(|c| (value(c.pos), c.pos))
                .max_by_key(|&(v, at)| (v, std::cmp::Reverse((at.x, at.y))));
            let Some((_, at)) = best.filter(|&(v, _)| v >= WELL_WORTH && v > now * 2) else {
                continue;
            };
            held.push(at);
            out.push(Command::Fracture {
                units: vec![id],
                pos: at,
            });
        }
    }
}
