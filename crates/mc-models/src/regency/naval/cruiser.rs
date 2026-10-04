//! The Pavise, the Regency's tech 2 cruiser (`regency_t2_cruiser`): the fleet's screen
//! against air and missiles, with one long arm at structures. A 52 m surface hull of dark
//! plate, tumblehome sides under a flared knuckle, the bow drawn to a point and courses of
//! plates lapped back along each flank into spikes past the stern, so from above it is an
//! arrowhead.
//!
//! The spine: a long raised spine down the middle, plates lapped back off it either side;
//! the cells in line on its forward half; a short bridge and a plated blade mast carrying
//! a swept scimitar array that turns; and aft an armoured silo for the heavy seeker.
//!
//! What it carries (weapon numbers as in the unit file):
//! - 0: twelve gravitic seeker cells in two hatched blocks of six (`CellBlock`), dark lids
//!   over red rims, a round in each with its plasma head lit.
//! - 1: the heavy gravitic seeker aft: one big charge in gravity containment, launched
//!   straight up from its silo; it is the ship's signature.
//! - 2: a Plasmeric Repeater forward on a gun house of its own.
//! - Two gravity lenses (`anti_missile_mounts`) on bronze posts, looking up
//!   (`turrets::rondel::lens_post`).
//! - The radar (2600 m), turning (`part::SPINNER`).

use glam::{Vec2, Vec3};

use crate::builder::{chamfered_rect, ngon, CellGrid, MeshBuilder, Section};
use crate::library::ModelDef;
use crate::material::*;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, hoop, red_slot, shaft, swept, Course, Frame};
use super::super::turrets::rondel::lens_post;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_cruiser", 27.0, 15.0, spine)];

// ---- The hull ------------------------------------------------------------------------

/// One cross-section at `x`, mirrored about y = 0: the keel's z, then (z, half beam) at
/// the chine, the knuckle (the widest, the side flaring out to it) and the deck edge,
/// drawn in from the knuckle (tumblehome).
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

/// Stern first, to the stem's point.
const HULL: [Station; 8] = [
    st(-24.5, -1.2, [-0.6, 4.2], [1.2, 5.4], [2.6, 4.9]),
    st(-18.0, -2.6, [-2.0, 5.2], [1.1, 6.6], [2.7, 6.0]),
    st(-8.0, -3.4, [-2.6, 5.6], [1.0, 7.0], [2.8, 6.3]),
    st(2.0, -3.5, [-2.6, 5.4], [1.1, 6.8], [2.9, 6.1]),
    st(11.0, -3.1, [-2.2, 4.4], [1.3, 5.6], [3.1, 5.0]),
    st(18.0, -2.4, [-1.5, 2.9], [1.7, 3.8], [3.4, 3.4]),
    st(23.5, -1.0, [-0.2, 1.3], [2.3, 1.8], [3.7, 1.6]),
    st(27.0, 1.8, [2.6, 0.0], [3.2, 0.0], [4.0, 0.0]),
];

/// The deck edge (z, half beam) at `x`.
fn deck_at(x: f32) -> (f32, f32) {
    let i = HULL
        .windows(2)
        .position(|w| x <= w[1].x)
        .unwrap_or(HULL.len() - 2);
    let (a, c) = (HULL[i], HULL[i + 1]);
    let t = ((x - a.x) / (c.x - a.x)).clamp(0.0, 1.0);
    (
        a.deck[0] + (c.deck[0] - a.deck[0]) * t,
        a.deck[1] + (c.deck[1] - a.deck[1]) * t,
    )
}

/// The hull lofted through its stations, its flank armour, the owner's colour across the
/// quarterdeck. Far off, a V through three stations.
fn hull(b: &mut MeshBuilder) {
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
    let coarse = b.coarse();
    let rings: Vec<Vec<Vec3>> = if coarse {
        [0, 3, 7].iter().map(|&i| ring(&HULL[i], true)).collect()
    } else {
        HULL.iter().map(|s| ring(s, false)).collect()
    };
    dark_plate(b);
    b.loft(&rings, true, true);
    let (qz, qh) = deck_at(-21.5);
    b.paint(TEAM);
    if coarse {
        b.face(&[
            v3(-23.0, -qh * 0.6, qz + 0.05),
            v3(-20.0, -qh * 0.6, qz + 0.05),
            v3(-20.0, qh * 0.6, qz + 0.05),
            v3(-23.0, qh * 0.6, qz + 0.05),
        ]);
        return;
    }
    b.plate(v3(-21.5, 0.0, qz), Vec2::new(2.4, qh * 1.3), 0.08, 0.03);
    flanks(b);
}

