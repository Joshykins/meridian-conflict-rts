//! The Slipway, the Regency naval factory, on its 8 x 8 lot (96 m square): a floating dock.
//!
//! It stands on the water (its origin is the surface, and nothing reaches under it). The
//! ship is made in the slip at the lot origin and leaves toward +x between two armoured
//! dock arms whose bows point out that way. From above it is a pair of blades either side
//! of a slip, a gantry across it, a turntable at its head.
//!
//! - Each arm is a pontoon under rows of plates either side of a ridge, lapped back
//!   like feathers into spikes past its stern, a bronze winch drum across it under the
//!   gantry. Its inner face, over the slip, is open machinery: bronze frames, mooring
//!   blocks, a lit seam. Its bow carries red optics and a violet
//!   edge where the ship leaves.
//! - Across the slip a gantry on bronze trestles standing in a winch bay open across each
//!   arm, two plated crabs riding its bridge, the fabricator heads hung under it aimed
//!   at the work (their mounts are `mc_core::print_heads`, where the nanite
//!   streams pour from). The violet runs hot while the dock builds.
//! - At the slip's head a caisson block, a toothed turntable turning on its roof
//!   (`part::SPINNER`).
//! - The owner's colour runs along each arm's ridge.
//!
//! Tech 2 (the Slipway II, for the 54 m tech 2 hulls, which fit the 24 m slip with room
//! to spare) adds a second, taller gantry aft of the first, over the hull's after half:
//! two plated crabs riding its bridge, a fabricator head hung under each, and an outboard
//! machine bank along each arm's outside.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;

use super::kit::{cable, dark_plate, metal, seam, v3};
use super::machine::*;

/// The arms: inner face, ridge and outer side (y), bow and stern (x).
const INNER: f32 = 12.0;
const RIDGE_Y: f32 = 20.0;
const OUTER: f32 = 28.5;
const BOW: f32 = 44.0;
const STERN: f32 = -34.0;
/// The arms' height: at the ridge, at the sides, and the deck the plates ride on.
const RIDGE_Z: f32 = 8.0;
const SIDE_Z: f32 = 3.2;
const THICK: f32 = 0.85;
/// The gantry's bridge across the slip.
const BRIDGE_X: f32 = 2.0;
const BRIDGE_Z: f32 = 16.5;
/// The winch bay open across each arm under the gantry (x, bow side and stern side).
const WINCH_BAY: [f32; 2] = [BRIDGE_X + 6.5, BRIDGE_X - 6.5];
/// The caisson at the slip's head, and the turntable on it (the spinner's pivot).
const CAISSON: [f32; 2] = [-42.0, -30.0];
const CAISSON_Z: f32 = 6.0;
const TURNTABLE: Vec3 = Vec3::new(-36.0, 0.0, 6.6);

/// The arm's deck height at `y`.
fn deck(y: f32) -> f32 {
    let t = if y < RIDGE_Y {
        (y - INNER) / (RIDGE_Y - INNER)
    } else {
        (OUTER - y) / (OUTER - RIDGE_Y)
    };
    SIDE_Z + (RIDGE_Z - SIDE_Z) * t.clamp(0.0, 1.0)
}

/// Where the arm's bow has swept back to at `y`: the point is on the ridge.
fn bow_x(y: f32) -> f32 {
    BOW - (y - RIDGE_Y).abs() * 0.85
}

/// The tech 2 heads' station along the slip, and the second gantry's bridge height.
const AFT_X: f32 = -18.0;
const AFT_Z: f32 = 20.6;

pub(super) fn tidebrood(b: &mut MeshBuilder, tech: u8) {
    b.set_spinner_pivot(TURNTABLE);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    b.mirror_y(|b| {
        arm(b);
        gallery(b);
        bow(b);
        trestle(b);
    });
    bridge(b);
    caisson(b);
    tier(b, tech, 2, 0.2, |b| {
        b.mirror_y(|b| {
            aft_trestle(b);
            outboard(b);
        });
        aft_bridge(b);
    });
}

