//! The Leviathan: the tech 3 battleship, the hero of the navy.
//!
//! A 142 m dreadnought with nine Arc Howitzers: a low, sleek, aggressive hull under a tall
//! pagoda. The hull sits low in the water with a sharply raked clipper bow and
//! a ram-like cutwater fin at the forefoot, a hard chine knuckle along its
//! flanks (a dark belt under it, the side tumbling in above it to a dark
//! deck-edge lip), a raised forecastle that steps down to a cut-down
//! quarterdeck, and a sculpted, raked transom with twin fins. Angular flared
//! sponson wings stand out along the citadel and carry four twin secondary
//! turrets, two a side, that rest trained outboard. Three flat, hard-edged
//! triple gunhouses on low faceted bases (two forward, superfiring, one
//! astern), each carrying three of the Trebuchet's Arc Howitzers
//! (`bolt_rifle::siege_howitzer`). Over the citadel, a tall, slender, stepped pagoda tower with a
//! raked face, the bridge glass high up under a dark brow, rangefinder arms and
//! a tall mast; behind it a low raked stack block and a stepped aft
//! superstructure carrying the light AA mount. Four missile-defence laser heads
//! (static, not guns) stand on the tower's flying bridge wings and on pylons
//! aft. No emitters: it is a warship, not a starship. Its dark parts are painted
//! `pattern::PLAIN`, so the surface shader lays no lights into them. The only light on it is a
//! ship's own (`lights`): red and green sidelights on the bridge wings, a white
//! masthead light, a stern light, a few warm deck lamps and lit scuttles, plus the
//! missile-defence lasers' red, and the howitzers' own blue plasma cells.
//! The waterline is z = 0, the keel goes to -11.5.

use super::*;
use crate::aster::bolt_rifle::siege_howitzer;

// ---- the hull --------------------------------------------------------------

/// Frames stern first: (x, keel z, half beam at the chine, half beam at the deck
/// edge, deck z). The hull is widest at the chine just above the waterline and
/// tumbles in to a low deck; forward the deck edge flares out past the chine, and
/// the deck rises with the sheer to the prow.
const FRAMES: [[f32; 5]; 16] = [
    [-70.0, -2.6, 7.4, 7.6, 6.3],
    [-64.0, -6.0, 9.4, 9.2, 6.1],
    [-56.0, -8.6, 11.0, 10.2, 6.0],
    [-44.0, -10.4, 12.2, 10.8, 6.1],
    [-30.0, -11.2, 12.8, 11.0, 6.3],
    [-12.0, -11.5, 13.0, 11.0, 6.5],
    [4.0, -11.5, 13.0, 11.0, 6.6],
    [18.0, -11.3, 12.7, 10.9, 7.0],
    [30.0, -10.8, 12.0, 10.8, 7.6],
    [40.0, -10.0, 10.9, 10.6, 8.2],
    [48.0, -8.8, 9.4, 10.0, 8.8],
    [55.0, -7.0, 7.4, 8.8, 9.4],
    [61.0, -5.4, 5.0, 6.8, 9.9],
    [66.0, -4.4, 2.6, 4.2, 10.2],
    [70.0, -3.6, 0.9, 1.6, 10.4],
    [72.0, -3.2, 0.0, 0.0, 10.5],
];
/// Height of the hard chine knuckle, where the side is widest.
const CHINE: f32 = 2.2;
/// How far the forecastle stands over the main deck.
const FORE_RISE: f32 = 0.8;
/// Where the raised forecastle ends aft, stepping down to the quarterdeck.
const FORECASTLE_AFT: f32 = -27.5;

// ---- the batteries (pivots and muzzles from the unit file) -----------------

const FORE_PIVOT: Vec3 = Vec3::new(44.0, 0.0, 11.0);
const SECOND_PIVOT: Vec3 = Vec3::new(24.0, 0.0, 14.6);
const AFT_PIVOT: Vec3 = Vec3::new(-40.0, 0.0, 8.8);
/// The main batteries' Arc Howitzers: the three bores either side of a house's
/// centreline (`spread` apart), breech and muzzle ahead of the pivot, and half the
/// breech housing's height.
struct Howitzers {
    spread: f32,
    breech: f32,
    muzzle: f32,
    r: f32,
}
const GUNS: Howitzers = Howitzers {
    spread: 3.4,
    breech: 0.5,
    muzzle: 22.0,
    r: 0.95,
};
/// Bores sit this far above the pivot.
const BORE_RISE: f32 = 0.4;
/// A house's floor sits this far under its pivot, its roof this far over its floor.
const HOUSE_SINK: f32 = 1.3;
const HOUSE_HEIGHT: f32 = 2.8;
/// How far the barrels kick back on a salvo.
const RECOIL: f32 = 2.0;
const AA_PIVOT: Vec3 = Vec3::new(-14.0, 0.0, 18.0);
const AA_MUZZLE_X: f32 = -11.4;
const AA_BORES: [f32; 2] = [-0.4, 0.4];
/// The twin secondary turrets (weapons 4..8): port fore, port aft, starboard fore,
/// starboard aft. Authored facing the nose like every house; they rest trained outboard.
const SECONDARIES: [(usize, Vec3); 4] = [
    (4, Vec3::new(8.0, 10.5, 9.0)),
    (5, Vec3::new(-20.0, 10.5, 9.0)),
    (6, Vec3::new(8.0, -10.5, 9.0)),
    (7, Vec3::new(-20.0, -10.5, 9.0)),
];
/// A secondary's barrels: their spacing either side of the house, rise over the pivot,
/// and length from the pivot.
const SECONDARY_BORE: f32 = 0.6;
const SECONDARY_RISE: f32 = 0.3;
const SECONDARY_BARREL: f32 = 7.0;
/// A secondary house's floor under its pivot, and its height.
const SECONDARY_SINK: f32 = 0.9;
const SECONDARY_HEIGHT: f32 = 2.0;
/// The sponson wings the secondaries stand on: from, to, how far out, top.
const WING: [f32; 4] = [-28.0, 15.5, 14.2, 7.9];
/// The missile-defence lasers' heads (static emitters, not guns).
const LASERS: [Vec3; 4] = [
    Vec3::new(4.0, 5.5, 22.0),
    Vec3::new(4.0, -5.5, 22.0),
    Vec3::new(-30.0, 6.0, 12.0),
    Vec3::new(-30.0, -6.0, 12.0),
];

// ---- the superstructure ----------------------------------------------------

