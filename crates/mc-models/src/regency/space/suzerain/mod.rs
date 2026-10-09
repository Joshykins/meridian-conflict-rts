//! Suzerain: the Regency's tech 3 warship (`regency_t3_space_cruiser`, mesh
//! `regency_space_cruiser`), the Resolute's peer. A narrow faceted gun hull of hard
//! chines (`body.rs`), dressed as the Ark is (`style.rs`): swept armour shards in
//! courses, twin ridges down the spine aft of a horned bridge, canted fins over the
//! stern, teeth past it and a keel blade under the bow.
//!
//! The weapons are the hull's: the chine swells into a blister at each of the six
//! suppression batteries, the ball seated in a pocket pressed into it under a hooded
//! plate; the two heavy anti-ship guns turn in armoured wells let into the deck, the
//! after one on a raised step of the hull so it fires over the forward one, armour
//! cheeks standing up either side of the forward one.
//! +X is forward, +Y port, the ground at z 0. Authored in metres as built.
//!
//! Contracts (tests.rs): the gun houses' pivots and bores are `space.ron`'s.

mod guns;
mod style;

use std::sync::LazyLock;

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::pattern;

use super::super::kit::{dark_plate, v3};
use super::super::machine::Frame;
use super::body::{st, Body, Station, BILGE, CHINE, DECK_EDGE, SPINE};
use super::dress::mark;
use guns::{ball, heavy};
use style::{bridge, course, drives, fin, lifts, on, prow, ridge, shard, teeth, Finish};

/// The heavy guns' bores ahead of their pivots, and the batteries'.
const HEAVY_REACH: f32 = 15.0;
const BALL_REACH: f32 = 12.0;
/// A battery ball's radius, and a heavy gun house's half-width.
const BALL_R: f32 = 5.0;
const HOUSE: f32 = 4.4;

/// The hull's stations, stern first, before the blisters: a raised step from x 30 to
/// 50 for the after heavy gun.
const STATIONS: [Station; 9] = [
    st(-92.0, 14.0, 6.0, 9.0, 15.0, 8.0, 23.0, 24.0),
    st(-70.0, 21.0, 9.0, 6.0, 14.0, 11.0, 25.0, 27.0),
    st(-10.0, 23.0, 10.0, 5.0, 14.0, 12.0, 25.0, 27.0),
    st(22.0, 21.0, 9.0, 5.5, 14.0, 11.0, 25.5, 27.5),
    st(30.0, 20.5, 9.0, 5.5, 14.0, 11.0, 28.0, 30.5),
    st(50.0, 18.5, 8.0, 6.0, 14.0, 10.0, 28.0, 30.5),
    st(57.0, 17.5, 8.0, 6.0, 14.0, 9.0, 24.0, 26.0),
    st(92.0, 8.0, 4.0, 8.0, 14.0, 4.0, 19.0, 20.0),
    st(110.0, 1.5, 0.8, 11.0, 14.0, 0.8, 15.0, 15.0),
];

/// The batteries' blisters down each chine, fore to aft (port of each pair first).
const MOUNTS: [f32; 3] = [12.0, -22.0, -56.0];
/// A blister's half-beam at the chine, and its half lengths: full, and tapered back
/// into the plain hull.
const BLISTER_W: f32 = 33.0;
const BLISTER: (f32, f32) = (10.0, 20.0);
/// The ball's pocket: how far the chine is pressed in at its bottom, its half length
/// there, and how far the ball's middle stands out of it.
const NOTCH: f32 = 0.75 * BALL_R;
const POCKET: f32 = 0.45 * BALL_R;
const BALL_OUT: f32 = NOTCH - 0.1 * BALL_R;

/// The heavy guns' wells, forward first: x, and how high the pivot stands off the deck.
const WELLS: [(f32, f32); 2] = [(76.0, 1.8), (40.0, 1.8)];

/// The hull: `STATIONS`, the chine and deck swollen into a blister at each mount and
/// the chine pressed into a pocket at the blister's middle.
static HULL: LazyLock<Body> = LazyLock::new(|| {
    let plain = Body {
        stations: STATIONS.to_vec(),
    };
    let mut stations: Vec<Station> = STATIONS
        .iter()
        .filter(|s| MOUNTS.iter().all(|&m| (s.x - m).abs() >= BLISTER.1))
        .copied()
        .collect();
    for &m in &MOUNTS {
        for (dx, w, notch) in [
            (BLISTER.1, None, 0.0),
            (BLISTER.0, Some(BLISTER_W), 0.0),
            (POCKET, Some(BLISTER_W), NOTCH),
            (-POCKET, Some(BLISTER_W), NOTCH),
            (-BLISTER.0, Some(BLISTER_W), 0.0),
            (-BLISTER.1, None, 0.0),
        ] {
            let s = plain.at(m + dx);
            let w = w.unwrap_or(s.w);
            // The deck widens with the blister, so its shoulder keeps its slope.
            stations.push(Station {
                deck_w: s.deck_w + (w - s.w) * 0.55,
                w,
                notch,
                ..s
            });
        }
    }
    stations.sort_by(|a, c| a.x.total_cmp(&c.x));
    Body { stations }
});

