//! The Colossus's hull, one swept form from stern to prow: cross-sections lofted along x, a
//! keeled belly, flanks widest below the shoulder, a deck with a low ridge, narrowing aft to
//! a point and forward to the prow the lance grows out of. Lames of plate lap over it, each
//! band a little proud of the one behind; a pod of plate bulges from each front flank round a
//! chin gun's turntable; a dorsal fin rises off the stern to the point the hull field is
//! thrown from. The whole hull turns on the crown.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::super::commander::form::OCT;
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::red_slot;
use super::{DECK, LANCE_PIVOT, RACE};

/// The tip of the dorsal fin, where the hull field is thrown from.
pub(super) const EMITTER: Vec3 = Vec3::new(-22.6, 0.0, 55.2);

/// A hull cross-section: half width, belly height, deck height and the ridge's height over
/// the deck, at `x`.
#[derive(Clone, Copy)]
struct Station {
    x: f32,
    w: f32,
    belly: f32,
    deck: f32,
    ridge: f32,
}

const fn st(x: f32, w: f32, belly: f32, deck: f32, ridge: f32) -> Station {
    Station {
        x,
        w,
        belly,
        deck,
        ridge,
    }
}

/// The hull's stations, prow first.
const HULL: [Station; 9] = [
    st(18.4, 3.6, 41.6, 46.4, 0.2),
    st(14.0, 6.4, 40.4, 47.2, 0.5),
    st(8.0, 8.4, 39.7, 47.5, 0.6),
    st(0.0, 9.2, 39.5, DECK, 0.4),
    st(-7.0, 8.7, 39.7, DECK, 0.3),
    st(-13.0, 7.0, 40.2, 46.6, 0.3),
    st(-17.5, 4.4, 41.0, 45.8, 0.2),
    st(-20.5, 1.8, 41.9, 45.0, 0.1),
    st(-22.0, 0.5, 42.6, 44.4, 0.0),
];

/// The hull's section at `x`, interpolated between stations.
fn station(x: f32) -> Station {
    for p in HULL.windows(2) {
        if x <= p[0].x && x >= p[1].x {
            let t = (p[0].x - x) / (p[0].x - p[1].x);
            let l = |a: f32, b: f32| a + (b - a) * t;
            return st(
                x,
                l(p[0].w, p[1].w),
                l(p[0].belly, p[1].belly),
                l(p[0].deck, p[1].deck),
                l(p[0].ridge, p[1].ridge),
            );
        }
    }
    if x > HULL[0].x {
        HULL[0]
    } else {
        HULL[HULL.len() - 1]
    }
}

/// How far out the hull's flank stands at `x`.
pub(super) fn flank(x: f32) -> f32 {
    station(x).w
}

/// The hull's section as a ring at `x`; `grow` swells the flank and belly (a lame lapped over
/// the hull) without lifting the deck.
fn hull_ring(x: f32, grow: f32) -> Vec<Vec3> {
    let s = station(x);
    let (w, h) = (s.w * grow, s.deck - s.belly);
    let (zb, zt) = (s.belly - (grow - 1.0) * h, s.deck);
    let pts: [(f32, f32); 11] = [
        (-0.38 * w, zb),
        (0.38 * w, zb),
        (0.92 * w, zb + 0.3 * h),
        (w, zb + 0.62 * h),
        (0.9 * w, zt - 0.14 * h),
        (0.72 * w, zt),
        (0.0, zt + s.ridge),
        (-0.72 * w, zt),
        (-0.9 * w, zt - 0.14 * h),
        (-w, zb + 0.62 * h),
        (-0.92 * w, zb + 0.3 * h),
    ];
    pts.iter().map(|&(y, z)| v3(x, y, z)).collect()
}

/// The shell: the hull's own loft, the lames over it, the pods, the fin.
pub(super) fn carapace(b: &mut MeshBuilder) {
    let fine = b.fine();
    dark_plate(b);
    let rings: Vec<Vec<Vec3>> = HULL.iter().map(|s| hull_ring(s.x, 1.0)).collect();
    b.with_facets(|b| b.loft(&rings, true, true));
    // Lames: a band of plate round the belly and flanks, its leading edge stepping up off
    // the hull and easing back down to it, each one lapping over the next aft of it.
    let bands: &[f32] = if fine {
        &[15.5, 10.0, 4.5, -1.0, -6.5, -12.0, -17.0]
    } else {
        &[12.0, 2.0, -9.0]
    };
    for &x in bands {
        let len = if fine { 6.4 } else { 11.0 };
        let rings = [
            hull_ring(x, 1.0),
            hull_ring(x - 0.5, 1.05),
            hull_ring(x - len * 0.55, 1.04),
            hull_ring(x - len, 1.0),
        ];
        dark_plate(b);
        b.with_facets(|b| b.loft(&rings, true, true));
    }
    if fine {
        // A seam all along the shoulder, red optics either side of the prow and lines of heat
        // down the flanks.
        b.mirror_y(|b| {
            red_slot(
                b,
                v3(16.0, flank(16.0) * 0.96 + 0.15, 44.0),
                v3(0.5, 0.86, 0.0),
                v3(-0.86, 0.5, 0.0),
                2.4,
                0.36,
            );
            red_slot(
                b,
                v3(-2.0, flank(-2.0) * 1.04 + 0.1, 39.4),
                Vec3::Y,
                Vec3::X,
                9.0,
                0.3,
            );
        });
        seam(b);
    }
    pods(b);
    fin(b);
}

