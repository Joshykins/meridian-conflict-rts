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
use super::strategy::Gambit;
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
/// Mass income a second before a side builds its radar ship.
const SPOTTER_INCOME: i32 = 25;

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
    /// Projects `player` has standing or going up, of each kind.
    pub(super) fn projects_held(&self, player: u8, kind: Project) -> usize {
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
        if energy_short {
            return None;
        }
        let builder = self.bp(row).builder.as_ref()?;
        // An answer to a threat may take twice as long to pay for.
        let affordable = |bp: &UnitBlueprint, urgent: bool| {
            let seconds =
                PROJECT_SECONDS * if bp.tech >= 5 { 2 } else { 1 } * if urgent { 2 } else { 1 };
            let budget = pl.mass_income * Fx::from_int(seconds) * Fx::ratio(PROJECT_SHARE, 100);
            let energy = pl.energy_income * Fx::from_int(seconds) * Fx::ratio(PROJECT_SHARE, 100);
            bp.cost_mass <= budget && bp.cost_energy <= energy
        };
        // A walker with no land route to the enemy would stand at home all match.
        let land_route = self.land_route_to_enemy(player);
        let buildable = |id: BlueprintId, urgent: bool| -> Option<(Project, BlueprintId)> {
            let bp = self.blueprints.unit(id);
            let kind = project_kind(bp)?;
            (affordable(bp, urgent) && !bp.water_only() && (land_route || !theatre::land_bound(bp)))
                .then_some((kind, id))
        };
        // Answers first: a silo seen gets an interceptor over the base, and
        // spaceships seen get a gun that reaches them. An answer goes up
        // alongside any other project, not after it.
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
            seen > 0
                && self.projects_held(player, ours) < seen.min(3)
                && builder
                    .builds
                    .iter()
                    .any(|&id| buildable(id, true).is_some_and(|(k, _)| k == ours))
        })
        .map(|(ours, _)| ours);
        // A siege or a nuke race keeps two going at once.
        let plan_room =
            (self.holds(player, Gambit::Siege) || self.holds(player, Gambit::NukeRace)) as usize;
        if answer.is_none() && planned.projects > mass_rich as usize + plan_room {
            return None;
        }
        let menu: Vec<(Project, BlueprintId)> = builder
            .builds
            .iter()
            .filter_map(|&id| buildable(id, answer.is_some()))
            .filter(|(k, _)| answer.is_none_or(|a| a == *k))
            .collect();
        if menu.is_empty() {
            return None;
        }
        // Otherwise the plans' kinds first (`strategy.rs`), then the doctrine's
        // order, the kind it holds fewest of first, so a long game sees the
        // whole strategic roster. With no land route, warships that cross the
        // water come first.
        let planned_kind = [
            (Gambit::NukeRace, Project::Nuke, 2),
            (Gambit::Siege, Project::MapGun, 3),
            (Gambit::WarpRaid, Project::Mobile, 2),
        ]
        .into_iter()
        .find(|&(g, k, most)| {
            self.holds(player, g)
                && menu.iter().any(|(m, _)| *m == k)
                && self.projects_held(player, k) < most
        })
        .map(|(_, k, _)| k);
        let order: &[Project] = match persona {
            _ if !land_route => &[Project::Mobile, Project::Nuke, Project::MapGun],
            Personality::Aggressive => &[Project::Mobile, Project::Nuke, Project::MapGun],
            Personality::Expander => &[Project::Nuke, Project::Mobile, Project::MapGun],
            Personality::Turtle => &[Project::MapGun, Project::Nuke, Project::Mobile],
        };
        let kind = answer.or(planned_kind).or_else(|| {
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

    /// One unarmed radar ship of the upper air to go with the army, once the
    /// side has the income to spare (`SPOTTER_INCOME`).
    pub(super) fn spotter_job(&self, row: usize, start: FxVec2, facing: Angle) -> Option<Job> {
        let player = self.state.units.owner[row];
        // Scouting as a plan: the sensor ship comes as soon as there is a little to spare.
        let income = if self.holds(player, Gambit::Scouting) {
            SPOTTER_INCOME / 3
        } else {
            SPOTTER_INCOME
        };
        if self.state.players[player as usize].mass_income < Fx::from_int(income) {
            return None;
        }
        let spotter = |id: BlueprintId| {
            let bp = self.blueprints.unit(id);
            bp.has(cat::SPACE) && bp.is_mobile() && bp.weapons.is_empty() && bp.radar > Fx::ZERO
        };
        let units = &self.state.units;
        let held = units
            .slots
            .iter()
            .any(|r| units.owner[r] == player && spotter(units.blueprint[r]))
            || self
                .planned_sites(player)
                .any(|(_, o)| spotter(o.blueprint));
        if held {
            return None;
        }
        let blueprint = self
            .bp(row)
            .builder
            .as_ref()?
            .builds
            .iter()
            .copied()
            .find(|&id| spotter(id))?;
        Some(Job {
            blueprint,
            near: start + FxVec2::from_angle(facing + Angle::HALF_TURN) * Fx::from_int(260),
            heading: self.blueprints.unit(blueprint).build_heading(),
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
