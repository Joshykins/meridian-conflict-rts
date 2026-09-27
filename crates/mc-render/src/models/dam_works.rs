//! The works round the canyon map's dam (`dam.rs`): the switchyard at its foot,
//! the lattice towers that carry its line away, and the operations town, each
//! built to its plan in `mc_map::landmark` so the bake's levelled ground, the
//! towers' spacing and the path plan meet the models.
//!
//! Like the dam, nothing here runs: the lake is at dead pool, the breakers
//! stand open, the town is empty. Nothing is lit. Steel is galvanised
//! (`METAL`); painted walls, roofs, gravel and dark window bands are the dam's
//! scenery concrete looks (scenery.wgsl's `dam_concrete`), chosen by pattern.
//!
//! Wires are open three-sided tubes, flat shaded, a little fatter than a real
//! conductor bundle so a span still reads as a line from the RTS camera. A
//! line's spans are props of their own (`landmark_span`), standing with each
//! tower: the renderer pitches each to meet the next tower's ground
//! (`mc_map::PropKind::span`), so the towers stand on the ground as it lies.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use mc_map::landmark::{GORGE_LINE as LINE, GORGE_TOWN as TOWN, GORGE_YARD as YARD};

use super::builder::MeshBuilder;
use super::library::ModelDef;
use super::material::*;
use crate::gpu_consts::scenery::{
    CONCRETE_CAST as CAST, CONCRETE_CHUTE as GRAVEL, CONCRETE_ROOF as ROOF,
    CONCRETE_SHADOW as SHADOW, CONCRETE_WHITE as WHITE,
};

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("landmark_switchyard", YARD.half.0 as f32, 36.0, switchyard),
    ModelDef::new("landmark_pylon", 7.0, 58.0, pylon),
    ModelDef::new("landmark_span", 1.0, 26.0, span),
    ModelDef::new("landmark_town", TOWN.half.0 as f32, 40.0, town),
];

/// Full-detail triangle budgets: a line of about twenty towers, and one yard
/// and one town on the map.
#[cfg(test)]
pub(super) fn triangles(key: &str) -> Option<usize> {
    match key {
        "landmark_pylon" => Some(1000),
        "landmark_span" => Some(700),
        "landmark_switchyard" => Some(9000),
        "landmark_town" => Some(9000),
        _ => None,
    }
}

/// A conductor's radius (a four-wire bundle drawn as one), and an earth wire's.
const WIRE: f32 = 0.34;
const EARTH_WIRE: f32 = 0.24;
/// How much fatter a span's wires are drawn at each level than the yard's.
const SPAN_FAT: [f32; 3] = [1.6, 3.0, 5.0];
/// The lowest a span reaches below its pivot, where it is built (the renderer
/// raises it into the air): the lowest phase at mid-span, fattest.
#[cfg(test)]
pub(super) const SPAN_FLOOR: f32 =
    (LINE.phases[0].1 - LINE.pivot - LINE.sag) as f32 - WIRE * SPAN_FAT[2] - 0.5;
/// How far below its clamps an earth wire hangs at mid-span, against a
/// conductor's `LINE.sag`: it is strung tighter.
const EARTH_SAG: f32 = 0.75;

// ---- shared pieces ---------------------------------------------------------------

/// A ring of `sides` corners round `at`, square to `along`; the first corner up.
fn ring(at: Vec3, along: Vec3, radius: f32, sides: usize) -> Vec<Vec3> {
    let d = along.normalize();
    let reference = if d.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
    let side = d.cross(reference).normalize();
    let up = side.cross(d);
    (0..sides)
        .map(|i| {
            let a = (i as f32 + 0.25) * TAU / sides as f32;
            at + (side * a.cos() + up * a.sin()) * radius
        })
        .collect()
}

/// A straight member from `a` to `c`: an open bar of `sides` flat sides. Steel
/// angle, tubes and insulator strings, whose ends are never seen.
fn member(b: &mut MeshBuilder, a: Vec3, c: Vec3, radius: f32, sides: usize) {
    let d = c - a;
    b.loft(
        &[ring(a, d, radius, sides), ring(c, d, radius, sides)],
        false,
        false,
    );
}

/// A wire strung from `a` to `c`, hanging `sag` below the chord at mid-span (a
/// parabola: near enough a catenary this taut), in `segments` straight runs.
fn hang(b: &mut MeshBuilder, a: Vec3, c: Vec3, sag: f32, radius: f32, segments: usize) {
    let at = |t: f32| a.lerp(c, t) - Vec3::Z * (4.0 * sag * t * (1.0 - t));
    let slope = |t: f32| (c - a) - Vec3::Z * (4.0 * sag * (1.0 - 2.0 * t));
    let rings: Vec<Vec<Vec3>> = (0..=segments)
        .map(|i| {
            let t = i as f32 / segments as f32;
            ring(at(t), slope(t), radius, 3)
        })
        .collect();
    b.paint(METAL);
    b.loft(&rings, false, false);
}

/// A flat rectangle at height `z`, facing up.
fn slab(b: &mut MeshBuilder, x0: f32, y0: f32, x1: f32, y1: f32, z: f32) {
    b.face(&[
        Vec3::new(x0, y0, z),
        Vec3::new(x1, y0, z),
        Vec3::new(x1, y1, z),
        Vec3::new(x0, y1, z),
    ]);
}

/// A dark band laid just proud of a wall: glazing, a door, a shadowed bay. The
/// wall faces along `out` (±x or ±y), at `plane` on that axis; the band runs
/// `from`..`to` along the wall and `z0`..`z1` up it.
fn band(b: &mut MeshBuilder, out: Vec2, plane: f32, from: f32, to: f32, z0: f32, z1: f32) {
    let on = |s: f32, z: f32| {
        if out.x != 0.0 {
            Vec3::new(plane + out.x * 0.06, s, z)
        } else {
            Vec3::new(s, plane + out.y * 0.06, z)
        }
    };
    let mut quad = [on(from, z0), on(to, z0), on(to, z1), on(from, z1)];
    let normal = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
    if normal.dot(out.extend(0.0)) < 0.0 {
        quad.reverse();
    }
    b.paint(CONCRETE).pattern(SHADOW);
    b.face(&quad);
}

/// The two walls' faces of a box along each axis, for [`band`]: (outward, plane,
/// the wall's run along it).
fn walls(x0: f32, y0: f32, x1: f32, y1: f32) -> [(Vec2, f32, f32, f32); 4] {
    [
        (Vec2::NEG_Y, y0, x0, x1),
        (Vec2::Y, y1, x0, x1),
        (Vec2::NEG_X, x0, y0, y1),
        (Vec2::X, x1, y0, y1),
    ]
}

// ---- the line's towers -------------------------------------------------------------

/// Each insulator string's length, arm tip to clamp.
const STRING: f32 = 6.0;
/// The feet's half spacing, the body's half-width at its waist (just under the
/// lowest arm) and at its top, and those heights.
const FOOT: f32 = 6.0;
const WAIST: (f32, f32) = (34.0, 2.0);
const TOP: (f32, f32) = (54.0, 1.5);

