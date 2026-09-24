//! Marlin: tech 2 destroyer. A 44 m hull, the Pike's bigger, meaner successor:
//! a long low hull with a raked stem over a tiled sonar bulb that carries the
//! four torpedo tube doors, a faceted twin rail-gun house forward with the blue
//! accelerator glow along its rails, a stealth-sloped deckhouse with the bridge
//! band and a pyramidal mast with the search radar turning on top, low side
//! exhausts, a hangar aft with a point-defence gun house on its roof, and the
//! interceptor tubes on the quarterdeck: two steep launch tubes that run down
//! through the deck to their doors in the hull bottom under the stern.
//!
//! Houses: 0 twin rail gun at (12, 0, 5.2); 2 point-defence gun at (-9, 0, 9).
//! Fixed: 1 torpedo tubes (16, ±0.8, -1.2 / -2.0) in the bulb; 3 interceptor
//! tubes (-14, ±1.4, -1.0) under the quarterdeck.
use super::*;

/// Stern first: a long lean hull, a flat run aft, the forefoot rising into a
/// raked stem over the bulb.
const HULL: [Station; 9] = [
    station(-22.0, -1.3, [-1.0, 2.5], [0.9, 3.15], [2.55, 3.25]),
    station(-17.0, -2.6, [-1.8, 2.85], [0.95, 3.55], [2.6, 3.65]),
    station(-10.0, -3.3, [-2.2, 3.05], [1.05, 3.8], [2.7, 3.9]),
    station(-2.0, -3.4, [-2.25, 3.05], [1.1, 3.82], [2.8, 3.92]),
    station(6.0, -3.2, [-2.05, 2.75], [1.25, 3.55], [2.95, 3.68]),
    station(11.5, -2.8, [-1.7, 2.1], [1.5, 2.9], [3.25, 3.05]),
    station(15.5, -1.9, [-1.1, 1.3], [1.8, 2.15], [3.6, 2.35]),
    station(19.0, -0.2, [0.3, 0.55], [2.3, 1.3], [4.05, 1.5]),
    station(21.8, 2.2, [2.6, 0.0], [3.2, 0.0], [4.45, 0.0]),
];

/// The rail-gun house's yaw axis and trunnion (`pivot` in the unit file); its two
/// barrels run at y ±GUN_Y to GUN_MUZZLE_X.
const GUN: Vec3 = Vec3::new(12.0, 0.0, 5.2);
const GUN_Y: f32 = 0.5;
const GUN_MUZZLE_X: f32 = 20.5;
/// The point-defence house's pivot, and where its barrels end (y ±0.3).
const PD: Vec3 = Vec3::new(-9.0, 0.0, 9.0);
const PD_MUZZLE: Vec3 = Vec3::new(-7.0, 0.3, 9.2);
/// Torpedo tube mouths in the bulb's face, as in the unit file.
const TUBES: [[f32; 3]; 4] = [[16.0, -0.8, -1.2], [16.0, 0.8, -1.2], [16.0, -0.8, -2.0], [16.0, 0.8, -2.0]];
/// Where the interceptor torpedoes leave, under the quarterdeck.
const INTERCEPT: [[f32; 3]; 2] = [[-14.0, -1.4, -1.0], [-14.0, 1.4, -1.0]];
/// Where the search radar turns.
const RADAR: Vec3 = Vec3::new(2.75, 0.0, 12.0);

/// One ring of the sonar bulb at `x`, scaled about its centre: a pentagon, keel
/// first then up the port side and down the starboard, like a hull station.
fn bulb_ring(x: f32, s: f32) -> Vec<Vec3> {
    let c = -1.7;
    let p = |y: f32, z: f32| v3(x, y * s, c + (z - c) * s);
    vec![p(0.0, -2.85), p(-1.45, -1.75), p(-1.05, -0.55), p(1.05, -0.55), p(1.45, -1.75)]
}

