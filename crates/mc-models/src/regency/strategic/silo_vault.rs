//! The Mangonel as a vault (design A): a low faceted bunker in two courses of sloped
//! plate on an octagonal platform, its corners swept out over the platform in lapped
//! plates whose tails are the outline's spikes. The tube goes down through its middle (the
//! model's pit shows it below ground) under two leaves of lapped plate riding bronze
//! rails. Six lens pylons stand round the mouth, leaning in, their gravity lenses aimed at
//! the line the warhead rises up. Along -y the warhead store, a plated casemate, its well
//! under a gravity lift: a double containment collar holding the next round's core,
//! floating over the well between three emitter posts. Exhaust hoods out along ±x; the
//! bank of pinch capacitors that feeds the lenses along +y.

use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3};

use crate::builder::{chamfered_rect, MeshBuilder};
use crate::{part, Pit};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::super::turrets::sunspear::lens;
use super::*;

/// The platform's deck, and the bunker's roof.
const DECK: f32 = 1.4;
const TOP: f32 = 8.0;
/// The tube's floor.
const FLOOR: f32 = -15.0;
/// The leaves: their underside, and their half width and reach along y.
const LEAF_Z: f32 = TOP + 0.4;
const LEAF_X: f32 = 6.2;
const LEAF_Y: f32 = 6.0;
const RAIL_X: f32 = 6.95;
const RAIL_Y: f32 = 12.2;
/// The store along -y: its middle, half size, roof; its hatch's middle and half size.
const STORE: Vec2 = Vec2::new(0.0, -32.4);
const STORE_HALF: Vec2 = Vec2::new(13.0, 7.4);
const STORE_TOP: f32 = 7.2;
const HATCH: Vec2 = Vec2::new(5.0, -32.4);
const WELL: f32 = 3.2;
/// Where the lenses aim: up the line the warhead rises.
const AIM: Vec3 = Vec3::new(0.0, 0.0, 24.0);
/// The lens pylons' bearings (degrees), clear of the leaves' run along y.
const PYLONS: [f32; 6] = [0.0, 40.0, 140.0, 180.0, 220.0, 320.0];

pub(in crate::regency) fn silo_vault(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        coarse(b);
        return;
    }
    b.set_pit(Pit {
        open: DECK,
        radius: BORE + 0.5,
        stroke: 0.0,
        section: 0.0,
        rack: [0.0, 0.0],
    });
    platform(b);
    bunker(b);
    bore(b, TOP, FLOOR);
    b.with_part(part::SILO_ROUND, |b| warhead(b, LEAF_Z - 1.6, FLOOR + 0.15));
    b.with_part(part::SILO_DOOR, |b| b.mirror_y(vault_leaf));
    rails(b, RAIL_X, RAIL_Y, TOP);
    drives(b);
    for deg in PYLONS {
        b.yawed(Vec3::ZERO, deg.to_radians(), pylon);
    }
    for k in 0..8 {
        // The two corners toward the store keep clear of it.
        if k == 5 || k == 6 {
            continue;
        }
        b.yawed(Vec3::ZERO, TAU * (k as f32 + 0.5) / 8.0, corner);
    }
    store(b);
    for yaw in [0.0, PI] {
        b.yawed(Vec3::ZERO, yaw, exhaust);
    }
    capacitors(b);
}

/// Far off: the bunker as one sloped drum, the leaves, the store's roof and the pylons.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.loft(
        &[
            octagon(Vec2::ZERO, 31.0, 0.0),
            octagon(Vec2::ZERO, 19.0, TOP),
        ],
        false,
        true,
    );
    b.with_part(part::SILO_DOOR, |b| b.mirror_y(vault_leaf));
    team_tab(b, v3(13.0, 0.0, TOP + 0.02), Vec3::X, 1.6);
    dark_plate(b);
    b.decal(STORE.extend(STORE_TOP), STORE_HALF * 2.0 - Vec2::splat(1.0));
    // The lens pylons, which carry its height.
    for deg in PYLONS {
        b.yawed(Vec3::ZERO, deg.to_radians(), |b| {
            let (a, c, s) = (v3(13.6, 0.0, TOP), v3(8.4, 0.0, 21.0), Vec3::Y * 1.4);
            b.face(&[a - s, a + s, c]);
            b.face(&[c, a + s, a - s]);
        });
    }
}

