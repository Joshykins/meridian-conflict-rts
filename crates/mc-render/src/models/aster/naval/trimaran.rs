//! The Narwhal (`aster_t3_rail_trimaran`, mesh "rail_trimaran"): the tech 3 anti-ship
//! trimaran, a Zenith rail laid down the keel of a 160 m ship.
//!
//! Three hulls: a long, slender centre hull with a wave-piercing plumb stem and flared
//! topsides, and two slim outriggers well out either side, joined to it by a broad aft
//! wing deck and a swept forward cross-beam. The gun is the ship. It lies along the
//! centreline in a gun house of its own (weapon 0): a slotted carriage and two trunnion
//! towers over the waist, and in them the cradle and the Zenith's own barrel, carried at
//! [`GUN_SCALE`] (`zenith::barrel`). The house trains only a few degrees; the hull turns
//! to aim, and the barrel elevates, its breech swinging down into an open well cut
//! into the centre hull behind the towers. At rest the barrel runs out over the foredeck
//! on a crutch and past the stem like a tusk.
//! Aft on the wing deck, the stepped superstructure looks forward along the gun: bridge,
//! mast and search radar, two uptakes. Nothing on the gun is lit (docs/STYLE.md, rail guns
//! are hardware); the only lights are a ship's own, as on the Leviathan.
//! The waterline is z = 0; the keel goes to -9.

use glam::{Affine3A, Vec2};

use super::*;
use crate::models::aster::zenith;
use crate::models::TurretRail;

// ---- the gun -----------------------------------------------------------------

/// The Zenith's barrel at this scale.
const GUN_SCALE: f32 = 0.8;
/// The trunnion (weapon 0's `pivot` in naval.ron); the muzzle is `GUN.muzzle` ahead of it.
const TRUNNION: Vec3 = Vec3::new(-22.0, 0.0, 24.0);
/// Weapon 0's charge anchors (renderer heavy_rail_fx.rs), in the barrel frame.
pub(crate) const RAIL: TurretRail = zenith::rail(GUN_SCALE);
/// How far the cradle and barrel kick back on the trunnion slides when it fires.
const RECOIL: f32 = 1.2;
/// The cradle (barrel frame): rear face, nose, half width, half height. It swallows
/// the barrel from the breech block's front to where the drawn jacket starts.
const CRADLE_BACK: f32 = -16.0;
const CRADLE_NOSE: f32 = 14.0;
const CRADLE_HW: f32 = 5.0;
const CRADLE_HH: f32 = 4.4;
/// The carriage (model space): its slotted sills either side of the well, their inner
/// and outer y and top; the trunnion towers' inner and outer faces.
const SILL: [f32; 2] = [6.6, 11.4];
const SILL_X: [f32; 2] = [-37.0, -8.0];
const SILL_TOP: f32 = 13.2;
const TOWER_Y: [f32; 2] = [7.2, 10.6];
/// The breech well in the centre hull: from, to (x), half width, floor.
const WELL_X: [f32; 2] = [-48.0, -16.5];
const WELL_HW: f32 = 6.0;
const WELL_FLOOR: f32 = -2.0;
/// The barrel crutch on the foredeck: x, the top of its saddle, its posts' y.
const CRUTCH: [f32; 3] = [44.0, 20.4, 7.6];

// ---- the hulls ---------------------------------------------------------------

