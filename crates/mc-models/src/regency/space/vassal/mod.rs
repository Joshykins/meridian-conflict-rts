//! Vassal: the Regency's tech 3 escort frigate (`regency_t2_space_frigate`, mesh
//! `regency_space_frigate`), the Valiant's peer. A small faceted gun hull (`body.rs`)
//! cut short at a split prow, two blades run out ahead over the lance, dressed as the
//! Suzerain is (`style.rs`): swept shards in courses, a horned bridge astern, canted
//! fins, cowled drives and slung lifts.
//!
//! One Pinch-plasma Lance hangs under the forward keel on a pylon and turns right round,
//! laying down onto the ground or across at a ship. Three Twin Escort Plasma Repeaters
//! keep the sky: one on the foredeck, one on a barbette amidships firing over it, one
//! hung under the stern.
//! +X is forward, +Y port, the ground at z 0. Authored in metres as built.
//!
//! Contracts (tests.rs): the houses' pivots and bores are `space.ron`'s.

mod guns;

use glam::Vec3;

use crate::builder::MeshBuilder;

use super::super::kit::{dark_plate, v3};
use super::body::{st, Body, Station, BILGE, CHINE, DECK_EDGE, SPINE};
use super::dress::mark;
use super::style::{bridge, course, drives, fin, lifts, ridge, slab, teeth, Finish};
use guns::{lance, pylon, twin};

/// The lance's pivot under the keel, its bore's reach ahead and its house's half-width.
const LANCE: Vec3 = Vec3::new(14.0, 0.0, 4.5);
const LANCE_REACH: f32 = 24.0;
const LANCE_S: f32 = 3.6;
/// The repeaters' pivots (weapons 1 to 3): foredeck, barbette, belly.
const TWINS: [Vec3; 3] = [
    Vec3::new(26.0, 0.0, 17.5),
    Vec3::new(-12.0, 0.0, 19.0),
    Vec3::new(-26.0, 0.0, 4.0),
];
const TWIN_REACH: f32 = 8.0;
const TWIN_GAP: f32 = 1.8;
const TWIN_S: f32 = 2.8;

/// The hull's stations, stern first: cut short at the bow, where the prow's blades run
/// out ahead of it.
const STATIONS: [Station; 6] = [
    st(-50.0, 10.0, 4.0, 8.0, 11.5, 6.0, 15.0, 16.0),
    st(-36.0, 15.0, 5.5, 6.5, 11.0, 8.5, 15.5, 16.5),
    st(-10.0, 16.0, 6.0, 6.0, 11.0, 9.0, 15.5, 17.0),
    st(16.0, 13.0, 5.5, 6.0, 11.0, 7.5, 15.0, 16.5),
    st(32.0, 8.0, 3.0, 7.5, 11.0, 4.0, 14.0, 15.0),
    st(40.0, 1.0, 0.4, 10.0, 11.0, 0.5, 12.0, 12.0),
];

/// Every gun's pivot and bores in weapon order (`space.ron`'s `pivot`, `muzzle` and
/// `muzzles`; the lance has one bore).
#[cfg(test)]
pub(crate) fn gun_mounts() -> Vec<(Vec3, Vec<Vec3>)> {
    let lance = (LANCE, vec![LANCE + Vec3::X * LANCE_REACH]);
    let twins = TWINS.map(|p| {
        let bores = [TWIN_GAP * 0.5, -TWIN_GAP * 0.5]
            .map(|y| p + v3(TWIN_REACH, y, 0.0))
            .to_vec();
        (p, bores)
    });
    std::iter::once(lance).chain(twins).collect()
}