/// Far off: the two arms, the bridge, the caisson, the owner's colour along the ridges.
/// From tech 2 the bridge stands as one block over both its own station and the tech
/// 2 heads', as high as what carries them.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    b.mirror_y(|b| {
        dark_plate(b);
        let ring = |z: f32, inset: f32| -> Vec<Vec3> {
            vec![
                v3(bow_x(RIDGE_Y) - inset, RIDGE_Y, z),
                v3(bow_x(OUTER) - inset, OUTER - inset, z),
                v3(STERN - 4.0 + inset, OUTER - inset, z),
                v3(STERN - 4.0 + inset, INNER + inset, z),
                v3(bow_x(INNER) - inset, INNER + inset, z),
            ]
        };
        b.loft(&[ring(0.0, 0.0), ring(RIDGE_Z + 0.8, 5.0)], false, true);
        b.paint(TEAM);
        b.face(&[
            v3(-10.0, RIDGE_Y - 1.2, RIDGE_Z + 0.85),
            v3(20.0, RIDGE_Y - 1.2, RIDGE_Z + 0.85),
            v3(20.0, RIDGE_Y + 1.2, RIDGE_Z + 0.85),
            v3(-10.0, RIDGE_Y + 1.2, RIDGE_Z + 0.85),
        ]);
    });
    dark_plate(b);
    if tech >= 2 {
        b.block(
            v3(AFT_X - 2.0, -(OUTER - 4.0), BRIDGE_Z - 1.2),
            v3(BRIDGE_X + 2.0, OUTER - 4.0, AFT_Z + 1.2),
        );
    } else {
        b.cuboid(v3(BRIDGE_X, 0.0, BRIDGE_Z), v3(4.0, OUTER * 2.0 - 8.0, 2.4));
    }
    b.frustum_open(
        v3((CAISSON[0] + CAISSON[1]) * 0.5, 0.0, 0.0),
        Vec2::new(CAISSON[1] - CAISSON[0], INNER * 2.0 + 2.0),
        Vec2::new(CAISSON[1] - CAISSON[0] - 3.0, INNER * 2.0 - 2.0),
        CAISSON_Z,
        Vec2::ZERO,
    );
}

/// One arm (+y): the pontoon, its bronze deck, two rows of plates lapped back either side
/// of the ridge, the drive shaft along the ridge and the owner's colour on it.
fn arm(b: &mut MeshBuilder) {
    let fine = b.fine();
    // The pontoon: a hull swept to a point at the bow, square at the stern.
    let hull = |z: f32, inset: f32| -> Vec<Vec3> {
        vec![
            v3(bow_x(RIDGE_Y) - inset, RIDGE_Y, z),
            v3(bow_x(OUTER) - inset, OUTER - inset, z),
            v3(STERN + inset, OUTER - inset, z),
            v3(STERN + inset, INNER + inset, z),
            v3(bow_x(INNER) - inset, INNER + inset, z),
        ]
    };
    seam(b);
    b.loft(&[hull(0.0, 0.0), hull(SIDE_Z - 0.6, 0.0)], false, true);
    // The bronze deck the plates ride on, peaked at the ridge.
    let section = [
        (INNER + 0.6, SIDE_Z - 0.6),
        (RIDGE_Y, RIDGE_Z - 0.9),
        (OUTER - 0.6, SIDE_Z - 0.6),
    ];
    let deck_ring = |x0: f32, dx: f32| -> Vec<Vec3> {
        section
            .iter()
            .map(|&(y, z)| v3(if dx > 0.0 { bow_x(y) - 4.0 } else { x0 }, y, z))
            .collect()
    };
    metal(b);
    b.loft(
        &[deck_ring(STERN + 0.5, -1.0), deck_ring(0.0, 1.0)],
        true,
        true,
    );
    // Rows of plates, one inside the ridge and two outside, each lapped back in two courses: from near
    // the bow to the winch bay under the gantry, and from the bay to past the stern.
    for (y0, y1) in [
        (RIDGE_Y + 0.4, INNER - 0.8),
        (RIDGE_Y + 0.4, RIDGE_Y + 4.6),
        (RIDGE_Y + 5.4, OUTER + 1.2),
    ] {
        let (z0, z1) = (deck(y0) + 0.3, deck(y1.clamp(INNER, OUTER)) - 0.2);
        let y = (y0 + y1) * 0.5;
        let across = v3(0.0, y1 - y0, z1 - z0);
        let normal = v3(0.0, -across.z, across.y) * across.y.signum();
        let courses = [
            (bow_x(y) - 2.0, WINCH_BAY[0], 3, 3.0),
            (WINCH_BAY[1], STERN + 3.0, 4, 5.0),
        ];
        let mut plates = Vec::new();
        for (from, to, count, tail) in courses {
            let len = (from - to) * 0.45;
            let f = Frame::new(v3(from, y, (z0 + z1) * 0.5), -Vec3::X, normal);
            dark_plate(b);
            plates.extend(
                Course {
                    count,
                    step: (from - to - len) / (count - 1) as f32,
                    len,
                    half: across.length() * 0.5,
                    tip: if y1 > y0 { -1.0 } else { 1.0 },
                    thick: THICK,
                    tail,
                }
                .lay(b, &f),
            );
        }
        if fine {
            metal(b);
            for &(g, long) in &plates {
                let top = g.at(long * 0.3, 0.0, 0.0);
                b.cylinder_between(top, top - g.n * 1.0, 0.45, 0.45, 4);
            }
        }
    }
    // The winch in the bay: a drum across the arm, its bearings, the gantry's
    // trestle standing on them.
    shaft(
        b,
        v3(BRIDGE_X, INNER + 1.0, RIDGE_Z - 1.6),
        v3(BRIDGE_X, OUTER - 1.0, RIDGE_Z - 1.6),
        2.0,
    );
    for y in [INNER + 1.5, OUTER - 1.5] {
        dark_plate(b);
        b.block(
            v3(BRIDGE_X - 2.4, y - 1.0, SIDE_Z - 0.6),
            v3(BRIDGE_X + 2.4, y + 1.0, RIDGE_Z - 1.0),
        );
    }
    // The owner's colour along the ridge.
    b.paint(TEAM);
    b.face(&[
        v3(-12.0, RIDGE_Y - 1.6, RIDGE_Z + 1.6),
        v3(24.0, RIDGE_Y - 1.6, RIDGE_Z + 1.6),
        v3(24.0, RIDGE_Y - 0.8, RIDGE_Z + 1.6),
        v3(-12.0, RIDGE_Y - 0.8, RIDGE_Z + 1.6),
    ]);
}

