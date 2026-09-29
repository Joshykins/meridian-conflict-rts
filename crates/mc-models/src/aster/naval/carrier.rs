//! The Atoll: tech 3 carrier refitted as the fleet's long-range air defence.
//!
//! A 120 m hull under a full-length flight deck that overhangs the hull on
//! sponsons both sides: the deck is the ship. An angled landing strip runs from
//! the port quarter forward, a launch lane over the bow with two catapult
//! tracks, the owner's band and a flux conduit along both deck edges. Aft in the
//! middle a hatch like the Roost's takes aircraft down to the hangar; a deck-edge
//! lift each side brings them up. The island stands to starboard a third of the
//! way from the stern: faceted, a glass bridge band, a broad mast with the fleet
//! radar's long double-faced array turning on it, and a pole with the red masthead
//! lamp. No gun at all: two armoured blocks of six SAM cells built onto the
//! island's plinth fore and aft, each cell under a hatch hinged on its outer edge
//! (`MeshBuilder::cell_block`), and interceptor tubes in the tunnel stern. Under the water: a bulbous
//! bow, twin skegs running into a tunnel stern that holds two shrouded
//! propulsors, and twin rudders behind them.
//!
//! Origin at the waterline; keel to -8 m; masthead at the data's 24 m.

use super::*;
use crate::builder::CellGrid;
use glam::Vec2;

/// The hull's deck edge, the sponson lip under the deck's overhang, the top of the
/// deck's structure, and the flight deck's surface (the SAM cells' muzzle height).
const HULL_DECK: f32 = 7.0;
const LIP: f32 = 7.9;
const SLAB: f32 = 8.35;
const DECK: f32 = 8.5;

/// Stations stern first. The afterbody is a tunnel stern: from x = -36 aft the
/// keel line rises above the chines, so the hull ends in twin skegs with the
/// propulsors and interceptor tubes in the tunnel between them.
const HULL: [Station; 9] = [
    station(-58.0, -1.0, [-2.2, 7.4], [3.2, 9.6], [HULL_DECK, 9.8]),
    station(-46.0, -1.4, [-2.8, 8.0], [3.2, 10.2], [HULL_DECK, 10.4]),
    station(-36.0, -2.4, [-3.4, 8.5], [3.2, 10.4], [HULL_DECK, 10.6]),
    station(-22.0, -7.4, [-5.4, 8.8], [3.2, 10.5], [HULL_DECK, 10.7]),
    station(0.0, -8.0, [-5.6, 8.8], [3.3, 10.5], [HULL_DECK, 10.7]),
    station(22.0, -7.6, [-5.2, 8.2], [3.5, 10.0], [7.1, 10.3]),
    station(40.0, -6.0, [-4.0, 6.2], [3.9, 8.2], [7.3, 8.7]),
    station(52.0, -3.6, [-2.2, 3.4], [4.5, 5.4], [7.5, 6.0]),
    station(59.5, 1.2, [2.2, 0.0], [5.2, 0.0], [7.8, 0.0]),
];

/// The flight deck's plan, stern first: (x, half width). It overhangs the transom
/// aft and stops short of the stem with a flat, square-ish bow.
const DECK_PLAN: [(f32, f32); 9] = [
    (-60.0, 13.6),
    (-50.0, 15.0),
    (-30.0, 15.5),
    (-8.0, 15.5),
    (14.0, 15.5),
    (30.0, 14.8),
    (42.0, 12.6),
    (52.0, 9.0),
    (57.5, 5.0),
];

/// The island's plan centre (starboard, a third from the stern) and its half plan.
const ISLAND: Vec3 = Vec3::new(-25.0, -12.0, DECK);
const ISLAND_HALF: Vec2 = Vec2::new(7.5, 2.9);
/// The island's roof, the mast's foot on it, the fleet radar's pivot on the masthead.
const ROOF: f32 = 15.45;
const MAST_X: f32 = ISLAND.x - 1.5;
const RADAR: Vec3 = Vec3::new(MAST_X, ISLAND.y, 20.4);
/// The lamp pole aft on the island roof: its top is the model's height.
const LAMP_POLE: Vec3 = Vec3::new(ISLAND.x + 5.2, ISLAND.y, ROOF);
const LAMP_Z: f32 = 23.4;

