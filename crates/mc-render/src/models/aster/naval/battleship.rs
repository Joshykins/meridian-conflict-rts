//! The Leviathan: the tech 3 battleship, the hero of the navy.
//!
//! A 116 m plasma dreadnought, lean and mean: a long flared hull with a raked
//! stem and a knuckle, a bulbous forefoot, a low transom stern over twin
//! rudders and two shrouded propulsors, three triple plasma gunhouses (two
//! forward, superfiring, one astern) on their own houses, and a stepped,
//! stealth-faceted citadel amidships: a forward tower carrying the bridge band
//! and a pyramidal integrated mast, an aft tower carrying the light AA mount.
//! Flux conduits run from the citadel to each barbette like power feeds, the
//! barrels carry conduit runs and blue plasma emitter rings, and the reactor
//! shows through slits in the citadel's flanks. Everything above the water is
//! in the data's frame: the waterline is z = 0, the keel goes to -9.

use glam::Vec2;

use super::*;

// ---- the hull --------------------------------------------------------------

/// Stern first. Amidships the side has a touch of tumblehome; forward the deck
/// edge flares well out over the knuckle, and the stem rakes forward and up.
const HULL: [Station; 14] = [
    station(-58.0, -3.2, [-2.2, 5.0], [2.2, 6.3], [5.0, 6.0]),
    station(-53.0, -5.8, [-3.4, 6.3], [2.4, 7.4], [5.2, 7.2]),
    station(-46.0, -7.8, [-4.4, 7.1], [2.6, 7.9], [5.4, 7.8]),
    station(-36.0, -8.8, [-4.9, 7.5], [2.8, 8.2], [5.6, 8.1]),
    station(-24.0, -9.0, [-5.1, 7.7], [3.0, 8.3], [5.8, 8.2]),
    station(-10.0, -9.0, [-5.1, 7.7], [3.1, 8.3], [6.0, 8.2]),
    station(4.0, -9.0, [-5.0, 7.6], [3.2, 8.3], [6.2, 8.15]),
    station(16.0, -8.8, [-4.8, 7.2], [3.4, 8.1], [6.5, 7.95]),
    station(27.0, -8.2, [-4.3, 6.3], [3.7, 7.5], [6.9, 7.4]),
    station(36.0, -7.2, [-3.6, 5.0], [4.1, 6.0], [7.4, 6.4]),
    station(44.0, -5.8, [-2.6, 3.5], [4.6, 4.6], [8.0, 5.2]),
    station(50.0, -3.8, [-1.4, 2.0], [5.2, 3.0], [8.6, 3.6]),
    station(54.5, -1.0, [0.4, 0.9], [6.0, 1.5], [9.1, 1.9]),
    station(58.0, 3.0, [4.2, 0.0], [7.2, 0.0], [9.6, 0.0]),
];
const HULL_COARSE: [usize; 4] = [0, 5, 10, 13];

// ---- the batteries (pivots and muzzles from the unit file) -----------------

const FORE_PIVOT: Vec3 = Vec3::new(30.0, 0.0, 12.0);
const FORE_MUZZLE_X: f32 = 48.0;
const SECOND_PIVOT: Vec3 = Vec3::new(14.0, 0.0, 15.5);
const SECOND_MUZZLE_X: f32 = 32.0;
const AFT_PIVOT: Vec3 = Vec3::new(-30.0, 0.0, 12.0);
/// Authored facing forward like the others; the shader turns the house astern.
const AFT_MUZZLE_X: f32 = -12.0;
const AA_PIVOT: Vec3 = Vec3::new(-4.0, 0.0, 22.0);
const AA_MUZZLE_X: f32 = -2.2;
/// The three bores of a battery, either side of the house's centreline.
const BORES: [f32; 3] = [-2.2, 0.0, 2.2];
/// How far the barrels kick back on a salvo.
const RECOIL: f32 = 1.6;

