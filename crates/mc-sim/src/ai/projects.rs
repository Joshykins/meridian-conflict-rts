//! Strategic projects: which blueprints are experimental units and strategic
//! structures (`project_kind`; the Commander's plans decide which get built),
//! and firing the nukes (`direct_nukes`).
//!
//! Nothing here names a unit. A project is any blueprint in a builder's menu
//! with the `Experimental`, `Strategic` or `Space` category; its kind comes from
//! its data: a `strategic` launcher (nuke or interceptor), a structure with
//! anti-air (an anti-spaceship gun), one with artillery (a map gun), or a
//! mobile unit.
use super::*;
use mc_data::strategic::StrategicKind;

/// Enemy value a warhead must be able to reach, in multiples of its own
/// mass cost, to be fired.
const NUKE_WORTH: i64 = 2;
/// A contact seen this recently (ticks) is still where it was seen, for a
/// unit that moves.
const FRESH: u32 = 100;
/// Value put on the enemy commander under a warhead: its loss ends the game.
const COMMANDER_WORTH: i64 = 60_000;

/// What a project is for, from its data.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Project {
    /// Shoots down warheads: answers an enemy silo.
    Interceptor,
    /// A structure that shoots at aircraft: answers enemy spaceships.
    SkyGun,
    Nuke,
    /// A structure that shells from afar.
    MapGun,
    /// An experimental or a warship, built on a lot and sent with the army.
    Mobile,
}

pub(super) fn project_kind(bp: &UnitBlueprint) -> Option<Project> {
    if bp.categories & (cat::EXPERIMENTAL | cat::STRATEGIC | cat::SPACE) == 0
        || bp.has(cat::REPLICATOR)
        || bp.mine.is_some()
        || bp.transport.is_some()
        || bp.cost_mass <= Fx::ZERO
        // An unarmed ship (the Vigil) is no project: counted as one, each Vigil
        // finished freed the slot for the next, and sides had 30 to 60 of them
        // by forty minutes (`spotter_job` builds the one a side wants).
        || (bp.is_mobile() && bp.weapons.is_empty())
    {
        return None;
    }
    Some(match bp.strategic.as_ref().map(|s| s.kind) {
        Some(StrategicKind::Interceptor) => Project::Interceptor,
        Some(StrategicKind::Nuke) => Project::Nuke,
        None if bp.is_mobile() => Project::Mobile,
        None if bp.has(cat::ANTI_AIR) => Project::SkyGun,
        None if bp.has(cat::ARTILLERY) => Project::MapGun,
        None => Project::Mobile,
    })
}

impl World {
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
                bp.cost_mass.floor_int() as i64
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
                .map(|r| self.bp(r).cost_mass.floor_int() as i64)
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
                value >= spec.round_mass.floor_int() as i64 * NUKE_WORTH
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
}
