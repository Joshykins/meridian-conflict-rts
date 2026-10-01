//! What the battle report shows, worked out once from the match's record
//! (`chronicle.rs`) when the report opens: each side's totals, the curves the
//! charts draw, the battles, the moments of the match and the honours.

use crate::chronicle::{Blast, Chronicle, Death, Frame, SAMPLE_TICKS};
use glam::Vec2;
use mc_core::TICKS_PER_SECOND;
use mc_data::{cat, BlueprintId, Blueprints, UnitBlueprint};

/// The curves a chart can show, one value per side per sample.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Metric {
    MassIncome,
    EnergyIncome,
    Collected,
    Spending,
    Stored,
    Efficiency,
    ArmyValue,
    ArmySize,
    Destroyed,
    Lost,
}

impl Metric {
    pub const ALL: [Metric; 10] = [
        Metric::MassIncome,
        Metric::EnergyIncome,
        Metric::Collected,
        Metric::Spending,
        Metric::Stored,
        Metric::Efficiency,
        Metric::ArmyValue,
        Metric::ArmySize,
        Metric::Destroyed,
        Metric::Lost,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Metric::MassIncome => "Materials Income",
            Metric::EnergyIncome => "Energy Income",
            Metric::Collected => "Materials Collected",
            Metric::Spending => "Spending",
            Metric::Stored => "Materials Stored",
            Metric::Efficiency => "Build Efficiency",
            Metric::ArmyValue => "Army Value",
            Metric::ArmySize => "Army Size",
            Metric::Destroyed => "Value Destroyed",
            Metric::Lost => "Value Lost",
        }
    }

    /// What a value is counted in, after the figure.
    pub fn unit(self) -> &'static str {
        match self {
            Metric::MassIncome | Metric::Spending => "/s",
            Metric::EnergyIncome => "e/s",
            Metric::Efficiency => "%",
            Metric::ArmySize => "units",
            _ => "",
        }
    }
}

/// Where a side's materials went, by what they bought.
pub const SPEND_KINDS: [&str; 6] = [
    "Army",
    "Experimental",
    "Engineers",
    "Economy",
    "Defence",
    "Industry",
];

/// Losses by where the unit fought.
pub const DOMAINS: [&str; 4] = ["Land", "Air", "Naval", "Structures"];

#[derive(Clone, Debug, Default)]
pub struct SideReport {
    pub name: String,
    pub team: u8,
    pub faction: String,
    pub ai: bool,
    pub victor: bool,
    pub defeated_at: Option<u32>,
    /// Materials made by mines and generators plus reclaim, over the match.
    pub collected: f32,
    pub reclaimed: f32,
    pub energy_collected: f32,
    pub spent: f32,
    pub peak_income: f32,
    /// Mean share of asked-for spending paid, and seconds spent under 90%.
    pub efficiency: f32,
    pub stalled: f32,
    pub peak_army: f32,
    pub peak_army_at: u32,
    pub built: u32,
    pub kills: u32,
    pub losses: u32,
    pub destroyed: f32,
    pub lost: f32,
    pub spend: [f32; SPEND_KINDS.len()],
    pub lost_by_domain: [f32; DOMAINS.len()],
    /// Unit types by the worth of what they destroyed: (type, kills, worth).
    pub deadliest: Vec<(BlueprintId, u32, f32)>,
    /// Unit types by how many were built: (type, count).
    pub most_built: Vec<(BlueprintId, u32)>,
    /// Unit types by the worth lost: (type, count, worth).
    pub most_lost: Vec<(BlueprintId, u32, f32)>,
    /// When the side first finished a builder of each tier (index 0 is tier 1).
    pub tier_at: [Option<u32>; 5],
}