/// Courses of plates along each flank at the knuckle, lapped back like feathers: the
/// forward run follows the bow's flare out, the after run trails past the stern into the
/// quarter spikes.
fn flanks(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count: 3,
            step: 6.6,
            len: 7.6,
            half: 0.85,
            tip: 0.0,
            thick: 0.3,
            tail: 0.8,
        }
        .lay(
            b,
            &Frame::new(
                v3(18.5, 3.7, 2.4),
                v3(-1.0, 0.13, -0.03),
                v3(0.0, 1.0, 0.45),
            ),
        );
        dark_plate(b);
        Course {
            count: 3,
            step: 6.4,
            len: 7.6,
            half: 0.9,
            tip: -0.3,
            thick: 0.3,
            tail: 2.6,
        }
        .lay(
            b,
            &Frame::new(v3(-1.5, 6.7, 2.0), v3(-1.0, -0.05, 0.0), v3(0.0, 1.0, 0.45)),
        );
        if fine {
            // The bronze strake in the gap under the plates, and a red slot at the bow.
            metal(b);
            b.beam(
                v3(19.0, 3.3, 1.9),
                v3(-23.5, 5.3, 1.3),
                Vec2::new(0.25, 0.3),
                Vec2::new(0.25, 0.3),
            );
            red_slot(
                b,
                v3(22.0, 2.15, 3.0),
                v3(0.25, 1.0, 0.0),
                v3(1.0, -0.25, 0.0),
                1.6,
                0.18,
            );
        }
    });
}

/// A pointed plan: the prow `f` ahead of the origin, square shoulders `h` out, the
/// after corners swept in, `a` astern.
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

