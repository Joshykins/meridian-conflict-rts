//! The Falchion, the Regency's tech 1 frigate: the one tech 1 Regency hull that stays on the
//! surface. A heavy plasmeric repeater forward on the unit's turret, a plasmeric flak organ
//! aft in a gun house of its own, and a search radar between them.
//!
//! The stepped tower: a long pointed hull whose flanks are clad in lapped plates swept
//! back into twin points past the stern, three tiers stepped up amidships over bronze
//! bands, and a pair of floating rings turning over the top tier (the Orrery's).
//!
//! The origin is the waterline; the keel runs below it. The guns' numbers are the unit
//! file's (`regency_t1_frigate` in `data/factions/regency/units/naval.ron`).

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::library::ModelDef;
use crate::material::*;
use crate::{part, rig};

use super::super::kit::{cable, dark_plate, metal, v3};
use super::super::machine::{armour, collar, hoop_on, red_slot, shaft, swept, Frame};

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_frigate", 15.5, 9.0, tower)];

/// The repeater's trunnion and muzzle (weapon 0, on the unit's turret).
const GUN: Vec3 = Vec3::new(8.0, 0.0, 4.0);
const MUZZLE: Vec3 = Vec3::new(12.6, 0.0, 4.2);
/// The flak organ's trunnion and the middle of its row of tube mouths (weapon 1, its own
/// gun house).
const FLAK: Vec3 = Vec3::new(-6.0, 0.0, 7.0);
const FLAK_MUZZLE: Vec3 = Vec3::new(-4.0, 0.0, 8.9);
/// The flak organ's tubes across its bore.
const TUBES: [f32; 4] = [-1.05, -0.35, 0.35, 1.05];

// ---- the hull -----------------------------------------------------------------------

/// A hull section at `x`: the keel's depth, and (height, half beam) at the bilge chine,
/// the knuckle and the deck edge.
#[derive(Clone, Copy, Debug)]
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

/// Lofts the hull through `stations`, stern first, in dark plate; the coarse level keeps
/// the `coarse` stations as a V from the keel to the deck edges.
fn hull(b: &mut MeshBuilder, stations: &[Station], coarse: &[usize]) {
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
    let rings: Vec<Vec<Vec3>> = if b.coarse() {
        coarse.iter().map(|&i| ring(&stations[i], true)).collect()
    } else {
        stations.iter().map(|s| ring(s, false)).collect()
    };
    dark_plate(b);
    b.loft(&rings, true, true);
}

/// Between stations at `x`: the deck's height and half beam, and the knuckle's.
fn at_x(stations: &[Station], x: f32) -> (Vec2, Vec2) {
    let i = stations
        .windows(2)
        .position(|w| x <= w[1].x)
        .unwrap_or(stations.len() - 2);
    let (a, c) = (stations[i], stations[i + 1]);
    let t = ((x - a.x) / (c.x - a.x)).clamp(0.0, 1.0);
    let lerp = |p: [f32; 2], q: [f32; 2]| Vec2::from(p).lerp(Vec2::from(q), t);
    (lerp(a.deck, c.deck), lerp(a.knuckle, c.knuckle))
}