/// A fight: deaths close together in place and time.
#[derive(Clone, Debug)]
pub struct Battle {
    pub from: u32,
    pub to: u32,
    pub at: Vec2,
    /// How far its deaths spread from `at`, metres.
    pub radius: f32,
    pub value: f32,
    /// Worth each side lost in it.
    pub losses: Vec<f32>,
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MomentKind {
    Start,
    FirstBlood,
    Tier,
    Experimental,
    ExperimentalLost,
    Warhead,
    Battle,
    Defeat,
    End,
}

#[derive(Clone, Debug)]
pub struct Moment {
    pub tick: u32,
    pub kind: MomentKind,
    pub side: Option<u8>,
    pub title: String,
    pub detail: String,
    pub at: Option<Vec2>,
}

#[derive(Clone, Debug)]
pub struct Award {
    pub title: &'static str,
    pub side: u8,
    pub figure: String,
    pub blurb: &'static str,
}

pub struct Analysis {
    pub sides: Vec<SideReport>,
    pub size: Vec2,
    /// The match's length in ticks: to the decision, or as far as it went.
    pub length: u32,
    pub winner: Option<u8>,
    /// Seconds into the match of each sample.
    pub times: Vec<f32>,
    /// `curves[metric][side]`, one value per sample.
    curves: Vec<Vec<Vec<f32>>>,
    /// Worth each side lost in each `BIN_TICKS` of the match.
    pub intensity: Vec<Vec<f32>>,
    /// Worth side `[a]` destroyed of side `[b]`.
    pub matrix: Vec<Vec<f32>>,
    pub battles: Vec<Battle>,
    pub moments: Vec<Moment>,
    pub awards: Vec<Award>,
    pub total_destroyed: f32,
    pub total_deaths: u32,
    /// Where everything stood every few seconds, for the battlefield replay.
    pub frames: Vec<Frame>,
    /// Every unit lost, in order, for the battlefield replay.
    pub fallen: Vec<Fallen>,
    pub blasts: Vec<Blast>,
}

/// A unit lost, as the battlefield replay draws it.
#[derive(Clone, Copy, Debug)]
pub struct Fallen {
    pub tick: u32,
    pub pos: Vec2,
    pub owner: u8,
    pub value: f32,
}

/// Ticks per bar of the intensity strip.
pub const BIN_TICKS: u32 = 30 * TICKS_PER_SECOND;

pub fn seconds(tick: u32) -> f32 {
    tick as f32 / TICKS_PER_SECOND as f32
}

/// `12:05`, or `1:02:05` past the hour.
pub fn clock(tick: u32) -> String {
    let s = tick / TICKS_PER_SECOND;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// `950`, `12.4k`, `1.20M`.
pub fn short(v: f32) -> String {
    let a = v.abs();
    if a >= 1e6 {
        format!("{:.2}M", v / 1e6)
    } else if a >= 1e4 {
        format!("{:.0}k", v / 1e3)
    } else if a >= 1e3 {
        format!("{:.1}k", v / 1e3)
    } else if a >= 10.0 || a == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

/// The worth of a unit for the report: its materials.
fn worth(bp: &UnitBlueprint) -> f32 {
    bp.cost_mass.to_f32()
}

fn experimental(bp: &UnitBlueprint) -> bool {
    bp.has(cat::EXPERIMENTAL) || bp.tech >= 4
}

fn spend_kind(bp: &UnitBlueprint) -> usize {
    if experimental(bp) {
        1
    } else if bp.is_mobile() {
        if bp.has(cat::ENGINEER) {
            2
        } else {
            0
        }
    } else if bp.has(cat::EXTRACTOR | cat::POWER | cat::STORAGE | cat::ECONOMY) {
        3
    } else if bp.has(cat::DEFENSE | cat::SHIELD | cat::WALL | cat::STRATEGIC) {
        4
    } else {
        5
    }
}

fn domain(bp: &UnitBlueprint) -> usize {
    if !bp.is_mobile() {
        3
    } else if bp.has(cat::AIR) {
        1
    } else if bp.has(cat::NAVAL) {
        2
    } else {
        0
    }
}

/// Where on the map a point is, in words: "the centre", "the north-east".
pub fn region(at: Vec2, size: Vec2) -> &'static str {
    let d = at / size - 0.5;
    if d.length() < 0.14 {
        return "the centre";
    }
    // The map's y runs north, up the chart (`preview::locate`).
    let (ns, ew) = (
        if d.y > 0.17 {
            1
        } else if d.y < -0.17 {
            2
        } else {
            0
        },
        if d.x < -0.17 {
            1
        } else if d.x > 0.17 {
            2
        } else {
            0
        },
    );
    match (ns, ew) {
        (1, 0) => "the north",
        (2, 0) => "the south",
        (0, 1) => "the west",
        (0, 2) => "the east",
        (1, 1) => "the north-west",
        (1, 2) => "the north-east",
        (2, 1) => "the south-west",
        (2, 2) => "the south-east",
        _ => "the centre",
    }
}

impl Analysis {
    pub fn of(c: &Chronicle, blueprints: &Blueprints) -> Analysis {
        let n = c.sides.len();
        let length = c.ended.map_or(c.tick, |e| e.0).max(1);
        let winner = c.ended.map(|e| e.1);
        let mut a = Analysis {
            sides: c
                .sides
                .iter()
                .map(|s| SideReport {
                    name: s.name.clone(),
                    team: s.team,
                    faction: blueprints
                        .factions
                        .get(s.faction as usize)
                        .map_or_else(String::new, |f| f.name.clone()),
                    ai: s.ai,
                    victor: winner == Some(s.team),
                    ..Default::default()
                })
                .collect(),
            size: c.size,
            length,
            winner,
            times: c.samples.iter().map(|s| seconds(s.tick)).collect(),
            curves: Vec::new(),
            intensity: vec![vec![0.0; n]; (length / BIN_TICKS + 1) as usize],
            matrix: vec![vec![0.0; n]; n],
            battles: Vec::new(),
            moments: Vec::new(),
            awards: Vec::new(),
            total_destroyed: 0.0,
            total_deaths: 0,
            frames: c.frames.clone(),
            fallen: c
                .deaths
                .iter()
                .map(|d| {
                    let bp = blueprints.unit(d.blueprint);
                    Fallen {
                        tick: d.tick,
                        pos: d.pos,
                        owner: d.owner,
                        value: if d.complete { worth(bp) } else { 0.0 },
                    }
                })
                .collect(),
            blasts: c.blasts.clone(),
        };
        for &(tick, side) in &c.defeats {
            if let Some(s) = a.sides.get_mut(side as usize) {
                s.defeated_at.get_or_insert(tick);
            }
        }
        a.economy(c);
        a.combat(c, blueprints);
        a.building(c, blueprints);
        a.curves(c, blueprints);
        a.battles = battles(c, blueprints, n);
        a.moments = moments(&a, c, blueprints);
        a.awards = awards(&a.sides);
        a
    }

    /// Totals of the economy, integrated over the samples.
    fn economy(&mut self, c: &Chronicle) {
        let dt = seconds(SAMPLE_TICKS);
        for sample in &c.samples {
            for (s, v) in self.sides.iter_mut().zip(&sample.sides) {
                if s.defeated_at.is_some_and(|t| sample.tick > t) {
                    continue;
                }
                s.collected += (v.mass_income + v.reclaim_income) * dt;
                s.reclaimed += v.reclaim_income * dt;
                s.energy_collected += v.energy_income * dt;
                s.spent += v.mass_spent * dt;
                s.peak_income = s.peak_income.max(v.mass_income + v.reclaim_income);
                s.efficiency += v.efficiency.clamp(0.0, 1.0);
                if v.efficiency < 0.9 && v.mass_spent + v.energy_spent > 0.0 {
                    s.stalled += dt;
                }
                if v.army_value > s.peak_army {
                    s.peak_army = v.army_value;
                    s.peak_army_at = sample.tick;
                }
            }
        }
        for (i, s) in self.sides.iter_mut().enumerate() {
            let alive = c
                .samples
                .iter()
                .filter(|x| s.defeated_at.is_none_or(|t| x.tick <= t))
                .filter(|x| x.sides.get(i).is_some())
                .count();
            s.efficiency /= alive.max(1) as f32;
        }
    }

    /// Kills, losses and who destroyed what.
    fn combat(&mut self, c: &Chronicle, blueprints: &Blueprints) {
        let n = self.sides.len();
        let mut weapons: Vec<Vec<(BlueprintId, u32, f32)>> = vec![Vec::new(); n];
        for k in &c.kills {
            let (by, victim) = (k.by as usize, k.victim as usize);
            if by >= n || victim >= n || self.sides[by].team == self.sides[victim].team {
                continue;
            }
            let bp = blueprints.unit(k.blueprint);
            let value = if k.complete { worth(bp) } else { 0.0 };
            self.sides[by].kills += 1;
            self.sides[by].destroyed += value;
            self.matrix[by][victim] += value;
            if let Some(w) = k.weapon_of {
                match weapons[by].iter_mut().find(|e| e.0 == w) {
                    Some(e) => {
                        e.1 += 1;
                        e.2 += value;
                    }
                    None => weapons[by].push((w, 1, value)),
                }
            }
        }
        let mut lost: Vec<Vec<(BlueprintId, u32, f32)>> = vec![Vec::new(); n];
        for d in &c.deaths {
            let Some(s) = self.sides.get_mut(d.owner as usize) else {
                continue;
            };
            let bp = blueprints.unit(d.blueprint);
            let value = if d.complete { worth(bp) } else { 0.0 };
            s.losses += 1;
            s.lost += value;
            s.lost_by_domain[domain(bp)] += value;
            self.total_deaths += 1;
            self.total_destroyed += value;
            let bin = (d.tick / BIN_TICKS) as usize;
            if let Some(b) = self.intensity.get_mut(bin) {
                b[d.owner as usize] += value;
            }
            let list = &mut lost[d.owner as usize];
            match list.iter_mut().find(|e| e.0 == d.blueprint) {
                Some(e) => {
                    e.1 += 1;
                    e.2 += value;
                }
                None => list.push((d.blueprint, 1, value)),
            }
        }
        for (s, (mut w, mut l)) in self.sides.iter_mut().zip(weapons.into_iter().zip(lost)) {
            w.sort_by(|a, b| b.2.total_cmp(&a.2).then(b.1.cmp(&a.1)).then(a.0.cmp(&b.0)));
            l.sort_by(|a, b| b.2.total_cmp(&a.2).then(b.1.cmp(&a.1)).then(a.0.cmp(&b.0)));
            s.deadliest = w;
            s.most_lost = l;
        }
    }

    /// What each side built, and when it reached each tier.
    fn building(&mut self, c: &Chronicle, blueprints: &Blueprints) {
        let mut counts: Vec<Vec<(BlueprintId, u32)>> = vec![Vec::new(); self.sides.len()];
        for b in &c.built {
            let Some(s) = self.sides.get_mut(b.owner as usize) else {
                continue;
            };
            let bp = blueprints.unit(b.blueprint);
            s.built += 1;
            s.spend[spend_kind(bp)] += worth(bp);
            let tier = (bp.tech.clamp(1, 5) - 1) as usize;
            if bp.builder.is_some() || experimental(bp) {
                s.tier_at[tier].get_or_insert(b.tick);
            }
            let list = &mut counts[b.owner as usize];
            match list.iter_mut().find(|e| e.0 == b.blueprint) {
                Some(e) => e.1 += 1,
                None => list.push((b.blueprint, 1)),
            }
        }
        for (s, mut list) in self.sides.iter_mut().zip(counts) {
            list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            s.most_built = list;
        }
    }

    /// Every metric's curve over the samples.
    fn curves(&mut self, c: &Chronicle, blueprints: &Blueprints) {
        let n = self.sides.len();
        let dt = seconds(SAMPLE_TICKS);
        // Running totals as the samples go by.
        let mut collected = vec![0.0f32; n];
        let (mut destroyed, mut lost) = (vec![0.0f32; n], vec![0.0f32; n]);
        let (mut ki, mut di) = (0, 0);
        self.curves = vec![vec![Vec::with_capacity(c.samples.len()); n]; Metric::ALL.len()];
        for sample in &c.samples {
            while let Some(k) = c.kills.get(ki).filter(|k| k.tick <= sample.tick) {
                let (by, victim) = (k.by as usize, k.victim as usize);
                if by < n
                    && victim < n
                    && self.sides[by].team != self.sides[victim].team
                    && k.complete
                {
                    destroyed[by] += worth(blueprints.unit(k.blueprint));
                }
                ki += 1;
            }
            while let Some(d) = c.deaths.get(di).filter(|d| d.tick <= sample.tick) {
                if (d.owner as usize) < n && d.complete {
                    lost[d.owner as usize] += worth(blueprints.unit(d.blueprint));
                }
                di += 1;
            }
            for i in 0..n {
                let v = sample.sides.get(i).copied().unwrap_or_default();
                let gone = self.sides[i].defeated_at.is_some_and(|t| sample.tick > t);
                if !gone {
                    collected[i] += (v.mass_income + v.reclaim_income) * dt;
                }
                let live = |x: f32| if gone { 0.0 } else { x };
                let values = [
                    live(v.mass_income + v.reclaim_income),
                    live(v.energy_income),
                    collected[i],
                    live(v.mass_spent),
                    live(v.mass),
                    live(v.efficiency.clamp(0.0, 1.0) * 100.0),
                    live(v.army_value),
                    live(v.army as f32),
                    destroyed[i],
                    lost[i],
                ];
                for (m, value) in values.into_iter().enumerate() {
                    self.curves[m][i].push(value);
                }
            }
        }
    }

    pub fn curve(&self, metric: Metric, side: usize) -> &[f32] {
        let m = Metric::ALL.iter().position(|&x| x == metric).unwrap_or(0);
        self.curves
            .get(m)
            .and_then(|c| c.get(side))
            .map_or(&[], |v| v.as_slice())
    }

    /// The teams in the match, in order of their first side.
    pub fn teams(&self) -> Vec<u8> {
        let mut teams: Vec<u8> = Vec::new();
        for s in &self.sides {
            if !teams.contains(&s.team) {
                teams.push(s.team);
            }
        }
        teams
    }

    /// A side's index on its own team, for naming a team after its first side.
    pub fn team_name(&self, team: u8) -> String {
        let members: Vec<&SideReport> = self.sides.iter().filter(|s| s.team == team).collect();
        match members.as_slice() {
            [one] => one.name.clone(),
            _ => format!("Team {}", team + 1),
        }
    }
}

/// Deaths close in place and time, gathered into fights, the biggest first.
fn battles(c: &Chronicle, blueprints: &Blueprints, n: usize) -> Vec<Battle> {
    // Cells a side of the map is cut into, and ticks a bucket covers.
    const CELLS: usize = 10;
    const SPAN: u32 = 30 * TICKS_PER_SECOND;
    let worth_of = |d: &Death| {
        if d.complete {
            worth(blueprints.unit(d.blueprint))
        } else {
            0.0
        }
    };
    let cell = |d: &Death| {
        let k = (d.pos / c.size * CELLS as f32).clamp(Vec2::ZERO, Vec2::splat(CELLS as f32 - 1.0));
        (k.x as i32, k.y as i32)
    };
    // (window, x, y) -> worth, in a list kept sorted by key for lookups.
    let mut buckets: Vec<((i32, i32, i32), f32)> = Vec::new();
    for d in &c.deaths {
        let v = worth_of(d);
        if v <= 0.0 {
            continue;
        }
        let (x, y) = cell(d);
        let key = ((d.tick / SPAN) as i32, x, y);
        match buckets.binary_search_by(|b| b.0.cmp(&key)) {
            Ok(i) => buckets[i].1 += v,
            Err(i) => buckets.insert(i, (key, v)),
        }
    }
    let total: f32 = buckets.iter().map(|b| b.1).sum();
    let floor = (total * 0.04).max(400.0);
    let mut claimed = vec![false; buckets.len()];
    let mut order: Vec<usize> = (0..buckets.len()).collect();
    order.sort_by(|&a, &b| buckets[b].1.total_cmp(&buckets[a].1).then(a.cmp(&b)));
    let mut found = Vec::new();
    for start in order {
        if claimed[start] || buckets[start].1 < floor {
            continue;
        }
        // Flood out to neighbouring buckets in place and time.
        let mut members = vec![start];
        claimed[start] = true;
        let mut k = 0;
        while k < members.len() {
            let (w, x, y) = buckets[members[k]].0;
            for dw in -1..=1 {
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let key = (w + dw, x + dx, y + dy);
                        if let Ok(i) = buckets.binary_search_by(|b| b.0.cmp(&key)) {
                            if !claimed[i] {
                                claimed[i] = true;
                                members.push(i);
                            }
                        }
                    }
                }
            }
            k += 1;
        }
        let keys: Vec<(i32, i32, i32)> = members.iter().map(|&i| buckets[i].0).collect();
        let mut b = Battle {
            from: u32::MAX,
            to: 0,
            at: Vec2::ZERO,
            radius: 0.0,
            value: 0.0,
            losses: vec![0.0; n],
            name: String::new(),
        };
        let inside: Vec<&Death> = c
            .deaths
            .iter()
            .filter(|d| {
                let (x, y) = cell(d);
                keys.contains(&((d.tick / SPAN) as i32, x, y)) && worth_of(d) > 0.0
            })
            .collect();
        for d in &inside {
            let v = worth_of(d);
            b.from = b.from.min(d.tick);
            b.to = b.to.max(d.tick);
            b.at += d.pos * v;
            b.value += v;
            if let Some(l) = b.losses.get_mut(d.owner as usize) {
                *l += v;
            }
        }
        // A fight takes a few losses: a lone death (a commander sniped) is not a battle.
        if b.value <= 0.0 || inside.len() < 4 {
            continue;
        }
        b.at /= b.value;
        b.radius = inside
            .iter()
            .map(|d| d.pos.distance(b.at) * worth_of(d))
            .sum::<f32>()
            / b.value;
        found.push(b);
    }
    found.sort_by(|a, b| b.value.total_cmp(&a.value).then(a.from.cmp(&b.from)));
    // The report names the six biggest: past that they are skirmishes (a cosmetic cap).
    found.truncate(6);
    for (i, b) in found.iter_mut().enumerate() {
        b.name = format!(
            "{} battle in {}",
            [
                "The great",
                "A second",
                "A third",
                "A fourth",
                "A fifth",
                "A sixth"
            ][i],
            region(b.at, c.size)
        );
    }
    found
}