/// The citadel base runs from the aft barbette's sweep to the second battery's.
const CITADEL: Vec3 = Vec3::new(-5.0, 0.0, 0.0);
const CITADEL_HALF: Vec2 = Vec2::new(11.0, 6.9);
const CITADEL_TOP: f32 = 10.9;
/// The forward tower's origin (its plan is drawn about this) and the mast on it.
const TOWER_X: f32 = 2.0;
const MAST: Vec3 = Vec3::new(0.8, 0.0, 19.2);
const MAST_TOP: f32 = 25.6;
/// The aft tower, carrying the AA mount.
const AFT_TOWER_X: f32 = -6.5;
const AFT_TOWER_TOP: f32 = 20.8;

pub(super) fn build(b: &mut MeshBuilder) {
    hull(b, &HULL, &HULL_COARSE);
    if b.coarse() {
        coarse(b);
        return;
    }
    underwater(b);
    decks(b);
    citadel(b);
    barbettes(b);
    fore_battery(b);
    second_battery(b);
    aft_battery(b);
    aa_mount(b);
    if b.fine() {
        deck_detail(b);
    }
}

// ---- the coarse level: a V hull, a citadel block, the two forward houses ----

fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.frustum_open(v3(-5.0, 0.0, 5.9), v2(22.0, 13.6), v2(11.0, 7.4), 15.4, v2(-1.0, 0.0));
    team_panel(b, v3(-13.0, 0.0, 10.9), v2(4.0, 8.0));
    b.paint(PLATING);
    b.frustum_open(v3(FORE_PIVOT.x - 0.8, 0.0, 7.0), v2(11.0, 11.6), v2(6.4, 7.6), 6.4, v2(-1.0, 0.0));
    b.frustum_open(v3(SECOND_PIVOT.x - 0.8, 0.0, 6.5), v2(11.0, 11.6), v2(6.4, 7.6), 10.5, v2(-1.0, 0.0));
    b.paint(METAL);
    for y in BORES {
        b.face(&[
            v3(FORE_PIVOT.x + 2.0, y - 0.45, 12.5),
            v3(FORE_MUZZLE_X, y - 0.35, 12.5),
            v3(FORE_MUZZLE_X, y + 0.35, 12.5),
            v3(FORE_PIVOT.x + 2.0, y + 0.45, 12.5),
        ]);
    }
    b.face(&[
        v3(SECOND_PIVOT.x + 2.0, -3.0, 16.0),
        v3(SECOND_MUZZLE_X, -2.6, 16.0),
        v3(SECOND_MUZZLE_X, 2.6, 16.0),
        v3(SECOND_PIVOT.x + 2.0, 3.0, 16.0),
    ]);
}

// ---- below the waterline ---------------------------------------------------

/// The bulbous forefoot, twin rudders and the two shrouded propulsors, all dark.
fn underwater(b: &mut MeshBuilder) {
    // The bulb, painted like the hull so it gets the antifouling.
    b.paint(PLATING).pattern(pattern::HULL);
    b.spheroid(v3(52.4, 0.0, -4.8), v3(5.6, 2.2, 2.6), b.sides(8), 3);
    // Twin rudders under the transom.
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.extrude_y(
            &[[-57.4, -1.6], [-54.6, -1.6], [-54.2, -7.2], [-56.6, -7.8]],
            3.2,
            3.56,
        );
        // Shrouded propulsor: an open ring with a hub cone inside it.
        let (y, z) = (3.6, -5.6);
        let sides = b.sides(10);
        let ring = |x: f32, r: f32| -> Vec<Vec3> {
            (0..sides)
                .map(|i| {
                    let a = (i as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
                    v3(x, y + a.cos() * r, z + a.sin() * r)
                })
                .collect()
        };
        b.paint(PLATING_DARK);
        b.loft(
            &[ring(-49.0, 2.4), ring(-53.6, 2.3), ring(-53.6, 1.9), ring(-49.0, 2.0)],
            false,
            false,
        );
        b.paint(METAL);
        b.cylinder_between(v3(-52.8, y, z), v3(-49.4, y, z), 0.7, 0.3, 6);
        if b.fine() {
            for i in 0..5 {
                let a = i as f32 * std::f32::consts::TAU / 5.0;
                let tip = v3(-51.2, y + a.cos() * 1.75, z + a.sin() * 1.75);
                b.beam(v3(-51.2, y, z), tip, v2(0.55, 0.12), v2(0.4, 0.08));
            }
            // Strut to the hull, a stern skeg between the rudders.
            b.paint(PLATING_DARK);
            b.beam(v3(-51.4, y, z + 2.2), v3(-51.4, y * 0.7, z + 5.4), v2(0.3, 1.2), v2(0.3, 1.4));
        }
    });
}

