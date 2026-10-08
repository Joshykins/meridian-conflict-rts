//! The Forge: the land factory.

use glam::{Vec2, Vec3};

use super::*;

/// Land foundry: a U of white halls round the pad, a road out +x, fabricators on
/// the halls' print shelves.
pub(in crate::aster) fn factory_land(b: &mut MeshBuilder, tech: u8) {
    const WALL_H: f32 = 8.4;
    const WALL_Y: f32 = 26.0;
    const HOUSE_Z: f32 = 13.6;
    const TOWER: f32 = FACTORY_TOWER;
    /// Top of the print shelves the fabricators stand on.
    const SHELF: f32 = WALL_H * 0.68 + 0.8;

    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(-26.0, 0.0, 0.0),
            v2(22.0, 36.0),
            v2(16.0, 28.0),
            HOUSE_Z,
            v2(-1.2, 0.0),
        );
        b.mirror_y(|b| b.cuboid_open(v3(2.0, WALL_Y, WALL_H * 0.5), v3(36.0, 8.0, WALL_H)));
        b.paint(ACCENT);
        b.frustum_open(
            v3(29.0, 0.0, 0.0),
            v2(31.0, 34.0),
            v2(27.0, 26.0),
            0.9,
            v2(0.0, 0.0),
        );
        let tower_h = TOWER - HOUSE_Z
            + match tech {
                1 => 0.0,
                2 => 8.0,
                _ => 16.0,
            };
        b.paint(PLATING);
        b.frustum_open(
            v3(-28.0, -16.0, HOUSE_Z),
            v2(9.0, 11.0),
            v2(
                if tech >= 2 { 4.0 } else { 6.4 },
                if tech >= 2 { 5.0 } else { 8.0 },
            ),
            tower_h,
            v2(0.0, 0.0),
        );
        team_panel(b, v3(-24.0, 0.0, HOUSE_Z), v2(10.0, 8.0));
        b.mirror_y(|b| team_panel(b, v3(2.0, WALL_Y, WALL_H), v2(8.0, 4.0)));
        return;
    }

    // Courtyard floor between the halls: graphite plate round the pad's well, so the
    // deck still shows when it drops.
    b.paint(PLATING_DARK);
    apron(b, v2(-13.4, -21.7), v2(21.0, 21.7), v2(13.5, 11.7), 0.28);
    factory_pad(b, 11.2, 9.4);

    // Rear house behind the pad: white walls on a black plinth, a white roof.
    b.paint(PLATING);
    b.extrude_y_chamfered(
        &[
            [-38.0, 0.0],
            [-14.2, 0.0],
            [-14.2, 7.2],
            [-20.0, HOUSE_Z],
            [-32.5, HOUSE_Z],
            [-38.0, 9.2],
        ],
        22.0,
        2.2,
    );
    b.paint(ACCENT);
    b.block(v3(-38.4, -21.2, 0.0), v3(-13.9, 21.2, 1.5));
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(-26.25, 0.0, HOUSE_Z), v2(11.8, 38.0), 0.2, 0.06);
    // The forge's mouth: a long bank of louvres over the door, the fire showing through.
    on_slope(b, [-14.35, 7.35], [-19.85, HOUSE_Z - 0.05], 0.5, |b| {
        b.paint(ACCENT);
        b.plate(Vec3::ZERO, v2(6.6, 36.0), 0.16, 0.05);
        b.paint(ACCENT).pattern(pattern::FURNACE);
        b.plate(v3(0.0, 0.0, 0.16), v2(3.0, 26.0), 0.06, 0.03);
    });
    // The back slope carries a run of conduit.
    on_slope(b, [-32.5, HOUSE_Z], [-38.0, 9.2], 0.5, |b| {
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.plate(Vec3::ZERO, v2(2.2, 34.0), 0.12, 0.04);
    });
    team_panel(b, v3(-26.0, 0.0, HOUSE_Z + 0.22), v2(8.0, 7.0));
    // The hull comes out of a dark aperture lined with conduit, under a white lintel.
    b.paint(ACCENT);
    b.block(v3(-14.2, -10.4, 0.0), v3(-13.6, 10.4, 7.6));
    b.paint(ACCENT).pattern(pattern::CONDUIT);
    b.block(v3(-14.28, -9.4, 1.0), v3(-13.5, 9.4, 6.8));
    b.paint(PLATING);
    b.block(v3(-14.3, -10.8, 7.6), v3(-13.3, 10.8, 8.4));
    // Roof exhaust: drawn, not modelled.
    b.paint(ACCENT).pattern(pattern::FURNACE);
    b.plate(v3(-26.0, -6.6, HOUSE_Z + 0.2), v2(6.4, 3.4), 0.08, 0.04);
    if b.fine() {
        // Black pilasters up the rear wall, so the long white face has a rhythm.
        for y in [-15.0, -5.0, 5.0, 15.0] {
            b.paint(ACCENT);
            b.block(v3(-38.35, y - 0.6, 1.5), v3(-37.9, y + 0.6, 8.9));
        }
    }

    // Side halls: white, a black plinth and cornice, one lid per hall, one inner cheek.
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.chamfered_box(v3(2.0, WALL_Y, WALL_H * 0.5), v3(38.0, 8.6, WALL_H), 1.05);
        b.paint(ACCENT);
        b.chamfered_box(v3(2.0, WALL_Y, 0.7), v3(38.4, 9.0, 1.4), 1.2);
        b.chamfered_box(v3(2.0, WALL_Y, WALL_H - 0.42), v3(38.3, 8.9, 0.96), 1.15);
        b.paint(PLATING);
        b.plate(v3(2.0, WALL_Y, WALL_H), v2(35.0, 7.2), 0.18, 0.06);
        team_panel(b, v3(-1.0, WALL_Y, WALL_H + 0.18), v2(6.0, 3.0));
        b.paint(ACCENT);
        b.block(
            v3(-15.0, WALL_Y - 4.42, 1.3),
            v3(18.0, WALL_Y - 4.05, WALL_H - 0.9),
        );
        if b.fine() {
            vent(b, v3(14.5, WALL_Y, WALL_H + 0.18), v2(4.6, 2.4), 3, ACCENT);
            // The outer wall: buttress ribs, and between them a band of conduit.
            for x in [-12.0, -4.0, 4.0, 12.0] {
                b.paint(ACCENT);
                b.frustum(
                    v3(x, WALL_Y + 4.3, 0.0),
                    v2(1.4, 1.6),
                    v2(1.0, 0.3),
                    WALL_H - 0.9,
                    v2(0.0, -0.6),
                );
            }
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(v3(-15.0, WALL_Y + 4.25, 3.2), v3(19.0, WALL_Y + 4.42, 4.6));
            // Brackets under the print shelf, into the wall.
            for x in [-9.0, 0.0, 10.0] {
                b.paint(ACCENT);
                b.beam(
                    v3(x, WALL_Y - 4.4, 2.2),
                    v3(x, WALL_Y - 6.6, SHELF - 1.4),
                    v2(0.6, 0.5),
                    v2(0.6, 0.5),
                );
            }
        }
        // Print shelf on the inner face: a ledge the fabricators stand on, deep
        // enough to read from above, lit along its face.
        conduit(
            b,
            v3(-11.0, WALL_Y - 7.3, SHELF - 1.48),
            v3(15.0, WALL_Y - 4.3, SHELF - 0.12),
        );
    });

    factory_tower(b, v3(-28.0, -16.0, 0.0), HOUSE_Z, TOWER);

    for (mount, s, aim) in heads("factory_land", 1) {
        fabricator(b, mount, aim, s, SHELF);
    }
    kit(b, tech, 2, 0.18, |b| {
        for (mount, s, aim) in heads("factory_land", 2) {
            fabricator(b, mount, aim, s, SHELF);
        }
    });
    kit(b, tech, 3, 0.14, |b| {
        // Two more on brackets off the forge door's frame.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(-14.4, 6.9, 5.9), v3(-12.0, 9.1, 6.8));
            b.paint(PLATING);
            b.block(v3(-14.3, 7.02, 6.8), v3(-12.1, 8.98, 6.92));
        });
        for (mount, s, aim) in heads("factory_land", 3) {
            fabricator(b, mount, aim, s, 6.92);
        }
    });

    // The road out: a black kerbed apron from the pad's lip to the lot edge.
    b.paint(ACCENT);
    b.frustum(
        v3(29.0, 0.0, 0.2),
        v2(31.0, 36.0),
        v2(27.0, 28.0),
        0.4,
        v2(0.4, 0.0),
    );
    b.paint(PLATING).pattern(pattern::ROADWAY);
    b.plate(v3(29.4, 0.0, 0.62), v2(27.0, 26.0), 0.12, 0.04);
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(16.0, 13.2, 0.55), v3(42.0, 14.2, 0.9));
        });
    }

    kit(b, tech, 2, 0.28, |b| {
        // A gate gantry over the courtyard's mouth, standing on the hall roofs, high
        // enough for the tallest walker to leave under it.
        portal_gantry(b, 18.5, WALL_Y, WALL_H, 26.0);
        b.paint(ACCENT);
        b.cuboid(v3(18.5, 0.0, 25.35), v3(4.0, 10.0, 1.3));
        b.paint(GLOW_AMBER);
        b.cuboid(v3(18.5, 0.0, 24.66), v3(3.0, 8.0, 0.08));
        // Fabricator drums on the house roof, piped down to the forge.
        for y in [9.5, 16.2] {
            fabricator_tank(b, v3(-26.4, y, HOUSE_Z + 0.2), 2.7, 6.0);
            b.paint(METAL);
            b.cylinder_between(
                v3(-23.8, y, HOUSE_Z + 1.2),
                v3(-20.6, y, HOUSE_Z + 0.6),
                0.35,
                0.35,
                6,
            );
        }
        // Assembly lofts on the hall roofs.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(-10.0, WALL_Y, WALL_H + 3.6), v3(10.0, 5.6, 7.2));
            b.paint(ACCENT);
            b.cuboid(v3(-10.0, WALL_Y, WALL_H + 0.7), v3(10.3, 5.9, 1.2));
            b.cuboid(v3(-10.0, WALL_Y, WALL_H + 6.63), v3(10.3, 5.9, 0.7));
            b.paint(PLATING);
            b.plate(v3(-10.0, WALL_Y, WALL_H + 7.2), v2(9.2, 5.0), 0.16, 0.05);
            if b.fine() {
                vent(
                    b,
                    v3(-10.0, WALL_Y, WALL_H + 7.36),
                    v2(4.0, 2.6),
                    3,
                    GLOW_AMBER,
                );
            }
        });
        b.paint(PLATING);
        b.prism(v3(-28.0, -16.0, TOWER), 8, 3.2, 2.6, 2.2);
        b.paint(ACCENT);
        b.prism(v3(-28.0, -16.0, TOWER + 2.2), 8, 2.6, 2.4, 0.3);
        b.paint(METAL);
        b.prism(v3(-28.4, -16.0, TOWER + 2.2), 6, 0.65, 0.26, 8.0);
    });

    kit(b, tech, 3, 0.32, |b| {
        b.paint(METAL);
        b.prism(v3(-28.4, -16.0, TOWER + 2.2), 6, 1.1, 0.42, 14.0);
        // Furnace feed pipes: a bank either side up the rear wall from a pump, over
        // onto the back slope and up it into a manifold at the roof's edge.
        b.mirror_y(|b| {
            const R: f32 = 0.42;
            const RISER_X: f32 = -39.0;
            let sides = b.sides(8);
            // Along the back slope, lifted clear of it.
            let (foot, top) = (Vec2::new(-38.0, 9.2), Vec2::new(-32.5, HOUSE_Z));
            let normal = (top - foot).perp().normalize() * (R + 0.35);
            let on_slope = |t: f32| foot.lerp(top, t) + normal;
            let (low, high) = (on_slope(0.0), on_slope(0.92));
            let ys = [7.8, 9.0, 10.2];
            b.paint(ACCENT);
            b.block(v3(-39.8, 7.0, 0.0), v3(-38.4, 11.0, 1.6));
            b.block(
                v3(high.x - 0.4, 6.9, high.y - 0.9),
                v3(high.x + 0.9, 11.1, high.y + 0.8),
            );
            for y in ys {
                let bend = v3(RISER_X, y, low.y - 0.3);
                let low = v3(low.x, y, low.y);
                b.paint(METAL);
                b.cylinder_between(v3(RISER_X, y, 1.4), bend, R, R, sides);
                b.cylinder_between(bend, low, R, R, sides);
                b.cylinder_between(low, v3(high.x, y, high.y), R, R, sides);
                if b.fine() {
                    b.paint(ACCENT);
                    for (a, c) in [(3.6, 4.0), (bend.z - 0.2, bend.z + 0.2)] {
                        b.cylinder_between(
                            v3(RISER_X, y, a),
                            v3(RISER_X, y, c),
                            R + 0.12,
                            R + 0.12,
                            sides,
                        );
                    }
                    let run = (v3(high.x, y, high.y) - low).normalize();
                    b.cylinder_between(low - run * 0.2, low + run * 0.2, R + 0.12, R + 0.12, sides);
                    // A sight glass on the riser, the furnace showing in it.
                    b.paint(GLOW_ORANGE);
                    b.cylinder_between(
                        v3(RISER_X, y, 5.6),
                        v3(RISER_X, y, 6.2),
                        R + 0.04,
                        R + 0.04,
                        sides,
                    );
                }
            }
        });
        // Tall print lofts beside the first, a conduit down their inner faces.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(7.0, WALL_Y + 0.4, WALL_H + 4.4), v3(8.4, 6.2, 8.8));
            b.paint(ACCENT);
            b.cuboid(v3(7.0, WALL_Y + 0.4, WALL_H + 0.7), v3(8.7, 6.5, 1.4));
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(
                v3(3.2, WALL_Y - 2.9, WALL_H + 2.0),
                v3(10.8, WALL_Y - 2.7, WALL_H + 7.4),
            );
            b.paint(PLATING);
            b.plate(
                v3(7.0, WALL_Y + 0.4, WALL_H + 8.8),
                v2(7.6, 5.6),
                0.16,
                0.05,
            );
        });
    });

    if tech >= 2 {
        b.paint(GLOW);
        for y in [9.5, 16.2] {
            b.prism(v3(-26.4, y, HOUSE_Z + 6.2), 8, 0.9, 0.6, 0.6);
        }
        if b.fine() {
            antenna(b, v3(-25.6, -20.0, TOWER + 2.2), 3.4, 0.08);
        }
    }
    if tech >= 3 {
        b.paint(GLOW);
        b.prism(v3(-28.4, -16.0, TOWER + 16.0), 6, 0.5, 0.2, 1.0);
        if b.fine() {
            b.mirror_y(|b| glow_strip(b, v3(7.0, WALL_Y + 0.4, WALL_H + 8.96), v2(3.4, 2.0), GLOW));
            antenna(b, v3(-31.0, -20.0, TOWER), 4.2, 0.04);
        }
    }
    if b.fine() {
        antenna(b, v3(-31.2, -12.0, TOWER), 4.8, 0.0);
    }
}
