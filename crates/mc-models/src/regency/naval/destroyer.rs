//! The Claymore, the Regency's tech 2 heavy destroyer (`regency_t2_destroyer`): the
//! Marlin's place, but it dives. A warship drawn to go under: low, long and streamlined,
//! its superstructure swept back into a fin, its guns sitting low in houses of their own,
//! dark plate lapped over bronze workings (docs/STYLE.md "The Regency look", "The navy").
//!
//! The origin is the waterline; the keel is drawn below it and the sea closes over the
//! whole hull when it dives. The weapon points are the unit file's:
//!
//! - House 0, the Pinched-plasmeric Cannon forward ([`GUN`]): a fork whose projectors
//!   reach past the bore and hold the charge between them ([`CHARGE`], the `muzzle`).
//! - 1, four torpedo doors in the bow's blunt face under the water ([`TUBES`]).
//! - House 2, the Plasmeric AA Repeater aft ([`FLAK`]): two short tubes held up at the sky.
//! - 3, two interceptor doors in the transom under the water ([`INTERCEPT`]).
//!
//! The hull shield's projector is set with `set_shield_emitter`, the search radar turns on
//! the fin (`part::SPINNER`).

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::library::ModelDef;
use crate::material::*;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, collar, hoop, hoop_on, mouth_rim, red_slot, swept, Frame};

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_destroyer", RADIUS, HEIGHT, build)];

const RADIUS: f32 = 27.0;
const HEIGHT: f32 = 11.0;

/// The Pinched-plasmeric Cannon's house: its yaw axis and trunnion (`pivot`), and the
/// middle of the charge its projectors hold ahead of the bore (`muzzle`).
const GUN: Vec3 = Vec3::new(14.0, 0.0, 4.6);
const CHARGE: Vec3 = Vec3::new(22.0, 0.0, 4.6);
/// Where the bore ends, short of the charge.
const BORE_MOUTH: f32 = 19.4;
/// How far round the charge the projectors stand.
#[cfg(test)]
const HOLD: f32 = 2.2;
/// The flak house's trunnion, and the middle of its row of tube mouths: the tubes are
/// held raised at the sky.
const FLAK: Vec3 = Vec3::new(-13.0, 0.0, 5.0);
const FLAK_MUZZLE: Vec3 = Vec3::new(-10.9, 0.0, 5.0);
/// The torpedo doors in the bow's face, as in the unit file.
const TUBES: [[f32; 3]; 4] = [
    [25.3, -0.7, -1.55],
    [25.3, 0.7, -1.55],
    [25.3, -0.7, -2.35],
    [25.3, 0.7, -2.35],
];
/// The interceptor doors in the transom.
const INTERCEPT: [[f32; 3]; 2] = [[-25.15, -1.4, -0.5], [-25.15, 1.4, -0.5]];
/// Where the bow's blunt face (the tube doors' plate) and the transom stand.
const BOW_FACE: f32 = 25.2;
const TRANSOM: f32 = -25.0;

// ---- the hull --------------------------------------------------------------------------

/// One station of a hull, stern first: its keel's depth, the bilge's half-beam and
/// height (the widest, under the water), the deck edge's (tumbled home over it), and the
/// deck's crown on the centreline.
#[derive(Clone, Copy, Debug)]
struct Station {
    x: f32,
    keel: f32,
    bilge: [f32; 2],
    side: [f32; 2],
    crown: f32,
}

