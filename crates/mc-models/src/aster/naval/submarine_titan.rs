//! The Megalodon, the tier 4 experimental submarine (`aster_t4_submarine`, 110 m).
//!
//! A blended manta wing 44 m across, riding low, with a boomer's missile hump standing out
//! of it down the middle ([`build`]). What the unit file places, it places: six heavy
//! torpedo doors in a raked bow plane at x = 52 ([`TUBES`]), four big square AEB strike
//! hatches on the hump aft of the sail with their door tops at the muzzles ([`AEB`]), the
//! nuclear silos' round lids (the boat's own warheads) that read apart from them, two rail
//! turrets on houses of their own on the hump (weapons 3 and 4, [`RAILS`]; the aft one
//! `rear`, authored facing forward), the interceptor tube doors in the transom
//! ([`INTERCEPTORS`]) and the hull shield's projector (`set_shield_emitter`). Kraken
//! lineage: black anechoic tiles, a white sail, dark doors. Nothing is lit but a ship's own
//! sidelights and the shield lens; the rails are bare hardware (`parts::rail_gun`).
//!
//! x forward, y left, z up; origin at the waterline, the keel about 7 m down.
use std::f32::consts::{FRAC_PI_2, TAU};

use super::*;

/// The bow is one raked plane through the tube row: x = BOW_X + BOW_RAKE * (z - TUBE_Z).
const BOW_X: f32 = 52.0;
const TUBE_Z: f32 = -2.3;
const BOW_RAKE: f32 = 0.12;
/// Weapon 0's muzzles: the six heavy tube doors.
const TUBES: [[f32; 3]; 6] = [
    [52.0, -1.4, -1.6],
    [52.0, 1.4, -1.6],
    [52.0, -1.4, -3.0],
    [52.0, 1.4, -3.0],
    [52.0, -2.8, -2.3],
    [52.0, 2.8, -2.3],
];
const TUBE_R: f32 = 0.6;
/// Weapon 1's muzzles: the AEB hatches' centres, their doors' tops at `AEB_TOP`.
const AEB: [[f32; 2]; 4] = [[-8.0, -3.0], [-8.0, 3.0], [-14.0, -3.0], [-14.0, 3.0]];
const AEB_TOP: f32 = 3.1;
/// The AEB hatch door's side, and its dark rim's.
const AEB_DOOR: f32 = 4.5;
const AEB_RIM: f32 = 5.4;
/// Weapon 2's muzzles, in the transom at `STERN_X`.
const STERN_X: f32 = -50.0;
const INTERCEPTORS: [[f32; 3]; 2] = [[-50.0, -2.0, -2.0], [-50.0, 2.0, -2.0]];
/// The deck rail turrets: weapon, pivot. The rails end `RAIL_REACH` ahead of the pivot at
/// `RAIL_Z` (weapon 3's muzzle; weapon 4 is the same turret astern).
const RAILS: [(usize, Vec3); 2] = [
    (3, Vec3::new(26.0, 0.0, 4.0)),
    (4, Vec3::new(-26.0, 0.0, 4.0)),
];
const RAIL_REACH: f32 = 10.0;
const RAIL_Z: f32 = 4.4;

/// A station of a hull: its x, then the left half of its section from the top ridge to
/// the keel as (y, z): ridge, shoulder, chine, tumblehome, keel. Mirrored about the hull's
/// own centreline into a ten-point ring.
type Station = (f32, [[f32; 2]; 5]);

/// The bow section every design ends in, on the bow plane round the tube doors.
const BOW: [[f32; 2]; 5] = [
    [1.6, 0.7],
    [3.4, 0.4],
    [4.0, -1.6],
    [3.7, -3.4],
    [1.8, -4.4],
];

fn bow_x(z: f32) -> f32 {
    BOW_X + BOW_RAKE * (z - TUBE_Z)
}

