//! The Nimbus's body: a hard-chined hull, white over a graphite
//! keel, drawn out into a long pointed nose (it is a drone: no glazing); a graphite deck
//! stacked on its back carrying the two dorsal AA mounts; an engine sponson along each
//! flank under the wing root, its intake forward and two nozzles aft, a missile-defence
//! laser head on its back behind the wing ([`LASERS`]).

use super::super::super::naval::pd_laser;
use super::dispenser::face_toward;
use super::*;

/// Hull stations, nose to tail: x, then (half width, height) at the keel, the chine, the
/// shoulder and the spine.
const HULL: [[f32; 9]; 7] = [
    [48.0, 0.1, 6.9, 0.25, 7.0, 0.25, 7.3, 0.1, 7.4],
    [38.0, 1.3, 4.9, 2.6, 5.7, 2.6, 8.0, 1.1, 8.7],
    [26.0, 2.6, 3.0, 5.0, 4.6, 5.0, 8.8, 2.3, 10.1],
    [14.0, 3.2, 2.0, 6.5, 4.0, 6.5, 9.2, 3.0, 11.0],
    [-22.0, 3.2, 2.0, 6.5, 4.0, 6.5, 9.2, 3.0, 11.0],
    [-33.0, 2.4, 3.6, 4.6, 5.0, 4.6, 8.8, 2.2, 10.0],
    [-40.0, 1.8, 4.6, 3.4, 5.6, 3.4, 8.4, 1.6, 9.2],
];

/// The missile-defence laser heads' centres, where the beams leave: one on each
/// sponson behind the wing (the unit file's `anti_missile_mounts`).
pub(crate) const LASERS: [Vec3; 2] = [Vec3::new(-20.0, 8.6, 8.2), Vec3::new(-20.0, -8.6, 8.2)];
const LASER_SIZE: f32 = 0.8;

/// The deck's roof, where the dorsal houses stand.
pub(super) const DECK_TOP: f32 = 13.0;
const DECK_FOOT: f32 = 10.4;
/// The deck's plan at its foot.
const DECK: [[f32; 2]; 8] = [
    [14.0, -2.6],
    [14.0, 2.6],
    [11.0, 4.6],
    [-24.0, 4.6],
    [-27.5, 3.0],
    [-27.5, -3.0],
    [-24.0, -4.6],
    [11.0, -4.6],
];

/// The engine sponson's section about its centre line `SPONSON_Y`: (y off it, z).
const SPONSON: [[f32; 2]; 6] = [
    [-2.6, 2.4],
    [1.6, 2.2],
    [2.4, 3.6],
    [2.0, 6.6],
    [-2.6, 6.9],
    [-3.2, 4.4],
];
const SPONSON_Y: f32 = 8.6;
const SPONSON_FRONT: f32 = 12.0;
const SPONSON_BACK: f32 = -30.0;
const NOZZLES: [f32; 2] = [-1.0, 1.0];

pub(super) fn build(b: &mut MeshBuilder) {
    b.with_facets(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(&band(&HULL, 1, 3), true, true);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.loft(&band(&HULL, 0, 1), true, true);
        // The deck: a second layer of graphite plate stacked on the spine.
        b.loft_z(
            &DECK,
            &[
                Section::new(DECK_FOOT, 1.0),
                Section::scaled(DECK_TOP, 0.94, 0.88).shifted(-0.4, 0.0),
            ],
        );
    });
    b.mirror_y(sponson);
    b.mirror_y(layers);
    if b.fine() {
        // A strake along each chine and a light panel let into the deck's roof.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        for side in [1.0, -1.0] {
            b.beam(
                v3(22.0, 6.2 * side, 4.4),
                v3(-28.0, 6.2 * side, 4.4),
                v2(0.6, 0.5),
                v2(0.6, 0.5),
            );
        }
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.cuboid(v3(-5.5, 0.0, DECK_TOP + 0.02), v3(16.0, 5.4, 0.08));
    }
    for side in [1.0, -1.0] {
        for dy in NOZZLES {
            let at = v3(SPONSON_BACK, (SPONSON_Y + dy * 1.2) * side, 4.5);
            b.add_exhaust(at, -Vec3::X, 1.0);
        }
    }
    team_panel(b, v3(-5.5, 0.0, DECK_TOP + 0.1), v2(6.0, 3.0));
    for at in LASERS {
        pd_laser(b, at, LASER_SIZE, Some(6.6));
    }
}

/// The plates layered over the left flank: a root extension running from the nose's
/// shoulder out into the wing's leading edge, and an armour cheek along the shoulder.
fn layers(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.with_facets(|b| {
        // The root extension: a thin faceted plate, its outer edge swept into the wing.
        let plate = |z: f32| {
            vec![
                v3(31.0, 3.4, z),
                v3(16.0, 11.0, z),
                v3(8.0, 12.5, z),
                v3(4.0, 5.0, z),
            ]
        };
        b.loft(&[plate(7.0), plate(7.7)], true, true);
    });
    if !b.fine() {
        return;
    }
    // The armour cheek: a long slab stood off the shoulder, its ends cut back.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.with_facets(|b| {
        b.extrude_y(
            &[
                [20.0, 8.1],
                [-26.0, 8.1],
                [-30.0, 9.0],
                [-26.0, 9.9],
                [17.0, 9.9],
            ],
            6.35,
            6.95,
        );
    });
    // The bay fairing under the keel, and its door line.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.with_facets(|b| {
        b.extrude_y(
            &[[10.0, 2.2], [6.0, 0.9], [-16.0, 0.9], [-20.0, 2.2]],
            -0.01,
            3.0,
        );
    });
}

/// The left engine sponson: a faceted box along the flank, a dark intake in its face and
/// two nozzles in its tail.
fn sponson(b: &mut MeshBuilder) {
    let ring = |x: f32, k: f32| -> Vec<Vec3> {
        SPONSON
            .iter()
            .map(|&[dy, z]| v3(x, SPONSON_Y + dy * k, 4.5 + (z - 4.5) * k))
            .collect()
    };
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.with_facets(|b| {
        b.loft(
            &[
                ring(SPONSON_FRONT, 0.92),
                ring(SPONSON_FRONT - 4.0, 1.0),
                ring(SPONSON_BACK + 4.0, 1.0),
                ring(SPONSON_BACK, 0.8),
            ],
            true,
            true,
        );
    });
    if !b.fine() {
        return;
    }
    b.paint(ACCENT);
    let mouth: Vec<Vec3> = ring(SPONSON_FRONT + 0.03, 0.7);
    face_toward(b, &mouth, Vec3::X);
    let n = b.sides(8);
    for dy in NOZZLES {
        let c = v3(SPONSON_BACK - 0.03, SPONSON_Y + dy * 1.2, 4.5);
        let nozzle: Vec<Vec3> = (0..n)
            .map(|i| {
                let a = (i as f32 + 0.5) * std::f32::consts::TAU / n as f32;
                c + v3(0.0, a.cos(), a.sin()) * 1.0
            })
            .collect();
        face_toward(b, &nozzle, -Vec3::X);
    }
}

/// Far away: the hull as a box through three stations.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.loft(&band(&[HULL[0], HULL[3], HULL[6]], 1, 2), false, false);
}
