//! The deck either side of the tunnel: the plated ramps aft of the main deck, and single
//! fittings, each its own size and none repeated in rows: round pads, domed hatches and
//! sensor blisters, raised housings, access covers, recessed circuit bays, two small
//! louvred vents; and the stepped transom under the landing mouth with its rods.

use super::*;

/// The ramps' top, aft on the main deck.
pub(super) const BLOCK_TOP: f32 = 10.6;

/// One fitting on a deck.
#[derive(Clone, Copy)]
enum Fit {
    /// A round pad with a raised rim, its radius.
    Pad(f32),
    /// A domed hatch or a sensor blister, its radius.
    Dome(f32),
    /// A raised housing: its plan and height.
    Housing(Vec2, f32),
    /// A bolted access cover: its plan.
    Cover(Vec2),
    /// A recessed service bay cut with circuit runs: its plan.
    Bay(Vec2),
    /// A small louvred vent: its plan.
    Vent(Vec2),
}

/// The fittings: where each stands (x, y, the deck's height there) and what it is.
const FITTINGS: [(f32, f32, f32, Fit); 24] = [
    // The stern's lower deck, over the tunnel and either side of it.
    (-56.0, 2.6, STERN_DECK, Fit::Dome(0.8)),
    (-53.6, -3.0, STERN_DECK, Fit::Dome(1.2)),
    (-52.0, 10.8, STERN_DECK, Fit::Pad(2.3)),
    (-52.0, -10.8, STERN_DECK, Fit::Pad(2.3)),
    (-48.0, 11.2, STERN_DECK, Fit::Vent(Vec2::new(2.4, 3.0))),
    // On the ramps.
    (-35.0, 12.2, BLOCK_TOP, Fit::Dome(1.4)),
    (-28.0, 11.2, BLOCK_TOP, Fit::Cover(Vec2::new(3.0, 2.2))),
    (
        -33.0,
        -12.0,
        BLOCK_TOP,
        Fit::Housing(Vec2::new(3.4, 2.6), 1.0),
    ),
    (-41.0, -11.4, BLOCK_TOP, Fit::Bay(Vec2::new(3.6, 3.0))),
    // The main deck round the cells.
    (-21.5, 12.0, DECK, Fit::Bay(Vec2::new(3.0, 4.0))),
    (-4.0, 12.5, DECK, Fit::Housing(Vec2::new(4.0, 2.4), 1.2)),
    (4.5, 10.5, DECK, Fit::Cover(Vec2::new(2.4, 2.0))),
    (10.5, 13.0, DECK, Fit::Dome(1.0)),
    (-21.0, -10.5, DECK, Fit::Cover(Vec2::new(2.2, 2.8))),
    (-3.5, -12.0, DECK, Fit::Vent(Vec2::new(2.6, 3.2))),
    (6.0, -12.5, DECK, Fit::Bay(Vec2::new(5.0, 3.4))),
    (11.5, -9.6, DECK, Fit::Housing(Vec2::new(2.2, 3.0), 0.7)),
    (-7.5, 8.0, DECK, Fit::Cover(Vec2::new(1.6, 1.4))),
    (1.0, -7.6, DECK, Fit::Dome(0.7)),
    // Beside the plateau's steps.
    (17.0, 12.6, DECK, Fit::Bay(Vec2::new(4.0, 5.0))),
    (27.5, -14.6, DECK, Fit::Cover(Vec2::new(2.8, 1.8))),
    (22.0, -13.2, DECK, Fit::Dome(0.9)),
    (28.0, 14.8, DECK, Fit::Housing(Vec2::new(2.6, 1.6), 0.6)),
    (-44.0, -13.8, BLOCK_TOP, Fit::Cover(Vec2::new(1.4, 1.6))),
];

pub(super) fn build(b: &mut MeshBuilder) {
    for side in [-1.0, 1.0] {
        ramp(b, side);
    }
    for (x, y, z, fit) in FITTINGS {
        let at = v3(x, y, z);
        match fit {
            Fit::Pad(r) => kit::pad(b, at, r),
            Fit::Dome(r) => kit::dome(b, at, r),
            Fit::Housing(plan, h) => kit::housing(b, at, plan, h),
            Fit::Cover(plan) => kit::cover(b, at, plan),
            Fit::Bay(plan) => kit::bay(b, at, plan),
            Fit::Vent(plan) => {
                b.paint(PLATING).pattern(pattern::LOUVRE);
                b.plate(at, plan, 0.14, 0.03);
            }
        }
    }
    transom(b);
}

/// The ramp at the after end of the main deck: from the deck at its inner edge it rises
/// to a flat top.
fn ramp(b: &mut MeshBuilder, side: f32) {
    let profile: Vec<[f32; 2]> = [
        [7.0, DECK],
        [10.0, BLOCK_TOP],
        [14.8, BLOCK_TOP],
        [15.2, BLOCK_TOP - 0.4],
        [15.2, DECK],
    ]
    .iter()
    .map(|&[y, z]| [side * y, z])
    .collect();
    b.paint(PLATING).pattern(pattern::HULL_PLATE);
    b.extrude_x(&profile, STERN_STEP + 0.8, -24.0);
}

/// The stern under the landing mouth: a dark transom stepped back in two tiers under the
/// apron, and three thin rods standing aft from it.
fn transom(b: &mut MeshBuilder) {
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(AFT - 2.4, -11.0, -0.6), v3(AFT + 0.5, 11.0, FLOOR - 0.1));
    b.block(v3(AFT - 4.0, -7.0, -0.3), v3(AFT - 2.4, 7.0, FLOOR - 0.6));
    if !b.mid() {
        return;
    }
    b.paint(METAL);
    for y in [-3.0, 0.0, 3.0] {
        b.cylinder_between(
            v3(AFT - 4.0, y, 0.4),
            v3(AFT - 10.0, y, 0.4),
            0.16,
            0.1,
            b.sides(6),
        );
    }
}