/// One hull ring about the centreline at `y0`: the ten-point section, or the six-point
/// ridge/chine/keel one. On `bow`, each point lies on the bow plane.
fn ring(s: &Station, y0: f32, bow: bool, simple: bool) -> Vec<Vec3> {
    let (x, p) = s;
    let at = |k: usize, sign: f32| {
        let [y, z] = p[k];
        v3(if bow { bow_x(z) } else { *x }, y0 + sign * y, z)
    };
    let order: &[usize] = if simple { &[0, 2, 4] } else { &[0, 1, 2, 3, 4] };
    let mut r: Vec<Vec3> = order.iter().map(|&k| at(k, 1.0)).collect();
    r.extend(order.iter().rev().map(|&k| at(k, -1.0)));
    r
}

/// Lofts `stations` (stern first) about y = `y0`: every station at full detail, `mid`
/// at the reduced level and `coarse` as six-point rings. On `bow`, the last one lies on
/// the bow plane.
fn hull(
    b: &mut MeshBuilder,
    stations: &[Station],
    y0: f32,
    bow: bool,
    mid: &[usize],
    coarse: &[usize],
) {
    let last = stations.len() - 1;
    let simple = b.coarse();
    let picks: Vec<usize> = if b.fine() {
        (0..=last).collect()
    } else if simple {
        coarse.to_vec()
    } else {
        mid.to_vec()
    };
    let rings: Vec<Vec<Vec3>> = picks
        .iter()
        .map(|&i| ring(&stations[i], y0, bow && i == last, simple))
        .collect();
    b.loft(&rings, true, true);
}

/// A ring round an x-axis line at (y, z).
fn axial(x: f32, y: f32, z: f32, r: f32, sides: usize) -> Vec<Vec3> {
    ngon(sides, r)
        .into_iter()
        .map(|[u, w]| v3(x, y + u, z + w))
        .collect()
}

/// A shrouded pumpjet: an open duct from `x0` back to `x1` round (y, z), drawn in toward
/// the back, with stator vanes and a hub cone at full detail.
fn duct(b: &mut MeshBuilder, x0: f32, x1: f32, y: f32, z: f32, r: f32, sides: usize) {
    let sides = b.sides(sides);
    let d = |x: f32, k: f32| axial(x, y, z, r * k, sides);
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.loft(
        &[
            d(x0, 1.0),
            d(x1, 0.88),
            d(x1, 0.76),
            d(x0, 0.88),
            d(x0, 1.0),
        ],
        false,
        false,
    );
    if b.fine() {
        let mid = (x0 + x1) * 0.5;
        for k in 0..4 {
            let a = k as f32 * TAU / 4.0 + FRAC_PI_2 * 0.5;
            b.beam(
                v3(mid, y, z),
                v3(mid, y + a.cos() * r * 0.84, z + a.sin() * r * 0.84),
                v2(0.08, r * 0.3),
                v2(0.08, r * 0.3),
            );
        }
        b.paint(METAL);
        b.cylinder_between(
            v3(x0 + 0.3, y, z),
            v3(x1 - r * 0.2, y, z),
            r * 0.3,
            r * 0.08,
            6,
        );
    }
}

/// A control surface: a thin diamond section from the root chord to the tip chord, each
/// given by its leading edge and its chord (run back along -x); `n` is the way its
/// thickness `t` (at the root, half that at the tip) lies.
fn fin(
    b: &mut MeshBuilder,
    root: Vec3,
    root_chord: f32,
    tip: Vec3,
    tip_chord: f32,
    n: Vec3,
    t: f32,
) {
    let n = n.normalize();
    let section = |le: Vec3, c: f32, t: f32| {
        vec![
            le,
            le - Vec3::X * (c * 0.4) + n * (t * 0.5),
            le - Vec3::X * c,
            le - Vec3::X * (c * 0.4) - n * (t * 0.5),
        ]
    };
    b.loft(
        &[
            section(root, root_chord, t),
            section(tip, tip_chord, t * 0.5),
        ],
        true,
        true,
    );
}

