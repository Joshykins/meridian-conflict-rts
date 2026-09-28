//! Gleaner: the tech 1 land reclaimer, a reclaim tower on tracks or wheels. One
//! head (weapon 0) set high, so it can look down into a gully beside the hull or
//! up a slope, and turn full circle while the hull drives on.
//!
//! Authored at blueprint scale (radius 4.6, height 3.4): model metres are unit metres.

use glam::Vec3;

use super::*;

/// Variant A, "Mast": the head on a tall telescoping mast over the tracks' rear.
pub(in crate::aster) const MAST_PIVOT: Vec3 = Vec3::new(-1.3, 0.0, 5.6);
/// Variant B, "Tower": a ball head on a short octagonal tower amidships.
pub(in crate::aster) const TOWER_PIVOT: Vec3 = Vec3::new(-0.4, 0.0, 4.55);
/// Variant C, "Crane": a lance head on an A-frame at the back of a wheeled carrier.
pub(in crate::aster) const CRANE_PIVOT: Vec3 = Vec3::new(-2.1, 0.0, 5.3);

fn tracks(b: &mut MeshBuilder) -> Roof {
    tracked_chassis(
        b,
        &Chassis {
            rear: -3.7,
            front: 3.8,
            track: (1.5, 2.6, 1.2),
            split_tracks: false,
            deck: 1.85,
            dark: true,
            lit: false,
        },
    )
}

/// The rest of a tracked Gleaner's deck: a scrap hopper forward, a team plate, lamps.
fn tracked_deck(b: &mut MeshBuilder, deck: &Roof, hopper_u: f32) {
    hopper(b, deck.at(hopper_u, 0.0), v2(1.7, 1.9), 0.55);
    team_panel(b, deck.at(0.08, 0.0), v2(0.7, 1.6));
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.cuboid(v3(3.3, 1.05, 1.62), v3(0.3, 0.3, 0.26));
        b.paint(GLASS);
        b.cuboid(v3(3.46, 1.05, 1.62), v3(0.04, 0.22, 0.18));
        // Exhaust out of the tail.
        b.paint(METAL);
        b.cylinder_between(v3(-3.0, 1.0, 1.9), v3(-3.0, 1.0, 2.7), 0.12, 0.1, 6);
    });
    whip(b, deck.at(0.05, 0.75), 1.1, 0.2);
}

/// A: tracked hull, a slim two-stage mast at the rear carrying a cradle head.
pub(in crate::aster) fn gleaner_a(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracks(b);
    let p = MAST_PIVOT;
    // Mast foot: a dark drum on the deck, bolted down.
    b.paint(PLATING_DARK);
    b.prism(v3(p.x, 0.0, deck.z - 0.1), b.sides(8), 1.15, 0.95, 0.6);
    let base = deck.z + 0.5;
    b.with_house(0, p, 0.0, |b| {
        b.with_part(crate::part::TURRET, |b| {
            // Two-stage mast that turns with the head, a hydraulic ram up its back
            // and a processor pack slung behind it.
            b.paint(ACCENT);
            b.prism(v3(p.x, 0.0, base), b.sides(10), 0.85, 0.8, 0.22);
            b.paint(PLATING);
            column(b, v3(p.x, 0.0, base + 0.2), 1.8, 0.5, 0.42);
            b.paint(METAL);
            b.prism(v3(p.x, 0.0, base + 1.95), 6, 0.5, 0.5, 0.18);
            b.paint(PLATING);
            column(b, v3(p.x, 0.0, base + 2.1), p.z - base - 2.6, 0.36, 0.3);
            b.paint(ACCENT);
            b.chamfered_box(v3(p.x - 0.72, 0.0, base + 1.0), v3(0.7, 1.0, 1.2), 0.15);
            // The haul falls from the head down a glazed chute into the pack.
            chute(
                b,
                v3(p.x - 0.5, 0.0, p.z - 0.75),
                v3(p.x - 0.62, 0.0, base + 1.55),
                0.3,
            );
            if !b.coarse() {
                b.paint(PLATING);
                b.plate(v3(p.x - 0.72, 0.0, base + 1.6), v2(0.6, 0.9), 0.06, 0.02);
                b.paint(METAL);
                b.cylinder_between(
                    v3(p.x - 0.5, 0.0, base + 1.8),
                    v3(p.x - 0.4, 0.0, p.z - 0.6),
                    0.09,
                    0.07,
                    5,
                );
            }
        });
    });
    reclaim_head(b, 0, p, p.z - 0.62, 1.0, Head::Cradle);
    tracked_deck(b, &deck, 0.78);
}

