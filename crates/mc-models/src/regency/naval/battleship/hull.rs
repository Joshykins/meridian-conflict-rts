//! The hull's common pieces: a whaleback hull lofted through stations, the deck and
//! flank under a point, plated bodies along x, the bow's torpedo doors and the
//! owner's colour.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::{BOW_FACE, TUBES};

/// One station of a hull, stern first: its keel's depth, the bilge's half-beam and height
/// (the widest, under the water), the deck edge's (tumbled home
/// over it), and the deck's crown on the centreline.
#[derive(Clone, Copy, Debug)]
pub(super) struct Station {
    pub(super) x: f32,
    pub(super) keel: f32,
    pub(super) bilge: [f32; 2],
    pub(super) side: [f32; 2],
    pub(super) crown: f32,
}

pub(super) const fn st(x: f32, keel: f32, bilge: [f32; 2], side: [f32; 2], crown: f32) -> Station {
    Station {
        x,
        keel,
        bilge,
        side,
        crown,
    }
}

pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The station's ring: keel, up the port side to the deck edge, rolled over the whaleback
/// crown and down the starboard. Far off, a V: keel and the two deck edges.
fn ring(b: &MeshBuilder, s: &Station) -> Vec<Vec3> {
    let [by, bz] = s.bilge;
    let [sy, sz] = s.side;
    let k = s.keel;
    if b.coarse() {
        return vec![v3(s.x, 0.0, k), v3(s.x, -sy, sz), v3(s.x, sy, sz)];
    }
    let half: Vec<[f32; 2]> = if b.fine() {
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
        vec![
            [by * 0.62, lerp(k, bz, 0.72)],
            [by, bz],
            [sy, sz],
            [sy * 0.6, lerp(sz, s.crown, 0.75)],
        ]
    };
    let mut r = vec![v3(s.x, 0.0, k)];
    r.extend(half.iter().map(|&[y, z]| v3(s.x, -y, z)));
    r.push(v3(s.x, 0.0, s.crown));
    r.extend(half.iter().rev().map(|&[y, z]| v3(s.x, y, z)));
    r
}

