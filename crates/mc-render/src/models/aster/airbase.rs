//! The Roost: an airbase dug into the ground.
//!
//! Almost nothing stands above the lot. A graphite deck covers the works; in its
//! middle a round shaft, wide enough for a cluster of aircraft side by side, goes
//! down to the lift. Four thick hatch leaves (`part::HATCH`) close it and
//! telescope apart under the deck while aircraft come in
//! (`UnitInstance::deploy`, 0 shut to 1 open). The shaft is the model's pit, so
//! it shows through the opening. At the corners, launch tunnels come up out of
//! the ground: open portals with lit throats, a roof over them, a lane leading
//! away. The sim puts an aircraft at the back of a tunnel and runs it out of the
//! mouth (`tunnels` and `run` in the unit file: the mouths here and there are the
//! same numbers). A low operations house stands on one arm; fuel on the other.
//!
//! Tier 1 has two tunnels; tier 2 opens the other two corners; tier 3 adds
//! floodlight masts at the arms' ends and a search radar on the house. Later
//! tiers' pieces ride as upgrade pieces.

use glam::{Vec2, Vec3};
use std::f32::consts::{FRAC_PI_4, TAU};

use super::parts::*;
use super::structures::kit;
use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, pattern, Pit};

/// Half the square the shaft and its hatch cover (`mc_sim::airbase::SHAFT_HALF`).
const WELL_SQUARE: f32 = 18.5;
/// Height of the shaft's opening plane (`ModelInfo::pit`; entity.wgsl's `AIRBASE_OPEN`):
/// just over the ground, and under everything that closes the hatch.
const SHAFT_OPEN: f32 = 0.3;
/// The lift at the bottom of the shaft: its top is `mc_sim::airbase::SHAFT_FLOOR`.
const FLOOR: f32 = -21.8;
/// The deck's top, and its cross: the arms' half width and the lot's half size.
const DECK: f32 = 1.6;
const ARM: f32 = 22.0;
const LOT: f32 = 36.0;
/// Where the hatch leaves lie, all above the opening: the inner pair level with the
/// deck (they run back into it), the outer pair riding on top of it (they come to
/// rest stacked on the deck's arms, clear over the kerb round the shaft).
const INNER_Z0: f32 = 0.36;
const INNER_Z1: f32 = 1.6;
const OUTER_Z0: f32 = 2.0;
const OUTER_Z1: f32 = 3.1;
/// A launch tunnel, along a diagonal: its portal `MOUTH` out (the mouth in the
/// unit file is `MOUTH / sqrt 2` each way), its roof reaching `HOOD` back from
/// there, open `PORTAL_W` wide and `PORTAL_H` tall inside.
const MOUTH: f32 = 42.0;
const HOOD: f32 = 13.0;
const PORTAL_W: f32 = 18.0;
const PORTAL_H: f32 = 9.0;
const WALL: f32 = 1.2;

