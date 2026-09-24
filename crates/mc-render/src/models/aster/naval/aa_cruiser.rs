//! Manta: tech 2 air-defence cruiser. A 44 m hull as wide as it is low: a flat
//! slab-sided beam flared out over two dark side keels, so from ahead it reads
//! almost as a catamaran. It is built round its big faceted radar tower: a
//! pyramid off the deckhouse roof with a dark phased-array panel let into each
//! face behind a lit blue seam, the search radar turning on top, and the
//! interceptor laser's lens on a ball mount on its front face. Forward of the
//! bridge a raised vertical-launch deck with four cell hatches (dark lids in
//! blue-lit rims); the deck gun in its own house on the foredeck ahead of it;
//! the twin flak in a house on the deckhouse roof, firing over a bridge kept low
//! for it. Decoy launchers, low side exhausts and a working quarterdeck aft.
//!
//! Houses: 1 twin flak at (-4, 0, 9.6); 2 deck gun at (14.5, 0, 5.4).
//! Fixed: 0 vertical-launch cells at (6 / 4, ±2, 6.4), the hatches.
use super::*;

/// Stern first: a broad flat run, the chine well inside the flared knuckle.
const HULL: [Station; 8] = [
    station(-22.0, -1.5, [-1.3, 3.7], [0.6, 4.7], [2.5, 4.9]),
    station(-15.0, -2.6, [-2.2, 4.3], [0.65, 5.5], [2.55, 5.7]),
    station(-6.0, -2.9, [-2.5, 4.5], [0.7, 5.7], [2.6, 5.9]),
    station(3.0, -2.9, [-2.5, 4.4], [0.8, 5.6], [2.7, 5.8]),
    station(10.0, -2.6, [-2.1, 3.5], [1.0, 4.7], [2.9, 4.9]),
    station(15.5, -2.0, [-1.5, 2.2], [1.4, 3.3], [3.2, 3.5]),
    station(19.5, -0.6, [-0.2, 0.9], [2.0, 1.7], [3.5, 1.9]),
    station(21.8, 1.4, [1.8, 0.0], [2.6, 0.0], [3.9, 0.0]),
];

/// The cell hatches (weapon 0's muzzles, as in the unit file).
const CELLS: [[f32; 3]; 4] = [[6.0, -2.0, 6.4], [6.0, 2.0, 6.4], [4.0, -2.0, 6.4], [4.0, 2.0, 6.4]];
/// The launch deck's top, under the hatches.
const CELL_DECK: f32 = 6.3;
/// The flak house's pivot, and where its barrels end (y ±0.4).
const FLAK: Vec3 = Vec3::new(-4.0, 0.0, 9.6);
const FLAK_MUZZLE: Vec3 = Vec3::new(-1.6, 0.4, 9.8);
/// The deck gun's pivot and muzzle.
const GUN: Vec3 = Vec3::new(14.5, 0.0, 5.4);
const GUN_MUZZLE: Vec3 = Vec3::new(19.5, 0.0, 5.4);
/// The tower's foot and its top, where the search radar turns.
const TOWER_X: f32 = -9.5;
const TOWER_FOOT: f32 = 7.75;
const RADAR: Vec3 = Vec3::new(TOWER_X, 0.0, 14.0);

/// Half-width of the tower's white faces at height `z` (its plan is square).
fn tower_half(z: f32) -> f32 {
    2.91 - (z - 8.35) * 0.3239
}