/// An arm's inner face over the slip (+y side): bronze frames, mooring blocks, a lit
/// seam.
fn gallery(b: &mut MeshBuilder) {
    let face = INNER - 0.3;
    let frames = [-26.0, -12.0, 12.0, 26.0];
    for &x in &frames {
        shaft(b, v3(x, face, 0.4), v3(x, face, SIDE_Z + 2.2), 0.6);
    }
    for x in [-19.0, 19.0, 33.0] {
        dark_plate(b);
        b.block(
            v3(x - 1.4, face - 1.6, SIDE_Z + 0.6),
            v3(x + 1.4, face + 1.0, SIDE_Z + 1.6),
        );
    }
    red_slot(b, v3(0.0, face - 0.1, 1.6), -Vec3::Y, Vec3::X, 40.0, 0.2);
}

/// An arm's bow (+y side): red optics either side of the point, a violet edge down its
/// inner side where the ship leaves.
fn bow(b: &mut MeshBuilder) {
    let tip = v3(bow_x(RIDGE_Y), RIDGE_Y, SIDE_Z - 0.6);
    for side in [-1.0f32, 1.0] {
        let along = v3(-0.85, side, 0.0).normalize();
        let out = v3(1.0, side * 0.85, 0.0).normalize();
        red_slot(
            b,
            tip + along * 3.0 + out * 0.05 - Vec3::Z * 1.0,
            out,
            along,
            3.0,
            0.3,
        );
    }
    b.paint(GLOW_VIOLET);
    b.beam(
        v3(bow_x(INNER) - 0.2, INNER - 0.2, 0.6),
        v3(bow_x(INNER) - 0.2, INNER - 0.2, SIDE_Z + 3.0),
        Vec2::new(0.35, 0.5),
        Vec2::new(0.35, 0.5),
    );
}

