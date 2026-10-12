//! Survival: one side holds out against round after round from a Precursor
//! facility, a foundry built into the map that nothing can harm.
//!
//! The facility does not conjure units. Every one is printed, a whole
//! formation at a time: the formation stands in its block on one of the print
//! bays built into the halls (or, for ships, a slip in the sea gate) as
//! construction sites while the bay's projector builds them all up together,
//! then marches off to muster at the head of its front. When the round's last
//! formation is out, the whole round is sent down its fronts, each formation
//! as one block, and on to wherever the defenders started. The bays are the
//! map's (`SurvivalConfig::bays`).
//!
//! A round is formations of the tier it prints at (`plan`), most of them one
//! faction's, and from the first T4 round a heavy wave: squads of
//! experimentals, each one kind of unit, worth a growing share of the round.
//! The schedule is `rules`; nothing in it has a ceiling or reacts to the
//! defenders.
//!
//! As the rounds climb the facility wakes: at the start of a round it fires
//! its ray from the heart at one or more cradles and raises Shapers
//! (replication nodes, `shapers`) in them, more each round. A Shaper is of one
//! domain and prints formations of its products straight at the defenders,
//! and grows the longer it stands. Shapers can be destroyed (while being
//! raised too), and a destroyed one leaves a rich wreck: they are the bonus
//! objectives.
//!
//! Nothing is handed out. The defenders' materials are what the engine sends them:
//! every unit it prints dies into a wreck worth most of its cost, so a
//! survival map carries little ore and the field is the economy (reclaim).
//!
//! All of it is state, driven from the match description (`SurvivalConfig`,
//! carried beside `MatchConfig` in the match options) and the tick: a replay
//! plays it out the same.

mod plan;
mod print;
mod rules;
mod shapers;
mod view;

pub use crate::mirror::NodeStatus;
use crate::mirror::SimEvent;
use crate::tables::NO_ORDER;
use crate::{SimError, UnitId};
use mc_core::{Angle, Fx, FxVec2, FxVec3, Rng, StateHasher};
use mc_data::survival::{Domain, SiteKind};
use mc_data::{BlueprintId, MoveLayer};
use plan::Formation;
pub use rules::{HeavyWave, SurvivalRules};
use serde::{Deserialize, Serialize};

/// Ticks per second.
const HZ: u32 = 10;
/// `BeamInstance::kind` of the engine's ray raising a node.
pub const BEAM_REPLICATION_RAY: u32 = 4;
/// `BeamInstance::kind` of a print beam building a unit up.
pub const BEAM_PRINT: u32 = 5;
/// A node prints in front of itself, this far out.
pub const NODE_PRINT_REACH: i32 = 50;
/// The node's print emitter height.
pub const NODE_EMITTER: i32 = 40;
/// Shapers one node site holds, side by side across the way they face.
pub const SHAPERS_PER_SITE: usize = 3;
/// Between neighbouring Shapers of one site, centre to centre (a lot is 60 m).
const SHAPER_SPACING: i32 = 72;
/// How long the ray takes to raise a node.
const RAISE_TICKS: u32 = 20 * HZ;
/// The engine stops printing while it has this many units in the field, so a
/// late endless round cannot outgrow the simulation. The HUD says when it holds.
const FIELD_CAP: usize = 900;
/// Idle hostile units are sent at the defenders this often.
const HUNT_EVERY: u32 = 5 * HZ;

/// One way the engine's forces come, from its side to the defenders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Front {
    pub domain: Domain,
    pub path: Vec<FxVec2>,
}

/// A Shaper site: where the ray raises a Shaper, which grows there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeSite {
    pub at: FxVec2,
    pub domain: Domain,
    /// The way its base faces; None: toward the first point of `path`, or the
    /// defenders when it has none.
    pub facing: Option<Angle>,
    pub kind: SiteKind,
    /// Radius of level, clear ground its base grows into.
    pub room: Fx,
    /// Route from the site toward the defenders; the last leg to them is the
    /// sim's. Aircraft fly straight.
    pub path: Vec<FxVec2>,
}