/// The centre hull, stern first: a long slender hull, flared topsides, plumb stem.
const MAIN: [Station; 12] = [
    station(-82.0, -2.5, [0.5, 6.5], [5.0, 9.0], [9.0, 10.0]),
    station(-74.0, -6.0, [-1.5, 7.6], [4.5, 10.6], [9.2, 11.6]),
    station(-58.0, -8.5, [-2.5, 8.2], [4.5, 11.4], [9.4, 12.4]),
    station(-36.0, -9.0, [-2.8, 8.4], [4.5, 11.6], [9.6, 12.6]),
    station(-12.0, -9.0, [-2.8, 8.2], [4.5, 11.4], [9.8, 12.4]),
    station(10.0, -8.6, [-2.6, 7.6], [4.6, 10.8], [10.1, 11.8]),
    station(30.0, -7.8, [-2.2, 6.4], [4.8, 9.4], [10.5, 10.4]),
    station(48.0, -6.8, [-1.8, 4.8], [5.0, 7.4], [10.9, 8.4]),
    station(62.0, -5.6, [-1.2, 3.0], [5.2, 5.0], [11.3, 5.8]),
    station(72.0, -4.4, [-0.6, 1.5], [5.4, 2.8], [11.6, 3.4]),
    station(78.0, -3.2, [0.0, 0.3], [5.6, 0.8], [11.8, 1.0]),
    station(80.0, -2.0, [0.2, 0.0], [5.8, 0.0], [12.0, 0.0]),
];
/// Stations kept at the coarse level.
const MAIN_COARSE: [usize; 3] = [0, 4, 11];
/// An outrigger, about its own centreline, and how far out that is.
const AMA: [Station; 7] = [
    station(-64.0, -1.5, [0.0, 2.4], [3.5, 3.0], [6.2, 3.0]),
    station(-50.0, -4.0, [-1.2, 3.2], [3.5, 3.6], [6.4, 3.6]),
    station(-20.0, -4.5, [-1.4, 3.4], [3.6, 3.8], [6.6, 3.8]),
    station(5.0, -4.2, [-1.2, 3.0], [3.8, 3.5], [6.9, 3.5]),
    station(20.0, -3.2, [-0.6, 2.0], [4.0, 2.6], [7.2, 2.6]),
    station(28.0, -2.0, [0.0, 0.8], [4.2, 1.2], [7.4, 1.2]),
    station(31.0, -0.8, [0.4, 0.0], [4.4, 0.0], [7.6, 0.0]),
];
const AMA_COARSE: [usize; 2] = [0, 6];
const AMA_Y: f32 = 27.0;
/// The aft wing deck's plan (one side, from the centre hull out), its underside and top.
const WING: [[f32; 2]; 6] = [
    [-40.0, 9.0],
    [-46.0, 24.0],
    [-50.0, 30.6],
    [-68.0, 30.6],
    [-74.0, 26.0],
    [-77.0, 9.0],
];
const WING_Z: [f32; 2] = [4.0, 8.6];
/// The swept forward cross-beam's plan (one side) and its underside and top.
const BEAM: [[f32; 2]; 4] = [[22.0, 9.0], [14.0, 30.0], [2.0, 30.0], [-6.0, 9.0]];
const BEAM_Z: [f32; 2] = [4.8, 7.6];

// ---- the superstructure ------------------------------------------------------

/// Its decks, lowest first: (front x, back x, half width, top z, rake of the face).
const DECKS: [[f32; 5]; 3] = [
    [-51.0, -76.0, 15.0, 14.0, 1.6],
    [-54.0, -73.0, 10.6, 19.0, 1.2],
    [-57.0, -70.0, 7.6, 23.0, 1.4],
];
/// The bridge glass band on the top deck and the brow over it.
const BRIDGE: [f32; 2] = [20.6, 22.2];
/// The mast's foot and top, where the masthead lamp goes; the radar array's height.
const MAST: Vec3 = Vec3::new(-64.0, 0.0, 23.0);
const MAST_TOP: f32 = 34.0;
const RADAR: f32 = 28.6;
/// The two uptakes on the lowest deck, outboard: x from, to, y, top.
const UPTAKE: [f32; 4] = [-73.5, -66.0, 11.5, 19.5];

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    centre_hull(b);
    b.mirror_y(|b| b.at(v3(0.0, AMA_Y, 0.0), |b| hull(b, &AMA, &AMA_COARSE)));
    cross_structure(b);
    superstructure(b);
    gun(b);
    lights(b);
    if b.fine() {
        deck_detail(b);
        underwater(b);
    }
}

// ---- the centre hull with its well -----------------------------------------------

/// The centre hull's station at `x`, between the table's.
fn main_at(x: f32) -> Station {
    let i = MAIN
        .windows(2)
        .position(|w| x <= w[1].x)
        .unwrap_or(MAIN.len() - 2);
    let (a, c) = (MAIN[i], MAIN[i + 1]);
    let t = ((x - a.x) / (c.x - a.x)).clamp(0.0, 1.0);
    let mix = |p: [f32; 2], q: [f32; 2]| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
    station(
        x,
        a.keel + (c.keel - a.keel) * t,
        mix(a.chine, c.chine),
        mix(a.knuckle, c.knuckle),
        mix(a.deck, c.deck),
    )
}

