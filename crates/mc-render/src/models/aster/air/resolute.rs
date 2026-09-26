//! Resolute: the ARC's tech 3 heavy frigate (`aster_t3_frigate`, mesh `space_frigate`), the
//! first warship of the upper air, built up of armoured sections: a narrow gun spine on top
//! running out past a flared keel hull below, and a tall engineering hull aft carrying the
//! bridge. The spinal rail cannon runs down a trench in the spine from the engineering hull's
//! bulkhead to a bore let flush into the blunt prow. It is fixed; the whole ship turns and
//! pitches to aim it. Two big twin-drive nacelles hang under swept armoured wings astern.
//! +X is forward, +Y port, the ground at z 0 with the legs down (it is built on its lot).
//!
//! Contracts, all at the top of this file:
//! - `SPINAL`: the spinal rail's muzzle, breech and the points along the rails where the
//!   charge crawls (for the rail's charge and fire effects). `space.ron` weapon 0 `muzzle`
//!   is `SPINAL.muzzle`.
//! - `TURRETS`: the four rail cannon houses in weapon order 1..=4 (pivot, rest facing),
//!   authored facing +X with the muzzle `TURRET_REACH` ahead of the pivot; `space.ron`
//!   carries the same pivots and muzzles.
//! - `CELLS`: the vertical-launch hatches of weapons 5 (port) and 6 (starboard).
//! - `NOZZLES`, `LIFT_JETS`, `LAMPS`, `RIG`: the shared spacecraft rig (`capital.rs`).
//!
//! ARC rails are unlit hardware: nothing on the spinal rail or the turret rails glows.

use super::capital::{self, CapitalRig, Leg};
use super::*;
use crate::models::builder::chamfered_rect;
use glam::Vec2;

/// Where effects attach to the spinal rail cannon (model space, metres).
pub(crate) struct SpinalRail {
    /// The centre of the muzzle mouth, on the bore axis: shots leave here.
    pub muzzle: [f32; 3],
    /// Where the rails leave the breech housing, on the bore axis.
    pub breech: [f32; 3],
    /// Points on top of the rails (the slot's centre line), breech forward, in the open
    /// lengths between the containment collars: where charge arcs can crawl along the gun.
    /// Forward of `PROW_ROOF` the rails run under the prow's armour.
    pub arcs: [[f32; 3]; 6],
}

/// Height of the spinal rail's bore axis.
const RAIL_Z: f32 = 47.0;
/// The rails' half height, and the inner and outer half width of the pair.
const RAIL_H: f32 = 5.0;
const RAIL_IN: f32 = 2.2;
const RAIL_OUT: f32 = 5.4;
/// Where the rails leave the breech housing, and the prow's face, where the bore opens.
const BREECH_X: f32 = -46.0;
const MUZZLE_X: f32 = 150.0;

pub(crate) const SPINAL: SpinalRail = SpinalRail {
    muzzle: [MUZZLE_X, 0.0, RAIL_Z],
    breech: [BREECH_X, 0.0, RAIL_Z],
    arcs: [
        [-40.0, 0.0, RAIL_Z + RAIL_H],
        [-24.0, 0.0, RAIL_Z + RAIL_H],
        [2.0, 0.0, RAIL_Z + RAIL_H],
        [22.0, 0.0, RAIL_Z + RAIL_H],
        [40.0, 0.0, RAIL_Z + RAIL_H],
        [66.0, 0.0, RAIL_Z + RAIL_H],
    ],
};

/// The rail cannon houses in weapon order (weapons 1..=4): pivot and rest facing in
/// degrees (+X forward, +Y port). A chin house slung under the spine's overhang, clear of
/// everything below and ahead; a dorsal house aft of the bridge facing astern, which only
/// dips a little before its rails would meet the deck (`space.ron` `depression`); and the
/// two flank houses slung low on sponsons, turned a little forward, so they depress onto
/// the ground under the ship and never swing back into the nacelles. The unit file's arcs
/// keep every house off the hull.
pub(super) const TURRETS: [([f32; 3], f32); 4] = [
    ([104.0, 0.0, 22.0], 0.0),
    ([-116.0, 0.0, 74.0], 180.0),
    ([10.0, 42.0, 34.0], 70.0),
    ([10.0, -42.0, 34.0], -70.0),
];
/// How far ahead of its pivot a turret's rails end (the unit file's `muzzle` - `pivot`).
pub(super) const TURRET_REACH: f32 = 26.0 * TURRET_SCALE;
/// The houses are authored at this fraction of their size.
const TURRET_SCALE: f32 = 1.3;

/// Deck height the rocket cells stand on, and their hatch tops (weapons 5 and 6: +y, -y).
const CELL_DECK: f32 = 58.0;
pub(super) const CELLS: [[f32; 3]; 8] = [
    [18.0, 13.8, 62.2],
    [18.0, 19.2, 62.2],
    [26.0, 13.8, 62.2],
    [26.0, 19.2, 62.2],
    [34.0, 13.8, 62.2],
    [34.0, 19.2, 62.2],
    [42.0, 13.8, 62.2],
    [42.0, 19.2, 62.2],
];

/// The flat belly of the keel hull and the engineering hull.
const KEEL: f32 = 18.0;
/// Half width of the spinal trench, and its floor.
const TRENCH: f32 = 8.0;
const FLOOR: f32 = 38.0;
/// The keel hull: its top (the spine sits on it), its flat belly's and its flare's half
/// widths, and where its raked bow meets the belly and the top.
const LOWER_TOP: f32 = 38.0;
const LOWER_BELLY: f32 = 24.0;
const LOWER_FLARE: f32 = 28.0;
const LOWER_BOW: [f32; 2] = [62.0, 82.0];
/// The engineering hull: from the stern forward to its raked bulkhead, its deck, and its
/// half width at the belly and at the sides.
const ENGINE_AFT: f32 = -130.0;
const ENGINE_FORE: f32 = -46.0;
const ENGINE_DECK: f32 = 66.0;
const ENGINE_BELLY: f32 = 26.0;
const ENGINE_SIDE: f32 = 33.0;

/// The engine nacelles: centre line |y| and height, half width and half height of their
/// armoured section, and x of their after end and their nose.
const NACELLE_Y: f32 = 57.0;
const NACELLE_Z: f32 = 42.0;
const NACELLE_HW: f32 = 20.0;
const NACELLE_HH: f32 = 13.5;
const NACELLE_AFT: f32 = -160.0;
const NACELLE_NOSE: f32 = -54.0;
/// The drive bells in the nacelles' tails, and each pair's half spacing.
const DRIVE_SIZE: f32 = 0.75;
const DRIVE_PAIR: f32 = 9.8;

/// Two drives side by side in each nacelle (mouths, facing aft): port inner, port outer,
/// starboard inner, starboard outer.
pub(crate) const NOZZLES: [[f32; 3]; 4] = [
    [-173.0, NACELLE_Y - DRIVE_PAIR, NACELLE_Z],
    [-173.0, NACELLE_Y + DRIVE_PAIR, NACELLE_Z],
    [-173.0, -NACELLE_Y + DRIVE_PAIR, NACELLE_Z],
    [-173.0, -NACELLE_Y - DRIVE_PAIR, NACELLE_Z],
];
/// Downward lift jets under the belly (mouth centres): a pair under the stern, a pair
/// under the prow.
pub(crate) const LIFT_JETS: [[f32; 3]; 4] = [
    [-114.0, -18.0, KEEL - 0.4],
    [-114.0, 18.0, KEEL - 0.4],
    [52.0, -11.0, KEEL - 0.4],
    [52.0, 11.0, KEEL - 0.4],
];
/// Leg size against the Bastion's (hinge 36 m over the foot).
const LEG: f32 = 0.6;
/// The bay doors' outer face, a little under the keel.
const BAY_SILL: f32 = KEEL - 0.7;

/// What `entity.wgsl` animates (`models::capital_rig`): two pairs of short legs that stow
/// into belly bays as the ship climbs off its lot, the drives' glow and iris vanes, and
/// the lift jets' glow. No ramp.
pub(crate) const RIG: CapitalRig = CapitalRig {
    legs: Some([
        Leg { hinge: [34.0, 20.0, 36.0 * LEG], stow: 1.0, bay: [14.0, 36.5, 16.5, 23.5], size: LEG },
        Leg { hinge: [-100.0, 22.0, 36.0 * LEG], stow: -1.0, bay: [-102.5, -80.0, 18.5, 25.5], size: LEG },
    ]),
    door_hinge: KEEL - 0.3,
    drives: Some(([NOZZLES[0][0], NOZZLES[0][2], NOZZLES[0][1], NOZZLES[1][1]], DRIVE_SIZE)),
    lift_jets: Some(([LIFT_JETS[3][0], LIFT_JETS[3][1], LIFT_JETS[1][0], LIFT_JETS[1][1]], LIFT_JETS[0][2])),
    ramp: None,
};