/// A stepped plated mass on `plan` at `x`, from `z0` to `z1`, its top drawn in by `top`,
/// and a dark seam band round its foot.
fn mass(b: &mut MeshBuilder, x: f32, plan: &[[f32; 2]], z0: f32, z1: f32, top: Vec2) {
    if b.coarse() {
        // Far off: its bounding box, drawn in as it rises.
        let (lo, hi) = plan.iter().fold(
            (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
            |(lo, hi), &[px, py]| (lo.min(Vec2::new(px, py)), hi.max(Vec2::new(px, py))),
        );
        dark_plate(b);
        b.frustum_open(
            v3(x + (lo.x + hi.x) * 0.5, 0.0, z0),
            hi - lo,
            (hi - lo) * top,
            z1 - z0,
            Vec2::ZERO,
        );
        return;
    }
    b.at(v3(x, 0.0, 0.0), |b| {
        seam(b);
        b.loft_z(
            plan,
            &[Section::new(z0, 1.03), Section::new(z0 + 0.35, 1.03)],
        );
        dark_plate(b);
        b.loft_z(
            plan,
            &[
                Section::new(z0 + 0.35, 1.0),
                Section::new(z1 - 0.4, 1.0 - (1.0 - top.x) * 0.6),
                Section::scaled(z1, top.x, top.y),
            ],
        );
    });
}

/// Red optics on the two forward faces of a [`pointed`] mass at `x`, `z` high, scaled
/// `k` from the plan.
fn visor(b: &mut MeshBuilder, x: f32, f: f32, h: f32, z: f32, k: f32) {
    if b.coarse() {
        return;
    }
    let (p, q) = (Vec2::new(f, 0.0) * k, Vec2::new(f * 0.35, h) * k);
    let mid = (p + q) * 0.5;
    let along = (q - p).normalize();
    let out = Vec2::new(-along.y, along.x);
    let out = if out.y < 0.0 { -out } else { out };
    b.mirror_y(|b| {
        red_slot(
            b,
            v3(x + mid.x, mid.y, z) + out.extend(0.0) * 0.02,
            out.extend(0.0),
            along.extend(0.0),
            (q - p).length() * 0.7,
            0.28,
        );
    });
}

// ---- Weapon 0: the seeker cells ---------------------------------------------------

/// A block of six cells, three along x by two across.
struct Cells {
    centre: Vec2,
    pitch: f32,
    /// The hatches' deck, and the armoured box's foot under it.
    deck: f32,
    foot: f32,
}

const CELL_FIRE: [(u8, u8); 6] = [(0, 0), (2, 1), (1, 0), (0, 1), (2, 0), (1, 1)];

impl Cells {
    fn half(&self) -> Vec2 {
        Vec2::new(self.pitch * 1.5 + 0.45, self.pitch + 0.45)
    }
}

/// The block's armoured box (drawn in as it rises, under a seam course), and on it a lid
/// for each cell hinged on its outer edge, over a red rim and the round standing in it.
fn cells(b: &mut MeshBuilder, c: &Cells) {
    if b.coarse() {
        // Far off, the hull draws the blocks with the spine they stand on.
        return;
    }
    let plan = chamfered_rect(c.half(), 0.6);
    b.at(c.centre.extend(0.0), |b| {
        dark_plate(b);
        b.loft_z(
            &plan,
            &[Section::new(c.foot, 1.06), Section::new(c.deck - 0.3, 1.0)],
        );
        seam(b);
        b.loft_z(
            &plan,
            &[Section::new(c.deck - 0.3, 1.0), Section::new(c.deck, 0.97)],
        );
    });
    let fine = b.fine();
    let grid = CellGrid {
        centre: c.centre,
        deck: c.deck,
        pitch: c.pitch,
        half: c.pitch * 0.42,
        nx: 3,
        ny: 2,
        hinge_y: true,
    };
    let lid = c.pitch * 0.8;
    for m in b.cell_block(grid, &CELL_FIRE) {
        let side = (m.y - c.centre.y).signum();
        if fine {
            b.paint(GLOW_LASER);
            b.plate(m.extend(c.deck), Vec2::splat(c.pitch * 0.9), 0.04, 0.01);
        }
        b.with_part(part::CELL_HATCH, |b| {
            dark_plate(b);
            b.plate(m.extend(c.deck + 0.03), Vec2::splat(lid), 0.2, 0.06);
            if fine {
                // A swept ridge across the lid, and the bronze knuckle it swings on.
                dark_plate(b);
                b.beam(
                    v3(m.x - lid * 0.4, m.y, c.deck + 0.28),
                    v3(m.x + lid * 0.3, m.y - side * lid * 0.25, c.deck + 0.28),
                    Vec2::new(0.22, 0.12),
                    Vec2::new(0.06, 0.1),
                );
                metal(b);
                b.cylinder_between(
                    v3(m.x - lid * 0.3, m.y + side * lid * 0.46, c.deck + 0.1),
                    v3(m.x + lid * 0.3, m.y + side * lid * 0.46, c.deck + 0.1),
                    0.09,
                    0.09,
                    4,
                );
            }
        });
        if !fine {
            continue;
        }
        // The round: a bronze body, and the contained charge's lit head over it, all
        // that shows over the rim when the lid swings open.
        b.with_part(part::CELL_ROUND, |b| {
            let top = c.deck - 0.25;
            metal(b);
            b.cylinder_between(m.extend(top - 2.0), m.extend(top - 0.7), 0.42, 0.42, 6);
            b.paint(GLOW_LASER);
            b.cylinder_between(m.extend(top - 0.7), m.extend(top), 0.42, 0.12, 6);
        });
    }
}

/// Where each of a block's missiles is launched from (the unit file's muzzles), in
/// firing order: under its cell's deck, a round's half length down.
#[cfg(test)]
fn muzzles(c: &Cells) -> Vec<Vec3> {
    let block = crate::CellBlock {
        centre: c.centre.to_array(),
        deck: c.deck,
        pitch: c.pitch,
        half: c.pitch * 0.42,
        nx: 3,
        ny: 2,
        hinge_y: true,
        first: 0,
        order: [0; crate::CellBlock::MAX_CELLS],
    };
    CELL_FIRE
        .iter()
        .map(|&(i, j)| {
            Vec2::from(block.grid_centre(i as usize, j as usize)).extend(c.deck - MUZZLE_DROP)
        })
        .collect()
}

/// How far under its cell's deck a seeker is launched from.
#[cfg(test)]
const MUZZLE_DROP: f32 = 1.6;

// ---- Weapon 2: the repeater house ---------------------------------------------------

/// The repeater's pivot and muzzle.
const GUN: Vec3 = Vec3::new(18.0, 0.0, 4.6);
const GUN_MUZZLE: Vec3 = Vec3::new(21.6, 0.0, 4.75);

/// The Plasmeric Repeater on its gun house: a low pointed house, a swept cheek plate
/// either side, and the gun: a bronze breech, a broad flat receiver and a wide mouth with
/// a red emitter face and a bronze focusing nozzle; what is in front of the breech kicks
/// back when it fires.
fn repeater(b: &mut MeshBuilder) {
    let deck = deck_at(GUN.x).0;
    if !b.coarse() {
        seam(b);
        b.prism(v3(GUN.x, 0.0, deck - 0.1), b.sides(10), 1.9, 1.8, 0.35);
    }
    b.with_house(2, GUN, 0.35, |b| {
        if b.coarse() {
            return;
        }
        let fine = b.fine();
        b.at(v3(GUN.x - 0.2, 0.0, 0.0), |b| {
            dark_plate(b);
            b.loft_z(
                &pointed(2.2, 2.0, 1.45),
                &[
                    Section::new(deck + 0.2, 1.0),
                    Section::new(GUN.z - 0.1, 1.0),
                    Section::scaled(GUN.z + 0.75, 0.7, 0.62).shifted(-0.35, 0.0),
                ],
            );
        });
        b.mirror_y(|b| {
            dark_plate(b);
            armour(
                b,
                &Frame::new(
                    v3(GUN.x + 0.9, 1.3, GUN.z + 0.1),
                    v3(-1.0, 0.22, -0.2),
                    v3(0.0, 1.0, 0.45),
                ),
                &swept(3.4, 0.5, 0.2, 0.45),
                0.2,
            );
        });
        if fine {
            red_slot(
                b,
                v3(GUN.x - 0.7, 0.0, GUN.z + 0.77),
                Vec3::Z,
                Vec3::Y,
                0.9,
                0.16,
            );
        }
        b.with_recoil(|b| {
            metal(b);
            b.cylinder_between(
                GUN + v3(0.6, 0.0, 0.0),
                GUN + v3(1.4, 0.0, 0.0),
                0.42,
                0.42,
                b.sides(8),
            );
            dark_plate(b);
            b.beam(
                GUN + v3(1.3, 0.0, 0.05),
                GUN_MUZZLE - v3(0.12, 0.0, 0.0),
                Vec2::new(0.8, 0.55),
                Vec2::new(1.05, 0.42),
            );
            b.paint(GLOW_LASER);
            b.beam(
                GUN_MUZZLE - v3(0.14, 0.0, 0.0),
                GUN_MUZZLE,
                Vec2::new(0.62, 0.12),
                Vec2::new(0.62, 0.12),
            );
            if fine {
                metal(b);
                b.cylinder_between(
                    GUN_MUZZLE - v3(0.8, 0.0, 0.0),
                    GUN_MUZZLE + v3(0.02, 0.0, 0.0),
                    0.13,
                    0.13,
                    6,
                );
            }
        });
    });
}

// ---- The spine ----------------------------------------------------------------------

/// A long raised spine down the middle, plates lapped back off it either side; the
/// cells in line on its forward half, one block behind the other; a short bridge and a
/// plated blade mast carrying a swept scimitar array that turns; and aft an armoured silo
/// for the heavy seeker, its four door plates standing open round the mouth like blades,
/// the charge sitting red in it and two gravity rings floating over it.
fn spine(b: &mut MeshBuilder, _tech: u8) {
    hull(b);
    let plan = [
        [17.0, 0.0],
        [11.5, 2.5],
        [-9.0, 2.7],
        [-14.5, 1.6],
        [-14.5, -1.6],
        [-9.0, -2.7],
        [11.5, -2.5],
    ];
    mass(
        b,
        0.0,
        &plan,
        deck_at(0.0).0 - 0.2,
        SPINE,
        Vec2::new(0.9, 0.88),
    );
    for x in CELL_X {
        cells(
            b,
            &Cells {
                centre: Vec2::new(x, 0.0),
                pitch: PITCH,
                deck: CELL_DECK,
                foot: SPINE - 0.1,
            },
        );
    }
    repeater(b);
    if b.coarse() {
        dark_plate(b);
        b.frustum_open(
            v3(-4.0, 0.0, SPINE),
            Vec2::new(6.0, 4.0),
            Vec2::new(1.6, 1.0),
            RADAR.z + 1.2 - SPINE,
            Vec2::new(-1.5, 0.0),
        );
        let (x0, x1) = (CELL_X[1] - PITCH * 1.5 - 0.5, CELL_X[0] + PITCH * 1.5 + 0.5);
        b.frustum_open(
            v3((x0 + x1) * 0.5, 0.0, SPINE - 0.1),
            Vec2::new(x1 - x0, PITCH * 2.0 + 1.0),
            Vec2::new(x1 - x0, PITCH * 2.0 + 0.9),
            CELL_DECK - SPINE + 0.1,
            Vec2::ZERO,
        );
        b.set_spinner_pivot(RADAR);
        return;
    }
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count: 4,
            step: 6.4,
            len: 7.2,
            half: 0.7,
            tip: 0.4,
            thick: 0.3,
            tail: 2.0,
        }
        .lay(
            b,
            &Frame::new(
                v3(13.5, 2.35, SPINE - 0.3),
                v3(-1.0, 0.01, -0.04),
                v3(0.0, 1.0, 0.75),
            ),
        );
    });
    mass(
        b,
        -3.4,
        &pointed(3.0, 3.4, 2.2),
        SPINE,
        8.6,
        Vec2::new(0.85, 0.8),
    );
    visor(b, -3.4, 3.0, 2.2, 7.8, 0.93);
    blade_mast(b);
    lens_post(b, DEFENCE[0], 8.6);
    lens_post(b, DEFENCE[1], SPINE);
    silo(b, HEAVY);
}

