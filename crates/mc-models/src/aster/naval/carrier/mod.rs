//! The Atoll (`carrier`): the fleet's carrier of air wings, and its long-range air
//! defence. A broad flat body over a shallow ship's hull, its plan closing in over
//! bow and stern in hard facets, its flanks angled in and out, its decks stepping down
//! toward both ends. The runway runs inside it: an enclosed flight tunnel down the
//! middle from a landing mouth in the stern to a launch mouth in the bow face, glimpsed
//! from above through skylights, a long slit and gratings between the deck blocks that
//! roof it. Its floor carries the lit lane and its walls ribs and lit slots.
//! The command plateau over the stern quarter is edged in bronze and zig-zags in steps down
//! both sides; on it stands a terraced citadel with the fleet radar, the bridge aft. Twelve SAM cells
//! lie flush in the deck either side of the tunnel, and four interceptor lasers stand on
//! the citadel and the deck.
//!
//! Large and medium forms are mesh, and each fitting is its own piece. The fine detail
//! is drawn by the surface (`gpu_consts::carrier_deck`, `flight_lane`): the uneven hull
//! plating, the tunnel's ribs and slot lights, circuit bays, the bronze trim, the lane,
//! two small vents. Nothing lit is modelled.
//!
//! Origin at the waterline; x forward, y to port, z up; metres at the data's radius 60.

use super::*;
use glam::Vec2;

mod citadel;
mod deck;
mod flanks;
mod kit;

/// Where aircraft meet the ship, for the sim.
pub(crate) struct FlightDeck {
    /// Where a landing aircraft touches down, inside the stern mouth.
    pub(crate) touchdown: Vec3,
    /// The way it flies on its final approach (a unit vector: forward, falling gently into
    /// the stern mouth).
    pub(crate) approach: Vec3,
    /// Where a launched aircraft leaves the ship, at the launch mouth in the bow face.
    pub(crate) launch: Vec3,
    /// The way it leaves (a unit vector: forward, climbing).
    pub(crate) launch_dir: Vec3,
    /// The tunnel's floor, where aircraft roll from touchdown to launch.
    pub(crate) deck: f32,
}

/// The ends of the body.
const AFT: f32 = -58.0;
const FORE: f32 = 58.0;
/// The body's half beam at its deck edge, stern first: it closes in over both ends in
/// hard facets, more sharply at the bow.
const PLAN: [(f32, f32); 7] = [
    (AFT, 12.0),
    (-50.0, 15.6),
    (-42.0, 17.0),
    (30.0, 17.0),
    (42.0, 16.0),
    (52.0, 13.0),
    (FORE, 10.0),
];
/// The body stands on the hull from `FOOT`; its main deck, and the lower decks it steps
/// down to over the stern and the bow.
const FOOT: f32 = 1.0;
const DECK: f32 = 9.0;
const STERN_DECK: f32 = 7.6;
const BOW_DECK: f32 = 7.9;
/// Where the main deck steps down to them.
const STERN_STEP: f32 = -46.0;
const BOW_STEP: f32 = 52.0;
/// The flight tunnel: floor, ceiling and half width, and how far its mouths flare.
const FLOOR: f32 = 1.5;
const CEILING: f32 = 7.2;
const TUNNEL: f32 = 5.5;
const FLARE: f32 = 1.4;
/// The command plateau's fore end and its deck.
const PLATEAU_FORE: f32 = -14.0;
const PLATEAU: f32 = 10.8;

/// The flight deck: touchdown a few lengths inside the stern mouth, launch out of the bow.
pub(crate) const FLIGHT_DECK: FlightDeck = FlightDeck {
    touchdown: Vec3::new(-50.0, 0.0, FLOOR),
    // 4 degrees down, level into the mouth.
    approach: Vec3::new(0.997_564, 0.0, -0.069_756),
    launch: Vec3::new(FORE, 0.0, FLOOR),
    // 8 degrees up.
    launch_dir: Vec3::new(0.990_268, 0.0, 0.139_173),
    deck: FLOOR,
};

