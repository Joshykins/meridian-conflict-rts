//! Spaceships and their warp drives, for the Commander's warship and landing
//! operations (`commander/ops_space.rs`).
//!
//! The AI flew every spaceship the long way: a strike crossed the map at a
//! warship's crawl and no drive was ever spooled. A strike of armed spaceships
//! jumps (`jump_then_attack`): it comes out `STANDOFF` short of its target on
//! the side it came from, out of any remembered dampener's field (`safe_mark`),
//! and attack-moves in; a ship far from home jumps back (`go_home`).
use super::*;
use crate::tables::WarpPhase;

/// A strike comes out this far short of its target.
const STANDOFF: Fx = Fx::from_int(260);
/// A mark nearer than this is flown to, not jumped.
const WORTH_A_JUMP: Fx = Fx::from_int(1400);
/// A jump comes out at least this far outside a remembered dampener's field.
const DAMPER_MARGIN: Fx = Fx::from_int(150);

impl World {
    /// Whether `row`'s drive is ready to spool and its side can pay the charge for a
    /// jump to `to`: priced by how far that is (`Warp::charge`), out of the store and
    /// what comes in over the charge's time.
    pub(super) fn can_jump(&self, row: usize, to: FxVec2) -> bool {
        let Some(drive) = self.bp(row).warp else {
            return false;
        };
        let units = &self.state.units;
        let w = &units.warp[row];
        let pl = &self.state.players[units.owner[row] as usize];
        let distance = units.pos[row].distance(to);
        let seconds =
            Fx::from_int(drive.charge_ticks(distance) as i32) / mc_core::TICKS_PER_SECOND as i32;
        w.phase == WarpPhase::Idle
            && w.recharge == 0
            && pl.energy + pl.energy_income * seconds >= drive.charge(distance)
    }

    /// `want`, moved out of every remembered enemy dampener's field, toward `from`.
    pub(super) fn safe_mark(&self, player: u8, want: FxVec2, from: FxVec2) -> FxVec2 {
        let mut mark = want;
        for c in &self.state.ai[player as usize].contacts {
            let Some(d) = self.blueprints.unit(c.blueprint).warp_damper else {
                continue;
            };
            let clear = d.radius + DAMPER_MARGIN;
            if mark.distance(c.pos) >= clear {
                continue;
            }
            let away = if mark == c.pos {
                from - c.pos
            } else {
                mark - c.pos
            };
            let away = if away.length() == Fx::ZERO {
                FxVec2::from_angle(Angle::ZERO)
            } else {
                away.normalize()
            };
            mark = c.pos + away * clear;
        }
        self.clamp_to_map(mark)
    }

    /// `rows` go at `target`: those whose drives are ready jump to a mark short
    /// of it and attack-move in from there, the rest fly.
    pub(super) fn jump_then_attack(
        &self,
        player: u8,
        rows: &[usize],
        target: FxVec2,
        from: FxVec2,
        out: &mut Vec<Command>,
    ) {
        let units = &self.state.units;
        let mark = self.safe_mark(player, offset_toward(target, from, STANDOFF), from);
        let (jump, fly): (Vec<usize>, Vec<usize>) = rows.iter().partition(|&&r| {
            self.can_jump(r, mark) && units.pos[r].distance(target) >= WORTH_A_JUMP
        });
        if !jump.is_empty() {
            let ids = self.ids_of(&jump);
            out.push(Command::Warp {
                units: ids.clone(),
                pos: mark,
                queue: false,
            });
            out.push(Command::AttackMove {
                units: ids,
                target,
                queue: true,
            });
        }
        if !fly.is_empty() {
            out.push(Command::AttackMove {
                units: self.ids_of(&fly),
                target,
                queue: false,
            });
        }
    }

    /// `row` back to `home`: by warp when it is far and the drive is ready.
    pub(super) fn go_home(&self, row: usize, home: FxVec2, out: &mut Vec<Command>) {
        let far = self.state.units.pos[row].distance(home) >= WORTH_A_JUMP;
        let units = vec![self.state.units.id(row)];
        out.push(if far && self.can_jump(row, home) {
            Command::Warp {
                units,
                pos: home,
                queue: false,
            }
        } else {
            Command::Move {
                units,
                target: home,
                queue: false,
            }
        });
    }
}
