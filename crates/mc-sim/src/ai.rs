//! The skirmish AI.
//!
//! It runs inside the simulation on every machine, reads only game state and
//! its own hashed `AiState`, and acts by queueing ordinary commands for the
//! next tick. It gets no information a player would not have except the enemy
//! start positions, and no resource bonus.
//!
//! The Commander (`commander/`, `docs/AI_COMMANDER.md`) decides: it holds game
//! plans at a stake, runs operations of grouped units, and runs the economy.
//! The rest of this module carries out what it decides about the base: builders
//! (spread into yards and farms, turrets over the mass points), factories,
//! upgrades and salvage, steered through its `Directives`.

mod adaptive;
mod adjacent;
mod arrival;
mod builders;
mod commander;
pub use commander::mind::{AiMind, MindEconomy, MindNote, MindOp, MindPlan};
mod danger;
mod energy;
mod groups;
mod layout;
mod lots;
mod menu;
mod mine_sites;
mod production;
mod projects;
mod salvage;
mod sea;
mod staging;
mod theatre;
mod upgrades;
mod warp_ops;
use crate::command::{Command, PlayerCommand, MAX_COMMAND_UNITS};
use crate::spatial::kind;
use crate::tables::*;
use crate::world::snap_to_build_grid;
use crate::{AiConfig, Doctrine, Skill};
use crate::{SimError, World};
use adaptive::Contact;
use danger::{Danger, Loss};
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
const BARE_MINE_PROBES: usize = 12;
/// Enemies this close to a held point count as a raid.
const RAID_RADIUS: Fx = Fx::from_int(550);
/// Commander stays inside this radius of the start.
const HOME_RADIUS: Fx = Fx::from_int(640);
/// Point defense this close to a mass point is covering it.
const GUARD_COVER: Fx = Fx::from_int(210);

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
    /// Rotates through the factory roster.
    pub production_counter: u32,
    /// Forward gun line, chosen once and held.
    firebase: Option<FxVec2>,
    /// Where its buildings were destroyed lately (`danger.rs`).
    #[serde(default)]
    losses: Vec<Loss>,
    /// Which enemy starts its land army can walk to, found on the first think
    /// (`theatre.rs`).
    #[serde(default)]
    land_route: Option<theatre::LandRoute>,
    /// The Commander's plans and operations (`commander/`).
    #[serde(default)]
    commander: commander::state::CommanderState,
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
        h.write_u64(self.production_counter as u64);
        h.write_u64(self.land_route.map_or(u64::MAX, |r| r.reaches as u64));
        self.commander.hash(h);
        match self.firebase {
            Some(p) => {
                h.write_u64(1);
                h.write_i64(p.x.0);
                h.write_i64(p.y.0);
            }
            None => h.write_u64(0),
        }
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
    /// Radar towers on land: where, and how far each one sees.
    radar: Vec<(FxVec2, Fx)>,
    pd: Vec<FxVec2>,
    artillery: Vec<FxVec2>,
    shields: Vec<FxVec2>,
    anti_air: usize,
    engineer_factories: usize,
    air_factories: usize,
    naval_factories: usize,
    /// Reclaim towers standing: where, and how far each reaches.
    towers: Vec<(FxVec2, Fx)>,
    /// Salvage units (`UnitBlueprint::is_salvager`), and those with no orders.
    salvagers: usize,
    salvagers_idle: Vec<usize>,
    storage: usize,
    /// Material fabricators standing.
    fabricators: Vec<usize>,
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
    /// Armed spaceships with no orders (`warp_ops.rs`).
    capital_idle: Vec<usize>,
    /// Lift ships, busy or not (`landing.rs`).
    lifts: Vec<usize>,
    /// Sensor ships with no orders (`warp_ops.rs`).
    sensor_idle: Vec<usize>,
    /// Land-only combat units with no orders on a map where they cannot walk to
    /// any enemy: they guard home and never go out with a wave (`theatre.rs`).
    home_guard: Vec<usize>,
    /// Land-only combat units the side has, busy or not.
    land_bound: usize,
    /// The land army can walk to an enemy still in the game.
    land_route: bool,
    support_idle: Vec<usize>,
    army_fast: usize,
    max_tech: u8,
    /// Energy a second the side would draw with everything at work, its next
    /// mine upgrade and tier step included (`energy.rs`).
    energy_need: Fx,
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
    naval_factories: usize,
    power: usize,
    /// Land radar standing, going up or queued, with its range.
    radars: Vec<(FxVec2, Fx)>,
    pd: usize,
    artillery: usize,
    shields: usize,
    storage: usize,
    /// Material fabricators going up or queued.
    fabricators_rising: usize,
    /// Reclaim towers standing, going up or queued, with their reach.
    towers: Vec<(FxVec2, Fx)>,
    /// The wreck fields near home that are safe to work (`salvage.rs`).
    salvage: Vec<salvage::Field>,
    /// Strategic projects going up (`projects.rs`).
    projects: usize,
    guards: Vec<FxVec2>,
    /// Deposits a mine was chosen for this think and could not be placed by.
    failed_mines: Vec<FxVec2>,
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