/// A cross-section of the centre hull: keel, up the port side, across the deck (or
/// down into the well and up out of it), down the starboard side.
fn main_ring(s: &Station, well: bool) -> Vec<Vec3> {
    let p = |y: f32, z: f32| v3(s.x, y, z);
    let mut ring = vec![
        p(0.0, s.keel),
        p(-s.chine[1], s.chine[0]),
        p(-s.knuckle[1], s.knuckle[0]),
        p(-s.deck[1], s.deck[0]),
    ];
    if well {
        ring.extend([
            p(-WELL_HW, s.deck[0]),
            p(-WELL_HW, WELL_FLOOR),
            p(WELL_HW, WELL_FLOOR),
            p(WELL_HW, s.deck[0]),
        ]);
    }
    ring.extend([
        p(s.deck[1], s.deck[0]),
        p(s.knuckle[1], s.knuckle[0]),
        p(s.chine[1], s.chine[0]),
    ]);
    ring
}

/// The centre hull in three lofts: aft of the well, the well's length (its ends cap
/// the well), forward of it.
fn centre_hull(b: &mut MeshBuilder) {
    let (w0, w1) = (WELL_X[0], WELL_X[1]);
    let aft: Vec<Station> = MAIN
        .iter()
        .copied()
        .filter(|s| s.x < w0)
        .chain([main_at(w0)])
        .collect();
    let waist: Vec<Station> = [main_at(w0)]
        .into_iter()
        .chain(MAIN.iter().copied().filter(|s| s.x > w0 && s.x < w1))
        .chain([main_at(w1)])
        .collect();
    let fore: Vec<Station> = [main_at(w1)]
        .into_iter()
        .chain(MAIN.iter().copied().filter(|s| s.x > w1))
        .collect();
    b.paint(PLATING).pattern(pattern::HULL);
    for (part, well) in [(&aft, false), (&waist, true), (&fore, false)] {
        let rings: Vec<Vec<Vec3>> = part.iter().map(|s| main_ring(s, well)).collect();
        b.loft(&rings, true, true);
    }
    // The well's lining: dark, with the slide rails the breech rides down.
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.block(
        v3(w0 + 0.2, -WELL_HW + 0.05, WELL_FLOOR),
        v3(w1 - 0.2, WELL_HW - 0.05, WELL_FLOOR + 0.3),
    );
    b.mirror_y(|b| {
        b.block(
            v3(w0 + 0.2, WELL_HW - 0.3, WELL_FLOOR),
            v3(w1 - 0.2, WELL_HW + 0.02, main_at(w1).deck[0] - 0.1),
        );
    });
    // A dark deck-edge lip, the whole length.
    rub_rail(b, &MAIN, -81.0, 78.5, 0.7);
    // The coaming round the well's mouth.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let (za, zc) = (main_at(w0).deck[0], main_at(w1).deck[0]);
    b.mirror_y(|b| {
        b.beam(
            v3(w0 - 0.6, WELL_HW + 0.5, za + 0.3),
            v3(w1 + 0.6, WELL_HW + 0.5, zc + 0.3),
            v2(1.0, 0.6),
            v2(1.0, 0.6),
        )
    });
    for (x, z) in [(w0 - 0.6, za), (w1 + 0.6, zc)] {
        b.beam(
            v3(x, -WELL_HW - 1.0, z + 0.3),
            v3(x, WELL_HW + 1.0, z + 0.3),
            v2(1.0, 0.6),
            v2(1.0, 0.6),
        );
    }
    // The wave-piercing stem: a dark cutwater blade up the plumb bow.
    b.extrude_y(
        &[
            [77.0, -3.4],
            [80.6, -2.0],
            [80.9, 4.0],
            [80.4, 11.8],
            [78.6, 11.6],
        ],
        -0.3,
        0.3,
    );
}

// ---- the wing deck and the forward beam ------------------------------------------

