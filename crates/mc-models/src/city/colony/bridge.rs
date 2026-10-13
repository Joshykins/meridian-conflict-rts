//! A road bridge over a river (`PropKind::ColonyBridge`): its deck from the origin
//! along +x for `COLONY_BRIDGE_M`, the road `COLONY_BRIDGE_HALF_M` either side of x
//! at z 0 (the map lays its walkable deck there and raises each landing to it), its
//! piers down to the river's bed `COLONY_BRIDGE_DEPTH_M` under the road. Precast
//! prefab girders on cast concrete piers with cutwaters, heavy abutments, parapets
//! over a dark band and lamp standards.

use glam::{Vec2, Vec3};
use mc_map::city::{COLONY_BRIDGE_DEPTH_M, COLONY_BRIDGE_HALF_M, COLONY_BRIDGE_M};

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// The deck's length, the road's half width and the river's bed under the road.
const LEN: f32 = COLONY_BRIDGE_M as f32;
const HALF: f32 = COLONY_BRIDGE_HALF_M as f32;
const BED: f32 = -(COLONY_BRIDGE_DEPTH_M as f32);
/// The parapets' outer faces, either side of x.
const EDGE: f32 = HALF + 1.0;
/// The parapets' top over the road.
const PARAPET: f32 = 1.45;

/// A box girder on two piers: three spans of precast segments with raking webs,
/// cutwater piers on crossheads.
pub(super) fn girder(b: &mut MeshBuilder, _tech: u8) {
    let piers = [40.0, 80.0];
    let soffit = -2.8;
    if !b.mid() {
        coarse(b, soffit, &piers, 0.0);
        return;
    }
    road(b, &piers);
    paint(b, pat::PREFAB);
    let section = [
        [-HALF, -0.05],
        [HALF, -0.05],
        [HALF, -0.6],
        [EDGE, -0.6],
        [EDGE, -1.3],
        [6.0, soffit],
        [-6.0, soffit],
        [-EDGE, -1.3],
        [-EDGE, -0.6],
        [-HALF, -0.6],
    ];
    b.extrude_x(&section, 0.0, LEN);
    parapets(b, -0.6);
    for x in piers {
        pier(b, x, soffit - 1.25, 5.2, 1.6);
        // The crosshead the girder bears on.
        paint(b, pat::CONCRETE);
        b.chamfered_box(v3(x, 0.0, soffit - 0.8), Vec3::new(4.0, 15.0, 0.9), 0.5);
        bearings(b, x, soffit);
    }
    for end in [0.0, LEN] {
        abutment(b, end, -0.6);
    }
    lamps(b, &[15.0, 45.0, 75.0, 105.0]);
}

/// A haunched slab: three spans, the middle one long, the girder deepening in arcs to
/// heavy cutwater piers, so its soffit reads as an arch over the river.
pub(super) fn haunched(b: &mut MeshBuilder, _tech: u8) {
    let piers = [30.0, 90.0];
    let (shallow, deep) = (-1.9, -7.0);
    if !b.mid() {
        coarse(b, deep, &piers, 0.0);
        return;
    }
    road(b, &piers);
    // The elevation: level along the top, its soffit arched between the piers.
    let soffit = |x: f32| -> f32 {
        // Level over each pier's head, 3 m either side of it.
        let f = if x <= piers[0] {
            x / (piers[0] - 3.0)
        } else if x >= piers[1] {
            (LEN - x) / (LEN - piers[1] - 3.0)
        } else {
            let mid = (piers[0] + piers[1]) * 0.5;
            (x - mid).abs() / (mid - piers[0] - 3.0)
        }
        .min(1.0);
        shallow + (deep - shallow) * f.powf(2.2)
    };
    let step = if b.fine() { 2.5 } else { 5.0 };
    let n = (LEN / step) as usize;
    let mut profile = vec![[0.0, -0.6], [LEN, -0.6]];
    for k in (0..=n).rev() {
        let x = LEN * k as f32 / n as f32;
        profile.push([x, soffit(x)]);
    }
    paint(b, pat::PREFAB);
    b.extrude_y(&profile, -EDGE, EDGE);
    // The road's slab between the parapets.
    boxed(b, v3(0.0, -HALF, -0.6), v3(LEN, HALF, -0.05), pat::CONCRETE);
    parapets(b, -0.6);
    // A dark drip band along the soffit's edge, either side.
    if b.fine() {
        for s in [-1.0f32, 1.0] {
            paint(b, pat::TRIM);
            b.extrude_x(
                &[
                    [s * (EDGE + 0.05), -0.6],
                    [s * (EDGE + 0.05), -1.0],
                    [s * (EDGE - 0.05), -1.0],
                    [s * (EDGE - 0.05), -0.6],
                ],
                0.0,
                LEN,
            );
        }
    }
    for x in piers {
        pier(b, x, deep + 0.3, 6.5, 2.4);
    }
    for end in [0.0, LEN] {
        abutment(b, end, shallow);
    }
    lamps(b, &[15.0, 45.0, 75.0, 105.0]);
}

