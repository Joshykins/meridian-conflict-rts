//! Survival mode's hostile machinery: the Replication Engine and the Replication
//! Nodes it raises, both Precursor work. Not Aster: pale alloy that reads as dressed
//! stone (`PRECURSOR`, whose faces the shader cuts into angular panels with light let
//! into some of the grooves, `pattern::PRECURSOR`), deep dark recesses and joints
//! between the plates (`PRECURSOR_DARK`), and one colour, the cold blue-white of
//! replication (`GLOW_PRECURSOR`), wherever matter is made or carried. Pieces float
//! apart with clean gaps; nothing is riveted, nothing is a pipe, every edge is chamfered.
//!
//! The engine is a monument, grounded: a stepped platform of sector slabs, a dark core
//! column with light running up its faces, two rings of floating arc plates round it,
//! eight jointed arms reaching out to the bay projectors, and a hovering crown holding
//! the ray's crystal, the Suppression Lance on a collar under it. The node is the
//! engine's engineer: a spine with its light, a crown of C-shaped arcs, four claw arms
//! hanging from it, a turning hologram disc and a pointed base, all hovering clear of
//! the ground.
//!
//! Contract with the sim (`data/factions/aster/units/survival.ron`; model space:
//! x forward, y left, z up, origin on the ground at the centre):
//! - Engine ray emitter: [`ENGINE_RAY_EMITTER`].
//! - Engine bay k at k * 45 degrees (counter-clockwise from +x): the projector
//!   head at radius [`BAY_PROJECTOR_RADIUS`], height [`BAY_PROJECTOR_Z`]; the
//!   printed unit stands on the ground at radius [`BAY_PRINT_RADIUS`].
//! - Lance muzzle (40, 0, 118), turning about the model's z axis.
//! - Node: ray lands on the crown at [`NODE_RAY_CATCH`]; prints from
//!   [`NODE_PRINT_EMITTER`] onto the ground at (+50, 0).

use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3};

use super::builder::{ngon, MeshBuilder, Section};
use super::library::ModelDef;
use super::material::*;
use super::part;

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("replication_engine", 110.0, 150.0, engine),
    ModelDef::new("replication_node", 26.0, 46.0, node),
];

/// Where the engine's node-raising ray leaves the crown.
pub const ENGINE_RAY_EMITTER: [f32; 3] = [0.0, 0.0, 140.0];
/// Print bays: eight, the first facing +x.
pub const BAY_COUNT: usize = 8;
pub const BAY_PROJECTOR_RADIUS: f32 = 100.0;
pub const BAY_PROJECTOR_Z: f32 = 55.0;
pub const BAY_PRINT_RADIUS: f32 = 150.0;
/// The Suppression Lance's muzzle (the blueprint's).
const LANCE_MUZZLE: Vec3 = Vec3::new(40.0, 0.0, 118.0);
/// Full-detail budget: the engine is one 240 m landmark per match.
#[cfg(test)]
pub const ENGINE_TRIANGLES: usize = 12000;
/// Where the ray lands on a node, and where a node prints from.
pub const NODE_RAY_CATCH: [f32; 3] = [0.0, 0.0, 46.0];
pub const NODE_PRINT_EMITTER: [f32; 3] = [0.0, 0.0, 40.0];

fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn deg(d: f32) -> f32 {
    d * PI / 180.0
}

fn pale(b: &mut MeshBuilder) {
    b.paint(PRECURSOR);
}

fn dark(b: &mut MeshBuilder) {
    b.paint(PRECURSOR_DARK);
}

fn light(b: &mut MeshBuilder) {
    b.paint(GLOW_PRECURSOR);
}

// ---- Precursor forms ----------------------------------------------------------

/// The convex plan `poly` (counter-clockwise) with every edge moved in by `d`.
fn inset(poly: &[Vec2], d: f32) -> Vec<Vec2> {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (prev, p, next) = (poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]);
            let n1 = (p - prev).normalize().perp();
            let n2 = (next - p).normalize().perp();
            p + (n1 + n2) * (d / (1.0 + n1.dot(n2)))
        })
        .collect()
}

/// A slab on the convex plan `poly` (counter-clockwise) from `z0` to `z1`, its top edges
/// chamfered by `c`, and its bottom edges too when it `floats`. Square below full detail.
fn slab(b: &mut MeshBuilder, poly: &[Vec2], z0: f32, z1: f32, c: f32, floats: bool) {
    let ring = |p: &[Vec2], z: f32| p.iter().map(|q| q.extend(z)).collect::<Vec<_>>();
    if !b.fine() || c <= 0.0 {
        b.loft(&[ring(poly, z0), ring(poly, z1)], true, true);
        return;
    }
    let top = inset(poly, c);
    let mut rings = Vec::with_capacity(4);
    if floats {
        rings.push(ring(&top, z0));
        rings.push(ring(poly, z0 + c));
    } else {
        rings.push(ring(poly, z0));
    }
    rings.push(ring(poly, z1 - c));
    rings.push(ring(&top, z1));
    b.loft(&rings, true, true);
}

