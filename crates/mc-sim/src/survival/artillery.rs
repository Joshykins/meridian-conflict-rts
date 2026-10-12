//! A Seedcaster's replication artillery (Shapers on `SiteKind::Artillery` sites). On a
//! beat that quickens with its level it fires a volley of shells from its crown, each
//! on a long high arc at a mark beside something of the defenders' (a structure or an
//! army in the field), and where one lands it bursts (a modest blast, shields first) and
//! prints a squad of its products on the spot, incomplete and quickly finished in the
//! replication violet, which then attack-moves at the defenders.
//!
//! The shells are not projectiles: nothing shoots them down, not flak, not missile
//! defence, not an anti-nuke. The answer is the Seedcaster itself, and the warning:
//! every shell's mark and time to land are shown to everyone from the moment it leaves
//! (`RenderFrame::shells`). A volley the field cap (`FIELD_CAP`) cannot take is held
//! back, and the HUD says so (`SurvivalStatus::held`, `ArtilleryCard::held`).
//!
//! Squad size, heavies, shells a volley, the beat and the burst grow with the level
//! (`rules.rs`); what a squad is made of are the Seedcaster's products, which it takes
//! on as it levels like a printing Shaper.

use super::print::Route;
use super::rules::{
    shell_apex, shell_blast, shell_flight_ticks, shell_squad, shells_per_volley, volley_ticks,
};
use super::{Spot, FIELD_CAP, HZ};
use crate::mirror::SimEvent;
use crate::well_area::{Aircraft, WellArea};
use crate::{SimError, UnitId};
use mc_core::{Angle, Fx, FxVec2, FxVec3, Rng, StateHasher};
use mc_data::survival::SiteKind;
use mc_data::{BlueprintId, MoveLayer};
use serde::{Deserialize, Serialize};

/// Ticks a held volley waits before it tries again.
const HELD_RETRY_TICKS: u32 = 5 * HZ;
/// Metres from the thing it is aimed at a shell's mark falls: it lands beside, not on it.
const NEAR: (i32, i32) = (40, 120);
/// Metres the ground a squad can stand on may lie from the mark.
const LAND_REACH: i32 = 150;
/// Marks tried for one shell before it gives up for this volley. A bound on the search.
const TRIES: usize = 4;

/// A replication shell in the air.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Shell {
    pub(super) serial: u32,
    /// The Seedcaster that fired it (it may be gone since).
    pub(super) source: UnitId,
    pub(super) site: u8,
    /// Its Seedcaster's crown, where it left.
    pub(super) from: FxVec3,
    /// The ground at its mark, where it lands.
    pub(super) to: FxVec3,
    /// Metres its arc tops out over the line from `from` to `to`.
    pub(super) apex: Fx,
    pub(super) launched: u32,
    /// Ticks it flies.
    pub(super) flight: u32,
    /// What it prints where it lands.
    pub(super) squad: Vec<BlueprintId>,
    /// Its Seedcaster's level when it fired: the burst's.
    pub(super) level: u16,
}

/// Every replication shell in the air (`Survival::shells`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Shells {
    pub(super) flying: Vec<Shell>,
    /// The last serial handed out.
    serial: u32,
    /// The auxiliary Shapers' own dice (shell and warhead marks), so their aim never
    /// changes what the rounds plan.
    pub(super) rng: Rng,
}

impl Shells {
    pub(super) fn new(seed: u64) -> Shells {
        Shells {
            flying: Vec::new(),
            serial: 0,
            rng: Rng::new(seed),
        }
    }

    pub(super) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.serial as u64 | (self.flying.len() as u64) << 32);
        h.write_u64(self.rng.state());
        for s in &self.flying {
            h.write_u64(s.serial as u64 | (s.source.0 as u64) << 32);
            h.write_u64(s.launched as u64 | (s.flight as u64) << 32);
            h.write_u64(s.site as u64 | (s.level as u64) << 8 | (s.squad.len() as u64) << 24);
            for v in [s.from, s.to] {
                h.write_i64(v.x.0);
                h.write_i64(v.y.0);
                h.write_i64(v.z.0);
            }
            h.write_i64(s.apex.0);
            for b in &s.squad {
                h.write_u64(b.0 as u64);
            }
        }
    }

    /// Units the shells in the air will print.
    pub(super) fn pending(&self) -> usize {
        self.flying.iter().map(|s| s.squad.len()).sum()
    }

    /// Every blueprint the shells carry, for `State::validate_ids`.
    pub(super) fn blueprints(&self) -> impl Iterator<Item = BlueprintId> + '_ {
        self.flying.iter().flat_map(|s| s.squad.iter().copied())
    }
}

