//! The canyon map's landmark: a colossal straight gravity dam across the gorge's
//! narrows, after the Three Gorges (`mc_map::PropKind::Dam`), built to the plan
//! the baker cuts the ground for, `mc_map::landmark::GORGE_DAM`, so the two meet.
//!
//! Frame: x along the prop's heading is upstream (into the lake), y along the
//! crest, z up; the origin is the middle of the downstream toe on the dry
//! riverbed, which the bake levels at the plan's `floor_z` under the whole
//! footprint. The lake has fallen to dead pool, below the toe's level: nothing
//! runs, the spillway and its stilling basin are dry and stained.
//!
//! What it carries, west (+y) to east: a powerhouse section, penstocks down its
//! face into a long low powerhouse at the toe; the spillway in the middle, a row
//! of tall gate piers on the crest over a smooth chute with deep outlets, into a
//! stilling basin; a second powerhouse section; and at the east end a ship lift's
//! towers. Red and white gantry cranes stand on the crest; nothing crosses it.
//! The concrete's looks (lifts, block joints, rust streaks, the mineral ring up
//! to the old full pool, the stained chute) are scenery.wgsl's `dam_concrete`,
//! chosen by pattern here; the ring's crisp top is an edge of the mesh.

use glam::{Vec2, Vec3};
use mc_map::landmark::GORGE_DAM as PLAN;

use super::builder::MeshBuilder;
use super::library::ModelDef;
use super::material::*;
use crate::gpu_consts::scenery::{
    CONCRETE_CAST as CAST, CONCRETE_CHUTE as CHUTE, CONCRETE_RED as RED, CONCRETE_RING as RING,
    CONCRETE_ROOF as ROOF, CONCRETE_SHADOW as SHADOW, CONCRETE_WET as WET, CONCRETE_WHITE as WHITE,
};

pub(super) const MODELS: &[ModelDef] = &[ModelDef::new("landmark_dam", 760.0, 220.0, dam)];

/// Full-detail triangle budget: one dam on one map, 1.5 km long.
#[cfg(test)]
pub(super) const TRIANGLES: usize = 14000;
/// The deepest the model reaches: the heel, sunk into the lake's bed.
pub(super) const FLOOR: f32 = -((PLAN.floor_z + PLAN.bed) as f32) - 13.0;

/// The crest over the toe.
const H: f32 = (PLAN.crest_z - PLAN.floor_z) as f32;
/// The upstream face, plumb, and where the downstream face's batter meets the crest.
const BASE: f32 = PLAN.base as f32;
const RUN: f32 = (PLAN.base - PLAN.crest_width) as f32;
/// Each end, keyed into the canyon's walls.
const END: f32 = (PLAN.length / 2.0 + PLAN.key) as f32;
/// The lake at dead pool, the dark wet band over it, and the old full pool's line.
const WATER: f32 = -PLAN.floor_z as f32;
const WET_TOP: f32 = WATER + 1.5;
const RING_TOP: f32 = (PLAN.ring_top - PLAN.floor_z) as f32;
const PARAPET: f32 = 1.4;

/// The spillway: its half length, its bays, the piers between them, the gates'
/// sill (the chute's crest) and how far down the face the deep outlets open.
const SPILL_HALF: f32 = 240.0;
const BAYS: usize = 22;
const PIER: f32 = 5.0;
const SILL: f32 = H - 25.0;
const OUTLET: (f32, f32) = (56.0, 68.0);
/// The chute's slope below the ogee (rise per run), and the stilling basin's reach
/// downstream of the toe.
const CHUTE_SLOPE: f32 = 1.52;
const BASIN: f32 = 170.0;
/// Powerhouse sections, (from, to) along the crest, and each unit's width.
const WEST_UNITS: (f32, f32) = (262.0, 696.0);
const EAST_UNITS: (f32, f32) = (-586.0, -262.0);
const UNIT: f32 = 36.0;
/// The powerhouse: its downstream wall and its roof over the toe.
const HOUSE_OUT: f32 = -72.0;
const HOUSE_ROOF: f32 = 46.0;
const PENSTOCK_R: f32 = 6.0;
/// Where the penstocks leave the downstream face.
const PENSTOCK_TOP: f32 = 118.0;
/// The ship lift at the east end: along the crest, and how far downstream.
const LIFT: (f32, f32) = (-696.0, -604.0);
const LIFT_OUT: f32 = -150.0;
const LIFT_TOP: f32 = 192.0;

