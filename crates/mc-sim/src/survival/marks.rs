//! What every side sees coming of the Progenitor's warp-ins (`RenderFrame::warp_ins`):
//! each Seedcaster squad marked (`warp_in.rs`) and each Gatewright fleet given its mark
//! (`fleet.rs`), from the moment it is announced until it comes in, mark, countdown and
//! all. The defenders always see them: the marks are the warning. A fleet's jump itself
//! is a real warp, also drawn and heard as any (`RenderFrame::warps`). Presentation only:
//! nothing here writes back.

use super::fleet::{Fleet, FleetStage};
use super::rules::arrival_shock;
use super::HZ;
use crate::tables::WarpPhase;
use crate::warp::{transit_ticks, warp_rate};
use mc_core::{Fx, FxVec2};
use mc_data::BlueprintId;

/// What is warping in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WarpInKind {
    /// A Seedcaster's squad, coming out on the ground.
    #[default]
    Squad = 0,
    /// A Gatewright's fleet of warships, coming out at their cruise height.
    Fleet = 1,
}

/// A squad or fleet marked to warp in, for the renderer and the interface.
#[derive(Clone, Debug, Default)]
#[expect(
    clippy::disallowed_types,
    reason = "presentation: the renderer and the warp-in marks draw from it"
)]
pub struct WarpInInstance {
    /// The mark's serial: `SimEvent::WarpInMarked`, `WarpInArrived` and `WarpInCancelled`
    /// for a squad, `SimEvent::FleetMarked` for a fleet.
    pub serial: u32,
    pub kind: WarpInKind,
    /// The Shaper that marked it (`UnitInstance::unit_id`; it may be gone), its site, and
    /// its crown (the mark itself once it is gone).
    pub source: u32,
    pub site: u8,
    pub from: [f32; 3],
    /// The mark: a squad's middle on the ground, a fleet's middle under it.
    pub at: [f32; 3],
    /// Radians (0 east, counter-clockwise) the squad or fleet faces as it comes out.
    pub heading: f32,
    /// Metres round the mark it comes out within.
    pub reach: f32,
    /// Where each of its units comes out: a squad's on the ground, a fleet's ships at
    /// their cruise height, as the jump keeps their formation.
    pub spots: Vec<[f32; 3]>,
    /// Seconds since it was marked, and until it comes in (its whole countdown is the
    /// two together). A fleet's reckons in its drives' spool and the jump, from their
    /// state once they spool.
    pub age: f32,
    pub eta: f32,
    /// A fleet's ships are spooling, in warp or coming out (their jumps are in
    /// `RenderFrame::warps`); false before, and always for a squad.
    pub jumping: bool,
    /// The shock where a squad comes in: reach (m) and damage; 0 for a fleet.
    pub radius: f32,
    pub damage: f32,
    /// Units (ships) it brings, the highest tech among them, and its Shaper's level when
    /// it marked it.
    pub units: u16,
    pub tech: u8,
    pub level: u16,
}