// ---- the deck --------------------------------------------------------------

fn decks(b: &mut MeshBuilder) {
    walkway(b, &HULL, 34.0, 55.6, 0.8);
    walkway(b, &HULL, -24.0, 8.0, 0.6);
    walkway(b, &HULL, -57.4, -36.0, 0.8);
    rub_rail(b, &HULL, -58.0, 57.4, 0.32);
    // The team's bands down the foredeck and across the quarterdeck.
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.mirror_y(|b| {
        let (z0, _) = deck_at(&HULL, 38.0);
        let (z1, _) = deck_at(&HULL, 50.0);
        b.beam(v3(38.0, 2.2, z0 + 0.06), v3(50.0, 1.7, z1 + 0.06), v2(0.9, 0.08), v2(0.7, 0.08));
    });
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(-41.0, 0.0, deck_at(&HULL, -41.0).0 + 0.05), v2(1.4, 12.6), 0.07, 0.03);
    // Flux conduits along the deck: power feeds from the citadel to each barbette.
    let feed = |b: &mut MeshBuilder, a: Vec3, c: Vec3| {
        b.paint(PLATING_DARK).pattern(pattern::FLUX);
        b.beam(a, c, v2(0.9, 0.4), v2(0.9, 0.4));
    };
    b.mirror_y(|b| {
        let z = |x: f32| deck_at(&HULL, x).0 + 0.2;
        feed(b, v3(6.0, 6.0, z(6.0)), v3(25.0, 6.0, z(25.0)));
        feed(b, v3(25.0, 6.0, z(25.0)), v3(28.6, 4.2, z(28.6)));
        feed(b, v3(6.0, 2.6, z(6.0)), v3(9.6, 2.6, z(9.6)));
        feed(b, v3(-16.0, 2.6, z(-16.0)), v3(-25.6, 2.6, z(-25.6)));
    });
}

// ---- the citadel -----------------------------------------------------------

