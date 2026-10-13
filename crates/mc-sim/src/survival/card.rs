//! What a Shaper shows: one `ShaperInstance` a Shaper in the render frame
//! (`RenderFrame::shapers`, for its model and effects), and the selection
//! card's figures (`World::shaper_card`): what it builds, its level and tier,
//! its base, and its shield, Lance and anti-nuke at its level. Presentation
//! only: nothing here writes back.

use super::rules::{
    launch_gap_ticks, pylon_feed, shaper_batch, shaper_height, shaper_silos, shaper_tier,
    shell_blast, shells_per_volley, volley_ticks,
};
use super::{Node, PieceKind, RAISE_TICKS};
use crate::{Handle, UnitId};
use mc_data::survival::{Domain, SiteKind};
use mc_data::BlueprintId;

/// A Shaper as the renderer draws it (`RenderFrame::shapers`). Its unit is in
/// `RenderFrame::units` as any other (`unit` is `UnitInstance::unit_id`).
#[derive(Clone, Debug, Default)]
#[expect(
    clippy::disallowed_types,
    reason = "presentation: the renderer draws Shapers from it"
)]
pub struct ShaperInstance {
    pub unit: u32,
    /// `SiteKind` as a number: 0 printer, 1 artillery, 2 nuke.
    pub kind: u8,
    pub level: u16,
    /// Visual tier (`shaper_tier`): a stage, collar or ring more each.
    pub tier: u8,
    /// Metres it is built up to now, climbing toward its level's height.
    pub height: f32,
    /// 0..1 through the growth step it is building (1 once it stands at its level's height).
    pub growing: f32,
    /// 0..1 while the ray raises it; 1 once it is online.
    pub raised: f32,
    /// 0..1 while its Lance charges; 0 cooling or idle.
    pub lance_charge: f32,
    /// Where the Lance aims (the target's middle); zero with nothing in reach.
    pub lance_target: [f32; 3],
    /// The shield's visual tier, by the Shaper's level.
    pub shield_tier: u8,
    /// An auxiliary Shaper: 0..1 through the beat to its next volley (a Seedcaster) or
    /// launch (an Unmaker), for a charge-up look; 0 for a printer.
    pub fire_charge: f32,
    /// Metres from its middle to the edge of its foot, as the sim blocks it now
    /// (`rules::shaper_foot` of its built height, whole path cells; it waits on a
    /// structure in the way). The model's footprint matches it.
    pub foot: f32,
}

#[expect(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    reason = "presentation: where the renderer and the sounds put its Lens"
)]
impl ShaperInstance {
    /// Metres over its foot of its Lens, where its Lance charges and strikes from: near
    /// the top of what is built so far (a printing Shaper's braced blades carry it at
    /// 0.86 of their height, an auxiliary's spire at 0.92).
    pub fn lens_height(&self) -> f32 {
        let share = if self.kind == SiteKind::Printer as u8 {
            0.86
        } else {
            0.92
        };
        self.height.max(20.0) * share
    }
}

/// A piece of a Shaper's base, for the card and the interface.
#[derive(Clone, Copy, Debug)]
pub struct PieceView {
    pub unit: UnitId,
    pub kind: PieceKind,
    pub raised: bool,
}

/// A Shaper's shield at its level, and what it holds now.
#[derive(Clone, Debug)]
#[expect(clippy::disallowed_types, reason = "presentation: the selection card")]
pub struct ShieldCard {
    pub radius: f32,
    pub health: f32,
    /// Hit points it holds now.
    pub charge: f32,
    /// Hit points a second it regains while up, its own.
    pub regen: f32,
    /// Hit points a second its pylons add.
    pub feed: f32,
}

/// A Shaper's Lance at its level.
#[derive(Clone, Debug)]
#[expect(clippy::disallowed_types, reason = "presentation: the selection card")]
pub struct LanceCard {
    pub charge_secs: f32,
    pub reload_secs: f32,
    /// Strikes a minute while it has targets.
    pub fire_rate: f32,
    pub damage: f32,
    /// Damage a second on one target: a strike per charge and reload.
    pub dps: f32,
    /// Metres round the mark a strike reaches.
    pub splash: f32,
    pub range: f32,
    /// 0..1 through the charge now.
    pub charging: f32,
}

/// A Shaper's anti-nuke at its level.
#[derive(Clone, Debug)]
#[expect(clippy::disallowed_types, reason = "presentation: the selection card")]
pub struct AntiNukeCard {
    /// Interceptors ready.
    pub stock: u8,
    /// The most it holds.
    pub capacity: u8,
    /// Metres round it a warhead coming down is shot at.
    pub coverage: f32,
    /// Seconds one interceptor takes to assemble.
    pub round_secs: f32,
}