/// Where the radar array turns, over the bridge.
const RADAR: [f32; 3] = [-82.0, 0.0, 107.0];

/// Lamps (`models::capital_lamps`): landing floods under the belly, red/green steady
/// sidelights on the nacelles' outboard faces, white strobes on the mast, the prow's
/// corners and the wing tips. No ramp, so no beacons or hold lamp.
pub(crate) const LAMPS: crate::models::CapitalLamps = crate::models::CapitalLamps {
    floods: &[[58.0, 5.0, KEEL - 0.9], [58.0, -5.0, KEEL - 0.9], [-122.0, 10.0, KEEL - 0.9], [-122.0, -10.0, KEEL - 0.9]],
    nav_port: [-104.0, NACELLE_Y + NACELLE_HW + 0.5, NACELLE_Z],
    nav_starboard: [-104.0, -NACELLE_Y - NACELLE_HW - 0.5, NACELLE_Z],
    strobes: &[
        [RADAR[0], 0.0, RADAR[2] + 7.5],
        [149.5, 18.0, 58.8],
        [149.5, -18.0, 58.8],
        [-150.0, NACELLE_Y + NACELLE_HW + 3.0, NACELLE_Z + NACELLE_HH + 5.0],
        [-150.0, -NACELLE_Y - NACELLE_HW - 3.0, NACELLE_Z + NACELLE_HH + 5.0],
    ],
    beacons: &[],
    hold: None,
};

/// Spine stations: x, outer half width, keel, deck. A long narrow armoured box from inside
/// the engineering hull to the prow, sitting on the keel hull, stepping in over its bow and
/// running on past it to the flat prow.
const STATIONS: [[f32; 4]; 6] = [
    [-52.0, 24.0, 34.0, 58.0],
    [58.0, 24.0, 34.0, 58.0],
    [66.0, 21.0, 34.0, 58.0],
    [112.0, 21.0, 34.0, 58.0],
    [120.0, 21.0, 34.0, 58.0],
    [MUZZLE_X, 20.0, 34.0, 57.0],
];

/// Width, keel and deck at `x`.
fn station(x: f32) -> (f32, f32, f32) {
    let s = &STATIONS;
    if x <= s[0][0] {
        return (s[0][1], s[0][2], s[0][3]);
    }
    for p in s.windows(2) {
        if x <= p[1][0] {
            let t = (x - p[0][0]) / (p[1][0] - p[0][0]);
            let l = |i: usize| p[0][i] + (p[1][i] - p[0][i]) * t;
            return (l(1), l(2), l(3));
        }
    }
    let l = s[s.len() - 1];
    (l[1], l[2], l[3])
}

/// The outline of the port half of the spine at `x`, (y, z) from the inner keel round the
/// outside to the trench lip: a flat keel, a short hard chamfer, a vertical armoured side
/// and a sharper chamfer onto the deck.
fn half_section(x: f32) -> Vec<Vec2> {
    let (w, k, c) = station(x);
    vec![
        v2(TRENCH, k),
        v2(w - 4.0, k),
        v2(w, k + 4.0),
        v2(w, c - 7.0),
        v2(w - 5.0, c),
        v2(TRENCH + 2.0, c),
        v2(TRENCH, c - 2.0),
    ]
}

/// Points of the port hull surface at `x` between outline corners `from..=to` of
/// [`half_section`], `u` of the way along each end edge, pushed `lift` metres out.
fn surface_run(x: f32, from: usize, to: usize, u0: f32, u1: f32, lift: f32) -> Vec<Vec2> {
    let s = half_section(x);
    let centre = v2(0.0, (s[0].y + s[5].y) * 0.5);
    let mut pts: Vec<Vec2> = Vec::new();
    pts.push(s[from] + (s[from + 1] - s[from]) * u0);
    for p in &s[from + 1..to] {
        pts.push(*p);
    }
    pts.push(s[to - 1] + (s[to] - s[to - 1]) * u1);
    // Push each point out along the averaged outward normal of its edges.
    let n = pts.len();
    (0..n)
        .map(|i| {
            let a = pts[if i == 0 { 0 } else { i - 1 }];
            let c = pts[if i + 1 == n { n - 1 } else { i + 1 }];
            let d = (c - a).normalize_or_zero();
            let mut normal = v2(d.y, -d.x);
            if normal.dot(pts[i] - centre) < 0.0 {
                normal = -normal;
            }
            pts[i] + normal * lift
        })
        .collect()
}

/// An armour plate lying on the port hull from `x0` to `x1` over outline corners
/// `from..=to` (`u0`/`u1` of the end edges), `thick` metres proud.
fn plate_on_hull(b: &mut MeshBuilder, x0: f32, x1: f32, from: usize, to: usize, u: [f32; 2], thick: f32) {
    let ring = |x: f32| {
        let outer = surface_run(x, from, to, u[0], u[1], thick);
        let inner = surface_run(x, from, to, u[0], u[1], -0.4);
        outer.iter().chain(inner.iter().rev()).map(|p| v3(x, p.x, p.y)).collect::<Vec<_>>()
    };
    b.loft(&[ring(x0), ring(x1)], true, true);
}



pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    spine(b);
    keel_hull(b);
    engineering(b);
    prow(b);
    spinal(b);
    bore(b);
    coil(b);
    capacitors(b);
    tower(b);
    cells(b);
    b.mirror_y(|b| {
        wing(b);
        nacelle(b);
    });
    stern(b);
    belly(b);
    capital::gear(b, &RIG, BAY_SILL);
    lamps(b);
    chin_post(b, Vec3::from(TURRETS[0].0));
    barbette(b, Vec3::from(TURRETS[1].0), ENGINE_DECK - 1.0);
    b.mirror_y(sponson);
    for (i, (at, _)) in TURRETS.into_iter().enumerate() {
        rail_turret(b, i + 1, Vec3::from(at));
    }
}

/// The sections in plan, the nacelles and the bridge: a few dozen triangles for strategy
/// zoom.
fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.extrude_z(&[[MUZZLE_X, 24.0], [MUZZLE_X, -24.0], [ENGINE_FORE, -LOWER_FLARE], [ENGINE_FORE, LOWER_FLARE]], KEEL, 58.0);
    b.extrude_z(&[[ENGINE_FORE, ENGINE_SIDE], [ENGINE_FORE, -ENGINE_SIDE], [ENGINE_AFT, -ENGINE_SIDE], [ENGINE_AFT, ENGINE_SIDE]], KEEL, ENGINE_DECK);
    b.paint(PLATING);
    b.cuboid_open(v3(-78.0, 0.0, ENGINE_DECK + 10.0), v3(40.0, 26.0, 20.0));
    b.paint(PLATING_DARK);
    for y in [NACELLE_Y, -NACELLE_Y] {
        b.cuboid_open(v3((NACELLE_AFT + NACELLE_NOSE) * 0.5, y, NACELLE_Z + 2.0), v3(NACELLE_NOSE - NACELLE_AFT, 2.0 * NACELLE_HW, 2.0 * NACELLE_HH + 4.0));
    }
}

/// The gun spine: two long armoured halves either side of the spinal trench, sitting on
/// the keel hull and running out past its bow to the prow; the keel under the trench,
/// armour belts down its sides, deck fittings.
fn spine(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        let rings = STATIONS
            .iter()
            .map(|s| half_section(s[0]).iter().map(|p| v3(s[0], p.x, p.y)).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        b.loft(&rings, true, true);
        // Armour belts over the side: thick pale slabs with dark gaps between.
        let belts: &[(f32, f32)] = if fine {
            &[(-40.0, -14.0), (-10.0, 16.0), (20.0, 46.0), (50.0, 76.0), (80.0, 110.0)]
        } else {
            &[(-40.0, 16.0), (20.0, 76.0), (80.0, 110.0)]
        };
        for &(x0, x1) in belts {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            plate_on_hull(b, x0, x1, 2, 4, [0.2, 0.2], 1.6);
            if fine {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                plate_on_hull(b, x0 + 2.0, x1 - 2.0, 3, 5, [0.25, 0.6], 0.5);
            }
        }
        if fine {
            // Deck strips and hatches between the trench lip and the deck edge.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for &(x0, x1) in &[(-8.0, 8.0), (54.0, 70.0), (106.0, 146.0)] {
                let (_, _, c) = station(x0);
                b.block(v3(x0, 11.5, c - 0.2), v3(x1, 16.0, c + 0.4));
            }
            b.paint(METAL).pattern(pattern::PLAIN);
            for x in [-4.0, 4.0, 58.0, 64.0, 112.0, 124.0, 136.0] {
                let (_, _, c) = station(x);
                b.plate(v3(x, 13.8, c + 0.4), v2(3.6, 3.6), 0.35, 0.12);
            }
        }
        b.paint(TEAM).pattern(pattern::PLAIN);
        plate_on_hull(b, 84.0, 90.0, 2, 3, [0.3, 1.0], 1.75);
        side_pod(b);
    });
    // The keel under the trench: its floor carries the rails.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let keel = STATIONS
        .iter()
        .map(|s| {
            let (x, k) = (s[0], s[2]);
            vec![v3(x, -TRENCH - 0.1, k), v3(x, TRENCH + 0.1, k), v3(x, TRENCH + 0.1, FLOOR), v3(x, -TRENCH - 0.1, FLOOR)]
        })
        .collect::<Vec<_>>();
    b.loft(&keel, true, true);
}