fn citadel(b: &mut MeshBuilder) {
    let base = chamfered_rect(CITADEL_HALF, 2.2);
    b.at(CITADEL, |b| {
        b.paint(ACCENT);
        b.loft_z(&base, &[Section::new(5.5, 1.02), Section::new(6.8, 1.02)]);
        b.paint(PLATING);
        b.loft_z(&base, &[Section::new(6.8, 1.0), Section::scaled(CITADEL_TOP - 0.3, 0.9, 0.86)]);
        b.paint(ACCENT);
        b.loft_z(
            &base,
            &[Section::scaled(CITADEL_TOP - 0.3, 0.91, 0.87), Section::scaled(CITADEL_TOP, 0.91, 0.87)],
        );
    });

    // The forward tower: a pointed, raked front, the dark bridge band, a white cap.
    let tower = [[4.8, -3.0], [4.8, 3.0], [2.0, 5.6], [-4.6, 5.6], [-4.6, -5.6], [2.0, -5.6]];
    b.at(v3(TOWER_X, 0.0, 0.0), |b| {
        b.paint(PLATING);
        b.loft_z(
            &tower,
            &[
                Section::new(CITADEL_TOP - 0.1, 1.0),
                Section::scaled(13.6, 0.92, 0.9).shifted(-0.3, 0.0),
                Section::scaled(16.6, 0.78, 0.76).shifted(-0.9, 0.0),
            ],
        );
        b.paint(GLASS);
        b.loft_z(
            &tower,
            &[
                Section::scaled(16.6, 0.78, 0.76).shifted(-0.9, 0.0),
                Section::scaled(18.0, 0.72, 0.7).shifted(-1.1, 0.0),
            ],
        );
        b.paint(PLATING);
        b.loft_z(
            &tower,
            &[
                Section::scaled(18.0, 0.74, 0.72).shifted(-1.1, 0.0),
                Section::scaled(MAST.z, 0.64, 0.62).shifted(-1.3, 0.0),
            ],
        );
    });
    // The integrated mast: a faceted pyramid off the bridge roof, the lamp on top.
    let mast = chamfered_rect(v2(2.6, 2.3), 0.7);
    b.paint(PLATING);
    b.loft_z(
        &mast,
        &[
            Section::new(MAST.z - 0.05, 1.0).shifted(MAST.x, 0.0),
            Section::scaled(22.6, 0.62, 0.64).shifted(MAST.x - 0.3, 0.0),
            Section::scaled(MAST_TOP, 0.3, 0.32).shifted(MAST.x - 0.55, 0.0),
        ],
    );
    beacon(b, v3(MAST.x - 0.55, 0.0, MAST_TOP));

    // The aft tower: a faceted block with a dark waist, the AA tub on its roof.
    let aft = chamfered_rect(v2(4.6, 5.0), 1.3);
    b.at(v3(AFT_TOWER_X, 0.0, 0.0), |b| {
        b.paint(PLATING);
        b.loft_z(&aft, &[Section::new(CITADEL_TOP - 0.1, 1.0), Section::scaled(15.6, 0.94, 0.82)]);
        b.paint(ACCENT);
        b.loft_z(&aft, &[Section::scaled(15.6, 0.95, 0.83), Section::scaled(16.2, 0.95, 0.83)]);
        b.paint(PLATING);
        b.loft_z(
            &aft,
            &[
                Section::scaled(16.2, 0.94, 0.82),
                Section::scaled(AFT_TOWER_TOP - 0.3, 0.86, 0.6).shifted(0.1, 0.0),
            ],
        );
        b.paint(ACCENT);
        b.loft_z(
            &aft,
            &[
                Section::scaled(AFT_TOWER_TOP - 0.3, 0.87, 0.61).shifted(0.1, 0.0),
                Section::scaled(AFT_TOWER_TOP, 0.87, 0.61).shifted(0.1, 0.0),
            ],
        );
    });
    // Team panel on the citadel base roof, behind the aft tower.
    team_panel(b, v3(-13.4, 0.0, CITADEL_TOP), v2(3.6, 7.6));

    // The reactor shows through slits in the citadel's flanks: dark panels, plasma-lit.
    b.paint(PLATING_DARK).pattern(pattern::PLASMA);
    b.mirror_y(|b| b.cuboid(v3(-7.0, 6.35, 8.7), v3(7.0, 0.5, 1.7)));
    // Low exhaust ducts on the aft tower's shoulders: pipework, not chimneys.
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.frustum(v3(-9.2, 4.9, CITADEL_TOP), v2(3.4, 1.4), v2(2.6, 0.9), 1.9, v2(-0.5, 0.15));
    });
    b.paint(PLATING_DARK).pattern(pattern::PLASMA);
    b.mirror_y(|b| b.cuboid(v3(-9.7, 5.05, CITADEL_TOP + 1.9), v3(2.4, 0.7, 0.16)));

    if !b.fine() {
        return;
    }
    // Lit sensor panels let into the mast's faces and the forward tower's cheeks.
    b.paint(GLOW);
    b.mirror_y(|b| {
        b.beam(v3(MAST.x + 2.2, 1.2, 20.2), v3(MAST.x + 1.2, 0.8, 22.4), v2(0.7, 0.14), v2(0.5, 0.14));
        b.beam(v3(MAST.x - 0.6, 2.25, 19.9), v3(MAST.x - 0.7, 1.55, 22.0), v2(1.4, 0.14), v2(1.0, 0.14));
        b.beam(v3(TOWER_X + 3.6, 3.15, 12.0), v3(TOWER_X + 3.0, 2.9, 15.6), v2(0.9, 0.16), v2(0.7, 0.16));
    });
    // Flux runs up the aft tower's flanks to the AA tub.
    b.paint(PLATING_DARK).pattern(pattern::FLUX);
    b.mirror_y(|b| {
        b.beam(v3(AFT_TOWER_X, 5.15, 11.2), v3(AFT_TOWER_X, 4.25, 15.5), v2(0.3, 1.1), v2(0.3, 0.9));
        b.beam(v3(AFT_TOWER_X + 0.1, 4.2, 16.3), v3(AFT_TOWER_X + 0.1, 3.15, 20.4), v2(0.3, 0.9), v2(0.3, 0.7));
    });
    // Plasma glow strips along the citadel base's roof edges.
    b.mirror_y(|b| glow_strip(b, v3(-9.0, 5.35, CITADEL_TOP), v2(11.0, 0.3), GLOW));
    // A yard across the mast with whips, ESM domes at the bridge wings.
    b.paint(METAL);
    b.beam(v3(MAST.x - 0.4, -2.6, 23.0), v3(MAST.x - 0.4, 2.6, 23.0), v2(0.16, 0.16), v2(0.16, 0.16));
    whip(b, v3(MAST.x - 0.4, 2.4, 23.05), 2.4, 0.05);
    whip(b, v3(MAST.x - 0.4, -2.4, 23.05), 2.0, 0.08);
    b.paint(PLATING);
    b.mirror_y(|b| b.spheroid(v3(TOWER_X - 2.6, 3.6, MAST.z + 0.35), v3(0.5, 0.5, 0.4), 6, 2));
    // Bridge wings: dark decoy launchers either side of the glass band.
    b.paint(ACCENT);
    b.mirror_y(|b| b.block(v3(TOWER_X + 0.4, 3.85, 16.7), v3(TOWER_X + 2.0, 4.6, 17.2)));
}