/// One corner of a cross-section: x (upstream), height, and the pattern of the
/// face from it to the next corner.
#[derive(Clone, Copy)]
struct Pt {
    x: f32,
    z: f32,
    pattern: u32,
}

const fn pt(x: f32, z: f32, pattern: u32) -> Pt {
    Pt { x, z, pattern }
}

/// Sweeps the open polyline `section` along the crest from `y0` to `y1` in
/// `blocks` blocks, one face per edge per block, so each block's faces end at its
/// joints. The section runs round the dam counter-clockwise seen with upstream to
/// the right (up the upstream side, across the top, down the downstream side), so
/// every face's outside is to the edge's right.
fn sweep(b: &mut MeshBuilder, y0: f32, y1: f32, blocks: usize, section: &[Pt]) {
    for i in 0..blocks {
        let s = y0 + (y1 - y0) * i as f32 / blocks as f32;
        let e = y0 + (y1 - y0) * (i + 1) as f32 / blocks as f32;
        for edge in section.windows(2) {
            let (p, q) = (edge[0], edge[1]);
            let normal = Vec3::new(q.z - p.z, 0.0, p.x - q.x);
            let at = |y: f32, c: Pt| Vec3::new(c.x, y, c.z);
            let mut quad = [at(s, p), at(s, q), at(e, q), at(e, p)];
            if (quad[1] - quad[0]).cross(quad[2] - quad[0]).dot(normal) < 0.0 {
                quad.reverse();
            }
            b.paint(CONCRETE).pattern(p.pattern);
            b.face(&quad);
        }
    }
}

/// A flat polygon `outline` (x, z) at `y`, facing along y by the sign of `facing`.
fn cap(b: &mut MeshBuilder, y: f32, outline: &[[f32; 2]], facing: f32, pattern: u32) {
    let mut points: Vec<Vec3> = outline.iter().map(|&[x, z]| Vec3::new(x, y, z)).collect();
    let n = (1..points.len() - 1)
        .map(|i| (points[i] - points[0]).cross(points[i + 1] - points[0]))
        .sum::<Vec3>();
    if n.y * facing < 0.0 {
        points.reverse();
    }
    b.paint(CONCRETE).pattern(pattern);
    b.face(&points);
}

fn blocks(y0: f32, y1: f32, fine: bool) -> usize {
    ((y1 - y0) / if fine { 20.0 } else { 70.0 })
        .round()
        .max(1.0) as usize
}

/// The upstream face from the heel to `top`: drowned concrete, the wet band over
/// the dead pool, the white ring to the old full pool, clean concrete above.
fn upstream_face(top: f32) -> Vec<Pt> {
    vec![
        pt(BASE, FLOOR, CAST),
        pt(BASE, WATER, WET),
        pt(BASE, WET_TOP, RING),
        pt(BASE, RING_TOP, CAST),
        pt(BASE, top, CAST),
    ]
}

/// A non-overflow block: the plumb upstream face, the crest between parapets, the
/// straight downstream batter to the toe and a skirt into the riverbed.
fn gravity_section() -> Vec<Pt> {
    let mut s = upstream_face(H);
    s.extend([
        pt(BASE, H + PARAPET, CAST),
        pt(BASE - 0.7, H + PARAPET, CAST),
        pt(BASE - 0.7, H + 0.2, CAST),
        pt(RUN + 0.7, H + 0.2, CAST),
        pt(RUN + 0.7, H + PARAPET, CAST),
        pt(RUN, H + PARAPET, CAST),
        pt(RUN, H, CAST),
        pt(0.0, 0.0, CAST),
        pt(-0.6, -10.0, CAST),
    ]);
    s
}