/// A course of armour plates down the left flank (mirrored by the caller), each from its
/// `x` running `len` aft along the side at height `z`, `half` tall, its trailing edge
/// swept up-aft into a spike. The last runs `tail` further, past the stern.
fn flank_course(
    b: &mut MeshBuilder,
    stations: &[Station],
    plates: &[(f32, f32)],
    z: f32,
    half: f32,
    tail: f32,
) {
    // The side's half beam at `x` at the plate's middle, and how far it leans in per metre
    // up: the plate lies against the side from its foot up, following the tumblehome.
    let lean = |x: f32| {
        let (deck, knuckle) = at_x(stations, x);
        ((knuckle.y - deck.y) / (deck.x - knuckle.x).max(0.1)).max(0.0)
    };
    let side = |x: f32| {
        let (deck, knuckle) = at_x(stations, x);
        let t = ((z - knuckle.x) / (deck.x - knuckle.x).max(0.1)).clamp(0.0, 1.0);
        knuckle.y + (deck.y - knuckle.y) * t + 0.06
    };
    for (k, &(x, len)) in plates.iter().enumerate() {
        let long = if k + 1 == plates.len() {
            len + tail
        } else {
            len
        };
        let o = v3(x, side(x), z);
        let end = v3(x - long, side(x - long.min(x - stations[0].x)), z + 0.15);
        let u = end - o;
        let n = u.cross(Vec3::Z).normalize() + Vec3::Z * lean(x - long * 0.5);
        dark_plate(b);
        armour(
            b,
            &Frame::new(o, u, n),
            &swept(u.length(), half, 0.45, 0.55),
            0.26,
        );
        if b.fine() && k + 1 < plates.len() {
            // The red hairline where this plate laps over the next.
            red_slot(
                b,
                o + v3(-len * 0.92, 0.3, -half * 0.2),
                n,
                Vec3::Z,
                half * 1.1,
                0.06,
            );
        }
    }
    if b.fine() {
        // A bronze conduit along the side under the course.
        let (first, last) = (plates[0], plates[plates.len() - 1]);
        let (x0, x1) = (first.0, (last.0 - last.1).max(stations[0].x + 0.5));
        let run: Vec<Vec3> = (0..=6)
            .map(|i| {
                let x = x0 + (x1 - x0) * i as f32 / 6.0;
                v3(x, side(x) + lean(x) * (half + 0.2), z - half - 0.2)
            })
            .collect();
        metal(b);
        cable(b, &run, 0.14);
    }
}

/// A tier of superstructure in plan `outline` from `z0` to `z1`: a bronze band set in under
/// its foot, the plate above it drawn in to `k` at the top and shifted `aft` back.
fn tier(b: &mut MeshBuilder, outline: &[[f32; 2]], z0: f32, z1: f32, k: f32, aft: f32) {
    if b.mid() {
        metal(b);
        b.loft_z(
            outline,
            &[Section::new(z0, 0.9), Section::new(z0 + 0.4, 0.9)],
        );
    }
    dark_plate(b);
    let foot = if b.mid() { z0 + 0.4 } else { z0 };
    b.loft_z(
        outline,
        &[
            Section::new(foot, 1.0),
            Section::new(z1, k).shifted(-aft, 0.0),
        ],
    );
}

/// A pointed plan: nose at `front`, full width `half` from a third back to `back`, and the
/// trailing edge notched into two points that reach `tail` further aft.
fn arrow(front: f32, back: f32, half: f32, tail: f32) -> Vec<[f32; 2]> {
    let shoulder = front - (front - back) * 0.38;
    vec![
        [front, 0.0],
        [shoulder, half],
        [back, half],
        [back - tail, half * 0.8],
        [back + tail * 0.4, 0.0],
        [back - tail, -half * 0.8],
        [back, -half],
        [shoulder, -half],
    ]
}

/// The owner's colour on a roof at `z`: a chevron pointing forward.
fn team_chevron(b: &mut MeshBuilder, x: f32, len: f32, half: f32, z: f32) {
    b.paint(TEAM);
    b.face(&[
        v3(x + len, 0.0, z),
        v3(x, half, z),
        v3(x + len * 0.35, 0.0, z),
    ]);
    b.face(&[
        v3(x + len, 0.0, z),
        v3(x + len * 0.35, 0.0, z),
        v3(x, -half, z),
    ]);
}

// ---- the guns -----------------------------------------------------------------------

/// A section ring of `shape` about `c` in the y-z plane.
fn ring_x(x: f32, w: f32, h: f32, lift: f32, shape: &[[f32; 2]]) -> Vec<Vec3> {
    shape
        .iter()
        .map(|[s, u]| v3(x, s * w * 0.5, lift + u * h * 0.5))
        .collect()
}

