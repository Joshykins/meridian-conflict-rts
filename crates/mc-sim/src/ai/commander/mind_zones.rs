//! The zones for an observer (`docs/AI_COMMANDER.md`, "Overlay"): the match's zone
//! outline, what a Commander believes each zone holds, and what each one really holds,
//! so the two can be laid side by side. Read-only: nothing here goes back into the sim.
use super::knowledge::{refresh, Built, Holding, ZoneKnowledge, BUILT, BUILT_KINDS};
use super::reach::NODE;
use super::zones::{Ground, Zones, NONE};
use crate::tables::Controller;
use crate::World;
use mc_core::Fx;
use std::sync::Arc;

/// No zone, in `ZoneMapView::id`.
pub const NO_ZONE: u16 = NONE;

/// One zone's place on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneShape {
    /// Metres.
    pub centre: (i32, i32),
    pub water: bool,
    /// Ore fields in it.
    pub ore: u16,
    /// Zones on the same ground it opens onto.
    pub adj: Vec<u16>,
}

/// The match's zones, the same for every side.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneMapView {
    /// Metres a side of a node.
    pub node: i32,
    /// Nodes across and down, and the zone of each, row by row (`NO_ZONE` for none).
    pub w: i32,
    pub h: i32,
    pub id: Vec<u16>,
    pub zones: Vec<ZoneShape>,
}

impl ZoneMapView {
    fn of(z: &Zones) -> ZoneMapView {
        let (w, h, id) = z.grid();
        ZoneMapView {
            node: NODE,
            w,
            h,
            id: id.to_vec(),
            zones: z
                .zones
                .iter()
                .map(|z| ZoneShape {
                    centre: (z.centre.x.floor_int(), z.centre.y.floor_int()),
                    water: z.ground == Ground::Water,
                    ore: z.ore,
                    adj: z.adj.clone(),
                })
                .collect(),
        }
    }
}

/// What one player holds in a zone: what a side believes of an enemy, or the truth.
#[derive(Clone, Debug, PartialEq)]
pub struct MindHolding {
    pub player: u8,
    /// Structures by kind with any standing: name, mass, count.
    pub built: Vec<(&'static str, i32, u16)>,
    /// How sure (0..=100) the structures are still there.
    pub built_sure: i32,
    /// Army mass seen, and believed there now.
    pub army_seen: i32,
    pub army: i32,
    pub army_sure: i32,
    pub builders: u16,
    /// Their commander stands there.
    pub king: bool,
    /// What the zone is to them: "main base", "forward base", "expansion", "firebase",
    /// "army", "builders" or "outpost".
    pub tag: &'static str,
    /// Seconds since the structures, and the army, were last seen (0 for the truth).
    pub built_ago: u32,
    pub army_ago: u32,
}

/// What a side believes of one zone.
#[derive(Clone, Debug, PartialEq)]
pub struct MindZone {
    pub zone: u16,
    /// Seconds since it was last mostly in view; `None`: never.
    pub seen_ago: Option<u32>,
    /// Per mille of it in view now.
    pub view: u16,
    /// Enemy units on radar there it cannot name.
    pub blips: u16,
    pub held: Vec<MindHolding>,
}

/// One of the side's fronts: where its ground meets an enemy's (`fronts.rs`).
#[derive(Clone, Debug, PartialEq)]
pub struct MindFront {
    pub id: u32,
    pub enemy: u8,
    pub water: bool,
    /// Its zones, and their zones beside it: the way through to them.
    pub zones: Vec<u16>,
    pub entries: Vec<u16>,
    /// Mass at stake behind it, ours and theirs.
    pub stake_us: i32,
    pub stake_them: i32,
    /// Armed mass near it: ours, and theirs as believed.
    pub ours: i32,
    pub theirs: i32,
    /// Zones from it to their most valuable ground.
    pub depth: u16,
    /// Seconds since it was first found.
    pub age: u32,
}

/// Mass of guns, anti-air, artillery and shields at which a zone without factories
/// is a firebase.
const FIREBASE: i32 = 1500;
/// Army mass believed in a zone that makes it the army's place.
const ARMY: i32 = 800;

fn tag(h: &Holding, start: bool, tick: u32) -> &'static str {
    let b = |k: Built| h.built[k as usize].floor_int();
    let factories = b(Built::Factory) + b(Built::Airfield) + b(Built::Shipyard);
    let guns = b(Built::Guns) + b(Built::AntiAir) + b(Built::Artillery) + b(Built::Shield);
    let built = h.built_mass().floor_int();
    if start && built > 0 || factories > 0 && h.king_here(tick) {
        "main base"
    } else if factories > 0 {
        "forward base"
    } else if guns >= FIREBASE {
        "firebase"
    } else if b(Built::Mine) + b(Built::Power) > 0
        && (b(Built::Mine) + b(Built::Power)) * 2 >= built
    {
        "expansion"
    } else if h.army_now(tick).floor_int() >= ARMY {
        "army"
    } else if h.builders > 0 && built == 0 {
        "builders"
    } else {
        "outpost"
    }
}

