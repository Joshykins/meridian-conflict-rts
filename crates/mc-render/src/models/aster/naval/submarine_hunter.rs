//! Moray: the tech 2 hunter-killer submarine (`aster_t2_submarine`, 28 m).
//!
//! Not a tube. The pressure hull is a chined flat-iron: a narrow top ridge, a
//! sloped glacis, a hard chine at its widest, a tumblehome under it and a keel
//! flat, lofted through ten-point rings that grow from a raked, flat chisel bow
//! to the arrowhead's base two thirds aft and then draw in to the tail. Six
//! torpedo doors sit in the bow face as dark hexagonal shutters in two banks.
//! Two stub dive wings come out of the chine amidships, each with a white motor
//! pod and a ducted thruster at its tip; the main shrouded pumpjet is on the
//! tail, three engines in all. A low swept sail sits well back with a glass
//! slit and lit sensor panels; the deck gun (weapon 1) turns on a house of its
//! own forward of it. Tech 2: blue sensor panels, a red running lamp.
//!
//! x forward, y left, z up; origin at the waterline. Muzzles as in the unit file.
use super::*;

/// A station of the pressure hull: its x, then the right-hand half of its section
/// from the top ridge down to the keel as (x offset, y, z): top, glacis, chine,
/// tumblehome, keel. Mirrored about y = 0 into a ten-point ring.
type Station = (f32, [[f32; 3]; 5]);

/// The hull's axis at the tail, where the pumpjet turns.
const AXIS_Z: f32 = -1.1;
/// The bow is one raked plane: x = BOW_X + BOW_RAKE * (z - BOW_KEEL).
const BOW_X: f32 = 13.3;
const BOW_RAKE: f32 = 0.1;
const BOW_KEEL: f32 = -3.15;

/// Stern first. The tail collapses to a point on the axis; the bow ring lies on the
/// raked plane so its cap is flat, with the tube doors in it.
const STATIONS: [Station; 10] = [
    (-13.4, [[0.0, 0.0, AXIS_Z]; 5]),
    (-12.5, [[0.0, 0.32, -0.45], [0.0, 0.58, -0.62], [0.0, 0.78, -1.05], [0.0, 0.55, -1.55], [0.0, 0.22, -1.8]]),
    (-11.0, [[0.0, 0.55, 0.1], [0.0, 1.0, -0.2], [0.0, 1.35, -0.95], [0.0, 0.9, -2.05], [0.0, 0.32, -2.5]]),
    (-8.5, [[0.0, 0.9, 0.7], [0.0, 1.7, 0.35], [0.0, 2.15, -0.7], [0.0, 1.35, -2.55], [0.0, 0.45, -3.1]]),
    (-5.5, [[0.0, 1.05, 1.05], [0.0, 2.1, 0.65], [0.0, 2.6, -0.6], [0.0, 1.6, -2.7], [0.0, 0.5, -3.4]]),
    (-1.5, [[0.0, 1.05, 1.15], [0.0, 2.05, 0.75], [0.0, 2.5, -0.6], [0.0, 1.55, -2.7], [0.0, 0.5, -3.4]]),
    (3.5, [[0.0, 0.95, 1.2], [0.0, 1.8, 0.8], [0.0, 2.2, -0.6], [0.0, 1.4, -2.7], [0.0, 0.5, -3.35]]),
    (8.5, [[0.0, 0.85, 1.05], [0.0, 1.5, 0.7], [0.0, 1.85, -0.65], [0.0, 1.35, -2.75], [0.0, 0.6, -3.25]]),
    (11.8, [[0.0, 0.8, 0.7], [0.0, 1.3, 0.4], [0.0, 1.6, -0.8], [0.0, 1.4, -2.85], [0.0, 0.75, -3.2]]),
    (
        BOW_X,
        [[0.345, 0.7, 0.3], [0.31, 1.2, -0.05], [0.215, 1.42, -1.0], [0.035, 1.38, -2.8], [0.0, 0.9, -3.15]],
    ),
];
/// Torpedo tube mouths, as in the unit file: two banks of three.
const TUBES: [[f32; 3]; 6] =
    [[13.4, -0.9, -1.0], [13.4, 0.9, -1.0], [13.4, -0.9, -1.9], [13.4, 0.9, -1.9], [13.4, -0.9, -2.8], [13.4, 0.9, -2.8]];