/// A print bay built into the facility.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bay {
    /// Where the printed unit stands.
    pub at: FxVec2,
    pub heading: Angle,
    /// The projector head: x, y, and metres over the ground (or the water) at `at`.
    pub emitter: FxVec3,
    /// Land bays print land units (and aircraft when no aerie is free), air bays
    /// aircraft, naval ones ships.
    pub domain: Domain,
    /// The biggest unit (collision radius) it takes; zero for any size.
    pub max_radius: Fx,
}

impl Bay {
    /// Room for a unit of `radius`, as a sort key: the smaller, the better the fit.
    fn room(&self, radius: Fx) -> Option<Fx> {
        if self.max_radius == Fx::ZERO {
            Some(Fx::from_int(1 << 20))
        } else if radius <= self.max_radius {
            Some(self.max_radius)
        } else {
            None
        }
    }

    /// A great bay: its projector works with two more beside it.
    fn great(&self) -> bool {
        self.max_radius == Fx::ZERO || self.max_radius > Fx::from_int(100)
    }
}

/// A gun the facility's side starts with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guard {
    pub key: String,
    pub at: FxVec2,
    pub heading: Angle,
}

/// The survival half of a match description. Built from the map's layout
/// and the set-up screen's rules; travels with the match options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurvivalConfig {
    /// The facility's side. Every other player defends.
    pub engine_player: u8,
    /// The facility's heart, where the ray leaves: x, y, and metres over the ground.
    pub engine: FxVec2,
    pub ray_height: Fx,
    pub bays: Vec<Bay>,
    pub guards: Vec<Guard>,
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
struct Print {
    unit: UnitId,
    /// Index into `SurvivalConfig::bays`, or `NODE_PRINT`.
    bay: u8,
    /// The node doing the printing; `Handle::NONE` for a bay.
    source: UnitId,
    /// Front it goes down; `NODE_PRINT` for a node's own.
    front: u8,
    /// The formation it is printed with: they finish and go together.
    formation: u16,
    started: u32,
    ticks: u32,
}

const NODE_PRINT: u8 = u8::MAX;

/// A printed unit waiting at the head of its front for the round to go.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Muster {
    unit: UnitId,
    front: u8,
    formation: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Node {
    unit: UnitId,
    site: u8,
    /// Where it stands; one site holds up to `SHAPERS_PER_SITE`.
    pos: FxVec2,
    /// What it prints for: its products are all of this domain.
    domain: Domain,
    /// What it prints, the first kind first; more as it levels.
    products: Vec<BlueprintId>,
    level: u16,
    /// The round it came online in (the round its ray started, until then).
    born: u16,
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
    /// Rounds begun so far; the current one while printing.
    pub round: u16,
    pub phase: Phase,
    /// Tick the next round starts printing.
    pub next_at: u32,
    queue: Vec<Formation>,
    printing: Vec<Print>,
    /// Printed and waiting at the head of their front for the round to go.
    mustered: Vec<Muster>,
    /// Every unit sent in a round and not yet dead.
    waves: Vec<UnitId>,
    nodes: Vec<Node>,
    rng: Rng,
    pub nodes_destroyed: u16,
    /// The last formation number handed out.
    next_formation: u16,
    /// What the round being printed is made of, per domain, then per tech.
    incoming: [[u16; 5]; 3],
    /// The heavy wave of the round being printed (or the next): each squad's kind and size.
    incoming_heavy: Vec<(BlueprintId, u16)>,
}

