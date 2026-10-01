//! What the Commander keeps between thinks: its plans, operations, the facts it
//! never forgets, and its memory of where it has looked. In `AiState`, hashed and
//! saved with the match.
use crate::tables::UnitId;
use mc_core::{Fx, FxVec2, StateHasher};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

/// Facts that stay true once learned: the highest tier seen, and when each kind of
/// force was last seen (0: never).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct Sticky {
    pub max_tech: u8,
    pub nukes_seen: u32,
    pub air_seen: u32,
    pub navy_seen: u32,
    pub subs_seen: u32,
    pub space_seen: u32,
}

/// A way to win, or not to lose (`plans.rs`). The number is fixed: it is saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::ai) enum PlanKind {
    /// The land army in waves.
    Pressure = 0,
    /// Fast groups on soft targets: engineers, outlying mines, power.
    Raid = 1,
    /// Lift ships carry a land force over and put it down by a weak spot.
    Landing = 2,
    /// Bombers and gunships at the economy, fighters to cover them.
    AirPower = 3,
    /// A surface fleet: hold the water, shell the coast.
    SeaControl = 4,
    /// Submarines where their sonar is thin.
    SubWar = 5,
    /// Armed spaceships, by warp.
    Warships = 6,
    /// Artillery and map guns from beyond their defences.
    Siege = 7,
    /// Nukes.
    Strategic = 8,
    /// One unit worth many minutes of income.
    Titan = 9,
    /// Defences, shields and interceptors at home.
    Fortify = 10,
    /// Economy first.
    Boom = 11,
    /// Eyes: scouts and sensor ships where the beliefs are least sure.
    Intel = 12,
    /// Anti-air at home and with the army.
    AirDefense = 13,
}

pub(in crate::ai) const PLANS: [PlanKind; 14] = [
    PlanKind::Pressure,
    PlanKind::Raid,
    PlanKind::Landing,
    PlanKind::AirPower,
    PlanKind::SeaControl,
    PlanKind::SubWar,
    PlanKind::Warships,
    PlanKind::Siege,
    PlanKind::Strategic,
    PlanKind::Titan,
    PlanKind::Fortify,
    PlanKind::Boom,
    PlanKind::Intel,
    PlanKind::AirDefense,
];

impl PlanKind {
    pub(in crate::ai) fn name(self) -> &'static str {
        match self {
            PlanKind::Pressure => "pressure",
            PlanKind::Raid => "raid",
            PlanKind::Landing => "landing",
            PlanKind::AirPower => "air",
            PlanKind::SeaControl => "sea",
            PlanKind::SubWar => "subs",
            PlanKind::Warships => "warships",
            PlanKind::Siege => "siege",
            PlanKind::Strategic => "nukes",
            PlanKind::Titan => "titan",
            PlanKind::Fortify => "fortify",
            PlanKind::Boom => "boom",
            PlanKind::Intel => "intel",
            PlanKind::AirDefense => "anti-air",
        }
    }
}

/// How much a plan is given (`docs/AI_COMMANDER.md`, "Plans").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::ai) enum Stake {
    #[default]
    Off = 0,
    /// Cheap, and tells the side something.
    Probe = 1,
    /// A real share of income.
    Invest = 2,
    /// The plan is the game.
    AllIn = 3,
}

impl Stake {
    pub(in crate::ai) fn up(self) -> Stake {
        match self {
            Stake::Off => Stake::Probe,
            Stake::Probe => Stake::Invest,
            _ => Stake::AllIn,
        }
    }
    pub(in crate::ai) fn down(self) -> Stake {
        match self {
            Stake::AllIn => Stake::Invest,
            Stake::Invest => Stake::Probe,
            _ => Stake::Off,
        }
    }
    pub(in crate::ai) fn name(self) -> &'static str {
        match self {
            Stake::Off => "off",
            Stake::Probe => "probe",
            Stake::Invest => "invest",
            Stake::AllIn => "all-in",
        }
    }
}

/// Mass an operation or plan has put in, destroyed and lost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct Ledger {
    pub committed: Fx,
    pub killed: Fx,
    pub lost: Fx,
}