/// The conductors' clamps (y out from the axis, z), low to high, one circuit
/// either side, and the earth wires' peaks: `GORGE_LINE`'s.
fn phases() -> [Vec2; 3] {
    LINE.phases.map(|(y, z)| Vec2::new(y as f32, z as f32))
}

fn earth() -> Vec2 {
    Vec2::new(LINE.earth.0 as f32, LINE.earth.1 as f32)
}

/// The body's half-width at height `z`.
fn half_width(z: f32) -> f32 {
    if z <= WAIST.0 {
        FOOT + (WAIST.1 - FOOT) * z / WAIST.0
    } else {
        WAIST.1 + (TOP.1 - WAIST.1) * (z - WAIST.0) / (TOP.0 - WAIST.0)
    }
}

/// A corner of the body at height `z`, on the side (`sx`, `sy`).
fn corner(z: f32, s: Vec2) -> Vec3 {
    (s * half_width(z)).extend(z)
}

const CORNERS: [Vec2; 4] = [
    Vec2::new(1.0, -1.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(-1.0, 1.0),
    Vec2::new(-1.0, -1.0),
];

/// A 500 kV double-circuit drum tower after the Three Gorges lines: four splayed
/// legs on concrete footings, a braced body tapering to a waist, three arms a
/// side (the middle one longest) with insulator strings down to the clamps and
/// two earth peaks.
fn pylon(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        pylon_far(b);
        return;
    }
    let fine = b.fine();
    tower_body(b, fine);
    for s in [-1.0f32, 1.0] {
        for p in phases() {
            crossarm(b, s, p.x, p.y + STRING, fine);
            b.paint(CONCRETE).pattern(WHITE);
            member(
                b,
                Vec3::new(0.0, s * p.x, p.y + STRING),
                Vec3::new(0.0, s * p.x, p.y),
                0.3,
                if fine { 6 } else { 3 },
            );
        }
        earth_peak(b, s, fine);
    }
    if fine {
        b.paint(CONCRETE).pattern(CAST);
        for c in CORNERS {
            let at = c * FOOT;
            b.cuboid_open(at.extend(-0.3), Vec3::new(1.8, 1.8, 1.8));
        }
    }
}

/// A span: the six conductors and two earth wires from a tower's clamps and
/// peaks on along +x to the next tower's, built about the pivot (z 0 is
/// `LINE.pivot` over the tower's foot), where the renderer raises and pitches
/// it.
/// A span's bounds are long, so it keeps its full level most of the way across
/// the map: the wires are drawn about as fat as a four-conductor bundle's
/// spread, a pixel wide from the RTS camera rather than breaking into dashes,
/// and fatter still at the further levels.
fn span(b: &mut MeshBuilder, _tech: u8) {
    let span = LINE.span as f32;
    let (segments, fat) = if b.fine() {
        (12, SPAN_FAT[0])
    } else if b.coarse() {
        (1, SPAN_FAT[2])
    } else {
        (5, SPAN_FAT[1])
    };
    let pivot = Vec3::Z * LINE.pivot as f32;
    for s in [-1.0f32, 1.0] {
        for p in phases() {
            let a = Vec3::new(0.0, s * p.x, p.y) - pivot;
            hang(
                b,
                a,
                a + Vec3::X * span,
                LINE.sag as f32,
                WIRE * fat,
                segments,
            );
        }
        let e = Vec3::new(0.0, s * earth().x, earth().y) - pivot;
        let sag = LINE.sag as f32 * EARTH_SAG;
        hang(b, e, e + Vec3::X * span, sag, EARTH_WIRE * fat, segments);
    }
}

/// The far level: the body as a tapered box and each pair of arms as a bar.
fn pylon_far(b: &mut MeshBuilder) {
    b.paint(METAL);
    b.frustum_open(
        Vec3::ZERO,
        Vec2::splat(2.0 * FOOT),
        Vec2::splat(2.0 * TOP.1),
        TOP.0,
        Vec2::ZERO,
    );
    for p in phases() {
        let z = p.y + STRING;
        b.cuboid_open(Vec3::new(0.0, 0.0, z), Vec3::new(1.2, 2.0 * p.x, 1.2));
    }
}

/// The legs, and each face's bracing: crossed diagonals and a strut at each
/// level at full detail, one diagonal a panel further off.
fn tower_body(b: &mut MeshBuilder, fine: bool) {
    b.paint(METAL);
    let (leg, brace, sides) = if fine {
        (0.36, 0.17, 4)
    } else {
        (0.4, 0.24, 3)
    };
    for c in CORNERS {
        member(b, corner(0.0, c), corner(WAIST.0, c), leg, sides);
        member(b, corner(WAIST.0, c), corner(TOP.0, c), leg * 0.8, sides);
    }
    let levels: &[f32] = if fine {
        &[0.0, 9.0, 17.0, 24.0, 30.0, 36.0, 44.0, 52.0]
    } else {
        &[0.0, 17.0, 36.0, 52.0]
    };
    let struts: &[f32] = if fine {
        &[17.0, 30.0, 36.0, 44.0, 52.0, TOP.0]
    } else {
        &[36.0, TOP.0]
    };
    for k in 0..4 {
        let (c0, c1) = (CORNERS[k], CORNERS[(k + 1) % 4]);
        for (i, w) in levels.windows(2).enumerate() {
            let (z0, z1) = (w[0], w[1]);
            if fine || i % 2 == 0 {
                member(b, corner(z0, c0), corner(z1, c1), brace, 3);
            }
            if fine || i % 2 == 1 {
                member(b, corner(z0, c1), corner(z1, c0), brace, 3);
            }
        }
        for &z in struts {
            member(b, corner(z, c0), corner(z, c1), brace, 3);
        }
    }
}

/// An arm on side `s` reaching `reach` out at height `z`: a tapering truss, its
/// chords meeting at the tip the string hangs from.
fn crossarm(b: &mut MeshBuilder, s: f32, reach: f32, z: f32, fine: bool) {
    b.paint(METAL);
    let tip = Vec3::new(0.0, s * reach, z);
    let depth = (TOP.0 - z).clamp(2.0, 3.0);
    let h = half_width(z);
    if fine {
        for x in [-h, h] {
            member(b, Vec3::new(x, s * h, z), tip, 0.2, 3);
            member(b, Vec3::new(x, s * h, z + depth), tip, 0.18, 3);
        }
        // A strut from the bottom chords' middle up to the top chords.
        let mid = s * (h + reach) * 0.5;
        member(
            b,
            Vec3::new(0.0, mid, z),
            Vec3::new(0.0, s * h, z + depth),
            0.14,
            3,
        );
    } else {
        member(b, Vec3::new(0.0, s * h, z), tip, 0.3, 3);
        member(b, Vec3::new(0.0, s * h, z + depth), tip, 0.26, 3);
    }
}