/// Plan of the part of a ring between radii `r0` and `r1` from angle `a0` to `a1`
/// (counter-clockwise, convex): the outer edge in `steps` chords, the inner one straight.
fn sector(r0: f32, r1: f32, a0: f32, a1: f32, steps: usize) -> Vec<Vec2> {
    let at = |r: f32, a: f32| v2(a.cos() * r, a.sin() * r);
    let mut p = vec![at(r0, a0)];
    for i in 0..=steps {
        p.push(at(r1, a0 + (a1 - a0) * i as f32 / steps as f32));
    }
    p.push(at(r0, a1));
    p
}

/// An octagon of vertex radius `r` as a plan, flats toward +x.
fn octagon(r: f32) -> Vec<Vec2> {
    ngon(8, r).iter().map(|p| v2(p[0], p[1])).collect()
}

/// A section in (radius, z) from its corners (inner foot, outer foot, outer top, inner
/// top), each corner cut by `c` (none below full detail).
fn section(b: &MeshBuilder, q: [(f32, f32); 4], c: f32) -> Vec<Vec2> {
    let q = q.map(|(r, z)| v2(r, z));
    if !b.fine() || c <= 0.0 {
        return q.to_vec();
    }
    let mut out = Vec::with_capacity(8);
    for i in 0..4 {
        let (prev, p, next) = (q[(i + 3) % 4], q[i], q[(i + 1) % 4]);
        out.push(p + (prev - p).normalize() * c);
        out.push(p + (next - p).normalize() * c);
    }
    out
}

/// A section in (radius, z) as given, corners and all.
fn outline(points: &[(f32, f32)]) -> Vec<Vec2> {
    points.iter().map(|&(r, z)| v2(r, z)).collect()
}

/// `section` swept round the z axis from `a0` to `a1` in `steps`, its ends dropped by
/// `droop` below its middle: a Precursor arc plate, a C seen from the side.
fn arc(b: &mut MeshBuilder, section: &[Vec2], a0: f32, a1: f32, steps: usize, droop: f32) {
    let steps = steps.max(1);
    let rings: Vec<Vec<Vec3>> = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let a = a0 + (a1 - a0) * t;
            let dz = -droop * (2.0 * t - 1.0).abs().powi(3);
            section.iter().map(|s| v3(a.cos() * s.x, a.sin() * s.x, s.y + dz)).collect()
        })
        .collect();
    b.loft(&rings, true, true);
}

/// A thin line of light along an arc plate: `section` (a small one) over the middle
/// `share` of an arc of half-angle `half` about `mid`, drooping with it.
fn arc_light(b: &mut MeshBuilder, section: &[Vec2], mid: f32, half: f32, share: f32, steps: usize, droop: f32) {
    light(b);
    arc(b, section, mid - half * share, mid + half * share, steps, droop * share.powi(3));
}

/// A bar from `p` to `q` with its long corners chamfered; `a` and `z` are (across,
/// thickness) at either end, as for `MeshBuilder::beam`, which it is below full detail.
fn bar(b: &mut MeshBuilder, p: Vec3, q: Vec3, a: Vec2, z: Vec2) {
    if !b.fine() {
        b.beam(p, q, a, z);
        return;
    }
    let axis = (q - p).normalize();
    let reference = if axis.y.abs() < 0.999 { Vec3::Y } else { Vec3::X };
    let side = (reference - axis * reference.dot(axis)).normalize();
    let up = axis.cross(side);
    let ring = |c: Vec3, s: Vec2| -> Vec<Vec3> {
        let k = s.min_element() * 0.24;
        let (w, h) = (s.x * 0.5, s.y * 0.5);
        [(-w + k, -h), (w - k, -h), (w, -h + k), (w, h - k), (w - k, h), (-w + k, h), (-w, h - k), (-w, -h + k)]
            .iter()
            .map(|&(x, y)| c + side * x + up * y)
            .collect()
    };
    b.loft(&[ring(p, a), ring(q, z)], true, true);
}

// ---- The Replication Engine ------------------------------------------------------

/// The dark pad under everything: an octagon of this vertex radius, flats to the bays.
const PAD_R: f32 = 124.0;
const PAD_TOP: f32 = 3.0;
/// The print beds: a sector slab under each bay, clean gaps between them.
const BED_IN: f32 = 40.0;
const BED_OUT: f32 = 116.0;
const BED_TOP: f32 = 10.0;
const BED_HALF: f32 = 11.0;
/// The step over each gap between the beds.
const STEP_IN: f32 = 28.0;
const STEP_OUT: f32 = 66.0;
const STEP_TOP: f32 = 21.0;
const STEP_HALF: f32 = 14.0;
/// The core column: (z, vertex radius) up its taper.
const CORE: [(f32, f32); 3] = [(PAD_TOP - 0.1, 21.0), (100.0, 14.0), (128.0, 11.5)];
/// The Lance's collar: turret pivot height and its top.
const COLLAR_LOW: f32 = 108.0;
const COLLAR_HIGH: f32 = 120.0;
/// The halo that turns over the crystal.
const HALO_Z: f32 = 159.0;

