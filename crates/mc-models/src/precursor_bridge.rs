//! The Progenitor's hard-light bridge (`mc_map::PropKind::PrecursorBridge`), built to
//! `mc_map::landmark::HARDLIGHT_BRIDGE`: a road of hard light from the origin along +x
//! for the plan's span, the plan's half either side, its walking surface at the
//! origin's height (the map raises each landing to it). The map's deck
//! (`mc_map::deck`) is that road, so units cross on it and ships sail under it.
//! Nothing reaches down into the water: the light holds it.
//!
//! The road itself is not modelled: it is hard light, drawn by the renderer over the
//! map's deck (`progenitor_fx`'s deck pieces, progenitor_fx.wgsl), clear enough to see
//! the sea through. Here is what holds it: pale alloy, a girder along each edge, a
//! flat span with nothing over it (the user's pick A of three, 2026-10-13), and at
//! each landing a portal of two steles marking the way on.
//!
//! Model space as for every prop: x along the heading, y left, z up, the origin on
//! the ground (the near landing).

use super::builder::MeshBuilder;
use super::library::ModelDef;
use super::precursor::{key_seam, pale, v3};
use mc_map::landmark::HARDLIGHT_BRIDGE as PLAN;

pub(super) const MODELS: &[ModelDef] =
    &[ModelDef::new("precursor_bridge", RADIUS, STELE.1, bridge)];

/// The span and half its width, as the map lays the deck.
const SPAN: f32 = PLAN.span as f32;
const HALF: f32 = PLAN.half as f32;
/// The authored radius: the span's half diagonal.
const RADIUS: f32 = 850.0;
/// Each edge girder: its width, its top over the road and its foot under it.
const GIRDER: (f32, f32, f32) = (PLAN.girder as f32, 5.0, 12.0);
/// A landing's stele: its foot half size, its top, how far in from the edge.
const STELE: (f32, f32) = (9.0, 64.0);

/// The girders along both edges, `[x0, x1]`, a working line of light along each
/// one's inner top edge.
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

/// The span's girders, and a portal at each landing.
fn bridge(b: &mut MeshBuilder, _tech: u8) {
    girders(b, 0.0, SPAN);
    portal(b, 0.0);
    portal(b, SPAN);
}