/// The earth wire's peak on side `s`: a small frame off the body's top out to
/// the point the wire is clamped at.
fn earth_peak(b: &mut MeshBuilder, s: f32, fine: bool) {
    b.paint(METAL);
    let peak = Vec3::new(0.0, s * earth().x, earth().y);
    let top = TOP.0;
    if fine {
        for x in [-TOP.1, TOP.1] {
            member(b, Vec3::new(x, s * TOP.1, top), peak, 0.2, 3);
        }
        member(
            b,
            Vec3::new(0.0, s * half_width(top - 3.0), top - 3.0),
            peak,
            0.16,
            3,
        );
    } else {
        member(b, Vec3::new(0.0, s * TOP.1, top), peak, 0.28, 3);
    }
}

// ---- the switchyard -----------------------------------------------------------------

/// The fence's half lengths (`GORGE_YARD`) and the line's first tower.
const YARD_X: f32 = YARD.half.0 as f32;
const YARD_Y: f32 = YARD.half.1 as f32;
const FIRST: f32 = YARD.first as f32;
/// The dead-end gantry: its beams across the line, the conductors' heights on
/// it (low phase to high, as on a tower), the earth peaks, and its columns.
const DEAD_END_X: f32 = 104.0;
const DEAD_END_Z: [f32; 3] = [18.0, 24.0, 30.0];
const DEAD_END_EARTH: f32 = 36.0;
const DEAD_END_COLUMN: f32 = 24.0;
/// The strain strings off the dead-end gantry toward the line.
const STRAIN: f32 = 4.0;
/// The bus gantries across the yard, and their girders' underside.
const BUS_X: [f32; 2] = [-40.0, 60.0];
const BUS_Z: f32 = 17.5;
/// The bays under the buses, one row of equipment a phase: the two line bays
/// under the line's circuits and a transformer bay outside each, their phases
/// `PHASE_GAP` apart across the yard.
const LINE_BAY: f32 = 12.5;
const TRANSFORMER_BAY: f32 = 45.0;
const PHASE_GAP: f32 = 7.5;
/// The main transformers' row, one single-phase unit a phase of each
/// transformer bay, and how high their bushings reach.
const TRANSFORMER_X: f32 = -78.0;
const BUSHING_TOP: f32 = 12.5;

/// Every phase's y across the yard: (bay centre's phases, a line bay's).
fn bay_phases() -> Vec<(f32, bool)> {
    let mut out = Vec::new();
    for s in [-1.0f32, 1.0] {
        for (centre, line) in [(LINE_BAY, true), (TRANSFORMER_BAY, false)] {
            for k in [-1.0f32, 0.0, 1.0] {
                out.push((s * (centre + k * PHASE_GAP), line));
            }
        }
    }
    out
}

/// Where a line bay's phase `k` (0 nearest the axis) on side `s` meets the
/// dead-end gantry: its line phase's y at that phase's height on the gantry.
fn dead_end(s: f32, k: usize) -> Vec3 {
    let p = phases()[k];
    Vec3::new(DEAD_END_X, s * p.x, DEAD_END_Z[k])
}

/// The dam's 500 kV switchyard: a walled gravel yard; the main transformers
/// along its -x side, fed by cable trenches from the powerhouse; two bus
/// gantries over rows of breakers and disconnectors; and at +x the dead-end
/// gantry the line leaves from, its first span up to the first tower.
fn switchyard(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        yard_far(b);
        return;
    }
    let fine = b.fine();
    yard_ground(b, fine);
    yard_houses(b, fine);
    for (y, line) in bay_phases() {
        if !line {
            transformer(b, y, fine);
        }
        bay(b, y, fine);
    }
    for x in BUS_X {
        bus_gantry(b, x, fine);
    }
    dead_end_gantry(b, fine);
    yard_wires(b, fine);
}

/// The far level: the yard's gravel, the transformer banks, the control
/// house, a bus gantry and the dead-end gantry as boxes.
fn yard_far(b: &mut MeshBuilder) {
    b.paint(CONCRETE).pattern(GRAVEL);
    slab(b, -YARD_X, -YARD_Y, YARD_X, YARD_Y, 0.1);
    b.paint(CONCRETE).pattern(WHITE);
    for s in [-1.0f32, 1.0] {
        b.cuboid_open(
            Vec3::new(TRANSFORMER_X, s * TRANSFORMER_BAY, 3.5),
            Vec3::new(12.0, 3.0 * PHASE_GAP, 7.0),
        );
    }
    b.cuboid_open(Vec3::new(-102.0, 22.0, 4.0), Vec3::new(24.0, 20.0, 8.0));
    b.paint(METAL);
    b.cuboid_open(
        Vec3::new(BUS_X[1], 0.0, BUS_Z + 0.5),
        Vec3::new(1.5, 124.0, 2.5),
    );
    b.cuboid_open(
        Vec3::new(DEAD_END_X, 0.0, 15.5),
        Vec3::new(3.0, 2.0 * DEAD_END_COLUMN + 3.0, 31.0),
    );
}

/// The gravel, the roads and trench covers, and the wall round it all with a
/// gate on the road in from the dam.
fn yard_ground(b: &mut MeshBuilder, fine: bool) {
    b.paint(CONCRETE).pattern(GRAVEL);
    slab(b, -YARD_X, -YARD_Y, YARD_X, YARD_Y, 0.1);
    b.paint(CONCRETE).pattern(CAST);
    // The road in through the gate and down the yard, and the one along the
    // transformers.
    slab(b, -YARD_X, -3.0, 96.0, 3.0, 0.2);
    slab(b, -62.0, -YARD_Y + 4.0, -56.0, -3.0, 0.2);
    slab(b, -62.0, 3.0, -56.0, YARD_Y - 4.0, 0.2);
    // Cable trenches from the powerhouse, under the wall to each bank.
    if fine {
        for s in [-1.0f32, 1.0] {
            for dy in [-3.0f32, 3.0] {
                let y = s * TRANSFORMER_BAY + dy;
                slab(b, -YARD_X, y - 0.8, TRANSFORMER_X - 6.5, y + 0.8, 0.25);
            }
        }
    }
    // The wall: 2.4 m of concrete, the gate a gap on the road.
    let (t, h) = (0.3, 2.4);
    let wall = |b: &mut MeshBuilder, x0: f32, y0: f32, x1: f32, y1: f32| {
        b.cuboid_open(
            Vec3::new((x0 + x1) * 0.5, (y0 + y1) * 0.5, h * 0.5 - 0.3),
            Vec3::new(x1 - x0, y1 - y0, h + 0.6),
        );
    };
    wall(b, -YARD_X, -YARD_Y, YARD_X, -YARD_Y + t);
    wall(b, -YARD_X, YARD_Y - t, YARD_X, YARD_Y);
    wall(b, YARD_X - t, -YARD_Y + t, YARD_X, YARD_Y - t);
    wall(b, -YARD_X, -YARD_Y + t, -YARD_X + t, -5.0);
    wall(b, -YARD_X, 5.0, -YARD_X + t, YARD_Y - t);
}