/// The octagonal platform: a dark foot, a seam, the deck, open over the bore.
fn platform(b: &mut MeshBuilder) {
    let c = Vec2::ZERO;
    dark_plate(b);
    drum(b, c, 40.0, 38.8, 0.0, 0.9, false);
    deck_ring(b, c, 37.8, 38.8, 0.9);
    seam(b);
    drum(b, c, 37.8, 37.8, 0.9, 1.1, false);
    deck_ring(b, c, 37.0, 37.8, 1.1);
    dark_plate(b);
    drum(b, c, 37.0, 36.2, 1.1, DECK, false);
    deck_ring(b, c, BORE, 36.2, DECK);
}

/// The bunker: a lower course of sloped plate, a seam band lit red on every facet, an upper
/// course to the roof, the roof round the mouth, a bronze ring round the mouth, and the
/// owner's colour on the roof's four diagonal facets.
fn bunker(b: &mut MeshBuilder) {
    let c = Vec2::ZERO;
    dark_plate(b);
    drum(b, c, 27.0, 24.4, DECK, 5.0, false);
    deck_ring(b, c, 23.6, 24.4, 5.0);
    seam(b);
    drum(b, c, 23.6, 23.6, 5.0, 5.6, false);
    deck_ring(b, c, 23.0, 23.6, 5.6);
    dark_plate(b);
    drum(b, c, 23.0, 19.0, 5.6, TOP, false);
    deck_ring(b, c, BORE, 19.0, TOP);
    if b.fine() {
        // A red line let into the seam band on each facet.
        let apothem = 23.6 * (TAU / 16.0).cos();
        for k in [1.0f32, 3.0, 5.0, 7.0] {
            let d = out(TAU * k / 8.0);
            red_slot(
                b,
                d * apothem + Vec3::Z * 5.3,
                d,
                d.cross(Vec3::Z),
                4.0,
                0.12,
            );
        }
    }
    // The mouth's ring, low enough for the leaves to ride over.
    metal(b);
    b.yawed(Vec3::ZERO, TAU / 16.0, |b| {
        hoop(b, Vec3::Z * (TOP + 0.12), BORE + 0.55, 1.0, 0.24, 8)
    });
    for k in [1.0f32, 3.0, 5.0, 7.0] {
        let d = out(TAU * k / 8.0);
        team_tab(b, d * 15.0 + Vec3::Z * (TOP + 0.02), d, 1.5);
    }
}

/// One leaf (+y): a chamfered plate, two more lapped over it, the bronze carriages over
/// the rails.
fn vault_leaf(b: &mut MeshBuilder) {
    let (x, y) = (LEAF_X, LEAF_Y);
    let plan = [
        Vec2::new(-x, 0.02),
        Vec2::new(x, 0.02),
        Vec2::new(x, y - 1.6),
        Vec2::new(x - 1.6, y),
        Vec2::new(-x + 1.6, y),
        Vec2::new(-x, y - 1.6),
    ];
    leaf(b, &plan, LEAF_Z, 0.7, 2, 1.3);
    carriages(b, RAIL_X, 1.4, 4.6, TOP, LEAF_Z + 0.3);
    if b.fine() {
        // The carriages' arms out from the plate to the rails.
        dark_plate(b);
        for sx in [-1.0f32, 1.0] {
            let (a, c) = (sx * (x - 0.2), sx * (RAIL_X + 0.5));
            b.block(
                v3(a.min(c), 1.2, LEAF_Z + 0.25),
                v3(a.max(c), 4.8, LEAF_Z + 0.75),
            );
        }
    }
}

/// The drives at the rails' ends: a plated housing across each end, a red slit on its
/// face.
fn drives(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        dark_plate(b);
        b.chamfered_box(v3(0.0, RAIL_Y + 1.4, TOP + 0.7), v3(16.4, 1.8, 1.4), 0.5);
        if b.fine() {
            red_slot(
                b,
                v3(0.0, RAIL_Y + 2.32, TOP + 0.7),
                Vec3::Y,
                Vec3::X,
                6.0,
                0.12,
            );
            metal(b);
            for x in [-RAIL_X, RAIL_X] {
                b.cylinder_between(
                    v3(x, RAIL_Y + 0.5, TOP + 0.55),
                    v3(x, RAIL_Y - 0.6, TOP + 0.55),
                    0.32,
                    0.32,
                    6,
                );
            }
        }
    });
}

