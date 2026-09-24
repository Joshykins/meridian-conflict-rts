//! Bastion: a faceted capital spacecraft with a closed stern and a ventral vehicle bay.
//! The belly ramp sits entirely beneath the hull: hinge (-16,34), lip (-84,0).
//! +X is forward. Drive mouths and effect sockets share NOZZLES.
//!
//! Its landing legs, drives, lift jets and gun houses come from the shared spacecraft rig
//! (`capital.rs`); `RIG` says where they are, and `entity.wgsl` animates from it.

use super::capital::{self, CapitalRig, Leg};
use super::*;

pub const RAMP_HINGE: [f32; 2] = [-16.0, 34.0];
pub const RAMP_LIP: f32 = -84.0;
pub const NOZZLES: [[f32; 3]; 4] = [
    [-164.0, -57.0, 54.0],
    [-164.0, -31.0, 54.0],
    [-164.0, 31.0, 54.0],
    [-164.0, 57.0, 54.0],
];
/// Downward lift jets under the belly (model space, mouth centres), fired as the ship
/// comes down and lifts off. The renderer's thrust wash and ground scorch read these.
pub const LIFT_JETS: [[f32; 3]; 4] = [
    [-118.0, -50.0, 33.0],
    [-118.0, 50.0, 33.0],
    [92.0, -30.0, 33.0],
    [92.0, 30.0, 33.0],
];
/// The four gun turrets (pivot, rest facing in degrees, +X forward, +Y left): nose,
/// port and starboard sponsons, stern. `air.ron` weapons carry the same pivots.
pub const TURRETS: [([f32; 3], f32); 4] = [
    ([160.0, 0.0, 38.0], 0.0),
    ([-40.0, 62.0, 50.0], 90.0),
    ([-40.0, -62.0, 50.0], -90.0),
    ([-156.0, 0.0, 70.0], 180.0),
];
/// What `entity.wgsl` animates (`models::capital_rig`): the legs in their belly bays, the
/// four drives, the lift jets and the ramp.
pub const RIG: CapitalRig = CapitalRig {
    legs: Some([
        Leg { hinge: [64.0, 26.0, 36.0], stow: 1.0, bay: [36.0, 68.0, 21.0, 33.0], size: 1.0 },
        Leg { hinge: [-78.0, 35.0, 36.0], stow: -1.0, bay: [-83.5, -47.5, 30.0, 44.0], size: 1.0 },
    ]),
    door_hinge: 29.7,
    drives: Some(([-164.0, 54.0, 31.0, 57.0], 1.0)),
    lift_jets: Some(([92.0, 30.0, -118.0, 50.0], 33.0)),
    ramp: Some(RAMP_HINGE),
};
/// Where the renderer's lamps shine from (`models::capital_lamps`); `lamps` builds a
/// fitting at each.
pub const LAMPS: crate::models::CapitalLamps = crate::models::CapitalLamps {
    floods: &[[118.0, 12.0, 35.2], [118.0, -12.0, 35.2], [-122.0, 40.0, 35.4], [-122.0, -40.0, 35.4]],
    nav_port: [-64.0, 61.8, 58.0],
    nav_starboard: [-64.0, -61.8, 58.0],
    strobes: &[[160.0, 0.0, 45.6], [-133.8, 69.0, 60.0], [-133.8, -69.0, 60.0]],
    beacons: &[[-12.0, 21.5, 27.5], [-12.0, -21.5, 27.5]],
    hold: Some(([4.0, 0.0, 66.5], RAMP_LIP)),
};

/// Small fittings for the lamps: flood housings and beacons under the belly, navigation
/// lights on the flanks, strobes on the chin beak and the stern, a lamp in the hold roof.
fn lamps(b: &mut MeshBuilder) {
    if !b.mid() {
        return;
    }
    let l = LAMPS;
    for p in l.floods.iter().chain(l.beacons) {
        let p = Vec3::from(*p);
        b.paint(ACCENT);
        b.cuboid(p + Vec3::Z * 1.0, v3(3.2, 3.2, 1.6));
    }
    for p in l.floods {
        b.paint(GLOW);
        b.cuboid(Vec3::from(*p) + Vec3::Z * 0.1, v3(2.4, 2.4, 0.2));
    }
    b.paint(GLOW_AMBER);
    for p in l.beacons {
        b.cuboid(Vec3::from(*p), v3(1.4, 1.4, 0.6));
    }
    for (p, glow) in [(l.nav_port, GLOW_RED), (l.nav_starboard, GLOW)] {
        let p = Vec3::from(p);
        b.paint(ACCENT);
        b.cuboid(p - Vec3::Y * p.y.signum() * 0.6, v3(1.8, 0.6, 1.8));
        b.paint(glow);
        b.cuboid(p, v3(1.0, 0.6, 1.0));
    }
    b.paint(GLOW);
    for p in l.strobes {
        b.cuboid(Vec3::from(*p), v3(0.9, 0.9, 0.9));
    }
    if let Some((p, _)) = l.hold {
        b.cuboid(Vec3::from(p) + Vec3::Z * 1.3, v3(6.0, 2.0, 0.3));
    }
}

