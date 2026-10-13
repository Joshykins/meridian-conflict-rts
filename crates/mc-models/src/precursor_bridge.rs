//! The Progenitor's hard-light bridge (`mc_map::PropKind::PrecursorBridge`), built to
//! `mc_map::landmark::HARDLIGHT_BRIDGE`: a road of hard light from the origin along +x
//! for the plan's span, the plan's half either side, its walking surface at the
//! origin's height (the map raises each landing to it). The map's deck
//! (`mc_map::deck`) is that road, so units cross on the surface drawn here and ships
//! sail under it. Nothing reaches down into the water: the light holds it.
//!
//! The road is dormant light: panes of dark glass (`PRECURSOR_INLAY`), stirring as a
//! survival facility wakes, framed by pale alloy, a girder along each edge. At each
//! landing a portal of two steles marks the way on.
//!
//! Variants for the design round (CLAUDE.md, section 9), by key:
//! * `precursor_bridge`: the flat span.
//! * `precursor_bridge~arch`: the span hung from a great arch over each edge.
//! * `precursor_bridge~float`: the span broken into floating segments, the gaps
//!   between them bare light, under three gateway rings.
//!
//! Model space as for every prop: x along the heading, y left, z up, the origin on
//! the ground (the near landing).

use glam::Vec3;

use super::builder::MeshBuilder;
use super::library::ModelDef;
use super::precursor::{dark, key_seam, light, pale, panel, v3};
use mc_map::landmark::HARDLIGHT_BRIDGE as PLAN;

pub(super) const MODELS: &[ModelDef] = &[
    ModelDef::new("precursor_bridge", RADIUS, ARCH_TOP, flat),
    ModelDef::new("precursor_bridge~arch", RADIUS, ARCH_TOP, arched),
    ModelDef::new("precursor_bridge~float", RADIUS, ARCH_TOP, floating),
];

/// The span and half its width, as the map lays the deck.
const SPAN: f32 = PLAN.span as f32;
const HALF: f32 = PLAN.half as f32;
/// The authored radius: the span's half diagonal.
const RADIUS: f32 = 850.0;
/// The highest anything stands over the road: the arch's crown.
const ARCH_TOP: f32 = 230.0;
/// The road's slab under its surface, and the keel under its middle.
const SLAB: f32 = 3.0;
const KEEL: (f32, f32) = (90.0, 14.0);
/// Each edge girder: its width, its top over the road and its foot under it.
const GIRDER: (f32, f32, f32) = (16.0, 5.0, 12.0);
/// The panes across the road between the girders, and their length along it.
const LANES: usize = 4;
const PANE: f32 = 100.0;
/// A rib between panes: its width and how proud of the road it stands.
const RIB: (f32, f32) = (2.4, 0.15);
/// A landing's stele: its foot half size, its top, how far in from the edge.
const STELE: (f32, f32) = (9.0, 64.0);

/// The road between the girders from `x0` to `x1`, as panes of dark glass framed
/// by pale ribs.
fn road(b: &mut MeshBuilder, x0: f32, x1: f32) {
    let inner = HALF - GIRDER.0;
    // The glass, its surface at the road's height.
    light(b);
    b.chamfered_box(
        v3((x0 + x1) * 0.5, 0.0, -SLAB * 0.5),
        v3(x1 - x0, inner * 2.0, SLAB),
        0.0,
    );
    if !b.fine() {
        return;
    }
    // The frame: ribs across every pane's length, and along between the lanes.
    pale(b);
    let up = Vec3::Z;
    let mut x = x0;
    while x <= x1 + 0.5 {
        let a = x.clamp(x0 + RIB.0 * 0.5, x1 - RIB.0 * 0.5);
        rib(b, v3(a, -inner, 0.0), v3(a, inner, 0.0), up);
        x += PANE;
    }
    for k in 1..LANES {
        let y = -inner + 2.0 * inner * k as f32 / LANES as f32;
        rib(b, v3(x0, y, 0.0), v3(x1, y, 0.0), up);
    }
}

/// A pale rib from `a` to `c`, lying on the face whose normal is `out`.
fn rib(b: &mut MeshBuilder, a: Vec3, c: Vec3, out: Vec3) {
    let side = (c - a).cross(out).normalize() * RIB.0 * 0.5;
    let quad = [a - side, c - side, c + side, a + side];
    panel(b, &quad, out, RIB.1, 0.0);
}

/// The girders along both edges, `[x0, x1]`, a working line of light along each
/// one's inner top edge; the keel of dark core under the road's middle.
fn girders(b: &mut MeshBuilder, x0: f32, x1: f32) {
    let (w, top, foot) = GIRDER;
    let mid = (x0 + x1) * 0.5;
    pale(b);
    for side in [-1.0f32, 1.0] {
        let y = side * (HALF - w * 0.5);
        b.chamfered_box(
            v3(mid, y, (top - foot) * 0.5),
            v3(x1 - x0, w, top + foot),
            if b.coarse() { 0.0 } else { 2.0 },
        );
        if b.fine() {
            let inner = side * (HALF - w - 0.2);
            key_seam(
                b,
                v3(x0 + 4.0, inner, top - 1.2),
                v3(x1 - 4.0, inner, top - 1.2),
                v3(0.0, -side, 0.0),
                1.2,
            );
        }
    }
    dark(b);
    b.chamfered_box(
        v3(mid, 0.0, -SLAB - KEEL.1 * 0.5),
        v3(x1 - x0, KEEL.0, KEEL.1),
        if b.coarse() { 0.0 } else { 6.0 },
    );
}