/// The match's turning points, in order.
fn moments(a: &Analysis, c: &Chronicle, blueprints: &Blueprints) -> Vec<Moment> {
    let mut out = vec![Moment {
        tick: 0,
        kind: MomentKind::Start,
        side: None,
        title: "Commanders deployed".into(),
        detail: format!("{} sides take the field", a.sides.len()),
        at: None,
    }];
    let team_of = |s: u8| a.sides.get(s as usize).map(|x| x.team);
    if let Some(k) = c
        .kills
        .iter()
        .find(|k| k.complete && team_of(k.by) != team_of(k.victim))
    {
        let victim = &blueprints.unit(k.blueprint).name;
        let by = a.sides.get(k.by as usize).map_or("", |s| s.name.as_str());
        let at = c
            .deaths
            .iter()
            .find(|d| d.tick >= k.tick && d.blueprint == k.blueprint && d.owner == k.victim)
            .map(|d| d.pos);
        out.push(Moment {
            tick: k.tick,
            kind: MomentKind::FirstBlood,
            side: Some(k.by),
            title: "First blood".into(),
            detail: format!("{by} destroys a {victim}"),
            at,
        });
    }
    for (i, s) in a.sides.iter().enumerate() {
        for tier in 1..3 {
            if let Some(t) = s.tier_at[tier] {
                out.push(Moment {
                    tick: t,
                    kind: MomentKind::Tier,
                    side: Some(i as u8),
                    title: format!("Tech {}", tier + 1),
                    detail: format!("{} reaches tier {}", s.name, tier + 1),
                    at: None,
                });
            }
        }
    }
    arsenal(a, c, blueprints, &mut out);
    for b in &a.battles {
        let worst = b
            .losses
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.total_cmp(y.1))
            .map(|(i, _)| i);
        out.push(Moment {
            tick: b.from,
            kind: MomentKind::Battle,
            side: None,
            title: capitalise(&b.name),
            detail: format!(
                "{} materials destroyed over {}{}",
                short(b.value),
                span(b.to - b.from),
                worst
                    .and_then(|i| a.sides.get(i))
                    .map_or(String::new(), |s| format!(", {} hit hardest", s.name))
            ),
            at: Some(b.at),
        });
    }
    for (i, s) in a.sides.iter().enumerate() {
        if let Some(t) = s.defeated_at {
            let at = c
                .deaths
                .iter()
                .find(|d| {
                    d.tick == t
                        && d.owner as usize == i
                        && blueprints.unit(d.blueprint).has(cat::COMMANDER)
                })
                .map(|d| d.pos);
            out.push(Moment {
                tick: t,
                kind: MomentKind::Defeat,
                side: Some(i as u8),
                title: format!("{} defeated", s.name),
                detail: if at.is_some() {
                    format!("{}'s commander is destroyed", s.name)
                } else {
                    format!("{} leaves the field", s.name)
                },
                at,
            });
        }
    }
    if let Some(team) = a.winner {
        out.push(Moment {
            tick: a.length,
            kind: MomentKind::End,
            side: a.sides.iter().position(|s| s.team == team).map(|i| i as u8),
            title: format!("{} holds the field", a.team_name(team)),
            detail: format!("The match is decided after {}", span(a.length)),
            at: None,
        });
    }
    out.sort_by_key(|m| m.tick);
    out
}