pub fn airbase(b: &mut MeshBuilder, tech: u8) {
    // The pit reaches a little past the square shaft's corners: only what is strictly
    // inside it is drawn through the opening. The shader takes the hatch's half width
    // back out of it: `(radius - 0.4) / sqrt 2`.
    b.set_pit(Pit {
        open: SHAFT_OPEN,
        radius: WELL_SQUARE * std::f32::consts::SQRT_2 + 0.4,
        stroke: 0.0,
        section: 0.0,
        rack: [0.0, 0.0],
        afloat_lift: 0.0,
    });
    b.set_spinner_pivot(v3(29.0, 0.0, DECK + 6.2));
    // Tier 1 has the north-east and south-west tunnels; tier 2 the other two.
    let corners = |tier: u8| -> [f32; 2] {
        if tier == 1 {
            [FRAC_PI_4, FRAC_PI_4 + TAU / 2.0]
        } else {
            [FRAC_PI_4 + TAU / 4.0, FRAC_PI_4 + 3.0 * TAU / 4.0]
        }
    };

    if b.coarse() {
        // Far off: the deck's cross as one plate, the leaves, and each tunnel as its
        // roof and the dark portal.
        let (w, l) = (ARM, LOT);
        b.paint(PLATING_DARK);
        b.face(&[
            v3(l, -w, DECK), v3(l, w, DECK), v3(w, w, DECK), v3(w, l, DECK),
            v3(-w, l, DECK), v3(-w, w, DECK), v3(-l, w, DECK), v3(-l, -w, DECK),
            v3(-w, -w, DECK), v3(-w, -l, DECK), v3(w, -l, DECK), v3(w, -w, DECK),
        ]);
        b.with_part(part::HATCH, |b| {
            b.paint(PLATING);
            let (s, h) = (WELL_SQUARE, WELL_SQUARE * 0.5);
            b.mirror_y(|b| {
                b.decal(v3(0.0, h * 0.5, INNER_Z1), v2(s * 2.0, h - 0.3));
                b.decal(v3(0.0, h * 1.5, OUTER_Z1), v2(s * 2.0, h - 0.3));
            });
        });
        let tiers: &[u8] = if tech >= 2 { &[1, 2] } else { &[1] };
        for &tier in tiers {
            for yaw in corners(tier) {
                b.yawed(Vec3::ZERO, yaw, |b| {
                    let half = PORTAL_W * 0.5 + WALL;
                    b.paint(ACCENT);
                    b.decal(v3(MOUTH - HOOD * 0.5, 0.0, PORTAL_H + WALL), v2(HOOD, half * 2.0));
                    b.paint(PLATING_DARK);
                    b.face(&[
                        v3(MOUTH, -half, 0.0),
                        v3(MOUTH, half, 0.0),
                        v3(MOUTH, half, PORTAL_H + WALL),
                        v3(MOUTH, -half, PORTAL_H + WALL),
                    ]);
                });
            }
        }
        return;
    }

    shaft(b);
    b.paint(PLATING_DARK).pattern(pattern::DECK);
    deck(b);
    well_rim(b);
    hatch(b);
    for tier in [1, 2] {
        kit(b, tech, tier, 0.35, |b| {
            for yaw in corners(tier) {
                b.yawed(Vec3::ZERO, yaw, tunnel);
            }
        });
    }
    operations(b);
    stores(b);
    kit(b, tech, 3, 0.6, tier_three);
}

/// The deck: a cross of graphite slabs round the well, its corners left for the tunnels.
fn deck(b: &mut MeshBuilder) {
    let slab = |b: &mut MeshBuilder, lo: Vec2, hi: Vec2| {
        if hi.x > lo.x && hi.y > lo.y {
            b.block(lo.extend(0.0), hi.extend(DECK));
        }
    };
    let (w, l, s) = (ARM, LOT, WELL_SQUARE);
    slab(b, v2(-w, s), v2(w, l));
    slab(b, v2(-w, -l), v2(w, -s));
    slab(b, v2(s, -s), v2(l, s));
    slab(b, v2(-l, -s), v2(-s, s));
    for (x0, x1) in [(w, l), (-l, -w)] {
        slab(b, v2(x0, s), v2(x1, w));
        slab(b, v2(x0, -w), v2(x1, -s));
    }
}

/// Black edging round the well with hazard chevrons: a solid kerb round the top
/// of the shaft, standing proud of the deck.
fn well_rim(b: &mut MeshBuilder) {
    let s = WELL_SQUARE;
    let (w, h) = (1.4, DECK + 0.35);
    b.paint(ACCENT).pattern(pattern::HAZARD);
    for (lo, hi) in [
        (v2(-s - w, s), v2(s + w, s + w)),
        (v2(-s - w, -s - w), v2(s + w, -s)),
        (v2(s, -s), v2(s + w, s)),
        (v2(-s - w, -s), v2(-s, s)),
    ] {
        b.block(lo.extend(DECK), hi.extend(h));
    }
    // A lamp at each corner of the kerb.
    b.paint(GLOW_AMBER);
    for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        b.cuboid(v3(sx * (s + w * 0.5), sy * (s + w * 0.5), h + 0.1), v3(0.7, 0.7, 0.2));
    }
}

