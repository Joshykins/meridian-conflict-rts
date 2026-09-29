//! What the AI's builders do: the job each idle builder takes, where it goes,
//! and helping with sites already started.
use super::*;

/// A firebase farther than this from the start gets no factory of its own.
const FORWARD_FACTORY_REACH: Fx = Fx::from_int(3000);

impl World {
    pub(super) fn direct_builders(
        &self,
        player: u8,
        census: &Census,
        intel: &Intel,
        stance: Stance,
        persona: Personality,
        start: FxVec2,
        facing: Angle,
        firebase: Option<FxVec2>,
        claimed: &mut Vec<Claim>,
        planned: &mut Planned,
        out: &mut Vec<Command>,
    ) {
        let pl = &self.state.players[player as usize];
        // Short when the store really drains, not when builders merely ask for more
        // than comes in: in a mass stall they get only a share of their energy and the
        // store fills while the demand reads high.
        let energy_short = pl.energy_spent > pl.energy_income || pl.energy < pl.energy_capacity / 5;
        let mass_rich = pl.mass > pl.mass_capacity * Fx::ratio(7, 10);
        let mass_income = pl.mass_income;
        let energy_income = pl.energy_income;

        let skill = self.state.ai[player as usize].config.skill();
        let home = (!census.builders_idle.is_empty())
            .then(|| self.home_ground(start, 1))
            .flatten();
        // Sites going up at once: more than the income can feed and every one
        // of them crawls. Five turrets half built and none finished at six
        // minutes, with ten engineers each on its own.
        let mass_stalling = pl.mass < pl.mass_capacity / 20 && pl.mass_demand > mass_income;
        let site_cap = if mass_stalling {
            1
        } else {
            3 + (mass_income / Fx::from_int(10)).floor_int() as usize
        };
        // The best builders choose first, so they take the big plants.
        let mut idle = census.builders_idle.clone();
        idle.sort_by_key(|&r| (std::cmp::Reverse(self.builder_tech(r)), r));
        let best = self.best_builder_tech(player);
        let power_wanted = energy_short || pl.energy_income < census.energy_need;
        for &row in idle.iter().take(skill.builders_per_think) {
            // A lesser builder helps raise the side's plants rather than starting
            // a small one of its own (`choose_job`).
            if power_wanted && self.builder_tech(row) < best {
                if let Some(site) = census
                    .sites
                    .iter()
                    .copied()
                    .filter(|&s| {
                        self.bp(s).has(cat::POWER)
                            && self.within_reach(row, self.state.units.pos[s])
                            && !intel.danger.hot(self.state.units.pos[s])
                    })
                    .min_by_key(|&s| {
                        (
                            self.state.units.pos[s].distance_sq(self.state.units.pos[row]),
                            s,
                        )
                    })
                {
                    out.push(Command::Assist {
                        units: vec![self.state.units.id(row)],
                        target: self.state.units.id(site),
                        queue: false,
                    });
                    continue;
                }
            }
            if let Some(site) = census
                .sites
                .iter()
                .copied()
                .filter(|&s| {
                    ((energy_short && self.bp(s).has(cat::POWER)) || census.sites.len() >= site_cap)
                        && self.within_reach(row, self.state.units.pos[s])
                        && !intel.danger.hot(self.state.units.pos[s])
                })
                .min_by_key(|&s| {
                    (
                        !self.bp(s).has(cat::POWER),
                        self.state.units.pos[s].distance_sq(self.state.units.pos[row]),
                    )
                })
            {
                out.push(Command::Assist {
                    units: vec![self.state.units.id(row)],
                    target: self.state.units.id(site),
                    queue: false,
                });
                continue;
            }
            let is_commander = self.bp(row).has(cat::COMMANDER);
            let job = self.choose_job(
                row,
                is_commander,
                census,
                intel,
                stance,
                persona,
                start,
                facing,
                firebase,
                claimed,
                planned,
                energy_short,
                mass_rich,
                mass_income,
                energy_income,
                home.as_ref(),
            );
            match job {
                Some(job) => {
                    let bp = self.blueprints.unit(job.blueprint).clone();
                    let site = match job.place {
                        Place::Around => self.find_site(
                            &bp,
                            job.near,
                            claimed,
                            facing,
                            job.min_r,
                            job.keep_off_deposits,
                            home.as_ref(),
                        ),
                        Place::Packed(radius) => self.pack_site(
                            &bp,
                            job.near,
                            radius,
                            1,
                            claimed,
                            job.keep_off_deposits,
                            home.as_ref(),
                        ),
                        Place::Farm => self.farm_site(&bp, start, facing, claimed, home.as_ref()),
                    };
                    if let Some(site) =
                        site.filter(|s| self.can_place(&bp, *s) && !intel.danger.hot(*s))
                    {
                        claimed.push(Claim {
                            pos: site,
                            foot: bp.footprint.0.max(bp.footprint.1) as i32,
                            mine: bp.mine.is_some(),
                            factory: bp.has(cat::FACTORY),
                            cover: bp.shield.as_ref().map_or(Fx::ZERO, |s| s.radius),
                        });
                        if bp.has(cat::FACTORY) {
                            planned.factories += 1;
                            if self.blueprint_trains_engineers(&bp) {
                                planned.engineer_factories += 1;
                            }
                            if bp.has(cat::AIR) {
                                planned.air_factories += 1;
                            }
                        }
                        planned.anti_air += bp.has(cat::DEFENSE | cat::ANTI_AIR) as usize;
                        planned.power += bp.has(cat::POWER) as usize;
                        if let Some(range) = land_radar(&bp) {
                            planned.radars.push((site, range));
                        }
                        if let Some(r) = bp.reclaimer.filter(|_| bp.is_structure()) {
                            planned.towers.push((site, r.range));
                        }
                        planned.artillery +=
                            (bp.has(cat::DEFENSE) && bp.has(cat::ARTILLERY)) as usize;
                        planned.shields += bp.has(cat::SHIELD) as usize;
                        planned.storage += bp.has(cat::STORAGE) as usize;
                        planned.projects += projects::project_kind(&bp).is_some() as usize;
                        if bp.has(cat::DEFENSE) && bp.has(cat::DIRECT_FIRE) {
                            planned.pd += 1;
                            planned.guards.push(site);
                        } else if bp.has(cat::SHIELD) || bp.has(cat::ARTILLERY) {
                            planned.guards.push(site);
                        }
                        out.push(Command::Build {
                            units: vec![self.state.units.id(row)],
                            blueprint: job.blueprint,
                            pos: site,
                            heading: if bp.is_structure() {
                                AI_BUILD_HEADING
                            } else {
                                job.heading
                            },
                            queue: false,
                        });
                    }
                }
                None => {
                    let pos = self.state.units.pos[row];
                    // A site under the enemy's guns is left until they are gone.
                    let safe = |s: &usize| !intel.danger.hot(self.state.units.pos[*s]);
                    let assist_site = census
                        .sites
                        .iter()
                        .copied()
                        .filter(safe)
                        .filter(|&s| self.within_reach(row, self.state.units.pos[s]))
                        .filter(|&s| !self.bp(s).has(cat::EXTRACTOR))
                        // A project first: it is what every spare builder at
                        // home should be putting up.
                        .min_by_key(|s| {
                            (
                                projects::project_kind(self.bp(*s)).is_none(),
                                self.state.units.pos[*s].distance_sq(pos),
                                *s as u32,
                            )
                        })
                        .or_else(|| {
                            census
                                .sites
                                .iter()
                                .copied()
                                .filter(safe)
                                .filter(|&s| self.within_reach(row, self.state.units.pos[s]))
                                .min_by_key(|s| {
                                    (self.state.units.pos[*s].distance_sq(pos), *s as u32)
                                })
                        });
                    // Then an engineer going up a tier: on its own power alone
                    // the upgrade takes minutes, longer still short of materials.
                    let assist_site = assist_site.or_else(|| self.engineer_upgrading_near(row));
                    if let Some(site) = assist_site {
                        out.push(Command::Assist {
                            units: vec![self.state.units.id(row)],
                            target: self.state.units.id(site),
                            queue: false,
                        });
                    } else if let Some(&hurt) = census
                        .damaged
                        .iter()
                        .min_by_key(|&&s| (self.state.units.pos[s].distance_sq(pos), s as u32))
                    {
                        out.push(Command::Assist {
                            units: vec![self.state.units.id(row)],
                            target: self.state.units.id(hurt),
                            queue: false,
                        });
                    } else if let Some(wreck) = self.near_wreck(pos, Fx::from_int(480)) {
                        out.push(Command::ReclaimWreck {
                            units: vec![self.state.units.id(row)],
                            wreck,
                            queue: false,
                        });
                    } else if let Some(spot) =
                        firebase.filter(|f| !is_commander && !intel.danger.hot(*f))
                    {
                        if pos.distance(spot) > Fx::from_int(220) {
                            out.push(Command::Move {
                                units: vec![self.state.units.id(row)],
                                target: spot,
                                queue: false,
                            });
                        } else if let Some(&f) = census.factories.first() {
                            out.push(Command::Assist {
                                units: vec![self.state.units.id(row)],
                                target: self.state.units.id(f),
                                queue: false,
                            });
                        }
                    } else if let Some(&f) = census
                        .factories
                        .iter()
                        .min_by_key(|&&f| (self.state.units.pos[f].distance_sq(pos), f as u32))
                    {
                        out.push(Command::Assist {
                            units: vec![self.state.units.id(row)],
                            target: self.state.units.id(f),
                            queue: false,
                        });
                    }
                }
            }
        }
    }