/// Vertex radius of the core at height `z`.
fn core_radius(z: f32) -> f32 {
    let [a, m, t] = CORE;
    let lerp = |p: (f32, f32), q: (f32, f32)| p.1 + (q.1 - p.1) * ((z - p.0) / (q.0 - p.0));
    if z <= m.0 {
        lerp(a, m)
    } else {
        lerp(m, t)
    }
}

pub fn engine(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(v3(0.0, 0.0, COLLAR_LOW));
    if b.coarse() {
        engine_coarse(b);
        return;
    }
    b.set_spinner_pivot(v3(0.0, 0.0, HALO_Z));
    platform(b);
    core(b);
    arc_rings(b);
    for k in 0..BAY_COUNT {
        b.yawed(Vec3::ZERO, k as f32 * TAU / BAY_COUNT as f32, bay_arm);
    }
    crown(b);
    b.with_part(part::TURRET, lance);
}

/// The platform: a dark pad, a pale bed under each bay and a step over each gap between
/// the beds, lights in the gaps and guide lights down each bed.
fn platform(b: &mut MeshBuilder) {
    dark(b);
    slab(b, &octagon(PAD_R), 0.0, PAD_TOP, 0.0, false);
    let chords = if b.fine() { 3 } else { 1 };
    for k in 0..BAY_COUNT {
        let a = k as f32 * TAU / BAY_COUNT as f32;
        let m = a + TAU / 16.0;
        pale(b);
        slab(b, &sector(BED_IN, BED_OUT, a - deg(BED_HALF), a + deg(BED_HALF), chords), PAD_TOP, BED_TOP, 1.6, false);
        slab(b, &sector(STEP_IN, STEP_OUT, m - deg(STEP_HALF), m + deg(STEP_HALF), chords), BED_TOP, STEP_TOP, 1.4, false);
        if b.fine() {
            light(b);
            // Down the gap between two beds, where the step does not cover it.
            b.yawed(Vec3::ZERO, m, |b| b.block(v3(STEP_OUT + 3.0, -0.6, PAD_TOP - 0.2), v3(BED_OUT - 6.0, 0.6, PAD_TOP + 0.3)));
            // Down the bed to where the printed unit stands up.
            b.yawed(Vec3::ZERO, a, |b| {
                for y in [-7.0f32, 7.0] {
                    b.block(v3(78.0, y - 0.5, BED_TOP - 0.2), v3(111.0, y + 0.5, BED_TOP + 0.3));
                }
            });
        }
    }
    if b.fine() {
        // Light pooled on the pad round the core's foot.
        light(b);
        let r = core_radius(PAD_TOP) + 1.6;
        b.loft_z(&ngon(8, 1.0), &[Section::new(PAD_TOP - 0.2, r), Section::new(PAD_TOP + 0.4, r)]);
    }
}

/// The core: a dark column with a line of light up each flat (the feed each arm draws
/// from), and pale sheath plates standing off it over the corners, two to a corner.
fn core(b: &mut MeshBuilder) {
    dark(b);
    let oct = ngon(8, 1.0);
    let sections: Vec<Section> = CORE.iter().map(|&(z, r)| Section::new(z, r)).collect();
    b.loft_z(&oct, &sections);
    light(b);
    let flat = |z: f32| core_radius(z) * (TAU / 16.0).cos() + 0.25;
    for k in 0..BAY_COUNT {
        b.yawed(Vec3::ZERO, k as f32 * TAU / BAY_COUNT as f32, |b| {
            b.beam(v3(flat(6.0), 0.0, 6.0), v3(flat(100.0), 0.0, 100.0), v2(1.8, 0.5), v2(1.5, 0.5));
            b.beam(v3(flat(100.0), 0.0, 100.0), v3(flat(126.0), 0.0, 126.0), v2(1.5, 0.5), v2(1.4, 0.5));
        });
    }
    pale(b);
    let steps = if b.fine() { 3 } else { 1 };
    let lower = section(b, [(22.2, 22.0), (31.5, 22.0), (27.5, 64.0), (19.5, 64.0)], 1.3);
    let upper = section(b, [(19.2, 67.0), (27.0, 67.0), (20.5, 104.0), (15.4, 104.0)], 1.1);
    for k in 0..BAY_COUNT {
        let m = (k as f32 + 0.5) * TAU / BAY_COUNT as f32;
        arc(b, &lower, m - deg(16.0), m + deg(16.0), steps, 0.0);
        arc(b, &upper, m - deg(15.0), m + deg(15.0), steps, 0.0);
    }
}

