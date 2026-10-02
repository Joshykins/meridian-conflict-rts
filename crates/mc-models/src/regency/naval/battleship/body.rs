//! The Flamberge's body: the Claymore grown to a capital ship. A broad arrowhead, its
//! whaleback deck rolling down into low sides, the beam widest aft where a course of plates
//! along each deck edge runs out past the stern into the quarters' points; one long low
//! fin down the middle, raked back, carrying the flak on its crown and the counter-seekers
//! on swept wings; a bronze trench of machinery along each side.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::{armour, red_slot, swept, Course, Frame};
use super::guns::{counter_seeker, flak, main_gun, secondary};
use super::hull::{
    body_x, deck, edge, flank, hull, mirrored, st, team_patch, top_at, tube_doors, Station, RIDGE,
};
use super::{BOW_FACE, DEFENCE, MAIN, SECONDARY};

const TRANSOM: f32 = -60.0;

const HULL: [Station; 10] = [
    st(TRANSOM, -1.6, [12.0, -0.5], [11.4, 2.7], 4.7),
    st(-50.0, -3.4, [13.6, -1.0], [12.8, 2.9], 5.2),
    st(-34.0, -4.1, [13.2, -1.4], [12.3, 3.0], 5.5),
    st(-14.0, -4.3, [12.2, -1.6], [11.2, 3.1], 5.6),
    st(6.0, -4.3, [10.8, -1.7], [9.8, 3.2], 5.6),
    st(22.0, -4.2, [8.8, -1.8], [7.8, 3.2], 5.4),
    st(38.0, -4.1, [6.4, -2.0], [5.6, 3.0], 4.9),
    st(50.0, -4.0, [4.0, -2.3], [3.2, 2.4], 3.6),
    st(57.0, -4.1, [3.0, -2.8], [2.4, 0.9], 1.2),
    st(BOW_FACE, -4.2, [2.5, -3.4], [2.1, -1.0], -0.6),
];

/// The fin's plan about [`FIN_X`]: a long pointed nose, a pointed tail.
const FIN: [[f32; 2]; 5] = [
    [21.0, 0.0],
    [14.0, 2.0],
    [2.0, 3.6],
    [-12.0, 3.6],
    [-21.0, 0.0],
];
const FIN_X: f32 = -8.0;
/// The fin's foot and crown.
const FIN_FOOT: f32 = 5.3;
const FIN_TOP: f32 = 13.9;

/// The fin's section at height `z`: drawn in and moved aft as it rises, so its leading
/// edge rakes back and its crown sits over the flak.
fn fin_section(z: f32, inset: f32) -> Section {
    let t = (z - FIN_FOOT) / (FIN_TOP - FIN_FOOT);
    Section::scaled(z, (1.0 - 0.54 * t) * inset, (1.0 - 0.58 * t) * inset).shifted(2.0 * t, 0.0)
}

pub(super) fn build(b: &mut MeshBuilder, _tech: u8) {
    hull(b, &HULL, &[0, 4, 9]);
    fin(b);
    for (i, p) in MAIN.iter().enumerate() {
        main_gun(b, i, deck(&HULL, p.x));
    }
    flak(b, FIN_TOP);
    for (k, p) in SECONDARY.iter().enumerate() {
        secondary(b, k, top_at(&HULL, p.x, p.y));
    }
    team_patch(b, -54.0, -48.0, 2.0, deck(&HULL, -51.0) + 0.06);
    if b.coarse() {
        return;
    }
    tube_doors(b);
    for at in DEFENCE {
        let foot = if at.x > -10.0 { 12.45 } else { 9.85 };
        counter_seeker(b, at, foot);
    }
    sides(b);
    decks(b);
}

