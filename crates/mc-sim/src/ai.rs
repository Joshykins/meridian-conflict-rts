//! The skirmish AI.
//!
//! It runs inside the simulation on every machine, reads only game state and
//! its own hashed `AiState`, and acts by queueing ordinary commands for the
//! next tick. It gets no information a player would not have except the enemy
//! start positions, and no resource bonus.
//!
//! Doctrine: spread the base into yards and farms instead of a blob, hold every
//! mass point with turrets, gather the army on a front, and push firebases
//! toward the enemy. Configurable doctrine and observed threats choose between expanding,
//! raiding extractors, sitting on a firebase, or committing to a wave.

mod adaptive;
mod danger;
mod groups;
mod layout;
mod lots;
mod sea;
mod staging;
use crate::command::{Command, PlayerCommand, MAX_COMMAND_UNITS};
use crate::spatial::kind;
use crate::tables::*;
use crate::world::snap_to_build_grid;
use crate::{AiConfig, Difficulty, Doctrine, Skill};
use crate::{SimError, World};
use adaptive::{Contact, Recovery};
use danger::{Danger, Loss};
use groups::clusters;
use layout::Place;
use mc_core::{Angle, Fx, FxVec2, StateHasher};
use mc_data::{cat, BlueprintId, UnitBlueprint};
use serde::{Deserialize, Serialize};

/// Build sites tried per placement search.
const MAX_SITE_PROBES: i32 = 360;
/// A builder this far from the start leaves the base's jobs, power too, to
/// those at home, and helps only with sites this close.
const FAR_FROM_HOME: Fx = Fx::from_int(1500);
/// Energy worth one mass when weighing an upgrade (a tech 1 mine's own ratio).
const ENERGY_PER_MASS: i32 = 6;
/// Mines a side puts down before anything but its first factory.
const FIRST_MINES: usize = 3;
/// Open-ground mine spots whose share is counted per search, nearest first.
const BARE_MINE_PROBES: usize = 8;
/// Enemies this close to a held point count as a raid.
const RAID_RADIUS: Fx = Fx::from_int(550);
/// Commander stays inside this radius of the start.
const HOME_RADIUS: Fx = Fx::from_int(640);
/// Point defense this close to a mass point is covering it.
const GUARD_COVER: Fx = Fx::from_int(210);
/// Army waiting this close to the staging point is gathered.
const STAGING_RADIUS: Fx = Fx::from_int(160);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
enum Stance {
    #[default]
    Expand = 0,
    Defend = 1,
    Raid = 2,
    Firebase = 3,
    Push = 4,
}

#[derive(Clone, Copy)]
enum Personality {
    Aggressive,
    Expander,
    Turtle,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AiState {
    pub config: AiConfig,
    contacts: Vec<Contact>,
    recovering: Vec<Recovery>,
    next_tactical_tick: u32,
    /// Attack waves sent so far; later waves wait for more units.
    pub waves: u32,
    /// Air/economic raids dispatched separately from the main ground wave.
    pub raids: u32,
    /// Rotates through the factory roster.
    pub production_counter: u32,
    stance: u8,
    /// Forward gun line, chosen once and held.
    firebase: Option<FxVec2>,
    scouts_sent: u32,
    /// Where its buildings were destroyed lately (`danger.rs`).
    #[serde(default)]
    losses: Vec<Loss>,
}

impl AiState {
    pub fn new(config: AiConfig) -> Self {
        Self {
            config: config.normalized(),
            ..Self::default()
        }
    }
    pub fn hash(&self, h: &mut StateHasher) {
        self.hash_adaptation(h);
        self.hash_losses(h);
        h.write_u64(self.raids as u64);
        h.write_u64(self.waves as u64 | (self.production_counter as u64) << 32);
        h.write_u64(self.stance as u64 | (self.scouts_sent as u64) << 32);
        match self.firebase {
            Some(p) => {
                h.write_u64(1);
                h.write_i64(p.x.0);
                h.write_i64(p.y.0);
            }
            None => h.write_u64(0),
        }
    }

    fn set_stance(&mut self, s: Stance) {
        self.stance = s as u8;
    }
}

#[derive(Default)]
struct Census {
    builders_idle: Vec<usize>,
    factories: Vec<usize>,
    factories_idle: Vec<usize>,
    extractors: Vec<usize>,
    extractor_pos: Vec<FxVec2>,
    power: Vec<FxVec2>,
    radar: Vec<FxVec2>,
    pd: Vec<FxVec2>,
    artillery: Vec<FxVec2>,
    shields: Vec<FxVec2>,
    anti_air: usize,
    engineer_factories: usize,
    air_factories: usize,
    reclaimers: usize,
    storage: usize,
    sites: Vec<usize>,
    damaged: Vec<usize>,
    engineers: usize,
    scouts_idle: Vec<usize>,
    army_idle: Vec<usize>,
    artillery_idle: Vec<usize>,
    bombers_idle: Vec<usize>,
    interceptors_idle: Vec<usize>,
    army: usize,
    scouts: usize,
    combat_rows: Vec<usize>,
    naval_idle: Vec<usize>,
    support_idle: Vec<usize>,
    army_fast: usize,
    max_tech: u8,
}

#[derive(Default)]
struct Intel {
    enemy_start: Option<FxVec2>,
    enemy_extractors: Vec<FxVec2>,
    enemy_factories: Vec<FxVec2>,
    enemy_army: usize,
    enemy_strength: i64,
    threats: Vec<(FxVec2, FxVec2)>,
    danger: Danger,
}

struct Planned {
    factories: usize,
    anti_air: usize,
    engineer_factories: usize,
    air_factories: usize,
    power: usize,
    radar: usize,
    pd: usize,
    artillery: usize,
    shields: usize,
    storage: usize,
    guards: Vec<FxVec2>,
}

struct Job {
    blueprint: BlueprintId,
    near: FxVec2,
    heading: Angle,
    min_r: Fx,
    keep_off_deposits: bool,
    place: Place,
}

/// A plot already spoken for this think, plus the structure's lot size so a
/// factory does not treat a mex as another factory-sized hole in the ground.
#[derive(Clone, Copy)]
struct Claim {
    pos: FxVec2,
    foot: i32,
    /// A core mine: it takes ground from any mine planned within its reach.
    mine: bool,
    /// A factory: the strip in front of its exit stays clear.
    factory: bool,
    /// A shield's radius, zero for anything else.
    cover: Fx,
}

/// The way every structure the AI orders faces.
const AI_BUILD_HEADING: Angle = Angle::from_degrees(270);

fn offset_toward(from: FxVec2, to: FxVec2, dist: Fx) -> FxVec2 {
    let d = to - from;
    if d.length() == Fx::ZERO {
        from
    } else {
        from + d.normalize() * dist
    }
}

impl World {
    pub(crate) fn run_ai(&mut self) -> Result<(), SimError> {
        for p in 0..self.state.players.len() {
            let player = &self.state.players[p];
            if player.controller != Controller::Ai || player.defeated {
                continue;
            }
            // Survival's engine side is run by `survival.rs`, not the skirmish AI.
            if self
                .state
                .survival
                .as_ref()
                .is_some_and(|s| s.config.engine_player as usize == p)
            {
                continue;
            }
            if (self.state.tick + p as u32 * 3) % self.state.ai[p].config.think_period() == 0 {
                self.think(p as u8);
            }
        }
        Ok(())
    }

