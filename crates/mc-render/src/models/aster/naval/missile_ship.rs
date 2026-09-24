//! Swordfish: cruise-missile ship (tech 2).
//!
//! A 40 m knife of a hull that is all launch rack: eight sea-skimmer cells in
//! two rows of four, canted forward on a dark plinth amidships with blast
//! deflectors behind them; a small faceted bridge far aft under a low mast
//! with its search radar turning; no gun at all. The cells are fixed hull
//! geometry (`turret_turn: 0`), so there is no house: the centre of every lid
//! is one of the weapon's muzzles. Tech 2: the seam round each lid glows
//! orange, the exhausts glow, and a few sensor panels on the mast are lit.
use super::*;

const HULL: [Station; 7] = [
    station(-19.5, -1.0, [-0.8, 2.3], [1.2, 2.9], [3.0, 3.0]),
    station(-13.0, -2.2, [-1.5, 2.6], [1.2, 3.25], [3.05, 3.35]),
    station(-4.0, -2.7, [-1.8, 2.7], [1.3, 3.35], [3.15, 3.45]),
    station(5.0, -2.7, [-1.8, 2.55], [1.4, 3.25], [3.3, 3.35]),
    station(12.0, -2.2, [-1.4, 1.7], [1.7, 2.5], [3.65, 2.7]),
    station(17.0, -1.4, [-0.7, 0.7], [2.1, 1.4], [4.1, 1.7]),
    station(20.0, 1.2, [1.5, 0.0], [2.6, 0.0], [4.5, 0.0]),
];

/// Lid centres of the cells (`muzzles` in the unit file): rows by x, one cell each side.
const CELL_X: [f32; 4] = [2.0, 0.0, -2.0, -4.0];
const CELL_Y: f32 = 1.5;
const CELL_Z: f32 = 6.0;
/// The cells' forward cant, and their height from root to lid.
const CANT: f32 = 15.0 * std::f32::consts::PI / 180.0;
const CELL_H: f32 = 2.05;
/// The dark plinth the rack stands on, and its top.
const RACK_X: f32 = -1.0;
const PLINTH_TOP: f32 = 3.95;
/// The mast's foot, the radar's axis on top of it, and the bridge's centre.
const MAST_X: f32 = -7.8;
const RADAR: Vec3 = Vec3::new(-8.12, 0.0, 8.6);
const BRIDGE_X: f32 = -12.2;

fn lids() -> impl Iterator<Item = Vec3> {
    CELL_X
        .iter()
        .flat_map(|&x| [v3(x, -CELL_Y, CELL_Z), v3(x, CELL_Y, CELL_Z)])
}

/// A strip laid on the deck from `x0` to `x1`, `half` wide, following the sheer.
/// Painted with whatever brush is set.
fn deck_strip(b: &mut MeshBuilder, x0: f32, x1: f32, half: f32) {
    let mut xs = vec![x0];
    xs.extend(HULL.iter().map(|s| s.x).filter(|&x| x > x0 + 0.3 && x < x1 - 0.3));
    xs.push(x1);
    let rings: Vec<Vec<Vec3>> = xs
        .iter()
        .map(|&x| {
            let z = deck_at(&HULL, x).0;
            vec![v3(x, -half, z - 0.03), v3(x, half, z - 0.03), v3(x, half, z + 0.07), v3(x, -half, z + 0.07)]
        })
        .collect();
    b.loft(&rings, true, true);
}

