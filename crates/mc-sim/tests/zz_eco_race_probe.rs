//! The economy race: how to play the economy, found by playing it. One scripted
//! player alone on a map with nothing to do but grow, until it finishes a Deep
//! Core; the score is the minute it does. A plan is a set of dials (what to build,
//! how much, when to switch), so running many plans side by side shows which
//! dials matter and where they should sit. Reclaim of the map's wreckage counts.
//!
//! `ECO=serac_divide,twin_shoals:0+1 ECO_PLANS='best; lean:mines=4; ...'
//! cargo test --profile gate -p mc-sim --test sim -- zz_eco_race_probe:: --ignored --nocapture`
//!
//! `ECO` is maps with the starts to play each from (default `:0+1`); `ECO_PLANS` is
//! `name[:dial=value,...]` separated by `;` (the dials are `Plan`'s fields and
//! default to `Plan::default`), or `@file` with one plan a line. `ECO_MIN` caps a
//! game in minutes (40), `ECO_JOBS` runs that many games at once (the CPU count),
//! `ECO_LOG=1` prints each game's minutes, `ECO_TRACE=N` every order of its first N
//! minutes, `ECO_DEBUG=1` the wreckage by distance and failed mine searches. Every
//! game prints one `RESULT` line. `scripts/eco-search.py` searches the dials with it.
use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, BlueprintId, Blueprints, UnitBlueprint};
use mc_jobs::Pool;
use mc_sim::command::{Command, PlayerCommand};
use mc_sim::focus::{Focus, Priority};
use mc_sim::tables::{Controller, OrderKind, NO_ORDER};
use mc_sim::world::snap_to_build_grid;
use mc_sim::{AiConfig, MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::{Arc, Mutex};

const TPS: u32 = 10;
/// Energy counted as worth one mass when weighing an upgrade (economy structures' ratio).
const ENERGY_PER_MASS: f32 = 6.0;

/// The dials. Rates are materials a second (the side's mass income).
#[derive(Clone, Debug)]
struct Plan {
    name: String,
    /// Tier 1 mines to hold.
    mines: usize,
    /// How far from the start a mine may stand.
    range: f32,
    /// A spot's yield, shared with the mines already there, must be this share of a lone mine's.
    eff: f32,
    /// Masons made by a Forge (0: no Forge at all).
    engineers: usize,
    /// Mines the commander puts down before the Forge.
    factory_after: usize,
    /// Energy income kept per mass of income, beyond the mines' upkeep.
    ratio: f32,
    /// Idle builders reclaim wrecks within this of the start (0: never).
    reclaim: f32,
    /// Scavenger towers put over the richest wreck fields.
    towers: usize,
    /// Income at which the tech 2 step starts, and the tech 3 step.
    tech2_at: f32,
    tech3_at: f32,
    /// The tech steps: 0 the commander's engineering suites, 1 the Forge's upgrades.
    tech: u8,
    /// Masons build their own next tier as the side's tech allows, up to this tier (1: never).
    eng_up: u8,
    /// While the materials store is over half full the Forge keeps making Masons, up to this many.
    engineers_max: usize,
    /// Builders reclaim only while the materials store is under this fraction full;
    /// over it they help spend it instead.
    reclaim_below: f32,
    /// Commander refits: Material Formation Engine and Salvage Drone Port at this
    /// income (negative: never).
    mfe_at: f32,
    drones_at: f32,
    /// Upgrade a mine besides the goal's when it pays back within this many seconds (0: never).
    horizon: f32,
    /// Such upgrades running at once.
    lanes: usize,
    /// Build the Deep Core outright on a new spot instead of upgrading the best mine.
    build_dc: bool,
    /// The goal's line of upgrades starts as soon as each tier opens (else only once the
    /// side has tech 3).
    early_line: bool,
    /// Economy focus: 0 never set, 1 power first when out of energy, mines first when
    /// out of mass, 2 mines always first (power first while out of energy).
    focus: u8,
    /// Idle builders help the goal (its upgrades, the commander's tech refits).
    assist: bool,
    /// Idle builders reclaim before they help the goal.
    reclaim_first: bool,
    /// Top reactor tier built, and the income each tier waits for.
    t2_power_at: f32,
    t3_power_at: f32,
}

/// The best plan the search found (docs/ECONOMY.md): Deep Core at 11-14 minutes.
impl Default for Plan {
    fn default() -> Self {
        Plan {
            name: "best".into(),
            mines: 6,
            range: 4500.0,
            eff: 0.7,
            engineers: 8,
            factory_after: 1,
            ratio: 9.0,
            reclaim: 4000.0,
            towers: 2,
            tech2_at: 20.0,
            tech3_at: 25.0,
            tech: 0,
            eng_up: 1,
            engineers_max: 8,
            reclaim_below: 0.5,
            mfe_at: 10.0,
            drones_at: -1.0,
            horizon: 0.0,
            lanes: 1,
            build_dc: false,
            early_line: true,
            focus: 2,
            assist: true,
            reclaim_first: false,
            t2_power_at: 3.0,
            t3_power_at: 80.0,
        }
    }
}

impl Plan {
    fn parse(spec: &str) -> Plan {
        let spec = spec.trim();
        let (name, dials) = spec.split_once(':').unwrap_or((spec, ""));
        let mut p = Plan {
            name: name.trim().into(),
            ..Plan::default()
        };
        for kv in dials.split(',').filter(|s| !s.trim().is_empty()) {
            let (k, v) = kv.split_once('=').expect("dial=value");
            let (k, v) = (k.trim(), v.trim());
            let f: f32 = v.parse().unwrap_or_else(|_| panic!("{k}={v}"));
            match k {
                "mines" => p.mines = f as usize,
                "range" => p.range = f,
                "eff" => p.eff = f,
                "engineers" => p.engineers = f as usize,
                "factory_after" => p.factory_after = f as usize,
                "ratio" => p.ratio = f,
                "reclaim" => p.reclaim = f,
                "towers" => p.towers = f as usize,
                "tech2_at" => p.tech2_at = f,
                "tech3_at" => p.tech3_at = f,
                "tech" => p.tech = f as u8,
                "eng_up" => p.eng_up = f as u8,
                "engineers_max" => p.engineers_max = f as usize,
                "reclaim_below" => p.reclaim_below = f,
                "mfe_at" => p.mfe_at = f,
                "drones_at" => p.drones_at = f,
                "horizon" => p.horizon = f,
                "lanes" => p.lanes = f as usize,
                "build_dc" => p.build_dc = f != 0.0,
                "early_line" => p.early_line = f != 0.0,
                "focus" => p.focus = f as u8,
                "assist" => p.assist = f != 0.0,
                "reclaim_first" => p.reclaim_first = f != 0.0,
                "t2_power_at" => p.t2_power_at = f,
                "t3_power_at" => p.t3_power_at = f,
                _ => panic!("unknown dial {k}"),
            }
        }
        p
    }
}

/// The blueprints the bot names.
struct Keys {
    mine: [BlueprintId; 4],
    power: [BlueprintId; 3],
    forge: BlueprintId,
    mason: BlueprintId,
    tower: BlueprintId,
}

impl Keys {
    fn new(b: &Blueprints) -> Keys {
        let id = |k: &str| b.id_of(k).unwrap_or_else(|| panic!("{k}"));
        Keys {
            mine: [
                id("aster_core_mine"),
                id("aster_core_mine_t2"),
                id("aster_core_mine_t3"),
                id("aster_core_mine_t4"),
            ],
            power: [
                id("aster_t1_power"),
                id("aster_t2_power"),
                id("aster_t3_power"),
            ],
            forge: id("aster_t1_land_factory"),
            mason: id("aster_t1_engineer"),
            tower: id("aster_t1_reclaimer"),
        }
    }
}

/// What the bot sees of its side this think.
#[derive(Default)]
struct View {
    commander: Option<usize>,
    builders: Vec<usize>,
    idle: Vec<usize>,
    masons: usize,
    forges: Vec<usize>,
    /// Finished mines.
    mines: Vec<usize>,
    /// Mines standing, going up or queued.
    mine_spots: Vec<FxVec2>,
    /// Structures going up or queued: where, and what.
    planned: Vec<(FxVec2, BlueprintId)>,
    power_going_up: usize,
    towers: Vec<FxVec2>,
    upkeep: f32,
    upgrading: Vec<usize>,
    /// Units some builder of ours is reclaiming, and how many builders on each.
    reclaiming: Vec<u32>,
    deep_core: bool,
    deep_core_going_up: Option<usize>,
    /// Structures of ours going up.
    site_rows: Vec<usize>,
    /// Mine lots ordered again and again without a site ever going up there
    /// (the builder cannot reach or use it): left alone.
    refused: Vec<FxVec2>,
}

fn head_kind(w: &World, row: usize) -> Option<OrderKind> {
    let h = w.state.units.order_head[row];
    (h != NO_ORDER).then(|| w.state.orders.order[h as usize].kind)
}

fn look(w: &World, p: u8, k: &Keys) -> View {
    let s = &w.state;
    let u = &s.units;
    let mut v = View::default();
    let sites: Vec<(FxVec2, BlueprintId)> = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == p && !u.is_active(r))
        .map(|r| (u.pos[r], u.blueprint[r]))
        .collect();
    for row in u.slots.iter().filter(|&r| u.owner[r] == p) {
        let bp = w.bp(row);
        let active = u.is_active(row);
        if !active {
            if bp.is_structure() {
                v.site_rows.push(row);
                v.planned.push((u.pos[row], bp.id));
                if bp.has(cat::POWER) {
                    v.power_going_up += 1;
                }
                if bp.mine.is_some() {
                    v.mine_spots.push(u.pos[row]);
                }
                if bp.id == k.mine[3] {
                    v.deep_core_going_up = Some(row);
                }
            } else if bp.has(cat::ENGINEER) {
                v.masons += 1;
            }
            continue;
        }
        let kind = head_kind(w, row);
        if kind == Some(OrderKind::Upgrade) {
            v.upgrading.push(row);
        }
        // Every structure a builder of ours has queued.
        let mut o = u.order_head[row];
        while o != NO_ORDER {
            let ord = &s.orders.order[o as usize];
            match ord.kind {
                // Once the site stands, the order building it is counted with the site.
                OrderKind::Build
                    if sites.iter().any(|&(p, id)| {
                        id == ord.blueprint && p.distance(ord.pos).to_f32() < 16.0
                    }) => {}
                OrderKind::Build => {
                    let b = w.blueprints.unit(ord.blueprint);
                    v.planned.push((ord.pos, ord.blueprint));
                    if b.has(cat::POWER) {
                        v.power_going_up += 1;
                    }
                    if b.mine.is_some() {
                        v.mine_spots.push(ord.pos);
                    }
                    if ord.blueprint == k.mine[3] {
                        v.deep_core_going_up = Some(row);
                    }
                    if ord.blueprint == k.tower {
                        v.towers.push(ord.pos);
                    }
                }
                OrderKind::Reclaim => v.reclaiming.push(ord.target.0),
                _ => {}
            }
            o = s.orders.next[o as usize];
        }
        if bp.has(cat::COMMANDER) {
            v.commander = Some(row);
        }
        if bp.builder.is_some() && bp.is_mobile() {
            v.builders.push(row);
            // Assisting and reclaiming never end on their own: such a builder is free
            // for anything better, and keeps at it otherwise.
            if matches!(
                kind,
                None | Some(OrderKind::Assist) | Some(OrderKind::Reclaim)
            ) {
                v.idle.push(row);
            }
            if bp.has(cat::ENGINEER) && !bp.has(cat::COMMANDER) {
                v.masons += 1;
            }
        }
        if bp.has(cat::FACTORY) && bp.is_structure() {
            v.forges.push(row);
        }
        if bp.mine.is_some() {
            v.mines.push(row);
            v.mine_spots.push(u.pos[row]);
            v.upkeep += bp.economy.energy_upkeep.to_f32();
            if bp.id == k.mine[3] {
                v.deep_core = true;
            }
        }
        if bp.reclaimer.is_some() && bp.is_structure() {
            v.towers.push(u.pos[row]);
        }
    }
    v
}