/// Where a shell from `from` to `to` that tops out `apex` over the line between them is
/// `t` (0..1) of the way through its flight: the one arc the sim flies and the interface
/// and the renderer draw (`ShellInstance::at`).
pub fn shell_at(from: FxVec3, to: FxVec3, apex: Fx, t: Fx) -> FxVec3 {
    let t = t.clamp(Fx::ZERO, Fx::ONE);
    let line = from + (to - from) * t;
    line + FxVec3::new(Fx::ZERO, Fx::ZERO, apex * 4 * t * (Fx::ONE - t))
}

impl crate::World {
    /// Shells that land this tick burst and print their squads; Seedcasters due fire
    /// their volleys.
    pub(super) fn survival_artillery(&mut self, now: u32) -> Result<(), SimError> {
        self.survival_shells_land(now)?;
        let s = self.state.survival.as_ref().unwrap();
        let due: Vec<usize> = (0..s.nodes.len())
            .filter(|&i| {
                let n = &s.nodes[i];
                n.raised && now >= n.next_fire && s.site_kind(n.site) == SiteKind::Artillery
            })
            .collect();
        for i in due {
            self.survival_volley(i, now)?;
        }
        Ok(())
    }

    /// What one shell of Shaper `i` prints: its heavies first, then its light units.
    pub(super) fn shell_squad_of(&self, i: usize) -> Vec<BlueprintId> {
        let n = &self.state.survival.as_ref().unwrap().nodes[i];
        let bps = self.blueprints.as_ref();
        let (light, heavy): (Vec<BlueprintId>, Vec<BlueprintId>) =
            n.products.iter().partition(|p| bps.unit(**p).tech < 4);
        let light_tech = light.iter().map(|p| bps.unit(*p).tech).max().unwrap_or(1);
        let (nl, nh) = shell_squad(n.level, light_tech, !heavy.is_empty());
        let mut squad: Vec<BlueprintId> = (0..nh).map(|k| heavy[k % heavy.len()]).collect();
        if !light.is_empty() {
            squad.extend((0..nl).map(|k| light[k % light.len()]));
        }
        squad
    }

    /// Shaper `i` fires a volley: a shell a mark, each its own. One the field cap cannot
    /// take is held back, and the volley tries again shortly.
    fn survival_volley(&mut self, i: usize, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_ref().unwrap();
        let n = s.nodes[i].clone();
        let side = s.config.engine_player;
        let Some(row) = self.state.units.row(n.unit) else {
            return Ok(());
        };
        let squad = self.shell_squad_of(i);
        let bps = self.blueprints.as_ref();
        let size_class = squad
            .iter()
            .filter_map(|b| bps.unit(*b).motion)
            .map(|m| m.size_class)
            .max()
            .unwrap_or(0);
        let crown = self.state.units.pos[row].extend(self.state.units.z[row] + n.height);
        let mut pending = s.shells.pending();
        let mut rng = s.shells.rng;
        let mut marks: Vec<FxVec2> = Vec::new();
        let mut held = false;
        let want = shells_per_volley(n.level);
        for _ in 0..want {
            if squad.is_empty() {
                break;
            }
            if self.hostile_count(side) + pending + squad.len() > FIELD_CAP {
                held = true;
                break;
            }
            let Some(mark) = self.shell_mark(&mut rng, size_class, &marks) else {
                break;
            };
            marks.push(mark);
            pending += squad.len();
        }
        let shells = &mut self.state.survival.as_mut().unwrap().shells;
        shells.rng = rng;
        let mut fired = Vec::new();
        for (k, mark) in marks.iter().enumerate() {
            shells.serial = shells.serial.wrapping_add(1);
            let ground = self
                .terrain
                .height_at(*mark)
                .max(self.terrain.water_level());
            let to = mark.extend(ground);
            let distance = crown.xy().distance(*mark);
            let flight = shell_flight_ticks(distance);
            fired.push(SimEvent::ReplicationShellFired {
                serial: shells.serial,
                source: n.unit,
                site: n.site,
                from: crown,
                to,
                flight_ticks: flight,
                units: squad.len().min(u16::MAX as usize) as u16,
                volley: k.min(u8::MAX as usize) as u8,
            });
            shells.flying.push(Shell {
                serial: shells.serial,
                source: n.unit,
                site: n.site,
                from: crown,
                to,
                apex: shell_apex(distance),
                launched: now,
                flight,
                squad: squad.clone(),
                level: n.level,
            });
        }
        self.events.extend(fired);
        let node = &mut self.state.survival.as_mut().unwrap().nodes[i];
        node.held = held;
        node.next_fire = if held && marks.is_empty() {
            now + HELD_RETRY_TICKS
        } else {
            now + volley_ticks(n.level)
        };
        Ok(())
    }

