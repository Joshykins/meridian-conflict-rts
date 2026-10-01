//! The Dominion's hull, broken into masses along its length rather than one even run:
//! a broad aft block over the engines, a pinched waist amidships with its wall drawn in
//! and a lit hangar recess let into it, and a forward block a little narrower than the
//! aft one, each stepping hard into the next. It rides tall on a keel blade down its
//! centre line, a deep ventral hull hangs under its middle with raked ends, and the
//! prow ([`super::prow`]) runs on from the forward block. The upper hull's underside
//! rises from the blade out to a hard chine, its walls stand tall above it. The
//! casemates stand out of the flanks on stepped sponsons; an engineering block under the
//! stern carries the drives. The stacked layers on its deck are [`super::stacked`]'s.

use super::*;

/// The upper hull: its underside at the blade, its chine and wall, its deck.
pub(super) const HULL_BOTTOM: f32 = 62.0;
pub(super) const CHINE_Z: f32 = 72.0;
pub(super) const WALL_TOP: f32 = 88.0;
pub(super) const DECK: f32 = 96.0;
/// How far the wall leans in from its foot at the chine to its head.
pub(super) const WALL_LEAN: f32 = 1.0;
/// The keel blade: its half width and its ends (it runs into the engineering block astern
/// and rakes up forward under the prow from `BLADE_RAKE`).
const BLADE: f32 = 22.0;
const BLADE_AFT: f32 = -150.0;
const BLADE_RAKE: f32 = 170.0;
const BLADE_FORE: f32 = 240.0;
/// Where the blade's armour is broken by a dark seam, aft to fore.
const SEAMS: [f32; 5] = [-100.0, -30.0, 36.0, 96.0, 140.0];
/// Where the hull runs into the prow.
pub(super) const HULL_FORE: f32 = 196.0;

/// The aft block's plan (x, half width at the chine): broad over the engines, its stern
/// cut square, its wall straight forward of x -96 to the waist.
const AFT: [[f32; 2]; 5] = [
    [STERN, 76.0],
    [-218.0, 76.0],
    [-120.0, 74.0],
    [-96.0, 72.0],
    [WAIST[0], 72.0],
];
/// The waist: its run in x and its half width at the chine.
pub(super) const WAIST: [f32; 2] = [-30.0, 36.0];
const WAIST_W: f32 = 62.0;
/// The forward block's half width at the chine, all along it.
pub(super) const FORE_W: f32 = 68.0;

/// The hull's half width at its chine at `x`: stepping in at the waist and out again.
pub(super) fn width(x: f32) -> f32 {
    if x < WAIST[0] {
        lerp_rows(&AFT, x)[1]
    } else if x <= WAIST[1] {
        WAIST_W
    } else {
        FORE_W
    }
}

/// The deck's edge (half width) at `x`, inboard of the wall's shoulder.
pub(super) fn deck_edge(x: f32) -> f32 {
    width(x) - 10.0
}

/// The edge the stacked layers are laid out from (half width) at `x`: smooth down the
/// ship, drawing in forward, whatever the walls below do.
pub(super) fn layer_edge(x: f32) -> f32 {
    lerp_rows(
        &[
            [STERN, 66.0],
            [-218.0, 66.0],
            [-120.0, 64.0],
            [-70.0, 60.0],
            [90.0, 56.0],
            [170.0, 50.0],
            [222.0, 46.0],
        ],
        x,
    )[1]
}

/// The wall at `x` for painting on: its |y| at the chine, the chine's height, its lean
/// per metre up.
pub(super) fn wall(x: f32) -> [f32; 3] {
    [width(x), CHINE_Z, WALL_LEAN / (WALL_TOP - CHINE_Z)]
}

/// The upper hull's port half at `x`: 0 underside middle, 1 underside at the blade,
/// 2 underside out, 3 chine, 4 wall head, 5 shoulder, 6 deck edge, 7 the deck's middle.
pub(super) fn hull_half(x: f32) -> Vec<[f32; 2]> {
    section(width(x))
}

/// The upper hull's port half with its chine `w` out.
pub(super) fn section(w: f32) -> Vec<[f32; 2]> {
    vec![
        [0.0, HULL_BOTTOM],
        [BLADE - 2.0, HULL_BOTTOM],
        [w - 12.0, HULL_BOTTOM + 6.0],
        [w, CHINE_Z],
        [w - WALL_LEAN, WALL_TOP],
        [w - 6.0, DECK - 2.0],
        [w - 10.0, DECK],
        [0.0, DECK],
    ]
}

/// The blade's port half at `x`: its keel rakes up under the prow.
fn blade_half(x: f32) -> Vec<[f32; 2]> {
    let k = if x > BLADE_RAKE {
        KEEL + (x - BLADE_RAKE) / (BLADE_FORE - BLADE_RAKE) * 28.0
    } else {
        KEEL
    };
    vec![
        [0.0, k],
        [BLADE - 4.0, k],
        [BLADE, k + 5.0],
        [BLADE, (HULL_BOTTOM - 6.0).max(k + 6.0)],
        [BLADE - 2.0, HULL_BOTTOM + 1.0],
        [0.0, HULL_BOTTOM + 1.0],
    ]
}