/// A free lot for `bp` near `near`, clear of what is planned and of the ore fields.
fn place(
    w: &World,
    bp: &UnitBlueprint,
    near: FxVec2,
    v: &View,
    keep_off_ore: bool,
) -> Option<FxVec2> {
    let foot = bp.footprint.0.max(bp.footprint.1) as i32;
    let pitch = ((foot + 2) * mc_map::BUILD_CELL_M) as f32;
    let ore = if keep_off_ore {
        w.ore_centres()
    } else {
        Vec::new()
    };
    let clear = |site: FxVec2| {
        w.can_place(bp, site)
            && v.planned.iter().all(|(p, id)| {
                let other = w.blueprints.unit(*id);
                let gap = (other.footprint.0.max(other.footprint.1) as i32 + foot)
                    * mc_map::BUILD_CELL_M
                    / 2
                    + 6;
                p.distance(site).to_f32() > gap as f32
            })
            && ore.iter().all(|o| o.distance(site).to_f32() > 120.0)
    };
    for ring in 0..40 {
        let n = (ring * 6).max(1);
        for i in 0..n {
            let a =
                Angle(((i as u64 * 65536) / n as u64) as u16 + (ring as u16).wrapping_mul(2701));
            let site = snap_to_build_grid(
                bp,
                near + FxVec2::from_angle(a) * Fx::from_f32(pitch * ring as f32),
            );
            if clear(site) {
                return Some(site);
            }
        }
    }
    None
}