/// The bay doors' outer face, a little under the keel (z 30).
const BAY_SILL: f32 = 29.3;
const HOLD_HALF: f32 = 20.0;
const FLOOR: f32 = 34.0;
const CEILING: f32 = 68.0;

// Cross section of one armored shoulder. It encloses the hold without filling it.
fn shoulder(x: f32, width: f32, keel: f32, crown: f32) -> Vec<Vec3> {
    vec![
        v3(x, 20.0, keel),
        v3(x, width - 10.0, keel),
        v3(x, width, keel + 12.0),
        v3(x, width, crown - 13.0),
        v3(x, width - 12.0, crown),
        v3(x, 20.0, crown),
    ]
}

// The narrow forward hull opens into the broad aft engine section in every LOD.
// x, outside half-width, keel, crown; the vehicle hold stays 40 m clear inside.
const SHOULDERS: [[f32; 4]; 8] = [
    [-100.0, 59.0, 30.0, 73.0],
    [-96.0, 59.0, 30.0, 73.0],
    [-64.0, 61.0, 30.0, 77.0],
    [-22.0, 42.0, 30.0, 66.0],
    [8.0, 40.0, 30.0, 66.0],
    [38.0, 42.0, 30.0, 81.0],
    [65.0, 44.0, 30.0, 80.0],
    [98.0, 36.0, 34.0, 67.0],
];
fn shoulder_surface(x: f32) -> (f32, f32) {
    for p in SHOULDERS.windows(2) {
        if x <= p[1][0] {
            let t = ((x - p[0][0]) / (p[1][0] - p[0][0])).clamp(0.0, 1.0);
            return (
                p[0][1] + (p[1][1] - p[0][1]) * t,
                p[0][3] + (p[1][3] - p[0][3]) * t,
            );
        }
    }
    (36.0, 67.0)
}

/// Upper and lower cheeks enclose a dark recessed prow instead of one flat cap.
fn prow(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &band(
            &[
                [66.0, 26.0, 34.0, 41.0, 47.0, 34.0, 64.0, 23.0, 69.0],
                [106.0, 21.0, 34.0, 31.0, 43.0, 26.0, 59.0, 18.0, 62.0],
                [153.0, 9.0, 43.0, 16.0, 47.0, 16.0, 53.0, 10.0, 55.0],
            ],
            0,
            3,
        ),
        true,
        true,
    );
    b.mirror_y(|b| {
        // Thick armor jaws rise above the central sensor trench. The inner wall
        // is deliberately visible; a solid dark deck lies beneath the channel.
        let cheek = |x: f32, inside: f32, outside: f32, base: f32, top: f32| {
            vec![
                v3(x, inside, base),
                v3(x, outside - 5.0, base - 2.0),
                v3(x, outside, base + 5.0),
                v3(x, outside - 7.0, top),
                v3(x, inside + 3.0, top),
            ]
        };
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                cheek(69.0, 8.0, 41.0, 62.0, 79.0),
                cheek(110.0, 7.0, 29.0, 58.0, 72.0),
                cheek(150.0, 6.0, 18.0, 52.0, 60.0),
            ],
            true,
            true,
        );
        // Lower chin outriggers leave a deep black separation under the brow.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                vec![
                    v3(83.0, 22.0, 35.0),
                    v3(83.0, 35.0, 40.0),
                    v3(83.0, 34.0, 46.0),
                    v3(83.0, 22.0, 43.0),
                ],
                vec![
                    v3(143.0, 7.0, 41.0),
                    v3(143.0, 20.0, 44.0),
                    v3(143.0, 18.0, 48.0),
                    v3(143.0, 7.0, 47.0),
                ],
            ],
            true,
            true,
        );
        if b.fine() {
            // Embedded cheek inlays follow the actual slope, with short ribs and
            // a sunk front-facing rangefinder behind a protective brow.
            b.paint(ACCENT);
            b.beam(
                v3(77.0, 24.0, 77.4),
                v3(136.0, 12.5, 64.3),
                v2(6.0, 1.4),
                v2(2.5, 1.2),
            );
            b.paint(PLATING_DARK);
            for (x, y, z) in [(83.0, 23.0, 76.4), (96.0, 20.0, 74.0), (109.0, 17.5, 71.9)] {
                b.beam(
                    v3(x - 2.0, y - 3.5, z),
                    v3(x + 1.0, y + 3.5, z),
                    v2(1.4, 1.6),
                    v2(1.4, 1.6),
                );
            }
            b.paint(TEAM);
            b.beam(
                v3(112.0, 19.0, 69.5),
                v3(135.0, 12.5, 63.5),
                v2(3.0, 0.6),
                v2(2.0, 0.6),
            );
        }
    });
    // A sensor keel in the channel gives the front a recognisable focal point.
    b.paint(ACCENT);
    b.loft(
        &band(
            &[[100.0, 7.0, 60.0, 5.0, 69.0], [142.0, 6.0, 53.0, 4.0, 60.0]],
            0,
            1,
        ),
        true,
        true,
    );
    b.paint(GLASS);
    b.cuboid(v3(142.2, 0.0, 56.8), v3(0.6, 7.0, 3.2));
    b.paint(GLOW);
    b.cuboid(v3(142.6, 0.0, 56.8), v3(0.25, 2.0, 0.8));
    if b.fine() {
        b.paint(METAL);
        for x in [85.0, 92.0, 99.0, 106.0, 113.0] {
            let z = 68.0 - (x - 85.0) * 0.19;
            b.beam(v3(x, -6.5, z), v3(x, 6.5, z), v2(1.1, 1.5), v2(1.1, 1.5));
        }
    }
}