/// The missile-defence lasers' heads (`anti_missile_mounts` in the unit file): the
/// island's, then three on sponsons out from the deck edge, both bows and the port
/// quarter.
const LASERS: [Vec3; 4] = [
    Vec3::new(-20.0, -13.3, 16.75),
    Vec3::new(36.0, 15.4, 9.1),
    Vec3::new(36.0, -15.4, 9.1),
    Vec3::new(-54.0, 16.1, 9.1),
];
/// Weapon 1: the interceptor tube mouths in the tunnel stern.
const TUBES: [Vec3; 2] = [Vec3::new(-40.0, -3.0, -2.0), Vec3::new(-40.0, 3.0, -2.0)];

/// The hatch's centre and half size; the lifts' centres and size.
const HATCH: Vec2 = Vec2::new(-44.0, 2.0);
const HATCH_HALF: f32 = 6.5;
const LIFTS: [Vec2; 2] = [Vec2::new(3.0, 11.8), Vec2::new(-1.0, -11.8)];
const LIFT_SIZE: Vec2 = Vec2::new(10.0, 6.4);

/// Weapon 0's cells: pitch, a hatch's half width, and the hatches' deck on the blocks.
const CELL_PITCH: f32 = 2.8;
const CELL_HALF: f32 = 1.15;
const CELL_DECK: f32 = DECK + 1.6;
/// The blocks' middles, fore and aft of the island on its centreline, three cells along
/// x by two along y each; the order each fires its cells in.
const CELL_BLOCKS: [Vec2; 2] = [
    Vec2::new(ISLAND.x + ISLAND_HALF.x + 5.2, ISLAND.y),
    Vec2::new(ISLAND.x - ISLAND_HALF.x - 5.2, ISLAND.y),
];
const CELL_FIRE: [(u8, u8); 6] = [(0, 0), (2, 1), (1, 0), (0, 1), (2, 0), (1, 1)];
/// Half a block's plan: its cells and the armour round them.
const BLOCK_HALF: Vec2 = Vec2::new(3.0 * CELL_PITCH * 0.5 + 0.7, CELL_PITCH + 0.7);

pub(super) fn build(b: &mut MeshBuilder) {
    hull(b, &HULL, &[0, 4, 8]);

    if b.coarse() {
        // Far off: the deck as one slab, the island and mast as one block, the
        // owner's square on the stern, and the cell blocks with the plinth between.
        b.paint(PLATING).pattern(pattern::DECK);
        b.cuboid(
            v3(-1.5, 0.0, (HULL_DECK + DECK) * 0.5),
            v3(118.0, 30.0, DECK - HULL_DECK),
        );
        b.paint(PLATING);
        b.frustum_open(
            ISLAND,
            v2(16.0, 6.0),
            v2(3.0, 2.4),
            22.0 - DECK,
            v2(-1.0, 0.0),
        );
        team_panel(b, v3(-52.0, 0.0, DECK), v2(6.0, 12.0));
        // The cell blocks and the island's plinth between them, as one box.
        b.paint(PLATING);
        let (fore, aft) = (
            CELL_BLOCKS[0].x + BLOCK_HALF.x,
            CELL_BLOCKS[1].x - BLOCK_HALF.x,
        );
        b.cuboid(
            v3((fore + aft) * 0.5, ISLAND.y, (DECK + CELL_DECK) * 0.5),
            v3(fore - aft, BLOCK_HALF.y * 2.0, CELL_DECK - DECK),
        );
        return;
    }

    flight_deck(b);
    markings(b);
    hatch(b);
    lifts(b);
    for c in CELL_BLOCKS {
        cell_block(b, c);
    }
    island(b);
    fleet_radar(b);
    laser_sponsons(b);
    below_the_waterline(b);
    if b.fine() {
        furniture(b);
    }
}

