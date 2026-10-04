//! The Mantlet, the Regency's tech 3 air-defence carrier (`regency_t3_carrier`): the
//! Atoll's place, done the Regency way. A 120 m surface hull under a flight deck of dark
//! plate, an island off to starboard, and no gun that reaches a ship: what it carries
//! holds the sky over the fleet.
//!
//! What it carries (weapon numbers as in the unit file):
//! - 0: twelve heavy gravitic seeker cells in two hatched blocks of six (`CellBlock`), dark
//!   lids over red rims, a round in each with its plasma head lit.
//! - 1, 2: a Plasmeric AA Repeater on a gun house of its own each side, port first: the
//!   Canopy's organ of four short tubes held up at the sky.
//! - 3: two interceptor doors in the transom under the water.
//! - Four gravity lenses (`anti_missile_mounts`) on bronze posts, looking up
//!   (`turrets::rondel::lens_post`).
//! - The radar (3800 m): the Orrery's floating rings (`regency/eye.rs`) over the island,
//!   turning (`part::SPINNER`).
//!
//! The shape is an arrowhead: one hull, the deck drawn to a point at the bow and out
//! into two quarter spikes past the stern; a long armoured ridge on the starboard edge
//! carries both cell blocks either side of the tower.
//!
//! The origin is the waterline; the keel is drawn below it.

use glam::{Vec2, Vec3};

use crate::builder::{chamfered_rect, CellGrid, MeshBuilder, Section};
use crate::library::ModelDef;
use crate::material::*;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{
    armour, collar, hoop, mouth_rim, red_slot, shaft, strut, swept, Course, Frame,
};
use super::super::turrets::rondel::lens_post;

pub(crate) const MODELS: &[ModelDef] =
    &[ModelDef::new("regency_carrier", RADIUS, HEIGHT, arrowhead)];

const RADIUS: f32 = 60.0;
const HEIGHT: f32 = 22.0;

/// The flight deck's top, and how deep its slab is.
const DECK: f32 = 9.0;
const SLAB: f32 = 1.6;

/// Where the ship puts what the unit file names: the two cell blocks (fore first) and
/// their hatches' deck, the AA houses' pivots and muzzles (port first), the
/// gravity lenses, the radar's bearing and the interceptor doors.
struct Layout {
    cells: [Vec2; 2],
    cell_deck: f32,
    aa: [(Vec3, Vec3); 2],
    defence: [Vec3; 4],
    radar: Vec3,
    tubes: [Vec3; 2],
}

/// The AA organ's muzzle from its pivot, the bore level (the sim holds it up at the sky
/// at rest).
const AA_REACH: Vec3 = Vec3::new(3.4, 0.0, 0.0);

const fn aa_houses(x: f32, y: f32, z: f32) -> [(Vec3, Vec3); 2] {
    [
        (
            Vec3::new(x, y, z),
            Vec3::new(x + AA_REACH.x, y, z + AA_REACH.z),
        ),
        (
            Vec3::new(x, -y, z),
            Vec3::new(x + AA_REACH.x, -y, z + AA_REACH.z),
        ),
    ]
}

// ---- The hull ------------------------------------------------------------------------

/// One cross-section at `x`, mirrored about its hull's middle: the keel's z, then (z, half
/// beam) at the chine, the knuckle (the widest) and the deck edge, drawn in from the
/// knuckle (tumblehome).
#[derive(Clone, Copy)]
struct Station {
    x: f32,
    keel: f32,
    chine: [f32; 2],
    knuckle: [f32; 2],
    deck: [f32; 2],
}

const fn st(x: f32, keel: f32, chine: [f32; 2], knuckle: [f32; 2], deck: [f32; 2]) -> Station {
    Station {
        x,
        keel,
        chine,
        knuckle,
        deck,
    }
}

