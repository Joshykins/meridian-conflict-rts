//! The assaults before the match: wrecks along the road the waves take, from
//! the forecourt under the Rampart, through the pass and across the lowland to
//! each defender start. The heaviest lie where the old assaults broke (the
//! forecourt and the pass), lighter ones on the way to the starts. Laid with
//! the plan (`planned_wrecks.rs`), since the map has no symmetry for
//! `wreckage::stamp` to copy by.

use super::*;
use crate::format::MapWreck;

/// The fields along the road (design metres): centre, radius and salvage
/// weight. Forecourt, pass, the lowland, then a branch to each start.
const FIELDS: [((f64, f64), f64, u32); 10] = [
    ((8_850.0, 8_050.0), 220.0, 100),
    ((8_900.0, 8_650.0), 200.0, 80),
    ((7_700.0, 8_450.0), 200.0, 90),
    ((6_900.0, 8_350.0), 200.0, 70),
    ((5_900.0, 7_950.0), 220.0, 70),
    ((5_000.0, 7_250.0), 200.0, 60),
    ((4_300.0, 8_250.0), 200.0, 50),
    ((3_400.0, 7_650.0), 180.0, 45),
    ((4_400.0, 6_650.0), 180.0, 40),
    ((3_000.0, 8_900.0), 180.0, 40),
];

impl Terrain {
    pub(in crate::bake) fn threshold_wrecks(&self) -> Vec<MapWreck> {
        let fields: Vec<_> = FIELDS
            .iter()
            .map(|&(c, r, w)| (self.tv(c), self.tl(r), w))
            .collect();
        self.planned_wrecks(&fields, 0x7468_7265)
    }
}