/// The flight deck: a dark structure lofted along the hull from its deck edge out
/// to the sponson lips, and the white deck plates on top of it.
fn flight_deck(b: &mut MeshBuilder) {
    let structure: Vec<Vec<Vec3>> = DECK_PLAN
        .iter()
        .map(|&(x, sh)| {
            let (dz, hh) = deck_at(&HULL, x);
            let hh = hh.min(sh - 1.5);
            vec![
                v3(x, -hh, dz),
                v3(x, -sh, LIP),
                v3(x, -sh, SLAB),
                v3(x, sh, SLAB),
                v3(x, sh, LIP),
                v3(x, hh, dz),
            ]
        })
        .collect();
    b.paint(ACCENT);
    b.loft(&structure, true, true);
    let plates: Vec<Vec<Vec3>> = DECK_PLAN
        .iter()
        .map(|&(x, sh)| {
            vec![
                v3(x, -sh, SLAB),
                v3(x, -sh, DECK),
                v3(x, sh, DECK),
                v3(x, sh, SLAB),
            ]
        })
        .collect();
    b.paint(PLATING).pattern(pattern::DECK);
    b.loft(&plates, true, true);
}

/// Painted on the deck: the angled landing strip from the port quarter, the launch
/// lane over the bow with its catapult tracks, the owner's band down both deck
/// edges and the flux conduit outboard of it.
fn markings(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::ROADWAY);
    b.extrude_z(
        &[[-59.5, -2.0], [14.0, -7.0], [14.0, 5.0], [-59.5, 10.0]],
        DECK,
        DECK + 0.06,
    );
    b.plate(v3(34.0, 1.0, DECK), v2(36.0, 9.0), 0.06, 0.02);
    b.paint(PLATING_DARK);
    for y in [-1.5, 3.5] {
        b.plate(v3(34.0, y, DECK), v2(32.0, 0.35), 0.1, 0.03);
    }
    for side in [-1.0, 1.0] {
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        for (x0, x1) in [(-50.0, -22.0), (-22.0, 4.0), (4.0, 28.0)] {
            b.plate(
                v3((x0 + x1) * 0.5, side * 13.9, DECK),
                v2(x1 - x0 - 0.4, 1.2),
                0.06,
                0.02,
            );
        }
        b.paint(ACCENT).pattern(pattern::FLUX);
        for (x0, x1) in [(-48.0, -12.0), (-12.0, 24.0)] {
            b.plate(
                v3((x0 + x1) * 0.5, side * 14.85, DECK),
                v2(x1 - x0 - 0.4, 0.4),
                0.08,
                0.02,
            );
        }
    }
    // The owner's square on the stern, where the landing strip begins.
    team_panel(b, v3(-54.0, 4.0, DECK + 0.06), v2(4.0, 8.0));
}

/// The hangar hatch, as on the Roost: a black hazard rim, four thick leaves, and
/// the plasma seams between them where they part.
fn hatch(b: &mut MeshBuilder) {
    let (cx, cy, s) = (HATCH.x, HATCH.y, HATCH_HALF);
    b.paint(ACCENT).pattern(pattern::HAZARD);
    for (lo, hi) in [
        (v2(-s - 1.2, s), v2(s + 1.2, s + 1.2)),
        (v2(-s - 1.2, -s - 1.2), v2(s + 1.2, -s)),
        (v2(s, -s), v2(s + 1.2, s)),
        (v2(-s - 1.2, -s), v2(-s, s)),
    ] {
        b.block(
            v3(cx + lo.x, cy + lo.y, DECK),
            v3(cx + hi.x, cy + hi.y, DECK + 0.16),
        );
    }
    b.paint(GLOW);
    b.plate(v3(cx, cy, DECK), v2(s * 2.0, 0.3), 0.1, 0.02);
    b.plate(v3(cx, cy, DECK), v2(0.3, s * 2.0), 0.1, 0.02);
    b.paint(PLATING).pattern(pattern::DECK);
    let leaf = s - 0.2;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let c = v3(
            cx + sx * (0.2 + leaf * 0.5),
            cy + sy * (0.2 + leaf * 0.5),
            DECK,
        );
        b.plate(c, v2(leaf - 0.1, leaf - 0.1), 0.12, 0.04);
    }
    if b.fine() {
        // Landing marks on the leaves, and the lamp on the rim's forward bar.
        b.paint(GLOW_AMBER);
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            b.cuboid(
                v3(cx + sx * 4.2, cy + sy * 4.2, DECK + 0.14),
                v3(0.9, 0.9, 0.06),
            );
        }
        b.paint(GLOW_RED);
        b.cuboid(v3(cx + s + 0.6, cy, DECK + 0.18), v3(0.5, 1.6, 0.06));
    }
}