/// A through girder: deep prefab walls either side of the road, ribbed and banded,
/// on two piers, and a heavy portal over the road at each end and each pier.
pub(super) fn through(b: &mut MeshBuilder, _tech: u8) {
    let piers = [40.0, 80.0];
    let (inner, outer) = (HALF + 0.2, HALF + 1.6);
    let (foot, top) = (-2.6, 3.8);
    let lintel = 7.2;
    if !b.mid() {
        coarse(b, foot, &piers, top);
        for x in [0.0, LEN] {
            paint(b, pat::PREFAB);
            b.loft(
                &[
                    rect_x(x, outer + 0.6, lintel, 1.4),
                    rect_x(x, outer + 0.6, lintel + 2.2, 1.4),
                ],
                true,
                true,
            );
        }
        return;
    }
    road(b, &piers);
    // The deck slab between the walls.
    boxed(
        b,
        v3(0.0, -inner, -1.5),
        v3(LEN, inner, -0.05),
        pat::CONCRETE,
    );
    if b.fine() {
        // Kerbs against the walls.
        for s in [-1.0f32, 1.0] {
            let (y0, y1) = (s * (HALF - 0.4), s * inner);
            boxed(
                b,
                v3(0.0, y0.min(y1), 0.0),
                v3(LEN, y0.max(y1), 0.25),
                pat::TRIM,
            );
        }
    }
    for s in [-1.0f32, 1.0] {
        let wall = Rect::new(0.0, s * inner, LEN, s * outer);
        let cut = 0.3;
        cut_walls(b, wall, cut, 1, foot, 2.2, all(pat::PREFAB), pat::PREFAB);
        recess(b, wall, cut, 1, 2.2, 2.6, 0.22, pat::TRIM);
        cut_walls(b, wall, cut, 1, 2.6, top, all(pat::PREFAB), pat::PREFAB);
        paint(b, pat::PREFAB);
        b.loft(
            &[
                cut_ring(wall, cut, 1, top),
                cut_ring(wall.grow(0.15), cut + 0.05, 1, top),
                cut_ring(wall.grow(0.15), cut + 0.05, 1, top + 0.2),
                cut_ring(wall.grow(-0.2), cut, 1, top + 0.45),
            ],
            false,
            true,
        );
        // Ribs up the outer face between the portals.
        // Every other one drops out below full detail.
        let face = s * outer;
        for (k, x) in cells(0.0, LEN, 6.6).enumerate() {
            if [0.0, 40.0, 80.0, LEN].iter().any(|p| (x - p).abs() < 2.0)
                || (!b.fine() && k % 2 == 1)
            {
                continue;
            }
            let (y0, y1) = (face, face + s * 0.5);
            boxed(
                b,
                v3(x - 0.4, y0.min(y1), foot),
                v3(x + 0.4, y0.max(y1), top + 0.2),
                pat::PREFAB,
            );
        }
        // The soffit under the walls, closing them off.
        if b.fine() {
            paint(b, pat::TRIM);
            facing(
                b,
                vec![
                    v3(0.0, s * inner, foot),
                    v3(LEN, s * inner, foot),
                    v3(LEN, s * outer, foot),
                    v3(0.0, s * outer, foot),
                ],
                Vec3::NEG_Z,
            );
        }
    }
    // Under the deck, between the walls.
    boxed(b, v3(0.0, -inner, foot), v3(LEN, inner, -1.5), pat::PREFAB);
    for x in piers {
        pier(b, x, foot - 0.9, 5.4, 1.6);
        paint(b, pat::CONCRETE);
        b.chamfered_box(
            v3(x, 0.0, foot - 0.45),
            Vec3::new(4.0, 2.0 * outer + 1.0, 0.9),
            0.5,
        );
    }
    for x in [0.0, 40.0, 80.0, LEN] {
        portal(b, x, outer, foot, lintel);
    }
    for end in [0.0, LEN] {
        abutment(b, end, foot);
    }
}