    /// The nearest engineer of `row`'s side at work on its own upgrade, within reach.
    fn engineer_upgrading_near(&self, row: usize) -> Option<usize> {
        let units = &self.state.units;
        let pos = units.pos[row];
        units
            .slots
            .iter()
            .filter(|&r| {
                r != row
                    && units.owner[r] == units.owner[row]
                    && units.is_active(r)
                    && self.bp(r).is_mobile()
                    && self.bp(r).has(cat::ENGINEER)
                    && self
                        .state
                        .orders
                        .front(units, r)
                        .is_some_and(|o| matches!(o.kind, OrderKind::Upgrade))
                    && self.within_reach(row, units.pos[r])
            })
            .min_by_key(|&r| (units.pos[r].distance_sq(pos), r))
    }

    /// The tier of the side's best finished mobile builder, its commander included.
    pub(super) fn best_builder_tech(&self, player: u8) -> u8 {
        let units = &self.state.units;
        units
            .slots
            .iter()
            .filter(|&r| {
                units.owner[r] == player
                    && units.is_active(r)
                    && self.bp(r).is_mobile()
                    && self.bp(r).builder.is_some()
            })
            .map(|r| self.builder_tech(r))
            .max()
            .unwrap_or(1)
    }

    pub(super) fn choose_job(
        &self,
        row: usize,
        is_commander: bool,
        census: &Census,
        intel: &Intel,
        stance: Stance,
        persona: Personality,
        start: FxVec2,
        facing: Angle,
        firebase: Option<FxVec2>,
        claimed: &[Claim],
        planned: &Planned,
        energy_short: bool,
        mass_rich: bool,
        mass_income: Fx,
        energy_income: Fx,
        home_ground: Option<&staging::HomeGround>,
    ) -> Option<Job> {
        let front = intel
            .enemy_start
            .unwrap_or(start + FxVec2::from_angle(facing) * Fx::from_int(400));
        let home = |p: FxVec2| p.distance(start) <= HOME_RADIUS;
        // Never under an enemy's guns, nor where the last one was just shot down.
        let allow = |p: FxVec2| (!is_commander || home(p)) && !intel.danger.hot(p);
        let skill = self.state.ai[self.state.units.owner[row] as usize]
            .config
            .skill();
        let want_factories = {
            let extra = match persona {
                Personality::Expander => 1,
                Personality::Aggressive if matches!(stance, Stance::Push | Stance::Firebase) => 1,
                _ => 0,
            };
            1 + extra + (mass_income / Fx::from_int(7)).floor_int().clamp(0, 5) as usize
        };
        let want_power = 2 + planned.factories * 3 + (energy_short as usize) * 3;
        let tech = self.builder_tech(row);
        // Once the side has a builder of a higher tier and some power, a tech 1 builder
        // leaves new power to it (a bigger plant is far cheaper per unit of energy) and
        // helps build that instead: idle builders assist power sites while energy is short.
        let owner = self.state.units.owner[row];
        // Materials going unspent: the factories cannot use what comes in. Another
        // factory, not another reactor or turret, is what the side lacks.
        let piling = {
            let pl = &self.state.players[owner as usize];
            pl.mass > pl.mass_capacity * Fx::ratio(2, 5) && pl.mass_demand < mass_income
        };
        // Or the factories standing could not spend the income if all were
        // busy: with 100 mass a second coming in, two factories sat on a full
        // store for twenty minutes.
        let underspent = self.factory_mass_draw(owner) < mass_income * Fx::ratio(7, 10);
        let want_factories = if piling || underspent {
            want_factories.max(planned.factories + 1)
        } else {
            want_factories
        }
        .min(skill.factory_cap as usize);
        // A plant from a lesser builder is a poor one: a Reactor II gives 250 a
        // second for 700 materials, a Reactor 20 for 75. Once the side has a
        // better builder, a lesser one helps raise its plants (`direct_builders`)
        // and starts none of its own, unless the side's energy has run out with
        // no plant going up at all. T1 builders used to dot 50 to 100 small
        // reactors about the base.
        let leave_power = tech < self.best_builder_tech(owner);
        // The biggest plant the builder can put up; a Reactor III only once the
        // income makes its price a couple of minutes' worth.
        let plant_tech = if tech >= 3 && mass_income >= Fx::from_int(22) {
            3
        } else {
            tech.clamp(1, 2)
        };
        let energy_gone = {
            let pl = &self.state.players[owner as usize];
            pl.energy < pl.energy_capacity / 20
        };
        // A builder far out works where it is; the base is left to those at home,
        // or it spends minutes walking back for every job.
        let far = self.state.units.pos[row].distance(start) > FAR_FROM_HOME;
        // Power goes in the base's farms (`layout`). A builder far out leaves it
        // to those at home: energy is the whole side's wherever it is made, and
        // plants dropped wherever a builder stood littered the map.
        let power_job = |ptech: u8| {
            if far {
                return None;
            }
            self.job_structure(row, cat::POWER, ptech, start, facing, Fx::ZERO, true)
                .map(|job| Job {
                    place: Place::Farm,
                    ..job
                })
        };
        let factory_job = || {
            // Past the first two, a factory is left to the side's best builders:
            // a tech 1 factory late on is nearly no build power at all.
            // A tech 1 builder still puts one up: it is upgraded in its turn.
            if planned.factories >= want_factories || (far && planned.factories >= 1) {
                return None;
            }
            let idx = planned.factories;
            let want_air = planned.air_factories == 0
                && planned.engineer_factories >= 1
                && (mass_income >= Fx::from_int(6)
                    || matches!(stance, Stance::Push | Stance::Raid));
            // A forward factory only at a firebase near enough to supply: on a
            // big map every factory past the second went eight kilometres out,
            // and none of them was ever finished.
            let near = match firebase {
                Some(firebase)
                    if idx >= 2
                        && matches!(stance, Stance::Firebase | Stance::Push)
                        && firebase.distance(start) <= FORWARD_FACTORY_REACH =>
                {
                    firebase
                }
                _ => self.yard_anchor(start, facing, idx),
            };
            if !allow(near) {
                return None;
            }
            // A new factory is built at the best tier the income can keep busy.
            let ftech = if tech >= 3 && mass_income >= Fx::from_int(60) {
                3
            } else if tech >= 2 && mass_income >= Fx::from_int(12) {
                2
            } else {
                1
            };
            self.pick_factory(row, ftech, want_air)
                .map(|blueprint| Job {
                    blueprint,
                    near,
                    heading: facing,
                    min_r: Fx::from_int(12),
                    keep_off_deposits: true,
                    place: Place::Around,
                })
        };

        if planned.engineer_factories == 0 {
            let near = self.yard_anchor(start, facing, 0);
            if allow(near) {
                if let Some(blueprint) = self.pick_factory(row, 1, false) {
                    return Some(Job {
                        blueprint,
                        near,
                        heading: facing,
                        min_r: Fx::from_int(8),
                        keep_off_deposits: true,
                        place: Place::Around,
                    });
                }
            }
        }
        // A side without mines has nothing to build with: its first few come
        // before anything else, on bare ground if there is no ore near.
        let mines = census.extractors.len()
            + census
                .sites
                .iter()
                .filter(|&&s| self.bp(s).has(cat::EXTRACTOR))
                .count();
        if mines < FIRST_MINES {
            let range = if is_commander {
                HOME_RADIUS
            } else {
                self.mex_range(false, census, persona, stance, skill)
            };
            let least = Some(Fx::ratio(skill.bare_mine_efficiency as i64, 100));
            if let Some(spot) = self.free_deposit(start, claimed, range, intel, least) {
                if allow(spot) {
                    return self.job_structure(
                        row,
                        cat::EXTRACTOR,
                        1,
                        spot,
                        facing,
                        Fx::ZERO,
                        false,
                    );
                }
            }
        }
        // Power before anything else in a stall. A lesser builder leaves new
        // plants to a better one unless the energy is gone and none is going up.
        let power_rising = census.sites.iter().any(|&s| self.bp(s).has(cat::POWER));
        if energy_short && (!leave_power || (energy_gone && !power_rising)) {
            if let Some(job) = power_job(plant_tech) {
                return Some(job);
            }
        }
        // Claim the mexes around the start before stacking reactors.
        if let Some(deposit) = self.free_deposit(start, claimed, Fx::from_int(480), intel, None) {
            if allow(deposit) {
                return self.job_structure(
                    row,
                    cat::EXTRACTOR,
                    1,
                    deposit,
                    facing,
                    Fx::ZERO,
                    false,
                );
            }
        }
        if energy_short && !leave_power && planned.power < want_power.min(6) {
            if let Some(job) = power_job(plant_tech) {
                return Some(job);
            }
        }
        // One watchtower first: it is cheap, and power always short of the
        // side's whole draw kept it from ever going up.
        if planned.radars.is_empty()
            && planned.power >= 2
            && (planned.pd >= 1 || census.extractors.len() >= FIRST_MINES)
        {
            let near = offset_toward(start, front, Fx::from_int(140))
                + FxVec2::from_angle(facing + Angle::QUARTER_TURN) * Fx::from_int(72);
            if allow(near) {
                return self.job_structure(
                    row,
                    cat::INTEL,
                    1,
                    near,
                    facing,
                    Fx::from_int(20),
                    true,
                );
            }
        }
        // Ahead of the stall, not in it: power for what every factory and
        // builder would draw at work, not only for what they ask for now,
        // and before far mines and turrets: behind them, the side
        // ran out of energy a tenth of the game.
        if !leave_power
            && (planned.power < want_power
                || energy_income < mass_income * skill.power_ratio
                || energy_income < census.energy_need)
        {
            // The AI pays power first in a stall (`direct_focus`), so a short side
            // still builds the biggest plant it can.
            if let Some(job) = power_job(plant_tech) {
                return Some(job);
            }
        }
        // A wreck field near home pays a cheap tower back in well under a minute.
        if !far && !energy_short {
            if let Some(job) = self.salvage_job(row, planned, &allow) {
                return Some(job);
            }
        }
        let owner = self.state.units.owner[row] as usize;
        let air_threats = self.state.ai[owner]
            .contacts
            .iter()
            .filter(|c| {
                let enemy = self.blueprints.unit(c.blueprint);
                enemy
                    .motion
                    .is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
                    && !enemy.weapons.is_empty()
            })
            .count();
        if self.state.ai[owner].config.adaptation > 0
            && !far
            && air_threats > 0
            && planned.anti_air < (1 + air_threats / 4).min(4)
        {
            let near = offset_toward(start, front, Fx::from_int(100));
            if let Some(job) = self.job_structure(
                row,
                cat::DEFENSE | cat::ANTI_AIR,
                tech.min(2),
                near,
                facing,
                Fx::from_int(16),
                true,
            ) {
                return Some(job);
            }
        }
        // Materials going unspent come before another far-off mine.
        if piling && !energy_short {
            if let Some(job) = factory_job() {
                return Some(job);
            }
        }
        // An experimental or a strategic weapon, once the economy carries one.
        if !far {
            if let Some(job) = self.project_job(row, persona, start, facing, planned, energy_short)
            {
                return Some(job);
            }
        }
        let mex_range = self.mex_range(is_commander, census, persona, stance, skill);
        let bare = (!energy_short && !census.pd.is_empty())
            .then(|| Fx::ratio(skill.bare_mine_efficiency as i64, 100));
        if let Some(deposit) = self.free_deposit(start, claimed, mex_range, intel, bare) {
            if allow(deposit) {
                return self.job_structure(
                    row,
                    cat::EXTRACTOR,
                    1,
                    deposit,
                    facing,
                    Fx::ZERO,
                    false,
                );
            }
        }
        if let Some(mex) = self.unguarded_mex(census, planned, intel, stance, mass_income) {
            let toward = intel
                .threats
                .iter()
                .find(|(at, _)| at.distance(mex) < Fx::from_int(40))
                .map(|(_, e)| *e)
                .or(intel.enemy_start)
                .unwrap_or(front);
            let near = offset_toward(mex, toward, Fx::from_int(84));
            let pd_tech = if census.max_tech >= 2
                && tech >= 2
                && (stance == Stance::Defend || mass_income >= Fx::from_int(10))
            {
                2
            } else {
                1
            };
            if allow(near) {
                return self.job_structure(
                    row,
                    cat::DEFENSE | cat::DIRECT_FIRE,
                    pd_tech,
                    near,
                    (toward - mex).angle(),
                    Fx::from_int(18),
                    false,
                );
            }
        }
        if let Some(job) = factory_job() {
            return Some(job);
        }
        if matches!(stance, Stance::Firebase | Stance::Push | Stance::Raid) {
            if let Some(spot) = firebase {
                if allow(spot) {
                    if let Some(job) =
                        self.firebase_job(row, census, planned, spot, facing, front, tech)
                    {
                        return Some(job);
                    }
                }
            }
            if let Some(job) = self.forward_gun(row, census, intel, planned, is_commander, tech) {
                return Some(job);
            }
        }
        if stance == Stance::Defend {
            if let Some(&(at, enemy)) = intel.threats.first() {
                let near = offset_toward(at, enemy, Fx::from_int(90));
                let covered = planned
                    .guards
                    .iter()
                    .any(|g| g.distance(near) < GUARD_COVER);
                if allow(near) && !covered {
                    return self.job_structure(
                        row,
                        cat::DEFENSE | cat::DIRECT_FIRE,
                        tech.clamp(1, 2),
                        near,
                        (enemy - at).angle(),
                        Fx::from_int(20),
                        false,
                    );
                }
            }
        }
        if tech >= 2
            && !far
            && mass_income >= Fx::from_int(10)
            && planned.shields < 1 + planned.factories / 3
        {
            // Over the part of the base no shield covers yet, and only where
            // that is worth the shield's cost and upkeep.
            if let Some(blueprint) = self.pick_structure(row, cat::SHIELD, 2) {
                let bp = self.blueprints.unit(blueprint);
                // The lot is chosen here: a base with no room for one goes on
                // to its next job instead of retrying the shield every think.
                if let Some(site) = self
                    .shield_spot(owner as u8, bp, start, claimed)
                    .and_then(|spot| {
                        self.shield_site(owner as u8, bp, spot, start, claimed, home_ground)
                    })
                    .filter(|&site| allow(site))
                {
                    return Some(Job {
                        blueprint,
                        near: site,
                        heading: facing,
                        min_r: Fx::ZERO,
                        keep_off_deposits: true,
                        place: Place::Packed(0),
                    });
                }
            }
        }
        if mass_rich && !far && planned.storage < 1 + planned.factories {
            let near = self.yard_anchor(start, facing, 0)
                + FxVec2::from_angle(facing + Angle::HALF_TURN) * Fx::from_int(40);
            if allow(near) {
                if let Some(blueprint) = self.pick_storage(row) {
                    // With the big plants, in rows, not dotted between the factories.
                    return Some(Job {
                        blueprint,
                        near,
                        heading: facing + Angle::HALF_TURN,
                        min_r: Fx::from_int(16),
                        keep_off_deposits: true,
                        place: Place::Farm,
                    });
                }
            }
        }
        // A tower only where no other one sees already: its range is kilometres.
        if !planned.radars.is_empty() {
            if let Some(&mex) = census
                .extractor_pos
                .iter()
                .find(|m| !radar_covers(&planned.radars, **m))
            {
                if allow(mex) {
                    return self.job_structure(
                        row,
                        cat::INTEL,
                        1,
                        offset_toward(mex, front, Fx::from_int(60)),
                        facing,
                        Fx::from_int(16),
                        true,
                    );
                }
            }
        }
        // A few turrets on the side facing the enemy; spare materials go to factories.
        if stance == Stance::Defend {
            let near = offset_toward(start, front, Fx::from_int(180));
            let at_base = planned
                .guards
                .iter()
                .filter(|g| g.distance(near) < Fx::from_int(260))
                .count();
            if allow(near) && at_base < 2 + planned.factories / 2 {
                return self.job_structure(
                    row,
                    cat::DEFENSE | cat::DIRECT_FIRE,
                    tech,
                    near,
                    facing,
                    Fx::from_int(28),
                    true,
                );
            }
        }
        None
    }

