//! What the presentation sees of warps, dampeners and stuns (`crate::warp`).
//!
//! Per unit, `UnitInstance::fx` carries the warp stretch and the stun, each last tick and
//! this, and `status[0]` two marks (`UNIT_WARP_DAMPED`, `UNIT_IN_WARP`). The ship is drawn
//! for one more tick after it jumps, at the place it left, stretching from nothing to a
//! full streak, and comes out on its first tick back going from a full streak to nothing:
//! so the jump happens between two ticks, and the renderer's own effects
//! (`RenderFrame::warps`) carry everything before, between and after.

use super::{DamperView, UnitInstance, WarpView};
use crate::tables::WarpPhase;
use crate::warp::EMERGE_DAMPED_TICKS;
use crate::World;
use mc_core::TICKS_PER_SECOND;

/// Units' `status[0]`: the jump it is in (spooling, in transit or coming out) is snagged
/// by a live enemy dampener: torn, slow, and it comes out hurt and stunned.
pub const UNIT_WARP_DAMPED: u32 = 1 << 12;
/// Units' `status[0]`: in warp, out of the world. Listed for its own side's interface
/// (selection, the card), never drawn.
pub const UNIT_IN_WARP: u32 = 1 << 13;
/// Ticks at the end of a stun over which it wears off (`fx[2..4]` fall from 1 to 0): the
/// ship's systems stutter back on.
const STUN_FADE: u16 = 3 * TICKS_PER_SECOND as u16;

/// How a unit is shown, given where it is in a jump.
pub(super) enum Shown {
    /// As usual.
    Plain,
    /// The tick it jumped: drawn where it left, streaking out.
    Leaving,
    /// In warp: listed for its own side, never drawn.
    Listed,
    /// Not at all.
    Hidden,
}

impl World {
    /// How `row` is shown to `viewer` while its drive is in a jump.
    pub(super) fn warp_shown(&self, viewer: Option<u8>, row: usize) -> Shown {
        let w = &self.state.units.warp[row];
        if w.phase != WarpPhase::Transit {
            return Shown::Plain;
        }
        let enemy = viewer.is_some_and(|v| self.are_enemies(v, self.state.units.owner[row]));
        let fogged = |p| {
            viewer.is_some_and(|v| {
                self.state.fog_enabled && !self.fog.is_detected(p, self.team_mask(v))
            })
        };
        match (w.ticks, enemy) {
            (0, true) if fogged(w.from) => Shown::Hidden,
            (0, _) => Shown::Leaving,
            (_, false) => Shown::Listed,
            (_, true) => Shown::Hidden,
        }
    }

    /// `[stretch last tick, stretch now, stun last tick, stun now]` and the `status[0]`
    /// marks for `row`.
    pub(super) fn warp_fx(&self, row: usize) -> ([f32; 4], u32) {
        let units = &self.state.units;
        let w = &units.warp[row];
        let damped = self.warp_damped(row);
        let stretch = match w.phase {
            // The tick it jumps: from nothing to a full streak.
            WarpPhase::Transit if w.ticks == 0 => [0.0, 1.0],
            WarpPhase::Transit => [1.0, 1.0],
            // A clean exit snaps in on its first tick back; a snagged one is torn out of
            // the streak slowly.
            WarpPhase::Emerge if damped => {
                let at = |t: u16| 1.0 - (t as f32 / EMERGE_DAMPED_TICKS as f32).min(1.0);
                [at(w.ticks.saturating_sub(1)), at(w.ticks)]
            }
            WarpPhase::Emerge if w.ticks == 0 => [1.0, 0.0],
            _ => [0.0, 0.0],
        };
        let [left, _] = units.stun[row];
        let stun = |left: u16| (left as f32 / STUN_FADE as f32).min(1.0);
        let fx = [
            stretch[0],
            stretch[1],
            if left > 0 { stun(left + 1) } else { 0.0 },
            stun(left),
        ];
        let mut marks = 0;
        if damped {
            marks |= UNIT_WARP_DAMPED;
        }
        if w.phase == WarpPhase::Transit && w.ticks > 0 {
            marks |= UNIT_IN_WARP;
        }
        (fx, marks)
    }

    /// The jump `row` is in is snagged by a dampener that still stands and has power.
    fn warp_damped(&self, row: usize) -> bool {
        let w = &self.state.units.warp[row];
        w.phase != WarpPhase::Idle
            && self
                .state
                .units
                .row(w.damper)
                .is_some_and(|d| self.damper_live(d))
    }

