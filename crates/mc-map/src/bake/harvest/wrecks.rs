//! The assaults before the match: wrecks along the ways the waves take into
//! the heartland. The heaviest where the old columns broke, at the mouths of
//! the passes and the valleys and up the base's valley; lighter ones on the
//! approaches to the starts and on the beaches where ships landed.

use super::*;
use crate::bake::planned_wrecks::Field;
use crate::format::MapWreck;

/// The fields: centre, radius and salvage weight.
const FIELDS: [Field; 17] = [
    // Up the valley to the base.
    ((12_300.0, 16_400.0), 230.0, 100),
    ((11_800.0, 13_800.0), 220.0, 90),
    ((12_700.0, 11_300.0), 220.0, 80),
    // The desert's passes' mouths.
    ((8_350.0, 11_650.0), 210.0, 80),
    ((9_700.0, 16_250.0), 200.0, 70),
    // The alpine valleys' mouths.
    ((16_250.0, 11_350.0), 210.0, 80),
    ((15_000.0, 16_000.0), 190.0, 60),
    // The fjords' way past the sound's head; the sandbar's landward end.
    ((17_850.0, 8_850.0), 200.0, 70),
    ((6_600.0, 8_550.0), 200.0, 70),
    // On toward the starts.
    ((10_150.0, 7_500.0), 200.0, 50),
    ((12_300.0, 8_300.0), 200.0, 60),
    ((14_550.0, 7_650.0), 200.0, 50),
    ((9_000.0, 6_000.0), 180.0, 40),
    ((15_600.0, 6_200.0), 180.0, 40),
    // Where ships put ashore, on the beaches below the starts.
    ((10_650.0, 2_950.0), 160.0, 30),
    ((14_100.0, 3_050.0), 160.0, 30),
    ((8_000.0, 3_300.0), 160.0, 25),
];

impl Terrain {
    pub(in crate::bake) fn harvest_wrecks(&self) -> Vec<MapWreck> {
        self.planned_wrecks(&FIELDS, 0x6861_7276)
    }
}