/// A body lofted along x through `stations` (x, width, height, lift).
fn loft_x(b: &mut MeshBuilder, stations: &[[f32; 4]], shape: &[[f32; 2]]) {
    let rings: Vec<Vec<Vec3>> = stations
        .iter()
        .map(|&[x, w, h, lift]| ring_x(x, w, h, lift, shape))
        .collect();
    b.loft(&rings, true, true);
}

/// A faceted section with a raised keel.
const KEELED: [[f32; 2]; 7] = [
    [1.0, -0.5],
    [0.7, -1.0],
    [-0.7, -1.0],
    [-1.0, -0.5],
    [-0.8, 0.55],
    [0.0, 1.0],
    [0.8, 0.55],
];
/// A box with its corners cut.
const CHAMFERED: [[f32; 2]; 8] = [
    [1.0, -0.6],
    [0.6, -1.0],
    [-0.6, -1.0],
    [-1.0, -0.6],
    [-1.0, 0.6],
    [-0.6, 1.0],
    [0.6, 1.0],
    [1.0, 0.6],
];

/// The repeater on the unit's turret, its saddle seated at `seat` on the barbette.
fn main_gun(b: &mut MeshBuilder, seat: f32) {
    let d = MUZZLE - GUN;
    let (pitch, len) = (d.z.atan2(d.x), d.length());
    b.set_turret_pivot(v3(GUN.x, 0.0, seat));
    b.set_arm_pivot(GUN);
    b.set_recoil(GUN, MUZZLE, 0.45);
    b.with_part(part::TURRET, |b| {
        if b.coarse() {
            b.with_limb(rig::ARM_GUN, |b| {
                dark_plate(b);
                b.beam(
                    GUN - Vec3::X * 1.8,
                    MUZZLE,
                    Vec2::new(2.0, 1.6),
                    Vec2::new(0.6, 0.4),
                );
            });
            return;
        }
        // The saddle on the barbette, a cheek either side of the gun swept back and up.
        dark_plate(b);
        let sides = b.sides(8);
        b.prism(v3(GUN.x - 0.3, 0.0, seat), sides, 2.1, 1.8, 0.6);
        let top = GUN.z + 0.7;
        b.mirror_y(|b| {
            dark_plate(b);
            b.block(v3(GUN.x - 1.5, 1.2, seat + 0.5), v3(GUN.x + 0.8, 1.55, top));
            armour(
                b,
                &Frame::new(
                    v3(GUN.x + 0.9, 1.55, seat + 1.0),
                    v3(-1.0, 0.0, 0.45),
                    Vec3::Y,
                ),
                &swept(3.4, 0.75, 0.2, 0.5),
                0.26,
            );
        });
        b.with_limb(rig::ARM_GUN, |b| {
            b.pitched(GUN, pitch, |b| repeater(b, len));
        });
    });
}