/// B: tracked hull, a squat faceted tower amidships with a ball head on it, and
/// four collector pylons at the deck corners, as on the reclaim tower.
pub(in crate::aster) fn gleaner_b(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracks(b);
    let p = TOWER_PIVOT;
    let (foot, top) = (deck.z - 0.1, p.z - 0.62);
    b.paint(ACCENT);
    b.at(v3(p.x, 0.0, 0.0), |b| {
        b.loft_z(
            &ngon(b.sides(8), 1.35),
            &[
                Section::new(foot, 1.0),
                Section::new(foot + 0.9, 0.94),
                Section::new(top, 0.72),
            ],
        );
    });
    if !b.coarse() {
        // White plates on the tower's cardinal faces, team on the rear one.
        let apothem = 1.35 * (std::f32::consts::PI / 8.0).cos();
        for (i, yaw) in [0.0f32, 90.0, 180.0, 270.0].into_iter().enumerate() {
            b.yawed(v3(p.x, 0.0, 0.0), yaw.to_radians(), |b| {
                on_slope(
                    b,
                    [apothem * 0.94, foot + 0.9],
                    [apothem * 0.72, top],
                    0.5,
                    |b| {
                        if i == 2 {
                            team_panel(b, Vec3::ZERO, v2(0.8, 0.7));
                        } else {
                            b.paint(PLATING);
                            b.plate(Vec3::ZERO, v2(0.9, 0.75), 0.07, 0.03);
                        }
                    },
                );
            });
        }
    }
    reclaim_head(b, 0, p, top, 1.05, Head::Ball);
    b.mirror_y(|b| {
        for x in [deck.at(0.02, 0.0).x, deck.at(0.98, 0.0).x] {
            pylon(b, v3(x, deck.half_width * 0.8, deck.z), 1.5, 0.16);
        }
    });
    hopper(b, deck.at(0.8, 0.0), v2(1.3, 1.5), 0.45);
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(METAL);
            b.cylinder_between(v3(-3.0, 1.0, 1.9), v3(-3.0, 1.0, 2.6), 0.12, 0.1, 6);
        });
    }
}

/// C: a six-wheeled carrier, cab forward, hoppers amidships, a lance head on an
/// A-frame over the tail.
pub(in crate::aster) fn gleaner_c(b: &mut MeshBuilder, _tech: u8) {
    let deck = wheeled_hull(
        b,
        &WheeledHull {
            rear: -3.8,
            front: 3.9,
            half_width: 1.75,
            belly: 0.8,
            deck: 1.75,
            axles: &[-2.7, -1.3, 2.6],
            wheel_r: 0.68,
            wheel_w: 0.6,
            track: 2.05,
        },
    );
    // Cab: a raked white box forward with a dark screen.
    let (cab0, cab1) = (1.7, 3.6);
    b.paint(PLATING);
    b.frustum(
        v3((cab0 + cab1) * 0.5, 0.0, deck.z - 0.05),
        v2(cab1 - cab0, 3.0),
        v2(cab1 - cab0 - 0.9, 2.5),
        0.95,
        v2(-0.35, 0.0),
    );
    if !b.coarse() {
        on_slope(b, [cab1, deck.z], [cab1 - 0.45, deck.z + 0.9], 0.5, |b| {
            b.paint(GLASS);
            b.plate(Vec3::ZERO, v2(0.6, 2.2), 0.05, 0.02);
        });
        team_panel(b, v3(2.3, 0.0, deck.z + 0.9), v2(0.8, 2.0));
    }
    // Hoppers in a row behind the cab.
    hopper(b, v3(0.55, 0.0, deck.z), v2(1.6, 2.4), 0.6);
    hopper(b, v3(-0.9, 0.0, deck.z), v2(1.1, 2.4), 0.6);

    // A-frame: two legs from the deck corners to a head block, fixed; the head turns on top.
    let p = CRANE_PIVOT;
    let apex = v3(p.x, 0.0, p.z - 0.85);
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.beam(
            v3(p.x - 0.9, 1.2, deck.z),
            apex + v3(0.0, 0.35, 0.0),
            v2(0.35, 0.35),
            v2(0.28, 0.28),
        );
        b.beam(
            v3(p.x + 0.9, 1.2, deck.z),
            apex + v3(0.0, 0.35, 0.0),
            v2(0.3, 0.3),
            v2(0.24, 0.24),
        );
    });
    b.paint(ACCENT);
    b.chamfered_box(apex - Vec3::Z * 0.1, v3(1.3, 1.3, 0.5), 0.2);
    // A chute from under the head block down into the rear hopper.
    chute(
        b,
        apex + v3(0.5, 0.0, -0.3),
        v3(-0.9, 0.0, deck.z + 0.65),
        0.34,
    );
    reclaim_head(b, 0, p, apex.z + 0.15, 1.05, Head::Lance);
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cuboid(v3(3.65, 1.25, 1.35), v3(0.3, 0.3, 0.26));
            b.paint(GLASS);
            b.cuboid(v3(3.81, 1.25, 1.35), v3(0.04, 0.22, 0.18));
        });
        whip(b, v3(1.9, -1.1, deck.z + 0.9), 1.0, 0.2);
    }
}