fn cross_structure(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        // The aft wing deck: light top, dark underside and leading edge.
        b.paint(PLATING).pattern(pattern::HULL);
        b.extrude_z(&WING, WING_Z[0] + 1.2, WING_Z[1]);
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        b.loft_z(
            &WING,
            &[
                Section::scaled(WING_Z[0], 0.985, 0.96).shifted(-0.8, 0.4),
                Section::new(WING_Z[0] + 1.2, 1.0),
            ],
        );
        // The swept forward beam.
        b.paint(PLATING).pattern(pattern::HULL);
        b.extrude_z(&BEAM, BEAM_Z[0] + 0.8, BEAM_Z[1]);
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        b.loft_z(
            &BEAM,
            &[
                Section::scaled(BEAM_Z[0], 0.97, 0.97).shifted(0.2, 0.3),
                Section::new(BEAM_Z[0] + 0.8, 1.0),
            ],
        );
        if !fine {
            return;
        }
        // Walkways laid on each, and a dark edge along the wing's leading edge.
        b.paint(PLATING).pattern(pattern::WALKWAY);
        b.extrude_z(
            &[[-44.0, 13.0], [-48.5, 26.0], [-52.0, 26.0], [-48.0, 13.0]],
            WING_Z[1],
            WING_Z[1] + 0.06,
        );
        b.extrude_z(
            &[[14.0, 12.0], [9.5, 26.0], [6.5, 26.0], [10.5, 12.0]],
            BEAM_Z[1],
            BEAM_Z[1] + 0.06,
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.beam(
            v3(-40.2, 9.5, WING_Z[1] - 0.3),
            v3(-49.8, 30.2, WING_Z[1] - 0.3),
            v2(0.6, 0.7),
            v2(0.6, 0.7),
        );
        b.beam(
            v3(21.6, 9.5, BEAM_Z[1] - 0.3),
            v3(13.8, 29.6, BEAM_Z[1] - 0.3),
            v2(0.6, 0.7),
            v2(0.6, 0.7),
        );
        team_panel(b, v3(-60.0, 23.5, WING_Z[1]), v2(10.0, 5.0));
    });
    // The outriggers' decks forward of the beam: a team panel and a dark lip.
    b.mirror_y(|b| {
        b.at(v3(0.0, AMA_Y, 0.0), |b| {
            rub_rail(b, &AMA, -63.0, 29.0, 0.45);
            team_panel(b, v3(22.0, 0.0, deck_at(&AMA, 22.0).0), v2(5.0, 2.4));
        })
    });
}

// ---- the superstructure --------------------------------------------------------

/// Deck `d` of [`DECKS`] as a raked block standing on `foot`.
fn deck_block(b: &mut MeshBuilder, d: [f32; 5], foot: f32) {
    let [front, back, half, top, rake] = d;
    let len = front - back;
    b.frustum_open(
        v3((front + back) * 0.5, 0.0, foot),
        v2(len, 2.0 * half),
        v2(len - rake, 2.0 * half - 0.8),
        top - foot,
        v2(-rake * 0.5, 0.0),
    );
}

