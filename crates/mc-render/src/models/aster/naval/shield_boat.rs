//! Nautilus: shield boat (tech 2). Unarmed.
//!
//! A 32 m round-shouldered, beamy hull carrying a shield generator to sea: a
//! big faceted emitter ring of angled white plates standing on raked struts
//! over the deck amidships, a blue crystal core turning under it on a dark
//! pedestal, transformer and coil blocks fore and aft with conduit feeds
//! running to the ring, a small faceted bridge forward and a low engine
//! casing aft. It echoes the tech 2 shield structure's language: white
//! column, blue faceted crystal in a dark cage, a wreath of lit pods.
use super::*;
use std::f32::consts::FRAC_PI_4;

const HULL: [Station; 7] = [
    station(-15.5, -1.0, [-0.7, 3.2], [1.4, 3.9], [3.0, 3.95]),
    station(-10.0, -2.4, [-1.6, 3.9], [1.3, 4.5], [3.05, 4.6]),
    station(-2.0, -3.0, [-2.0, 4.0], [1.3, 4.65], [3.1, 4.75]),
    station(6.0, -2.9, [-1.9, 3.7], [1.4, 4.4], [3.25, 4.5]),
    station(11.5, -2.2, [-1.3, 2.4], [1.7, 3.2], [3.55, 3.4]),
    station(14.5, -1.2, [-0.5, 1.0], [2.1, 1.7], [3.9, 1.9]),
    station(16.0, 1.0, [1.4, 0.0], [2.6, 0.0], [4.2, 0.0]),
];

/// The generator's axis, its pedestal top, and the ring's centre, mid radius and half sizes.
const GEN_X: f32 = -1.5;
const PEDESTAL_TOP: f32 = 4.9;
const RING_Z: f32 = 9.6;
const RING_R: f32 = 4.2;
const RING_T: f32 = 0.8;
const RING_H: f32 = 0.95;
const BRIDGE_X: f32 = 9.0;
const CASING_X: f32 = -10.75;

fn ring(sides: usize, radius: f32, z: f32) -> Vec<Vec3> {
    ngon(sides, radius).into_iter().map(|[x, y]| v3(GEN_X + x, y, z)).collect()
}

/// A strip laid on the deck from `x0` to `x1`, `half` wide, following the sheer.
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

/// A transformer block on the deck at `x`: a dark chamfered bunker, coil drums across
/// its top under a white lid, and a conduit trunk running `toward` the pedestal.
fn coil_block(b: &mut MeshBuilder, x: f32, toward: f32) {
    let z = deck_at(&HULL, x).0;
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(x, 0.0, z + 0.85), v3(2.4, 5.0, 1.7), 0.3);
    b.paint(ACCENT);
    b.block(v3(x - 1.25, -2.55, z - 0.05), v3(x + 1.25, 2.55, z + 0.4));
    b.paint(METAL);
    let sides = b.sides(10);
    for dx in [-0.72, 0.0, 0.72] {
        b.cylinder_between(v3(x + dx, -2.1, z + 1.7), v3(x + dx, 2.1, z + 1.7), 0.34, 0.34, sides);
    }
    b.paint(PLATING);
    b.mirror_y(|b| b.plate(v3(x, 1.3, z + 2.04), v2(2.2, 1.3), 0.1, 0.04));
    // The feed to the pedestal: a dark trunk with power running down it.
    b.paint(PLATING_DARK).pattern(pattern::CONDUIT);
    let (x0, x1) = if toward > x { (x + 1.2, toward) } else { (toward, x - 1.2) };
    b.block(v3(x0, -0.9, z + 0.05), v3(x1, 0.9, z + 0.95));
    if b.fine() {
        b.paint(ACCENT);
        b.mirror_y(|b| {
            for dx in [-0.72, 0.0, 0.72] {
                b.cylinder_between(v3(x + dx, 2.1, z + 1.7), v3(x + dx, 2.3, z + 1.7), 0.4, 0.4, 8);
            }
        });
        b.paint(GLOW);
        b.mirror_y(|b| b.block(v3(x - 0.9, 2.52, z + 0.55), v3(x + 0.9, 2.58, z + 0.85)));
    }
}

