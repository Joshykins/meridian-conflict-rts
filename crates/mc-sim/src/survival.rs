//! Survival: one side holds out against round after round from the
//! Replication Engine, a foundry nothing can harm.
//!
//! The engine does not conjure units. Every one is printed: it stands on one
//! of the engine's eight bays (or, for ships, in its harbor) as a construction
//! site while a print beam builds it up, then walks off to muster at the head
//! of its front. When the round's last unit is out, the whole round is sent
//! down its fronts together and on to wherever the defenders started.
//!
//! Between rounds the engine may raise a replication node: it fires its ray
//! across the map at a node site and builds the node up under it. A node
//! online prints one kind of unit, again and again, and sends each straight
//! at the defenders. Nodes can be destroyed (while being raised too), and
//! a destroyed node leaves a rich wreck: they are the bonus objectives.
//!
//! Nothing is handed out. The defenders' mass is what the engine sends them:
//! every unit it prints dies into a wreck worth most of its cost, so a
//! survival map carries little ore and the field is the economy (reclaim).
//!
//! All of it is state, driven from the match description (`SurvivalConfig`,
//! carried beside `MatchConfig` in the match options) and the tick: a replay
//! plays it out the same.

use crate::mirror::SimEvent;
use crate::reclaim::BeamInstance;
use crate::tables::{flag, NO_ORDER};
use crate::{Command, SimError, UnitId};
use mc_core::{Angle, Fx, FxVec2, Rng, StateHasher};
use mc_data::survival::Domain;
use mc_data::{cat, BlueprintId, MoveLayer};
use serde::{Deserialize, Serialize};

/// Ticks per second.
const HZ: u32 = 10;
/// `BeamInstance::kind` of the engine's ray raising a node.
pub const BEAM_REPLICATION_RAY: u32 = 4;
/// `BeamInstance::kind` of a print beam building a unit up.
pub const BEAM_PRINT: u32 = 5;
/// The engine's print bays, evenly round it; bay 0 faces the way it faces.
pub const BAYS: usize = 8;
/// Where a printed unit stands, from the engine's middle (metres).
pub const BAY_REACH: i32 = 150;
/// A bay's projector head: out from the middle, and up.
pub const BAY_EMITTER: (i32, i32) = (100, 55);
/// The engine's crown, where the ray leaves.
pub const CROWN: i32 = 140;
/// A node prints in front of itself, this far out.
pub const NODE_PRINT_REACH: i32 = 50;
/// The node's print emitter height.
pub const NODE_EMITTER: i32 = 40;
/// Ships printed at once in the harbor.
const HARBOR_SLIPS: usize = 3;
/// How long the ray takes to raise a node.
const RAISE_TICKS: u32 = 20 * HZ;
/// The engine stops printing while it has this many units in the field.
const FIELD_CAP: usize = 900;
/// Most units in one round.
const ROUND_CAP: usize = 160;
/// Idle hostile units are sent at the defenders this often.
const HUNT_EVERY: u32 = 5 * HZ;

/// What the player chooses on the survival set-up screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SurvivalRules {
    /// Rounds to survive; zero is endless.
    pub rounds: u16,
    /// Seconds before the first round starts printing.
    pub grace_secs: u16,
    /// Seconds from one round's launch to the next round's printing.
    pub interval_secs: u16,
    /// Round size, in thousandths of standard.
    pub intensity: u16,
    /// Which fronts attack: `Domain::bit`s.
    pub fronts: u8,
    /// The highest tech the engine replicates.
    pub tier_cap: u8,
    /// Replication nodes: 0 none, 1 rare, 2 regular, 3 frequent.
    pub nodes: u8,
}

impl Default for SurvivalRules {
    fn default() -> Self {
        SurvivalRules {
            rounds: 15,
            grace_secs: 240,
            interval_secs: 150,
            intensity: 1000,
            fronts: 7,
            tier_cap: 3,
            nodes: 2,
        }
    }
}

impl SurvivalRules {
    pub fn has(&self, domain: Domain) -> bool {
        self.fronts & domain.bit() != 0
    }

    /// The tech the engine prints at in `round` (1-based).
    pub fn tier_at(&self, round: u16) -> u8 {
        let cap = self.tier_cap.clamp(1, mc_data::MAX_TECH) as u32;
        let r = round.max(1) as u32 - 1;
        // Endless climbs a tier every six rounds.
        let tier = if self.rounds == 0 {
            1 + r / 6
        } else {
            1 + r * cap / self.rounds as u32
        };
        tier.min(cap) as u8
    }

    /// Mass the engine plans to spend on `round`: 800 at the first, a
    /// quarter more each round after. Tiers cost about four times the one
    /// below, so this keeps the rounds' numbers up as the tiers climb. What it
    /// spends is also most of what the defenders can reclaim afterwards.
    pub fn budget(&self, round: u16) -> i64 {
        let mut mass: i64 = 800;
        for _ in 1..round.max(1).min(200) {
            mass = (mass * 5 / 4).min(1 << 40);
        }
        mass * self.intensity as i64 / 1000
    }

    /// The budget the engine actually spends: the planned one, or a share of
    /// what the defenders take in (mines and reclaim) over one gap when that is
    /// more, 15% at the first round rising 6 points a round, so a defender who
    /// builds up is met in kind.
    pub fn budget_against(&self, round: u16, income: Fx) -> i64 {
        let r = round.max(1) as i64 - 1;
        let taken = income.round_int().max(0) as i64 * self.interval_secs.max(30) as i64;
        let share = 15 + 6 * r;
        self.budget(round)
            .max(taken * share / 100 * self.intensity as i64 / 1000)
    }