/// The hull under the body, stern first: the old Atoll's underwater body (keel, chines,
/// tunnel stern) widened to carry it, its deck edge under the body's foot.
const HULL: [Station; 9] = [
    station(AFT, -1.0, [-2.6, 9.0], [0.4, 11.2], [FOOT, 11.6]),
    station(-50.0, -1.4, [-3.0, 11.6], [0.4, 14.8], [FOOT, 15.2]),
    station(-36.0, -2.6, [-3.6, 12.4], [0.4, 16.2], [FOOT, 16.6]),
    station(-22.0, -7.4, [-5.4, 12.4], [0.4, 16.2], [FOOT, 16.6]),
    station(0.0, -8.0, [-5.6, 12.4], [0.4, 16.2], [FOOT, 16.6]),
    station(22.0, -7.6, [-5.2, 12.0], [0.4, 16.2], [FOOT, 16.6]),
    station(40.0, -6.0, [-4.0, 9.5], [0.5, 15.4], [FOOT, 15.8]),
    station(52.0, -3.6, [-2.2, 5.0], [0.6, 12.2], [FOOT, 12.6]),
    station(FORE, 0.4, [0.2, 3.0], [0.8, 9.2], [FOOT, 9.6]),
];

/// Weapon 0's two blocks of cells, flush in the deck either side of the tunnel.
const CELLS: [Vec2; 2] = [Vec2::new(14.0, 11.5), Vec2::new(14.0, -11.5)];
/// The interceptor lasers (`anti_missile_mounts`): the citadel's, the plateau's fore
/// corners, and the starboard ramp.
const LASERS: [Vec3; 4] = [
    Vec3::new(-45.6, 0.0, 19.5),
    Vec3::new(-36.0, -14.4, 11.7),
    Vec3::new(-36.0, 14.4, 11.7),
    Vec3::new(40.0, -13.6, 11.5),
];

pub(in crate::aster::naval) fn build(b: &mut MeshBuilder) {
    b.with_facets(|b| {
        hull(b, &HULL, &[0, 4, 8]);
        if b.coarse() {
            coarse(b);
            return;
        }
        if b.fine() {
            // Under the water: only close in does any of it show.
            below_the_waterline(b);
        }
        for side in [-1.0, 1.0] {
            body(b, side);
        }
        tunnel(b);
        roof(b);
        deck::build(b);
        flanks::build(b);
        citadel::build(b);
        for c in CELLS {
            kit::cell_battery(b, c, DECK);
        }
        for (k, at) in LASERS.iter().enumerate() {
            let foot = match k {
                0 => citadel::HOUSE_ROOF,
                1 | 2 => PLATEAU,
                _ => deck::BLOCK_TOP,
            };
            pd_laser(b, *at, 0.6, Some(foot));
        }
    });
}

/// Far off: the hull, the body as one box, the plateau and the citadel.
fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.cuboid_open(
        v3(-1.0, 0.0, (FOOT + DECK) * 0.5),
        v3(FORE - AFT - 6.0, 31.0, DECK - FOOT),
    );
    b.cuboid_open(
        v3((AFT + PLATEAU_FORE) * 0.5, 0.0, (DECK + PLATEAU) * 0.5),
        v3(PLATEAU_FORE - AFT - 6.0, 28.0, PLATEAU - DECK),
    );
    // The citadel and its mast, drawn in to the radar's height.
    b.frustum_open(
        v3(-40.0, 0.0, 11.0),
        v2(20.0, 14.0),
        v2(6.0, 4.0),
        10.5,
        v2(2.0, 0.0),
    );
    for c in CELLS {
        kit::cell_battery(b, c, DECK);
    }
    team_panel(b, v3(40.0, 0.0, DECK), v2(4.0, 20.0));
}