/// The heavy repeater in its own frame (trunnion at the origin, +x down the bore): a
/// hunched receiver with heat louvres standing open along its back over red slits, a vent
/// flap swung open each side, and a wide flat mouth with a red emitter face and a bronze
/// focusing nozzle that kicks back when it fires.
fn repeater(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    collar(b, Vec3::ZERO, Vec3::Y, 0.55, 2.4);
    dark_plate(b);
    loft_x(
        b,
        &[
            [-2.4, 1.1, 0.8, 0.1],
            [-1.5, 1.8, 1.6, 0.45],
            [0.2, 2.0, 2.0, 0.55],
            [1.7, 1.8, 1.5, 0.3],
            [2.7, 1.3, 0.9, 0.0],
        ],
        &KEELED,
    );
    if fine {
        for (x, z) in [(-1.1f32, 1.15), (-0.35, 1.4), (0.4, 1.5), (1.15, 1.38)] {
            dark_plate(b);
            fin(
                b,
                v3(x, 0.0, z),
                v3(-0.5, 0.0, 1.0),
                Vec3::X,
                0.75,
                1.3,
                1.0,
            );
            red_slot(b, v3(x + 0.33, 0.0, z + 0.02), Vec3::Z, Vec3::Y, 0.7, 0.06);
        }
        shaft(b, v3(-1.9, 0.0, -0.6), v3(2.3, 0.0, -0.6), 0.3);
    }
    b.mirror_y(|b| {
        dark_plate(b);
        fin(
            b,
            v3(0.6, 0.92, 1.0),
            v3(0.0, 0.85, -0.3),
            v3(0.0, 0.3, 1.0),
            1.0,
            1.6,
            1.3,
        );
    });
    b.with_recoil(|b| {
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(v3(2.5, 0.0, 0.0), v3(3.0, 0.0, 0.0), 0.5, 0.5, sides);
        dark_plate(b);
        loft_x(
            b,
            &[
                [2.9, 1.3, 0.85, 0.0],
                [3.8, 2.2, 1.0, 0.0],
                [len, 2.3, 0.85, 0.0],
            ],
            &CHAMFERED,
        );
        b.paint(GLOW_LASER);
        loft_x(
            b,
            &[[len - 0.06, 1.4, 0.12, 0.0], [len + 0.02, 1.4, 0.12, 0.0]],
            &CHAMFERED,
        );
        metal(b);
        let sides = b.sides(6);
        b.cylinder_between(v3(len - 0.7, 0.0, 0.0), v3(len, 0.0, 0.0), 0.2, 0.2, sides);
    });
}

/// A plated fin standing out of a surface at `root` toward `out`, its face toward `face`:
/// `len` long, `w0` wide at the root narrowing to `w1`.
fn fin(b: &mut MeshBuilder, root: Vec3, out: Vec3, face: Vec3, len: f32, w0: f32, w1: f32) {
    armour(
        b,
        &Frame::new(root, out, face),
        &[
            [0.0, -w0 * 0.5],
            [0.0, w0 * 0.5],
            [len, w1 * 0.5],
            [len, -w1 * 0.5],
        ],
        0.12,
    );
}

/// The flak organ in its gun house (weapon 1): a hexagonal drum, a cheek either side swept
/// back and up into a spike, and four short flak tubes in one clamped block held up at the
/// sky, their hot rims red. The block pitches and kicks back as one.
fn flak_house(b: &mut MeshBuilder) {
    let d = FLAK_MUZZLE - FLAK;
    let (pitch, len) = (d.z.atan2(d.x), d.length());
    b.with_house(1, FLAK, 0.25, |b| {
        if b.coarse() {
            b.with_recoil(|b| {
                dark_plate(b);
                b.beam(
                    FLAK - d * 0.25,
                    FLAK_MUZZLE,
                    Vec2::new(2.8, 1.2),
                    Vec2::new(0.5, 0.4),
                );
            });
            return;
        }
        let fine = b.fine();
        dark_plate(b);
        let sides = b.sides(6);
        b.prism(FLAK - Vec3::Z * 1.1, sides, 1.7, 1.45, 0.6);
        b.mirror_y(|b| {
            dark_plate(b);
            b.block(
                v3(FLAK.x - 1.1, 1.55, FLAK.z - 0.5),
                v3(FLAK.x + 0.7, 1.85, FLAK.z + 0.6),
            );
            if fine {
                armour(
                    b,
                    &Frame::new(
                        v3(FLAK.x + 0.8, 1.85, FLAK.z - 0.2),
                        v3(-1.0, 0.0, 0.55),
                        Vec3::Y,
                    ),
                    &swept(2.6, 0.55, 0.0, 0.45),
                    0.24,
                );
            }
        });
        b.with_recoil(|b| {
            b.pitched(FLAK, pitch, |b| {
                collar(b, Vec3::ZERO, Vec3::Y, 0.42, 3.1);
                dark_plate(b);
                loft_x(
                    b,
                    &[[-1.2, 2.9, 1.1, 0.0], [-0.2, 3.0, 1.2, 0.0]],
                    &CHAMFERED,
                );
                let clamps: &[f32] = if fine {
                    &[0.5, len - 0.9]
                } else {
                    &[len - 0.9]
                };
                for &x in clamps {
                    dark_plate(b);
                    loft_x(
                        b,
                        &[[x - 0.2, 3.1, 0.95, 0.0], [x + 0.2, 3.1, 0.95, 0.0]],
                        &CHAMFERED,
                    );
                }
                if fine {
                    red_slot(b, v3(-0.7, 0.0, 0.62), Vec3::Z, Vec3::Y, 1.8, 0.06);
                }
                let sides = if fine { 8 } else { 5 };
                for y in TUBES {
                    dark_plate(b);
                    b.cylinder_between(v3(-0.2, y, 0.0), v3(len, y, 0.0), 0.32, 0.35, sides);
                    if fine {
                        b.paint(GLOW_LASER);
                        hoop_on(b, v3(len - 0.04, y, 0.0), Vec3::X, 0.26, 0.1, 0.08, sides);
                    }
                }
            });
        });
    });
}