/// The control house by the gate, and a relay house across the road from it.
fn yard_houses(b: &mut MeshBuilder, fine: bool) {
    for (x0, y0, x1, y1, h) in [
        (-114.0, 12.0, -90.0, 32.0, 8.0),
        (-112.0, -30.0, -94.0, -14.0, 5.0),
    ] {
        b.paint(CONCRETE).pattern(WHITE);
        b.block(Vec3::new(x0, y0, -0.3), Vec3::new(x1, y1, h));
        b.paint(CONCRETE).pattern(ROOF);
        b.block(
            Vec3::new(x0 + 0.4, y0 + 0.4, h),
            Vec3::new(x1 - 0.4, y1 - 0.4, h + 0.5),
        );
        for (out, plane, from, to) in walls(x0, y0, x1, y1) {
            let storeys = if h > 6.0 { 2 } else { 1 };
            for k in 0..storeys {
                if fine || k == 0 {
                    let z = 1.4 + 3.8 * k as f32;
                    band(b, out, plane, from + 1.5, to - 1.5, z, z + 1.6);
                }
            }
        }
    }
}

/// A single-phase main transformer at `y`: a plinth, the tank, a radiator bank,
/// the conservator along its top and the bushings, and a fire wall off each side
/// of it but the bank's ends.
fn transformer(b: &mut MeshBuilder, y: f32, fine: bool) {
    let x = TRANSFORMER_X;
    b.paint(CONCRETE).pattern(CAST);
    b.cuboid_open(Vec3::new(x + 0.5, y, 0.15), Vec3::new(14.0, 6.6, 0.9));
    b.paint(CONCRETE).pattern(WHITE);
    b.block(
        Vec3::new(x - 4.0, y - 2.4, 0.6),
        Vec3::new(x + 4.0, y + 2.4, 6.5),
    );
    b.paint(METAL);
    if fine {
        for dy in [-1.7f32, -0.6, 0.6, 1.7] {
            b.block(
                Vec3::new(x + 4.0, y + dy - 0.15, 1.2),
                Vec3::new(x + 6.6, y + dy + 0.15, 6.0),
            );
        }
        b.cylinder_between(
            Vec3::new(x - 2.5, y - 2.3, 8.2),
            Vec3::new(x - 2.5, y + 2.3, 8.2),
            0.8,
            0.8,
            8,
        );
        member(
            b,
            Vec3::new(x - 2.5, y, 6.5),
            Vec3::new(x - 2.5, y, 7.5),
            0.25,
            3,
        );
        b.paint(CONCRETE).pattern(WHITE);
        for dy in [-1.3f32, 1.3] {
            member(
                b,
                Vec3::new(x - 0.5, y + dy, 6.5),
                Vec3::new(x - 0.5, y + dy, 8.8),
                0.22,
                6,
            );
        }
    } else {
        b.block(
            Vec3::new(x + 4.0, y - 2.0, 1.2),
            Vec3::new(x + 6.6, y + 2.0, 6.0),
        );
    }
    b.paint(CONCRETE).pattern(WHITE);
    member(
        b,
        Vec3::new(x + 1.5, y, 6.5),
        Vec3::new(x + 1.5, y, BUSHING_TOP),
        0.42,
        if fine { 6 } else { 3 },
    );
    // Fire walls between the units of a bank.
    let bank = y.signum() * TRANSFORMER_BAY;
    if (y - bank).abs() < 1.0 {
        b.paint(CONCRETE).pattern(CAST);
        for dy in [-0.5f32, 0.5] {
            let w = y + dy * PHASE_GAP;
            b.cuboid_open(Vec3::new(x, w, 4.5), Vec3::new(13.0, 0.5, 9.0));
        }
    }
}

/// An insulator post on a steel stand at (`x`, `y`): the stand, and the porcelain
/// up to `top`. Returns the post's head.
fn post(b: &mut MeshBuilder, x: f32, y: f32, top: f32, fine: bool) -> Vec3 {
    let head = Vec3::new(x, y, top);
    if !fine {
        // Further off, one pale post from the ground.
        b.paint(CONCRETE).pattern(WHITE);
        member(b, Vec3::new(x, y, 0.0), head, 0.3, 3);
        return head;
    }
    let stand = Vec3::new(x, y, top - 3.2);
    b.paint(METAL);
    member(b, Vec3::new(x, y, 0.0), stand, 0.3, 4);
    b.paint(CONCRETE).pattern(WHITE);
    member(b, stand, head, 0.26, 6);
    head
}

/// One phase's row of equipment along x at `y` under the buses: a disconnector
/// either end (two posts, the blade open), a current transformer and a
/// dead-tank breaker between.
fn bay(b: &mut MeshBuilder, y: f32, fine: bool) {
    for x in [-22.0f32, 38.0] {
        let a = post(b, x - 2.2, y, 7.0, fine);
        let c = post(b, x + 2.2, y, 7.0, fine);
        if fine {
            // The blade, open: raised off the far post.
            b.paint(METAL);
            member(b, a, c + Vec3::new(-1.2, 0.0, 3.2), 0.12, 3);
        }
    }
    // The current transformer: a tall post with its oil head.
    let head = post(b, -4.0, y, 8.0, fine);
    b.paint(METAL);
    b.cuboid_open(head + Vec3::Z * 0.6, Vec3::new(1.4, 1.0, 1.2));
    // The breaker: its tank on a stand, a bushing up off each end.
    let x = 16.0;
    b.paint(METAL);
    b.cuboid_open(Vec3::new(x, y, 1.1), Vec3::new(2.0, 1.2, 2.2));
    if fine {
        b.cylinder_between(
            Vec3::new(x - 2.8, y, 3.1),
            Vec3::new(x + 2.8, y, 3.1),
            0.9,
            0.9,
            8,
        );
    } else {
        b.block(
            Vec3::new(x - 2.8, y - 0.8, 2.3),
            Vec3::new(x + 2.8, y + 0.8, 3.9),
        );
    }
    b.paint(CONCRETE).pattern(WHITE);
    for dx in [-1.0f32, 1.0] {
        member(
            b,
            Vec3::new(x + dx * 2.0, y, 3.6),
            Vec3::new(x + dx * 3.4, y, 8.6),
            0.28,
            if fine { 6 } else { 3 },
        );
    }
}

/// A bus gantry across the yard at `x`: A-frame tubular columns on footings
/// between the bays, a lattice girder along their tops, lightning rods over the
/// end columns.
fn bus_gantry(b: &mut MeshBuilder, x: f32, fine: bool) {
    let columns = [-62.0f32, -29.0, 29.0, 62.0];
    let (lo, hi) = (BUS_Z, BUS_Z + 2.0);
    let sides = if fine { 6 } else { 3 };
    for y in columns {
        b.paint(METAL);
        for dx in [-3.0f32, 3.0] {
            member(
                b,
                Vec3::new(x + dx, y, 0.0),
                Vec3::new(x, y, hi),
                0.36,
                sides,
            );
        }
        if fine {
            b.paint(CONCRETE).pattern(CAST);
            b.cuboid_open(Vec3::new(x, y, 0.1), Vec3::new(8.0, 1.6, 0.8));
        }
    }
    b.paint(METAL);
    let (y0, y1) = (columns[0], columns[3]);
    for z in [lo, hi] {
        member(b, Vec3::new(x, y0, z), Vec3::new(x, y1, z), 0.28, 3);
    }
    let panels = if fine { 20 } else { 6 };
    for i in 0..panels {
        let ya = y0 + (y1 - y0) * i as f32 / panels as f32;
        let yb = y0 + (y1 - y0) * (i + 1) as f32 / panels as f32;
        let (za, zb) = if i % 2 == 0 { (lo, hi) } else { (hi, lo) };
        member(b, Vec3::new(x, ya, za), Vec3::new(x, yb, zb), 0.16, 3);
    }
    for y in [y0, y1] {
        member(b, Vec3::new(x, y, hi), Vec3::new(x, y, hi + 8.0), 0.2, 3);
    }
}