    pub(super) fn firebase_job(
        &self,
        row: usize,
        census: &Census,
        planned: &Planned,
        spot: FxVec2,
        facing: Angle,
        front: FxVec2,
        tech: u8,
    ) -> Option<Job> {
        let around = |p: &FxVec2| p.distance(spot) < Fx::from_int(280);
        let pd = census
            .pd
            .iter()
            .chain(planned.guards.iter())
            .filter(|p| around(p))
            .count();
        let power = census.power.iter().filter(|p| around(p)).count();
        let arty = census.artillery.iter().filter(|p| around(p)).count();
        if pd < 2 {
            let near = offset_toward(spot, front, Fx::from_int(70));
            return self.job_structure(
                row,
                cat::DEFENSE | cat::DIRECT_FIRE,
                tech.clamp(1, 2),
                near,
                facing,
                Fx::from_int(16),
                false,
            );
        }
        if power < 2 {
            // Side by side just behind the guns, not strewn around the spot.
            return self
                .job_structure(
                    row,
                    cat::POWER,
                    1,
                    spot + FxVec2::from_angle(facing + Angle::HALF_TURN) * Fx::from_int(70),
                    facing + Angle::HALF_TURN,
                    Fx::ZERO,
                    true,
                )
                .map(|job| Job {
                    place: Place::Packed(90),
                    ..job
                });
        }
        if !radar_covers(&planned.radars, spot) {
            return self.job_structure(
                row,
                cat::INTEL,
                1,
                offset_toward(spot, front, Fx::from_int(40)),
                facing,
                Fx::from_int(16),
                true,
            );
        }
        if tech >= 2 && arty < 1 && planned.artillery < 2 {
            return self.job_structure(
                row,
                cat::DEFENSE | cat::ARTILLERY,
                2,
                spot + FxVec2::from_angle(facing + Angle::HALF_TURN) * Fx::from_int(90),
                facing,
                Fx::from_int(24),
                true,
            );
        }
        if tech >= 2 && planned.towers.is_empty() {
            if let Some(blueprint) = self.pick_reclaimer(row) {
                return Some(Job {
                    blueprint,
                    near: spot,
                    heading: facing,
                    min_r: Fx::from_int(20),
                    keep_off_deposits: true,
                    place: Place::Around,
                });
            }
        }
        None
    }