/// The citadel's armoured deckhouse, clear of the aft battery's and the secondaries' sweeps.
const CITADEL: [f32; 2] = [-29.5, 15.0];
const CITADEL_HALF: f32 = 6.4;
const CITADEL_TOP: f32 = 9.5;
/// The pagoda's decks: (front x, back x, half width, top z, rake of the face).
const TOWER: [[f32; 5]; 5] = [
    [12.5, -3.0, 5.8, 13.5, 1.0],
    [10.5, -2.4, 4.8, 17.5, 1.0],
    [9.0, -1.8, 4.0, 21.2, 1.0],
    [7.6, -1.2, 3.4, 24.8, 0.9],
    [6.6, -0.6, 2.9, 28.4, 0.8],
];
/// The bridge glass band, the dark brow over it, and the director on the roof.
const BRIDGE: [f32; 2] = [28.5, 30.1];
const BROW: f32 = 30.9;
const DIRECTOR_TOP: f32 = 32.6;
/// The mast's foot and its top, where the lamp goes.
const MAST: Vec3 = Vec3::new(2.4, 0.0, DIRECTOR_TOP);
const MAST_TOP: f32 = 39.5;
/// The mast's lower yard.
const YARD: f32 = DIRECTOR_TOP + 2.6;
/// The aft superstructure's decks, as the tower's; the last one carries the AA mount.
const AFT: [[f32; 5]; 3] = [
    [-3.0, -27.0, 5.8, 12.2, 0.6],
    [-8.6, -22.0, 4.4, 14.5, 0.6],
    [-10.0, -18.0, 3.2, 16.9, 0.4],
];
/// The raked stack block between the tower and the aft superstructure.
const STACK: [f32; 4] = [-3.0, -8.4, 3.0, 15.6];

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    hull(b);
    underwater(b);
    decks(b);
    citadel(b);
    tower(b);
    aft_superstructure(b);
    barbettes(b);
    gunhouse(b, 0, FORE_PIVOT);
    gunhouse(b, 1, SECOND_PIVOT);
    gunhouse(b, 2, AFT_PIVOT);
    aa_mount(b);
    for (weapon, pivot) in SECONDARIES {
        secondary(b, weapon, pivot);
    }
    for at in LASERS {
        laser(b, at);
    }
    lights(b);
    if b.fine() {
        deck_detail(b);
    }
}

// ---- frames ----------------------------------------------------------------

/// The frame at `x`, between the table's.
fn frame_at(x: f32) -> [f32; 5] {
    let i = FRAMES
        .windows(2)
        .position(|w| x <= w[1][0])
        .unwrap_or(FRAMES.len() - 2);
    let (a, c) = (FRAMES[i], FRAMES[i + 1]);
    let t = ((x - a[0]) / (c[0] - a[0])).clamp(0.0, 1.0);
    std::array::from_fn(|k| a[k] + (c[k] - a[k]) * t)
}

/// The deck edge (z, half beam) at `x`.
fn deck(x: f32) -> (f32, f32) {
    let f = frame_at(x);
    (f[4], f[3])
}

/// The frames as the shared helpers' stations (they only read the deck edge).
fn stations() -> Vec<Station> {
    FRAMES
        .iter()
        .map(|f| station(f[0], f[1], [0.0, 0.0], [0.0, 0.0], [f[4], f[3]]))
        .collect()
}

/// How far a point at height `z` on the frame at `x` (deck `d`) moves along x: the
/// clipper stem rakes hard forward and up, the transom aft and up.
fn rake(x: f32, z: f32, d: f32) -> f32 {
    let bow = 0.75 * ((x - 44.0) / 28.0).clamp(0.0, 1.0);
    let stern = 0.3 * ((-64.0 - x) / 6.0).clamp(0.0, 1.0);
    (bow - stern) * (z - d)
}

/// One side of a frame's section, keel to deck edge, as (half beam, z): the bottom,
/// the waterline drawn in under the chine, the hard chine, a ledge over it, the
/// deck-edge knuckle, the deck edge.
fn section(f: [f32; 5]) -> [[f32; 2]; 7] {
    let [_, k, bb, bd, d] = f;
    let ledge = (0.1 * bb).min(0.9);
    [
        [0.5 * bb, k + 0.6],
        [0.9 * bb, (k * 0.45).min(-1.6)],
        [0.93 * bb, -0.4],
        [bb, CHINE],
        [bb - ledge, CHINE + 0.3],
        [bd + 0.25 * bd.min(1.0), d - 0.8],
        [bd, d],
    ]
}

/// The hull's side half beam at height `z` between the waterline and the chine.
fn side_at(bb: f32, z: f32) -> f32 {
    let t = ((z + 0.4) / (CHINE + 0.4)).clamp(0.0, 1.0);
    bb * (0.93 + 0.07 * t)
}

/// The hull's section up to the deck-edge knuckle; below full detail it drops the
/// floor, bilge and waterline points, which are under the water.
fn hull_ring(f: [f32; 5], fine: bool) -> Vec<Vec3> {
    let side = section(f);
    let side = if fine { &side[..6] } else { &side[3..6] };
    let p = |y: f32, z: f32| v3(f[0] + rake(f[0], z, f[4]), y, z);
    let mut ring = vec![p(0.0, f[1])];
    ring.extend(side.iter().map(|&[y, z]| p(-y, z)));
    ring.extend(side.iter().rev().map(|&[y, z]| p(y, z)));
    ring
}

/// The deck-edge lip, from the knuckle in to the deck edge and across the deck.
fn lip_ring(f: [f32; 5]) -> Vec<Vec3> {
    let side = section(f);
    let p = |y: f32, z: f32| v3(f[0] + rake(f[0], z, f[4]), y, z);
    let [knuckle, edge] = [side[5], side[6]];
    vec![
        p(-knuckle[0], knuckle[1]),
        p(-edge[0], edge[1]),
        p(edge[0], edge[1]),
        p(knuckle[0], knuckle[1]),
    ]
}

fn hull(b: &mut MeshBuilder) {
    let fine = b.fine();
    let rings: Vec<Vec<Vec3>> = FRAMES.iter().map(|&f| hull_ring(f, fine)).collect();
    b.paint(PLATING).pattern(pattern::HULL);
    b.loft(&rings, true, true);
    // A dark lip round the deck edge: the frame the hull's plating hangs from.
    let rings: Vec<Vec<Vec3>> = FRAMES.iter().map(|&f| lip_ring(f)).collect();
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(&rings, true, true);

    // The armour belt: a dark strake under the chine, in three runs with plating
    // breaks between them, its ends chamfered back in.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for (x0, x1) in [(-62.0, -26.0), (-25.2, 14.0), (14.8, 47.0)] {
        belt(b, x0, x1);
    }
    // The cutwater: a dark ram-like fin out ahead of the stem at the waterline.
    b.extrude_y(
        &[
            [61.0, -2.8],
            [66.5, -0.9],
            [68.2, 0.2],
            [67.0, 1.2],
            [64.5, 1.6],
        ],
        -0.35,
        0.35,
    );
    // Twin fins either side of the raked transom.
    b.mirror_y(|b| {
        b.extrude_y(
            &[
                [-68.3, 0.3],
                [-71.0, 0.9],
                [-71.4, 3.8],
                [-69.3, 5.0],
                [-68.0, 4.0],
            ],
            5.2,
            5.7,
        );
    });
}

