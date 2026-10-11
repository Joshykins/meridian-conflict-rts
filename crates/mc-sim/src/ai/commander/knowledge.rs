//! What a side knows of each zone (`docs/AI_COMMANDER.md`, "Knowledge"): for every
//! zone, what each enemy player was seen to hold there (structures by kind, army by
//! domain, builders, their commander) and how long ago, kept between thinks and saved
//! with the match. It is the side's long memory of the map, beside the contacts'
//! short memory of single units.
//!
//! Each think a zone is refreshed by how much of it is in view: wholly in view, what
//! is seen there now replaces what was known; half in view, half of the old is kept;
//! out of view, the old stands. A unit seen on radar only (known before, not in sight)
//! is never less than what is kept. What is kept fades as it ages when it is read:
//! a structure for many minutes, an army in a minute or two.
use super::profile::{role, Domain, Profile};
use super::zones::{Zones, SAMPLE};
use crate::ai::adaptive::{domain as adaptive_domain, Contact};
use crate::World;
use mc_core::{Fx, FxVec2, StateHasher};
use mc_data::{cat, UnitBlueprint};
use serde::{Deserialize, Serialize};

/// Kinds of structure a zone is known to hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub(in crate::ai) enum Built {
    /// Mines: what their income comes from.
    Mine = 0,
    /// Power and storage.
    Power = 1,
    /// Factories by what they make.
    Factory = 2,
    Airfield = 3,
    Shipyard = 4,
    /// Guns that hold ground against land and sea.
    Guns = 5,
    AntiAir = 6,
    /// Artillery and map guns: guns that reach far past their ground.
    Artillery = 7,
    /// Strategic launchers.
    Launcher = 8,
    Interceptor = 9,
    Shield = 10,
    /// Radar, sonar and other eyes.
    Sensor = 11,
    /// Walls and the rest.
    Other = 12,
}

pub(in crate::ai) const BUILT: usize = 13;
/// Domains an army is counted by (`profile::Domain as usize`).
pub(in crate::ai) const DOMAINS: usize = 7;

impl Built {
    pub(in crate::ai) fn of(bp: &UnitBlueprint, p: &Profile) -> Built {
        if p.has(role::STRATEGIC) {
            Built::Launcher
        } else if p.has(role::INTERCEPTOR) {
            Built::Interceptor
        } else if p.has(role::MAP_GUN) || p.has(role::ARTILLERY) {
            Built::Artillery
        } else if bp.has(cat::FACTORY) {
            match adaptive_domain(bp) {
                1 => Built::Airfield,
                2 => Built::Shipyard,
                _ => Built::Factory,
            }
        } else if p.has(role::ANTI_AIR) {
            Built::AntiAir
        } else if p.has(role::DEFENSE) {
            Built::Guns
        } else if bp.shield.is_some() {
            Built::Shield
        } else if bp.has(cat::EXTRACTOR) {
            Built::Mine
        } else if bp.has(cat::POWER) || p.has(role::ECONOMY) {
            Built::Power
        } else if bp.has(cat::INTEL) || bp.radar > Fx::ZERO || bp.sonar > Fx::ZERO {
            Built::Sensor
        } else {
            Built::Other
        }
    }

    pub(in crate::ai) fn name(self) -> &'static str {
        match self {
            Built::Mine => "mines",
            Built::Power => "power",
            Built::Factory => "factories",
            Built::Airfield => "airfields",
            Built::Shipyard => "shipyards",
            Built::Guns => "guns",
            Built::AntiAir => "anti-air",
            Built::Artillery => "artillery",
            Built::Launcher => "launchers",
            Built::Interceptor => "interceptors",
            Built::Shield => "shields",
            Built::Sensor => "sensors",
            Built::Other => "other",
        }
    }
}

pub(in crate::ai) const BUILT_KINDS: [Built; BUILT] = [
    Built::Mine,
    Built::Power,
    Built::Factory,
    Built::Airfield,
    Built::Shipyard,
    Built::Guns,
    Built::AntiAir,
    Built::Artillery,
    Built::Launcher,
    Built::Interceptor,
    Built::Shield,
    Built::Sensor,
    Built::Other,
];

/// Ticks a structure seen is as good as certain; it fades after, to `BUILT_FLOOR`
/// at `BUILT_FADE` (structures stand until killed, and a look clears them).
const BUILT_SURE: u32 = 6000;
const BUILT_FADE: u32 = 18_000;
const BUILT_FLOOR: i32 = 40;
/// Ticks an army seen is as good as certain, and after which nothing of it is kept:
/// an army three minutes gone could be anywhere.
const ARMY_SURE: u32 = 200;
const ARMY_FORGET: u32 = 1800;
/// Per mille of a zone that must be in view for it to count as looked at.
const LOOKED: i32 = 500;

