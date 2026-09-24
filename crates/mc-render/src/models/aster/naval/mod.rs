//! Aster warships: the Skiff patrol boat, the Pike frigate, the Barracuda
//! submarine, and the Hydrophone sonar buoy moored in open water.
//!
//! A hull's origin is its waterline: `height` is what stands above the water,
//! and the keel is drawn at negative z. Hulls are lofted through cross-sections
//! (keel, chine, knuckle, deck edge) and painted by the waterline in the
//! shader (`pattern::HULL`), so the antifouling, boot-top, strakes, draught
//! marks and hull number cost no triangles. Nothing here is running gear:
//! `part::LOCOMOTION` is lifted on water by the vertex shader, and a hull must
//! stay where it floats. Tech 1 is unlit: dark bores, no emitters, except the
//! sonar buoy's red obstruction lamp, as on the radar mast (and its tech 3
//! transducer band, the one emitter it earns).

use glam::Vec3;

use super::parts::*;
use crate::models::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

// The rest of the roster, a file each (docs/NAVY.md). Tech 2 and 3 hulls earn emitters,
// and their guns turn on houses of their own (`MeshBuilder::with_house`).
mod aa_cruiser;
mod battleship;
mod carrier;
mod destroyer;
mod missile_ship;
mod salvage_boat;
mod shield_boat;
mod submarine_hunter;
mod submarine_strategic;

pub fn salvage_boat(b: &mut MeshBuilder, _tech: u8) {
    salvage_boat::build(b);
}
pub fn destroyer(b: &mut MeshBuilder, _tech: u8) {
    destroyer::build(b);
}
pub fn aa_cruiser(b: &mut MeshBuilder, _tech: u8) {
    aa_cruiser::build(b);
}
pub fn missile_ship(b: &mut MeshBuilder, _tech: u8) {
    missile_ship::build(b);
}
pub fn submarine_hunter(b: &mut MeshBuilder, _tech: u8) {
    submarine_hunter::build(b);
}
pub fn shield_boat(b: &mut MeshBuilder, _tech: u8) {
    shield_boat::build(b);
}
pub fn battleship(b: &mut MeshBuilder, _tech: u8) {
    battleship::build(b);
}
pub fn carrier(b: &mut MeshBuilder, _tech: u8) {
    carrier::build(b);
}
pub fn submarine_strategic(b: &mut MeshBuilder, _tech: u8) {
    submarine_strategic::build(b);
}

// ---- hulls -----------------------------------------------------------------

/// One cross-section of a hull at `x`, mirrored about y = 0: the keel's z, then
/// (z, half beam) at the chine, where the bottom turns up into the side, at the
/// knuckle, where the side flares out, and at the deck edge. A stem has no beam.
#[derive(Clone, Copy)]
pub(super) struct Station {
    pub(super) x: f32,
    pub(super) keel: f32,
    pub(super) chine: [f32; 2],
    pub(super) knuckle: [f32; 2],
    pub(super) deck: [f32; 2],
}

pub(super) const fn station(x: f32, keel: f32, chine: [f32; 2], knuckle: [f32; 2], deck: [f32; 2]) -> Station {
    Station { x, keel, chine, knuckle, deck }
}