/// The deck gun's yaw axis and trunnion, and its muzzle (`pivot`, `muzzle` in the unit file).
const GUN: Vec3 = Vec3::new(5.5, 0.0, 2.2);
const GUN_MUZZLE: Vec3 = Vec3::new(8.6, 0.0, 2.3);
/// The sail's plan origin.
const SAIL_X: f32 = -3.0;

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

/// x of the bow plane at height `z`.
fn bow_x(z: f32) -> f32 {
    BOW_X + BOW_RAKE * (z - BOW_KEEL)
}

/// A point on the bow plane at (y, z), `proud` metres out along its normal.
fn on_bow(y: f32, z: f32, proud: f32) -> Vec3 {
    let n = v3(1.0, 0.0, -BOW_RAKE).normalize();
    v3(bow_x(z), y, z) + n * proud
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

/// A ring round an x-axis line at (y, z): ducts and pods.
fn axial(x: f32, y: f32, z: f32, r: f32, sides: usize) -> Vec<Vec3> {
    ngon(sides, r).into_iter().map(|[u, w]| v3(x, y + u, z + w)).collect()
}

/// A shrouded thruster: an open duct from `x_front` back to `x_back` round (y, z),
/// its outer skin drawn in toward the back, and the hub inside it at full detail.
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
        // Stator vanes from the hub to the shroud, and the hub's cone out of the back.
        let mid = (x_front + x_back) * 0.5;
        for k in 0..3 {
            let a = k as f32 * std::f32::consts::TAU / 3.0 + std::f32::consts::FRAC_PI_2;
            b.beam(
                v3(mid, y, z),
                v3(mid, y + a.cos() * r * 0.8, z + a.sin() * r * 0.8),
                v2(0.05, r * 0.4),
                v2(0.05, r * 0.4),
            );
        }
        b.paint(METAL);
        b.cylinder_between(v3(mid, y, z), v3(x_back - r * 0.25, y, z), r * 0.3, r * 0.1, 6);
    }
}

