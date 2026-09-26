//! Strategic projects: the experimental units and strategic structures a
//! side's best builders put up once its economy can carry them, and the nukes
//! it fires. Before this the AI never built anything past tech 3, so a long
//! game against it never reached the part of the game worth playing for.
//!
//! Nothing here names a unit. A project is any blueprint in a builder's menu
//! with the `Experimental`, `Strategic` or `Space` category; its kind comes from
//! its data: a `strategic` launcher (nuke or interceptor), a structure with
//! anti-air (an anti-spaceship gun), one with artillery (a map gun), or a
//! mobile unit.
use super::*;
use mc_data::strategic::StrategicKind;

/// Seconds of the side's mass income, at the share below, a tech 4 project
/// may cost to be worth starting; tech 5 gets twice as long.
const PROJECT_SECONDS: i32 = 300;
/// The share of mass income a project may take while it goes up (percent).
const PROJECT_SHARE: i64 = 60;
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
    /// Projects `player` has standing or going up, of each kind.
    fn projects_held(&self, player: u8, kind: Project) -> usize {
        let units = &self.state.units;
        units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && project_kind(self.bp(r)) == Some(kind))
            .count()
    }

    /// Enemy contacts `player` remembers whose blueprint is of `kind`.
    fn enemy_projects(&self, player: u8, kind: Project) -> usize {
        self.state.ai[player as usize]
            .contacts
            .iter()
            .filter(|c| project_kind(self.blueprints.unit(c.blueprint)) == Some(kind))
            .count()
    }

    /// The project an idle builder should start, if the side can carry one.
    pub(super) fn project_job(
        &self,
        row: usize,
        persona: Personality,
        start: FxVec2,
        facing: Angle,
        planned: &Planned,
        energy_short: bool,
    ) -> Option<Job> {
        let player = self.state.units.owner[row];
        let pl = &self.state.players[player as usize];
        let mass_rich = pl.mass > pl.mass_capacity * Fx::ratio(7, 10);
        if energy_short || planned.projects > mass_rich as usize {
            return None;
        }
        let builder = self.bp(row).builder.as_ref()?;
        let affordable = |bp: &UnitBlueprint| {
            let seconds = PROJECT_SECONDS * if bp.tech >= 5 { 2 } else { 1 };
            let budget = pl.mass_income * Fx::from_int(seconds) * Fx::ratio(PROJECT_SHARE, 100);
            let energy = pl.energy_income * Fx::from_int(seconds) * Fx::ratio(PROJECT_SHARE, 100);
            bp.cost_mass <= budget && bp.cost_energy <= energy
        };
        let menu: Vec<(Project, BlueprintId)> = builder
            .builds
            .iter()
            .filter_map(|&id| {
                let bp = self.blueprints.unit(id);
                let kind = project_kind(bp)?;
                (affordable(bp) && !bp.water_only()).then_some((kind, id))
            })
            .collect();
        if menu.is_empty() {
            return None;
        }
        // Answers first: a silo seen gets an interceptor over the base, and
        // spaceships seen get a gun that reaches them.
        let answer = [
            (Project::Interceptor, Project::Nuke),
            (Project::SkyGun, Project::Mobile),
        ]
        .into_iter()
        .find(|&(ours, theirs)| {
            let seen = match theirs {
                Project::Mobile => self.enemy_spaceships(player),
                _ => self.enemy_projects(player, theirs),
            };
            seen > 0 && self.projects_held(player, ours) < seen.min(3)
        })
        .map(|(ours, _)| ours);
        // Otherwise the doctrine's order, the kind it holds fewest of first,
        // so a long game sees the whole strategic roster.
        let order: &[Project] = match persona {
            Personality::Aggressive => &[Project::Mobile, Project::Nuke, Project::MapGun],
            Personality::Expander => &[Project::Nuke, Project::Mobile, Project::MapGun],
            Personality::Turtle => &[Project::MapGun, Project::Nuke, Project::Mobile],
        };
        let kind = answer.or_else(|| {
            order
                .iter()
                .enumerate()
                .filter(|(_, k)| menu.iter().any(|(m, _)| m == *k))
                .min_by_key(|&(i, k)| (self.projects_held(player, *k), i))
                .map(|(_, k)| *k)
        })?;
        // The best of that kind the side can carry.
        let (_, blueprint) = menu
            .iter()
            .filter(|(k, _)| *k == kind)
            .max_by_key(|(_, id)| {
                (
                    self.blueprints.unit(*id).tech,
                    self.blueprints.unit(*id).cost_mass,
                    std::cmp::Reverse(id.0),
                )
            })?;
        let bp = self.blueprints.unit(*blueprint);
        let back = FxVec2::from_angle(facing + Angle::HALF_TURN);
        // Guns that answer something stand toward the enemy; the rest behind
        // the factories, where a raid reaches them last.
        let near = match kind {
            Project::SkyGun => start + FxVec2::from_angle(facing) * Fx::from_int(160),
            _ => start + back * Fx::from_int(260),
        };
        Some(Job {
            blueprint: *blueprint,
            near,
            heading: bp.build_heading(),
            min_r: Fx::from_int(24),
            keep_off_deposits: true,
            place: Place::Around,
        })
    }

    /// Enemy spaceships `player` remembers.
    fn enemy_spaceships(&self, player: u8) -> usize {
        self.state.ai[player as usize]
            .contacts
            .iter()
            .filter(|c| {
                let bp = self.blueprints.unit(c.blueprint);
                bp.has(cat::SPACE) && bp.transport.is_none() && !bp.weapons.is_empty()
            })
            .count()
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