/// One run of the belt from `x0` to `x1`, both sides, following the side's flare.
fn belt(b: &mut MeshBuilder, x0: f32, x1: f32) {
    let mut xs = vec![x0, x0 + 1.2];
    xs.extend(
        FRAMES
            .iter()
            .map(|f| f[0])
            .filter(|&x| x > x0 + 1.5 && x < x1 - 1.5),
    );
    xs.extend([x1 - 1.2, x1]);
    b.mirror_y(|b| {
        let rings: Vec<Vec<Vec3>> = xs
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                let bb = frame_at(x)[2];
                let end = i == 0 || i == xs.len() - 1;
                let proud = if end { 0.02 } else { 0.3 };
                let (lo, hi) = if end {
                    (0.7, CHINE - 0.5)
                } else {
                    (0.3, CHINE - 0.08)
                };
                let (ylo, yhi) = (side_at(bb, lo), side_at(bb, hi));
                vec![
                    v3(x, ylo - 0.2, lo),
                    v3(x, ylo + proud, lo + 0.2),
                    v3(x, yhi + proud, hi),
                    v3(x, yhi - 0.2, hi + 0.05),
                ]
            })
            .collect();
        b.loft(&rings, true, true);
    });
}

// ---- the coarse level: a wedge, a tower block, the two forward houses -------

fn coarse(b: &mut MeshBuilder) {
    let ring = |f: [f32; 5]| {
        let z = f[4];
        vec![
            v3(f[0] + rake(f[0], f[1], z), 0.0, f[1]),
            v3(f[0], -f[3], z),
            v3(f[0], f[3], z),
        ]
    };
    b.paint(PLATING).pattern(pattern::HULL);
    b.loft(
        &[
            ring(FRAMES[0]),
            ring(FRAMES[4]),
            ring(FRAMES[9]),
            ring(FRAMES[15]),
        ],
        true,
        true,
    );
    b.paint(PLATING);
    b.frustum_open(
        v3(-4.0, 0.0, 6.3),
        v2(40.0, 13.0),
        v2(6.0, 4.0),
        26.0,
        v2(6.0, 0.0),
    );
    team_panel(b, v3(2.0, 0.0, 32.3), v2(5.0, 3.4));
    b.paint(PLATING);
    let house = |b: &mut MeshBuilder, pivot: Vec3, foot: f32| {
        let roof = pivot.z - HOUSE_SINK + HOUSE_HEIGHT;
        b.frustum_open(
            v3(pivot.x - 1.0, 0.0, foot),
            v2(14.0, 13.0),
            v2(8.0, 9.0),
            roof - foot,
            v2(-1.5, 0.0),
        );
    };
    house(b, FORE_PIVOT, 8.8);
    house(b, SECOND_PIVOT, 7.0);
    b.paint(METAL);
    for pivot in [FORE_PIVOT, SECOND_PIVOT] {
        let (x0, x1, z) = (pivot.x + 4.0, pivot.x + GUNS.muzzle, pivot.z + BORE_RISE);
        let (w0, w1) = (GUNS.spread + GUNS.r * 0.7, GUNS.spread + GUNS.r * 0.4);
        b.face(&[v3(x0, -w0, z), v3(x1, -w1, z), v3(x1, w1, z), v3(x0, w0, z)]);
    }
}

// ---- below the waterline ---------------------------------------------------

/// Twin rudders and the two shrouded propulsors under the stern, all dark.
fn underwater(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        b.extrude_y(
            &[[-63.0, -4.0], [-59.4, -4.0], [-59.0, -11.0], [-62.2, -11.4]],
            3.4,
            3.8,
        );
        let (y, z) = (4.6, -9.6);
        let sides = b.sides(10);
        let ring = |x: f32, r: f32| -> Vec<Vec3> {
            (0..sides)
                .map(|i| {
                    let a = (i as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
                    v3(x, y + a.cos() * r, z + a.sin() * r)
                })
                .collect()
        };
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        b.loft(
            &[
                ring(-51.0, 2.1),
                ring(-56.4, 2.0),
                ring(-56.4, 1.6),
                ring(-51.0, 1.7),
            ],
            false,
            false,
        );
        b.paint(METAL);
        b.cylinder_between(v3(-55.6, y, z), v3(-51.4, y, z), 0.7, 0.3, 6);
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        b.beam(
            v3(-53.6, y, z + 1.8),
            v3(-53.6, y * 0.7, z + 5.0),
            v2(0.3, 1.4),
            v2(0.3, 1.6),
        );
    });
}

// ---- the deck --------------------------------------------------------------

/// A raised layer laid along the deck from `x0` to `x1`, following the sheer: its
/// foot `inset` in from the deck edge, its top `rise` over the deck and `slope`
/// further in. A pointed layer narrows to a prow over its last `point` metres.
fn layer(b: &mut MeshBuilder, x0: f32, x1: f32, inset: f32, rise: f32, slope: f32, point: f32) {
    let mut xs = vec![x0];
    let end = if point > 0.0 { x1 - point } else { x1 };
    xs.extend(
        FRAMES
            .iter()
            .map(|f| f[0])
            .filter(|&x| x > x0 + 0.3 && x < end - 0.3),
    );
    xs.push(end);
    if point > 0.0 {
        xs.push(x1);
    }
    let rings: Vec<Vec<Vec3>> = xs
        .iter()
        .map(|&x| {
            let (z, half) = deck(x);
            let mut w = (half - inset).max(0.3);
            if point > 0.0 && x > end {
                w = 0.4;
            }
            let top = (w - slope).max(0.2);
            vec![
                v3(x, -w, z - 0.3),
                v3(x, w, z - 0.3),
                v3(x, top, z + rise),
                v3(x, -top, z + rise),
            ]
        })
        .collect();
    b.loft(&rings, true, true);
}