/// The shaft down to the hangar, seen through the opening: square, straight down
/// from the hatch's edge, lined with dark plate and lamps, and the lift the
/// aircraft come down onto at the bottom.
fn shaft(b: &mut MeshBuilder) {
    let s = WELL_SQUARE;
    let corner = [v2(s, s), v2(-s, s), v2(-s, -s), v2(s, -s)];
    // Walls face in. Seen from the middle, each side winds as the mine's pit faces do.
    let bands = [(FLOOR, -12.0), (-12.0, -3.0), (-3.0, SHAFT_OPEN)];
    for (i, (z0, z1)) in bands.into_iter().enumerate() {
        b.paint(if i == 1 { ACCENT } else { PLATING_DARK });
        if i == 1 {
            b.pattern(pattern::CONDUIT);
        }
        // Each wall in pieces along its length, as the floor is laid in tiles.
        for k in 0..4 {
            let (p, q) = (corner[k], corner[(k + 1) % 4]);
            for n in 0..6 {
                let (a, c) = (p.lerp(q, n as f32 / 6.0), p.lerp(q, (n + 1) as f32 / 6.0));
                b.face(&[a.extend(z0), a.extend(z1), c.extend(z1), c.extend(z0)]);
            }
        }
    }
    // Lamps down each wall.
    b.paint(GLOW);
    for k in 0..4 {
        let (p, q) = (corner[k], corner[(k + 1) % 4]);
        let inward = -(p + q).normalize() * 0.25;
        for t in [0.2, 0.4, 0.6, 0.8] {
            let at = p.lerp(q, t) + inward;
            for z in [-18.0, -12.5, -7.0] {
                b.cuboid(at.extend(z), v3(0.6, 0.6, 1.4));
            }
        }
    }
    // The lift: a deck over the whole floor, a lit border let into it, landing marks,
    // and the owner's colour. Each piece stands well clear of the one under it: down
    // here depth is drawn squeezed toward the opening, and close layers would fight.
    // Laid as tiles, not one slab: a vertex hidden behind the near wall keeps its true
    // depth, and across one big face that dropped the whole floor behind the ground.
    b.paint(PLATING_DARK).pattern(pattern::DECK);
    let tiles = if b.fine() { 8 } else { 4 };
    let step = 2.0 * s / tiles as f32;
    for i in 0..tiles {
        for j in 0..tiles {
            let at = v2(-s + (i as f32 + 0.5) * step, -s + (j as f32 + 0.5) * step);
            b.decal(at.extend(FLOOR + 0.8), v2(step, step));
        }
    }
    let top = FLOOR + 0.8 + 0.25;
    b.paint(GLOW_AMBER);
    let (o, i) = (s - 1.2, s - 2.0);
    for (lo, hi) in [
        (v2(-o, i), v2(o, o)),
        (v2(-o, -o), v2(o, -i)),
        (v2(i, -i), v2(o, i)),
        (v2(-o, -i), v2(-i, i)),
    ] {
        // Four pieces a side, for the same reason as the floor.
        for k in 0..4 {
            let (a, c) = (lo.lerp(hi, k as f32 / 4.0), lo.lerp(hi, (k + 1) as f32 / 4.0));
            let (a, c) = if (hi - lo).x > (hi - lo).y {
                (v2(a.x, lo.y), v2(c.x, hi.y))
            } else {
                (v2(lo.x, a.y), v2(hi.x, c.y))
            };
            b.decal(((a + c) * 0.5).extend(top), c - a);
        }
    }
    team_panel(b, v3(0.0, 0.0, top), v2(6.0, 6.0));
    b.paint(GLOW_AMBER);
    for k in 0..4 {
        let at = Vec2::from_angle(k as f32 * TAU / 4.0 + FRAC_PI_4) * 10.0;
        b.cuboid(at.extend(top + 0.05), v3(1.2, 1.2, 0.1));
    }
}

/// The hatch: on each side two thick armoured leaves, shut over the square. They
/// telescope apart (`part::HATCH`): the inner one, level with the deck, runs the
/// whole half width back into it; the outer one, on top, half as far, and rests
/// on the deck's arm.
fn hatch(b: &mut MeshBuilder) {
    let s = WELL_SQUARE;
    let split = s * 0.5;
    b.with_part(part::HATCH, |b| {
        b.mirror_y(|b| {
            // Inner leaf: from the middle to the split, riding low.
            b.paint(PLATING).pattern(pattern::HAZARD);
            b.block(v3(-s, 0.05, INNER_Z0), v3(s, split - 0.15, INNER_Z1));
            // Outer leaf: from the split to the edge, over the inner one's path.
            b.paint(PLATING).pattern(pattern::DECK);
            b.block(v3(-s, split + 0.15, OUTER_Z0), v3(s, s, OUTER_Z1));
            // Black leading edges, so each slab shows its thickness as it moves.
            b.paint(ACCENT);
            b.block(v3(-s, 0.05, INNER_Z0 - 0.01), v3(s, 0.7, INNER_Z1 + 0.02));
            b.block(v3(-s, split + 0.15, OUTER_Z0 - 0.01), v3(s, split + 0.8, OUTER_Z1 + 0.02));
            if b.fine() {
                // Seams across the inner leaves; the owner's colour and a lamp on top.
                for x in [-13.0, -6.5, 0.0, 6.5, 13.0] {
                    b.block(v3(x - 0.5, 1.0, INNER_Z1), v3(x + 0.5, split - 1.0, INNER_Z1 + 0.02));
                }
                b.paint(TEAM);
                b.block(v3(-s + 1.0, split + 2.0, OUTER_Z1), v3(-s + 5.0, split + 6.0, OUTER_Z1 + 0.04));
                b.paint(GLOW_RED);
                b.block(v3(-1.2, 0.1, INNER_Z1), v3(1.2, 0.6, INNER_Z1 + 0.12));
            }
        });
    });
}