pub(super) fn build(b: &mut MeshBuilder) {
    hull(b, &HULL, &[0, 3, 6]);
    b.set_spinner_pivot(v3(GEN_X, 0.0, PEDESTAL_TOP));

    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(v3(BRIDGE_X, 0.0, 3.35), v2(4.8, 4.8), v2(4.0, 3.8), 3.1, v2(-0.3, 0.0));
        team_panel(b, v3(BRIDGE_X - 0.3, 0.0, 6.45), v2(2.0, 2.2));
        b.paint(GLOW);
        b.cuboid_open(v3(GEN_X, 0.0, 6.0), v3(6.2, 6.2, 5.6));
        b.paint(PLATING);
        b.loft(&[ring(6, RING_R + RING_T, RING_Z - 0.3), ring(6, RING_R - RING_T, RING_Z + 0.3)], false, false);
        return;
    }
    let sides = b.sides(12);

    // ---- the generator ------------------------------------------------------------------
    // Dark octagonal pedestal with a coping, the crystal core turning in a cage of ribs.
    b.paint(PLATING_DARK);
    b.loft_z(
        &ngon(8, 3.3),
        &[Section::new(3.0, 1.0).shifted(GEN_X, 0.0), Section::new(PEDESTAL_TOP - 0.3, 0.92).shifted(GEN_X, 0.0)],
    );
    b.paint(ACCENT);
    b.prism(v3(GEN_X, 0.0, PEDESTAL_TOP - 0.3), 8, 3.1, 3.05, 0.3);
    b.paint(PLATING);
    b.mirror_y(|b| b.plate(v3(GEN_X, 1.85, PEDESTAL_TOP), v2(3.6, 1.4), 0.1, 0.04));
    b.paint(METAL);
    b.prism(v3(GEN_X, 0.0, PEDESTAL_TOP), 8, 1.7, 1.6, 0.35);
    b.with_part(part::SPINNER, |b| {
        b.paint(GLOW);
        b.prism(v3(GEN_X, 0.0, PEDESTAL_TOP + 0.35), 6, 1.35, 1.05, RING_Z - RING_H - PEDESTAL_TOP - 1.0);
        b.prism(v3(GEN_X, 0.0, RING_Z - RING_H - 0.75), 6, 0.9, 0.55, 1.6);
    });
    b.at(v3(GEN_X, 0.0, 0.0), |b| {
        b.paint(ACCENT);
        b.radial(4, |b| {
            b.beam(v3(1.75, 0.0, PEDESTAL_TOP + 0.3), v3(1.35, 0.0, RING_Z - RING_H - 0.4), v2(0.34, 0.26), v2(0.24, 0.2));
        });
        // Four raked struts from the pedestal's shoulders up to the ring's underside.
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                b.paint(ACCENT);
                b.beam(v3(2.9, 0.0, PEDESTAL_TOP - 0.2), v3(RING_R, 0.0, RING_Z - RING_H + 0.15), v2(0.55, 0.45), v2(0.36, 0.3));
                b.paint(PLATING);
                b.beam(v3(3.0, 0.0, PEDESTAL_TOP + 0.05), v3(RING_R - 0.1, 0.0, RING_Z - RING_H), v2(0.3, 0.16), v2(0.2, 0.12));
            });
        });
    });
    // The emitter ring: a diamond section, white facets above, dark below.
    let (outer, inner, mid) = (RING_R + RING_T, RING_R - RING_T, RING_R);
    b.paint(PLATING);
    b.loft(
        &[ring(sides, outer, RING_Z), ring(sides, mid, RING_Z + RING_H), ring(sides, inner, RING_Z)],
        false,
        false,
    );
    b.paint(ACCENT);
    b.loft(
        &[ring(sides, inner, RING_Z), ring(sides, mid, RING_Z - RING_H), ring(sides, outer, RING_Z)],
        false,
        false,
    );
    // Emitter pods round the ridge: the lit wreath of the shield structure, at sea.
    b.at(v3(GEN_X, 0.0, 0.0), |b| {
        b.radial(6, |b| {
            b.paint(METAL);
            b.prism(v3(mid, 0.0, RING_Z + RING_H - 0.15), b.sides(6), 0.42, 0.34, 0.45);
            b.paint(GLOW);
            b.prism(v3(mid, 0.0, RING_Z + RING_H + 0.3), b.sides(6), 0.28, 0.16, 0.3);
        });
    });

    // ---- transformer blocks fore and aft, feeding the ring ---------------------------------
    coil_block(b, 4.7, GEN_X + 3.0);
    coil_block(b, -6.4, GEN_X - 3.0);

    // ---- bridge forward ------------------------------------------------------------------
    let bridge = [[2.4, -1.5], [2.4, 1.5], [1.2, 2.4], [-2.4, 2.4], [-2.4, -2.4], [1.2, -2.4]];
    b.at(v3(BRIDGE_X, 0.0, 0.0), |b| {
        b.paint(ACCENT);
        b.loft_z(&bridge, &[Section::new(3.3, 1.02), Section::new(3.8, 1.02)]);
        b.paint(PLATING);
        b.loft_z(&bridge, &[Section::new(3.8, 1.0), Section::scaled(5.6, 0.94, 0.9)]);
        b.paint(GLASS);
        b.loft_z(&bridge, &[Section::scaled(5.6, 0.94, 0.9), Section::scaled(6.2, 0.86, 0.82)]);
        b.paint(PLATING);
        b.loft_z(&bridge, &[Section::scaled(6.2, 0.88, 0.84), Section::scaled(6.5, 0.84, 0.8)]);
    });
    team_panel(b, v3(BRIDGE_X - 0.5, 0.0, 6.5), v2(2.0, 2.2));
    // A short sensor mast on the bridge roof.
    b.paint(PLATING);
    b.prism(v3(BRIDGE_X - 1.3, 0.0, 6.5), b.sides(6), 0.32, 0.2, 1.9);
    b.paint(ACCENT);
    b.prism(v3(BRIDGE_X - 1.3, 0.0, 8.4), b.sides(6), 0.28, 0.22, 0.2);

    // ---- engine casing aft, low exhausts ---------------------------------------------------
    let casing = chamfered_rect(v2(2.25, 2.7), 0.6);
    b.at(v3(CASING_X, 0.0, 0.0), |b| {
        b.paint(ACCENT);
        b.loft_z(&casing, &[Section::new(2.95, 1.02), Section::new(3.35, 1.02)]);
        b.paint(PLATING);
        b.loft_z(&casing, &[Section::new(3.35, 1.0), Section::scaled(4.7, 0.95, 0.92)]);
        b.paint(ACCENT);
        b.loft_z(&casing, &[Section::scaled(4.7, 0.96, 0.93), Section::scaled(4.85, 0.96, 0.93)]);
    });
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(CASING_X, 0.0, 4.85), v2(3.2, 3.6), 0.08, 0.03);
    b.mirror_y(|b| {
        let (a, e) = (v3(CASING_X - 1.6, 1.4, 4.7), v3(CASING_X - 3.3, 1.5, 5.25));
        let d = (e - a).normalize();
        b.paint(PLATING);
        b.cylinder_between(a, e, 0.42, 0.36, b.sides(8));
        b.paint(ACCENT);
        b.cylinder_between(e - d * 0.35, e + d * 0.05, 0.4, 0.4, b.sides(8));
        if b.fine() {
            b.paint(GLOW_ORANGE);
            b.cylinder_between(e - d * 0.02, e + d * 0.06, 0.28, 0.28, 8);
        }
    });

    // ---- decks ---------------------------------------------------------------------------
    walkway(b, &HULL, 11.6, 15.4, 0.25);
    walkway(b, &HULL, -8.0, 3.2, 0.3);
    walkway(b, &HULL, -15.2, -13.2, 0.25);
    rub_rail(b, &HULL, -15.5, 15.8, 0.24);
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    deck_strip(b, 12.0, 14.6, 0.7);

    if !b.fine() {
        return;
    }
    // ---- fine kit ---------------------------------------------------------------------------
    // Lit sensor panels on the bridge, antennae, ESM dome, whips.
    b.paint(GLOW);
    b.block(v3(BRIDGE_X + 2.28, -0.5, 4.5), v3(BRIDGE_X + 2.4, 0.5, 4.8));
    b.mirror_y(|b| b.block(v3(BRIDGE_X - 1.5, 2.26, 4.4), v3(BRIDGE_X - 0.3, 2.38, 4.7)));
    antenna(b, v3(BRIDGE_X - 1.3, 0.0, 8.6), 1.4, 0.1);
    b.paint(PLATING);
    b.spheroid(v3(BRIDGE_X + 0.9, 0.0, 6.5), v3(0.35, 0.35, 0.32), 8, 3);
    whip(b, v3(BRIDGE_X - 2.0, 1.4, 6.5), 1.8, 0.08);
    whip(b, v3(CASING_X + 1.6, -1.8, 4.85), 1.6, 0.1);
    // Cage ribs and slit windows onto the crystal, as on the structure.
    b.at(v3(GEN_X, 0.0, 0.0), |b| {
        b.radial(4, |b| {
            b.paint(GLASS);
            b.prism(v3(1.55, 0.0, PEDESTAL_TOP + 1.2), 4, 0.3, 0.26, 1.6);
        });
    });
    // Liferafts on the casing's sides, a vent each side of the pedestal, bollards, rails.
    for x in [CASING_X - 0.8, CASING_X + 0.6] {
        b.mirror_y(|b| liferaft(b, v3(x, 2.05, 4.85), 0.9, 0.28));
    }
    b.mirror_y(|b| vent(b, v3(GEN_X, 3.9, deck_at(&HULL, GEN_X).0 + 0.05), v2(1.4, 0.7), 3, METAL));
    for x in [13.4, -14.6] {
        let (z, half) = deck_at(&HULL, x);
        b.mirror_y(|b| bollard(b, v3(x, half - 0.55, z + 0.05), 0.42));
    }
    rails(b, &HULL, 11.8, 15.2, 0.8, 0.16);
    rails(b, &HULL, -15.1, -13.3, 0.8, 0.16);
    rails(b, &HULL, -7.8, 3.0, 0.8, 0.16);
    // The anchor windlass and hawse pipes.
    let wz = deck_at(&HULL, 13.6).0;
    b.paint(METAL);
    b.cylinder_between(v3(13.6, -0.6, wz + 0.3), v3(13.6, 0.6, wz + 0.3), 0.26, 0.26, 8);
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.beam(v3(13.6, 0.35, wz + 0.1), v3(14.7, 0.85, wz + 0.05), v2(0.12, 0.1), v2(0.12, 0.1));
        b.cylinder_between(v3(14.8, 0.95, wz + 0.05), v3(15.1, 1.15, wz - 0.4), 0.18, 0.18, 6);
    });
    // Capacitor banks on the pedestal's flanks, lit.
    b.at(v3(GEN_X, 0.0, 0.0), |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                b.paint(ACCENT);
                b.prism(v3(3.35, 0.0, 3.1), 6, 0.55, 0.5, 1.4);
                b.paint(GLOW);
                b.prism(v3(3.35, 0.0, 4.5), 6, 0.36, 0.24, 0.22);
            });
        });
    });
}