    fn think(&mut self, player: u8) {
        self.remember_enemies(player);
        let mut census = self.survey_own(player);
        let intel = self.survey_intel(player, &census);
        let start = self.state.players[player as usize].start;
        let facing = intel
            .enemy_start
            .map(|e| (e - start).angle())
            .unwrap_or(Angle::ZERO);
        let persona = self.ai_personality(player, &census, &intel);

        if self.state.ai[player as usize].firebase.is_none()
            && census.factories.len() >= 1
            && census.extractors.len() >= 2
        {
            if let Some(spot) = self.pick_firebase(start, &intel, persona) {
                self.state.ai[player as usize].firebase = Some(spot);
            }
        }
        let firebase = self.state.ai[player as usize].firebase;
        let stance = self.decide_stance(&census, &intel, firebase, persona, player);
        self.state.ai[player as usize].set_stance(stance);

        let mut out: Vec<Command> = Vec::new();
        let mut claimed: Vec<Claim> = self
            .planned_sites(player)
            .map(|(_, o)| {
                let bp = self.blueprints.unit(o.blueprint);
                let fp = bp.footprint;
                Claim {
                    pos: o.pos,
                    foot: fp.0.max(fp.1) as i32,
                    mine: bp.mine.is_some(),
                    factory: bp.has(cat::FACTORY),
                    cover: bp.shield.as_ref().map_or(Fx::ZERO, |s| s.radius),
                }
            })
            .collect();
        let mut planned = Planned {
            factories: census.factories.len(),
            anti_air: census.anti_air,
            engineer_factories: census.engineer_factories,
            air_factories: census.air_factories,
            power: census.power.len(),
            radar: census.radar.len(),
            pd: census.pd.len(),
            artillery: census.artillery.len(),
            shields: census.shields.len(),
            storage: census.storage,
            guards: census
                .pd
                .iter()
                .chain(&census.artillery)
                .chain(&census.shields)
                .copied()
                .collect(),
        };

        for &row in &census.sites {
            let bp = self.bp(row);
            planned.factories += bp.has(cat::FACTORY) as usize;
            planned.engineer_factories +=
                (bp.has(cat::FACTORY) && self.blueprint_trains_engineers(bp)) as usize;
            planned.air_factories += bp.has(cat::FACTORY | cat::AIR) as usize;
            planned.power += bp.has(cat::POWER) as usize;
            planned.anti_air += bp.has(cat::DEFENSE | cat::ANTI_AIR) as usize;
            // A shield going up covers already: without this a second one was
            // ordered beside it while the first was still a frame.
            planned.shields += bp.has(cat::SHIELD) as usize;
        }
        self.direct_builders(
            player,
            &census,
            &intel,
            stance,
            persona,
            start,
            facing,
            firebase,
            &mut claimed,
            &mut planned,
            &mut out,
        );
        self.direct_factories(player, &census, stance, persona, &mut out);
        self.direct_upgrades(player, &census, &mut out);
        self.direct_scouts(player, &census, &intel, start, firebase, &mut out);
        self.react_tactically(player, &mut census, &intel, &mut out);
        self.direct_army(
            player, &census, &intel, stance, persona, start, facing, firebase, &mut out,
        );

        let out = self.route_ai_commands(out);
        self.state.ai_pending.extend(
            out.into_iter()
                .map(|command| PlayerCommand { player, command }),
        );
    }

    fn survey_own(&self, player: u8) -> Census {
        let units = &self.state.units;
        let mut c = Census::default();
        for row in units.slots.iter() {
            if units.owner[row] != player || units.has_flag(row, flag::IN_FACTORY) {
                continue;
            }
            let bp = self.bp(row);
            c.max_tech = c.max_tech.max(bp.tech);
            if !units.is_active(row) {
                if bp.is_structure() {
                    c.sites.push(row);
                }
                continue;
            }
            let recovering = self.state.ai[player as usize]
                .recovering
                .iter()
                .any(|r| r.id == units.id(row));
            let head = units.order_head[row];
            let finished_assist = bp.builder.is_some()
                && bp.is_mobile()
                && head != NO_ORDER
                && matches!(
                    self.state.orders.order[head as usize].kind,
                    OrderKind::Assist
                )
                && units
                    .row(self.state.orders.order[head as usize].target)
                    .is_some_and(|target| {
                        units.is_active(target)
                            && self.bp(target).is_structure()
                            && units.health[target] >= self.bp(target).health
                    });
            let idle = (head == NO_ORDER || finished_assist) && !recovering;
            if bp.is_mobile()
                && !bp.weapons.is_empty()
                && !bp.has(cat::ENGINEER)
                && !bp.has(cat::COMMANDER)
            {
                c.combat_rows.push(row);
                if !recovering {
                    c.army += 1;
                }
            }
            let pos = units.pos[row];
            if bp.is_structure() && units.health[row] < bp.health * Fx::ratio(7, 10) {
                c.damaged.push(row);
            }
            if bp.has(cat::DEFENSE | cat::ANTI_AIR) {
                c.anti_air += 1;
            }
            if bp.has(cat::FACTORY) {
                c.factories.push(row);
                if self.factory_trains_engineers(row) {
                    c.engineer_factories += 1;
                }
                if bp.has(cat::AIR) {
                    c.air_factories += 1;
                }
                if idle {
                    c.factories_idle.push(row);
                }
            } else if bp.has(cat::EXTRACTOR) {
                c.extractors.push(row);
                c.extractor_pos.push(pos);
            } else if bp.has(cat::POWER) {
                c.power.push(pos);
            } else if bp.has(cat::INTEL) && !bp.water_only() {
                // A sonar station out on the water is no radar cover for the base.
                c.radar.push(pos);
            } else if bp.has(cat::SHIELD) {
                c.shields.push(pos);
            } else if bp.has(cat::DEFENSE) && bp.has(cat::ARTILLERY) {
                c.artillery.push(pos);
            } else if bp.has(cat::DEFENSE) && bp.has(cat::DIRECT_FIRE) {
                c.pd.push(pos);
            } else if bp.has(cat::STORAGE) {
                c.storage += 1;
            } else if bp.reclaimer.is_some() {
                c.reclaimers += 1;
            } else if bp.is_mobile() && bp.builder.is_some() {
                if bp.has(cat::ENGINEER) && !bp.has(cat::COMMANDER) {
                    c.engineers += 1;
                }
                if idle {
                    c.builders_idle.push(row);
                }
            } else if bp.is_mobile() && bp.has(cat::SCOUT) {
                c.scouts += 1;
                if idle {
                    c.scouts_idle.push(row);
                }
            } else if bp.is_mobile() && bp.weapons.is_empty() {
                if idle
                    && (bp.shield.is_some()
                        || bp.radar > Fx::ZERO
                        || bp.anti_missile > Fx::ZERO
                        || bp.drone.is_some())
                {
                    c.support_idle.push(row);
                }
            } else if bp
                .motion
                .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval)
            {
                if idle && !bp.weapons.is_empty() {
                    c.naval_idle.push(row);
                }
            } else if bp.is_mobile() && bp.has(cat::AIR) {
                if idle {
                    if bp.has(cat::ANTI_AIR) {
                        c.interceptors_idle.push(row);
                    } else {
                        c.bombers_idle.push(row);
                    }
                }
            } else if bp.is_mobile() && !bp.weapons.is_empty() {
                if bp.has(cat::ARTILLERY) {
                    if idle {
                        c.artillery_idle.push(row);
                    }
                } else {
                    if bp.tech == 1 && !bp.has(cat::ARTILLERY) {
                        c.army_fast += 1;
                    }
                    if idle {
                        c.army_idle.push(row);
                    }
                }
            }
        }
        c
    }