/// The six tube doors in the bow plane: a dark panel, and each door a steel ring round a
/// dark shutter. One panel below full detail (the hull's bow cap carries the muzzles).
fn bow_doors(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    let n = v3(1.0, 0.0, -BOW_RAKE).normalize();
    let on = |y: f32, z: f32, out: f32| v3(bow_x(z), y, z) + n * out;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.beam(
        on(0.0, TUBE_Z, -0.05),
        on(0.0, TUBE_Z, 0.03),
        v2(7.4, 2.9),
        v2(7.4, 2.9),
    );
    if !b.fine() {
        return;
    }
    for [_, y, z] in TUBES {
        b.paint(METAL);
        b.cylinder_between(on(y, z, 0.0), on(y, z, 0.08), TUBE_R, TUBE_R * 0.95, 8);
        b.paint(PLATING_DARK);
        b.cylinder_between(
            on(y, z, 0.08),
            on(y, z, 0.12),
            TUBE_R * 0.78,
            TUBE_R * 0.78,
            8,
        );
    }
}

/// The interceptor tube doors in the transom plane.
fn stern_doors(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    for [x, y, z] in INTERCEPTORS {
        b.paint(METAL);
        b.cylinder_between(
            v3(x + 0.02, y, z),
            v3(x - 0.06, y, z),
            0.62,
            0.6,
            b.sides(8),
        );
        b.paint(ACCENT);
        b.cylinder_between(
            v3(x - 0.06, y, z),
            v3(x - 0.1, y, z),
            0.46,
            0.46,
            b.sides(8),
        );
    }
}

/// The four AEB strike hatches, big square doors on a deck at `deck` (under `AEB_TOP`):
/// a dark rim, a white door in two leaves, and the hinge blocks down each outer side.
fn aeb_hatches(b: &mut MeshBuilder, deck: f32) {
    for [hx, hy] in AEB {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.plate(v3(hx, hy, deck), v2(AEB_RIM, AEB_RIM), 0.05, 0.02);
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.plate(
            v3(hx, hy, deck + 0.05),
            v2(AEB_DOOR, AEB_DOOR),
            AEB_TOP - deck - 0.05,
            0.04,
        );
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.block(
                v3(hx - AEB_DOOR * 0.46, hy - 0.05, AEB_TOP - 0.01),
                v3(hx + AEB_DOOR * 0.46, hy + 0.05, AEB_TOP + 0.02),
            );
            let out = hy.signum() * (AEB_DOOR * 0.5 + 0.2);
            for dx in [-1.4, 1.4] {
                b.block(
                    v3(hx + dx - 0.45, hy + out - 0.2, deck),
                    v3(hx + dx + 0.45, hy + out + 0.2, AEB_TOP - 0.05),
                );
            }
        }
    }
}

/// A nuclear silo lid centred on (x, y) with its top at `top`: a raised collar with a
/// hazard-striped coaming, a dark round lid split across in two leaves with a hinge boss
/// either side. It reads apart from the square AEB doors at every zoom.
fn silo_lid(b: &mut MeshBuilder, x: f32, y: f32, base: f32, top: f32, r: f32) {
    let sides = b.sides(16);
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.prism(v3(x, y, base), sides, r * 1.34, r * 1.2, top - 0.22 - base);
    b.paint(ACCENT).pattern(pattern::HAZARD);
    b.prism(v3(x, y, top - 0.22), sides, r * 1.16, r * 1.08, 0.08);
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.prism(v3(x, y, top - 0.22), sides, r, r * 0.94, 0.22);
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(x - 0.06, y - r * 0.92, top - 0.01),
            v3(x + 0.06, y + r * 0.92, top + 0.03),
        );
        for s in [-1.0, 1.0] {
            b.cylinder_between(
                v3(x - 0.9, y + s * r * 1.12, top - 0.08),
                v3(x + 0.9, y + s * r * 1.12, top - 0.08),
                0.22,
                0.22,
                6,
            );
        }
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.prism(v3(x, y, top), 8, 0.5, 0.42, 0.06);
    }
}