pub(super) fn build(b: &mut MeshBuilder) {
    // The pressure hull: black anechoic tiles over the whole chined loft.
    b.paint(PLATING_DARK).pattern(pattern::TILES);
    let rings: Vec<Vec<Vec3>> = if b.coarse() {
        [0usize, 4, 7, 9].iter().map(|&i| ring(&STATIONS[i], true)).collect()
    } else if b.fine() {
        STATIONS.iter().map(|s| ring(s, false)).collect()
    } else {
        [0usize, 2, 4, 6, 8, 9].iter().map(|&i| ring(&STATIONS[i], false)).collect()
    };
    b.loft(&rings, true, true);

    // The sail: low, long and swept back, white over the black hull; a glass slit
    // near the top; the team's colour on its roof.
    let sail = [[3.2, 0.0], [2.2, 0.55], [-1.6, 0.6], [-2.6, 0.3], [-2.6, -0.3], [-1.6, -0.6], [2.2, -0.55]];
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(v3(SAIL_X, 0.0, 1.1), v2(5.8, 1.2), v2(3.8, 0.8), 3.1, v2(-1.4, 0.0));
        team_panel(b, v3(SAIL_X - 1.4, 0.0, 4.2), v2(2.4, 0.55));
        // The wings and the tube doors as flat plates; the gun as a bar to its muzzle.
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            b.face(&[v3(2.0, 2.2, -0.5), v3(-1.2, 5.4, -0.5), v3(-3.8, 5.4, -0.5), v3(-3.8, 2.2, -0.5)]);
        });
        b.paint(ACCENT);
        bow_quad(b, -1.15, 1.15, -3.05, -0.75);
        b.with_house(1, GUN, 0.3, |b| {
            b.with_recoil(|b| {
                b.paint(METAL);
                b.face(&[v3(5.7, -0.1, 2.3), v3(GUN_MUZZLE.x, -0.08, 2.3), v3(GUN_MUZZLE.x, 0.08, 2.3), v3(5.7, 0.1, 2.3)]);
            });
        });
        return;
    }
    b.at(v3(SAIL_X, 0.0, 0.0), |b| {
        b.loft_z(
            &sail,
            &[
                Section::scaled(1.1, 1.12, 1.15),
                Section::new(1.55, 1.0),
                Section::scaled(3.3, 0.9, 0.92).shifted(-0.85, 0.0),
            ],
        );
        b.paint(GLASS);
        b.loft_z(
            &sail,
            &[
                Section::scaled(3.3, 0.88, 0.9).shifted(-0.85, 0.0),
                Section::scaled(3.7, 0.82, 0.84).shifted(-1.1, 0.0),
            ],
        );
        b.paint(PLATING);
        b.loft_z(
            &sail,
            &[
                Section::scaled(3.7, 0.84, 0.86).shifted(-1.1, 0.0),
                Section::scaled(4.2, 0.72, 0.74).shifted(-1.5, 0.0),
            ],
        );
    });
    team_panel(b, v3(SAIL_X - 1.4, 0.0, 4.2), v2(2.4, 0.55));
    // Lit sensor panels let into the sail's flanks, and the running lamp aft on its roof.
    b.paint(GLOW);
    b.mirror_y(|b| b.cuboid(v3(SAIL_X - 0.3, 0.585, 2.45), v3(1.6, 0.05, 0.2)));
    b.paint(GLOW_RED);
    b.cuboid(v3(SAIL_X - 3.1, 0.0, 4.26), v3(0.16, 0.16, 0.12));

    // The dive wings: a thin diamond section swept back from the chine, a white motor
    // pod on each tip with its ducted thruster behind.
    let wing = |y: f32, le: f32, te: f32, t: f32| {
        let mid = le * 0.6 + te * 0.4;
        vec![v3(le, y, -0.6), v3(mid, y, -0.6 + t), v3(te, y, -0.6), v3(mid, y, -0.6 - t)]
    };
    b.mirror_y(|b| {
        b.paint(PLATING_DARK);
        b.loft(&[wing(2.3, 2.0, -3.8, 0.32), wing(5.4, -1.2, -3.8, 0.12)], true, true);
        let sides = b.sides(8);
        b.paint(PLATING);
        b.loft(
            &[axial(-0.4, 5.55, -0.6, 0.0, sides), axial(-1.1, 5.55, -0.6, 0.3, sides), axial(-2.2, 5.55, -0.6, 0.42, sides)],
            true,
            true,
        );
        duct(b, -2.2, -4.1, 5.55, -0.6, 0.52);
        if b.fine() {
            // A white leading edge on the wing.
            b.paint(PLATING);
            b.beam(v3(2.0, 2.35, -0.6), v3(-1.2, 5.3, -0.6), v2(0.24, 0.1), v2(0.18, 0.08));
        }
    });

    // The main pumpjet on the tail, and the cruciform stern planes with white tips.
    duct(b, -11.6, -13.7, 0.0, AXIS_Z, 1.3);
    b.paint(PLATING_DARK);
    b.beam(v3(-10.0, 0.0, 0.5), v3(-10.9, 0.0, 2.3), v2(0.14, 1.4), v2(0.1, 0.7));
    b.beam(v3(-10.0, 0.0, -2.6), v3(-10.7, 0.0, -3.9), v2(0.14, 1.4), v2(0.1, 0.7));
    b.mirror_y(|b| b.beam(v3(-10.0, 1.2, -0.9), v3(-10.6, 2.9, -0.9), v2(1.4, 0.14), v2(0.8, 0.1)));

    // The deck gun forward of the sail: a dark pedestal on the casing (hull), then the
    // house that turns with weapon 1 and the tube that pitches and kicks inside it.
    b.paint(ACCENT);
    b.prism(v3(GUN.x, 0.0, 1.1), b.sides(8), 0.62, 0.5, 0.6);
    b.with_house(1, GUN, 0.3, |b| {
        b.paint(METAL);
        b.prism(v3(GUN.x, 0.0, 1.66), b.sides(8), 0.56, 0.56, 0.08);
        b.paint(PLATING);
        b.at(v3(GUN.x - 0.15, 0.0, 0.0), |b| {
            b.loft_z(
                &turret_plan(1.7, 1.25),
                &[
                    Section::new(1.74, 0.9),
                    Section::new(2.0, 1.0),
                    Section::scaled(2.62, 0.7, 0.72).shifted(-0.12, 0.0),
                ],
            );
        });
        b.with_recoil(|b| {
            cannon(b, v3(5.95, 0.0, GUN_MUZZLE.z), GUN_MUZZLE, 0.085, Emitter::Unlit);
            b.paint(ACCENT);
            b.block(v3(5.85, -0.22, 2.08), v3(6.25, 0.22, 2.5));
        });
        if b.fine() {
            b.paint(ACCENT);
            b.block(v3(GUN.x - 0.3, -0.12, 2.62), v3(GUN.x + 0.1, 0.12, 2.8));
            b.paint(GLASS);
            b.block(v3(GUN.x + 0.1, -0.1, 2.66), v3(GUN.x + 0.13, 0.1, 2.76));
        }
    });

    // The tube doors: below full detail one dark plate per bank.
    b.paint(ACCENT);
    if !b.fine() {
        bow_quad(b, 0.45, 1.35, -3.05, -0.75);
        bow_quad(b, -1.35, -0.45, -3.05, -0.75);
        return;
    }
    for m in TUBES {
        b.paint(METAL);
        bow_hex(b, m[1], m[2], 0.4, 0.02);
        b.paint(ACCENT);
        bow_hex(b, m[1], m[2], 0.33, 0.04);
    }

    // Flank sonar arrays: long dark strakes just under the chine, both sides.
    b.paint(ACCENT);
    b.mirror_y(|b| {
        for w in STATIONS[3..9].windows(2) {
            let a = v3(w[0].0, w[0].1[2][1] + 0.03, w[0].1[2][2] - 0.35);
            let c = v3(w[1].0, w[1].1[2][1] + 0.03, w[1].1[2][2] - 0.35);
            b.beam(a, c, v2(0.06, 0.45), v2(0.06, 0.45));
        }
    });
    // The team's band on the foredeck casing, escape trunks fore and aft of the sail,
    // and countermeasure launcher mouths in the after casing.
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(9.6, 0.0, 0.97), v2(2.0, 1.3), 0.05, 0.02);
    b.paint(ACCENT);
    b.prism(v3(1.8, 0.0, 1.19), 8, 0.36, 0.34, 0.07);
    b.prism(v3(-8.0, 0.0, 0.78), 8, 0.34, 0.32, 0.07);
    for y in [-0.55, 0.55] {
        let pts: Vec<Vec3> = ngon(6, 0.22).into_iter().map(|[u, w]| v3(-7.0 + u, y + w, 0.92)).collect();
        b.face(&pts);
    }
    // Cleats along the casing edge, and the towed array's fairing down the after casing.
    b.mirror_y(|b| {
        for (x, z) in [(-9.5, 0.55), (-6.5, 0.97), (0.5, 1.12), (7.0, 1.05)] {
            b.block(v3(x - 0.18, 0.62, z), v3(x + 0.18, 0.74, z + 0.09));
        }
    });
    b.paint(METAL);
    let run: Vec<Vec3> = STATIONS[2..6].iter().map(|s| v3(s.0, -s.1[0][1] * 0.8, s.1[0][2] + 0.06)).collect();
    for w in run.windows(2) {
        b.cylinder_between(w[0], w[1], 0.09, 0.09, 6);
    }
    // Masts in the sail's roof: periscope, ESM mast with its head, the snorkel.
    b.paint(METAL);
    b.cylinder_between(v3(SAIL_X - 0.5, 0.0, 4.2), v3(SAIL_X - 0.5, 0.0, 4.85), 0.08, 0.07, 6);
    b.cylinder_between(v3(SAIL_X - 1.3, 0.12, 4.2), v3(SAIL_X - 1.3, 0.12, 4.6), 0.07, 0.07, 6);
    b.paint(ACCENT);
    b.spheroid(v3(SAIL_X - 1.3, 0.12, 4.68), v3(0.14, 0.14, 0.1), 6, 3);
    b.chamfered_box(v3(SAIL_X - 2.2, 0.0, 4.36), v3(0.4, 0.22, 0.32), 0.08);
    // White tips on the stern planes.
    b.paint(PLATING);
    b.beam(v3(-10.85, 0.0, 2.25), v3(-10.95, 0.0, 2.45), v2(0.1, 0.72), v2(0.1, 0.72));
    b.mirror_y(|b| b.beam(v3(-10.55, 2.85, -0.9), v3(-10.6, 3.05, -0.9), v2(0.82, 0.1), v2(0.82, 0.1)));
}