/// The command tower on the waist island: a pale citadel, a dark raked band of bridge
/// windows under an overhanging visor, bridge wings, a dark sensor house, and a mast
/// with a turning radar bar (`part::SPINNER`).
fn bridge(b: &mut MeshBuilder) {
    let deck = BRIDGE_DECK;
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum(v3(-35.0, 0.0, deck), v2(28.0, 21.0), v2(25.0, 18.5), 9.0, v2(-1.0, 0.0));
    // A dark strake round the citadel's head, then the window band leaning out.
    b.paint(ACCENT);
    b.frustum(v3(-36.0, 0.0, deck + 9.0), v2(25.6, 19.1), v2(25.6, 19.1), 1.0, v2(0.0, 0.0));
    // Dark glass, raked back a little so the camera sees it, under a thin visor.
    b.paint(GLASS);
    b.frustum(v3(-36.0, 0.0, deck + 10.0), v2(25.2, 18.8), v2(23.0, 16.6), 3.6, v2(-0.6, 0.0));
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum(v3(-36.6, 0.0, deck + 13.6), v2(24.4, 17.8), v2(22.0, 15.8), 1.4, v2(-0.8, 0.0));
    // Sensor house, set back, with its own slit of glass.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(v3(-40.5, 0.0, deck + 15.0), v2(15.0, 12.0), v2(12.0, 10.0), 4.6, v2(-1.0, 0.0));
    b.paint(GLASS);
    b.frustum(v3(-33.6, 0.0, deck + 16.6), v2(1.4, 9.0), v2(0.6, 8.6), 1.4, v2(-0.8, 0.0));
    b.paint(TEAM);
    b.frustum(v3(-35.0, 0.0, deck + 1.4), v2(28.0, 21.2), v2(27.7, 20.9), 1.2, v2(-0.1, 0.0));
    b.mirror_y(|b| {
        // Bridge wings: glazed lookouts out past the deck edge.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum(v3(-28.5, 12.0, deck + 9.2), v2(8.0, 6.0), v2(8.0, 6.0), 4.0, v2(0.0, 0.6));
        b.paint(GLASS);
        b.cuboid(v3(-24.4, 12.4, deck + 11.4), v3(0.5, 5.0, 1.8));
    });
    // The mast, its yard and the radar bar that turns on top.
    b.paint(METAL);
    let top = deck + 19.6;
    b.beam(v3(-42.5, 0.0, top), v3(-43.0, 0.0, top + 18.0), v2(1.8, 1.8), v2(0.9, 0.9));
    if b.mid() {
        b.beam(v3(-42.8, -7.0, top + 9.0), v3(-42.8, 7.0, top + 9.0), v2(0.7, 0.7), v2(0.7, 0.7));
        b.set_spinner_pivot(v3(-43.0, 0.0, top + 18.0));
        b.with_part(part::SPINNER, |b| {
            b.paint(ACCENT);
            b.cuboid(v3(-43.0, 0.0, top + 18.9), v3(2.0, 14.0, 1.8));
            b.paint(METAL);
            b.cuboid(v3(-43.0, 0.0, top + 18.2), v3(1.6, 1.6, 0.8));
        });
    }
    if b.fine() {
        b.paint(METAL);
        b.mirror_y(|b| {
            // Struts steady the post; whips stand at the visor's corners.
            b.beam(v3(-38.5, 4.2, top), v3(-42.7, 0.7, top + 8.0), v2(0.6, 0.6), v2(0.4, 0.4));
            b.beam(v3(-46.0, 7.2, deck + 14.8), v3(-46.3, 7.2, deck + 24.0), v2(0.35, 0.35), v2(0.18, 0.18));
            b.beam(v3(-28.5, 7.0, deck + 14.8), v3(-28.7, 7.0, deck + 19.0), v2(0.3, 0.3), v2(0.15, 0.15));
            b.paint(GLOW_AMBER);
            b.cuboid(v3(-22.6, 8.4, deck + 9.6), v3(0.6, 1.6, 0.4));
            b.paint(METAL);
        });
        b.paint(PLATING);
        b.spheroid(v3(-45.5, 0.0, top + 1.8), v3(2.6, 2.6, 2.1), 8, 4);
        b.paint(GLOW_RED);
        b.cuboid(v3(-43.0, 0.0, top + 20.4), v3(0.8, 0.8, 0.8));
        b.cuboid(v3(-42.8, 7.4, top + 9.2), v3(0.6, 0.6, 0.6));
        b.cuboid(v3(-42.8, -7.4, top + 9.2), v3(0.6, 0.6, 0.6));
    }
}