/// The hull shield's projector at `at` (its lens): a steel drum on a short pedestal from
/// `base`, a blue-white lens on top.
fn shield_projector(b: &mut MeshBuilder, at: Vec3, base: f32) {
    b.set_shield_emitter(at);
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.prism(
        v3(at.x, at.y, base),
        b.sides(8),
        0.9,
        0.7,
        at.z - 0.35 - base,
    );
    b.paint(METAL);
    b.prism(v3(at.x, at.y, at.z - 0.35), b.sides(10), 0.85, 0.75, 0.25);
    b.paint(GLOW_SHIELD);
    b.prism(at - Vec3::Z * 0.1, b.sides(8), 0.28, 0.24, 0.1);
}

/// A deck rail turret bound to `weapon`, turning about `pivot`, authored facing +x: a fixed
/// barbette from `deck`, then the angular house on its ring and the rail gun with its
/// mantlet pitching and kicking inside it.
fn rail_house(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, deck: f32) {
    let p = pivot;
    let muzzle = v3(p.x + RAIL_REACH, 0.0, RAIL_Z);
    if b.coarse() {
        // Far off the houses are left to the hump's outline.
        return;
    }
    let sides = b.sides(12);
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.prism(
        v3(p.x, 0.0, deck - 0.3),
        sides,
        4.1,
        3.8,
        p.z - 0.4 - (deck - 0.3),
    );
    b.with_house(weapon, pivot, 0.6, |b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.prism(v3(p.x, 0.0, p.z - 0.4), sides, 3.7, 3.7, 0.3);
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.at(v3(p.x - 0.4, 0.0, 0.0), |b| {
            b.loft_z(
                &turret_plan(8.4, 6.2),
                &[
                    Section::new(p.z - 0.1, 0.94),
                    Section::new(p.z + 0.85, 1.0),
                    Section::scaled(p.z + 1.45, 0.74, 0.76).shifted(-0.5, 0.0),
                ],
            );
        });
        b.with_recoil(|b| {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.chamfered_box(v3(p.x + 3.6, 0.0, RAIL_Z), v3(1.2, 2.2, 1.5), 0.35);
            rail_gun(
                b,
                v3(p.x + 0.8, 0.0, RAIL_Z),
                muzzle,
                v2(0.34, 0.56),
                0.42,
                Emitter::Unlit,
            );
        });
        team_panel(b, v3(p.x - 1.4, 0.0, p.z + 1.45), v2(2.2, 2.6));
        if b.fine() {
            // Rangefinder across the back of the roof, a vent block either cheek.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.chamfered_box(v3(p.x - 3.5, 0.0, p.z + 1.62), v3(0.8, 4.2, 0.36), 0.15);
            b.mirror_y(|b| {
                b.chamfered_box(v3(p.x - 0.6, 2.7, p.z + 0.6), v3(2.4, 0.3, 0.7), 0.1);
            });
        }
    });
}

/// Both rail turrets on a deck at `deck`.
fn rail_turrets(b: &mut MeshBuilder, deck: f32) {
    for (weapon, pivot) in RAILS {
        rail_house(b, weapon, pivot, deck);
    }
}

/// A tall sail on the plan `plan` at `x`, from `z0` to `top`: white plating with a
/// glass band near the top, the owner's colour on its roof, sidelights either flank.
/// `rake` shifts the top section back (m).
fn sail(b: &mut MeshBuilder, plan: &[[f32; 2]], x: f32, z0: f32, top: f32, rake: f32) {
    let h = top - z0;
    let band = z0 + h * 0.72;
    b.paint(PLATING).pattern(pattern::GENERIC);
    if b.coarse() {
        b.at(v3(x, 0.0, 0.0), |b| {
            b.loft_z(
                &[plan[0], plan[2], plan[4], plan[plan.len() - 2]],
                &[
                    Section::new(z0, 1.0),
                    Section::scaled(top, 0.7, 0.8).shifted(-rake, 0.0),
                ],
            );
        });
        return;
    }
    let at = |t: f32| -rake * t;
    let scale = |t: f32| 1.0 - 0.3 * t;
    b.at(v3(x, 0.0, 0.0), |b| {
        let sec = |z: f32, k: f32| {
            let t = (z - z0) / h;
            Section::scaled(z, scale(t), scale(t) * k).shifted(at(t), 0.0)
        };
        b.loft_z(plan, &[sec(z0, 1.08), sec(z0 + 0.8, 1.0), sec(band, 1.0)]);
        b.paint(GLASS);
        b.loft_z(plan, &[sec(band, 0.98), sec(band + 0.5, 0.97)]);
        b.paint(PLATING);
        b.loft_z(plan, &[sec(band + 0.5, 0.98), sec(top, 1.0)]);
    });
    let roof_x = x + at(1.0) + plan[2][0] * scale(1.0) * 0.4;
    let half = plan[2][1] * scale(1.0);
    team_panel(b, v3(roof_x, 0.0, top), v2(plan[0][0] * 0.9, half * 1.2));
    if b.fine() {
        // Sidelights: port red, starboard green, just under the glass.
        let lx = x + at(0.6) + plan[1][0] * scale(0.6) * 0.8;
        let ly = plan[1][1] * scale(0.6) * 0.95;
        b.paint(GLOW_NAV_RED);
        b.cuboid(v3(lx, ly, band - 0.3), v3(0.4, 0.08, 0.2));
        b.paint(GLOW_NAV_GREEN);
        b.cuboid(v3(lx, -ly, band - 0.3), v3(0.4, 0.08, 0.2));
    }
}