fn decks(b: &mut MeshBuilder) {
    let st = stations();
    if b.fine() {
        walkway(b, &st, -69.6, 71.0, 0.5);
    }
    // The raised forecastle, pointed at the prow over a dark foot, ending aft in a
    // step down to the cut-down quarterdeck.
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        layer(b, FORECASTLE_AFT - 0.4, 65.0, 1.5, 0.35, 0.2, 9.0);
    }
    b.paint(PLATING);
    layer(b, FORECASTLE_AFT, 64.0, 1.9, FORE_RISE, 0.5, 9.0);
    if b.fine() {
        b.paint(PLATING).pattern(pattern::WALKWAY);
        b.at(v3(0.0, 0.0, FORE_RISE + 0.03), |b| {
            layer(b, FORECASTLE_AFT + 0.6, 62.4, 2.7, 0.06, 0.0, 8.0)
        });
    }

    // The sponson wings along the citadel's flanks, the secondaries' footing.
    wings(b);

    // The team's bands down the forecastle and across the quarterdeck.
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.mirror_y(|b| {
        let (z0, _) = deck(53.0);
        let (z1, _) = deck(60.0);
        b.beam(
            v3(53.0, 2.4, z0 + FORE_RISE + 0.13),
            v3(60.0, 1.4, z1 + FORE_RISE + 0.13),
            v2(1.0, 0.08),
            v2(0.7, 0.08),
        );
    });
    b.plate(
        v3(-56.6, 0.0, deck(-56.6).0 + 0.05),
        v2(1.4, 9.0),
        0.07,
        0.03,
    );
    team_panel(b, v3(-52.0, 0.0, deck(-52.0).0 + 0.05), v2(4.0, 6.0));
    // The after control house at the stern, low enough for the aft battery to fire over.
    let qd = deck(-64.0).0;
    tier(b, -59.6, -67.4, 3.6, qd, qd + 1.2, 0.6);

    // Cable trunking along the forecastle to the fore barbette.
    if !b.fine() {
        return;
    }
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let z = |x: f32| deck(x).0 + FORE_RISE + 0.1;
        b.beam(
            v3(32.4, 5.2, z(32.4)),
            v3(36.0, 5.2, z(36.0)),
            v2(0.9, 0.35),
            v2(0.9, 0.35),
        );
        b.beam(
            v3(36.0, 5.2, z(36.0)),
            v3(38.6, 4.0, z(38.6)),
            v2(0.9, 0.35),
            v2(0.9, 0.35),
        );
    });
}

/// The sponson wings: angular, flared shelves standing out from the hull side along
/// the citadel, a sloped underside running back into the hull over the chine, a
/// flat top, swept ends.
fn wings(b: &mut MeshBuilder) {
    let [x0, x1, out, top] = WING;
    let xs = [x0, x0 + 3.5, x1 - 3.0, x1];
    b.paint(PLATING);
    b.mirror_y(|b| {
        let rings: Vec<Vec<Vec3>> = xs
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                let f = frame_at(x);
                let (bb, bd, d) = (f[2], f[3], f[4]);
                let o = if i == 0 || i == 3 { bd - 0.2 } else { out };
                vec![
                    v3(x, 5.8, d - 0.4),
                    v3(x, bb - 1.6, CHINE + 1.0),
                    v3(x, o - 0.9, 5.4),
                    v3(x, o, 6.5),
                    v3(x, o - 0.35, top),
                    v3(x, 5.8, top),
                ]
            })
            .collect();
        b.loft(&rings, true, true);
        if !b.fine() {
            return;
        }
        // Hatches on its top between the turrets, liferafts in their cradles.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for x in [-12.4, -10.6] {
            b.plate(v3(x, 12.2, top), v2(1.4, 1.2), 0.08, 0.03);
        }
        for x in [-6.4, -1.8] {
            liferaft(b, v3(x, 12.4, top), 2.4, 0.5);
        }
        b.paint(PLATING);
    });
}

// ---- the citadel and its towers ----------------------------------------------

/// Plan of a deck with a pointed, faceted front, about its own middle.
fn pointed_plan(half_length: f32, half_width: f32, nose: f32) -> Vec<[f32; 2]> {
    let k = (half_width - nose) * 0.8;
    let c = (half_width * 0.25).min(1.2);
    vec![
        [half_length, -nose],
        [half_length, nose],
        [half_length - k, half_width],
        [-half_length + c, half_width],
        [-half_length, half_width - c],
        [-half_length, -half_width + c],
        [-half_length + c, -half_width],
        [half_length - k, -half_width],
    ]
}

/// One deck of a stepped tower from `z0` to `z1`: a recessed dark frame band at its
/// foot, then light plating with the face raked back by `rake` and the sides drawn in.
fn tier(b: &mut MeshBuilder, front: f32, back: f32, half_width: f32, z0: f32, z1: f32, rake: f32) {
    let length = front - back;
    let plan = pointed_plan(length * 0.5, half_width, half_width * 0.3);
    let band = 0.55_f32.min((z1 - z0) * 0.3);
    let sx = (length - rake) / length;
    let sy = 1.0 - 0.35 / half_width;
    b.at(v3((front + back) * 0.5, 0.0, 0.0), |b| {
        // Below full detail the lip above carries the step alone.
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.loft_z(
                &plan,
                &[
                    Section::scaled(z0, 0.97, 0.95),
                    Section::scaled(z0 + band, 0.97, 0.95),
                ],
            );
        }
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::new(z0 + band, 1.0),
                Section::scaled(z1, sx, sy).shifted(-rake * 0.5, 0.0),
            ],
        );
        // A dark deck lip overhanging the face and flanks: the pagoda's platforms.
        let (lx, ly) = (sx + 0.6 / length, sy + 0.5 / half_width);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(
            &plan,
            &[
                Section::scaled(z1 - 0.12, lx, ly).shifted(-rake * 0.5 + 0.2, 0.0),
                Section::scaled(z1 + 0.22, lx, ly).shifted(-rake * 0.5 + 0.2, 0.0),
            ],
        );
    });
}

/// Where a tier's face and flank stand at height `z` (front x, half width).
fn tier_at(t: [f32; 5], z0: f32, z: f32) -> (f32, f32) {
    let s = ((z - z0) / (t[3] - z0)).clamp(0.0, 1.0);
    (t[0] - t[4] * s, t[2] - 0.35 * s)
}

fn citadel(b: &mut MeshBuilder) {
    // The armoured deckhouse: a dark foot, light plating with the sides sloped, a coping.
    let (front, back) = (CITADEL[1], CITADEL[0]);
    let plan = pointed_plan((front - back) * 0.5, CITADEL_HALF, 3.0);
    b.at(v3((front + back) * 0.5, 0.0, 0.0), |b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(&plan, &[Section::new(6.0, 0.99), Section::new(7.4, 0.99)]);
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::new(7.4, 1.0),
                Section::scaled(CITADEL_TOP - 0.3, 0.98, 0.93),
            ],
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(
            &plan,
            &[
                Section::scaled(CITADEL_TOP - 0.3, 0.985, 0.94),
                Section::scaled(CITADEL_TOP, 0.985, 0.94),
            ],
        );
    });
    // Engine-room louvres in its flanks, over the wings.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        b.block(
            v3(-24.0, CITADEL_HALF - 0.35, 8.2),
            v3(-8.0, CITADEL_HALF + 0.02, 8.9),
        );
        b.block(
            v3(-6.0, CITADEL_HALF - 0.35, 8.2),
            v3(8.0, CITADEL_HALF + 0.02, 8.9),
        );
    });
    if b.fine() {
        team_panel(b, v3(-28.2, 0.0, CITADEL_TOP), v2(1.6, 5.0));
    }
}