// ---- barbettes and gunhouses -----------------------------------------------

/// The armoured drum a house turns on: a dark ring, a white barrel, a dark collar.
fn barbette(b: &mut MeshBuilder, x: f32, top: f32, radius: f32) {
    let deck = deck_at(&HULL, x).0;
    let sides = b.sides(12);
    if !b.fine() {
        b.paint(PLATING);
        b.prism(v3(x, 0.0, deck - 0.15), sides, radius + 0.2, radius - 0.2, top - deck + 0.15);
        return;
    }
    b.paint(ACCENT);
    b.prism(v3(x, 0.0, deck - 0.15), sides, radius + 0.3, radius + 0.1, 0.9);
    b.paint(PLATING);
    b.prism(v3(x, 0.0, deck + 0.75), sides, radius + 0.1, radius, top - 0.45 - deck - 0.75);
    b.paint(ACCENT);
    b.prism(v3(x, 0.0, top - 0.45), sides, radius - 0.1, radius - 0.35, 0.45);
}

fn barbettes(b: &mut MeshBuilder) {
    barbette(b, FORE_PIVOT.x, 9.4, 5.0);
    barbette(b, SECOND_PIVOT.x, 13.0, 5.4);
    barbette(b, AFT_PIVOT.x, 9.4, 5.0);
}

/// Plan of a triple-gun house: a wide raked face, swept cheeks, a short bustle,
/// drawn about the house's centre (0.8 m behind the pivot).
fn house_plan() -> Vec<[f32; 2]> {
    vec![
        [5.4, -3.6],
        [5.4, 3.6],
        [3.0, 5.8],
        [-3.0, 5.8],
        [-5.6, 4.0],
        [-5.6, -4.0],
        [-3.0, -5.8],
        [3.0, -5.8],
    ]
}

/// A plasma gun: a gunmetal tube in a dark jacket, dark conduit runs down the
/// sides, blue emitter rings near the muzzle and a flared muzzle shroud with a
/// lit bore. Tech 3 earns the emitters.
fn plasma_gun(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, r: f32) {
    let d = (muzzle - breech).normalize();
    let length = (muzzle - breech).length();
    let at = |t: f32| breech + d * (length * t);
    let sides = b.sides(8);
    b.paint(METAL);
    b.cylinder_between(breech, muzzle - d * 0.05, r, r * 0.68, sides);
    if !b.fine() {
        return;
    }
    b.paint(ACCENT);
    b.cylinder_between(at(0.05), at(0.42), r * 1.5, r * 1.3, sides);
    b.cylinder_between(at(0.93), muzzle, r * 1.0, r * 1.18, sides);
    // Conduit runs let into the sides of the tube.
    b.paint(PLATING_DARK).pattern(pattern::CONDUIT);
    for y in [-1.0, 1.0] {
        b.beam(at(0.42) + Vec3::Y * (r * 0.95 * y), at(0.88) + Vec3::Y * (r * 0.78 * y), v2(r * 0.5, r * 0.6), v2(r * 0.5, r * 0.5));
    }
    // Emitter rings and the bore.
    b.paint(GLOW);
    for t in [0.80, 0.90] {
        b.cylinder_between(at(t - 0.014), at(t + 0.014), r * 1.06, r * 1.06, 6);
    }
    b.cylinder_between(muzzle - d * 0.06, muzzle + d * 0.02, r * 0.5, r * 0.5, 4);
}