    /// A mark beside something of the defenders': six shells in ten at a structure, the
    /// rest at an army on the ground, each a little way off it on ground a squad of
    /// `size_class` can stand on, and not within a burst of a mark already chosen.
    fn shell_mark(&self, rng: &mut Rng, size_class: u8, taken: &[FxVec2]) -> Option<FxVec2> {
        let side = self.state.survival.as_ref()?.config.engine_player;
        let u = &self.state.units;
        let (mut structures, mut armies) = (Vec::new(), Vec::new());
        for r in u.slots.iter() {
            if !u.is_active(r) || !self.are_enemies(side, u.owner[r]) {
                continue;
            }
            let bp = self.bp(r);
            match bp.motion {
                None => structures.push(u.pos[r]),
                Some(m) if m.layer != MoveLayer::Air => armies.push(u.pos[r]),
                Some(_) => {}
            }
        }
        let apart = Fx::from_int(NEAR.1 * 2);
        for _ in 0..TRIES {
            let structure = armies.is_empty() || (!structures.is_empty() && rng.below(10) < 6);
            let pool = if structure { &structures } else { &armies };
            if pool.is_empty() {
                return None;
            }
            let at = pool[rng.below(pool.len() as u32) as usize];
            let turn = Angle(rng.below(1 << 16) as u16);
            let off = Fx::from_int(NEAR.0 + rng.below((NEAR.1 - NEAR.0) as u32) as i32);
            let mark = self.clamp_to_map(at + FxVec2::from_angle(turn) * off);
            let Some(ground) = self
                .nav
                .nearest_passable(MoveLayer::Land, size_class, mark)
                .filter(|p| p.distance(mark) <= Fx::from_int(LAND_REACH))
            else {
                continue;
            };
            if taken.iter().all(|t| t.distance(ground) >= apart) {
                return Some(ground);
            }
        }
        None
    }

    /// Shells whose flight is over burst where they land and print their squads.
    fn survival_shells_land(&mut self, now: u32) -> Result<(), SimError> {
        let s = self.state.survival.as_mut().unwrap();
        let side = s.config.engine_player;
        let (landed, flying): (Vec<Shell>, Vec<Shell>) = std::mem::take(&mut s.shells.flying)
            .into_iter()
            .partition(|sh| now >= sh.launched + sh.flight);
        s.shells.flying = flying;
        for sh in landed {
            let (radius, damage) = shell_blast(sh.level);
            self.crush_area(&WellArea {
                mark: sh.to.xy(),
                z: sh.to.z,
                radius,
                owner: side,
                source: sh.source,
                per_tick: damage,
                aircraft: Aircraft::Spared,
            });
            self.events.push(SimEvent::ReplicationShellLanded {
                serial: sh.serial,
                at: sh.to,
                radius,
                units: sh.squad.len().min(u16::MAX as usize) as u16,
            });
            // The squad comes out of the burst facing on the way the shell flew.
            let heading = (sh.to.xy() - sh.from.xy()).angle();
            let spot = Spot::Landing {
                at: sh.to.xy(),
                heading,
            };
            let source = crate::Handle::NONE;
            self.survival_print_formation(&sh.squad, spot, source, Route::Free, false, now)?;
        }
        Ok(())
    }
}