fn tower(b: &mut MeshBuilder) {
    let mut z0 = CITADEL_TOP;
    for t in TOWER {
        tier(b, t[0], t[1], t[2], z0, t[3], t[4]);
        z0 = t[3];
    }
    let n = TOWER.len();
    // The bridge: glass raked out over the deck below, a dark brow overhanging it.
    let top = TOWER[n - 1];
    let (bf, bw) = tier_at(top, TOWER[n - 2][3], top[3]);
    let length = bf - top[1];
    let plan = pointed_plan(length * 0.5, bw, bw * 0.3);
    b.at(v3((bf + top[1]) * 0.5, 0.0, 0.0), |b| {
        b.paint(GLASS);
        b.loft_z(
            &plan,
            &[
                Section::scaled(BRIDGE[0], 0.97, 0.97),
                Section::new(BRIDGE[1], 1.04).shifted(0.4, 0.0),
            ],
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(
            &plan,
            &[
                Section::new(BRIDGE[1], 1.12).shifted(0.5, 0.0),
                Section::scaled(BROW - 0.3, 1.12, 1.1).shifted(0.3, 0.0),
                Section::scaled(BROW, 1.0, 0.98),
            ],
        );
    });
    // The director on the roof: a faceted block with its rangefinder arms out either side.
    let dx = MAST.x;
    b.paint(PLATING);
    b.at(v3(dx + 0.6, 0.0, 0.0), |b| {
        b.loft_z(
            &pointed_plan(2.2, 1.9, 0.8),
            &[
                Section::new(BROW, 1.0),
                Section::scaled(DIRECTOR_TOP, 0.85, 0.9).shifted(-0.3, 0.0),
            ],
        );
    });
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(
        v3(dx + 0.4, -5.4, BROW + 0.8),
        v3(dx + 0.4, 5.4, BROW + 0.8),
        v2(0.6, 0.5),
        v2(0.6, 0.5),
    );
    b.mirror_y(|b| b.chamfered_box(v3(dx + 0.4, 5.2, BROW + 0.8), v3(1.8, 0.9, 0.9), 0.3));

    // Flying bridge wings off the third deck: the forward lasers stand on them.
    let wing = TOWER[2][3];
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.frustum(
            v3(4.0, 4.9, wing - 0.7),
            v2(3.4, 3.4),
            v2(3.8, 3.8),
            0.7,
            v2(0.0, 0.2),
        );
    });

    // The mast: a tall faceted pole with two yards, the masthead light on top (`lights`).
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2(1.1, 1.0), 0.35),
        &[
            Section::new(DIRECTOR_TOP - 0.05, 1.0).shifted(dx, 0.0),
            Section::scaled(DIRECTOR_TOP + 2.8, 0.6, 0.6).shifted(dx - 0.35, 0.0),
            Section::scaled(MAST_TOP, 0.22, 0.22).shifted(dx - 0.8, 0.0),
        ],
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(
        v3(dx - 0.4, -4.2, YARD),
        v3(dx - 0.4, 4.2, YARD),
        v2(0.3, 0.3),
        v2(0.3, 0.3),
    );
    b.beam(
        v3(dx - 0.6, -2.6, YARD + 2.4),
        v3(dx - 0.6, 2.6, YARD + 2.4),
        v2(0.2, 0.2),
        v2(0.2, 0.2),
    );

    if !b.fine() {
        return;
    }
    // Sensor panels on the top deck's cheeks.
    b.mirror_y(|b| {
        let z0 = TOWER[n - 2][3];
        let (_, w) = tier_at(top, z0, z0 + 1.2);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(v3(0.6, w - 0.05, z0 + 0.5), v3(4.0, w + 0.12, z0 + 2.0));
    });
    // Rangefinder lenses, the director's sighting ports, a radar panel on the mast.
    b.paint(GLASS);
    b.mirror_y(|b| {
        b.block(
            v3(dx + 1.3, 4.9, BROW + 0.5),
            v3(dx + 1.35, 5.5, BROW + 1.1),
        )
    });
    b.mirror_y(|b| {
        b.block(
            v3(dx + 2.5, 0.5, BROW + 0.6),
            v3(dx + 2.65, 1.2, BROW + 0.95),
        )
    });
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(
        v3(dx - 0.2, 0.0, YARD - 2.2),
        v3(dx - 0.35, 0.0, YARD - 0.8),
        v2(3.4, 0.3),
        v2(3.0, 0.3),
    );
    b.mirror_y(|b| b.spheroid(v3(dx - 0.4, 4.0, YARD + 0.4), v3(0.4, 0.4, 0.35), 6, 2));
    whip(b, v3(dx - 0.4, 3.6, YARD + 0.15), 2.6, 0.05);
    whip(b, v3(dx - 0.4, -3.6, YARD + 0.15), 2.4, 0.08);
    // Rails round the flying bridge wings.
    b.paint(METAL);
    b.mirror_y(|b| {
        b.beam(
            v3(2.2, 6.6, wing + 0.9),
            v3(5.8, 6.6, wing + 0.9),
            v2(0.06, 0.06),
            v2(0.06, 0.06),
        )
    });
    // Doors on the lower decks' flanks.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let (_, w) = tier_at(TOWER[0], CITADEL_TOP, CITADEL_TOP + 1.6);
        b.block(
            v3(4.0, w - 0.05, CITADEL_TOP + 0.5),
            v3(5.2, w + 0.1, CITADEL_TOP + 2.4),
        );
        let (_, w) = tier_at(AFT[0], CITADEL_TOP, CITADEL_TOP + 1.2);
        b.block(
            v3(-20.0, w - 0.05, CITADEL_TOP + 0.5),
            v3(-18.8, w + 0.1, CITADEL_TOP + 2.2),
        );
    });
}