/// The port side pod on the spine's midships (mirrored): a long armoured box slung along
/// the spine's lower side above the keel hull, raked at both ends, so the flank reads in
/// layers; pale armour slabs on its face.
fn side_pod(b: &mut MeshBuilder) {
    let (x0, x1) = (-44.0, 54.0);
    let (y0, y1, z0, z1) = (23.0, 30.5, 38.5, 52.0);
    let ring = |x: f32, s: f32| {
        let out = y0 + (y1 - y0) * s;
        vec![v3(x, y0, z0), v3(x, out - 2.0, z0), v3(x, out, z0 + 3.0), v3(x, out, z1 - 3.0), v3(x, out - 3.0, z1), v3(x, y0, z1)]
    };
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&[ring(x0, 0.2), ring(x0 + 8.0, 1.0), ring(x1 - 10.0, 1.0), ring(x1, 0.2)], true, true);
    let slabs: &[(f32, f32)] = if b.fine() { &[(-34.0, -12.0), (-8.0, 16.0), (20.0, 42.0)] } else { &[] };
    for &(a, c) in slabs {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.block(v3(a, y1 - 0.4, z0 + 3.5), v3(c, y1 + 1.2, z1 - 3.5));
    }
    if b.fine() {
        b.paint(TEAM).pattern(pattern::PLAIN);
        b.block(v3(-4.0, y1 + 1.0, z0 + 4.0), v3(2.0, y1 + 1.5, z1 - 4.0));
    }
}

/// A closed section through a hull symmetric about y 0: `half` is the port half from the
/// keel's middle round to the deck's middle, (y, z); the ring runs up the port side and
/// back down the starboard side.
fn full_ring(x: f32, half: &[[f32; 2]], shift: impl Fn(f32) -> f32) -> Vec<Vec3> {
    let mut ring: Vec<Vec3> = half.iter().map(|p| v3(x + shift(p[1]), p[0], p[1])).collect();
    ring.extend(half.iter().rev().filter(|p| p[0] > 0.0).map(|p| v3(x + shift(p[1]), -p[0], p[1])));
    ring
}

/// The keel hull: a long flared armoured hull under the spine's forward two thirds, its
/// sides flaring out below a sloped shoulder, its bow raked back from the top. Rows of
/// ports down its flanks, sensor prongs out of the bow, armour slabs on the flare.
fn keel_hull(b: &mut MeshBuilder) {
    let fine = b.fine();
    let half = |belly: f32, flare: f32| {
        vec![
            [0.0, KEEL],
            [belly, KEEL],
            [flare, KEEL + 7.0],
            [flare, KEEL + 13.0],
            [flare - 5.0, LOWER_TOP],
            [0.0, LOWER_TOP],
        ]
    };
    let full = half(LOWER_BELLY, LOWER_FLARE);
    // The bow: raked back from the top, so its foot is `LOWER_BOW[0]` and its head `[1]`.
    let rake = |z: f32| (z - KEEL) / (LOWER_TOP - KEEL) * (LOWER_BOW[1] - LOWER_BOW[0]);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            full_ring(ENGINE_FORE - 12.0, &full, |_| 0.0),
            full_ring(LOWER_BOW[0] - 14.0, &full, |_| 0.0),
            full_ring(LOWER_BOW[0], &half(LOWER_BELLY - 4.0, LOWER_FLARE - 3.0), rake),
        ],
        true,
        true,
    );
    b.mirror_y(|b| {
        // Armour slabs on the flare, in a run of separate plates.
        let slabs: &[(f32, f32)] = if fine {
            &[(-40.0, -14.0), (-10.0, 18.0), (24.0, 46.0)]
        } else {
            &[(-40.0, 46.0)]
        };
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        for &(x0, x1) in slabs {
            b.loft(
                &[
                    vec![v3(x0, LOWER_BELLY + 0.5, KEEL + 1.0), v3(x0, LOWER_FLARE + 1.4, KEEL + 7.0), v3(x0, LOWER_FLARE + 1.4, KEEL + 12.5), v3(x0, LOWER_FLARE - 0.2, KEEL + 12.5), v3(x0, LOWER_FLARE - 0.2, KEEL + 7.2), v3(x0, LOWER_BELLY, KEEL + 1.0)],
                    vec![v3(x1, LOWER_BELLY + 0.5, KEEL + 1.0), v3(x1, LOWER_FLARE + 1.4, KEEL + 7.0), v3(x1, LOWER_FLARE + 1.4, KEEL + 12.5), v3(x1, LOWER_FLARE - 0.2, KEEL + 12.5), v3(x1, LOWER_FLARE - 0.2, KEEL + 7.2), v3(x1, LOWER_BELLY, KEEL + 1.0)],
                ],
                true,
                true,
            );
        }
        // Rows of ports along the shoulder: dark slots, lit by nothing.
        if fine {
            b.paint(TREAD).pattern(pattern::NONE);
            let step = 6.0;
            for &(x0, x1) in &[(-36.0, 54.0)] {
                let mut x = x0;
                while x <= x1 {
                    b.block(v3(x - 1.6, LOWER_FLARE - 2.2, LOWER_TOP - 5.4), v3(x + 1.6, LOWER_FLARE - 1.0, LOWER_TOP - 3.4));
                    x += step;
                }
            }
        }
        // Team colour on the bow's flare.
        b.paint(TEAM).pattern(pattern::PLAIN);
        b.block(v3(48.0, LOWER_FLARE + 1.2, KEEL + 7.5), v3(54.0, LOWER_FLARE + 1.6, KEEL + 12.0));
        // Sensor prongs out of the bow, forward under the spine's overhang.
        b.paint(METAL).pattern(pattern::PLAIN);
        b.cylinder_between(v3(LOWER_BOW[0] + 6.0, 13.0, KEEL + 6.0), v3(MUZZLE_X - 1.0, 13.0, KEEL + 6.0), 0.9, 0.5, b.sides(6));
        b.cylinder_between(v3(LOWER_BOW[0] + 8.0, 17.0, KEEL + 11.0), v3(MUZZLE_X - 12.0, 17.0, KEEL + 11.0), 0.7, 0.4, b.sides(6));
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.chamfered_box(v3(LOWER_BOW[0] + 5.0, 13.0, KEEL + 6.0), v3(4.0, 4.0, 4.0), 1.0);
    });
    // The keel strake along the belly.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(-56.0, -3.0, KEEL - 1.0), v3(-46.0, 3.0, KEEL + 0.2));
    // The ventral pod: a deeper armoured keel slung under the midships, its ends raked,
    // ribbed underneath.
    let (x0, x1, hw, z) = (-44.0, 8.0, 15.0, 8.0);
    let pod = |x: f32, bottom: f32, half: f32| {
        vec![v3(x, -half, bottom), v3(x, half, bottom), v3(x, hw, KEEL + 0.5), v3(x, -hw, KEEL + 0.5)]
    };
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&[pod(x0 - 8.0, KEEL - 0.5, hw - 4.0), pod(x0 + 4.0, z, hw - 3.0), pod(x1 - 6.0, z, hw - 3.0), pod(x1 + 6.0, KEEL - 0.5, hw - 4.0)], true, true);
    if fine {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for x in [-32.0, -20.0, -8.0] {
            b.block(v3(x - 1.0, -hw + 3.4, z - 0.6), v3(x + 1.0, hw - 3.4, z + 0.2));
        }
    }
}

