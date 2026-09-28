//! Thresher: the tech 2 land reclaimer, a wheeled carrier with three reclaim heads
//! (weapons 0, 1 and 2) placed so that between them they see every way, and red
//! missile-defence laser heads (`anti_missile_mounts`).
//!
//! Authored at blueprint scale (radius 6.2, height 4.4): model metres are unit metres.

use glam::Vec3;

use super::super::naval::pd_laser;
use super::*;

/// Three heads down the centreline, the middle one raised
/// on a pedestal to see over the other two.
pub(in crate::aster) const PIVOTS: [Vec3; 3] = [
    Vec3::new(3.3, 0.0, 3.95),
    Vec3::new(-0.3, 0.0, 5.25),
    Vec3::new(-3.8, 0.0, 3.95),
];
/// The missile-defence lasers (the unit file's `anti_missile_mounts`).
pub(in crate::aster) const LASERS: [Vec3; 2] =
    [Vec3::new(-5.3, 1.75, 4.1), Vec3::new(-5.3, -1.75, 4.1)];

/// The wheels' extent for the coarse level: rear, front, outer half width, height.
const GEAR: (f32, f32, f32, f32) = (-6.1, 6.2, 3.1, 1.9);

fn eight_wheels(b: &mut MeshBuilder) -> Roof {
    wheeled_hull(
        b,
        &WheeledHull {
            rear: -6.2,
            front: 6.3,
            half_width: 2.45,
            belly: 1.0,
            deck: 2.7,
            axles: &[-4.5, -2.8, 1.6, 3.3],
            wheel_r: 0.95,
            wheel_w: 0.75,
            track: 2.7,
        },
    )
}

/// The Thresher's glacis and cab: a sloped nose with the driver's slit, lamps.
fn nose(b: &mut MeshBuilder, deck: &Roof, front: f32) {
    if !b.fine() {
        return;
    }
    on_slope(b, [front, 2.0], [deck.front, deck.z], 0.55, |b| {
        b.paint(GLASS);
        b.plate(Vec3::ZERO, v2(0.3, 2.4), 0.05, 0.02);
    });
    b.mirror_y(|b| {
        b.paint(ACCENT);
        // Lamps on the nose's cheeks, tow hooks on its point (the plan narrows to ~0.5 m).
        b.cuboid(v3(front - 0.75, 1.0, 1.75), v3(0.32, 0.34, 0.28));
        b.paint(GLASS);
        b.cuboid(v3(front - 0.58, 1.0, 1.75), v3(0.04, 0.26, 0.2));
        b.paint(METAL);
        b.block(v3(front - 0.25, 0.22, 1.55), v3(front + 0.1, 0.42, 1.75));
    });
}

/// The engine deck aft: louvres, two stacks.
fn engine(b: &mut MeshBuilder, deck: &Roof) {
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        vent(b, deck.at(0.08, 0.55), v2(1.2, 0.7), 3, METAL);
        b.paint(METAL);
        b.cylinder_between(
            v3(deck.rear + 0.2, deck.half_width - 0.2, deck.z),
            v3(deck.rear + 0.2, deck.half_width - 0.2, deck.z + 0.8),
            0.15,
            0.12,
            6,
        );
    });
}

/// Three Cradle heads on the spine, the middle one on a pedestal; lasers at the tail corners.
pub(in crate::aster) fn thresher(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        let [a, b1, c] = PIVOTS;
        coarse(b, GEAR, 2.75, &[(a, 2.75), (b1, 2.75), (c, 2.75)]);
        return;
    }
    let deck = eight_wheels(b);
    let [front, mid, rear] = PIVOTS;
    // Pedestal: a white faceted block amidships, hoppers either side of it.
    b.paint(PLATING);
    b.frustum(
        v3(mid.x, 0.0, deck.z - 0.05),
        v2(2.6, 2.4),
        v2(1.8, 1.8),
        mid.z - 0.62 - deck.z,
        v2(-0.1, 0.0),
    );
    if b.fine() {
        b.paint(ACCENT);
        b.block(
            v3(mid.x - 1.2, -1.15, deck.z + 0.5),
            v3(mid.x + 1.2, 1.15, deck.z + 0.75),
        );
        team_panel(b, v3(mid.x - 1.35, 0.0, deck.z), v2(0.3, 1.6));
    }
    cradle_turret(b, 0, front, deck.z, 0.95);
    cradle_turret(b, 1, mid, mid.z - 0.62, 1.0);
    cradle_turret(b, 2, rear, deck.z, 0.95);
    b.mirror_y(|b| {
        hopper(b, v3(mid.x + 0.1, 1.55, deck.z), v2(2.2, 0.85), 0.45);
        // Glazed chutes down the pedestal's flanks into the hoppers.
        chute(
            b,
            v3(mid.x - 0.5, 0.75, mid.z - 0.75),
            v3(mid.x - 0.3, 1.35, deck.z + 0.5),
            0.28,
        );
    });
    for at in LASERS {
        pd_laser(b, at, 0.42, Some(deck.z));
    }
    nose(b, &deck, 6.3);
    engine(b, &deck);
}
