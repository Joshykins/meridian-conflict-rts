//! Thresher: the tech 2 land reclaimer, a wheeled carrier with three reclaim heads
//! (weapons 0, 1 and 2) placed so that between them they see every way, and red
//! missile-defence laser heads (`anti_missile_mounts`).
//!
//! Authored at blueprint scale (radius 6.2, height 4.4): model metres are unit metres.

use glam::Vec3;

use super::super::naval::pd_laser;
use super::*;

/// Variant A, "Spine": three heads down the centreline, the middle one raised
/// on a pedestal to see over the other two.
pub(in crate::aster) const SPINE_PIVOTS: [Vec3; 3] = [
    Vec3::new(3.3, 0.0, 3.95),
    Vec3::new(-0.3, 0.0, 5.25),
    Vec3::new(-3.8, 0.0, 3.95),
];
pub(in crate::aster) const SPINE_LASERS: [Vec3; 2] =
    [Vec3::new(-5.3, 1.75, 4.1), Vec3::new(-5.3, -1.75, 4.1)];

/// Variant B, "Flanks": a high central head on a mast, and two ball heads in
/// sponsons over the wheels, each leaning out over its own side.
pub(in crate::aster) const FLANK_PIVOTS: [Vec3; 3] = [
    Vec3::new(-1.2, 0.0, 6.0),
    Vec3::new(1.6, 2.55, 3.45),
    Vec3::new(1.6, -2.55, 3.45),
];
pub(in crate::aster) const FLANK_LASERS: [Vec3; 2] =
    [Vec3::new(4.4, 0.0, 4.0), Vec3::new(-5.3, 0.0, 4.0)];

/// Variant C, "Harvester": a six-wheeler with two lance heads on the front
/// shoulders and a ball head on a tower over the tail.
pub(in crate::aster) const HARVESTER_PIVOTS: [Vec3; 3] = [
    Vec3::new(-3.4, 0.0, 6.1),
    Vec3::new(2.8, 1.45, 3.85),
    Vec3::new(2.8, -1.45, 3.85),
];
pub(in crate::aster) const HARVESTER_LASERS: [Vec3; 2] =
    [Vec3::new(0.2, 1.95, 4.2), Vec3::new(0.2, -1.95, 4.2)];

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
        b.cuboid(v3(front - 0.35, 1.7, 1.75), v3(0.32, 0.34, 0.28));
        b.paint(GLASS);
        b.cuboid(v3(front - 0.18, 1.7, 1.75), v3(0.04, 0.26, 0.2));
        b.paint(METAL);
        b.block(v3(front - 0.75, 1.0, 1.55), v3(front - 0.4, 1.2, 1.8));
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

/// A: three heads on the spine, the middle one on a pedestal; lasers at the tail corners.
pub(in crate::aster) fn thresher_a(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        let [a, b1, c] = SPINE_PIVOTS;
        coarse(b, GEAR, 2.75, &[(a, 2.75), (b1, 2.75), (c, 2.75)]);
        return;
    }
    let deck = eight_wheels(b);
    let [front, mid, rear] = SPINE_PIVOTS;
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
    reclaim_head(b, 0, front, deck.z, 0.95, Head::Cradle);
    reclaim_head(b, 1, mid, mid.z - 0.62, 1.0, Head::Cradle);
    reclaim_head(b, 2, rear, deck.z, 0.95, Head::Cradle);
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
    for at in SPINE_LASERS {
        pd_laser(b, at, 0.42, Some(deck.z));
    }
    nose(b, &deck, 6.3);
    engine(b, &deck);
}