/// A square lattice column of the dead-end gantry at `y`, 3 m across and as
/// high as its top beam.
fn gantry_column(b: &mut MeshBuilder, y: f32, fine: bool) {
    let top = DEAD_END_Z[2] + 1.0;
    let h = 1.5;
    let at = |c: Vec2, z: f32| Vec3::new(DEAD_END_X + c.x * h, y + c.y * h, z);
    let levels: &[f32] = if fine {
        &[0.0, 8.0, 16.0, 23.0, top]
    } else {
        &[0.0, 16.0, top]
    };
    b.paint(METAL);
    for c in CORNERS {
        member(b, at(c, 0.0), at(c, top), 0.3, if fine { 4 } else { 3 });
    }
    for k in 0..4 {
        let (c0, c1) = (CORNERS[k], CORNERS[(k + 1) % 4]);
        for w in levels.windows(2) {
            member(b, at(c0, w[0]), at(c1, w[1]), 0.14, 3);
            if fine {
                member(b, at(c1, w[0]), at(c0, w[1]), 0.14, 3);
            }
        }
    }
    if fine {
        b.paint(CONCRETE).pattern(CAST);
        b.cuboid_open(Vec3::new(DEAD_END_X, y, 0.1), Vec3::new(4.4, 4.4, 0.8));
    }
}

/// The dead-end gantry the line leaves from: two lattice columns either side of
/// the line, a beam for each phase's height between them, earth peaks on the
/// top one, and the strain strings off each phase toward the line.
fn dead_end_gantry(b: &mut MeshBuilder, fine: bool) {
    let c = DEAD_END_COLUMN;
    for y in [-c, c] {
        gantry_column(b, y, fine);
    }
    b.paint(METAL);
    for z in DEAD_END_Z {
        for dz in [-0.6f32, 0.6] {
            member(
                b,
                Vec3::new(DEAD_END_X, -c, z + dz),
                Vec3::new(DEAD_END_X, c, z + dz),
                0.22,
                3,
            );
        }
        if fine {
            for i in 0..8 {
                let ya = -c + 2.0 * c * i as f32 / 8.0;
                let yb = -c + 2.0 * c * (i + 1) as f32 / 8.0;
                let (za, zb) = if i % 2 == 0 { (-0.6, 0.6) } else { (0.6, -0.6) };
                member(
                    b,
                    Vec3::new(DEAD_END_X, ya, z + za),
                    Vec3::new(DEAD_END_X, yb, z + zb),
                    0.12,
                    3,
                );
            }
        }
    }
    let e = earth();
    for s in [-1.0f32, 1.0] {
        let peak = Vec3::new(DEAD_END_X, s * e.x, DEAD_END_EARTH);
        for dy in [-3.0f32, 3.0] {
            member(
                b,
                Vec3::new(DEAD_END_X, s * e.x + dy, DEAD_END_Z[2] + 0.6),
                peak,
                0.2,
                3,
            );
        }
        b.paint(CONCRETE).pattern(WHITE);
        for k in 0..3 {
            let p = dead_end(s, k);
            member(b, p, p + Vec3::X * STRAIN, 0.3, if fine { 6 } else { 3 });
        }
        b.paint(METAL);
    }
}

/// The yard's wires: each transformer's bushing up to the first bus gantry,
/// every phase's bus between the gantries, the line bays' jumpers up to the
/// dead-end gantry, and the line's first span, climbing to the first tower.
fn yard_wires(b: &mut MeshBuilder, fine: bool) {
    let seg = |n: usize| if fine { n } else { (n / 2).max(2) };
    for (y, line) in bay_phases() {
        let bus = |x: f32| Vec3::new(x, y, BUS_Z);
        if !line {
            let bushing = Vec3::new(TRANSFORMER_X + 1.5, y, BUSHING_TOP);
            hang(b, bushing, bus(BUS_X[0]), 0.8, WIRE, seg(6));
        }
        hang(b, bus(BUS_X[0]), bus(BUS_X[1]), 1.5, WIRE, seg(8));
    }
    let span = FIRST - DEAD_END_X - STRAIN;
    for s in [-1.0f32, 1.0] {
        for (k, p) in phases().into_iter().enumerate() {
            let bay_y = s * (LINE_BAY + (k as f32 - 1.0) * PHASE_GAP);
            hang(
                b,
                Vec3::new(BUS_X[1], bay_y, BUS_Z),
                dead_end(s, k),
                1.0,
                WIRE,
                seg(6),
            );
            // The slack span, less tight than the line's: its sag scaled to its
            // length, doubled.
            let sag = 2.0 * LINE.sag as f32 * (span / LINE.span as f32).powi(2);
            hang(
                b,
                dead_end(s, k) + Vec3::X * STRAIN,
                Vec3::new(FIRST, s * p.x, p.y),
                sag,
                WIRE,
                seg(12),
            );
        }
        let e = earth();
        let sag = 2.0 * LINE.sag as f32 * EARTH_SAG * (span / LINE.span as f32).powi(2);
        hang(
            b,
            Vec3::new(DEAD_END_X, s * e.x, DEAD_END_EARTH),
            Vec3::new(FIRST, s * e.x, e.y),
            sag,
            EARTH_WIRE,
            seg(12),
        );
    }
}

// ---- the town -----------------------------------------------------------------------

/// What stands on each of the town's lots.
#[derive(Clone, Copy)]
enum Building {
    Office,
    Flats,
    Workshop,
    Shed,
    Warehouse,
    WaterTower,
    Monument,
}

/// The town's buildings, each on its lot: `PropKind::DamTown`'s solid plan,
/// rectangle for rectangle, (centre x, centre y, half x, half y) in metres.
/// Streets at least 24 m wide run between every two of them.
const BUILDINGS: [(Building, (i32, i32, i32, i32)); 10] = [
    (Building::Office, (0, 70, 30, 26)),
    (Building::Flats, (-115, 38, 45, 9)),
    (Building::Flats, (-115, 92, 45, 9)),
    (Building::Flats, (115, 38, 45, 9)),
    (Building::Flats, (115, 92, 45, 9)),
    (Building::Workshop, (-115, -42, 40, 14)),
    (Building::Shed, (-115, -98, 40, 12)),
    (Building::Warehouse, (110, -42, 40, 14)),
    (Building::WaterTower, (110, -98, 8, 8)),
    (Building::Monument, (0, -65, 4, 4)),
];