/// The engineering hull: the tall, broad after section the bridge stands on, square-sided
/// over a flared belly, with a raked bulkhead forward where the spine and keel hull run in.
/// Heavy armour panels in two tiers down its sides.
fn engineering(b: &mut MeshBuilder) {
    let fine = b.fine();
    let half = |belly: f32, side: f32, keel: f32, deck: f32| {
        vec![
            [0.0, keel],
            [belly, keel],
            [side, keel + 8.0],
            [side, deck - 10.0],
            [side - 7.0, deck],
            [0.0, deck],
        ]
    };
    let full = half(ENGINE_BELLY, ENGINE_SIDE, KEEL, ENGINE_DECK);
    // Forward the bulkhead rakes back from its foot to its head.
    let rake = |z: f32| -(z - KEEL) / (ENGINE_DECK - KEEL) * 8.0;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            full_ring(ENGINE_AFT, &half(ENGINE_BELLY - 4.0, ENGINE_SIDE - 3.0, KEEL + 6.0, ENGINE_DECK - 4.0), |_| 0.0),
            full_ring(ENGINE_AFT + 6.0, &full, |_| 0.0),
            full_ring(ENGINE_FORE - 14.0, &full, |_| 0.0),
            full_ring(ENGINE_FORE, &half(ENGINE_BELLY - 2.0, ENGINE_SIDE - 5.0, KEEL + 2.0, ENGINE_DECK - 4.0), rake),
        ],
        true,
        true,
    );
    b.mirror_y(|b| {
        // Two tiers of armour panels down the side, the upper tier standing proud of the lower.
        let panels: &[(f32, f32)] = if fine {
            &[(-122.0, -100.0), (-96.0, -80.0), (-76.0, -62.0)]
        } else {
            &[(-122.0, -62.0)]
        };
        for &(x0, x1) in panels {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.block(v3(x0, ENGINE_SIDE - 0.4, ENGINE_DECK - 22.0), v3(x1, ENGINE_SIDE + 2.2, ENGINE_DECK - 10.5));
            b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
            b.block(v3(x0 + 2.0, ENGINE_SIDE - 0.4, KEEL + 10.0), v3(x1 - 2.0, ENGINE_SIDE + 1.0, ENGINE_DECK - 23.0));
        }
        // A deep dark seam between the tiers, and the owner's colour on the upper tier.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(v3(-124.0, ENGINE_SIDE - 0.4, ENGINE_DECK - 23.6), v3(-58.0, ENGINE_SIDE + 1.4, ENGINE_DECK - 22.2));
        b.paint(TEAM).pattern(pattern::PLAIN);
        b.block(v3(-70.0, ENGINE_SIDE + 2.0, ENGINE_DECK - 20.0), v3(-65.0, ENGINE_SIDE + 2.5, ENGINE_DECK - 12.5));
        if fine {
            // A row of ports under the deck edge.
            b.paint(TREAD).pattern(pattern::NONE);
            let mut x = -118.0;
            while x < -64.0 {
                b.block(v3(x - 1.4, ENGINE_SIDE - 5.2, ENGINE_DECK - 5.0), v3(x + 1.4, ENGINE_SIDE - 3.6, ENGINE_DECK - 3.2));
                x += 7.0;
            }
        }
        // Armoured shoulders on the bulkhead where the spine runs out.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                vec![v3(ENGINE_FORE - 20.0, 24.0, ENGINE_DECK - 4.0), v3(ENGINE_FORE - 20.0, ENGINE_SIDE - 1.0, ENGINE_DECK - 10.0), v3(ENGINE_FORE - 20.0, ENGINE_SIDE - 1.0, ENGINE_DECK + 1.0), v3(ENGINE_FORE - 20.0, 24.0, ENGINE_DECK + 3.0)],
                vec![v3(ENGINE_FORE - 6.0, 24.0, ENGINE_DECK - 8.0), v3(ENGINE_FORE - 6.0, ENGINE_SIDE - 4.0, ENGINE_DECK - 12.0), v3(ENGINE_FORE - 6.0, ENGINE_SIDE - 4.0, ENGINE_DECK - 6.0), v3(ENGINE_FORE - 6.0, 24.0, ENGINE_DECK - 4.0)],
            ],
            true,
            true,
        );
    });
}

/// Where the prow's armour roof closes over the spinal trench.
const PROW_ROOF: f32 = 72.0;

/// The prow: an armour roof closing over the trench from `PROW_ROOF` to the face, a heavy
/// armoured head round the spine's forward end standing proud of it, and the chin under
/// the overhang.
fn prow(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    let roof = |x: f32| {
        let (_, _, c) = station(x);
        let w = TRENCH + 2.4;
        vec![v3(x, -w, c - 2.5), v3(x, w, c - 2.5), v3(x, w, c + 0.6), v3(x, 6.0, c + 2.2), v3(x, -6.0, c + 2.2), v3(x, -w, c + 0.6)]
    };
    b.loft(&[roof(PROW_ROOF), roof(118.0), roof(MUZZLE_X)], true, true);
    if fine {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for x in [80.0, 106.0] {
            let (_, _, c) = station(x);
            b.block(v3(x - 0.6, -7.0, c + 1.4), v3(x + 0.6, 7.0, c + 2.5));
        }
    }
    // Cheek armour down the last stretch in two plates, lying flush on the sides and ending
    // square at the face: the prow is the spine's own end, no wider.
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        plate_on_hull(b, 114.0, 131.0, 2, 4, [0.1, 0.3], 1.4);
        plate_on_hull(b, 133.0, MUZZLE_X, 2, 4, [0.1, 0.3], 1.4);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        plate_on_hull(b, 116.0, MUZZLE_X, 0, 2, [0.3, 1.0], 1.0);
        b.paint(TEAM).pattern(pattern::PLAIN);
        plate_on_hull(b, 138.0, 143.0, 2, 3, [0.3, 1.0], 1.55);
        if fine {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            plate_on_hull(b, 116.0, MUZZLE_X - 1.0, 3, 5, [0.25, 0.6], 0.5);
        }
    });
}

/// The coil ring the rails leave the engineering hull's bulkhead through.
fn coil(b: &mut MeshBuilder) {
    let ring = chamfered_rect(v2(RAIL_OUT + 3.0, RAIL_H + 3.2), 2.6);
    let ring = ring.iter().map(|p| [p[0], p[1] + RAIL_Z]).collect::<Vec<_>>();
    b.paint(METAL).pattern(pattern::PLAIN);
    b.extrude_x(&ring, BREECH_X - 6.0, BREECH_X + 5.6);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.extrude_x(&ring.iter().map(|p| [p[0] * 1.08, (p[1] - RAIL_Z) * 1.08 + RAIL_Z]).collect::<Vec<_>>(), BREECH_X + 3.0, BREECH_X + 4.4);
}

/// The capacitor banks either side of the trench ahead of the bulkhead: armoured trays of
/// capacitor drums, bus bars back into the bulkhead.
fn capacitors(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        let (x0, x1, y0, y1) = (-36.0, -16.0, 11.2, 19.6);
        let (_, _, c) = station(-26.0);
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.frustum(v3((x0 + x1) * 0.5, (y0 + y1) * 0.5, c - 0.5), v2(x1 - x0, y1 - y0), v2(x1 - x0 - 2.0, y1 - y0 - 2.0), 3.5, v2(0.0, 0.0));
        let top = c + 3.0;
        let sides = if fine { 10 } else { 6 };
        let y = (y0 + y1) * 0.5;
        b.paint(METAL).pattern(pattern::PLAIN);
        b.cylinder_between(v3(x0 + 2.0, y, top + 2.2), v3(x1 - 2.0, y, top + 2.2), 2.6, 2.6, sides);
        if fine {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for x in [x0 + 5.0, x1 - 5.0] {
                b.cylinder_between(v3(x - 0.8, y, top + 2.2), v3(x + 0.8, y, top + 2.2), 2.9, 2.9, sides);
            }
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.beam(v3(x0 + 1.0, y, c + 1.4), v3(ENGINE_FORE - 3.0, y, c + 4.0), v2(2.0, 1.6), v2(2.0, 1.6));
    });
}