/// The tunnel wall's half width at `x`: flared out at both mouths.
fn inner(x: f32) -> f32 {
    let near = (x - AFT).min(FORE - x);
    TUNNEL + FLARE * (1.0 - near / 5.0).clamp(0.0, 1.0)
}

/// The body's half beam at its deck edge at `x` (`PLAN`).
fn outer(x: f32) -> f32 {
    let i = PLAN
        .windows(2)
        .position(|w| x <= w[1].0)
        .unwrap_or(PLAN.len() - 2);
    let (a, c) = (PLAN[i], PLAN[i + 1]);
    let t = ((x - a.0) / (c.0 - a.0)).clamp(0.0, 1.0);
    a.1 + (c.1 - a.1) * t
}

/// The deck's height at `x`: the main deck, stepping down over the stern and the bow.
fn deck_z(x: f32) -> f32 {
    if x < STERN_STEP {
        STERN_DECK
    } else if x > BOW_STEP {
        BOW_DECK
    } else {
        DECK
    }
}

/// One side of the body: a plumb tunnel wall inside, the deck with its edge cut in a hard
/// chamfer, and a flank that angles in under the deck edge, out at a knuckle and in again
/// to the hull. Its ends are raked and its decks step down over stern and bow.
fn body(b: &mut MeshBuilder, side: f32) {
    let mut xs: Vec<(f32, f32)> = vec![(AFT, STERN_DECK - 0.4)];
    for x in [AFT + 5.0, -50.0, STERN_STEP - 0.01] {
        xs.push((x, STERN_DECK));
    }
    for x in [STERN_STEP + 0.8, -42.0, 30.0, 42.0, BOW_STEP - 0.8] {
        xs.push((x, DECK));
    }
    for x in [BOW_STEP + 0.01, FORE - 5.0] {
        xs.push((x, BOW_DECK));
    }
    xs.push((FORE, BOW_DECK - 0.6));
    let rings: Vec<Vec<Vec3>> = xs
        .iter()
        .map(|&(x, top)| {
            let (yi, yo) = (inner(x), outer(x));
            [
                (yi, FOOT),
                (yi, top - 0.3),
                (yi + 0.3, top),
                (yo - 1.0, top),
                (yo, top - 0.9),
                (yo - 0.3, 6.2),
                (yo + 0.5, 5.0),
                (yo + 0.5, 3.4),
                (yo - 0.5, FOOT),
            ]
            .iter()
            .map(|&(y, z)| {
                // The end faces are raked: the transom falls back toward the deck, the bow
                // leans out over the water.
                let up = (z - FOOT) / (top - FOOT);
                let rake = if x <= AFT {
                    2.0 * up
                } else if x >= FORE {
                    1.6 * up
                } else {
                    0.0
                };
                v3(x + rake, side * y, z)
            })
            .collect()
        })
        .collect();
    b.paint(PLATING).pattern(pattern::HULL_PLATE);
    b.loft(&rings, true, true);
}