/// Lofts a hull through `stations`, stern first, painted by the waterline. The
/// coarse level keeps only the `coarse` stations, as a V from the keel to the deck edges.
pub(super) fn hull(b: &mut MeshBuilder, stations: &[Station], coarse: &[usize]) {
    let ring = |s: &Station, simple: bool| -> Vec<Vec3> {
        let p = |y: f32, z: f32| v3(s.x, y, z);
        if simple {
            return vec![p(0.0, s.keel), p(-s.deck[1], s.deck[0]), p(s.deck[1], s.deck[0])];
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
    b.paint(PLATING).pattern(pattern::HULL);
    b.loft(&rings, true, true);
}

/// The deck edge (z, half beam) at `x`, between stations.
pub(super) fn deck_at(stations: &[Station], x: f32) -> (f32, f32) {
    let i = stations
        .windows(2)
        .position(|w| x <= w[1].x)
        .unwrap_or(stations.len() - 2);
    let (a, c) = (stations[i], stations[i + 1]);
    let t = ((x - a.x) / (c.x - a.x)).clamp(0.0, 1.0);
    (
        a.deck[0] + (c.deck[0] - a.deck[0]) * t,
        a.deck[1] + (c.deck[1] - a.deck[1]) * t,
    )
}

/// A walkway laid on the deck from `x0` to `x1`, `inset` in from the deck edge:
/// non-skid inside a white margin, following the sheer.
pub(super) fn walkway(b: &mut MeshBuilder, stations: &[Station], x0: f32, x1: f32, inset: f32) {
    let mut xs = vec![x0];
    xs.extend(stations.iter().map(|s| s.x).filter(|&x| x > x0 + 0.3 && x < x1 - 0.3));
    xs.push(x1);
    let rings: Vec<Vec<Vec3>> = xs
        .iter()
        .map(|&x| {
            let (z, half) = deck_at(stations, x);
            let w = (half - inset).max(0.05);
            vec![v3(x, -w, z - 0.02), v3(x, w, z - 0.02), v3(x, w, z + 0.05), v3(x, -w, z + 0.05)]
        })
        .collect();
    b.paint(PLATING).pattern(pattern::WALKWAY);
    b.loft(&rings, true, true);
}

/// A dark rubbing strake along the deck edge from `x0` to `x1`, both sides.
pub(super) fn rub_rail(b: &mut MeshBuilder, stations: &[Station], x0: f32, x1: f32, size: f32) {
    let mut xs = vec![x0];
    xs.extend(stations.iter().map(|s| s.x).filter(|&x| x > x0 + 0.3 && x < x1 - 0.3));
    xs.push(x1);
    b.paint(ACCENT);
    b.mirror_y(|b| {
        for w in xs.windows(2) {
            let (za, ha) = deck_at(stations, w[0]);
            let (zc, hc) = deck_at(stations, w[1]);
            b.beam(
                v3(w[0], ha + size * 0.2, za - size * 0.9),
                v3(w[1], hc + size * 0.2, zc - size * 0.9),
                v2(size * 0.7, size),
                v2(size * 0.7, size),
            );
        }
    });
}

/// Guard rails along both deck edges from `x0` to `x1`: stanchions and a top rail.
pub(super) fn rails(b: &mut MeshBuilder, stations: &[Station], x0: f32, x1: f32, height: f32, inset: f32) {
    let posts = ((x1 - x0) / 3.0).ceil().max(1.0) as usize;
    b.paint(METAL);
    b.mirror_y(|b| {
        let at = |x: f32| {
            let (z, half) = deck_at(stations, x);
            v3(x, half - inset, z)
        };
        for k in 0..=posts {
            let p = at(x0 + (x1 - x0) * k as f32 / posts as f32);
            b.cylinder_between(p, p + Vec3::Z * height, 0.035, 0.035, 4);
        }
        for k in 0..posts {
            let (p, q) = (
                at(x0 + (x1 - x0) * k as f32 / posts as f32),
                at(x0 + (x1 - x0) * (k + 1) as f32 / posts as f32),
            );
            b.beam(p + Vec3::Z * height, q + Vec3::Z * height, v2(0.05, 0.05), v2(0.05, 0.05));
        }
    });
}

/// A mooring bollard: a pair of short posts on a plate.
pub(super) fn bollard(b: &mut MeshBuilder, at: Vec3, size: f32) {
    b.paint(ACCENT);
    b.cuboid(at + v3(0.0, 0.0, size * 0.08), v3(size * 1.6, size * 0.7, size * 0.16));
    for dx in [-0.45, 0.45] {
        let p = at + v3(dx * size, 0.0, size * 0.16);
        b.cylinder_between(p, p + Vec3::Z * size * 0.55, size * 0.2, size * 0.26, 6);
    }
}

/// A liferaft canister in its cradle, lying along x.
pub(super) fn liferaft(b: &mut MeshBuilder, at: Vec3, length: f32, radius: f32) {
    b.paint(ACCENT);
    b.cuboid(at + v3(0.0, 0.0, radius * 0.25), v3(length * 0.7, radius * 1.6, radius * 0.5));
    b.paint(PLATING);
    let c = at + Vec3::Z * (radius * 1.05);
    b.cylinder_between(c - Vec3::X * (length * 0.5), c + Vec3::X * (length * 0.5), radius, radius, 6);
}

/// A plain gun tube from `breech` to `muzzle`: gunmetal, a dark jacket over the
/// back half, a short flash hider and a dark bore. Nothing lit at tech 1.
pub(super) fn gun_tube(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, radius: f32) {
    let r = radius;
    let d = (muzzle - breech).normalize();
    let length = (muzzle - breech).length();
    let at = |t: f32| breech + d * (length * t);
    if b.coarse() {
        b.paint(METAL);
        b.beam(breech, muzzle, v2(r * 2.0, r * 2.0), v2(r * 1.8, r * 1.8));
        return;
    }
    let sides = b.sides(6);
    b.paint(METAL);
    b.cylinder_between(breech, muzzle - d * 0.02, r * 1.05, r * 0.9, sides);
    b.paint(ACCENT);
    b.cylinder_between(at(0.0), at(0.45), r * 1.7, r * 1.5, sides);
    b.cylinder_between(at(0.88), muzzle, r * 1.3, r * 1.45, sides);
    if b.fine() && r > 0.08 {
        b.paint(METAL);
        for t in [0.18, 0.34] {
            b.cylinder_between(at(t - 0.02), at(t + 0.02), r * 1.8, r * 1.8, 6);
        }
    }
}

/// Red obstruction lamp on a short stalk (the radar mast's).
pub(super) fn beacon(b: &mut MeshBuilder, at: Vec3) {
    b.paint(ACCENT);
    b.prism(at, 6, 0.12, 0.08, 0.3);
    b.paint(GLOW_RED);
    if b.fine() {
        b.spheroid(at + Vec3::Z * 0.42, Vec3::splat(0.18), 6, 2);
    } else {
        b.cuboid(at + Vec3::Z * 0.42, Vec3::splat(0.32));
    }
}

// ---- Skiff: attack boat ----------------------------------------------------
//
// A 12 m fast patrol boat: a deep-V hull with a hard chine and a flared bow, a
// raked wheelhouse with a dark wraparound screen, a radar arch, twin outboard
// drives on the transom and a three-barrel rotary gun on a ring at the bow.

const SKIFF_HULL: [Station; 5] = [
    station(-5.6, -0.5, [-0.22, 1.3], [0.7, 1.5], [1.45, 1.5]),
    station(-2.0, -0.8, [-0.3, 1.38], [0.7, 1.6], [1.5, 1.62]),
    station(1.8, -0.8, [-0.28, 1.28], [0.72, 1.55], [1.55, 1.56]),
    station(4.4, -0.52, [-0.05, 0.8], [0.88, 1.16], [1.8, 1.12]),
    station(6.2, 0.55, [0.62, 0.0], [1.0, 0.0], [2.1, 0.0]),
];
/// Ring mount: the gun's yaw axis and its trunnion (`pivot` in the unit file).
const SKIFF_GUN: Vec3 = Vec3::new(3.3, 0.0, 3.1);
const SKIFF_MUZZLE: Vec3 = Vec3::new(4.6, 0.0, 3.1);

/// The Skiff's gun: a motor and receiver on the cradle, three barrels in two clamps
/// that spin about the bore line, a belt box on the left and the ejection port on the
/// right, where the casings come out (`casings` in the unit file is the port's distance
/// back from the muzzle).
fn skiff_minigun(b: &mut MeshBuilder) {
    let z = SKIFF_GUN.z;
    let (back, front) = (2.66, 3.34);
    b.paint(ACCENT);
    b.cylinder_between(v3(back, 0.0, z), v3(front, 0.0, z), 0.13, 0.12, b.sides(8));
    if b.mid() {
        // Motor cap, the port on the right, the belt box and its chute on the left.
        b.paint(METAL);
        b.cylinder_between(v3(back - 0.12, 0.0, z), v3(back, 0.0, z), 0.08, 0.11, b.sides(8));
        b.paint(PLATING_DARK);
        b.block(v3(3.0, -0.15, z - 0.06), v3(3.2, -0.11, z + 0.05));
        b.paint(METAL);
        b.block(v3(2.8, 0.16, z - 0.24), v3(3.16, 0.42, z + 0.02));
        b.beam(v3(2.98, 0.17, z - 0.02), v3(3.06, 0.11, z + 0.02), v2(0.1, 0.05), v2(0.1, 0.05));
    }
    b.with_spin(v3(0.0, 0.0, z), |b| {
        b.paint(METAL);
        if b.mid() {
            for i in 0..3 {
                let a = i as f32 * std::f32::consts::TAU / 3.0 + std::f32::consts::FRAC_PI_2;
                let off = v3(0.0, a.cos() * 0.058, a.sin() * 0.058);
                b.cylinder_between(v3(front, 0.0, z) + off, SKIFF_MUZZLE + off, 0.03, 0.027, 6);
            }
            b.paint(ACCENT);
            for x in [3.86, SKIFF_MUZZLE.x - 0.1] {
                b.cylinder_between(v3(x, 0.0, z), v3(x + 0.07, 0.0, z), 0.1, 0.1, b.sides(6));
            }
            // The spindle down the middle.
            b.paint(PLATING_DARK);
            b.cylinder_between(v3(front, 0.0, z), v3(SKIFF_MUZZLE.x - 0.1, 0.0, z), 0.025, 0.025, 4);
        } else {
            // From further off the barrels read as one tube.
            b.cylinder_between(v3(front, 0.0, z), SKIFF_MUZZLE, 0.09, 0.08, 6);
        }
    });
}

pub fn attack_boat(b: &mut MeshBuilder, _tech: u8) {
    hull(b, &SKIFF_HULL, &[0, 2, 4]);
    let ring_z = deck_at(&SKIFF_HULL, SKIFF_GUN.x).0 + 0.02;

    // Wheelhouse: a white lower house, a dark raked screen all round, a white roof.
    let plan = chamfered_rect(v2(2.0, 1.12), 0.45);
    let house = v3(-0.5, 0.0, 0.0);
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(v3(-0.6, 0.0, 1.45), v2(4.0, 2.2), v2(3.0, 1.8), 1.8, v2(-0.3, 0.0));
        team_panel(b, v3(-0.9, 0.0, 3.25), v2(1.8, 1.2));
    } else {
        b.at(house, |b| {
            b.loft_z(&plan, &[Section::new(1.45, 1.0), Section::new(2.35, 0.98)]);
            b.paint(GLASS);
            b.loft_z(
                &plan,
                &[
                    Section::new(2.35, 0.98),
                    Section::scaled(2.98, 0.84, 0.86).shifted(-0.3, 0.0),
                ],
            );
            b.paint(PLATING);
            b.loft_z(
                &plan,
                &[
                    Section::scaled(2.98, 0.88, 0.9).shifted(-0.26, 0.0),
                    Section::scaled(3.1, 0.84, 0.86).shifted(-0.3, 0.0),
                ],
            );
        });
        team_panel(b, v3(-0.75, 0.0, 3.1), v2(1.9, 1.3));
        // Dark trim at the foot of the house: the frame the plating hangs on.
        b.paint(ACCENT);
        b.at(house, |b| {
            b.loft_z(&chamfered_rect(v2(2.06, 1.18), 0.47), &[Section::new(1.45, 1.0), Section::new(1.68, 1.0)])
        });
    }

    // Radar arch over the back of the house, a small scanner turning on it.
    b.set_spinner_pivot(v3(-2.2, 0.0, 3.92));
    if b.mid() {
        b.paint(ACCENT);
        b.mirror_y(|b| {
            b.beam(v3(-1.95, 0.85, 3.08), v3(-2.2, 0.55, 3.86), v2(0.16, 0.22), v2(0.13, 0.18));
        });
        b.beam(v3(-2.2, -0.62, 3.86), v3(-2.2, 0.62, 3.86), v2(0.2, 0.14), v2(0.2, 0.14));
    }
    b.with_part(part::SPINNER, |b| {
        if b.coarse() {
            return;
        }
        b.paint(ACCENT);
        b.prism(v3(-2.2, 0.0, 3.92), b.sides(6), 0.14, 0.12, 0.16);
        b.paint(PLATING);
        b.beam(v3(-2.2, -0.62, 4.16), v3(-2.2, 0.62, 4.16), v2(0.12, 0.16), v2(0.12, 0.16));
    });

    // The bow gun: a ring, a post, a split shield, and the rotary gun that elevates on its cradle.
    b.set_turret_pivot(v3(SKIFF_GUN.x, 0.0, ring_z));
    b.set_arm_pivot(SKIFF_GUN);
    b.with_part(part::TURRET, |b| {
        if b.coarse() {
            b.paint(ACCENT);
            b.frustum_open(v3(SKIFF_GUN.x, 0.0, ring_z), v2(0.9, 0.9), v2(0.3, 0.5), 3.0 - ring_z, v2(0.0, 0.0));
            b.paint(METAL);
            b.face(&[
                v3(2.9, -0.07, SKIFF_GUN.z),
                v3(SKIFF_MUZZLE.x, -0.06, SKIFF_GUN.z),
                v3(SKIFF_MUZZLE.x, 0.06, SKIFF_GUN.z),
                v3(2.9, 0.07, SKIFF_GUN.z),
            ]);
            return;
        }
        b.paint(ACCENT);
        b.prism(v3(SKIFF_GUN.x, 0.0, ring_z), b.sides(8), 0.55, 0.5, 0.14);
        b.paint(METAL);
        b.cylinder_between(v3(SKIFF_GUN.x, 0.0, ring_z + 0.14), v3(SKIFF_GUN.x, 0.0, 2.95), 0.1, 0.08, b.sides(6));
        b.with_limb(crate::models::rig::ARM_GUN, |b| skiff_minigun(b));
        if b.mid() {
            // The shield: two raked wings either side of the barrel.
            b.paint(PLATING);
            b.mirror_y(|b| {
                b.loft(
                    &[
                        vec![v3(3.62, 0.1, 2.62), v3(3.62, 0.6, 2.62), v3(3.52, 0.62, 3.4), v3(3.52, 0.1, 3.34)],
                        vec![v3(3.7, 0.1, 2.62), v3(3.7, 0.6, 2.62), v3(3.6, 0.62, 3.4), v3(3.6, 0.1, 3.34)],
                    ],
                    true,
                    true,
                );
            });
            b.paint(ACCENT);
            b.block(v3(3.25, -0.62, 2.55), v3(3.66, 0.62, 2.66));
        }
    });

    if b.coarse() {
        return;
    }
    walkway(b, &SKIFF_HULL, 1.55, 5.7, 0.22);
    walkway(b, &SKIFF_HULL, -5.45, -2.55, 0.18);
    rub_rail(b, &SKIFF_HULL, -5.6, 6.0, 0.16);

    // Twin outboard drives on the transom: dark cowlings with white caps, legs into the water.
    b.mirror_y(|b| {
        let y = 0.68;
        b.paint(ACCENT);
        b.chamfered_box(v3(-6.0, y, 0.85), v3(0.72, 0.5, 0.95), 0.12);
        b.paint(PLATING);
        b.chamfered_box(v3(-6.02, y, 1.38), v3(0.66, 0.44, 0.14), 0.1);
        b.paint(METAL);
        b.beam(v3(-6.0, y, 0.4), v3(-6.05, y, -0.55), v2(0.12, 0.24), v2(0.1, 0.2));
        if b.fine() {
            b.block(v3(-6.3, y - 0.2, -0.15), v3(-5.75, y + 0.2, -0.1));
            b.paint(ACCENT);
            b.cylinder_between(v3(-6.0, y, -0.55), v3(-6.3, y, -0.55), 0.09, 0.08, 6);
        }
    });

    if !b.fine() {
        return;
    }
    // Screen wipers and a sun visor lip, a hatch on the foredeck, the anchor at the stem.
    b.paint(ACCENT);
    b.block(v3(1.0, -0.9, 3.08), v3(1.15, 0.9, 3.12));
    b.paint(PLATING);
    b.plate(v3(4.35, 0.0, deck_at(&SKIFF_HULL, 4.35).0 + 0.05), v2(0.7, 0.7), 0.06, 0.03);
    b.paint(METAL);
    b.cylinder_between(v3(5.85, 0.0, 1.78), v3(6.25, 0.0, 1.62), 0.06, 0.06, 4);
    // Pulpit rail round the bow, guard rails aft.
    rails(b, &SKIFF_HULL, 4.3, 5.8, 0.55, 0.12);
    rails(b, &SKIFF_HULL, -5.3, -2.7, 0.5, 0.12);
    for x in [5.0, -5.0] {
        let (z, half) = deck_at(&SKIFF_HULL, x);
        b.mirror_y(|b| bollard(b, v3(x, half - 0.3, z + 0.05), 0.28));
    }
    // Fenders hung over the side.
    b.paint(TREAD);
    b.mirror_y(|b| {
        for x in [-3.2, 0.4] {
            let (z, half) = deck_at(&SKIFF_HULL, x);
            b.cylinder_between(v3(x, half + 0.1, z - 0.15), v3(x, half + 0.1, z - 0.6), 0.14, 0.14, 6);
        }
    });
    // Liferaft on the aft deck, whips on the arch, a vent box on each side of the house.
    liferaft(b, v3(-3.9, 0.0, deck_at(&SKIFF_HULL, -3.9).0 + 0.05), 0.9, 0.26);
    whip(b, v3(-2.2, 0.5, 3.93), 0.9, 0.06);
    whip(b, v3(-2.2, -0.5, 3.93), 0.7, 0.1);
    b.mirror_y(|b| {
        vent(b, v3(0.2, 0.0, 3.1) + v3(0.0, 0.62, 0.0), v2(0.6, 0.3), 3, METAL);
    });
}

// ---- Pike: frigate ---------------------------------------------------------
//
// A 30 m frigate: a long flared hull with a raked bow and a knuckle, faceted
// superstructure with tumblehome, a dark wraparound bridge screen under a
// pyramidal integrated mast with its search radar turning on top, a stealth
// gunhouse forward, a twin AA mount on its own on the hangar roof, and a working
// quarterdeck aft.

const FRIGATE_HULL: [Station; 7] = [
    station(-14.6, -1.2, [-0.95, 2.4], [0.9, 2.95], [2.42, 3.02]),
    station(-10.0, -2.4, [-1.6, 2.62], [0.9, 3.25], [2.45, 3.32]),
    station(-3.0, -3.0, [-1.9, 2.72], [1.0, 3.36], [2.5, 3.42]),
    station(4.0, -3.0, [-1.9, 2.52], [1.1, 3.22], [2.7, 3.32]),
    station(9.0, -2.6, [-1.6, 1.82], [1.3, 2.66], [3.05, 2.9]),
    station(13.0, -1.8, [-1.0, 0.82], [1.7, 1.72], [3.5, 2.02]),
    station(15.6, 0.9, [1.2, 0.0], [2.3, 0.0], [3.95, 0.0]),
];
/// The deck gun's yaw axis and trunnion (`pivot` in the unit file), and its muzzle.
const FRIGATE_GUN: Vec3 = Vec3::new(8.8, 0.0, 4.1);
const FRIGATE_MUZZLE: Vec3 = Vec3::new(13.4, 0.0, 4.1);
/// The twin AA mount's trunnion, and where its barrels end (y ±0.35).
const FRIGATE_AA: Vec3 = Vec3::new(-6.4, 0.0, 8.1);
const FRIGATE_AA_MUZZLE_X: f32 = -4.2;

pub fn frigate(b: &mut MeshBuilder, _tech: u8) {
    hull(b, &FRIGATE_HULL, &[0, 3, 6]);
    let gun_deck = deck_at(&FRIGATE_HULL, FRIGATE_GUN.x).0;

    // Deckhouse and hangar: a dark plinth, white walls drawn in as they rise, a dark coping.
    let house = chamfered_rect(v2(5.15, 2.62), 0.9);
    let house_at = v3(-4.05, 0.0, 0.0);
    // Bridge: a pointed front, a dark screen all round, a white cap.
    let bridge = [[6.0, -1.1], [6.0, 1.1], [4.7, 2.36], [0.2, 2.36], [0.2, -2.36], [4.7, -2.36]];
    let mast_base = v3(1.9, 0.0, 6.7);
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(v3(-1.6, 0.0, 2.4), v2(15.4, 5.0), v2(13.6, 4.2), 4.2, v2(0.4, 0.0));
        b.frustum_open(mast_base - Vec3::Z * 0.2, v2(2.6, 2.8), v2(0.6, 0.6), 4.0, v2(-0.3, 0.0));
        team_panel(b, v3(-5.0, 0.0, 6.6), v2(6.0, 3.0));
    } else {
        b.at(house_at, |b| {
            b.paint(ACCENT);
            b.loft_z(&house, &[Section::new(2.3, 1.02), Section::new(2.9, 1.02)]);
            b.paint(PLATING);
            b.loft_z(&house, &[Section::new(2.9, 1.0), Section::scaled(5.45, 0.98, 0.88)]);
            b.paint(ACCENT);
            b.loft_z(&house, &[Section::scaled(5.45, 0.99, 0.9), Section::scaled(5.62, 0.99, 0.9)]);
        });
        b.paint(PLATING);
        b.loft_z(&bridge, &[Section::new(2.5, 1.0), Section::scaled(5.6, 0.97, 0.94)]);
        b.paint(GLASS);
        b.loft_z(&bridge, &[Section::scaled(5.6, 0.97, 0.94), Section::scaled(6.3, 0.9, 0.86)]);
        b.paint(PLATING);
        b.loft_z(&bridge, &[Section::scaled(6.3, 0.92, 0.88), Section::scaled(6.7, 0.88, 0.84)]);
        // The integrated mast: a faceted pyramid off the bridge roof.
        b.loft_z(
            &chamfered_rect(v2(1.35, 1.4), 0.35),
            &[
                Section::new(6.7, 1.0).shifted(mast_base.x, 0.0),
                Section::scaled(8.6, 0.72, 0.7).shifted(mast_base.x - 0.25, 0.0),
                Section::scaled(10.2, 0.42, 0.4).shifted(mast_base.x - 0.45, 0.0),
            ],
        );
        team_panel(b, v3(-1.0, 0.0, 5.62), v2(2.6, 3.2));
        // The team's band down the hangar roof.
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(-4.3, 0.0, 5.62), v2(3.4, 4.0), 0.08, 0.03);
    }

    // Search radar on the masthead: a pedestal and a flat array, always turning.
    let radar = v3(mast_base.x - 0.45, 0.0, 10.2);
    b.set_spinner_pivot(radar);
    b.with_part(part::SPINNER, |b| {
        if b.coarse() {
            return;
        }
        b.paint(ACCENT);
        b.prism(radar, b.sides(8), 0.32, 0.26, 0.3);
        b.paint(PLATING);
        b.beam(radar + v3(0.0, -1.35, 0.78), radar + v3(0.0, 1.35, 0.78), v2(0.2, 0.9), v2(0.2, 0.9));
        if b.mid() {
            b.paint(ACCENT);
            b.beam(radar + v3(0.12, -1.25, 0.78), radar + v3(0.12, 1.25, 0.78), v2(0.06, 0.72), v2(0.06, 0.72));
            b.beam(radar + v3(0.0, 0.0, 0.3), radar + v3(0.0, 0.0, 0.34), v2(0.4, 0.3), v2(0.4, 0.3));
        }
    });

    // Deck gun: a ring, a faceted gunhouse, the long rifle elevating in its slot.
    b.set_turret_pivot(v3(FRIGATE_GUN.x, 0.0, gun_deck));
    b.set_arm_pivot(FRIGATE_GUN);
    let breech = v3(9.2, 0.0, FRIGATE_GUN.z);
    b.set_recoil(breech, FRIGATE_MUZZLE, 0.5);
    b.with_part(part::TURRET, |b| {
        let house_x = FRIGATE_GUN.x - 0.15;
        b.paint(PLATING);
        if b.coarse() {
            b.frustum_open(v3(house_x, 0.0, gun_deck), v2(3.0, 2.4), v2(1.9, 1.5), 4.6 - gun_deck, v2(-0.3, 0.0));
            b.paint(METAL);
            b.face(&[
                v3(9.6, -0.15, FRIGATE_GUN.z),
                v3(FRIGATE_MUZZLE.x, -0.12, FRIGATE_GUN.z),
                v3(FRIGATE_MUZZLE.x, 0.12, FRIGATE_GUN.z),
                v3(9.6, 0.15, FRIGATE_GUN.z),
            ]);
            return;
        }
        b.paint(ACCENT);
        b.prism(v3(FRIGATE_GUN.x, 0.0, gun_deck - 0.05), b.sides(10), 1.4, 1.35, 0.22);
        b.paint(PLATING);
        b.at(v3(house_x, 0.0, 0.0), |b| {
            b.loft_z(
                &turret_plan(3.3, 2.5),
                &[
                    Section::new(gun_deck + 0.15, 0.95),
                    Section::new(gun_deck + 0.6, 1.0),
                    Section::scaled(4.62, 0.66, 0.66).shifted(-0.25, 0.0),
                ],
            );
        });
        b.with_limb(crate::models::rig::ARM_GUN, |b| {
            b.with_recoil(|b| {
                gun_tube(b, breech, FRIGATE_MUZZLE, 0.11);
                if b.mid() {
                    // The white blast sleeve at the root of the barrel.
                    b.paint(PLATING);
                    b.cylinder_between(v3(9.5, 0.0, FRIGATE_GUN.z), v3(10.9, 0.0, FRIGATE_GUN.z), 0.26, 0.2, b.sides(8));
                }
            });
            b.paint(ACCENT);
            b.block(v3(9.3, -0.32, FRIGATE_GUN.z - 0.34), v3(9.95, 0.32, FRIGATE_GUN.z + 0.3));
        });
        if b.fine() {
            // A hatch on the roof, a vent at the back, lifting eyes.
            b.paint(ACCENT);
            b.plate(v3(house_x - 0.5, 0.45, 4.62), v2(0.6, 0.5), 0.05, 0.02);
            b.block(v3(house_x - 1.62, -0.5, 3.4), v3(house_x - 1.5, 0.5, 3.9));
            b.paint(METAL);
            for y in [-0.6, 0.6] {
                b.cuboid(v3(house_x - 0.1, y, 4.66), v3(0.14, 0.06, 0.1));
            }
        }
    });

    // The AA mount on its tub over the hangar: turns and elevates on its own.
    if b.mid() {
        b.paint(PLATING);
        b.at(v3(FRIGATE_AA.x, 0.0, 0.0), |b| {
            b.loft_z(&ngon(b.sides(8), 1.5), &[Section::new(5.5, 1.1), Section::new(7.45, 0.92)]);
        });
        b.paint(ACCENT);
        b.at(v3(FRIGATE_AA.x, 0.0, 0.0), |b| {
            // A dark collar round the tub's lip, standing proud of it: no face shares its plane.
            b.loft_z(&ngon(b.sides(8), 1.5), &[Section::new(7.28, 0.96), Section::new(7.52, 0.96)]);
        });
    }
    b.with_mount(FRIGATE_AA, 0.18, |b| {
        let (x, z) = (FRIGATE_AA.x, FRIGATE_AA.z);
        if b.coarse() {
            return;
        }
        if b.mid() {
            b.paint(METAL);
            // The turning ring sits just clear of the collar, so no yaw lays it in the collar's plane.
            b.prism(v3(x, 0.0, 7.54), b.sides(8), 0.95, 0.9, 0.14);
        }
        b.paint(PLATING);
        b.at(v3(x - 0.15, 0.0, 0.0), |b| {
            b.loft_z(
                &chamfered_rect(v2(0.85, 0.62), 0.28),
                &[Section::new(7.66, 1.0), Section::new(8.25, 1.0), Section::scaled(8.62, 0.8, 0.82).shifted(-0.08, 0.0)],
            );
        });
        b.with_recoil(|b| {
            for y in [-0.35, 0.35] {
                gun_tube(b, v3(x + 0.45, y, z), v3(FRIGATE_AA_MUZZLE_X, y, z), 0.06);
            }
        });
        if b.fine() {
            // Ammunition feeds each side, a sight on top.
            b.paint(ACCENT);
            b.mirror_y(|b| b.block(v3(x - 0.7, 0.58, 7.75), v3(x + 0.2, 0.78, 8.3)));
            b.block(v3(x - 0.1, -0.12, 8.58), v3(x + 0.25, 0.12, 8.8));
            b.paint(GLASS);
            b.block(v3(x + 0.25, -0.1, 8.65), v3(x + 0.28, 0.1, 8.76));
        }
    });

    if b.coarse() {
        return;
    }
    walkway(b, &FRIGATE_HULL, -14.35, 14.6, 0.2);
    rub_rail(b, &FRIGATE_HULL, -14.6, 15.0, 0.22);
    // The quarterdeck: a towed-array winch on its bed and an A-frame over the transom
    // to stream it, depth-charge rails down each quarter. A working stern, no flight deck.
    let qz = deck_at(&FRIGATE_HULL, -11.5).0 + 0.05;
    b.paint(ACCENT);
    b.block(v3(-12.3, -1.2, qz), v3(-10.5, 1.2, qz + 0.3));
    b.mirror_y(|b| b.block(v3(-11.8, 0.95, qz + 0.3), v3(-11.0, 1.1, qz + 1.35)));
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(-11.4, -0.95, qz + 0.95), v3(-11.4, 0.95, qz + 0.95), 0.62, 0.62, b.sides(10));
    b.paint(PLATING);
    b.mirror_y(|b| b.beam(v3(-14.3, 1.5, qz), v3(-14.0, 0.9, qz + 2.3), v2(0.26, 0.26), v2(0.2, 0.2)));
    b.beam(v3(-14.0, -1.0, qz + 2.3), v3(-14.0, 1.0, qz + 2.3), v2(0.28, 0.28), v2(0.28, 0.28));
    b.paint(METAL);
    b.mirror_y(|b| b.beam(v3(-14.3, 2.3, qz), v3(-9.9, 2.3, qz), v2(0.5, 0.12), v2(0.5, 0.12)));
    // The exhaust: a raked, faceted stack with a black cap.
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2(0.95, 1.1), 0.3),
        &[Section::new(5.5, 1.0).shifted(-0.7, 0.0), Section::scaled(7.9, 0.72, 0.8).shifted(-1.15, 0.0)],
    );
    b.paint(ACCENT);
    b.loft_z(
        &chamfered_rect(v2(0.95, 1.1), 0.3),
        &[Section::scaled(7.9, 0.73, 0.81).shifted(-1.15, 0.0), Section::scaled(8.15, 0.66, 0.74).shifted(-1.2, 0.0)],
    );
    // Hangar door on the aft face.
    b.paint(ACCENT);
    b.block(v3(-9.32, -1.8, 2.9), v3(-9.12, 1.8, 5.1));
    // A yard across the mast.
    b.paint(METAL);
    b.beam(v3(1.55, -1.25, 9.1), v3(1.55, 1.25, 9.1), v2(0.12, 0.12), v2(0.12, 0.12));
    // A breakwater ahead of the gun.
    b.paint(PLATING);
    b.mirror_y(|b| {
        let z = deck_at(&FRIGATE_HULL, 11.3).0;
        b.beam(v3(11.9, 0.0, z + 0.25), v3(10.9, 1.5, z + 0.25), v2(0.1, 0.5), v2(0.1, 0.5));
    });

    if !b.fine() {
        return;
    }
    // Sensor panels flush in the mast's faces and dark decoy launchers on the bridge wings.
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.beam(v3(2.4, 1.02, 7.9), v3(2.0, 0.74, 9.3), v2(0.9, 0.05), v2(0.7, 0.05));
        b.block(v3(4.1, 1.35, 6.7), v3(4.8, 1.85, 6.95));
        b.paint(METAL);
        for i in 0..3 {
            let p = v3(4.25 + i as f32 * 0.2, 1.6, 6.95);
            b.cylinder_between(p, p + v3(0.12, 0.1, 0.2), 0.07, 0.07, 5);
        }
        b.paint(ACCENT);
    });
    // ESM domes on the masthead corners, whips off the yard.
    b.paint(PLATING);
    b.mirror_y(|b| b.spheroid(v3(1.2, 0.55, 10.25), v3(0.22, 0.22, 0.2), 6, 2));
    whip(b, v3(1.55, 1.15, 9.15), 2.6, 0.05);
    whip(b, v3(1.55, -1.15, 9.15), 2.2, 0.08);
    whip(b, v3(-14.3, 0.0, 2.45), 1.8, 0.1);
    // Liferafts on the hangar roof, vents between them.
    for x in [-3.2, -2.1] {
        b.mirror_y(|b| liferaft(b, v3(x, 1.95, 5.62), 0.9, 0.28));
    }
    b.mirror_y(|b| vent(b, v3(-7.6, 1.55, 5.62), v2(1.2, 0.8), 3, METAL));
    // The anchor: windlass, cables to the hawse pipes, the pipes in the flare.
    b.paint(METAL);
    let wz = deck_at(&FRIGATE_HULL, 12.3).0;
    b.cylinder_between(v3(12.3, -0.7, wz + 0.3), v3(12.3, 0.7, wz + 0.3), 0.28, 0.28, 8);
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.beam(v3(12.3, 0.4, wz + 0.1), v3(13.7, 1.05, wz + 0.02), v2(0.14, 0.1), v2(0.14, 0.1));
        b.cylinder_between(v3(13.8, 1.2, wz + 0.05), v3(14.2, 1.45, wz - 0.4), 0.2, 0.2, 6);
    });
    for x in [6.4, -9.6] {
        let (z, half) = deck_at(&FRIGATE_HULL, x);
        b.mirror_y(|b| bollard(b, v3(x, half - 0.55, z + 0.05), 0.42));
    }
    rails(b, &FRIGATE_HULL, 5.0, 14.4, 0.8, 0.14);
    // The winch's flanges and the cable out to the A-frame's sheave; depth charges on the rails.
    b.paint(METAL);
    for y in [-0.98, 0.98] {
        b.cylinder_between(v3(-11.4, y - 0.04, qz + 0.95), v3(-11.4, y + 0.04, qz + 0.95), 0.8, 0.8, 6);
    }
    b.paint(ACCENT);
    b.cylinder_between(v3(-11.4, 0.0, qz + 1.55), v3(-14.0, 0.0, qz + 2.1), 0.05, 0.05, 4);
    b.cylinder_between(v3(-14.0, -0.2, qz + 2.05), v3(-14.0, 0.2, qz + 2.05), 0.2, 0.2, 5);
    b.mirror_y(|b| {
        for x in [-13.2, -12.4] {
            b.cylinder_between(v3(x, 2.3, qz + 0.12), v3(x, 2.3, qz + 0.72), 0.26, 0.26, 6);
        }
    });
}