/// The port armoured wing (mirrored): a thick swept slab from the engineering hull's side
/// out over the nacelle, its leading edge raked back, carrying the nacelle beneath it; a
/// strut under it to the nacelle's inboard face.
fn wing(b: &mut MeshBuilder) {
    let fine = b.fine();
    let root = ENGINE_SIDE - 1.0;
    let tip = NACELLE_Y + NACELLE_HW + 4.0;
    let top = NACELLE_Z + NACELLE_HH;
    // Root chord along the hull's side, tip chord over the nacelle's outer edge.
    let chord = |y: f32, x0: f32, x1: f32, z0: f32, z1: f32| {
        vec![v3(x0, y, z0), v3(x1 - 10.0, y, z0), v3(x1, y, z0 + (z1 - z0) * 0.5), v3(x1 - 4.0, y, z1), v3(x0, y, z1)]
    };
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&[chord(root, ENGINE_AFT + 2.0, -50.0, top - 2.0, top + 5.0), chord(tip, NACELLE_AFT + 4.0, -104.0, top + 0.5, top + 4.0)], true, true);
    // Armour plates on its back, stepping out along the span, and a raked tip plate.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    let plates: &[(f32, f32)] = if fine { &[(38.0, 52.0), (54.0, 68.0), (70.0, 80.0)] } else { &[(38.0, 80.0)] };
    for &(y0, y1) in plates {
        let lead = |y: f32| -50.0 + (y - root) / (tip - root) * (-104.0 + 50.0);
        let trail = |y: f32| ENGINE_AFT + 2.0 + (y - root) / (tip - root) * (NACELLE_AFT + 4.0 - ENGINE_AFT - 2.0);
        let z = |y: f32| top + 5.0 + (y - root) / (tip - root) * (4.0 - 5.0) + 0.1;
        b.loft(
            &[
                vec![v3(trail(y0) + 8.0, y0, z(y0)), v3(lead(y0) - 8.0, y0, z(y0)), v3(lead(y0) - 8.0, y0, z(y0) + 1.4), v3(trail(y0) + 8.0, y0, z(y0) + 1.4)],
                vec![v3(trail(y1) + 8.0, y1, z(y1)), v3(lead(y1) - 8.0, y1, z(y1)), v3(lead(y1) - 8.0, y1, z(y1) + 1.4), v3(trail(y1) + 8.0, y1, z(y1) + 1.4)],
            ],
            true,
            true,
        );
    }
    // The owner's panel on the wing, where the Paris wears its crest.
    b.paint(TEAM).pattern(pattern::PLAIN);
    b.block(v3(-128.0, 56.0, top + 5.0), v3(-112.0, 62.0, top + 5.6));
    // The strut down to the nacelle.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            vec![v3(ENGINE_AFT + 4.0, root, NACELLE_Z - 8.0), v3(-80.0, root, NACELLE_Z - 8.0), v3(-70.0, root, top), v3(ENGINE_AFT + 4.0, root, top)],
            vec![v3(NACELLE_AFT + 10.0, NACELLE_Y - NACELLE_HW + 1.0, NACELLE_Z - 6.0), v3(-92.0, NACELLE_Y - NACELLE_HW + 1.0, NACELLE_Z - 6.0), v3(-84.0, NACELLE_Y - NACELLE_HW + 1.0, top), v3(NACELLE_AFT + 10.0, NACELLE_Y - NACELLE_HW + 1.0, top)],
        ],
        true,
        true,
    );
}

/// A ring round the port nacelle's armoured section at `x`, `scale` of full size.
fn nacelle_ring(x: f32, scale: f32, cut: f32) -> Vec<Vec3> {
    chamfered_rect(v2(NACELLE_HW * scale, NACELLE_HH * scale), cut * scale)
        .iter()
        .map(|p| v3(x, NACELLE_Y + p[0], NACELLE_Z + p[1]))
        .collect()
}

/// The port engine nacelle (mirrored for starboard): a big flat armoured pod under the
/// wing holding two drives side by side, a stepped nose with an upper cowl running ahead of
/// a dark intake, belted flanks, a squared thrust frame round both bells. It reaches well
/// astern of the engineering hull.
fn nacelle(b: &mut MeshBuilder) {
    let fine = b.fine();
    let cut = 5.0;
    let (y, z) = (NACELLE_Y, NACELLE_Z);
    let (hw, hh) = (NACELLE_HW, NACELLE_HH);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            nacelle_ring(NACELLE_AFT, 0.94, cut),
            nacelle_ring(NACELLE_AFT + 4.0, 1.0, cut),
            nacelle_ring(NACELLE_NOSE - 22.0, 1.0, cut),
            nacelle_ring(NACELLE_NOSE - 10.0, 0.8, cut),
        ],
        true,
        true,
    );
    // The upper cowl: an armoured lid over the top half, running on past the body to a
    // wedge nose; the dark intake mouth beneath it.
    let cowl = |x: f32, s: f32| {
        vec![
            v3(x, y - hw * s, z),
            v3(x, y + hw * s, z),
            v3(x, y + hw * s, z + hh * 0.7),
            v3(x, y + (hw - 6.0) * s, z + hh + 1.6),
            v3(x, y - (hw - 6.0) * s, z + hh + 1.6),
            v3(x, y - hw * s, z + hh * 0.7),
        ]
    };
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&[cowl(-134.0, 1.03), cowl(NACELLE_NOSE - 20.0, 1.03), cowl(NACELLE_NOSE, 0.7)], true, true);
    b.paint(TREAD).pattern(pattern::NONE);
    b.block(v3(NACELLE_NOSE - 10.4, y - hw * 0.72, z - hh * 0.62), v3(NACELLE_NOSE - 9.6, y + hw * 0.72, z - 0.8));
    // Belts along the outboard face, and a band of the owner's colour.
    let belts: &[(f32, f32)] = if fine { &[(-154.0, -132.0), (-128.0, -106.0), (-102.0, -80.0)] } else { &[(-154.0, -80.0)] };
    for &(x0, x1) in belts {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.block(v3(x0, y + hw - 0.4, z - hh * 0.6), v3(x1, y + hw + 1.4, z + hh * 0.5));
    }
    if fine {
        b.paint(TEAM).pattern(pattern::PLAIN);
        b.block(v3(-76.0, y + hw - 0.4, z - hh * 0.6), v3(-72.0, y + hw + 1.6, z + hh * 0.5));
    }
    // Radiator ribs along the underside.
    if fine {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for k in 0..6 {
            let x = -142.0 + k as f32 * 9.0;
            b.block(v3(x - 0.7, y - hw + 5.0, z - hh - 0.8), v3(x + 0.7, y + hw - 5.0, z - hh + 0.4));
        }
    }
    // The thrust frame: a squared collar round both bells' necks, and a web between them.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(&[nacelle_ring(NACELLE_AFT + 0.5, 0.97, cut), nacelle_ring(NACELLE_AFT - 3.0, 0.92, cut)], false, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.block(v3(NACELLE_AFT - 12.0, y - 1.2, z - hh * 0.7), v3(NACELLE_AFT - 2.0, y + 1.2, z + hh * 0.7));
    for port in [NOZZLES[0], NOZZLES[1]] {
        capital::drive_deep(b, Vec3::from(port), DRIVE_SIZE);
    }
}

/// The engineering hull's stern: a dark stern plate, a pair of small manoeuvring drives,
/// radiator fins along the deck edges.
fn stern(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(ENGINE_AFT - 1.5, -24.0, KEEL + 9.0), v3(ENGINE_AFT, 24.0, ENGINE_DECK - 8.0));
    b.mirror_y(|b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.chamfered_box(v3(ENGINE_AFT - 2.0, 13.0, 42.0), v3(5.0, 12.0, 12.0), 2.0);
        b.paint(TREAD).pattern(pattern::NONE);
        b.block(v3(ENGINE_AFT - 4.7, 9.0, 38.0), v3(ENGINE_AFT - 4.5, 17.0, 46.0));
        let fins = if fine { 6 } else { 0 };
        for k in 0..fins {
            let x = ENGINE_AFT + 6.0 + k as f32 * (40.0 / 5.0);
            b.paint(PLATING_DARK).pattern(pattern::PLAIN);
            b.beam(v3(x, 22.0, ENGINE_DECK - 0.5), v3(x - 3.0, 22.0, ENGINE_DECK + 4.5), v2(1.0, 6.0), v2(0.8, 5.0));
        }
    });
}

/// The belly: lift jets in dark wells.
fn belly(b: &mut MeshBuilder) {
    for port in LIFT_JETS {
        let p = Vec3::from(port);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum(p + Vec3::Z * 0.6, v2(13.0, 12.0), v2(10.0, 9.5), 1.6, v2(0.0, 0.0));
        capital::lift_jet(b, p, 0.85);
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(ENGINE_AFT + 4.0, -3.0, KEEL - 1.0), v3(-106.0, 3.0, KEEL + 0.2));
}