/// The ventral hull hung under the middle: x, half width, belly. Its ends are raked, the
/// forward one back from its foot.
const VENTRAL: [[f32; 3]; 4] = [
    [-132.0, 18.0, 58.0],
    [-112.0, 32.0, 40.0],
    [118.0, 32.0, 40.0],
    [150.0, 16.0, 58.0],
];

fn ventral_half(x: f32) -> Vec<[f32; 2]> {
    let [_, w, bot] = lerp_rows(&VENTRAL, x);
    vec![
        [0.0, bot],
        [w - 7.0, bot],
        [w, bot + 7.0],
        [w + 3.0, HULL_BOTTOM - 4.0],
        [w + 1.0, HULL_BOTTOM + 1.0],
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

/// Everything under the stacked layers: the hull and its flanks, the blade, the prow,
/// the engineering block and the drives, the casemates on their sponsons, the rifles, the
/// belly and the lamps, the lettering.
pub(super) fn build(b: &mut MeshBuilder, lamps_of: &crate::CapitalLamps) {
    hull(b);
    super::flanks::build(b);
    blade(b);
    super::prow::build(b);
    engine(b);
    b.mirror_y(|b| {
        for (k, face) in CASEMATE_FACES.into_iter().enumerate() {
            sponson(b, CASEMATES[2 * k], face);
        }
    });
    for (i, p) in CASEMATES.into_iter().enumerate() {
        casemate(b, i, Vec3::from(p), CASEMATE_FACES[i / 2]);
    }
    rifles(b);
    belly(b);
    lamps(b, lamps_of);
}

/// The strategy-zoom hull: the long hull and the prow.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.extrude_z(
        &[
            [HULL_FORE, FORE_W],
            [HULL_FORE, -FORE_W],
            [-120.0, -74.0],
            [STERN, -76.0],
            [STERN, 76.0],
            [-120.0, 74.0],
        ],
        KEEL,
        DECK,
    );
    super::prow::coarse(b);
}

/// The upper hull in its three blocks, each lofted and capped on its own so the steps
/// between them stand as faces.
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
    let ring = |x: f32, w: f32| full_ring(x, &section(w));
    let mut aft = vec![raked(STERN)];
    aft.extend([-214.0, -120.0, -96.0].map(|x| ring(x, width(x))));
    aft.push(ring(WAIST[0], AFT[4][1]));
    b.paint(PLATING_DARK).pattern(pattern::GENERIC);
    b.loft(&aft, true, true);
    b.loft(
        &[ring(WAIST[0] - 1.0, WAIST_W), ring(WAIST[1] + 1.0, WAIST_W)],
        true,
        true,
    );
    b.loft(
        &[ring(WAIST[1], FORE_W), ring(HULL_FORE, FORE_W)],
        true,
        true,
    );
    ventral(b);
}

/// The ventral hull under the middle, armoured in courses, a dark strake at its foot.
fn ventral(b: &mut MeshBuilder) {
    let xs: Vec<f32> = VENTRAL.iter().map(|r| r[0]).collect();
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    hull_loft(b, &xs, &ventral_half);
    let half = |x: f32| ventral_half(x);
    b.mirror_y(|b| {
        for (k, &(x0, x1)) in [(-104.0, -48.0), (-40.0, 30.0), (40.0, 110.0)]
            .iter()
            .enumerate()
        {
            b.paint(if k % 2 == 0 { PLATING } else { PLATING_DARK })
                .pattern(pattern::GENERIC);
            plate_on(b, &half, [x0, x1], 2, 4, [0.2, 0.7], 1.4);
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        plate_on(b, &half, [-110.0, 116.0], 1, 2, [0.1, 0.9], 0.6);
    });
}

/// The keel blade: one armoured run down the centre line under the hull, broken into
/// segments by dark seams, armour slabs on each segment's flanks.
fn blade(b: &mut MeshBuilder) {
    let half = |x: f32| blade_half(x);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    hull_loft(b, &[BLADE_AFT, BLADE_RAKE, BLADE_FORE], &half);
    let mut edges = vec![BLADE_AFT];
    edges.extend(SEAMS);
    edges.push(BLADE_RAKE);
    for seg in edges.windows(2) {
        let (x0, x1) = (seg[0], seg[1]);
        b.mirror_y(|b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            plate_on(b, &half, [x0 + 2.5, x1 - 2.5], 2, 4, [0.1, 0.6], 1.4);
        });
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for x in SEAMS {
        b.mirror_y(|b| plate_on(b, &half, [x - 1.2, x + 1.2], 2, 4, [0.0, 1.0], 0.5));
    }
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

/// The forward block's and the stern's strobes, which the ship carries besides its mast's.
pub(super) const BOW_STROBES: [[f32; 3]; 4] = [
    [190.0, FORE_W - 5.4, DECK - 1.4],
    [190.0, -(FORE_W - 5.4), DECK - 1.4],
    [-212.0, 64.0, DECK + 0.8],
    [-212.0, -64.0, DECK + 0.8],
];