/// To the quarter metre, so the unit file can give the same numbers exactly.
fn snap(p: Vec3) -> Vec3 {
    (p * 4.0).round() / 4.0
}

/// The heavy guns' pivots, forward first (weapons 0 and 1).
fn heavy_pivots() -> [Vec3; 2] {
    WELLS.map(|(x, up)| snap(deck(&HULL, x) + Vec3::Z * up))
}

/// The batteries' pivots in weapon order (weapons 2 to 7): each ball's middle in its
/// pocket, port then starboard, fore to aft.
fn battery_pivots() -> [Vec3; 6] {
    std::array::from_fn(|k| {
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let p = snap(HULL.out(MOUNTS[k / 2], CHINE, BALL_OUT));
        v3(p.x, p.y * side, p.z)
    })
}

/// Every gun's pivot and bore in weapon order (`space.ron`'s `pivot` and `muzzle`).
#[cfg(test)]
pub(crate) fn gun_mounts() -> Vec<(Vec3, Vec3)> {
    let heavy = heavy_pivots().map(|p| (p, p + Vec3::X * HEAVY_REACH));
    let balls = battery_pivots().map(|p| (p, p + Vec3::X * BALL_REACH));
    heavy.into_iter().chain(balls).collect()
}

pub(super) fn build(b: &mut MeshBuilder, _tech: u8) {
    let body: &Body = &HULL;
    if b.coarse() {
        slab(
            b,
            &[
                [110.0, 0.0],
                [57.0, 17.5],
                [MOUNTS[0] + BLISTER.0, BLISTER_W],
                [MOUNTS[2] - BLISTER.0, BLISTER_W],
                [-92.0, 14.0],
            ],
            8.0,
            26.0,
        );
        mark(b, v3(14.0, 0.0, 28.1), 11.0);
        return;
    }
    dark_plate(b);
    body.shell(b);
    b.mirror_y(|b| {
        armour(b, body, (108.0, -90.0));
        for &m in &MOUNTS {
            hood(b, body, m);
        }
        cheeks(b, body, WELLS[0].0);
        teeth(b, body, -86.0, &[3.0, 8.0], 2.2, 10.0);
        ridge(b, body, (-34.0, -84.0), 5.0, 3.0);
        fin(b, deck(body, -60.0) + v3(0.0, 7.0, -1.5), 26.0, 4.0);
        drives(b, &[(v3(-92.0, 6.5, 15.0), 4.0, 10.0)]);
        lifts(
            b,
            &[
                (v3(40.0, 6.0, 6.0), 3.6, 2.5),
                (v3(-40.0, 7.0, 5.5), 3.6, 2.5),
            ],
        );
    });
    bridge(b, body, (-30.0, -4.0), 9.0, 6.0);
    prow(b, (72.0, 116.0), (6.0, 11.5));
    mark(b, deck(body, 14.0) + Vec3::Z * 1.3, 11.0);
    for (k, p) in heavy_pivots().into_iter().enumerate() {
        well(b, p, HOUSE * 1.9);
        heavy(b, k, p, HEAVY_REACH, HOUSE);
    }
    for (k, p) in battery_pivots().into_iter().enumerate() {
        ball(b, 2 + k, p, BALL_REACH, BALL_R);
    }
}

/// The spine's top at `x`.
fn deck(body: &Body, x: f32) -> Vec3 {
    body.point(x, SPINE)
}

