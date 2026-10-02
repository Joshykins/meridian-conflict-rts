//! The Barbican as a launch block and a mast (design A): the four cells sunk in a long
//! faceted block of sloped plate under two leaves of lapped plate on bronze rails. On -x
//! the tracking sensor, an Orrery's kin: a bronze mast braced by three plated legs, its
//! sleeves of lapped plate, and over it three sensor rings floating round a bronze
//! spindle, each leaning its own way, turning together, red slits in their carriages and
//! red optics on the cap. On +x the magazine, its well under a gravity lift (a double
//! collar floating between three emitter posts), and a plated duct into the block.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::{chamfered_rect, MeshBuilder};
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::*;

const DECK: f32 = 0.8;
/// The block's roof, and its half size there and at its foot.
const TOP: f32 = 6.0;
const BLOCK: Vec2 = Vec2::new(6.6, 11.0);
const BLOCK_FOOT: Vec2 = Vec2::new(7.6, 11.8);
const CHAMFER: f32 = 3.0;
const LEAF_Z: f32 = TOP + 0.3;
const LEAF_X: f32 = 4.85;
const LEAF_Y: f32 = 4.7;
const RAIL_X: f32 = 5.6;
const RAIL_Y: f32 = 10.2;
/// The mast's foot, and the sensor rings' pivot over it.
const MAST: Vec2 = Vec2::new(-14.0, 0.0);
const SPIN: Vec3 = Vec3::new(-14.0, 0.0, 16.6);
/// Each ring: height, radius, the bearing it leans toward (degrees).
const RINGS: [(f32, f32, f32); 3] = [(14.6, 2.7, 0.0), (16.6, 3.3, 120.0), (18.6, 3.9, 240.0)];
const CAP: f32 = 20.2;
/// The magazine on +x, and its well.
const MAG: Vec2 = Vec2::new(14.0, -1.2);
const MAG_HALF: Vec2 = Vec2::new(3.8, 6.4);
const MAG_TOP: f32 = 4.4;
const HATCH: Vec2 = Vec2::new(14.0, 2.6);
const WELL: f32 = 0.95;

pub(in crate::regency) fn array_mast(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(SPIN);
    if b.coarse() {
        coarse(b);
        return;
    }
    plinth(b);
    block(b);
    cells(b, TOP);
    b.with_part(part::SILO_ROUND, |b| {
        for (sx, sy) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
            interceptor(b, Vec2::new(sx * CELL, sy * CELL), LEAF_Z - 1.0);
        }
    });
    b.with_part(part::SILO_DOOR, |b| b.mirror_y(mast_leaf));
    rails(b, RAIL_X, RAIL_Y, TOP);
    mast(b);
    magazine(b);
}

fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.face(&octagon(Vec2::ZERO, 18.4, DECK));
    let ring = |half: Vec2, z: f32| -> Vec<Vec3> {
        chamfered_rect(half, CHAMFER)
            .iter()
            .map(|p| v3(p[0], p[1], z))
            .collect()
    };
    dark_plate(b);
    b.loft(&[ring(BLOCK_FOOT, 0.0), ring(BLOCK, TOP)], false, true);
    b.with_part(part::SILO_DOOR, |b| b.mirror_y(mast_leaf));
    b.prism(MAST.extend(0.0), 3, 2.6, 0.8, CAP);
    team_tab(b, v3(0.0, 8.6, TOP + 0.02), Vec3::X, 1.0);
    b.cuboid_open(MAG.extend(MAG_TOP * 0.5), (MAG_HALF * 2.0).extend(MAG_TOP));
}

/// The plinth: a low octagon of dark plate and its seam.
fn plinth(b: &mut MeshBuilder) {
    let c = Vec2::ZERO;
    dark_plate(b);
    drum(b, c, 18.4, 17.8, 0.0, 0.5, false);
    deck_ring(b, c, 17.3, 17.8, 0.5);
    seam(b);
    drum(b, c, 17.3, 17.3, 0.5, 0.62, false);
    deck_ring(b, c, 16.9, 17.3, 0.62);
    dark_plate(b);
    drum(b, c, 16.9, 16.5, 0.62, DECK, true);
    for (x, y) in [(-8.5, 10.5), (-8.5, -10.5), (9.0, 11.0)] {
        team_tab(b, v3(x, y, DECK + 0.02), Vec3::X, 1.0);
    }
}

