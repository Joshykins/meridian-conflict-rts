//! The Flamberge's body: the Claymore grown to a capital ship. A broad arrowhead, its
//! whaleback deck rolling down into low sides, the beam widest aft. Every hull is dressed
//! the same way (this file): an armour belt of plates lapped back along the waterline, a
//! strake of plates along each deck edge, a sponson swelling out of the flank under each
//! beam secondary, a cutwater blade on the stem, drive bays in the transom, plating, bays
//! and lockers over the deck. What stands amidships is the superstructure ([`keep`]).

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::{armour, swept, Course, Frame};
use super::super::super::parts::{bay, inlay, locker, ridged};
use super::super::super::turrets::rondel::lens_post;
use super::guns::{aa_house, secondary};
use super::house::main_gun;
use super::hull::{
    body_x, deck, edge, flank, hull, st, team_patch, top_at, tube_doors, Station, CHAMFERED,
};
use super::keep;
use super::{BOW_FACE, DEFENCE, MAIN, SECONDARY};

pub(super) const TRANSOM: f32 = -60.0;

/// The dreadnought's lines: the deck's sheer climbing forward to a raised forecastle whose
/// sides flare out over the bow, the flanks tumbling home above the waterline, stepping
/// down aft to a low quarterdeck as the beam draws in to a narrow transom.
const HULL: [Station; 10] = [
    st(TRANSOM, -1.4, [7.2, -0.4], [6.8, 2.4], 3.8),
    st(-50.0, -3.2, [10.0, -1.0], [9.4, 2.6], 4.2),
    st(-34.0, -4.1, [12.0, -1.4], [11.0, 3.0], 4.8),
    st(-14.0, -4.3, [12.4, -1.6], [11.2, 3.3], 5.6),
    st(6.0, -4.3, [11.0, -1.7], [10.0, 3.5], 5.8),
    st(22.0, -4.2, [9.0, -1.8], [8.4, 3.9], 6.0),
    st(38.0, -4.1, [6.6, -2.0], [6.8, 4.5], 6.2),
    st(50.0, -4.0, [4.2, -2.3], [5.0, 4.9], 6.0),
    st(57.0, -4.1, [3.0, -2.8], [3.4, 5.1], 5.8),
    st(BOW_FACE, -4.2, [2.5, -3.4], [1.6, 5.2], 5.5),
];

/// The superstructure's crown, where the AA house stands, in every keep.
pub(super) const CROWN: f32 = 13.9;

/// The ship.
pub(super) fn build(b: &mut MeshBuilder, _tech: u8) {
    let h = &HULL;
    hull(b, h, &[0, 4, 9]);
    if !b.coarse() {
        clipper(b, h);
    }
    keep::build(b, h);
    if !b.coarse() {
        // The houses' gear works off each house's own charge; nothing spins about a hub.
        b.set_charge_gear(MAIN[1], 1.0);
    }
    for (i, p) in MAIN.iter().enumerate() {
        main_gun(b, i, deck(h, p.x));
    }
    aa_house(b, CROWN);
    for (k, p) in SECONDARY.iter().enumerate() {
        secondary(b, k, top_at(h, p.x, p.y));
    }
    team_patch(b, -54.0, -48.0, 2.0, deck(h, -51.0) + 0.06);
    if b.coarse() {
        return;
    }
    tube_doors(b);
    for at in DEFENCE {
        lens_post(b, at, keep::lens_foot(at));
    }
    belt(b, h);
    strake(b, h);
    for p in SECONDARY.iter().filter(|p| p.y > 0.0) {
        sponson(b, h, p.x);
    }
    prow(b, h);
    stern(b, h);
    decks(b, h);
}

/// The raised forecastle's clipper prow: a V of plate from the flared bow raked out past the
/// torpedo face over the water to a stem, its foot sweeping up off the bow's face.
fn clipper(b: &mut MeshBuilder, h: &[Station]) {
    let vee = |x: f32, half: f32, top: f32, foot: f32| {
        vec![v3(x, 0.0, foot), v3(x, -half, top), v3(x, half, top)]
    };
    let (half, top) = edge(h, 52.0);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                vee(52.0, half + 0.15, top + 0.1, -1.4),
                vee(57.5, edge(h, 57.5).0 + 0.15, edge(h, 57.5).1 + 0.15, -0.9),
                vee(BOW_FACE + 0.3, 2.1, edge(h, BOW_FACE).1 + 0.25, -0.5),
                vee(67.0, 1.1, edge(h, BOW_FACE).1 + 0.5, 2.2),
                vee(70.5, 0.12, edge(h, BOW_FACE).1 + 0.7, 5.0),
            ],
            false,
            true,
        );
    });
}

/// A point on the port flank at `x`, `z` up, standing `off` proud of it, and the flank's
/// outward normal there.
fn on_flank(h: &[Station], x: f32, z: f32, off: f32) -> (Vec3, Vec3) {
    let (y, n) = flank(h, x, z);
    (v3(x, y, z) + n * off, n)
}