/// The port side's armour from `bow` back to `stern`: shard courses on the keel (at
/// full detail), the flank, the chine, the shoulder (its optics lit) and the deck.
fn armour(b: &mut MeshBuilder, body: &Body, (bow, stern): (f32, f32)) {
    let fine = b.fine();
    let n = |k: usize| if fine { k } else { k.div_ceil(2) };
    if fine {
        course(
            b,
            body,
            (bow - 10.0, stern),
            0.5,
            7,
            (2.2, 0.5),
            Finish::Dark,
            0,
        );
    }
    course(
        b,
        body,
        (bow - 4.0, stern),
        BILGE + 0.4,
        n(9),
        (4.0, 0.6),
        Finish::Dark,
        0,
    );
    course(
        b,
        body,
        (bow, stern),
        CHINE + 0.6,
        n(10),
        (4.5, 0.7),
        Finish::Dark,
        4,
    );
    course(
        b,
        body,
        (bow - 3.0, stern),
        CHINE + 1.5,
        n(10),
        (4.0, 0.6),
        Finish::Dark,
        0,
    );
    // Along the chine itself, over the blisters' flanks.
    course(
        b,
        body,
        (bow - 8.0, stern),
        CHINE - 0.1,
        n(10),
        (3.6, 0.6),
        Finish::Dark,
        0,
    );
    course(
        b,
        body,
        (bow - 6.0, stern),
        DECK_EDGE + 0.3,
        n(9),
        (3.4, 0.6),
        Finish::Dark,
        0,
    );
    course(
        b,
        body,
        (bow - 14.0, stern),
        DECK_EDGE + 0.75,
        n(8),
        (3.0, 0.5),
        Finish::Dark,
        0,
    );
}

/// A plan slab: `half` the port half from bow to stern, mirrored, from `low` to `high`.
fn slab(b: &mut MeshBuilder, half: &[[f32; 2]], low: f32, high: f32) {
    let mut outline = half.to_vec();
    outline.extend(
        half.iter()
            .rev()
            .filter(|p| p[1] > 0.0)
            .map(|&[x, y]| [x, -y]),
    );
    dark_plate(b);
    b.extrude_z(&outline, low, high);
}

/// A stack of faceted blocks in `plan` (about its own middle `at`): each level `(z,
/// scale)`.
fn prism(b: &mut MeshBuilder, at: Vec3, plan: &[[f32; 2]], levels: &[(f32, f32)]) {
    let sections: Vec<Section> = levels.iter().map(|&(z, s)| Section::new(z, s)).collect();
    b.at(at, |b| b.with_facets(|b| b.loft_z(plan, &sections)));
}

/// The port hood over the blister at `m`: a long plate swept back from ahead of the
/// pocket over its top into a spike behind it, and a lesser one under it.
fn hood(b: &mut MeshBuilder, body: &Body, m: f32) {
    let f = on(body, m + BLISTER.0 + 2.0, CHINE + 0.35, 0.3, 0.12);
    shard(b, &f, (BLISTER.0 * 2.6, 3.0, 0.3), 0.8, Finish::Dark);
    if b.fine() {
        let g = on(body, m + BLISTER.0, CHINE - 0.4, 0.3, 0.1);
        shard(b, &g, (BLISTER.0 * 2.3, 2.4, 0.2), 0.6, Finish::Dark);
    }
}

/// An armoured well let into the deck round a heavy gun's `pivot`: a faceted ring of
/// radius `r` from under the deck up to the house's waist, its rim's gasket lit.
fn well(b: &mut MeshBuilder, pivot: Vec3, r: f32) {
    let plan: Vec<[f32; 2]> = (0..8)
        .map(|k| {
            let a = std::f32::consts::TAU * (k as f32 + 0.5) / 8.0;
            [a.cos() * r, a.sin() * r]
        })
        .collect();
    let top = pivot.z - HOUSE * 0.25;
    dark_plate(b);
    prism(
        b,
        v3(pivot.x, pivot.y, 0.0),
        &plan,
        &[(top - 4.0, 1.0), (top - 0.6, 1.0), (top, 0.94)],
    );
    if !b.coarse() {
        dark_plate(b);
        b.pattern(pattern::EMBER_RIM);
        prism(
            b,
            v3(pivot.x, pivot.y, 0.0),
            &plan,
            &[(top - 0.05, 0.95), (top + 0.25, 0.9)],
        );
    }
}

/// The port armour cheek standing up beside the forward heavy gun at `x`: a plate
/// raked up off the deck ahead of the well and swept back past it into a spike.
fn cheeks(b: &mut MeshBuilder, body: &Body, x: f32) {
    let at = deck(body, x + 12.0);
    let f = Frame::new(
        v3(at.x, HOUSE * 1.7, at.z - 0.8),
        v3(-1.0, 0.1, 0.12),
        v3(0.0, 0.8, 1.0),
    );
    shard(b, &f, (30.0, 2.8, 0.6), 0.9, Finish::Dark);
}
