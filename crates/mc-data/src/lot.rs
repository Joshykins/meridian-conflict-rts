//! Which way a structure or a site-built unit faces on its lot, and the lot that
//! makes: lots stay on the build grid, so a site faces one of four ways, and a
//! quarter turn swaps the lot's sides.

use crate::{cat, UnitBlueprint};
use mc_core::Angle;

impl UnitBlueprint {
    /// Which way it faces when raised on its lot unturned. Structures face south. A
    /// unit built on a lot (a lift ship) lies along the lot's long side, nose east
    /// when the lot is wider than deep, so its hull and the lot agree.
    pub fn build_heading(&self) -> Angle {
        if !self.is_site_built_unit() {
            Angle::from_degrees(270)
        } else if self.footprint.0 >= self.footprint.1 {
            Angle::ZERO
        } else {
            Angle::from_degrees(90)
        }
    }

    /// Whether a player may turn it by quarter turns while placing it. A wall joins
    /// its neighbours whatever way it faces, and a core mine stands over its deposit
    /// as the survey draws it; both always face `build_heading`.
    pub fn turnable(&self) -> bool {
        self.built_on_site() && !self.has(cat::WALL) && self.mine.is_none()
    }

    /// Quarter turns anticlockwise from `build_heading` nearest `heading`, 0..4.
    /// Always 0 for what is not `turnable`.
    pub fn lot_turn(&self, heading: Angle) -> u8 {
        if !self.turnable() {
            return 0;
        }
        let from_build = (heading - self.build_heading()).0;
        (from_build.wrapping_add(Angle::QUARTER_TURN.0 / 2) / Angle::QUARTER_TURN.0) as u8
    }

    /// `heading` snapped to the nearest way its lot can face (`lot_turn`). An order's
    /// heading is the player's to send, so the sim snaps every one it builds by.
    pub fn lot_heading(&self, heading: Angle) -> Angle {
        self.build_heading() + Angle(self.lot_turn(heading) as u16 * Angle::QUARTER_TURN.0)
    }

    /// `footprint` turned to `heading`: its sides swap on a quarter or three-quarter turn.
    pub fn footprint_at(&self, heading: Angle) -> (u8, u8) {
        if self.lot_turn(heading) % 2 == 1 {
            (self.footprint.1, self.footprint.0)
        } else {
            self.footprint
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Blueprints;
    use std::path::Path;

    fn blueprints() -> Blueprints {
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
    }

    #[test]
    fn turns_snap_to_the_nearest_quarter() {
        let all = blueprints();
        let bp = (all.units.iter())
            .find(|b| b.is_site_built_unit() && b.footprint == (8, 19))
            .expect("a site-built unit on an 8x19 lot");
        let build = bp.build_heading();
        assert_eq!(bp.lot_turn(build), 0);
        assert_eq!(bp.lot_turn(build + Angle::from_degrees(80)), 1);
        assert_eq!(bp.lot_turn(build + Angle::from_degrees(170)), 2);
        assert_eq!(bp.lot_turn(build - Angle::from_degrees(100)), 3);
        assert_eq!(bp.lot_turn(build - Angle::from_degrees(30)), 0);
        assert_eq!(
            bp.lot_heading(build + Angle::from_degrees(100)),
            build + Angle::QUARTER_TURN
        );
        assert_eq!(bp.footprint_at(build + Angle::QUARTER_TURN), (19, 8));
        assert_eq!(bp.footprint_at(build + Angle::HALF_TURN), (8, 19));
    }

    #[test]
    fn walls_and_mines_face_one_way() {
        let all = blueprints();
        for bp in all
            .units
            .iter()
            .filter(|b| b.has(cat::WALL) || b.mine.is_some())
        {
            let turned = bp.build_heading() + Angle::QUARTER_TURN;
            assert_eq!(bp.lot_heading(turned), bp.build_heading(), "{}", bp.key);
        }
    }
}