/// A triple plasma gunhouse on its barbette: the faceted house yaws about the
/// pivot, the barrels with their blast bags pitch and recoil inside it.
fn gunhouse(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, base: f32, muzzle_x: f32) {
    b.with_house(weapon, pivot, RECOIL, |b| {
        let cx = pivot.x - 0.8;
        let roof = base + 4.0;
        b.paint(PLATING);
        b.at(v3(cx, 0.0, 0.0), |b| {
            b.loft_z(
                &house_plan(),
                &[
                    Section::new(base + 0.1, 0.94),
                    Section::new(base + 1.0, 1.0),
                    Section::scaled(roof, 0.58, 0.66).shifted(-1.1, 0.0),
                ],
            );
        });
        // A dark frame at the foot of the house, standing proud of the collar.
        b.paint(ACCENT);
        b.at(v3(cx, 0.0, 0.0), |b| {
            b.loft_z(&house_plan(), &[Section::new(base + 0.05, 0.95), Section::new(base + 0.5, 0.97)]);
        });
        let bore_z = pivot.z + 0.5;
        b.with_recoil(|b| {
            for y in BORES {
                let breech = v3(pivot.x + 1.2, y, bore_z);
                let muzzle = v3(muzzle_x, y, bore_z);
                plasma_gun(b, breech, muzzle, 0.5);
                // The blast bag at the root of each barrel.
                b.paint(PLATING);
                b.cylinder_between(breech + Vec3::X * 1.4, breech + Vec3::X * 3.6, 0.8, 0.6, b.sides(8));
            }
            // The mantlet: a dark bar across the face that the bags sit on.
            b.paint(ACCENT);
            b.block(v3(pivot.x + 1.6, -3.4, bore_z - 1.1), v3(pivot.x + 2.9, 3.4, bore_z + 0.9));
        });
        // Team panel on the roof.
        team_panel(b, v3(cx - 1.8, 0.0, roof), v2(2.2, 3.4));
        if !b.fine() {
            return;
        }
        // Rangefinder hood across the back of the roof, lit sights on the front corners.
        b.paint(ACCENT);
        b.chamfered_box(v3(cx - 2.4, 0.0, roof + 0.35), v3(1.2, 6.8, 0.7), 0.25);
        b.paint(GLOW);
        b.mirror_y(|b| b.cuboid(v3(cx - 2.4, 3.48, roof + 0.35), v3(0.7, 0.08, 0.36)));
        b.mirror_y(|b| b.cuboid(v3(cx + 1.6, 2.2, roof + 0.03), v3(1.0, 0.5, 0.07)));
        // Hatches on the roof.
        b.paint(ACCENT);
        b.plate(v3(cx + 0.2, 0.0, roof), v2(1.0, 0.9), 0.06, 0.02);
    });
}

fn fore_battery(b: &mut MeshBuilder) {
    gunhouse(b, 0, FORE_PIVOT, 9.4, FORE_MUZZLE_X);
}

fn second_battery(b: &mut MeshBuilder) {
    gunhouse(b, 1, SECOND_PIVOT, 13.0, SECOND_MUZZLE_X);
}

fn aft_battery(b: &mut MeshBuilder) {
    gunhouse(b, 2, AFT_PIVOT, 9.4, AFT_MUZZLE_X);
}

// ---- the AA mount ----------------------------------------------------------