/// The spine's top, the cells' middles along it, their pitch and deck.
const SPINE: f32 = 5.0;
const CELL_X: [f32; 2] = [9.2, 3.6];
const PITCH: f32 = 1.6;
const CELL_DECK: f32 = 6.4;
/// The radar bearing, gravity lenses and heavy seeker's muzzle.
const RADAR: Vec3 = Vec3::new(-6.8, 0.0, 12.8);
const DEFENCE: [Vec3; 2] = [Vec3::new(-1.2, 0.0, 9.9), Vec3::new(-12.0, 0.0, 6.6)];
const HEAVY: Vec3 = Vec3::new(-19.2, 0.0, 7.6);

/// The blade mast: two plates back to back raked aft off the bridge roof round a bronze
/// mast, and on top a drum and the scimitar array, its two arms swept back to points, a
/// row of red slots along its face. The array turns.
fn blade_mast(b: &mut MeshBuilder) {
    let fine = b.fine();
    let at = RADAR;
    shaft(b, v3(-4.6, 0.0, 8.4), at, 0.35);
    for side in [-1.0f32, 1.0] {
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(-2.6, side * 0.05, 8.5),
                v3(-0.5, 0.0, 1.0),
                v3(0.0, side, 0.0),
            ),
            &[[0.0, -1.2], [0.0, 1.2], [4.6, 0.2], [5.0, -0.6]],
            0.25,
        );
    }
    b.set_spinner_pivot(at);
    b.with_part(part::SPINNER, |b| {
        metal(b);
        b.cylinder_between(at, at + Vec3::Z * 0.7, 0.75, 0.6, b.sides(10));
        for side in [-1.0f32, 1.0] {
            dark_plate(b);
            armour(
                b,
                &Frame::new(
                    at + v3(0.15, 0.0, 1.2),
                    v3(-0.22, side, 0.0),
                    v3(1.0, 0.0, 0.15),
                ),
                &swept(5.0, 0.75, -0.5 * side, 0.55),
                0.3,
            );
            if fine {
                for k in 1..4 {
                    let y = side * 1.15 * k as f32;
                    red_slot(
                        b,
                        at + v3(0.47 - 0.25 * k as f32, y, 1.2),
                        v3(1.0, 0.22 * side, 0.15),
                        v3(-0.22, side, 0.0),
                        0.8,
                        0.18,
                    );
                }
            }
        }
    });
}