/// The best spot for a new tier 1 mine for a builder at `from`: yield over time to get there.
fn mine_spot(
    w: &World,
    plan: &Plan,
    k: &Keys,
    start: FxVec2,
    from: FxVec2,
    v: &View,
) -> Option<FxVec2> {
    let bp = w.blueprints.unit(k.mine[0]);
    let m = bp.mine?;
    let mut spots: Vec<FxVec2> = w
        .ore_centres()
        .into_iter()
        .filter(|o| o.distance(start).to_f32() <= plan.range)
        .collect();
    let step = 350.0;
    let rings = (plan.range / step) as i32;
    for ring in 1..=rings {
        let r = step * ring as f32;
        let n = ((std::f32::consts::TAU * r / step) as i32).max(6);
        for i in 0..n {
            let a = Angle(((i as u64 * 65536) / n as u64) as u16);
            let p = start + FxVec2::from_angle(a) * Fx::from_f32(r);
            if w.terrain.in_bounds(p) {
                spots.push(p);
            }
        }
    }
    let mut best: Option<(f32, FxVec2)> = None;
    for spot in spots {
        // A planned mine takes its ground as if it stood.
        if v.mine_spots
            .iter()
            .any(|p| p.distance(spot).to_f32() < 600.0)
            || v.refused.iter().any(|p| p.distance(spot).to_f32() < 150.0)
        {
            continue;
        }
        let share = w.mine_share_at(bp, spot);
        let rate = share.rate(&m).to_f32();
        if share.efficiency(&m).to_f32() < plan.eff {
            continue;
        }
        let travel = spot.distance(from).to_f32() / 28.0;
        let score = rate / (60.0 + travel);
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, spot));
        }
    }
    if best.is_none() && std::env::var("ECO_DEBUG").is_ok() {
        let effs: Vec<String> = w
            .ore_centres()
            .into_iter()
            .map(|o| {
                let s = w.mine_share_at(bp, o);
                format!(
                    "{:.0}m eff {:.2} rate {:.1}",
                    o.distance(start).to_f32(),
                    s.efficiency(&m).to_f32(),
                    s.rate(&m).to_f32()
                )
            })
            .collect();
        eprintln!("no mine spot; ore: {effs:?}");
    }
    let spot = best?.1;
    // The lot nearest the spot that takes a mine.
    for ring in 0..6 {
        let n = (ring * 6).max(1);
        for i in 0..n {
            let a = Angle(((i as u64 * 65536) / n as u64) as u16);
            let site =
                snap_to_build_grid(bp, spot + FxVec2::from_angle(a) * Fx::from_int(ring * 24));
            if w.can_place(bp, site)
                && v.planned
                    .iter()
                    .all(|(p, _)| p.distance(site).to_f32() > 40.0)
            {
                return Some(site);
            }
        }
    }
    None
}