/// What the HUD shows of survival; published with the sim status.
#[derive(Clone, Debug, Default)]
#[expect(
    clippy::disallowed_types,
    reason = "presentation: the survival HUD reads it"
)]
pub struct SurvivalStatus {
    pub round: u16,
    /// Zero: endless.
    pub rounds: u16,
    pub phase: Phase,
    /// Ticks until the next round starts printing (0 while one is).
    pub next_in: u32,
    /// Tech the engine prints at now.
    pub tier: u8,
    /// The next tier and the round it starts in; None at the top.
    pub next_tier: Option<(u8, u16)>,
    /// The round being printed (or the next one's forecast between rounds):
    /// units per domain (`Domain::ALL` order), per tech 1..=5.
    pub incoming: [[u16; 5]; 3],
    /// Its heavy wave: each squad's kind and size.
    pub heavy: Vec<(BlueprintId, u16)>,
    /// Units printed so far of the round being printed, and its size.
    pub printed: (u16, u16),
    /// Hostile units in the field.
    pub hostile: u32,
    /// The field is full (`FIELD_CAP`): printing waits for room.
    pub held: bool,
    /// Units of the rounds sent that are still alive (the final phase ends at zero).
    pub remaining: u32,
    pub engine: [f32; 2],
    /// How awake the facility is, 0..1: it climbs with the rounds and the
    /// Shapers online. The renderer brightens the Precursor light with it.
    pub activity: f32,
    pub nodes: Vec<NodeStatus>,
    pub nodes_destroyed: u16,
    /// Shapers the facility starts raising with the next round (or this one,
    /// while it is being printed).
    pub shapers_next: u8,
    /// Materials per second the defender takes in from wrecks now.
    pub reclaim: f32,
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
        h.write_u64((self.round as u64) << 32 | (self.phase as u64) << 48);

        h.write_u64(self.next_at as u64 | (self.nodes_destroyed as u64) << 32);
        h.write_u64(self.rng.state() ^ self.next_formation as u64);
        h.write_u64(self.queue.len() as u64);
        for f in &self.queue {
            h.write_u64(f.units.len() as u64 | (f.front as u64) << 32 | (f.heavy as u64) << 40);
            for b in &f.units {
                h.write_u64(b.0 as u64);
            }
        }
        for p in &self.printing {
            h.write_u64(p.unit.0 as u64 | (p.source.0 as u64) << 32);
            h.write_u64(
                p.started as u64
                    | (p.bay as u64) << 32
                    | (p.front as u64) << 40
                    | (p.formation as u64) << 48,
            );
        }
        for m in &self.mustered {
            h.write_u64(m.unit.0 as u64 | (m.front as u64) << 32 | (m.formation as u64) << 40);
        }
        h.write_u64(self.waves.len() as u64);
        for n in &self.nodes {
            h.write_u64(
                n.unit.0 as u64
                    | (n.site as u64) << 32
                    | (n.raised as u64) << 40
                    | (n.level as u64) << 48,
            );
            h.write_u64(n.pos.x.raw() as u32 as u64 | (n.pos.y.raw() as u32 as u64) << 32);
            h.write_u64(n.next_print as u64 | (n.printed as u64) << 32);
            for b in &n.products {
                h.write_u64(b.0 as u64);
            }
        }
    }
}