/// The heavy seeker, its nose at `m`: an armoured octagonal silo, a seam course and a
/// bronze rim at its mouth, four door plates standing open outward round it, the red
/// charge seated in the mouth, and two bronze gravity rings floating over it.
fn silo(b: &mut MeshBuilder, m: Vec3) {
    let fine = b.fine();
    let deck = deck_at(m.x).0;
    let mouth = m.z - 1.2;
    b.at(v3(m.x, 0.0, 0.0), |b| {
        dark_plate(b);
        b.loft_z(
            &ngon(8, 2.6),
            &[
                Section::new(deck - 0.1, 1.0),
                Section::new(mouth - 1.4, 0.95),
                Section::new(mouth - 0.3, 0.82),
            ],
        );
        seam(b);
        b.loft_z(
            &ngon(8, 2.6),
            &[Section::new(mouth - 0.3, 0.82), Section::new(mouth, 0.78)],
        );
    });
    metal(b);
    hoop(
        b,
        v3(m.x, 0.0, mouth + 0.1),
        1.45,
        0.4,
        0.3,
        if fine { 16 } else { 8 },
    );
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(m.x, 0.0, mouth) + d * 1.75,
                d * 0.55 + Vec3::Z,
                d - Vec3::Z * 0.5,
            ),
            &swept(2.8, 0.95, 0.0, 0.5),
            0.25,
        );
    }
    b.paint(GLOW_LASER);
    b.spheroid(
        v3(m.x, 0.0, mouth + 0.1),
        v3(1.05, 1.05, 0.9),
        b.sides(10),
        if fine { 5 } else { 3 },
    );
    metal(b);
    for (dz, r) in [(1.6, 1.25), (2.7, 0.85)] {
        hoop(
            b,
            v3(m.x, 0.0, mouth + dz),
            r,
            0.28,
            0.25,
            if fine { 14 } else { 8 },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_model, rig, MeshVertex, Model};

    /// The two cell blocks, fore first.
    fn blocks() -> [Cells; 2] {
        CELL_X.map(|x| Cells {
            centre: Vec2::new(x, 0.0),
            pitch: PITCH,
            deck: CELL_DECK,
            foot: 0.0,
        })
    }

    fn nearest(model: &Model, lod: usize, at: Vec3, keep: impl Fn(&MeshVertex) -> bool) -> f32 {
        model.lods[lod]
            .vertices
            .iter()
            .filter(|v| keep(v))
            .map(|v| Vec3::from(v.pos).distance(at))
            .fold(f32::MAX, f32::min)
    }

    /// Fits the blueprint (27 m, 15 m) and the library's naval rules: budget, levels of
    /// detail, keel under the waterline, owner's colour on top, dark plate, no ARC light.
    #[test]
    fn it_fits_its_blueprint() {
        let key = "regency_cruiser";
        let model = build_model(key).unwrap();
        let tris = |lod: usize| model.lods[lod].indices.len() / 3;
        let (full, mid, coarse) = (tris(0), tris(1), tris(2));
        assert!((250..=7000).contains(&full), "{key}: {full} triangles");
        assert!(
            mid as f32 <= full as f32 * 0.45 + 20.0,
            "{key}: {full}/{mid}"
        );
        assert!(coarse < 60, "{key}: coarse {coarse}");
        for (lod, mesh) in model.lods.iter().enumerate() {
            let name = format!("{key} lod{lod}");
            let top = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
            assert!((12.0..=18.75).contains(&top), "{name}: top {top}");
            let low = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::min);
            assert!(low >= -4.5, "{name}: keel at {low}");
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(0.0, f32::max);
            assert!((20.25..=35.1).contains(&reach), "{name}: reach {reach}");
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == TEAM && v.normal[2] > 0.5),
                "{name}: no upward team colour"
            );
            assert!(mesh.vertices.iter().any(|v| v.material == PLATING_DARK));
            assert!(
                !mesh
                    .vertices
                    .iter()
                    .any(|v| v.material == GLOW || v.material == GLOW_ORANGE),
                "{name}: ARC's blue or orange light"
            );
        }
    }

    /// Two blocks of six cells, a round under each muzzle; the repeater's house on its
    /// pivot reaching its muzzle; red on each gravity lens; a turning radar; the
    /// heavy seeker's nose at its muzzle.
    #[test]
    fn it_carries_its_weapons() {
        let (key, blocks) = ("regency_cruiser", blocks());
        let model = build_model(key).unwrap();
        assert_eq!(model.cells.len(), 2, "{key}: two cell blocks");
        for (k, m) in blocks.iter().flat_map(muzzles).enumerate() {
            let (b, (i, j)) = (k / 6, CELL_FIRE[k % 6]);
            let c = Vec2::from(model.cells[b].grid_centre(i as usize, j as usize));
            assert!(c.distance(m.truncate()) < 1e-3, "{key}: muzzle {k}");
            let round = nearest(&model, 0, m, |v| v.part == part::CELL_ROUND);
            assert!(round < 1.5, "{key}: no round near muzzle {k}");
        }
        assert_eq!(model.houses.len(), 1, "{key}: one gun house");
        let house = model.houses[0];
        assert_eq!(house.weapon, 2);
        assert!(Vec3::from(house.pivot).distance(GUN) < 1e-3);
        let of_house = |v: &MeshVertex| v.rig & rig::LIMB_MASK == rig::HOUSE_FIRST;
        for lod in 0..2 {
            let barrel = nearest(&model, lod, GUN_MUZZLE, |v| {
                of_house(v) && v.rig & rig::RECOIL != 0
            });
            assert!(
                barrel < 0.4,
                "{key} lod{lod}: barrel {barrel} from its muzzle"
            );
            let house = model.lods[lod].vertices.iter().filter(|v| of_house(v));
            let past = house.clone().map(|v| v.pos[0]).fold(f32::MIN, f32::max);
            assert!(past <= GUN_MUZZLE.x + 0.1, "{key}: house reaches {past}");
            let foot = house.map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(foot <= GUN.z, "{key}: the house floats");
            for at in DEFENCE {
                let red = nearest(&model, lod, at, |v| v.material == GLOW_LASER);
                assert!(red < 0.9, "{key} lod{lod}: no red at the head {at}");
            }
        }
        assert_eq!(Vec3::from(model.spinner_pivot), RADAR);
        assert!(
            model.lods[0]
                .vertices
                .iter()
                .any(|v| v.part == part::SPINNER),
            "{key}: the radar turns"
        );
        let nose = nearest(&model, 0, HEAVY, |_| true);
        assert!(
            nose < 0.3,
            "{key}: heavy seeker's nose {nose} from its muzzle"
        );
    }

    /// The unit file's numbers are the model's.
    #[test]
    fn the_unit_files_numbers_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t2_cruiser").unwrap());
        assert_eq!(bp.visual.mesh, "regency_cruiser");
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let cells: Vec<Vec3> = blocks().iter().flat_map(muzzles).collect();
        let seekers = &bp.weapons[0];
        assert_eq!(seekers.muzzles.len(), cells.len());
        for (m, c) in seekers.muzzles.iter().zip(&cells) {
            assert!(
                v(*m).distance(*c) < 1e-3,
                "seeker muzzle {:?} for {c}",
                v(*m)
            );
        }
        assert!(v(seekers.muzzle).distance(cells[0]) < 1e-3);
        assert!(v(bp.weapons[1].muzzle).distance(HEAVY) < 1e-3);
        assert!(v(bp.weapons[2].pivot.unwrap()).distance(GUN) < 1e-3);
        assert!(v(bp.weapons[2].muzzle).distance(GUN_MUZZLE) < 1e-3);
        let mounts: Vec<Vec3> = bp.anti_missile_mounts.iter().map(|&m| v(m)).collect();
        assert_eq!(mounts.len(), 2);
        for (m, d) in mounts.iter().zip(DEFENCE) {
            assert!(m.distance(d) < 1e-3, "gravity lens {m} for {d}");
        }
    }

    /// Prints the model's numbers in the unit file's form.
    #[test]
    #[ignore = "prints numbers: cargo test -p mc-models cruiser_numbers -- --ignored --nocapture"]
    fn cruiser_numbers() {
        let cells: Vec<String> = blocks()
            .iter()
            .flat_map(muzzles)
            .map(|m| format!("({:.2}, {:.2}, {:.2})", m.x, m.y, m.z))
            .collect();
        println!("muzzles: [{}]", cells.join(", "));
        println!("heavy {HEAVY}, defence {DEFENCE:?}, gun {GUN} {GUN_MUZZLE}");
        let model = build_model("regency_cruiser").unwrap();
        let tris = model.lods.each_ref().map(|l| l.indices.len() / 3);
        println!("regency_cruiser: {tris:?}");
    }
}
