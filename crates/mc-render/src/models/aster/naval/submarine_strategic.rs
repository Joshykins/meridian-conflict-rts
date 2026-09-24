//! Kraken: the tech 3 strategic submarine (`aster_t3_submarine`, 60 m).
//!
//! A weapon platform, not a tube. The pressure hull is a wide, flat diamond in
//! section, almost a flying wing seen from ahead: a narrow top ridge, a long
//! glacis out to a hard chine six metres off the centreline, a tumblehome under
//! it to a keel flat six metres down. Lofted through ten-point rings from a raked
//! flat chisel bow, with eight torpedo doors in it as dark hexagonal shutters,
//! back to a broad flat transom that the two big shrouded pumpjets come out of.
//! Behind the tall knife-edged sail a raised missile deck carries four square
//! hatches in a 2x2 block, dark-rimmed with a blue seam round each door. Two
//! long wings sweep back from the chine aft with a white sensor pod on each tip
//! and a ducted thruster on each: four engines. Cruciform stern planes, flank
//! arrays, and for tech 3 the reactor's plasma louvres with flux conduits
//! running forward to the missile deck.
//!
//! x forward, y left, z up; origin at the waterline. Muzzles as in the unit file.
use super::*;

/// A station of the pressure hull: its x, then the right-hand half of its section
/// from the top ridge down to the keel as (x offset, y, z): top, glacis, chine,
/// tumblehome, keel. Mirrored about y = 0 into a ten-point ring.
type Station = (f32, [[f32; 3]; 5]);

/// The bow is one raked plane: x = BOW_X + BOW_RAKE * (z - BOW_KEEL).
const BOW_X: f32 = 28.05;
const BOW_RAKE: f32 = 0.1;
const BOW_KEEL: f32 = -3.6;
/// The stern pumpjets' axes, either side of the transom.
const JET_Y: f32 = 2.3;
const JET_Z: f32 = -1.0;

/// Stern first. The transom ring is flat (a plane at x = -27) so its cap is a face;
/// the bow ring lies on the raked plane with the tube doors in it.
const STATIONS: [Station; 10] = [
    (-27.0, [[0.0, 1.8, 0.0], [0.0, 3.0, -0.2], [0.0, 3.8, -1.0], [0.0, 3.0, -1.9], [0.0, 1.8, -2.1]]),
    (-24.0, [[0.0, 2.3, 0.55], [0.0, 4.2, 0.1], [0.0, 5.2, -1.1], [0.0, 3.6, -3.0], [0.0, 1.6, -3.8]]),
    (-19.0, [[0.0, 2.0, 1.0], [0.0, 4.8, 0.5], [0.0, 6.0, -1.2], [0.0, 4.0, -4.2], [0.0, 1.4, -5.4]]),
    (-13.0, [[0.0, 1.8, 1.1], [0.0, 4.9, 0.6], [0.0, 6.4, -1.2], [0.0, 4.2, -4.6], [0.0, 1.4, -6.0]]),
    (-6.0, [[0.0, 1.8, 1.1], [0.0, 4.8, 0.6], [0.0, 6.3, -1.2], [0.0, 4.1, -4.6], [0.0, 1.4, -6.0]]),
    (1.0, [[0.0, 1.9, 1.2], [0.0, 4.6, 0.7], [0.0, 6.0, -1.2], [0.0, 3.9, -4.5], [0.0, 1.4, -5.9]]),
    (9.0, [[0.0, 1.9, 1.25], [0.0, 4.2, 0.75], [0.0, 5.4, -1.2], [0.0, 3.6, -4.3], [0.0, 1.4, -5.6]]),
    (17.0, [[0.0, 1.8, 1.1], [0.0, 3.5, 0.6], [0.0, 4.4, -1.3], [0.0, 3.2, -4.0], [0.0, 1.4, -5.0]]),
    (23.5, [[0.0, 1.7, 0.8], [0.0, 2.9, 0.4], [0.0, 3.5, -1.3], [0.0, 2.9, -3.6], [0.0, 1.4, -4.3]]),
    (BOW_X, [[0.4, 1.5, 0.4], [0.37, 2.6, 0.1], [0.23, 3.0, -1.3], [0.06, 2.75, -3.0], [0.0, 1.6, -3.6]]),
];
/// Torpedo tube mouths, as in the unit file: two rows of four.
const TUBES: [[f32; 3]; 8] = [
    [28.0, -1.0, -1.5],
    [28.0, 1.0, -1.5],
    [28.0, -2.2, -1.5],
    [28.0, 2.2, -1.5],
    [28.0, -1.0, -2.6],
    [28.0, 1.0, -2.6],
    [28.0, -2.2, -2.6],
    [28.0, 2.2, -2.6],
];
/// The missile hatches' centres (the muzzles in the unit file); the doors' tops are at z 1.2.
const HATCHES: [[f32; 2]; 4] = [[-2.0, -2.0], [-2.0, 2.0], [-6.0, -2.0], [-6.0, 2.0]];
const HATCH_TOP: f32 = 1.2;
/// The sail's plan origin.
const SAIL_X: f32 = 7.0;