/// The big machines fielded and lost, and the warheads that landed.
fn arsenal(a: &Analysis, c: &Chronicle, blueprints: &Blueprints, out: &mut Vec<Moment>) {
    // Each side's first of each experimental, and every experimental lost.
    let mut seen: Vec<(u8, BlueprintId)> = Vec::new();
    for b in &c.built {
        let bp = blueprints.unit(b.blueprint);
        if experimental(bp) && !seen.contains(&(b.owner, b.blueprint)) {
            seen.push((b.owner, b.blueprint));
            let name = a
                .sides
                .get(b.owner as usize)
                .map_or("", |s| s.name.as_str());
            out.push(Moment {
                tick: b.tick,
                kind: MomentKind::Experimental,
                side: Some(b.owner),
                title: format!("{} fielded", bp.name),
                detail: format!(
                    "{name} completes a tier {} {}",
                    bp.tech,
                    bp.role.to_lowercase()
                ),
                at: None,
            });
        }
    }
    for d in c.deaths.iter().filter(|d| d.complete) {
        let bp = blueprints.unit(d.blueprint);
        if experimental(bp) {
            let name = a
                .sides
                .get(d.owner as usize)
                .map_or("", |s| s.name.as_str());
            out.push(Moment {
                tick: d.tick,
                kind: MomentKind::ExperimentalLost,
                side: Some(d.owner),
                title: format!("{} destroyed", bp.name),
                detail: format!(
                    "{name} loses {} materials of experimental",
                    short(worth(bp))
                ),
                at: Some(d.pos),
            });
        }
    }
    for b in &c.blasts {
        let name = a
            .sides
            .get(b.owner as usize)
            .map_or("", |s| s.name.as_str());
        out.push(Moment {
            tick: b.tick,
            kind: MomentKind::Warhead,
            side: Some(b.owner),
            title: "Nuclear strike".into(),
            detail: format!("{name}'s warhead lands in {}", region(b.pos, c.size)),
            at: Some(b.pos),
        });
    }
}