/// The coarse level, a few boxes: the deck from `under` to the road (walls to `wall`
/// over it when it is a through girder) and plain piers down to the bed.
fn coarse(b: &mut MeshBuilder, under: f32, piers: &[f32], wall: f32) {
    paint(b, pat::PREFAB);
    let top = if wall > 0.0 { wall } else { PARAPET };
    let (o, i) = (EDGE + 0.6 * f32::from(wall > 0.0), HALF);
    let section = [
        [-o, under],
        [o, under],
        [o, top],
        [i, top],
        [i, 0.0],
        [-i, 0.0],
        [-i, top],
        [-o, top],
    ];
    let ring = |x: f32| -> Vec<Vec3> { section.iter().map(|p| v3(x, p[0], p[1])).collect() };
    b.loft(&[ring(0.0), ring(LEN)], false, false);
    paint(b, pat::CONCRETE);
    for &x in piers {
        b.loft(
            &[rect_x(x, 5.5, BED - 1.0, 1.8), rect_x(x, 5.5, under, 1.8)],
            false,
            false,
        );
    }
}

/// A level rectangle round `x`, `half_y` either side of it and `half_x` along it,
/// at height `z`.
fn rect_x(x: f32, half_y: f32, z: f32, half_x: f32) -> Vec<Vec3> {
    Rect::new(x - half_x, -half_y, x + half_x, half_y).ring(z)
}

/// The road at z 0: paving between the parapets, a joint across it over each pier.
fn road(b: &mut MeshBuilder, joints: &[f32]) {
    deck(b, Rect::new(0.0, -HALF, LEN, HALF), 0.0, pat::PAVING);
    if !b.fine() {
        return;
    }
    for &x in joints {
        paint(b, pat::STEEL);
        facing(
            b,
            vec![
                v3(x - 0.2, -HALF, 0.02),
                v3(x + 0.2, -HALF, 0.02),
                v3(x + 0.2, HALF, 0.02),
                v3(x - 0.2, HALF, 0.02),
            ],
            Vec3::Z,
        );
    }
}