/// Deck-edge lifts: a dark tray let into the deck with the lift's plate in it, a
/// hazard stripe inboard and plasma strips along its sides.
fn lifts(b: &mut MeshBuilder) {
    for (i, c) in LIFTS.iter().enumerate() {
        let side = if c.y > 0.0 { 1.0 } else { -1.0 };
        let (w, l) = (LIFT_SIZE.x, LIFT_SIZE.y);
        b.paint(ACCENT);
        b.plate(v3(c.x, c.y, DECK), v2(w, l), 0.1, 0.03);
        b.paint(PLATING_DARK).pattern(pattern::DECK);
        b.plate(v3(c.x, c.y, DECK + 0.1), v2(w - 0.9, l - 0.9), 0.06, 0.02);
        b.paint(ACCENT).pattern(pattern::HAZARD);
        b.plate(
            v3(c.x, c.y - side * (l * 0.5 + 0.35), DECK),
            v2(w + 0.4, 0.7),
            0.14,
            0.03,
        );
        if b.fine() {
            for dx in [-(w * 0.5 - 0.2), w * 0.5 - 0.2] {
                glow_strip(b, v3(c.x + dx, c.y, DECK + 0.1), v2(0.22, l - 1.2), GLOW);
            }
            // A number on the lift and the owner's colour on the other.
            if i == 0 {
                team_panel(b, v3(c.x, c.y, DECK + 0.16), v2(1.6, 1.6));
            }
        }
    }
}