/// Masts out of a sail's roof round (x, top): two periscopes, a radar mast with its
/// head and the snorkel block.
fn masts(b: &mut MeshBuilder, x: f32, top: f32) {
    if !b.fine() {
        return;
    }
    b.paint(METAL);
    b.cylinder_between(
        v3(x + 0.6, 0.0, top),
        v3(x + 0.6, 0.0, top + 1.2),
        0.16,
        0.12,
        6,
    );
    b.cylinder_between(
        v3(x - 0.6, 0.3, top),
        v3(x - 0.6, 0.3, top + 0.9),
        0.13,
        0.11,
        6,
    );
    b.cylinder_between(
        v3(x - 1.8, -0.2, top),
        v3(x - 1.8, -0.2, top + 0.8),
        0.13,
        0.13,
        6,
    );
    b.paint(ACCENT);
    b.spheroid(v3(x - 1.8, -0.2, top + 0.95), v3(0.34, 0.34, 0.2), 6, 3);
    b.chamfered_box(v3(x - 3.0, 0.0, top + 0.3), v3(0.9, 0.5, 0.6), 0.15);
}

/// Flank sonar arrays: dark strakes just under the chine between `from` and `to`
/// stations, about y = `y0`.
fn flank_arrays(b: &mut MeshBuilder, stations: &[Station], y0: f32) {
    if !b.fine() {
        return;
    }
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        for w in stations.windows(2) {
            let a = v3(w[0].0, y0 + w[0].1[2][0] + 0.04, w[0].1[2][1] - 0.7);
            let c = v3(w[1].0, y0 + w[1].1[2][0] + 0.04, w[1].1[2][1] - 0.7);
            b.beam(a, c, v2(0.1, 0.9), v2(0.1, 0.9));
        }
    });
}

// ---- the boat -------------------------------------------------------------------------