/// One hull ring: the full ten-point section, or the six-point top/chine/keel one.
fn ring(s: &Station, simple: bool) -> Vec<Vec3> {
    let (x, p) = s;
    let at = |k: usize, sign: f32| v3(x + p[k][0], sign * p[k][1], p[k][2]);
    if simple {
        vec![at(0, 1.0), at(2, 1.0), at(4, 1.0), at(4, -1.0), at(2, -1.0), at(0, -1.0)]
    } else {
        vec![
            at(0, 1.0),
            at(1, 1.0),
            at(2, 1.0),
            at(3, 1.0),
            at(4, 1.0),
            at(4, -1.0),
            at(3, -1.0),
            at(2, -1.0),
            at(1, -1.0),
            at(0, -1.0),
        ]
    }
}

/// A point on the bow plane at (y, z), `proud` metres out along its normal.
fn on_bow(y: f32, z: f32, proud: f32) -> Vec3 {
    let n = v3(1.0, 0.0, -BOW_RAKE).normalize();
    v3(BOW_X + BOW_RAKE * (z - BOW_KEEL), y, z) + n * proud
}

/// A quad on the bow plane over the tubes from y0..y1, z0..z1, facing forward.
fn bow_quad(b: &mut MeshBuilder, y0: f32, y1: f32, z0: f32, z1: f32) {
    b.face(&[on_bow(y0, z0, 0.02), on_bow(y1, z0, 0.02), on_bow(y1, z1, 0.02), on_bow(y0, z1, 0.02)]);
}

/// A hexagonal door on the bow plane round (y, z).
fn bow_hex(b: &mut MeshBuilder, y: f32, z: f32, radius: f32, proud: f32) {
    let w = v3(BOW_RAKE, 0.0, 1.0).normalize();
    let c = on_bow(y, z, proud);
    let pts: Vec<Vec3> = (0..6)
        .map(|i| {
            let a = (i as f32 + 0.5) * std::f32::consts::TAU / 6.0;
            c + Vec3::Y * (a.cos() * radius) + w * (a.sin() * radius)
        })
        .collect();
    b.face(&pts);
}

/// A ring round an x-axis line at (y, z): ducts, pods and tail cones.
fn axial(x: f32, y: f32, z: f32, r: f32, sides: usize) -> Vec<Vec3> {
    ngon(sides, r).into_iter().map(|[u, w]| v3(x, y + u, z + w)).collect()
}

/// A shrouded thruster: an open duct from `x_front` back to `x_back` round (y, z),
/// its outer skin drawn in toward the back, with stator vanes and a hub at full detail.
fn duct(b: &mut MeshBuilder, x_front: f32, x_back: f32, y: f32, z: f32, r: f32) {
    let sides = b.sides(8);
    let d = |x: f32, k: f32| axial(x, y, z, r * k, sides);
    b.paint(PLATING_DARK);
    b.loft(
        &[d(x_front, 1.0), d(x_back, 0.86), d(x_back, 0.72), d(x_front, 0.86), d(x_front, 1.0)],
        false,
        false,
    );
    if b.fine() {
        let mid = (x_front + x_back) * 0.5;
        for k in 0..3 {
            let a = k as f32 * std::f32::consts::TAU / 3.0 + std::f32::consts::FRAC_PI_2;
            b.beam(
                v3(mid, y, z),
                v3(mid, y + a.cos() * r * 0.8, z + a.sin() * r * 0.8),
                v2(0.06, r * 0.35),
                v2(0.06, r * 0.35),
            );
        }
        b.paint(METAL);
        b.cylinder_between(v3(mid, y, z), v3(x_back - r * 0.25, y, z), r * 0.28, r * 0.1, 6);
    }
}