/// A launch tunnel, built along +x out to its portal at `MOUTH`: an open throat
/// with a lit floor and walls, a roof over it, a black frame with a hazard
/// lintel round the mouth, and a lane out from it.
fn tunnel(b: &mut MeshBuilder) {
    let half = PORTAL_W * 0.5;
    let back = MOUTH - HOOD;
    // Walls and roof: black outside, dark within.
    b.paint(ACCENT).pattern(pattern::DECK);
    for y in [-half - WALL, half] {
        b.block(v3(back, y, 0.0), v3(MOUTH, y + WALL, PORTAL_H));
    }
    b.block(v3(back, -half - WALL, PORTAL_H), v3(MOUTH, half + WALL, PORTAL_H + WALL));
    // Earth banked up against its sides, so it reads as coming out of the ground.
    b.paint(ROCK);
    for side in [-1.0f32, 1.0] {
        let y = side * (half + WALL);
        b.frustum_open(
            v3(back + HOOD * 0.5, y + side * 2.5, 0.0),
            v2(HOOD, 5.0),
            v2(HOOD, 0.4),
            PORTAL_H,
            v2(0.0, -side * 2.3),
        );
    }
    // The back wall, deep in shadow, and a white roof plate with the owner's band.
    b.paint(PLATING_DARK).pattern(pattern::NONE);
    b.block(v3(back, -half, 0.0), v3(back + 0.8, half, PORTAL_H));
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(back + HOOD * 0.5, 0.0, PORTAL_H + WALL), v2(HOOD - 1.0, 3.2), 0.12, 0.05);
    // Inside: a lit floor, and lamps down both walls.
    b.paint(PLATING).pattern(pattern::ROADWAY);
    b.plate(v3(back + HOOD * 0.5 + 0.4, 0.0, 0.0), v2(HOOD - 0.8, PORTAL_W - 0.4), 0.15, 0.04);
    b.paint(GLOW);
    for k in 0..4 {
        let x = back + 2.0 + k as f32 * (HOOD - 3.0) / 3.0;
        for y in [-half + 0.25, half - 0.25] {
            b.cuboid(v3(x, y, PORTAL_H * 0.55), v3(1.4, 0.3, 0.5));
        }
        glow_strip(b, v3(x, 0.0, 0.16), v2(1.0, 0.5), GLOW_AMBER);
    }
    // The mouth: a black frame and a hazard lintel.
    b.paint(ACCENT);
    for y in [-half - WALL - 0.5, half + WALL - 0.4] {
        b.block(v3(MOUTH - 0.2, y, 0.0), v3(MOUTH + 0.9, y + 0.9, PORTAL_H + WALL + 0.6));
    }
    b.paint(ACCENT).pattern(pattern::HAZARD);
    b.block(v3(MOUTH - 0.2, -half - WALL - 0.5, PORTAL_H), v3(MOUTH + 0.9, half + WALL + 0.5, PORTAL_H + WALL + 0.8));
    // The lane: a short roadway out, lights along its edges.
    b.paint(PLATING).pattern(pattern::ROADWAY);
    b.plate(v3(MOUTH + 3.0, 0.0, 0.0), v2(5.2, PORTAL_W - 1.0), 0.12, 0.04);
    if b.fine() {
        for y in [-half + 0.4, half - 0.4] {
            for x in [MOUTH + 1.4, MOUTH + 4.4] {
                glow_strip(b, v3(x, y, 0.13), v2(1.4, 0.4), GLOW_AMBER);
            }
        }
    }
}