/// The body: one blended flying wing 44 m across whose edges ride just clear of the water,
/// low over the middle so the hump stands proud of it, a deep keel under the centre.
const BODY: [Station; 9] = [
    (
        STERN_X,
        [
            [2.0, 0.8],
            [3.6, 0.6],
            [4.2, -1.0],
            [3.6, -2.8],
            [2.0, -3.4],
        ],
    ),
    (
        -45.0,
        [
            [2.6, 1.0],
            [9.0, 0.8],
            [18.0, 0.1],
            [9.0, -2.0],
            [2.6, -5.0],
        ],
    ),
    (
        -38.0,
        [
            [2.6, 1.1],
            [10.0, 0.95],
            [22.0, 0.15],
            [10.0, -2.5],
            [2.6, -6.4],
        ],
    ),
    (
        -24.0,
        [
            [2.6, 1.2],
            [10.0, 1.05],
            [21.5, 0.2],
            [10.0, -2.9],
            [2.6, -7.2],
        ],
    ),
    (
        -6.0,
        [
            [2.6, 1.2],
            [9.0, 1.05],
            [18.5, 0.2],
            [9.0, -3.0],
            [2.6, -7.4],
        ],
    ),
    (
        12.0,
        [
            [2.6, 1.2],
            [7.5, 1.0],
            [14.0, 0.2],
            [7.5, -3.0],
            [2.6, -7.2],
        ],
    ),
    (
        28.0,
        [[2.4, 1.1], [5.8, 0.9], [9.5, 0.1], [5.8, -2.8], [2.4, -6.2]],
    ),
    (
        42.0,
        [
            [2.0, 0.9],
            [4.4, 0.7],
            [6.0, -0.6],
            [4.4, -3.0],
            [2.0, -5.0],
        ],
    ),
    (BOW_X, BOW),
];
/// The missile hump down the middle of the wing, in plan: from its prow over the foredeck
/// back to a blunt tail over the pumpjets. Its flat top is `DECK`; its sides flare out and
/// down into the wing (`HUMP_SECTIONS`).
const HUMP: [[f32; 2]; 10] = [
    [47.0, 1.4],
    [40.0, 4.0],
    [22.0, 5.4],
    [-30.0, 5.9],
    [-44.0, 5.2],
    [-44.0, -5.2],
    [-30.0, -5.9],
    [22.0, -5.4],
    [40.0, -4.0],
    [47.0, -1.4],
];
/// The hump's top (under the AEB door tops, `AEB_TOP`).
const DECK: f32 = 3.0;
/// The hump's sections, foot to top: its skirt buried in the wing, a flared shoulder, the
/// deck edge.
const HUMP_SECTIONS: [(f32, f32); 3] = [(0.4, 1.45), (2.2, 1.12), (DECK, 1.0)];
/// The nuclear silos at the hump's after end, behind the aft turret, side by side.
const SILO_X: f32 = -38.0;
const SILO_Y: f32 = 3.0;
/// The sail: a long white knife raked back, grown out of the hump.
const SAIL_PLAN: [[f32; 2]; 7] = [
    [7.4, 0.0],
    [4.6, 1.5],
    [-4.4, 1.6],
    [-7.4, 0.65],
    [-7.4, -0.65],
    [-4.4, -1.6],
    [4.6, -1.5],
];
const SAIL_X: f32 = 9.0;
const SAIL_TOP: f32 = 7.4;
/// The pumpjets along the trailing edge, clear of the hump's skirt: their y.
const JETS: [f32; 4] = [-14.5, -8.6, 8.6, 14.5];