pub(super) fn build(b: &mut MeshBuilder, _tech: u8) {
    let body = Body {
        stations: STATIONS.to_vec(),
    };
    let (stern, bow) = body.span();
    if b.coarse() {
        let mid = body.at(-10.0);
        slab(
            b,
            &[
                [bow, 0.0],
                [20.0, body.at(20.0).w],
                [-10.0, mid.w],
                [stern, body.at(stern).w],
            ],
            7.0,
            17.0,
        );
        mark(b, deck(&body, 4.0) + Vec3::Z * 0.3, 5.0);
        return;
    }
    dark_plate(b);
    body.shell(b);
    b.mirror_y(|b| {
        armour(b, &body, (bow - 2.0, stern + 2.0));
        teeth(b, &body, stern + 4.0, &[2.5, 6.0], 1.6, 6.0);
        ridge(b, &body, (-22.0, stern + 6.0), 3.2, 2.0);
        fin(b, deck(&body, stern + 14.0) + v3(0.0, 5.0, -3.0), 18.0, 3.0);
        let half = body.at(stern).w * 0.45;
        drives(b, &[(v3(stern, half, 11.5), 2.6, 7.0)]);
        lifts(
            b,
            &[
                (v3(24.0, 4.5, 7.0), 2.4, 2.0),
                (v3(-30.0, 5.5, 7.0), 2.4, 2.0),
            ],
        );
    });
    prongs(b, bow);
    bridge(b, &body, (stern + 4.0, -28.0), 5.0, 3.6);
    mark(b, deck(&body, 4.0) + Vec3::Z * 1.0, 5.0);
    // The lance's pylon out of the keel.
    pylon(
        b,
        v3(LANCE.x, 0.0, body.at(LANCE.x).keel + 0.6),
        LANCE + Vec3::Z * LANCE_S * 0.4,
        LANCE_S * 0.7,
    );
    lance(b, 0, LANCE, LANCE_REACH, LANCE_S);
    for (k, p) in TWINS.into_iter().enumerate() {
        let hull_z = if p.z > 10.0 {
            body.at(p.x).spine - 0.6
        } else {
            body.at(p.x).keel + 0.6
        };
        let roof = if p.z > 10.0 { -0.4 } else { 0.4 };
        pylon(
            b,
            v3(p.x, 0.0, hull_z),
            p + Vec3::Z * (roof * TWIN_S),
            TWIN_S * 0.9,
        );
        twin(b, 1 + k, p, TWIN_REACH, TWIN_GAP, TWIN_S);
    }
}

/// The spine's top at `x`.
fn deck(body: &Body, x: f32) -> Vec3 {
    body.point(x, SPINE)
}

/// The port side's armour from `bow` back to `stern`: courses on the bilge, the chine
/// (lit), over the shoulder and along the deck edge.
fn armour(b: &mut MeshBuilder, body: &Body, (bow, stern): (f32, f32)) {
    let fine = b.fine();
    let n = |k: usize| if fine { k } else { k.div_ceil(2) };
    let rows = [
        (bow - 4.0, BILGE + 0.4, n(7), (2.6, 0.4), 0),
        (bow, CHINE + 0.6, n(8), (3.0, 0.5), 4),
        (bow - 3.0, CHINE + 1.5, n(8), (2.6, 0.4), 0),
        (bow - 6.0, CHINE - 0.1, n(7), (2.4, 0.4), 0),
        (bow - 6.0, DECK_EDGE + 0.3, n(7), (2.2, 0.4), 0),
    ];
    for (from, u, count, size, lit) in rows {
        course(b, body, (from, stern), u, count, size, Finish::Dark, lit);
    }
}

/// The split prow: two mandible blades from the hull's shoulders out past the `bow`,
/// raked in toward their tips, a gap between them over the lance. Both sides.
fn prongs(b: &mut MeshBuilder, bow: f32) {
    dark_plate(b);
    b.mirror_y(|b| {
        let rings: Vec<Vec<Vec3>> = [
            (bow - 18.0, 6.0, 3.0, 8.0, 13.5),
            (bow + 6.0, 4.5, 2.0, 8.5, 12.5),
            (bow + 20.0, 2.6, 0.6, 9.5, 11.0),
        ]
        .iter()
        .map(|&(x, y, half, low, high)| {
            vec![
                v3(x, y - half, low),
                v3(x, y + half, low + 0.8),
                v3(x, y + half * 0.7, high),
                v3(x, y - half * 0.7, high - 0.4),
            ]
        })
        .collect();
        b.with_facets(|b| b.loft(&rings, true, true));
    });
}