const fn st(x: f32, keel: f32, bilge: [f32; 2], side: [f32; 2], crown: f32) -> Station {
    Station {
        x,
        keel,
        bilge,
        side,
        crown,
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The station's ring: keel, up the port side to the crown and down the starboard.
fn ring(b: &MeshBuilder, s: &Station) -> Vec<Vec3> {
    let [by, bz] = s.bilge;
    let [sy, sz] = s.side;
    let k = s.keel;
    let half: Vec<[f32; 2]> = if b.coarse() {
        vec![[by, bz], [sy, sz]]
    } else if b.fine() {
        vec![
            [by * 0.34, lerp(k, bz, 0.3)],
            [by * 0.62, lerp(k, bz, 0.68)],
            [by * 0.88, lerp(k, bz, 0.92)],
            [by, bz],
            [lerp(by, sy, 0.3), lerp(bz, sz, 0.35)],
            [lerp(by, sy, 0.6), lerp(bz, sz, 0.68)],
            [sy, sz],
            [sy * 0.75, lerp(sz, s.crown, 0.55)],
            [sy * 0.4, lerp(sz, s.crown, 0.9)],
        ]
    } else {
        vec![[by * 0.62, lerp(k, bz, 0.72)], [by, bz], [sy, sz]]
    };
    let mut r = vec![v3(s.x, 0.0, k)];
    r.extend(half.iter().map(|&[y, z]| v3(s.x, -y, z)));
    if !b.coarse() {
        r.push(v3(s.x, 0.0, s.crown));
    }
    r.extend(half.iter().rev().map(|&[y, z]| v3(s.x, y, z)));
    r
}

/// The hull lofted through `stations`, or through `coarse` of them far off; capped at the
/// transom and at the bow's blunt face.
fn hull(b: &mut MeshBuilder, stations: &[Station], coarse: &[usize]) {
    let rings: Vec<Vec<Vec3>> = if b.coarse() {
        coarse.iter().map(|&i| ring(b, &stations[i])).collect()
    } else if b.fine() {
        // Close up, a station between each pair as well: a smoother run.
        let mut all = vec![ring(b, &stations[0])];
        for w in stations.windows(2) {
            all.push(ring(b, &station_at(stations, (w[0].x + w[1].x) * 0.5)));
            all.push(ring(b, &w[1]));
        }
        all
    } else {
        stations.iter().map(|s| ring(b, s)).collect()
    };
    dark_plate(b);
    b.loft(&rings, true, true);
}

/// The station at `x`, between the two either side of it.
fn station_at(stations: &[Station], x: f32) -> Station {
    let i = stations
        .windows(2)
        .position(|w| x >= w[0].x && x <= w[1].x)
        .unwrap_or(if x < stations[0].x {
            0
        } else {
            stations.len() - 2
        });
    let (a, c) = (stations[i], stations[i + 1]);
    let t = ((x - a.x) / (c.x - a.x)).clamp(0.0, 1.0);
    let l2 = |p: [f32; 2], q: [f32; 2]| [lerp(p[0], q[0], t), lerp(p[1], q[1], t)];
    st(
        x,
        lerp(a.keel, c.keel, t),
        l2(a.bilge, c.bilge),
        l2(a.side, c.side),
        lerp(a.crown, c.crown, t),
    )
}

/// The deck's crown at `x`.
fn deck(stations: &[Station], x: f32) -> f32 {
    station_at(stations, x).crown
}

/// The flank's half-beam at `x` and height `z` (between bilge and deck edge), and its
/// outward normal there (port side, +y).
fn flank(stations: &[Station], x: f32, z: f32) -> (f32, Vec3) {
    let s = station_at(stations, x);
    let [by, bz] = s.bilge;
    let [sy, sz] = s.side;
    let t = ((z - bz) / (sz - bz)).clamp(0.0, 1.0);
    let n = v3(0.0, sz - bz, -(sy - by)).normalize();
    (lerp(by, sy, t), n)
}

/// The torpedo doors in the bow's face: a dark ring sunk round each, a bronze door.
fn tube_doors(b: &mut MeshBuilder) {
    let sides = b.sides(8);
    for [x, y, z] in TUBES {
        seam(b);
        b.cylinder_between(
            v3(BOW_FACE - 0.2, y, z),
            v3(x - 0.06, y, z),
            0.4,
            0.4,
            sides,
        );
        metal(b);
        b.cylinder_between(v3(x - 0.08, y, z), v3(x, y, z), 0.3, 0.28, sides);
    }
}

/// The interceptor doors in the transom.
fn interceptor_doors(b: &mut MeshBuilder) {
    let sides = b.sides(8);
    for [x, y, z] in INTERCEPT {
        seam(b);
        b.cylinder_between(
            v3(TRANSOM + 0.2, y, z),
            v3(x + 0.06, y, z),
            0.36,
            0.36,
            sides,
        );
        metal(b);
        b.cylinder_between(v3(x + 0.08, y, z), v3(x, y, z), 0.27, 0.25, sides);
    }
}

/// The owner's colour as a flat patch on an upward face at height `z`.
fn team_patch(b: &mut MeshBuilder, x0: f32, x1: f32, half: f32, z: f32) {
    b.paint(TEAM);
    b.face(&[
        v3(x0, -half, z),
        v3(x1, -half, z),
        v3(x1, half, z),
        v3(x0, half, z),
    ]);
}

/// The hull shield's projector: a dark plated drum from `base` up to `at`, a bronze race
/// round it and the field's lens on top.
fn shield_projector(b: &mut MeshBuilder, at: Vec3, base: f32) {
    b.set_shield_emitter(at);
    if b.coarse() {
        return;
    }
    let sides = b.sides(10);
    dark_plate(b);
    b.prism(v3(at.x, at.y, base), sides, 0.8, 0.7, at.z - 0.3 - base);
    if b.fine() {
        metal(b);
        hoop(b, v3(at.x, at.y, at.z - 0.2), 0.62, 0.22, 0.2, sides);
    }
    b.paint(GLOW_SHIELD);
    b.prism(v3(at.x, at.y, at.z - 0.3), sides, 0.28, 0.16, 0.18);
}

/// The search radar on its bearing at `at`: a bronze hub and a swept dark array, a red
/// line let into its face, always turning.
fn radar(b: &mut MeshBuilder, at: Vec3) {
    b.set_spinner_pivot(at);
    if b.coarse() {
        return;
    }
    b.with_part(part::SPINNER, |b| {
        let sides = b.sides(8);
        metal(b);
        b.prism(at, sides, 0.4, 0.32, 0.4);
        dark_plate(b);
        b.mirror_y(|b| {
            let f = Frame::new(
                at + v3(0.35, 0.0, 0.45),
                v3(-0.45, 1.0, 0.0),
                v3(0.25, 0.0, 1.0),
            );
            armour(b, &f, &swept(2.0, 0.42, 0.6, 0.55), 0.12);
        });
        if b.fine() {
            red_slot(b, at + v3(0.4, 0.0, 0.6), Vec3::X, Vec3::Y, 1.4, 0.08);
        }
    });
}

// ---- the guns --------------------------------------------------------------------------

/// A section of `shape` across y and z about `c`, `w` wide and `h` high.
fn section(c: Vec3, w: f32, h: f32, shape: &[[f32; 2]]) -> Vec<Vec3> {
    shape
        .iter()
        .map(|[s, u]| c + v3(0.0, s * w * 0.5, u * h * 0.5))
        .collect()
}

/// A box with its corners cut.
const CHAMFERED: [[f32; 2]; 8] = [
    [1.0, -0.6],
    [0.6, -1.0],
    [-0.6, -1.0],
    [-1.0, -0.6],
    [-1.0, 0.6],
    [-0.6, 1.0],
    [0.6, 1.0],
    [1.0, 0.6],
];

/// A body lofted along x through `stations` (x, width, height, centre height).
fn body_x(b: &mut MeshBuilder, stations: &[[f32; 4]], shape: &[[f32; 2]]) {
    let rings: Vec<Vec<Vec3>> = stations
        .iter()
        .map(|&[x, w, h, z]| section(v3(x, 0.0, z), w, h, shape))
        .collect();
    b.loft(&rings, true, true);
}

/// A plan outline: `half` of it (y >= 0) from the front round to the back, mirrored.
fn mirrored(half: &[[f32; 2]]) -> Vec<[f32; 2]> {
    half.iter()
        .copied()
        .chain(
            half.iter()
                .rev()
                .filter(|p| p[1] > 0.0)
                .map(|&[x, y]| [x, -y]),
        )
        .collect()
}

/// A projector's emitter: a bronze boss at `at` with a red lens turned to `toward`.
fn emitter(b: &mut MeshBuilder, at: Vec3, toward: Vec3, r: f32) {
    let d = (toward - at).normalize();
    let sides = b.sides(8);
    metal(b);
    b.cylinder_between(at - d * r * 0.6, at, r, r * 0.85, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(at, at + d * r * 0.25, r * 0.7, r * 0.5, sides);
}

/// The forward house: a fixed barbette from the deck at `deck`, then, turning on its
/// bronze race, a low armoured wedge with cheek plates swept back into spikes, and the
/// fork: a square bore and two flat projectors reaching past its mouth either side of the
/// charge, red emitters on their inner faces. The fork pitches; the wedge only turns.
fn pinch_gun(b: &mut MeshBuilder, deck: f32) {
    let base = GUN.z - 0.95;
    if !b.coarse() {
        let sides = b.sides(12);
        dark_plate(b);
        b.prism(
            v3(GUN.x, 0.0, deck - 0.3),
            sides,
            2.7,
            2.45,
            base - deck + 0.3,
        );
        if b.fine() {
            metal(b);
            hoop(b, v3(GUN.x, 0.0, base - 0.05), 2.2, 0.5, 0.14, sides);
        }
    }
    b.with_house(0, GUN, 0.5, |b| {
        if b.coarse() {
            dark_plate(b);
            b.beam(
                v3(GUN.x - 3.0, 0.0, GUN.z),
                v3(BORE_MOUTH, 0.0, GUN.z),
                Vec2::new(3.6, 1.6),
                Vec2::new(1.4, 1.0),
            );
            return;
        }
        let fine = b.fine();
        let wedge = mirrored(&[
            [3.2, 0.9],
            [1.6, 2.3],
            [-2.2, 2.4],
            [-3.4, 1.4],
            [-3.8, 0.0],
        ]);
        dark_plate(b);
        b.at(v3(GUN.x, 0.0, 0.0), |b| {
            b.loft_z(
                &wedge,
                &[
                    Section::new(base, 1.0),
                    Section::new(base + 0.85, 1.0),
                    Section::scaled(GUN.z + 0.75, 0.82, 0.7).shifted(-0.4, 0.0),
                ],
            );
        });
        team_patch(b, GUN.x - 2.6, GUN.x - 1.2, 0.8, GUN.z + 0.76);
        b.mirror_y(|b| {
            let f = Frame::new(
                v3(GUN.x + 2.4, 1.95, GUN.z - 0.35),
                v3(-1.0, 0.12, 0.0),
                v3(0.0, 1.0, 0.35),
            );
            dark_plate(b);
            armour(b, &f, &swept(7.2, 0.62, 0.4, 0.5), 0.22);
        });
        if fine {
            red_slot(
                b,
                v3(GUN.x + 2.0, 0.0, GUN.z + 0.5),
                v3(1.0, 0.0, 0.8),
                Vec3::Y,
                1.1,
                0.07,
            );
        }
        b.with_recoil(fork);
    });
}

/// The fork in the house's frame, level: trunnion, bore, projectors.
fn fork(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x, z) = (GUN.x, GUN.z);
    collar(b, GUN, Vec3::Y, 0.75, 3.0);
    dark_plate(b);
    body_x(
        b,
        &[
            [x + 0.6, 1.7, 1.5, z],
            [x + 2.4, 1.5, 1.3, z],
            [BORE_MOUTH - 0.3, 1.1, 0.95, z],
        ],
        &CHAMFERED,
    );
    if fine {
        for bx in [x + 3.2, x + 4.1] {
            collar(b, v3(bx, 0.0, z), Vec3::X, 0.78, 0.3);
        }
        red_slot(b, v3(x + 3.7, 0.0, z + 0.5), Vec3::Z, Vec3::X, 2.6, 0.08);
    }
    metal(b);
    let sides = b.sides(10);
    b.cylinder_between(
        v3(BORE_MOUTH - 0.4, 0.0, z),
        v3(BORE_MOUTH, 0.0, z),
        0.5,
        0.46,
        sides,
    );
    b.paint(GLOW_LASER);
    hoop_on(
        b,
        v3(BORE_MOUTH - 0.03, 0.0, z),
        Vec3::X,
        0.38,
        0.12,
        0.08,
        sides,
    );
    b.mirror_y(|b| {
        dark_plate(b);
        b.beam(
            v3(x + 1.2, 1.3, z),
            v3(CHARGE.x + 0.4, 1.5, z),
            Vec2::new(0.6, 1.5),
            Vec2::new(0.45, 1.0),
        );
        let f = Frame::new(
            v3(CHARGE.x + 0.3, 1.8, z),
            v3(-1.0, 0.04, 0.0),
            v3(0.0, 1.0, 0.0),
        );
        armour(b, &f, &swept(6.4, 0.68, 0.0, 0.6), 0.2);
        emitter(
            b,
            v3(CHARGE.x - 0.2, 1.28, z),
            v3(CHARGE.x - 0.2, 0.0, z),
            0.34,
        );
        if b.fine() {
            red_slot(b, v3(CHARGE.x - 1.8, 1.26, z), -Vec3::Y, Vec3::X, 2.0, 0.12);
        }
    });
    if fine {
        // Bronze ties under the bore, holding the projectors apart.
        metal(b);
        for tx in [x + 3.0, CHARGE.x - 2.6] {
            b.beam(
                v3(tx, -1.3, z - 0.55),
                v3(tx, 1.3, z - 0.55),
                Vec2::new(0.3, 0.3),
                Vec2::new(0.3, 0.3),
            );
        }
    }
}

/// The flak house aft: a barbette from the deck at `deck`, a hex step on a bronze race
/// with a plated cheek either side swept back into spikes, and between them two short
/// flak tubes in a clamped breech, held up at the sky, red rims at their mouths.
fn flak(b: &mut MeshBuilder, deck: f32) {
    let base = FLAK.z - 0.75;
    if !b.coarse() {
        let sides = b.sides(10);
        dark_plate(b);
        b.prism(
            v3(FLAK.x, 0.0, deck - 0.3),
            sides,
            1.9,
            1.7,
            base - deck + 0.3,
        );
        if b.fine() {
            metal(b);
            hoop(b, v3(FLAK.x, 0.0, base - 0.04), 1.45, 0.4, 0.12, sides);
        }
    }
    let d = FLAK_MUZZLE - FLAK;
    let len = d.length();
    b.with_house(2, FLAK, 0.25, |b| {
        if b.coarse() {
            return;
        }
        dark_plate(b);
        b.prism(v3(FLAK.x, 0.0, base), 6, 1.5, 1.3, 0.45);
        b.mirror_y(|b| {
            dark_plate(b);
            b.block(
                v3(FLAK.x - 0.9, 0.95, base + 0.4),
                v3(FLAK.x + 0.7, 1.3, FLAK.z + 0.5),
            );
            let f = Frame::new(
                v3(FLAK.x + 0.7, 1.3, FLAK.z + 0.2),
                v3(-1.0, 0.0, 0.35),
                Vec3::Y,
            );
            armour(b, &f, &swept(2.8, 0.55, 0.3, 0.45), 0.2);
        });
        b.with_recoil(|b| {
            b.at(FLAK, |b| organ(b, len));
        });
    });
}

/// The flak organ in its own frame (the trunnion at the origin, +x up the bore).
fn organ(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    collar(b, Vec3::ZERO, Vec3::Y, 0.4, 1.9);
    dark_plate(b);
    body_x(
        b,
        &[[-0.7, 1.7, 0.9, 0.0], [0.4, 1.8, 0.95, 0.0]],
        &CHAMFERED,
    );
    let sides = b.sides(8);
    for y in [-0.42f32, 0.42] {
        dark_plate(b);
        b.cylinder_between(v3(0.3, y, 0.0), v3(len - 0.5, y, 0.0), 0.3, 0.28, sides);
        if fine {
            metal(b);
            b.cylinder_between(
                v3(len - 0.6, y, 0.0),
                v3(len - 0.35, y, 0.0),
                0.33,
                0.33,
                sides,
            );
        }
        dark_plate(b);
        b.cylinder_between(v3(len - 0.4, y, 0.0), v3(len, y, 0.0), 0.29, 0.32, sides);
        if fine {
            mouth_rim(b, v3(len, y, 0.0), 0.32, sides);
        }
    }
    if fine {
        red_slot(b, v3(-0.2, 0.0, 0.48), Vec3::Z, Vec3::Y, 1.1, 0.07);
    }
}

// ---- the arrowhead --------------------------------------------------------------------

/// A broad flat arrowhead, a whaleback deck rolling down into low sides, the beam
/// widest aft where the quarters run out into two long swept points; a raised spine from
/// the gun back to a long, low fin; bow planes forward like a submarine's; a bronze trench
/// of machinery along each side under the whaleback.
const HULL: [Station; 9] = [
    st(TRANSOM, -1.2, [5.6, -0.3], [5.3, 1.3], 2.5),
    st(-19.0, -2.4, [6.2, -0.7], [5.9, 1.4], 2.85),
    st(-11.0, -3.2, [6.0, -0.9], [5.6, 1.5], 3.05),
    st(-2.0, -3.5, [5.3, -1.0], [4.9, 1.55], 3.1),
    st(7.0, -3.4, [4.3, -1.1], [3.9, 1.6], 3.05),
    st(14.0, -3.2, [3.2, -1.2], [2.85, 1.6], 2.85),
    st(19.5, -3.0, [2.3, -1.4], [1.9, 1.5], 2.55),
    st(23.0, -2.95, [1.7, -1.7], [1.05, 1.0], 1.6),
    st(BOW_FACE, -2.9, [1.5, -1.95], [0.95, -1.15], -0.95),
];

/// The fin's plan: long and low, a pointed nose, flared aft.
const FIN: [[f32; 2]; 8] = [
    [7.0, 0.0],
    [4.0, 1.0],
    [-1.0, 1.7],
    [-6.0, 1.9],
    [-8.0, 0.0],
    [-6.0, -1.9],
    [-1.0, -1.7],
    [4.0, -1.0],
];
const FIN_X: f32 = -2.0;

fn build(b: &mut MeshBuilder, _tech: u8) {
    hull(b, &HULL, &[0, 3, 8]);
    fin(b);
    pinch_gun(b, deck(&HULL, GUN.x));
    flak(b, deck(&HULL, FLAK.x));
    if b.coarse() {
        team_patch(b, -18.5, -16.0, 1.4, deck(&HULL, -18.0) + 0.05);
        return;
    }
    tube_doors(b);
    interceptor_doors(b);
    let hull = &HULL;
    b.mirror_y(|b| {
        // The whaleback's edge: a long plate over the side, the trench of bronze
        // machinery in its shadow, the quarter's point running out past the stern.
        let (zs, ys) = (1.55, |x: f32| station_at(hull, x).side[0]);
        metal(b);
        b.beam(
            v3(18.0, ys(18.0) - 0.15, zs - 0.1),
            v3(-21.0, ys(-21.0) - 0.15, zs - 0.1),
            Vec2::new(0.5, 0.45),
            Vec2::new(0.5, 0.45),
        );
        if b.fine() {
            for x in [12.0f32, 6.0, 0.0, -6.0, -12.0, -18.0] {
                metal(b);
                b.cylinder_between(
                    v3(x, ys(x) - 0.4, zs - 0.1),
                    v3(x, ys(x) + 0.12, zs - 0.1),
                    0.32,
                    0.32,
                    6,
                );
            }
        }
        let f = Frame::new(
            v3(-13.0, ys(-13.0) - 0.4, zs + 0.15),
            v3(-1.0, 0.1, -0.02),
            Vec3::Z,
        );
        dark_plate(b);
        armour(b, &f, &swept(16.0, 1.05, 0.75, 0.55), 0.28);
        // Bow planes: swept plates off the flanks forward, as a submarine's.
        let (y, n) = flank(hull, 17.5, 0.6);
        let f = Frame::new(
            v3(17.5, y - 0.1, 0.6),
            v3(-1.0, 0.85, -0.05),
            Vec3::Z + n * 0.05,
        );
        armour(b, &f, &swept(3.6, 0.8, -0.6, 0.45), 0.2);
        // Two plates lapped along the whaleback either side of the spine.
        for (x, len) in [(11.0, 7.5), (-14.6, 6.0)] {
            if !b.fine() {
                break;
            }
            let z = deck(hull, x) - 0.35;
            let f = Frame::new(v3(x, 2.2, z), v3(-1.0, 0.12, -0.03), v3(0.0, 0.3, 1.0));
            dark_plate(b);
            armour(b, &f, &swept(len, 0.9, 0.5, 0.55), 0.22);
        }
    });
    quarterdeck(b);
}

/// The arrowhead's fin: long and low, its top a separate plate on a bronze band, run back
/// into the spike; a raised spine from the gun's barbette to its nose.
fn fin(b: &mut MeshBuilder) {
    let z0 = deck(&HULL, FIN_X) - 0.15;
    if b.coarse() {
        dark_plate(b);
        b.frustum_open(
            v3(FIN_X - 1.0, 0.0, z0),
            Vec2::new(14.0, 3.6),
            Vec2::new(6.0, 1.8),
            9.4 - z0,
            Vec2::new(-4.0, 0.0),
        );
        return;
    }
    let band = 6.4;
    b.at(v3(FIN_X, 0.0, 0.0), |b| {
        dark_plate(b);
        b.loft_z(
            &FIN,
            &[
                Section::new(z0, 1.0),
                Section::scaled(z0 + 1.0, 0.96, 0.92).shifted(-0.5, 0.0),
                Section::scaled(band, 0.8, 0.72).shifted(-2.0, 0.0),
                Section::scaled(8.2, 0.5, 0.42).shifted(-3.2, 0.0),
                Section::scaled(8.9, 0.36, 0.3).shifted(-3.6, 0.0),
            ],
        );
        metal(b);
        b.loft_z(
            &FIN,
            &[
                Section::scaled(band - 0.45, 0.86, 0.79).shifted(-1.6, 0.0),
                Section::scaled(band - 0.05, 0.84, 0.77).shifted(-1.9, 0.0),
            ],
        );
    });
    // The spine: a plated ridge from the barbette's back to the fin's nose.
    let (zs, zf) = (deck(&HULL, 11.0), deck(&HULL, 3.0));
    dark_plate(b);
    body_x(
        b,
        &[
            [11.6, 1.6, 1.4, zs + 0.2],
            [7.0, 2.0, 1.8, zs + 0.35],
            [3.4, 2.3, 2.2, zf + 0.5],
        ],
        &[
            [1.0, -1.0],
            [0.55, 0.6],
            [0.0, 1.0],
            [-0.55, 0.6],
            [-1.0, -1.0],
        ],
    );
    let top = 8.9;
    radar(b, v3(FIN_X - 5.6, 0.0, top - 1.15));
    shield_projector(b, v3(FIN_X - 2.6, 0.0, top + 0.6), top - 0.1);
    // The crest's plate, swept back off the top past the fin's back into its spike.
    dark_plate(b);
    let f = Frame::new(
        v3(FIN_X - 0.2, 0.0, top - 0.25),
        v3(-1.0, 0.0, -0.22),
        Vec3::Z,
    );
    armour(b, &f, &swept(9.0, 1.1, 0.0, 0.5), 0.24);
    if b.fine() {
        let nose = FIN_X - 2.0 + FIN[0][0] * 0.8;
        red_slot(
            b,
            v3(nose + 0.1, 0.0, band + 0.2),
            v3(1.0, 0.0, 0.3),
            Vec3::Y,
            0.8,
            0.12,
        );
        b.mirror_y(|b| {
            red_slot(
                b,
                v3(9.0, 0.95, deck(&HULL, 9.0) + 0.8),
                v3(0.0, 1.0, 0.6),
                Vec3::X,
                3.0,
                0.08,
            );
        });
    }
}

/// The arrowhead's deck: the owner's colour on the quarterdeck, hatches, a stern light.
fn quarterdeck(b: &mut MeshBuilder) {
    let hull = &HULL;
    team_patch(b, -19.5, -16.2, 1.4, deck(hull, -18.0) + 0.06);
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        metal(b);
        b.prism(v3(-22.0, 1.5, deck(hull, -22.0) - 0.1), 6, 0.55, 0.45, 0.25);
        b.prism(v3(17.0, 1.0, deck(hull, 17.0) - 0.1), 6, 0.45, 0.38, 0.2);
    });
    red_slot(b, v3(TRANSOM - 0.02, 0.0, 1.2), -Vec3::X, Vec3::Y, 3.6, 0.1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_model, material, rig, Model};

    fn house_verts(model: &Model, lod: usize, weapon: u8) -> Vec<Vec3> {
        let slot = model
            .houses
            .iter()
            .position(|h| h.weapon == weapon)
            .expect("a house for the weapon") as u32;
        model.lods[lod]
            .vertices
            .iter()
            .filter(|v| v.rig & rig::LIMB_MASK == rig::HOUSE_FIRST + slot)
            .map(|v| Vec3::from(v.pos))
            .collect()
    }

    /// The destroyer floats, fits its blueprint, wears dark plate and the owner's colour,
    /// keeps its houses on the unit file's pivots, and holds its weapons where they fire.
    #[test]
    fn it_fits_and_holds_its_weapons() {
        let key = "regency_destroyer";
        let model = build_model(key).unwrap();
        let houses: Vec<(u8, Vec3)> = model
            .houses
            .iter()
            .map(|h| (h.weapon, Vec3::from(h.pivot)))
            .collect();
        assert_eq!(houses, vec![(0, GUN), (2, FLAK)], "{key}: houses");
        let shield = Vec3::from(model.shield_emitter.expect("a hull shield"));
        assert!(shield.z > 8.0, "{key}: shield projector at {shield}");
        for (lod, mesh) in model.lods.iter().enumerate() {
            let name = format!("{key} lod{lod}");
            let (lo, hi) = mesh
                .vertices
                .iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                    (lo.min(v.pos[2]), hi.max(v.pos[2]))
                });
            assert!(
                (HEIGHT * 0.8..=HEIGHT * 1.25).contains(&hi),
                "{name}: top {hi}"
            );
            assert!((-4.5..-2.0).contains(&lo), "{name}: keel {lo}");
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(0.0, f32::max);
            assert!(
                (RADIUS * 0.75..=RADIUS * 1.3).contains(&reach),
                "{name}: reach {reach}"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == material::TEAM && v.normal[2] > 0.5),
                "{name}: no upward team colour"
            );
            assert!(mesh
                .vertices
                .iter()
                .any(|v| v.material == material::PLATING_DARK));
            assert!(
                !mesh.vertices.iter().any(|v| v.material == material::GLOW
                    || v.material == material::GLOW_ORANGE
                    || v.part == part::LOCOMOTION),
                "{name}: ARC's light or running gear"
            );
            let gun = house_verts(&model, lod, 0);
            assert!(!gun.is_empty(), "{name}: no gun house");
            let past = gun.iter().map(|p| p.x).fold(f32::MIN, f32::max);
            assert!(past <= CHARGE.x + 0.5, "{name}: the fork reaches {past}");
            if lod == 2 {
                continue;
            }
            // The charge is held between the projectors, clear of them.
            let near = gun
                .iter()
                .map(|p| p.distance(CHARGE))
                .fold(f32::MAX, f32::min);
            assert!(
                near > HOLD * 0.4,
                "{name}: projectors {near} m into the charge"
            );
            let round: Vec<Vec3> = gun
                .iter()
                .map(|p| *p - CHARGE)
                .filter(|d| d.length() < HOLD)
                .map(|d| d.with_x(0.0))
                .collect();
            assert!(
                round.iter().any(|a| round.iter().any(|c| a.dot(*c) < 0.0)),
                "{name}: nothing holds the charge from both sides"
            );
            let flak = house_verts(&model, lod, 2);
            let near = flak
                .iter()
                .map(|p| p.distance(FLAK_MUZZLE))
                .fold(f32::MAX, f32::min);
            assert!(near < 0.4, "{name}: flak tubes {near} m from the muzzle");
            for m in TUBES.iter().chain(INTERCEPT.iter()) {
                let m = Vec3::from(*m);
                let near = mesh
                    .vertices
                    .iter()
                    .filter(|v| v.rig & rig::LIMB_MASK == 0)
                    .map(|v| Vec3::from(v.pos).distance(m))
                    .fold(f32::MAX, f32::min);
                assert!(near < 0.4, "{name}: no door within {near} m of {m}");
            }
        }
    }

    /// The unit file's pivots and muzzles are the model's.
    #[test]
    fn the_unit_files_weapons_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t2_destroyer").unwrap());
        assert_eq!(bp.visual.mesh, "regency_destroyer");
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let w = &bp.weapons;
        let close = |a: Vec3, b: Vec3| a.distance(b) < 0.02;
        assert!(close(v(w[0].pivot.unwrap()), GUN) && close(v(w[0].muzzle), CHARGE));
        assert!(close(v(w[2].pivot.unwrap()), FLAK) && close(v(w[2].muzzle), FLAK_MUZZLE));
        let tubes: Vec<Vec3> = w[1].muzzles.iter().map(|&p| v(p)).collect();
        assert_eq!(tubes.len(), TUBES.len());
        assert!(tubes
            .iter()
            .zip(TUBES)
            .all(|(a, b)| close(*a, Vec3::from(b))));
        let ints: Vec<Vec3> = w[3].muzzles.iter().map(|&p| v(p)).collect();
        assert_eq!(ints.len(), INTERCEPT.len());
        assert!(ints
            .iter()
            .zip(INTERCEPT)
            .all(|(a, b)| close(*a, Vec3::from(b))));
    }
}