/// The Megalodon: a blended manta wing 44 m across, with a boomer's missile hump standing
/// three metres out of the water down its middle. On the hump, fore to aft: the forward
/// rail turret, the tall raked knife sail with its fairwater planes, the shield projector,
/// the four AEB doors, the aft rail turret and the twin round nuclear silos. Twin canted
/// fins on the wings aft and four pumpjets along the trailing edge.
pub(in crate::aster) fn build(b: &mut MeshBuilder, _tech: u8) {
    b.paint(PLATING_DARK).pattern(pattern::TILES);
    hull(b, &BODY, 0.0, true, &[0, 1, 2, 4, 6, 7, 8], &[0, 3, 8]);

    // The hump: open at its foot, which is buried in the wing.
    b.paint(PLATING_DARK).pattern(pattern::TILES);
    let ring = |plan: &[[f32; 2]], z: f32, k: f32| -> Vec<Vec3> {
        plan.iter().map(|p| v3(p[0], p[1] * k, z)).collect()
    };
    if b.coarse() {
        let plan = [HUMP[1], HUMP[4], HUMP[5], HUMP[8]];
        let (foot, top) = (HUMP_SECTIONS[0], HUMP_SECTIONS[2]);
        b.loft(
            &[ring(&plan, foot.0, foot.1), ring(&plan, top.0, top.1)],
            false,
            true,
        );
    } else {
        let rings: Vec<Vec<Vec3>> = HUMP_SECTIONS
            .iter()
            .map(|&(z, k)| ring(&HUMP, z, k))
            .collect();
        b.loft(&rings, false, true);
    }
    sail(b, &SAIL_PLAN, SAIL_X, DECK - 0.2, SAIL_TOP, 1.4);
    rail_turrets(b, DECK);
    if b.coarse() {
        team_panel(b, v3(SAIL_X - 1.4, 0.0, SAIL_TOP), v2(4.0, 1.4));
        return;
    }
    masts(b, SAIL_X - 1.0, SAIL_TOP);
    // Fairwater planes high on the sail.
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        fin(
            b,
            v3(SAIL_X + 2.4, 1.1, 5.6),
            3.2,
            v3(SAIL_X + 1.0, 5.0, 5.6),
            1.7,
            Vec3::Z,
            0.34,
        )
    });

    aeb_hatches(b, DECK - 0.02);
    for s in [-1.0, 1.0] {
        silo_lid(b, SILO_X, s * SILO_Y, DECK - 0.1, DECK + 0.9, 2.1);
    }
    shield_projector(b, v3(-2.4, 0.0, DECK + 1.2), DECK);

    // The hump's flanks: a white strake along each shoulder, and a dark vent bank
    // either side of the sail.
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        for w in HUMP[1..4].windows(2) {
            let at = |p: [f32; 2]| v3(p[0], p[1] * 1.06 + 0.05, 2.62);
            b.beam(at(w[0]), at(w[1]), v2(0.12, 0.34), v2(0.12, 0.34));
        }
    });
    if b.fine() {
        b.mirror_y(|b| vent(b, v3(SAIL_X - 1.0, 3.6, DECK), v2(6.0, 1.2), 5, ACCENT));
    }

    // Twin fins canted out on the wings aft, four pumpjets along the trailing edge.
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.mirror_y(|b| {
        fin(
            b,
            v3(-34.5, 13.0, 0.7),
            6.0,
            v3(-40.5, 14.6, 5.0),
            2.6,
            v3(0.0, 4.3, -1.6),
            0.34,
        );
    });
    for y in JETS {
        duct(b, -41.0, -46.0, y, -0.6, 1.9, 10);
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        b.cylinder_between(v3(-36.0, y, -0.6), v3(-41.2, y, -0.6), 1.2, 1.5, b.sides(8));
    }
    stern_doors(b);
    bow_doors(b);
    flank_arrays(b, &BODY[4..8], 0.0);

    // Team colour on the wings and the hump's prow.
    team_panel(b, v3(-30.0, 16.0, 0.72), v2(4.0, 3.0));
    team_panel(b, v3(-30.0, -16.0, 0.72), v2(4.0, 3.0));
    if b.fine() {
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(41.0, 0.0, DECK), v2(3.0, 3.6), 0.06, 0.02);
        // A walkway down the hump between the doors, escape trunks, leading-edge strakes.
        b.paint(PLATING).pattern(pattern::WALKWAY);
        b.plate(v3(-11.0, 0.0, DECK), v2(10.0, 1.0), 0.03, 0.01);
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.prism(v3(-19.5, 0.0, DECK), 8, 0.6, 0.55, 0.1);
        b.prism(v3(36.5, 0.0, DECK), 8, 0.6, 0.55, 0.1);
        b.mirror_y(|b| {
            for w in BODY[2..8].windows(2) {
                let a = v3(w[0].0, w[0].1[2][0] - 0.3, w[0].1[2][1] + 0.08);
                let c = v3(w[1].0, w[1].1[2][0] - 0.3, w[1].1[2][1] + 0.08);
                b.beam(a, c, v2(0.5, 0.06), v2(0.5, 0.06));
            }
        });
    }
}

/// The deck rail turrets' rails (weapons 3 and 4), for their charge and heat
/// (`turret_rail`): the breech 0.8 m and the muzzle 10 m ahead of the pivot, rails
/// 0.34 m wide and 0.56 m tall either side of the slot.
pub(crate) const RAIL: crate::TurretRail = crate::TurretRail {
    breech: 0.8,
    muzzle: 10.0,
    rail_y: 0.43,
    rail_top: 0.28,
    arcs: [2.2, 3.5, 4.8, 6.1, 7.4, 8.7],
    arc_half: 0.5,
};