/// What one enemy player was seen to hold in a zone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct Holding {
    pub player: u8,
    /// Structure mass, and count, by `Built`.
    pub built: [Fx; BUILT],
    pub count: [u16; BUILT],
    /// Tick the structures were last refreshed.
    pub built_at: u32,
    /// Armed mobile mass by domain (builders and the commander apart).
    pub army: [Fx; DOMAINS],
    pub builders: u16,
    /// Tick the army was last refreshed.
    pub army_at: u32,
    /// Tick their commander was last seen here (0: not lately).
    pub king_at: u32,
}

impl Holding {
    fn empty(player: u8) -> Holding {
        Holding {
            player,
            ..Holding::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.built.iter().all(|m| *m == Fx::ZERO)
            && self.army.iter().all(|m| *m == Fx::ZERO)
            && self.builders == 0
            && self.king_at == 0
    }

    /// How sure (0..=100) the structures are still there, `tick` now.
    pub(in crate::ai) fn built_sure(&self, tick: u32) -> i32 {
        let age = tick.saturating_sub(self.built_at);
        if age <= BUILT_SURE {
            100
        } else if age >= BUILT_FADE {
            BUILT_FLOOR
        } else {
            100 - (100 - BUILT_FLOOR) * (age - BUILT_SURE) as i32 / (BUILT_FADE - BUILT_SURE) as i32
        }
    }

    /// How sure (0..=100) the army is still here.
    pub(in crate::ai) fn army_sure(&self, tick: u32) -> i32 {
        let age = tick.saturating_sub(self.army_at);
        if age <= ARMY_SURE {
            100
        } else if age >= ARMY_FORGET {
            0
        } else {
            100 * (ARMY_FORGET - age) as i32 / (ARMY_FORGET - ARMY_SURE) as i32
        }
    }

    /// Structure mass of all kinds.
    pub(in crate::ai) fn built_mass(&self) -> Fx {
        self.built.iter().copied().sum()
    }

    /// Army mass believed here now: what was seen, by how sure it still is.
    pub(in crate::ai) fn army_now(&self, tick: u32) -> Fx {
        self.army.iter().copied().sum::<Fx>() * self.army_sure(tick) / 100
    }

    /// Whether their commander is believed here now.
    pub(in crate::ai) fn king_here(&self, tick: u32) -> bool {
        self.king_at > 0 && tick.saturating_sub(self.king_at) < ARMY_FORGET
    }

    fn hash(&self, h: &mut StateHasher) {
        h.write_u64(u64::from(self.player) | u64::from(self.builders) << 8);
        for (m, c) in self.built.iter().zip(&self.count) {
            h.write_i64(m.0);
            h.write_u64(u64::from(*c));
        }
        for m in &self.army {
            h.write_i64(m.0);
        }
        h.write_u64(u64::from(self.built_at) | u64::from(self.army_at) << 32);
        h.write_u64(u64::from(self.king_at));
    }
}

/// What a side knows of one zone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct ZoneKnowledge {
    /// Tick the zone was last mostly in view (0: never).
    pub seen: u32,
    /// Per mille of it in view at the last think.
    pub view: u16,
    /// Enemy units on radar there at the last think that it cannot name.
    pub blips: u16,
    /// What each enemy player was seen to hold there, by player.
    pub held: Vec<Holding>,
}

impl ZoneKnowledge {
    pub(in crate::ai) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(
            u64::from(self.seen) | u64::from(self.view) << 32 | u64::from(self.blips) << 48,
        );
        h.write_u64(self.held.len() as u64);
        for k in &self.held {
            k.hash(h);
        }
    }
}

/// Scales `m` by what share (per mille) of it is kept.
fn keep(m: Fx, kept: i32) -> Fx {
    m * kept / 1000
}

/// One think's update of a zone: what is seen there now (`now`, by player) over what
/// was known (`old`), `view` per mille of the zone in view.
pub(in crate::ai) fn refresh(
    old: &[Holding],
    now: &[Holding],
    view: i32,
    tick: u32,
) -> Vec<Holding> {
    let kept = 1000 - view.clamp(0, 1000);
    let mut players: Vec<u8> = old.iter().chain(now).map(|h| h.player).collect();
    players.sort_unstable();
    players.dedup();
    let mut out = Vec::with_capacity(players.len());
    for p in players {
        let zero = Holding::empty(p);
        let o = old.iter().find(|h| h.player == p).unwrap_or(&zero);
        let s = now.iter().find(|h| h.player == p).unwrap_or(&zero);
        let mut h = Holding::empty(p);
        for k in 0..BUILT {
            h.built[k] = s.built[k].max(keep(o.built[k], kept));
            h.count[k] = s.count[k].max((i32::from(o.count[k]) * kept / 1000) as u16);
        }
        let saw_built = s.built.iter().any(|m| *m > Fx::ZERO);
        h.built_at = if saw_built || view >= LOOKED {
            tick
        } else {
            o.built_at
        };
        // An army long unseen is forgotten, not carried as a ghost.
        let army_gone = tick.saturating_sub(o.army_at) >= ARMY_FORGET;
        for d in 0..DOMAINS {
            let old_army = if army_gone {
                Fx::ZERO
            } else {
                keep(o.army[d], kept)
            };
            h.army[d] = s.army[d].max(old_army);
        }
        let old_builders = if army_gone {
            0
        } else {
            (i32::from(o.builders) * kept / 1000) as u16
        };
        h.builders = s.builders.max(old_builders);
        let saw_army = s.army.iter().any(|m| *m > Fx::ZERO) || s.builders > 0;
        h.army_at = if saw_army || view >= LOOKED {
            tick
        } else {
            o.army_at
        };
        h.king_at = if s.king_at > 0 {
            tick
        } else if view >= LOOKED || tick.saturating_sub(o.king_at) >= ARMY_FORGET {
            0
        } else {
            o.king_at
        };
        if !h.is_empty() {
            out.push(h);
        }
    }
    out
}