/// How far a land radar tower sees. A sonar station out on the water is no
/// radar cover for the base, nor is a sensor ship that flies off.
fn land_radar(bp: &UnitBlueprint) -> Option<Fx> {
    (bp.has(cat::INTEL) && bp.is_structure() && !bp.water_only() && bp.radar > Fx::ZERO)
        .then_some(bp.radar)
}

/// Whether `at` is already well inside some tower's radar: within half its
/// range, so a raid still shows up on the way in.
fn radar_covers(radars: &[(FxVec2, Fx)], at: FxVec2) -> bool {
    radars
        .iter()
        .any(|&(pos, range)| pos.distance(at) < range / 2)
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
            // Sides think in turn, spread evenly over the period: with 32 of them
            // a fixed step would pile several onto one tick.
            let period = self.state.ai[p].config.think_period();
            let turn = p as u32 * period / self.state.players.len() as u32;
            if (self.state.tick + turn).is_multiple_of(period) {
                self.think(p as u8);
            }
        }
        Ok(())
    }

    fn think(&mut self, player: u8) {
        let span = mc_core::perf_span!("ai.remember");
        self.remember_enemies(player);
        drop(span);
        self.find_land_route_once(player);
        let span = mc_core::perf_span!("ai.survey");
        let census = self.survey_own(player);
        let intel = self.survey_intel(player, &census);
        let start = self.state.players[player as usize].start;
        let facing = intel
            .enemy_start
            .map(|e| (e - start).angle())
            .unwrap_or(Angle::ZERO);
        drop(span);
        let persona = self.ai_personality(player, &census, &intel);

        if self.state.ai[player as usize].firebase.is_none()
            && !census.factories.is_empty()
            && census.extractors.len() >= 2
        {
            if let Some(spot) = self.pick_firebase(start, &intel, persona) {
                self.state.ai[player as usize].firebase = Some(spot);
            }
        }
        let firebase = self.state.ai[player as usize].firebase;
        let stance = self.decide_stance(&census, &intel, firebase, persona, player);

        let span = mc_core::perf_span!("ai.claimed");
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
        // Deposits it lately failed to place a mine by are skipped.
        claimed.extend(self.blocked_claims(player));
        drop(span);
        let span = mc_core::perf_span!("ai.plan");
        let mut planned = self.plan_counts(player, &census);
        drop(span);
        let span = mc_core::perf_span!("ai.wrecks");
        planned.salvage = self.wreck_fields(start, &intel);
        drop(span);
        self.plan_economy(player, &census, &intel);
        let span = mc_core::perf_span!("ai.builders");
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
        self.note_failed_mines(player, &planned.failed_mines);
        drop(span);
        let span = mc_core::perf_span!("ai.rest");
        self.direct_factories(player, &census, &planned.salvage, &mut out);
        self.direct_salvagers(&census, &planned.salvage, &mut out);
        self.direct_upgrades(player, &census, &mut out);
        self.direct_fabricators(player, &census, &mut out);
        self.direct_focus(player, &mut out);
        drop(span);
        let span = mc_core::perf_span!("ai.commander");
        self.command(player, &census, &intel, &mut out);
        drop(span);
        let _span = mc_core::perf_span!("ai.route");
        let out = self.route_ai_commands(out);
        self.state.ai_pending.extend(
            out.into_iter()
                .map(|command| PlayerCommand { player, command }),
        );
    }

    /// What the side has standing, going up and queued, for this think's
    /// build choices to add to.
    fn plan_counts(&self, player: u8, census: &Census) -> Planned {
        let mut planned = Planned {
            factories: census.factories.len(),
            anti_air: census.anti_air,
            engineer_factories: census.engineer_factories,
            air_factories: census.air_factories,
            naval_factories: census.naval_factories,
            power: census.power.len(),
            radars: census.radar.clone(),
            pd: census.pd.len(),
            artillery: census.artillery.len(),
            shields: census.shields.len(),
            storage: census.storage,
            fabricators_rising: 0,
            towers: census.towers.clone(),
            salvage: Vec::new(),
            projects: 0,
            guards: census
                .pd
                .iter()
                .chain(&census.artillery)
                .chain(&census.shields)
                .copied()
                .collect(),
            failed_mines: Vec::new(),
        };

        for &row in &census.sites {
            let bp = self.bp(row);
            planned.factories += bp.has(cat::FACTORY) as usize;
            planned.engineer_factories +=
                (bp.has(cat::FACTORY) && self.blueprint_trains_engineers(bp)) as usize;
            planned.air_factories += bp.has(cat::FACTORY | cat::AIR) as usize;
            planned.naval_factories += (bp.has(cat::FACTORY) && adaptive::domain(bp) == 2) as usize;
            planned.power += bp.has(cat::POWER) as usize;
            planned.anti_air += bp.has(cat::DEFENSE | cat::ANTI_AIR) as usize;
            // A shield going up covers already: without this a second one was
            // ordered beside it while the first was still a frame.
            planned.shields += bp.has(cat::SHIELD) as usize;
            planned.projects += projects::project_kind(bp).is_some() as usize;
            planned.fabricators_rising += bp.fabricator.is_some() as usize;
            if let Some(range) = land_radar(bp) {
                planned.radars.push((self.state.units.pos[row], range));
            }
            if let Some(r) = bp.reclaimer.filter(|_| bp.is_structure()) {
                planned.towers.push((self.state.units.pos[row], r.range));
            }
        }
        // Towers a builder is walking to count too: without them every idle
        // builder of the next think ordered another radar and Scavenger.
        for (_, order) in self.planned_sites(player) {
            let bp = self.blueprints.unit(order.blueprint);
            planned.fabricators_rising += bp.fabricator.is_some() as usize;
            if let Some(range) = land_radar(bp) {
                planned.radars.push((order.pos, range));
            }
            if let Some(r) = bp.reclaimer.filter(|_| bp.is_structure()) {
                planned.towers.push((order.pos, r.range));
            }
        }
        planned
    }

    fn survey_own(&self, player: u8) -> Census {
        let units = &self.state.units;
        let land_route = self.land_route_to_enemy(player);
        let mut c = Census {
            land_route,
            ..Census::default()
        };
        // Land units and ships that are where they were sent count as free, though the
        // crowd there keeps their order from ending (`arrival.rs`).
        let arrived = self.arrived_army(player);
        for row in units.slots.iter() {
            if units.owner[row] != player || units.has_flag(row, flag::IN_FACTORY) {
                continue;
            }
            let bp = self.bp(row);
            c.max_tech = c.max_tech.max(bp.tech);
            if !units.is_active(row) {
                // An experimental raised on a lot is a site to help with too.
                if bp.is_structure() || bp.is_site_built_unit() {
                    c.sites.push(row);
                }
                continue;
            }
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
            // One with an upgrade queued behind its job is spoken for: a new
            // job would replace the upgrade.
            let idle = head == NO_ORDER || (finished_assist && !self.upgrading(row));
            if bp.is_mobile()
                && !bp.weapons.is_empty()
                && !bp.has(cat::ENGINEER)
                && !bp.has(cat::COMMANDER)
                && bp.transport.is_none()
            {
                c.combat_rows.push(row);
                let stuck = !land_route && theatre::land_bound(bp);
                c.land_bound += theatre::land_bound(bp) as usize;
                if !stuck {
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
                c.naval_factories += (adaptive::domain(bp) == 2) as usize;
                if idle {
                    c.factories_idle.push(row);
                }
            } else if bp.has(cat::EXTRACTOR) {
                c.extractors.push(row);
                c.extractor_pos.push(pos);
            } else if bp.has(cat::POWER) {
                c.power.push(pos);
            } else if bp.fabricator.is_some() && bp.is_structure() {
                c.fabricators.push(row);
            } else if let Some(range) = land_radar(bp) {
                c.radar.push((pos, range));
            } else if bp.has(cat::SHIELD) {
                c.shields.push(pos);
            } else if bp.has(cat::DEFENSE) && bp.has(cat::ARTILLERY) {
                c.artillery.push(pos);
            } else if bp.has(cat::DEFENSE) && bp.has(cat::DIRECT_FIRE) {
                c.pd.push(pos);
            } else if bp.has(cat::STORAGE) {
                c.storage += 1;
            } else if let Some(r) = bp.reclaimer.filter(|_| bp.is_structure()) {
                c.towers.push((pos, r.range));
            } else if bp.is_salvager() {
                if units.drone_parent[row] == crate::Handle::NONE {
                    c.salvagers += 1;
                    if idle {
                        c.salvagers_idle.push(row);
                    }
                }
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
            } else if bp.is_mobile() && bp.transport.is_some() {
                c.lifts.push(row);
            } else if menu::sensor_ship(bp) {
                if idle {
                    c.sensor_idle.push(row);
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
                let idle = idle || arrived.binary_search(&row).is_ok();
                if idle && !bp.weapons.is_empty() {
                    c.naval_idle.push(row);
                }
            } else if bp.is_mobile() && bp.has(cat::SPACE) {
                if idle && !bp.weapons.is_empty() {
                    c.capital_idle.push(row);
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
                let idle = idle || arrived.binary_search(&row).is_ok();
                if !land_route && theatre::land_bound(bp) {
                    if idle {
                        c.home_guard.push(row);
                    }
                } else if bp.has(cat::ARTILLERY) {
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
        // Room for the next mine upgrade and the next tier too: power built only
        // up to what the side draws now left none for them, and they waited on
        // it for twenty minutes.
        let mine = self
            .mine_to_upgrade(player, &c)
            .map_or(Fx::ZERO, |(row, next)| self.upgrade_draw(row, next));
        let tech = self.tech_step(player, &c).map_or(Fx::ZERO, |row| {
            let next = self.bp(row).upgrades_to.map(|n| self.blueprints.unit(n));
            next.map_or(Fx::ZERO, |next| self.upgrade_draw(row, next))
        });
        c.energy_need = self.energy_need(player) + mine + tech;
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
        let friends = self.team_mask(player);
        let mut held = vec![start];
        held.extend(census.extractor_pos.iter().copied().take(16));
        if let Some(f) = self.state.ai[player as usize].firebase {
            held.push(f);
        }
        // Flooded only once something turns up near a held point.
        let mut home: Option<Option<staging::HomeGround>> = None;
        for at in held {
            if let Some(e) = self
                .index
                .nearest_foe(at, RAID_RADIUS, kind::UNIT, friends, |e| {
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
                    // mine or a boat off the coast used to hold the whole army
                    // at home, sending a few units at it every think, for as long
                    // as it stayed; anti-air and the navy deal with those.
                    && self.bp(e.row as usize).motion.is_some_and(|m| {
                        !matches!(m.layer, mc_data::MoveLayer::Air | mc_data::MoveLayer::Naval)
                    })
                })
            {
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
        // An army big enough to push: the first wave's size by personality.
        let base = match persona {
            Personality::Aggressive => 6,
            Personality::Expander => 8,
            Personality::Turtle => 10,
        };
        let delta = self.state.ai[player as usize].config.skill().wave_delta;
        let wave = (base + delta).clamp(4, 40) as usize;
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

    /// The factory a builder should put up: of domain `only` if given
    /// (`adaptive::domain`), else the domain the side most wants.
    fn pick_factory(
        &self,
        builder_row: usize,
        max_tech: u8,
        want_air: bool,
        only: Option<usize>,
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
                    && only.is_none_or(|d| d == domain)
                    && bp.builder.as_ref().is_some_and(|f| !f.builds.is_empty())
                    && bp.tech <= max_tech
                    && self.state.ai[player].config.domain_weights[domain] > 0
                    && if domain == 2 {
                        // A shipyard goes on the water nearest home (`theatre.rs`).
                        self.shipyard_anchor(bp, self.state.players[player].start)
                            .is_some()
                    } else {
                        self.find_site(
                            bp,
                            self.state.units.pos[builder_row],
                            &[],
                            Angle::ZERO,
                            Fx::ZERO,
                            true,
                            None,
                        )
                        .is_some()
                    }
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