fn aft_superstructure(b: &mut MeshBuilder) {
    let mut z0 = CITADEL_TOP;
    for t in AFT {
        tier(b, t[0], t[1], t[2], z0, t[3], t[4]);
        z0 = t[3];
    }
    // The stack block: low, faceted and raked aft, a dark cap over lit grilles.
    let [front, back, half, top] = STACK;
    let length = front - back;
    let plan = pointed_plan(length * 0.5, half, 1.0);
    let (rake, cap) = (2.4, 0.4);
    b.at(v3((front + back) * 0.5, 0.0, 0.0), |b| {
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::new(AFT[0][3], 1.0),
                Section::scaled(top - cap, 0.86, 0.84).shifted(-rake, 0.0),
            ],
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(
            &plan,
            &[
                Section::scaled(top - cap, 0.88, 0.86).shifted(-rake, 0.0),
                Section::scaled(top, 0.8, 0.78).shifted(-rake - 0.2, 0.0),
            ],
        );
    });
    let cx = (front + back) * 0.5 - rake - 0.2;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cuboid(v3(cx, 0.0, top + 0.05), v3(length * 0.5, half * 0.9, 0.1));

    // Pylons aft carrying the after lasers, clear of the aft battery's sweep.
    b.paint(PLATING);
    for at in &LASERS[2..] {
        b.at(v3(at.x, at.y, 0.0), |b| {
            b.loft_z(
                &chamfered_rect(v2(1.2, 1.0), 0.45),
                &[
                    Section::new(deck(at.x).0 - 0.2, 1.0),
                    Section::scaled(at.z - 0.55, 0.7, 0.75).shifted(0.4, 0.0),
                ],
            );
        });
    }
    if !b.fine() {
        return;
    }
    // Vents on the deck, a direction-finding frame and whips aft.
    b.mirror_y(|b| vent(b, v3(-24.8, 3.0, AFT[0][3]), v2(2.2, 1.0), 4, METAL));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(
        v3(-20.5, 0.0, AFT[1][3]),
        v3(-20.5, 0.0, AFT[1][3] + 3.4),
        v2(0.3, 0.3),
        v2(0.2, 0.2),
    );
    b.beam(
        v3(-20.5, -2.0, AFT[1][3] + 3.0),
        v3(-20.5, 2.0, AFT[1][3] + 3.0),
        v2(0.16, 0.16),
        v2(0.16, 0.16),
    );
    whip(b, v3(-20.5, 1.8, AFT[1][3] + 3.05), 2.4, 0.1);
    whip(b, v3(-20.5, -1.8, AFT[1][3] + 3.05), 2.0, 0.12);
    // Louvres down the stack's flanks.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        for k in 0..2 {
            let z = AFT[0][3] + 0.8 + k as f32 * 1.2;
            let s = (z - AFT[0][3]) / (top - cap - AFT[0][3]);
            let w = half * (1.0 - 0.16 * s);
            let x = (front + back) * 0.5 - rake * s;
            b.block(v3(x - 1.6, w - 0.05, z), v3(x + 1.0, w + 0.1, z + 0.45));
        }
    });
}

// ---- the ship's lights ---------------------------------------------------------

/// A ship's own lights, and nothing else lit: the red port and green starboard
/// sidelights at the bridge wings' tips, each with its screen inboard; a white
/// masthead light; a stern light on the transom; at full detail, warm floodlights
/// under the tower's first deck lighting the forecastle and quarterdeck, and rows of
/// lit scuttles in the tower's lower decks.
fn lights(b: &mut MeshBuilder) {
    let wing = TOWER[2][3];
    for (side, glow) in [(1.0, GLOW_NAV_RED), (-1.0, GLOW_NAV_GREEN)] {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(4.6, side * 6.3 - 0.05, wing),
            v3(6.0, side * 6.3 + 0.05, wing + 0.9),
        );
        b.paint(glow);
        b.cuboid(v3(5.6, side * 6.55, wing + 0.45), v3(0.45, 0.35, 0.4));
    }
    let mast = v3(MAST.x - 0.8, 0.0, MAST_TOP);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(mast, 6, 0.14, 0.1, 0.3);
    b.paint(GLOW_LAMP);
    b.cuboid(mast + Vec3::Z * 0.45, Vec3::splat(0.36));
    let z = 5.6;
    let x = -70.0 - rake(-70.0, z, FRAMES[0][4]);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cuboid(v3(x - 0.1, 0.0, z), v3(0.3, 0.7, 0.45));
    b.paint(GLOW_LAMP);
    b.cuboid(v3(x - 0.3, 0.0, z), v3(0.14, 0.4, 0.26));
    if !b.fine() {
        return;
    }
    // Floods under the first deck's lip: forward over the fore batteries, aft over the aft one.
    let (front, _) = tier_at(TOWER[0], CITADEL_TOP, TOWER[0][3] - 0.5);
    let back = AFT[0][1];
    b.paint(GLOW_LAMP);
    b.mirror_y(|b| {
        b.cuboid(v3(front + 0.05, 3.2, TOWER[0][3] - 0.5), v3(0.12, 0.6, 0.3));
        b.cuboid(v3(back - 0.05, 3.0, AFT[0][3] - 0.5), v3(0.12, 0.6, 0.3));
    });
    // Scuttles: small lit ports along the tower's two lowest decks.
    b.paint(WINDOWS);
    b.mirror_y(|b| {
        for (i, t) in TOWER.iter().enumerate().take(2) {
            let z0 = if i == 0 { CITADEL_TOP } else { TOWER[i - 1][3] };
            let z = (z0 + t[3]) * 0.5 + 0.2;
            let (_, w) = tier_at(*t, z0, z);
            for k in 0..4 {
                let x = t[1] + 1.4 + k as f32 * (t[0] - t[1] - 4.0) / 3.0;
                b.block(
                    v3(x - 0.22, w - 0.04, z - 0.22),
                    v3(x + 0.22, w + 0.06, z + 0.22),
                );
            }
        }
    });
}

// ---- missile-defence lasers --------------------------------------------------

/// A missile-defence laser: the shared head (`pd_laser`), its red band marking it out.
/// Static: not a gun.
fn laser(b: &mut MeshBuilder, at: Vec3) {
    pd_laser(b, at, 0.72, None);
}

// ---- barbettes and gunhouses -----------------------------------------------

/// A low faceted armoured base the house turns on, just above the deck and just under
/// the house: a sloped light glacis and a dark ring.
fn barbette(b: &mut MeshBuilder, pivot: Vec3, foot: f32) {
    let top = pivot.z - HOUSE_SINK - 0.05;
    let plan = chamfered_rect(v2(6.0, 6.0), 2.4);
    b.at(v3(pivot.x, 0.0, 0.0), |b| {
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[Section::new(foot, 1.16), Section::new(top - 0.2, 1.04)],
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(
            &plan,
            &[Section::new(top - 0.2, 0.99), Section::new(top, 0.99)],
        );
    });
}

fn barbettes(b: &mut MeshBuilder) {
    barbette(b, FORE_PIVOT, deck(FORE_PIVOT.x).0 + 0.5);
    barbette(b, AFT_PIVOT, deck(AFT_PIVOT.x).0 - 0.2);

    // The second battery stands on a stepped pedestal: two pointed armoured blocks,
    // each over a dark frame band, then its own low base on top.
    let p = SECOND_PIVOT;
    let low = pointed_plan(9.0, 7.6, 2.4);
    let high = pointed_plan(7.4, 6.8, 2.0);
    b.at(v3(p.x - 0.4, 0.0, 0.0), |b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(&low, &[Section::new(6.8, 0.99), Section::new(8.2, 0.99)]);
        b.paint(PLATING);
        b.loft_z(
            &low,
            &[
                Section::new(8.2, 1.0),
                Section::scaled(10.6, 0.95, 0.92).shifted(-0.3, 0.0),
            ],
        );
    });
    b.at(v3(p.x - 0.8, 0.0, 0.0), |b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(&high, &[Section::new(10.5, 0.97), Section::new(10.9, 0.97)]);
        b.paint(PLATING);
        b.loft_z(
            &high,
            &[
                Section::new(10.9, 1.0),
                Section::scaled(12.7, 0.94, 0.9).shifted(-0.3, 0.0),
            ],
        );
    });
    barbette(b, p, 12.6);
}