/// `4 min 10 s`, `40 s`.
pub fn span(ticks: u32) -> String {
    let s = ticks / TICKS_PER_SECOND;
    match (s / 60, s % 60) {
        (0, s) => format!("{s} s"),
        (m, 0) => format!("{m} min"),
        (m, s) => format!("{m} min {s} s"),
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// Honours for the side best at each thing, when there is more than one side to beat.
fn awards(sides: &[SideReport]) -> Vec<Award> {
    if sides.len() < 2 {
        return Vec::new();
    }
    let best = |key: &dyn Fn(&SideReport) -> f32| {
        sides
            .iter()
            .enumerate()
            .map(|(i, s)| (i, key(s)))
            .filter(|(_, v)| *v > 0.0)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    };
    let mut out = Vec::new();
    let mut give =
        |title, blurb, key: &dyn Fn(&SideReport) -> f32, figure: &dyn Fn(f32) -> String| {
            if let Some((i, v)) = best(key) {
                out.push(Award {
                    title,
                    side: i as u8,
                    figure: figure(v),
                    blurb,
                });
            }
        };
    give(
        "Warlord",
        "Most enemy worth destroyed",
        &|s| s.destroyed,
        &|v| format!("{} destroyed", short(v)),
    );
    give(
        "Industrialist",
        "Most materials collected",
        &|s| s.collected,
        &|v| format!("{} collected", short(v)),
    );
    give(
        "Grand Army",
        "Largest army in the field",
        &|s| s.peak_army,
        &|v| format!("{} at its peak", short(v)),
    );
    give(
        "Clean Trades",
        "Best worth destroyed for worth lost",
        &|s| {
            if s.destroyed > 0.0 {
                s.destroyed / s.lost.max(1.0)
            } else {
                0.0
            }
        },
        &|v| format!("{v:.1} to 1"),
    );
    give(
        "Salvager",
        "Most materials reclaimed",
        &|s| s.reclaimed,
        &|v| format!("{} reclaimed", short(v)),
    );
    give("Architect", "Most built", &|s| s.built as f32, &|v| {
        format!("{v:.0} units and structures")
    });
    out
}