impl crate::World {
    /// Every squad and fleet marked to warp in, for every side.
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: fills the render frame's warp-in list"
    )]
    pub(crate) fn write_warp_ins(&self, out: &mut Vec<WarpInInstance>) {
        out.clear();
        let Some(s) = self.state.survival.as_ref() else {
            return;
        };
        let now = self.state.tick;
        let tps = HZ as f32;
        let ground = |p: FxVec2| self.terrain.height_at(p).max(self.terrain.water_level());
        let tech = |units: &mut dyn Iterator<Item = BlueprintId>| {
            units
                .map(|b| self.blueprints.unit(b).tech)
                .max()
                .unwrap_or(1)
        };
        for w in &s.warp_ins.pending {
            let (radius, damage) = arrival_shock(w.level);
            out.push(WarpInInstance {
                serial: w.serial,
                kind: WarpInKind::Squad,
                source: w.source.0,
                site: w.site,
                from: self.crown_of(w.source).unwrap_or(w.at.to_f32()),
                at: w.at.to_f32(),
                heading: w.heading.to_radians_f32(),
                reach: w.reach.to_f32(),
                spots: w
                    .places
                    .iter()
                    .map(|p| p.extend(ground(*p)).to_f32())
                    .collect(),
                age: now.saturating_sub(w.marked) as f32 / tps,
                eta: w.arrive.saturating_sub(now) as f32 / tps,
                jumping: false,
                radius: radius.to_f32(),
                damage: damage.to_f32(),
                units: w.squad.len().min(u16::MAX as usize) as u16,
                tech: tech(&mut w.squad.iter().copied()),
                level: w.level,
            });
        }
        for f in &s.fleets {
            if !matches!(f.stage, FleetStage::Marked | FleetStage::Warping) {
                continue;
            }
            let at = f.mark.extend(ground(f.mark));
            let spots = self.fleet_exits(f);
            let reach = spots
                .iter()
                .map(|(p, r)| p.xy().distance(f.mark) + *r)
                .max()
                .unwrap_or(Fx::ZERO);
            let level = s
                .nodes
                .iter()
                .find(|n| n.unit == f.shaper)
                .map_or(1, |n| n.level);
            let ships = f.ships.iter().filter_map(|u| self.state.units.row(*u));
            let heading = f
                .ships
                .first()
                .and_then(|u| self.state.units.row(*u))
                .map_or(0.0, |r| self.state.units.heading[r].to_radians_f32());
            out.push(WarpInInstance {
                serial: f.serial,
                kind: WarpInKind::Fleet,
                source: f.shaper.0,
                site: f.site,
                from: self.crown_of(f.shaper).unwrap_or(at.to_f32()),
                at: at.to_f32(),
                heading,
                reach: reach.to_f32(),
                spots: spots.iter().map(|(p, _)| p.to_f32()).collect(),
                age: now.saturating_sub(f.marked) as f32 / tps,
                eta: self.fleet_eta(f),
                jumping: f.stage == FleetStage::Warping,
                radius: 0.0,
                damage: 0.0,
                units: f.ships.len().min(u16::MAX as usize) as u16,
                tech: tech(&mut ships.map(|r| self.state.units.blueprint[r])),
                level,
            });
        }
    }

    /// The crown of the Shaper `unit` (its foot and its built height over it), if it stands.
    #[expect(
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: the warp-in list's `from`"
    )]
    fn crown_of(&self, unit: crate::UnitId) -> Option<[f32; 3]> {
        let s = self.state.survival.as_ref()?;
        let row = self.state.units.row(unit)?;
        let n = s.nodes.iter().find(|n| n.unit == unit)?;
        let u = &self.state.units;
        Some(u.pos[row].extend(u.z[row] + n.height).to_f32())
    }

    /// Where each ship of fleet `f` comes out (at its cruise height) and its radius: its
    /// jump's exit once it spools, else its place about the mark as `order_warp` keeps it.
    fn fleet_exits(&self, f: &Fleet) -> Vec<(mc_core::FxVec3, Fx)> {
        let u = &self.state.units;
        let rows: Vec<usize> = f.ships.iter().filter_map(|id| u.row(*id)).collect();
        let Some(&first) = rows.first() else {
            return Vec::new();
        };
        // The middle of the fleet, taken about one ship as `order_warp` does.
        let base = u.pos[first];
        let sum = rows
            .iter()
            .fold(FxVec2::ZERO, |sum, &r| sum + (u.pos[r] - base));
        let n = rows.len() as i32;
        let middle = base + FxVec2::new(sum.x / n, sum.y / n);
        rows.iter()
            .map(|&r| {
                let w = &u.warp[r];
                let to = if w.phase == WarpPhase::Idle {
                    self.clamp_to_map(f.mark + (u.pos[r] - middle))
                } else {
                    w.to
                };
                let bp = self.bp(r);
                let cruise = bp.motion.map_or(Fx::ZERO, |m| m.altitude);
                let surface = self.terrain.height_at(to).max(self.terrain.water_level());
                (to.extend(surface + cruise), bp.radius)
            })
            .collect()
    }

    /// Seconds until the last ship of fleet `f` comes out of its jump: the lead left
    /// before its ships spool, then each one's spool at full power (the Progenitor's
    /// side never runs short) and its transit; from its jump's own state once it spools.
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        reason = "presentation: the fleet's countdown on the marks and the card"
    )]
    pub(super) fn fleet_eta(&self, f: &Fleet) -> f32 {
        let u = &self.state.units;
        let now = self.state.tick;
        let lead = f.jump_at.saturating_sub(now);
        let exits = self.fleet_exits(f);
        let ticks = f
            .ships
            .iter()
            .filter_map(|id| u.row(*id))
            .zip(&exits)
            .map(|(r, (to, _))| {
                let Some(drive) = self.bp(r).warp else {
                    return 0;
                };
                let w = &u.warp[r];
                match w.phase {
                    WarpPhase::Idle => {
                        let distance = u.pos[r].distance(to.xy());
                        let jump = crate::warp::WarpState {
                            from: u.pos[r],
                            to: to.xy(),
                            ..Default::default()
                        };
                        lead + drive.charge_ticks(distance) as u32
                            + transit_ticks(&jump, drive.speed) as u32
                    }
                    WarpPhase::Spool => {
                        let rate = warp_rate(w).max(Fx(1));
                        let left = ((w.need - w.charge).max(Fx::ZERO) / rate).ceil_int();
                        left.max(0) as u32 + transit_ticks(w, drive.speed) as u32
                    }
                    WarpPhase::Transit => w.length.saturating_sub(w.ticks) as u32,
                    WarpPhase::Emerge => 0,
                }
            })
            .max()
            .unwrap_or(0);
        ticks as f32 / HZ as f32
    }
}