/// Adds a unit seen now to what is seen in its zone.
pub(in crate::ai) fn add(
    now: &mut Vec<Holding>,
    player: u8,
    bp: &UnitBlueprint,
    p: &Profile,
    tick: u32,
) {
    let i = match now.iter().position(|h| h.player == player) {
        Some(i) => i,
        None => {
            now.push(Holding::empty(player));
            now.len() - 1
        }
    };
    let h = &mut now[i];
    if bp.is_structure() {
        let k = Built::of(bp, p) as usize;
        h.built[k] += bp.cost_mass;
        h.count[k] = h.count[k].saturating_add(1);
    } else if bp.has(cat::COMMANDER) {
        h.king_at = tick;
    } else if bp.has(cat::ENGINEER) || p.has(role::BUILDER) && !p.armed() {
        h.builders = h.builders.saturating_add(1);
    } else if p.armed() {
        let d = p.domain.unwrap_or(Domain::Land) as usize;
        h.army[d] += bp.cost_mass;
    }
}

impl World {
    /// Updates what `player` knows of every zone from what it detects now: `seen`, the
    /// enemy units it can name (in sight or on radar), and `blips`, those it cannot.
    pub(in crate::ai) fn learn_zones(&mut self, player: u8, seen: &[Contact], blips: &[FxVec2]) {
        let zones = self.zones();
        let profiles = self.profiles();
        let tick = self.state.tick;
        let n = zones.zones.len();
        let view = self.zone_view_share(player, &zones);
        let mut now: Vec<Vec<Holding>> = vec![Vec::new(); n];
        let mut radar = vec![0u16; n];
        for c in seen {
            let Some(z) = zones.of(c.pos) else { continue };
            let owner = self
                .state
                .units
                .row(c.id)
                .map_or(u8::MAX, |r| self.state.units.owner[r]);
            if owner == u8::MAX {
                continue;
            }
            let bp = self.blueprints.unit(c.blueprint);
            add(
                &mut now[z as usize],
                owner,
                bp,
                profiles.get(c.blueprint),
                tick,
            );
        }
        for b in blips {
            if let Some(z) = zones.of(*b) {
                radar[z as usize] = radar[z as usize].saturating_add(1);
            }
        }
        let known = &mut self.state.ai[player as usize].commander.knowledge;
        if known.len() != n {
            *known = vec![ZoneKnowledge::default(); n];
        }
        for (z, k) in known.iter_mut().enumerate() {
            let v = view[z];
            k.held = refresh(&k.held, &now[z], v, tick);
            k.view = v as u16;
            k.blips = radar[z];
            if v >= LOOKED {
                k.seen = tick;
            }
        }
    }

    /// Per mille of each zone in `player`'s team's view now, from every other node
    /// each way. Without fog every zone is wholly in view.
    fn zone_view_share(&self, player: u8, zones: &Zones) -> Vec<i32> {
        let n = zones.zones.len();
        if !self.state.fog_enabled {
            return vec![1000; n];
        }
        let mask = self.team_mask(player);
        let (w, h, id) = zones.grid();
        let mut lit = vec![0u32; n];
        let node = super::reach::NODE;
        for y in (0..h).step_by(SAMPLE as usize) {
            for x in (0..w).step_by(SAMPLE as usize) {
                let z = id[(y * w + x) as usize];
                if z == super::zones::NONE {
                    continue;
                }
                let at = FxVec2::from_ints(x * node + node / 2, y * node + node / 2);
                if self.fog.is_visible(at, mask) {
                    lit[z as usize] += 1;
                }
            }
        }
        zones
            .zones
            .iter()
            .zip(&lit)
            .map(|(z, &l)| (l * 1000 / z.samples.max(1)) as i32)
            .collect()
    }
}

#[cfg(test)]
mod tests;