/// The spillway chute's surface: the gates' sill, an ogee curving over into a
/// straight run down to the toe's apron.
fn chute(fine: bool) -> Vec<[f32; 2]> {
    let top: &[[f32; 2]] = if fine {
        &[
            [140.0, SILL],
            [132.0, SILL - 1.5],
            [124.0, SILL - 6.0],
            [117.0, SILL - 12.5],
        ]
    } else {
        &[[140.0, SILL], [124.0, SILL - 6.0], [117.0, SILL - 12.5]]
    };
    let [x, z] = top[top.len() - 1];
    let foot = x - (z - 3.0) / CHUTE_SLOPE;
    let mut c = top.to_vec();
    c.extend([[foot, 3.0], [foot - 6.0, 0.6], [0.0, 0.6]]);
    c
}

/// A spillway bay: the upstream face to the sill, the chute, the apron at the toe.
fn spillway_section(fine: bool) -> Vec<Pt> {
    let mut s = upstream_face(SILL);
    let c = chute(fine);
    s.extend(c.iter().map(|&[x, z]| pt(x, z, CHUTE)));
    s.push(pt(0.0, -10.0, CAST));
    s
}

fn dam(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();
    let section = gravity_section();
    for (y0, y1) in [(-END, -SPILL_HALF), (SPILL_HALF, END)] {
        sweep(b, y0, y1, blocks(y0, y1, fine), &section);
    }
    let outline = [
        [BASE, FLOOR],
        [BASE, H],
        [RUN, H],
        [0.0, 0.0],
        [-0.6, -10.0],
    ];
    cap(b, -END, &outline, -1.0, CAST);
    cap(b, END, &outline, 1.0, CAST);
    spillway(b, fine);
    stilling_basin(b, fine);
    for (y0, y1) in [WEST_UNITS, EAST_UNITS] {
        powerhouse(b, y0, y1, fine);
    }
    ship_lift(b, fine);
    for y in [-100.0, 100.0, 380.0, 580.0, -420.0] {
        gantry_crane(b, y, fine);
    }
    if fine {
        // Service houses along the crest, clear of the cranes' rails.
        for y in [-520.0, -330.0, 300.0, 470.0, 650.0] {
            b.paint(CONCRETE).pattern(WHITE);
            b.block(
                Vec3::new(RUN + 3.0, y - 9.0, H + 0.2),
                Vec3::new(RUN + 13.0, y + 9.0, H + 7.0),
            );
            b.paint(CONCRETE).pattern(ROOF);
            b.block(
                Vec3::new(RUN + 2.5, y - 9.5, H + 7.0),
                Vec3::new(RUN + 13.5, y + 9.5, H + 7.8),
            );
        }
    }
}

/// The far level: the wall, the spillway's pier row and the powerhouses as boxes.
fn coarse(b: &mut MeshBuilder) {
    let ring = |y: f32| -> Vec<Vec3> {
        [[BASE, FLOOR], [BASE, H], [RUN, H], [0.0, 0.0]]
            .iter()
            .map(|&[x, z]| Vec3::new(x, y, z))
            .collect()
    };
    b.paint(CONCRETE).pattern(CAST);
    b.loft(&[ring(-END), ring(END)], true, true);
    b.cuboid_open(
        Vec3::new(133.0, 0.0, (SILL + H + 2.0) * 0.5),
        Vec3::new(42.0, SPILL_HALF * 2.0, H + 2.0 - SILL),
    );
    for (y0, y1) in [WEST_UNITS, EAST_UNITS] {
        b.cuboid_open(
            Vec3::new((HOUSE_OUT + 20.0) * 0.5, (y0 + y1) * 0.5, HOUSE_ROOF * 0.5),
            Vec3::new(20.0 - HOUSE_OUT, y1 - y0, HOUSE_ROOF),
        );
    }
    b.paint(CONCRETE).pattern(WHITE);
    b.cuboid_open(
        Vec3::new(LIFT_OUT * 0.5, (LIFT.0 + LIFT.1) * 0.5, LIFT_TOP * 0.5),
        Vec3::new(-LIFT_OUT, LIFT.1 - LIFT.0, LIFT_TOP),
    );
}