/// Plan of a triple gunhouse about its middle: a sharp wedge face, cheeks swept back
/// from it, a squared bustle.
fn house_plan() -> Vec<[f32; 2]> {
    vec![
        [7.6, 0.0],
        [5.4, 3.8],
        [3.4, 6.4],
        [-5.2, 6.6],
        [-7.0, 5.4],
        [-7.0, -5.4],
        [-5.2, -6.6],
        [3.4, -6.4],
        [5.4, -3.8],
    ]
}

/// A triple gunhouse: flat, wide and hard-edged. A sharp wedge face, cheeks and
/// face sloped in two facets with a crease between them, a flat roof with a dark slab
/// and the rangefinder low across its back. The house yaws about the pivot; the
/// howitzers and the mantlet pitch and recoil inside it.
fn gunhouse(b: &mut MeshBuilder, weapon: usize, pivot: Vec3) {
    b.with_house(weapon, pivot, RECOIL, |b| {
        let cx = pivot.x - 1.2;
        let base = pivot.z - HOUSE_SINK;
        let roof = base + HOUSE_HEIGHT;
        b.at(v3(cx, 0.0, 0.0), |b| {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.loft_z(
                &house_plan(),
                &[Section::new(base, 0.95), Section::new(base + 0.35, 0.95)],
            );
            b.paint(PLATING);
            b.loft_z(
                &house_plan(),
                &[
                    Section::new(base + 0.35, 1.0),
                    Section::new(base + 0.75, 1.0),
                    Section::scaled(base + 1.8, 0.9, 0.9).shifted(-0.7, 0.0),
                    Section::scaled(roof, 0.72, 0.74).shifted(-1.7, 0.0),
                ],
            );
            // A dark roof slab laid on top, drawn in from the edges.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.loft_z(
                &house_plan(),
                &[
                    Section::scaled(roof - 0.02, 0.62, 0.64).shifted(-2.0, 0.0),
                    Section::scaled(roof + 0.12, 0.6, 0.62).shifted(-2.05, 0.0),
                ],
            );
        });
        let bore_z = pivot.z + BORE_RISE;
        b.with_recoil(|b| {
            for y in [-GUNS.spread, 0.0, GUNS.spread] {
                siege_howitzer(
                    b,
                    v3(pivot.x + GUNS.breech, y, bore_z),
                    v3(pivot.x + GUNS.muzzle, y, bore_z),
                    GUNS.r,
                );
            }
            b.paint(ACCENT).pattern(pattern::PLAIN);
            let half = GUNS.spread + GUNS.r * 1.4;
            b.block(
                v3(pivot.x + 2.6, -half, bore_z - 0.6),
                v3(pivot.x + 4.0, half, bore_z + 0.6),
            );
        });
        // The rangefinder: a low hood across the back of the roof, its arms out past the cheeks.
        b.paint(PLATING);
        b.chamfered_box(v3(cx - 4.6, 0.0, roof + 0.2), v3(1.4, 7.6, 0.4), 0.4);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.beam(
            v3(cx - 4.6, -7.6, roof + 0.15),
            v3(cx - 4.6, 7.6, roof + 0.15),
            v2(0.7, 0.4),
            v2(0.7, 0.4),
        );
        team_panel(b, v3(cx - 0.8, 0.0, roof + 0.12), v2(2.6, 4.0));
        if !b.fine() {
            return;
        }
        b.paint(GLASS);
        b.mirror_y(|b| {
            b.block(
                v3(cx - 4.28, 7.2, roof - 0.02),
                v3(cx - 4.22, 7.6, roof + 0.32),
            )
        });
        // Sighting hoods at the front corners.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.mirror_y(|b| b.chamfered_box(v3(cx + 0.4, 3.5, roof + 0.28), v3(1.0, 0.7, 0.32), 0.2));
        b.paint(GLASS);
        b.mirror_y(|b| {
            b.block(
                v3(cx + 0.88, 3.3, roof + 0.16),
                v3(cx + 0.94, 3.7, roof + 0.38),
            )
        });
        // Hatches and a vent on the roof.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.plate(v3(cx - 2.2, 2.6, roof + 0.12), v2(1.0, 1.0), 0.06, 0.02);
        b.plate(v3(cx - 2.2, -2.6, roof + 0.12), v2(1.0, 1.0), 0.06, 0.02);
        vent(b, v3(cx + 1.4, 0.0, roof + 0.12), v2(0.9, 1.4), 3, METAL);
    });
}

// ---- the secondaries ---------------------------------------------------------

/// Plan of a secondary house about its middle: a pointed wedge face, swept cheeks.
fn secondary_plan() -> Vec<[f32; 2]> {
    vec![
        [3.0, 0.0],
        [2.2, 1.5],
        [1.2, 2.4],
        [-2.0, 2.5],
        [-2.7, 1.9],
        [-2.7, -1.9],
        [-2.0, -2.5],
        [1.2, -2.4],
        [2.2, -1.5],
    ]
}

