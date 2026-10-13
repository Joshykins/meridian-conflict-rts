//! The assaults before the match: wrecks where the old columns broke on the
//! Causeway and the bridges' landings, heaviest at their ends on the hold;
//! lighter ones up the ways into each region and on the hold's beaches where
//! ships ran ashore.

use super::*;
use crate::bake::planned_wrecks::Field;
use crate::format::MapWreck;

/// The fields: centre, radius and salvage weight.
const FIELDS: [Field; 16] = [
    // The Causeway, and its landing on each shore.
    ((11_650.0, 10_200.0), 230.0, 100),
    ((10_450.0, 10_250.0), 220.0, 90),
    ((12_900.0, 10_250.0), 210.0, 70),
    // The north bridge's landings and the way up to it.
    ((7_752.0, 15_000.0), 190.0, 80),
    ((7_900.0, 12_350.0), 200.0, 70),
    ((7_500.0, 15_150.0), 190.0, 50),
    // The south bridge's landings and the way up to it.
    ((8_400.0, 5_350.0), 200.0, 80),
    ((8_350.0, 7_950.0), 200.0, 70),
    ((8_100.0, 5_200.0), 190.0, 50),
    // The west bridge's landings and the way up to it.
    ((3_350.0, 10_152.0), 170.0, 70),
    ((5_800.0, 10_200.0), 200.0, 60),
    ((3_200.0, 10_300.0), 170.0, 40),
    // Where ships put ashore on the hold.
    ((5_500.0, 12_350.0), 150.0, 30),
    ((10_350.0, 12_250.0), 150.0, 30),
    ((5_650.0, 8_250.0), 150.0, 30),
    ((10_400.0, 8_300.0), 150.0, 30),
];

impl Terrain {
    pub(in crate::bake) fn rimehold_wrecks(&self) -> Vec<MapWreck> {
        self.planned_wrecks(&FIELDS, 0x7269_6d65)
    }
}