// ---- the stepped tower --------------------------------------------------------------

const TOWER_HULL: [Station; 7] = [
    st(-14.6, 0.1, [-0.3, 2.2], [1.0, 2.7], [2.1, 2.5]),
    st(-11.0, -1.6, [-1.1, 3.0], [0.9, 3.6], [2.15, 3.4]),
    st(-4.0, -2.5, [-1.6, 3.15], [1.0, 3.85], [2.2, 3.6]),
    st(3.0, -2.6, [-1.6, 2.9], [1.0, 3.6], [2.3, 3.35]),
    st(9.0, -2.2, [-1.3, 2.0], [1.1, 2.75], [2.5, 2.55]),
    st(13.2, -1.3, [-0.7, 0.95], [1.3, 1.55], [2.75, 1.45]),
    st(15.9, 1.0, [1.4, 0.0], [2.2, 0.0], [3.0, 0.0]),
];

/// The frigate: the hull, its flank courses, the stepped tiers and the radar rings.
fn tower(b: &mut MeshBuilder, _tech: u8) {
    hull(b, &TOWER_HULL, &[0, 3, 6]);
    let gun_deck = at_x(&TOWER_HULL, GUN.x).0.x;
    let seat = gun_deck + 0.8;
    const RADAR: Vec3 = Vec3::new(-0.6, 0.0, 7.3);
    b.set_spinner_pivot(RADAR);
    if b.coarse() {
        dark_plate(b);
        b.loft_z(
            &[
                [4.5, 0.0],
                [-1.0, 2.6],
                [-11.0, 2.4],
                [-11.0, -2.4],
                [-1.0, -2.6],
            ],
            &[Section::new(2.2, 1.0), Section::scaled(6.4, 0.7, 0.6)],
        );
        team_chevron(b, -3.0, 3.0, 1.2, 6.42);
    } else {
        // The barbette: a bronze race under a dark plated drum.
        metal(b);
        let sides = b.sides(10);
        b.prism(v3(GUN.x, 0.0, gun_deck - 0.1), sides, 2.0, 2.0, 0.5);
        dark_plate(b);
        b.prism(
            v3(GUN.x, 0.0, gun_deck + 0.3),
            sides,
            2.3,
            2.0,
            seat - gun_deck - 0.3,
        );
        b.mirror_y(|b| {
            flank_course(
                b,
                &TOWER_HULL,
                &[
                    (12.6, 4.4),
                    (8.8, 4.8),
                    (4.6, 5.2),
                    (0.0, 5.2),
                    (-4.8, 5.2),
                    (-9.6, 5.0),
                ],
                1.45,
                0.72,
                2.2,
            );
        });
        // Tier 1: the long casemate, the flak plinth on its tail.
        tier(b, &arrow(5.4, -11.0, 2.75, 1.6), 2.15, 4.2, 0.9, 0.3);
        // Tier 2 and the crown, stepped up amidships.
        tier(b, &arrow(3.6, -3.2, 2.05, 1.0), 4.2, 6.0, 0.88, 0.25);
        tier(b, &arrow(1.9, -2.3, 1.4, 0.7), 6.0, RADAR.z, 0.85, 0.2);
        // The flak plinth.
        metal(b);
        b.prism(v3(FLAK.x, 0.0, 4.2), sides, 1.6, 1.6, 0.4);
        dark_plate(b);
        b.prism(
            v3(FLAK.x, 0.0, 4.6),
            b.sides(6),
            2.0,
            1.7,
            FLAK.z - 1.1 - 4.6,
        );
        if b.fine() {
            b.mirror_y(|b| {
                // The tiers' eyes and hot slots: red hairlines let into their faces.
                red_slot(
                    b,
                    v3(2.75, 1.3, 6.65),
                    v3(0.6, 1.0, 0.0),
                    v3(-1.0, 0.6, 0.0),
                    1.2,
                    0.08,
                );
                red_slot(
                    b,
                    v3(3.0, 1.5, 5.3),
                    v3(0.6, 1.0, 0.0),
                    v3(-1.0, 0.6, 0.0),
                    1.6,
                    0.08,
                );
                red_slot(b, v3(-6.5, 2.5, 3.3), Vec3::Y, Vec3::X, 3.0, 0.08);
                // Swept plates down the casemate's flanks.
                for (x, len) in [(2.0f32, 4.2f32), (-2.6, 4.6)] {
                    dark_plate(b);
                    armour(
                        b,
                        &Frame::new(v3(x, 2.62, 3.4), v3(-1.0, 0.0, 0.18), Vec3::Y),
                        &swept(len, 0.55, 0.4, 0.5),
                        0.22,
                    );
                }
                cable(
                    b,
                    &[v3(-3.0, 1.9, 4.25), v3(-4.6, 1.9, 4.25), v3(-5.2, 1.4, 4.7)],
                    0.12,
                );
            });
        }
        // The radar: two rings held floating round a bronze spindle by red-tipped pinch
        // studs, each leaning its own way, turning together; the owner's colour round the
        // top one.
        b.with_part(part::SPINNER, |b| {
            shaft(b, RADAR, RADAR + Vec3::Z * 2.5, 0.3);
            let segs = if b.fine() { 18 } else { 6 };
            for (z, r, lean) in [(1.0f32, 1.4f32, 0.0f32), (1.9, 1.85, 180.0)] {
                let a = lean.to_radians();
                let axis = v3(a.cos() * 0.12, a.sin() * 0.12, 1.0);
                let c = RADAR + Vec3::Z * z;
                dark_plate(b);
                hoop_on(b, c, axis, r, 0.36, 0.24, segs);
                if b.mid() {
                    collar(b, c, Vec3::Z, 0.45, 0.3);
                }
                if b.fine() {
                    for k in 0..3 {
                        let t = (k as f32 * 120.0 + lean + 60.0).to_radians();
                        let dir = v3(t.cos(), t.sin(), 0.0);
                        metal(b);
                        b.cylinder_between(c + dir * 0.4, c + dir * (r * 0.62), 0.09, 0.07, 5);
                        b.paint(GLOW_LASER);
                        b.cylinder_between(
                            c + dir * (r * 0.62),
                            c + dir * (r * 0.62 + 0.12),
                            0.07,
                            0.04,
                            5,
                        );
                    }
                }
            }
            b.paint(TEAM);
            hoop_on(
                b,
                RADAR + v3(-0.02, 0.0, 2.04),
                v3(-0.12, 0.0, 1.0),
                1.85,
                0.24,
                0.02,
                segs,
            );
            b.paint(GLOW_LASER);
            let sides = b.sides(6);
            b.prism(RADAR + Vec3::Z * 2.5, sides, 0.3, 0.12, 0.2);
        });
    }
    main_gun(b, seat);
    flak_house(b);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_model, material};

    const RADIUS: f32 = 15.5;
    const HEIGHT: f32 = 9.0;

    /// Floats with its keel under the waterline, fits its radius and height,
    /// keeps to its budget, wears the owner's colour and dark plate at every level, the
    /// turret reaches the repeater's muzzle, the gun house the organ's, and the radar turns.
    fn fits(key: &str) {
        let model = build_model(key).expect(key);
        let tris = |lod: usize| model.lods[lod].indices.len() / 3;
        let (full, mid, coarse) = (tris(0), tris(1), tris(2));
        assert!((250..=4500).contains(&full), "{key}: {full} triangles");
        assert!(
            mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
            "{key}: {full}/{mid}/{coarse}"
        );
        assert_eq!(model.houses.len(), 1, "{key}: one gun house");
        assert_eq!(model.houses[0].weapon, 1, "{key}: the flak's house");
        assert!(Vec3::from(model.houses[0].pivot).distance(FLAK) < 1e-3);
        for (lod, mesh) in model.lods.iter().enumerate() {
            let name = format!("{key} lod{lod}");
            let top = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
            assert!(
                (HEIGHT * 0.8..=HEIGHT * 1.25).contains(&top),
                "{name}: top {top}"
            );
            let keel = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::min);
            assert!((-4.5..-0.5).contains(&keel), "{name}: keel {keel}");
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(0.0, f32::max);
            assert!(
                (RADIUS * 0.75..=RADIUS * 1.3).contains(&reach),
                "{name}: reach {reach}"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == material::TEAM && v.normal[2] > 0.5),
                "{name}: no upward team colour"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == material::PLATING_DARK),
                "{name}: no dark plate"
            );
            assert!(
                !mesh
                    .vertices
                    .iter()
                    .any(|v| v.material == material::GLOW || v.material == material::GLOW_ORANGE),
                "{name}: ARC's light"
            );
            let near = |p: Vec3, f: &dyn Fn(&crate::MeshVertex) -> bool| {
                mesh.vertices
                    .iter()
                    .filter(|v| f(v))
                    .map(|v| Vec3::from(v.pos).distance(p))
                    .fold(f32::MAX, f32::min)
            };
            let turret = near(MUZZLE, &|v| v.part == part::TURRET);
            assert!(turret < 0.4, "{name}: turret {turret} m from the muzzle");
            let past = mesh
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET)
                .map(|v| v.pos[0])
                .fold(f32::MIN, f32::max);
            assert!(past <= MUZZLE.x + 0.5, "{name}: turret reaches {past}");
            let house = near(FLAK_MUZZLE, &|v| v.rig & rig::LIMB_MASK == rig::HOUSE_FIRST);
            assert!(
                house < 0.4,
                "{name}: gun house {house} m from the flak muzzle"
            );
            if lod < 2 {
                assert!(
                    mesh.vertices.iter().any(|v| v.part == part::SPINNER),
                    "{name}: no radar"
                );
            }
        }
    }

    #[test]
    fn the_frigate_fits() {
        fits("regency_frigate");
    }

    /// The unit file's guns and size are the model's own.
    #[test]
    fn the_unit_files_guns_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t1_frigate").unwrap());
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        assert_eq!(bp.visual.mesh, "regency_frigate");
        assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
        assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
        assert!(bp.radar.to_f32() > 0.0, "it carries radar");
        let (gun, flak) = (&bp.weapons[0], &bp.weapons[1]);
        assert!(!gun.mount && flak.mount);
        assert!(v(gun.muzzle).distance(MUZZLE) < 1e-3);
        assert!(v(gun.pivot.unwrap()).distance(GUN) < 1e-3);
        assert!(v(flak.muzzle).distance(FLAK_MUZZLE) < 1e-3);
        assert!(v(flak.pivot.unwrap()).distance(FLAK) < 1e-3);
    }
}