/// A dark gallery belt round an island's sloping sides, `at` of the way up.
fn gallery(b: &mut MeshBuilder, island: (f32, f32, f32, f32, f32, f32), at: f32) {
    let (x, length, width, top, height, shift) = island;
    let size = |f: f32| v2(length + (length - 14.0 - length) * f + 0.8, width + (top - width) * f + 0.8);
    let (f0, f1) = (at, at + 1.6 / height);
    b.paint(ACCENT);
    b.frustum_open(
        v3(x + shift * f0, 0.0, ISLAND_BASE + height * f0),
        size(f0),
        size(f1),
        height * (f1 - f0),
        v2(shift * (f1 - f0), 0.0),
    );
}

/// A point-defence blister: a low dome with a short twin gun, facing `yaw`.
fn blister(b: &mut MeshBuilder, at: Vec3, yaw: f32) {
    b.paint(PLATING_DARK);
    let n = b.sides(10);
    b.prism(at - Vec3::Z * 0.4, n, 3.0, 2.6, 1.2);
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.spheroid(at + Vec3::Z * 0.8, v3(2.3, 2.3, 1.5), b.sides(8), 3);
    if b.fine() {
        b.yawed(at, yaw, |b| {
            b.paint(METAL);
            for y in [-0.45, 0.45] {
                b.cylinder_between(at + v3(1.2, y, 1.3), at + v3(5.2, y, 1.6), 0.22, 0.18, 5);
            }
        });
    }
}

/// The dorsal islands: x, base length, base width, top width, height, top shift.
const ISLANDS: [(f32, f32, f32, f32, f32, f32); 3] = [
    (-87.0, 51.0, 38.0, 30.0, 12.0, -4.0),
    (-31.0, 45.0, 33.0, 27.0, 8.0, -3.0),
    (40.0, 69.0, 34.0, 22.0, 16.0, -9.0),
];
const ISLAND_BASE: f32 = 71.5;
/// The bridge island's roof, where the command tower stands.
const BRIDGE_DECK: f32 = ISLAND_BASE + 8.0;

fn dorsal(b: &mut MeshBuilder) {
    // Three distinct islands with exposed machinery between them. The aft
    // reactor drops towards the engines; the taller bridge sits at the waist.
    for island in ISLANDS {
        let (x, length, width, top, height, shift) = island;
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.frustum_open(
            v3(x, 0.0, ISLAND_BASE),
            v2(length, width),
            v2(length - 14.0, top),
            height,
            v2(shift, 0.0),
        );
        gallery(b, island, 0.42);
    }
    bridge(b);
    // Bridges over the mechanical wells root the islands into a common backbone.
    b.paint(METAL);
    for x in [-60.0, 0.0] {
        b.cuboid(v3(x, 0.0, 73.0), v3(11.0, 18.0, 3.0));
        if b.fine() {
            vent(b, v3(x, 0.0, 74.55), v2(9.0, 15.0), 5, ACCENT);
        }
    }
    // Reactor island: a radiator bank of tall fins over a hot tray.
    let top = 83.5;
    b.paint(ACCENT);
    b.plate(v3(-91.0, 0.0, top), v2(31.0, 25.0), 0.8, 0.3);
    if b.mid() {
        for k in 0..8 {
            let x = -102.0 + k as f32 * 3.1;
            b.paint(PLATING_DARK);
            b.beam(v3(x, -11.0, top + 2.4), v3(x, 11.0, top + 2.4), v2(0.9, 3.4), v2(0.9, 3.4));
            if b.fine() && k < 7 {
                b.paint(GLOW_ORANGE);
                b.beam(v3(x + 1.55, -10.0, top + 0.9), v3(x + 1.55, 10.0, top + 0.9), v2(0.5, 0.2), v2(0.5, 0.2));
            }
        }
        b.paint(METAL);
        b.mirror_y(|b| {
            b.beam(v3(-104.0, 10.5, top + 4.4), v3(-79.0, 10.5, top + 4.4), v2(0.8, 0.6), v2(0.8, 0.6));
        });
    }
    // Forward island: plated decks either side of a service trench with a pipe run,
    // hatches, and two point-defence blisters.
    let top = 87.5;
    b.paint(ACCENT);
    b.plate(v3(31.0, 0.0, top), v2(54.0, 7.0), 0.15, 0.05);
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.plate(v3(31.0, 7.2, top), v2(52.0, 7.4), 1.1, 0.4);
        if b.fine() {
            b.paint(METAL).pattern(pattern::PLAIN);
            for x in [18.0, 44.0] {
                b.plate(v3(x, 7.2, top + 1.1), v2(4.2, 4.2), 0.35, 0.12);
            }
            b.paint(GLOW);
            b.cuboid(v3(57.2, 3.2, top + 0.4), v3(0.6, 0.6, 0.5));
            b.cuboid(v3(4.8, 3.2, top + 0.4), v3(0.6, 0.6, 0.5));
        }
    });
    if b.fine() {
        b.paint(METAL);
        for y in [-1.4, 1.4] {
            b.cylinder_between(v3(5.0, y, top + 0.85), v3(57.0, y, top + 0.85), 0.7, 0.7, 6);
        }
        b.paint(ACCENT);
        for x in [12.0, 31.0, 50.0] {
            b.cuboid(v3(x, 0.0, top + 1.0), v3(1.4, 5.4, 1.5));
        }
    }
    blister(b, v3(49.0, -7.2, top + 1.6), 0.4);
    blister(b, v3(11.0, 7.2, top + 1.6), 2.6);
    if b.fine() {
        b.mirror_y(|b| {
            // Short rear-facing cooling fins give the reactor a stepped profile.
            for x in [-103.0, -95.0, -87.0, -79.0] {
                b.paint(PLATING_DARK);
                b.beam(
                    v3(x, 12.0, 82.0),
                    v3(x - 3.0, 18.0, 87.0),
                    v2(2.0, 3.0),
                    v2(1.2, 2.0),
                );
            }
        });
    }
}

