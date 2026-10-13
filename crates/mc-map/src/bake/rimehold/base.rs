//! The Progenitor's base on the mainland east of the hold: the shared
//! installation (`bake/facility.rs`) laid on its level floor at [`BASE_H`],
//! facing west down the Causeway; a sea gate on each lagoon either side of
//! the Causeway's landing, its slips printing the rounds' ships.

use super::shape::{SEA_GATES, SEA_GATE_HEADING};
use super::*;
use crate::bake::machine::{Machine, PrecursorSite};
use crate::format::PropKind;

/// A sea gate's slips across its channel (its y), each at `SLIP_X`, and
/// where a slip's print beam leaves its projector head (x, height)
/// (`precursor_forge.rs`); the biggest ship each takes.
#[cfg(test)]
const SLIPS: [f64; 3] = [-110.0, 0.0, 110.0];
#[cfg(test)]
const SLIP_X: f64 = -150.0;
#[cfg(test)]
const SLIP_EMITTER: (f64, f64) = (-230.0, 140.0);
#[cfg(test)]
const SLIP_FITS: f64 = 38.0;
#[cfg(test)]
const GREAT_SLIP_FITS: f64 = 180.0;

impl Terrain {
    /// The sea gates.
    pub(super) fn rh_sea_gates(&self) -> Vec<PrecursorSite> {
        SEA_GATES
            .iter()
            .map(|&(x, y)| PrecursorSite {
                kind: PropKind::PrecursorSeaGate,
                x,
                y,
                heading: SEA_GATE_HEADING,
                scale: 1.0,
            })
            .collect()
    }

    /// The machine: the tower ring round the Heart, the bastions either side.
    pub(in crate::bake) fn machine_rimehold(&self) -> Machine<'_> {
        self.facility_machine(&FACILITY)
    }

    /// The sidecar's facility block for the survival rules: the Heart, the
    /// print bays and slips, the sites.
    #[cfg(test)]
    pub(super) fn rimehold_block(&self) -> String {
        use crate::bake::facility::world;
        let mut out = self.facility_bays(&FACILITY);
        for &g in &SEA_GATES {
            for &sy in &SLIPS {
                let stand = world(g, SEA_GATE_HEADING, (SLIP_X, sy));
                let head = world(g, SEA_GATE_HEADING, (SLIP_EMITTER.0, sy));
                let fits = if sy == 0.0 {
                    GREAT_SLIP_FITS
                } else {
                    SLIP_FITS
                };
                out += &format!(
                    "    (at: ({:.0}, {:.0}), facing: 180, emitter: ({:.0}, {:.0}, {:.0}), domain: Naval, max_radius: Some({fits:.0})),\n",
                    stand.0, stand.1, head.0, head.1, SLIP_EMITTER.1
                );
            }
        }
        out += "],\nnode_sites: [\n";
        for (s, way) in SITES.iter().zip(&self.rh.plan.ways) {
            let path = if s.kind == Kind::Printer {
                // From the room's edge on, every 400 m or so.
                way.points_from(s.room() + 10.0, 400.0)
            } else {
                Vec::new()
            };
            out += &crate::bake::sites::site_line(s, &path, MIDDLE);
        }
        out += "],\n";
        out
    }
}