/// A twin secondary turret on its wing: a low faceted base, a small hard-edged house
/// with a wedge face, two plain gunmetal GUNS. Authored facing the nose; it rests trained outboard.
fn secondary(b: &mut MeshBuilder, weapon: usize, pivot: Vec3) {
    let floor = pivot.z - SECONDARY_SINK;
    let top = WING[3];
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.at(v3(pivot.x, pivot.y, 0.0), |b| {
        b.loft_z(
            &chamfered_rect(v2(2.4, 2.4), 0.9),
            &[
                Section::new(top - 0.1, 1.04),
                Section::new(floor - 0.05, 1.0),
            ],
        );
    });
    b.with_house(weapon, pivot, 0.5, |b| {
        let roof = floor + SECONDARY_HEIGHT;
        b.at(v3(pivot.x - 0.3, pivot.y, 0.0), |b| {
            if b.fine() {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.loft_z(
                    &secondary_plan(),
                    &[Section::new(floor, 0.94), Section::new(floor + 0.25, 0.94)],
                );
            }
            b.paint(PLATING);
            b.loft_z(
                &secondary_plan(),
                &[
                    Section::new(floor + 0.25, 1.0),
                    Section::new(floor + 0.7, 1.0),
                    Section::scaled(roof, 0.72, 0.74).shifted(-0.6, 0.0),
                ],
            );
            if !b.fine() {
                return;
            }
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.loft_z(
                &secondary_plan(),
                &[
                    Section::scaled(roof - 0.02, 0.6, 0.62).shifted(-0.8, 0.0),
                    Section::scaled(roof + 0.1, 0.58, 0.6).shifted(-0.82, 0.0),
                ],
            );
        });
        let z = pivot.z + SECONDARY_RISE;
        let muzzle_x = pivot.x + SECONDARY_BARREL;
        b.with_recoil(|b| {
            for dy in [-SECONDARY_BORE, SECONDARY_BORE] {
                let y = pivot.y + dy;
                // A tapered gunmetal tube, a blast bag at the face.
                let sides = b.sides(6);
                b.paint(METAL);
                b.cylinder_between(
                    v3(pivot.x + 0.9, y, z),
                    v3(muzzle_x, y, z),
                    0.21,
                    0.15,
                    sides,
                );
                if b.fine() {
                    b.paint(ACCENT).pattern(pattern::PLAIN);
                    b.cylinder_between(
                        v3(pivot.x + 1.4, y, z),
                        v3(pivot.x + 2.4, y, z),
                        0.34,
                        0.25,
                        sides,
                    );
                    b.cylinder_between(
                        v3(muzzle_x - 0.03, y, z),
                        v3(muzzle_x + 0.01, y, z),
                        0.08,
                        0.08,
                        4,
                    );
                }
            }
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.block(
                v3(pivot.x + 0.8, pivot.y - 1.1, z - 0.45),
                v3(pivot.x + 1.6, pivot.y + 1.1, z + 0.45),
            );
        });
        if b.fine() {
            // A sight hood on the roof and a hatch behind it.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.chamfered_box(
                v3(pivot.x - 0.9, pivot.y + 0.9, roof + 0.25),
                v3(0.8, 0.5, 0.3),
                0.15,
            );
            b.paint(GLASS);
            b.block(
                v3(pivot.x - 0.52, pivot.y + 0.72, roof + 0.16),
                v3(pivot.x - 0.48, pivot.y + 1.08, roof + 0.34),
            );
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.plate(
                v3(pivot.x - 1.8, pivot.y - 0.6, roof + 0.1),
                v2(0.7, 0.7),
                0.05,
                0.02,
            );
        }
    });
}

// ---- the AA mount ----------------------------------------------------------

/// The light twin AA on the aft superstructure's top deck.
fn aa_mount(b: &mut MeshBuilder) {
    let (x, z) = (AA_PIVOT.x, AA_PIVOT.z);
    let deck_top = AFT[AFT.len() - 1][3];
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.at(v3(x, 0.0, 0.0), |b| {
        b.loft_z(
            &chamfered_rect(v2(1.8, 1.8), 0.7),
            &[
                Section::new(deck_top - 0.02, 1.0),
                Section::new(deck_top + 0.4, 0.94),
            ],
        );
    });
    b.with_house(3, AA_PIVOT, 0.14, |b| {
        b.paint(PLATING);
        b.at(v3(x - 0.3, 0.0, 0.0), |b| {
            b.loft_z(
                &pointed_plan(1.2, 0.9, 0.35),
                &[
                    Section::new(deck_top + 0.42, 1.0),
                    Section::new(z - 0.2, 1.0),
                    Section::scaled(z + 0.45, 0.75, 0.85).shifted(-0.2, 0.0),
                ],
            );
        });
        b.with_recoil(|b| {
            for y in AA_BORES {
                gun_tube(
                    b,
                    v3(x + 0.6, y, z + 0.2),
                    v3(AA_MUZZLE_X, y, z + 0.2),
                    0.07,
                );
            }
        });
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.mirror_y(|b| b.block(v3(x - 1.1, 0.88, deck_top + 0.6), v3(x + 0.1, 1.08, z)));
        }
    });
}

// ---- deck detail at full detail ----------------------------------------------

fn deck_detail(b: &mut MeshBuilder) {
    let st = stations();
    // A chevron breakwater ahead of the fore battery.
    let bw = deck(54.0).0 + FORE_RISE;
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.beam(
            v3(55.4, 0.0, bw + 0.45),
            v3(52.0, 4.4, bw + 0.45),
            v2(0.18, 0.9),
            v2(0.18, 0.9),
        )
    });
    // Guard rails round the prow.
    rails(b, &st, 58.0, 70.0, 1.0, 0.35);
    // Bollards, the anchor gear on the prow.
    for x in [62.0, -62.0] {
        let (z, half) = deck(x);
        b.mirror_y(|b| bollard(b, v3(x, half - 1.0, z + 0.05), 0.7));
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let (z, half) = deck(64.0);
        b.block(v3(63.0, half - 1.4, z), v3(65.0, half - 0.5, z + 0.6));
        b.beam(
            v3(64.0, half - 0.9, z + 0.3),
            v3(66.4, half * 0.5, z + 0.3),
            v2(0.2, 0.2),
            v2(0.2, 0.2),
        );
    });
    b.paint(METAL);
    let wz = deck(60.5).0 + FORE_RISE + 0.45;
    b.cylinder_between(v3(60.5, -1.4, wz), v3(60.5, 1.4, wz), 0.45, 0.45, 8);
    // The after control house's vent.
    let qz = deck(-64.0).0 + 1.2;
    vent(b, v3(-65.0, 0.0, qz + 0.22), v2(1.6, 2.0), 3, METAL);
    // Hawse pipes in the flare of the bow.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let f = frame_at(62.0);
        b.cylinder_between(
            v3(62.0, f[3] - 0.3, f[4] - 1.4),
            v3(62.0, f[3] + 0.35, f[4] - 1.7),
            0.45,
            0.45,
            6,
        );
    });
    // Hatches along the forecastle and the quarterdeck.
    for x in [30.0, 36.5, -44.0, -56.0] {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        let top = if x > FORECASTLE_AFT {
            FORE_RISE + 0.09
        } else {
            0.05
        };
        b.mirror_y(|b| b.plate(v3(x, 4.4, deck(x).0 + top), v2(1.4, 1.1), 0.08, 0.03));
    }
    // A fire-control dome on the quarterdeck for the aft battery.
    let dz = deck(-49.0).0 + 0.05;
    b.paint(PLATING);
    b.spheroid(v3(-49.5, 0.0, dz + 0.4), v3(1.0, 1.0, 0.8), 8, 2);
    b.paint(GLASS);
    b.cuboid(v3(-49.5, 0.0, dz + 1.22), v3(0.6, 0.6, 0.08));
    // The transom: a dark panel carrying the propulsor feeds.
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    let transom = |z: f32| -70.0 - rake(-70.0, z, FRAMES[0][4]);
    b.beam(
        v3(transom(2.6) - 0.1, 0.0, 2.6),
        v3(transom(4.8) - 0.1, 0.0, 4.8),
        v2(8.6, 0.3),
        v2(8.6, 0.3),
    );
}