// ---- Barracuda: submarine --------------------------------------------------
//
// A 20 m attack submarine: a round pressure hull under a flat casing, both in
// black anechoic tiles, a white faceted sail with its planes and masts, four
// bow tubes behind shutters, cruciform stern planes and a shrouded pumpjet.

/// Centre line of the pressure hull, and its (x, radius) profile, stern first.
const SUB_AXIS_Z: f32 = -0.6;
const SUB_HULL: [(f32, f32); 11] = [
    (-9.4, 0.28),
    (-8.6, 0.62),
    (-7.2, 1.05),
    (-5.5, 1.4),
    (-3.5, 1.5),
    (6.8, 1.5),
    (8.6, 1.45),
    (9.35, 1.3),
    (9.75, 1.0),
    (9.98, 0.55),
    (10.08, 0.1),
];
/// Torpedo tube mouths, as in the unit file.
const SUB_TUBES: [[f32; 3]; 4] = [[9.6, -0.7, -0.8], [9.6, 0.7, -0.8], [9.6, -0.7, -1.6], [9.6, 0.7, -1.6]];

pub(super) fn sub_ring(x: f32, radius: f32, sides: usize) -> Vec<Vec3> {
    ngon(sides, radius)
        .into_iter()
        .map(|[y, z]| v3(x, y, SUB_AXIS_Z + z))
        .collect()
}