/// The broad aft engine section: two armoured housings carrying the four drives in a
/// thrust frame, the stern boom with its gun between them, heat shields and gantries.
fn stern(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        let section = |x: f32, keel: f32, out: f32, crown: f32| {
            vec![
                v3(x, 17.0, keel),
                v3(x, out - 12.0, keel),
                v3(x, out, keel + 9.0),
                v3(x, out, crown - 11.0),
                v3(x, out - 9.0, crown),
                v3(x, 17.0, crown),
            ]
        };
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                section(-99.0, 29.8, 62.0, 75.0),
                section(-112.0, 36.0, 75.0, 76.0),
                section(-127.0, 36.0, 75.0, 76.0),
                section(-133.0, 38.0, 72.0, 73.0),
            ],
            true,
            true,
        );
        // Armour belt down the housing's flank, standing proud with a lit seam.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                vec![v3(-113.0, 72.0, 44.0), v3(-113.0, 76.4, 46.0), v3(-113.0, 76.4, 63.0), v3(-113.0, 72.0, 66.0)],
                vec![v3(-127.0, 72.0, 44.0), v3(-127.0, 76.4, 46.0), v3(-127.0, 76.4, 63.0), v3(-127.0, 72.0, 66.0)],
            ],
            true,
            true,
        );
        if b.fine() {
            b.paint(GLOW_AMBER);
            b.beam(v3(-114.0, 76.5, 54.5), v3(-126.0, 76.5, 54.5), v2(0.3, 0.4), v2(0.3, 0.4));
            b.paint(ACCENT);
            for x in [-116.5, -120.0, -123.5] {
                b.beam(v3(x, 76.5, 47.0), v3(x, 76.5, 62.0), v2(0.8, 1.2), v2(0.8, 1.2));
            }
        }
        // Thrust frame: a square collar round each can, braced to it.
        for y in [31.0, 57.0] {
            b.paint(ACCENT);
            b.loft(
                &[
                    vec![v3(-132.0, y - 12.5, 41.5), v3(-132.0, y + 12.5, 41.5), v3(-132.0, y + 12.5, 66.5), v3(-132.0, y - 12.5, 66.5)],
                    vec![v3(-136.0, y - 11.5, 42.5), v3(-136.0, y + 11.5, 42.5), v3(-136.0, y + 11.5, 65.5), v3(-136.0, y - 11.5, 65.5)],
                ],
                false,
                true,
            );
        }
        // Heat shield between the inner and outer drives.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                vec![v3(-135.0, 43.0, 40.0), v3(-135.0, 45.0, 40.0), v3(-135.0, 45.0, 68.0), v3(-135.0, 43.0, 68.0)],
                vec![v3(-150.0, 43.4, 45.0), v3(-150.0, 44.6, 45.0), v3(-150.0, 44.6, 63.0), v3(-150.0, 43.4, 63.0)],
            ],
            true,
            true,
        );
        // Lift jets under the housing and the chin.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum(v3(92.0, 30.0, 35.8), v2(16.0, 15.0), v2(20.0, 12.0), 6.0, v2(-1.0, 1.5));
        if b.fine() {
            // Radiator bank on the housing's roof.
            b.paint(ACCENT);
            b.plate(v3(-114.0, 53.0, 76.0), v2(20.0, 24.0), 0.5, 0.2);
            b.paint(PLATING_DARK);
            for k in 0..6 {
                let y = 44.0 + k as f32 * 3.6;
                b.beam(v3(-123.0, y, 77.8), v3(-105.0, y, 77.8), v2(0.8, 3.0), v2(0.8, 3.0));
            }
            // Stern lights: red at the outboard corners, amber along the frame head.
            b.paint(GLOW_RED);
            b.cuboid(v3(-133.3, 64.0, 69.5), v3(0.8, 1.2, 1.2));
            b.paint(GLOW_AMBER);
            b.cuboid(v3(-133.3, 45.0, 71.8), v3(0.3, 40.0, 0.4));
        }
    });
    blister(b, v3(-116.0, 30.0, 76.4), 1.4);
    blister(b, v3(-116.0, -30.0, 76.4), -1.4);
    // The centre block closes the hold astern; the boom carries the stern gun.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.block(v3(-141.0, -21.0, 34.0), v3(-103.0, 21.0, 72.0));
    b.loft(
        &band(
            &[
                [-140.5, 15.0, 40.0, 16.0, 56.0, 12.0, 64.0],
                [-160.0, 12.0, 44.0, 13.0, 55.0, 10.0, 62.5],
                [-168.0, 8.0, 47.0, 9.0, 54.0, 7.0, 60.0],
            ],
            0,
            2,
        ),
        true,
        true,
    );
    b.paint(PLATING);
    let n = b.sides(12);
    b.prism(v3(-156.0, 0.0, 58.0), n, 8.0, 7.2, 5.4);
    if b.fine() {
        b.paint(GLOW_RED);
        b.cuboid(v3(-168.3, 0.0, 57.0), v3(0.6, 2.0, 1.0));
        b.paint(ACCENT);
        for z in [48.0, 52.0] {
            b.cuboid(v3(-165.0, 0.0, z), v3(6.0, 20.5, 0.8));
        }
    }
    for port in LIFT_JETS {
        capital::lift_jet(b, Vec3::from(port), 1.0);
    }
    for port in NOZZLES {
        capital::drive(b, Vec3::from(port), 1.0);
    }
}

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        // Preserve the slim forward hull and broad engine forks at strategy zoom.
        let mut plan = vec![
            [153.0, 0.0],
            [140.0, 20.0],
            [65.0, 44.0],
            [-64.0, 61.0],
            [-112.0, 75.0],
            [-166.0, 69.0],
        ];
        let opposite = plan[1..].iter().rev().map(|p| [p[0], -p[1]]).collect::<Vec<_>>();
        plan.extend(opposite);
        b.paint(PLATING_DARK);
        b.extrude_z(&plan, 35.0, 70.0);
        b.paint(PLATING);
        b.frustum_open(
            v3(-29.0, 0.0, 70.0),
            v2(64.0, 29.0),
            v2(32.0, 19.0),
            24.0,
            v2(-8.0, 0.0),
        );
        return;
    }
    b.mirror_y(|b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        let rings = SHOULDERS
            .iter()
            .map(|p| shoulder(p[0], p[1], p[2], p[3]))
            .collect::<Vec<_>>();
        b.loft(&rings, true, true);
        // Armour brows along the slim forward hull; aft of the waist the side carries
        // the sponsons and a service shelf with its lit rail instead.
        for (x0, x1, brow) in [
            (-94.0, -56.0, false),
            (-24.0, 5.0, false),
            (12.0, 35.0, true),
            (41.0, 62.0, true),
            (69.0, 94.0, true),
        ] {
            let (w0, h0) = shoulder_surface(x0);
            let (w1, h1) = shoulder_surface(x1);
            if brow {
                let section = |x: f32| {
                    let (w, h) = shoulder_surface(x);
                    vec![
                        v3(x, w - 17.0, h - 1.0),
                        v3(x, w - 10.5, h + 1.8),
                        v3(x, w + 2.0, h - 12.0),
                        v3(x, w + 1.2, h - 19.0),
                        v3(x, w - 2.0, h - 20.0),
                    ]
                };
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.loft(&[section(x0), section(x1)], true, true);
                // A dark run of hatches let into the brow's sloping top.
                let on_top = |x: f32, u: f32, lift: f32| {
                    let (w, h) = shoulder_surface(x);
                    let (a, c) = (v2(w - 17.0, h - 1.0), v2(w - 10.5, h + 1.8));
                    let n = v2(-(c.y - a.y), c.x - a.x).normalize();
                    let q = a + (c - a) * u + n * lift;
                    v3(x, q.x, q.y)
                };
                let strip = |x: f32, lift: f32| {
                    vec![on_top(x, 0.2, -0.3), on_top(x, 0.8, -0.3), on_top(x, 0.8, lift), on_top(x, 0.2, lift)]
                };
                b.paint(ACCENT);
                b.loft(&[strip(x0 + 2.5, 0.25), strip(x1 - 2.5, 0.25)], true, true);
                if b.fine() {
                    b.paint(METAL).pattern(pattern::PLAIN);
                    let cells = ((x1 - x0 - 5.0) / 5.5).floor().max(1.0) as usize;
                    for k in 0..cells {
                        let xa = x0 + 2.5 + (x1 - x0 - 5.0) * (k as f32 + 0.15) / cells as f32;
                        let xb = x0 + 2.5 + (x1 - x0 - 5.0) * (k as f32 + 0.85) / cells as f32;
                        let lid = |x: f32| {
                            vec![on_top(x, 0.28, 0.2), on_top(x, 0.72, 0.2), on_top(x, 0.72, 0.5), on_top(x, 0.28, 0.5)]
                        };
                        b.loft(&[lid(xa), lid(xb)], true, true);
                    }
                }
            }
            // A true deep service shelf along the flank.
            b.paint(ACCENT);
            b.beam(
                v3(x0, w0 + 0.1, h0 - 22.0),
                v3(x1, w1 + 0.1, h1 - 22.0),
                v2(3.0, 7.0),
                v2(3.0, 7.0),
            );
            if b.fine() {
                b.paint(PLATING_DARK);
                for t in [0.15, 0.38, 0.62, 0.85] {
                    let x = x0 + (x1 - x0) * t;
                    let (w, h) = shoulder_surface(x);
                    b.beam(
                        v3(x, w + 1.0, h - 24.5),
                        v3(x, w + 1.0, h - 19.0),
                        v2(1.5, 1.7),
                        v2(1.5, 1.7),
                    );
                }
                b.paint(GLOW_AMBER);
                b.beam(
                    v3(x0 + 2.0, w0 + 1.0, h0 - 19.0),
                    v3(x1 - 2.0, w1 + 1.0, h1 - 19.0),
                    v2(0.5, 0.55),
                    v2(0.5, 0.55),
                );
            }
        }
        // Deep radiator terraces occupy the wide shoulders, with a visible lower
        // gallery between them and the central superstructure.
        for (x, y, z, l, w) in [
            (-80.0, 35.0, 75.0, 25.0, 24.0),
            (51.0, 24.5, 80.5, 23.0, 14.0),
        ] {
            b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
            b.frustum_open(
                v3(x, y, z - 2.8),
                v2(l + 5.0, w),
                v2(l, w - 6.0),
                5.0,
                v2(-2.0, 0.0),
            );
            b.paint(PLATING);
            b.plate(v3(x - 2.0, y, z + 2.0), v2(l - 2.0, w - 8.0), 1.5, 0.6);
            if b.fine() {
                vent(b, v3(x - 2.0, y, z + 3.55), v2(l - 5.0, w - 10.0), 6, METAL);
            }
            b.paint(TEAM);
            b.cuboid(
                v3(x - 2.0, y + (w - 8.0) * 0.5, z + 3.6),
                v3(l - 5.0, 1.0, 0.35),
            );
        }
        // The waist exposes connected hydraulic/service runs in an armored recess.
        b.paint(ACCENT);
        b.cuboid(v3(-7.0, 29.0, 65.0), v3(29.0, 12.0, 3.0));
        if b.fine() {
            for y in [25.0, 29.0, 33.0] {
                b.paint(METAL);
                b.cylinder_between(v3(-20.0, y, 67.0), v3(6.0, y, 67.0), 0.9, 0.9, 6);
            }
            b.paint(PLATING);
            for x in [-17.0, 3.0] {
                b.cuboid(v3(x, 29.0, 68.0), v3(3.0, 12.0, 2.0));
            }
        }
        // Belly gravity rails between the gear bays, and the unchanged transport hold.
        b.paint(ACCENT);
        b.cuboid(v3(-5.0, 28.0, 30.0), v3(70.0, 10.0, 4.0));
        b.paint(GLOW);
        for x in [-28.0, -5.0, 18.0] {
            b.cuboid(v3(x, 28.0, 27.8), v3(19.0, 5.0, 0.4));
        }
        b.paint(PLATING_DARK).pattern(pattern::WALKWAY);
        b.block(v3(-16.0, 0.0, 32.0), v3(66.0, 20.0, FLOOR));
        b.paint(ACCENT);
        b.block(v3(-103.0, 19.5, 34.0), v3(66.0, 20.5, CEILING));
        b.paint(GLOW_AMBER);
        b.block(v3(-93.0, 19.1, 65.0), v3(61.0, 19.6, 65.6));
    });
    prow(b);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.block(v3(-108.0, -20.0, CEILING), v3(68.0, 20.0, 72.0));
    dorsal(b);
    stern(b);
    capital::gear(b, &RIG, BAY_SILL);
    lamps(b);
    // Belly ramp: folds flush with the keel, and opens under the aft half of the hull.
    b.with_part(part::RAMP, |b| {
        let [hx, hz] = RAMP_HINGE;
        b.paint(PLATING_DARK).pattern(pattern::WALKWAY);
        b.extrude_y(
            &[
                [hx, hz],
                [RAMP_LIP, 0.0],
                [RAMP_LIP + 1.0, 0.0],
                [hx, hz - 1.5],
            ],
            -HOLD_HALF,
            HOLD_HALF,
        );
        b.mirror_y(|b| {
            b.paint(METAL);
            b.beam(
                v3(hx, 19.0, hz),
                v3(RAMP_LIP + 1.0, 19.0, 0.5),
                v2(1.0, 1.0),
                v2(0.8, 0.8),
            );
            b.paint(GLOW_AMBER);
            b.beam(
                v3(hx - 1.0, 18.4, hz),
                v3(RAMP_LIP + 2.0, 18.4, 1.0),
                v2(0.2, 0.2),
                v2(0.2, 0.2),
            );
        });
    });
    // The four guns, in `TURRETS` (weapon) order, each on a mount of its own.
    chin_mount(b);
    b.mirror_y(|b| sponson(b));
    for (w, (at, _)) in TURRETS.into_iter().enumerate() {
        capital::rotary_house(b, w, Vec3::from(at), w == 0, 1.0);
    }
}