/// The flight tunnel inside: its dark floor with the rails and the lit lane, its ribbed
/// walls with their lit slots and a shelf at half height, its dark ceiling; a portal round
/// each mouth.
fn tunnel(b: &mut MeshBuilder) {
    b.paint(ACCENT).pattern(pattern::GENERIC);
    b.block(
        v3(AFT, -TUNNEL - FLARE, FLOOR - 0.5),
        v3(FORE, TUNNEL + FLARE, FLOOR),
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        v3(AFT + 2.0, -TUNNEL, CEILING - 0.2),
        v3(FORE - 2.0, TUNNEL, CEILING + 0.2),
    );
    b.paint(ACCENT).pattern(pattern::TRENCH);
    for side in [-1.0, 1.0] {
        let y0 = side * (TUNNEL - 0.12);
        b.block(
            v3(AFT + 5.0, y0.min(side * TUNNEL), FLOOR),
            v3(FORE - 5.0, y0.max(side * TUNNEL), CEILING - 0.2),
        );
    }
    // The lane: chevrons and running lamps the whole way, the touchdown bars, the owner's
    // colour at the threshold.
    b.paint(PLATING).pattern(pattern::FLIGHT_LANE);
    b.plate(
        v3(0.0, 0.0, FLIGHT_DECK.deck),
        v2(FORE - AFT - 1.0, TUNNEL * 1.2),
        0.05,
        0.02,
    );
    let td = FLIGHT_DECK.touchdown;
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    let across = v2(-FLIGHT_DECK.approach.y, FLIGHT_DECK.approach.x).normalize_or(Vec2::Y);
    for k in [-1.0, 1.0] {
        let c = td.truncate() + across * k * TUNNEL * 0.4;
        b.plate(c.extend(FLIGHT_DECK.deck + 0.05), v2(2.6, 0.8), 0.04, 0.01);
    }
    team_panel(b, v3(AFT + 2.5, 0.0, FLOOR + 0.05), v2(2.4, TUNNEL * 1.2));
    rails(b);
    if b.mid() {
        ledges(b);
    }
    for (x, out) in [(AFT, -1.0), (FORE, 1.0)] {
        portal(b, x, out);
    }
}

/// Two catapult rails down the tunnel's floor from the touchdown to the bow mouth, tied
/// across every few metres, rising a little toward the mouth as `launch_dir` lifts.
fn rails(b: &mut MeshBuilder) {
    let (x0, x1) = (FLIGHT_DECK.touchdown.x + 6.0, FLIGHT_DECK.launch.x - 1.0);
    let lift = FLIGHT_DECK.launch_dir.z / FLIGHT_DECK.launch_dir.x.max(0.1);
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    for y in [-2.4, 2.4] {
        b.beam(
            v3(x0, y, FLOOR + 0.1),
            v3(x1, y, FLOOR + 0.1 + lift * 0.3),
            v2(0.7, 0.3),
            v2(0.7, 0.3),
        );
    }
    if !b.fine() {
        return;
    }
    let mut x = x0 + 1.5;
    while x < x1 - 1.0 {
        b.block(v3(x - 0.3, -3.8, FLOOR), v3(x + 0.3, 3.8, FLOOR + 0.15));
        x += 4.0;
    }
}

/// A shelf along each tunnel wall at half its height: the tunnel reads two storeys deep.
fn ledges(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    for side in [-1.0, 1.0] {
        let (y0, y1) = (side * (TUNNEL - 0.9), side * TUNNEL);
        b.block(
            v3(AFT + 6.0, y0.min(y1), 4.4),
            v3(FORE - 6.0, y0.max(y1), 4.9),
        );
    }
}

/// The portal round a mouth at `x` (`out` the way out, -1 the stern, +1 the bow): dark
/// jambs raked back from the face, and a heavy lintel over the opening.
fn portal(b: &mut MeshBuilder, x: f32, out: f32) {
    let half = TUNNEL + FLARE;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for side in [-1.0, 1.0] {
        b.beam(
            v3(x - out * 0.2, side * (half + 0.4), FLOOR - 0.3),
            v3(x - out * 0.2, side * (half + 0.1), CEILING + 0.4),
            v2(1.0, 1.1),
            v2(1.0, 0.7),
        );
    }
    b.block(
        v3(x - out * 0.2 - 0.5, -half - 0.6, CEILING),
        v3(x - out * 0.2 + 0.5, half + 0.6, CEILING + 0.6),
    );
}

