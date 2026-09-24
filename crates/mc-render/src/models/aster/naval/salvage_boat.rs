//! Trawler: salvage boat (tech 1). Unlit but for the red lamp.
//!
//! A stubby 16 m working boat, white over dark like the Skiff: a broad hull, a
//! wheelhouse forward, and over the aft deck the salvage ray on a lattice mast
//! that lies folded down along the deck on passage and stands up when the boat
//! deploys to work. The mast is authored raised and tagged `rig::DEPLOY`; the
//! vertex shader folds it (`entity.wgsl`: what stands aft of x = -5.25 swings
//! forward about the hinge at (-5.2, 0, 1.22), so every piece of the mast and
//! its head lives aft of that line, on a dark hinge trunk that hides the foot
//! either way, with a crutch amidships for it to lie in). The emitter head at
//! the mast top is `reclaimer.emitter` in the unit file. Winches, a crane stub,
//! bollards, fenders, rails and a rub rail; no turret parts.
use super::*;

const HULL: [Station; 6] = [
    station(-7.6, -0.5, [-0.3, 2.0], [0.9, 2.35], [1.6, 2.4]),
    station(-4.0, -1.0, [-0.55, 2.2], [0.85, 2.55], [1.65, 2.6]),
    station(0.5, -1.2, [-0.6, 2.1], [0.9, 2.5], [1.75, 2.55]),
    station(4.5, -1.0, [-0.4, 1.5], [1.1, 2.0], [2.0, 2.1]),
    station(7.0, -0.4, [0.1, 0.6], [1.5, 1.1], [2.35, 1.2]),
    station(8.2, 0.7, [0.9, 0.0], [1.7, 0.0], [2.6, 0.0]),
];

/// The mast's centre line, its foot on the deck and the top of its lattice; the
/// emitter head sits on that (`reclaimer.emitter` in the unit file).
const MAST_X: f32 = -6.0;
const MAST_FOOT: f32 = 1.6;
const MAST_TOP: f32 = 6.3;
const HEAD: Vec3 = Vec3::new(-6.0, 0.0, 7.0);
const HOUSE_X: f32 = 3.2;

/// The salvage ray's mast and head: a tapering lattice of four dark legs with white
/// rungs, a white cap, the dark emitter housing and its dish looking aft and down.
fn mast(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(ACCENT);
        b.beam(v3(MAST_X, 0.0, MAST_FOOT), HEAD, v2(0.6, 0.6), v2(0.45, 0.45));
        return;
    }
    let (foot, top) = (0.32, 0.2);
    b.paint(ACCENT);
    for (sy, sx) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)] {
        b.beam(
            v3(MAST_X + sx * foot, sy * foot, MAST_FOOT),
            v3(MAST_X + sx * top, sy * top, MAST_TOP),
            v2(0.12, 0.12),
            v2(0.09, 0.09),
        );
    }
    // Rungs at two heights, and (fine) a diagonal in each side bay.
    let rung = |b: &mut MeshBuilder, z: f32| {
        let h = foot + (top - foot) * (z - MAST_FOOT) / (MAST_TOP - MAST_FOOT);
        b.paint(PLATING);
        for (a, e) in [
            (v3(MAST_X - h, -h, z), v3(MAST_X + h, -h, z)),
            (v3(MAST_X - h, h, z), v3(MAST_X + h, h, z)),
            (v3(MAST_X - h, -h, z), v3(MAST_X - h, h, z)),
            (v3(MAST_X + h, -h, z), v3(MAST_X + h, h, z)),
        ] {
            b.beam(a, e, v2(0.07, 0.07), v2(0.07, 0.07));
        }
    };
    if b.fine() {
        for z in [2.9, 4.2, 5.4] {
            rung(b, z);
        }
        b.paint(ACCENT);
        b.mirror_y(|b| {
            for (z0, z1) in [(MAST_FOOT + 0.2, 2.9), (2.9, 4.2), (4.2, 5.4)] {
                let h0 = foot + (top - foot) * (z0 - MAST_FOOT) / (MAST_TOP - MAST_FOOT);
                let h1 = foot + (top - foot) * (z1 - MAST_FOOT) / (MAST_TOP - MAST_FOOT);
                b.beam(v3(MAST_X - h0, h0, z0), v3(MAST_X + h1, h1, z1), v2(0.05, 0.05), v2(0.05, 0.05));
            }
        });
    } else {
        rung(b, 4.0);
    }
    // Cap, neck, the head housing and its dish.
    b.paint(PLATING);
    b.cuboid(v3(MAST_X, 0.0, MAST_TOP + 0.12), v3(0.7, 0.7, 0.26));
    b.paint(METAL);
    b.cylinder_between(v3(MAST_X, 0.0, MAST_TOP + 0.24), v3(MAST_X, 0.0, HEAD.z - 0.25), 0.16, 0.16, b.sides(6));
    b.paint(ACCENT);
    b.chamfered_box(HEAD + Vec3::Z * 0.05, v3(0.95, 0.85, 0.5), 0.12);
    let (a, e) = (HEAD + v3(-0.45, 0.0, -0.02), HEAD + v3(-1.15, 0.0, -0.55));
    b.cylinder_between(a, e, 0.2, 0.6, b.sides(8));
    b.paint(PLATING);
    b.plate(HEAD + Vec3::Z * 0.3, v2(0.7, 0.6), 0.06, 0.02);
    if b.fine() {
        // Claw prongs round the dish's mouth, a sight box on the housing.
        b.paint(ACCENT);
        let d = (e - a).normalize();
        let side = Vec3::Y;
        let up = d.cross(side).normalize();
        for k in 0..3 {
            let ang = k as f32 * std::f32::consts::TAU / 3.0 + std::f32::consts::FRAC_PI_2;
            let r = side * ang.cos() + up * ang.sin();
            b.beam(e + r * 0.5, e + r * 0.62 + d * 0.4, v2(0.1, 0.1), v2(0.06, 0.06));
        }
        b.paint(ACCENT);
        b.block(HEAD + v3(0.2, -0.2, 0.3), HEAD + v3(0.48, 0.2, 0.5));
    }
}