fn superstructure(b: &mut MeshBuilder) {
    let fine = b.fine();
    let mut foot = WING_Z[1];
    for (i, d) in DECKS.iter().enumerate() {
        b.paint(PLATING);
        deck_block(b, *d, foot);
        // A dark roof on each deck, drawn in from its edges.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        let [front, back, half, top, rake] = *d;
        b.block(
            v3(back + 0.6, -half + 0.9, top - 0.02),
            v3(front - rake - 0.4, half - 0.9, top + 0.14),
        );
        foot = if i == 0 { top } else { d[3] };
    }
    // The bridge: a glass band round the top deck's face and sides, a dark brow over it.
    let [front, back, half, top, rake] = DECKS[2];
    let face = |z: f32| front - rake * (z - DECKS[1][3]) / (top - DECKS[1][3]);
    b.paint(GLASS);
    b.loft(
        &[
            vec![
                v3(face(BRIDGE[0]) + 0.08, -half - 0.05, BRIDGE[0]),
                v3(face(BRIDGE[0]) + 0.08, half + 0.05, BRIDGE[0]),
                v3(face(BRIDGE[1]) + 0.08, half + 0.05, BRIDGE[1]),
                v3(face(BRIDGE[1]) + 0.08, -half - 0.05, BRIDGE[1]),
            ],
            vec![
                v3(back + 6.0, -half - 0.05, BRIDGE[0]),
                v3(back + 6.0, half + 0.05, BRIDGE[0]),
                v3(back + 6.0, half + 0.05, BRIDGE[1]),
                v3(back + 6.0, -half - 0.05, BRIDGE[1]),
            ],
        ],
        false,
        false,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        v3(face(top) - 0.2, -half - 0.6, top - 0.5),
        v3(face(top) + 1.4, half + 0.6, top + 0.1),
    );
    // Bridge wings out over the lowest deck, where the sidelights go.
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.block(
            v3(face(BRIDGE[0]) - 3.4, half, BRIDGE[0] - 0.8),
            v3(face(BRIDGE[0]) - 0.6, half + 3.2, BRIDGE[0] - 0.2),
        )
    });
    team_panel(b, v3(back + 3.8, 0.0, top + 0.14), v2(4.0, 6.0));

    // The uptakes: raked stacks outboard on the lowest deck, dark caps.
    b.mirror_y(|b| {
        let [x0, x1, y, z] = UPTAKE;
        b.paint(PLATING);
        b.frustum_open(
            v3((x0 + x1) * 0.5, y, DECKS[0][3]),
            v2(x1 - x0, 4.2),
            v2(x1 - x0 - 2.4, 3.2),
            z - DECKS[0][3],
            v2(-1.4, 0.0),
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(x0 + 0.4 - 1.4 + 1.2, y - 1.4, z),
            v3(x1 - 0.4 - 1.4 - 1.2, y + 1.4, z + 0.5),
        );
    });

    // The mast: a raked tripod to a platform, the search radar, a pole to the lamp.
    b.paint(METAL);
    let platform = v3(MAST.x, 0.0, RADAR - 1.0);
    b.cylinder_between(MAST, platform, 0.6, 0.4, 6);
    b.mirror_y(|b| b.cylinder_between(v3(MAST.x - 3.2, 3.6, top), platform, 0.35, 0.3, 5));
    b.paint(PLATING);
    b.chamfered_box(platform + Vec3::Z * 0.4, v3(3.2, 3.2, 0.8), 0.6);
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    // The search radar: a flat slatted array across the mast, facing forward.
    b.block(
        v3(MAST.x + 0.2, -4.2, RADAR),
        v3(MAST.x + 0.8, 4.2, RADAR + 2.2),
    );
    if fine {
        b.paint(METAL);
        for k in 0..5 {
            let z = RADAR + 0.25 + 0.42 * k as f32;
            b.block(v3(MAST.x + 0.8, -4.0, z), v3(MAST.x + 1.0, 4.0, z + 0.12));
        }
    }
    b.paint(METAL);
    b.cylinder_between(
        v3(MAST.x, 0.0, RADAR + 2.2),
        v3(MAST.x, 0.0, MAST_TOP - 0.4),
        0.22,
        0.14,
        5,
    );
    if fine {
        // A yard across the pole.
        b.beam(
            v3(MAST.x, -3.0, RADAR + 3.6),
            v3(MAST.x, 3.0, RADAR + 3.6),
            v2(0.2, 0.2),
            v2(0.2, 0.2),
        );
        antenna_unlit(b, v3(MAST.x - 1.0, 1.6, top + 0.1), 4.0, 0.1);
        antenna_unlit(b, v3(MAST.x - 1.0, -1.6, top + 0.1), 4.0, -0.1);
    }
}

// ---- the gun -----------------------------------------------------------------

/// Weapon 0's house: the slotted carriage and the trunnion towers turn (a few degrees);
/// the cradle and the Zenith's barrel inside it elevate and kick back.
fn gun(b: &mut MeshBuilder) {
    b.with_house(0, TRUNNION, RECOIL, |b| {
        carriage(b);
        b.with_recoil(|b| {
            b.at(TRUNNION, cradle);
            b.with(
                Affine3A::from_translation(TRUNNION) * Affine3A::from_scale(Vec3::splat(GUN_SCALE)),
                zenith::barrel,
            );
        });
    });
    // The crutch the barrel lies in at rest, on the foredeck: two posts and a saddle.
    let [x, saddle, y] = CRUTCH;
    let deck = main_at(x).deck[0];
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.frustum_open(
            v3(x, y, deck),
            v2(3.6, 2.2),
            v2(2.2, 1.4),
            saddle - deck - 0.6,
            Vec2::ZERO,
        )
    });
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        v3(x - 1.2, -y - 1.0, saddle - 0.9),
        v3(x + 1.2, y + 1.0, saddle),
    );
    // The power trunk from the carriage forward under the barrel, armoured and low.
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    let (za, zc) = (main_at(-6.0).deck[0], main_at(40.0).deck[0]);
    b.beam(
        v3(-6.0, 0.0, za + 0.8),
        v3(40.0, 0.0, zc + 0.6),
        v2(3.4, 1.6),
        v2(2.4, 1.2),
    );
}

