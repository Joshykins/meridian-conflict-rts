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

mod base;
mod card;
mod fleet;
mod foot;
mod marks;
mod plan;
mod print;
mod rules;
mod shapers;
mod view;
mod warheads;
mod warp_in;

pub use crate::mirror::NodeStatus;
use crate::mirror::SimEvent;
use crate::tables::NO_ORDER;
use crate::{SimError, UnitId};
pub use base::Kit;
pub use card::{
    AntiNukeCard, ArtilleryCard, FleetCard, LanceCard, PieceView, ShaperCard, ShaperInstance,
    ShieldCard, WarheadCard,
};
pub use fleet::FleetStage;
pub use marks::{WarpInInstance, WarpInKind};
use mc_core::{Angle, Fx, FxVec2, FxVec3, Rng, StateHasher};
use mc_data::survival::{Domain, SiteKind};
use mc_data::{BlueprintId, MoveLayer};
use plan::Formation;
use print::Route;
pub use rules::{
    arrival_shock, aux_first_round, fleet_classes, fleet_gap_ticks, fleet_size, launch_gap_ticks,
    shaper_foot, shaper_height, shaper_platforms, shaper_pylons, shaper_silos, shaper_tier,
    squad_size, squads_per_volley, volley_ticks, warp_in_ticks, HeavyWave, SurvivalRules,
    FLEET_LEAD_TICKS,
};
use serde::{Deserialize, Serialize};
pub use warp_in::WARP_REACH;

/// Ticks per second.
const HZ: u32 = 10;
/// `BeamInstance::kind` of the engine's ray raising a node.
pub const BEAM_REPLICATION_RAY: u32 = 4;
/// `BeamInstance::kind` of a print beam building a unit up.
pub const BEAM_PRINT: u32 = 5;
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

/// Where a unit is printed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Spot {
    /// A print bay of the facility (index into `SurvivalConfig::bays`).
    Bay(u8),
    /// A Shaper's print platform (the platform's unit).
    Pad(UnitId),
    /// Where a Seedcaster's squad warps in (`warp_in.rs`): it prints round `at`, no beam.
    WarpIn { at: FxVec2, heading: Angle },
    /// A Gatewright's berth in front of it (`fleet.rs`): its fleet prints in the air
    /// round `at`.
    Berth { at: FxVec2, heading: Angle },
}

/// Why an auxiliary Shaper's last volley or fleet was held back. The numbers may reach
/// the interface: never renumber one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hold {
    #[default]
    None = 0,
    /// The field cap (`FIELD_CAP`): it waits for room.
    FieldFull = 1,
    /// Every target lies in the defenders' warp dampening fields (`warp_in.rs`).
    Dampened = 2,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Print {
    unit: UnitId,
    spot: Spot,
    /// The Shaper whose beam prints it; `Handle::NONE` on a bay.
    source: UnitId,
    /// The way it goes once printed.
    route: Route,
    /// Printed for the round: it musters and goes with it. Else a Shaper's own,
    /// sent as soon as it is done.
    round: bool,
    /// The formation it is printed with: they finish and go together.
    formation: u16,
    started: u32,
    ticks: u32,
}

/// A printed unit waiting for the round to go, near the head of its route.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Muster {
    unit: UnitId,
    route: Route,
    formation: u16,
}

