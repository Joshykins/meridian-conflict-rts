//! The Dominion's hull: an arrowhead in plan, a little broader astern
//! and drawn to a point at the bore, riding tall on a segmented keel blade down its centre
//! line. The upper hull's underside rises from the blade out to a hard chine, its walls
//! stand tall above it; the spinal AEB runs inside the blade, each coil stage glowing in a
//! gap between the blade's armoured segments. An angular prow block overhangs the blade's
//! raked chin with the bore in its face; the casemates stand out of the flanks on stepped
//! sponsons; an engineering block under the stern carries the drives. The stacked layers
//! on its deck are [`super::stacked`]'s.

use super::*;

/// The upper hull: its underside at the blade, its chine and wall, its deck.
pub(super) const HULL_BOTTOM: f32 = 62.0;
const CHINE_Z: f32 = 72.0;
const WALL_TOP: f32 = 88.0;
pub(super) const DECK: f32 = 96.0;
/// The keel blade: its half width and its ends (it runs into the engineering block astern
/// and under the prow block forward).
const BLADE: f32 = 22.0;
const BLADE_AFT: f32 = -150.0;
const BLADE_FORE: f32 = 212.0;
/// Each coil stage shows in a gap this long in the blade.
const GAP: f32 = 8.0;
/// Where the hull runs into the prow block, and the bow face.
pub(super) const HULL_FORE: f32 = 204.0;
pub(super) const BOW: f32 = 248.0;
/// The coil rings' radius about the bore, and where the bore begins.
pub(super) const COIL_R: f32 = 14.0;
pub(super) const BREECH_X: f32 = -46.0;
/// The coil stages along the bore (x), breech to mouth: clear of the forward legs' bays,
/// the lift jets and the floods, which need the blade's belly.
pub(super) const STAGES: [f32; 8] = [-34.0, -4.0, 26.0, 56.0, 132.0, 170.0, 184.0, MUZZLE_X - 8.0];

/// The AEB (`models::spinal_bore`), the same under every deck: seven stages of coils in
/// the blade's gaps, the last in the mouth.
pub(crate) const BORE: SpinalBore = SpinalBore {
    muzzle: [MUZZLE_X, 0.0, AXIS_Z],
    breech: [BREECH_X, 0.0, AXIS_Z],
    coils: [
        [STAGES[0], 0.0, AXIS_Z],
        [STAGES[1], 0.0, AXIS_Z],
        [STAGES[2], 0.0, AXIS_Z],
        [STAGES[3], 0.0, AXIS_Z],
        [STAGES[4], 0.0, AXIS_Z],
        [STAGES[5], 0.0, AXIS_Z],
        [STAGES[6], 0.0, AXIS_Z],
        [STAGES[7], 0.0, AXIS_Z],
    ],
    coil_radius: COIL_R,
    // The blade's flanks below the hull's underside, and its keel.
    skin: [
        BLADE + 0.6,
        AXIS_Z - (HULL_BOTTOM - 6.0),
        AXIS_Z - KEEL + 0.6,
    ],
};

/// The plan: x, the upper hull's half width at its chine. One tapering hull: a little
/// broader astern than through the casemates, its stern cut square, drawn hard in to the
/// prow block.
const PLAN: [[f32; 2]; 7] = [
    [STERN, 76.0],
    [-218.0, 76.0],
    [-120.0, 74.0],
    [-70.0, 70.0],
    [90.0, 66.0],
    [170.0, 56.0],
    [HULL_FORE, 48.0],
];

/// The hull's half width at its chine at `x`.
pub(super) fn width(x: f32) -> f32 {
    lerp_rows(&PLAN, x)[1]
}

/// The deck's edge (half width) at `x`, inboard of the wall's shoulder.
pub(super) fn deck_edge(x: f32) -> f32 {
    width(x) - 10.0
}