/// The armour belt: plates lapped back along each flank at the waterline, each swept into
/// a point over the head of the one behind it; a shorter course over them forward.
fn belt(b: &mut MeshBuilder, h: &[Station]) {
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        let xs: &[f32] = if fine {
            &[
                52.0, 43.0, 34.0, 25.0, 16.0, 7.0, -2.0, -11.0, -20.0, -29.0, -38.0, -47.0,
            ]
        } else {
            &[50.0, 34.0, 18.0, 2.0, -14.0, -30.0, -46.0]
        };
        let len = if fine { 10.5 } else { 17.5 };
        for &x in xs {
            let (o, n) = on_flank(h, x, 0.6, 0.02);
            let (next, _) = on_flank(h, x - len, 0.6, 0.02);
            armour(
                b,
                &Frame::new(o, next - o, n),
                &swept(len, 1.05, -0.3, 0.72),
                0.32,
            );
        }
        if fine {
            for x in [46.0f32, 37.0, 28.0] {
                let (o, n) = on_flank(h, x, 2.0, 0.3);
                let (next, _) = on_flank(h, x - 8.0, 2.0, 0.3);
                armour(
                    b,
                    &Frame::new(o, next - o, n),
                    &swept(8.0, 0.55, 0.4, 0.7),
                    0.24,
                );
            }
        }
    });
}

/// The deck edges: a strake of plates along each, lapped back, a quarter plate run out past
/// the stern into its point; a trench of graphite machinery in their shadow; bow planes.
fn strake(b: &mut MeshBuilder, h: &[Station]) {
    let fine = b.fine();
    b.mirror_y(|b| {
        let at = |x: f32| {
            let (y, z) = edge(h, x);
            v3(x, y, z)
        };
        dark_plate(b);
        let xs: &[f32] = if fine {
            &[48.0, 38.0, 28.0, 18.0, 8.0, -2.0, -12.0, -22.0]
        } else {
            &[48.0, 28.0, 8.0, -12.0]
        };
        let len = if fine { 11.0 } else { 21.0 };
        for &x in xs {
            let o = at(x) + v3(0.0, -0.5, 0.25);
            let f = Frame::new(o, at(x - len) - at(x), v3(0.0, 0.4, 1.0));
            armour(b, &f, &swept(len, 1.15, 0.6, 0.62), 0.32);
        }
        armour(
            b,
            &Frame::new(
                at(-30.0) + v3(0.0, -1.6, 0.3),
                at(-60.0) - at(-30.0),
                v3(0.0, 0.4, 1.0),
            ),
            &[
                [0.0, 1.6],
                [0.0, -1.2],
                [22.0, -1.6],
                [34.0, -1.2],
                [22.0, 1.4],
            ],
            0.34,
        );
        let xs = [48.0f32, 30.0, 12.0, -8.0, -28.0, -46.0, -57.0];
        metal(b);
        for w in xs.windows(2) {
            b.beam(
                at(w[0]) + v3(0.0, 0.25, -0.5),
                at(w[1]) + v3(0.0, 0.25, -0.5),
                Vec2::new(0.6, 0.55),
                Vec2::new(0.6, 0.55),
            );
        }
        if fine {
            for &x in &xs[1..xs.len() - 1] {
                let p = at(x) + v3(0.0, 0.25, -0.5);
                b.cylinder_between(p - Vec3::Y * 0.5, p + Vec3::Y * 0.2, 0.45, 0.45, 8);
            }
        }
        let (y, n) = flank(h, 46.0, 1.8);
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(46.0, y - 0.1, 1.8),
                v3(-1.0, 0.85, -0.05),
                Vec3::Z + n * 0.05,
            ),
            &swept(7.0, 1.4, -0.6, 0.45),
            0.3,
        );
    });
}