/// The chin beak run out under the prow's tip to carry the nose gun clear of the hull.
fn chin_mount(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &band(
            &[
                [118.0, 5.0, 35.2, 11.0, 38.5],
                [148.0, 5.0, 42.0, 8.0, 44.5],
                [166.0, 3.0, 43.0, 3.5, 45.0],
            ],
            0,
            1,
        ),
        true,
        true,
    );
    if b.fine() {
        b.paint(GLOW_RED);
        b.cuboid(v3(166.2, 0.0, 44.0), v3(0.5, 1.2, 0.8));
    }
}

/// A side sponson growing out of the flank under the port gun (the mirror makes starboard).
/// Its deck sits well under the gun, so the barrels depress to the pitch limit clear of it.
fn sponson(b: &mut MeshBuilder) {
    let ([x, y, z], _) = TURRETS[1];
    let deck = z - 6.5;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            vec![v3(x - 12.0, 36.0, deck - 10.0), v3(x + 12.0, 36.0, deck - 10.0), v3(x + 12.0, 36.0, deck), v3(x - 12.0, 36.0, deck)],
            vec![v3(x - 6.0, y + 5.0, deck - 2.5), v3(x + 6.0, y + 5.0, deck - 2.5), v3(x + 6.0, y + 5.0, deck), v3(x - 6.0, y + 5.0, deck)],
        ],
        true,
        true,
    );
    b.paint(PLATING);
    let n = b.sides(12);
    b.prism(v3(x, y, deck - 1.0), n, 7.0, 6.6, 1.0);
    if b.mid() {
        // Gussets under the deck, and a lit rim.
        b.paint(PLATING_DARK);
        for dx in [-8.0, 8.0] {
            b.beam(v3(x + dx, 44.0, deck - 13.0), v3(x + dx * 0.6, y + 2.0, deck - 2.5), v2(1.6, 1.6), v2(1.2, 1.2));
        }
    }
    if b.fine() {
        b.paint(GLOW_AMBER);
        b.cuboid(v3(x, y + 5.1, deck - 0.6), v3(10.0, 0.3, 0.3));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::build_model_fitted;
    #[test]
    fn the_lift_ship_keeps_its_budgets_and_rig() {
        let model = build_model_fitted("lift_ship", 160.0, 95.0, 2, &[]).unwrap();
        let tris: Vec<_> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
        println!("Bastion triangles: {tris:?}");
        assert!(tris[0] <= 14000 && tris[1] <= tris[0] / 2 && tris[2] < 60);
        assert_eq!(model.houses.len(), 4);
        for (house, (pivot, _)) in model.houses.iter().zip(TURRETS) {
            assert_eq!(house.pivot, pivot);
        }
        for lod in &model.lods[..2] {
            for part in [part::RAMP, part::GEAR, part::GEAR_STRUT, part::GEAR_FOOT, part::GEAR_DOOR, part::DRIVE] {
                assert!(lod.vertices.iter().any(|v| v.part == part));
            }
            assert!(!lod.vertices.iter().any(|v| v.part == part::FAN));
        }
        let mesh = &model.lods[0];
        let span = |axis: usize| {
            mesh.vertices
                .iter()
                .map(|v| v.pos[axis])
                .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
        };
        let (lo, hi) = span(0);
        assert!(hi - lo >= 288.0, "capital hull length {}", hi - lo);
        assert!(span(2).0 >= -0.01);
        // Stowed (pads folded, strut in, leg swung up), every leg piece is inside the hull.
        for v in &mesh.vertices {
            if [part::GEAR, part::GEAR_STRUT, part::GEAR_FOOT].contains(&v.part) {
                let r = capital::stowed(&RIG, Vec3::from(v.pos), v.part, v.material);
                assert!(r.z >= 30.4, "{:?} stows to {r}", v.part);
            }
        }
        // The rig's numbers are the ones the model was built on.
        assert_eq!(RIG.drives.unwrap().0, [NOZZLES[0][0], NOZZLES[0][2], NOZZLES[2][1], NOZZLES[3][1]]);
        let jets = RIG.lift_jets.unwrap();
        assert_eq!(jets.0, [LIFT_JETS[2][0], LIFT_JETS[3][1], LIFT_JETS[0][0], LIFT_JETS[1][1]]);
        assert_eq!(jets.1, LIFT_JETS[0][2]);
        assert_eq!(crate::models::capital_rig("lift_ship"), Some(RIG.gpu()));
    }
}