    pub(super) fn forward_gun(
        &self,
        row: usize,
        census: &Census,
        intel: &Intel,
        planned: &Planned,
        is_commander: bool,
        tech: u8,
    ) -> Option<Job> {
        if is_commander || intel.enemy_extractors.is_empty() {
            return None;
        }
        let army_pos = census.army_idle.iter().map(|&r| self.state.units.pos[r]);
        for &mex in &intel.enemy_extractors {
            let nearby_army = army_pos
                .clone()
                .filter(|p| p.distance(mex) < Fx::from_int(420))
                .count();
            if nearby_army < 3 {
                continue;
            }
            if planned.guards.iter().any(|g| g.distance(mex) < GUARD_COVER) {
                continue;
            }
            let home = self.state.units.pos[row];
            let near = offset_toward(mex, home, Fx::from_int(110));
            if intel.danger.hot(near) {
                continue;
            }
            return self.job_structure(
                row,
                cat::DEFENSE | cat::DIRECT_FIRE,
                tech.clamp(1, 2),
                near,
                (mex - near).angle(),
                Fx::from_int(16),
                false,
            );
        }
        None
    }

    pub(super) fn job_structure(
        &self,
        builder_row: usize,
        categories: u32,
        max_tech: u8,
        near: FxVec2,
        heading: Angle,
        min_r: Fx,
        keep_off_deposits: bool,
    ) -> Option<Job> {
        self.pick_structure(builder_row, categories, max_tech)
            .map(|blueprint| Job {
                blueprint,
                near,
                heading,
                min_r,
                keep_off_deposits,
                place: Place::Around,
            })
    }

