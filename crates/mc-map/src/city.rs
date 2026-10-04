//! The city kit: what each city structure ([`PropKind::is_city`]) is, in
//! numbers that the baker lays streets round, the simulation blocks, hits and
//! knocks down, and the models (`mc-models/src/city*.rs`) are built to.
//!
//! A structure is a set of solid parts in its own frame: x along its heading,
//! y to its left, z up from the ground at its origin. Each part is a box
//! `(cx, cy, hx, hy)` (centre and half extents, metres at scale 1) standing
//! from the ground to its `top`. The parts block the ground for walking
//! (`PropKind::solid_plan`), stop shots and the line of fire, and come down
//! together when the structure's health runs out. What a model adds beyond
//! them (a spire, a gate's bridge, eaves) is drawn but never solid.

use crate::format::PropKind;

/// A city structure's numbers. See the module docs for the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Structure {
    /// The model's catalogue key (`mc-models`).
    pub model: &'static str,
    /// Solid parts `(cx, cy, hx, hy)`, metres at scale 1.
    pub plan: &'static [(i32, i32, i32, i32)],
    /// Each part's top over the ground, metres at scale 1; one per `plan` part.
    pub tops: &'static [i32],
    /// Structural health at scale 1; scaled by the square of the prop's scale.
    /// Zero: scenery that nothing can hit or knock down.
    pub health: u32,
    /// How much of the facade is glass, 0 to 255: how much there is to shatter.
    pub glazing: u8,
    /// Whether it takes fire once badly hit (stone and concrete works do not).
    pub burns: bool,
}

const fn s(
    model: &'static str,
    plan: &'static [(i32, i32, i32, i32)],
    tops: &'static [i32],
    health: u32,
    glazing: u8,
    burns: bool,
) -> Structure {
    Structure {
        model,
        plan,
        tops,
        health,
        glazing,
        burns,
    }
}

/// The city wall's run: one [`PropKind::CityWall`] segment is this long along
/// its heading, laid end to end at scale 1.
pub const WALL_SEGMENT_M: i32 = 64;
/// The wall's thickness (y), metres.
pub const WALL_THICK_M: i32 = 20;
/// The wall's parapet over the ground, metres.
pub const WALL_TOP_M: i32 = 26;
/// The road through a [`PropKind::CityGate`], between its towers, metres.
pub const GATE_PASSAGE_M: i32 = 48;