/// The island: a dark plinth, a faceted white body drawn in as it rises, the glass
/// bridge band, a white cap under a dark coping; the lamp pole aft; lit sensor panels,
/// the interceptor laser's lens, and flux running up from the deck.
fn island(b: &mut MeshBuilder) {
    let plan = chamfered_rect(ISLAND_HALF, 1.3);
    let at = v3(ISLAND.x, ISLAND.y, 0.0);
    b.at(at, |b| {
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[Section::new(DECK, 1.03), Section::new(DECK + 0.5, 1.03)],
        );
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::new(DECK + 0.5, 1.0),
                Section::scaled(13.4, 0.94, 0.9),
            ],
        );
        b.paint(GLASS);
        b.loft_z(
            &plan,
            &[
                Section::scaled(13.4, 0.94, 0.9),
                Section::scaled(14.5, 0.9, 0.84),
            ],
        );
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::scaled(14.5, 0.92, 0.86),
                Section::scaled(15.2, 0.86, 0.8),
            ],
        );
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[
                Section::scaled(15.2, 0.87, 0.81),
                Section::scaled(ROOF, 0.87, 0.81),
            ],
        );
    });
    team_panel(b, v3(ISLAND.x + 3.4, ISLAND.y - 0.5, ROOF), v2(2.6, 2.6));

    // The lamp pole aft on the roof, a yard across it, the red lamp on top.
    b.paint(ACCENT);
    b.cylinder_between(
        LAMP_POLE,
        v3(LAMP_POLE.x, LAMP_POLE.y, LAMP_Z),
        0.16,
        0.1,
        b.sides(6),
    );
    beacon(b, v3(LAMP_POLE.x, LAMP_POLE.y, LAMP_Z));
    b.paint(METAL);
    b.beam(
        v3(LAMP_POLE.x, LAMP_POLE.y - 1.6, 20.6),
        v3(LAMP_POLE.x, LAMP_POLE.y + 1.6, 20.6),
        v2(0.12, 0.12),
        v2(0.12, 0.12),
    );

    // The island's interceptor laser on a post on the forward roof.
    pd_laser(b, LASERS[0], 0.6, Some(ROOF));

    // Flux: a conduit from the deck up the island's inboard face to the roof, then
    // across to the mast's foot and the lens.
    b.paint(ACCENT).pattern(pattern::FLUX);
    let inboard = |z: f32| {
        // The inboard (+y) face, following the body's tumblehome.
        let s = 1.0 - 0.1 * ((z - DECK - 0.5) / (13.4 - DECK - 0.5)).clamp(0.0, 1.0);
        ISLAND.y + ISLAND_HALF.y * s
    };
    let edge = ISLAND.y + ISLAND_HALF.y * 0.81;
    b.beam(
        v3(ISLAND.x - 3.0, inboard(DECK + 0.5) + 0.12, DECK + 0.5),
        v3(ISLAND.x - 3.0, inboard(13.4) + 0.12, 13.4),
        v2(0.7, 0.2),
        v2(0.7, 0.2),
    );
    b.beam(
        v3(ISLAND.x - 3.0, inboard(13.4) + 0.12, 13.4),
        v3(ISLAND.x - 3.0, edge + 0.12, ROOF + 0.1),
        v2(0.7, 0.2),
        v2(0.7, 0.2),
    );
    b.beam(
        v3(ISLAND.x - 3.0, edge - 0.2, ROOF + 0.1),
        v3(ISLAND.x + 3.6, edge - 0.2, ROOF + 0.1),
        v2(0.4, 0.2),
        v2(0.4, 0.2),
    );
    // And on the deck: from the island's foot aft and round to the hatch's rim, and
    // forward to the SAM battery.
    let rim_x = HATCH.x + HATCH_HALF + 1.4;
    b.block(
        v3(rim_x, ISLAND.y - 0.35, DECK),
        v3(ISLAND.x - ISLAND_HALF.x - 0.2, ISLAND.y + 0.35, DECK + 0.3),
    );
    b.block(
        v3(rim_x, ISLAND.y - 0.35, DECK),
        v3(rim_x + 0.7, HATCH.y - HATCH_HALF - 1.4, DECK + 0.3),
    );

    if !b.fine() {
        return;
    }
    // Lit sensor panels flush in the body's faces, following their slope.
    b.paint(GLOW);
    let face = |dir: Vec2, z: f32| -> Vec3 {
        let t = ((z - DECK - 0.5) / (13.4 - DECK - 0.5)).clamp(0.0, 1.0);
        let s = Vec2::new(1.0 - 0.06 * t, 1.0 - 0.1 * t);
        let p = dir * ISLAND_HALF * s;
        v3(ISLAND.x + p.x, ISLAND.y + p.y, z)
    };
    b.beam(
        face(v2(1.0, 0.0), 10.2) + v3(0.06, -1.2, 0.0),
        face(v2(1.0, 0.0), 12.4) + v3(0.06, -1.2, 0.0),
        v2(1.4, 0.1),
        v2(1.4, 0.1),
    );
    b.beam(
        face(v2(1.0, 0.0), 10.2) + v3(0.06, 1.2, 0.0),
        face(v2(1.0, 0.0), 12.4) + v3(0.06, 1.2, 0.0),
        v2(1.4, 0.1),
        v2(1.4, 0.1),
    );
    for x in [-4.5, -1.5, 1.5] {
        b.beam(
            face(v2(0.0, -1.0), 10.6) + v3(x, -0.06, 0.0),
            face(v2(0.0, -1.0), 12.2) + v3(x, -0.06, 0.0),
            v2(2.0, 0.1),
            v2(2.0, 0.1),
        );
    }
    // Dark decoy launchers on the roof's aft corners, whips off the yard, a vent.
    b.paint(ACCENT);
    for y in [-1.7, 1.7] {
        b.block(
            v3(ISLAND.x - 5.4, ISLAND.y + y - 0.4, ROOF),
            v3(ISLAND.x - 4.4, ISLAND.y + y + 0.4, ROOF + 0.35),
        );
        b.paint(METAL);
        for i in 0..3 {
            let p = v3(ISLAND.x - 5.2 + i as f32 * 0.3, ISLAND.y + y, ROOF + 0.35);
            b.cylinder_between(p, p + v3(-0.15, 0.0, 0.3), 0.1, 0.1, 5);
        }
        b.paint(ACCENT);
    }
    whip(b, v3(LAMP_POLE.x, LAMP_POLE.y + 1.5, 20.65), 2.4, 0.05);
    whip(b, v3(LAMP_POLE.x, LAMP_POLE.y - 1.5, 20.65), 2.0, 0.08);
    vent(
        b,
        v3(ISLAND.x + 4.8, ISLAND.y + 1.3, ROOF),
        v2(1.8, 1.0),
        3,
        GLOW,
    );
    // ESM domes on the mast's shoulders.
    b.paint(PLATING);
    for y in [-1.3, 1.3] {
        b.spheroid(v3(MAST_X, ISLAND.y + y, 18.4), v3(0.3, 0.3, 0.26), 6, 2);
    }
}