    pub(super) fn unguarded_mex(
        &self,
        census: &Census,
        planned: &Planned,
        intel: &Intel,
        stance: Stance,
        mass_income: Fx,
    ) -> Option<FxVec2> {
        let want = if stance == Stance::Defend { 2 } else { 1 };
        // A turret on every mine in the opening cost the first factories: a
        // quiet mine waits until the opening's mines stand, one under attack
        // does not.
        let settled = mass_income >= Fx::from_int(6) || census.extractors.len() > FIRST_MINES;
        let mut best: Option<(i32, Fx, FxVec2)> = None;
        for &mex in &census.extractor_pos {
            let guards = planned
                .guards
                .iter()
                .filter(|g| g.distance(mex) <= GUARD_COVER)
                .count();
            if guards >= want {
                continue;
            }
            let threatened = intel
                .threats
                .iter()
                .any(|(at, _)| at.distance(mex) < RAID_RADIUS);
            if !threatened && !settled {
                continue;
            }
            let urgency = if threatened { 0 } else { 1 };
            if best
                .as_ref()
                .is_none_or(|(u, x, _)| (urgency, mex.x) < (*u, *x))
            {
                best = Some((urgency, mex.x, mex));
            }
        }
        best.map(|(_, _, p)| p)
    }

    pub(super) fn yard_anchor(&self, start: FxVec2, facing: Angle, index: usize) -> FxVec2 {
        let back = FxVec2::from_angle(facing + Angle::HALF_TURN);
        let side = FxVec2::from_angle(facing + Angle::QUARTER_TURN);
        let slot = index as i32;
        // 8-cell factory (96 m) plus a two-cell lane (see `lots`), staggered off the start.
        start + back * Fx::from_int(96) + side * Fx::from_int(slot * 120 - 36)
    }

    /// A site worth a builder's walk to help with: the commander stays home.
    pub(super) fn within_reach(&self, row: usize, site: FxVec2) -> bool {
        let owner = self.state.units.owner[row] as usize;
        if self.bp(row).has(cat::COMMANDER) {
            site.distance(self.state.players[owner].start) <= HOME_RADIUS
        } else {
            site.distance(self.state.units.pos[row]) <= FAR_FROM_HOME
        }
    }

    pub(super) fn mex_range(
        &self,
        is_commander: bool,
        census: &Census,
        persona: Personality,
        stance: Stance,
        skill: Skill,
    ) -> Fx {
        if is_commander {
            return Fx::from_int(520);
        }
        let mut reach = 1000 + census.extractors.len() as i32 * 280 + census.army as i32 * 16;
        if matches!(persona, Personality::Expander | Personality::Aggressive) {
            reach += 400;
        }
        if matches!(stance, Stance::Firebase | Stance::Push) {
            reach += 800;
        }
        // A builder sent across the map spends minutes walking and often dies on the way.
        Fx::from_int(reach.clamp(900, skill.mine_travel))
    }
}