/// A replication shell in the air, for the renderer and the interface
/// (`RenderFrame::shells`). Every side sees every one, mark and all.
#[derive(Clone, Debug, Default)]
#[expect(
    clippy::disallowed_types,
    reason = "presentation: the renderer and the landing marks draw shells from it"
)]
pub struct ShellInstance {
    pub serial: u32,
    /// The Seedcaster that fired it (`UnitInstance::unit_id`; it may be gone).
    pub source: u32,
    /// Its Shaper site.
    pub site: u8,
    /// Its Seedcaster's crown, where it left.
    pub from: [f32; 3],
    /// The ground at its mark.
    pub to: [f32; 3],
    /// Metres its arc tops out over the line from `from` to `to`.
    pub apex: f32,
    /// Where it is now and was a tick ago (`at` at this tick's and the last's share).
    pub pos: [f32; 3],
    pub prev_pos: [f32; 3],
    /// Seconds since it left, and its whole flight.
    pub age: f32,
    pub flight: f32,
    /// Seconds until it lands.
    pub eta: f32,
    /// Its burst's reach (m) and damage.
    pub radius: f32,
    pub damage: f32,
    /// Units it prints where it lands, and the highest tech among them.
    pub units: u16,
    pub tech: u8,
    /// Its Seedcaster's level when it fired.
    pub level: u16,
}

#[expect(
    clippy::float_arithmetic,
    clippy::disallowed_types,
    reason = "presentation: the shell's arc for the renderer and the landing marks"
)]
impl ShellInstance {
    /// Where it is `t` (0..1) of the way through its flight: the arc the sim flies
    /// (`shell_at`), in floats.
    pub fn at(&self, t: f32) -> [f32; 3] {
        let t = t.clamp(0.0, 1.0);
        let lift = self.apex * 4.0 * t * (1.0 - t);
        [
            self.from[0] + (self.to[0] - self.from[0]) * t,
            self.from[1] + (self.to[1] - self.from[1]) * t,
            self.from[2] + (self.to[2] - self.from[2]) * t + lift,
        ]
    }
}

impl crate::World {
    /// Every replication shell in the air, for the renderer and the interface.
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: fills the render frame's shell list"
    )]
    pub(crate) fn write_shells(&self, out: &mut Vec<ShellInstance>) {
        out.clear();
        let Some(s) = self.state.survival.as_ref() else {
            return;
        };
        let now = self.state.tick;
        let tps = HZ as f32;
        for sh in &s.shells.flying {
            let flight = sh.flight.max(1);
            let share =
                |tick: u32| Fx::ratio(tick.saturating_sub(sh.launched) as i64, flight as i64);
            let pos = shell_at(sh.from, sh.to, sh.apex, share(now));
            let prev = shell_at(sh.from, sh.to, sh.apex, share(now.saturating_sub(1)));
            let (radius, damage) = shell_blast(sh.level);
            let bps = self.blueprints.as_ref();
            out.push(ShellInstance {
                serial: sh.serial,
                source: sh.source.0,
                site: sh.site,
                from: sh.from.to_f32(),
                to: sh.to.to_f32(),
                apex: sh.apex.to_f32(),
                pos: pos.to_f32(),
                prev_pos: prev.to_f32(),
                age: now.saturating_sub(sh.launched) as f32 / tps,
                flight: flight as f32 / tps,
                eta: (sh.launched + flight).saturating_sub(now) as f32 / tps,
                radius: radius.to_f32(),
                damage: damage.to_f32(),
                units: sh.squad.len().min(u16::MAX as usize) as u16,
                tech: sh
                    .squad
                    .iter()
                    .map(|b| bps.unit(*b).tech)
                    .max()
                    .unwrap_or(1),
                level: sh.level,
            });
        }
    }
}