fn holding(h: &Holding, start: bool, tick: u32) -> MindHolding {
    let army_seen: Fx = h.army.iter().copied().sum();
    MindHolding {
        player: h.player,
        built: (0..BUILT)
            .filter(|&k| h.built[k] > Fx::ZERO)
            .map(|k| (BUILT_KINDS[k].name(), h.built[k].floor_int(), h.count[k]))
            .collect(),
        built_sure: h.built_sure(tick),
        army_seen: army_seen.floor_int(),
        army: h.army_now(tick).floor_int(),
        army_sure: h.army_sure(tick),
        builders: h.builders,
        king: h.king_here(tick),
        tag: tag(h, start, tick),
        built_ago: tick.saturating_sub(h.built_at) / 10,
        army_ago: tick.saturating_sub(h.army_at) / 10,
    }
}

impl World {
    /// The match's zones, built on first use and shared.
    pub fn ai_zone_map(&self) -> Arc<ZoneMapView> {
        self.ai_shared
            .zone_view
            .get_or_init(|| Arc::new(ZoneMapView::of(&self.zones())))
            .clone()
    }

    /// `player`'s fronts at its last think, with what it read of each.
    pub(in crate::ai) fn mind_fronts(&self, player: u8) -> Vec<MindFront> {
        let c = &self.state.ai[player as usize].commander;
        let tick = self.state.tick;
        c.scratch
            .fronts
            .iter()
            .map(|f| MindFront {
                id: f.id,
                enemy: f.enemy,
                water: f.water,
                zones: f.zones.clone(),
                entries: f.entries.clone(),
                stake_us: f.stake_us,
                stake_them: f.stake_them,
                ours: f.ours,
                theirs: f.theirs,
                depth: f.depth,
                age: c
                    .fronts
                    .iter()
                    .find(|r| r.id == f.id)
                    .map_or(0, |r| tick.saturating_sub(r.since) / 10),
            })
            .collect()
    }

    /// What `player`'s AI believes of each zone it knows anything of.
    pub(in crate::ai) fn mind_zones(&self, known: &[ZoneKnowledge]) -> Vec<MindZone> {
        let tick = self.state.tick;
        let starts = self
            .ai_shared
            .zones
            .get()
            .map(|z| z.starts.clone())
            .unwrap_or_default();
        known
            .iter()
            .enumerate()
            .filter(|(_, k)| k.seen > 0 || !k.held.is_empty() || k.view > 0)
            .map(|(z, k)| MindZone {
                zone: z as u16,
                seen_ago: (k.seen > 0).then(|| tick.saturating_sub(k.seen) / 10),
                view: k.view,
                blips: k.blips,
                held: k
                    .held
                    .iter()
                    .map(|h| {
                        let start = starts.get(h.player as usize) == Some(&(z as u16));
                        holding(h, start, tick)
                    })
                    .collect(),
            })
            .collect()
    }

    /// What every player really holds in each zone, by zone: the observer's truth,
    /// counted the way a side counts what it sees. Empty when no AI plays.
    pub fn ai_zone_truth(&self) -> Vec<Vec<MindHolding>> {
        if !self
            .state
            .players
            .iter()
            .any(|p| p.controller == Controller::Ai)
        {
            return Vec::new();
        }
        let zones = self.zones();
        let profiles = self.profiles();
        let tick = self.state.tick;
        let mut now: Vec<Vec<Holding>> = vec![Vec::new(); zones.zones.len()];
        let units = &self.state.units;
        for r in units.slots.iter() {
            if !units.is_active(r) || units.has_flag(r, crate::tables::flag::IN_FACTORY) {
                continue;
            }
            let Some(z) = zones.of(units.pos[r]) else {
                continue;
            };
            let b = units.blueprint[r];
            super::knowledge::add(
                &mut now[z as usize],
                units.owner[r],
                self.blueprints.unit(b),
                profiles.get(b),
                tick,
            );
        }
        now.iter()
            .enumerate()
            .map(|(z, seen)| {
                let mut seen = seen.clone();
                seen.sort_by_key(|h| h.player);
                refresh(&[], &seen, 1000, tick)
                    .iter()
                    .map(|h| {
                        holding(
                            h,
                            zones.starts.get(h.player as usize) == Some(&(z as u16)),
                            tick,
                        )
                    })
                    .collect()
            })
            .collect()
    }
}