/// A block of weapon 0's cells built onto the island's plinth at `c`: an armoured box
/// drawn in as it rises under a dark coaming, and on it six hatches, each hinged on its
/// outer edge along y with its knuckles, over the missile standing in the cell.
fn cell_block(b: &mut MeshBuilder, c: Vec2) {
    let plan = chamfered_rect(BLOCK_HALF, 0.8);
    b.at(c.extend(0.0), |b| {
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::new(DECK, 1.08),
                Section::new(CELL_DECK - 0.35, 1.0),
            ],
        );
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[
                Section::new(CELL_DECK - 0.35, 1.0),
                Section::new(CELL_DECK, 0.98),
            ],
        );
    });
    let grid = CellGrid {
        centre: c,
        deck: CELL_DECK,
        pitch: CELL_PITCH,
        half: CELL_HALF,
        nx: 3,
        ny: 2,
        hinge_y: true,
    };
    for m in b.cell_block(grid, &CELL_FIRE) {
        let side = (m.y - c.y).signum();
        let hinge = m.y + side * CELL_HALF;
        b.with_part(part::CELL_HATCH, |b| {
            b.paint(PLATING);
            b.cuboid(
                v3(m.x, m.y, CELL_DECK + 0.12),
                v3(CELL_HALF * 2.0, CELL_HALF * 2.0, 0.24),
            );
            if b.fine() {
                // Raised ribs across the lid, and the knuckles it swings on.
                b.paint(PLATING_DARK);
                for dx in [-0.55, 0.55] {
                    b.cuboid(
                        v3(m.x + dx, m.y, CELL_DECK + 0.3),
                        v3(0.2, CELL_HALF * 1.6, 0.12),
                    );
                }
                b.paint(METAL);
                for dx in [-0.65, 0.65] {
                    b.cylinder_between(
                        v3(m.x + dx - 0.22, hinge, CELL_DECK + 0.08),
                        v3(m.x + dx + 0.22, hinge, CELL_DECK + 0.08),
                        0.15,
                        0.15,
                        4,
                    );
                }
            }
            b.paint(ACCENT);
            b.decal(
                v3(m.x, m.y - side * 0.7, CELL_DECK + 0.25),
                v2(CELL_HALF * 1.6, 0.24),
            );
        });
        b.with_part(part::CELL_ROUND, |b| {
            let top = CELL_DECK - 0.25;
            // Only its head shows over the cell's rim from further off.
            let foot = if b.fine() { top - 5.6 } else { top - 2.0 };
            b.paint(PLATING);
            b.cylinder_between(
                v3(m.x, m.y, foot),
                v3(m.x, m.y, top - 1.0),
                0.45,
                0.45,
                b.sides(6),
            );
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(m.x, m.y, top - 1.0),
                v3(m.x, m.y, top),
                0.45,
                0.06,
                b.sides(6),
            );
        });
    }
}