/// The parapets the deck's full length, either side: a dark recessed band from
/// `foot` up past the road, a prefab wall over it, a bevelled cap, a handrail on
/// posts, and a heavy post at each end.
fn parapets(b: &mut MeshBuilder, foot: f32) {
    for s in [-1.0f32, 1.0] {
        let y = |d: f32| s * d;
        let band = Rect::new(0.0, y(HALF), LEN, y(EDGE - 0.25));
        boxed(
            b,
            v3(0.0, band.min.y, foot),
            v3(LEN, band.max.y, 0.3),
            pat::TRIM,
        );
        boxed(
            b,
            v3(0.0, y(HALF).min(y(EDGE)), 0.3),
            v3(LEN, y(HALF).max(y(EDGE)), 1.15),
            pat::PREFAB,
        );
        paint(b, pat::PREFAB);
        b.extrude_x(
            &[
                [y(HALF - 0.1), 1.15],
                [y(EDGE + 0.1), 1.15],
                [y(EDGE + 0.1), 1.28],
                [y(EDGE - 0.15), PARAPET],
                [y(HALF + 0.15), PARAPET],
                [y(HALF - 0.1), 1.28],
            ],
            0.0,
            LEN,
        );
        // The heavy posts at the ends, banded in the colony's accent.
        for x in [0.6, LEN - 0.6] {
            paint(b, pat::PREFAB);
            b.chamfered_box(v3(x, y(EDGE - 0.4), 0.6), Vec3::new(1.6, 1.6, 4.4), 0.35);
            if b.fine() {
                paint(b, pat::ACCENT);
                b.chamfered_box(v3(x, y(EDGE - 0.4), 2.0), Vec3::new(1.7, 1.7, 0.5), 0.37);
                paint(b, pat::PREFAB);
                b.chamfered_box(v3(x, y(EDGE - 0.4), 2.95), Vec3::new(1.2, 1.2, 0.3), 0.25);
            }
        }
        if b.fine() {
            // A steel handrail on posts over the cap.
            paint(b, pat::STEEL);
            let rail_y = y(EDGE - 0.5);
            let h = PARAPET + 0.75;
            b.beam(
                v3(1.6, rail_y, h),
                v3(LEN - 1.6, rail_y, h),
                Vec2::splat(0.09),
                Vec2::splat(0.09),
            );
            for x in cells(1.6, LEN - 3.2, 4.0) {
                b.beam(
                    v3(x, rail_y, PARAPET),
                    v3(x, rail_y, h),
                    Vec2::splat(0.08),
                    Vec2::splat(0.08),
                );
            }
        }
    }
}

/// A cast pier at `x` from under the bed up to `top`: its plan long across the
/// deck, pointed upstream and down into cutwaters, battered, with a dark band
/// under its head.
fn pier(b: &mut MeshBuilder, x: f32, top: f32, half_y: f32, half_x: f32) {
    let nose = half_x * 1.4;
    let plan = [
        [half_x, -half_y],
        [half_x, half_y],
        [0.0, half_y + nose],
        [-half_x, half_y],
        [-half_x, -half_y],
        [0.0, -half_y - nose],
    ];
    let ring = |z: f32, k: f32| -> Vec<Vec3> {
        plan.iter().map(|p| v3(x + p[0] * k, p[1] * k, z)).collect()
    };
    let band = top - 1.4;
    paint(b, pat::CONCRETE);
    b.loft(
        &[
            ring(BED - 2.0, 1.14),
            ring(BED + 2.5, 1.08),
            ring(band, 1.0),
        ],
        false,
        true,
    );
    paint(b, pat::TRIM);
    b.loft(&[ring(band, 0.94), ring(band + 0.5, 0.94)], false, false);
    paint(b, pat::CONCRETE);
    b.loft(&[ring(band + 0.5, 1.0), ring(top, 1.0)], true, true);
}

/// The bearings a box girder sits on over a pier at `x`: a steel block under each
/// web, between the crosshead and the soffit at `z`.
fn bearings(b: &mut MeshBuilder, x: f32, z: f32) {
    for y in [-4.8, 4.8] {
        boxed(
            b,
            v3(x - 0.8, y - 0.7, z - 0.4),
            v3(x + 0.8, y + 0.7, z + 0.05),
            pat::STEEL,
        );
    }
}