/// The sponson under the port secondary at `x` (and its starboard twin): a plated blister
/// swelling out of the flank, a faceted plate over its shoulder, an optic slot in its face
/// and a plate lapped back off its tail.
fn sponson(b: &mut MeshBuilder, h: &[Station], x: f32) {
    let fine = b.fine();
    b.mirror_y(|b| {
        let (y, _) = edge(h, x);
        dark_plate(b);
        body_x(
            b,
            y - 0.4,
            &[
                [x + 6.5, 0.6, 1.0, 1.8],
                [x + 4.0, 3.0, 3.6, 2.0],
                [x - 3.5, 3.2, 3.8, 2.0],
                [x - 6.5, 0.8, 1.4, 1.8],
            ],
            &CHAMFERED,
        );
        if !b.mid() {
            return;
        }
        ridged(
            b,
            &Frame::new(v3(x + 3.6, y + 1.1, 3.75), -Vec3::X, v3(0.0, 0.5, 1.0)),
            &[[0.0, -1.0], [0.0, 0.7], [7.0, 0.7], [7.6, -1.0]],
            0.26,
            [3.6, 0.0],
            0.16,
        );
        armour(
            b,
            &Frame::new(v3(x - 4.5, y + 1.15, 2.0), v3(-1.0, -0.1, 0.0), Vec3::Y),
            &swept(5.0, 0.8, 0.0, 0.5),
            0.26,
        );
        if fine {
            inlay(
                b,
                &[
                    v3(x + 1.6, y + 1.21, 1.7),
                    v3(x - 1.6, y + 1.21, 1.7),
                    v3(x - 1.6, y + 1.21, 2.05),
                    v3(x + 1.6, y + 1.21, 2.05),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        }
    });
}

/// The stem's cutwater: a faceted blade along the bow's crown running out to the stem, and
/// an optic slot either side of the bow.
fn prow(b: &mut MeshBuilder, h: &[Station]) {
    // On the raised forecastle it runs out level past the stem.
    let fall = 0.03;
    dark_plate(b);
    ridged(
        b,
        &Frame::new(
            v3(44.0, 0.0, deck(h, 44.0) - 0.1),
            v3(1.0, 0.0, fall),
            Vec3::Z,
        ),
        &[
            [0.0, -0.9],
            [0.0, 0.9],
            [12.0, 0.5],
            [18.5, 0.0],
            [12.0, -0.5],
        ],
        0.3,
        [8.0, 0.0],
        0.35,
    );
    if b.fine() {
        b.mirror_y(|b| {
            let (o, n) = on_flank(h, 55.0, 0.2, 0.03);
            inlay(
                b,
                &[
                    o,
                    o + v3(-2.2, 0.35, 0.0),
                    o + v3(-2.2, 0.35, 0.3),
                    o + v3(0.0, 0.0, 0.3),
                ],
                n,
                pattern::EMBER_SLOT,
            );
        });
    }
}

/// The stern: a faceted plate over the quarterdeck, two drive bays in the transom's face
/// either side of a slot, plates lapped down over the transom's corners.
fn stern(b: &mut MeshBuilder, h: &[Station]) {
    let fine = b.fine();
    let z = deck(h, -52.0);
    dark_plate(b);
    ridged(
        b,
        &Frame::new(v3(-46.0, 0.0, z - 0.1), -Vec3::X, Vec3::Z),
        &[
            [0.0, -5.6],
            [0.0, 5.6],
            [10.0, 5.4],
            [14.0, 3.4],
            [14.0, -3.4],
            [10.0, -5.4],
        ],
        0.45,
        [6.0, 0.0],
        0.3,
    );
    if !b.mid() {
        return;
    }
    b.mirror_y(|b| {
        bay(
            b,
            &Frame::new(v3(TRANSOM - 0.02, 3.0, 0.2), Vec3::Z, -Vec3::X),
            1.6,
            1.8,
        );
        if fine {
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3(TRANSOM + 0.2, 5.4, 2.4), v3(0.0, 0.3, -1.0), -Vec3::X),
                &swept(2.8, 0.9, 0.4, 0.5),
                0.22,
            );
        }
    });
    inlay(
        b,
        &[
            v3(TRANSOM - 0.04, -1.2, 1.6),
            v3(TRANSOM - 0.04, 1.2, 1.6),
            v3(TRANSOM - 0.04, 1.2, 2.2),
            v3(TRANSOM - 0.04, -1.2, 2.2),
        ],
        -Vec3::X,
        pattern::EMBER_SLOT,
    );
}

/// The whaleback's deck: plates lapped back either side of the superstructure and on the
/// forecastle, machinery bays between the forward houses and either side of the aft one,
/// lockers.
fn decks(b: &mut MeshBuilder, h: &[Station]) {
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        dark_plate(b);
        let z = top_at(h, 12.0, 6.2) - 0.15;
        Course {
            count: 3,
            step: 13.0,
            len: 14.0,
            half: 1.0,
            tip: 0.5,
            thick: 0.25,
            tail: 3.0,
        }
        .lay(
            b,
            &Frame::new(v3(12.0, 6.2, z), v3(-1.0, 0.04, -0.005), v3(0.0, 0.3, 1.0)),
        );
        let z = deck(h, 54.0) - 0.2;
        armour(
            b,
            &Frame::new(v3(55.0, 0.9, z), v3(-1.0, 0.16, -0.12), v3(0.0, 0.2, 1.0)),
            &swept(10.0, 0.9, 0.5, 0.5),
            0.25,
        );
        for (x, y) in [(-44.0f32, 4.4f32), (-55.0, 3.0), (30.0, 3.4), (47.0, 1.6)] {
            let z = top_at(h, x, y) - 0.05;
            locker(
                b,
                &Frame::new(v3(x, y, z), -Vec3::X, v3(0.0, y * 0.03, 1.0)),
                v3(2.4, 1.4, 0.5),
            );
        }
        for (x, y, len) in [
            (29.5f32, 2.6f32, 3.0f32),
            (-27.0, 6.8, 5.0),
            (-43.5, 6.2, 4.0),
        ] {
            let z = top_at(h, x, y) - 0.05;
            bay(
                b,
                &Frame::new(v3(x, y, z), -Vec3::X, v3(0.0, y * 0.03, 1.0)),
                len,
                0.7,
            );
        }
    });
}