impl crate::World {
    /// Turns this match into survival. Call once, right after `World::new`,
    /// before the first tick: removes the facility side's commander, raises
    /// its guns, and starts the clock. The facility itself is the map's.
    pub fn begin_survival(&mut self, config: SurvivalConfig) -> Result<(), SimError> {
        let side = config.engine_player;
        if side as usize >= self.state.players.len() {
            return Err(SimError::Setup("survival: no such engine side".into()));
        }
        if self.blueprints.id_of("replication_node").is_none() {
            return Err(SimError::Setup(
                "survival needs blueprint replication_node".into(),
            ));
        }
        if !config.bays.iter().any(|b| b.domain == Domain::Land) {
            return Err(SimError::Setup(
                "survival: the facility has no land print bays".into(),
            ));
        }
        if let Some(row) = self
            .state
            .units
            .row(self.state.players[side as usize].commander)
        {
            self.remove_unit_row(row, false)?;
        }
        self.state.players[side as usize].commander = crate::Handle::NONE;
        self.state.players[side as usize].free_build = true;

        // Its guns, where the map puts them.
        for g in &config.guards {
            let Some(bp) = self.blueprints.id_of(&g.key) else {
                continue;
            };
            let ubp = self.blueprints.unit(bp);
            let pos = crate::world::snap_to_build_grid(ubp, g.at);
            if self.can_place(ubp, pos) {
                self.spawn_unit(bp, side, pos, g.heading, true)?;
            }
        }

        let first = config.rules.grace_secs as u32 * HZ;
        let seed = self.state.rng.state() ^ 0x5EED_5A1A;
        self.state.survival = Some(Survival {
            config,
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
            next_formation: 0,
            incoming: [[0; 5]; 3],
            incoming_heavy: Vec::new(),
        });
        self.survival_forecast(1);
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
                let mut rng = s.rng;
                let plan = self.plan_round(s, round, &mut rng);
                let incoming = print::tally(self.blueprints.as_ref(), &plan);
                let heavy = print::heavy_squads(&plan);
                let s = self.state.survival.as_mut().unwrap();
                s.rng = rng;
                s.round = round;
                s.phase = Phase::Printing;
                s.queue = plan;
                s.incoming = incoming;
                s.incoming_heavy = heavy.clone();
                self.events.push(SimEvent::RoundPrinting { round });
                let tier = rules.tier_at(round);
                if round > 1 && tier > rules.tier_at(round - 1) {
                    self.events.push(SimEvent::TierReached { round, tier });
                }
                if !heavy.is_empty() {
                    self.events.push(SimEvent::HeavyWave {
                        round,
                        squads: heavy,
                    });
                }
                self.survival_level_shapers(round);
                for _ in 0..rules.nodes_raised(round) {
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
        if now.is_multiple_of(HUNT_EVERY) {
            self.survival_hunt()?;
        }
        self.survival_check_won();
        Ok(())
    }

    /// What round `round` will bring, for the HUD between rounds: planned
    /// with dice of its own, so the forecast changes nothing.
    fn survival_forecast(&mut self, round: u16) {
        let Some(s) = self.state.survival.as_ref() else {
            return;
        };
        let mut rng = Rng::new(s.rng.state() ^ round as u64);
        let plan = self.plan_round(s, round, &mut rng);
        let incoming = print::tally(self.blueprints.as_ref(), &plan);
        let heavy = print::heavy_squads(&plan);
        let s = self.state.survival.as_mut().unwrap();
        s.incoming = incoming;
        s.incoming_heavy = heavy;
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
                    && !s.mustered.iter().any(|m| m.unit == u.id(r))
            })
            .map(|r| u.id(r))
            .collect();
        if idle.is_empty() {
            return Ok(());
        }
        self.survival_send(&idle, print::Route::Free, false)
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
        let mut mask: mc_core::PlayerMask = 0;
        for p in 0..self.state.players.len() {
            if p as u8 != side {
                mask |= self.team_mask(p as u8);
            }
        }
        let heart = s.config.engine;
        let bays: Vec<FxVec2> = s.config.bays.iter().map(|b| b.at).collect();
        // A standing Shaper shows its whole site, the ground its base grows into.
        let nodes: Vec<(FxVec2, Fx)> = s
            .nodes
            .iter()
            .filter_map(|n| {
                let room = s.config.node_sites.get(n.site as usize)?.room;
                self.state
                    .units
                    .row(n.unit)
                    .map(|r| (self.state.units.pos[r], room.max(Fx::from_int(160))))
            })
            .collect();
        // The facility hides nothing: its heart and its print bays are always seen.
        self.fog.reveal(heart, Fx::from_int(600), Fx::ZERO, mask);
        for at in bays {
            self.fog.reveal(at, Fx::from_int(160), Fx::ZERO, mask);
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
        for (at, room) in nodes {
            self.fog.reveal(at, room, Fx::ZERO, mask);
        }
    }
}