/// B: a mast head amidships and two ball heads in sponsons over the wheels.
pub(in crate::aster) fn thresher_b(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        let [a, b1, c] = FLANK_PIVOTS;
        coarse(b, GEAR, 2.75, &[(a, 2.75), (b1, 2.75), (c, 2.75)]);
        return;
    }
    let deck = eight_wheels(b);
    let [top, left, right] = FLANK_PIVOTS;
    // Sponsons: dark tubs hung off the flanks between the wheel pairs.
    for p in [left, right] {
        let side = p.y.signum();
        let inner = side * (deck.half_width - 0.3);
        let (y0, y1) = if side > 0.0 {
            (inner, p.y + 0.7)
        } else {
            (p.y - 0.7, inner)
        };
        b.paint(PLATING_DARK);
        b.block(v3(p.x - 1.0, y0, 2.2), v3(p.x + 1.0, y1, deck.z - 0.05));
        if b.fine() {
            b.paint(ACCENT);
            b.block(v3(p.x - 0.9, y0, 1.95), v3(p.x + 0.9, y1 - 0.1 * side, 2.2));
        }
    }
    reclaim_head(b, 1, left, deck.z - 0.05, 0.9, Head::Ball);
    reclaim_head(b, 2, right, deck.z - 0.05, 0.9, Head::Ball);
    // Mast: a drum on the deck, and the column that turns with the top head.
    b.paint(PLATING_DARK);
    b.prism(v3(top.x, 0.0, deck.z - 0.05), b.sides(8), 1.3, 1.1, 0.55);
    let base = deck.z + 0.45;
    b.with_house(0, top, 0.0, |b| {
        b.with_part(crate::part::TURRET, |b| {
            b.paint(PLATING);
            column(b, v3(top.x, 0.0, base), top.z - 0.62 - base, 0.55, 0.42);
            b.paint(ACCENT);
            b.chamfered_box(v3(top.x - 0.8, 0.0, base + 0.9), v3(0.7, 1.1, 1.3), 0.15);
            chute(
                b,
                v3(top.x - 0.55, 0.0, top.z - 0.75),
                v3(top.x - 0.7, 0.0, base + 1.5),
                0.3,
            );
        });
    });
    reclaim_head(b, 0, top, top.z - 0.62, 1.05, Head::Cradle);
    hopper(b, deck.at(0.25, 0.0), v2(2.2, 2.4), 0.55);
    team_panel(b, deck.at(0.72, 0.0), v2(1.2, 2.0));
    for at in FLANK_LASERS {
        pd_laser(b, at, 0.42, Some(deck.z));
    }
    nose(b, &deck, 6.3);
    engine(b, &deck);
}

/// C: six wheels, two lance heads on the front shoulders, a tower head aft.
pub(in crate::aster) fn thresher_c(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        let [a, b1, c] = HARVESTER_PIVOTS;
        coarse(b, GEAR, 2.75, &[(a, 2.75), (b1, 2.75), (c, 2.75)]);
        return;
    }
    let deck = wheeled_hull(
        b,
        &WheeledHull {
            rear: -6.0,
            front: 6.2,
            half_width: 2.55,
            belly: 1.05,
            deck: 2.9,
            axles: &[-4.2, -2.3, 3.2],
            wheel_r: 1.05,
            wheel_w: 0.85,
            track: 2.85,
        },
    );
    let [tower, left, right] = HARVESTER_PIVOTS;
    // Shoulders: raised white blocks the front heads sit on.
    b.paint(PLATING);
    for p in [left, right] {
        b.frustum(
            v3(p.x, p.y, deck.z - 0.05),
            v2(2.2, 1.5),
            v2(1.7, 1.2),
            p.z - 0.45 - deck.z,
            v2(-0.1, 0.0),
        );
    }
    reclaim_head(b, 1, left, left.z - 0.45, 0.85, Head::Lance);
    reclaim_head(b, 2, right, right.z - 0.45, 0.85, Head::Lance);
    // Tower over the tail: a tapering octagonal drum.
    let (foot, crown) = (deck.z - 0.05, tower.z - 0.66);
    b.paint(ACCENT);
    b.at(v3(tower.x, 0.0, 0.0), |b| {
        b.loft_z(
            &ngon(b.sides(8), 1.7),
            &[
                Section::new(foot, 1.0),
                Section::new(foot + 1.2, 0.92),
                Section::new(crown, 0.66),
            ],
        );
    });
    if b.fine() {
        b.paint(PLATING);
        b.prism(v3(tower.x, 0.0, foot + 1.1), b.sides(8), 1.62, 1.55, 0.3);
        team_panel(b, v3(tower.x - 1.55, 0.0, foot), v2(0.25, 1.0));
    }
    // A glazed chute down the tower's front face into the hopper.
    chute(
        b,
        v3(tower.x + 0.9, 0.0, crown - 0.1),
        v3(tower.x + 1.75, 0.0, deck.z + 0.5),
        0.4,
    );
    reclaim_head(b, 0, tower, crown, 1.1, Head::Ball);
    hopper(b, v3(0.3, 0.0, deck.z), v2(2.4, 2.6), 0.55);
    for at in HARVESTER_LASERS {
        pd_laser(b, at, 0.42, Some(deck.z));
    }
    nose(b, &deck, 6.2);
    engine(b, &deck);
}