/// Two rings of floating arc plates round the core, over the gaps between the arms,
/// each a C drooping at its ends with a line of light along it.
fn arc_rings(b: &mut MeshBuilder) {
    let (great_steps, upper_steps) = if b.fine() { (5, 3) } else { (3, 2) };
    let great = section(b, [(38.0, 76.0), (72.0, 67.0), (72.0, 73.0), (38.0, 86.0)], 1.4);
    let upper = section(b, [(30.0, 99.0), (50.0, 95.0), (50.0, 99.0), (30.0, 106.0)], 1.2);
    for k in 0..BAY_COUNT {
        let m = (k as f32 + 0.5) * TAU / BAY_COUNT as f32;
        pale(b);
        arc(b, &great, m - deg(19.0), m + deg(19.0), great_steps, 7.0);
        arc(b, &upper, m - deg(13.0), m + deg(13.0), upper_steps, 2.5);
        // The light along each outer face's foot.
        let strip = section(b, [(72.1, 68.5), (72.8, 68.5), (72.8, 70.0), (72.1, 70.0)], 0.0);
        arc_light(b, &strip, m, deg(19.0), 0.82, great_steps, 7.0);
        if b.fine() {
            let strip = section(b, [(50.1, 95.8), (50.6, 95.8), (50.6, 97.0), (50.1, 97.0)], 0.0);
            arc_light(b, &strip, m, deg(13.0), 0.8, upper_steps, 2.5);
        }
    }
}

/// One bay's arm, facing +x: a socket out of the core's light to the shoulder, an upper
/// arm under a floating guard plate, the elbow, a forearm down to the projector head
/// aimed at the printed unit's middle, its lens between two claws.
fn bay_arm(b: &mut MeshBuilder) {
    let head = v3(BAY_PROJECTOR_RADIUS, 0.0, BAY_PROJECTOR_Z);
    let aim = (v3(BAY_PRINT_RADIUS, 0.0, 6.0) - head).normalize();
    let wrist = head - aim * 9.0;
    let sides = b.sides(8);
    let (shoulder, elbow) = (v3(30.0, 0.0, 96.0), v3(74.0, 0.0, 86.0));
    dark(b);
    b.beam(v3(10.0, 0.0, 96.0), shoulder, v2(3.4, 4.0), v2(3.4, 4.0));
    b.cylinder_between(shoulder - Vec3::Y * 4.2, shoulder + Vec3::Y * 4.2, 5.2, 5.2, sides);
    b.cylinder_between(elbow - Vec3::Y * 3.8, elbow + Vec3::Y * 3.8, 4.6, 4.6, sides);
    pale(b);
    let (u0, u1) = (v3(32.0, 0.0, 98.5), v3(71.0, 0.0, 88.5));
    bar(b, u0, u1, v2(7.5, 7.0), v2(6.5, 6.0));
    bar(b, v3(75.5, 0.0, 83.0), wrist, v2(6.5, 6.5), v2(6.0, 5.5));
    // The head: its housing, the lens at the mouth.
    bar(b, wrist, head - aim * 1.5, v2(9.0, 9.0), v2(8.0, 8.0));
    light(b);
    b.cylinder_between(head - aim * 1.7, head + aim * 0.6, 3.4, 2.6, sides);
    // The light the arm carries, along its top, in the gap under the guard plate.
    let up = Vec3::Y.cross(u1 - u0).normalize() * -1.0;
    let up = if up.z < 0.0 { -up } else { up };
    b.beam(u0.lerp(u1, 0.04) + up * 3.35, u0.lerp(u1, 0.96) + up * 2.85, v2(1.5, 0.5), v2(1.3, 0.5));
    if b.fine() {
        pale(b);
        bar(b, u0.lerp(u1, 0.12) + up * 5.3, u0.lerp(u1, 0.88) + up * 4.7, v2(9.0, 1.4), v2(7.6, 1.2));
        // The owner's colour on the guard plate, seen from above.
        b.paint(TEAM);
        b.beam(u0.lerp(u1, 0.4) + up * 6.05, u0.lerp(u1, 0.6) + up * 5.9, v2(1.0, 0.3), v2(1.0, 0.3));
        // Two claws either side of the lens.
        pale(b);
        for s in [-1.0f32, 1.0] {
            bar(b, head - aim * 4.0 + Vec3::Y * (s * 4.6), head + aim * 5.5 + Vec3::Y * (s * 2.0), v2(1.6, 2.4), v2(0.3, 0.4));
        }
    }
}