impl Ledger {
    pub(in crate::ai) fn add(&mut self, o: &Ledger) {
        self.committed += o.committed;
        self.killed += o.killed;
        self.lost += o.lost;
    }
    /// Mass destroyed per mass lost; 1 when nothing has happened yet.
    pub(in crate::ai) fn trade(&self) -> Fx {
        if self.killed + self.lost < Fx::from_int(200) {
            Fx::ONE
        } else {
            self.killed / self.lost.max(Fx::from_int(100))
        }
    }
    fn hash(&self, h: &mut StateHasher) {
        h.write_i64(self.committed.0);
        h.write_i64(self.killed.0);
        h.write_i64(self.lost.0);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct Plan {
    pub kind: PlanKind,
    pub stake: Stake,
    /// Tick it was taken up, and tick its stake last changed.
    pub since: u32,
    pub changed: u32,
    /// Its operations' trade since its stake last changed.
    pub ledger: Ledger,
    /// Reviews in a row it traded badly.
    pub strikes: u8,
}

/// What an operation does (`ops.rs`). The number is fixed: it is saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::ai) enum OpKind {
    Army = 0,
    Defend = 1,
    Raid = 2,
    Landing = 3,
    Strike = 4,
    AirGuard = 5,
    Fleet = 6,
    Wolfpack = 7,
    Warships = 8,
    Siege = 9,
    Scout = 10,
    /// Fast units held among the side's mines that answer raids anywhere.
    Guard = 11,
}

pub(in crate::ai) const OPS: [OpKind; 12] = [
    OpKind::Army,
    OpKind::Defend,
    OpKind::Raid,
    OpKind::Landing,
    OpKind::Strike,
    OpKind::AirGuard,
    OpKind::Fleet,
    OpKind::Wolfpack,
    OpKind::Warships,
    OpKind::Siege,
    OpKind::Scout,
    OpKind::Guard,
];

impl OpKind {
    pub(in crate::ai) fn name(self) -> &'static str {
        match self {
            OpKind::Army => "army",
            OpKind::Defend => "defend",
            OpKind::Raid => "raid",
            OpKind::Landing => "landing",
            OpKind::Strike => "strike",
            OpKind::AirGuard => "air guard",
            OpKind::Fleet => "fleet",
            OpKind::Wolfpack => "wolfpack",
            OpKind::Warships => "warships",
            OpKind::Siege => "siege",
            OpKind::Scout => "scout",
            OpKind::Guard => "guard",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::ai) enum Phase {
    /// Taking units in at its rally point.
    #[default]
    Gathering = 0,
    /// Going for its target.
    Executing = 1,
    /// The whole group falling back: it was losing.
    Withdrawing = 2,
    /// Finished: dropped next think, its units freed.
    Done = 3,
}

impl Phase {
    pub(in crate::ai) fn name(self) -> &'static str {
        match self {
            Phase::Gathering => "gathering",
            Phase::Executing => "going",
            Phase::Withdrawing => "falling back",
            Phase::Done => "done",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct Operation {
    pub id: u32,
    pub kind: OpKind,
    pub plan: PlanKind,
    pub phase: Phase,
    /// Its units, with what each cost (for the ledger when one dies).
    pub units: Vec<(UnitId, Fx)>,
    pub target: FxVec2,
    pub rally: FxVec2,
    pub since: u32,
    pub phase_since: u32,
    /// Mass it waits for before it goes.
    pub want: Fx,
    pub ledger: Ledger,
    /// A landing's lift ship.
    pub carrier: UnitId,
    /// Tick of its last order.
    pub ordered: u32,
    /// Mass it set out with: it falls back once it has lost a third of it.
    #[serde(default)]
    pub launched: Fx,
    /// Its units' health at the last think: losing it while gathering means it
    /// is under fire from something it may not see.
    #[serde(default)]
    pub health: Fx,
}

impl Operation {
    pub(in crate::ai) fn has(&self, id: UnitId) -> bool {
        self.units.iter().any(|(u, _)| *u == id)
    }
    pub(in crate::ai) fn mass(&self) -> Fx {
        self.units.iter().map(|(_, c)| *c).sum()
    }
}

/// What killed the side's units, by the killer's kind (`CommanderState::hurt`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(in crate::ai) enum Hurt {
    /// Ground units in the line.
    Land = 0,
    /// Artillery and map guns: guns that out-range what they hit.
    Artillery = 1,
    Air = 2,
    /// Warships of the upper air.
    Space = 3,
    /// Ships and submarines.
    Sea = 4,
    /// Turrets and other static guns.
    Static = 5,
}

pub(in crate::ai) const HURTS: usize = 6;

impl Hurt {
    pub(in crate::ai) fn of(bp: &mc_data::UnitBlueprint) -> Hurt {
        use mc_data::{cat, MoveLayer};
        if bp.has(cat::ARTILLERY) {
            return Hurt::Artillery;
        }
        match bp.motion.map(|m| m.layer) {
            None => Hurt::Static,
            Some(MoveLayer::Air) if bp.has(cat::SPACE) || bp.is_capital_ship() => Hurt::Space,
            Some(MoveLayer::Air) => Hurt::Air,
            Some(MoveLayer::Naval) => Hurt::Sea,
            Some(_) => Hurt::Land,
        }
    }
}

/// One decision, kept for the overlay and reports: what changed and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct Note {
    pub tick: u32,
    pub plan: PlanKind,
    pub stake: Stake,
    pub why: Why,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::ai) enum Why {
    /// Its appeal rose past what it held.
    Appeal = 0,
    /// Its operations traded well: pushed harder.
    Working = 1,
    /// Traded badly twice running: cut back.
    Failing = 2,
    /// The enemy answered it, or it can no longer be built.
    Answered = 3,
    /// Hedged against a belief.
    Hedge = 4,
    /// Held long enough with nothing to show: something else gets a turn.
    Stale = 5,
}

impl Why {
    pub(in crate::ai) fn name(self) -> &'static str {
        match self {
            Why::Appeal => "looks good",
            Why::Working => "working",
            Why::Failing => "losing trades",
            Why::Answered => "answered",
            Why::Hedge => "hedge",
            Why::Stale => "stale",
        }
    }
}

/// Notes kept for the overlay.
pub(in crate::ai) const NOTES: usize = 12;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct CommanderState {
    /// Tick each world-model cell was last seen.
    pub seen: Vec<u32>,
    pub sticky: Sticky,
    pub plans: Vec<Plan>,
    pub ops: Vec<Operation>,
    pub next_op: u32,
    /// Kills its units made since the last think: who, and the mass destroyed.
    pub kills: Vec<(UnitId, Fx)>,
    /// The project it is saving toward, and the tick it booked it.
    pub project: Option<(BlueprintId, u32)>,
    pub next_review: u32,
    /// Orders it may still give this minute (`attention.rs`).
    pub attention: Fx,
    pub notes: Vec<Note>,
    /// Nuke salvos launched.
    pub salvos: u32,
    /// Ground walkable from home, a bit per `reach::NODE` square (`reach.rs`).
    pub land: Vec<u64>,
    /// Structures the plans want built, best first (`plans.rs`): the builders take
    /// the first one they can place.
    pub wants: Vec<BlueprintId>,
    /// Each plan's appeal at the last review, by `PlanKind as usize`: why it holds
    /// what it holds, for the overlay and reports.
    #[serde(default)]
    pub appeal: [i16; 14],
    /// Every operation's trade over the match, by `OpKind as usize`: which kinds of
    /// operation work for this side.
    #[serde(default)]
    pub trades: [Ledger; 12],
    /// Where and when the side last lost something to aircraft or a warship.
    #[serde(default)]
    pub hit_from_above: Option<(FxVec2, u32)>,
    /// Mass the side has lost lately to each kind of killer (`Hurt`), fading.
    #[serde(default)]
    pub hurt: [Fx; HURTS],
    /// Where the army gathers, held until the front moves (`commander.rs`).
    #[serde(default)]
    pub rally: Option<FxVec2>,
    /// Metres the army's rally point stands back from the front line: it grows while
    /// the gathering wave is shelled and creeps back once it is quiet.
    #[serde(default)]
    pub rally_back: Fx,
}

impl CommanderState {
    pub(in crate::ai) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.seen.len() as u64);
        for s in &self.seen {
            h.write_u64(*s as u64);
        }
        self.sticky.hash(h);
        h.write_u64(self.plans.len() as u64);
        for p in &self.plans {
            h.write_u64(p.kind as u64 | (p.stake as u64) << 8 | (p.strikes as u64) << 16);
            h.write_u64(p.since as u64 | (p.changed as u64) << 32);
            p.ledger.hash(h);
        }
        h.write_u64(self.ops.len() as u64 | (self.next_op as u64) << 32);
        for o in &self.ops {
            h.write_u64(
                o.id as u64
                    | (o.kind as u64) << 32
                    | (o.plan as u64) << 40
                    | (o.phase as u64) << 48,
            );
            for (u, c) in &o.units {
                h.write_u64(u.0 as u64);
                h.write_i64(c.0);
            }
            h.write_i64(o.target.x.0);
            h.write_i64(o.target.y.0);
            h.write_i64(o.rally.x.0);
            h.write_i64(o.rally.y.0);
            h.write_u64(o.since as u64 | (o.phase_since as u64) << 32);
            h.write_i64(o.want.0);
            o.ledger.hash(h);
            h.write_u64(o.carrier.0 as u64 | (o.ordered as u64) << 32);
            h.write_i64(o.launched.0);
            h.write_i64(o.health.0);
        }
        for (u, m) in &self.kills {
            h.write_u64(u.0 as u64);
            h.write_i64(m.0);
        }
        match self.project {
            Some((id, t)) => h.write_u64(id.0 as u64 | (t as u64) << 16),
            None => h.write_u64(u64::MAX),
        }
        h.write_u64(self.next_review as u64 | (self.salvos as u64) << 32);
        h.write_i64(self.attention.0);
        for b in &self.land {
            h.write_u64(*b);
        }
        for w in &self.wants {
            h.write_u64(w.0 as u64);
        }
        h.write_i64(self.rally_back.0);
        match self.rally {
            Some(r) => {
                h.write_i64(r.x.0);
                h.write_i64(r.y.0);
            }
            None => h.write_u64(u64::MAX),
        }
        match self.hit_from_above {
            Some((p, t)) => {
                h.write_i64(p.x.0);
                h.write_i64(p.y.0);
                h.write_u64(t as u64);
            }
            None => h.write_u64(u64::MAX),
        }
        for t in &self.trades {
            t.hash(h);
        }
        for a in &self.appeal {
            h.write_u64(*a as u16 as u64);
        }
        for x in &self.hurt {
            h.write_i64(x.0);
        }
        for n in &self.notes {
            h.write_u64(
                n.tick as u64
                    | (n.plan as u64) << 32
                    | (n.stake as u64) << 40
                    | (n.why as u64) << 48,
            );
        }
    }

    /// Plans, operations and the last decisions, for reports.
    pub(in crate::ai) fn summary(&self) -> String {
        let plans: Vec<String> = self
            .plans
            .iter()
            .filter(|p| p.stake > Stake::Off)
            .map(|p| format!("{}:{}", p.kind.name(), p.stake.name()))
            .collect();
        let ops: Vec<String> = self
            .ops
            .iter()
            .map(|o| {
                let k = |m: Fx| format!("{}.{}", m.floor_int() / 1000, m.floor_int() / 100 % 10);
                format!(
                    "{}({} {}u {}k kill {}k lost {}k)",
                    o.kind.name(),
                    o.phase.name(),
                    o.units.len(),
                    k(o.mass()),
                    k(o.ledger.killed),
                    k(o.ledger.lost),
                )
            })
            .collect();
        let notes: Vec<String> = self
            .notes
            .iter()
            .rev()
            .take(4)
            .map(|n| {
                format!(
                    "{}m {}->{} ({})",
                    n.tick / 600,
                    n.plan.name(),
                    n.stake.name(),
                    n.why.name()
                )
            })
            .collect();
        let mut appeal: Vec<(i16, PlanKind)> = PLANS
            .iter()
            .map(|&k| (self.appeal[k as usize], k))
            .collect();
        appeal.sort_by_key(|&(a, k)| (std::cmp::Reverse(a), k));
        let appeal: Vec<String> = appeal
            .iter()
            .take(6)
            .map(|(a, k)| format!("{}={a}", k.name()))
            .collect();
        let k = |m: Fx| m.floor_int() / 1000;
        let trades: Vec<String> = self
            .trades
            .iter()
            .enumerate()
            .filter(|(_, t)| t.killed + t.lost > Fx::ZERO)
            .map(|(i, t)| format!("{}:{}k/{}k", OPS[i].name(), k(t.killed), k(t.lost)))
            .collect();
        format!(
            "trades [{}] plans [{}] appeal [{}] ops [{}] notes [{}] salvos {}",
            trades.join(" "),
            plans.join(" "),
            appeal.join(" "),
            ops.join(" "),
            notes.join("; "),
            self.salvos
        )
    }

    /// One line per operation: what, where it stands, its target, its trade.
    pub(in crate::ai) fn op_lines(&self) -> Vec<String> {
        self.ops
            .iter()
            .map(|o| {
                format!(
                    "#{} {} {} {}u {}m to ({},{}) rally ({},{}) want {} kill {} lost {}",
                    o.id,
                    o.kind.name(),
                    o.phase.name(),
                    o.units.len(),
                    o.mass().floor_int(),
                    o.target.x.floor_int(),
                    o.target.y.floor_int(),
                    o.rally.x.floor_int(),
                    o.rally.y.floor_int(),
                    o.want.floor_int(),
                    o.ledger.killed.floor_int(),
                    o.ledger.lost.floor_int()
                )
            })
            .collect()
    }

    pub(in crate::ai) fn plan(&self, kind: PlanKind) -> Stake {
        self.plans
            .iter()
            .find(|p| p.kind == kind)
            .map_or(Stake::Off, |p| p.stake)
    }
}