/// The gantry's trestle on an arm (+y side): two bronze legs up to the bridge, braced by
/// a plated strut.
fn trestle(b: &mut MeshBuilder) {
    let top = v3(BRIDGE_X, RIDGE_Y - 2.0, BRIDGE_Z - 1.2);
    for dx in [-4.5f32, 4.5] {
        shaft(
            b,
            v3(BRIDGE_X + dx, RIDGE_Y - 1.0, RIDGE_Z - 0.4),
            top + v3(dx * 0.3, 0.0, 0.0),
            0.7,
        );
    }
    strut(
        b,
        v3(BRIDGE_X, OUTER - 3.0, SIDE_Z + 1.0),
        top + v3(0.0, 1.2, -1.5),
        0.55,
    );
}

/// The gantry's bridge across the slip, plated, with the fabricator heads hung under it.
fn bridge(b: &mut MeshBuilder) {
    dark_plate(b);
    b.beam(
        v3(BRIDGE_X, -(RIDGE_Y + 1.5), BRIDGE_Z),
        v3(BRIDGE_X, RIDGE_Y + 1.5, BRIDGE_Z),
        Vec2::new(4.2, 2.4),
        Vec2::new(4.2, 2.4),
    );
    // Its crabs: plated trolleys riding the bridge over the heads, bronze wheels.
    for y in [-7.75f32, 7.75] {
        dark_plate(b);
        b.block(
            v3(BRIDGE_X - 2.8, y - 2.4, BRIDGE_Z + 1.2),
            v3(BRIDGE_X + 2.8, y + 2.4, BRIDGE_Z + 2.8),
        );
        if b.fine() {
            for dx in [-2.0f32, 2.0] {
                collar(b, v3(BRIDGE_X + dx, y, BRIDGE_Z + 1.3), Vec3::Y, 0.8, 5.2);
            }
        }
        red_slot(
            b,
            v3(BRIDGE_X + 2.85, y, BRIDGE_Z + 2.0),
            Vec3::X,
            Vec3::Y,
            3.0,
            0.25,
        );
    }
    for (mount, s, aim) in tier_heads("regency_tidebrood", 1) {
        dark_plate(b);
        b.beam(
            v3(mount.x, mount.y, BRIDGE_Z - 1.0),
            mount + Vec3::Z * 0.3,
            Vec2::new(1.4, 1.2),
            Vec2::new(1.0, 0.9),
        );
        fabricator(b, mount, aim, s);
    }
}

/// The caisson across the slip's head: a plated block, the turntable turning on it.
fn caisson(b: &mut MeshBuilder) {
    let fine = b.fine();
    let [back, front] = CAISSON;
    seam(b);
    b.frustum_open(
        v3((back + front) * 0.5, 0.0, 0.0),
        Vec2::new(front - back, INNER * 2.0 + 2.0),
        Vec2::new(front - back - 2.0, INNER * 2.0),
        CAISSON_Z,
        Vec2::ZERO,
    );
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(front + 1.0, 7.5, CAISSON_Z + 0.3),
            -Vec3::X,
            v3(0.0, 0.25, 1.0),
        );
        dark_plate(b);
        Course {
            count: 2,
            step: 6.0,
            len: 8.0,
            half: 3.8,
            tip: -1.0,
            thick: THICK,
            tail: 3.0,
        }
        .lay(b, &f);
    });
    b.with_part(part::SPINNER, |b| {
        metal(b);
        b.prism(TURNTABLE - Vec3::Z * 0.6, b.sides(16), 3.6, 3.6, 1.0);
        if fine {
            teeth(b, TURNTABLE - Vec3::Z * 0.1, 3.4, 14, v3(0.7, 0.8, 0.9));
        }
        dark_plate(b);
        b.beam(
            TURNTABLE + v3(-2.6, 0.0, 0.5),
            TURNTABLE + v3(2.6, 0.0, 0.5),
            Vec2::new(1.4, 1.0),
            Vec2::new(1.4, 1.0),
        );
    });
}

