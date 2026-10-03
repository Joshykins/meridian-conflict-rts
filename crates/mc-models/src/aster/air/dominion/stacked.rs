//! The Dominion's upper hulls, stacked on the main one ([`super::hull`]): a second,
//! narrower armoured hull lies on the main one, set back from its bow with a raked prow
//! and a raked stern of its own, and a third, smaller one lies on that carrying the bridge,
//! set back further, so the ship reads as three stacked masses. Swept-back vertebrae wrap
//! the upper layers like armour bands.

use super::hull::{self, layer_edge, DECK};
use super::*;

/// The second layer: its top, and its plan (x, share of the deck's half width, top).
/// Raked up from its stern, full over the ship's length, raked down to the deck forward
/// over the prow.
pub(super) const L2_TOP: f32 = 112.0;
const L2: [[f32; 3]; 6] = [
    [-214.0, 0.58, DECK + 6.0],
    [-202.0, 0.66, L2_TOP],
    [-70.0, 0.66, L2_TOP],
    [90.0, 0.66, L2_TOP],
    [200.0, 0.66, L2_TOP],
    [222.0, 0.4, DECK + 2.0],
];
/// The third layer, as the second: set back from both ends, carrying the bridge.
pub(super) const L3_TOP: f32 = 134.0;
const L3: [[f32; 3]; 4] = [
    [-120.0, 0.26, L2_TOP + 6.0],
    [-110.0, 0.3, L3_TOP],
    [40.0, 0.3, L3_TOP],
    [74.0, 0.2, L2_TOP + 4.5],
];
/// The vertebrae: over the third layer, and over the second clear of the rifles.
const RIBS_3: [f32; 4] = [-106.0, -92.0, 14.0, 36.0];
const RIBS_2: [f32; 5] = [-204.0, -186.0, 84.0, 104.0, 128.0];
const RIB_H: f32 = 3.5;

const MAST_X: f32 = -62.0;
const MAST_TOP: f32 = 158.0;
const SHIELD: Vec3 = Vec3::new(-45.0, 0.0, 144.4);

const STROBES: [[f32; 3]; 5] = [
    [MAST_X, 0.0, MAST_TOP + 1.2],
    hull::BOW_STROBES[0],
    hull::BOW_STROBES[1],
    hull::BOW_STROBES[2],
    hull::BOW_STROBES[3],
];
pub(crate) const LAMPS: crate::CapitalLamps = hull::lamps_for(&STROBES);

/// A layer's half width and top at `x`, from its plan table.
fn layer<const N: usize>(plan: &[[f32; 3]; N], x: f32) -> (f32, f32) {
    let [_, share, top] = lerp_rows(plan, x);
    (layer_edge(x) * share, top)
}

/// The second layer's half width at `x`.
pub(super) fn l2_width(x: f32) -> f32 {
    layer(&L2, x).0
}

/// The third layer's half width at `x`.
pub(super) fn l3_width(x: f32) -> f32 {
    layer(&L3, x).0
}

/// A layer's port half section: its foot on `foot`, chamfered sides, a flat top.
pub(super) fn layer_half(w: f32, foot: f32, top: f32) -> Vec<[f32; 2]> {
    let h = top - foot;
    vec![
        [0.0, foot - 0.5],
        [w, foot - 0.5],
        [w - 1.5, foot + h * 0.35],
        [w - 3.0, top - 2.0],
        [w - 6.0, top],
        [0.0, top],
    ]
}

pub(super) fn build(b: &mut MeshBuilder, bow: super::prow::Bow) {
    if b.coarse() {
        hull::coarse(b, bow);
        b.paint(PLATING);
        b.extrude_z(
            &[
                [200.0, 32.0],
                [200.0, -32.0],
                [-214.0, -44.0],
                [-214.0, 44.0],
            ],
            DECK,
            L2_TOP,
        );
        b.cuboid_open(v3(-30.0, 0.0, L3_TOP - 7.0), v3(150.0, 30.0, 14.0));
        return;
    }
    hull::build(b, &LAMPS, bow);
    layers(b);
    super::detail::build(b);
    for x in RIBS_3 {
        band(b, x, true);
    }
    for x in RIBS_2 {
        band(b, x, false);
    }
    for p in RIFLES {
        rifle_pedestal(b, p, DECK - 1.0);
    }
    sam_cells(b, L2_TOP - 1.0);
    bridge(b);
}