/// The numbers of a city kind; `None` for every other kind.
pub const fn structure(kind: PropKind) -> Option<Structure> {
    use PropKind::*;
    Some(match kind {
        // The outskirts.
        CityHouse => s("city_house", &[(0, 0, 8, 6)], &[9], 1_400, 60, true),
        CityRowhouses => s("city_rowhouses", &[(0, 0, 20, 6)], &[12], 2_600, 70, true),
        CityShops => s("city_shops", &[(0, 0, 22, 9)], &[9], 2_200, 150, true),
        CityFarmstead => s(
            "city_farmstead",
            &[(-6, 0, 14, 9), (14, 4, 4, 4)],
            &[13, 22],
            2_400,
            10,
            true,
        ),
        CityWarehouse => s("city_warehouse", &[(0, 0, 32, 18)], &[13], 5_000, 30, true),
        CityFactory => s(
            "city_factory",
            &[(-6, 0, 30, 22), (30, -12, 4, 4)],
            &[18, 48],
            8_000,
            40,
            true,
        ),
        CityTankFarm => s(
            "city_tank_farm",
            &[
                (14, 14, 11, 11),
                (-14, 14, 11, 11),
                (14, -14, 11, 11),
                (-14, -14, 11, 11),
            ],
            &[16, 16, 16, 16],
            3_000,
            0,
            true,
        ),
        // The city's blocks.
        CityTenement => s("city_tenement", &[(0, 0, 24, 8)], &[21], 5_000, 80, true),
        CityCourtyard => s(
            "city_courtyard",
            &[
                (0, 28, 36, 8),
                (0, -28, 36, 8),
                (28, 0, 8, 20),
                (-28, 0, 8, 20),
            ],
            &[20, 20, 20, 20],
            12_000,
            80,
            true,
        ),
        CityApartments => s(
            "city_apartments",
            &[(0, 0, 30, 9)],
            &[46],
            12_000,
            110,
            true,
        ),
        CityOffice => s("city_office", &[(0, 0, 18, 18)], &[46], 12_000, 190, true),
        CityHighrise => s(
            "city_highrise",
            &[(0, 0, 28, 28), (0, 0, 20, 20)],
            &[14, 132],
            25_000,
            235,
            true,
        ),
        CitySkyscraper => s(
            "city_skyscraper",
            &[(0, 0, 30, 30), (0, 0, 22, 22)],
            &[16, 212],
            45_000,
            245,
            true,
        ),
        CitySpire => s(
            "city_spire",
            &[(0, 0, 36, 36), (0, 0, 26, 26)],
            &[20, 320],
            90_000,
            250,
            true,
        ),
        CitySlab => s(
            "city_slab",
            &[(0, 0, 32, 16), (0, 0, 30, 13)],
            &[12, 110],
            22_000,
            225,
            true,
        ),
        CityCivic => s("city_civic", &[(0, 0, 40, 28)], &[34], 15_000, 60, false),
        CityStation => s("city_station", &[(0, 0, 60, 25)], &[24], 14_000, 140, false),
        CityGarage => s("city_garage", &[(0, 0, 30, 20)], &[14], 9_000, 0, false),
        CityMall => s("city_mall", &[(0, 0, 45, 35)], &[18], 14_000, 160, true),
        CityChurch => s(
            "city_church",
            &[(0, 0, 30, 12), (-34, 0, 7, 7)],
            &[24, 72],
            10_000,
            40,
            true,
        ),
        CityRuin => s("city_ruin", &[(0, 0, 20, 12)], &[15], 2_500, 0, false),
        // The wall.
        CityWall => s(
            "city_wall",
            &[(0, 0, WALL_SEGMENT_M / 2, WALL_THICK_M / 2)],
            &[WALL_TOP_M],
            20_000,
            0,
            false,
        ),
        CityWallTower => s(
            "city_wall_tower",
            &[(0, 0, 16, 16)],
            &[38],
            32_000,
            20,
            false,
        ),
        CityGate => s(
            "city_gate",
            &[
                (0, GATE_PASSAGE_M / 2 + 12, 14, 12),
                (0, -GATE_PASSAGE_M / 2 - 12, 14, 12),
            ],
            &[40, 40],
            40_000,
            20,
            false,
        ),
        CityRubble => s("city_rubble", &[], &[], 0, 0, false),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_city_kind_has_whole_numbers() {
        let mut models = std::collections::BTreeSet::new();
        for kind in PropKind::ALL {
            let Some(s) = structure(kind) else {
                assert!(!kind.is_city(), "{kind:?} is a city kind without numbers");
                continue;
            };
            assert!(kind.is_city(), "{kind:?} has city numbers");
            assert!(models.insert(s.model), "{kind:?} shares model {}", s.model);
            assert_eq!(s.plan.len(), s.tops.len(), "{kind:?}: a top per part");
            assert_eq!(
                s.plan.is_empty(),
                s.health == 0,
                "{kind:?}: solid iff it can be hit"
            );
            assert!(s.tops.iter().all(|&t| t > 0));
            assert!(s.plan.iter().all(|&(_, _, hx, hy)| hx > 0 && hy > 0));
            assert_eq!(kind.solid_plan(), s.plan);
        }
    }

    /// A gate's towers leave the road open between them, and close it beside.
    #[test]
    fn the_gate_passage_is_open() {
        let gate = structure(PropKind::CityGate).unwrap();
        let inner = gate.plan.iter().map(|&(_, cy, _, hy)| cy.abs() - hy).min();
        assert_eq!(inner, Some(GATE_PASSAGE_M / 2));
    }
}