/// The hull, transom first to the stem's point.
const HULL: [Station; 8] = [
    st(-50.0, -3.2, [-2.2, 7.6], [2.6, 9.0], [7.6, 8.4]),
    st(-38.0, -4.0, [-2.8, 8.6], [2.4, 10.2], [7.6, 9.4]),
    st(-15.0, -4.4, [-3.2, 9.0], [2.3, 10.6], [7.6, 9.8]),
    st(12.0, -4.4, [-3.2, 8.8], [2.4, 10.4], [7.7, 9.6]),
    st(32.0, -4.0, [-2.8, 7.4], [2.7, 9.0], [7.8, 8.2]),
    st(45.0, -3.0, [-2.0, 5.0], [3.2, 6.4], [8.0, 5.8]),
    st(54.0, -1.4, [-0.4, 2.4], [4.0, 3.2], [8.2, 2.8]),
    st(59.5, 2.5, [3.8, 0.0], [5.0, 0.0], [8.4, 0.0]),
];

/// The hull lofted through `stations`. Far off, a V from the transom to the stem.
fn hull(b: &mut MeshBuilder, stations: &[Station]) {
    let ring = |s: &Station, simple: bool| -> Vec<Vec3> {
        let p = |y: f32, z: f32| v3(s.x, y, z);
        if simple {
            return vec![
                p(0.0, s.keel),
                p(-s.deck[1], s.deck[0]),
                p(s.deck[1], s.deck[0]),
            ];
        }
        vec![
            p(0.0, s.keel),
            p(-s.chine[1], s.chine[0]),
            p(-s.knuckle[1], s.knuckle[0]),
            p(-s.deck[1], s.deck[0]),
            p(s.deck[1], s.deck[0]),
            p(s.knuckle[1], s.knuckle[0]),
            p(s.chine[1], s.chine[0]),
        ]
    };
    dark_plate(b);
    if b.coarse() {
        let n = stations.len();
        let rings: Vec<Vec<Vec3>> = [0, n - 1]
            .iter()
            .map(|&i| ring(&stations[i], true))
            .collect();
        b.loft(&rings, true, false);
        return;
    }
    let rings: Vec<Vec<Vec3>> = stations.iter().map(|s| ring(s, false)).collect();
    b.loft(&rings, true, false);
}

/// The interceptor doors in the transom: a dark sleeve and a bronze mouth each.
fn interceptor_doors(b: &mut MeshBuilder, tubes: &[Vec3; 2]) {
    let sides = b.sides(8);
    for t in tubes {
        seam(b);
        b.cylinder_between(
            *t + v3(0.6, 0.0, 0.0),
            *t - v3(0.1, 0.0, 0.0),
            0.6,
            0.6,
            sides,
        );
        metal(b);
        b.cylinder_between(
            *t - v3(0.08, 0.0, 0.0),
            *t - v3(0.2, 0.0, 0.0),
            0.45,
            0.42,
            sides,
        );
    }
}

/// Courses of plates along a hull's flank at the knuckle, `y` out from the middle (the
/// near side; mirrored), lapped back like feathers from `x0` for `len`, the last one
/// trailing out into a spike; a bronze strake in the gap under them.
fn flank(b: &mut MeshBuilder, x0: f32, len: f32, y: f32, z: f32) {
    let fine = b.fine();
    let count = (len / 11.0).round().max(2.0) as usize;
    let step = len / count as f32;
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count,
            step,
            len: step * 1.15,
            half: 1.1,
            tip: -0.3,
            thick: 0.4,
            tail: 3.0,
        }
        .lay(
            b,
            &Frame::new(v3(x0, y, z), v3(-1.0, 0.0, -0.02), v3(0.0, 1.0, 0.4)),
        );
        if fine {
            metal(b);
            b.beam(
                v3(x0 + 1.0, y - 0.35, z - 0.9),
                v3(x0 - len, y - 0.35, z - 0.9),
                Vec2::new(0.35, 0.4),
                Vec2::new(0.35, 0.4),
            );
        }
    });
}

// ---- The flight deck -----------------------------------------------------------------