/// A lens pylon along +x (turned into place by the caller): a plated foot on the roof, a
/// bronze spine leaning in to a plated head, plates lapped down its back into a spike, and
/// in the head's inner face a gravity lens aimed up the warhead's line.
fn pylon(b: &mut MeshBuilder) {
    let fine = b.fine();
    let foot = v3(13.6, 0.0, TOP);
    let head = v3(8.4, 0.0, 20.0);
    dark_plate(b);
    b.frustum(
        foot,
        Vec2::new(3.6, 3.0),
        Vec2::new(2.4, 2.2),
        1.6,
        Vec2::new(-0.4, 0.0),
    );
    // The body: a plated strut over a bronze spine showing at its foot and under its head.
    shaft(b, foot + v3(-0.4, 0.0, 1.2), head, 0.55);
    let down = (foot + v3(1.6, 0.0, 0.0) - head).normalize();
    let back = v3(-down.z, 0.0, down.x);
    strut(
        b,
        head + down * 1.2,
        foot + v3(0.0, 0.0, 1.0) - down * 1.6,
        0.85,
    );
    dark_plate(b);
    Course {
        count: if fine { 3 } else { 2 },
        step: 2.2,
        len: 3.4,
        half: 1.5,
        tip: 0.0,
        thick: 0.4,
        tail: 1.6,
    }
    .lay(b, &Frame::new(head + back * 1.0 + down * 0.6, down, back));
    // The head: a plated housing swept back over the spine.
    dark_plate(b);
    b.beam(
        head + v3(1.3, 0.0, 0.5),
        head + v3(-0.5, 0.0, -0.2),
        Vec2::new(2.2, 2.0),
        Vec2::new(1.8, 1.6),
    );
    lens(b, head + v3(-0.75, 0.0, -0.2), AIM, 0.75);
    if fine {
        red_slot(b, foot + v3(1.81, 0.0, 0.8), Vec3::X, Vec3::Y, 1.6, 0.1);
    }
}

/// A corner of the bunker (along +x, turned by the caller): plates lapped down its slope
/// and out over the platform, the last running out into a spike.
fn corner(b: &mut MeshBuilder) {
    let u = v3(1.0, 0.0, -0.3);
    let n = v3(0.3, 0.0, 1.0);
    dark_plate(b);
    Course {
        count: if b.fine() { 3 } else { 2 },
        step: 3.6,
        len: 5.4,
        half: 2.8,
        tip: 0.0,
        thick: 0.55,
        tail: 3.2,
    }
    .lay(b, &Frame::new(v3(25.0, 0.0, 4.8), u, n));
    if b.fine() {
        red_slot(b, v3(27.2, 0.0, 4.55), n, Vec3::Y, 2.6, 0.1);
    }
}