pub(super) fn build(b: &mut MeshBuilder) {
    hull(b, &HULL, &[0, 3, 8]);
    let gun_deck = deck_at(&HULL, GUN.x).0;

    // The sonar bulb under the forefoot, tiled like a submarine, with the four
    // torpedo doors in its face: dark shutters round the tube mouths.
    if !b.coarse() {
        b.paint(PLATING_DARK).pattern(pattern::TILES);
        b.loft(
            &[bulb_ring(10.0, 0.3), bulb_ring(12.4, 0.8), bulb_ring(15.0, 1.0), bulb_ring(16.05, 0.92)],
            true,
            true,
        );
        b.paint(ACCENT);
        for m in TUBES {
            let [x, y, z] = m;
            let x = x + 0.08;
            b.face(&[v3(x, y - 0.33, z - 0.3), v3(x, y + 0.33, z - 0.3), v3(x, y + 0.33, z + 0.3), v3(x, y - 0.33, z + 0.3)]);
        }
    }

    // Deckhouse: a dark plinth, white faceted walls with tumblehome, a dark coping.
    let house = chamfered_rect(v2(6.5, 3.05), 1.0);
    let house_at = v3(2.0, 0.0, 0.0);
    // Bridge: a pointed front, the screen all round, a white cap.
    let bridge = [[3.6, -1.3], [3.6, 1.3], [2.3, 2.5], [-2.6, 2.5], [-2.6, -2.5], [2.3, -2.5]];
    let bridge_at = v3(4.0, 0.0, 0.0);
    // Hangar aft, the point-defence tub on its roof.
    let hangar = chamfered_rect(v2(4.3, 2.75), 0.8);
    let hangar_at = v3(-8.8, 0.0, 0.0);
    let hangar_roof = 7.75;
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(v3(-2.3, 0.0, 2.6), v2(21.6, 5.6), v2(19.4, 4.7), 4.5, v2(0.4, 0.0));
        b.frustum_open(v3(2.9, 0.0, 6.9), v2(4.0, 3.2), v2(0.8, 0.8), 5.1, v2(-0.2, 0.0));
        team_panel(b, v3(-6.5, 0.0, 7.1), v2(6.0, 3.2));
    } else {
        b.at(house_at, |b| {
            b.paint(ACCENT);
            b.loft_z(&house, &[Section::new(2.7, 1.02), Section::new(3.25, 1.02)]);
            b.paint(PLATING);
            b.loft_z(&house, &[Section::new(3.25, 1.0), Section::scaled(6.4, 0.98, 0.9)]);
            b.paint(ACCENT);
            b.loft_z(&house, &[Section::scaled(6.4, 0.99, 0.92), Section::scaled(6.56, 0.99, 0.92)]);
        });
        b.at(bridge_at, |b| {
            b.paint(PLATING);
            b.loft_z(&bridge, &[Section::new(6.56, 1.0), Section::scaled(8.0, 0.97, 0.95)]);
            b.paint(GLASS);
            b.loft_z(&bridge, &[Section::scaled(8.0, 0.97, 0.95), Section::scaled(8.7, 0.9, 0.86)]);
            b.paint(PLATING);
            b.loft_z(&bridge, &[Section::scaled(8.7, 0.92, 0.88), Section::scaled(9.05, 0.88, 0.84)]);
        });
        // The integrated mast: a faceted pyramid off the bridge roof, leaning aft.
        b.loft_z(
            &chamfered_rect(v2(1.6, 1.7), 0.4),
            &[
                Section::new(9.05, 1.0).shifted(3.2, 0.0),
                Section::scaled(10.7, 0.7, 0.68).shifted(2.95, 0.0),
                Section::scaled(RADAR.z, 0.42, 0.4).shifted(RADAR.x, 0.0),
            ],
        );
        b.at(hangar_at, |b| {
            b.paint(ACCENT);
            b.loft_z(&hangar, &[Section::new(2.55, 1.02), Section::new(3.1, 1.02)]);
            b.paint(PLATING);
            b.loft_z(&hangar, &[Section::new(3.1, 1.0), Section::scaled(7.6, 0.98, 0.9)]);
            b.paint(ACCENT);
            b.loft_z(&hangar, &[Section::scaled(7.6, 0.99, 0.92), Section::scaled(hangar_roof, 0.99, 0.92)]);
        });
        team_panel(b, v3(-2.0, 0.0, 6.56), v2(2.4, 3.2));
        // The team's band down the hangar roof, forward of the tub.
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(-5.9, 0.0, hangar_roof), v2(2.2, 3.4), 0.08, 0.03);
    }

    // Search radar on the masthead: a pedestal and a flat array, always turning.
    b.set_spinner_pivot(RADAR);
    b.with_part(part::SPINNER, |b| {
        if b.coarse() {
            return;
        }
        b.paint(ACCENT);
        b.prism(RADAR, b.sides(8), 0.36, 0.28, 0.32);
        b.paint(PLATING);
        b.beam(RADAR + v3(0.0, -1.5, 0.8), RADAR + v3(0.0, 1.5, 0.8), v2(0.22, 0.95), v2(0.22, 0.95));
        if b.mid() {
            b.paint(GLOW);
            b.beam(RADAR + v3(0.13, -1.4, 0.8), RADAR + v3(0.13, 1.4, 0.8), v2(0.06, 0.7), v2(0.06, 0.7));
            b.paint(ACCENT);
            b.beam(RADAR + v3(0.0, 0.0, 0.32), RADAR + v3(0.0, 0.0, 0.36), v2(0.45, 0.32), v2(0.45, 0.32));
        }
    });

    // The rail-gun house: a dark ring on the deck, a faceted gunhouse turning on it,
    // the twin accelerators and their mantlet elevating together.
    b.paint(ACCENT);
    if !b.coarse() {
        b.prism(v3(GUN.x, 0.0, gun_deck - 0.05), b.sides(10), 1.8, 1.72, 0.26);
    }
    b.with_house(0, GUN, 0.9, |b| {
        let house_x = GUN.x - 0.4;
        b.paint(PLATING);
        if b.coarse() {
            b.frustum_open(v3(house_x, 0.0, gun_deck), v2(4.4, 3.2), v2(2.6, 1.9), 6.0 - gun_deck, v2(-0.3, 0.0));
            b.paint(METAL);
            for y in [-GUN_Y, GUN_Y] {
                b.face(&[
                    v3(13.6, y - 0.15, GUN.z),
                    v3(GUN_MUZZLE_X, y - 0.12, GUN.z),
                    v3(GUN_MUZZLE_X, y + 0.12, GUN.z),
                    v3(13.6, y + 0.15, GUN.z),
                ]);
            }
            return;
        }
        b.at(v3(house_x, 0.0, 0.0), |b| {
            b.loft_z(
                &turret_plan(4.6, 3.3),
                &[
                    Section::new(gun_deck + 0.21, 0.95),
                    Section::new(gun_deck + 0.75, 1.0),
                    Section::scaled(6.0, 0.62, 0.6).shifted(-0.3, 0.0),
                ],
            );
        });
        b.with_recoil(|b| {
            for y in [-GUN_Y, GUN_Y] {
                rail_gun(b, v3(12.9, y, GUN.z), v3(GUN_MUZZLE_X, y, GUN.z), v2(0.13, 0.36), 0.16, Emitter::Blue);
            }
            // One mantlet over both breeches.
            b.paint(ACCENT);
            b.block(v3(13.45, -1.05, GUN.z - 0.55), v3(14.3, 1.05, GUN.z + 0.55));
            if b.fine() {
                // The accelerator glow along the outside of each rail pair.
                b.paint(GLOW);
                for y in [-GUN_Y - 0.29, GUN_Y + 0.29] {
                    b.beam(v3(16.0, y, GUN.z), v3(GUN_MUZZLE_X - 0.5, y, GUN.z), v2(0.04, 0.12), v2(0.04, 0.09));
                }
            }
        });
        if b.fine() {
            // A hatch on the roof, a vent at the back, the ranging optic on the front slope.
            b.paint(ACCENT);
            b.plate(v3(house_x - 0.6, 0.55, 6.0), v2(0.6, 0.5), 0.05, 0.02);
            b.block(v3(house_x - 2.3, -0.55, 4.5), v3(house_x - 2.18, 0.55, 5.1));
            b.paint(GLASS);
            b.block(v3(house_x + 1.35, -0.16, 5.85), v3(house_x + 1.5, 0.16, 5.95));
        }
    });

    // The point-defence tub over the hangar, its gun house turning on the collar.
    if b.mid() {
        b.paint(PLATING);
        b.at(v3(PD.x, 0.0, 0.0), |b| {
            b.loft_z(&ngon(b.sides(8), 1.35), &[Section::new(hangar_roof - 0.05, 1.05), Section::new(8.45, 0.92)]);
        });
        b.paint(ACCENT);
        b.at(v3(PD.x, 0.0, 0.0), |b| {
            b.loft_z(&ngon(b.sides(8), 1.35), &[Section::new(8.35, 0.96), Section::new(8.6, 0.96)]);
        });
    }
    b.with_house(2, PD, 0.2, |b| {
        if b.coarse() {
            return;
        }
        let (x, z) = (PD.x, PD_MUZZLE.z);
        b.paint(METAL);
        b.prism(v3(x, 0.0, 8.62), b.sides(8), 0.9, 0.85, 0.12);
        b.paint(PLATING);
        b.at(v3(x - 0.15, 0.0, 0.0), |b| {
            b.loft_z(
                &chamfered_rect(v2(0.82, 0.62), 0.26),
                &[Section::new(8.74, 1.0), Section::new(9.25, 1.0), Section::scaled(9.55, 0.78, 0.8).shifted(-0.1, 0.0)],
            );
        });
        b.with_recoil(|b| {
            for y in [-PD_MUZZLE.y, PD_MUZZLE.y] {
                gun_tube(b, v3(x + 0.45, y, z), v3(PD_MUZZLE.x, y, z), 0.055);
            }
        });
        if b.fine() {
            // Ammunition feeds each side, the tracker on top with its lit window.
            b.paint(ACCENT);
            b.mirror_y(|b| b.block(v3(x - 0.7, 0.6, 8.8), v3(x + 0.2, 0.8, 9.3)));
            b.block(v3(x - 0.2, -0.14, 9.5), v3(x + 0.25, 0.14, 9.72));
            b.paint(GLOW);
            b.block(v3(x + 0.25, -0.1, 9.55), v3(x + 0.28, 0.1, 9.68));
        }
    });

    if b.coarse() {
        return;
    }
    walkway(b, &HULL, 8.6, 20.8, 0.2);
    walkway(b, &HULL, -21.7, -13.4, 0.2);
    if b.fine() {
        rub_rail(b, &HULL, -22.0, 21.2, 0.24);
    }

    // Low exhausts: raked stubs out of the deckhouse's aft quarters, hot inside, and a
    // pipe run low along each side of the hangar to a dark mouth at its aft corner.
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.cylinder_between(v3(-3.0, 2.55, 5.3), v3(-3.95, 3.35, 5.7), 0.42, 0.46, b.sides(8));
        b.paint(METAL);
        b.cylinder_between(v3(-3.95, 3.35, 5.7), v3(-4.02, 3.41, 5.73), 0.3, 0.3, b.sides(8));
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(v3(-4.6, 3.15, 4.4), v3(-12.7, 3.05, 3.85), 0.26, 0.26, 6);
            b.cylinder_between(v3(-12.7, 3.05, 3.85), v3(-13.1, 3.05, 3.82), 0.32, 0.3, 6);
        }
    });
    // Hangar door on the aft face, a shutter.
    b.paint(ACCENT).pattern(pattern::SHUTTER);
    b.block(v3(-13.28, -1.9, 3.2), v3(-13.08, 1.9, 6.9));

    // Interceptor tubes on the quarterdeck: two dark tubes standing steeply aft-down
    // in cradles, breech caps at the top, running on down through the deck to the
    // launch points under the hull, where their doors are.
    let qz = deck_at(&HULL, -14.0).0;
    b.mirror_y(|b| {
        let (top, mouth) = (v3(-11.2, 1.5, qz + 1.75), Vec3::from(INTERCEPT[1]));
        b.paint(ACCENT);
        b.cylinder_between(top, mouth, 0.34, 0.3, b.sides(8));
        b.paint(PLATING_DARK);
        b.cylinder_between(top + (top - mouth).normalize() * 0.35, top, 0.4, 0.4, b.sides(8));
        if b.fine() {
            b.paint(ACCENT);
            b.block(v3(-12.3, 1.05, qz), v3(-11.5, 1.95, qz + 0.9));
            b.beam(v3(-11.9, 1.5, qz + 0.9), v3(-11.5, 1.5, qz + 1.4), v2(0.5, 0.2), v2(0.5, 0.2));
        }
    });
    // A yard across the mast, the breakwater ahead of the gun.
    b.paint(METAL);
    b.beam(v3(2.7, -1.55, 11.0), v3(2.7, 1.55, 11.0), v2(0.12, 0.12), v2(0.12, 0.12));
    b.paint(PLATING);
    b.mirror_y(|b| {
        let z = deck_at(&HULL, 15.2).0;
        b.beam(v3(15.9, 0.0, z + 0.28), v3(14.8, 1.7, z + 0.28), v2(0.1, 0.56), v2(0.1, 0.56));
    });
    // The red obstruction lamp on a stalk off the mast's front face.
    b.paint(METAL);
    b.cylinder_between(v3(4.0, 0.0, 10.9), v3(4.55, 0.0, 10.9), 0.05, 0.05, 4);
    beacon(b, v3(4.55, 0.0, 10.9));

    if !b.fine() {
        return;
    }
    // Lit sensor panels flush in the mast's faces (tech 2 earns them), and dark
    // decoy launchers on the bridge wings.
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.beam(v3(3.4, 1.6, 9.35), v3(3.05, 1.2, 10.55), v2(0.05, 0.95), v2(0.05, 0.75));
        b.paint(GLOW);
        b.beam(v3(3.4, 1.62, 9.45), v3(3.1, 1.24, 10.45), v2(0.03, 0.12), v2(0.03, 0.1));
        b.paint(ACCENT);
        b.block(v3(5.2, 1.6, 6.56), v3(5.9, 2.2, 6.82));
    });
    // Sensor panel on the bridge front under the screen: blue, tech 2.
    b.paint(GLOW);
    b.block(v3(7.62, -0.5, 7.2), v3(7.66, 0.5, 7.32));
    // ESM domes on the masthead corners, whips off the yard and the transom.
    b.paint(PLATING);
    b.mirror_y(|b| b.spheroid(v3(2.3, 0.55, 12.05), v3(0.22, 0.22, 0.2), 6, 2));
    whip(b, v3(2.7, 1.45, 11.05), 2.6, 0.05);
    whip(b, v3(2.7, -1.45, 11.05), 2.2, 0.08);
    // Liferafts on the deckhouse roof sides, vents on the hangar roof aft of the tub.
    b.mirror_y(|b| liferaft(b, v3(-2.0, 2.2, 6.56), 0.9, 0.28));
    b.mirror_y(|b| vent(b, v3(-11.6, 1.5, hangar_roof), v2(1.2, 0.8), 3, METAL));
    // The anchor: windlass, cables to the hawse pipes, the pipes in the flare.
    b.paint(METAL);
    let wz = deck_at(&HULL, 17.2).0;
    b.cylinder_between(v3(17.2, -0.7, wz + 0.3), v3(17.2, 0.7, wz + 0.3), 0.28, 0.28, 8);
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.beam(v3(17.2, 0.4, wz + 0.1), v3(18.9, 0.95, wz + 0.02), v2(0.14, 0.1), v2(0.14, 0.1));
        b.cylinder_between(v3(19.0, 1.05, wz + 0.05), v3(19.5, 1.3, wz - 0.4), 0.2, 0.2, 6);
    });
    let (z, half) = deck_at(&HULL, 9.6);
    b.mirror_y(|b| bollard(b, v3(9.6, half - 0.6, z + 0.05), 0.42));
    // Doors in the hull bottom where the interceptors leave: dark plates in the
    // bottom's slope at x -14 (keel -2.9, chine (2.94, -1.97) there).
    b.paint(ACCENT);
    b.mirror_y(|b| {
        let z = |y: f32| -2.9 + y * 0.3163 - 0.03;
        b.face(&[
            v3(-14.55, 1.1, z(1.1)),
            v3(-14.55, 1.75, z(1.75)),
            v3(-13.45, 1.75, z(1.75)),
            v3(-13.45, 1.1, z(1.1)),
        ]);
    });
    // A towed-sonar reel low on the transom, and rings round the quarterdeck.
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(-20.6, -0.8, qz + 0.75), v3(-20.6, 0.8, qz + 0.75), 0.55, 0.55, 6);
    b.paint(ACCENT);
    b.mirror_y(|b| b.block(v3(-21.0, 0.8, qz), v3(-20.2, 0.95, qz + 1.1)));
}