/// A landing's abutment at `end` (the deck's start or end): a battered concrete block
/// from the bed up under the deck's soffit `seat`, its wing walls back into the bank.
fn abutment(b: &mut MeshBuilder, end: f32, seat: f32) {
    let d = if end < LEN * 0.5 { 1.0 } else { -1.0 };
    let back = end - d * 8.0;
    let at = |front: f32, half: f32, z: f32| -> Vec<Vec3> {
        Rect::new(back, -half, end + d * front, half).ring(z)
    };
    paint(b, pat::CONCRETE);
    b.loft(
        &[
            at(6.0, EDGE + 3.0, BED - 2.0),
            at(2.5, EDGE + 1.6, seat - 0.6),
            at(2.5, EDGE + 1.6, seat),
        ],
        false,
        true,
    );
    // The wing walls' prefab caps along the bank.
    for s in [-1.0f32, 1.0] {
        let y0 = s * (EDGE + 0.1);
        let y1 = s * (EDGE + 1.6);
        paint(b, pat::PREFAB);
        b.loft(
            &[
                Rect::new(back, y0, end + d * 2.5, y1).ring(-0.6),
                Rect::new(back, y0, end + d * 2.5, y1).ring(0.5),
                Rect::new(back, y0, end + d * 2.5, y1).grow(-0.2).ring(0.75),
            ],
            false,
            true,
        );
    }
}

/// Lamp standards over the parapets at each `x`, alternating sides, each arm reaching
/// out over the road to a lamp head lit by its surface. Full detail.
fn lamps(b: &mut MeshBuilder, xs: &[f32]) {
    if !b.fine() {
        return;
    }
    for (k, &x) in xs.iter().enumerate() {
        let s = if k % 2 == 0 { 1.0 } else { -1.0 };
        let foot = v3(x, s * (EDGE - 0.5), PARAPET);
        let h = 8.5;
        paint(b, pat::STEEL);
        b.beam(
            foot,
            foot + Vec3::Z * h,
            Vec2::splat(0.3),
            Vec2::splat(0.18),
        );
        let tip = v3(x, s * (HALF - 2.5), PARAPET + h + 0.3);
        b.beam(
            foot + Vec3::Z * (h - 0.2),
            tip,
            Vec2::splat(0.14),
            Vec2::splat(0.12),
        );
        boxed(
            b,
            tip + v3(-0.35, -0.7, -0.3),
            tip + v3(0.35, 0.7, 0.0),
            pat::LED,
        );
    }
}

/// A heavy portal over the road at `x`: a chamfered pylon either side rising from
/// the walls' foot, an accent band near its top, a lintel across at `lintel` with
/// lamps let into its underside.
fn portal(b: &mut MeshBuilder, x: f32, outer: f32, foot: f32, lintel: f32) {
    let y = outer + 0.2;
    let top = lintel + 3.0;
    for s in [-1.0f32, 1.0] {
        let pylon = Rect::centred(x, s * y, 1.6, 1.6);
        if !b.fine() {
            solid(b, pylon, foot, top + 0.4, pat::PREFAB, pat::PREFAB);
            continue;
        }
        cut_slab(b, pylon, 0.5, 1, foot, top, pat::PREFAB);
        cut_slab(
            b,
            pylon.grow(0.05),
            0.52,
            1,
            top - 1.55,
            top - 0.85,
            pat::ACCENT,
        );
        cut_slab(b, pylon.grow(-0.2), 0.4, 1, top, top + 0.4, pat::PREFAB);
    }
    let span = Rect::new(x - 1.4, -y, x + 1.4, y);
    if !b.fine() {
        solid(b, span, lintel, lintel + 2.2, pat::PREFAB, pat::PREFAB);
        return;
    }
    cut_slab(b, span, 0.0, 1, lintel, lintel + 1.1, pat::PREFAB);
    recess(b, span, 0.0, 1, lintel + 1.1, lintel + 1.5, 0.18, pat::TRIM);
    paint(b, pat::PREFAB);
    b.loft(
        &[
            span.ring(lintel + 1.5),
            span.grow(0.15).ring(lintel + 1.5),
            span.grow(0.15).ring(lintel + 1.8),
            span.grow(-0.2).ring(lintel + 2.2),
        ],
        false,
        true,
    );
    for yy in [-4.0, 0.0, 4.0] {
        boxed(
            b,
            v3(x - 0.4, yy - 1.0, lintel - 0.1),
            v3(x + 0.4, yy + 1.0, lintel),
            pat::LED,
        );
    }
}