/// A Seedcaster's replication artillery at its level (`artillery.rs`).
#[derive(Clone, Debug)]
#[expect(clippy::disallowed_types, reason = "presentation: the selection card")]
pub struct ArtilleryCard {
    /// Seconds between volleys, and until the next.
    pub interval_secs: f32,
    pub next_in_secs: f32,
    pub shells_per_volley: u16,
    /// Units one shell prints where it lands: light (T1-T3), heavies; and their highest tech.
    pub squad: (u16, u16),
    pub squad_tech: u8,
    /// Its burst where it lands: reach (m) and damage.
    pub blast_radius: f32,
    pub blast_damage: f32,
    /// Its shells in the air now.
    pub in_flight: u16,
    /// Its last volley was held back by the field cap: the field is full.
    pub held: bool,
}

/// An Unmaker's warheads at its level (`warheads.rs`): all its silos together.
#[derive(Clone, Debug)]
#[expect(clippy::disallowed_types, reason = "presentation: the selection card")]
pub struct WarheadCard {
    /// Silos standing (raised), and how many its level calls for.
    pub silos: u8,
    pub silos_wanted: u8,
    /// Warheads ready across its silos, and the most they hold.
    pub stock: u16,
    pub capacity: u16,
    /// Seconds one silo takes to assemble a warhead.
    pub round_secs: f32,
    /// Seconds between two of its launches, and until it may launch again.
    pub gap_secs: f32,
    pub next_in_secs: f32,
    /// The blast: reach and core (m), and damage in the core.
    pub blast_radius: f32,
    pub blast_core: f32,
    pub blast_damage: f32,
    /// Its warheads in the air now.
    pub in_flight: u16,
}

/// Everything a Shaper's selection card shows (`World::shaper_card`).
#[derive(Clone, Debug)]
#[expect(clippy::disallowed_types, reason = "presentation: the selection card")]
pub struct ShaperCard {
    pub unit: UnitId,
    pub site: u8,
    pub kind: SiteKind,
    /// What it prints for.
    pub domain: Domain,
    pub level: u16,
    pub tier: u8,
    /// The level the round asks of a Shaper (`SurvivalRules::catch_up_level`):
    /// one below it evolves quickly up to it.
    pub catch_up: u16,
    /// Metres it is built up to now, and its level's height.
    pub height: f32,
    pub full_height: f32,
    /// Metres from its middle to the edge of its foot now (`ShaperInstance::foot`).
    pub foot: f32,
    /// 0..1 while the ray raises it; 1 online.
    pub raised: f32,
    /// What it builds, the first kind first.
    pub products: Vec<BlueprintId>,
    /// Units a formation of its holds: light (T1-T3), heavies.
    pub batch: (u16, u16),
    pub printed: u32,
    /// Seconds until it prints its next formation (0: as soon as it has one).
    pub next_print: f32,
    /// Its base: platforms, great platforms and pylons, standing or being printed.
    pub pieces: Vec<PieceView>,
    pub shield: Option<ShieldCard>,
    pub lance: Option<LanceCard>,
    pub anti_nuke: Option<AntiNukeCard>,
    /// A Seedcaster's artillery; None for any other Shaper.
    pub artillery: Option<ArtilleryCard>,
    /// An Unmaker's warheads; None for any other Shaper.
    pub warheads: Option<WarheadCard>,
}

/// 0..1 through the growth step a Shaper `height` metres tall is building at
/// `level` (1 once it stands at its level's height).
#[expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "presentation: ShaperInstance::growing"
)]
fn growing(height: mc_core::Fx, level: u16) -> f32 {
    if height >= shaper_height(level) {
        return 1.0;
    }
    let mut step = 1;
    while step < level && shaper_height(step + 1) <= height {
        step += 1;
    }
    let (lo, hi) = (shaper_height(step), shaper_height(step + 1));
    ((height - lo) / (hi - lo).max(mc_core::Fx::ONE))
        .to_f32()
        .clamp(0.0, 1.0)
}