/// The main street's half width along x through the middle, the cross streets
/// either side of the square, and the square (from the main street to the
/// town's south edge, the monument in it) with the office's forecourt north.
const MAIN_STREET: f32 = 15.0;
const CROSS_STREETS: [(f32, f32); 2] = [(-65.0, -35.0), (35.0, 65.0)];
const SQUARE: f32 = 35.0;
/// How far a building's walls stand in from its lot's edges.
const SETBACK: f32 = 0.3;

/// A lot's rectangle, (x0, y0, x1, y1).
#[derive(Clone, Copy)]
struct Lot {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Lot {
    fn of((cx, cy, hx, hy): (i32, i32, i32, i32)) -> Lot {
        Lot {
            x0: (cx - hx) as f32,
            y0: (cy - hy) as f32,
            x1: (cx + hx) as f32,
            y1: (cy + hy) as f32,
        }
    }

    fn inset(self, d: f32) -> Lot {
        Lot {
            x0: self.x0 + d,
            y0: self.y0 + d,
            x1: self.x1 - d,
            y1: self.y1 - d,
        }
    }

    fn centre(self) -> Vec2 {
        Vec2::new(self.x0 + self.x1, self.y0 + self.y1) * 0.5
    }

    /// The side of the lot toward the main street: outward, and its plane.
    fn street_side(self) -> (Vec2, f32) {
        if self.centre().y < 0.0 {
            (Vec2::Y, self.y1)
        } else {
            (Vec2::NEG_Y, self.y0)
        }
    }
}

/// A flat roof's slab on a box from (`x0`, `y0`) to (`x1`, `y1`) whose walls
/// stop at `z`, a little inside its parapet.
fn flat_roof(b: &mut MeshBuilder, l: Lot, z: f32) {
    b.paint(CONCRETE).pattern(CAST);
    b.block(
        Vec3::new(l.x0 + 0.4, l.y0 + 0.4, z),
        Vec3::new(l.x1 - 0.4, l.y1 - 0.4, z + 0.4),
    );
}

/// The dam's operations town: an office with its tower over the square, four
/// blocks of workers' flats, a workshop with its boiler chimney, a shed, a
/// warehouse, a water tower and a monument to the builders, on paved streets.
fn town(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        town_far(b);
        return;
    }
    let fine = b.fine();
    streets(b);
    for (kind, plan) in BUILDINGS {
        let l = Lot::of(plan).inset(SETBACK);
        match kind {
            Building::Office => office(b, l, fine),
            Building::Flats => flats(b, l, fine),
            Building::Workshop => {
                shed(b, l, 10.0, 4.0, fine);
                b.paint(CONCRETE).pattern(CAST);
                b.prism(
                    Vec3::new(l.x1 - 6.0, l.y0 + 5.0, -0.3),
                    if fine { 10 } else { 6 },
                    1.6,
                    1.1,
                    26.0,
                );
            }
            Building::Shed => shed(b, l, 6.5, 3.0, fine),
            Building::Warehouse => warehouse(b, l, fine),
            Building::WaterTower => water_tower(b, l, fine),
            Building::Monument => monument(b, l),
        }
    }
}

/// The far level: the office and the flats as boxes, the big sheds' roofs.
fn town_far(b: &mut MeshBuilder) {
    for (kind, plan) in BUILDINGS {
        let l = Lot::of(plan).inset(SETBACK);
        let c = l.centre();
        let size = Vec2::new(l.x1 - l.x0, l.y1 - l.y0);
        match kind {
            Building::Office | Building::Flats => {
                let h = if matches!(kind, Building::Office) {
                    16.0
                } else {
                    15.0
                };
                b.paint(CONCRETE).pattern(WHITE);
                b.cuboid_open(c.extend(h * 0.5), size.extend(h));
            }
            Building::Workshop | Building::Warehouse => {
                b.paint(CONCRETE).pattern(ROOF);
                slab(b, l.x0, l.y0, l.x1, l.y1, 11.0);
            }
            _ => {}
        }
    }
}

/// The paving: the main street, the cross streets, the square and the
/// office's forecourt.
fn streets(b: &mut MeshBuilder) {
    let (hx, hy) = (TOWN.half.0 as f32, TOWN.half.1 as f32);
    b.paint(CONCRETE).pattern(CAST);
    slab(b, -hx, -MAIN_STREET, hx, MAIN_STREET, 0.15);
    for (x0, x1) in CROSS_STREETS {
        slab(b, x0, MAIN_STREET, x1, hy, 0.15);
        slab(b, x0, -hy, x1, -MAIN_STREET, 0.15);
    }
    let office = Lot::of(BUILDINGS[0].1);
    slab(b, -SQUARE, -hy + 5.0, SQUARE, -MAIN_STREET, 0.15);
    slab(b, -SQUARE, MAIN_STREET, SQUARE, office.y0, 0.15);
}

/// The office and control building: four storeys, their windows in one band a
/// storey all round, a tower of seven over the entrance on the square's side, and
/// a canopy on pillars before the door.
fn office(b: &mut MeshBuilder, l: Lot, fine: bool) {
    const STOREY: f32 = 4.0;
    let (h, tower_top) = (4.0 * STOREY, 7.0 * STOREY + 2.0);
    let porch = 6.0;
    let body = Lot {
        y0: l.y0 + porch,
        ..l
    };
    let cx = l.centre().x;
    let tower = Lot {
        x0: cx - 7.0,
        y0: body.y0 - 2.0,
        x1: cx + 7.0,
        y1: body.y0 + 12.0,
    };
    b.paint(CONCRETE).pattern(WHITE);
    b.block(
        Vec3::new(body.x0, body.y0, -0.3),
        Vec3::new(body.x1, body.y1, h),
    );
    b.block(
        Vec3::new(tower.x0, tower.y0, -0.3),
        Vec3::new(tower.x1, tower.y1, tower_top),
    );
    flat_roof(b, body, h);
    flat_roof(b, tower, tower_top);
    let storeys = if fine { 4 } else { 2 };
    for (out, plane, from, to) in walls(body.x0, body.y0, body.x1, body.y1) {
        // The front's bands stop either side of the tower.
        let runs: &[(f32, f32)] = if out == Vec2::NEG_Y {
            &[(from + 2.0, tower.x0 - 1.0), (tower.x1 + 1.0, to - 2.0)]
        } else {
            &[(from + 2.0, to - 2.0)]
        };
        for k in 0..storeys {
            let z = STOREY * (k * 4 / storeys) as f32 + 1.3;
            for &(a, c) in runs {
                band(b, out, plane, a, c, z, z + 2.0);
            }
        }
    }
    for (out, plane, from, to) in walls(tower.x0, tower.y0, tower.x1, tower.y1) {
        // Only where the tower stands clear of the body: its front all the way
        // up, over the roof on the other sides.
        let first = if out == Vec2::NEG_Y { 1 } else { 4 };
        let step = if fine { 1 } else { 2 };
        for k in (first..7).step_by(step) {
            let z = STOREY * k as f32 + 1.3;
            band(b, out, plane, from + 2.0, to - 2.0, z, z + 2.0);
        }
    }
    // The doors, and the canopy on its pillars.
    band(b, Vec2::NEG_Y, tower.y0, cx - 3.0, cx + 3.0, 0.2, 3.2);
    b.paint(CONCRETE).pattern(CAST);
    b.block(
        Vec3::new(cx - 12.0, l.y0, 4.6),
        Vec3::new(cx + 12.0, tower.y0, 5.2),
    );
    if fine {
        for x in [-11.0f32, -5.0, 5.0, 11.0] {
            b.block(
                Vec3::new(cx + x - 0.4, l.y0 + 0.3, -0.3),
                Vec3::new(cx + x + 0.4, l.y0 + 1.1, 4.6),
            );
        }
    }
}