/// Under the water: the bulbous bow, twin rudders, the two shrouded propulsors in
/// the tunnel stern, and the interceptor tubes' launcher in its roof.
fn below_the_waterline(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::HULL);
    b.spheroid(
        v3(55.5, 0.0, -2.4),
        v3(4.5, 1.8, 1.9),
        b.sides(12),
        if b.fine() { 5 } else { 3 },
    );
    // Propulsors: a shroud open at both ends round a hub, stators inside at full detail.
    let duct_sides = b.sides(12);
    let ring = |x: f32, y: f32, r: f32| -> Vec<Vec3> {
        ngon(duct_sides, r)
            .into_iter()
            .map(|[dy, dz]| v3(x, y + dy, -3.2 + dz))
            .collect()
    };
    for y in [-5.0, 5.0] {
        b.paint(PLATING_DARK);
        b.loft(
            &[
                ring(-49.5, y, 1.7),
                ring(-54.5, y, 1.45),
                ring(-54.5, y, 1.25),
                ring(-49.5, y, 1.5),
                ring(-49.5, y, 1.7),
            ],
            false,
            false,
        );
        b.paint(METAL);
        b.cylinder_between(
            v3(-48.5, y, -3.2),
            v3(-53.8, y, -3.2),
            0.55,
            0.3,
            b.sides(8),
        );
        if b.fine() {
            b.paint(PLATING_DARK);
            for (dy, dz) in [(0.0, 1.0), (0.87, -0.5), (-0.87, -0.5)] {
                b.beam(
                    v3(-52.0, y, -3.2),
                    v3(-52.0, y + dy * 1.35, -3.2 + dz * 1.35),
                    v2(0.08, 0.6),
                    v2(0.08, 0.6),
                );
            }
        }
        // A rudder behind each propulsor, hung from the skeg.
        b.paint(PLATING_DARK);
        b.beam(
            v3(-56.6, y, -1.6),
            v3(-56.9, y, -4.6),
            v2(0.22, 1.5),
            v2(0.14, 1.0),
        );
    }
    // Interceptor tubes: a dark launcher in the tunnel roof, two mouths facing aft.
    b.paint(ACCENT);
    b.block(v3(-40.0, -4.0, -2.7), v3(-34.0, 4.0, -1.5));
    for m in TUBES {
        b.paint(METAL);
        b.cylinder_between(m + Vec3::X * 0.5, m - Vec3::X * 0.15, 0.5, 0.5, b.sides(8));
        b.paint(PLATING_DARK);
        b.cylinder_between(
            m - Vec3::X * 0.15,
            m - Vec3::X * 0.2,
            0.38,
            0.38,
            b.sides(8),
        );
    }
}

/// The deck-edge missile-defence lasers: a dark platform hung out from the deck edge
/// on a strut, the red-banded head (`pd_laser`) standing on it.
fn laser_sponsons(b: &mut MeshBuilder) {
    for at in &LASERS[1..] {
        let side = at.y.signum();
        let inner = side * (at.y.abs() - 2.2);
        let outer = side * (at.y.abs() + 0.9);
        let (y0, y1) = (inner.min(outer), inner.max(outer));
        b.paint(PLATING_DARK);
        b.block(
            v3(at.x - 1.3, y0, DECK - 0.75),
            v3(at.x + 1.3, y1, DECK - 0.3),
        );
        if b.fine() {
            b.paint(ACCENT);
            b.beam(
                v3(at.x, side * (at.y.abs() - 2.0), LIP - 0.8),
                v3(at.x, side * (at.y.abs() + 0.2), DECK - 0.75),
                v2(0.3, 0.3),
                v2(0.25, 0.25),
            );
        }
        pd_laser(b, *at, 0.6, Some(DECK - 0.3));
    }
}