/// The flight deck: a slab of dark plate on `plan`, a seam course under its lip. Far off,
/// a plain slab on `rough`.
fn flight_deck(b: &mut MeshBuilder, plan: &[[f32; 2]], rough: &[[f32; 2]]) {
    let under = DECK - SLAB;
    if b.coarse() {
        dark_plate(b);
        b.loft_z(rough, &[Section::new(under, 1.0), Section::new(DECK, 1.0)]);
        return;
    }
    seam(b);
    b.loft_z(
        plan,
        &[
            Section::scaled(under, 0.985, 0.94),
            Section::new(under + 0.45, 1.0),
        ],
    );
    dark_plate(b);
    b.loft_z(
        plan,
        &[
            Section::new(under + 0.45, 1.0),
            Section::new(DECK - 0.3, 1.0),
            Section::scaled(DECK, 0.996, 0.985),
        ],
    );
}

/// Plated struts under the deck's overhang, from the hull's knuckle out to the deck's
/// underside: (x, the knuckle's y and z, the deck edge's y), the near side; mirrored.
fn overhang(b: &mut MeshBuilder, braces: &[[f32; 4]]) {
    let fine = b.fine();
    b.mirror_y(|b| {
        for &[x, ky, kz, ey] in braces {
            let foot = v3(x, ky - 0.4, kz + 0.6);
            let head = v3(x - 1.5, ey - 1.6, DECK - SLAB + 0.05);
            strut(b, foot, head, 0.35);
            if fine {
                collar(b, foot, head - foot, 0.5, 0.7);
            }
        }
    });
}

/// Lapped plates along the deck's rim from `a` to `c` (the near side; mirrored), their
/// faces turned out and down over the edge, the last running `tail` on into a spike.
/// Full detail only.
fn rim(b: &mut MeshBuilder, a: Vec2, c: Vec2, tail: f32) {
    if b.fine() {
        b.mirror_y(|b| rim_one(b, a, c, tail));
    }
}

/// [`rim`] along one edge.
fn rim_one(b: &mut MeshBuilder, a: Vec2, c: Vec2, tail: f32) {
    let along = c - a;
    let count = (along.length() / 9.0).round().max(1.0) as usize;
    let step = along.length() / count as f32;
    let out = Vec2::new(along.y, -along.x).normalize();
    let out = if ((a + c) * 0.5).dot(out) < 0.0 {
        -out
    } else {
        out
    };
    dark_plate(b);
    Course {
        count,
        step,
        len: step * 1.12,
        half: 0.6,
        tip: 0.7,
        thick: 0.3,
        tail,
    }
    .lay(
        b,
        &Frame::new(
            (a + out * 0.15).extend(DECK - 0.75),
            along.extend(-0.05),
            out.extend(-0.35),
        ),
    );
}

/// A landing lane on the deck from `a` to `c`, `half` wide: bronze strips let into the
/// deck down both edges.
fn lane(b: &mut MeshBuilder, a: Vec2, c: Vec2, half: f32) {
    let d = (c - a).normalize();
    let side = Vec2::new(-d.y, d.x) * half;
    metal(b);
    for s in [side, -side] {
        b.beam(
            (a + s).extend(DECK - 0.05),
            (c + s).extend(DECK - 0.05),
            Vec2::new(0.35, 0.16),
            Vec2::new(0.35, 0.16),
        );
    }
}

/// A deck lift: a plate let into the deck with a seam round it.
fn lift(b: &mut MeshBuilder, at: Vec2, size: Vec2) {
    seam(b);
    b.plate(at.extend(DECK - 0.1), size, 0.14, 0.04);
    dark_plate(b);
    b.plate(at.extend(DECK - 0.1), size - Vec2::splat(0.6), 0.2, 0.08);
}

/// Two bronze launch tracks along x from `x0` to `x1`, `y` apart about `mid`.
fn tracks(b: &mut MeshBuilder, x0: f32, x1: f32, mid: f32, y: f32) {
    metal(b);
    for s in [-0.5, 0.5] {
        b.beam(
            v3(x0, mid + s * y, DECK - 0.04),
            v3(x1, mid + s * y, DECK - 0.04),
            Vec2::new(0.5, 0.14),
            Vec2::new(0.5, 0.14),
        );
    }
}