/// The carriage, in the house: two sills either side of the well, joined ahead of it,
/// each carrying a trunnion tower with the bearing at the top.
fn carriage(b: &mut MeshBuilder) {
    let fine = b.fine();
    let deck = main_at(TRUNNION.x).deck[0];
    let [x0, x1] = SILL_X;
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.frustum_open(
            v3((x0 + x1) * 0.5, (SILL[0] + SILL[1]) * 0.5, deck - 0.2),
            v2(x1 - x0, SILL[1] - SILL[0]),
            v2(x1 - x0 - 2.0, SILL[1] - SILL[0] - 0.6),
            SILL_TOP - deck + 0.2,
            Vec2::ZERO,
        );
    });
    // The bridge piece ahead of the well joining the sills.
    b.frustum_open(
        v3(x1 - 2.0, 0.0, deck - 0.2),
        v2(4.0, 2.0 * SILL[1]),
        v2(3.0, 2.0 * SILL[1] - 0.6),
        SILL_TOP - 1.0 - deck + 0.2,
        Vec2::ZERO,
    );
    // The towers: raked trapezoids narrowing to the bearing, a dark web between legs.
    let (ti, to) = (TOWER_Y[0], TOWER_Y[1]);
    let t = TRUNNION;
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.extrude_y(
            &[
                [t.x - 12.0, SILL_TOP],
                [t.x + 12.0, SILL_TOP],
                [t.x + 5.0, t.z - 0.5],
                [t.x + 2.6, t.z + 3.2],
                [t.x - 2.6, t.z + 3.2],
                [t.x - 5.0, t.z - 0.5],
            ],
            ti,
            to,
        );
        // The bearing: a round boss through the tower's top.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(
            v3(t.x, ti - 0.3, t.z),
            v3(t.x, to + 0.6, t.z),
            3.0,
            2.8,
            b.sides(12),
        );
        if fine {
            // Stiffeners down the tower's outer face, and a hatch.
            b.paint(PLATING_DARK).pattern(pattern::PLAIN);
            for dx in [-7.0, 7.0] {
                b.beam(
                    v3(t.x + dx, to + 0.15, SILL_TOP),
                    v3(t.x + dx * 0.4, to + 0.15, t.z - 2.0),
                    v2(0.3, 0.8),
                    v2(0.3, 0.6),
                );
            }
            b.block(
                v3(t.x - 1.6, to - 0.05, SILL_TOP + 1.0),
                v3(t.x + 1.6, to + 0.25, SILL_TOP + 4.0),
            );
        }
    });
    if fine {
        // Power leads up the towers' inner faces into the trunnion (the rails' feed).
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.mirror_y(|b| {
            for dx in [-1.6, 1.6] {
                b.cylinder_between(
                    v3(t.x + dx * 3.0, ti - 0.35, SILL_TOP),
                    v3(t.x + dx, ti - 0.35, t.z - 2.8),
                    0.4,
                    0.4,
                    5,
                );
            }
        });
    }
}

