//! The Wharf: the naval factory.

use glam::Vec3;

use super::*;

/// The quay's berth face: everything the Wharf has stands behind it (-y).
const QUAY_FACE: f32 = -24.0;
const QUAY_BACK: f32 = -46.0;
const QUAY_X: f32 = 44.0;
/// Deck level over the water (a naval structure's origin is on the water).
const DECK: f32 = 5.0;
const CRANE_X: [f32; 2] = [-24.0, 20.0];
const CRANE_Y: f32 = -31.0;
const CRANE_Z: f32 = 21.0;
const JIB_TIP: f32 = -11.0;
const WHARF_TOWER: Vec3 = Vec3::new(35.0, -38.0, DECK);
/// The suite III hammerhead crane: where its tower stands, and the top of that tower.
const HAMMER: Vec3 = Vec3::new(0.0, -37.0, 33.0);

/// Naval yard: a one-sided quay on piles driven into the seabed. The berth is open
/// water at the lot origin, where the hull floats while it is printed; nothing stands
/// on the far side of it, so a ship longer or wider than the yard still fits. Cranes
/// reach out over the berth to hang their fabricators over the hull.
pub(in crate::aster) fn factory_naval(b: &mut MeshBuilder, tech: u8) {
    let quay_mid = (QUAY_FACE + QUAY_BACK) * 0.5;
    let quay_w = QUAY_FACE - QUAY_BACK;
    // Works hall at the back of the quay: x0..x1 along it, its berth side at `front`.
    let (hx0, hx1, back, front) = (-40.0, -6.0, -45.0, -34.0);
    let hall_top = DECK + 11.0;

    if b.coarse() {
        // The quay down to the seabed (`part::PILE`: its foot is let down onto the bottom).
        b.with_part(part::PILE, |b| {
            b.paint(PLATING_DARK);
            b.cuboid_open(
                v3(0.0, quay_mid, DECK * 0.5),
                v3(QUAY_X * 2.0, quay_w, DECK),
            );
        });
        b.paint(PLATING);
        b.cuboid_open(
            v3((hx0 + hx1) * 0.5, (back + front) * 0.5, DECK + 5.5),
            v3(hx1 - hx0, front - back, 11.0),
        );
        b.cuboid_open(WHARF_TOWER + Vec3::Z * 10.4, v3(7.0, 7.0, 20.8));
        for x in CRANE_X {
            b.cuboid_open(v3(x, CRANE_Y, CRANE_Z * 0.5), v3(3.0, 3.0, CRANE_Z));
            b.face(&[
                v3(x - 1.2, CRANE_Y - 10.0, CRANE_Z + 1.2),
                v3(x + 1.2, CRANE_Y - 10.0, CRANE_Z + 1.2),
                v3(x + 1.2, JIB_TIP, CRANE_Z + 1.2),
                v3(x - 1.2, JIB_TIP, CRANE_Z + 1.2),
            ]);
        }
        team_panel(b, v3((hx0 + hx1) * 0.5, -41.0, hall_top), v2(18.0, 5.0));
        if tech == 2 {
            b.paint(METAL);
            b.face(&[
                WHARF_TOWER + v3(-0.5, 0.0, 20.8),
                WHARF_TOWER + v3(0.5, 0.0, 20.8),
                WHARF_TOWER + v3(0.5, 0.0, 30.7 - DECK),
                WHARF_TOWER + v3(-0.5, 0.0, 30.7 - DECK),
            ]);
        }
        if tech >= 3 {
            b.paint(PLATING);
            b.face(&[
                v3(HAMMER.x - 2.0, HAMMER.y, DECK),
                v3(HAMMER.x + 2.0, HAMMER.y, DECK),
                v3(HAMMER.x + 2.0, HAMMER.y, HAMMER.z + 7.0),
                v3(HAMMER.x - 2.0, HAMMER.y, HAMMER.z + 7.0),
            ]);
        }
        return;
    }

    // Piles, down to the seabed (`part::PILE`): a row standing proud of the berth face,
    // capped at the coping, and a row under the back of the deck. A wale ties each row at
    // the waterline, and cross-bracing runs between them, half in the water.
    let piles = [-40.0, -24.0, -8.0, 8.0, 24.0, 40.0];
    let (front_row, back_row) = (QUAY_FACE + 1.3, QUAY_BACK + 3.0);
    for x in piles {
        b.with_part(part::PILE, |b| {
            b.paint(PLATING_DARK).pattern(pattern::PILE);
            b.cylinder_between(
                v3(x, front_row, 0.0),
                v3(x, front_row, DECK + 0.5),
                1.2,
                1.2,
                b.sides(8),
            );
            b.paint(PLATING_DARK).pattern(pattern::PILE);
            b.cylinder_between(
                v3(x, back_row, 0.0),
                v3(x, back_row, DECK - 1.6),
                1.1,
                1.1,
                b.sides(8),
            );
        });
        b.paint(ACCENT);
        b.cuboid(v3(x, front_row, DECK + 0.7), v3(3.0, 3.0, 0.5));
        if b.fine() {
            b.paint(PLATING);
            b.plate(v3(x, front_row, DECK + 0.95), v2(2.6, 2.6), 0.12, 0.04);
            b.paint(ACCENT);
            b.cuboid(v3(x, back_row, DECK - 2.0), v3(3.0, 3.0, 0.8));
            b.paint(METAL);
            b.beam(
                v3(x, front_row, DECK - 2.2),
                v3(x, back_row, -8.0),
                v2(0.5, 0.5),
                v2(0.5, 0.5),
            );
            b.beam(
                v3(x, back_row, DECK - 2.2),
                v3(x, front_row, -8.0),
                v2(0.5, 0.5),
                v2(0.5, 0.5),
            );
        }
    }
    for y in [front_row, back_row] {
        b.paint(PLATING_DARK).pattern(pattern::PILE);
        b.cuboid(v3(0.0, y, 0.3), v3(QUAY_X * 2.0 - 4.0, 1.0, 1.4));
    }
    if b.fine() {
        // Walings along the exposed row, level with the deck's underside.
        b.paint(METAL);
        b.cuboid(
            v3(0.0, front_row, DECK - 2.4),
            v3(QUAY_X * 2.0 - 4.0, 0.6, 0.6),
        );
    }

    // The deck: a graphite slab, a white deck on it, a hazard-striped coping along
    // the berth, and a conduit run on the berth face under it.
    b.paint(PLATING_DARK);
    b.cuboid(v3(0.0, quay_mid, DECK - 1.0), v3(QUAY_X * 2.0, quay_w, 2.0));
    b.paint(PLATING);
    b.plate(
        v3(0.0, quay_mid - 1.0, DECK),
        v2(QUAY_X * 2.0 - 1.0, quay_w - 3.0),
        0.12,
        0.04,
    );
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.block(
        v3(-QUAY_X, QUAY_FACE - 2.4, DECK),
        v3(QUAY_X, QUAY_FACE + 0.3, DECK + 0.7),
    );
    conduit(
        b,
        v3(-QUAY_X + 1.0, QUAY_FACE - 0.1, DECK - 1.8),
        v3(QUAY_X - 1.0, QUAY_FACE + 0.35, DECK - 0.8),
    );
    // Fenders on the berth face between the piles, down into the water.
    b.paint(TREAD);
    for x in [-32.0, -16.0, 0.0, 16.0, 32.0] {
        b.cylinder_between(
            v3(x, QUAY_FACE + 0.9, -2.6),
            v3(x, QUAY_FACE + 0.9, DECK - 0.4),
            0.9,
            0.9,
            b.sides(8),
        );
    }
    if b.fine() {
        // Bollards along the coping, lamps at its ends, ladders down the face.
        for x in [-40.0, -28.0, -12.0, 4.0, 16.0, 32.0] {
            b.paint(ACCENT);
            b.cylinder_between(
                v3(x, QUAY_FACE - 1.2, DECK + 0.7),
                v3(x, QUAY_FACE - 1.2, DECK + 1.5),
                0.45,
                0.55,
                8,
            );
            b.cylinder_between(
                v3(x, QUAY_FACE - 1.2, DECK + 1.5),
                v3(x, QUAY_FACE - 1.2, DECK + 1.7),
                0.7,
                0.7,
                8,
            );
        }
        for x in [-42.5, 42.5] {
            beacon(b, v3(x, QUAY_FACE - 1.0, DECK + 0.7));
        }
        for x in [-30.0, 22.0] {
            b.paint(METAL);
            for dx in [-0.35, 0.35] {
                b.block(
                    v3(x + dx - 0.06, QUAY_FACE + 0.3, -3.0),
                    v3(x + dx + 0.06, QUAY_FACE + 0.42, DECK + 0.7),
                );
            }
            for k in 0..6 {
                let z = -2.5 + k as f32 * 1.0;
                b.block(
                    v3(x - 0.35, QUAY_FACE + 0.32, z),
                    v3(x + 0.35, QUAY_FACE + 0.4, z + 0.08),
                );
            }
        }
    }

    // Works hall: white, a black plinth and cornice, shutters on the berth side.
    b.paint(PLATING);
    b.extrude_x(
        &[
            [back, DECK],
            [front, DECK],
            [front, DECK + 8.4],
            [front - 2.4, hall_top],
            [back + 1.0, hall_top],
            [back, hall_top - 1.4],
        ],
        hx0,
        hx1,
    );
    b.paint(ACCENT);
    b.block(
        v3(hx0 - 0.3, back - 0.3, DECK),
        v3(hx1 + 0.3, front + 0.3, DECK + 1.3),
    );
    b.block(
        v3(hx0 - 0.25, front - 1.6, DECK + 8.0),
        v3(hx1 + 0.25, front + 0.25, DECK + 8.8),
    );
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(
        v3((hx0 + hx1) * 0.5, -42.0, hall_top),
        v2(hx1 - hx0 - 2.0, 3.8),
        0.14,
        0.05,
    );
    for x in [-32.0, -15.0] {
        b.paint(ACCENT);
        b.block(
            v3(x - 6.4, front, DECK),
            v3(x + 6.4, front + 0.35, DECK + 7.6),
        );
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.block(
            v3(x - 5.8, front + 0.1, DECK + 1.2),
            v3(x + 5.8, front + 0.45, DECK + 6.6),
        );
    }
    if b.fine() {
        vent(b, v3(-33.0, -38.3, hall_top), v2(4.0, 2.4), 3, ACCENT);
        vent(b, v3(-13.0, -38.3, hall_top), v2(4.0, 2.4), 3, ACCENT);
        b.paint(ACCENT).pattern(pattern::FURNACE);
        b.plate(v3(-23.0, -38.3, hall_top), v2(5.0, 2.4), 0.08, 0.04);
        // Pilasters along the back wall.
        for x in [-34.0, -23.0, -12.0] {
            b.paint(ACCENT);
            b.block(
                v3(x - 0.6, back - 0.35, DECK + 1.3),
                v3(x + 0.6, back + 0.1, hall_top - 1.6),
            );
        }
    }

    // Fabricator cranes: a white tower on a black plinth, a slewing house, a jib out
    // over the berth with the fabricator hung under its tip, a counter-jib and weight.
    for x in CRANE_X {
        let foot = v3(x, CRANE_Y, DECK);
        b.paint(ACCENT);
        b.cuboid_open(foot + Vec3::Z * 0.7, v3(5.0, 5.0, 1.4));
        b.paint(PLATING);
        b.cuboid_open(
            foot + Vec3::Z * (1.4 + (CRANE_Z - DECK - 3.4) * 0.5),
            v3(3.0, 3.0, CRANE_Z - DECK - 3.4),
        );
        if b.fine() {
            // The tower's lattice, drawn as black bracing on each white face.
            for n in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y] {
                b.paint(ACCENT);
                let side = Vec3::Z.cross(n) * 1.25;
                let at = foot + n * 1.56;
                b.beam(
                    at + side + Vec3::Z * 2.0,
                    at - side + Vec3::Z * 8.6,
                    v2(0.3, 0.3),
                    v2(0.3, 0.3),
                );
                b.beam(
                    at - side + Vec3::Z * 8.6,
                    at + side + Vec3::Z * 15.2,
                    v2(0.3, 0.3),
                    v2(0.3, 0.3),
                );
            }
        }
        // Slewing house, glazed toward the berth.
        b.paint(ACCENT);
        b.cuboid(v3(x, CRANE_Y - 1.0, CRANE_Z - 1.2), v3(4.0, 6.0, 2.4));
        b.paint(PLATING);
        b.plate(v3(x, CRANE_Y - 1.4, CRANE_Z), v2(3.6, 5.0), 0.12, 0.04);
        team_panel(b, v3(x, CRANE_Y - 2.6, CRANE_Z + 0.12), v2(2.6, 2.0));
        if b.fine() {
            b.paint(GLASS);
            b.block(
                v3(x - 1.6, CRANE_Y + 1.95, CRANE_Z - 2.0),
                v3(x + 1.6, CRANE_Y + 2.05, CRANE_Z - 0.9),
            );
        }
        // Jib out to the tip over the berth; counter-jib back over the quay, its weight.
        b.paint(PLATING);
        b.beam(
            v3(x, CRANE_Y + 2.0, CRANE_Z + 0.8),
            v3(x, JIB_TIP, CRANE_Z + 0.5),
            v2(2.2, 1.8),
            v2(1.6, 1.3),
        );
        b.paint(ACCENT);
        b.beam(
            v3(x, CRANE_Y + 2.0, CRANE_Z - 0.35),
            v3(x, JIB_TIP + 0.4, CRANE_Z - 0.05),
            v2(1.8, 0.5),
            v2(1.3, 0.4),
        );
        b.beam(
            v3(x, CRANE_Y - 4.0, CRANE_Z + 0.4),
            v3(x, CRANE_Y - 10.0, CRANE_Z + 0.4),
            v2(1.8, 1.2),
            v2(1.8, 1.2),
        );
        b.paint(PLATING_DARK);
        b.cuboid(v3(x, CRANE_Y - 10.5, CRANE_Z - 0.6), v3(3.0, 3.0, 3.2));
        conduit(
            b,
            v3(x - 0.2, CRANE_Y + 2.0, CRANE_Z + 1.62),
            v3(x + 0.2, JIB_TIP + 1.5, CRANE_Z + 1.75),
        );
        // A mast over the house, stays to both ends.
        b.paint(ACCENT);
        b.frustum(
            v3(x, CRANE_Y - 1.0, CRANE_Z + 0.1),
            v2(1.2, 1.2),
            v2(0.4, 0.4),
            4.5,
            v2(0.0, 0.0),
        );
        if b.fine() {
            b.paint(METAL);
            b.cylinder_between(
                v3(x, CRANE_Y - 1.0, CRANE_Z + 4.4),
                v3(x, JIB_TIP + 3.0, CRANE_Z + 1.3),
                0.1,
                0.1,
                4,
            );
            b.cylinder_between(
                v3(x, CRANE_Y - 1.0, CRANE_Z + 4.4),
                v3(x, CRANE_Y - 10.0, CRANE_Z + 1.0),
                0.1,
                0.1,
                4,
            );
            antenna(b, v3(x + 0.35, CRANE_Y - 1.0, CRANE_Z + 4.5), 2.0, 0.0);
        }
        // Tip block, the fabricator hung under it.
        b.paint(ACCENT);
        b.cuboid(v3(x, JIB_TIP - 0.6, CRANE_Z - 0.2), v3(2.4, 2.4, 1.4));
    }
    for (mount, s, aim) in heads("factory_naval", 1) {
        fabricator(b, mount, aim, s, CRANE_Z - 0.7);
    }

    // Harbour tower at the far end of the quay.
    factory_tower(b, WHARF_TOWER, 0.0, 24.0 - DECK);
    beacon(b, WHARF_TOWER + v3(2.2, -2.6, 24.14 - DECK));
    if b.fine() {
        antenna(b, WHARF_TOWER + v3(-2.5, 3.0, 24.0 - DECK), 3.6, 0.06);
    }

    // Suite II: fabricators on the coping, drums on the deck, the tower's radar mast.
    kit(b, tech, 2, 0.22, |b| {
        // A fabrication gallery along the berth, on posts, under the crane jibs.
        let gz = DECK + 10.2;
        for x in [-36.0, -16.0, 14.0, 36.0] {
            b.paint(ACCENT);
            b.cuboid_open(
                v3(x, -27.0, DECK + (gz - 1.0 - DECK) * 0.5),
                v3(1.3, 1.3, gz - 1.0 - DECK),
            );
        }
        b.paint(ACCENT);
        b.cuboid(v3(0.0, -27.0, gz - 0.8), v3(83.0, 2.0, 0.5));
        b.paint(PLATING);
        b.chamfered_box(v3(0.0, -27.0, gz), v3(84.0, 2.6, 1.6), 0.4);
        conduit(b, v3(-40.0, -25.75, gz - 0.55), v3(40.0, -25.55, gz + 0.35));
        glow_strip(b, v3(0.0, -27.0, gz + 0.8), v2(70.0, 0.4), GLOW);
        // An upper storey on the works hall.
        b.paint(PLATING);
        b.cuboid_open(v3(-27.0, -42.15, hall_top + 3.0), v3(18.0, 4.7, 6.0));
        b.paint(ACCENT);
        b.cuboid(v3(-27.0, -42.15, hall_top + 5.63), v3(18.3, 5.0, 0.7));
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.block(
            v3(-35.0, -39.8, hall_top + 1.2),
            v3(-19.0, -39.65, hall_top + 4.4),
        );
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(-27.0, -42.15, hall_top + 6.0), v2(17.0, 4.0), 0.12, 0.04);
        for (mount, s, aim) in heads("factory_naval", 2) {
            b.paint(ACCENT);
            b.block(
                v3(mount.x - 1.6, QUAY_FACE - 3.4, DECK + 0.7),
                v3(mount.x + 1.6, QUAY_FACE - 0.4, DECK + 2.6),
            );
            b.paint(PLATING);
            b.block(
                v3(mount.x - 1.5, QUAY_FACE - 3.3, DECK + 2.6),
                v3(mount.x + 1.5, QUAY_FACE - 0.5, DECK + 2.74),
            );
            fabricator(b, mount, aim, s, DECK + 2.74);
        }
        for x in [9.0, 15.5] {
            fabricator_tank(b, v3(x, -41.5, DECK + 0.12), 2.9, 7.0);
        }
        b.paint(METAL);
        b.cylinder_between(
            v3(6.1, -41.5, DECK + 1.2),
            v3(hx1, -41.5, DECK + 1.2),
            0.35,
            0.35,
            6,
        );
        b.paint(ACCENT);
        b.prism(WHARF_TOWER + v3(-0.4, 0.0, 24.14 - DECK), 8, 2.6, 2.0, 1.6);
        b.paint(METAL);
        b.prism(WHARF_TOWER + v3(-0.4, 0.0, 25.7 - DECK), 6, 0.5, 0.25, 5.0);
        b.paint(PLATING);
        b.beam(
            WHARF_TOWER + v3(-0.4, -2.4, 29.4 - DECK),
            WHARF_TOWER + v3(-0.4, 2.4, 29.4 - DECK),
            v2(0.3, 1.1),
            v2(0.3, 1.1),
        );
    });

    // Suite III: a hammerhead crane amidships, taller than the rest, with a trolley
    // of two fabricators run out over the berth.
    kit(b, tech, 3, 0.3, |b| {
        let (x, y, top) = (HAMMER.x, HAMMER.y, HAMMER.z);
        let jib_z = top + 1.0;
        b.paint(ACCENT);
        b.cuboid_open(v3(x, y, DECK + 0.8), v3(6.4, 6.4, 1.6));
        b.paint(PLATING);
        b.frustum_open(
            v3(x, y, DECK + 1.6),
            v2(4.4, 4.4),
            v2(3.2, 3.2),
            top - DECK - 3.8,
            v2(0.0, 0.0),
        );
        conduit(
            b,
            v3(x + 1.9, y - 0.6, DECK + 3.0),
            v3(x + 2.2, y + 0.6, top - 5.0),
        );
        // Slewing house, the jib from the quay's back edge out over the berth.
        b.paint(ACCENT);
        b.cuboid(v3(x, y, top - 1.2), v3(5.0, 7.0, 2.8));
        b.paint(PLATING);
        b.chamfered_box(v3(x, (-44.0 + -4.5) * 0.5, jib_z), v3(3.0, 39.5, 2.2), 0.5);
        b.paint(ACCENT);
        b.cuboid(v3(x, (-40.0 + -5.0) * 0.5, jib_z - 1.3), v3(2.4, 35.0, 0.4));
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(x, -20.0, jib_z + 1.1), v2(2.2, 26.0), 0.1, 0.04);
        // Counterweight under the jib's back end, a mast over the house.
        b.paint(PLATING_DARK);
        b.cuboid(v3(x, -43.8, jib_z - 2.8), v3(4.0, 4.0, 3.6));
        b.paint(ACCENT);
        b.frustum(
            v3(x, y, jib_z + 1.1),
            v2(1.4, 1.4),
            v2(0.4, 0.4),
            5.0,
            v2(0.0, 0.0),
        );
        // The trolley: a cross-beam under the jib carrying the pair.
        b.paint(ACCENT);
        b.cuboid(v3(x, -7.0, jib_z - 1.9), v3(12.0, 3.4, 1.6));
        b.paint(PLATING);
        b.plate(v3(x, -7.0, jib_z - 1.1), v2(11.4, 3.0), 0.1, 0.04);
        for (mount, s, aim) in heads("factory_naval", 3) {
            fabricator(b, mount, aim, s, jib_z - 2.5);
        }
        // The harbour tower's mast raised.
        b.paint(PLATING);
        b.cuboid_open(WHARF_TOWER + v3(-0.4, 0.0, 32.5 - DECK), v3(1.8, 1.8, 4.0));
        b.paint(METAL);
        b.prism(WHARF_TOWER + v3(-0.4, 0.0, 34.5 - DECK), 6, 0.4, 0.15, 4.0);
    });

    if tech >= 2 {
        b.paint(GLOW);
        b.cuboid(
            v3((hx0 + hx1) * 0.5, front + 0.35, DECK + 8.4),
            v3(hx1 - hx0 - 1.0, 0.2, 0.3),
        );
        if b.fine() {
            antenna(b, WHARF_TOWER + v3(-3.0, -3.2, 24.0 - DECK), 2.8, 0.1);
        }
    }
    if tech >= 3 {
        b.paint(GLOW);
        b.prism(v3(HAMMER.x, HAMMER.y, HAMMER.z + 7.1), 6, 0.35, 0.15, 0.8);
        if b.fine() {
            glow_strip(b, v3(HAMMER.x, -30.0, HAMMER.z + 2.2), v2(1.0, 12.0), GLOW);
        }
    }
}