    /// Rounds at which a node is raised, or none.
    pub fn raises_node(&self, round: u16) -> bool {
        let (first, every) = match self.nodes {
            0 => return false,
            1 => (4, 5),
            2 => (3, 3),
            _ => (2, 2),
        };
        round >= first && (round - first) % every == 0
    }

    /// Most nodes standing at once.
    pub fn node_limit(&self) -> usize {
        match self.nodes {
            0 => 0,
            1 => 3,
            2 => 6,
            _ => 10,
        }
    }
}

/// One way the engine's forces come, from its side to the defenders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Front {
    pub domain: Domain,
    pub path: Vec<FxVec2>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeSite {
    pub at: FxVec2,
    pub domain: Domain,
}

/// The survival half of a match description. Built from the map's layout
/// and the set-up screen's rules; travels with the match options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurvivalConfig {
    /// The engine's side. Every other player defends.
    pub engine_player: u8,
    pub engine: FxVec2,
    pub harbor: Option<FxVec2>,
    pub fronts: Vec<Front>,
    pub node_sites: Vec<NodeSite>,
    pub rules: SurvivalRules,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// Before the first round.
    #[default]
    Grace,
    /// A round is being printed and mustered.
    Printing,
    /// The round has gone; the next is `next_at`.
    Launched,
    /// The last round has gone: kill what is left to win.
    Final,
    /// Held out.
    Won,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Pending {
    blueprint: BlueprintId,
    front: u8,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Print {
    unit: UnitId,
    /// The engine's bay (0..BAYS), a harbor slip (BAYS..), or `NODE_PRINT`.
    bay: u8,
    /// Engine or node doing the printing.
    source: UnitId,
    /// Front it goes down; `u8::MAX` for a node's own.
    front: u8,
    started: u32,
    ticks: u32,
}

const NODE_PRINT: u8 = u8::MAX;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Node {
    unit: UnitId,
    site: u8,
    product: BlueprintId,
    /// Tick the ray started. Online once `raised`.
    started: u32,
    raised: bool,
    next_print: u32,
    printed: u32,
}

/// Survival's part of the game state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Survival {
    pub config: SurvivalConfig,
    engine: UnitId,
    /// Rounds begun so far; the current one while printing.
    pub round: u16,
    pub phase: Phase,
    /// Tick the next round starts printing.
    pub next_at: u32,
    queue: Vec<Pending>,
    printing: Vec<Print>,
    /// Printed and waiting at the head of their front for the round to go.
    mustered: Vec<(UnitId, u8)>,
    /// Every unit sent in a round and not yet dead.
    waves: Vec<UnitId>,
    nodes: Vec<Node>,
    rng: Rng,
    pub nodes_destroyed: u16,
    /// What the round being printed is made of, per domain, then per tech.
    incoming: [[u16; 5]; 3],
}

/// What the HUD shows of survival; published with the sim status.
#[derive(Clone, Debug, Default)]
pub struct SurvivalStatus {
    pub round: u16,
    /// Zero: endless.
    pub rounds: u16,
    pub phase: Phase,
    /// Ticks until the next round starts printing (0 while one is).
    pub next_in: u32,
    /// Tech the engine prints at now.
    pub tier: u8,
    /// The round being printed (or the next one's forecast between rounds):
    /// units per domain (`Domain::ALL` order), per tech 1..=5.
    pub incoming: [[u16; 5]; 3],
    /// Units printed so far of the round being printed, and its size.
    pub printed: (u16, u16),
    /// Hostile units in the field.
    pub hostile: u32,
    /// Units of the rounds sent that are still alive (the final phase ends at zero).
    pub remaining: u32,
    pub engine: [f32; 2],
    pub nodes: Vec<NodeStatus>,
    pub nodes_destroyed: u16,
    /// Mass per second the defender takes in from wrecks now.
    pub reclaim: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct NodeStatus {
    pub site: u8,
    pub pos: [f32; 2],
    pub product: BlueprintId,
    /// 0..1 while the ray raises it; 1 online.
    pub raised: f32,
    pub printed: u32,
}

fn domain_of(layer: MoveLayer) -> Domain {
    match layer {
        MoveLayer::Air => Domain::Air,
        MoveLayer::Naval => Domain::Naval,
        _ => Domain::Land,
    }
}

fn domain_index(d: Domain) -> usize {
    match d {
        Domain::Land => 0,
        Domain::Air => 1,
        Domain::Naval => 2,
    }
}

impl Survival {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.engine.0 as u64 | (self.round as u64) << 32 | (self.phase as u64) << 48);
        h.write_u64(self.next_at as u64 | (self.nodes_destroyed as u64) << 32);
        h.write_u64(self.rng.state());
        h.write_u64(self.queue.len() as u64);
        for p in &self.queue {
            h.write_u64(p.blueprint.0 as u64 | (p.front as u64) << 32);
        }
        for p in &self.printing {
            h.write_u64(p.unit.0 as u64 | (p.source.0 as u64) << 32);
            h.write_u64(p.started as u64 | (p.bay as u64) << 32 | (p.front as u64) << 40);
        }
        for (u, f) in &self.mustered {
            h.write_u64(u.0 as u64 | (*f as u64) << 32);
        }
        h.write_u64(self.waves.len() as u64);
        for n in &self.nodes {
            h.write_u64(n.unit.0 as u64 | (n.site as u64) << 32 | (n.raised as u64) << 40);
            h.write_u64(n.next_print as u64 | (n.printed as u64) << 32);
        }
    }
}