/// A pod of plate off each front flank, bulging from the hull's shoulder to round the chin
/// gun's turntable: an octagonal loft growing from the flank, swept forward to a blunt nose.
fn pods(b: &mut MeshBuilder) {
    let stations: [(f32, f32, f32, f32); 5] = [
        (3.0, 7.9, 1.2, 1.4),
        (8.0, 8.2, 2.3, 2.5),
        (13.0, 8.5, 2.8, 2.8),
        (17.0, 8.6, 2.8, 2.7),
        (19.2, 8.6, 1.9, 1.9),
    ];
    b.mirror_y(|b| {
        dark_plate(b);
        let rings: Vec<Vec<Vec3>> = stations
            .iter()
            .map(|&(x, y, w, h)| {
                OCT.iter()
                    .map(|&[s, t]| v3(x, y + s * w, 44.9 + t * h))
                    .collect()
            })
            .collect();
        b.with_facets(|b| b.loft(&rings, true, true));
        if b.fine() {
            seam(b);
            let rings: Vec<Vec<Vec3>> = [(12.6, 2.86), (13.1, 2.86)]
                .iter()
                .map(|&(x, r)| {
                    OCT.iter()
                        .map(|&[s, t]| v3(x, 8.5 + s * r, 44.9 + t * r))
                        .collect()
                })
                .collect();
            b.with_facets(|b| b.loft(&rings, true, true));
        }
    });
}

/// The dorsal fin: a thin ridge off the stern's deck that grows into a tall swept blade, its
/// point raked back over the tail, the hull field's emitter at its tip; a bronze rib runs up
/// its front edge.
fn fin(b: &mut MeshBuilder) {
    dark_plate(b);
    let sections: [(f32, f32, f32); 5] = [
        // (x, base z, height)
        (-14.4, 46.6, 0.9),
        (-16.4, 46.1, 4.2),
        (-18.6, 45.6, 7.4),
        (-20.8, 45.1, 9.6),
        (-22.6, 44.8, 10.4),
    ];
    let rings: Vec<Vec<Vec3>> = sections
        .iter()
        .map(|&(x, z, h)| {
            vec![
                v3(x, 0.0, z + h),
                v3(x, 0.55, z + h * 0.45),
                v3(x, 0.7, z),
                v3(x, -0.7, z),
                v3(x, -0.55, z + h * 0.45),
            ]
        })
        .collect();
    b.with_facets(|b| b.loft(&rings, true, true));
    if b.fine() {
        metal(b);
        b.cylinder_between(
            v3(-14.6, 0.0, 47.4),
            v3(EMITTER.x + 0.5, 0.0, EMITTER.z - 0.7),
            0.3,
            0.2,
            b.sides(6),
        );
    }
}

/// Far off: the hull a wedge with the team colour on it, the lance's jaw halves two bars.
pub(super) fn coarse(b: &mut MeshBuilder) {
    use glam::Vec2;
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        let top = DECK + 3.0;
        b.loft_z(
            &[[18.0, 0.0], [-20.0, 9.0], [-20.0, -9.0]],
            &[
                crate::builder::Section::new(RACE, 0.8),
                crate::builder::Section::new(top, 0.7),
            ],
        );
        b.paint(TEAM);
        b.face(&[
            v3(0.0, 0.0, top + 0.02),
            v3(-7.0, 2.4, top + 0.02),
            v3(-7.0, -2.4, top + 0.02),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            b.mirror_y(|b| {
                b.beam(
                    LANCE_PIVOT + v3(8.0, 3.0, 0.0),
                    LANCE_PIVOT + v3(26.0, 2.0, 0.0),
                    Vec2::new(3.0, 8.0),
                    Vec2::new(2.4, 4.0),
                );
            });
        });
    });
}