/// The cradle, in the barrel frame: a long armoured block that swallows the barrel from
/// its breech block to where the jacket comes out, the trunnion pin through it.
fn cradle(b: &mut MeshBuilder) {
    let (hw, hh) = (CRADLE_HW, CRADLE_HH);
    let profile = [
        [CRADLE_BACK, -hh],
        [CRADLE_NOSE - 5.0, -hh],
        [CRADLE_NOSE, -hh + 1.8],
        [CRADLE_NOSE, hh - 1.8],
        [CRADLE_NOSE - 5.0, hh],
        [CRADLE_BACK + 1.5, hh],
        [CRADLE_BACK, hh - 1.5],
    ];
    b.paint(PLATING);
    b.extrude_y_chamfered(&profile, hw, 0.6);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for x in [-9.0, 6.0] {
        b.cuboid(v3(x, 0.0, 0.0), v3(1.2, 2.0 * hw + 0.4, 2.0 * hh + 0.4));
    }
    // The nose ring the jacket comes out of.
    let (_, jacket_r) = zenith::BARREL_JACKET;
    b.cylinder_between(
        v3(CRADLE_NOSE - 0.2, 0.0, 0.0),
        v3(CRADLE_NOSE + 0.8, 0.0, 0.0),
        jacket_r * GUN_SCALE + 0.7,
        jacket_r * GUN_SCALE + 0.45,
        b.sides(16),
    );
    // The trunnion pin, into the bearings.
    b.paint(METAL);
    b.cylinder_between(
        v3(0.0, -TOWER_Y[0] - 0.6, 0.0),
        v3(0.0, TOWER_Y[0] + 0.6, 0.0),
        2.0,
        2.0,
        b.sides(10),
    );
    team_panel(b, v3(-3.0, 0.0, hh), v2(3.2, 6.0));
    if b.fine() {
        // Hatches and a vent grille on the cradle's flanks.
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        b.mirror_y(|b| {
            b.block(v3(-6.5, hw - 0.05, -2.4), v3(-2.5, hw + 0.2, 2.4));
            for k in 0..3 {
                let x = 8.4 + 1.1 * k as f32;
                b.block(v3(x, hw - 0.05, -2.8), v3(x + 0.5, hw + 0.25, 2.8));
            }
        });
    }
}

// ---- lights ------------------------------------------------------------------

/// A ship's own lights, as the Leviathan's: red and green sidelights on the bridge
/// wings, a white masthead lamp and stern light, deck floods and lit scuttles.
fn lights(b: &mut MeshBuilder) {
    let [front, _, half, top, rake] = DECKS[2];
    let wing_x = front - rake * (BRIDGE[0] - DECKS[1][3]) / (top - DECKS[1][3]) - 1.8;
    for (side, glow) in [(1.0, GLOW_NAV_RED), (-1.0, GLOW_NAV_GREEN)] {
        b.paint(glow);
        b.cuboid(
            v3(wing_x, side * (half + 3.3), BRIDGE[0] - 0.1),
            v3(0.6, 0.35, 0.45),
        );
    }
    b.paint(GLOW_LAMP);
    b.cuboid(v3(MAST.x, 0.0, MAST_TOP), Vec3::splat(0.45));
    let stern = MAIN[0];
    b.cuboid(
        v3(stern.x - 0.2, 0.0, stern.deck[0] - 1.0),
        v3(0.2, 0.6, 0.35),
    );
    if !b.fine() {
        return;
    }
    // Floods under the first deck's lip, looking forward over the gun.
    b.mirror_y(|b| {
        b.cuboid(
            v3(DECKS[0][0] - DECKS[0][4] - 0.1, 5.0, DECKS[0][3] - 0.6),
            v3(0.14, 0.8, 0.3),
        )
    });
    // Scuttles along the lowest deck's sides.
    b.paint(WINDOWS);
    b.mirror_y(|b| {
        let [front, back, half, top, _] = DECKS[0];
        let z = (WING_Z[1] + top) * 0.5 + 0.3;
        for k in 0..6 {
            let x = back + 2.5 + k as f32 * (front - back - 6.0) / 5.0;
            b.block(
                v3(x - 0.3, half - 0.25, z - 0.3),
                v3(x + 0.3, half + 0.03, z + 0.3),
            );
        }
    });
}

// ---- detail ------------------------------------------------------------------

fn deck_detail(b: &mut MeshBuilder) {
    // Walkways down the foredeck either side of the trunk, and aft of the well.
    walkway(b, &MAIN, -7.0, 70.0, 7.0);
    rails(b, &MAIN, 0.0, 72.0, 1.1, 0.4);
    for x in [60.0, 30.0, -80.0] {
        let (z, half) = deck_at(&MAIN, x);
        b.mirror_y(|b| bollard(b, v3(x, half - 1.4, z), 1.1));
    }
    // Liferafts along the wing deck's outboard edge.
    b.mirror_y(|b| {
        for x in [-56.0, -62.0, -68.0] {
            liferaft(b, v3(x, 28.8, WING_Z[1]), 2.2, 0.55);
        }
    });
    // Vents on the outriggers' decks.
    b.mirror_y(|b| {
        for x in [-40.0, -10.0] {
            let (z, _) = deck_at(&AMA, x);
            vent(b, v3(x, AMA_Y, z), v2(3.0, 1.8), 3, METAL);
        }
    });
}

