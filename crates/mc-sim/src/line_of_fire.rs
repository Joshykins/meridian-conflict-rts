//! Line of fire: a gun that shoots straight holds its fire while the ground stands between
//! its muzzle and its target, turns to something it can hit instead, and a unit sent to
//! attack walks on until it has a shot.
//!
//! What is checked, and when:
//! - Only guns that fly flat (`Trajectory::Direct`, not homing, not curving) on units
//!   that are not aircraft. Shells that lob, missiles that climb and thrown charges that
//!   curve go over hills; that is what they are for. A cone's wake rolls over them
//!   (`wake.rs`).
//! - Torpedoes too, but only land that breaks the surface blocks them: they climb over a
//!   mound under the water (`World::bed_climb`).
//! - Every `CHECK_EVERY` ticks for a unit, spread over the rows, and at once when a gun
//!   takes a new target: one or two raycasts against the heightfield per gun.
//! - City blocks stand in the way as the ground does (`city.rs`): the line asks the
//!   same question a shot's sweep does, so a gun never pours its shells into a wall
//!   with its target behind it. Attack-ground is not checked: a gun told to fire on a
//!   point behind or in a block fires, and its shells meet the block.
//!
//! The answer is `Units::shot_blocked`, set by `World::run_targeting`. The weapons phase
//! does not fire a blocked gun (its turret keeps tracking), and the orders phase walks an
//! attacking unit on instead of stopping it (`World::has_clear_target`).

use crate::spatial::kind;
use crate::World;
use mc_core::{Fx, FxVec3};
use mc_data::{Trajectory, Weapon, MAX_WEAPONS};
use mc_map::RAYCAST_MAX_LENGTH_M;

/// Ticks between line checks on one unit.
pub(crate) const CHECK_EVERY: u32 = 4;
/// Most other targets tried, nearest first, when the one a gun is on is hidden.
const MAX_CANDIDATES: usize = 4;

impl World {
    /// Whether weapon `weapon` of `row` needs to see what it shoots.
    pub(crate) fn needs_line(&self, row: usize, weapon: &Weapon) -> bool {
        let flat = if weapon.torpedo {
            // Interceptors run at torpedoes, not at anything a unit is laid on.
            !weapon.intercepts
        } else {
            weapon.trajectory == Trajectory::Direct
                && !weapon.guided
                && weapon.curve.0 == 0
                && weapon.cone.is_none()
        };
        flat && self
            .bp(row)
            .motion
            .is_none_or(|m| m.layer != mc_data::MoveLayer::Air)
    }

    /// Whether this tick is `row`'s turn to look again.
    pub(crate) fn line_check_due(&self, row: usize) -> bool {
        (self.state.tick as usize + row).is_multiple_of(CHECK_EVERY as usize)
    }

    /// Whether `weapon` of `row` has a clear line to `target`: from the height of its
    /// muzzle to the middle of the target, or failing that to its top, so a tank
    /// hull-down behind a crest can still be hit. The line stops a target's radius short,
    /// so the ground right under a target on a reverse slope does not hide it.
    pub(crate) fn clear_shot(&self, row: usize, weapon: &Weapon, target: usize) -> bool {
        let units = &self.state.units;
        if weapon.torpedo {
            let (from, to) = (units.pos[row], units.pos[target]);
            let len = from.distance(to);
            let short = self.bp(target).radius;
            return len <= short * 2
                || self.torpedo_run_clear(from, from + (to - from) * ((len - short) / len));
        }
        let muzzle_z = units.z[row] + weapon.pivot.unwrap_or(weapon.muzzle).z;
        self.clear_from(units.pos[row].extend(muzzle_z), target)
    }