/// The long low fin, a bronze band round it, a crest plate swept back off its crown past
/// its tail into a spike, swept wings carrying the counter-seekers, and a plated spine
/// from the second house's barbette to its nose.
fn fin(b: &mut MeshBuilder) {
    let plan = mirrored(&FIN);
    let z0 = FIN_FOOT;
    if b.coarse() {
        dark_plate(b);
        b.frustum_open(
            v3(FIN_X, 0.0, z0),
            Vec2::new(40.0, 7.0),
            Vec2::new(19.0, 3.0),
            FIN_TOP - z0,
            Vec2::new(2.0, 0.0),
        );
        return;
    }
    let band = 11.0;
    b.at(v3(FIN_X, 0.0, 0.0), |b| {
        dark_plate(b);
        b.loft_z(
            &plan,
            &[
                Section::new(z0 - 0.4, 1.0),
                fin_section(z0, 1.0),
                fin_section(band - 0.2, 1.0),
            ],
        );
        metal(b);
        b.loft_z(
            &plan,
            &[
                fin_section(band - 0.2, 0.97),
                fin_section(band + 0.25, 0.97),
            ],
        );
        dark_plate(b);
        b.loft_z(
            &plan,
            &[
                fin_section(band + 0.25, 1.0),
                fin_section(FIN_TOP - 0.4, 1.0),
                fin_section(FIN_TOP, 0.93),
            ],
        );
    });
    // The crest plate, off the crown's tail past the fin's back into its spike.
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(-12.0, 0.0, FIN_TOP - 0.2), v3(-1.0, 0.0, -0.3), Vec3::Z),
        &swept(14.0, 1.2, 0.0, 0.5),
        0.3,
    );
    b.mirror_y(|b| {
        // The wings under the counter-seekers, swept out and back.
        dark_plate(b);
        armour(
            b,
            &Frame::new(v3(1.5, 1.4, 12.2), v3(-0.45, 1.0, 0.0), Vec3::Z),
            &swept(4.8, 0.9, 0.0, 0.55),
            0.25,
        );
        armour(
            b,
            &Frame::new(v3(-17.0, 1.6, 9.6), v3(-0.5, 1.0, 0.0), Vec3::Z),
            &swept(4.4, 0.9, 0.0, 0.55),
            0.25,
        );
        // Plates lapped back down the fin's flank under the band.
        if !b.fine() {
            return;
        }
        Course {
            count: 3,
            step: 9.0,
            len: 10.0,
            half: 0.9,
            tip: -0.4,
            thick: 0.25,
            tail: 3.0,
        }
        .lay(
            b,
            &Frame::new(v3(9.0, 2.3, 8.2), v3(-1.0, 0.06, 0.0), v3(0.0, 1.0, 0.3)),
        );
        if b.fine() {
            red_slot(
                b,
                v3(4.4, 1.0, 12.7),
                v3(0.85, 0.5, 0.2),
                v3(-0.5, 0.85, 0.0),
                1.6,
                0.22,
            );
        }
    });
    // The spine: a plated ridge from the second barbette's back to the fin's nose.
    let zs = deck(&HULL, 15.0);
    dark_plate(b);
    body_x(
        b,
        0.0,
        &[[16.5, 3.0, 2.4, zs + 0.6], [11.0, 3.6, 3.6, zs + 1.1]],
        &RIDGE,
    );
}

/// The deck edges: a course of plates along each, lapped back and running out past the
/// stern into the quarters' points; the trench of bronze machinery in their shadow; bow
/// planes forward.
fn sides(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        let at = |x: f32| {
            let (y, z) = edge(&HULL, x);
            v3(x, y, z)
        };
        dark_plate(b);
        Course {
            count: 3,
            step: 26.0,
            len: 28.0,
            half: 1.2,
            tip: 0.6,
            thick: 0.32,
            tail: 0.0,
        }
        .lay(
            b,
            &Frame::new(
                at(46.0) + v3(0.0, -0.6, 0.25),
                v3(-1.0, 0.085, -0.012),
                v3(0.0, 0.4, 1.0),
            ),
        );
        // The quarter: a broad plate over the deck edge aft, run out past the stern into
        // its point.
        armour(
            b,
            &Frame::new(
                at(-34.0) + v3(0.0, -1.6, 0.3),
                v3(-1.0, 0.06, -0.03),
                v3(0.0, 0.4, 1.0),
            ),
            &[
                [0.0, 1.6],
                [0.0, -1.2],
                [26.0, -1.6],
                [34.0, -1.2],
                [24.0, 1.4],
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
        // Bow planes: swept plates off the flanks forward, as a submarine's.
        let (y, n) = flank(&HULL, 46.0, 1.8);
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

/// The whaleback's deck: plates lapped back either side of the fin and on the
/// forecastle, hatches, a stern light.
fn decks(b: &mut MeshBuilder) {
    let fine = b.fine();
    if !fine {
        return;
    }
    b.mirror_y(|b| {
        dark_plate(b);
        let z = top_at(&HULL, 12.0, 5.0) - 0.15;
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
            &Frame::new(v3(12.0, 5.0, z), v3(-1.0, 0.04, -0.005), v3(0.0, 0.3, 1.0)),
        );
        let z = deck(&HULL, 54.0) - 0.2;
        armour(
            b,
            &Frame::new(v3(55.0, 0.6, z), v3(-1.0, 0.16, -0.12), v3(0.0, 0.2, 1.0)),
            &swept(10.0, 0.9, 0.5, 0.5),
            0.25,
        );
        if fine {
            metal(b);
            for (x, y) in [(-44.0f32, 3.0f32), (-57.0, 2.5), (28.0, 2.6), (47.0, 1.2)] {
                let z = top_at(&HULL, x, y) - 0.1;
                b.prism(v3(x, y, z), 6, 0.6, 0.5, 0.28);
            }
        }
    });
    red_slot(
        b,
        v3(TRANSOM - 0.02, 0.0, 2.2),
        -Vec3::X,
        Vec3::Y,
        6.0,
        0.14,
    );
}