pub fn submarine(b: &mut MeshBuilder, _tech: u8) {
    let sides = if b.fine() { 12 } else if b.mid() { 8 } else { 4 };
    let profile: Vec<(f32, f32)> = if b.coarse() {
        vec![(-9.4, 0.3), (-6.0, 1.3), (7.5, 1.5), (10.08, 0.3)]
    } else {
        SUB_HULL.to_vec()
    };
    b.paint(PLATING_DARK).pattern(pattern::TILES);
    let rings: Vec<Vec<Vec3>> = profile.iter().map(|&(x, r)| sub_ring(x, r, sides)).collect();
    b.loft(&rings, true, true);

    // The sail: white, faceted, raked back a little, with the team's colour on top.
    let sail = [[5.7, 0.0], [5.0, 0.46], [2.9, 0.46], [2.2, 0.2], [2.2, -0.2], [2.9, -0.46], [5.0, -0.46]];
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(v3(3.9, 0.0, 0.7), v2(3.4, 0.9), v2(2.8, 0.7), 2.65, v2(-0.25, 0.0));
        team_panel(b, v3(3.65, 0.0, 3.35), v2(1.8, 0.5));
        // The bow tubes' shutters, as one plate.
        b.paint(ACCENT);
        b.face(&[v3(9.62, -0.9, -1.8), v3(9.62, 0.9, -1.8), v3(9.62, 0.9, -0.6), v3(9.62, -0.9, -0.6)]);
        return;
    }
    b.loft_z(
        &sail,
        &[
            Section::scaled(0.7, 1.04, 1.1),
            Section::new(1.2, 1.0),
            Section::scaled(3.35, 0.93, 0.82).shifted(0.02, 0.0),
        ],
    );
    team_panel(b, v3(3.7, 0.0, 3.35), v2(1.7, 0.46));

    // The casing: a flat deck faired down into the hull, tiled like it.
    b.paint(PLATING_DARK).pattern(pattern::TILES);
    let casing = |x: f32, half: f32, top: f32| {
        vec![v3(x, -half * 1.35, -0.1), v3(x, half * 1.35, -0.1), v3(x, half, top), v3(x, -half, top)]
    };
    b.loft(
        &[casing(-7.0, 0.25, 0.75), casing(-5.6, 0.72, 1.1), casing(7.0, 0.72, 1.1), casing(8.9, 0.3, 0.72)],
        true,
        true,
    );

    // Sail planes, and the cruciform stern planes with white tips.
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.beam(v3(4.1, 0.4, 2.45), v3(4.0, 1.8, 2.45), v2(1.0, 0.12), v2(0.7, 0.08));
        b.beam(v3(-7.6, 0.8, SUB_AXIS_Z), v3(-7.9, 2.35, SUB_AXIS_Z), v2(1.3, 0.14), v2(0.8, 0.1));
    });
    b.beam(v3(-7.6, 0.0, 0.2), v3(-7.9, 0.0, 1.45), v2(0.14, 1.3), v2(0.1, 0.8));
    b.beam(v3(-7.6, 0.0, -1.4), v3(-7.9, 0.0, -2.55), v2(0.14, 1.3), v2(0.1, 0.8));

    // The pumpjet: a shroud round the tail, open at both ends.
    let duct_sides = if b.fine() { 12 } else { 6 };
    let duct = |x: f32, r: f32| sub_ring(x, r, duct_sides);
    b.paint(PLATING_DARK);
    b.loft(
        &[duct(-8.75, 0.98), duct(-9.95, 0.82), duct(-9.95, 0.7), duct(-8.75, 0.86), duct(-8.75, 0.98)],
        false,
        false,
    );

    // Tube shutters on the bow, dark.
    b.paint(ACCENT);
    if !b.fine() {
        b.face(&[v3(9.62, -0.9, -1.8), v3(9.62, 0.9, -1.8), v3(9.62, 0.9, -0.6), v3(9.62, -0.9, -0.6)]);
        return;
    }
    for m in SUB_TUBES {
        let m = Vec3::from(m);
        b.cylinder_between(m - Vec3::X * 0.3, m + Vec3::X * 0.12, 0.2, 0.2, 8);
    }
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.beam(v3(-7.95, 2.3, SUB_AXIS_Z), v3(-7.95, 2.45, SUB_AXIS_Z), v2(0.85, 0.12), v2(0.85, 0.12));
    });
    b.beam(v3(-7.95, 0.0, 1.38), v3(-7.95, 0.0, 1.55), v2(0.12, 0.85), v2(0.12, 0.85));
    // Stator vanes inside the shroud, a hub.
    b.paint(PLATING_DARK);
    for (y, z) in [(0.0, 1.0), (0.87, -0.5), (-0.87, -0.5)] {
        b.beam(v3(-9.2, 0.0, SUB_AXIS_Z), v3(-9.2, y * 0.8, SUB_AXIS_Z + z * 0.8), v2(0.06, 0.4), v2(0.06, 0.4));
    }
    b.paint(METAL);
    b.cylinder_between(v3(-9.4, 0.0, SUB_AXIS_Z), v3(-9.9, 0.0, SUB_AXIS_Z), 0.26, 0.12, 8);
    // Masts in the sail: periscope, a radar/ESM mast, the snorkel.
    b.paint(METAL);
    b.cylinder_between(v3(4.65, 0.0, 3.35), v3(4.65, 0.0, 3.95), 0.09, 0.07, 6);
    b.cylinder_between(v3(4.2, 0.12, 3.35), v3(4.2, 0.12, 3.75), 0.07, 0.07, 6);
    b.paint(ACCENT);
    b.spheroid(v3(4.2, 0.12, 3.82), v3(0.14, 0.14, 0.1), 6, 3);
    b.chamfered_box(v3(3.2, 0.0, 3.52), v3(0.36, 0.22, 0.34), 0.08);
    // A white hatch saddle forward of the sail, the escape trunk behind it.
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(6.9, 0.0, 1.1), v2(1.9, 1.1), 0.05, 0.02);
    b.paint(ACCENT);
    b.prism(v3(1.2, 0.0, 1.1), 8, 0.36, 0.34, 0.07);
    b.prism(v3(-4.4, 0.0, 1.1), 8, 0.36, 0.34, 0.07);
    // Flank arrays down each side, cleats on the casing.
    b.mirror_y(|b| {
        b.beam(v3(-3.0, 1.49, SUB_AXIS_Z), v3(5.5, 1.49, SUB_AXIS_Z), v2(0.05, 0.5), v2(0.05, 0.5));
        for x in [-5.0, 0.2, 5.8] {
            b.block(v3(x - 0.18, 0.55, 1.1), v3(x + 0.18, 0.66, 1.18));
        }
    });
}