    /// Whether the ground leaves a clear line from `from` to `target`, as `clear_shot`
    /// reads it: `from` is never taken lower than just over the ground under it.
    pub(crate) fn clear_from(&self, from: FxVec3, target: usize) -> bool {
        let units = &self.state.units;
        let from = from
            .xy()
            .extend(from.z.max(self.terrain.height_at(from.xy()) + Fx::HALF));
        let bp = self.bp(target);
        let to = units.pos[target];
        let across = to - from.xy();
        let len = across.length();
        if len <= bp.radius * 2 {
            return true;
        }
        // Where along the line to stop: a radius short of the target.
        let keep = (len - bp.radius) / len;
        let seen = |z: Fx| {
            let end = FxVec3::new(
                from.x + across.x * keep,
                from.y + across.y * keep,
                from.z + (z - from.z) * keep,
            );
            self.line_clear(from, end)
        };
        seen(units.z[target] + bp.height / 2) || seen(units.z[target] + bp.height)
    }

    /// Whether the segment stays above the ground all the way, in pieces the raycast
    /// takes, and meets no city block standing (the same test a shot's sweep makes).
    fn line_clear(&self, from: FxVec3, to: FxVec3) -> bool {
        let d = to - from;
        let longest = d.x.abs().max(d.y.abs()).max(d.z.abs());
        let limit = Fx::from_int(RAYCAST_MAX_LENGTH_M);
        let pieces = (longest / limit).ceil_int().max(1);
        mc_core::perf_count!("los.rays");
        mc_core::perf_count!("los.pieces", pieces);
        let mut a = from;
        for i in 1..=pieces {
            let b = if i == pieces {
                to
            } else {
                from + FxVec3::new(d.x * i / pieces, d.y * i / pieces, d.z * i / pieces)
            };
            if self.terrain.raycast(a, b).is_some() {
                return false;
            }
            a = b;
        }
        self.city_shapes
            .first_hit(&self.state.city, from, to)
            .is_none()
    }

    /// The nearest target other than `hidden` that `weapon` of `row` may shoot and can
    /// see, trying at most `MAX_CANDIDATES` of them.
    pub(crate) fn visible_alternative(
        &self,
        row: usize,
        weapon: &Weapon,
        hidden: usize,
    ) -> Option<usize> {
        let units = &self.state.units;
        let at = units.pos[row];
        let mut near: Vec<(Fx, usize)> = Vec::new();
        let friends = self.team_mask(units.owner[row]);
        self.index
            .query_foes(at, weapon.range_max, kind::UNIT, friends, |e| {
                let t = e.row as usize;
                if t != hidden
                    && self.unit_entry_is_current(e)
                    && self.is_valid_target(row, t, weapon)
                {
                    near.push((at.distance(units.pos[t]), t));
                }
                true
            });
        near.sort_unstable();
        near.into_iter()
            .take(MAX_CANDIDATES)
            .map(|(_, t)| t)
            .find(|&t| self.clear_shot(row, weapon, t))
    }

    /// Whether weapon `w` of `row` holds its fire for the ground in the way.
    pub(crate) fn shot_blocked(&self, row: usize, w: usize) -> bool {
        self.state.units.shot_blocked[row] & (1 << w) != 0
    }

    /// The target `row`'s guns are laid on but cannot see, when none of them can see
    /// anything: what the interface shows as out of its line of fire.
    pub(crate) fn hidden_target(&self, row: usize) -> Option<usize> {
        let units = &self.state.units;
        if units.shot_blocked[row] == 0 || self.has_clear_target(row) {
            return None;
        }
        (0..MAX_WEAPONS)
            .filter(|&w| self.shot_blocked(row, w))
            .find_map(|w| units.row(units.weapon_target[row][w]))
    }

    /// Whether any of `row`'s guns has a target it can see: the unit is where it can fight.
    pub(crate) fn has_clear_target(&self, row: usize) -> bool {
        let units = &self.state.units;
        (0..MAX_WEAPONS)
            .any(|w| units.row(units.weapon_target[row][w]).is_some() && !self.shot_blocked(row, w))
    }
}