/// The spinal rail itself: two pale conductor rails down the trench from the breech
/// housing to the prow's face, a ladder of dark clamp yokes, heavy containment collars
/// bridging the trench, and the power runs along the trench lips.
fn spinal(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (h, i, o) = (RAIL_H, RAIL_IN, RAIL_OUT);
    // The rails, bevelled on their outer edges.
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let bevel = 1.0;
        b.extrude_x(
            &[
                [i, RAIL_Z - h],
                [o - bevel, RAIL_Z - h],
                [o, RAIL_Z - h + bevel],
                [o, RAIL_Z + h - bevel],
                [o - bevel, RAIL_Z + h],
                [i, RAIL_Z + h],
            ],
            BREECH_X - 4.0,
            MUZZLE_X - 16.0,
        );
    });
    // The rail bed: a dark insulator between the rails and the keel.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(BREECH_X, -o - 0.8, FLOOR - 0.1), v3(MUZZLE_X - 14.0, o + 0.8, RAIL_Z - h + 0.4));
    if fine {
        // The bus in the slot between the rails, low down.
        b.paint(METAL).pattern(pattern::PLAIN);
        b.block(v3(BREECH_X, -0.9, RAIL_Z - h + 0.3), v3(MUZZLE_X - 16.0, 0.9, RAIL_Z - h + 1.3));
    }
    // Clamp yokes: frames round both rails, every 10 m (every third at the middle level).
    let yoke = chamfered_rect(v2(o + 1.0, h + 1.0), 1.4);
    let yoke = yoke.iter().map(|p| [p[0], p[1] + RAIL_Z]).collect::<Vec<_>>();
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let mut k = 0;
    let mut x = BREECH_X + 6.0;
    while x < PROW_ROOF {
        let clear = COLLARS.iter().all(|c| (x - c).abs() > 5.0);
        if clear && (fine || k % 3 == 0) {
            b.extrude_x(&yoke, x - 0.6, x + 0.6);
        }
        k += 1;
        x += 10.0;
    }
    // Containment collars: squared rings bridging the trench, standing proud of the deck.
    for &cx in COLLARS.iter() {
        collar(b, cx);
    }
    // A dark liner down the trench walls, so the pale rails stand out of a black channel.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let lips = |to: f32| {
        let mut xs = vec![BREECH_X + 2.0];
        xs.extend(STATIONS.iter().map(|s| s[0]).filter(|&x| x > BREECH_X + 2.0 && x < to));
        xs.push(to);
        xs
    };
    b.mirror_y(|b| {
        let at = |x: f32| {
            let (_, _, c) = station(x);
            vec![v3(x, TRENCH - 0.35, FLOOR), v3(x, TRENCH - 0.05, FLOOR), v3(x, TRENCH - 0.05, c - 1.0), v3(x, TRENCH - 0.35, c - 1.0)]
        };
        b.loft(&lips(PROW_ROOF).iter().map(|&x| at(x)).collect::<Vec<_>>(), true, true);
    });
    // Coamings: square armoured walls along both trench lips, standing above the deck.
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let rings = lips(PROW_ROOF + 2.0)
            .iter()
            .map(|&x| {
                let (_, _, c) = station(x);
                vec![v3(x, TRENCH - 0.3, c - 1.2), v3(x, TRENCH + 2.4, c - 1.2), v3(x, TRENCH + 2.4, c + 2.4), v3(x, TRENCH - 0.3, c + 2.4)]
            })
            .collect::<Vec<_>>();
        b.loft(&rings, true, true);
    });
    // Power runs along both trench lips: paired conduits from the capacitor banks forward.
    b.paint(METAL).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        let ys: &[f32] = if fine { &[11.4, 13.0] } else { &[12.2] };
        for &y in ys {
            let mut last: Option<Vec3> = None;
            for x in [-16.0, 8.0, 52.0, PROW_ROOF] {
                let (_, _, c) = station(x);
                let p = v3(x, y, c + 0.8);
                if let Some(a) = last {
                    b.cylinder_between(a, p, 0.7, 0.7, if fine { 6 } else { 4 });
                }
                last = Some(p);
            }
        }
    });
}

/// x of the containment collars (forward of `PROW_ROOF` the prow's armour closes over the
/// rails).
const COLLARS: [f32; 3] = [-12.0, 56.0, 68.0];

/// One containment collar at `x`: a squared armoured ring round the rails, bridging the
/// trench and standing proud of the deck, with junction boxes where the power runs feed it.
fn collar(b: &mut MeshBuilder, x: f32) {
    let (_, _, c) = station(x);
    let top = c + 5.0;
    let (hx, span) = (2.4, TRENCH + 4.0);
    let bottom = FLOOR + 0.5;
    let mid = (top + bottom) * 0.5;
    let fine = b.fine();
    let frame = |half: Vec2, cut: f32| {
        if fine {
            chamfered_rect(half, cut)
        } else {
            vec![[half.x, -half.y], [half.x, half.y], [-half.x, half.y], [-half.x, -half.y]]
        }
    };
    let outer = frame(v2(span, (top - bottom) * 0.5), 2.0);
    let inner = frame(v2(RAIL_OUT + 1.4, RAIL_H + 1.4), 1.4);
    let ring = |x: f32, pts: &Vec<[f32; 2]>, z0: f32| pts.iter().map(|p| v3(x, p[0], p[1] + z0)).collect::<Vec<_>>();
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(x - hx, &inner, RAIL_Z),
            ring(x - hx, &outer, mid),
            ring(x + hx, &outer, mid),
            ring(x + hx, &inner, RAIL_Z),
            ring(x - hx, &inner, RAIL_Z),
        ],
        false,
        false,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(x - 0.9, -span - 0.3, top - 2.4), v3(x + 0.9, span + 0.3, top + 0.4));
    if fine {
        b.mirror_y(|b| {
            b.cuboid(v3(x - hx - 2.0, 10.8, c + 1.6), v3(3.0, 3.2, 2.4));
        });
    }
}

/// The bore: an octagonal mouth let flush into the prow's flat face, a dark frame round
/// it standing a hand proud, and a black throat narrowing back onto the rails' ends. No
/// barrel stands out of the hull.
fn bore(b: &mut MeshBuilder) {
    let oct = |r: f32| (0..8).map(|k| {
        let a = (k as f32 + 0.5) * std::f32::consts::TAU / 8.0;
        [a.cos() * r, a.sin() * r]
    }).collect::<Vec<_>>();
    let at = |x: f32, pts: &[[f32; 2]]| pts.iter().map(|p| v3(x, p[0], p[1] + RAIL_Z)).collect::<Vec<_>>();
    let (_, _, c) = station(MUZZLE_X);
    // The face plate over the trench's end: an octagonal ring from the trench's square
    // outline in to the bore.
    let square = |x: f32| {
        let (hy, lo, hi) = (TRENCH + 0.2, FLOOR - RAIL_Z, c - 2.0 - RAIL_Z);
        let pts = [[hy, 0.0], [hy, hi], [0.0, hi], [-hy, hi], [-hy, 0.0], [-hy, lo], [0.0, lo], [hy, lo]];
        // Match the octagon's corner order (starting at 22.5 degrees).
        let order = [1, 2, 3, 4, 5, 6, 7, 0];
        order.iter().map(|&i| v3(x, pts[i][0], pts[i][1] + RAIL_Z)).collect::<Vec<_>>()
    };
    let mouth = oct(6.2);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&[at(MUZZLE_X, &mouth), square(MUZZLE_X), square(MUZZLE_X - 3.0), at(MUZZLE_X - 3.0, &mouth), at(MUZZLE_X, &mouth)], false, false);
    // The frame: a dark octagonal band round the mouth.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(&[at(MUZZLE_X + 0.5, &oct(6.2)), at(MUZZLE_X + 0.5, &oct(7.8)), at(MUZZLE_X - 0.5, &oct(7.8)), at(MUZZLE_X - 0.5, &oct(6.2)), at(MUZZLE_X + 0.5, &oct(6.2))], false, false);
    // The throat: black, down onto the rails.
    b.paint(TREAD).pattern(pattern::NONE);
    b.loft(&[at(MUZZLE_X - 0.5, &oct(6.2)), at(MUZZLE_X - 16.0, &oct(5.6))], false, true);
    if b.fine() {
        // Bolt heads round the frame.
        b.paint(METAL).pattern(pattern::PLAIN);
        for p in oct(7.0) {
            b.cuboid(v3(MUZZLE_X + 0.6, p[0], RAIL_Z + p[1]), v3(0.6, 0.9, 0.9));
        }
    }
}