// ---- Hydrophone: sonar buoy -------------------------------------------------
//
// A moored sonar buoy: a discus float through the waterline with the team's
// colour on its deck, an instrument housing and a slim mast with a radar
// reflector and the red lamp, and under the water a ballast spar with the
// hydrophone line hung from it. Three chains moor it. Each tier is refitted in
// place and changes the outline seen from the game camera, not only what hangs
// under the water:
// - tech 1: the bare float and one pole mast. Nothing lit.
// - tech 2: an outer float ring and three outrigger sponsons (a three-pointed
//   plan), each with a gantry streaming its own array deep; a lattice tripod
//   round the pole; blue strips on the deck.
// - tech 3: a big white radome on the tripod, a lit active array turning at the
//   masthead (it stops when the power does), a lit transducer band round the
//   ring and on each sponson, and under the float the active transducer head.
// Authored heights 12, 15, 18 m; origin at the waterline.

/// The float's deck, and the mast's top at each tier (where the lamp goes).
const BUOY_DECK: f32 = 1.1;
const BUOY_MAST: [f32; 3] = [11.2, 14.1, 17.2];
/// Tech 2's sponsons: how far out, and the headings they stand at (between the chains).
const BUOY_SPONSON: f32 = 5.0;
const BUOY_SPONSON_AT: [f32; 3] = [30.0, 150.0, 270.0];
/// Top of tech 2's tripod, where tech 3's radome sits.
const BUOY_TRIPOD: f32 = 7.4;
/// Height of tech 3's turning array.
const BUOY_ARRAY: f32 = 15.7;