impl crate::World {
    /// Turns this match into survival. Call once, right after `World::new`,
    /// before the first tick: removes the engine side's commander, raises the
    /// engine and its defences, and starts the clock.
    pub fn begin_survival(&mut self, config: SurvivalConfig) -> Result<(), SimError> {
        let side = config.engine_player;
        if side as usize >= self.state.players.len() {
            return Err(SimError::Setup("survival: no such engine side".into()));
        }
        let bp_of = |w: &crate::World, key: &str| {
            w.blueprints
                .id_of(key)
                .ok_or_else(|| SimError::Setup(format!("survival needs blueprint {key}")))
        };
        let engine_bp = bp_of(self, "replication_engine")?;
        if let Some(row) = self
            .state
            .units
            .row(self.state.players[side as usize].commander)
        {
            self.remove_unit_row(row, false)?;
        }
        self.state.players[side as usize].commander = crate::Handle::NONE;
        self.state.players[side as usize].free_build = true;

        let target = self.survival_target_of(&config);
        let heading = (target - config.engine).angle();
        let engine_pos =
            crate::world::snap_to_build_grid(self.blueprints.unit(engine_bp), config.engine);
        let row = self.spawn_unit(engine_bp, side, engine_pos, heading, true)?;
        self.state.units.flags[row] |= flag::INVULNERABLE;
        let engine = self.state.units.id(row);

        // Its guard: a ring of turrets, heavier toward the defenders.
        let guard: [(&str, i32, i32, i32); 7] = [
            // key, count, reach (m), spread either side of the facing (degrees; 180 = all round)
            ("aster_t2_point_defense", 8, 330, 110),
            ("aster_t2_point_defense", 4, 360, 180),
            ("aster_t2_aa", 6, 300, 180),
            ("aster_t3_sam", 3, 420, 180),
            ("aster_t3_shatter", 2, 280, 90),
            ("aster_t2_artillery", 3, 250, 60),
            ("aster_t2_shield", 3, 390, 70),
        ];
        for (key, count, reach, spread) in guard {
            let Some(bp) = self.blueprints.id_of(key) else {
                continue;
            };
            for i in 0..count {
                let t = if count == 1 {
                    0
                } else {
                    i * 2 * spread / (count - 1) - spread
                };
                let t = if spread >= 180 { i * 360 / count } else { t };
                let dir = heading + Angle::from_degrees(t);
                let want = engine_pos + FxVec2::from_angle(dir) * Fx::from_int(reach);
                let ubp = self.blueprints.unit(bp);
                let pos = crate::world::snap_to_build_grid(ubp, want);
                if self.can_place(ubp, pos) {
                    self.spawn_unit(bp, side, pos, dir, true)?;
                }
            }
        }

        let first = config.rules.grace_secs as u32 * HZ;
        let seed = self.state.rng.state() ^ 0x5EED_5A1A;
        self.state.survival = Some(Survival {
            config,
            engine,
            round: 0,
            phase: Phase::Grace,
            next_at: self.state.tick + first.max(1),
            queue: Vec::new(),
            printing: Vec::new(),
            mustered: Vec::new(),
            waves: Vec::new(),
            nodes: Vec::new(),
            rng: Rng::new(seed),
            nodes_destroyed: 0,
            incoming: [[0; 5]; 3],
        });
        let forecast = self.plan_round(1, false);
        if let Some(s) = self.state.survival.as_mut() {
            s.incoming = tally(self.blueprints.as_ref(), &forecast);
        }
        self.rebuild_index();
        Ok(())
    }

    /// Where the defenders are: the first defender's start.
    fn survival_target_of(&self, config: &SurvivalConfig) -> FxVec2 {
        self.state
            .players
            .iter()
            .enumerate()
            .find(|(p, _)| *p as u8 != config.engine_player)
            .map_or(config.engine, |(_, p)| p.start)
    }