/// The spillway: bays of chute between tall piers, closed surface gates at the
/// sill, a deck over the piers for the cranes, divider walls down the chute and
/// the deep outlets' dark mouths half way down it.
fn spillway(b: &mut MeshBuilder, fine: bool) {
    let bay = 2.0 * SPILL_HALF / BAYS as f32;
    let section = spillway_section(fine);
    let c = chute(fine);
    // The chute runs whole under the piers; they stand on it.
    sweep(b, -SPILL_HALF, SPILL_HALF, BAYS, &section);
    for k in 0..BAYS {
        let y0 = -SPILL_HALF + bay * k as f32 + PIER * 0.5;
        let y1 = y0 + bay - PIER;
        // The gate, shut, in its slot.
        b.paint(METAL);
        b.block(Vec3::new(140.5, y0, SILL), Vec3::new(143.5, y1, H - 6.0));
        if fine {
            // The deep outlet's mouth on the chute, dark, with rust bled under it.
            let n = Vec2::new(-CHUTE_SLOPE, 1.0).normalize() * 0.08;
            let on = |y: f32, z: f32| {
                let x = 117.0 - (SILL - 12.5 - z) / CHUTE_SLOPE;
                Vec3::new(x + n.x, y, z + n.y)
            };
            let (w0, w1) = (y0 + 3.5, y1 - 3.5);
            b.paint(CONCRETE).pattern(SHADOW);
            b.face(&[
                on(w0, OUTLET.0),
                on(w1, OUTLET.0),
                on(w1, OUTLET.1),
                on(w0, OUTLET.1),
            ]);
        }
    }
    // Piers, each with a rounded nose upstream, rising over the deck.
    for k in 0..=BAYS {
        let y = -SPILL_HALF + bay * k as f32;
        b.paint(CONCRETE).pattern(CAST);
        b.block(
            Vec3::new(112.0, y - PIER * 0.5, SILL - 14.0),
            Vec3::new(BASE + 4.0, y + PIER * 0.5, H + 3.0),
        );
        if fine {
            b.prism(
                Vec3::new(BASE + 4.0, y, SILL - 14.0),
                8,
                PIER * 0.5,
                PIER * 0.5,
                H + 17.0 - SILL,
            );
            // A divider wall down the chute, 7 m over it.
            let mut wall: Vec<[f32; 2]> = c[2..c.len() - 1]
                .iter()
                .map(|&[x, z]| [x, z + 7.0])
                .collect();
            wall.extend(c[2..c.len() - 1].iter().rev().map(|&[x, z]| [x, z - 1.0]));
            b.paint(CONCRETE).pattern(CAST);
            b.extrude_y(&wall, y - 1.5, y + 1.5);
        }
    }
    // The deck over the piers, and the crane rails' beam under the gates' hoists.
    b.paint(CONCRETE).pattern(CAST);
    b.block(
        Vec3::new(BASE - 12.0, -SPILL_HALF, H - 2.5),
        Vec3::new(BASE + 2.0, SPILL_HALF, H + 0.2),
    );
    b.block(
        Vec3::new(112.0, -SPILL_HALF, H - 2.0),
        Vec3::new(121.0, SPILL_HALF, H + 0.2),
    );
}