fn buoy_ring(sides: usize, z: f32, radius: f32) -> Vec<Vec3> {
    ngon(sides, radius).into_iter().map(|[x, y]| v3(x, y, z)).collect()
}

/// A flat ring round the z axis from `r0` to `r1`, `z0` to `z1`: a float collar.
fn buoy_annulus(b: &mut MeshBuilder, sides: usize, z0: f32, z1: f32, r0: f32, r1: f32) {
    b.loft(
        &[
            buoy_ring(sides, z0, r1),
            buoy_ring(sides, z1, r1),
            buoy_ring(sides, z1, r0),
            buoy_ring(sides, z0, r0),
            buoy_ring(sides, z0, r1),
        ],
        false,
        false,
    );
}

/// Runs `f` once per sponson, in a frame turned so the sponson lies along +x.
fn buoy_sponsons(b: &mut MeshBuilder, f: impl Fn(&mut MeshBuilder)) {
    for a in BUOY_SPONSON_AT {
        b.yawed(Vec3::ZERO, a.to_radians(), |b| f(b));
    }
}

pub fn sonar(b: &mut MeshBuilder, tech: u8) {
    let deck = BUOY_DECK;
    let top = BUOY_MAST[tech.clamp(1, 3) as usize - 1];
    // Mooring chains, out and down at thirds of a turn.
    let chain = |k: usize| {
        let a = (90.0 + 120.0 * k as f32).to_radians();
        let (c, s) = (a.cos(), a.sin());
        (v3(2.3 * c, 2.3 * s, -0.9), v3(4.4 * c, 4.4 * s, -6.5))
    };
    if b.coarse() {
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, -0.8), 6, 3.0, 2.7, deck + 0.8);
        team_panel(b, v3(1.4, 0.0, deck), v2(0.9, 1.8));
        b.frustum_open(v3(0.0, 0.0, deck), v2(0.6, 0.6), v2(0.25, 0.25), top - deck, v2(0.0, 0.0));
        if tech >= 2 {
            b.prism(v3(0.0, 0.0, -0.4), 6, 4.1, 4.1, 1.1);
            buoy_sponsons(b, |b| {
                b.paint(PLATING);
                b.prism(v3(BUOY_SPONSON, 0.0, -0.6), 5, 0.7, 0.62, 1.4);
                b.paint(ACCENT);
                b.beam(v3(3.9, 0.0, 0.4), v3(4.5, 0.0, 0.4), v2(0.4, 0.35), v2(0.4, 0.35));
            });
            b.frustum_open(v3(0.0, 0.0, deck), v2(2.4, 2.4), v2(0.5, 0.5), BUOY_TRIPOD - deck, v2(0.0, 0.0));
        }
        if tech >= 3 {
            b.paint(PLATING);
            b.spheroid(v3(0.0, 0.0, BUOY_TRIPOD + 1.3), v3(1.45, 1.45, 1.35), 6, 2);
            b.paint(GLOW);
            b.beam(v3(-1.7, 0.0, BUOY_ARRAY), v3(1.7, 0.0, BUOY_ARRAY), v2(0.3, 0.9), v2(0.3, 0.9));
        }
        b.paint(ACCENT);
        for k in 0..3 {
            let (a, e) = chain(k);
            let side = Vec3::Z.cross(e - a).normalize() * 0.08;
            b.face(&[a - side, e - side, e + side, a + side]);
        }
        return;
    }
    let sides = if b.fine() { 16 } else { 8 };

    // The float: a discus hull through the waterline, painted by it, a dark rim, a deck.
    b.paint(PLATING).pattern(pattern::HULL);
    b.loft(
        &[buoy_ring(sides, -1.0, 2.1), buoy_ring(sides, -0.35, 2.95), buoy_ring(sides, 0.7, 3.1), buoy_ring(sides, deck, 2.75)],
        true,
        true,
    );
    b.paint(ACCENT);
    b.loft_z(&ngon(sides, 2.78), &[Section::new(deck - 0.06, 1.0), Section::new(deck + 0.12, 1.0)]);
    b.paint(PLATING).pattern(pattern::WALKWAY);
    b.loft_z(&ngon(sides, 2.5), &[Section::new(deck + 0.1, 1.0), Section::new(deck + 0.17, 1.0)]);
    let floor = deck + 0.17;
    for x in [1.65, -1.65] {
        team_panel(b, v3(x, 0.0, floor), v2(0.8, 1.7));
    }
    // Instrument housing, the mast, and the radar reflector on it.
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, floor), b.sides(8), 1.0, 0.85, 1.1);
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, floor + 1.1), b.sides(8), 0.88, 0.78, 0.12);
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, floor + 1.22), b.sides(6), 0.26, 0.17, BUOY_MAST[0] - floor - 1.22);
    b.paint(METAL);
    b.spheroid(v3(0.0, 0.0, 8.4), v3(0.45, 0.45, 0.55), 4, 2);
    if tech < 2 {
        beacon(b, v3(0.0, 0.0, BUOY_MAST[0]));
    }

    // Under the water: the ballast spar, its weight, and the hydrophone line under it.
    b.paint(PLATING_DARK).pattern(pattern::PILE);
    b.cylinder_between(v3(0.0, 0.0, -1.0), v3(0.0, 0.0, -6.0), 0.35, 0.35, b.sides(8));
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(0.0, 0.0, -6.0), v3(0.0, 0.0, -7.0), 0.75, 0.6, b.sides(8));
    b.paint(ACCENT);
    b.cylinder_between(v3(0.0, 0.0, -7.0), v3(0.0, 0.0, -10.4), 0.04, 0.04, 4);
    if b.fine() {
        for z in [-8.0, -9.0, -10.0] {
            b.cylinder_between(v3(0.0, 0.0, z - 0.2), v3(0.0, 0.0, z + 0.2), 0.12, 0.12, 6);
        }
        b.paint(PLATING_DARK);
        b.spheroid(v3(0.0, 0.0, -10.6), v3(0.3, 0.3, 0.3), 6, 2);
    }
    b.paint(ACCENT);
    for k in 0..3 {
        let (a, e) = chain(k);
        b.beam(a, e, v2(0.12, 0.12), v2(0.12, 0.12));
    }

    if b.fine() {
        // A rubbing band round the float, lifting eyes, a whip, a hatch in the housing.
        b.paint(TREAD);
        b.loft_z(&ngon(sides, 3.17), &[Section::new(0.25, 1.0), Section::new(0.5, 1.0)]);
        b.paint(METAL);
        for k in 0..3 {
            let a = (30.0 + 120.0 * k as f32).to_radians();
            b.cuboid(v3(2.3 * a.cos(), 2.3 * a.sin(), floor + 0.08), v3(0.16, 0.16, 0.16));
        }
        whip(b, v3(0.55, 0.45, floor + 1.22), 1.6, 0.05);
        b.paint(ACCENT);
        b.block(v3(0.92, -0.3, floor + 0.2), v3(1.0, 0.3, floor + 0.85));
    }

    // Tech 2: an outer float ring, three sponsons streaming arrays, a tripod, a taller mast.
    super::structures::kit(b, tech, 2, 0.15, |b| {
        let sides = if b.fine() { 16 } else { 8 };
        b.paint(PLATING).pattern(pattern::HULL);
        buoy_annulus(b, sides, -0.4, 0.75, 3.3, 4.1);
        b.paint(ACCENT);
        b.loft_z(&ngon(sides, 4.12), &[Section::new(0.72, 1.0), Section::new(0.82, 1.0)]);
        for k in 0..4 {
            let a = (45.0 + 90.0 * k as f32).to_radians();
            let (c, s) = (a.cos(), a.sin());
            b.beam(v3(2.9 * c, 2.9 * s, 0.35), v3(3.4 * c, 3.4 * s, 0.35), v2(0.3, 0.3), v2(0.3, 0.3));
        }
        // The sponsons: a boom off the ring, a float pod, and over its well a gantry
        // lowering an array far below the buoy's own.
        buoy_sponsons(b, |b| {
            let x = BUOY_SPONSON;
            b.paint(ACCENT);
            b.beam(v3(3.9, 0.0, 0.4), v3(x - 0.5, 0.0, 0.4), v2(0.4, 0.35), v2(0.34, 0.3));
            b.paint(PLATING).pattern(pattern::HULL);
            b.prism(v3(x, 0.0, -0.7), b.sides(8), 0.72, 0.64, 1.5);
            b.paint(ACCENT);
            b.prism(v3(x, 0.0, 0.8), b.sides(8), 0.62, 0.55, 0.1);
            team_panel(b, v3(x - 0.02, 0.0, 0.9), v2(0.5, 0.5));
            if b.fine() {
                b.paint(PLATING);
                for y in [-0.45, 0.45] {
                    b.beam(v3(x, y, 0.9), v3(x, y * 0.25, 2.6), v2(0.12, 0.12), v2(0.1, 0.1));
                }
                b.paint(METAL);
                b.cuboid(v3(x, 0.0, 2.7), v3(0.3, 0.34, 0.3));
            } else {
                b.paint(PLATING);
                b.beam(v3(x, 0.0, 0.9), v3(x, 0.0, 2.8), v2(0.2, 0.2), v2(0.2, 0.2));
            }
            b.paint(ACCENT);
            b.cylinder_between(v3(x, 0.0, 2.55), v3(x, 0.0, -11.0), 0.035, 0.035, 4);
            b.paint(PLATING_DARK);
            b.cylinder_between(v3(x, 0.0, -11.0), v3(x, 0.0, -13.6), 0.16, 0.16, 6);
            if b.fine() {
                b.cylinder_between(v3(x, 0.0, -13.6), v3(x, 0.0, -14.2), 0.16, 0.4, 6);
                b.paint(ACCENT);
                for z in [-11.8, -12.8] {
                    b.cylinder_between(v3(x, 0.0, z - 0.12), v3(x, 0.0, z + 0.12), 0.2, 0.2, 6);
                }
            }
        });
        // The tripod: three legs from the deck's edge to a platform round the pole.
        let foot = |k: usize, r: f32, z: f32| {
            let a = (90.0 + 120.0 * k as f32).to_radians();
            v3(r * a.cos(), r * a.sin(), z)
        };
        let leg = |k: usize, t: f32| foot(k, 2.2, floor).lerp(foot(k, 0.4, BUOY_TRIPOD), t);
        b.paint(PLATING);
        for k in 0..3 {
            b.beam(leg(k, 0.0), leg(k, 1.0), v2(0.18, 0.18), v2(0.14, 0.14));
        }
        b.paint(METAL);
        for t in if b.fine() { &[0.3, 0.62][..] } else { &[0.45][..] } {
            for k in 0..3 {
                b.beam(leg(k, *t), leg((k + 1) % 3, *t), v2(0.07, 0.07), v2(0.07, 0.07));
            }
        }
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, BUOY_TRIPOD - 0.1), b.sides(8), 1.0, 1.0, 0.2);
        // Blue strips on the deck: the first lit kit.
        for y in [1.35, -1.35] {
            glow_strip(b, v3(0.0, y, floor), v2(1.1, 0.14), GLOW);
        }
        // The pole run up higher, a yard with aerials.
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, BUOY_MAST[0]), b.sides(6), 0.17, 0.12, BUOY_MAST[1] - BUOY_MAST[0]);
        b.paint(METAL);
        b.beam(v3(0.0, -0.95, 10.3), v3(0.0, 0.95, 10.3), v2(0.1, 0.1), v2(0.1, 0.1));
        if b.fine() {
            whip(b, v3(0.0, 0.9, 10.35), 1.5, 0.04);
            whip(b, v3(0.0, -0.9, 10.35), 1.2, 0.08);
        }
        if tech < 3 {
            beacon(b, v3(0.0, 0.0, BUOY_MAST[1]));
        }
    });

    // Tech 3: the radome, the turning active array, lit transducer bands, and under the
    // float the active transducer head with stabiliser fins.
    super::structures::kit(b, tech, 3, 0.2, |b| {
        let sides = if b.fine() { 16 } else { 8 };
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, BUOY_TRIPOD + 0.1), if b.fine() { 12 } else { 8 }, 1.05, 1.15, 0.22);
        b.paint(PLATING);
        let (dome_sides, dome_rings) = if b.fine() { (12, 6) } else { (8, 3) };
        b.spheroid(v3(0.0, 0.0, BUOY_TRIPOD + 1.4), v3(1.45, 1.45, 1.35), dome_sides, dome_rings);
        b.paint(GLOW);
        b.loft_z(&ngon(sides, 4.16), &[Section::new(0.5, 1.0), Section::new(0.68, 1.0)]);
        buoy_sponsons(b, |b| {
            b.prism(v3(BUOY_SPONSON, 0.0, 0.3), if b.fine() { 8 } else { 6 }, 0.68, 0.66, 0.16);
        });

        // Under the float, seen only through the water: the transducer head and fins.
        if b.fine() {
            b.paint(ACCENT);
            b.prism(v3(0.0, 0.0, -3.7), 8, 1.4, 1.25, 1.5);
            b.paint(GLOW);
            b.prism(v3(0.0, 0.0, -3.05), 8, 1.44, 1.44, 0.14);
            b.paint(PLATING_DARK);
            b.radial(4, |b| {
                b.beam(v3(1.9, 0.0, -1.5), v3(3.7, 0.0, -1.5), v2(0.1, 1.1), v2(0.08, 0.7));
            });
        }

        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, BUOY_MAST[1]), b.sides(6), 0.13, 0.09, BUOY_MAST[2] - BUOY_MAST[1]);
        // The active array: a yard turning on the mast, a lit face at each end.
        b.set_spinner_pivot(v3(0.0, 0.0, BUOY_ARRAY));
        b.with_part(part::SPINNER, |b| {
            b.paint(ACCENT);
            b.prism(v3(0.0, 0.0, BUOY_ARRAY - 0.2), b.sides(8), 0.3, 0.26, 0.4);
            b.paint(METAL);
            b.beam(v3(-1.5, 0.0, BUOY_ARRAY), v3(1.5, 0.0, BUOY_ARRAY), v2(0.12, 0.12), v2(0.12, 0.12));
            for s in [1.0, -1.0] {
                if !b.fine() {
                    b.paint(GLOW);
                    b.block(v3(1.5 * s - 0.1, -0.5, BUOY_ARRAY - 0.55), v3(1.5 * s + 0.1, 0.5, BUOY_ARRAY + 0.55));
                    continue;
                }
                b.paint(ACCENT);
                b.block(v3(1.5 * s - 0.1, -0.5, BUOY_ARRAY - 0.55), v3(1.5 * s + 0.1, 0.5, BUOY_ARRAY + 0.55));
                b.paint(GLOW);
                let face = 1.5 * s + 0.1 * s;
                let (x0, x1) = if s > 0.0 { (face, face + 0.04) } else { (face - 0.04, face) };
                b.block(v3(x0, -0.42, BUOY_ARRAY - 0.47), v3(x1, 0.42, BUOY_ARRAY + 0.47));
            }
        });
        beacon(b, v3(0.0, 0.0, BUOY_MAST[2]));
    });
}