/// The refit kit of the commander's module `key`, if it fits now and is not fitted.
fn refit_kit(w: &World, row: usize, key: &str) -> Option<BlueprintId> {
    let (set, loadout) = w.blueprints.loadout(w.bp(row).id)?;
    for (s, slot) in set.slots.iter().enumerate() {
        for (m, module) in slot.modules.iter().enumerate() {
            if module.key == key {
                if loadout.module(s) == Some(m as u8) {
                    return None;
                }
                return set
                    .fit(&loadout.fitted, s, m as u8)
                    .is_ok()
                    .then_some(module.kit);
            }
        }
    }
    None
}

fn has_module(w: &World, row: usize, key: &str) -> bool {
    let Some((set, loadout)) = w.blueprints.loadout(w.bp(row).id) else {
        return false;
    };
    set.slots.iter().enumerate().any(|(s, slot)| {
        loadout.module(s).is_some_and(|m| {
            slot.modules[m as usize].key == key
                || (key == "eng_2" && slot.modules[m as usize].key == "eng_3")
        })
    })
}

struct Bot {
    plan: Plan,
    p: u8,
    /// Seconds since the energy store last ran dry, for the stall count.
    stall_ticks: u32,
    log: Vec<String>,
    tech2: Option<u32>,
    tech3: Option<u32>,
    line_start: Option<u32>,
    income_at: [f32; 5],
    /// Every mine lot ordered, and how many times.
    ordered: Vec<(FxVec2, u32)>,
}

/// What one think knows about its side.
struct Now {
    start: FxVec2,
    tech: u8,
    secs: u32,
    income: f32,
    m_frac: f32,
    e_short: bool,
    e_dry: bool,
    builders: usize,
    /// The goal mine or Deep Core site while it climbs or goes up: what to help.
    goal_busy: Option<usize>,
}

impl Bot {
    fn think(&mut self, w: &World, k: &Keys) -> Vec<Command> {
        let plan = self.plan.clone();
        let pl = &w.state.players[self.p as usize];
        let mut v = look(w, self.p, k);
        v.refused = self
            .ordered
            .iter()
            .filter(|&&(_, n)| n >= 3)
            .map(|&(p, _)| p)
            .collect();
        let tech = w.side_tech(self.p);
        let secs = w.state.tick / TPS;
        if tech >= 2 && self.tech2.is_none() {
            self.tech2 = Some(secs);
        }
        if tech >= 3 && self.tech3.is_none() {
            self.tech3 = Some(secs);
        }
        let income = pl.mass_income.to_f32();
        let e_income = pl.energy_income.to_f32();
        let e_frac = pl.energy.to_f32() / pl.energy_capacity.to_f32().max(1.0);
        let m_frac = pl.mass.to_f32() / pl.mass_capacity.to_f32().max(1.0);
        let e_dry = e_frac < 0.1 && pl.energy_spent.to_f32() >= e_income * 0.95;
        let mut now = Now {
            start: pl.start,
            tech,
            secs,
            income,
            m_frac,
            e_short: e_income < plan.ratio * income + v.upkeep || e_dry,
            e_dry,
            builders: v.builders.len(),
            goal_busy: None,
        };
        let mut out = Vec::new();
        if plan.focus > 0 {
            let m_dry = m_frac < 0.05 && pl.mass_demand > pl.mass_income;
            let want = Focus {
                power: if e_dry {
                    Priority::First
                } else {
                    Priority::Even
                },
                mines: if !e_dry && (plan.focus == 2 || m_dry) {
                    Priority::First
                } else {
                    Priority::Even
                },
            };
            if want != pl.focus {
                out.push(Command::SetFocus { focus: want });
            }
        }
        let goal = self.climb_goal(w, &now, &v, &mut out);
        now.goal_busy = goal.filter(|g| {
            v.upgrading.contains(g)
                || out.iter().any(
                    |c| matches!(c, Command::Upgrade { units } if units[0] == w.state.units.id(*g)),
                )
        });
        if let Some(r) = v.deep_core_going_up {
            now.goal_busy.get_or_insert(r);
        }
        self.side_upgrades(w, &now, &v, goal, &mut out);
        self.run_forges(w, k, &now, &mut v, &mut out);
        let mut wrecks_taken: Vec<u32> = v.reclaiming.clone();
        for row in v.idle.clone() {
            let job = self
                .build_job(w, k, &now, &mut v, row)
                .or_else(|| self.help_job(w, &now, &v, row, &mut wrecks_taken));
            if let Some(Some(c)) = job {
                out.push(c);
            }
        }
        out
    }