/// A piece of a Shaper's base (`base.rs`). The numbers are the slot order of a
/// Shaper's base, and may reach the interface: never renumber one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PieceKind {
    /// A print platform in front (a slip at a naval site).
    Platform = 0,
    /// A great print platform, further out, for the big units.
    GreatPlatform = 1,
    /// A pylon at the sides or behind that feeds the Shaper's shield.
    Pylon = 2,
    /// An Unmaker's silo behind it, where its warheads are assembled and launched.
    Silo = 3,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Piece {
    unit: UnitId,
    kind: PieceKind,
    /// Its place in its kind's arc (`base::slot_at`).
    slot: u8,
    /// Tick its printing started.
    started: u32,
    raised: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Node {
    unit: UnitId,
    site: u8,
    /// What it is: a printer, or the auxiliary kind its site raised
    /// (`shapers::raise_kind`; never `Auxiliary`).
    kind: SiteKind,
    /// Where it stands: on its site, one Shaper to a site.
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
    /// Metres it is built up to now, climbing toward its level's height
    /// (`rules::shaper_height`).
    height: Fx,
    /// Tick of its next catch-up level: while it stands below the round's
    /// `SurvivalRules::catch_up_level` it gains one every `CATCH_UP_TICKS`.
    next_level_at: u32,
    /// Its base: platforms and pylons, raised or being printed (`base.rs`).
    pieces: Vec<Piece>,
    /// Tick it may start its next piece.
    next_piece_at: u32,
    /// An auxiliary Shaper's next volley (a Seedcaster), launch (an Unmaker) or fleet
    /// (a Gatewright).
    next_fire: u32,
    /// Why its last volley or fleet was held back, if it was.
    hold: Hold,
    /// Metres out its foot is blocked now (`foot.rs`), growing with its height.
    foot: Fx,
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
    /// Per site: the first round its ray may raise a Shaper there again; a
    /// site whose Shaper was destroyed stands empty for a while.
    reseed_from: Vec<u16>,
    /// What a Shaper's base is made of.
    kit: base::Kit,
    /// Squads marked to warp in (`warp_in.rs`), and the auxiliary Shapers' dice.
    warp_ins: warp_in::WarpIns,
    /// The Gatewrights' fleets (`fleet.rs`).
    fleets: Vec<fleet::Fleet>,
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
    /// A Seedcaster's volley is held: every target lies in the defenders' warp
    /// dampening fields.
    pub dampened: bool,
    /// Units of the rounds sent that are still alive (the final phase ends at zero).
    pub remaining: u32,
    pub engine: [f32; 2],
    /// How awake the facility is, 0..1: it climbs with the rounds and the
    /// Shapers online. The renderer brightens the Precursor light with it.
    pub activity: f32,
    pub nodes: Vec<NodeStatus>,
    /// Sites whose Shaper was destroyed, standing empty: (site, the first round
    /// the ray may reseed it in).
    pub reseeding: Vec<(u8, u16)>,
    /// The level a Shaper raised now evolves to (`SurvivalRules::catch_up_level`).
    pub catch_up: u16,
    pub nodes_destroyed: u16,
    /// Shapers the facility starts raising with the next round (or this one,
    /// while it is being printed).
    pub shapers_next: u8,
    /// Materials per second the defender takes in from wrecks now.
    pub reclaim: f32,
    /// The facility's side: its warheads' marks are shown to the defenders.
    pub engine_player: u8,
}

fn spot_bits(s: Spot) -> u64 {
    match s {
        Spot::Bay(b) => b as u64,
        Spot::Pad(u) => 1 << 40 | u.0 as u64,
        Spot::WarpIn { at, heading } | Spot::Berth { at, heading } => {
            (2 + matches!(s, Spot::Berth { .. }) as u64) << 40
                ^ (at.x.raw() as u32 as u64) << 8
                ^ (at.y.raw() as u32 as u64).rotate_left(29)
                ^ heading.0 as u64
        }
    }
}

fn route_bits(r: Route) -> u64 {
    match r {
        Route::Front(f) => f as u64,
        Route::Site(i) => 1 << 40 | i as u64,
        Route::Free => 2 << 40,
        Route::Held => 3 << 40,
    }
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
            h.write_u64(p.started as u64 | (p.formation as u64) << 32 | (p.round as u64) << 48);
            h.write_u64(spot_bits(p.spot) ^ route_bits(p.route).rotate_left(17));
        }
        for m in &self.mustered {
            h.write_u64(m.unit.0 as u64 | (m.formation as u64) << 32);
            h.write_u64(route_bits(m.route));
        }
        h.write_u64(self.waves.len() as u64);
        for r in &self.reseed_from {
            h.write_u64(*r as u64);
        }
        for n in &self.nodes {
            h.write_u64(
                n.unit.0 as u64
                    | (n.site as u64) << 32
                    | (n.raised as u64) << 40
                    | (n.level as u64) << 48
                    | (n.kind as u64) << 41,
            );
            h.write_u64(n.pos.x.raw() as u32 as u64 | (n.pos.y.raw() as u32 as u64) << 32);
            h.write_u64(n.next_print as u64 | (n.printed as u64) << 32);
            h.write_i64(n.height.0);
            h.write_u64(n.next_level_at as u64);
            h.write_u64(n.next_piece_at as u64 | (n.pieces.len() as u64) << 32);
            h.write_u64(n.next_fire as u64 | (n.hold as u64) << 32);
            h.write_i64(n.foot.0);
            for p in &n.pieces {
                h.write_u64(
                    p.unit.0 as u64
                        | (p.kind as u64) << 32
                        | (p.slot as u64) << 40
                        | (p.raised as u64) << 48,
                );
                h.write_u64(p.started as u64);
            }
            for b in &n.products {
                h.write_u64(b.0 as u64);
            }
        }
        self.warp_ins.hash(h);
        h.write_u64(self.fleets.len() as u64);
        for f in &self.fleets {
            f.hash(h);
        }
    }

    /// Every blueprint id the survival state holds, for `State::validate_ids`.
    pub(crate) fn blueprint_ids(&self) -> Vec<BlueprintId> {
        let mut ids: Vec<BlueprintId> = self.kit.blueprints().collect();
        ids.extend(self.queue.iter().flat_map(|f| f.units.iter().copied()));
        ids.extend(self.nodes.iter().flat_map(|n| n.products.iter().copied()));
        ids.extend(self.incoming_heavy.iter().map(|h| h.0));
        ids.extend(self.warp_ins.blueprints());
        ids
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
        // Every Shaper and piece of a base it raises, before anything is changed.
        let kit = Kit::load(self.blueprints.as_ref())?;
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
        let sites = config.node_sites.len();
        self.state.survival = Some(Survival {
            config,
            round: 0,
            phase: Phase::Grace,
            next_at: self.state.tick + first.max(1),
            queue: Vec::new(),
            printing: Vec::new(),
            mustered: Vec::new(),
            waves: Vec::new(),
            reseed_from: vec![0; sites],
            kit,
            warp_ins: warp_in::WarpIns::new(seed ^ 0xA87_111E),
            fleets: Vec::new(),
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
        // Held out, or the defenders are gone: the engine falls quiet (but in a debug run
        // that plays on, `SurvivalRules::play_on`).
        if s.phase == Phase::Won || (self.state.winner.is_some() && !s.config.rules.play_on) {
            return Ok(());
        }
        // Playing on with the defenders gone, it prints no more rounds: they still come
        // and its Shapers grow with them, but the field never fills to its cap and stalls.
        if self.state.winner.is_some() {
            if let Some(s) = self.state.survival.as_mut() {
                s.queue.clear();
            }
        }
        let now = self.state.tick;
        self.survival_nodes_lost()?;
        self.survival_prints(now)?;
        self.survival_raise(now)?;
        self.survival_grow(now);
        self.survival_feet();
        self.survival_base(now)?;
        self.survival_node_prints(now)?;
        self.survival_warp_ins(now)?;
        self.survival_warheads(now)?;
        self.survival_fleets(now)?;

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
                let done = s.queue.is_empty() && !s.printing.iter().any(|p| p.round);
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
                    && !s.fleets.iter().any(|f| f.holds(u.id(r)))
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