/// The owner's colour on the deck: a chevron pointing forward with its tip at `x`.
fn team_chevron(b: &mut MeshBuilder, x: f32, y: f32, len: f32, half: f32) {
    let w = len * 0.35;
    b.paint(TEAM);
    b.at(v3(0.0, y, 0.0), |b| {
        b.mirror_y(|b| {
            b.face(&[
                v3(x, 0.0, DECK + 0.04),
                v3(x - len, half, DECK + 0.04),
                v3(x - len - w, half, DECK + 0.04),
                v3(x - w, 0.0, DECK + 0.04),
            ]);
        });
    });
}

// ---- Superstructure ------------------------------------------------------------------

/// A pointed plan: the prow `f` ahead of the origin, square shoulders `h` out, the after
/// corners swept in, `a` astern.
fn pointed(f: f32, a: f32, h: f32) -> Vec<[f32; 2]> {
    vec![
        [f, 0.0],
        [f * 0.35, h],
        [-a * 0.75, h],
        [-a, h * 0.55],
        [-a, -h * 0.55],
        [-a * 0.75, -h],
        [f * 0.35, -h],
    ]
}

/// A stepped plated mass on `plan` about `at`, from `z0` to `z1`, its top drawn in by
/// `top`, a dark seam band round its foot.
fn mass(b: &mut MeshBuilder, at: Vec2, plan: &[[f32; 2]], z0: f32, z1: f32, top: Vec2) {
    b.at(at.extend(0.0), |b| {
        seam(b);
        b.loft_z(
            plan,
            &[Section::new(z0, 1.03), Section::new(z0 + 0.4, 1.03)],
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

/// Red optics on the two forward faces of a [`pointed`] mass about `at`, `z` high,
/// scaled `k` from the plan.
fn visor(b: &mut MeshBuilder, at: Vec2, f: f32, h: f32, z: f32, k: f32) {
    let (p, q) = (Vec2::new(f, 0.0) * k, Vec2::new(f * 0.35, h) * k);
    let mid = (p + q) * 0.5;
    let along = (q - p).normalize();
    let out = Vec2::new(along.y, -along.x);
    let out = if out.x < 0.0 { -out } else { out };
    b.at(at.extend(0.0), |b| {
        b.mirror_y(|b| {
            red_slot(
                b,
                (mid + out * 0.02).extend(z),
                out.extend(0.0),
                along.extend(0.0),
                (q - p).length() * 0.7,
                0.4,
            );
        });
    });
}

/// Two plates back to back raked aft off a roof at `foot`, round the bronze mast up to
/// the radar's bearing `at`.
fn blade_mast(b: &mut MeshBuilder, foot: Vec3, at: Vec3, len: f32) {
    shaft(b, foot, at, 0.5);
    for side in [-1.0f32, 1.0] {
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                foot + v3(1.6, side * 0.08, 0.0),
                v3(-0.45, 0.0, 1.0),
                v3(0.0, side, 0.0),
            ),
            &[[0.0, -1.8], [0.0, 1.6], [len, 0.3], [len + 0.6, -0.9]],
            0.3,
        );
    }
}

/// The island about `at`, `len` long and `half` wide, up from `foot`: three plated tiers
/// stepped in, red optics on the forward faces of the lower two, a course of plates lapped
/// back down each flank of the lowest into spikes, and the blade mast carrying the
/// radar's rings on `radar` over the bridge.
fn island(b: &mut MeshBuilder, at: Vec2, len: f32, half: f32, foot: f32, radar: Vec3) {
    let tiers = [
        (Vec2::ZERO, len * 0.5, len * 0.5, half, foot, 15.2),
        (
            Vec2::new(-len * 0.06, 0.0),
            len * 0.36,
            len * 0.34,
            half * 0.8,
            15.2,
            18.2,
        ),
        (
            Vec2::new(-len * 0.1, 0.0),
            len * 0.2,
            len * 0.2,
            half * 0.6,
            18.2,
            19.8,
        ),
    ];
    for (k, &(off, f, a, h, z0, z1)) in tiers.iter().enumerate() {
        let top = if k == 2 {
            Vec2::new(0.8, 0.75)
        } else {
            Vec2::new(0.9, 0.86)
        };
        mass(b, at + off, &pointed(f, a, h), z0, z1, top);
        if k < 2 {
            visor(b, at + off, f, h, z1 - 0.9, 0.93);
        }
    }
    let fine = b.fine();
    b.at(at.extend(0.0), |b| {
        b.mirror_y(|b| {
            dark_plate(b);
            Course {
                count: 2,
                step: len * 0.3,
                len: len * 0.34,
                half: 0.8,
                tip: -0.4,
                thick: 0.3,
                tail: 2.4,
            }
            .lay(
                b,
                &Frame::new(
                    v3(len * 0.22, half + 0.1, 13.6),
                    v3(-1.0, 0.0, -0.08),
                    v3(0.0, 1.0, 0.5),
                ),
            );
            if fine {
                red_slot(
                    b,
                    v3(-len * 0.25, half * 0.8 + 0.05, 16.8),
                    Vec3::Y,
                    Vec3::X,
                    len * 0.2,
                    0.2,
                );
            }
        });
    });
    blade_mast(b, radar.with_z(19.7), radar, 2.6);
    orrery(b, radar, &[(1.2, 3.6, 0.0), (3.2, 4.4, 140.0)]);
}

// ---- The radar -----------------------------------------------------------------------

/// The fleet radar on its bearing `at`: the Orrery's floating rings over a bronze
/// spindle, each ring clear of the spindle, held by red-tipped pinch studs on a collar,
/// plated carriages on it with their plates swept back and a red slot on each. The rings
/// turn together (`part::SPINNER`), each leaning a different way.
fn orrery(b: &mut MeshBuilder, at: Vec3, rings: &[(f32, f32, f32)]) {
    b.set_spinner_pivot(at);
    let top = rings.iter().map(|r| r.0).fold(0.0, f32::max) + 0.9;
    let fine = b.fine();
    metal(b);
    b.cylinder_between(at, at + Vec3::Z * top, 0.45, 0.35, b.sides(8));
    dark_plate(b);
    b.prism(at + Vec3::Z * top, 6, 0.7, 0.3, 0.7);
    for &(dz, r, toward) in rings {
        let c = at + Vec3::Z * dz;
        collar(b, c, Vec3::Z, 0.75, 0.5);
        for k in 0..3 {
            let a = (120.0 * k as f32 + toward).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            metal(b);
            b.cylinder_between(c + d * 0.6, c + d * (r - 1.4), 0.12, 0.1, 4);
            b.paint(GLOW_LASER);
            b.cuboid(c + d * (r - 1.3), Vec3::splat(0.28));
        }
        b.with_part(part::SPINNER, |b| ring(b, c, r, toward, fine));
    }
}

/// One of the radar's rings about `c`, `r` to its middle, leaning toward `toward`.
fn ring(b: &mut MeshBuilder, c: Vec3, r: f32, toward: f32, fine: bool) {
    b.yawed(c, toward.to_radians(), |b| {
        b.pitched(Vec3::ZERO, 7.0f32.to_radians(), |b| {
            metal(b);
            hoop(b, Vec3::ZERO, r, 0.7, 0.5, if fine { 24 } else { 12 });
            for k in 0..4 {
                let a = (90.0 * k as f32).to_radians();
                let d = v3(a.cos(), a.sin(), 0.0);
                let back = v3(a.sin(), -a.cos(), 0.0);
                let at = d * r;
                dark_plate(b);
                b.beam(
                    at - back * 0.8 + Vec3::Z * 0.1,
                    at + back * 0.8 + Vec3::Z * 0.1,
                    Vec2::new(1.2, 0.75),
                    Vec2::new(1.0, 0.65),
                );
                if fine {
                    dark_plate(b);
                    Course {
                        count: 1,
                        step: 0.0,
                        len: 1.6,
                        half: 0.5,
                        tip: -0.6,
                        thick: 0.2,
                        tail: 0.8,
                    }
                    .lay(
                        b,
                        &Frame::new(at - back * 0.5 + Vec3::Z * 0.5, back, Vec3::Z + d * 0.25),
                    );
                }
                red_slot(b, at + d * 0.62, d, back, 0.9, 0.2);
            }
        });
    });
}

// ---- Weapon 0: the seeker cells ------------------------------------------------------

/// The cells' pitch, how far a round's middle (its muzzle) is under its cell's deck, and
/// the order a block fires its six cells in.
const PITCH: f32 = 2.6;
#[cfg(test)]
const MUZZLE_DROP: f32 = 3.3;
const CELL_FIRE: [(u8, u8); 6] = [(0, 0), (2, 1), (1, 0), (0, 1), (2, 0), (1, 1)];

fn cell_grid(centre: Vec2, deck: f32) -> CellGrid {
    CellGrid {
        centre,
        deck,
        pitch: PITCH,
        half: PITCH * 0.42,
        nx: 3,
        ny: 2,
        hinge_y: true,
    }
}

/// Half a block's plan: its cells and the armour round them.
fn block_half() -> Vec2 {
    Vec2::new(PITCH * 1.5 + 0.5, PITCH + 0.5)
}

/// A block of six cells about `centre`, its hatches' deck at `deck`, its armoured box up
/// from `foot`: on it a lid for each cell hinged on its outer edge, over a red rim and the
/// round standing in it. Drawn from the reduced level up: far off, the block its ship
/// stands it on is drawn instead.
fn cells(b: &mut MeshBuilder, centre: Vec2, deck: f32, foot: f32) {
    let plan = chamfered_rect(block_half(), 0.8);
    b.at(centre.extend(0.0), |b| {
        dark_plate(b);
        b.loft_z(
            &plan,
            &[Section::new(foot, 1.05), Section::new(deck - 0.35, 1.0)],
        );
        seam(b);
        b.loft_z(
            &plan,
            &[Section::new(deck - 0.35, 1.0), Section::new(deck, 0.97)],
        );
    });
    let fine = b.fine();
    let lid = PITCH * 0.8;
    for m in b.cell_block(cell_grid(centre, deck), &CELL_FIRE) {
        let side = (m.y - centre.y).signum();
        if fine {
            b.paint(GLOW_LASER);
            b.plate(m.extend(deck), Vec2::splat(PITCH * 0.9), 0.04, 0.01);
        }
        b.with_part(part::CELL_HATCH, |b| {
            dark_plate(b);
            b.plate(m.extend(deck + 0.03), Vec2::splat(lid), 0.25, 0.08);
            if fine {
                // A swept ridge across the lid, and the bronze knuckle it swings on.
                dark_plate(b);
                b.beam(
                    v3(m.x - lid * 0.4, m.y, deck + 0.34),
                    v3(m.x + lid * 0.3, m.y - side * lid * 0.25, deck + 0.34),
                    Vec2::new(0.35, 0.16),
                    Vec2::new(0.1, 0.14),
                );
                metal(b);
                b.cylinder_between(
                    v3(m.x - lid * 0.3, m.y + side * lid * 0.46, deck + 0.12),
                    v3(m.x + lid * 0.3, m.y + side * lid * 0.46, deck + 0.12),
                    0.12,
                    0.12,
                    4,
                );
            }
        });
        if !fine {
            continue;
        }
        // The round: a bronze body, and the contained charge's lit head over it.
        b.with_part(part::CELL_ROUND, |b| {
            let top = deck - 0.3;
            metal(b);
            b.cylinder_between(m.extend(top - 3.0), m.extend(top - 1.0), 0.6, 0.6, 6);
            b.paint(GLOW_LASER);
            b.cylinder_between(m.extend(top - 1.0), m.extend(top), 0.6, 0.16, 6);
        });
    }
}

// ---- Weapons 1, 2: the AA houses ---------------------------------------------------

/// A AA house, weapon `weapon`, on its pivot and muzzle: a barbette up from the deck, a
/// hex step on a bronze race, a plated cheek either side swept back into a spike, and the
/// organ of four AA tubes between them held up at the sky, red rims at their mouths.
fn aa_house(b: &mut MeshBuilder, weapon: usize, (pivot, muzzle): (Vec3, Vec3)) {
    let base = pivot.z - 1.1;
    let sides = b.sides(10);
    dark_plate(b);
    b.prism(pivot.with_z(DECK - 0.2), sides, 2.6, 2.3, base - DECK + 0.2);
    if b.fine() {
        metal(b);
        hoop(b, pivot.with_z(base - 0.05), 2.0, 0.5, 0.16, sides);
    }
    let d = muzzle - pivot;
    let len = d.length();
    b.with_house(weapon, pivot, 0.35, |b| {
        dark_plate(b);
        b.prism(pivot.with_z(base), 6, 2.1, 1.8, 0.6);
        b.mirror_y(|b| {
            dark_plate(b);
            b.block(
                pivot + v3(-1.3, 1.4, base - pivot.z + 0.5),
                pivot + v3(0.9, 1.85, 0.7),
            );
            armour(
                b,
                &Frame::new(pivot + v3(0.9, 1.85, 0.3), v3(-1.0, 0.0, 0.4), Vec3::Y),
                &swept(3.6, 0.75, 0.3, 0.45),
                0.25,
            );
        });
        b.with_recoil(|b| {
            b.at(pivot, |b| organ(b, len));
        });
    });
}

/// The AA organ in its own frame (the trunnion at the origin, +x up the bore), `len` to
/// its mouths: a bronze trunnion, a plated breech, four tubes side by side whose sleeves
/// are rimmed red.
fn organ(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    let p = v3;
    collar(b, Vec3::ZERO, Vec3::Y, 0.55, 3.0);
    dark_plate(b);
    b.beam(
        p(-1.1, 0.0, 0.0),
        p(0.6, 0.0, 0.0),
        Vec2::new(2.7, 1.3),
        Vec2::new(2.5, 1.2),
    );
    let sides = b.sides(8);
    for y in [-0.93f32, -0.31, 0.31, 0.93] {
        dark_plate(b);
        b.cylinder_between(p(0.4, y, 0.0), p(len - 0.6, y, 0.0), 0.28, 0.26, sides);
        if fine {
            metal(b);
            b.cylinder_between(
                p(len - 0.7, y, 0.0),
                p(len - 0.4, y, 0.0),
                0.31,
                0.31,
                sides,
            );
        }
        dark_plate(b);
        b.cylinder_between(p(len - 0.45, y, 0.0), p(len, y, 0.0), 0.27, 0.3, sides);
        if fine {
            mouth_rim(b, p(len, y, 0.0), 0.3, sides);
        }
    }
    if fine {
        red_slot(b, p(-0.4, 0.0, 0.66), Vec3::Z, Vec3::Y, 1.6, 0.12);
    }
}

// ---- The arrowhead -------------------------------------------------------------------

const LAYOUT: Layout = Layout {
    cells: [Vec2::new(-1.0, -11.0), Vec2::new(-29.0, -11.0)],
    cell_deck: 10.9,
    aa: aa_houses(21.0, 16.4, 10.6),
    defence: [
        Vec3::new(38.0, 10.5, 10.6),
        Vec3::new(38.0, -10.5, 10.6),
        Vec3::new(-53.0, 14.0, 10.4),
        Vec3::new(-53.0, -14.0, 10.4),
    ],
    radar: Vec3::new(-16.7, -11.0, 20.4),
    tubes: [Vec3::new(-50.0, -3.0, -2.0), Vec3::new(-50.0, 3.0, -2.0)],
};

/// The arrowhead's deck, bow first round the port side and back by starboard.
const PLAN: [[f32; 2]; 14] = [
    [61.0, 0.0],
    [46.0, 7.5],
    [28.0, 13.8],
    [-24.0, 15.6],
    [-42.0, 15.0],
    [-61.0, 18.5],
    [-53.0, 10.5],
    [-51.5, 0.0],
    [-53.0, -10.5],
    [-61.0, -18.5],
    [-42.0, -15.0],
    [-24.0, -15.6],
    [28.0, -13.8],
    [46.0, -7.5],
];
const ROUGH: [[f32; 2]; 5] = [
    [61.0, 0.0],
    [28.0, 13.8],
    [-58.0, 17.0],
    [-58.0, -17.0],
    [28.0, -13.8],
];

/// The island's tower about its middle, its half plan.
const TOWER: Vec2 = Vec2::new(-15.0, -11.0);

/// One hull under an arrowhead deck: the bow drawn to a point, the quarters out into two
/// spikes past the stern; a long armoured ridge on the starboard edge with a cell block
/// at each end of it and the tower between, the radar's rings over the tower; an AA repeater
/// house on a sponson either side forward.
fn arrowhead(b: &mut MeshBuilder, _tech: u8) {
    let l = &LAYOUT;
    hull(b, &HULL);
    flight_deck(b, &PLAN, &ROUGH);
    team_chevron(b, -38.0, 0.0, 9.0, 6.5);
    if b.coarse() {
        // Far off: the ridge and the island as one block drawn in to the rings' height.
        dark_plate(b);
        b.frustum_open(
            v3(-8.0, l.cells[0].y, DECK - 0.1),
            Vec2::new(48.0, 7.0),
            Vec2::new(6.0, 4.0),
            l.radar.z + 3.4 - DECK,
            Vec2::new(TOWER.x + 8.0, 0.0),
        );
        return;
    }
    // The ridge: a low armoured plinth the length of the cells and the tower.
    let ridge = [
        [22.0, 0.0],
        [15.0, 3.6],
        [-12.0, 3.8],
        [-14.0, 3.6],
        [-30.0, 3.6],
        [-38.0, 2.0],
        [-38.0, -2.0],
        [-30.0, -3.6],
        [-14.0, -3.6],
        [-12.0, -3.8],
        [15.0, -3.6],
    ];
    let ridge_top = l.cell_deck - 0.6;
    mass(
        b,
        Vec2::new(-7.0, l.cells[0].y),
        &ridge,
        DECK - 0.1,
        ridge_top,
        Vec2::new(0.98, 0.9),
    );
    for c in l.cells {
        cells(b, c, l.cell_deck, ridge_top - 0.1);
    }
    island(b, TOWER, 17.0, 3.4, ridge_top, l.radar);
    for (k, f) in l.aa.into_iter().enumerate() {
        sponson(b, f.0, Vec2::new(f.0.x, f.0.y.signum() * 13.4));
        aa_house(b, 1 + k, f);
    }
    for d in l.defence {
        lens_post(b, d, DECK);
    }
    interceptor_doors(b, &l.tubes);
    flank(b, 44.0, 88.0, 10.0, 2.6);
    overhang(
        b,
        &[
            [30.0, 8.8, 2.7, 13.6],
            [8.0, 10.2, 2.4, 14.8],
            [-14.0, 10.4, 2.3, 15.4],
            [-36.0, 9.8, 2.4, 15.1],
        ],
    );
    rim(b, Vec2::new(46.0, 7.5), Vec2::new(28.0, 13.8), 0.0);
    rim(b, Vec2::new(28.0, 13.8), Vec2::new(-24.0, 15.6), 0.0);
    rim(b, Vec2::new(-42.0, 15.0), Vec2::new(-61.0, 18.5), 2.0);
    lane(b, Vec2::new(-46.0, 3.0), Vec2::new(30.0, 3.0), 4.5);
    tracks(b, 20.0, 54.0, 0.0, 3.0);
    lift(b, Vec2::new(-2.0, 10.0), Vec2::new(10.0, 6.5));
    lift(b, Vec2::new(-40.0, -6.0), Vec2::new(9.0, 6.0));
}

/// A swept plate sponson out from the deck edge at `edge`, carrying the house on `at`.
fn sponson(b: &mut MeshBuilder, at: Vec3, edge: Vec2) {
    let s = (at.y - edge.y).signum();
    dark_plate(b);
    armour(
        b,
        &Frame::new(
            v3(at.x + 4.5, edge.y - s * 1.0, DECK - 1.2),
            v3(-1.0, s * 0.18, 0.0),
            Vec3::Z,
        ),
        &[
            [0.0, s * 0.5],
            [0.0, -s * 1.5],
            [3.0, -s * 6.6],
            [9.5, -s * 6.0],
            [13.0, -s * 1.5],
            [11.0, s * 0.5],
        ],
        1.2,
    );
    strut(
        b,
        v3(at.x, edge.y - s * 3.0, 2.8),
        v3(at.x - 1.0, at.y - s * 0.5, DECK - 1.25),
        0.4,
    );
}

#[cfg(test)]
mod tests;