/// The stilling basin below the spillway, dry: a stained floor, baffle blocks,
/// an end sill, and training walls parting it from the powerhouses.
fn stilling_basin(b: &mut MeshBuilder, fine: bool) {
    b.paint(CONCRETE).pattern(CHUTE);
    b.block(
        Vec3::new(-BASIN, -SPILL_HALF, -3.0),
        Vec3::new(0.5, SPILL_HALF, 0.5),
    );
    b.block(
        Vec3::new(-BASIN - 8.0, -SPILL_HALF, -3.0),
        Vec3::new(-BASIN, SPILL_HALF, 5.0),
    );
    b.paint(CONCRETE).pattern(CAST);
    for side in [-1.0f32, 1.0] {
        let y = side * (SPILL_HALF + 2.0);
        b.block(
            Vec3::new(-BASIN - 8.0, y - 2.0, -3.0),
            Vec3::new(26.0, y + 2.0, 24.0),
        );
    }
    if fine {
        b.paint(CONCRETE).pattern(CHUTE);
        for (row, x) in [-58.0f32, -82.0].into_iter().enumerate() {
            for k in 0..14 {
                let y = -SPILL_HALF + (k as f32 + 0.5 + 0.5 * row as f32) * 2.0 * SPILL_HALF / 14.5;
                b.block(
                    Vec3::new(x - 3.0, y - 3.5, 0.4),
                    Vec3::new(x + 3.0, y + 3.5, 6.0),
                );
            }
        }
    }
}

/// A powerhouse section from `y0` to `y1`: the long low hall at the toe (a glazed
/// band under its roof, the draft tubes' dark mouths along its foot, transformers
/// on its roof), a penstock down the face into it for every unit, and the units'
/// intakes on the upstream face.
fn powerhouse(b: &mut MeshBuilder, y0: f32, y1: f32, fine: bool) {
    let units = ((y1 - y0) / UNIT).floor() as usize;
    let section = [
        pt(RUN * HOUSE_ROOF / H + 2.0, HOUSE_ROOF, ROOF),
        pt(HOUSE_OUT + 1.0, HOUSE_ROOF, CAST),
        pt(HOUSE_OUT + 1.0, HOUSE_ROOF + 1.5, CAST),
        pt(HOUSE_OUT, HOUSE_ROOF + 1.5, CAST),
        pt(HOUSE_OUT, -6.0, CAST),
    ];
    sweep(b, y0, y1, if fine { units } else { 1 }, &section);
    let outline = [
        [RUN * HOUSE_ROOF / H + 2.0, -6.0],
        [HOUSE_OUT, -6.0],
        [HOUSE_OUT, HOUSE_ROOF + 1.5],
        [HOUSE_OUT + 1.0, HOUSE_ROOF + 1.5],
        [HOUSE_OUT + 1.0, HOUSE_ROOF],
        [RUN * HOUSE_ROOF / H + 2.0, HOUSE_ROOF],
    ];
    cap(b, y0, &outline, -1.0, CAST);
    cap(b, y1, &outline, 1.0, CAST);
    // One long glazed band under the eaves, broken only by each unit's pilaster.
    let x = HOUSE_OUT - 0.1;
    for u in 0..units {
        let u0 = y0 + (y1 - y0) * u as f32 / units as f32;
        let u1 = y0 + (y1 - y0) * (u + 1) as f32 / units as f32;
        let (a0, a1) = (u0 + 1.2, u1 - 1.2);
        b.paint(WINDOWS);
        b.face(&[
            Vec3::new(x, a1, HOUSE_ROOF - 9.0),
            Vec3::new(x, a0, HOUSE_ROOF - 9.0),
            Vec3::new(x, a0, HOUSE_ROOF - 4.0),
            Vec3::new(x, a1, HOUSE_ROOF - 4.0),
        ]);
        let mid = (u0 + u1) * 0.5;
        if fine {
            b.paint(CONCRETE).pattern(SHADOW);
            b.face(&[
                Vec3::new(x, mid + 7.0, 0.0),
                Vec3::new(x, mid - 7.0, 0.0),
                Vec3::new(x, mid - 7.0, 11.0),
                Vec3::new(x, mid + 7.0, 11.0),
            ]);
            b.paint(METAL);
            b.block(
                Vec3::new(-8.0, mid - 6.0, HOUSE_ROOF),
                Vec3::new(2.0, mid + 6.0, HOUSE_ROOF + 7.0),
            );
            b.paint(CONCRETE).pattern(SHADOW);
            b.face(&[
                Vec3::new(BASE + 0.08, mid - 6.0, 96.0),
                Vec3::new(BASE + 0.08, mid + 6.0, 96.0),
                Vec3::new(BASE + 0.08, mid + 6.0, 116.0),
                Vec3::new(BASE + 0.08, mid - 6.0, 116.0),
            ]);
        }
        penstock(b, mid, fine);
    }
}

