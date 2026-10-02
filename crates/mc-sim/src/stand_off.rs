//! Jets that stand off (`mc_data::Motion::stand_off`, the Regency's Voulge): engaged, they
//! fly a circle round their mark near the reach of their guns and fire from it, never
//! making a run over it, so they stay out of short-range anti-air. Fighters are the answer.
use crate::tables::flag;
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2};

/// A jet that stands off circles its mark at this share of its reach: inside it, so a
/// mark that moves a little stays in reach, and as far out as that allows.
const STAND_OFF: Fx = Fx::ratio(17, 20);
/// How far ahead of itself on its circle a standing-off jet steers, degrees.
const STAND_OFF_LEAD: i32 = 40;

impl World {
    /// A jet that stands off (`Motion::stand_off`) flies a circle round `center` at
    /// `STAND_OFF` of its reach, its guns on it, never passing over it: it chases a point
    /// `STAND_OFF_LEAD` ahead of itself on the circle, so it flies the circle rather than
    /// cutting inside it. From further out it comes in onto the circle the same way.
    pub(crate) fn air_stand_off(&mut self, row: usize, center: FxVec2) -> Result<(), SimError> {
        let delta = self.state.units.pos[row] - center;
        let ring = self.bp(row).max_weapon_range() * STAND_OFF;
        let bearing = if delta.length() > Fx::ONE {
            delta.angle()
        } else {
            self.state.units.heading[row]
        };
        // Half the flight goes round one way, half the other, as gunships do.
        let lead = if row.is_multiple_of(2) {
            STAND_OFF_LEAD
        } else {
            -STAND_OFF_LEAD
        };
        // Steering for a point `lead` ahead on a circle of radius `g` settles on the circle
        // where that point lies square off the nose, `g cos lead` out: aim wide by as much.
        let lead = Angle::from_degrees(lead);
        let chase = ring / FxVec2::from_angle(lead).x.max(Fx::HALF);
        let goal = self.clamp_to_map(center + FxVec2::from_angle(bearing + lead) * chase);
        self.ensure_moving(row, goal, goal)?;
        self.state.units.air_aim[row] = center;
        self.state.units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }
}