/// A landing's portal at `x`: a stele at each edge, tapering, a lens of live
/// light near its top facing along the road.
fn portal(b: &mut MeshBuilder, x: f32) {
    let (half, top) = STELE;
    for side in [-1.0f32, 1.0] {
        let y = side * (HALF - GIRDER.0 * 0.5);
        pale(b);
        b.prism(
            v3(x, y, -GIRDER.2),
            4,
            half * 1.41,
            half * 0.6,
            top + GIRDER.2,
        );
        if b.fine() {
            let face = if x < SPAN * 0.5 { 1.0 } else { -1.0 };
            key_seam(
                b,
                v3(x + face * half * 0.62, y, top * 0.55),
                v3(x + face * half * 0.45, y, top * 0.85),
                v3(face, 0.0, 0.0),
                2.0,
            );
        }
    }
}

/// A: the flat span.
fn flat(b: &mut MeshBuilder, _tech: u8) {
    road(b, 0.0, SPAN);
    girders(b, 0.0, SPAN);
    portal(b, 0.0);
    portal(b, SPAN);
}

/// The arch's height over the road at `x`: a shallow curve from the landings to its crown.
fn arch_z(x: f32) -> f32 {
    let t = x / SPAN * 2.0 - 1.0;
    GIRDER.1 + (ARCH_TOP - GIRDER.1) * (1.0 - t * t)
}

/// B: the span hung from an arch over each edge, hangers of dark glass down to the girders.
fn arched(b: &mut MeshBuilder, tech: u8) {
    flat(b, tech);
    let (pieces, sides) = match () {
        _ if b.fine() => (40, 8),
        _ if b.coarse() => (12, 4),
        _ => (16, 5),
    };
    for side in [-1.0f32, 1.0] {
        let y = side * (HALF - GIRDER.0 * 0.5);
        pale(b);
        for k in 0..pieces {
            let (x0, x1) = (
                SPAN * k as f32 / pieces as f32,
                SPAN * (k + 1) as f32 / pieces as f32,
            );
            b.cylinder_between(
                v3(x0, y, arch_z(x0)),
                v3(x1, y, arch_z(x1)),
                7.0,
                7.0,
                sides,
            );
        }
        if !b.fine() {
            continue;
        }
        light(b);
        let mut x = PANE;
        while x < SPAN - 1.0 {
            b.cylinder_between(v3(x, y, GIRDER.1), v3(x, y, arch_z(x) - 4.0), 1.2, 1.2, 6);
            x += PANE;
        }
    }
}

/// The floating span's segments: how many, and the gap of bare light between two.
const SEGMENTS: usize = 8;
const GAP: f32 = 18.0;

/// C: the span broken into floating segments, the road bare light across the gaps;
/// a gateway half-ring of alloy over the road at each third.
fn floating(b: &mut MeshBuilder, _tech: u8) {
    let seg = (SPAN - GAP * (SEGMENTS - 1) as f32) / SEGMENTS as f32;
    // (Far off, one span.)
    let segments = if b.coarse() { 0 } else { SEGMENTS };
    if b.coarse() {
        road(b, 0.0, SPAN);
        girders(b, 0.0, SPAN);
    }
    for k in 0..segments {
        let x0 = k as f32 * (seg + GAP);
        road(b, x0, x0 + seg);
        girders(b, x0, x0 + seg);
        if k + 1 < SEGMENTS {
            // The gap: glass alone, a hair lower, its edges lit.
            light(b);
            b.chamfered_box(
                v3(x0 + seg + GAP * 0.5, 0.0, -SLAB * 0.5 - 0.1),
                v3(GAP + 2.0, (HALF - GIRDER.0) * 2.0, SLAB),
                0.0,
            );
        }
    }
    portal(b, 0.0);
    portal(b, SPAN);
    let (pieces, sides) = match () {
        _ if b.fine() => (28, 8),
        _ if b.coarse() => (10, 4),
        _ => (12, 5),
    };
    let reach = HALF - GIRDER.0 * 0.5;
    for third in [1.0f32, 2.0] {
        let x = SPAN * third / 3.0;
        pale(b);
        for k in 0..pieces {
            let a = |i: usize| std::f32::consts::PI * i as f32 / pieces as f32;
            let at = |t: f32| v3(x, reach * t.cos(), GIRDER.1 + reach * 0.55 * t.sin());
            b.cylinder_between(at(a(k)), at(a(k + 1)), 9.0, 9.0, sides);
        }
    }
}
