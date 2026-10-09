//! The Aerie: the air factory.

use glam::Vec3;

use super::*;

const AIR_HANGAR_Y: f32 = 27.0;
const AIR_GANTRY_X: f32 = -19.0;
const AIR_GANTRY_Z: f32 = 18.0;
const AIR_TOWER: Vec3 = Vec3::new(33.0, -37.0, 0.0);
const EAVES: f32 = 8.5;
const RIDGE: f32 = 13.0;

/// Air hangar: a clear lift deck with nothing over it, a white hangar either side
/// with its shutters on the deck, a portal gantry over the deck's back edge carrying
/// the fabricators, a blast fence behind that, a marked launch lane out +x and a
/// control tower beside the lane.
pub(in crate::aster) fn factory_air(b: &mut MeshBuilder, tech: u8) {
    const HANGAR_X0: f32 = -44.0;
    const HANGAR_X1: f32 = 18.0;
    let y0 = AIR_HANGAR_Y;

    if b.coarse() {
        b.paint(PLATING_DARK);
        b.face(&[
            v3(-46.0, -26.0, 0.3),
            v3(46.0, -26.0, 0.3),
            v3(46.0, 26.0, 0.3),
            v3(-46.0, 26.0, 0.3),
        ]);
        b.paint(PLATING);
        b.mirror_y(|b| {
            b.frustum_open(
                v3((HANGAR_X0 + HANGAR_X1) * 0.5, y0 + 9.0, 0.0),
                v2(HANGAR_X1 - HANGAR_X0, 18.0),
                v2(HANGAR_X1 - HANGAR_X0, 14.0),
                RIDGE,
                v2(0.0, -2.0),
            )
        });
        b.cuboid_open(
            v3(AIR_GANTRY_X, 0.0, AIR_GANTRY_Z + 1.2),
            v3(3.0, 54.0, 2.4),
        );
        b.cuboid_open(AIR_TOWER + Vec3::Z * 12.0, v3(7.0, 7.0, 24.0));
        team_panel(b, v3(-13.0, y0 + 7.0, RIDGE), v2(20.0, 3.0));
        if tech >= 2 {
            let top = if tech >= 3 { 40.0 } else { 31.0 };
            b.paint(METAL);
            b.face(&[
                AIR_TOWER + v3(-0.5, 0.0, 24.0),
                AIR_TOWER + v3(0.5, 0.0, 24.0),
                AIR_TOWER + v3(0.5, 0.0, top),
                AIR_TOWER + v3(-0.5, 0.0, top),
            ]);
        }
        return;
    }

    // Deck: a graphite apron across the lot, the lift let into it.
    b.paint(PLATING_DARK);
    apron(b, v2(-46.0, -26.0), v2(46.0, 26.0), v2(17.3, 17.3), 0.3);
    factory_pad(b, 15.0, 15.0);

    // Launch lane: white, chevrons toward the way out, lights that run while it builds.
    b.paint(ACCENT);
    b.frustum(
        v3(31.6, 0.0, 0.3),
        v2(28.8, 26.0),
        v2(28.0, 25.0),
        0.3,
        v2(0.0, 0.0),
    );
    b.paint(PLATING).pattern(pattern::ROADWAY);
    b.plate(v3(31.6, 0.0, 0.6), v2(27.6, 23.0), 0.1, 0.04);

    // Hangars: white shells on a black plinth, shutters facing the deck.
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.extrude_x(
            &[
                [y0, 0.0],
                [y0 + 18.0, 0.0],
                [y0 + 18.0, EAVES],
                [y0 + 16.0, EAVES + 1.2],
                [y0 + 1.0, RIDGE],
                [y0, RIDGE - 0.6],
            ],
            HANGAR_X0,
            HANGAR_X1,
        );
        b.paint(ACCENT);
        b.block(
            v3(HANGAR_X0 - 0.3, y0 - 0.3, 0.0),
            v3(HANGAR_X1 + 0.3, y0 + 18.3, 1.4),
        );
        // Gable ends: black edging along the roofline.
        let edged: &[f32] = if b.fine() {
            &[HANGAR_X0 - 0.2, HANGAR_X1 + 0.2]
        } else {
            &[]
        };
        for &x in edged {
            b.beam(
                v3(x, y0 - 0.2, RIDGE - 0.4),
                v3(x, y0 + 1.0, RIDGE + 0.2),
                v2(0.8, 0.6),
                v2(0.8, 0.6),
            );
            b.beam(
                v3(x, y0 + 1.0, RIDGE + 0.2),
                v3(x, y0 + 16.0, EAVES + 1.4),
                v2(0.8, 0.6),
                v2(0.8, 0.6),
            );
            b.beam(
                v3(x, y0 + 16.0, EAVES + 1.4),
                v3(x, y0 + 18.3, EAVES + 0.2),
                v2(0.8, 0.6),
                v2(0.8, 0.6),
            );
        }
        // Roof: a catwalk down the long slope, close up only.
        if b.fine() {
            on_slope_y(b, [y0 + 16.0, EAVES + 1.2], [y0 + 1.0, RIDGE], 0.5, |b| {
                b.paint(PLATING);
                b.plate(v3(-13.0, -2.5, 0.0), v2(56.0, 3.2), 0.14, 0.05);
            });
        }
        // Deck face: black pilasters between three shutter bays, conduit over them.
        b.paint(ACCENT);
        for x in [-40.0, -22.0, -4.0, 14.0] {
            b.block(
                v3(x - 0.9, y0 - 0.6, 0.0),
                v3(x + 0.9, y0 + 0.2, RIDGE - 0.8),
            );
        }
        // Between them the hangar stands open: a dark mouth, lights let into it.
        for x in [-31.0, -13.0, 5.0] {
            b.paint(ACCENT);
            b.block(v3(x - 8.0, y0 - 0.2, 1.4), v3(x + 8.0, y0 + 0.1, 8.6));
        }
        conduit(b, v3(-41.0, y0 - 0.9, 9.0), v3(15.0, y0 - 0.1, 10.0));
        if b.fine() {
            // Outer wall: a band of black at the eaves, lights let into it.
            b.paint(ACCENT);
            b.block(
                v3(HANGAR_X0 + 1.0, y0 + 18.0, EAVES - 1.6),
                v3(HANGAR_X1 - 1.0, y0 + 18.25, EAVES - 0.4),
            );
        }
    });

    // Blast fence behind the deck: white plates raked toward it on black ribs.
    b.paint(ACCENT);
    b.block(v3(-31.0, -24.0, 0.0), v3(-29.6, 24.0, 1.2));
    on_slope(b, [-25.2, 6.8], [-30.2, 1.1], 0.5, |b| {
        b.paint(PLATING);
        b.plate(Vec3::ZERO, v2(7.4, 46.0), 0.25, 0.08);
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.plate(v3(0.0, 0.0, 0.25), v2(1.0, 40.0), 0.08, 0.03);
        }
    });
    if b.fine() {
        for y in [-22.0, -11.0, 0.0, 11.0, 22.0] {
            b.paint(ACCENT);
            b.extrude_y(
                &[[-31.2, 0.0], [-29.0, 0.0], [-24.6, 6.8], [-26.0, 7.2]],
                y - 0.4,
                y + 0.4,
            );
        }
    }

    // Portal gantry over the deck's back edge.
    portal_gantry(b, AIR_GANTRY_X, 23.6, 0.0, AIR_GANTRY_Z);
    // The carriage: a black trolley under the bridge, the fabricators hung from it.
    b.paint(ACCENT);
    b.cuboid(
        v3(AIR_GANTRY_X, 0.0, AIR_GANTRY_Z - 0.6),
        v3(4.2, 16.0, 1.2),
    );
    b.paint(PLATING);
    b.mirror_y(|b| b.plate(v3(AIR_GANTRY_X, 2.0, AIR_GANTRY_Z), v2(4.4, 2.6), 0.1, 0.04));
    for (mount, s, aim) in heads("factory_air", 1) {
        fabricator(b, mount, aim, s, AIR_GANTRY_Z - 1.0);
    }

    // Control tower beside the lane.
    factory_tower(b, AIR_TOWER, 0.0, 24.0);
    if b.fine() {
        antenna_unlit(b, AIR_TOWER + v3(-2.5, 3.0, 24.0), 3.6, 0.06);
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.block(
            AIR_TOWER + v3(4.3, -2.0, 2.0),
            AIR_TOWER + v3(4.65, 2.0, 16.0),
        );
    }

    // Suite II: a second portal over the deck's front edge, fabricators on the hangars'
    // deck faces, drums behind the fence, a radar mast.
    kit(b, tech, 2, 0.22, |b| {
        portal_gantry(b, 19.5, 23.6, 0.3, AIR_GANTRY_Z);
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(-4.0, y0 - 3.0, 9.0), v3(4.0, y0 - 0.6, 10.2));
            b.paint(PLATING);
            b.block(v3(-3.8, y0 - 2.9, 10.2), v3(3.8, y0 - 0.7, 10.34));
        });
        for (mount, s, aim) in heads("factory_air", 2) {
            fabricator(b, mount, aim, s, 10.34);
        }
        for y in [-13.0, 13.0] {
            fabricator_tank(b, v3(-38.0, y, 0.3), 3.6, 9.0);
            b.paint(METAL);
            b.cylinder_between(v3(-34.6, y, 2.0), v3(-30.4, y * 0.8, 1.0), 0.4, 0.4, 6);
        }
        b.paint(ACCENT);
        b.prism(AIR_TOWER + v3(-0.4, 0.0, 24.14), 8, 2.6, 2.0, 1.6);
        b.paint(METAL);
        b.prism(AIR_TOWER + v3(-0.4, 0.0, 25.7), 6, 0.5, 0.25, 5.0);
        b.paint(PLATING);
        b.beam(
            AIR_TOWER + v3(-0.4, -2.4, 29.4),
            AIR_TOWER + v3(-0.4, 2.4, 29.4),
            v2(0.3, 1.1),
            v2(0.3, 1.1),
        );
    });

    // Suite III: cantilever gantries on the hangar roofs with fabricators of their
    // own, and the tower's mast raised.
    kit(b, tech, 3, 0.3, |b| {
        // Roof lanterns along each hangar's ridge, louvred along the deck side.
        b.mirror_y(|b| {
            for (x0, x1) in [(-40.0, -12.0), (-3.0, 14.0)] {
                let cx = (x0 + x1) * 0.5;
                b.paint(PLATING);
                b.cuboid_open(v3(cx, y0 + 5.0, 13.3), v3(x1 - x0, 4.0, 3.8));
                b.paint(ACCENT);
                b.cuboid(v3(cx, y0 + 5.0, 15.0), v3(x1 - x0 + 0.3, 4.3, 0.5));
                b.paint(PLATING);
                b.plate(v3(cx, y0 + 5.0, 15.25), v2(x1 - x0 - 1.0, 3.4), 0.12, 0.04);
            }
        });
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(-8.0, y0 + 4.0, RIDGE - 1.0), v3(4.2, 4.2, 2.0));
            b.paint(ACCENT);
            b.frustum_open(
                v3(-8.0, y0 + 4.0, RIDGE),
                v2(3.0, 3.0),
                v2(2.2, 2.2),
                11.8,
                v2(0.0, -0.4),
            );
            conduit(
                b,
                v3(-9.8, y0 + 2.6, RIDGE + 1.0),
                v3(-9.45, y0 + 4.4, 23.0),
            );
            b.paint(ACCENT);
            b.cuboid(v3(-8.0, y0 - 5.0, 24.2), v3(2.4, 21.0, 0.6));
            b.paint(PLATING);
            b.chamfered_box(v3(-8.0, y0 - 5.0, 25.4), v3(2.8, 22.0, 1.8), 0.4);
            b.paint(ACCENT);
            b.cuboid(v3(-8.0, 17.0, 23.2), v3(3.2, 3.6, 1.4));
        });
        for (mount, s, aim) in heads("factory_air", 3) {
            fabricator(b, mount, aim, s, 22.6);
        }
        b.paint(PLATING);
        b.cuboid_open(AIR_TOWER + v3(-0.4, 0.0, 32.5), v3(1.8, 1.8, 4.0));
        b.paint(METAL);
        b.prism(AIR_TOWER + v3(-0.4, 0.0, 34.5), 6, 0.4, 0.15, 4.0);
    });

    if tech >= 2 && b.fine() {
        antenna_unlit(b, AIR_TOWER + v3(-3.0, -3.2, 24.0), 2.8, 0.1);
    }
    marks(b, tech);
}