/// The crown over the core: a floating cup, the ray's crystal in it, four tall arc
/// blades round it with light down their inner faces, a halo of light turning over it
/// and a spire floating above everything.
fn crown(b: &mut MeshBuilder) {
    let emitter = Vec3::from(ENGINE_RAY_EMITTER);
    let sides = b.sides(8);
    let oct = ngon(sides, 1.0);
    pale(b);
    b.loft_z(&oct, &[Section::new(129.5, 5.0), Section::new(133.0, 10.5), Section::new(135.0, 10.5)]);
    b.loft_z(&oct, &[Section::new(161.5, 1.2), Section::new(164.0, 5.0), Section::new(168.0, 4.6), Section::new(184.0, 0.0)]);
    let steps = if b.fine() { 4 } else { 2 };
    let blade = section(b, [(12.5, 133.0), (19.5, 131.0), (22.0, 151.0), (15.5, 157.0)], 1.0);
    let inner = section(b, [(12.3, 137.0), (12.9, 137.0), (14.9, 151.0), (14.3, 151.0)], 0.0);
    for k in 0..4 {
        let m = (k as f32 + 0.5) * TAU / 4.0;
        pale(b);
        arc(b, &blade, m - deg(19.0), m + deg(19.0), steps, 3.0);
        if b.fine() {
            arc_light(b, &inner, m, deg(19.0), 0.75, steps, 3.0);
        }
    }
    light(b);
    // The crystal the ray leaves from: a long diamond.
    b.spheroid(emitter, v3(5.5, 5.5, 9.5), 4, 2);
    if b.mid() {
        let ring = section(b, [(9.0, HALO_Z - 0.4), (10.4, HALO_Z - 0.4), (10.4, HALO_Z + 0.4), (9.0, HALO_Z + 0.4)], 0.0);
        let arcs = if b.fine() { 3 } else { 2 };
        b.with_part(part::SPINNER, |b| {
            for j in 0..arcs {
                let a0 = j as f32 * TAU / arcs as f32;
                arc(b, &ring, a0, a0 + TAU / arcs as f32 - deg(22.0), if b.fine() { 6 } else { 3 }, 0.0);
            }
        });
    }
}

/// The Suppression Lance on its collar (`part::TURRET`): seven pale blocks on a dark
/// ring round the core, and where the eighth would be, the Lance's housing with two
/// blades out to the muzzle and the bore lit between them.
fn lance(b: &mut MeshBuilder) {
    dark(b);
    b.prism(v3(0.0, 0.0, COLLAR_LOW), b.sides(8), 15.5, 15.0, COLLAR_HIGH - COLLAR_LOW);
    pale(b);
    let block = section(b, [(15.8, COLLAR_LOW + 0.5), (24.5, COLLAR_LOW + 1.5), (24.5, COLLAR_HIGH - 0.5), (15.8, COLLAR_HIGH + 0.5)], 1.0);
    let steps = if b.fine() { 2 } else { 1 };
    for k in 1..BAY_COUNT {
        let m = k as f32 * TAU / BAY_COUNT as f32;
        arc(b, &block, m - deg(19.0), m + deg(19.0), steps, 0.0);
    }
    if b.mid() {
        light(b);
        b.prism(v3(0.0, 0.0, COLLAR_LOW + 5.5), b.sides(8), 15.9, 15.9, 1.0);
    }
    pale(b);
    bar(b, v3(13.0, 0.0, 114.5), v3(30.0, 0.0, 117.5), v2(12.0, 11.0), v2(8.5, 8.0));
    for y in [-1.0f32, 1.0] {
        bar(b, v3(24.0, 2.9 * y, 118.0), v3(LANCE_MUZZLE.x, 2.3 * y, LANCE_MUZZLE.z), v2(2.2, 6.0), v2(1.4, 3.0));
    }
    light(b);
    b.cylinder_between(v3(26.0, 0.0, LANCE_MUZZLE.z), LANCE_MUZZLE, 1.0, 0.8, b.sides(6));
}

/// A handful of solids: platform, arc ring, core, crystal, the Lance.
fn engine_coarse(b: &mut MeshBuilder) {
    pale(b);
    b.cuboid(v3(0.0, 0.0, BED_TOP * 0.5), v3(210.0, 210.0, BED_TOP));
    b.yawed(Vec3::ZERO, TAU / 8.0, |b| {
        b.frustum(v3(0.0, 0.0, 56.0), v2(120.0, 120.0), v2(70.0, 70.0), 48.0, Vec2::ZERO);
    });
    dark(b);
    b.frustum(v3(0.0, 0.0, BED_TOP), v2(32.0, 32.0), v2(18.0, 18.0), 118.0, Vec2::ZERO);
    light(b);
    b.spheroid(Vec3::from(ENGINE_RAY_EMITTER), v3(9.0, 9.0, 14.0), 4, 2);
    b.with_part(part::TURRET, |b| {
        pale(b);
        b.beam(v3(0.0, 0.0, LANCE_MUZZLE.z), LANCE_MUZZLE, v2(10.0, 8.0), v2(4.0, 3.0));
    });
}

// ---- The Replication Node ---------------------------------------------------------

/// Where the node's hologram turns.
const HOLO_Z: f32 = 12.5;

pub fn node(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        node_coarse(b);
        return;
    }
    b.set_spinner_pivot(v3(0.0, 0.0, HOLO_Z));
    spine(b);
    body(b);
    for k in 0..4 {
        let m = TAU / 8.0 + k as f32 * TAU / 4.0;
        node_crown(b, m);
        b.yawed(Vec3::ZERO, m, claw_arm);
    }
    if b.fine() {
        // Pods floating low between the arms, each on a dark pin.
        for k in 0..4 {
            b.yawed(Vec3::ZERO, k as f32 * TAU / 4.0, |b| {
                pale(b);
                let plan = [v2(16.0, -1.8), v2(18.4, -1.8), v2(18.4, 1.8), v2(16.0, 1.8)];
                slab(b, &plan, 8.5, 12.5, 0.45, true);
                dark(b);
                b.beam(v3(13.2, 0.0, 10.5), v3(16.1, 0.0, 10.5), v2(0.6, 0.6), v2(0.6, 0.6));
            });
        }
    }
}