/// A penstock for the unit at `y`, out of the downstream face and down it into the
/// powerhouse's roof, with concrete anchor blocks where it emerges and half way.
fn penstock(b: &mut MeshBuilder, y: f32, fine: bool) {
    let n = Vec3::new(-H, 0.0, RUN).normalize();
    let at = |z: f32| Vec3::new(RUN * z / H, y, z) + n * (PENSTOCK_R + 0.6);
    b.paint(METAL);
    b.cylinder_between(
        at(PENSTOCK_TOP),
        at(HOUSE_ROOF - 2.0),
        PENSTOCK_R,
        PENSTOCK_R,
        if fine { 12 } else { 6 },
    );
    if fine {
        b.paint(CONCRETE).pattern(CAST);
        for z in [PENSTOCK_TOP - 4.0, (PENSTOCK_TOP + HOUSE_ROOF) * 0.5] {
            let c = at(z);
            b.block(
                c - Vec3::new(PENSTOCK_R + 2.0, PENSTOCK_R + 1.5, 4.0),
                c + Vec3::new(PENSTOCK_R + 1.0, PENSTOCK_R + 1.5, 4.0),
            );
        }
    }
}

/// A red and white portal crane on the crest at `y`: A-frame legs on rails either
/// side of the crest, twin girders, and its machinery house on top.
fn gantry_crane(b: &mut MeshBuilder, y: f32, fine: bool) {
    let top = H + 34.0;
    let (up, down) = (BASE - 3.0, RUN + 3.0);
    b.paint(CONCRETE).pattern(RED);
    if fine {
        for x in [up, down] {
            for s in [-1.0f32, 1.0] {
                b.beam(
                    Vec3::new(x, y + s * 8.0, H + 0.2),
                    Vec3::new(x, y + s * 3.0, top),
                    Vec2::new(1.8, 1.8),
                    Vec2::new(1.6, 1.6),
                );
            }
            b.block(
                Vec3::new(x - 1.5, y - 9.5, H + 0.2),
                Vec3::new(x + 1.5, y + 9.5, H + 2.4),
            );
        }
        for s in [-1.0f32, 1.0] {
            b.block(
                Vec3::new(down - 6.0, y + s * 3.0 - 1.4, top),
                Vec3::new(up + 6.0, y + s * 3.0 + 1.4, top + 3.2),
            );
        }
    } else {
        for x in [up, down] {
            b.block(
                Vec3::new(x - 1.2, y - 6.0, H + 0.2),
                Vec3::new(x + 1.2, y + 6.0, top),
            );
        }
        b.block(
            Vec3::new(down - 6.0, y - 4.4, top),
            Vec3::new(up + 6.0, y + 4.4, top + 3.2),
        );
    }
    b.paint(CONCRETE).pattern(WHITE);
    b.block(
        Vec3::new(down + 4.0, y - 6.5, top + 3.2),
        Vec3::new(up - 6.0, y + 6.5, top + 12.0),
    );
    if fine {
        b.paint(CONCRETE).pattern(RED);
        b.block(
            Vec3::new(down + 3.6, y - 6.9, top + 11.0),
            Vec3::new(up - 5.6, y + 6.9, top + 12.4),
        );
    }
}

