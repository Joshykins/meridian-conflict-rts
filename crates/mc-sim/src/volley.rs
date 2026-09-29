//! Broadside fire (`Weapon::volley`): a battleship's batteries fire as one broadside
//! (docs/NAVY.md "The Leviathan"). `Units::volley` is a hull's record, a bit per weapon.
//!
//! - Each tick a battery on its mark and ready says so (ready); one that bears but is still
//!   turning or reloading holds the others up (busy), as does the hull still coming round
//!   (`HULL`). `World::volley_turn` then decides whether the ready ones go: once none is
//!   busy, or once they have held `HOLD` ticks.
//! - A battery with a charge (`charged`) holds before its charge, not after it: its reload
//!   stops one tick short of the charge until its broadside goes, then it charges and fires
//!   straight on, so the charge seen and heard runs into the shot. Only a battery due within
//!   `HOLD` holds the others up, not one that has just fired, so batteries that fell out of
//!   step come back into it within a reload.
//! - A battery without a charge holds with its shot ready.

use crate::tables::Units;
use crate::World;
use mc_data::Weapon;

/// Longest a ready battery holds for the others: 4 s.
pub(crate) const HOLD: u16 = 40;
/// Busy bit for a hull still turning onto its lay.
pub(crate) const HULL: u16 = 1 << 15;

/// Whether `weapon` holds for its broadside before its charge rather than before its
/// shot: a charged battery whose reload runs longer than the charge.
pub(crate) fn charged(weapon: &Weapon) -> bool {
    weapon.volley && weapon.charge_ticks > 0 && weapon.reload_ticks > weapon.charge_ticks
}

/// The countdown a `charged` battery waits at: one tick short of its charge.
fn charge_wait(weapon: &Weapon) -> u16 {
    weapon.charge_ticks + 1
}

/// Whether gun `w` of `row` holds its countdown this tick: a `charged` battery at its
/// charge whose broadside has not gone.
pub(crate) fn waits_to_charge(units: &Units, row: usize, w: usize, weapon: &Weapon) -> bool {
    charged(weapon)
        && units.weapon_salvo_left[row][w] == 0
        && units.weapon_cooldown[row][w] == charge_wait(weapon)
        && units.volley[row][4] & (1 << w) == 0
}

/// Records a `charged` battery still counting down: ready when it waits at its charge laid
/// on a mark it bears on (`laid`, `bears`), busy when it bears and is due within `HOLD` (or
/// waits there not yet laid). False for any other gun, which follows the usual rule.
pub(crate) fn countdown(
    units: &mut Units,
    row: usize,
    w: usize,
    weapon: &Weapon,
    laid: bool,
    bears: bool,
) -> bool {
    let left = units.weapon_cooldown[row][w];
    if !charged(weapon) || left == 0 || units.weapon_salvo_left[row][w] > 0 {
        return false;
    }
    let wait = charge_wait(weapon);
    if left == wait && laid && bears {
        units.volley[row][0] |= 1 << w;
    } else if bears && left >= wait && left - wait <= HOLD {
        units.volley[row][1] |= 1 << w;
    }
    true
}

impl World {
    /// Decides whether this tick is a broadside: the batteries that were ready last tick go
    /// once none of the others that bear is still turning or reloading, or once they have
    /// held `HOLD` ticks. Go is the mask of those that were ready. Then clears the record.
    pub(crate) fn volley_turn(&mut self, row: usize) {
        let v = &mut self.state.units.volley[row];
        let (ready, busy) = (v[0], v[1]);
        let go = ready != 0 && (busy == 0 || v[3] >= HOLD);
        v[3] = if ready != 0 && !go {
            v[3].saturating_add(1)
        } else {
            0
        };
        v[4] = if go { ready } else { 0 };
        v[0] = 0;
        v[1] = 0;
    }
}