    /// Every jump `viewer` may see: its own side's all through; an enemy's spool and exit
    /// where it detects the ship, and its transit where it can see where the ship will come
    /// out, or when one of its own dampeners has the jump.
    pub(super) fn write_warps(&self, viewer: Option<u8>, out: &mut Vec<WarpView>) {
        out.clear();
        let s = &self.state;
        for row in s.units.slots.iter() {
            let w = &s.units.warp[row];
            if w.phase == WarpPhase::Idle {
                continue;
            }
            let owner = s.units.owner[row];
            let damper = s.units.row(w.damper).filter(|&d| self.damper_live(d));
            if let Some(v) = viewer.filter(|&v| self.are_enemies(v, owner)) {
                let mask = self.team_mask(v);
                let seen = match w.phase {
                    WarpPhase::Transit => {
                        self.fog.is_detected(w.to, mask)
                            || damper.is_some_and(|d| !self.are_enemies(v, s.units.owner[d]))
                            || !s.fog_enabled
                    }
                    _ => self.detects_for_team(v, row),
                };
                if !seen {
                    continue;
                }
            }
            let bp = self.bp(row);
            let cruise = bp.motion.map_or(0.0, |m| m.altitude.to_f32());
            let surface = |p: mc_core::FxVec2| {
                self.terrain
                    .height_at(p)
                    .max(self.terrain.water_level())
                    .to_f32()
            };
            let [fx, fy] = w.from.to_f32();
            let [tx, ty] = w.to.to_f32();
            out.push(WarpView {
                unit_id: s.units.id(row).0,
                owner,
                blueprint: s.units.blueprint[row],
                phase: w.phase,
                ticks: w.ticks,
                length: w.length,
                from: [fx, fy, surface(w.from) + cruise],
                to: [tx, ty, surface(w.to) + cruise],
                bearing: (w.to - w.from).angle().to_radians_f32(),
                radius: bp.radius.to_f32(),
                dampened: damper.is_some(),
                charge: match (w.phase, bp.warp) {
                    (WarpPhase::Spool, Some(d)) => (w.charge / d.energy).to_f32().min(1.0),
                    _ => 1.0,
                },
                energy: bp.warp.map_or(0.0, |d| d.energy.to_f32()),
                draw: bp.warp.map_or(0.0, |d| {
                    d.energy.to_f32() * TICKS_PER_SECOND as f32 / d.spool_ticks.max(1) as f32
                }),
            });
        }
    }

    /// Seconds before the drive in `row` may spool again (`UnitOrders::warp_recharge`).
    pub(super) fn warp_recharge_seconds(&self, row: usize) -> f32 {
        let w = &self.state.units.warp[row];
        if w.phase == WarpPhase::Idle {
            w.recharge as f32 / TICKS_PER_SECOND as f32
        } else {
            0.0
        }
    }

    /// Seconds before the stun on `row` wears off (`UnitOrders::stunned`).
    pub(super) fn stun_seconds(&self, row: usize) -> f32 {
        self.state.units.stun[row][0] as f32 / TICKS_PER_SECOND as f32
    }

    /// Every warp dampener `viewer` knows of: its own side's, and an enemy's it detects.
    pub(super) fn write_dampers(&self, viewer: Option<u8>, out: &mut Vec<DamperView>) {
        out.clear();
        let s = &self.state;
        for row in s.units.slots.iter() {
            let Some(spec) = self.bp(row).warp_damper else {
                continue;
            };
            let owner = s.units.owner[row];
            if !s.units.is_active(row)
                || viewer
                    .is_some_and(|v| self.are_enemies(v, owner) && !self.detects_for_team(v, row))
            {
                continue;
            }
            let [x, y] = s.units.pos[row].to_f32();
            out.push(DamperView {
                unit_id: s.units.id(row).0,
                owner,
                pos: [x, y, s.units.z[row].to_f32()],
                radius: spec.radius.to_f32(),
                live: self.damper_live(row),
            });
        }
    }
}

impl UnitInstance {
    /// How far into its warp streak it is drawn, this tick and last blended by `t`:
    /// 0 whole, 1 a streak of light.
    pub fn warp_stretch(&self, t: f32) -> f32 {
        self.fx[0] + (self.fx[1] - self.fx[0]) * t
    }

    /// How stunned it is, 0 to 1, blended by `t`.
    pub fn stun(&self, t: f32) -> f32 {
        self.fx[2] + (self.fx[3] - self.fx[2]) * t
    }

    /// In warp: listed, never drawn (`UNIT_IN_WARP`).
    pub fn in_warp(&self) -> bool {
        self.owner_flags & (super::KIND_WRECK | super::KIND_PROP | super::KIND_GHOST) == 0
            && self.status[0] & UNIT_IN_WARP != 0
    }
}