    fn survey_intel(&self, player: u8, census: &Census) -> Intel {
        let start = self.state.players[player as usize].start;
        let mut intel = Intel {
            enemy_start: self
                .state
                .players
                .iter()
                .enumerate()
                .filter(|(i, p)| !p.defeated && self.are_enemies(player, *i as u8))
                .map(|(_, p)| p.start)
                .min_by_key(|s| (s.distance_sq(start), s.x, s.y)),
            ..Intel::default()
        };
        let units = &self.state.units;
        for row in units.slots.iter() {
            if units.owner[row] == player
                || units.has_flag(row, flag::IN_FACTORY)
                || !self.are_enemies(player, units.owner[row])
                || !self.detects(player, row)
                || (self.state.fog_enabled
                    && !self.fog.is_identified(
                        row,
                        units.id(row).generation(),
                        self.team_mask(player),
                    ))
            {
                continue;
            }
            let bp = self.bp(row);
            let pos = units.pos[row];
            if bp.has(cat::EXTRACTOR) && intel.enemy_extractors.len() < 24 {
                intel.enemy_extractors.push(pos);
            } else if bp.has(cat::FACTORY) && intel.enemy_factories.len() < 12 {
                intel.enemy_factories.push(pos);
            } else if bp.is_mobile() && !bp.weapons.is_empty() && !bp.has(cat::ENGINEER) {
                intel.enemy_army += 1;
                intel.enemy_strength += adaptive::strength(bp);
            }
        }
        for c in &self.state.ai[player as usize].contacts {
            let bp = self.blueprints.unit(c.blueprint);
            if bp.has(cat::EXTRACTOR) && !intel.enemy_extractors.contains(&c.pos) {
                intel.enemy_extractors.push(c.pos);
            } else if bp.has(cat::FACTORY) && !intel.enemy_factories.contains(&c.pos) {
                intel.enemy_factories.push(c.pos);
            }
        }
        let mut held = vec![start];
        held.extend(census.extractor_pos.iter().copied().take(16));
        if let Some(f) = self.state.ai[player as usize].firebase {
            held.push(f);
        }
        // Flooded only once something turns up near a held point.
        let mut home: Option<Option<staging::HomeGround>> = None;
        for at in held {
            if let Some(e) = self.index.nearest(at, RAID_RADIUS, kind::UNIT, |e| {
                self.unit_entry_is_current(e)
                    && self.are_enemies(player, self.state.units.owner[e.row as usize])
                    && self.detects(player, e.row as usize)
                    && (!self.state.fog_enabled
                        || self.fog.is_identified(
                            e.row as usize,
                            self.state.units.id(e.row as usize).generation(),
                            self.team_mask(player),
                        ))
                    && self.bp(e.row as usize).is_mobile()
                    && !self.bp(e.row as usize).weapons.is_empty()
                    // The land army answers threats it can reach. A gunship over a
                    // mine or a boat off an offshore one used to hold the whole army
                    // at home, sending a few units at it every think, for as long
                    // as it stayed; anti-air and the navy deal with those.
                    && self.bp(e.row as usize).motion.is_some_and(|m| {
                        !matches!(m.layer, mc_data::MoveLayer::Air | mc_data::MoveLayer::Naval)
                    })
            }) {
                // A hover tank out on a lake or a unit up on a shelf is out of
                // the army's reach: sent at it every think, the whole army
                // piled up on the shore, never went idle and never left.
                let home = home.get_or_insert_with(|| self.home_ground(start, 0));
                if self.land_can_answer(e.pos, home.as_ref()) {
                    intel.threats.push((at, e.pos));
                }
            }
        }
        intel.danger = self.ai_danger(player);
        intel
    }

    fn decide_stance(
        &self,
        census: &Census,
        intel: &Intel,
        firebase: Option<FxVec2>,
        persona: Personality,
        player: u8,
    ) -> Stance {
        let start = self.state.players[player as usize].start;
        let base_raid = intel
            .threats
            .iter()
            .any(|(_, enemy)| enemy.distance(start) < Fx::from_int(420));
        let mex_raid = intel
            .threats
            .iter()
            .any(|(_, enemy)| enemy.distance(start) >= Fx::from_int(420));
        let firebase_ready = firebase.is_some_and(|f| {
            census
                .pd
                .iter()
                .filter(|p| p.distance(f) < Fx::from_int(260))
                .count()
                >= 2
        });
        let army = census.army;
        if base_raid || (mex_raid && army < intel.enemy_army.saturating_add(4)) {
            return Stance::Defend;
        }
        let wave = self.wave_size(player, persona);
        let own_strength: i64 = census
            .combat_rows
            .iter()
            .map(|&r| adaptive::strength(self.bp(r)))
            .sum();
        let favorable = own_strength * 10 >= intel.enemy_strength * 12;
        match persona {
            Personality::Aggressive
                if army >= wave.min(8) && census.extractors.len() >= 2 && favorable =>
            {
                Stance::Push
            }
            Personality::Turtle if base_raid || mex_raid => Stance::Defend,
            _ if firebase.is_some() && !firebase_ready && census.engineers >= 1 => Stance::Firebase,
            _ if army >= wave && census.extractors.len() >= 3 && favorable => Stance::Push,
            _ if !intel.enemy_extractors.is_empty()
                && census.army_idle.len() + census.army_fast >= 5
                && !matches!(persona, Personality::Turtle) =>
            {
                Stance::Raid
            }
            _ if census.factories.len() >= 2
                && census.engineers >= 2
                && firebase.is_some()
                && !firebase_ready =>
            {
                Stance::Firebase
            }
            _ => Stance::Expand,
        }
    }

    fn wave_size(&self, player: u8, persona: Personality) -> usize {
        let waves = self.state.ai[player as usize].waves;
        let size = match persona {
            Personality::Aggressive => 6 + waves * 2,
            Personality::Expander => 8 + waves * 3,
            Personality::Turtle => 10 + waves * 4,
        };
        let delta = self.state.ai[player as usize].config.skill().wave_delta;
        (size as i32 + delta).clamp(4, 40) as usize
    }