/// The spine: dark, a line of light up four faces, pale sheath plates over the rest;
/// the print band round it, the catch crystal where it opens and the spire over that.
fn spine(b: &mut MeshBuilder) {
    let oct = ngon(8, 1.0);
    dark(b);
    b.loft_z(&oct, &[Section::new(19.5, 3.4), Section::new(43.5, 2.5)]);
    light(b);
    let flat = |z: f32| (3.4 + (2.5 - 3.4) * (z - 19.5) / 24.0) * (TAU / 16.0).cos() + 0.12;
    for k in 0..4 {
        b.yawed(Vec3::ZERO, k as f32 * TAU / 4.0, |b| {
            b.beam(v3(flat(20.5), 0.0, 20.5), v3(flat(43.0), 0.0, 43.0), v2(0.7, 0.3), v2(0.6, 0.3));
        });
    }
    // The print band: where a node prints from.
    let print = Vec3::from(NODE_PRINT_EMITTER);
    b.loft_z(&oct, &[Section::new(print.z - 0.6, 3.2), Section::new(print.z + 0.6, 3.2)]);
    pale(b);
    let sheath = section(b, [(3.0, 30.5), (4.8, 31.0), (4.2, 38.4), (2.9, 38.6)], 0.4);
    for k in 0..4 {
        let m = TAU / 8.0 + k as f32 * TAU / 4.0;
        arc(b, &sheath, m - deg(30.0), m + deg(30.0), if b.fine() { 2 } else { 1 }, 0.0);
    }
    // The catch, and the spire floating over it.
    light(b);
    b.spheroid(Vec3::from(NODE_RAY_CATCH), v3(1.7, 1.7, 2.6), 4, 2);
    pale(b);
    b.loft_z(&oct, &[Section::new(49.0, 0.6), Section::new(50.5, 2.2), Section::new(56.0, 0.0)]);
}

/// Hat, core, the body pointing down, the turning hologram and the floating base.
fn body(b: &mut MeshBuilder) {
    let oct = ngon(8, 1.0);
    pale(b);
    slab(b, &octagon(8.0), 26.5, 29.2, 0.8, true);
    // The owner's colour, let into the hat's top.
    b.paint(TEAM);
    for k in 0..4 {
        b.yawed(Vec3::ZERO, k as f32 * TAU / 4.0, |b| b.block(v3(4.6, -0.7, 29.15), v3(6.2, 0.7, 29.4)));
    }
    dark(b);
    b.loft_z(&oct, &[Section::new(19.5, 5.5), Section::new(26.0, 6.5)]);
    light(b);
    b.loft_z(&oct, &[Section::new(22.4, 6.25), Section::new(23.9, 6.45)]);
    pale(b);
    b.loft_z(&oct, &[Section::new(15.0, 1.2), Section::new(17.0, 4.0), Section::new(19.6, 5.3)]);
    // The hologram: broken rings of light turning under the body.
    let rings: &[f32] = if b.fine() { &[4.0, 8.0] } else { &[7.0] };
    b.with_part(part::SPINNER, |b| {
        light(b);
        for (i, &r) in rings.iter().enumerate() {
            let ring = section(b, [(r - 0.3, HOLO_Z - 0.1), (r + 0.3, HOLO_Z - 0.1), (r + 0.3, HOLO_Z + 0.1), (r - 0.3, HOLO_Z + 0.1)], 0.0);
            for j in 0..3 {
                let a0 = j as f32 * TAU / 3.0 + i as f32 * 0.6;
                arc(b, &ring, a0, a0 + deg(96.0), if b.fine() { 4 } else { 2 }, 0.0);
            }
        }
    });
    // The base: a plate, a point under it, hovering.
    pale(b);
    slab(b, &octagon(6.5), 8.0, 10.0, 0.5, true);
    dark(b);
    b.loft_z(&oct, &[Section::new(2.5, 0.0), Section::new(8.05, 3.6)]);
}

/// One quarter of the crown at angle `m`: a great arc over the arm, a smaller one over
/// it, each a C with a line of light along the great one's outer face.
fn node_crown(b: &mut MeshBuilder, m: f32) {
    let (great_steps, small_steps) = if b.fine() { (5, 4) } else { (3, 2) };
    pale(b);
    // Hooked in section: a top plate running out, a lip turned down at its outer edge.
    let great = outline(&[(9.5, 31.0), (19.0, 29.6), (20.2, 25.2), (22.6, 24.6), (23.6, 30.4), (21.8, 33.0), (9.5, 34.6)]);
    arc(b, &great, m - deg(26.0), m + deg(26.0), great_steps, 3.0);
    let small = outline(&[(5.5, 41.0), (11.2, 40.0), (11.9, 37.6), (13.4, 37.3), (14.0, 40.6), (12.8, 42.3), (5.5, 44.2)]);
    arc(b, &small, m - deg(22.0), m + deg(22.0), small_steps, 1.6);
    if b.fine() {
        let strip = section(b, [(23.62, 27.0), (23.9, 27.0), (23.9, 28.4), (23.62, 28.4)], 0.0);
        arc_light(b, &strip, m, deg(26.0), 0.8, great_steps, 3.0);
    }
}