/// The deck blocks roofing the tunnel between the two sides of the body, with gaps
/// between them through which the lane shows: skylights open to the tunnel, gratings of
/// bars across a gap, and a long slit down the middle of one block.
fn roof(b: &mut MeshBuilder) {
    // Out to where the body's deck starts at the top of its chamfer, and no further: the
    // two tops would lie in one plane and fight.
    let w = TUNNEL + 0.3;
    // (x0, x1, how the block is cut): 0 whole, 1 split down the middle by a slit, 2 split
    // by a slit off to starboard.
    let blocks = [
        (AFT + 0.5, STERN_STEP - 6.0, 0),
        (-10.5, 1.0, 2),
        (5.0, 15.0, 0),
        (20.0, 34.0, 1),
        (38.0, 46.0, 0),
        (BOW_STEP, FORE - 1.0, 0),
    ];
    b.paint(PLATING).pattern(pattern::HULL_PLATE);
    for &(x0, x1, cut) in &blocks {
        let top = deck_z((x0 + x1) * 0.5);
        // The slit's sides, if any.
        let (s0, s1) = match cut {
            1 => (-1.0, 1.0),
            2 => (-3.4, -2.0),
            _ => (w, w),
        };
        b.block(v3(x0, -w, CEILING), v3(x1, s0, top));
        if s1 < w {
            b.block(v3(x0, s1, CEILING), v3(x1, w, top));
        }
    }
    // Two openings are only part of the roof's width: plates fill the rest.
    for (x0, x1, y0, y1) in [(15.0, 20.0, 1.5, w), (PLATEAU_FORE, -10.5, -w, -2.5)] {
        b.block(v3(x0, y0, CEILING), v3(x1, y1, deck_z((x0 + x1) * 0.5)));
    }
    // The gaps between the blocks: open skylights, and gratings of bars.
    let gaps = [
        (PLATEAU_FORE, -10.5, false),
        (1.0, 5.0, true),
        (15.0, 20.0, false),
        (34.0, 38.0, true),
        (46.0, BOW_STEP, false),
    ];
    if !b.fine() {
        return;
    }
    for (x0, x1, grating) in gaps {
        let top = deck_z((x0 + x1) * 0.5);
        // A dark coaming round the opening's edge.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for y in [-w, w - 0.4] {
            b.block(v3(x0, y, top - 0.6), v3(x1, y + 0.4, top + 0.15));
        }
        if grating {
            b.paint(PLATING_DARK).pattern(pattern::PLAIN);
            let n = ((x1 - x0) / 0.8).floor().max(1.0) as usize;
            for i in 0..n {
                let x = x0 + (x1 - x0) * (i as f32 + 0.5) / n as f32;
                b.block(v3(x - 0.12, -w, top - 0.35), v3(x + 0.12, w, top - 0.1));
            }
        }
    }
}
/// Weapon 1: the interceptor tube mouths in the tunnel stern.
const TUBES: [Vec3; 2] = [Vec3::new(-40.0, -3.0, -2.0), Vec3::new(-40.0, 3.0, -2.0)];

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

#[cfg(test)]
mod tests {
    use super::FLIGHT_DECK as D;
    use crate::build_model;

    /// The lane: touchdown aft and launch at the bow on the trench's floor, the approach
    /// diving and the launch climbing, and the run between them clear 4 m up over its
    /// middle (the cross-beam and the plateau's tunnel stand higher).
    #[test]
    fn the_lane_is_clear_from_touchdown_to_launch() {
        const { assert!(D.touchdown.x < -45.0 && D.launch.x >= super::FORE - 0.5) };
        const { assert!(D.approach.x > 0.9 && D.approach.z < 0.0) };
        const { assert!(D.launch_dir.x > 0.9 && D.launch_dir.z > 0.0) };
        for p in [D.touchdown, D.launch] {
            assert_eq!(p.z, D.deck, "{p} off the floor");
            assert!(p.y.abs() < super::TUNNEL, "{p} off the lane");
        }
        let model = build_model("carrier").unwrap();
        let blocked = model.lods[0].vertices.iter().find(|v| {
            let [x, y, z] = v.pos;
            (D.touchdown.x..D.launch.x).contains(&x)
                && y.abs() < 1.5
                && (D.deck + 0.5..D.deck + 4.0).contains(&z)
        });
        assert!(
            blocked.is_none(),
            "{:?} stands in the lane",
            blocked.map(|v| v.pos)
        );
    }
}