    fn direct_builders(
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
        let energy_short =
            pl.energy_demand > pl.energy_income || pl.energy < pl.energy_capacity / 5;
        let mass_rich = pl.mass > pl.mass_capacity * Fx::ratio(7, 10);
        let mass_income = pl.mass_income;
        let energy_income = pl.energy_income;

        let skill = self.state.ai[player as usize].config.skill();
        let home = (!census.builders_idle.is_empty())
            .then(|| self.home_ground(start, 1))
            .flatten();
        for &row in census.builders_idle.iter().take(skill.builders_per_think) {
            if let Some(site) = census
                .sites
                .iter()
                .copied()
                .filter(|&s| {
                    ((energy_short && self.bp(s).has(cat::POWER)) || census.sites.len() >= 3)
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
                        planned.radar += bp.has(cat::INTEL) as usize;
                        planned.artillery +=
                            (bp.has(cat::DEFENSE) && bp.has(cat::ARTILLERY)) as usize;
                        planned.shields += bp.has(cat::SHIELD) as usize;
                        planned.storage += bp.has(cat::STORAGE) as usize;
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
                        .min_by_key(|s| (self.state.units.pos[*s].distance_sq(pos), *s as u32))
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

    fn choose_job(
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
        let want_factories = if piling {
            want_factories.max(planned.factories + 1)
        } else {
            want_factories
        }
        .min(skill.factory_cap as usize);
        let leave_power = tech < 2
            && energy_income >= Fx::from_int(150)
            && self.state.units.slots.iter().any(|r| {
                self.state.units.owner[r] == owner
                    && self.state.units.is_active(r)
                    && self.bp(r).is_mobile()
                    && self.bp(r).builder.is_some()
                    && self.builder_tech(r) >= 2
            });
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
            if planned.factories >= want_factories
                || (planned.factories >= 2 && leave_power)
                || (far && planned.factories >= 1)
            {
                return None;
            }
            let idx = planned.factories;
            let want_air = planned.air_factories == 0
                && planned.engineer_factories >= 1
                && (mass_income >= Fx::from_int(6)
                    || matches!(stance, Stance::Push | Stance::Raid));
            let near = if idx >= 2
                && firebase.is_some()
                && matches!(stance, Stance::Firebase | Stance::Push)
            {
                firebase.unwrap()
            } else {
                self.yard_anchor(start, facing, idx)
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
        if energy_short && planned.power >= 2 && !leave_power {
            let ptech = if tech >= 3 && energy_income >= Fx::from_int(400) {
                3
            } else if tech >= 2 && energy_income >= Fx::from_int(100) {
                2
            } else {
                1
            };
            if let Some(job) = power_job(ptech) {
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
        if energy_short && planned.power < want_power.min(6) {
            if let Some(job) = power_job(1) {
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
        if self.state.ai[owner as usize].config.adaptation > 0
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
        if let Some(mex) = self.unguarded_mex(census, planned, intel, stance) {
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
        if planned.radar == 0 && planned.pd >= 1 && planned.power >= 2 {
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
        if !leave_power
            && (planned.power < want_power || energy_income < mass_income * skill.power_ratio)
        {
            // Power is paid first in a stall, so a short side still builds the biggest plant it can.
            let ptech = if mass_income >= Fx::from_int(22) && tech >= 3 {
                3
            } else if mass_income >= Fx::from_int(10) && tech >= 2 {
                2
            } else {
                1
            };
            if let Some(job) = power_job(ptech) {
                return Some(job);
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
                        tech.min(2).max(1),
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
        if planned.radar < 1 + census.extractor_pos.len() / 4 {
            if let Some(&mex) = census.extractor_pos.iter().find(|m| {
                m.distance(start) > Fx::from_int(400)
                    && !census
                        .radar
                        .iter()
                        .any(|r| r.distance(**m) < Fx::from_int(500))
            }) {
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

    fn firebase_job(
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
        let radar = census.radar.iter().filter(|p| around(p)).count();
        let arty = census.artillery.iter().filter(|p| around(p)).count();
        if pd < 2 {
            let near = offset_toward(spot, front, Fx::from_int(70));
            return self.job_structure(
                row,
                cat::DEFENSE | cat::DIRECT_FIRE,
                tech.min(2).max(1),
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
        if radar < 1 {
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
        if tech >= 2 && census.reclaimers < 1 {
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

    fn forward_gun(
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
                tech.min(2).max(1),
                near,
                (mex - near).angle(),
                Fx::from_int(16),
                false,
            );
        }
        None
    }

    fn job_structure(
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

    fn unguarded_mex(
        &self,
        census: &Census,
        planned: &Planned,
        intel: &Intel,
        stance: Stance,
    ) -> Option<FxVec2> {
        let want = if stance == Stance::Defend { 2 } else { 1 };
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

    fn yard_anchor(&self, start: FxVec2, facing: Angle, index: usize) -> FxVec2 {
        let back = FxVec2::from_angle(facing + Angle::HALF_TURN);
        let side = FxVec2::from_angle(facing + Angle::QUARTER_TURN);
        let slot = index as i32;
        // 8-cell factory (96 m) plus a two-cell lane (see `lots`), staggered off the start.
        start + back * Fx::from_int(96) + side * Fx::from_int(slot * 120 - 36)
    }

    /// A site worth a builder's walk to help with: the commander stays home.
    fn within_reach(&self, row: usize, site: FxVec2) -> bool {
        let owner = self.state.units.owner[row] as usize;
        if self.bp(row).has(cat::COMMANDER) {
            site.distance(self.state.players[owner].start) <= HOME_RADIUS
        } else {
            site.distance(self.state.units.pos[row]) <= FAR_FROM_HOME
        }
    }

    fn mex_range(
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

    fn direct_factories(
        &mut self,
        player: u8,
        census: &Census,
        stance: Stance,
        persona: Personality,
        out: &mut Vec<Command>,
    ) {
        let mut counter = self.state.ai[player as usize].production_counter;
        let mut composition = self.ai_composition(player);
        let mut planned_engineers = 0;
        let mut planned_scouts = 0;
        // No rally point: a finished unit rolls out idle and the army sends it
        // to the staging point with the rest. A rally among the base's buildings
        // jammed: units stuck a few metres short of it in the crowd never
        // finished the move, never counted as idle and never joined a wave.
        for &row in &census.factories_idle {
            let Some(builder) = &self.bp(row).builder else {
                continue;
            };
            let want_engineers = {
                let n = 2 + census.factories.len() * 2;
                match persona {
                    Personality::Expander => n + 2,
                    Personality::Turtle => n + 1,
                    Personality::Aggressive => n,
                }
            };
            let engineer = builder
                .builds
                .iter()
                .copied()
                .filter(|b| {
                    let u = self.blueprints.unit(*b);
                    u.has(cat::ENGINEER) && !u.has(cat::COMMANDER)
                })
                .max_by_key(|b| (self.blueprints.unit(*b).tech, std::cmp::Reverse(b.0)));
            let missing_tech_builder = engineer.is_some_and(|id| {
                self.blueprints.unit(id).tech > 1 && composition.get(&id).copied().unwrap_or(0) == 0
            });
            let scout = builder
                .builds
                .iter()
                .copied()
                .find(|b| self.blueprints.unit(*b).has(cat::SCOUT));
            let fighters: Vec<BlueprintId> = builder
                .builds
                .iter()
                .copied()
                .filter(|b| {
                    let u = self.blueprints.unit(*b);
                    u.is_mobile()
                        && (!u.weapons.is_empty()
                            || u.shield.is_some()
                            || u.radar > Fx::ZERO
                            || u.anti_missile > Fx::ZERO
                            || u.drone.is_some())
                        && !u.has(cat::COMMANDER)
                        && !u.has(cat::ENGINEER)
                })
                .collect();
            let blueprint = if (census.engineers + planned_engineers < want_engineers
                || missing_tech_builder)
                && engineer.is_some()
                && (counter % 4 == 0 || stance == Stance::Firebase)
            {
                planned_engineers += 1;
                engineer
            } else if census.scouts + planned_scouts < 2 && scout.is_some() && counter % 5 == 1 {
                planned_scouts += 1;
                scout
            } else {
                self.choose_combat_unit(player, &fighters, &composition, stance, counter)
            };
            if let Some(id) = blueprint {
                *composition.entry(id).or_insert(0) += 1;
            }
            counter = counter.wrapping_add(1);
            if let Some(blueprint) = blueprint {
                out.push(Command::Produce {
                    factories: vec![self.state.units.id(row)],
                    blueprint,
                    count: 1,
                });
            }
        }
        self.state.ai[player as usize].production_counter = counter;
    }

    fn direct_upgrades(&self, player: u8, census: &Census, out: &mut Vec<Command>) {
        let pl = &self.state.players[player as usize];
        let income = pl.mass_income;
        let mass_rich = pl.mass > pl.mass_capacity * Fx::ratio(6, 10);
        let base_ready =
            !census.pd.is_empty() && census.power.len() >= 2 && !census.radar.is_empty();
        if !base_ready {
            return;
        }
        // Not held back by a brief dip, but not started in a stall either: an
        // upgrade is paid first, energy too, and would stop every factory.
        if pl.efficiency >= Fx::ratio(9, 10) && pl.energy > pl.energy_capacity / 10 {
            if let Some(row) = self.mine_to_upgrade(player, census, mass_rich) {
                out.push(Command::Upgrade {
                    units: vec![self.state.units.id(row)],
                });
                return;
            }
        }
        if pl.energy_income < pl.energy_demand
            || pl.energy < pl.energy_capacity / 4
            || pl.efficiency < Fx::ratio(8, 10)
        {
            return;
        }
        let skill = self.state.ai[player as usize].config.skill();
        let mut candidates: Vec<(u8, usize)> = Vec::new();
        if income >= Fx::from_int(10) && pl.mass > Fx::from_int(400) {
            if let Some(cmd) = self.state.units.row(pl.commander) {
                if (self.bp(cmd).upgrades_to.is_some() || self.ai_next_refit(cmd).is_some())
                    && self.state.units.order_head[cmd] == NO_ORDER
                {
                    candidates.push((1, cmd));
                }
            }
        }
        if income >= Fx::from_int(8) && !census.radar.is_empty() {
            for row in self.state.units.slots.iter() {
                if self.state.units.owner[row] != player {
                    continue;
                }
                let bp = self.bp(row);
                if bp.has(cat::INTEL)
                    && bp.upgrades_to.is_some()
                    && self.state.units.order_head[row] == NO_ORDER
                    && self.state.units.is_active(row)
                {
                    candidates.push((2, row));
                }
            }
        }
        let surplus = mass_rich || (skill.eager_tech && pl.mass > Fx::from_int(400));
        // A factory being upgraded builds nothing: a few at a time, however
        // often this AI thinks, so the army keeps coming.
        let upgrading = census
            .factories
            .iter()
            .filter(|&&row| self.upgrading(row))
            .count();
        if income >= Fx::from_int(skill.tech_income)
            && surplus
            && upgrading < 1 + census.factories.len() / 4
        {
            for &row in &census.factories_idle {
                if self.bp(row).upgrades_to.is_some() {
                    candidates.push((3, row));
                }
            }
        }
        candidates.sort_by_key(|(p, r)| (*p, *r));
        if let Some((_, row)) = candidates.first() {
            let units = vec![self.state.units.id(*row)];
            out.push(match self.ai_next_refit(*row) {
                Some(kit) => Command::Refit { units, kit },
                None => Command::Upgrade { units },
            });
        }
    }

    fn upgrading(&self, row: usize) -> bool {
        let head = self.state.units.order_head[row];
        head != NO_ORDER
            && matches!(
                self.state.orders.order[head as usize].kind,
                OrderKind::Upgrade
            )
    }

    /// The mine whose next tier pays back its cost soonest, if soon enough for this AI.
    fn mine_to_upgrade(&self, player: u8, census: &Census, mass_rich: bool) -> Option<usize> {
        let pl = &self.state.players[player as usize];
        let skill = self.state.ai[player as usize].config.skill();
        let units = &self.state.units;
        if pl.mass_income < Fx::from_int(8) {
            return None;
        }
        let upgrading = census
            .extractors
            .iter()
            .filter(|&&row| self.upgrading(row))
            .count() as i32;
        // One at a time while the side is small, more as it grows.
        if upgrading >= 1 + (pl.mass_income / Fx::from_int(60)).floor_int() {
            return None;
        }
        let side_tech = self.side_tech(player);
        let open = census.extractors.iter().copied().filter_map(|row| {
            let next = self.blueprints.unit(self.bp(row).upgrades_to?);
            (self.blueprints.upgrade_needs(next) <= side_tech && units.order_head[row] == NO_ORDER)
                .then_some((row, next))
        });
        let horizon = Fx::from_int(skill.upgrade_payback * if mass_rich { 2 } else { 1 });
        // No saving up first: a mine upgrade is paid before the factories in a
        // stall, and a good one is the best thing the side can spend on.
        // Energy counts at what a tech 1 mine costs in it for each unit of mass.
        open.filter_map(|(row, next)| {
            let state = self.state.mines.by_unit.get(&units.id(row))?;
            let gain = state.full_rate(&next.mine?) - state.full_rate(&self.bp(row).mine?);
            let cost = next.cost_mass + next.cost_energy / ENERGY_PER_MASS;
            (gain > Fx::ZERO).then(|| (row, cost / gain))
        })
        .filter(|&(_, payback)| payback <= horizon)
        .min_by_key(|&(row, payback)| (payback, row))
        .map(|(row, _)| row)
    }

    /// The cheapest module that goes on this unit without taking another off.
    fn ai_next_refit(&self, row: usize) -> Option<mc_data::BlueprintId> {
        let (set, loadout) = self.blueprints.loadout(self.bp(row).id)?;
        set.slots
            .iter()
            .enumerate()
            .flat_map(|(s, slot)| (0..slot.modules.len() as u8).map(move |m| (s, m)))
            .filter(|&(s, m)| {
                set.fit(&loadout.fitted, s, m).is_ok()
                    && set.replaces(&loadout.fitted, s, m).is_none()
            })
            .min_by_key(|&(s, m)| (set.module(s, m).cost_mass, s, m))
            .map(|(s, m)| set.module(s, m).kit)
    }

    fn direct_scouts(
        &mut self,
        player: u8,
        census: &Census,
        intel: &Intel,
        start: FxVec2,
        firebase: Option<FxVec2>,
        out: &mut Vec<Command>,
    ) {
        if census.scouts_idle.is_empty() {
            return;
        }
        let mut sent = self.state.ai[player as usize].scouts_sent;
        let enemy = intel.enemy_start.unwrap_or(start);
        for &row in &census.scouts_idle {
            let dest = match sent % 4 {
                0 => enemy,
                1 => intel
                    .enemy_extractors
                    .first()
                    .copied()
                    .or_else(|| {
                        self.ore_centres()
                            .into_iter()
                            .max_by_key(|d| (d.distance_sq(start), d.x, d.y))
                    })
                    .unwrap_or(enemy),
                2 => firebase.unwrap_or_else(|| start.lerp(enemy, Fx::ratio(1, 2))),
                _ => {
                    let ore = self.ore_centres();
                    ore.get((sent as usize / 4) % ore.len().max(1))
                        .copied()
                        .unwrap_or(enemy)
                }
            };
            sent = sent.wrapping_add(1);
            out.push(Command::AttackMove {
                units: vec![self.state.units.id(row)],
                target: dest,
                queue: false,
            });
        }
        self.state.ai[player as usize].scouts_sent = sent;
    }

    fn direct_army(
        &mut self,
        player: u8,
        census: &Census,
        intel: &Intel,
        stance: Stance,
        persona: Personality,
        start: FxVec2,
        facing: Angle,
        firebase: Option<FxVec2>,
        out: &mut Vec<Command>,
    ) {
        self.direct_fleet(player, census, out);
        for &row in &census.support_idle {
            let escort = census
                .combat_rows
                .iter()
                .copied()
                .filter(|&r| r != row)
                .min_by_key(|&r| self.state.units.pos[r].distance_sq(self.state.units.pos[row]));
            if let Some(escort) = escort {
                out.push(Command::Move {
                    units: vec![self.state.units.id(row)],
                    target: offset_toward(self.state.units.pos[escort], start, Fx::from_int(100)),
                    queue: false,
                });
            }
        }
        if census.army_idle.is_empty()
            && census.artillery_idle.is_empty()
            && census.bombers_idle.is_empty()
            && census.interceptors_idle.is_empty()
        {
            return;
        }
        let staging = firebase
            .filter(|_| matches!(stance, Stance::Firebase | Stance::Push | Stance::Raid))
            .unwrap_or_else(|| {
                offset_toward(
                    start,
                    intel
                        .enemy_start
                        .unwrap_or(start + FxVec2::from_angle(facing) * Fx::from_int(400)),
                    Fx::from_int(240),
                )
            });

        let mut army_idle = census.army_idle.clone();
        if let Some(&(at, enemy)) = intel.threats.first().filter(|(_, enemy)| {
            stance == Stance::Defend || enemy.distance(start) < Fx::from_int(420)
        }) {
            let target = if at.distance(start) < HOME_RADIUS {
                enemy
            } else {
                at
            };
            if enemy.distance(start) < Fx::from_int(420) {
                // The base itself: everything goes.
                let ids: Vec<UnitId> = census
                    .army_idle
                    .iter()
                    .chain(&census.artillery_idle)
                    .chain(&census.bombers_idle)
                    .take(MAX_COMMAND_UNITS)
                    .map(|&r| self.state.units.id(r))
                    .collect();
                if !ids.is_empty() {
                    out.push(Command::AttackMove {
                        units: ids,
                        target,
                        queue: false,
                    });
                    return;
                }
            } else if !army_idle.is_empty() {
                // A raid on an outlying mine: the closest few answer it and the
                // rest carry on. Returning here held the whole army at home, a
                // squad at a time, for as long as any raider stayed near a mine.
                let units = &self.state.units;
                army_idle.sort_by_key(|&r| (units.pos[r].distance_sq(target), r));
                let squad: Vec<usize> = army_idle.drain(..army_idle.len().min(6)).collect();
                out.push(Command::AttackMove {
                    units: squad.iter().map(|&r| units.id(r)).collect(),
                    target,
                    queue: false,
                });
                army_idle.sort_unstable();
            }
        }

        self.direct_air(player, census, intel, start, staging, out);

        let wave = self.wave_size(player, persona);
        let ids = |rows: &[usize]| -> Vec<UnitId> {
            rows.iter()
                .take(MAX_COMMAND_UNITS)
                .map(|&r| self.state.units.id(r))
                .collect()
        };
        // Units only ever leave in one group: from the staging point, or, out
        // in the field after a wave, together with the rest of it.
        // A staging point across a cliff or a river from home is never reached.
        let size = army_idle
            .iter()
            .filter_map(|&r| self.bp(r).motion)
            .map(|m| m.size_class)
            .max()
            .unwrap_or(0);
        let ground = self.home_ground(start, size);
        let staging = ground
            .as_ref()
            .map_or(staging, |g| self.reachable_staging(start, staging, g));
        let mut at_stage = Vec::new();
        let mut gathering = Vec::new();
        let mut forward = Vec::new();
        let stage_reach = staging.distance(start) + Fx::from_int(400);
        for &row in &army_idle {
            let pos = self.state.units.pos[row];
            if pos.distance(staging) <= STAGING_RADIUS {
                at_stage.push(row);
            } else if pos.distance(start) > stage_reach {
                forward.push(row);
            } else if ground.as_ref().is_some_and(|g| !g.reaches(pos)) {
                // Made by a factory on another shelf: it cannot walk to the
                // staging point, so it goes with the wave from where it is.
                at_stage.push(row);
            } else {
                gathering.push(row);
            }
        }
        if !forward.is_empty() {
            // A wave that took its target moves on only as a wave: enough of it
            // left presses on to the next, a remnant falls back to join the
            // next wave. Each group of the army out there (600 m clusters,
            // busy units counted) waits until most of it is done fighting.
            // Sending each unit on as it went idle strung the wave out into a
            // stream that arrived, and was beaten, a unit at a time.
            let out_there: Vec<usize> = census
                .combat_rows
                .iter()
                .copied()
                .filter(|&r| {
                    self.state.units.pos[r].distance(start) > stage_reach
                        && adaptive::domain(self.bp(r)) == 0
                })
                .collect();
            for (seed, members) in clusters(&self.state.units.pos, &out_there, Fx::from_int(600)) {
                let idle: Vec<usize> = members
                    .iter()
                    .copied()
                    .filter(|r| forward.contains(r))
                    .collect();
                if idle.is_empty() || idle.len() * 3 < members.len() * 2 {
                    continue;
                }
                let press = members.len() >= (wave / 2).max(4);
                let target = press
                    .then(|| self.attack_target(player, seed, intel, stance))
                    .flatten()
                    .unwrap_or(staging);
                out.push(Command::AttackMove {
                    units: ids(&idle),
                    target,
                    queue: false,
                });
            }
        }
        if stance == Stance::Raid && !intel.enemy_extractors.is_empty() {
            let raiders: Vec<UnitId> = at_stage
                .iter()
                .copied()
                .take(12)
                .map(|r| self.state.units.id(r))
                .collect();
            if raiders.len() >= 6 {
                if let Some(mex) = intel.enemy_extractors.get(
                    self.state.ai[player as usize].waves as usize % intel.enemy_extractors.len(),
                ) {
                    self.state.ai[player as usize].raids += 1;
                    out.push(Command::AttackMove {
                        units: raiders,
                        target: *mex,
                        queue: false,
                    });
                    return;
                }
            }
        }
        if !gathering.is_empty() {
            out.push(Command::AttackMove {
                units: ids(&gathering),
                target: staging,
                queue: false,
            });
        }
        // Only what has gathered goes: a wave that leaves while half of it is
        // still on the road arrives strung out and is beaten a few at a time.
        let ready = at_stage.len() + census.artillery_idle.len() >= wave
            || (stance == Stance::Push && at_stage.len() * 3 >= wave * 2);
        if ready {
            if let Some(target) = self.attack_target(player, staging, intel, stance) {
                self.state.ai[player as usize].waves += 1;
                if !at_stage.is_empty() {
                    out.push(Command::AttackMove {
                        units: ids(&at_stage),
                        target,
                        queue: false,
                    });
                }
                if !census.artillery_idle.is_empty() {
                    let range = census
                        .artillery_idle
                        .iter()
                        .flat_map(|&r| self.bp(r).weapons.iter().map(|w| w.range_max))
                        .min()
                        .unwrap_or(Fx::from_int(350));
                    let siege = offset_toward(target, staging, range * Fx::ratio(9, 10));
                    let ids: Vec<UnitId> = census
                        .artillery_idle
                        .iter()
                        .take(MAX_COMMAND_UNITS)
                        .map(|&r| self.state.units.id(r))
                        .collect();
                    out.push(Command::AttackMove {
                        units: ids,
                        target: siege,
                        queue: false,
                    });
                }
            }
        } else if !census.artillery_idle.is_empty() {
            let hold = offset_toward(staging, start, Fx::from_int(90));
            let ids: Vec<UnitId> = census
                .artillery_idle
                .iter()
                .take(MAX_COMMAND_UNITS)
                .map(|&r| self.state.units.id(r))
                .collect();
            out.push(Command::AttackMove {
                units: ids,
                target: hold,
                queue: false,
            });
        }
    }

    /// The most advanced structure with all of `categories` this builder can make,
    /// not exceeding `max_tech`.
    fn pick_structure(
        &self,
        builder_row: usize,
        categories: u32,
        max_tech: u8,
    ) -> Option<BlueprintId> {
        let builder = self.bp(builder_row).builder.as_ref()?;
        builder
            .builds
            .iter()
            .copied()
            .filter(|b| {
                let bp = self.blueprints.unit(*b);
                // Land jobs only: a structure that stands on open water (a sonar) is never one.
                bp.has(categories)
                    && bp.is_structure()
                    && !bp.has(cat::WALL)
                    && !bp.water_only()
                    && bp.tech <= max_tech
            })
            .max_by_key(|b| (self.blueprints.unit(*b).tech, std::cmp::Reverse(b.0)))
    }

    fn factory_trains_engineers(&self, row: usize) -> bool {
        self.blueprint_trains_engineers(self.bp(row))
    }

    fn blueprint_trains_engineers(&self, bp: &UnitBlueprint) -> bool {
        bp.builder.as_ref().is_some_and(|b| {
            b.builds.iter().any(|id| {
                let u = self.blueprints.unit(*id);
                u.has(cat::ENGINEER) && !u.has(cat::COMMANDER)
            })
        })
    }

    fn pick_factory(
        &self,
        builder_row: usize,
        max_tech: u8,
        want_air: bool,
    ) -> Option<BlueprintId> {
        let builder = self.bp(builder_row).builder.as_ref()?;
        let mut cands: Vec<BlueprintId> = builder
            .builds
            .iter()
            .copied()
            .filter(|b| {
                let bp = self.blueprints.unit(*b);
                let player = self.state.units.owner[builder_row] as usize;
                let domain = adaptive::domain(bp);
                // A factory with nothing to train (a yard before its hulls exist) is no use.
                bp.has(cat::FACTORY)
                    && bp.is_structure()
                    && bp.builder.as_ref().is_some_and(|f| !f.builds.is_empty())
                    && bp.tech <= max_tech
                    && self.state.ai[player].config.domain_weights[domain] > 0
                    && self
                        .find_site(
                            bp,
                            self.state.units.pos[builder_row],
                            &[],
                            Angle::ZERO,
                            Fx::ZERO,
                            true,
                            None,
                        )
                        .is_some()
            })
            .collect();
        // Air factories also train engineers, and their keys sort before land,
        // so "any engineer factory" always resolved to air. Pick by domain.
        let typed: Vec<BlueprintId> = cands
            .iter()
            .copied()
            .filter(|b| self.blueprints.unit(*b).has(cat::AIR) == want_air)
            .collect();
        let owner = self.state.units.owner[builder_row];
        let any_factory = self
            .state
            .units
            .slots
            .iter()
            .any(|r| self.state.units.owner[r] == owner && self.bp(r).has(cat::FACTORY));
        if !any_factory && !typed.is_empty() {
            cands = typed;
        }
        cands.into_iter().max_by_key(|b| {
            let bp = self.blueprints.unit(*b);
            let owner = self.state.units.owner[builder_row];
            (
                self.factory_domain_score(owner, bp),
                bp.tech,
                std::cmp::Reverse(b.0),
            )
        })
    }

    fn pick_storage(&self, builder_row: usize) -> Option<BlueprintId> {
        let builder = self.bp(builder_row).builder.as_ref()?;
        builder.builds.iter().copied().find(|b| {
            let bp = self.blueprints.unit(*b);
            bp.has(cat::STORAGE) && bp.economy.mass_storage > Fx::ZERO
        })
    }

    fn pick_reclaimer(&self, builder_row: usize) -> Option<BlueprintId> {
        let builder = self.bp(builder_row).builder.as_ref()?;
        builder
            .builds
            .iter()
            .copied()
            .find(|b| self.blueprints.unit(*b).reclaimer.is_some())
    }

    fn builder_tech(&self, builder_row: usize) -> u8 {
        self.bp(builder_row)
            .builder
            .as_ref()
            .map(|b| {
                b.builds
                    .iter()
                    .map(|id| self.blueprints.unit(*id).tech)
                    .max()
                    .unwrap_or(1)
            })
            .unwrap_or(1)
    }

    /// Nearest ore field with room for another mine, within `range` of `from`,
    /// staying off the enemy's doorstep until the army can contest it; with
    /// `bare`, else open ground where a mine gets at least that share of what a
    /// whole circle of land would give it. Its centre; the builder's site search finds the lot.
    fn free_deposit(
        &self,
        from: FxVec2,
        claimed: &[Claim],
        range: Fx,
        intel: &Intel,
        bare: Option<Fx>,
    ) -> Option<FxVec2> {
        let mine = self
            .blueprints
            .units
            .iter()
            .find(|b| b.mine.is_some() && b.tech == 1)?;
        // Mines may stand close, but split the ground between them: keep
        // them a reach apart, where each still has about 80% of its circle.
        let spacing = mine.mine?.reach;
        let open = |d: &FxVec2| {
            d.distance(from) <= range
                && !self.mine_within(*d, spacing)
                // A planned mine counts like a built one. Its site can stand well off
                // the deposit (the middle may be steep), so a check near the site
                // alone sent every idle builder back to the same deposit, one
                // think after another, and piled mines up around it.
                && !claimed.iter().any(|c| {
                    c.pos.distance(*d) < if c.mine { spacing } else { Fx::from_int(32) }
                })
                && intel.enemy_start.is_none_or(|e| {
                    d.distance(e) > Fx::from_int(480) || d.distance(from) < d.distance(e)
                })
                && !intel.danger.hot(*d)
        };
        let ore = self
            .ore_centres()
            .into_iter()
            .filter(|d| open(d))
            .min_by_key(|d| (d.distance_sq(from), d.x, d.y));
        // No ore left in range: a bare mine still pays, but only where it keeps
        // enough ground. Filling the gaps between mines only takes ground from
        // them: the side gains little more than the new shaft's base.
        ore.or_else(|| {
            let least = bare?;
            let mut spots: Vec<FxVec2> = (1..=(range * 2 / spacing).floor_int().max(1))
                .flat_map(|ring| {
                    let r = spacing * ring / 2;
                    let n = 6 * ring;
                    (0..n)
                        .map(move |k| from + FxVec2::from_angle(Angle((k * 65536 / n) as u16)) * r)
                })
                .filter(|d| self.terrain.in_bounds(*d) && open(d))
                .collect();
            spots.sort_by_key(|d| (d.distance_sq(from), d.x, d.y));
            let m = mine.mine?;
            // Against a whole circle of land, so the sea and the map's edge count as lost ground.
            let hectares = m.reach * m.reach * Fx::ratio(355, 113) / 10000;
            let whole = crate::mines::land_rate(&m, hectares, Fx::ZERO);
            spots.into_iter().take(BARE_MINE_PROBES).find(|d| {
                let share = self.mine_share_at(mine, *d);
                crate::mines::land_rate(&m, share.ground, share.ore) >= whole * least
            })
        })
    }

    fn pick_firebase(&self, start: FxVec2, intel: &Intel, persona: Personality) -> Option<FxVec2> {
        let enemy = intel.enemy_start?;
        let frac = match persona {
            Personality::Aggressive => Fx::ratio(11, 20),
            Personality::Expander => Fx::ratio(2, 5),
            Personality::Turtle => Fx::ratio(3, 10),
        };
        let aim = start.lerp(enemy, frac);
        let along = enemy - start;
        let span = along.length().max(Fx::ONE);
        let deposit = self.ore_centres().into_iter().filter(|d| {
            let t = (*d - start).dot(along) / span;
            t > Fx::from_int(350)
                && t < span - Fx::from_int(600)
                && d.distance(start) > Fx::from_int(380)
                && d.distance(enemy) > Fx::from_int(520)
                && self.structure_at(*d, 0).is_none()
        });
        deposit
            .min_by_key(|d| (d.distance_sq(aim), d.x, d.y))
            .or_else(|| {
                let mut p = aim;
                if p.distance(enemy) < Fx::from_int(700) {
                    p = offset_toward(enemy, start, Fx::from_int(720));
                }
                Some(p)
            })
    }

    /// Ring search around `near`, biased along `facing`, with lanes between lots.
    fn find_site(
        &self,
        bp: &UnitBlueprint,
        near: FxVec2,
        claimed: &[Claim],
        facing: Angle,
        min_r: Fx,
        keep_off_deposits: bool,
        home: Option<&staging::HomeGround>,
    ) -> Option<FxVec2> {
        let foot = bp.footprint.0.max(bp.footprint.1) as i32;
        let extra = if bp.has(cat::FACTORY) { 1 } else { 0 };
        let cell = mc_map::BUILD_CELL_M;
        let pitch = Fx::from_int((foot + 1 + extra) * cell);
        let salt = bp.categories.wrapping_mul(0x9E37_79B9)
            ^ (claimed.len() as u32).wrapping_mul(0x85EB_CA6B);
        let origin = snap_to_build_grid(bp, near);
        let ore = if keep_off_deposits {
            self.ore_centres()
        } else {
            Vec::new()
        };
        let free = |site: FxVec2| self.lot_free(bp, site, claimed, &ore, home);
        if free(origin) {
            return Some(origin);
        }
        let min_ring = (min_r / pitch).floor_int().max(1);
        let mut probes = 0i32;
        for ring in min_ring..40 {
            let slots = (ring.max(1) * 6) as u32;
            let start_k = salt % slots;
            let twist = Angle(
                (ring as u16)
                    .wrapping_mul(2701)
                    .wrapping_add((salt >> 8) as u16),
            );
            let jitter = pitch * Fx::ratio((salt as i64 >> 3) & 3, 14);
            for i in 0..slots {
                if probes >= MAX_SITE_PROBES {
                    return None;
                }
                probes += 1;
                let k = (start_k + i) % slots;
                let ang = facing + twist + Angle(((k as u64 * 65536) / slots as u64) as u16);
                let dist = pitch * ring + jitter;
                let site = snap_to_build_grid(bp, near + FxVec2::from_angle(ang) * dist);
                if free(site) {
                    return Some(site);
                }
            }
        }
        None
    }

    fn near_wreck(&self, from: FxVec2, range: Fx) -> Option<WreckId> {
        self.index
            .nearest(from, range, kind::WRECK, |e| {
                let w = e.row as usize;
                self.state.wrecks.slots.is_alive(w) && self.state.wrecks.pos[w] == e.pos
            })
            .map(|e| self.state.wrecks.slots.handle(e.row as usize))
    }

    /// Where to send a wave: a seen extractor or factory if we have one, else
    /// the closest enemy commander or start.
    fn attack_target(
        &self,
        player: u8,
        from: FxVec2,
        intel: &Intel,
        stance: Stance,
    ) -> Option<FxVec2> {
        // Survival: the engine cannot be hurt, so an AI defender goes only
        // for the nearest replication node, and otherwise holds.
        if self.state.survival.is_some() {
            return self
                .survival_nodes()
                .into_iter()
                .min_by_key(|p| (p.distance_sq(from), p.x, p.y));
        }
        if stance == Stance::Push {
            if let Some(c) = self.state.ai[player as usize]
                .contacts
                .iter()
                .filter(|c| self.blueprints.unit(c.blueprint).has(cat::COMMANDER))
                .min_by_key(|c| c.pos.distance_sq(from))
            {
                return Some(c.pos);
            }
        }
        if stance == Stance::Raid {
            if let Some(p) = intel
                .enemy_extractors
                .iter()
                .min_by_key(|p| (self.ai_objective_cost(player, **p, from), p.x, p.y))
            {
                return Some(*p);
            }
        }
        if let Some(p) = intel
            .enemy_factories
            .iter()
            .min_by_key(|p| (self.ai_objective_cost(player, **p, from), p.x, p.y))
        {
            return Some(*p);
        }
        if let Some(p) = intel
            .enemy_extractors
            .iter()
            .min_by_key(|p| (self.ai_objective_cost(player, **p, from), p.x, p.y))
        {
            return Some(*p);
        }
        self.state
            .players
            .iter()
            .enumerate()
            .filter(|(i, p)| !p.defeated && self.are_enemies(player, *i as u8))
            .map(|(_, p)| {
                self.state.units.row(p.commander).map_or(p.start, |row| {
                    if self.detects(player, row) {
                        self.state.units.pos[row]
                    } else {
                        p.start
                    }
                })
            })
            .min_by_key(|t| (t.distance_sq(from), t.x, t.y))
    }
}

#[cfg(test)]
mod tests;