/// A claw arm hanging from under a great arc, facing +x: shoulder, upper arm, elbow,
/// forearm with its light, and the talons.
fn claw_arm(b: &mut MeshBuilder) {
    let sides = if b.fine() { 8 } else { 4 };
    let (shoulder, elbow, wrist) = (v3(15.8, 0.0, 28.3), v3(21.0, 0.0, 19.6), v3(19.6, 0.0, 11.0));
    dark(b);
    b.cylinder_between(shoulder - Vec3::Y * 1.4, shoulder + Vec3::Y * 1.4, 1.6, 1.6, sides);
    b.cylinder_between(elbow - Vec3::Y * 1.2, elbow + Vec3::Y * 1.2, 1.4, 1.4, sides);
    pale(b);
    bar(b, v3(16.2, 0.0, 27.4), v3(20.6, 0.0, 20.6), v2(2.6, 2.4), v2(2.3, 2.2));
    let fore = v3(21.0, 0.0, 18.6);
    bar(b, fore, wrist, v2(3.4, 3.0), v2(2.6, 2.4));
    bar(b, wrist, v3(17.2, 0.0, 5.2), v2(2.4, 2.2), v2(0.25, 0.25));
    if b.fine() {
        bar(b, wrist + v3(0.3, 0.0, 0.5), v3(22.4, 0.0, 6.8), v2(1.4, 1.4), v2(0.2, 0.2));
        // The light down the forearm's outer face.
        light(b);
        let out = (wrist - fore).cross(Vec3::Y).normalize();
        let out = if out.x < 0.0 { -out } else { out };
        b.beam(fore.lerp(wrist, 0.12) + out * 1.45, fore.lerp(wrist, 0.85) + out * 1.2, v2(0.7, 0.25), v2(0.6, 0.25));
    }
}