/// Tech 2: a second gantry's trestle on an arm (+y side), taller than the
/// first, two bronze legs off a bearing block on the ridge and a plated strut from the
/// outer side.
fn aft_trestle(b: &mut MeshBuilder) {
    let top = v3(AFT_X, RIDGE_Y - 2.0, AFT_Z - 1.2);
    dark_plate(b);
    b.block(
        v3(AFT_X - 6.4, RIDGE_Y - 2.2, RIDGE_Z - 1.4),
        v3(AFT_X + 6.4, RIDGE_Y + 0.4, RIDGE_Z + 0.6),
    );
    for dx in [-5.0f32, 5.0] {
        shaft(
            b,
            v3(AFT_X + dx, RIDGE_Y - 1.0, RIDGE_Z + 0.4),
            top + v3(dx * 0.25, 0.0, 0.0),
            0.75,
        );
    }
    strut(
        b,
        v3(AFT_X, OUTER - 2.5, SIDE_Z + 0.8),
        top + v3(0.0, 1.2, -1.8),
        0.6,
    );
}

/// Tech 2: the second gantry's bridge across the slip, a crab riding it over each tech 2
/// head, the head hung under the crab on a plated drop.
fn aft_bridge(b: &mut MeshBuilder) {
    dark_plate(b);
    b.beam(
        v3(AFT_X, -(RIDGE_Y + 1.5), AFT_Z),
        v3(AFT_X, RIDGE_Y + 1.5, AFT_Z),
        Vec2::new(4.2, 2.4),
        Vec2::new(4.2, 2.4),
    );
    for (mount, s, aim) in tier_heads("regency_tidebrood", 2) {
        dark_plate(b);
        b.block(
            v3(AFT_X - 2.8, mount.y - 2.4, AFT_Z + 1.2),
            v3(AFT_X + 2.8, mount.y + 2.4, AFT_Z + 2.8),
        );
        if b.fine() {
            for dx in [-2.0f32, 2.0] {
                collar(b, v3(AFT_X + dx, mount.y, AFT_Z + 1.3), Vec3::Y, 0.8, 5.2);
            }
        }
        red_slot(
            b,
            v3(AFT_X - 2.85, mount.y, AFT_Z + 2.0),
            -Vec3::X,
            Vec3::Y,
            3.0,
            0.25,
        );
        dark_plate(b);
        b.beam(
            v3(mount.x, mount.y, AFT_Z - 1.0),
            mount + Vec3::Z * 0.3,
            Vec2::new(1.4, 1.2),
            Vec2::new(1.0, 0.9),
        );
        fabricator(b, mount, aim, s);
    }
}

/// Tech 2: an outboard machine bank along an arm's outside (+y side): a low
/// plated housing on its own pontoon, plates lapped back along its outer edge into a
/// spike, feed lines up the arm's side.
fn outboard(b: &mut MeshBuilder) {
    let (y0, y1, x0, x1, top) = (OUTER - 0.5, OUTER + 6.5, -30.0, 16.0, 4.6);
    seam(b);
    b.frustum_open(
        v3((x0 + x1) * 0.5, (y0 + y1) * 0.5, 0.0),
        Vec2::new(x1 - x0, y1 - y0),
        Vec2::new(x1 - x0 - 2.0, y1 - y0 - 1.6),
        top,
        Vec2::ZERO,
    );
    let f = Frame::new(
        v3(x1 + 1.0, y1 - 2.6, top + 0.4),
        -Vec3::X,
        v3(0.0, 0.35, 1.0),
    );
    dark_plate(b);
    Course {
        count: 4,
        step: 10.5,
        len: 13.0,
        half: 3.0,
        tip: -1.0,
        thick: THICK,
        tail: 4.0,
    }
    .lay(b, &f);
    red_slot(
        b,
        v3(x1 - 0.9, (y0 + y1) * 0.5, top * 0.5),
        Vec3::X,
        Vec3::Y,
        4.0,
        0.3,
    );
    if b.fine() {
        metal(b);
        for x in [-20.0, -4.0, 10.0] {
            cable(
                b,
                &[
                    v3(x, y0 + 1.6, top),
                    v3(x, OUTER - 0.2, top + 0.4),
                    v3(x, OUTER - 1.6, SIDE_Z + 0.6),
                ],
                0.3,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_tidebrood_fits_its_lot_at_both_tiers() {
        let key = "regency_tidebrood";
        for (tech, height) in [(1, 20.0), (2, 24.0)] {
            super::super::check_at(key, tech, 46.0, height, Some(8), &[]);
            super::super::check_heads_at(key, tech, 46.0, height);
        }
    }
}