/// The bridge: a stepped armoured citadel on the breech housing, a dark window band under
/// a visor, armour wings, and the mast with the search radar's turning array and a
/// fire-control dome.
fn tower(b: &mut MeshBuilder) {
    let fine = b.fine();
    let x = -76.0;
    let deck = 66.0;
    // Three armoured tiers, stepping back and in.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum(v3(x, 0.0, deck), v2(40.0, 30.0), v2(37.0, 27.0), 11.0, v2(-1.0, 0.0));
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.frustum(v3(x - 1.0, 0.0, deck + 11.0), v2(37.0, 27.0), v2(37.0, 27.0), 0.8, v2(0.0, 0.0));
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(v3(x - 3.0, 0.0, deck + 11.8), v2(30.0, 22.0), v2(29.0, 21.0), 6.0, v2(-0.5, 0.0));
    b.paint(GLASS).pattern(pattern::PLAIN);
    b.frustum(v3(x - 3.5, 0.0, deck + 17.8), v2(29.0, 21.0), v2(27.0, 19.0), 2.4, v2(-0.6, 0.0));
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum(v3(x - 4.2, 0.0, deck + 20.2), v2(29.4, 21.4), v2(24.0, 17.0), 1.8, v2(-1.2, 0.0));
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(v3(x - 7.0, 0.0, deck + 22.0), v2(16.0, 13.0), v2(13.0, 10.0), 5.0, v2(-0.8, 0.0));
    b.paint(TEAM).pattern(pattern::PLAIN);
    b.frustum(v3(x, 0.0, deck + 2.0), v2(40.1, 30.1), v2(39.8, 29.8), 1.4, v2(-0.2, 0.0));
    b.mirror_y(|b| {
        // Armour wings down the citadel's flanks.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                vec![v3(x - 20.0, 15.5, deck), v3(x - 20.0, 17.5, deck + 1.0), v3(x - 20.0, 17.5, deck + 9.0), v3(x - 20.0, 15.0, deck + 10.0)],
                vec![v3(x + 16.0, 15.5, deck), v3(x + 16.0, 17.5, deck + 1.0), v3(x + 16.0, 16.8, deck + 7.0), v3(x + 16.0, 15.0, deck + 8.0)],
            ],
            true,
            true,
        );
        if fine {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            for dx in [-14.0, -6.0, 2.0, 10.0] {
                b.beam(v3(x + dx, 17.7, deck + 1.5), v3(x + dx, 17.7, deck + 7.8), v2(0.9, 1.2), v2(0.9, 1.2));
            }
        }
    });
    // The mast and the search radar: a broad slatted array turning on its head.
    let top = deck + 27.0;
    let [rx, _, rz] = RADAR;
    b.paint(METAL).pattern(pattern::PLAIN);
    b.beam(v3(rx, 0.0, top), v3(rx, 0.0, rz - 1.5), v2(3.0, 3.0), v2(1.6, 1.6));
    b.paint(PLATING_DARK);
    b.prism(v3(rx, 0.0, rz - 2.2), b.sides(8), 2.6, 2.2, 1.4);
    if b.mid() {
        b.set_spinner_pivot(v3(rx, 0.0, rz));
        b.with_part(part::SPINNER, |b| {
            // The array face, backed by a truss, tilted up a little.
            b.paint(PLATING);
            b.loft(
                &[
                    vec![v3(rx + 1.6, -15.0, rz - 0.6), v3(rx + 1.6, 15.0, rz - 0.6), v3(rx + 0.6, 15.0, rz + 5.6), v3(rx + 0.6, -15.0, rz + 5.6)],
                    vec![v3(rx + 0.2, -15.0, rz - 0.6), v3(rx + 0.2, 15.0, rz - 0.6), v3(rx - 0.8, 15.0, rz + 5.6), v3(rx - 0.8, -15.0, rz + 5.6)],
                ],
                true,
                true,
            );
            b.paint(ACCENT);
            b.cuboid(v3(rx - 1.4, 0.0, rz + 2.4), v3(2.4, 26.0, 1.2));
            b.cuboid(v3(rx, 0.0, rz + 0.2), v3(2.6, 3.0, 1.6));
            if b.fine() {
                // Slats across the face.
                for k in 0..5 {
                    let z = rz + 0.2 + k as f32 * 1.2;
                    b.cuboid(v3(rx + 1.5 - k as f32 * 0.2, 0.0, z), v3(0.3, 29.4, 0.3));
                }
            }
        });
    }
    if fine {
        b.paint(METAL);
        b.mirror_y(|b| {
            b.beam(v3(rx + 4.0, 4.0, top), v3(rx + 0.4, 0.6, top + 11.0), v2(0.7, 0.7), v2(0.5, 0.5));
            b.beam(v3(x - 12.0, 5.0, deck + 27.0), v3(x - 12.2, 5.0, deck + 36.0), v2(0.4, 0.4), v2(0.2, 0.2));
        });
        // The fire-control dome on the top tier, forward of the mast.
        b.paint(PLATING);
        b.spheroid(v3(x - 1.0, 0.0, deck + 27.2), v3(3.6, 3.6, 3.0), 10, 5);
        b.paint(GLOW_RED);
        b.cuboid(v3(rx, 3.0, top + 12.0), v3(0.7, 0.7, 0.7));
        b.cuboid(v3(rx, -3.0, top + 12.0), v3(0.7, 0.7, 0.7));
    }
}

/// The two vertical-launch rocket cell blocks either side of the spine: armoured boxes
/// with a grid of square hatches whose tops are the rockets' muzzles (`CELLS`).
fn cells(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        let (x0, x1, y0, y1) = (13.0, 47.0, 10.8, 22.2);
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.frustum(v3((x0 + x1) * 0.5, (y0 + y1) * 0.5, CELL_DECK - 1.0), v2(x1 - x0, y1 - y0), v2(x1 - x0 - 1.6, y1 - y0 - 1.6), 4.0, v2(0.0, 0.0));
        let top = CELL_DECK + 3.0;
        if fine {
            for c in CELLS {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.cuboid_open(v3(c[0], c[1], top + 0.5), v3(6.6, 5.4, 1.0));
                b.paint(METAL).pattern(pattern::PLAIN);
                b.cuboid_open(v3(c[0], c[1], top + 1.1), v3(5.0, 3.8, 0.2));
            }
            vent(b, v3((x0 + x1) * 0.5, y1 - 1.0, top), v2(x1 - x0 - 6.0, 1.2), 8, ACCENT);
        } else {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.cuboid_open(v3((x0 + x1) * 0.5, (y0 + y1) * 0.5, top + 0.5), v3(x1 - x0 - 3.0, y1 - y0 - 3.0, 1.0));
        }
    });
}

/// Fittings for `LAMPS`: flood housings under the belly, steady sidelights on the
/// nacelles, strobes.
fn lamps(b: &mut MeshBuilder) {
    if !b.mid() {
        return;
    }
    let l = LAMPS;
    for p in l.floods {
        let p = Vec3::from(*p);
        b.paint(ACCENT);
        b.cuboid(p + Vec3::Z * 1.0, v3(4.0, 4.0, 1.8));
        b.paint(GLOW_LAMP);
        b.cuboid(p + Vec3::Z * 0.05, v3(3.0, 3.0, 0.2));
    }
    for (p, glow) in [(l.nav_port, GLOW_NAV_RED), (l.nav_starboard, GLOW_NAV_GREEN)] {
        let p = Vec3::from(p);
        b.paint(ACCENT);
        b.cuboid(p - Vec3::Y * p.y.signum() * 0.9, v3(3.6, 1.0, 2.8));
        b.paint(glow);
        b.cuboid(p, v3(2.0, 0.8, 1.6));
    }
    b.paint(GLOW_LAMP);
    for p in l.strobes {
        b.cuboid(Vec3::from(*p), v3(1.2, 1.2, 1.2));
    }
}

/// The chin house's post: a slim armoured drum from the spine's keel down to the house's
/// roof, so the house hangs clear under the overhang and its rails depress freely.
fn chin_post(b: &mut MeshBuilder, pivot: Vec3) {
    let roof = pivot.z + HOUSE_ROOF * TURRET_SCALE;
    let (_, keel, _) = station(pivot.x);
    b.at(v3(pivot.x, pivot.y, 0.0), |b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.prism(v3(-3.0, 0.0, roof - 0.2), b.sides(10), 6.4, 8.4, keel - roof + 0.8);
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.prism(v3(-3.0, 0.0, roof - 0.6), b.sides(10), 7.0, 7.0, 1.2);
        }
    });
}

/// A turret's barbette: an armoured drum from `foot` up under the house.
fn barbette(b: &mut MeshBuilder, pivot: Vec3, foot: f32) {
    let top = pivot.z - (HOUSE_SINK + 1.0) * TURRET_SCALE;
    let plan = chamfered_rect(v2(13.0, 13.0), 4.8);
    b.at(v3(pivot.x, pivot.y, 0.0), |b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.loft_z(&plan, &[Section::new(foot, 1.12), Section::new(top - 1.2, 1.0)]);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft_z(&plan, &[Section::new(top - 1.2, 0.94), Section::new(top, 0.94)]);
    });
}

