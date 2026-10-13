//! The Progenitor's base at the valley's head: the shared installation
//! (`bake/facility.rs`) laid on the level floor at [`BASE_H`], facing south down
//! the valley, nothing of it walled off. The three base Shapers' rooms
//! ([`SITES`]) are kept clear of all of it.

use super::*;
use crate::bake::facility::{Facility, Facing};
use crate::bake::machine::{Machine, PrecursorSite};

/// The base: the Heart at the valley's head, facing down it.
pub(super) const FACILITY: Facility = Facility {
    heart: HEART,
    facing: Facing::South,
    floor: BASE_H,
    sites: SITES,
};

impl Terrain {
    /// Every piece of the installation on the floor; the machine on the
    /// heights is `machine_harvest`.
    pub(super) fn lay_facility_harvest(&self) -> Vec<PrecursorSite> {
        self.facility_pieces(&FACILITY)
    }

    /// The machine: the tower ring round the Heart, the bastions either side.
    pub(in crate::bake) fn machine_harvest(&self) -> Machine<'_> {
        self.facility_machine(&FACILITY)
    }

    /// The sidecar's facility block for the survival rules: the Heart, the
    /// print bays, the sites.
    #[cfg(test)]
    pub(super) fn harvest_block(&self) -> String {
        let mut out = self.facility_bays(&FACILITY);
        out += "],\nnode_sites: [\n";
        let toward = SPAWNS
            .iter()
            .fold((0.0, 0.0), |a, s| (a.0 + s.0 .0 / 4.0, a.1 + s.0 .1 / 4.0));
        for (s, way) in SITES.iter().zip(&self.hv.plan.ways) {
            let path = if s.kind == Kind::Printer {
                // From the room's edge on, every 400 m or so.
                way.points_from(s.room() + 10.0, 400.0)
            } else {
                Vec::new()
            };
            out += &crate::bake::sites::site_line(s, &path, toward);
        }
        out += "],\n";
        out
    }
}