/// A block of workers' flats: five storeys, three stairwells up the back with
/// their stair houses on the roof, and a balcony band a storey across the front
/// of each stairwell's section over one glazed band.
fn flats(b: &mut MeshBuilder, l: Lot, fine: bool) {
    const STOREY: f32 = 3.0;
    const STOREYS: usize = 5;
    let h = STOREY * STOREYS as f32;
    let (out, _) = l.street_side();
    // Balconies on the street side, stairwells behind.
    let (front, back) = if out.y > 0.0 {
        (l.y1 - 1.5, l.y0 + 1.5)
    } else {
        (l.y0 + 1.5, l.y1 - 1.5)
    };
    let body = Lot {
        y0: front.min(back),
        y1: front.max(back),
        ..l
    };
    b.paint(CONCRETE).pattern(WHITE);
    b.block(
        Vec3::new(body.x0, body.y0, -0.3),
        Vec3::new(body.x1, body.y1, h),
    );
    flat_roof(b, body, h);
    let section = (l.x1 - l.x0) / 3.0;
    for i in 0..3 {
        let sx0 = l.x0 + section * i as f32;
        let (sx1, mid) = (sx0 + section, sx0 + section * 0.5);
        // The stairwell, standing out behind and over the roof.
        let (sy0, sy1) = if out.y > 0.0 {
            (l.y0, back + 3.0)
        } else {
            (back - 3.0, l.y1)
        };
        b.paint(CONCRETE).pattern(WHITE);
        b.block(
            Vec3::new(mid - 2.2, sy0, -0.3),
            Vec3::new(mid + 2.2, sy1, h + 2.6),
        );
        if fine {
            b.paint(CONCRETE).pattern(CAST);
            b.block(
                Vec3::new(mid - 2.5, sy0, h + 2.6),
                Vec3::new(mid + 2.5, sy1, h + 2.9),
            );
            let stair_plane = if out.y > 0.0 { l.y0 } else { l.y1 };
            band(b, -out, stair_plane, mid - 0.7, mid + 0.7, 1.0, h + 1.0);
            // The back's windows either side of the stairwell.
            for k in 0..STOREYS {
                let z = STOREY * k as f32 + 1.1;
                for (a, c) in [(sx0 + 1.5, mid - 3.0), (mid + 3.0, sx1 - 1.5)] {
                    band(b, -out, back, a, c, z, z + 1.4);
                }
            }
        }
    }
    // The balconies: a solid parapet band a storey from the first floor up, one
    // run a section (one along the whole front further off).
    let (y0, y1) = if out.y > 0.0 {
        (front, l.y1)
    } else {
        (l.y0, front)
    };
    let runs = if fine { 3 } else { 1 };
    let run = (l.x1 - l.x0) / runs as f32;
    b.paint(CONCRETE).pattern(CAST);
    for k in 1..STOREYS {
        let z = STOREY * k as f32;
        for i in 0..runs {
            let x0 = l.x0 + run * i as f32;
            b.block(
                Vec3::new(x0 + 1.0, y0, z - 0.2),
                Vec3::new(x0 + run - 1.0, y1, z + 1.0),
            );
        }
    }
    // The front's glazing, one band a storey behind the balconies.
    for k in 0..STOREYS {
        let z = STOREY * k as f32 + 1.2;
        band(b, out, front, l.x0 + 1.0, l.x1 - 1.0, z, z + 1.5);
    }
    if fine {
        // Water tanks on the roof.
        b.paint(METAL);
        for x in [l.x0 + 12.0, l.x1 - 12.0] {
            let c = Vec3::new(x, (body.y0 + body.y1) * 0.5, h + 0.4);
            b.cylinder_between(c - Vec3::X * 2.5, c + Vec3::X * 2.5, 0.9, 0.9, 8);
        }
    }
}

/// A long gabled shed filling its lot: walls `wall` high, the roof rising
/// `rise` to its ridge along x, big doors to the street and a strip of high
/// windows along both long sides.
fn shed(b: &mut MeshBuilder, l: Lot, wall: f32, rise: f32, fine: bool) {
    b.paint(CONCRETE).pattern(WHITE);
    b.block(Vec3::new(l.x0, l.y0, -0.3), Vec3::new(l.x1, l.y1, wall));
    b.paint(CONCRETE).pattern(ROOF);
    let cy = l.centre().y;
    b.extrude_x(&[[l.y0, wall], [l.y1, wall], [cy, wall + rise]], l.x0, l.x1);
    let (out, plane) = l.street_side();
    let doors = 4;
    let bay = (l.x1 - l.x0) / doors as f32;
    for i in 0..doors {
        let x = l.x0 + bay * (i as f32 + 0.5);
        band(b, out, plane, x - 3.5, x + 3.5, 0.1, (wall - 2.0).min(6.0));
    }
    if fine {
        for (out, plane) in [(Vec2::NEG_Y, l.y0), (Vec2::Y, l.y1)] {
            band(
                b,
                out,
                plane,
                l.x0 + 2.0,
                l.x1 - 2.0,
                wall - 2.2,
                wall - 1.0,
            );
        }
    }
}

/// The warehouse: a shed set back from the street behind its loading dock, a
/// canopy over the dock.
fn warehouse(b: &mut MeshBuilder, l: Lot, fine: bool) {
    let (out, plane) = l.street_side();
    let dock = 3.5;
    let body = if out.y > 0.0 {
        Lot {
            y1: l.y1 - dock,
            ..l
        }
    } else {
        Lot {
            y0: l.y0 + dock,
            ..l
        }
    };
    shed(b, body, 9.0, 3.0, fine);
    let (y0, y1) = if out.y > 0.0 {
        (body.y1, plane)
    } else {
        (plane, body.y0)
    };
    b.paint(CONCRETE).pattern(CAST);
    b.block(
        Vec3::new(l.x0 + 6.0, y0, -0.3),
        Vec3::new(l.x1 - 6.0, y1, 1.2),
    );
    b.paint(CONCRETE).pattern(ROOF);
    b.block(
        Vec3::new(l.x0 + 4.0, y0, 6.2),
        Vec3::new(l.x1 - 4.0, y1, 6.6),
    );
}