    fn survival_target(&self) -> FxVec2 {
        let Some(s) = self.state.survival.as_ref() else {
            return FxVec2::ZERO;
        };
        // The nearest living defender commander, else where they started.
        let from = s.config.engine;
        let mut best: Option<(Fx, FxVec2)> = None;
        for (p, pl) in self.state.players.iter().enumerate() {
            if p as u8 == s.config.engine_player || pl.defeated {
                continue;
            }
            let at = self
                .state
                .units
                .row(pl.commander)
                .map_or(pl.start, |r| self.state.units.pos[r]);
            let d = at.distance_sq(from);
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, at));
            }
        }
        best.map_or_else(|| self.survival_target_of(&s.config), |(_, at)| at)
    }

    /// Survival's tick: after the AI, before victory is checked.
    pub(crate) fn run_survival(&mut self) -> Result<(), SimError> {
        let Some(s) = self.state.survival.as_ref() else {
            return Ok(());
        };
        // Held out, or the defenders are gone: the engine falls quiet.
        if s.phase == Phase::Won || self.state.winner.is_some() {
            return Ok(());
        }
        let now = self.state.tick;
        self.survival_nodes_lost();
        self.survival_prints(now)?;
        self.survival_raise(now)?;
        self.survival_node_prints(now)?;

        let s = self.state.survival.as_ref().unwrap();
        let rules = s.config.rules;
        match s.phase {
            Phase::Grace | Phase::Launched if now >= s.next_at => {
                let round = s.round + 1;
                let plan = self.plan_round(round, true);
                let incoming = tally(self.blueprints.as_ref(), &plan);
                let s = self.state.survival.as_mut().unwrap();
                s.round = round;
                s.phase = Phase::Printing;
                s.queue = plan;
                s.incoming = incoming;
                self.events.push(SimEvent::RoundPrinting { round });
                if rules.raises_node(round) {
                    self.survival_start_node(now)?;
                }
            }
            Phase::Printing => {
                let done = s.queue.is_empty() && !s.printing.iter().any(|p| p.front != NODE_PRINT);
                if done {
                    self.survival_launch(now)?;
                }
            }
            _ => {}
        }
        self.survival_bays(now)?;
        if now % HUNT_EVERY == 0 {
            self.survival_hunt()?;
        }
        self.survival_check_won();
        Ok(())
    }

    /// The units of `round`: spend its budget over the fronts it has.
    fn plan_round(&mut self, round: u16, consume: bool) -> Vec<Pending> {
        let Some(s) = self.state.survival.as_ref() else {
            return Vec::new();
        };
        let rules = s.config.rules;
        let tier = rules.tier_at(round);
        let fronts: Vec<(usize, Domain)> = s
            .config
            .fronts
            .iter()
            .enumerate()
            .filter(|(_, f)| rules.has(f.domain))
            .map(|(i, f)| (i, f.domain))
            .collect();
        let mut rng = if consume {
            s.rng.clone()
        } else {
            Rng::new(s.rng.state() ^ round as u64)
        };
        // Shares of the budget: land carries most, then air, then the sea.
        let weight = |d: Domain| match d {
            Domain::Land => 60,
            Domain::Air => 25,
            Domain::Naval => 20,
        };
        let domains: Vec<Domain> = Domain::ALL
            .into_iter()
            .filter(|d| fronts.iter().any(|(_, f)| f == d) && !self.roster(*d, tier).is_empty())
            .collect();
        let total: i64 = domains.iter().map(|d| weight(*d)).sum();
        let side = s.config.engine_player;
        let income = self
            .state
            .players
            .iter()
            .enumerate()
            .filter(|(p, pl)| *p as u8 != side && !pl.defeated)
            .fold(Fx::ZERO, |sum, (_, pl)| {
                sum + pl.mass_income + pl.reclaim_income
            });
        let budget = rules.budget_against(round, income);
        let mut out = Vec::new();
        for d in &domains {
            let mut left = budget * weight(*d) / total.max(1);
            let roster = self.roster(*d, tier);
            let lanes: Vec<u8> = fronts
                .iter()
                .filter(|(_, f)| f == d)
                .map(|(i, _)| *i as u8)
                .collect();
            let mut guard = 0;
            while left > 0 && out.len() < ROUND_CAP && guard < 400 {
                guard += 1;
                // Mostly the newest tier, some of the ones before it.
                let want = if tier > 1 && rng.below(100) < 30 {
                    1 + rng.below(tier as u32 - 1) as u8
                } else {
                    tier
                };
                let pick: Vec<&(BlueprintId, u8, i64)> =
                    roster.iter().filter(|u| u.1 == want).collect();
                let pick = if pick.is_empty() {
                    roster.iter().collect()
                } else {
                    pick
                };
                let (bp, _, cost) = *pick[rng.below(pick.len() as u32) as usize];
                // The first unit is always bought, so a lean budget still sends something.
                if cost > left
                    && !out
                        .iter()
                        .any(|p: &Pending| domain_of_bp(self, p.blueprint) == *d)
                {
                    left = 0;
                } else if cost > left {
                    break;
                } else {
                    left -= cost;
                }
                let front = lanes[rng.below(lanes.len() as u32) as usize];
                out.push(Pending {
                    blueprint: bp,
                    front,
                });
            }
        }
        if consume {
            self.state.survival.as_mut().unwrap().rng = rng;
        }
        out
    }

    /// Fighting units of `domain` up to `tier`, of any faction: blueprint, tech, mass.
    fn roster(&self, domain: Domain, tier: u8) -> Vec<(BlueprintId, u8, i64)> {
        let bps = self.blueprints.as_ref();
        bps.units
            .iter()
            .filter(|u| {
                bps.is_listed(u.id)
                    && u.tech <= tier
                    && u.categories
                        & (cat::COMMANDER
                            | cat::ENGINEER
                            | cat::SCOUT
                            | cat::REPLICATOR
                            | cat::EXPERIMENTAL)
                        == 0
                    && !u.weapons.is_empty()
                    && u.motion.is_some_and(|m| domain_of(m.layer) == domain)
                    && !bps.units.iter().any(|o| o.drone == Some(u.id))
            })
            .map(|u| (u.id, u.tech, u.cost_mass.round_int().max(1) as i64))
            .collect()
    }

    fn engine_row(&self) -> Option<usize> {
        self.state.units.row(self.state.survival.as_ref()?.engine)
    }

    /// Hands queued units to free bays and harbor slips.
    fn survival_bays(&mut self, now: u32) -> Result<(), SimError> {
        let Some(engine) = self.engine_row() else {
            return Ok(());
        };
        let s = self.state.survival.as_ref().unwrap();
        if s.queue.is_empty() {
            return Ok(());
        }
        let side = s.config.engine_player;
        if self.hostile_count(side) >= FIELD_CAP {
            return Ok(());
        }
        let harbor = s.config.harbor;
        let pos = self.state.units.pos[engine];
        let heading = self.state.units.heading[engine];
        let source = self.state.units.id(engine);
        let mut queue = std::mem::take(&mut self.state.survival.as_mut().unwrap().queue);
        let mut i = 0;
        while i < queue.len() {
            let Some(motion) = self.blueprints.unit(queue[i].blueprint).motion else {
                queue.remove(i);
                continue;
            };
            let naval = domain_of(motion.layer) == Domain::Naval;
            if naval && harbor.is_none() {
                queue.remove(i);
                continue;
            }
            let mut slots = if naval {
                BAYS..BAYS + HARBOR_SLIPS
            } else {
                0..BAYS
            };
            let Some(bay) = slots.find(|b| !self.survival_bay_taken(*b as u8)) else {
                i += 1;
                continue;
            };
            let at = match harbor {
                Some(h) if naval => {
                    let k = (bay - BAYS) as i32 - 1;
                    h + FxVec2::from_angle(heading + Angle::QUARTER_TURN) * Fx::from_int(70 * k)
                }
                _ => {
                    let dir = heading + Angle(((bay as u32 * 65536) / BAYS as u32) as u16);
                    pos + FxVec2::from_angle(dir) * Fx::from_int(BAY_REACH)
                }
            };
            let at = self
                .nav
                .nearest_passable(motion.layer, motion.size_class, at)
                .unwrap_or(at);
            let Pending { blueprint, front } = queue.remove(i);
            let row = self.spawn_unit(blueprint, side, at, heading, false)?;
            let ticks = print_ticks(self.blueprints.unit(blueprint).tech);
            let unit = self.state.units.id(row);
            self.state.survival.as_mut().unwrap().printing.push(Print {
                unit,
                bay: bay as u8,
                source,
                front,
                started: now,
                ticks,
            });
        }
        self.state.survival.as_mut().unwrap().queue = queue;
        Ok(())
    }

    /// A finished unit still standing on its bay keeps the bay.
    fn survival_bay_taken(&self, bay: u8) -> bool {
        let s = self.state.survival.as_ref().unwrap();
        s.printing.iter().any(|p| p.bay == bay)
    }

    fn hostile_count(&self, side: u8) -> usize {
        let u = &self.state.units;
        u.slots
            .iter()
            .filter(|&r| u.owner[r] == side && self.blueprints.unit(u.blueprint[r]).is_mobile())
            .count()
    }

    /// Builds up everything being printed; releases what is done.
    fn survival_prints(&mut self, now: u32) -> Result<(), SimError> {
        let Some(s) = self.state.survival.as_mut() else {
            return Ok(());
        };
        let prints = std::mem::take(&mut s.printing);
        let mut keep = Vec::with_capacity(prints.len());
        let mut done = Vec::new();
        for p in prints {
            let Some(row) = self.state.units.row(p.unit) else {
                continue; // shot down on the bay
            };
            if self.state.units.row(p.source).is_none() {
                // Its node fell: what it was printing falls apart with it.
                self.state.units.health[row] = Fx::ZERO;
                continue;
            }
            let bp = self.blueprints.unit(self.state.units.blueprint[row]);
            let t = Fx::ratio((now - p.started) as i64, p.ticks.max(1) as i64).min(Fx::ONE);
            self.state.units.build_progress[row] = bp.build_time * t;
            let full = self.unit_max_health(row);
            let floor = full / 10;
            let want = floor + (full - floor) * t;
            if self.state.units.health[row] < want {
                self.state.units.health[row] = want.min(full);
            }
            if t >= Fx::ONE {
                self.complete_unit(row)?;
                done.push(p);
            } else {
                keep.push(p);
            }
        }
        self.state.survival.as_mut().unwrap().printing = keep;
        for p in done {
            if p.front == NODE_PRINT {
                self.survival_send(&[p.unit], None)?;
            } else {
                self.survival_muster(p.unit, p.front)?;
            }
        }
        Ok(())
    }

    /// Sends a printed round unit to wait at the head of its front.
    fn survival_muster(&mut self, unit: UnitId, front: u8) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let Some(head) = s
            .config
            .fronts
            .get(front as usize)
            .and_then(|f| f.path.first().copied())
        else {
            return self.survival_send(&[unit], None);
        };
        let side = s.config.engine_player;
        let n = s.mustered.iter().filter(|(_, f)| *f == front).count() as i32;
        // Spread the muster so the round does not stand on one point.
        let ring = Angle(((n as u32 * 40503) & 0xFFFF) as u16);
        let at = head + FxVec2::from_angle(ring) * Fx::from_int(12 + 6 * (n % 9));
        self.state
            .survival
            .as_mut()
            .unwrap()
            .mustered
            .push((unit, front));
        self.apply_as(
            side,
            &Command::Move {
                units: vec![unit],
                target: at,
                queue: false,
            },
        )
    }

    /// The round goes: every mustered unit down its front, then at the defenders.
    fn survival_launch(&mut self, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_mut().unwrap();
        let mustered = std::mem::take(&mut s.mustered);
        let round = s.round;
        let rules = s.config.rules;
        let last = rules.rounds != 0 && round >= rules.rounds;
        s.phase = if last { Phase::Final } else { Phase::Launched };
        s.next_at = now + rules.interval_secs as u32 * HZ;
        let fronts = s.config.fronts.len();
        let mut sent = 0u16;
        for f in 0..fronts {
            let units: Vec<UnitId> = mustered
                .iter()
                .filter(|(u, lane)| *lane as usize == f && self.state.units.row(*u).is_some())
                .map(|(u, _)| *u)
                .collect();
            if units.is_empty() {
                continue;
            }
            sent += units.len() as u16;
            self.survival_send(&units, Some(f))?;
        }
        let s = self.state.survival.as_mut().unwrap();
        s.waves.extend(mustered.iter().map(|(u, _)| *u));
        let forecast = if last {
            Vec::new()
        } else {
            self.plan_round(round + 1, false)
        };
        let s = self.state.survival.as_mut().unwrap();
        s.incoming = tally(self.blueprints.as_ref(), &forecast);
        self.events
            .push(SimEvent::RoundLaunched { round, units: sent });
        Ok(())
    }

    /// Attack-moves `units` down front `front` (if any), then at the defenders.
    fn survival_send(&mut self, units: &[UnitId], front: Option<usize>) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let side = s.config.engine_player;
        let path: Vec<FxVec2> = front
            .and_then(|f| s.config.fronts.get(f))
            .map_or(Vec::new(), |f| f.path.iter().skip(1).copied().collect());
        let target = self.survival_target();
        for chunk in units.chunks(crate::command::MAX_COMMAND_UNITS) {
            let mut queue = false;
            for &p in path.iter().chain(std::iter::once(&target)) {
                self.apply_as(
                    side,
                    &Command::AttackMove {
                        units: chunk.to_vec(),
                        target: p,
                        queue,
                    },
                )?;
                queue = true;
            }
        }
        Ok(())
    }

    /// Idle hostile units that are not waiting to muster go at the defenders.
    fn survival_hunt(&mut self) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let side = s.config.engine_player;
        let u = &self.state.units;
        let idle: Vec<UnitId> = u
            .slots
            .iter()
            .filter(|&r| {
                u.owner[r] == side
                    && u.order_head[r] == NO_ORDER
                    && u.is_active(r)
                    && self.blueprints.unit(u.blueprint[r]).is_mobile()
                    && !s.mustered.iter().any(|(m, _)| *m == u.id(r))
            })
            .map(|r| u.id(r))
            .collect();
        if idle.is_empty() {
            return Ok(());
        }
        self.survival_send(&idle, None)
    }

    /// Picks a free site and starts the ray on it.
    fn survival_start_node(&mut self, now: u32) -> Result<(), SimError> {
        if self.engine_row().is_none() {
            return Ok(());
        }
        let s = self.state.survival.as_ref().unwrap();
        let rules = s.config.rules;
        if s.nodes.len() >= rules.node_limit() || s.nodes.iter().any(|n| !n.raised) {
            return Ok(());
        }
        let free: Vec<usize> = s
            .config
            .node_sites
            .iter()
            .enumerate()
            .filter(|(i, site)| {
                !s.nodes.iter().any(|n| n.site as usize == *i)
                    && match site.domain {
                        Domain::Naval => rules.has(Domain::Naval) || rules.has(Domain::Air),
                        _ => rules.has(Domain::Land) || rules.has(Domain::Air),
                    }
            })
            .map(|(i, _)| i)
            .collect();
        if free.is_empty() {
            return Ok(());
        }
        let tier = rules.tier_at(s.round);
        let mut rng = s.rng.clone();
        let site_i = free[rng.below(free.len() as u32) as usize];
        let site = s.config.node_sites[site_i];
        // A land site prints for land, a sea site for the sea; either may print aircraft.
        let mut kinds = Vec::new();
        if rules.has(site.domain) && site.domain != Domain::Air {
            kinds.push(site.domain);
        }
        if rules.has(Domain::Air) {
            kinds.push(Domain::Air);
        }
        if kinds.is_empty() {
            return Ok(());
        }
        let side = s.config.engine_player;
        let domain = kinds[rng.below(kinds.len() as u32) as usize];
        let roster = self.roster(domain, tier);
        let top: Vec<_> = roster.iter().filter(|u| u.1 == tier).collect();
        let pick = if top.is_empty() {
            roster.iter().collect::<Vec<_>>()
        } else {
            top
        };
        if pick.is_empty() {
            return Ok(());
        }
        let product = pick[rng.below(pick.len() as u32) as usize].0;
        self.state.survival.as_mut().unwrap().rng = rng;

        let node_bp = self
            .blueprints
            .id_of("replication_node")
            .ok_or_else(|| SimError::Setup("survival needs blueprint replication_node".into()))?;
        let nbp = self.blueprints.unit(node_bp);
        let pos = crate::world::snap_to_build_grid(nbp, site.at);
        if !self.can_place(nbp, pos) {
            return Ok(());
        }
        let heading = (self.survival_target() - pos).angle();
        let row = self.spawn_unit(node_bp, side, pos, heading, false)?;
        let unit = self.state.units.id(row);
        let s = self.state.survival.as_mut().unwrap();
        s.nodes.push(Node {
            unit,
            site: site_i as u8,
            product,
            started: now,
            raised: false,
            next_print: 0,
            printed: 0,
        });
        self.events.push(SimEvent::NodeRaising {
            site: site_i as u8,
            pos,
            product,
        });
        Ok(())
    }

    /// The ray builds up the node it is on.
    fn survival_raise(&mut self, now: u32) -> Result<(), SimError> {
        let Some(s) = self.state.survival.as_ref() else {
            return Ok(());
        };
        let Some(i) = s.nodes.iter().position(|n| !n.raised) else {
            return Ok(());
        };
        let n = s.nodes[i];
        let Some(row) = self.state.units.row(n.unit) else {
            return Ok(());
        };
        let t = Fx::ratio((now - n.started) as i64, RAISE_TICKS as i64).min(Fx::ONE);
        let bp = self.blueprints.unit(self.state.units.blueprint[row]);
        self.state.units.build_progress[row] = bp.build_time * t;
        let full = self.unit_max_health(row);
        let floor = full / 10;
        let want = floor + (full - floor) * t;
        // Damage taken while it goes up stays taken.
        if self.state.units.health[row] < want && now % HZ == 0 {
            self.state.units.health[row] = (self.state.units.health[row]
                + (full - floor) * HZ as i32 / RAISE_TICKS as i32)
                .min(want);
        }
        if t >= Fx::ONE {
            self.complete_unit(row)?;
            let pos = self.state.units.pos[row];
            let s = self.state.survival.as_mut().unwrap();
            s.nodes[i].raised = true;
            s.nodes[i].next_print = now + 3 * HZ;
            self.events.push(SimEvent::NodeOnline {
                site: n.site,
                pos,
                product: n.product,
            });
        }
        Ok(())
    }

    /// Online nodes print their one kind of unit, on a beat set by its tech.
    fn survival_node_prints(&mut self, now: u32) -> Result<(), SimError> {
        let Some(s) = self.state.survival.as_ref() else {
            return Ok(());
        };
        let side = s.config.engine_player;
        let nodes = s.nodes.clone();
        for (i, n) in nodes.iter().enumerate() {
            if !n.raised || now < n.next_print {
                continue;
            }
            let Some(row) = self.state.units.row(n.unit) else {
                continue;
            };
            let s = self.state.survival.as_ref().unwrap();
            if s.printing.iter().any(|p| p.source == n.unit) {
                continue;
            }
            let tech = self.blueprints.unit(n.product).tech;
            let Some(motion) = self.blueprints.unit(n.product).motion else {
                continue;
            };
            if self.hostile_count(side) >= FIELD_CAP {
                continue;
            }
            let pos = self.state.units.pos[row];
            let heading = self.state.units.heading[row];
            let at = pos + FxVec2::from_angle(heading) * Fx::from_int(NODE_PRINT_REACH);
            let at = self
                .nav
                .nearest_passable(motion.layer, motion.size_class, at)
                .unwrap_or(at);
            let urow = self.spawn_unit(n.product, side, at, heading, false)?;
            let unit = self.state.units.id(urow);
            let s = self.state.survival.as_mut().unwrap();
            s.printing.push(Print {
                unit,
                bay: NODE_PRINT,
                source: n.unit,
                front: NODE_PRINT,
                started: now,
                ticks: print_ticks(tech),
            });
            s.nodes[i].printed += 1;
            // Slower for bigger units; faster as the rounds climb.
            let beat = (18 + 14 * tech as u32) * HZ * 10 / (10 + s.round.min(20) as u32 / 2);
            s.nodes[i].next_print = now + beat;
        }
        Ok(())
    }

    /// Nodes that died since last tick: tell the defenders what their wreck holds.
    fn survival_nodes_lost(&mut self) {
        let Some(s) = self.state.survival.as_mut() else {
            return;
        };
        let units = &self.state.units;
        let mut lost = Vec::new();
        s.nodes.retain(|n| {
            if units.row(n.unit).is_some() {
                true
            } else {
                lost.push(*n);
                false
            }
        });
        for n in lost {
            let node = self
                .blueprints
                .id_of("replication_node")
                .map(|b| self.blueprints.unit(b));
            let wreck = match node {
                Some(bp) if n.raised => {
                    (bp.cost_mass * bp.wreck_fraction).round_int().max(0) as u32
                }
                _ => 0,
            };
            let s = self.state.survival.as_mut().unwrap();
            s.nodes_destroyed += 1;
            let pos = s
                .config
                .node_sites
                .get(n.site as usize)
                .map_or(FxVec2::ZERO, |x| x.at);
            self.events.push(SimEvent::NodeDestroyed {
                site: n.site,
                pos,
                product: n.product,
                wreck,
                raised: n.raised,
            });
        }
    }

    /// The last round has gone and none of it is left: the defenders win.
    fn survival_check_won(&mut self) {
        let Some(s) = self.state.survival.as_mut() else {
            return;
        };
        let units = &self.state.units;
        s.waves.retain(|u| units.row(*u).is_some());
        if s.phase != Phase::Final || !s.waves.is_empty() || !s.mustered.is_empty() {
            return;
        }
        let side = s.config.engine_player;
        let rounds = s.round;
        s.phase = Phase::Won;
        let team = self
            .state
            .players
            .iter()
            .enumerate()
            .find(|(p, pl)| *p as u8 != side && !pl.defeated)
            .map(|(_, pl)| pl.team);
        if let Some(team) = team {
            if self.state.winner.is_none() {
                self.state.winner = Some(team);
                self.events.push(SimEvent::SurvivalWon { rounds });
                self.events.push(SimEvent::MatchOver { winner_team: team });
            }
        }
    }

    /// Everyone the engine side can see: its engine and nodes are never hidden from the defenders.
    pub(crate) fn survival_reveal(&mut self) {
        let Some(s) = self.state.survival.as_ref() else {
            return;
        };
        let side = s.config.engine_player;
        let mut mask = 0u8;
        for p in 0..self.state.players.len() {
            if p as u8 != side {
                mask |= self.team_mask(p as u8);
            }
        }
        let engine = self
            .state
            .units
            .row(s.engine)
            .map(|r| self.state.units.pos[r]);
        let nodes: Vec<FxVec2> = s
            .nodes
            .iter()
            .filter_map(|n| {
                self.state
                    .units
                    .row(n.unit)
                    .map(|r| self.state.units.pos[r])
            })
            .collect();
        if let Some(at) = engine {
            self.fog.reveal(at, Fx::from_int(420), Fx::ZERO, mask);
        }
        // After the last round has gone, what is left of it is shown, so the
        // defenders can hunt the stragglers down and finish.
        if s.phase == Phase::Final {
            let left: Vec<FxVec2> = s
                .waves
                .iter()
                .filter_map(|u| self.state.units.row(*u).map(|r| self.state.units.pos[r]))
                .collect();
            for at in left {
                self.fog.reveal(at, Fx::from_int(60), Fx::ZERO, mask);
            }
        }
        for at in nodes {
            self.fog.reveal(at, Fx::from_int(160), Fx::ZERO, mask);
        }
    }

    /// The ray and the print beams, for the renderer. The ray is always
    /// shown; a print beam only where the viewer sees either end.
    pub(crate) fn write_survival_beams(
        &self,
        viewer: Option<u8>,
        out: &mut Vec<BeamInstance>,
        sources: &mut Vec<u32>,
    ) {
        let Some(s) = self.state.survival.as_ref() else {
            return;
        };
        let st = &self.state;
        let seen = |p: FxVec2| {
            viewer.is_none_or(|v| !st.fog_enabled || self.fog.is_detected(p, self.team_mask(v)))
        };
        let now = st.tick;
        if let Some(engine) = st.units.row(s.engine) {
            let crown = st.units.pos[engine].extend(st.units.z[engine] + Fx::from_int(CROWN));
            for n in s.nodes.iter().filter(|n| !n.raised) {
                let Some(row) = st.units.row(n.unit) else {
                    continue;
                };
                let top = st.units.pos[row].extend(st.units.z[row] + self.bp(row).height);
                let t = ((now - n.started) as f32 / RAISE_TICKS as f32).min(1.0);
                sources.push(n.unit.0 | 1 << 30);
                out.push(BeamInstance {
                    from: crown.to_f32(),
                    kind: BEAM_REPLICATION_RAY,
                    to_prev: top.to_f32(),
                    radius: 7.0,
                    to: top.to_f32(),
                    height: t,
                });
            }
        }
        for p in &s.printing {
            let (Some(row), Some(src)) = (st.units.row(p.unit), st.units.row(p.source)) else {
                continue;
            };
            let at = st.units.pos[row];
            if !seen(at) && !seen(st.units.pos[src]) {
                continue;
            }
            let from = if p.bay == NODE_PRINT {
                st.units.pos[src].extend(st.units.z[src] + Fx::from_int(NODE_EMITTER))
            } else if (p.bay as usize) < BAYS {
                let dir =
                    st.units.heading[src] + Angle(((p.bay as u32 * 65536) / BAYS as u32) as u16);
                (st.units.pos[src] + FxVec2::from_angle(dir) * Fx::from_int(BAY_EMITTER.0))
                    .extend(st.units.z[src] + Fx::from_int(BAY_EMITTER.1))
            } else {
                st.units.pos[src].extend(st.units.z[src] + Fx::from_int(CROWN))
            };
            let bp = self.bp(row);
            sources.push(p.unit.0 | 1 << 31);
            out.push(BeamInstance {
                from: from.to_f32(),
                kind: BEAM_PRINT,
                to_prev: st.units.prev_pos[row].extend(st.units.prev_z[row]).to_f32(),
                radius: bp.radius.to_f32(),
                to: at.extend(st.units.z[row]).to_f32(),
                height: bp.height.to_f32(),
            });
        }
    }

    /// Where the replication nodes stand, raised or rising.
    pub fn survival_nodes(&self) -> Vec<FxVec2> {
        let Some(s) = self.state.survival.as_ref() else {
            return Vec::new();
        };
        s.nodes
            .iter()
            .filter_map(|n| {
                self.state
                    .units
                    .row(n.unit)
                    .map(|r| self.state.units.pos[r])
            })
            .collect()
    }

    /// Ids of units a replicator is printing right now (sorted), for the mirror's violet fill.
    pub fn survival_printing(&self) -> Vec<u32> {
        let Some(s) = self.state.survival.as_ref() else {
            return Vec::new();
        };
        let mut ids: Vec<u32> = s
            .printing
            .iter()
            .map(|p| p.unit)
            .chain(s.nodes.iter().filter(|n| !n.raised).map(|n| n.unit))
            .map(|u| u.0)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// What the HUD shows. None outside survival.
    pub fn survival_status(&self) -> Option<SurvivalStatus> {
        let s = self.state.survival.as_ref()?;
        let st = &self.state;
        let rules = s.config.rules;
        let side = s.config.engine_player;
        let round_size: u16 = s.incoming.iter().flatten().sum();
        let left = s.queue.len() as u16
            + s.printing.iter().filter(|p| p.front != NODE_PRINT).count() as u16;
        let nodes = s
            .nodes
            .iter()
            .filter_map(|n| {
                let row = st.units.row(n.unit)?;
                let raised = if n.raised {
                    1.0
                } else {
                    ((st.tick - n.started) as f32 / RAISE_TICKS as f32).min(0.999)
                };
                Some(NodeStatus {
                    site: n.site,
                    pos: st.units.pos[row].to_f32(),
                    product: n.product,
                    raised,
                    printed: n.printed,
                })
            })
            .collect();
        Some(SurvivalStatus {
            round: s.round,
            rounds: rules.rounds,
            phase: s.phase,
            next_in: match s.phase {
                Phase::Grace | Phase::Launched => s.next_at.saturating_sub(st.tick),
                _ => 0,
            },
            tier: rules.tier_at(s.round.max(1)),
            incoming: s.incoming,
            printed: (round_size.saturating_sub(left), round_size),
            remaining: s
                .waves
                .iter()
                .filter(|u| st.units.row(**u).is_some())
                .count() as u32,
            hostile: self.hostile_count(side) as u32,
            engine: s.config.engine.to_f32(),
            nodes,
            nodes_destroyed: s.nodes_destroyed,
            reclaim: st
                .players
                .iter()
                .enumerate()
                .filter(|(p, _)| *p as u8 != side)
                .map(|(_, pl)| pl.reclaim_income.to_f32())
                .sum(),
        })
    }
}

fn domain_of_bp(w: &crate::World, bp: BlueprintId) -> Domain {
    w.blueprints
        .unit(bp)
        .motion
        .map_or(Domain::Land, |m| domain_of(m.layer))
}

/// Units per domain and tech.
fn tally(bps: &mc_data::Blueprints, plan: &[Pending]) -> [[u16; 5]; 3] {
    let mut out = [[0u16; 5]; 3];
    for p in plan {
        let u = bps.unit(p.blueprint);
        let d = u.motion.map_or(Domain::Land, |m| domain_of(m.layer));
        let t = (u.tech.clamp(1, 5) - 1) as usize;
        out[domain_index(d)][t] += 1;
    }
    out
}

/// Ticks to print a unit of `tech`.
fn print_ticks(tech: u8) -> u32 {
    (3 + 2 * tech as u32) * HZ
}