/// The upper hull's port half at `x`: 0 underside middle, 1 underside at the blade,
/// 2 underside out, 3 chine, 4 wall head, 5 shoulder, 6 deck edge, 7 the deck's middle.
pub(super) fn hull_half(x: f32) -> Vec<[f32; 2]> {
    let w = width(x);
    vec![
        [0.0, HULL_BOTTOM],
        [BLADE - 2.0, HULL_BOTTOM],
        [w - 12.0, HULL_BOTTOM + 6.0],
        [w, CHINE_Z],
        [w - 1.0, WALL_TOP],
        [w - 6.0, DECK - 2.0],
        [w - 10.0, DECK],
        [0.0, DECK],
    ]
}

/// The blade's port half at `x`: its keel rakes up into a chin under the prow block.
fn blade_half(x: f32) -> Vec<[f32; 2]> {
    let k = if x > 190.0 {
        KEEL + (x - 190.0) / (BLADE_FORE - 190.0) * 17.0
    } else {
        KEEL
    };
    vec![
        [0.0, k],
        [BLADE - 4.0, k],
        [BLADE, k + 5.0],
        [BLADE, HULL_BOTTOM - 6.0],
        [BLADE - 2.0, HULL_BOTTOM + 1.0],
        [0.0, HULL_BOTTOM + 1.0],
    ]
}

/// The engineering block under the stern: x, half width, top, belly.
const ENGINE: [[f32; 4]; 4] = [
    [-196.0, 44.0, 64.0, 38.0],
    [-184.0, 60.0, 72.0, KEEL],
    [-152.0, 60.0, 72.0, KEEL],
    [-128.0, 30.0, 66.0, 40.0],
];

fn engine_half(x: f32) -> Vec<[f32; 2]> {
    let [_, w, top, bot] = lerp_rows(&ENGINE, x);
    vec![
        [0.0, bot],
        [w - 24.0, bot],
        [w - 10.0, bot + 9.0],
        [w, bot + 21.0],
        [w, top - 4.0],
        [w - 6.0, top],
        [0.0, top],
    ]
}

/// Everything under the stacked layers: the hull and its flanks, the blade and its coils,
/// the front and the mouth, the engineering block and the drives, the casemates on their
/// sponsons, the rifles, the belly and the lamps.
pub(super) fn build(b: &mut MeshBuilder, lamps_of: &crate::CapitalLamps) {
    hull(b);
    super::flanks::build(b);
    blade(b);
    super::prow::build(b);
    engine(b);
    coils(b, &BORE, 7, 2, 4.0, BOW - 2.0);
    b.mirror_y(|b| {
        sponson(b, CASEMATES[0], 56.0);
        sponson(b, CASEMATES[2], 60.0);
    });
    for (i, p) in CASEMATES.into_iter().enumerate() {
        let face = if i < 2 { 56.0 } else { 60.0 };
        casemate(b, 1 + i, Vec3::from(p), face);
    }
    rifles(b);
    belly(b);
    lamps(b, lamps_of);
}

/// The strategy-zoom hull: the arrowhead and the prow.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.extrude_z(
        &[
            [HULL_FORE, 48.0],
            [HULL_FORE, -48.0],
            [90.0, -66.0],
            [STERN, -76.0],
            [STERN, 76.0],
            [90.0, 66.0],
        ],
        KEEL,
        DECK,
    );
    super::prow::coarse(b);
}

/// The upper hull, lofted along the arrowhead.
fn hull(b: &mut MeshBuilder) {
    // The stern is raked: the hull's first section drawn in and down toward its middle.
    let raked = |x: f32| {
        let mid = (HULL_BOTTOM + DECK) * 0.5;
        let h: Vec<[f32; 2]> = hull_half(x)
            .iter()
            .map(|p| [p[0] * 0.9, mid + (p[1] - mid) * 0.72])
            .collect();
        full_ring(x, &h)
    };
    let mut rings = vec![raked(STERN)];
    rings.extend(
        [-214.0, -120.0, -70.0, 90.0, 170.0, HULL_FORE]
            .iter()
            .map(|&x| full_ring(x, &hull_half(x))),
    );
    b.paint(PLATING_DARK).pattern(pattern::GENERIC);
    b.loft(&rings, true, true);
}