/// The light twin AA on its tub high on the aft tower.
fn aa_mount(b: &mut MeshBuilder) {
    let (x, z) = (AA_PIVOT.x, AA_PIVOT.z);
    let sides = b.sides(8);
    b.paint(ACCENT);
    b.prism(v3(x, 0.0, AFT_TOWER_TOP - 0.02), sides, 1.7, 1.6, 0.5);
    b.paint(METAL);
    b.prism(v3(x, 0.0, AFT_TOWER_TOP + 0.5), sides, 1.1, 1.05, 0.14);
    b.with_house(3, AA_PIVOT, 0.12, |b| {
        b.paint(PLATING);
        b.at(v3(x - 0.2, 0.0, 0.0), |b| {
            b.loft_z(
                &chamfered_rect(v2(1.0, 0.75), 0.3),
                &[
                    Section::new(AFT_TOWER_TOP + 0.64, 1.0),
                    Section::new(z + 0.3, 1.0),
                    Section::scaled(z + 0.7, 0.8, 0.82).shifted(-0.1, 0.0),
                ],
            );
        });
        b.with_recoil(|b| {
            for y in [-0.35, 0.35] {
                gun_tube(b, v3(x + 0.55, y, z + 0.2), v3(AA_MUZZLE_X, y, z + 0.2), 0.07);
            }
        });
        if b.fine() {
            b.paint(ACCENT);
            b.mirror_y(|b| b.block(v3(x - 0.8, 0.7, AFT_TOWER_TOP + 0.75), v3(x + 0.2, 0.92, z + 0.3)));
            b.block(v3(x - 0.1, -0.14, z + 0.68), v3(x + 0.3, 0.14, z + 0.9));
            b.paint(GLOW_ORANGE);
            for y in [-0.35, 0.35] {
                b.cylinder_between(v3(AA_MUZZLE_X - 0.05, y, z + 0.2), v3(AA_MUZZLE_X + 0.02, y, z + 0.2), 0.04, 0.04, 4);
            }
        }
    });
}

// ---- deck detail at full detail ----------------------------------------------

fn deck_detail(b: &mut MeshBuilder) {
    // The breakwater ahead of the fore house.
    let bw = deck_at(&HULL, 39.5).0;
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.beam(v3(41.5, 0.0, bw + 0.4), v3(38.4, 4.7, bw + 0.4), v2(0.14, 0.8), v2(0.14, 0.8));
    });
    // Guard rails on the quarterdeck and a pulpit at the stem.
    rails(b, &HULL, -56.0, -38.0, 1.0, 0.35);
    // Bollards fore and aft, liferafts along the citadel flanks.
    for x in [47.0, -50.0] {
        let (z, half) = deck_at(&HULL, x);
        b.mirror_y(|b| bollard(b, v3(x, half - 1.0, z + 0.05), 0.6));
    }
    for x in [2.0, -18.0] {
        let (z, half) = deck_at(&HULL, x);
        b.mirror_y(|b| liferaft(b, v3(x, half - 1.1, z + 0.05), 2.4, 0.55));
    }
    // Hawse blocks and the anchor at the bow, a capstan on the quarterdeck.
    b.paint(ACCENT);
    b.mirror_y(|b| b.block(v3(51.0, 2.0, 7.4), v3(52.6, 2.9, 8.2)));
    b.paint(METAL);
    b.cylinder_between(v3(52.6, 0.0, 8.6), v3(53.2, 0.0, 8.6), 0.16, 0.16, 6);
    let qz = deck_at(&HULL, -52.0).0;
    b.paint(ACCENT);
    b.prism(v3(-52.0, 0.0, qz + 0.05), 8, 0.9, 0.8, 0.5);
    b.paint(PLATING);
    b.prism(v3(-52.0, 0.0, qz + 0.55), 8, 0.7, 0.75, 0.45);
    // A sensor dome on the quarterdeck for the aft battery's control.
    b.paint(PLATING);
    b.spheroid(v3(-40.0, 0.0, deck_at(&HULL, -40.0).0 + 0.4), v3(0.9, 0.9, 0.7), 8, 2);
    b.paint(GLOW);
    b.cuboid(v3(-40.0, 0.0, deck_at(&HULL, -40.0).0 + 1.12), v3(0.6, 0.6, 0.08));
    // The transom: a dark panel carrying the propulsor feeds.
    b.paint(PLATING_DARK).pattern(pattern::CONDUIT);
    b.cuboid(v3(-58.05, 0.0, 2.6), v3(0.3, 9.6, 3.2));
    // Vents let into the citadel base roof by the forward tower.
    b.mirror_y(|b| vent(b, v3(-12.8, 4.6, CITADEL_TOP), v2(1.8, 0.7), 4, GLOW));
}