/// The hull lofted through `stations`, or through `coarse` of them far off; capped at the
/// transom and at the bow's blunt face.
pub(super) fn hull(b: &mut MeshBuilder, stations: &[Station], coarse: &[usize]) {
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
pub(super) fn station_at(stations: &[Station], x: f32) -> Station {
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
pub(super) fn deck(stations: &[Station], x: f32) -> f32 {
    station_at(stations, x).crown
}

/// The deck edge (half-beam, height) at `x`.
pub(super) fn edge(stations: &[Station], x: f32) -> (f32, f32) {
    let s = station_at(stations, x);
    (s.side[0], s.side[1])
}

/// The flank's half-beam at `x` and height `z` (between bilge and deck edge), and its
/// outward normal there (port side, +y).
pub(super) fn flank(stations: &[Station], x: f32, z: f32) -> (f32, Vec3) {
    let s = station_at(stations, x);
    let [by, bz] = s.bilge;
    let [sy, sz] = s.side;
    let t = ((z - bz) / (sz - bz)).clamp(0.0, 1.0);
    let n = v3(0.0, sz - bz, -(sy - by)).normalize();
    (lerp(by, sy, t), n)
}

/// The torpedo doors in the bow's face: a dark ring sunk round each, a bronze door.
pub(super) fn tube_doors(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    let sides = b.sides(8);
    for [x, y, z] in TUBES {
        seam(b);
        b.cylinder_between(
            v3(BOW_FACE - 0.3, y, z),
            v3(x - 0.08, y, z),
            0.5,
            0.5,
            sides,
        );
        metal(b);
        b.cylinder_between(v3(x - 0.1, y, z), v3(x, y, z), 0.38, 0.35, sides);
    }
}

/// The owner's colour as a flat patch on an upward face at height `z`.
pub(super) fn team_patch(b: &mut MeshBuilder, x0: f32, x1: f32, half: f32, z: f32) {
    b.paint(TEAM);
    b.face(&[
        v3(x0, -half, z),
        v3(x1, -half, z),
        v3(x1, half, z),
        v3(x0, half, z),
    ]);
}

/// A section of `shape` across y and z about `c`, `w` wide and `h` high.
fn section(c: Vec3, w: f32, h: f32, shape: &[[f32; 2]]) -> Vec<Vec3> {
    shape
        .iter()
        .map(|[s, u]| c + v3(0.0, s * w * 0.5, u * h * 0.5))
        .collect()
}

/// A box with its corners cut.
pub(super) const CHAMFERED: [[f32; 2]; 8] = [
    [1.0, -0.6],
    [0.6, -1.0],
    [-0.6, -1.0],
    [-1.0, -0.6],
    [-1.0, 0.6],
    [-0.6, 1.0],
    [0.6, 1.0],
    [1.0, 0.6],
];

/// A ridge: flat sides, a peaked top.
pub(super) const RIDGE: [[f32; 2]; 5] = [
    [1.0, -1.0],
    [0.55, 0.6],
    [0.0, 1.0],
    [-0.55, 0.6],
    [-1.0, -1.0],
];

/// A box's section, for a chamfered one past full detail.
const BOX: [[f32; 2]; 4] = [[1.0, -1.0], [-1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];

/// A body lofted along x at `y` through `stations` (x, width, height, centre height); a
/// [`CHAMFERED`] one is a plain box past full detail.
pub(super) fn body_x(b: &mut MeshBuilder, y: f32, stations: &[[f32; 4]], shape: &[[f32; 2]]) {
    let shape = if !b.fine() && shape == CHAMFERED.as_slice() {
        BOX.as_slice()
    } else {
        shape
    };
    let rings: Vec<Vec<Vec3>> = stations
        .iter()
        .map(|&[x, w, h, z]| section(v3(x, y, z), w, h, shape))
        .collect();
    b.loft(&rings, true, true);
}

/// A plan outline: `half` of it (y >= 0) from the front round to the back, mirrored.
pub(super) fn mirrored(half: &[[f32; 2]]) -> Vec<[f32; 2]> {
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

/// A pointed plan: the prow `f` ahead of the origin, square shoulders `h` out, the after
/// corners swept in, `a` astern.
pub(super) fn pointed(f: f32, a: f32, h: f32) -> Vec<[f32; 2]> {
    mirrored(&[[f, 0.0], [f * 0.35, h], [-a * 0.75, h], [-a, h * 0.55]])
}

/// A plated tier on `plan` at `x`, from `z0` to `z1`, its top drawn in by `top`, a dark
/// seam band round its foot. Far off, its bounding box drawn in as it rises.
pub(super) fn tier(b: &mut MeshBuilder, x: f32, plan: &[[f32; 2]], z0: f32, z1: f32, top: Vec2) {
    if b.coarse() {
        let (lo, hi) = plan.iter().fold(
            (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
            |(lo, hi), &[px, py]| (lo.min(Vec2::new(px, py)), hi.max(Vec2::new(px, py))),
        );
        dark_plate(b);
        b.frustum_open(
            v3(x + (lo.x + hi.x) * 0.5, 0.0, z0),
            hi - lo,
            (hi - lo) * top,
            z1 - z0,
            Vec2::new(-(hi.x - lo.x) * (1.0 - top.x) * 0.3, 0.0),
        );
        return;
    }
    b.at(v3(x, 0.0, 0.0), |b| {
        seam(b);
        b.loft_z(
            plan,
            &[Section::new(z0, 1.02), Section::new(z0 + 0.4, 1.02)],
        );
        dark_plate(b);
        b.loft_z(
            plan,
            &[
                Section::new(z0 + 0.4, 1.0),
                Section::new(z1 - 0.5, 1.0 - (1.0 - top.x) * 0.6),
                Section::scaled(z1, top.x, top.y),
            ],
        );
    });
}

/// The top of the hull at `x`, `y` out from the centreline: the deck as the full-detail
/// section draws it, and the deck edge's height past it.
pub(super) fn top_at(stations: &[Station], x: f32, y: f32) -> f32 {
    let s = station_at(stations, x);
    let ([sy, sz], c) = (s.side, s.crown);
    let pts = [
        [0.0, c],
        [sy * 0.4, lerp(sz, c, 0.9)],
        [sy * 0.75, lerp(sz, c, 0.55)],
        [sy, sz],
    ];
    let y = y.abs();
    pts.windows(2)
        .find(|w| y <= w[1][0])
        .map(|w| lerp(w[0][1], w[1][1], (y - w[0][0]) / (w[1][0] - w[0][0])))
        .unwrap_or(sz)
}