/// The keel blade: armoured segments down the centre line under the hull, gaps between
/// them where each coil stage shows; armour slabs on each segment's flanks.
fn blade(b: &mut MeshBuilder) {
    let mut edges = vec![BLADE_AFT];
    for x in STAGES.iter().take(7) {
        edges.push(x - GAP * 0.5);
        edges.push(x + GAP * 0.5);
    }
    edges.push(BLADE_FORE);
    let half = |x: f32| blade_half(x);
    for seg in edges.chunks(2) {
        let (x0, x1) = (seg[0], seg[1]);
        let mut xs = vec![x0];
        if x0 < 190.0 && x1 > 190.0 {
            xs.push(190.0);
        }
        xs.push(x1);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        hull_loft(b, &xs, &half);
        if x1 - x0 > 12.0 {
            b.mirror_y(|b| {
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                plate_on(b, &half, [x0 + 2.0, x1 - 2.0], 2, 4, [0.1, 0.6], 1.4);
            });
        }
    }
    // The breech housing in the blade where the bore begins.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let ring = octagon(COIL_R + 1.0)
        .iter()
        .map(|p| [p[0], p[1] + AXIS_Z])
        .collect::<Vec<_>>();
    b.extrude_x(&ring, BREECH_X - 4.0, BREECH_X + 2.0);
}

/// The engineering block under the stern, broad over the drives, armoured in two tiers.
fn engine(b: &mut MeshBuilder) {
    let fine = b.fine();
    let xs: Vec<f32> = ENGINE.iter().map(|r| r[0]).collect();
    b.paint(PLATING_DARK).pattern(pattern::GENERIC);
    hull_loft(b, &xs, &engine_half);
    let half = |x: f32| engine_half(x);
    b.mirror_y(|b| {
        let panels: &[(f32, f32)] = if fine {
            &[(-214.0, -190.0), (-186.0, -166.0), (-162.0, -146.0)]
        } else {
            &[(-214.0, -146.0)]
        };
        for &(x0, x1) in panels {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            plate_on(b, &half, [x0, x1], 2, 4, [0.3, 0.5], 1.8);
        }
    });
    super::stern::build(b);
}

/// A casemate's sponson (port; mirrored): two armoured steps run out of the blade's and
/// the hull's undersides under the drum, the outer one's face at `face`.
fn sponson(b: &mut MeshBuilder, pivot: [f32; 3], face: f32) {
    let [x, _, z] = pivot;
    let step = |x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32, cut: f32| {
        [x0, x1]
            .map(|x| {
                vec![
                    v3(x, y0, z0),
                    v3(x, y1 - cut, z0),
                    v3(x, y1, z0 + cut),
                    v3(x, y1, z1),
                    v3(x, y0, z1),
                ]
            })
            .to_vec()
    };
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &step(
            x - 22.0,
            x + 22.0,
            BLADE - 1.0,
            44.0,
            z - 16.0,
            HULL_BOTTOM + 8.0,
            6.0,
        ),
        true,
        true,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &step(x - 15.0, x + 15.0, 40.0, face, z - 11.0, z + 12.0, 4.0),
        true,
        true,
    );
}

/// The ship's lamps on this hull, with its `strobes`.
pub(super) const fn lamps_for(strobes: &'static [[f32; 3]]) -> crate::CapitalLamps {
    crate::CapitalLamps {
        floods: &FLOODS,
        nav_port: [-140.0, 75.0, 72.0],
        nav_starboard: [-140.0, -75.0, 72.0],
        strobes,
        beacons: &[],
        hold: None,
    }
}

/// The bow's and the stern's strobes, which the ship carries besides its mast's.
pub(super) const BOW_STROBES: [[f32; 3]; 4] = [
    [200.0, 49.6, 72.0],
    [200.0, -49.6, 72.0],
    [-212.0, 64.0, DECK + 0.8],
    [-212.0, -64.0, DECK + 0.8],
];