impl crate::World {
    /// 0..1 of the ray's raising of `n` (1 online).
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        reason = "presentation: the Shaper's instance and card"
    )]
    fn raised_share(&self, n: &Node) -> f32 {
        if n.raised {
            1.0
        } else {
            ((self.state.tick - n.started) as f32 / RAISE_TICKS as f32).min(0.999)
        }
    }

    /// The visual tier (`shaper_tier`) of the Shaper `unit`; None for any other unit.
    pub(crate) fn shaper_tier_of(&self, unit: UnitId) -> Option<u8> {
        let s = self.state.survival.as_ref()?;
        s.nodes
            .iter()
            .find(|n| n.unit == unit)
            .map(|n| shaper_tier(n.level))
    }

    /// Every standing Shaper, for the renderer. They are never hidden from the
    /// defenders (`survival_reveal`).
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: fills the render frame's Shaper list"
    )]
    pub(crate) fn write_shapers(&self, out: &mut Vec<ShaperInstance>) {
        out.clear();
        let Some(s) = self.state.survival.as_ref() else {
            return;
        };
        for n in &s.nodes {
            let Some(row) = self.state.units.row(n.unit) else {
                continue;
            };
            let kind = s
                .config
                .node_sites
                .get(n.site as usize)
                .map_or(SiteKind::Printer, |site| site.kind);
            let lance = self.state.lances.by_unit.get(&n.unit);
            let spec = self.lance_spec(row);
            let (charge, target) = match (lance, spec) {
                (Some(c), Some(spec)) if c.target != Handle::NONE && c.cooling == 0 => (
                    (c.charged as f32 / spec.charge_ticks.max(1) as f32).min(1.0),
                    c.aim.to_f32(),
                ),
                _ => (0.0, [0.0; 3]),
            };
            let tier = shaper_tier(n.level);
            out.push(ShaperInstance {
                unit: n.unit.0,
                kind: kind as u8,
                level: n.level,
                tier,
                height: n.height.to_f32(),
                growing: growing(n.height, n.level),
                raised: self.raised_share(n),
                lance_charge: charge,
                lance_target: target,
                shield_tier: tier,
                fire_charge: self.fire_charge(n, kind),
                foot: n.foot.to_f32(),
            });
        }
    }

    /// The selection card's figures for the Shaper `unit`; None for any other unit.
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: the Shaper's selection card"
    )]
    pub fn shaper_card(&self, unit: UnitId) -> Option<ShaperCard> {
        let s = self.state.survival.as_ref()?;
        let n = s.nodes.iter().find(|n| n.unit == unit)?;
        let row = self.state.units.row(unit)?;
        let site = s.config.node_sites.get(n.site as usize);
        let bps = self.blueprints.as_ref();
        let light = n.products.iter().filter(|p| bps.unit(**p).tech < 4);
        let light_tech = light.clone().map(|p| bps.unit(*p).tech).max().unwrap_or(1);
        let has_heavy = n.products.iter().any(|p| bps.unit(*p).tech >= 4);
        let (nl, nh) = shaper_batch(n.level, light_tech, has_heavy);
        let pylons = n
            .pieces
            .iter()
            .filter(|p| p.kind == PieceKind::Pylon && p.raised)
            .count();
        let tps = mc_core::TICKS_PER_SECOND as f32;
        let stats = self.grown_stats(row);
        let shield = stats.shield.map(|sh| ShieldCard {
            radius: sh.radius.to_f32(),
            health: sh.health.to_f32(),
            charge: self.state.units.shield_hp[row].to_f32(),
            regen: sh.regen.to_f32(),
            feed: pylon_feed(sh.regen, pylons).to_f32(),
        });
        let charging = self
            .state
            .lances
            .by_unit
            .get(&unit)
            .map_or(0, |c| c.charged);
        let lance = stats.lance.map(|l| {
            let charge_secs = l.charge_ticks as f32 / tps;
            let reload_secs = l.reload_ticks as f32 / tps;
            let cycle = (charge_secs + reload_secs).max(0.1);
            LanceCard {
                charge_secs,
                reload_secs,
                fire_rate: 60.0 / cycle,
                damage: l.damage.to_f32(),
                dps: l.damage.to_f32() / cycle,
                splash: l.splash.to_f32(),
                range: l.range.to_f32(),
                charging: (charging as f32 / l.charge_ticks.max(1) as f32).min(1.0),
            }
        });
        let anti_nuke = self.launcher_spec(row).and_then(|spec| {
            let (capacity, coverage, round) = stats.interceptors?;
            let power = spec.power.to_f32().max(0.001);
            Some(AntiNukeCard {
                stock: self
                    .state
                    .strategic
                    .launchers
                    .get(&unit)
                    .map_or(0, |l| l.stock),
                capacity,
                coverage: coverage.to_f32(),
                round_secs: round.to_f32() / power,
            })
        });
        Some(ShaperCard {
            unit,
            site: n.site,
            kind: site.map_or(SiteKind::Printer, |s| s.kind),
            domain: n.domain,
            level: n.level,
            tier: shaper_tier(n.level),
            catch_up: s.config.rules.catch_up_level(s.round.max(1)),
            height: n.height.to_f32(),
            full_height: shaper_height(n.level).to_f32(),
            foot: n.foot.to_f32(),
            raised: self.raised_share(n),
            products: n.products.clone(),
            batch: (
                nl.min(u16::MAX as usize) as u16,
                nh.min(u16::MAX as usize) as u16,
            ),
            printed: n.printed,
            next_print: n.next_print.saturating_sub(self.state.tick) as f32 / tps,
            pieces: n
                .pieces
                .iter()
                .map(|p| PieceView {
                    unit: p.unit,
                    kind: p.kind,
                    raised: p.raised,
                })
                .collect(),
            shield,
            lance,
            anti_nuke,
            artillery: (site.map(|s| s.kind) == Some(SiteKind::Artillery))
                .then(|| self.artillery_card(n)),
            warheads: (site.map(|s| s.kind) == Some(SiteKind::Nuke)).then(|| self.warhead_card(n)),
        })
    }

    /// 0..1 through an auxiliary Shaper's beat to its next volley or launch.
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        reason = "presentation: ShaperInstance::fire_charge"
    )]
    fn fire_charge(&self, n: &Node, kind: SiteKind) -> f32 {
        let beat = match kind {
            SiteKind::Printer => return 0.0,
            SiteKind::Artillery => volley_ticks(n.level),
            SiteKind::Nuke => launch_gap_ticks(n.level),
        };
        if !n.raised {
            return 0.0;
        }
        let left = n.next_fire.saturating_sub(self.state.tick).min(beat);
        1.0 - left as f32 / beat.max(1) as f32
    }

    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: the Seedcaster's selection card"
    )]
    fn artillery_card(&self, n: &Node) -> ArtilleryCard {
        let s = self.state.survival.as_ref().unwrap();
        let tps = mc_core::TICKS_PER_SECOND as f32;
        let i = s.nodes.iter().position(|m| m.unit == n.unit).unwrap_or(0);
        let squad = self.shell_squad_of(i);
        let bps = self.blueprints.as_ref();
        let heavy = squad.iter().filter(|b| bps.unit(**b).tech >= 4).count();
        let (radius, damage) = shell_blast(n.level);
        let in_flight = s
            .shells
            .flying
            .iter()
            .filter(|sh| sh.source == n.unit)
            .count();
        ArtilleryCard {
            interval_secs: volley_ticks(n.level) as f32 / tps,
            next_in_secs: n.next_fire.saturating_sub(self.state.tick) as f32 / tps,
            shells_per_volley: shells_per_volley(n.level).min(u16::MAX as usize) as u16,
            squad: (
                (squad.len() - heavy).min(u16::MAX as usize) as u16,
                heavy.min(u16::MAX as usize) as u16,
            ),
            squad_tech: squad.iter().map(|b| bps.unit(*b).tech).max().unwrap_or(1),
            blast_radius: radius.to_f32(),
            blast_damage: damage.to_f32(),
            in_flight: in_flight.min(u16::MAX as usize) as u16,
            held: n.held,
        }
    }

    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: the Unmaker's selection card"
    )]
    fn warhead_card(&self, n: &Node) -> WarheadCard {
        let tps = mc_core::TICKS_PER_SECOND as f32;
        let silos: Vec<UnitId> = n
            .pieces
            .iter()
            .filter(|p| p.kind == PieceKind::Silo && p.raised)
            .map(|p| p.unit)
            .collect();
        let (mut stock, mut capacity) = (0u16, 0u16);
        let mut spec = None;
        for id in &silos {
            let Some(row) = self.state.units.row(*id) else {
                continue;
            };
            let held = self
                .state
                .strategic
                .launchers
                .get(id)
                .map_or(0, |l| l.stock);
            stock = stock.saturating_add(held as u16);
            if let Some(s) = self.launcher_spec(row) {
                capacity = capacity.saturating_add(s.stock as u16);
                spec = Some(s.into_owned());
            }
        }
        // With no silo standing yet, what one raised at its level would be.
        let spec = spec.or_else(|| {
            let s = self.state.survival.as_ref()?;
            self.blueprints.unit(s.kit.silo).strategic.clone()
        });
        let blast = spec.as_ref().and_then(|s| s.blast);
        let owner = self.state.survival.as_ref().map(|s| s.config.engine_player);
        let in_flight = self
            .state
            .strategic
            .missiles
            .iter()
            .filter(|m| {
                m.kind == crate::nukes::MissileKind::Warhead
                    && silos.contains(&m.source)
                    && Some(m.owner) == owner
            })
            .count();
        WarheadCard {
            silos: silos.len().min(u8::MAX as usize) as u8,
            silos_wanted: shaper_silos(n.level).min(u8::MAX as usize) as u8,
            stock,
            capacity,
            round_secs: spec.as_ref().map_or(0.0, |s| s.round_seconds()),
            gap_secs: launch_gap_ticks(n.level) as f32 / tps,
            next_in_secs: n.next_fire.saturating_sub(self.state.tick) as f32 / tps,
            blast_radius: blast.map_or(0.0, |b| b.radius.to_f32()),
            blast_core: blast.map_or(0.0, |b| b.core.to_f32()),
            blast_damage: blast.map_or(0.0, |b| b.damage.to_f32()),
            in_flight: in_flight.min(u16::MAX as usize) as u16,
        }
    }
}