/// The operations house on the +x arm: a low white block with a band of windows,
/// a mast, and a lamp on it.
fn operations(b: &mut MeshBuilder) {
    let at = v3(29.0, 0.0, DECK);
    b.paint(PLATING);
    b.cuboid_open(at + v3(0.0, 0.0, 2.4), v3(8.0, 10.0, 4.8));
    b.paint(ACCENT);
    b.block(at + v3(-4.2, -5.2, 4.8), at + v3(4.2, 5.2, 5.3));
    if b.fine() {
        b.paint(WINDOWS);
        b.block(at + v3(-4.1, -5.05, 2.8), at + v3(4.1, -4.95, 4.1));
        b.block(at + v3(-4.05, -5.0, 2.8), at + v3(-3.95, 5.0, 4.1));
        team_panel(b, at + v3(1.0, 2.0, 5.3), v2(4.5, 3.6));
        antenna(b, at + v3(2.6, -3.4, 5.3), 7.0, 0.0);
        vent(b, at + v3(-2.0, 2.2, 5.3), v2(2.8, 2.0), 3, ACCENT);
    }
    // Conduit from the house to the well's edge.
    b.paint(ACCENT).pattern(pattern::CONDUIT);
    b.block(v3(WELL_SQUARE + 1.4, -0.6, DECK), v3(25.0, 0.6, DECK + 0.5));
}

/// Fuel on the -x arm: three tanks on a skid.
fn stores(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.block(v3(-33.0, -6.5, DECK), v3(-23.0, 6.5, DECK + 0.4));
    b.paint(METAL);
    for y in [-4.2, 0.0, 4.2] {
        b.cylinder_between(v3(-32.0, y, DECK + 2.0), v3(-24.0, y, DECK + 2.0), 1.7, 1.7, b.sides(10));
    }
    if b.fine() {
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        for y in [-4.2, 0.0, 4.2] {
            b.cylinder_between(v3(-28.5, y, DECK + 2.0), v3(-27.5, y, DECK + 2.0), 1.8, 1.8, b.sides(10));
        }
    }
}

/// Tier 3: floodlight masts at the arms' ends and a search radar turning on the house.
fn tier_three(b: &mut MeshBuilder) {
    for k in 0..4 {
        let d = Vec2::from_angle(k as f32 * TAU / 4.0);
        let side = Vec2::new(-d.y, d.x);
        for s in [-1.0, 1.0] {
            let foot = (d * (LOT - 2.0) + side * s * (ARM - 3.0)).extend(DECK);
            b.paint(ACCENT);
            b.cylinder_between(foot, foot + Vec3::Z * 10.0, 0.35, 0.25, 6);
            b.paint(GLOW);
            b.cuboid(foot + Vec3::Z * 10.2, v3(1.2, 1.2, 0.5));
        }
    }
    let top = v3(29.0, 0.0, DECK + 5.3);
    b.paint(ACCENT);
    b.cylinder_between(top, top + Vec3::Z * 0.9, 0.5, 0.5, 8);
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING);
        b.block(top + v3(-0.3, -2.6, 0.9), top + v3(0.3, 2.6, 2.2));
        b.paint(ACCENT);
        b.block(top + v3(0.3, -2.4, 1.2), top + v3(0.5, 2.4, 1.9));
    });
}

#[cfg(test)]
mod tests {
    use crate::models::{build_model_fitted, part};

    #[test]
    fn every_roost_tier_fits_its_budgets_and_rigs_its_hatch() {
        for (tech, height) in [(1, 9.0), (2, 10.0), (3, 12.0)] {
            let model = build_model_fitted("airbase", 34.0, height, tech, &[]).expect("airbase model");
            let tris: Vec<usize> = model.lods.iter().map(|l| l.indices.len() / 3).collect();
            assert!(tris[0] >= tris[1] && tris[1] >= tris[2], "T{tech}: {tris:?}");
            assert!(tris[2] < 60 && tris[0] < 6000, "T{tech}: {tris:?}");
            for lod in &model.lods {
                assert!(lod.vertices.iter().any(|v| v.part == part::HATCH), "every LOD has the hatch");
                for v in &lod.vertices {
                    assert!(v.pos.iter().chain(&v.normal).all(|c| c.is_finite()));
                    assert!((glam::Vec3::from(v.normal).length() - 1.0).abs() < 1e-4);
                }
            }
            // The shaft is dug in: its floor is well below the ground.
            let lowest = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(lowest < -20.0, "T{tech}: {lowest}");
        }
    }
}