/// Waterjet ducts under the centre hull's transom and a rudder under each outrigger.
fn underwater(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        b.cylinder_between(v3(-76.0, 3.4, -3.8), v3(-82.6, 3.4, -2.8), 1.4, 1.2, 8);
        b.extrude_y(
            &[[-60.0, -1.0], [-57.0, -1.0], [-57.4, -5.6], [-59.8, -5.8]],
            AMA_Y - 0.25,
            AMA_Y + 0.25,
        );
    });
}

// ---- the coarse level ---------------------------------------------------------

/// Far off, under 60 triangles: three V hulls, the wing and beam as flat decks, one
/// block for the superstructure and its mast, the gun as one bar in its house, and the
/// owner's colour.
fn coarse(b: &mut MeshBuilder) {
    hull(b, &MAIN, &MAIN_COARSE);
    b.mirror_y(|b| b.at(v3(0.0, AMA_Y, 0.0), |b| hull(b, &AMA, &AMA_COARSE)));
    b.paint(PLATING);
    let flat = |b: &mut MeshBuilder, x0: f32, x1: f32, half: f32, z: f32| {
        b.face(&[
            v3(x0, -half, z),
            v3(x1, -half, z),
            v3(x1, half, z),
            v3(x0, half, z),
        ])
    };
    flat(b, WING[3][0], WING[0][0], WING[2][1], WING_Z[1]);
    flat(b, BEAM[3][0] + 4.0, BEAM[0][0] - 4.0, BEAM[1][1], BEAM_Z[1]);
    b.frustum_open(
        v3(-63.5, 0.0, WING_Z[1]),
        v2(25.0, 30.0),
        v2(4.0, 3.0),
        MAST_TOP - 4.0 - WING_Z[1],
        v2(-0.5, 0.0),
    );
    team_panel(b, v3(-40.0, 20.0, WING_Z[1]), v2(8.0, 6.0));
    let t = TRUNNION;
    let square = |x: f32, h: f32| {
        vec![
            v3(t.x + x, -h, t.z - h),
            v3(t.x + x, h, t.z - h),
            v3(t.x + x, h, t.z + h),
            v3(t.x + x, -h, t.z + h),
        ]
    };
    b.with_house(0, TRUNNION, RECOIL, |b| {
        b.with_recoil(|b| {
            // An uncapped square bar: its ends are never seen from this far.
            b.paint(PLATING_DARK);
            b.loft(
                &[square(RAIL.breech, 4.5), square(RAIL.muzzle, 2.8)],
                false,
                true,
            );
        });
    });
}

#[cfg(test)]
mod tests {
    use super::{CRADLE_BACK, CRADLE_NOSE, GUN_SCALE, TRUNNION, WELL_FLOOR, WELL_HW, WELL_X};
    use crate::models::aster::zenith;

    /// The cradle swallows the barrel's hidden stretch: the breech block's front sits in
    /// it and the drawn jacket starts inside its nose.
    #[test]
    fn the_cradle_covers_the_undrawn_barrel() {
        let (jacket_from, _) = zenith::BARREL_JACKET;
        assert!(jacket_from * GUN_SCALE < CRADLE_NOSE - 1.0);
        assert!(zenith::rail(GUN_SCALE).breech < CRADLE_BACK);
    }

    /// Elevated to 85 degrees, the breech swings down into the well, clear of its walls
    /// and floor, and the well reaches from behind the breech at rest to past it raised.
    #[test]
    fn the_breech_swings_into_the_well() {
        let (tail, radius) = zenith::BARREL_BREECH;
        let (tail, radius) = (tail * GUN_SCALE, radius * GUN_SCALE);
        assert!(
            TRUNNION.x + tail > WELL_X[0] + 1.0,
            "the breech at rest overhangs the well"
        );
        assert!(radius < WELL_HW - 1.5, "the breech is wider than the well");
        for degrees in [30.0f32, 60.0, 85.0] {
            let (s, c) = degrees.to_radians().sin_cos();
            // The breech's lowest corner, and its most forward one.
            let low = TRUNNION.z + tail * s - radius * c;
            let fore = TRUNNION.x + tail * c + radius * s;
            assert!(
                low > WELL_FLOOR + 0.5,
                "{degrees}: breech {low} under the floor"
            );
            assert!(
                fore < WELL_X[1],
                "{degrees}: breech {fore} past the well's front"
            );
        }
    }
}