/// The launch block: sloped plate on a seam-dark footing, the roof round the cells, a red
/// line round its shoulder, plates lapped down its ends.
fn block(b: &mut MeshBuilder) {
    let ring = |half: Vec2, ch: f32, z: f32| -> Vec<Vec3> {
        chamfered_rect(half, ch)
            .iter()
            .map(|p| v3(p[0], p[1], z))
            .collect()
    };
    seam(b);
    b.chamfered_box(
        Vec3::Z * (DECK + 0.25),
        (BLOCK_FOOT * 2.0 + Vec2::splat(1.0)).extend(0.5),
        CHAMFER + 0.4,
    );
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                ring(BLOCK_FOOT, CHAMFER + 0.3, DECK),
                ring(BLOCK, CHAMFER, TOP),
            ],
            false,
            false,
        )
    });
    let outer: Vec<Vec2> = chamfered_rect(BLOCK, CHAMFER)
        .iter()
        .map(|p| Vec2::new(p[0], p[1]))
        .collect();
    cell_roof(b, TOP, &outer);
    if b.fine() {
        for sx in [-1.0f32, 1.0] {
            for y in [-4.0f32, 4.0] {
                red_slot(
                    b,
                    v3(sx * (BLOCK.x + 0.2), y, TOP - 0.7),
                    Vec3::X * sx,
                    Vec3::Y,
                    4.0,
                    0.1,
                );
            }
        }
        for sy in [-1.0f32, 1.0] {
            dark_plate(b);
            armour(
                b,
                &Frame::new(
                    v3(0.0, sy * (BLOCK.y - 1.4), TOP + 0.02),
                    v3(0.0, sy, -0.55),
                    v3(0.0, sy * 0.55, 1.0),
                ),
                &swept(3.4, 3.2, 0.0, 0.55),
                0.35,
            );
        }
    }
}

/// One leaf (+y): a chamfered plate, a second lapped over it, carriages over the rails.
fn mast_leaf(b: &mut MeshBuilder) {
    let (x, y) = (LEAF_X, LEAF_Y);
    let plan = [
        Vec2::new(-x, 0.02),
        Vec2::new(x, 0.02),
        Vec2::new(x, y - 1.0),
        Vec2::new(x - 1.0, y),
        Vec2::new(-x + 1.0, y),
        Vec2::new(-x, y - 1.0),
    ];
    leaf(b, &plan, LEAF_Z, 0.45, 1, 1.0);
    carriages(b, RAIL_X, 1.0, 3.6, TOP, LEAF_Z + 0.2);
}

/// The sensor mast: three plated legs braced to a bronze mast, its sleeves, the spindle,
/// the rings turning round it and the cap.
fn mast(b: &mut MeshBuilder) {
    let fine = b.fine();
    let c = MAST.extend(0.0);
    seam(b);
    b.prism(c + Vec3::Z * DECK, 8, 2.4, 2.0, 0.8);
    shaft(b, c + Vec3::Z * DECK, c + Vec3::Z * 13.6, 0.75);
    for k in 0..3 {
        let a = TAU * k as f32 / 3.0 + TAU / 6.0;
        let d = out(a);
        let root = c + d * 0.8 + Vec3::Z * 8.0;
        let foot = c + d * 5.6 + Vec3::Z * DECK;
        shaft(b, root, foot + Vec3::Z * 0.8, 0.32);
        let down = (foot - root).normalize();
        let n = (d - down * d.dot(down)).normalize();
        dark_plate(b);
        Course {
            count: if fine { 3 } else { 2 },
            step: 1.9,
            len: 2.8,
            half: 0.75,
            tip: 0.0,
            thick: 0.26,
            tail: 0.9,
        }
        .lay(b, &Frame::new(root + n * 0.45, down, n));
        dark_plate(b);
        b.yawed(foot, a, |b| {
            b.block(v3(-1.2, -0.9, -DECK), v3(1.2, 0.9, 0.9))
        });
        if fine {
            red_slot(
                b,
                foot + d * 1.21 + Vec3::Z * 0.4,
                d,
                d.cross(Vec3::Z),
                1.0,
                0.1,
            );
        }
    }
    // The sleeves: plates lapped down round the mast.
    for (k, &z) in [6.0f32, 9.6, 13.0].iter().enumerate() {
        let spread = 1.3 - 0.12 * k as f32;
        for q in 0..4 {
            let d = out(TAU * (q as f32 + 0.5) / 4.0);
            dark_plate(b);
            Course {
                count: if fine { 2 } else { 1 },
                step: 1.1,
                len: 2.0,
                half: 0.7,
                tip: 0.0,
                thick: 0.24,
                tail: 0.7,
            }
            .lay(
                b,
                &Frame::new(
                    c + d * (0.75 * spread) + Vec3::Z * z,
                    d * 0.28 - Vec3::Z,
                    d + Vec3::Z * 0.2,
                ),
            );
        }
    }
    metal(b);
    hoop(
        b,
        c + Vec3::Z * 13.8,
        1.2,
        1.0,
        0.6,
        if fine { 16 } else { 8 },
    );
    shaft(b, c + Vec3::Z * 13.8, c + Vec3::Z * (CAP - 1.0), 0.5);
    for &(z, r, lean) in &RINGS {
        collar(b, c + Vec3::Z * z, Vec3::Z, 0.85, 0.6);
        b.with_orbit(Vec3::Z, 0.6, |b| sensor_ring(b, c + Vec3::Z * z, r, lean));
    }
    // The cap: a plated crown, red optics round it, the owner's colour on top.
    dark_plate(b);
    b.prism(c + Vec3::Z * (CAP - 1.0), 8, 1.1, 0.6, 1.0);
    if fine {
        for k in 0..4 {
            let d = out(TAU * k as f32 / 4.0);
            red_slot(
                b,
                c + d * 0.92 + Vec3::Z * (CAP - 0.6),
                d,
                d.cross(Vec3::Z),
                0.5,
                0.14,
            );
        }
    }
    team_tab(b, c + Vec3::Z * (CAP + 0.01), Vec3::X, 0.35);
}