pub(super) fn build(b: &mut MeshBuilder) {
    hull(b, &HULL, &[0, 3, 7]);
    let gun_deck = deck_at(&HULL, GUN.x).0;

    // The side keels under the flare: two long dark sponsons, the catamaran in her.
    if !b.coarse() {
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            if b.fine() {
                b.chamfered_box(v3(-4.0, 4.35, -1.2), v3(20.0, 1.3, 1.9), 0.3);
            } else {
                b.cuboid(v3(-4.0, 4.35, -1.2), v3(20.0, 1.3, 1.9));
            }
        });
    }

    // The launch deck: a faceted raised block forward of the bridge, its hatches on top.
    let cells = chamfered_rect(v2(3.0, 3.5), 0.55);
    let cells_at = v3(5.0, 0.0, 0.0);
    // Deckhouse: the long faceted block the bridge, the flak and the tower stand on.
    let house = chamfered_rect(v2(7.5, 3.7), 1.1);
    let house_at = v3(-5.5, 0.0, 0.0);
    // Bridge: pointed, low, the screen all round.
    let bridge = [[2.9, -1.5], [2.9, 1.5], [1.7, 2.9], [-1.6, 2.9], [-1.6, -2.9], [1.7, -2.9]];
    let bridge_at = v3(-1.0, 0.0, 0.0);
    let tower = chamfered_rect(v2(3.0, 3.0), 0.6);
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(cells_at + Vec3::Z * 2.6, v2(6.0, 7.0), v2(5.8, 6.6), CELL_DECK - 2.6, v2(0.0, 0.0));
        b.frustum_open(house_at + Vec3::Z * 2.55, v2(15.0, 7.4), v2(14.4, 6.6), TOWER_FOOT - 2.55, v2(0.0, 0.0));
        b.frustum_open(v3(TOWER_X, 0.0, TOWER_FOOT), v2(6.0, 6.0), v2(2.2, 2.2), RADAR.z - TOWER_FOOT, v2(0.0, 0.0));
        team_panel(b, v3(5.0, 0.0, CELL_DECK), v2(2.6, 1.6));
        b.paint(PLATING_DARK);
        for [x, y, z] in CELLS {
            b.face(&[v3(x - 0.7, y - 0.7, z), v3(x + 0.7, y - 0.7, z), v3(x + 0.7, y + 0.7, z), v3(x - 0.7, y + 0.7, z)]);
        }
    } else {
        b.at(cells_at, |b| {
            b.paint(ACCENT);
            b.loft_z(&cells, &[Section::new(2.6, 1.02), Section::new(3.1, 1.02)]);
            b.paint(PLATING);
            b.loft_z(&cells, &[Section::new(3.1, 1.0), Section::scaled(CELL_DECK, 0.98, 0.95)]);
        });
        // The hatches: a lit rim showing round a dark lid.
        for [x, y, z] in CELLS {
            b.paint(ACCENT);
            b.plate(v3(x, y, CELL_DECK), v2(1.7, 1.7), 0.05, 0.02);
            b.paint(GLOW);
            b.plate(v3(x, y, CELL_DECK + 0.05), v2(1.52, 1.52), 0.03, 0.01);
            b.paint(PLATING_DARK);
            b.plate(v3(x, y, CELL_DECK + 0.08), v2(1.36, 1.36), z - CELL_DECK - 0.08, 0.03);
        }
        team_panel(b, v3(5.0, 0.0, CELL_DECK), v2(2.6, 1.6));
        b.at(house_at, |b| {
            b.paint(ACCENT);
            b.loft_z(&house, &[Section::new(2.55, 1.02), Section::new(3.1, 1.02)]);
            b.paint(PLATING);
            b.loft_z(&house, &[Section::new(3.1, 1.0), Section::scaled(7.6, 0.98, 0.9)]);
            b.paint(ACCENT);
            b.loft_z(&house, &[Section::scaled(7.6, 0.99, 0.92), Section::scaled(TOWER_FOOT, 0.99, 0.92)]);
        });
        b.at(bridge_at, |b| {
            b.paint(PLATING);
            b.loft_z(&bridge, &[Section::new(TOWER_FOOT, 1.0), Section::scaled(8.65, 0.97, 0.95)]);
            b.paint(GLASS);
            b.loft_z(&bridge, &[Section::scaled(8.65, 0.97, 0.95), Section::scaled(9.3, 0.9, 0.86)]);
            b.paint(PLATING);
            b.loft_z(&bridge, &[Section::scaled(9.3, 0.92, 0.88), Section::scaled(9.55, 0.88, 0.84)]);
        });
        // The tower: a dark foot, then the white pyramid up to the radar pedestal.
        b.at(v3(TOWER_X, 0.0, 0.0), |b| {
            b.paint(ACCENT);
            b.loft_z(&tower, &[Section::new(TOWER_FOOT, 1.0), Section::new(8.35, 0.97)]);
            b.paint(PLATING);
            b.loft_z(&tower, &[Section::new(8.35, 0.97), Section::new(RADAR.z, 0.36)]);
        });
        // The phased arrays: a lit seam laid on each face, the dark panel over it.
        let (z0, z1) = (9.0, 12.6);
        let (h0, h1) = (tower_half(z0), tower_half(z1));
        let (w0, w1) = (3.4, 2.0);
        for (glow, off, grow) in [(true, 0.05, 0.18), (false, 0.1, 0.0)] {
            if glow && !b.fine() {
                continue;
            }
            b.paint(if glow { GLOW } else { GLASS });
            let (a, c) = (w0 + grow, w1 + grow);
            b.beam(v3(TOWER_X + h0 + off, 0.0, z0 - grow * 0.5), v3(TOWER_X + h1 + off, 0.0, z1 + grow * 0.5), v2(a, 0.12), v2(c, 0.12));
            b.beam(v3(TOWER_X - h0 - off, 0.0, z0 - grow * 0.5), v3(TOWER_X - h1 - off, 0.0, z1 + grow * 0.5), v2(a, 0.12), v2(c, 0.12));
            b.mirror_y(|b| {
                b.beam(v3(TOWER_X, h0 + off, z0 - grow * 0.5), v3(TOWER_X, h1 + off, z1 + grow * 0.5), v2(0.12, a), v2(0.12, c));
            });
        }
        // The interceptor laser: a ball on a yoke off the tower's front, its lens lit blue.
        let ball = v3(TOWER_X + tower_half(13.2) + 0.55, 0.0, 13.2);
        b.paint(ACCENT);
        b.block(ball - v3(0.7, 0.16, 0.2), ball - v3(0.2, -0.16, -0.2));
        b.paint(PLATING);
        b.spheroid(ball, Vec3::splat(0.42), b.sides(8), if b.fine() { 4 } else { 2 });
        b.paint(GLOW);
        b.cylinder_between(ball + Vec3::X * 0.36, ball + Vec3::X * 0.56, 0.2, 0.17, b.sides(8));
    }

    // Search radar on the tower top: a pedestal and a broad flat array, always turning.
    b.set_spinner_pivot(RADAR);
    b.with_part(part::SPINNER, |b| {
        if b.coarse() {
            return;
        }
        b.paint(ACCENT);
        b.prism(RADAR, b.sides(8), 0.4, 0.3, 0.35);
        b.paint(PLATING);
        b.beam(RADAR + v3(0.0, -1.7, 0.85), RADAR + v3(0.0, 1.7, 0.85), v2(0.24, 1.05), v2(0.24, 1.05));
        if b.mid() {
            b.paint(GLOW);
            b.beam(RADAR + v3(0.14, -1.6, 0.85), RADAR + v3(0.14, 1.6, 0.85), v2(0.06, 0.8), v2(0.06, 0.8));
            b.paint(ACCENT);
            b.beam(RADAR + v3(0.0, 0.0, 0.35), RADAR + v3(0.0, 0.0, 0.39), v2(0.5, 0.36), v2(0.5, 0.36));
        }
    });

    // The flak tub on the deckhouse roof, its house turning on the collar.
    if b.mid() {
        b.paint(PLATING);
        b.at(v3(FLAK.x, 0.0, 0.0), |b| {
            b.loft_z(&ngon(b.sides(8), 1.35), &[Section::new(TOWER_FOOT - 0.05, 1.0), Section::new(8.85, 0.9)]);
        });
        b.paint(ACCENT);
        b.at(v3(FLAK.x, 0.0, 0.0), |b| {
            b.loft_z(&ngon(b.sides(8), 1.35), &[Section::new(8.75, 0.94), Section::new(9.0, 0.94)]);
        });
    }
    b.with_house(1, FLAK, 0.3, |b| {
        if b.coarse() {
            return;
        }
        let (x, z) = (FLAK.x, FLAK_MUZZLE.z);
        b.paint(METAL);
        b.prism(v3(x, 0.0, 9.02), b.sides(8), 0.9, 0.85, 0.12);
        b.paint(PLATING);
        b.at(v3(x - 0.15, 0.0, 0.0), |b| {
            b.loft_z(
                &chamfered_rect(v2(0.85, 0.7), 0.28),
                &[Section::new(9.14, 1.0), Section::new(9.7, 1.0), Section::scaled(10.0, 0.78, 0.8).shifted(-0.1, 0.0)],
            );
        });
        b.with_recoil(|b| {
            for y in [-FLAK_MUZZLE.y, FLAK_MUZZLE.y] {
                gun_tube(b, v3(x + 0.5, y, z), v3(FLAK_MUZZLE.x, y, z), 0.07);
            }
        });
        if b.fine() {
            // Ammunition drums each side, the tracker on top with its lit window.
            b.paint(ACCENT);
            b.mirror_y(|b| b.block(v3(x - 0.75, 0.66, 9.2), v3(x + 0.25, 0.9, 9.75)));
            b.block(v3(x - 0.2, -0.14, 9.95), v3(x + 0.25, 0.14, 10.17));
            b.paint(GLOW);
            b.block(v3(x + 0.25, -0.1, 10.0), v3(x + 0.28, 0.1, 10.13));
        }
    });

    // The deck gun forward of the cells: a dark ring, a faceted gunhouse, one long rifle.
    if !b.coarse() {
        b.paint(ACCENT);
        b.prism(v3(GUN.x, 0.0, gun_deck - 0.05), b.sides(10), 1.4, 1.34, 0.22);
    }
    b.with_house(2, GUN, 0.6, |b| {
        if b.coarse() {
            return;
        }
        let house_x = GUN.x - 0.2;
        b.paint(PLATING);
        b.at(v3(house_x, 0.0, 0.0), |b| {
            b.loft_z(
                &turret_plan(3.3, 2.5),
                &[
                    Section::new(gun_deck + 0.17, 0.95),
                    Section::new(gun_deck + 0.6, 1.0),
                    Section::scaled(5.95, 0.64, 0.64).shifted(-0.25, 0.0),
                ],
            );
        });
        b.with_recoil(|b| {
            cannon(b, v3(GUN.x + 0.5, 0.0, GUN.z), GUN_MUZZLE, 0.13, Emitter::Unlit);
            b.paint(ACCENT);
            b.block(v3(GUN.x + 0.9, -0.42, GUN.z - 0.4), v3(GUN.x + 1.5, 0.42, GUN.z + 0.4));
        });
        if b.fine() {
            b.paint(ACCENT);
            b.plate(v3(house_x - 0.5, 0.45, 5.95), v2(0.6, 0.5), 0.05, 0.02);
            b.block(v3(house_x - 1.62, -0.5, 4.3), v3(house_x - 1.5, 0.5, 4.8));
        }
    });

    if b.coarse() {
        return;
    }
    walkway(b, &HULL, 8.5, 20.8, 0.2);
    walkway(b, &HULL, -21.7, -13.4, 0.22);
    if b.fine() {
        rub_rail(b, &HULL, -22.0, 21.2, 0.24);
    }
    // The team's band across the quarterdeck.
    let qz = deck_at(&HULL, -17.5).0;
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(-17.5, 0.0, qz + 0.05), v2(3.2, 6.4), 0.07, 0.03);

    // Low exhausts: raked stubs out of the deckhouse's aft quarters, hot inside, and a
    // pipe run low along each quarter to a dark mouth near the stern.
    b.mirror_y(|b| {
        if !b.fine() {
            return;
        }
        b.paint(ACCENT);
        b.cylinder_between(v3(-11.0, 3.25, 5.2), v3(-11.9, 4.1, 5.6), 0.42, 0.46, b.sides(8));
        b.paint(METAL);
        b.cylinder_between(v3(-11.9, 4.1, 5.6), v3(-11.97, 4.17, 5.63), 0.3, 0.3, b.sides(8));
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(v3(-12.3, 4.25, 4.0), v3(-19.5, 4.0, 3.4), 0.26, 0.26, 6);
            b.cylinder_between(v3(-19.5, 4.0, 3.4), v3(-19.9, 4.0, 3.38), 0.32, 0.3, 6);
        }
    });
    if b.fine() {
        // Decoy launchers on the quarterdeck, angled outboard; a towed-sonar reel aft.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(-15.3, 3.2, qz), v3(-14.5, 4.1, qz + 0.45));
            b.paint(METAL);
            for i in 0..2 {
                let p = v3(-15.1 + i as f32 * 0.4, 3.7, qz + 0.45);
                b.cylinder_between(p, p + v3(0.0, 0.35, 0.55), 0.09, 0.09, 6);
            }
        });
        b.paint(PLATING_DARK);
        b.cylinder_between(v3(-21.0, -0.7, qz + 0.65), v3(-21.0, 0.7, qz + 0.65), 0.5, 0.5, 6);
        b.paint(ACCENT);
        b.mirror_y(|b| b.block(v3(-21.4, 0.7, qz), v3(-20.6, 0.85, qz + 1.0)));
    }
    // The breakwater ahead of the gun, the yard across the tower.
    b.paint(PLATING);
    b.mirror_y(|b| {
        let z = deck_at(&HULL, 17.4).0;
        b.beam(v3(18.0, 0.0, z + 0.28), v3(16.9, 1.6, z + 0.28), v2(0.1, 0.56), v2(0.1, 0.56));
    });
    b.paint(METAL);
    if b.fine() {
        b.beam(v3(TOWER_X, -2.2, 12.9), v3(TOWER_X, 2.2, 12.9), v2(0.12, 0.12), v2(0.12, 0.12));
    }
    // The red obstruction lamp on a stalk off the tower's aft face, clear of the array.
    let lamp = v3(TOWER_X - tower_half(12.2) - 0.6, 0.0, 12.2);
    b.cylinder_between(lamp + Vec3::X * 0.5, lamp, 0.05, 0.05, 4);
    beacon(b, lamp);

    if !b.fine() {
        return;
    }
    // Lit sensor panels on the bridge sides and front (tech 2), ESM domes on the
    // tower's shoulders.
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.block(v3(-1.2, 2.72, 8.0), v3(0.4, 2.8, 8.5));
        b.paint(GLOW);
        b.block(v3(-1.1, 2.8, 8.12), v3(0.3, 2.83, 8.38));
    });
    b.paint(GLOW);
    b.block(v3(1.88, -0.5, 8.05), v3(1.92, 0.5, 8.2));
    whip(b, v3(TOWER_X, 2.1, 12.95), 2.4, 0.05);
    whip(b, v3(TOWER_X, -2.1, 12.95), 2.0, 0.08);
    // Liferafts along the deckhouse roof edges, vents behind the tower.
    b.mirror_y(|b| liferaft(b, v3(-6.6, 3.05, TOWER_FOOT), 0.9, 0.28));
    b.mirror_y(|b| vent(b, v3(-12.6, 1.4, TOWER_FOOT), v2(1.0, 0.8), 2, METAL));
    // The anchor: windlass, cables to the hawse pipes, the pipes in the flare.
    b.paint(METAL);
    let wz = deck_at(&HULL, 18.6).0;
    b.cylinder_between(v3(18.6, -0.6, wz + 0.3), v3(18.6, 0.6, wz + 0.3), 0.26, 0.26, 8);
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.beam(v3(18.6, 0.35, wz + 0.1), v3(19.9, 0.75, wz + 0.02), v2(0.14, 0.1), v2(0.14, 0.1));
        b.cylinder_between(v3(20.0, 0.85, wz + 0.05), v3(20.4, 1.05, wz - 0.4), 0.18, 0.18, 6);
    });
}