    /// The goal: the mine whose line leads to the Deep Core, taking each next tier
    /// as it opens. Returns the goal mine.
    fn climb_goal(
        &mut self,
        w: &World,
        now: &Now,
        v: &View,
        out: &mut Vec<Command>,
    ) -> Option<usize> {
        let plan = &self.plan;
        let goal = v.mines.iter().copied().max_by_key(|&r| {
            let bp = w.bp(r);
            let rate = w
                .state
                .mines
                .by_unit
                .get(&w.state.units.id(r))
                .map_or(0, |m| {
                    (m.full_rate(&bp.mine.unwrap()).to_f32() * 100.0) as i64
                });
            (bp.tech, rate, r)
        })?;
        let bp = w.bp(goal);
        if v.upgrading.contains(&goal) || (plan.build_dc && bp.tech >= 3) {
            return Some(goal);
        }
        let next = w.blueprints.unit(bp.upgrades_to?);
        let open = w.blueprints.upgrade_needs(next) <= now.tech;
        let due = plan.early_line || now.tech >= 3 || plan.build_dc;
        if open && due && (!plan.build_dc || next.tech <= 3) {
            out.push(Command::Upgrade {
                units: vec![w.state.units.id(goal)],
            });
            self.line_start.get_or_insert(now.secs);
        }
        Some(goal)
    }