/// A sensor ring round the spindle at `c`: a bronze hoop of radius `r` leaning toward
/// bearing `lean`, three plated carriages on it swept back against its turn, a red slit on
/// each.
fn sensor_ring(b: &mut MeshBuilder, c: Vec3, r: f32, lean: f32) {
    let fine = b.fine();
    b.yawed(c, lean.to_radians(), |b| {
        b.pitched(Vec3::ZERO, 7f32.to_radians(), |b| {
            metal(b);
            hoop(b, Vec3::ZERO, r, 0.5, 0.36, if fine { 24 } else { 10 });
            for k in 0..3 {
                let a = TAU * k as f32 / 3.0;
                let d = out(a);
                let along = Vec3::Z.cross(d);
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(d * (r + 0.2) - along * 0.8, along, d),
                    &swept(2.2, 0.36, 0.0, 0.55),
                    0.22,
                );
                if fine {
                    red_slot(b, d * (r + 0.44) + along * 0.4, d, along, 0.7, 0.1);
                }
            }
        })
    });
}

/// The magazine on +x: a plated casemate, its well and gravity lift, the owner's colour,
/// and a plated duct into the block.
fn magazine(b: &mut MeshBuilder) {
    let fine = b.fine();
    let ring = |half: Vec2, ch: f32, z: f32| -> Vec<Vec3> {
        chamfered_rect(half, ch)
            .iter()
            .map(|p| v3(p[0] + MAG.x, p[1] + MAG.y, z))
            .collect()
    };
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                ring(MAG_HALF, 1.6, DECK),
                ring(MAG_HALF - Vec2::splat(0.6), 1.2, MAG_TOP),
            ],
            false,
            true,
        )
    });
    gravity_lift(
        b,
        HATCH.extend(MAG_TOP),
        WELL,
        ARRAY_LOAD,
        Lift {
            posts: 3,
            first: 90.0,
            rings: 2,
            round: true,
        },
    );
    team_tab(b, v3(MAG.x, MAG.y - 3.4, MAG_TOP + 0.02), Vec3::X, 1.0);
    // The duct into the block.
    dark_plate(b);
    b.with_facets(|b| {
        b.beam(
            v3(MAG.x - MAG_HALF.x + 0.4, MAG.y, 2.4),
            v3(BLOCK.x + 0.4, MAG.y, 2.4),
            Vec2::new(3.0, 2.4),
            Vec2::new(3.0, 2.4),
        )
    });
    if fine {
        red_slot(
            b,
            v3(MAG.x + MAG_HALF.x - 0.29, MAG.y, 2.6),
            Vec3::X,
            Vec3::Y,
            7.0,
            0.12,
        );
        metal(b);
        collar(b, v3(BLOCK.x + 1.2, MAG.y, 2.4), Vec3::X, 1.8, 0.6);
    }
}