pub(super) fn build(b: &mut MeshBuilder) {
    // The pressure hull: black anechoic tiles over the whole diamond loft.
    b.paint(PLATING_DARK).pattern(pattern::TILES);
    if b.coarse() {
        let tail = (-27.5, [[0.0, 0.0, JET_Z]; 5]);
        let rings = vec![ring(&tail, true), ring(&STATIONS[3], true), ring(&STATIONS[6], true), ring(&STATIONS[9], true)];
        b.loft(&rings, true, true);
    } else {
        let picks: &[usize] = if b.fine() { &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9] } else { &[0, 1, 3, 5, 7, 8, 9] };
        let rings: Vec<Vec<Vec3>> = picks.iter().map(|&i| ring(&STATIONS[i], false)).collect();
        b.loft(&rings, true, true);
    }

    // The sail: a tall, thin knife raked back, white over the black hull, a glass band
    // near the top, the team's colour on its roof.
    let sail = [[5.0, 0.0], [3.2, 0.85], [-2.8, 0.9], [-4.6, 0.42], [-4.6, -0.42], [-2.8, -0.9], [3.2, -0.85]];
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(v3(SAIL_X, 0.0, 1.15), v2(9.6, 1.8), v2(6.7, 1.3), 3.85, v2(-1.1, 0.0));
        team_panel(b, v3(SAIL_X - 1.1, 0.0, 5.0), v2(4.2, 0.85));
        // The wings, the missile deck and the tube doors as flat plates.
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            b.face(&[v3(-7.5, 5.8, -1.1), v3(-12.5, 10.5, -1.1), v3(-17.5, 10.5, -1.1), v3(-17.5, 5.8, -1.1)]);
        });
        b.decal(v3(-4.0, 0.0, HATCH_TOP), v2(8.4, 7.6));
        b.paint(ACCENT);
        bow_quad(b, -2.7, 2.7, -3.05, -1.05);
        return;
    }
    b.at(v3(SAIL_X, 0.0, 0.0), |b| {
        b.loft_z(
            &sail,
            &[
                Section::scaled(1.15, 1.1, 1.15),
                Section::new(1.7, 1.0),
                Section::scaled(3.4, 0.9, 0.92).shifted(-0.5, 0.0),
            ],
        );
        b.paint(GLASS);
        b.loft_z(
            &sail,
            &[
                Section::scaled(3.4, 0.88, 0.9).shifted(-0.5, 0.0),
                Section::scaled(3.9, 0.84, 0.86).shifted(-0.7, 0.0),
            ],
        );
        b.paint(PLATING);
        b.loft_z(
            &sail,
            &[
                Section::scaled(3.9, 0.86, 0.88).shifted(-0.7, 0.0),
                Section::scaled(5.0, 0.7, 0.72).shifted(-1.2, 0.0),
            ],
        );
    });
    team_panel(b, v3(SAIL_X - 1.1, 0.0, 5.0), v2(4.2, 0.85));
    // Lit sensor bands along the sail's flanks, a glow slit down its leading edge.
    b.paint(GLOW);
    b.mirror_y(|b| b.cuboid(v3(SAIL_X - 0.5, 0.86, 2.6), v3(4.0, 0.05, 0.22)));
    b.cuboid(v3(SAIL_X + 4.75, 0.0, 2.4), v3(0.08, 0.12, 1.4));

    // The missile deck: a low faceted block raised on the casing behind the sail, its
    // four hatches each a dark rim, a blue seam and a white door whose top is the muzzle.
    b.paint(PLATING_DARK);
    b.at(v3(-4.0, 0.0, 0.0), |b| {
        b.loft_z(&chamfered_rect(v2(5.8, 4.0), 0.9), &[Section::scaled(0.55, 1.14, 1.16), Section::new(1.06, 1.0)]);
    });
    for [hx, hy] in HATCHES {
        b.paint(ACCENT);
        b.plate(v3(hx, hy, 1.06), v2(3.5, 3.5), 0.06, 0.02);
        b.paint(GLOW);
        b.plate(v3(hx, hy, 1.12), v2(3.25, 3.25), 0.03, 0.01);
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.plate(v3(hx, hy, 1.15), v2(2.95, 2.95), HATCH_TOP - 1.15, 0.02);
    }

    // The wings: a thin diamond section swept back from the chine aft, a white sensor
    // pod on each tip, and a thruster nacelle with its duct on each.
    let wing = |y: f32, le: f32, te: f32, t: f32| {
        let mid = le * 0.6 + te * 0.4;
        vec![v3(le, y, -1.2), v3(mid, y, -1.2 + t), v3(te, y, -1.2), v3(mid, y, -1.2 - t)]
    };
    b.mirror_y(|b| {
        b.paint(PLATING_DARK);
        b.loft(&[wing(5.8, -7.5, -17.5, 0.55), wing(10.5, -12.5, -17.5, 0.2)], true, true);
        let sides = b.sides(8);
        b.paint(PLATING);
        b.spheroid(v3(-14.8, 10.7, -1.2), v3(2.4, 0.6, 0.6), sides, if b.fine() { 3 } else { 2 });
        b.paint(PLATING_DARK);
        b.loft(
            &[axial(-13.5, 8.4, -1.2, 0.0, sides), axial(-14.8, 8.4, -1.2, 0.5, sides), axial(-16.2, 8.4, -1.2, 0.72, sides)],
            true,
            true,
        );
        duct(b, -16.2, -18.3, 8.4, -1.2, 0.85);
    });

    // The stern: a tail cone out of each side of the transom in its shrouded pumpjet.
    b.mirror_y(|b| {
        let sides = b.sides(8);
        b.paint(PLATING_DARK);
        b.loft(
            &[axial(-26.8, JET_Y, JET_Z, 1.0, sides), axial(-28.2, JET_Y, JET_Z, 0.7, sides), axial(-29.3, JET_Y, JET_Z, 0.25, sides)],
            true,
            true,
        );
        duct(b, -26.5, -29.3, JET_Y, JET_Z, 1.45);
    });
    // Cruciform stern planes, white-tipped.
    b.paint(PLATING_DARK);
    b.beam(v3(-23.0, 0.0, 1.0), v3(-25.2, 0.0, 4.2), v2(0.2, 2.6), v2(0.12, 1.2));
    b.beam(v3(-23.0, 0.0, -3.5), v3(-24.6, 0.0, -6.0), v2(0.2, 2.2), v2(0.12, 1.0));
    b.mirror_y(|b| b.beam(v3(-23.0, 4.8, -1.1), v3(-24.5, 8.5, -1.1), v2(2.6, 0.22), v2(1.3, 0.14)));

    // The reactor: plasma louvres in a low housing on the after casing, and the flux
    // conduits from it forward to the missile deck.
    b.paint(PLATING_DARK).pattern(pattern::PLASMA);
    b.plate(v3(-19.0, 0.0, 0.98), v2(3.4, 3.2), 0.3, 0.06);
    b.paint(PLATING_DARK).pattern(pattern::FLUX);
    b.mirror_y(|b| b.beam(v3(-17.3, 1.3, 1.1), v3(-9.9, 1.3, 1.12), v2(0.7, 0.32), v2(0.7, 0.32)));
    // ...and on up the middle of the deck between the two columns of hatches.
    b.beam(v3(-9.9, 0.0, 1.06), v3(-0.2, 0.0, 1.06), v2(0.4, 0.14), v2(0.4, 0.14));

    // Team colour on the foredeck casing too.
    team_panel(b, v3(16.0, 0.0, 1.13), v2(2.8, 2.4));

    // The tube doors: below full detail one dark plate per bank.
    b.paint(ACCENT);
    if !b.fine() {
        bow_quad(b, 0.4, 2.8, -3.1, -1.0);
        bow_quad(b, -2.8, -0.4, -3.1, -1.0);
        return;
    }
    for m in TUBES {
        b.paint(METAL);
        bow_hex(b, m[1], m[2], 0.5, 0.02);
        b.paint(ACCENT);
        bow_hex(b, m[1], m[2], 0.42, 0.04);
    }

    // Each hatch is two leaves: the split line down its middle.
    b.paint(ACCENT);
    for [hx, hy] in HATCHES {
        b.face(&[
            v3(hx - 1.4, hy - 0.04, HATCH_TOP + 0.01),
            v3(hx + 1.4, hy - 0.04, HATCH_TOP + 0.01),
            v3(hx + 1.4, hy + 0.04, HATCH_TOP + 0.01),
            v3(hx - 1.4, hy + 0.04, HATCH_TOP + 0.01),
        ]);
    }
    // Flank sonar arrays: long dark strakes just under the chine, both sides.
    b.mirror_y(|b| {
        for w in STATIONS[2..9].windows(2) {
            let a = v3(w[0].0, w[0].1[2][1] + 0.04, w[0].1[2][2] - 0.5);
            let c = v3(w[1].0, w[1].1[2][1] + 0.04, w[1].1[2][2] - 0.5);
            b.beam(a, c, v2(0.08, 0.7), v2(0.08, 0.7));
        }
    });
    // Escape trunks fore and aft, the team's band forward of the sail, decoy mouths
    // in the after casing, cleats along the casing edge.
    b.paint(ACCENT);
    b.prism(v3(13.0, 0.0, 1.2), 8, 0.45, 0.42, 0.08);
    b.prism(v3(-12.0, 0.0, 1.12), 8, 0.45, 0.42, 0.08);
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(21.0, 0.0, 0.92), v2(2.6, 2.4), 0.06, 0.02);
    b.paint(ACCENT);
    for y in [-0.9, 0.9] {
        let pts: Vec<Vec3> = ngon(6, 0.3).into_iter().map(|[u, w]| v3(-15.0 + u, y + w, 1.13)).collect();
        b.face(&pts);
    }
    b.mirror_y(|b| {
        for x in [-10.0, 3.0, 14.0, 22.0] {
            b.block(v3(x - 0.25, 1.4, 1.02), v3(x + 0.25, 1.55, 1.12));
        }
    });
    // Masts in the sail's roof: two periscopes, a radar mast with its head, the snorkel.
    b.paint(METAL);
    b.cylinder_between(v3(SAIL_X - 0.2, 0.0, 5.0), v3(SAIL_X - 0.2, 0.0, 5.9), 0.1, 0.08, 6);
    b.cylinder_between(v3(SAIL_X - 1.0, 0.2, 5.0), v3(SAIL_X - 1.0, 0.2, 5.7), 0.08, 0.07, 6);
    b.cylinder_between(v3(SAIL_X - 2.0, -0.15, 5.0), v3(SAIL_X - 2.0, -0.15, 5.6), 0.08, 0.08, 6);
    b.paint(ACCENT);
    b.spheroid(v3(SAIL_X - 2.0, -0.15, 5.72), v3(0.2, 0.2, 0.14), 6, 3);
    b.chamfered_box(v3(SAIL_X - 3.0, 0.0, 5.18), v3(0.5, 0.28, 0.36), 0.1);
    // White tips on the stern planes; a glow strip along the missile deck's edges.
    b.paint(PLATING);
    b.beam(v3(-25.1, 0.0, 4.15), v3(-25.25, 0.0, 4.4), v2(0.12, 1.25), v2(0.12, 1.25));
    b.mirror_y(|b| b.beam(v3(-24.4, 8.4, -1.1), v3(-24.5, 8.7, -1.1), v2(1.35, 0.14), v2(1.35, 0.14)));
    b.mirror_y(|b| glow_strip(b, v3(-4.0, 3.75, 1.06), v2(9.0, 0.12), GLOW));
}