/// The water tower: a concrete shaft, the tank on a coned underside, a low
/// conical cap.
fn water_tower(b: &mut MeshBuilder, l: Lot, fine: bool) {
    let c = l.centre().extend(0.0);
    let r = (l.x1 - l.x0) * 0.5 - 0.2;
    let sides = if fine { 12 } else { 8 };
    b.paint(CONCRETE).pattern(CAST);
    b.prism(c - Vec3::Z * 0.3, sides, 3.0, 2.6, 22.3);
    b.paint(CONCRETE).pattern(WHITE);
    b.prism(c + Vec3::Z * 22.0, sides, 2.6, r, 3.0);
    b.prism(c + Vec3::Z * 25.0, sides, r, r, 6.5);
    b.paint(CONCRETE).pattern(ROOF);
    b.prism(c + Vec3::Z * 31.5, sides, r, 1.0, 2.2);
    if fine {
        // Its gallery's rail.
        b.paint(METAL);
        b.prism(c + Vec3::Z * 25.05, sides, r + 0.15, r + 0.15, 0.3);
    }
}

/// The builders' monument in the square: a stepped plinth and a stele.
fn monument(b: &mut MeshBuilder, l: Lot) {
    let c = l.centre();
    b.paint(CONCRETE).pattern(CAST);
    b.block(Vec3::new(l.x0, l.y0, -0.3), Vec3::new(l.x1, l.y1, 0.6));
    b.block(
        Vec3::new(l.x0 + 1.0, l.y0 + 1.0, 0.6),
        Vec3::new(l.x1 - 1.0, l.y1 - 1.0, 1.2),
    );
    b.paint(CONCRETE).pattern(WHITE);
    b.block(
        Vec3::new(c.x - 0.9, c.y - 1.6, 1.2),
        Vec3::new(c.x + 0.9, c.y + 1.6, 13.0),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model, MeshLod};
    use mc_map::PropKind;

    /// A vertex within `r` of `p`.
    fn reaches(mesh: &MeshLod, p: Vec3, r: f32) -> bool {
        mesh.vertices
            .iter()
            .any(|v| (Vec3::from(v.pos) - p).length() <= r + 1e-3)
    }

    fn furthest_x(mesh: &MeshLod) -> f32 {
        mesh.vertices
            .iter()
            .map(|v| v.pos[0])
            .fold(f32::MIN, f32::max)
    }

    /// A tower's clamps and peaks stand at the line's phases and earth; its
    /// span's wires leave them (about the pivot) and end at the same (y, z) a
    /// span on along +x: the next tower's clamps.
    #[test]
    fn a_span_runs_from_a_towers_clamps_to_the_next() {
        let tower = build_model("landmark_pylon").unwrap();
        let wires = build_model("landmark_span").unwrap();
        let span = LINE.span as f32;
        let pivot = Vec3::Z * LINE.pivot as f32;
        // Each level's wires, as fat as that level draws them.
        for ((mesh, strung), fat) in tower.lods[..2].iter().zip(&wires.lods).zip(SPAN_FAT) {
            for s in [-1.0f32, 1.0] {
                let clamps = phases()
                    .map(|p| (p, WIRE))
                    .into_iter()
                    .chain([(earth(), EARTH_WIRE)]);
                for (p, r) in clamps {
                    let clamp = Vec3::new(0.0, s * p.x, p.y);
                    assert!(reaches(mesh, clamp, 0.5), "no clamp at {clamp}");
                    let r = r * fat;
                    assert!(reaches(strung, clamp - pivot, r), "no wire at {clamp}");
                    let far = clamp - pivot + Vec3::X * span;
                    assert!(reaches(strung, far, r), "{clamp} +span");
                }
            }
            let far = furthest_x(strung);
            assert!((far - span).abs() < WIRE * fat, "reaches {far}");
            assert!(furthest_x(mesh) < 20.0, "the tower carries no wires");
        }
        // The feet stand inside the tower's solid plan.
        let &[(0, 0, hx, hy)] = PropKind::DamPylon.solid_plan() else {
            panic!("the tower's plan is one square round its foot");
        };
        for mesh in &tower.lods {
            for v in &mesh.vertices {
                if v.pos[2] < 2.0 {
                    assert!(v.pos[0].abs() <= hx as f32 && v.pos[1].abs() <= hy as f32);
                }
            }
        }
    }

    /// The yard's first span ends at the first tower's clamps.
    #[test]
    fn the_yard_carries_its_line_to_the_first_tower() {
        let model = build_model("landmark_switchyard").unwrap();
        for mesh in &model.lods[..2] {
            for s in [-1.0f32, 1.0] {
                for p in phases() {
                    let clamp = Vec3::new(FIRST, s * p.x, p.y);
                    assert!(reaches(mesh, clamp, WIRE), "no wire to {clamp}");
                }
                let peak = Vec3::new(FIRST, s * earth().x, earth().y);
                assert!(reaches(mesh, peak, EARTH_WIRE), "no earth wire to {peak}");
            }
            let far = furthest_x(mesh);
            assert!((far - FIRST).abs() < WIRE, "reaches {far}");
        }
        const {
            assert!(DEAD_END_X + STRAIN < YARD_X && YARD_X < FIRST);
            assert!(DEAD_END_COLUMN > 14.0 && TRANSFORMER_BAY + 2.0 * PHASE_GAP < YARD_Y);
        }
    }

    /// The town is built to the path plan: every piece above the paving stands
    /// on one of its buildings' lots, every lot has its building, and streets at
    /// least 24 m wide part every two of them.
    #[test]
    fn the_town_stands_on_its_plan() {
        let plan: Vec<_> = BUILDINGS.iter().map(|&(_, lot)| lot).collect();
        assert_eq!(plan, PropKind::DamTown.solid_plan());
        let lots: Vec<Lot> = plan.iter().map(|&p| Lot::of(p)).collect();
        let (hx, hy) = (TOWN.half.0 as f32, TOWN.half.1 as f32);
        for l in &lots {
            assert!(l.x0 >= -hx && l.x1 <= hx && l.y0 >= -hy && l.y1 <= hy);
        }
        for (i, a) in lots.iter().enumerate() {
            for c in &lots[i + 1..] {
                let gap = (c.x0 - a.x1)
                    .max(a.x0 - c.x1)
                    .max(c.y0 - a.y1)
                    .max(a.y0 - c.y1);
                assert!(
                    gap >= 24.0,
                    "{:?} and {:?}: {gap} m apart",
                    a.centre(),
                    c.centre()
                );
            }
        }
        let model = build_model("landmark_town").unwrap();
        for (lod, mesh) in model.lods.iter().enumerate() {
            let mut used = vec![false; lots.len()];
            for v in &mesh.vertices {
                let [x, y, z] = v.pos;
                if z <= 0.3 {
                    continue;
                }
                let on = lots.iter().position(|l| {
                    (l.x0 - 1e-3..=l.x1 + 1e-3).contains(&x)
                        && (l.y0 - 1e-3..=l.y1 + 1e-3).contains(&y)
                });
                let Some(on) = on else {
                    panic!("lod {lod}: ({x}, {y}, {z}) off every lot");
                };
                used[on] = true;
            }
            if lod == 0 {
                assert!(used.iter().all(|&u| u), "an empty lot: {used:?}");
            }
        }
    }
}