/// The ship lift at the east end, downstream against the dam: four towers round
/// the chamber's shaft, tied by beams, a machine hall across their tops, and the
/// steel chamber lowered to the dry riverbed.
fn ship_lift(b: &mut MeshBuilder, fine: bool) {
    let (y0, y1) = LIFT;
    let xs = [(LIFT_OUT, LIFT_OUT + 28.0), (-38.0, -10.0)];
    let ys = [(y0, y0 + 22.0), (y1 - 22.0, y1)];
    b.paint(CONCRETE).pattern(CAST);
    for &(x0, x1) in &xs {
        for &(a, c) in &ys {
            b.block(Vec3::new(x0, a, -4.0), Vec3::new(x1, c, LIFT_TOP));
            if fine {
                // A setback crown on each tower.
                b.block(
                    Vec3::new(x0 + 2.0, a + 2.0, LIFT_TOP),
                    Vec3::new(x1 - 2.0, c - 2.0, LIFT_TOP + 4.0),
                );
            }
        }
    }
    if fine {
        for z in [48.0, 96.0, 144.0] {
            for &(a, c) in &ys {
                b.block(
                    Vec3::new(LIFT_OUT + 28.0, a + 4.0, z),
                    Vec3::new(-38.0, c - 4.0, z + 5.0),
                );
            }
        }
    }
    b.paint(CONCRETE).pattern(WHITE);
    b.block(
        Vec3::new(LIFT_OUT - 2.0, y0 - 2.0, LIFT_TOP + 4.0),
        Vec3::new(-8.0, y1 + 2.0, LIFT_TOP + 18.0),
    );
    b.paint(CONCRETE).pattern(ROOF);
    b.block(
        Vec3::new(LIFT_OUT - 3.0, y0 - 3.0, LIFT_TOP + 18.0),
        Vec3::new(-7.0, y1 + 3.0, LIFT_TOP + 19.5),
    );
    b.paint(METAL);
    b.block(
        Vec3::new(LIFT_OUT + 32.0, y0 + 26.0, 4.0),
        Vec3::new(-42.0, y1 - 26.0, 20.0),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model, material};

    /// Height of triangle `abc` over `p` in plan, if it covers it.
    fn height_at(a: Vec3, b: Vec3, c: Vec3, p: Vec2) -> Option<f32> {
        let (a2, b2, c2) = (a.truncate(), b.truncate(), c.truncate());
        let d = (b2 - a2).perp_dot(c2 - a2);
        if d.abs() < 1e-6 {
            return None;
        }
        let u = (b2 - p).perp_dot(c2 - p) / d;
        let v = (c2 - p).perp_dot(a2 - p) / d;
        let w = 1.0 - u - v;
        (u >= 0.0 && v >= 0.0 && w >= 0.0).then_some(a.z * u + b.z * v + c.z * w)
    }

    /// Over its non-overflow blocks the dam's concrete top is the plan's
    /// surface: the downstream batter, and the crest between its parapets.
    #[test]
    fn the_wall_follows_the_plan() {
        let model = build_model("landmark_dam").unwrap();
        for mesh in &model.lods {
            for y in [-740.0f32, -300.0, 250.0, 560.0, 755.0] {
                for x in [2.0f32, 40.0, 90.0, 120.0, 140.0] {
                    let top = mesh
                        .indices
                        .chunks(3)
                        .filter_map(|t| {
                            let [v0, v1, v2] = [t[0], t[1], t[2]]
                                .map(|i| Vec3::from(mesh.vertices[i as usize].pos));
                            height_at(v0, v1, v2, Vec2::new(x, y))
                        })
                        .fold(f32::MIN, f32::max);
                    let want = PLAN.surface(x as f64, y as f64).unwrap() as f32;
                    assert!(top >= want - 0.01, "x {x} y {y}: top {top}, plan {want}");
                }
            }
        }
    }

    /// The mineral ring's faces end at the old full pool; the dam reaches down to
    /// its heel and no further.
    #[test]
    fn ring_band_and_heel() {
        let model = build_model("landmark_dam").unwrap();
        let mesh = &model.lods[0];
        let top = mesh
            .vertices
            .iter()
            .filter(|v| v.material == material::CONCRETE && v.surface & 0xFF == RING)
            .map(|v| v.pos[2])
            .fold(f32::MIN, f32::max);
        assert!((top - RING_TOP).abs() < 0.01, "ring top {top}");
        let low = mesh
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min);
        assert!((low - FLOOR).abs() < 0.01, "{low}");
        const { assert!(RING_TOP > WET_TOP && WET_TOP > WATER && SILL > RING_TOP) };
    }
}