/// One missile cell rooted in the plinth, canted forward so its lid's centre is `lid`:
/// a dark angular box, a black lid, and (tech 2) the orange seam round it.
fn cell(b: &mut MeshBuilder, lid: Vec3) {
    let (s, c) = CANT.sin_cos();
    let base = lid - v3(CELL_H * s, 0.0, CELL_H * c);
    b.pitched(base, -CANT, |b| {
        let top = CELL_H - 0.12;
        b.paint(PLATING_DARK);
        if b.fine() {
            b.chamfered_box(v3(0.0, 0.0, (top - 0.25) * 0.5), v3(1.55, 1.55, top + 0.25), 0.16);
        } else {
            b.cuboid(v3(0.0, 0.0, (top - 0.25) * 0.5), v3(1.55, 1.55, top + 0.25));
        }
        b.paint(ACCENT);
        b.plate(v3(0.0, 0.0, top), v2(1.28, 1.28), 0.12, 0.04);
        if b.fine() {
            // The seam round the lid glows: the cell is armed.
            for side in [-1.0, 1.0] {
                glow_strip(b, v3(side * 0.7, 0.0, top), v2(0.09, 1.28), GLOW_ORANGE);
                glow_strip(b, v3(0.0, side * 0.7, top), v2(1.5, 0.09), GLOW_ORANGE);
            }
            // Lid hinge along the back edge, a latch at the front.
            b.paint(METAL);
            b.block(v3(-0.74, -0.5, top), v3(-0.62, 0.5, top + 0.2));
            b.block(v3(0.5, -0.12, top + 0.1), v3(0.66, 0.12, top + 0.16));
        }
    });
}