/// A few solids: base point, body and crown, spine, the catch.
fn node_coarse(b: &mut MeshBuilder) {
    pale(b);
    b.frustum(v3(0.0, 0.0, 3.0), v2(1.0, 1.0), v2(10.0, 10.0), 7.0, Vec2::ZERO);
    b.yawed(Vec3::ZERO, TAU / 8.0, |b| {
        b.frustum(v3(0.0, 0.0, 12.0), v2(8.0, 8.0), v2(40.0, 40.0), 22.0, Vec2::ZERO);
    });
    dark(b);
    b.frustum(v3(0.0, 0.0, 34.0), v2(6.0, 6.0), v2(1.0, 1.0), 20.0, Vec2::ZERO);
    light(b);
    b.spheroid(Vec3::from(NODE_RAY_CATCH), v3(3.0, 3.0, 4.0), 4, 2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model_scaled, Model};

    fn tops(m: &Model, lod: usize) -> (f32, f32, f32) {
        let v = &m.lods[lod].vertices;
        let top = v.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
        let x = v.iter().map(|v| v.pos[0].abs()).fold(0.0, f32::max);
        let y = v.iter().map(|v| v.pos[1].abs()).fold(0.0, f32::max);
        (top, x, y)
    }

    fn nearest(m: &Model, lod: usize, p: Vec3, material: u32) -> f32 {
        let mesh = &m.lods[lod];
        mesh.indices
            .chunks(3)
            .filter(|t| mesh.vertices[t[0] as usize].material == material)
            .flat_map(|t| t.iter().map(|&i| Vec3::from(mesh.vertices[i as usize].pos)))
            .map(|q| q.distance(p))
            .fold(f32::MAX, f32::min)
    }

    #[test]
    fn engine_keeps_the_sims_contract() {
        let m = build_model_scaled("replication_engine", 110.0, 150.0, 5).unwrap();
        for lod in 0..3 {
            let (top, x, y) = tops(&m, lod);
            assert!((120.0..=187.0).contains(&top), "lod{lod} top {top}");
            assert!(x <= 120.0 && y <= 120.0, "lod{lod} extent {x} x {y} outside the 20x20 lot");
            // Nothing of the turret past the muzzle.
            let over = m.lods[lod]
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET)
                .map(|v| v.pos[0] - LANCE_MUZZLE.x)
                .fold(f32::MIN, f32::max);
            assert!(over < 0.5, "lod{lod}: lance overshoots its muzzle by {over}");
        }
        assert!(Vec3::from(m.turret_pivot).truncate().length() < 1e-4);
        // The ray leaves from inside the crystal; each bay's head has its lens at the contract point.
        assert!(nearest(&m, 0, Vec3::from(ENGINE_RAY_EMITTER), GLOW_PRECURSOR) < 12.0);
        for k in 0..BAY_COUNT {
            let a = k as f32 * TAU / BAY_COUNT as f32;
            let head = Vec3::new(a.cos() * BAY_PROJECTOR_RADIUS, a.sin() * BAY_PROJECTOR_RADIUS, BAY_PROJECTOR_Z);
            assert!(nearest(&m, 0, head, GLOW_PRECURSOR) < 5.0, "bay {k}: no lens at the projector");
            // The printed unit's spot is clear of the model.
            let spot = Vec3::new(a.cos() * BAY_PRINT_RADIUS, a.sin() * BAY_PRINT_RADIUS, 0.0);
            assert!(m.lods[0].vertices.iter().all(|v| Vec3::from(v.pos).truncate().distance(spot.truncate()) > 25.0));
        }
        // The muzzle is on the Lance's bore.
        assert!(nearest(&m, 0, LANCE_MUZZLE, GLOW_PRECURSOR) < 1.5);
        let tris: Vec<usize> = m.lods.iter().map(|l| l.indices.len() / 3).collect();
        assert!(tris[2] < 60 && tris[1] as f32 <= tris[0] as f32 * 0.45 + 20.0, "{tris:?}");
        assert!(tris[0] <= ENGINE_TRIANGLES, "{tris:?}");
    }

    /// Software previews of both models: `MODEL_DUMP_DIR` (default target/model-dump).
    #[test]
    #[ignore = "writes inspection files"]
    fn dump_replicators() {
        let dir = std::path::PathBuf::from(std::env::var("MODEL_DUMP_DIR").unwrap_or_else(|_| "target/model-dump".into()));
        std::fs::create_dir_all(&dir).unwrap();
        for (key, r, h) in [("replication_engine", 110.0, 150.0), ("replication_node", 26.0, 46.0)] {
            let m = build_model_scaled(key, r, h, 3).unwrap();
            for az in [-38.0f32, 60.0, 142.0] {
                crate::models::preview::render(&m.lods[0], 768, az)
                    .write_ppm(&dir.join(format!("{key}_{az}.ppm")))
                    .unwrap();
            }
            for lod in 1..3 {
                crate::models::preview::render(&m.lods[lod], 384, -38.0)
                    .write_ppm(&dir.join(format!("{key}_lod{lod}.ppm")))
                    .unwrap();
            }
            println!("{key}: {:?}", m.lods.iter().map(|l| l.indices.len() / 3).collect::<Vec<_>>());
        }
    }

    /// The library-wide mesh checks, run on these two alone (the shared ones stop at the
    /// first model that fails anywhere in the roster).
    #[test]
    fn meshes_are_sound() {
        for (key, r, h) in [("replication_engine", 110.0, 150.0), ("replication_node", 26.0, 46.0)] {
            let m = build_model_scaled(key, r, h, 3).unwrap();
            for (lod, mesh) in m.lods.iter().enumerate() {
                for v in &mesh.vertices {
                    assert!(v.pos[2] >= -1e-3, "{key} lod{lod}: below ground");
                    assert!((Vec3::from(v.normal).length() - 1.0).abs() < 1e-4);
                    assert!(v.material <= LAST && [part::HULL, part::TURRET, part::SPINNER].contains(&v.part));
                    assert!(Vec3::from(v.pos).length() <= m.bounds_radius + 1e-3);
                }
                for t in mesh.indices.chunks(3) {
                    let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(mesh.vertices[i as usize].pos));
                    let g = (b - a).cross(c - a);
                    assert!(g.length() * 0.5 > 1e-7, "{key} lod{lod}: degenerate at {a}");
                    for &i in t {
                        let n = Vec3::from(mesh.vertices[i as usize].normal);
                        assert!(g.normalize().dot(n) > 0.5, "{key} lod{lod}: winding at {a}");
                    }
                }
            }
        }
    }

    #[test]
    fn node_keeps_the_sims_contract() {
        let m = build_model_scaled("replication_node", 26.0, 46.0, 3).unwrap();
        for lod in 0..3 {
            let (top, x, y) = tops(&m, lod);
            assert!((46.0 * 0.8..=46.0 * 1.25).contains(&top), "lod{lod} top {top}");
            assert!(x <= 30.0 && y <= 30.0 && x >= 16.0, "lod{lod} extent {x} x {y}");
        }
        assert!(nearest(&m, 0, Vec3::from(NODE_RAY_CATCH), GLOW_PRECURSOR) < 4.0);
        // The library's budgets for an ordinary model (`models::tests`), checked here too.
        let tris: Vec<usize> = m.lods.iter().map(|l| l.indices.len() / 3).collect();
        assert!(tris[0] <= 2600 && tris[2] < 60 && tris[1] as f32 <= tris[0] as f32 * 0.45 + 20.0, "{tris:?}");
        // It hovers: nothing within two metres of the ground.
        assert!(m.lods[0].vertices.iter().all(|v| v.pos[2] > 2.0));
        assert!(nearest(&m, 0, Vec3::from(NODE_PRINT_EMITTER), GLOW_PRECURSOR) < 3.5);
    }
}