/// Full detail only: deck-edge nets, catapult blast deflectors, bollards, the
/// liferafts along the sponson sides, and cranes at the deck's corners.
fn furniture(b: &mut MeshBuilder) {
    // Safety nets slung out from the deck edge on struts.
    b.paint(ACCENT);
    for side in [-1.0, 1.0] {
        for x in [-46.0, -38.0, -10.0, 8.0, 26.0] {
            let sh = DECK_PLAN
                .windows(2)
                .find(|w| x <= w[1].0)
                .map(|w| {
                    let t = (x - w[0].0) / (w[1].0 - w[0].0);
                    w[0].1 + (w[1].1 - w[0].1) * t
                })
                .unwrap_or(15.0);
            b.beam(
                v3(x, side * (sh - 0.1), DECK - 0.1),
                v3(x, side * (sh + 1.8), DECK - 0.5),
                v2(0.14, 0.12),
                v2(0.1, 0.08),
            );
            b.beam(
                v3(x - 3.0, side * (sh + 1.8), DECK - 0.5),
                v3(x + 3.0, side * (sh + 1.8), DECK - 0.5),
                v2(0.08, 0.08),
                v2(0.08, 0.08),
            );
        }
    }
    // Jet blast deflectors behind the catapults' start, raised a little.
    b.paint(PLATING_DARK);
    for y in [-1.5, 3.5] {
        b.frustum(
            v3(17.5, y, DECK + 0.1),
            v2(1.2, 3.6),
            v2(0.3, 3.4),
            0.9,
            v2(-0.5, 0.0),
        );
    }
    // Bollards at the deck's corners, liferafts down the sponson lips.
    for (x, y) in [(-57.0, 12.0), (-57.0, -12.0), (50.0, 7.6), (50.0, -7.6)] {
        bollard(b, v3(x, y, DECK + 0.06), 0.55);
    }
    // Liferaft canisters racked on the sponsons' outer faces, under the deck edge.
    b.paint(PLATING);
    for x in [-40.0, -34.0, 2.0, 8.0, 22.0, 28.0] {
        for side in [-1.0, 1.0] {
            let sh = DECK_PLAN
                .windows(2)
                .find(|w| x <= w[1].0)
                .map(|w| w[0].1 + (w[1].1 - w[0].1) * (x - w[0].0) / (w[1].0 - w[0].0))
                .unwrap_or(15.0);
            let y = side * (sh + 0.3);
            b.cylinder_between(
                v3(x - 0.8, y, LIP + 0.22),
                v3(x + 0.8, y, LIP + 0.22),
                0.28,
                0.28,
                6,
            );
        }
    }
    // A crane on the starboard quarter, folded along the deck.
    b.paint(ACCENT);
    b.prism(v3(-52.0, -11.5, DECK + 0.06), 8, 0.6, 0.5, 1.2);
    b.paint(PLATING);
    b.beam(
        v3(-52.0, -11.5, DECK + 1.2),
        v3(-42.0, -13.0, DECK + 1.9),
        v2(0.5, 0.5),
        v2(0.3, 0.3),
    );
}

/// The fleet radar: a broad faceted mast off the island roof and a long double-faced
/// array turning on its head.
fn fleet_radar(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2(3.0, 2.3), 0.6),
        &[
            Section::new(ROOF, 1.0).shifted(MAST_X, ISLAND.y),
            Section::scaled(19.2, 0.5, 0.55).shifted(MAST_X, ISLAND.y),
            Section::scaled(RADAR.z, 0.3, 0.35).shifted(MAST_X, ISLAND.y),
        ],
    );
    let pivot = RADAR;
    b.set_spinner_pivot(pivot);
    b.with_part(part::SPINNER, |b| {
        b.paint(ACCENT);
        b.prism(pivot, b.sides(8), 0.8, 0.7, 0.6);
        b.paint(PLATING);
        b.beam(
            pivot + v3(0.0, -6.0, 1.9),
            pivot + v3(0.0, 6.0, 1.9),
            v2(0.7, 2.6),
            v2(0.7, 2.6),
        );
        b.paint(PLATING_DARK);
        for s in [-1.0, 1.0] {
            b.beam(
                pivot + v3(s * 0.38, -5.6, 1.9),
                pivot + v3(s * 0.38, 5.6, 1.9),
                v2(0.06, 2.2),
                v2(0.06, 2.2),
            );
        }
        if b.mid() {
            b.paint(ACCENT);
            b.beam(
                pivot + v3(0.0, -0.8, 0.6),
                pivot + v3(0.0, 0.8, 0.6),
                v2(0.9, 0.6),
                v2(0.9, 0.6),
            );
        }
    });
}