/// A flank sponson (port; mirrored for starboard) run out low from the hull side to carry
/// the flank house clear of the hull, its deck well under the gun so the rails depress
/// to the pitch limit over nothing but air.
fn sponson(b: &mut MeshBuilder) {
    let [x, y, z] = TURRETS[2].0;
    // The deck under the house's ring, low enough that the rails at 35 degrees down
    // pass over its edge; a slim post carries the ring.
    let deck = z - 10.5;
    let ring = z - (HOUSE_SINK + 1.0) * TURRET_SCALE;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            vec![v3(x - 22.0, 27.0, KEEL + 1.0), v3(x + 18.0, 27.0, KEEL + 1.0), v3(x + 18.0, 28.5, 31.0), v3(x - 22.0, 28.5, 31.0)],
            vec![v3(x - 9.0, y - 3.0, deck - 5.0), v3(x + 7.0, y - 3.0, deck - 5.0), v3(x + 7.0, y - 3.0, deck), v3(x - 9.0, y - 3.0, deck)],
        ],
        true,
        true,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.prism(v3(x, y, deck - 5.0), b.sides(10), 7.6, 7.2, 5.0);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(x, y, deck), b.sides(10), 5.0, 5.0, ring - deck);
    b.prism(v3(x, y, deck - 7.0), b.sides(10), 4.0, 7.6, 2.0);
    if b.fine() {
        // Braces under the sponson.
        b.paint(PLATING_DARK);
        for dx in [-8.0, 6.0] {
            b.beam(v3(x + dx, 28.0, KEEL + 2.0), v3(x + dx * 0.7, y - 5.0, deck - 5.0), v2(2.0, 2.0), v2(1.4, 1.4));
        }
    }
}

/// How far the house's floor sits under its pivot (the rail's axis), as authored.
const HOUSE_SINK: f32 = 3.6;
/// The house roof over the pivot.
const HOUSE_ROOF: f32 = 4.2;

/// Plan of a rail cannon house about its pivot: a pointed wedge face, swept cheeks, a
/// squared bustle.
fn house_plan() -> Vec<[f32; 2]> {
    vec![
        [9.5, 0.0],
        [7.0, 5.2],
        [3.0, 7.4],
        [-8.0, 7.4],
        [-10.5, 5.8],
        [-10.5, -5.8],
        [-8.0, -7.4],
        [3.0, -7.4],
        [7.0, -5.2],
    ]
}

/// A rail cannon turret bound to `weapon`, turning about `pivot`, authored facing +X with
/// its rails ending `TURRET_REACH` ahead: a low angular house on a ring, the rail gun
/// (`parts::rail_gun`) and its mantlet pitching and kicking inside it.
fn rail_turret(b: &mut MeshBuilder, weapon: usize, pivot: Vec3) {
    b.with_house(weapon, pivot, 2.0, |b| {
        let at = Affine3A::from_translation(pivot) * Affine3A::from_scale(Vec3::splat(TURRET_SCALE));
        b.with(at, turret_body);
    });
}

/// The house and its gun about the pivot at the origin, at the authored size.
fn turret_body(b: &mut MeshBuilder) {
    let base = -HOUSE_SINK;
    let roof = HOUSE_ROOF;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(0.0, 0.0, base - 1.0), b.sides(12), 8.2, 8.2, 1.05);
    b.loft_z(&house_plan(), &[Section::new(base, 0.95), Section::new(base + 0.6, 0.95)]);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft_z(
        &house_plan(),
        &[
            Section::new(base + 0.6, 1.0),
            Section::new(base + 2.4, 1.0),
            Section::scaled(roof, 0.78, 0.78).shifted(-1.8, 0.0),
        ],
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(
        &house_plan(),
        &[
            Section::scaled(roof - 0.02, 0.62, 0.62).shifted(-2.4, 0.0),
            Section::scaled(roof + 0.3, 0.6, 0.6).shifted(-2.5, 0.0),
        ],
    );
    b.with_recoil(|b| {
        // The mantlet the rails pass through, and the rails.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.chamfered_box(v3(6.4, 0.0, 0.0), v3(5.0, 7.2, 5.4), 1.2);
        rail_gun(b, -Vec3::X * 2.0, Vec3::X * (TURRET_REACH / TURRET_SCALE), v2(1.3, 3.6), 2.2, Emitter::Unlit);
    });
    // Rangefinder across the back of the roof and the owner's panel.
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.chamfered_box(v3(-6.8, 0.0, HOUSE_ROOF + 0.4), v3(2.0, 12.0, 0.8), 0.5);
    team_panel(b, v3(-1.5, 0.0, HOUSE_ROOF + 0.3), v2(3.4, 5.0));
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.mirror_y(|b| {
            b.chamfered_box(v3(0.8, 4.6, HOUSE_ROOF + 0.5), v3(1.8, 1.4, 0.6), 0.3);
            b.plate(v3(-4.4, 3.2, HOUSE_ROOF + 0.3), v2(1.6, 1.6), 0.1, 0.03);
        });
        vent(b, v3(-9.4, 0.0, HOUSE_ROOF - 1.2), v2(1.2, 6.0), 4, METAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::build_model_fitted;

    #[test]
    fn the_frigate_keeps_its_budgets_rig_and_guns() {
        let model = build_model_fitted("space_frigate", 150.0, 70.0, 3, &[]).unwrap();
        let tris: Vec<_> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
        println!("Resolute triangles: {tris:?}");
        assert!(tris[0] <= 17000 && tris[1] as f32 <= tris[0] as f32 * 0.45 + 20.0 && tris[2] < 60, "{tris:?}");
        assert_eq!(model.houses.len(), 4);
        for (house, (pivot, _)) in model.houses.iter().zip(TURRETS) {
            assert_eq!(house.pivot, pivot);
        }
        for (i, house) in model.houses.iter().enumerate() {
            assert_eq!(house.weapon as usize, i + 1);
        }
        let mesh = &model.lods[0];
        for part in [part::GEAR, part::GEAR_STRUT, part::GEAR_FOOT, part::GEAR_DOOR, part::DRIVE] {
            assert!(mesh.vertices.iter().any(|v| v.part == part), "no part {part}");
        }
        let span = |axis: usize| {
            mesh.vertices.iter().map(|v| v.pos[axis]).fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
        };
        let (lo, hi) = span(0);
        // Shorter than the Bastion (about 330 m).
        assert!((290.0..=328.0).contains(&(hi - lo)), "hull length {}", hi - lo);
        assert!((hi - MUZZLE_X).abs() < 1.0, "the bore is in the foremost face: {hi}");
        let (ylo, yhi) = span(1);
        assert!(yhi <= 86.0 && ylo >= -86.0, "beam {ylo}..{yhi}");
        assert!(span(2).0 >= -0.01);
        // Stowed, every leg piece is inside the hull, above the keel.
        for v in &mesh.vertices {
            if [part::GEAR, part::GEAR_STRUT, part::GEAR_FOOT].contains(&v.part) {
                let r = capital::stowed(&RIG, Vec3::from(v.pos), v.part, v.material);
                assert!(r.z >= KEEL + 0.2, "{:?} stows to {r}", v.part);
            }
        }
        // Nothing on the rails glows: ARC rails are unlit hardware.
        for v in &mesh.vertices {
            let p = Vec3::from(v.pos);
            if p.y.abs() < TRENCH && p.x > BREECH_X && p.x < MUZZLE_X + 1.0 && p.z > FLOOR && p.z < 70.0 {
                assert!(![GLOW, GLOW_ORANGE, GLOW_AMBER].contains(&v.material), "a lit spinal part at {p}");
            }
        }
        // The rig's numbers are the ones the model was built on.
        assert_eq!(crate::models::capital_rig("space_frigate"), Some(RIG.gpu()));
        assert_eq!(crate::models::lift_jets("space_frigate"), &LIFT_JETS[..]);
        assert_eq!(crate::models::aircraft_exhausts("space_frigate"), &NOZZLES[..]);
    }

    /// Software previews of the frigate from several sides (PPM), for a quick look.
    #[test]
    #[ignore]
    fn frigate_previews() {
        let dir = std::path::PathBuf::from(std::env::var("MODEL_DUMP_DIR").unwrap_or("/tmp/frigate".into()));
        std::fs::create_dir_all(&dir).unwrap();
        let model = build_model_fitted("space_frigate", 150.0, 70.0, 3, &[]).unwrap();
        for az in [-38.0f32, 30.0, 90.0, 142.0, 200.0] {
            crate::models::preview::render(&model.lods[0], 900, az)
                .write_ppm(&dir.join(format!("frigate_{}.ppm", az as i32)))
                .unwrap();
        }
        crate::models::preview::render(&model.lods[1], 600, -38.0).write_ppm(&dir.join("frigate_lod1.ppm")).unwrap();
        crate::models::preview::render(&model.lods[2], 600, -38.0).write_ppm(&dir.join("frigate_lod2.ppm")).unwrap();
    }
}