    /// Other mines climb while their upgrade pays back within the horizon.
    fn side_upgrades(
        &self,
        w: &World,
        now: &Now,
        v: &View,
        goal: Option<usize>,
        out: &mut Vec<Command>,
    ) {
        let plan = &self.plan;
        if plan.horizon <= 0.0 || now.e_short {
            return;
        }
        let running = v
            .upgrading
            .iter()
            .filter(|&&r| w.bp(r).mine.is_some() && Some(r) != goal)
            .count();
        let mut cands: Vec<(f32, usize)> = Vec::new();
        for &r in &v.mines {
            if Some(r) == goal || v.upgrading.contains(&r) {
                continue;
            }
            let bp = w.bp(r);
            let Some(next) = bp.upgrades_to.map(|n| w.blueprints.unit(n)) else {
                continue;
            };
            if next.tech > 3 || w.blueprints.upgrade_needs(next) > now.tech {
                continue;
            }
            let Some(ms) = w.state.mines.by_unit.get(&w.state.units.id(r)) else {
                continue;
            };
            let gain = ms.full_rate(&next.mine.unwrap()).to_f32()
                - ms.full_rate(&bp.mine.unwrap()).to_f32();
            let (cm, ce) = w.blueprints.upgrade_cost(next);
            let payback = (cm.to_f32() + ce.to_f32() / ENERGY_PER_MASS) / gain.max(0.01);
            if payback < plan.horizon {
                cands.push((payback, r));
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, r) in cands.into_iter().take(plan.lanes.saturating_sub(running)) {
            out.push(Command::Upgrade {
                units: vec![w.state.units.id(r)],
            });
        }
    }

    /// The Forge makes Masons, and takes its own tiers when it carries the tech.
    fn run_forges(&self, w: &World, k: &Keys, now: &Now, v: &mut View, out: &mut Vec<Command>) {
        let plan = &self.plan;
        for &f in &v.forges.clone() {
            if w.state.units.order_head[f] != NO_ORDER {
                continue;
            }
            let fbp = w.bp(f);
            let at = if fbp.tech == 1 {
                plan.tech2_at
            } else {
                plan.tech3_at
            };
            if plan.tech == 1 && fbp.upgrades_to.is_some() && now.income >= at {
                out.push(Command::Upgrade {
                    units: vec![w.state.units.id(f)],
                });
                continue;
            }
            let more = now.m_frac > 0.5 && v.masons < plan.engineers_max;
            if v.masons < plan.engineers || more {
                out.push(Command::Produce {
                    factories: vec![w.state.units.id(f)],
                    blueprint: k.mason,
                    count: 1,
                });
                v.masons += 1;
            }
        }
    }

    /// Something to build (or refit, or climb to) for this idle builder, in order:
    /// power while short, the Forge, mines, its own next tier, the commander's
    /// refits, Scavengers, the Deep Core outright.
    fn build_job(
        &mut self,
        w: &World,
        k: &Keys,
        now: &Now,
        v: &mut View,
        row: usize,
    ) -> Option<Option<Command>> {
        let plan = self.plan.clone();
        let bp = w.bp(row);
        let id = w.state.units.id(row);
        let pos = w.state.units.pos[row];
        let is_cmdr = bp.has(cat::COMMANDER);
        let btech = w.blueprints.builds_tech(bp).min(3);
        let builds = |b: BlueprintId| bp.builder.as_ref().is_some_and(|x| x.builds.contains(&b));
        let build = |b: BlueprintId, site: FxVec2, v: &mut View| {
            v.planned.push((site, b));
            Some(Some(Command::Build {
                units: vec![id],
                blueprint: b,
                pos: site,
                heading: w.blueprints.unit(b).build_heading(),
                queue: false,
            }))
        };
        if now.e_short && v.power_going_up < 1 + now.builders / 3 {
            let tier = if btech >= 3 && now.income >= plan.t3_power_at {
                2
            } else if btech >= 2 && now.income >= plan.t2_power_at {
                1
            } else {
                0
            };
            let b = k.power[tier];
            if let Some(site) = builds(b)
                .then(|| place(w, w.blueprints.unit(b), pos, v, true))
                .flatten()
            {
                v.power_going_up += 1;
                return build(b, site, v);
            }
        }
        if is_cmdr
            && plan.engineers > 0
            && v.forges.is_empty()
            && !v.planned.iter().any(|(_, b)| *b == k.forge)
            && v.mine_spots.len() >= plan.factory_after
        {
            if let Some(site) = place(w, w.blueprints.unit(k.forge), now.start, v, true) {
                return build(k.forge, site, v);
            }
        }
        if v.mine_spots.len() < plan.mines && builds(k.mine[0]) {
            if let Some(site) = mine_spot(w, &plan, k, now.start, pos, v) {
                match self
                    .ordered
                    .iter_mut()
                    .find(|(p, _)| p.distance(site).to_f32() < 1.0)
                {
                    Some((_, n)) => *n += 1,
                    None => self.ordered.push((site, 1)),
                }
                v.mine_spots.push(site);
                return build(k.mine[0], site, v);
            }
        }
        if let Some(next) = bp
            .upgrades_to
            .map(|n| w.blueprints.unit(n))
            .filter(|_| !is_cmdr)
        {
            if next.tech <= plan.eng_up
                && w.blueprints.upgrade_needs(next) <= now.tech
                && !now.e_dry
            {
                return Some(Some(Command::Upgrade { units: vec![id] }));
            }
        }
        if is_cmdr {
            let mut kit = None;
            if plan.tech == 0 {
                if now.income >= plan.tech2_at {
                    kit = kit.or_else(|| refit_kit(w, row, "eng_2"));
                }
                if now.income >= plan.tech3_at && has_module(w, row, "eng_2") {
                    kit = kit.or_else(|| refit_kit(w, row, "eng_3"));
                }
            }
            if plan.mfe_at >= 0.0 && now.income >= plan.mfe_at {
                kit = kit.or_else(|| refit_kit(w, row, "mfe"));
            }
            if plan.drones_at >= 0.0 && now.income >= plan.drones_at && !has_module(w, row, "mfe") {
                kit = kit.or_else(|| refit_kit(w, row, "drone_port"));
            }
            if let Some(kit) = kit {
                return Some(Some(Command::Refit {
                    units: vec![id],
                    kit,
                }));
            }
        }
        if v.towers.len() < plan.towers && builds(k.tower) {
            if let Some(field) = wreck_field(w, now.start, 640.0, &v.towers) {
                if let Some(site) = place(w, w.blueprints.unit(k.tower), field, v, true) {
                    v.towers.push(site);
                    return build(k.tower, site, v);
                }
            }
        }
        if plan.build_dc && v.deep_core_going_up.is_none() && !v.deep_core && builds(k.mine[3]) {
            // Any spot will do for the goal itself.
            let anywhere = Plan {
                eff: 0.3,
                ..plan.clone()
            };
            if let Some(site) = mine_spot(w, &anywhere, k, now.start, pos, v) {
                v.deep_core_going_up = Some(row);
                self.line_start.get_or_insert(now.secs);
                return build(k.mine[3], site, v);
            }
        }
        None
    }

    /// Nothing to build: reclaim while the store has room (first or after the
    /// goal, by plan), help the goal, or help whatever else is going up. `Some(None)`
    /// keeps the builder at what it is doing.
    fn help_job(
        &self,
        w: &World,
        now: &Now,
        v: &View,
        row: usize,
        taken: &mut Vec<u32>,
    ) -> Option<Option<Command>> {
        let plan = &self.plan;
        let id = w.state.units.id(row);
        let pos = w.state.units.pos[row];
        let kind = head_kind(w, row);
        let assisting = (kind == Some(OrderKind::Assist))
            .then(|| w.state.orders.order[w.state.units.order_head[row] as usize].target);
        let assist = |t: usize| {
            let target = w.state.units.id(t);
            Some((assisting != Some(target)).then_some(Command::Assist {
                units: vec![id],
                target,
                queue: false,
            }))
        };
        let reclaim_ok = now.m_frac < plan.reclaim_below;
        let reclaim = |taken: &mut Vec<u32>| {
            if kind == Some(OrderKind::Reclaim) {
                return Some(None);
            }
            reclaim_job(w, plan, now.start, id, pos, taken).map(Some)
        };
        if plan.reclaim_first && reclaim_ok {
            if let Some(c) = reclaim(taken) {
                return Some(c);
            }
        }
        if plan.assist {
            let refitting = v
                .commander
                .filter(|&c| c != row && head_kind(w, c) == Some(OrderKind::Upgrade));
            if let Some(t) = now.goal_busy.or(refitting).filter(|&t| t != row) {
                return assist(t);
            }
        }
        if reclaim_ok {
            if let Some(c) = reclaim(taken) {
                return Some(c);
            }
        }
        v.upgrading
            .iter()
            .copied()
            .filter(|&r| r != row)
            .chain(v.site_rows.iter().copied())
            .min_by_key(|&r| w.state.units.pos[r].distance(pos))
            .and_then(assist)
    }
}

/// Reclaim the nearest wreck no one of ours is on, while there is room to store it.
fn reclaim_job(
    w: &World,
    plan: &Plan,
    start: FxVec2,
    id: mc_sim::UnitId,
    pos: FxVec2,
    taken: &mut Vec<u32>,
) -> Option<Command> {
    if plan.reclaim <= 0.0 {
        return None;
    }
    let wr = nearest_wreck(w, start, pos, plan.reclaim, taken)?;
    let h = w.state.wrecks.slots.handle(wr);
    taken.push(h.0);
    Some(Command::ReclaimWreck {
        units: vec![id],
        wreck: h,
        queue: false,
    })
}

/// The wreck nearest `from` within `radius` of the start with materials left,
/// no builder of ours already on it.
fn nearest_wreck(
    w: &World,
    start: FxVec2,
    from: FxVec2,
    radius: f32,
    taken: &[u32],
) -> Option<usize> {
    let wr = &w.state.wrecks;
    wr.slots
        .iter()
        .filter(|&r| wr.mass[r] > Fx::ZERO && wr.pos[r].distance(start).to_f32() <= radius)
        .filter(|&r| !taken.contains(&wr.slots.handle(r).0))
        .min_by_key(|&r| (wr.pos[r].distance_sq(from), r))
}

/// The middle of the richest wreck field not yet in a tower's reach.
fn wreck_field(w: &World, start: FxVec2, reach: f32, towers: &[FxVec2]) -> Option<FxVec2> {
    let wr = &w.state.wrecks;
    let live: Vec<usize> = wr
        .slots
        .iter()
        .filter(|&r| wr.mass[r] > Fx::ZERO && wr.pos[r].distance(start).to_f32() < 3500.0)
        .filter(|&r| {
            towers
                .iter()
                .all(|t| t.distance(wr.pos[r]).to_f32() > reach)
        })
        .collect();
    let mut best: Option<(f32, FxVec2)> = None;
    for &a in &live {
        let sum: f32 = live
            .iter()
            .filter(|&&b| wr.pos[a].distance(wr.pos[b]).to_f32() < reach * 0.8)
            .map(|&b| wr.mass[b].to_f32())
            .sum();
        // A field far off pays later: worth a little less.
        let score = sum / (1.0 + wr.pos[a].distance(start).to_f32() / 3000.0);
        if sum > 400.0 && best.is_none_or(|(s, _)| score > s) {
            best = Some((score, wr.pos[a]));
        }
    }
    best.map(|b| b.1)
}

struct Outcome {
    done: Option<u32>,
    line: String,
}

fn play(
    name: &str,
    map: &mc_map::MapFile,
    bps: &Arc<Blueprints>,
    plan: &Plan,
    start: u8,
    minutes: u32,
    log: bool,
) -> Outcome {
    let config = MatchConfig {
        seed: 7,
        players: vec![PlayerSetup {
            name: "eco".into(),
            faction: "Aster".into(),
            ai: AiConfig::default(),
            team: 0,
            controller: Controller::Human,
            start,
        }],
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let k = Keys::new(bps);
    let mut w = World::new(map, bps.clone(), Arc::new(Pool::new(1)), &config).unwrap();
    let mut bot = Bot {
        plan: plan.clone(),
        p: 0,
        stall_ticks: 0,
        log: Vec::new(),
        tech2: None,
        tech3: None,
        line_start: None,
        income_at: [0.0; 5],
        ordered: Vec::new(),
    };
    if std::env::var("ECO_DEBUG").is_ok() {
        let wr = &w.state.wrecks;
        let start = w.state.players[0].start;
        let mut bands = [(0, 0.0f32); 6];
        for r in wr.slots.iter() {
            let b = ((wr.pos[r].distance(start).to_f32() / 1000.0) as usize).min(5);
            bands[b].0 += 1;
            bands[b].1 += wr.mass[r].to_f32();
        }
        eprintln!("wrecks by km from start (count, mass): {bands:?}");
    }
    let trace: u32 = std::env::var("ECO_TRACE")
        .ok()
        .and_then(|m| m.parse().ok())
        .unwrap_or(0);
    let mut done = None;
    let mut pending: Vec<PlayerCommand> = Vec::new();
    for t in 0..minutes * 60 * TPS {
        w.tick(&pending).unwrap();
        pending.clear();
        let pl = &w.state.players[0];
        if pl.energy.to_f32() < 1.0 && pl.energy_spent > Fx::ZERO {
            bot.stall_ticks += 1;
        }
        if t % TPS == 0 {
            let cmds = bot.think(&w, &k);
            if trace > 0 && w.state.tick < trace * 60 * TPS {
                for c in &cmds {
                    let what = match c {
                        Command::Build { blueprint, pos, .. } => format!(
                            "build {} at {:.0} m",
                            w.blueprints.unit(*blueprint).key,
                            pos.distance(w.state.players[0].start).to_f32()
                        ),
                        Command::Refit { kit, .. } => {
                            format!("refit {}", w.blueprints.unit(*kit).key)
                        }
                        Command::Upgrade { units } => {
                            format!("upgrade {}", w.bp(w.state.units.row(units[0]).unwrap()).key)
                        }
                        Command::Assist { target, .. } => {
                            format!("assist {}", w.bp(w.state.units.row(*target).unwrap()).key)
                        }
                        Command::ReclaimWreck { .. } => "reclaim".into(),
                        Command::Produce { .. } => "produce".into(),
                        c => format!("{c:?}"),
                    };
                    eprintln!("{:>5}s {what}", w.state.tick / TPS);
                }
            }
            pending = cmds
                .into_iter()
                .map(|c| PlayerCommand {
                    player: 0,
                    command: c,
                })
                .collect();
            if look(&w, 0, &k).deep_core {
                done = Some(w.state.tick / TPS);
                break;
            }
        }
        let secs = w.state.tick / TPS;
        if w.state.tick.is_multiple_of(60 * TPS) {
            let m = secs / 60;
            if m.is_multiple_of(5) && (m / 5) as usize <= 5 && m > 0 {
                bot.income_at[(m / 5 - 1) as usize] = pl.mass_income.to_f32();
            }
            if log {
                let v = look(&w, 0, &k);
                let tiers = v.mines.iter().fold([0; 4], |mut a, &r| {
                    a[(w.bp(r).tech as usize - 1).min(3)] += 1;
                    a
                });
                bot.log.push(format!(
                    "  {m:>2}m mass {:>6.1}/s (reclaim {:>5.1}) store {:>6.0}/{:<6.0} energy {:>6.0}/s store {:>6.0}/{:<6.0} tech {} mines {:?} builders {} upgrading {} stall {:>4}s",
                    pl.mass_income.to_f32(),
                    pl.reclaim_income.to_f32(),
                    pl.mass.to_f32(),
                    pl.mass_capacity.to_f32(),
                    pl.energy_income.to_f32(),
                    pl.energy.to_f32(),
                    pl.energy_capacity.to_f32(),
                    w.side_tech(0),
                    tiers,
                    v.builders.len(),
                    v.upgrading.len(),
                    bot.stall_ticks / TPS,
                ));
                let jobs: Vec<String> = v
                    .builders
                    .iter()
                    .map(|&r| {
                        let what = match head_kind(&w, r) {
                            None => "idle".to_string(),
                            Some(k) => {
                                let o = &w.state.orders.order[w.state.units.order_head[r] as usize];
                                match k {
                                    OrderKind::Assist | OrderKind::Build | OrderKind::Upgrade => {
                                        format!(
                                            "{k:?} {}",
                                            if k == OrderKind::Assist {
                                                w.state
                                                    .units
                                                    .row(o.target)
                                                    .map_or("?".into(), |t| w.bp(t).key.clone())
                                            } else {
                                                w.blueprints.unit(o.blueprint).key.clone()
                                            }
                                        )
                                    }
                                    k => format!("{k:?}"),
                                }
                            }
                        };
                        format!("{}={what}", w.bp(r).key.trim_start_matches("aster_"))
                    })
                    .collect();
                bot.log.push(format!("       {}", jobs.join(" | ")));
            }
        }
    }
    let pl = &w.state.players[0];
    let fmt = |t: Option<u32>| t.map_or("-".to_string(), |s| format!("{:.1}", s as f32 / 60.0));
    let v = look(&w, 0, &k);
    let plants = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| {
            w.state.units.owner[r] == 0
                && w.state.units.is_active(r)
                && w.bp(r).has(cat::POWER)
                && w.bp(r).is_structure()
        })
        .fold([0; 3], |mut a, r| {
            a[(w.bp(r).tech as usize - 1).min(2)] += 1;
            a
        });
    let line = format!(
        "RESULT plan={} map={} start={} dc={} tech2={} tech3={} line={} inc5={:.1} inc10={:.1} inc15={:.1} inc20={:.1} reclaimed={:.0} stall={} mines={} plants={:?} builders={}",
        plan.name,
        name,
        start,
        fmt(done),
        fmt(bot.tech2),
        fmt(bot.tech3),
        fmt(bot.line_start),
        bot.income_at[0],
        bot.income_at[1],
        bot.income_at[2],
        bot.income_at[3],
        pl.reclaimed_mass.to_f32(),
        bot.stall_ticks / TPS,
        v.mines.len(),
        plants,
        v.builders.len(),
    );
    let mut text = line.clone();
    for l in &bot.log {
        text.push('\n');
        text.push_str(l);
    }
    Outcome { done, line: text }
}

#[test]
#[ignore]
fn eco_race() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let spec = std::env::var("ECO").unwrap_or_else(|_| "serac_divide".into());
    let plans_spec = std::env::var("ECO_PLANS").unwrap_or_else(|_| "base".into());
    let plans_text = match plans_spec.strip_prefix('@') {
        Some(file) => std::fs::read_to_string(file)
            .unwrap()
            .lines()
            .collect::<Vec<_>>()
            .join(";"),
        None => plans_spec,
    };
    let plans: Vec<Plan> = plans_text
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .map(Plan::parse)
        .collect();
    let minutes: u32 = std::env::var("ECO_MIN")
        .ok()
        .and_then(|m| m.parse().ok())
        .unwrap_or(40);
    let log = std::env::var("ECO_LOG").is_ok();
    let jobs: usize = std::env::var("ECO_JOBS")
        .ok()
        .and_then(|m| m.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));
    let mut games = Vec::new();
    for m in spec.split(',') {
        let (name, starts) = m.split_once(':').unwrap_or((m, "0+1"));
        let map = Arc::new(mc_map::MapFile::open(root.join(format!("maps/{name}.mcmap"))).unwrap());
        for s in starts.split('+') {
            let s: u8 = s.parse().unwrap();
            for plan in &plans {
                games.push((name.to_string(), map.clone(), plan.clone(), s));
            }
        }
    }
    let queue = Arc::new(Mutex::new(
        games.into_iter().enumerate().collect::<Vec<_>>(),
    ));
    let results = Arc::new(Mutex::new(Vec::new()));
    let threads: Vec<_> = (0..jobs)
        .map(|_| {
            let (queue, results, bps) = (queue.clone(), results.clone(), bps.clone());
            std::thread::spawn(move || loop {
                let Some((i, (name, map, plan, start))) = queue.lock().unwrap().pop() else {
                    break;
                };
                let game = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    play(&name, &map, &bps, &plan, start, minutes, log)
                }));
                let done = match game {
                    Ok(o) => {
                        println!("{}", o.line);
                        o.done
                    }
                    Err(_) => {
                        println!(
                            "RESULT plan={} map={name} start={start} dc=panic",
                            plan.name
                        );
                        None
                    }
                };
                results.lock().unwrap().push((i, plan.name.clone(), done));
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
}