/// The warhead store along -y: a plated casemate on a seam-dark footing, plates lapped
/// over its roof, its hatch, the owner's colour, and bronze conduits into the bunker.
fn store(b: &mut MeshBuilder) {
    let ring = |half: Vec2, ch: f32, z: f32| -> Vec<Vec3> {
        chamfered_rect(half, ch)
            .iter()
            .map(|p| v3(p[0] + STORE.x, p[1] + STORE.y, z))
            .collect()
    };
    seam(b);
    b.chamfered_box(
        STORE.extend(DECK + 0.3),
        (STORE_HALF * 2.0 + Vec2::splat(1.4)).extend(0.6),
        2.0,
    );
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                ring(STORE_HALF, 2.2, DECK),
                ring(STORE_HALF - Vec2::splat(1.2), 1.6, STORE_TOP),
            ],
            false,
            true,
        )
    });
    gravity_lift(
        b,
        HATCH.extend(STORE_TOP),
        WELL,
        SILO_LOAD,
        Lift {
            posts: 3,
            first: 0.0,
            rings: 2,
            round: true,
        },
    );
    team_tab(b, v3(-9.6, STORE.y, STORE_TOP + 0.02), Vec3::X, 1.6);
    if b.fine() {
        // Lapped plates along the roof's edges, behind the well.
        dark_plate(b);
        for sy in [-1.0f32, 1.0] {
            let y = STORE.y + sy * 5.2;
            armour(
                b,
                &Frame::new(v3(-11.0, y, STORE_TOP), Vec3::X, Vec3::Z),
                &swept(12.0, 0.6, 0.0, 0.85),
                0.25,
            );
        }
        // A red line along the casemate's long faces.
        for sy in [-1.0f32, 1.0] {
            let y = STORE.y + sy * (STORE_HALF.y - 0.62);
            red_slot(b, v3(0.0, y, 4.2), Vec3::Y * sy, Vec3::X, 16.0, 0.12);
        }
        // Bronze conduits from the store into the bunker.
        metal(b);
        for x in [-7.0f32, 7.0] {
            super::super::kit::cable(
                b,
                &[
                    v3(x, STORE.y + STORE_HALF.y - 0.6, 4.0),
                    v3(x, -24.2, 4.0),
                    v3(x * 0.9, -22.6, 5.8),
                ],
                0.5,
            );
        }
    }
}

/// An exhaust hood out along +x: a plated duct from the bunker to a hood, red heat seen
/// through the slots of its grille.
fn exhaust(b: &mut MeshBuilder) {
    let fine = b.fine();
    dark_plate(b);
    b.with_facets(|b| {
        b.beam(
            v3(25.5, 0.0, 3.0),
            v3(31.0, 0.0, 2.6),
            Vec2::new(4.6, 2.8),
            Vec2::new(4.6, 2.8),
        )
    });
    let c = v3(33.2, 0.0, DECK);
    b.frustum(
        c,
        Vec2::new(5.4, 8.0),
        Vec2::new(4.2, 6.6),
        2.6,
        Vec2::new(0.4, 0.0),
    );
    seam(b);
    b.block(c + v3(-1.6, -2.8, 2.6), c + v3(2.4, 2.8, 2.75));
    if fine {
        for k in 0..4 {
            let y = -2.1 + 1.4 * k as f32;
            red_slot(b, c + v3(0.4, y, 2.7), Vec3::Z, Vec3::X, 3.4, 0.16);
        }
        metal(b);
        for k in 0..5 {
            let y = -2.8 + 1.4 * k as f32;
            b.block(c + v3(-1.6, y - 0.08, 2.75), c + v3(2.4, y + 0.08, 3.0));
        }
    }
}

/// The pinch capacitors along +y: three plated drums on a seam-dark skid, bronze
/// collars, a red line on each, and bronze cables into the bunker.
fn capacitors(b: &mut MeshBuilder) {
    let fine = b.fine();
    seam(b);
    b.chamfered_box(v3(0.0, 30.0, DECK + 0.25), v3(22.0, 7.0, 0.5), 1.2);
    for x in [-7.0f32, 0.0, 7.0] {
        let c = Vec2::new(x, 30.0);
        dark_plate(b);
        drum(b, c, 2.8, 2.5, DECK + 0.5, DECK + 4.4, true);
        metal(b);
        b.yawed(c.extend(0.0), TAU / 16.0, |b| {
            hoop(b, Vec3::Z * (DECK + 3.0), 2.75, 0.4, 0.5, 8);
            hoop(b, Vec3::Z * (DECK + 4.5), 1.9, 1.0, 0.25, 8);
        });
        if fine {
            red_slot(
                b,
                c.extend(DECK + 2.0) - Vec3::Y * 2.45,
                -Vec3::Y,
                Vec3::Z,
                1.4,
                0.12,
            );
            metal(b);
            super::super::kit::cable(
                b,
                &[
                    c.extend(DECK + 1.2) - Vec3::Y * 2.5,
                    v3(x * 0.85, 25.6, DECK + 1.0),
                    v3(x * 0.8, 23.4, DECK + 1.6),
                ],
                0.4,
            );
        }
    }
}