pub(super) fn build(b: &mut MeshBuilder) {
    hull(b, &HULL, &[0, 3, 6]);
    let (s, c) = CANT.sin_cos();

    if b.coarse() {
        // Bridge, plinth, mast, and a lid quad per cell for the muzzles.
        b.paint(PLATING);
        b.frustum_open(v3(BRIDGE_X, 0.0, 3.05), v2(6.0, 4.8), v2(5.0, 3.8), 3.3, v2(-0.3, 0.0));
        team_panel(b, v3(BRIDGE_X - 0.3, 0.0, 6.35), v2(2.2, 2.4));
        b.paint(ACCENT);
        b.cuboid_open(v3(RACK_X, 0.0, 4.35), v3(9.6, 4.8, 2.5));
        b.paint(PLATING);
        b.loft(
            &[
                vec![v3(MAST_X + 1.1, -1.2, 3.15), v3(MAST_X + 1.1, 1.2, 3.15), v3(MAST_X - 1.1, 0.0, 3.15)],
                vec![v3(RADAR.x + 0.3, -0.35, RADAR.z), v3(RADAR.x + 0.3, 0.35, RADAR.z), v3(RADAR.x - 0.3, 0.0, RADAR.z)],
            ],
            false,
            true,
        );
        b.paint(PLATING_DARK);
        let along = v3(c, 0.0, -s) * 0.62;
        let across = Vec3::Y * 0.62;
        for m in lids() {
            b.face(&[m - along - across, m + along - across, m + along + across, m - along + across]);
        }
        return;
    }

    // ---- the rack ----------------------------------------------------------------
    // A dark plinth over the midship deck, a non-skid lane between the rows, the cells,
    // and raked blast deflectors behind them that take the launch exhaust.
    b.paint(ACCENT);
    b.at(v3(RACK_X, 0.0, 0.0), |b| {
        b.loft_z(
            &chamfered_rect(v2(5.4, 2.8), 0.6),
            &[Section::new(3.0, 1.0), Section::new(3.5, 1.0), Section::new(PLINTH_TOP, 0.95)],
        );
    });
    b.paint(PLATING).pattern(pattern::WALKWAY);
    b.plate(v3(-1.3, 0.0, PLINTH_TOP), v2(7.4, 1.1), 0.06, 0.02);
    for m in lids() {
        cell(b, m);
    }
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.extrude_y(
            &[[-5.45, PLINTH_TOP - 0.05], [-6.35, PLINTH_TOP - 0.05], [-6.15, 6.35], [-5.55, 6.45]],
            0.4,
            2.55,
        );
    });
    if b.mid() {
        // White facing plates on the plinth's flanks, over the dark frame.
        b.paint(PLATING);
        b.mirror_y(|b| {
            b.block(v3(-4.6, 2.62, 3.55), v3(-0.6, 2.7, PLINTH_TOP - 0.15));
            b.block(v3(0.0, 2.62, 3.55), v3(3.4, 2.7, PLINTH_TOP - 0.15));
        });
        b.paint(ACCENT);
        b.mirror_y(|b| b.block(v3(-6.4, 0.4, 6.4), v3(-5.5, 2.55, 6.55)));
    }

    // ---- bridge, far aft ------------------------------------------------------------
    // A pointed, faceted house: a dark sill, white walls drawn in as they rise, a dark
    // screen all round, a white cap with the team's colour on it.
    let bridge = [[3.0, -1.6], [3.0, 1.6], [1.6, 2.5], [-3.0, 2.5], [-3.0, -2.5], [1.6, -2.5]];
    b.at(v3(BRIDGE_X, 0.0, 0.0), |b| {
        b.paint(ACCENT);
        b.loft_z(&bridge, &[Section::new(2.95, 1.02), Section::new(3.5, 1.02)]);
        b.paint(PLATING);
        b.loft_z(&bridge, &[Section::new(3.5, 1.0), Section::scaled(5.4, 0.93, 0.9)]);
        b.paint(GLASS);
        b.loft_z(&bridge, &[Section::scaled(5.4, 0.93, 0.9), Section::scaled(6.0, 0.86, 0.82)]);
        b.paint(PLATING);
        b.loft_z(&bridge, &[Section::scaled(6.0, 0.88, 0.84), Section::scaled(6.35, 0.84, 0.8)]);
    });
    team_panel(b, v3(BRIDGE_X - 0.4, 0.0, 6.35), v2(2.2, 2.4));

    // ---- the mast, just forward of the bridge -------------------------------------------
    // A low faceted pyramid, the search radar turning on top of it.
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2(1.2, 1.3), 0.35),
        &[
            Section::new(3.1, 1.0).shifted(MAST_X, 0.0),
            Section::scaled(5.4, 0.78, 0.78).shifted(MAST_X - 0.12, 0.0),
            Section::scaled(7.7, 0.48, 0.5).shifted(MAST_X - 0.28, 0.0),
            Section::scaled(RADAR.z, 0.36, 0.38).shifted(MAST_X - 0.32, 0.0),
        ],
    );
    b.paint(ACCENT);
    b.loft_z(
        &chamfered_rect(v2(1.24, 1.34), 0.36),
        &[Section::new(3.1, 1.0).shifted(MAST_X, 0.0), Section::new(3.45, 0.99).shifted(MAST_X, 0.0)],
    );
    b.set_spinner_pivot(RADAR);
    b.with_part(part::SPINNER, |b| {
        b.paint(ACCENT);
        b.prism(RADAR, b.sides(8), 0.3, 0.24, 0.3);
        b.paint(PLATING);
        b.beam(RADAR + v3(0.0, -1.3, 0.78), RADAR + v3(0.0, 1.3, 0.78), v2(0.2, 0.86), v2(0.2, 0.86));
        if b.mid() {
            b.paint(ACCENT);
            b.beam(RADAR + v3(0.12, -1.2, 0.78), RADAR + v3(0.12, 1.2, 0.78), v2(0.06, 0.7), v2(0.06, 0.7));
            b.beam(RADAR + v3(0.0, 0.0, 0.3), RADAR + v3(0.0, 0.0, 0.34), v2(0.4, 0.3), v2(0.4, 0.3));
        }
    });

    // ---- decks -------------------------------------------------------------------------
    walkway(b, &HULL, 5.2, 18.8, 0.25);
    walkway(b, &HULL, -19.2, -15.5, 0.25);
    rub_rail(b, &HULL, -19.5, 19.8, 0.24);
    // The team's band down the foredeck, ahead of the rack.
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    deck_strip(b, 7.5, 14.5, 0.8);
    // A breakwater ahead of the rack.
    b.paint(PLATING);
    b.mirror_y(|b| {
        let z = deck_at(&HULL, 5.6).0;
        b.beam(v3(6.5, 0.0, z + 0.3), v3(5.3, 2.5, z + 0.3), v2(0.1, 0.6), v2(0.1, 0.6));
    });
    // Low twin exhausts out of the bridge's back, raked aft: pipework, not funnels.
    b.mirror_y(|b| {
        let (a, e) = (v3(-14.6, 1.6, 4.8), v3(-16.9, 1.7, 5.35));
        b.paint(PLATING);
        b.cylinder_between(a, e, 0.44, 0.38, b.sides(8));
        b.paint(ACCENT);
        b.cylinder_between(e - (e - a).normalize() * 0.35, e + (e - a).normalize() * 0.05, 0.42, 0.42, b.sides(8));
        if b.fine() {
            b.paint(GLOW_ORANGE);
            b.cylinder_between(e - (e - a).normalize() * 0.02, e + (e - a).normalize() * 0.06, 0.3, 0.3, 8);
        }
    });

    if !b.fine() {
        return;
    }
    // ---- fine kit ----------------------------------------------------------------------
    // Lit sensor panels on the mast's forward face, a yard, ESM domes and whips.
    b.mirror_y(|b| {
        on_slope(b, [MAST_X + 0.86, 5.4], [MAST_X + 0.35, 7.7], 0.5, |b| {
            glow_strip(b, v3(0.0, 0.42, 0.02), v2(1.1, 0.3), GLOW);
        });
    });
    b.paint(METAL);
    b.beam(v3(MAST_X - 0.15, -1.3, 7.3), v3(MAST_X - 0.15, 1.3, 7.3), v2(0.12, 0.12), v2(0.12, 0.12));
    b.paint(PLATING);
    b.mirror_y(|b| b.spheroid(v3(MAST_X - 0.3, 0.55, RADAR.z + 0.05), v3(0.2, 0.2, 0.18), 6, 2));
    whip(b, v3(MAST_X - 0.15, 1.2, 7.35), 2.4, 0.05);
    whip(b, v3(MAST_X - 0.15, -1.2, 7.35), 2.0, 0.08);
    whip(b, v3(-19.0, 0.0, 3.05), 1.6, 0.1);
    // Bridge wings with a decoy launcher each, a lit panel on the bridge front.
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.block(v3(BRIDGE_X - 1.4, 2.35, 5.4), v3(BRIDGE_X + 0.6, 3.0, 5.62));
        b.block(v3(BRIDGE_X - 0.8, 2.5, 5.62), v3(BRIDGE_X - 0.1, 2.95, 5.85));
        b.paint(METAL);
        for i in 0..3 {
            let p = v3(BRIDGE_X - 0.7 + i as f32 * 0.2, 2.72, 5.85);
            b.cylinder_between(p, p + v3(0.1, 0.1, 0.2), 0.06, 0.06, 5);
        }
    });
    b.paint(GLOW);
    b.block(v3(BRIDGE_X + 2.84, -0.5, 4.2), v3(BRIDGE_X + 2.96, 0.5, 4.5));
    // Liferafts beside the mast, vents on the plinth's ends, a hatch on the foredeck.
    b.mirror_y(|b| {
        liferaft(b, v3(-6.6, 2.55, deck_at(&HULL, -6.6).0 + 0.05), 0.9, 0.28);
        vent(b, v3(3.6, 1.8, PLINTH_TOP), v2(1.0, 0.7), 3, METAL);
    });
    b.paint(PLATING);
    b.plate(v3(15.5, 0.0, deck_at(&HULL, 15.5).0 + 0.05), v2(1.0, 0.9), 0.07, 0.03);
    // The anchor: windlass, cables to the hawse pipes in the flare.
    let wz = deck_at(&HULL, 16.8).0;
    b.paint(METAL);
    b.cylinder_between(v3(16.8, -0.6, wz + 0.3), v3(16.8, 0.6, wz + 0.3), 0.26, 0.26, 8);
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.beam(v3(16.8, 0.35, wz + 0.1), v3(18.2, 0.85, wz + 0.05), v2(0.12, 0.1), v2(0.12, 0.1));
        b.cylinder_between(v3(18.3, 0.95, wz + 0.05), v3(18.7, 1.15, wz - 0.4), 0.18, 0.18, 6);
    });
    for x in [15.0, -17.8] {
        let (z, half) = deck_at(&HULL, x);
        b.mirror_y(|b| bollard(b, v3(x, half - 0.55, z + 0.05), 0.4));
    }
    rails(b, &HULL, 6.0, 18.6, 0.8, 0.16);
    rails(b, &HULL, -19.0, -15.7, 0.8, 0.16);
    // Cable trays along the plinth's foot, feeding the cells.
    b.paint(ACCENT);
    b.mirror_y(|b| b.block(v3(-5.8, 2.75, 3.15), v3(4.0, 3.05, 3.38)));
    b.paint(METAL);
    b.mirror_y(|b| b.block(v3(-5.6, 2.82, 3.38), v3(3.8, 2.98, 3.46)));
}