/// What the Aerie's surface carries where (docs/STANDARD.md, "Texture detail and marks").
fn marks(b: &mut MeshBuilder, tech: u8) {
    use crate::gpu_consts::surface_mark as mark;
    // Holes and slats at a building's pitch, not a vehicle's.
    const PITCH: f32 = 0.22;
    let y0 = AIR_HANGAR_Y;
    // The blast fence takes the jets: armour, a warning at each end.
    b.mark(
        v3(-30.4, -23.0, 1.0),
        v3(-25.0, 23.0, 7.2),
        mark::ARMOUR,
        0.0,
    );
    // Each hangar's number on its outer wall, between its doors.
    for (side, number) in [(1.0, 2.0), (-1.0, 3.0)] {
        let y = side * (y0 + 18.0);
        b.mark(
            v3(-16.0, y - 0.3, 2.0),
            v3(-10.0, y + 0.3, 6.4),
            mark::STENCIL,
            number,
        );
    }
    // The control tower: a door toward the lane, lettering over it.
    b.mark(
        AIR_TOWER + v3(-1.2, 5.5, 1.6),
        AIR_TOWER + v3(1.2, 6.1, 4.8),
        mark::ACCESS,
        0.0,
    );
    b.mark(
        AIR_TOWER + v3(-1.8, 5.5, 5.2),
        AIR_TOWER + v3(1.8, 6.1, 6.0),
        mark::LABEL,
        1.0,
    );
    b.mirror_y(|b| {
        b.mark(
            v3(-28.6, 18.6, 2.4),
            v3(-26.4, 20.6, 5.6),
            mark::WARNING,
            0.0,
        );
        // The roof: grilles over the bays, the owner's colour.
        on_slope_y(b, [y0 + 16.0, EAVES + 1.2], [y0 + 1.0, RIDGE], 0.5, |b| {
            for x in [-31.0, -13.0, 5.0] {
                b.mark(
                    v3(x - 1.8, 2.2, -0.1),
                    v3(x + 1.8, 4.2, 0.2),
                    mark::GRILLE,
                    PITCH,
                );
            }
            team_panel(b, v3(12.5, 3.0, 0.0), v2(8.0, 3.6));
        });
        // The outer wall: two doors.
        for x in [-36.0, 8.0] {
            b.mark(
                v3(x - 1.3, y0 + 17.7, 1.5),
                v3(x + 1.3, y0 + 18.3, 4.6),
                mark::ACCESS,
                0.0,
            );
        }
        // The gable on the lane: louvres under the roof.
        b.mark(
            v3(17.7, y0 + 5.0, 5.4),
            v3(18.4, y0 + 13.0, 8.2),
            mark::LOUVRES,
            PITCH,
        );
        if tech >= 3 {
            // The roof lanterns: louvres down the deck side, the owner's colour along the top.
            for (x0, x1) in [(-40.0, -12.0), (-3.0, 14.0)] {
                b.mark(
                    v3(x0 + 1.0, y0 + 2.7, 12.9),
                    v3(x1 - 1.0, y0 + 3.3, 14.5),
                    mark::LOUVRES,
                    PITCH,
                );
                team_panel(
                    b,
                    v3((x0 + x1) * 0.5, y0 + 5.0, 15.37),
                    v2(x1 - x0 - 3.0, 1.2),
                );
            }
        }
    });
}