/// The two upper layers, each lofted along its plan with its own chamfered sides and
/// raked ends; belts down the second layer's sides, a dark band at each layer's foot,
/// ports along the third's, hatches on the second's back.
fn layers(b: &mut MeshBuilder) {
    let fine = b.fine();
    let ring2 = |x: f32| {
        let (w, top) = layer(&L2, x);
        full_ring(x, &layer_half(w, DECK, top))
    };
    let ring3 = |x: f32| {
        let (w, top) = layer(&L3, x);
        full_ring(x, &layer_half(w, L2_TOP, top))
    };
    b.paint(PLATING).pattern(pattern::WARSHIP);
    b.loft(
        &L2.iter().map(|r| ring2(r[0])).collect::<Vec<_>>(),
        true,
        true,
    );
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.loft(
        &L3.iter().map(|r| ring3(r[0])).collect::<Vec<_>>(),
        true,
        true,
    );
    let half2 = |x: f32| {
        let (w, top) = layer(&L2, x);
        layer_half(w, DECK, top)
    };
    b.mirror_y(|b| {
        let belts: &[(f32, f32)] = if fine {
            &[
                (-196.0, -150.0),
                (-146.0, -100.0),
                (-40.0, 6.0),
                (10.0, 60.0),
                (96.0, 150.0),
                (156.0, 198.0),
            ]
        } else {
            &[(-196.0, -100.0), (-40.0, 198.0)]
        };
        for &(x0, x1) in belts {
            b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
            plate_on(b, &half2, [x0, x1], 1, 3, [0.2, 0.6], 1.2);
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for (plan, foot, from, to) in [
            (&L2[..], DECK, -200.0, 200.0),
            (&L3[..], L2_TOP, -108.0, 40.0),
        ] {
            let band = |x: f32| {
                let [_, share, _] = lerp_rows(plan, x);
                let w = layer_edge(x) * share;
                vec![
                    v3(x, w - 0.6, foot - 0.4),
                    v3(x, w + 0.8, foot - 0.4),
                    v3(x, w + 0.8, foot + 0.8),
                    v3(x, w - 0.6, foot + 0.8),
                ]
            };
            b.loft(&[band(from), band(to)], true, true);
        }
        if fine {
            b.paint(TREAD).pattern(pattern::NONE);
            let mut x = -100.0;
            while x < 36.0 {
                let (w, _) = layer(&L3, x);
                b.block(v3(x - 1.5, w - 2.2, 119.0), v3(x + 1.5, w - 1.2, 121.0));
                x += 8.0;
            }
            let mut x = -196.0;
            while x < 196.0 {
                let clear = RIBS_2
                    .iter()
                    .chain(RIBS_3.iter())
                    .all(|r| (x - r).abs() > 9.0)
                    && !(-40.0..0.0).contains(&x)
                    && RIFLES.iter().all(|p| (x - p[0]).abs() > 16.0);
                if clear {
                    let (w2, _) = layer(&L2, x);
                    let w3 = if (-120.0..74.0).contains(&x) {
                        layer(&L3, x).0
                    } else {
                        0.0
                    };
                    let y = (w2 - 6.0 + w3) * 0.5;
                    b.paint(ACCENT).pattern(pattern::PLAIN);
                    b.plate(v3(x, y, L2_TOP), v2(7.0, 5.0), 0.35, 0.12);
                    b.paint(METAL).pattern(pattern::PLAIN);
                    b.plate(v3(x, y, L2_TOP + 0.35), v2(4.4, 2.6), 0.2, 0.08);
                }
                x += 26.0;
            }
        }
    });
    b.paint(TEAM).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let (w, _) = layer(&L2, 60.0);
        b.block(
            v3(56.0, w - 9.0, L2_TOP - 0.2),
            v3(64.0, w - 4.0, L2_TOP + 0.5),
        );
    });
}

/// A vertebra wrapping a layer at `x` (the third when `upper`, else the second): an
/// armour band over the layer's back and down its sides onto the level below.
fn band(b: &mut MeshBuilder, x: f32, upper: bool) {
    let (w, top, foot) = if upper {
        let (w, t) = layer(&L3, x);
        (w, t, L2_TOP)
    } else {
        let (w, t) = layer(&L2, x);
        (w, t, DECK)
    };
    let h = top - foot;
    let under = [
        [0.0, top - 0.8],
        [w - 6.0, top - 0.8],
        [w - 3.0, top - 2.8],
        [w - 1.5, foot + h * 0.35],
        [w + 3.0, foot - 0.6],
    ];
    let over = [
        [0.0, top + RIB_H],
        [w - 5.0, top + RIB_H],
        [w - 0.5, top - 1.0],
        [w + 1.5, foot + h * 0.35],
        [w + 3.0, foot + 2.0],
    ];
    rib(b, x, &under, &over, 6.0, 0.14);
}

/// The bridge on the third layer: stepped tiers, the glass under its brow, the mast and
/// radar, the shield projector on its forward tier.
fn bridge(b: &mut MeshBuilder) {
    tier(b, [-82.0, -40.0], 12.0, [L3_TOP - 0.5, 140.0], 4.0, 5.0);
    tier(b, [-78.0, -52.0], 9.0, [140.0, 146.0], 3.0, 3.0);
    b.paint(GLASS).pattern(pattern::PLAIN);
    b.at(v3(-65.0, 0.0, 0.0), |b| {
        b.loft_z(
            &pointed_plan(11.0, 8.0, 2.5),
            &[Section::new(146.0, 1.0), Section::new(148.4, 0.98)],
        );
    });
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.at(v3(-65.5, 0.0, 0.0), |b| {
        b.loft_z(
            &pointed_plan(12.5, 8.8, 3.0),
            &[
                Section::new(148.4, 1.0),
                Section::scaled(150.6, 0.9, 0.9).shifted(-1.0, 0.0),
            ],
        );
    });
    mast(b, MAST_X, 150.6, MAST_TOP);
    shield_projector(b, SHIELD);
}