pub(super) fn build(b: &mut MeshBuilder) {
    hull(b, &HULL, &[0, 3, 5]);

    // The hinge trunk the mast stands in, fixed to the deck: wide enough to hide the foot
    // raised and folded (the fold drifts it a metre forward).
    b.paint(ACCENT);
    if b.coarse() {
        b.cuboid_open(v3(-5.55, 0.0, 2.0), v3(2.1, 1.5, 0.9));
    } else {
        b.block(v3(-6.6, -0.72, 1.5), v3(-4.5, 0.72, 2.45));
        b.paint(PLATING);
        b.plate(v3(-5.55, 0.0, 2.45), v2(1.9, 1.3), 0.1, 0.04);
        b.paint(METAL);
        b.cylinder_between(v3(-5.4, -0.85, 2.0), v3(-5.4, 0.85, 2.0), 0.14, 0.14, b.sides(8));
    }
    b.with_deploy(|b| mast(b));

    // Wheelhouse forward: dark sill, white house, dark screen all round, white roof.
    let plan = chamfered_rect(v2(1.6, 1.55), 0.42);
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(v3(HOUSE_X, 0.0, 1.85), v2(3.2, 3.1), v2(2.6, 2.6), 2.65, v2(-0.2, 0.0));
        team_panel(b, v3(HOUSE_X - 0.15, 0.0, 4.5), v2(1.6, 1.5));
        return;
    }
    b.at(v3(HOUSE_X, 0.0, 0.0), |b| {
        b.paint(ACCENT);
        b.loft_z(&chamfered_rect(v2(1.66, 1.61), 0.44), &[Section::new(1.85, 1.0), Section::new(2.15, 1.0)]);
        b.paint(PLATING);
        b.loft_z(&plan, &[Section::new(2.1, 1.0), Section::new(3.55, 0.97)]);
        b.paint(GLASS);
        b.loft_z(&plan, &[Section::new(3.55, 0.97), Section::scaled(4.25, 0.88, 0.9).shifted(-0.15, 0.0)]);
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[Section::scaled(4.25, 0.9, 0.92).shifted(-0.13, 0.0), Section::scaled(4.5, 0.86, 0.88).shifted(-0.17, 0.0)],
        );
    });
    team_panel(b, v3(HOUSE_X - 0.2, 0.0, 4.5), v2(1.6, 1.5));
    // A short mast on the roof with the red lamp, and the exhaust beside the house.
    b.paint(ACCENT);
    b.prism(v3(HOUSE_X - 0.75, 0.0, 4.5), b.sides(6), 0.11, 0.08, 1.0);
    beacon(b, v3(HOUSE_X - 0.75, 0.0, 5.5));
    b.paint(PLATING);
    b.cylinder_between(v3(HOUSE_X - 1.5, -1.25, 3.4), v3(HOUSE_X - 2.4, -1.35, 4.4), 0.17, 0.15, b.sides(8));
    b.paint(ACCENT);
    b.cylinder_between(v3(HOUSE_X - 2.35, -1.35, 4.35), v3(HOUSE_X - 2.45, -1.36, 4.47), 0.19, 0.19, b.sides(8));

    // Decks: non-skid aft and forward, the rub rail, the crutch the folded mast lies in.
    walkway(b, &HULL, -7.3, 1.2, 0.22);
    walkway(b, &HULL, 5.2, 7.8, 0.2);
    rub_rail(b, &HULL, -7.6, 8.0, 0.18);
    b.paint(ACCENT);
    b.block(v3(-1.15, -0.14, 1.72), v3(-0.85, 0.14, 2.7));
    b.mirror_y(|b| b.block(v3(-1.15, 0.14, 2.5), v3(-0.85, 0.42, 3.0)));
    // Winches: a drum on dark cheeks each side of the aft deck, a capstan at the stern.
    b.mirror_y(|b| {
        let z = deck_at(&HULL, -3.2).0;
        b.paint(ACCENT);
        for y in [0.55, 1.95] {
            b.block(v3(-3.65, y - 0.08, z), v3(-2.75, y + 0.08, z + 0.75));
        }
        b.paint(METAL);
        b.cylinder_between(v3(-3.2, 0.63, z + 0.5), v3(-3.2, 1.87, z + 0.5), 0.36, 0.36, b.sides(8));
        b.paint(PLATING_DARK);
        b.cylinder_between(v3(-3.2, 0.72, z + 0.5), v3(-3.2, 1.78, z + 0.5), 0.4, 0.4, b.sides(8));
    });
    b.paint(METAL);
    b.prism(v3(-6.9, 1.35, deck_at(&HULL, -6.9).0), b.sides(8), 0.3, 0.28, 0.6);
    // Crane stub on the port quarter of the working deck: a post and a short jib.
    let cz = deck_at(&HULL, 0.3).0;
    b.paint(ACCENT);
    b.cylinder_between(v3(0.3, -1.75, cz), v3(0.3, -1.75, cz + 1.9), 0.16, 0.13, b.sides(8));
    b.paint(PLATING);
    b.beam(v3(0.3, -1.75, cz + 1.75), v3(-1.3, -1.75, cz + 2.3), v2(0.2, 0.22), v2(0.14, 0.14));

    if !b.fine() {
        return;
    }
    // Fenders over the side, bollards, rails, a hatch on the foredeck, the anchor.
    b.paint(TREAD);
    b.mirror_y(|b| {
        for x in [-5.2, -2.2, 0.8] {
            let (z, half) = deck_at(&HULL, x);
            b.cylinder_between(v3(x, half + 0.1, z - 0.15), v3(x, half + 0.1, z - 0.7), 0.16, 0.16, 6);
        }
    });
    for x in [6.4, -6.9] {
        let (z, half) = deck_at(&HULL, x);
        b.mirror_y(|b| bollard(b, v3(x, half - 0.32, z + 0.05), 0.3));
    }
    rails(b, &HULL, -7.2, -0.3, 0.6, 0.14);
    rails(b, &HULL, 5.0, 7.7, 0.55, 0.12);
    b.paint(PLATING);
    b.plate(v3(6.2, 0.0, deck_at(&HULL, 6.2).0 + 0.05), v2(0.8, 0.8), 0.06, 0.03);
    b.paint(METAL);
    b.cylinder_between(v3(7.6, 0.0, 2.4), v3(8.0, 0.0, 2.2), 0.06, 0.06, 4);
    // A cable from the crane's jib, a hook on it; a hose reel on the house's back.
    b.paint(ACCENT);
    b.cylinder_between(v3(-1.3, -1.75, cz + 2.2), v3(-1.3, -1.75, cz + 1.2), 0.03, 0.03, 4);
    b.block(v3(-1.4, -1.85, cz + 1.0), v3(-1.2, -1.65, cz + 1.2));
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(HOUSE_X - 1.7, 0.5, 2.9), v3(HOUSE_X - 1.7, 1.2, 2.9), 0.32, 0.32, b.sides(8));
    whip(b, v3(HOUSE_X - 1.0, 0.9, 4.5), 1.1, 0.08);
    // Screen wipers: a lip along the screen's foot.
    b.paint(ACCENT);
    b.block(v3(HOUSE_X + 1.35, -1.0, 3.5), v3(HOUSE_X + 1.5, 1.0, 3.55));
}
